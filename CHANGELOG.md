# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/),
and this project adheres to [Semantic Versioning](https://semver.org/).

## [0.1.0] - Unreleased

### Added

- CLI-first `rrulex` binary with subcommands:
  - `expand`
  - `lint`
  - `explain`
- Core library APIs for:
  - RFC5545 recurrence expansion via `rrule` crate
  - lint findings for common RRULE footguns
  - explain output for include/exclude debugging
- Minimal ICS parser support for:
  - `DTSTART`
  - `RRULE`
  - `RDATE`
  - `EXRULE`
  - `EXDATE`
  - `TZID`
- Deterministic JSON output:
  - canonical object-key ordering
  - stable occurrence sorting
- Safety behavior and exit code contracts:
  - unbounded expansion protection
  - hard limit handling
- Fixture + golden test suite with 38 CLI cases, including:
  - weekly/monthly/yearly rule patterns
  - DST boundaries for `Europe/Berlin`
  - `UNTIL`/`DTSTART` mismatch lint coverage
  - RDATE/EXDATE/EXRULE explainability cases
- Cross-platform CI checks retained (Linux/macOS/Windows)
