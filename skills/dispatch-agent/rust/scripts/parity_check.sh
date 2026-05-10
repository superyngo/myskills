#!/usr/bin/env bash
# Compare Rust dispatch-agent output against the golden reference outputs
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR/.."  # go to crate root

if [ ! -f Cargo.toml ]; then
  echo "Error: must run from or locate crate root" >&2
  exit 1
fi

BINARY="./target/release/dispatch-agent"
if [ ! -f "$BINARY" ]; then
  echo "Binary not found — building..."
  cargo build --release --quiet
fi

PASS=0
FAIL=0

check() {
  local name="$1"
  local actual="$2"
  local golden="$3"
  # note: echo appends a newline; golden files are also newline-terminated
  if diff <(echo "$actual") <(cat "$golden") > /dev/null 2>&1; then
    echo "PASS: $name"
    PASS=$((PASS + 1))
  else
    echo "FAIL: $name"
    echo "--- Expected (golden):"
    cat "$golden"
    echo "--- Actual:"
    echo "$actual"
    echo "--- Diff:"
    diff <(cat "$golden") <(echo "$actual") || true
    FAIL=$((FAIL + 1))
  fi
}

# Check detect output
DETECT_OUT=$(DISPATCH_AGENT_TEMPLATES="tests/fixtures/inputs/fake-detect-templates.toml" \
  "$BINARY" detect 2>/dev/null)
check "detect JSON" "$DETECT_OUT" "tests/fixtures/golden/detect_output.json"

# Check init output
OUTPUT_PATH=$("$BINARY" init < tests/fixtures/inputs/init_canonical.json 2>/dev/null)
INIT_OUT=$(cat "$OUTPUT_PATH")
rm -f "$OUTPUT_PATH"
check "init TOML" "$INIT_OUT" "tests/fixtures/golden/init_output.toml"

echo ""
echo "Results: $PASS passed, $FAIL failed"
[ "$FAIL" -eq 0 ]
