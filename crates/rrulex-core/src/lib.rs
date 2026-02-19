use chrono::{DateTime, NaiveDate, NaiveDateTime, TimeZone, Utc};
use chrono_tz::Tz as ChronoTz;
use rrule::{RRule, RRuleSet, Tz, Unvalidated};
use serde::Serialize;
use std::collections::{BTreeMap, HashMap};
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DateValueType {
    Date,
    DateTime,
}

#[derive(Debug, Clone)]
pub struct RecurrenceSpec {
    pub dtstart: DateTime<Tz>,
    pub dtstart_type: DateValueType,
    pub tz: String,
    pub rrules: Vec<String>,
    pub rdates: Vec<DateTime<Tz>>,
    pub exrules: Vec<String>,
    pub exdates: Vec<DateTime<Tz>>,
}

#[derive(Debug, Clone)]
pub enum ExpandQuery {
    Between {
        start: DateTime<Tz>,
        end: DateTime<Tz>,
    },
    After {
        start: DateTime<Tz>,
        count: usize,
    },
    Unbounded,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "UPPERCASE")]
pub enum OccurrenceSource {
    Rrule,
    Rdate,
}

#[derive(Debug, Clone, Serialize)]
pub struct Occurrence {
    pub start_local: String,
    pub start_utc: String,
    pub tz: String,
    pub source: OccurrenceSource,
    pub rule_index: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct RulesMeta {
    pub rrule: Vec<String>,
    pub rdate: Vec<String>,
    pub exrule: Vec<String>,
    pub exdate: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct WindowMeta {
    pub start: Option<String>,
    pub end: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ExpandMeta {
    pub dtstart: String,
    pub tz: String,
    pub rules: RulesMeta,
    pub window: WindowMeta,
    pub limit: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct ExpandResult {
    pub meta: ExpandMeta,
    pub occurrences: Vec<Occurrence>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Finding {
    pub code: String,
    pub message: String,
    pub details: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct Findings {
    pub errors: Vec<Finding>,
    pub warnings: Vec<Finding>,
    pub hints: Vec<Finding>,
}

impl Findings {
    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ExplainResult {
    pub at: String,
    pub included: bool,
    pub generated_by: Option<OccurrenceSource>,
    pub generated_rule_index: Option<usize>,
    pub excluded_by: Option<String>,
    pub notes: Vec<String>,
}

#[derive(Debug, Error)]
pub enum CoreError {
    #[error("unknown timezone '{0}'")]
    InvalidTimezone(String),

    #[error("invalid datetime '{input}': {reason}")]
    InvalidDateTime { input: String, reason: String },

    #[error("invalid RRULE '{rule}': {reason}")]
    InvalidRrule { rule: String, reason: String },

    #[error("missing required field: {0}")]
    MissingField(String),

    #[error("invalid ICS input: {0}")]
    InvalidIcs(String),

    #[error("hard limit exceeded ({limit}). Use a smaller window or a higher --limit")]
    LimitExceeded { limit: usize },

    #[error("invalid limit '{0}': limit must be > 0")]
    InvalidLimit(usize),

    #[error("invalid count '{0}': count must be > 0")]
    InvalidCount(usize),

    #[error("unbounded RRULE requires --between, --after/--count, or explicit --limit")]
    UnsafeUnboundedRule,
}

pub fn parse_timezone(value: &str) -> Result<Tz, CoreError> {
    value
        .parse::<ChronoTz>()
        .map(Tz::from)
        .map_err(|_| CoreError::InvalidTimezone(value.to_string()))
}

pub fn parse_iso_datetime(
    value: &str,
    tz: &Tz,
) -> Result<(DateTime<Tz>, DateValueType), CoreError> {
    if let Ok(date) = NaiveDate::parse_from_str(value, "%Y-%m-%d") {
        let local = date
            .and_hms_opt(0, 0, 0)
            .ok_or_else(|| CoreError::InvalidDateTime {
                input: value.to_string(),
                reason: "could not build midnight datetime".to_string(),
            })?;
        return localize(*tz, local, value).map(|dt| (dt, DateValueType::Date));
    }

    if let Ok(fixed) = DateTime::parse_from_rfc3339(value) {
        return Ok((fixed.with_timezone(tz), DateValueType::DateTime));
    }

    if let Ok(local) = NaiveDateTime::parse_from_str(value, "%Y-%m-%dT%H:%M:%S") {
        return localize(*tz, local, value).map(|dt| (dt, DateValueType::DateTime));
    }

    Err(CoreError::InvalidDateTime {
        input: value.to_string(),
        reason: "expected YYYY-MM-DD, YYYY-MM-DDTHH:MM:SS, or RFC3339".to_string(),
    })
}

pub fn parse_ics_spec(input: &str, fallback_tz: Option<&str>) -> Result<RecurrenceSpec, CoreError> {
    let lines = unfold_ics_lines(input);

    let mut dtstart: Option<DateTime<Tz>> = None;
    let mut dtstart_type = DateValueType::DateTime;
    let mut tz_name: Option<String> = fallback_tz.map(ToOwned::to_owned);
    let mut rrules = Vec::new();
    let mut rdates = Vec::new();
    let mut exrules = Vec::new();
    let mut exdates = Vec::new();

    for line in lines {
        let Some((head, raw_value)) = line.split_once(':') else {
            continue;
        };

        let value = raw_value.trim();
        let (name, params) = parse_property_head(head);

        match name.as_str() {
            "DTSTART" => {
                let value_type = match params.get("VALUE") {
                    Some(v) if v.eq_ignore_ascii_case("DATE") => DateValueType::Date,
                    _ => {
                        if value.len() == 8 && value.chars().all(|c| c.is_ascii_digit()) {
                            DateValueType::Date
                        } else {
                            DateValueType::DateTime
                        }
                    }
                };

                let tzid = params
                    .get("TZID")
                    .cloned()
                    .or_else(|| fallback_tz.map(ToOwned::to_owned));

                let tz = if value.ends_with('Z') {
                    Tz::UTC
                } else if let Some(ref tzid) = tzid {
                    parse_timezone(tzid)?
                } else {
                    return Err(CoreError::InvalidIcs(
                        "DTSTART without TZID must be UTC (..Z) or --tz must be provided"
                            .to_string(),
                    ));
                };

                let parsed = parse_ics_datetime_value(value, &tz, value_type)?;
                dtstart = Some(parsed);
                dtstart_type = value_type;
                tz_name = Some(tzid.unwrap_or_else(|| tz.name().to_string()));
            }
            "RRULE" => rrules.push(value.to_string()),
            "EXRULE" => exrules.push(value.to_string()),
            "RDATE" => {
                let tz = resolve_property_tz(&params, tz_name.as_deref())?;
                let value_type = parse_value_type_for_multi(&params, value);
                let parsed = parse_ics_multi_datetime_values(value, &tz, value_type)?;
                rdates.extend(parsed);
            }
            "EXDATE" => {
                let tz = resolve_property_tz(&params, tz_name.as_deref())?;
                let value_type = parse_value_type_for_multi(&params, value);
                let parsed = parse_ics_multi_datetime_values(value, &tz, value_type)?;
                exdates.extend(parsed);
            }
            _ => {}
        }
    }

    let dtstart = dtstart.ok_or_else(|| CoreError::MissingField("DTSTART".to_string()))?;
    let tz = tz_name.unwrap_or_else(|| dtstart.timezone().name().to_string());

    if rrules.is_empty() && rdates.is_empty() {
        return Err(CoreError::MissingField(
            "at least one RRULE or RDATE".to_string(),
        ));
    }

    Ok(RecurrenceSpec {
        dtstart,
        dtstart_type,
        tz,
        rrules,
        rdates,
        exrules,
        exdates,
    })
}

pub fn lint(spec: &RecurrenceSpec, has_between: bool, has_limit: bool) -> Findings {
    let mut out = Findings::default();

    for rule in spec.rrules.iter().chain(spec.exrules.iter()) {
        let fields = parse_rule_fields(rule);

        if let Some(until) = fields.get("UNTIL") {
            let until_is_date = until.len() == 8 && until.chars().all(|c| c.is_ascii_digit());
            let until_is_datetime = until.contains('T');

            if spec.dtstart_type == DateValueType::Date && until_is_datetime {
                out.errors.push(Finding {
                    code: "E001".to_string(),
                    message: "UNTIL value type must match DTSTART".to_string(),
                    details: Some(
                        "DTSTART is DATE but UNTIL is DATE-TIME. Use UNTIL=YYYYMMDD.".to_string(),
                    ),
                });
            }

            if spec.dtstart_type == DateValueType::DateTime && until_is_date {
                out.errors.push(Finding {
                    code: "E001".to_string(),
                    message: "UNTIL value type must match DTSTART".to_string(),
                    details: Some(
                        "DTSTART is DATE-TIME but UNTIL is DATE. Use UNTIL=YYYYMMDDTHHMMSS(Z)."
                            .to_string(),
                    ),
                });
            }

            if until_is_datetime && !until.ends_with('Z') {
                out.warnings.push(Finding {
                    code: "W001".to_string(),
                    message: "UNTIL appears as local/floating time".to_string(),
                    details: Some(
                        "Prefer UNTIL with 'Z' (UTC) to avoid timezone ambiguity across systems."
                            .to_string(),
                    ),
                });
            }
        }

        let has_count = fields.contains_key("COUNT");
        let has_until = fields.contains_key("UNTIL");
        if !has_count && !has_until && !has_between && !has_limit {
            out.warnings.push(Finding {
                code: "W002".to_string(),
                message: "Potentially unbounded rule".to_string(),
                details: Some(
                    "No COUNT/UNTIL and no --between/--limit context was provided.".to_string(),
                ),
            });
        }

        if fields.contains_key("BYSETPOS") {
            let has_context = [
                "BYMONTH",
                "BYWEEKNO",
                "BYYEARDAY",
                "BYMONTHDAY",
                "BYDAY",
                "BYHOUR",
                "BYMINUTE",
                "BYSECOND",
            ]
            .iter()
            .any(|key| fields.contains_key(*key));

            if !has_context {
                out.warnings.push(Finding {
                    code: "W003".to_string(),
                    message: "BYSETPOS without BYxxx context".to_string(),
                    details: Some(
                        "BYSETPOS is typically only meaningful with BYDAY/BYMONTHDAY/etc."
                            .to_string(),
                    ),
                });
            }
        }
    }

    out
}

pub fn is_potentially_unbounded(spec: &RecurrenceSpec) -> bool {
    spec.rrules
        .iter()
        .any(|rule| !rule_has_count_or_until(rule))
}

pub fn expand(
    spec: &RecurrenceSpec,
    query: &ExpandQuery,
    hard_limit: usize,
) -> Result<Vec<Occurrence>, CoreError> {
    if hard_limit == 0 {
        return Err(CoreError::InvalidLimit(hard_limit));
    }

    let tz = parse_timezone(&spec.tz)?;
    let (rrules, exrules) = parse_validated_rules(spec)?;

    let mut set = RRuleSet::new(spec.dtstart)
        .set_rrules(rrules.clone())
        .set_exrules(exrules.clone());
    for dt in &spec.rdates {
        set = set.rdate(*dt);
    }
    for dt in &spec.exdates {
        set = set.exdate(*dt);
    }

    let dates = collect_dates(set, query, hard_limit)?;

    let rdate_index: HashMap<i64, usize> = spec
        .rdates
        .iter()
        .enumerate()
        .map(|(i, dt)| (dt.timestamp(), i))
        .collect();

    let mut out = Vec::with_capacity(dates.len());
    for dt in dates {
        let local = dt.with_timezone(&tz);
        let ts = local.timestamp();

        let (source, rule_index) = if let Some(index) = rdate_index.get(&ts) {
            (OccurrenceSource::Rdate, *index)
        } else {
            let mut found = None;
            for (idx, rule) in rrules.iter().enumerate() {
                if matches_rule_at(spec.dtstart, rule, local) {
                    found = Some(idx);
                    break;
                }
            }
            (OccurrenceSource::Rrule, found.unwrap_or(0))
        };

        out.push(Occurrence {
            start_local: local.format("%Y-%m-%dT%H:%M:%S").to_string(),
            start_utc: local
                .with_timezone(&Utc)
                .format("%Y-%m-%dT%H:%M:%SZ")
                .to_string(),
            tz: spec.tz.clone(),
            source,
            rule_index,
        });
    }

    out.sort_by(|a, b| {
        a.start_utc
            .cmp(&b.start_utc)
            .then_with(|| a.start_local.cmp(&b.start_local))
            .then_with(|| a.rule_index.cmp(&b.rule_index))
    });

    Ok(out)
}

pub fn expand_result(
    spec: &RecurrenceSpec,
    query: &ExpandQuery,
    hard_limit: usize,
) -> Result<ExpandResult, CoreError> {
    let occurrences = expand(spec, query, hard_limit)?;

    let (window_start, window_end) = match query {
        ExpandQuery::Between { start, end } => (
            Some(start.format("%Y-%m-%dT%H:%M:%S").to_string()),
            Some(end.format("%Y-%m-%dT%H:%M:%S").to_string()),
        ),
        ExpandQuery::After { start, .. } => {
            (Some(start.format("%Y-%m-%dT%H:%M:%S").to_string()), None)
        }
        ExpandQuery::Unbounded => (None, None),
    };

    let meta = ExpandMeta {
        dtstart: spec.dtstart.format("%Y-%m-%dT%H:%M:%S").to_string(),
        tz: spec.tz.clone(),
        rules: RulesMeta {
            rrule: spec.rrules.clone(),
            rdate: spec
                .rdates
                .iter()
                .map(|d| d.format("%Y-%m-%dT%H:%M:%S").to_string())
                .collect(),
            exrule: spec.exrules.clone(),
            exdate: spec
                .exdates
                .iter()
                .map(|d| d.format("%Y-%m-%dT%H:%M:%S").to_string())
                .collect(),
        },
        window: WindowMeta {
            start: window_start,
            end: window_end,
        },
        limit: hard_limit,
    };

    Ok(ExpandResult { meta, occurrences })
}

pub fn explain(spec: &RecurrenceSpec, at: DateTime<Tz>) -> Result<ExplainResult, CoreError> {
    let tz = parse_timezone(&spec.tz)?;
    let at_local = at.with_timezone(&tz);
    let at_ts = at_local.timestamp();

    let (rrules, exrules) = parse_validated_rules(spec)?;

    let rdate_index: HashMap<i64, usize> = spec
        .rdates
        .iter()
        .enumerate()
        .map(|(i, dt)| (dt.timestamp(), i))
        .collect();

    let exdate_hit = spec.exdates.iter().any(|d| d.timestamp() == at_ts);

    let mut generated_by = None;
    let mut generated_rule_index = None;

    if let Some(idx) = rdate_index.get(&at_ts) {
        generated_by = Some(OccurrenceSource::Rdate);
        generated_rule_index = Some(*idx);
    } else {
        for (idx, rule) in rrules.iter().enumerate() {
            if matches_rule_at(spec.dtstart, rule, at_local) {
                generated_by = Some(OccurrenceSource::Rrule);
                generated_rule_index = Some(idx);
                break;
            }
        }
    }

    let exrule_hit = exrules
        .iter()
        .any(|rule| matches_exrule_at(spec.dtstart, rule, at_local));

    let excluded_by = if exdate_hit {
        Some("EXDATE".to_string())
    } else if exrule_hit {
        Some("EXRULE".to_string())
    } else {
        None
    };

    let included = generated_by.is_some() && excluded_by.is_none();

    let mut notes = Vec::new();
    if let Some(source) = &generated_by {
        match source {
            OccurrenceSource::Rrule => notes.push("Generated by RRULE".to_string()),
            OccurrenceSource::Rdate => notes.push("Generated by RDATE".to_string()),
        }
    } else {
        notes.push("Not generated by RRULE/RDATE".to_string());
    }

    if let Some(excluded) = &excluded_by {
        notes.push(format!("Excluded by {excluded}"));
    }

    Ok(ExplainResult {
        at: at_local.format("%Y-%m-%dT%H:%M:%S").to_string(),
        included,
        generated_by,
        generated_rule_index,
        excluded_by,
        notes,
    })
}

pub fn canonical_json(value: &serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Object(map) => {
            let mut ordered: BTreeMap<String, serde_json::Value> = BTreeMap::new();
            for (k, v) in map {
                ordered.insert(k.clone(), canonical_json(v));
            }
            let mut out = serde_json::Map::new();
            for (k, v) in ordered {
                out.insert(k, v);
            }
            serde_json::Value::Object(out)
        }
        serde_json::Value::Array(values) => {
            serde_json::Value::Array(values.iter().map(canonical_json).collect())
        }
        _ => value.clone(),
    }
}

fn collect_dates(
    set: RRuleSet,
    query: &ExpandQuery,
    hard_limit: usize,
) -> Result<Vec<DateTime<Tz>>, CoreError> {
    let limit_plus_one = hard_limit
        .checked_add(1)
        .ok_or(CoreError::LimitExceeded { limit: hard_limit })?;
    let limit_u16 = u16::try_from(limit_plus_one)
        .map_err(|_| CoreError::LimitExceeded { limit: hard_limit })?;

    match query {
        ExpandQuery::Between { start, end } => {
            let result = set.after(*start).before(*end).all(limit_u16);
            if result.dates.len() > hard_limit {
                return Err(CoreError::LimitExceeded { limit: hard_limit });
            }
            Ok(result.dates)
        }
        ExpandQuery::After { start, count } => {
            if *count == 0 {
                return Err(CoreError::InvalidCount(*count));
            }
            if *count > hard_limit {
                return Err(CoreError::LimitExceeded { limit: hard_limit });
            }
            let count_u16 = u16::try_from(*count)
                .map_err(|_| CoreError::LimitExceeded { limit: hard_limit })?;
            let result = set.after(*start).all(count_u16);
            Ok(result.dates)
        }
        ExpandQuery::Unbounded => {
            let hard_limit_u16 = u16::try_from(hard_limit)
                .map_err(|_| CoreError::LimitExceeded { limit: hard_limit })?;
            let result = set.all(hard_limit_u16);
            Ok(result.dates)
        }
    }
}

fn parse_validated_rules(spec: &RecurrenceSpec) -> Result<(Vec<RRule>, Vec<RRule>), CoreError> {
    let mut rrules = Vec::with_capacity(spec.rrules.len());
    for raw in &spec.rrules {
        rrules.push(parse_validated_rule(raw, spec.dtstart)?);
    }

    let mut exrules = Vec::with_capacity(spec.exrules.len());
    for raw in &spec.exrules {
        exrules.push(parse_validated_rule(raw, spec.dtstart)?);
    }

    Ok((rrules, exrules))
}

fn parse_validated_rule(value: &str, dtstart: DateTime<Tz>) -> Result<RRule, CoreError> {
    let unvalidated: RRule<Unvalidated> =
        value
            .parse::<RRule<Unvalidated>>()
            .map_err(|err: rrule::RRuleError| CoreError::InvalidRrule {
                rule: value.to_string(),
                reason: err.to_string(),
            })?;

    unvalidated
        .validate(dtstart)
        .map_err(|err: rrule::RRuleError| CoreError::InvalidRrule {
            rule: value.to_string(),
            reason: err.to_string(),
        })
}

fn matches_rule_at(dtstart: DateTime<Tz>, rule: &RRule, at: DateTime<Tz>) -> bool {
    let result = RRuleSet::new(dtstart)
        .rrule(rule.clone())
        .after(at)
        .before(at)
        .all(1);
    !result.dates.is_empty()
}

fn matches_exrule_at(dtstart: DateTime<Tz>, rule: &RRule, at: DateTime<Tz>) -> bool {
    let result = RRuleSet::new(dtstart)
        .exrule(rule.clone())
        .after(at)
        .before(at)
        .all(1);
    !result.dates.is_empty()
}

fn parse_rule_fields(rule: &str) -> HashMap<String, String> {
    rule.split(';')
        .filter_map(|part| {
            let (k, v) = part.split_once('=')?;
            Some((k.trim().to_ascii_uppercase(), v.trim().to_string()))
        })
        .collect()
}

fn rule_has_count_or_until(rule: &str) -> bool {
    let fields = parse_rule_fields(rule);
    fields.contains_key("COUNT") || fields.contains_key("UNTIL")
}

fn unfold_ics_lines(input: &str) -> Vec<String> {
    let normalized = input.replace("\r\n", "\n").replace('\r', "\n");
    let mut lines: Vec<String> = Vec::new();

    for raw in normalized.lines() {
        if raw.starts_with(' ') || raw.starts_with('\t') {
            if let Some(last) = lines.last_mut() {
                last.push_str(raw.trim_start());
            }
        } else {
            lines.push(raw.to_string());
        }
    }

    lines
}

fn parse_property_head(head: &str) -> (String, HashMap<String, String>) {
    let mut parts = head.split(';');
    let name = parts
        .next()
        .map(|s| s.trim().to_ascii_uppercase())
        .unwrap_or_default();

    let mut params = HashMap::new();
    for part in parts {
        if let Some((k, v)) = part.split_once('=') {
            params.insert(k.trim().to_ascii_uppercase(), v.trim().to_string());
        }
    }

    (name, params)
}

fn resolve_property_tz(
    params: &HashMap<String, String>,
    fallback_tz: Option<&str>,
) -> Result<Tz, CoreError> {
    if let Some(tzid) = params.get("TZID") {
        return parse_timezone(tzid);
    }

    if let Some(tz) = fallback_tz {
        return parse_timezone(tz);
    }

    Ok(Tz::UTC)
}

fn parse_value_type_for_multi(params: &HashMap<String, String>, raw: &str) -> DateValueType {
    if params
        .get("VALUE")
        .is_some_and(|v| v.eq_ignore_ascii_case("DATE"))
    {
        return DateValueType::Date;
    }

    let first = raw.split(',').next().unwrap_or(raw);
    if first.len() == 8 && first.chars().all(|c| c.is_ascii_digit()) {
        DateValueType::Date
    } else {
        DateValueType::DateTime
    }
}

fn parse_ics_multi_datetime_values(
    raw: &str,
    tz: &Tz,
    value_type: DateValueType,
) -> Result<Vec<DateTime<Tz>>, CoreError> {
    raw.split(',')
        .map(|part| parse_ics_datetime_value(part.trim(), tz, value_type))
        .collect()
}

fn parse_ics_datetime_value(
    value: &str,
    tz: &Tz,
    value_type: DateValueType,
) -> Result<DateTime<Tz>, CoreError> {
    match value_type {
        DateValueType::Date => {
            let date = NaiveDate::parse_from_str(value, "%Y%m%d").map_err(|err| {
                CoreError::InvalidDateTime {
                    input: value.to_string(),
                    reason: err.to_string(),
                }
            })?;
            let local = date
                .and_hms_opt(0, 0, 0)
                .ok_or_else(|| CoreError::InvalidDateTime {
                    input: value.to_string(),
                    reason: "could not build midnight datetime".to_string(),
                })?;
            localize(*tz, local, value)
        }
        DateValueType::DateTime => {
            if let Some(stripped) = value.strip_suffix('Z') {
                let naive =
                    NaiveDateTime::parse_from_str(stripped, "%Y%m%dT%H%M%S").map_err(|err| {
                        CoreError::InvalidDateTime {
                            input: value.to_string(),
                            reason: err.to_string(),
                        }
                    })?;
                let utc_dt = Utc.from_utc_datetime(&naive);
                Ok(utc_dt.with_timezone(tz))
            } else {
                let local =
                    NaiveDateTime::parse_from_str(value, "%Y%m%dT%H%M%S").map_err(|err| {
                        CoreError::InvalidDateTime {
                            input: value.to_string(),
                            reason: err.to_string(),
                        }
                    })?;
                localize(*tz, local, value)
            }
        }
    }
}

fn localize(tz: Tz, local: NaiveDateTime, input: &str) -> Result<DateTime<Tz>, CoreError> {
    tz.from_local_datetime(&local)
        .single()
        .ok_or_else(|| CoreError::InvalidDateTime {
            input: input.to_string(),
            reason: "ambiguous or invalid local time in timezone".to_string(),
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn berlin() -> Tz {
        parse_timezone("Europe/Berlin").expect("timezone")
    }

    #[test]
    fn parses_iso_datetime() {
        let tz = berlin();
        let (dt, kind) = parse_iso_datetime("2026-03-01T10:00:00", &tz).expect("datetime");
        assert_eq!(kind, DateValueType::DateTime);
        assert_eq!(
            dt.format("%Y-%m-%dT%H:%M:%S").to_string(),
            "2026-03-01T10:00:00"
        );
    }

    #[test]
    fn expands_weekly_rule() {
        let tz = berlin();
        let dtstart = tz.with_ymd_and_hms(2026, 3, 2, 10, 0, 0).unwrap();
        let spec = RecurrenceSpec {
            dtstart,
            dtstart_type: DateValueType::DateTime,
            tz: "Europe/Berlin".to_string(),
            rrules: vec!["FREQ=WEEKLY;BYDAY=MO,WE;COUNT=4".to_string()],
            rdates: vec![],
            exrules: vec![],
            exdates: vec![],
        };

        let occ = expand(&spec, &ExpandQuery::Unbounded, 100).expect("expand");
        assert_eq!(occ.len(), 4);
        assert_eq!(occ[0].start_local, "2026-03-02T10:00:00");
        assert_eq!(occ[1].start_local, "2026-03-04T10:00:00");
    }

    #[test]
    fn lint_until_type_mismatch() {
        let tz = berlin();
        let dtstart = tz.with_ymd_and_hms(2026, 1, 1, 10, 0, 0).unwrap();
        let spec = RecurrenceSpec {
            dtstart,
            dtstart_type: DateValueType::DateTime,
            tz: "Europe/Berlin".to_string(),
            rrules: vec!["FREQ=DAILY;UNTIL=20260110".to_string()],
            rdates: vec![],
            exrules: vec![],
            exdates: vec![],
        };

        let findings = lint(&spec, false, false);
        assert_eq!(findings.errors.len(), 1);
        assert_eq!(findings.errors[0].code, "E001");
    }

    #[test]
    fn explains_exdate_exclusion() {
        let tz = berlin();
        let dtstart = tz.with_ymd_and_hms(2026, 3, 1, 10, 0, 0).unwrap();
        let blocked = tz.with_ymd_and_hms(2026, 3, 3, 10, 0, 0).unwrap();
        let spec = RecurrenceSpec {
            dtstart,
            dtstart_type: DateValueType::DateTime,
            tz: "Europe/Berlin".to_string(),
            rrules: vec!["FREQ=DAILY;COUNT=5".to_string()],
            rdates: vec![],
            exrules: vec![],
            exdates: vec![blocked],
        };

        let result = explain(&spec, blocked).expect("explain");
        assert!(!result.included);
        assert_eq!(result.excluded_by.as_deref(), Some("EXDATE"));
    }

    #[test]
    fn parses_minimal_ics() {
        let raw = "BEGIN:VCALENDAR\nBEGIN:VEVENT\nDTSTART;TZID=Europe/Berlin:20260301T100000\nRRULE:FREQ=WEEKLY;COUNT=2\nRDATE;TZID=Europe/Berlin:20260310T100000\nEND:VEVENT\nEND:VCALENDAR\n";
        let spec = parse_ics_spec(raw, None).expect("ics parse");
        assert_eq!(spec.tz, "Europe/Berlin");
        assert_eq!(spec.rrules.len(), 1);
        assert_eq!(spec.rdates.len(), 1);
    }
}
