# rrulex

Deterministic RFC 5545 recurrence tooling for CLI workflows.

`rrulex` expands RRULE-based schedules, lints common spec footguns, and explains inclusion/exclusion for a concrete datetime.

## Install

```sh
cargo install rrulex
```

## Commands

### `expand`

Expand occurrences from `DTSTART + RRULE (+ optional RDATE/EXDATE/EXRULE)`.

```sh
rrulex expand \
  --dtstart "2026-03-01T10:00:00" \
  --tz "Europe/Berlin" \
  --rrule "FREQ=WEEKLY;BYDAY=MO,WE;COUNT=10" \
  --format json
```

From ICS:

```sh
rrulex expand --ics ./fixtures/ics/basic_weekly.ics --format json
```

Windowed query:

```sh
rrulex expand \
  --dtstart "2026-03-01T10:00:00" \
  --tz "Europe/Berlin" \
  --rrule "FREQ=DAILY;COUNT=100" \
  --between "2026-03-01T00:00:00" "2026-03-31T23:59:59" \
  --limit 1000 \
  --format json
```

### `lint`

Lint RRULE specs without expansion.

```sh
rrulex lint \
  --dtstart "2026-03-01T10:00:00" \
  --tz "Europe/Berlin" \
  --rrule "FREQ=DAILY;UNTIL=20260310" \
  --format json
```

Current rule set (v0.1):
- `E001`: `UNTIL` value type must match `DTSTART` (DATE vs DATE-TIME)
- `W001`: `UNTIL` as local/floating time (no `Z`)
- `W002`: potentially unbounded rule in lint context without window/limit
- `W003`: suspicious `BYSETPOS` usage without BYxxx context

### `explain`

Explain whether a datetime is included/excluded and by which ruleset component.

```sh
rrulex explain \
  --at "2026-03-11T10:00:00" \
  --dtstart "2026-03-01T10:00:00" \
  --tz "Europe/Berlin" \
  --rrule "FREQ=DAILY;COUNT=20" \
  --exdate "2026-03-11T10:00:00" \
  --format json
```

## Input Modes

Exactly one of:
- `--ics <path>`
- Direct flags: `--dtstart <iso> --tz <iana> --rrule <string> ...`

Direct mode supports repeatable:
- `--rrule`
- `--rdate`
- `--exrule`
- `--exdate`

## Deterministic JSON Contract

`expand --format json` returns:

```json
{
  "meta": {
    "dtstart": "...",
    "tz": "Europe/Berlin",
    "rules": {
      "rrule": ["..."],
      "rdate": ["..."],
      "exrule": ["..."],
      "exdate": ["..."]
    },
    "window": { "start": "...", "end": "..." },
    "limit": 1000
  },
  "occurrences": [
    {
      "start_local": "2026-03-02T10:00:00",
      "start_utc": "2026-03-02T09:00:00Z",
      "tz": "Europe/Berlin",
      "source": "RRULE",
      "rule_index": 0
    }
  ]
}
```

Determinism guarantees:
- occurrences are sorted consistently
- stable key ordering via canonical JSON helper
- stable array ordering

## Exit Codes

- `0`: success
- `2`: input/validation errors
- `3`: safety errors (limit exceeded, unsafe unbounded expansion)

## Development

```sh
cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all
```

Golden refresh:

```sh
UPDATE_GOLDEN=1 cargo test -p rrulex --test golden_tests
```

## Test Fixtures

- `fixtures/cases/`: declarative CLI fixture cases (38 cases in v0.1)
- `fixtures/ics/`: minimal ICS inputs
- `golden/cases/`: expected stdout snapshots

Includes mandatory scenarios:
- weekly MO/WE with COUNT
- monthly first Friday (`BYDAY=1FR`)
- yearly `BYMONTH + BYDAY`
- DST boundaries (`Europe/Berlin`, spring/fall)
- `UNTIL`/`DTSTART` mismatch linting
- RDATE/EXDATE include/exclude behavior
