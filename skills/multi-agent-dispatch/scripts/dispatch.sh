#!/usr/bin/env bash
# dispatch.sh — Multi-agent dispatch helper
# Usage: source dispatch.sh, then call dispatch_agent or dispatch_parallel
#
# Requires: at least one of: claude, gemini, codex, opencode

set -euo pipefail

# ── Configuration ──────────────────────────────────────────────────────────────

DISPATCH_TIMEOUT_QUICK=120
DISPATCH_TIMEOUT_DEEP=600
DISPATCH_TIMEOUT_DEFAULT=300
DISPATCH_LOG_DIR="${DISPATCH_LOG_DIR:-/tmp/dispatch_logs}"
DISPATCH_FALLBACK_ENABLED="${DISPATCH_FALLBACK_ENABLED:-true}"
DISPATCH_MAX_PARALLEL="${DISPATCH_MAX_PARALLEL:-4}"

mkdir -p "$DISPATCH_LOG_DIR"

# ── Agent Availability Check ───────────────────────────────────────────────────

check_agents() {
  echo "=== Agent Availability ==="
  for agent in claude gemini codex opencode amp; do
    if command -v "$agent" &>/dev/null; then
      echo "  ✓ $agent"
    else
      echo "  ✗ $agent (not found)"
    fi
  done
  # gh copilot
  if gh copilot --version &>/dev/null 2>&1; then
    echo "  ✓ gh copilot"
  else
    echo "  ✗ gh copilot (not found or not authenticated)"
  fi
}

# ── Core Dispatch Function ─────────────────────────────────────────────────────

# dispatch_agent CATEGORY "PROMPT" [--timeout N] [--model MODEL] [--cwd PATH]
# Outputs result to stdout. Returns exit code from agent.
dispatch_agent() {
  local category="$1"
  local prompt="$2"
  local timeout_val="$DISPATCH_TIMEOUT_DEFAULT"
  local model_override=""
  local cwd_override=""

  # Parse optional flags
  shift 2
  while [[ $# -gt 0 ]]; do
    case "$1" in
      --timeout) timeout_val="$2"; shift 2 ;;
      --model)   model_override="$2"; shift 2 ;;
      --cwd)     cwd_override="$2"; shift 2 ;;
      *) shift ;;
    esac
  done

  # Select CLI + model based on category
  local cli model
  _select_cli_and_model "$category" "$model_override"
  cli="$_SELECTED_CLI"
  model="$_SELECTED_MODEL"

  local log_file="$DISPATCH_LOG_DIR/${category}_$(date +%s%N).log"
  local start_ts
  start_ts=$(date +%s)

  echo "[dispatch] category=$category cli=$cli model=$model timeout=${timeout_val}s" >&2

  local result exit_code=0
  result=$(
    _run_agent "$cli" "$model" "$prompt" "$timeout_val" "$cwd_override" 2>"$log_file"
  ) || exit_code=$?

  local end_ts duration_s
  end_ts=$(date +%s)
  duration_s=$((end_ts - start_ts))
  _log_json "$category" "$cli" "$model" "$exit_code" "$duration_s" "false"

  if [[ $exit_code -ne 0 ]] || [[ -z "$result" ]]; then
    echo "[dispatch] Primary agent failed (exit=$exit_code, empty=$([ -z "$result" ] && echo true || echo false))" >&2
    if [[ "$DISPATCH_FALLBACK_ENABLED" == "true" ]]; then
      result=$(_dispatch_fallback "$category" "$prompt" "$timeout_val" "$cwd_override") || exit_code=$?
    fi
  fi

  echo "$result"
  return $exit_code
}

# ── Parallel Dispatch ──────────────────────────────────────────────────────────

# dispatch_parallel RESULT_VAR_1 CATEGORY_1 "PROMPT_1" RESULT_VAR_2 CATEGORY_2 "PROMPT_2" ...
# Each triplet: result_var category prompt
# Waits for all to finish, populates result vars.
# Respects DISPATCH_MAX_PARALLEL concurrency limit.
dispatch_parallel() {
  local -a pids result_vars
  local tmpdir
  tmpdir=$(mktemp -d)

  local i=0 running=0
  while [[ $# -ge 3 ]]; do
    local result_var="$1"
    local category="$2"
    local prompt="$3"
    shift 3

    local outfile="$tmpdir/result_${i}"
    result_vars+=("$result_var")

    # Throttle: wait for a slot if at max parallel
    while (( running >= DISPATCH_MAX_PARALLEL )); do
      wait -n 2>/dev/null || true
      running=$((running - 1))
    done

    # Fire in background
    (dispatch_agent "$category" "$prompt" > "$outfile" 2>&1) &
    pids+=($!)
    running=$((running + 1))
    ((i++))
  done

  # Wait for all
  local all_ok=true
  for pid in "${pids[@]}"; do
    wait "$pid" || all_ok=false
  done

  # Collect results into named vars
  for j in "${!result_vars[@]}"; do
    local var="${result_vars[$j]}"
    local outfile="$tmpdir/result_${j}"
    printf -v "$var" '%s' "$(cat "$outfile" 2>/dev/null)"
  done

  rm -rf "$tmpdir"
  [[ "$all_ok" == "true" ]]
}

# ── Internal Helpers ───────────────────────────────────────────────────────────

_SELECTED_CLI=""
_SELECTED_MODEL=""

_select_cli_and_model() {
  local category="$1"
  local model_override="$2"

  if [[ -n "$model_override" ]]; then
    # User specified model — pick CLI that supports it
    _SELECTED_MODEL="$model_override"
    case "$model_override" in
      gpt-*|o3|o4-*|codex-*)
        _SELECTED_CLI="codex" ;;
      gemini-*)
        _SELECTED_CLI="gemini" ;;
      claude-*|claude_*)
        _SELECTED_CLI="claude" ;;
      kimi-*|glm-*|opencode/*)
        _SELECTED_CLI="opencode" ;;
      *)
        _SELECTED_CLI="claude" ;;
    esac
    return
  fi

  # Default routing by category
  case "$category" in
    orchestrate)
      if command -v claude &>/dev/null && [[ -n "${ANTHROPIC_API_KEY:-}" ]]; then
        _SELECTED_CLI="claude"; _SELECTED_MODEL="claude-opus-4-6"
      elif command -v opencode &>/dev/null; then
        _SELECTED_CLI="opencode"; _SELECTED_MODEL="kimi-k2.5"
      else
        _SELECTED_CLI="claude"; _SELECTED_MODEL="claude-sonnet-4-6"
      fi ;;
    deep)
      if command -v codex &>/dev/null && [[ -n "${OPENAI_API_KEY:-}" ]]; then
        _SELECTED_CLI="codex"; _SELECTED_MODEL="o4-mini"
      elif command -v claude &>/dev/null; then
        _SELECTED_CLI="claude"; _SELECTED_MODEL="claude-opus-4-6"
      else
        _SELECTED_CLI="gemini"; _SELECTED_MODEL="gemini-2.5-pro"
      fi ;;
    quick)
      if command -v gemini &>/dev/null && [[ -n "${GEMINI_API_KEY:-${GOOGLE_AI_API_KEY:-}}" ]]; then
        _SELECTED_CLI="gemini"; _SELECTED_MODEL="gemini-2.5-flash"
      elif command -v claude &>/dev/null; then
        _SELECTED_CLI="claude"; _SELECTED_MODEL="claude-haiku-4-5-20251001"
      else
        _SELECTED_CLI="codex"; _SELECTED_MODEL="o4-mini"
      fi ;;
    ultrabrain)
      if command -v claude &>/dev/null && [[ -n "${ANTHROPIC_API_KEY:-}" ]]; then
        _SELECTED_CLI="claude"; _SELECTED_MODEL="claude-opus-4-6"
      elif command -v codex &>/dev/null; then
        _SELECTED_CLI="codex"; _SELECTED_MODEL="o3"
      else
        _SELECTED_CLI="gemini"; _SELECTED_MODEL="gemini-2.5-pro"
      fi ;;
    visual|research)
      if command -v gemini &>/dev/null && [[ -n "${GEMINI_API_KEY:-${GOOGLE_AI_API_KEY:-}}" ]]; then
        _SELECTED_CLI="gemini"; _SELECTED_MODEL="gemini-2.5-pro"
      else
        _SELECTED_CLI="claude"; _SELECTED_MODEL="claude-sonnet-4-6"
      fi ;;
    librarian)
      if command -v gemini &>/dev/null; then
        _SELECTED_CLI="gemini"; _SELECTED_MODEL="gemini-2.5-flash"
      else
        _SELECTED_CLI="claude"; _SELECTED_MODEL="claude-haiku-4-5-20251001"
      fi ;;
    *)
      _SELECTED_CLI="claude"; _SELECTED_MODEL="claude-sonnet-4-6" ;;
  esac
}

_run_agent() {
  local cli="$1" model="$2" prompt="$3" timeout_val="$4" cwd="$5"

  local cwd_flag=""
  [[ -n "$cwd" ]] && cwd_flag="--cwd $cwd"

  case "$cli" in
    claude)
      timeout "$timeout_val" claude -p "$prompt" \
        --model "$model" \
        --output-format text \
        ${cwd_flag:+$cwd_flag}
      ;;
    gemini)
      timeout "$timeout_val" gemini --no-interactive \
        -p "$prompt" \
        --model "$model"
      ;;
    codex)
      timeout "$timeout_val" codex "$prompt" \
        --model "$model" \
        --approval-mode full-auto \
        --quiet \
        ${cwd_flag:+$cwd_flag}
      ;;
    opencode)
      timeout "$timeout_val" opencode run \
        --headless \
        --prompt "$prompt" \
        --model "$model" \
        ${cwd_flag:+$cwd_flag}
      ;;
    amp)
      timeout "$timeout_val" amp -p "$prompt" \
        --model "$model"
      ;;
    *)
      echo "ERROR: Unknown CLI '$cli'" >&2
      return 1 ;;
  esac
}

_dispatch_fallback() {
  local category="$1" prompt="$2" timeout_val="$3" cwd="$4"

  echo "[dispatch:fallback] Trying fallback for category=$category" >&2

  # Try claude as universal fallback
  if command -v claude &>/dev/null && [[ -n "${ANTHROPIC_API_KEY:-}" ]]; then
    echo "[dispatch:fallback] Using claude-sonnet as fallback" >&2
    timeout "$timeout_val" claude -p "$prompt" \
      --model claude-sonnet-4-6 \
      --output-format text \
      ${cwd:+--cwd "$cwd"}
    return $?
  fi

  # Try gemini as second fallback
  if command -v gemini &>/dev/null; then
    echo "[dispatch:fallback] Using gemini-flash as fallback" >&2
    timeout "$timeout_val" gemini --no-interactive \
      -p "$prompt" \
      --model gemini-2.5-flash
    return $?
  fi

  echo "ERROR: All fallbacks exhausted for category=$category" >&2
  return 1
}

# ── Structured Logging ─────────────────────────────────────────────────────────

DISPATCH_JSON_LOG="${DISPATCH_LOG_DIR}/dispatch.jsonl"

_log_json() {
  local category="$1" cli="$2" model="$3" exit_code="$4" duration_s="$5" is_fallback="$6"
  printf '{"ts":"%s","category":"%s","cli":"%s","model":"%s","exit_code":%s,"duration_s":%s,"fallback":%s}\n' \
    "$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
    "$category" "$cli" "$model" "$exit_code" "$duration_s" "$is_fallback" \
    >> "$DISPATCH_JSON_LOG"
}

# ── Result Aggregation ─────────────────────────────────────────────────────────

# aggregate_results DIR
# Reads all result files from a temp directory and prints a combined summary.
aggregate_results() {
  local result_dir="$1"
  local total=0 success=0 failed=0

  echo "=== Dispatch Results ==="
  for f in "$result_dir"/result_*; do
    [[ -f "$f" ]] || continue
    total=$((total + 1))
    local content
    content=$(cat "$f" 2>/dev/null)
    if [[ -n "$content" ]]; then
      success=$((success + 1))
      echo "── result_${total} (ok, $(wc -c < "$f" | tr -d ' ') bytes) ──"
    else
      failed=$((failed + 1))
      echo "── result_${total} (EMPTY) ──"
    fi
    echo "$content"
    echo ""
  done
  echo "=== Summary: ${success}/${total} succeeded, ${failed} failed ==="
}

# ── Quick Test ─────────────────────────────────────────────────────────────────

if [[ "${1:-}" == "--test" ]]; then
  echo "Testing agent availability..."
  check_agents
  echo ""
  echo "Testing quick dispatch..."
  result=$(dispatch_agent quick "Say 'dispatch_ok' and nothing else" --timeout 30)
  if echo "$result" | grep -q "dispatch_ok"; then
    echo "  ✓ Quick dispatch works: $result"
  else
    echo "  ✗ Quick dispatch unexpected output: $result"
  fi
fi
