# Contributing

## Workflow

1. Create a branch
2. Make changes
3. Run local checks:
   ```sh
   cargo fmt --all
   cargo clippy --all-targets --all-features -- -D warnings
   cargo test --all
   ```
4. Commit and open a pull request

## Fixtures & Golden Files

`rrulex` integration tests are fixture-driven.

- Fixture cases live in `fixtures/cases/*.json`
- ICS inputs live in `fixtures/ics/*.ics`
- Golden snapshots live in `golden/cases/*`

Case file shape:

```json
{
  "args": ["expand", "--dtstart", "..."],
  "expected_exit": 0,
  "golden": "expand_example.json"
}
```

Error-case fixtures omit `golden` and assert stderr substring:

```json
{
  "args": ["expand", "..."],
  "expected_exit": 3,
  "stderr_contains": "hard limit exceeded"
}
```

Refresh snapshots:

```sh
UPDATE_GOLDEN=1 cargo test -p rrulex --test golden_tests
```
