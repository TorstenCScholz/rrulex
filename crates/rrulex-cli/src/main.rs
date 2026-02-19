use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::{Context, Result, anyhow, bail};
use clap::{ArgAction, Args, Parser, Subcommand, ValueEnum};
use rrulex_core::{
    CoreError, DateValueType, ExpandQuery, ExplainResult, Findings, RecurrenceSpec, canonical_json,
    expand_result, explain, is_potentially_unbounded, lint, parse_ics_spec, parse_iso_datetime,
    parse_timezone,
};

#[derive(Parser, Debug)]
#[command(version, about = "RFC5545 RRULE Expander + Linter + Explain")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Expand occurrences for a recurrence specification.
    Expand(ExpandArgs),
    /// Lint recurrence rules for common footguns.
    Lint(LintArgs),
    /// Explain why a concrete datetime is included/excluded.
    Explain(ExplainArgs),
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum OutputFormat {
    Json,
    Text,
}

#[derive(Args, Debug, Clone)]
struct InputArgs {
    /// iCalendar input file (minimal parser for DTSTART/RRULE/RDATE/EXDATE/EXRULE)
    #[arg(long)]
    ics: Option<PathBuf>,

    /// DTSTART as ISO datetime/date
    #[arg(long)]
    dtstart: Option<String>,

    /// IANA timezone (e.g. Europe/Berlin)
    #[arg(long)]
    tz: Option<String>,

    /// RRULE strings (repeatable)
    #[arg(long, action = ArgAction::Append)]
    rrule: Vec<String>,

    /// RDATE values (repeatable)
    #[arg(long, action = ArgAction::Append)]
    rdate: Vec<String>,

    /// EXRULE strings (repeatable)
    #[arg(long, action = ArgAction::Append)]
    exrule: Vec<String>,

    /// EXDATE values (repeatable)
    #[arg(long, action = ArgAction::Append)]
    exdate: Vec<String>,
}

#[derive(Args, Debug)]
struct ExpandArgs {
    #[command(flatten)]
    input: InputArgs,

    /// Window [start end] inclusive
    #[arg(long, num_args = 2, value_names = ["START", "END"])]
    between: Option<Vec<String>>,

    /// Start datetime for after/count query
    #[arg(long)]
    after: Option<String>,

    /// Number of occurrences to return with --after
    #[arg(long)]
    count: Option<usize>,

    /// Hard safety limit (default: 1000)
    #[arg(long)]
    limit: Option<usize>,

    #[arg(long, default_value = "json")]
    format: OutputFormat,
}

#[derive(Args, Debug)]
struct LintArgs {
    #[command(flatten)]
    input: InputArgs,

    /// Optional context window [start end] for unbounded warning suppression
    #[arg(long, num_args = 2, value_names = ["START", "END"])]
    between: Option<Vec<String>>,

    /// Optional context limit for unbounded warning suppression
    #[arg(long)]
    limit: Option<usize>,

    #[arg(long, default_value = "json")]
    format: OutputFormat,
}

#[derive(Args, Debug)]
struct ExplainArgs {
    #[command(flatten)]
    input: InputArgs,

    /// Datetime to explain
    #[arg(long)]
    at: String,

    #[arg(long, default_value = "json")]
    format: OutputFormat,
}

fn run() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Expand(args) => run_expand(args),
        Commands::Lint(args) => run_lint(args),
        Commands::Explain(args) => run_explain(args),
    }
}

fn run_expand(args: ExpandArgs) -> Result<()> {
    let spec = build_spec(&args.input)?;
    let hard_limit = args.limit.unwrap_or(1000);

    if hard_limit == 0 {
        return Err(anyhow!(CoreError::InvalidLimit(hard_limit)));
    }

    let query = build_query(
        &spec,
        args.between.as_ref(),
        args.after.as_deref(),
        args.count,
    )?;

    if matches!(query, ExpandQuery::Unbounded)
        && is_potentially_unbounded(&spec)
        && args.limit.is_none()
    {
        return Err(anyhow!(CoreError::UnsafeUnboundedRule));
    }

    let result = expand_result(&spec, &query, hard_limit)?;

    match args.format {
        OutputFormat::Json => print_json(&result)?,
        OutputFormat::Text => print_expand_text(&result.occurrences),
    }

    Ok(())
}

fn run_lint(args: LintArgs) -> Result<()> {
    let spec = build_spec(&args.input)?;
    let findings = lint(&spec, args.between.is_some(), args.limit.is_some());

    match args.format {
        OutputFormat::Json => print_json(&findings)?,
        OutputFormat::Text => print_lint_text(&findings),
    }

    Ok(())
}

fn run_explain(args: ExplainArgs) -> Result<()> {
    let spec = build_spec(&args.input)?;
    let tz = parse_timezone(&spec.tz)?;
    let (at, _) = parse_iso_datetime(&args.at, &tz)?;

    let result = explain(&spec, at)?;

    match args.format {
        OutputFormat::Json => print_json(&result)?,
        OutputFormat::Text => print_explain_text(&result),
    }

    Ok(())
}

fn build_query(
    spec: &RecurrenceSpec,
    between: Option<&Vec<String>>,
    after: Option<&str>,
    count: Option<usize>,
) -> Result<ExpandQuery> {
    let has_between = between.is_some();
    let has_after = after.is_some();
    let has_count = count.is_some();

    if has_between && (has_after || has_count) {
        bail!("--between cannot be combined with --after/--count");
    }

    if has_after ^ has_count {
        bail!("--after and --count must be provided together");
    }

    let tz = parse_timezone(&spec.tz)?;

    if let Some(values) = between {
        let (start, _) = parse_iso_datetime(&values[0], &tz)?;
        let (end, _) = parse_iso_datetime(&values[1], &tz)?;
        if start > end {
            bail!("--between start must be <= end");
        }
        return Ok(ExpandQuery::Between { start, end });
    }

    if let (Some(after), Some(count)) = (after, count) {
        if count == 0 {
            return Err(anyhow!(CoreError::InvalidCount(count)));
        }
        let (start, _) = parse_iso_datetime(after, &tz)?;
        return Ok(ExpandQuery::After { start, count });
    }

    Ok(ExpandQuery::Unbounded)
}

fn build_spec(input: &InputArgs) -> Result<RecurrenceSpec> {
    if let Some(path) = &input.ics {
        reject_extra_direct_flags(input)?;
        let raw = fs::read_to_string(path)
            .with_context(|| format!("failed to read ICS file {}", path.display()))?;
        return parse_ics_spec(&raw, input.tz.as_deref()).map_err(Into::into);
    }

    let dtstart_raw = input
        .dtstart
        .as_deref()
        .ok_or_else(|| anyhow!("--dtstart is required when --ics is not used"))?;
    let tz_raw = input
        .tz
        .as_deref()
        .ok_or_else(|| anyhow!("--tz is required when --ics is not used"))?;

    if input.rrule.is_empty() {
        bail!("at least one --rrule is required when --ics is not used");
    }

    let tz = parse_timezone(tz_raw)?;
    let (dtstart, dtstart_type) = parse_iso_datetime(dtstart_raw, &tz)?;

    let mut rdates = Vec::with_capacity(input.rdate.len());
    for raw in &input.rdate {
        let (dt, _kind) = parse_iso_datetime(raw, &tz)?;
        rdates.push(dt);
    }

    let mut exdates = Vec::with_capacity(input.exdate.len());
    for raw in &input.exdate {
        let (dt, _kind) = parse_iso_datetime(raw, &tz)?;
        exdates.push(dt);
    }

    Ok(RecurrenceSpec {
        dtstart,
        dtstart_type: match dtstart_type {
            DateValueType::Date => DateValueType::Date,
            DateValueType::DateTime => DateValueType::DateTime,
        },
        tz: tz_raw.to_string(),
        rrules: input.rrule.clone(),
        rdates,
        exrules: input.exrule.clone(),
        exdates,
    })
}

fn reject_extra_direct_flags(input: &InputArgs) -> Result<()> {
    if input.dtstart.is_some()
        || !input.rrule.is_empty()
        || !input.rdate.is_empty()
        || !input.exrule.is_empty()
        || !input.exdate.is_empty()
    {
        bail!("--ics cannot be combined with --dtstart/--rrule/--rdate/--exrule/--exdate");
    }
    Ok(())
}

fn print_json<T: serde::Serialize>(value: &T) -> Result<()> {
    let raw = serde_json::to_value(value)?;
    let canonical = canonical_json(&raw);
    println!("{}", serde_json::to_string_pretty(&canonical)?);
    Ok(())
}

fn print_expand_text(occurrences: &[rrulex_core::Occurrence]) {
    for occ in occurrences {
        println!(
            "{} ({}) {} idx={}",
            occ.start_local,
            occ.start_utc,
            match occ.source {
                rrulex_core::OccurrenceSource::Rrule => "RRULE",
                rrulex_core::OccurrenceSource::Rdate => "RDATE",
            },
            occ.rule_index
        );
    }
}

fn print_lint_text(findings: &Findings) {
    for finding in &findings.errors {
        println!("ERROR {}: {}", finding.code, finding.message);
    }
    for finding in &findings.warnings {
        println!("WARN {}: {}", finding.code, finding.message);
    }
    for finding in &findings.hints {
        println!("HINT {}: {}", finding.code, finding.message);
    }
}

fn print_explain_text(result: &ExplainResult) {
    println!("at: {}", result.at);
    println!("included: {}", result.included);
    if let Some(source) = &result.generated_by {
        println!(
            "generated_by: {}",
            match source {
                rrulex_core::OccurrenceSource::Rrule => "RRULE",
                rrulex_core::OccurrenceSource::Rdate => "RDATE",
            }
        );
    }
    if let Some(idx) = result.generated_rule_index {
        println!("generated_rule_index: {}", idx);
    }
    if let Some(excluded) = &result.excluded_by {
        println!("excluded_by: {excluded}");
    }
    for note in &result.notes {
        println!("note: {note}");
    }
}

fn exit_code_for_error(err: &anyhow::Error) -> u8 {
    if let Some(CoreError::LimitExceeded { .. } | CoreError::UnsafeUnboundedRule) =
        err.downcast_ref::<CoreError>()
    {
        3
    } else {
        2
    }
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("Error: {err:#}");
            ExitCode::from(exit_code_for_error(&err))
        }
    }
}
