# wtools-template

> A template repository for small, weird-but-useful Rust CLI tools.

[![CI](https://github.com/OWNER/REPO/actions/workflows/ci.yml/badge.svg)](https://github.com/OWNER/REPO/actions/workflows/ci.yml)

## Problem

_(Describe what problem this tool solves.)_

## Install

```sh
cargo install tool-cli
```

Or download a pre-built binary from [Releases](https://github.com/OWNER/REPO/releases).

## Usage

```sh
# Human-readable output
tool-cli file.txt

# JSON output
tool-cli --format json file.txt

# Multiple files
tool-cli --format json *.txt

# Verbose logging
tool-cli --verbose file.txt
```

### Example output (text)

```
--- file.txt ---
  Lines:            42
  Words:            300
  Characters:       1800
  Bytes:            1800
  Most common word: the
  Unique words:     150
```

### Example output (JSON)

```json
{
  "lines": 42,
  "words": 300,
  "chars": 1800,
  "bytes": 1800,
  "most_common_word": "the",
  "unique_words": 150
}
```

## Output Contract

JSON output fields (single file):

| Field               | Type            | Description                        |
|---------------------|-----------------|------------------------------------|
| `lines`             | `integer`       | Number of lines                    |
| `words`             | `integer`       | Number of whitespace-delimited words |
| `chars`             | `integer`       | Number of Unicode characters       |
| `bytes`             | `integer`       | Number of bytes                    |
| `most_common_word`  | `string\|null`  | Most frequent word (lowercased), `null` if no words |
| `unique_words`      | `integer`       | Number of distinct words (lowercased) |

When multiple files are passed, the output is an object keyed by file path.

## Using This Template

1. Clone/copy this repo
2. Run the rename script:
   ```sh
   ./scripts/rename_tool.sh my-tool "My awesome tool description" "https://github.com/user/my-tool"
   ```
3. Replace the `TextStats` logic in `crates/tool-core/src/lib.rs` with your own
4. Update fixtures and golden files:
   ```sh
   UPDATE_GOLDEN=1 cargo test
   ```

### Optional: cargo-dist

This template ships with a manual release workflow. To switch to [cargo-dist](https://opensource.axo.dev/cargo-dist/):

```sh
cargo install cargo-dist
cargo dist init
```

## Development

```sh
# Run tests
cargo test --all

# Update golden files after changing output
UPDATE_GOLDEN=1 cargo test

# Lint
cargo clippy --all-targets -- -D warnings

# Format
cargo fmt --all
```

## Limitations

_(List known limitations here.)_

## Roadmap

- [ ] Placeholder item

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or [MIT License](LICENSE-MIT) at your option.
