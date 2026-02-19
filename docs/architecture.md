# rrulex Architecture (v0.1)

## Core split

- `crates/rrulex-core`
  - `RecurrenceSpec`
  - `expand` / `expand_result`
  - `lint`
  - `explain`
  - minimal ICS parser (`DTSTART`, `RRULE`, `RDATE`, `EXRULE`, `EXDATE`, `TZID`)
  - canonical JSON helper
- `crates/rrulex-cli`
  - clap command surface (`expand`, `lint`, `explain`)
  - input validation and file IO
  - text/json rendering
  - exit code mapping

## RRULE engine decision

Chosen engine: [`rrule`](https://crates.io/crates/rrule) with `exrule` feature.

### Evaluation summary (`rrule` vs `rrules`)

- `rrule`
  - full `RRuleSet` model (`DTSTART`, `RRULE`, `RDATE`, `EXDATE`, `EXRULE`)
  - IANA timezone support through `chrono-tz`
  - practical RFC5545 alignment for CLI-grade interoperability
- `rrules`
  - lighter API and smaller feature surface
  - no equivalent `RRuleSet` feature depth for this project scope

Decision rationale:
- required sets (`RRULE + RDATE + EXDATE + EXRULE`) are first-class in `rrule`
- timezone/DST behavior is testable and deterministic for our fixtures
- integration complexity is lower for v0.1 goals

## Query model

`expand` supports:

- `--between <start> <end>`
- `--after <start> --count <n>`
- unbounded mode (guarded by safety checks)

Hard cap (`--limit`, default 1000) protects expansion volume.

## Determinism contract

- occurrence sorting is stable (`start_utc`, then tie-breakers)
- JSON object keys are canonicalized recursively
- arrays preserve deterministic insertion order

## Explain strategy

`explain --at <datetime>` computes:

- inclusion source (`RRULE` or `RDATE`)
- exclusion source (`EXDATE` or `EXRULE`)
- short notes for debugability

v0.1 does not attempt full BYxxx proof traces.

## Test strategy

Fixture-driven CLI snapshots:

- 38 fixture cases in `fixtures/cases/`
- golden outputs in `golden/cases/`
- required scenarios covered:
  - weekly MO/WE + COUNT
  - monthly first Friday (`BYDAY=1FR`)
  - yearly `BYMONTH + BYDAY`
  - DST spring/fall (`Europe/Berlin`)
  - `UNTIL` vs `DTSTART` type mismatch lint
  - RDATE/EXDATE inclusion/exclusion

Safety assertions included for:
- unbounded expansion without explicit constraints
- hard limit exceed behavior
