# Contributing

Thanks for your interest in contributing!

## Workflow

1. Fork the repository
2. Create a feature branch (`git checkout -b my-feature`)
3. Make your changes
4. Run checks locally:
   ```sh
   cargo fmt --all -- --check
   cargo clippy --all-targets -- -D warnings
   cargo test --all
   ```
5. Commit and push
6. Open a Pull Request

## Adding a Fixture

If you add a new text fixture to `fixtures/`:

1. Place your `.txt` file in `fixtures/`
2. Generate the golden file:
   ```sh
   UPDATE_GOLDEN=1 cargo test -p tool-cli --test golden_tests
   ```
3. Review the generated `golden/<name>.json` for correctness
4. Commit both the fixture and golden file

## Code Style

- Run `cargo fmt` before committing
- All clippy warnings must be resolved (`-D warnings`)
- Keep dependencies minimal
