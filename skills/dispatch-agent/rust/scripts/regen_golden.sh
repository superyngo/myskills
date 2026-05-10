#!/usr/bin/env bash
# Regenerate golden test outputs
# Usage: ./scripts/regen_golden.sh

set -euo pipefail

# Must run from crate root
if [ ! -f Cargo.toml ]; then
  echo "Error: run from the crate root (rust/ directory)" >&2
  exit 1
fi

echo "Building release binary..."
cargo build --release --quiet

BINARY="./target/release/dispatch-agent"

# Generate detect golden output
echo "Generating detect golden output..."
DISPATCH_AGENT_TEMPLATES="tests/fixtures/inputs/fake-detect-templates.toml" \
  "$BINARY" detect > tests/fixtures/golden/detect_output.json

# Generate init golden output
echo "Generating init golden output..."

# Run init with canonical input
OUTPUT_PATH=$("$BINARY" init < tests/fixtures/inputs/init_canonical.json 2>/dev/null)
# Read the generated TOML and save to golden
cat "$OUTPUT_PATH" > tests/fixtures/golden/init_output.toml
rm -f "$OUTPUT_PATH"

echo "Golden files regenerated successfully."
