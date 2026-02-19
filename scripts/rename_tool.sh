#!/usr/bin/env bash
set -euo pipefail

if [ $# -lt 1 ]; then
    echo "Usage: $0 <new-name> [description] [repository-url]"
    echo ""
    echo "Example: $0 my-tool \"A cool tool\" \"https://github.com/user/my-tool\""
    exit 1
fi

NEW_NAME="$1"
DESCRIPTION="${2:-A CLI tool}"
REPO_URL="${3:-https://github.com/OWNER/REPO}"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

echo "Renaming tool to: $NEW_NAME"
echo "Description: $DESCRIPTION"
echo "Repository: $REPO_URL"
echo ""

# Rename crate directories
if [ -d "$ROOT_DIR/crates/tool-core" ]; then
    mv "$ROOT_DIR/crates/tool-core" "$ROOT_DIR/crates/${NEW_NAME}-core"
fi
if [ -d "$ROOT_DIR/crates/tool-cli" ]; then
    mv "$ROOT_DIR/crates/tool-cli" "$ROOT_DIR/crates/${NEW_NAME}-cli"
fi

# Update workspace Cargo.toml
sed -i.bak \
    -e "s|\"crates/tool-core\"|\"crates/${NEW_NAME}-core\"|g" \
    -e "s|\"crates/tool-cli\"|\"crates/${NEW_NAME}-cli\"|g" \
    -e "s|https://github.com/OWNER/REPO|${REPO_URL}|g" \
    "$ROOT_DIR/Cargo.toml"
rm -f "$ROOT_DIR/Cargo.toml.bak"

# Update core crate Cargo.toml
sed -i.bak \
    -e "s|name = \"tool-core\"|name = \"${NEW_NAME}-core\"|g" \
    -e "s|Core analysis library for wtools-template|${DESCRIPTION} — core library|g" \
    "$ROOT_DIR/crates/${NEW_NAME}-core/Cargo.toml"
rm -f "$ROOT_DIR/crates/${NEW_NAME}-core/Cargo.toml.bak"

# Update CLI crate Cargo.toml
sed -i.bak \
    -e "s|name = \"tool-cli\"|name = \"${NEW_NAME}-cli\"|g" \
    -e "s|name = \"tool-cli\"|name = \"${NEW_NAME}\"|g" \
    -e "s|CLI for wtools-template|${DESCRIPTION}|g" \
    -e "s|tool-core|${NEW_NAME}-core|g" \
    "$ROOT_DIR/crates/${NEW_NAME}-cli/Cargo.toml"
rm -f "$ROOT_DIR/crates/${NEW_NAME}-cli/Cargo.toml.bak"

# Update binary name in [[bin]] section
sed -i.bak \
    -e "s|name = \"tool-cli\"|name = \"${NEW_NAME}\"|g" \
    "$ROOT_DIR/crates/${NEW_NAME}-cli/Cargo.toml"
rm -f "$ROOT_DIR/crates/${NEW_NAME}-cli/Cargo.toml.bak"

# Update use/extern crate in main.rs (tool_core -> new_name_core)
NEW_CRATE_NAME=$(echo "${NEW_NAME}-core" | tr '-' '_')
sed -i.bak \
    -e "s|tool_core|${NEW_CRATE_NAME}|g" \
    "$ROOT_DIR/crates/${NEW_NAME}-cli/src/main.rs"
rm -f "$ROOT_DIR/crates/${NEW_NAME}-cli/src/main.rs.bak"

# Update golden test binary reference
sed -i.bak \
    -e "s|CARGO_BIN_EXE_tool-cli|CARGO_BIN_EXE_${NEW_NAME}|g" \
    "$ROOT_DIR/crates/${NEW_NAME}-cli/tests/golden_tests.rs"
rm -f "$ROOT_DIR/crates/${NEW_NAME}-cli/tests/golden_tests.rs.bak"

# Update release workflow binary name
sed -i.bak \
    -e "s|tool-cli|${NEW_NAME}|g" \
    "$ROOT_DIR/.github/workflows/release.yml"
rm -f "$ROOT_DIR/.github/workflows/release.yml.bak"

# Update README badges and references
sed -i.bak \
    -e "s|https://github.com/OWNER/REPO|${REPO_URL}|g" \
    -e "s|wtools-template|${NEW_NAME}|g" \
    -e "s|tool-cli|${NEW_NAME}|g" \
    "$ROOT_DIR/README.md"
rm -f "$ROOT_DIR/README.md.bak"

echo "Done! Renamed to ${NEW_NAME}."
echo ""
echo "Next steps:"
echo "  1. Review the changes: git diff"
echo "  2. Update crates/*/src/ with your own logic"
echo "  3. Update fixtures and golden files: UPDATE_GOLDEN=1 cargo test"
echo "  4. Verify: cargo build && cargo test"
