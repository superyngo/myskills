---
name: multi-agent-dispatch
description: |
  Orchestrate multiple AI coding agents (Claude Code, Gemini CLI, OpenCode/oh-my-openagent,
  Codex, Copilot CLI) via CLI subprocess calls. Use this skill whenever the user wants to:
  - delegate a coding task to a specific AI agent by name
  - run tasks in parallel across multiple agents and compare results
  - use a "best model for the job" routing strategy (e.g., Claude for architecture, Codex for deep edits)
  - ask "which agent should I use for X"
  - orchestrate a pipeline where one agent's output feeds into another
  - reproduce oh-my-opencode/oh-my-openagent style agent roles (Sisyphus, Hephaestus, Oracle, Librarian) but in Claude Code context
  Always trigger this skill when the conversation involves multi-agent workflows, cross-tool delegation, or subprocess-based AI CLI calls.
allowed-tools: bash, view, edit, grep, glob, task
---

# Multi-Agent Dispatch Skill

Inspired by oh-my-openagent's orchestration model. Enables Claude Code to act as an
**Orchestrator** that spawns specialized agents via CLI subprocess calls — each agent
running on the model best suited for its task category.

---

## Core Concept: Intent → Category → Agent → Model

```
User Intent
    │
    ▼
[IntentGate] ─── classify ──→ Task Category
                                    │
                    ┌───────────────┼───────────────┐
                    ▼               ▼               ▼
              deep-work         quick-edit      ultrabrain
                    │               │               │
              Hephaestus       Hephaestus       Prometheus
            (claude / codex)  (gemini / codex)  (claude opus)
```

### Task Categories

| Category | Description | Default Agent | Preferred Model |
|----------|-------------|---------------|-----------------|
| `deep` | Autonomous exploration + end-to-end implementation | Hephaestus | `codex` / `opencode` |
| `quick` | Single-file edits, typos, small fixes | Hephaestus | `gemini` (fast/cheap) |
| `ultrabrain` | Hard logic, architecture, security review | Prometheus | `claude-opus` |
| `visual` | Frontend, UI/UX, CSS, design | Hephaestus | `gemini` (creative) |
| `research` | Codebase exploration, pattern finding, read-only | Oracle | `gemini` / `claude` |
| `orchestrate` | Multi-step task requiring sub-delegation | Sisyphus | `claude-opus` / `kimi` |

---

## Agent Roles (omo-inspired)

### Sisyphus — Orchestrator
Plans and delegates. Never stops until the task is done.
- Model: `claude-opus` (best) or `kimi-k2.5` (budget)
- Used for: `orchestrate`, multi-step tasks
- Does NOT write code directly — decomposes and delegates

### Hephaestus — Deep Worker
Autonomous implementer. Give it a goal, not a recipe.
- Model: `codex` (deep reasoning) or `gemini` (speed)
- Used for: `deep`, `quick`, `visual`
- Works end-to-end without hand-holding

### Prometheus — Strategic Planner
Interview mode: questions before execution.
- Model: `claude-opus`
- Used for: `ultrabrain`, architecture decisions
- Always produces a written plan before any execution

### Oracle — Research Specialist
Read-only codebase explorer and pattern researcher.
- Model: `gemini` (large context window advantage)
- Used for: `research`, `explore`
- Never modifies files

### Librarian — Documentation Agent
Writes, updates, and audits documentation.
- Model: `gemini` or `claude-haiku` (cost-efficient)
- Used for: docs, CHANGELOG, README updates

---

## CLI Interface Reference

See `cli-interfaces.md` for exact subprocess call syntax for each agent.

Key principle: **always use non-interactive (headless) mode** to capture stdout/stderr.

Quick reference:
```bash
# Claude Code
claude -p "PROMPT" --output-format text

# Gemini CLI  
gemini -p "PROMPT" --model gemini-2.5-pro

# OpenCode (with oh-my-openagent)
opencode run --prompt "PROMPT" --agent hephaestus

# Codex CLI
codex "PROMPT"

# Copilot CLI (gh copilot)
gh copilot suggest "PROMPT"
```

---

## Orchestration Workflow

### Step 1: IntentGate Classification

Before dispatching, classify the user's true intent using the IntentGate classifier:

```bash
# Classify user intent → JSON {category, agent, confidence}
INTENT=$(python3 scripts/classify_intent.py "$USER_PROMPT")
CATEGORY=$(echo "$INTENT" | python3 -c "import sys,json; print(json.load(sys.stdin)['category'])")
AGENT=$(echo "$INTENT" | python3 -c "import sys,json; print(json.load(sys.stdin)['agent'])")
CONFIDENCE=$(echo "$INTENT" | python3 -c "import sys,json; print(json.load(sys.stdin)['confidence'])")

# If low confidence or multi-category → use Sisyphus to decompose
if [[ "$CONFIDENCE" == "low" ]] || [[ "$CATEGORY" == "orchestrate" ]]; then
  # Delegate to Sisyphus for task decomposition
  CATEGORY="orchestrate"
  AGENT="sisyphus"
fi
```

Supported categories: `orchestrate`, `deep`, `quick`, `ultrabrain`, `visual`, `research`, `librarian`

If the intent spans multiple categories → use Sisyphus to decompose first.

### Step 2: Select Agent + Model

Use `model-routing.md` for the full routing table.

Simplified rules:
- Cost-sensitive? → `gemini-flash` for quick, `kimi-k2.5` for deep
- Quality-first? → `claude-opus` for orchestration, `codex` for deep implementation
- Large codebase? → `gemini` (2M context) for research pass first

### Step 3: Dispatch via Helper

Use `dispatch.sh` for automated CLI selection, timeout, and fallback:

```bash
source scripts/dispatch.sh

# Simple dispatch — auto-selects CLI + model based on category
RESULT=$(dispatch_agent "$CATEGORY" "$USER_PROMPT")

# With overrides
RESULT=$(dispatch_agent deep "implement auth" --timeout 600 --model claude-opus-4-6)
```

Or build manually (see `cli-interfaces.md` for exact flags):
```bash
RESULT=$(timeout 300 claude -p "$PROMPT" --model claude-sonnet-4-6 --output-format text)
EXIT_CODE=$?
```

### Step 4: Handle Output

```bash
if [ $EXIT_CODE -ne 0 ] || [ -z "$RESULT" ]; then
  # Automatic fallback via dispatch.sh, or manual via model-routing.md chains
fi

# Parse Sisyphus delegation blocks → executable commands
echo "$RESULT" | python3 scripts/parse_delegation.py --format dispatch

# Parse completion reports → JSON
echo "$RESULT" | python3 scripts/parse_delegation.py --format json
```

### Step 5: Parallel Dispatch (optional)

For independent sub-tasks, use `dispatch_parallel` (respects `DISPATCH_MAX_PARALLEL`):

```bash
source scripts/dispatch.sh

# Triplets: RESULT_VAR CATEGORY "PROMPT"
dispatch_parallel \
  auth_result   deep     "implement auth module" \
  pattern_result research "find all auth patterns in codebase"

echo "$auth_result"
echo "$pattern_result"
```

See `dispatch.sh` for the full parallel dispatch helper with structured JSON logging.

---

## Environment Setup Check

Before dispatching, verify which agents are available:

```bash
# Check availability
command -v claude   && echo "claude: ok"
command -v gemini   && echo "gemini: ok"  
command -v opencode && echo "opencode: ok"
command -v codex    && echo "codex: ok"
gh copilot --version 2>/dev/null && echo "gh copilot: ok"
```

If the desired agent is unavailable, fall back per `model-routing.md`.

---

## End-to-End Example: Multi-Step Feature Implementation

This example shows the full orchestration flow for "Build a user authentication system":

```bash
source scripts/dispatch.sh

# ── Step 1: Classify intent ──
INTENT=$(python3 scripts/classify_intent.py "build a complete user authentication system with JWT")
# → {"category": "orchestrate", "agent": "sisyphus", "confidence": "high"}

# ── Step 2: Sisyphus decomposes the task ──
PLAN=$(dispatch_agent orchestrate "Decompose this task into sub-tasks with delegation blocks:
Build a complete user authentication system with JWT for a Node.js Express API.
Requirements: signup, login, token refresh, middleware guard, password hashing." --timeout 600)

# ── Step 3: Parse delegation blocks → dispatch commands ──
echo "$PLAN" | python3 scripts/parse_delegation.py --format dispatch
# → dispatch_agent research "Find existing auth patterns in codebase" --timeout 300
# → dispatch_agent deep "Implement JWT auth module with signup/login/refresh" --timeout 600
# → dispatch_agent deep "Implement auth middleware guard" --timeout 600
# → dispatch_agent librarian "Update API documentation with auth endpoints" --timeout 300

# ── Step 4: Execute in parallel waves ──
# Wave 1: Research (no deps)
RESEARCH=$(dispatch_agent research "Find existing auth patterns and user model schema")

# Wave 2: Implementation (parallel, depends on research)
dispatch_parallel \
  auth_result deep "Implement JWT auth module. Context: $RESEARCH" \
  guard_result deep "Implement auth middleware guard. Context: $RESEARCH"

# Wave 3: Documentation (depends on implementation)
DOCS=$(dispatch_agent librarian "Document the new auth API endpoints. Files changed: $auth_result")

# ── Step 5: Parse completion reports ──
echo "$auth_result" | python3 scripts/parse_delegation.py --format summary
```

---

## Reference Files

- `cli-interfaces.md` — Exact CLI flags, env vars, output formats for each agent
- `model-routing.md` — Full routing table + fallback chains per task category
- `sisyphus.md` — Sisyphus orchestrator system prompt template
- `hephaestus.md` — Hephaestus deep worker system prompt template
- `prometheus.md` — Prometheus strategic planner system prompt template
- `oracle.md` — Oracle research specialist system prompt template
- `librarian.md` — Librarian documentation agent system prompt template
- `dispatch.sh` — Bash helper for parallel dispatch with timeout + fallback
- `classify_intent.py` — IntentGate classifier (user prompt → category + agent)
- `parse_delegation.py` — Parser for Sisyphus `<delegate>` blocks
