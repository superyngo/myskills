# CLI Interfaces Reference

## Claude Code (`claude`)

```bash
# Non-interactive prompt mode
claude -p "PROMPT"
claude --prompt "PROMPT"
claude -p "PROMPT" --output-format text     # plain text (default)
claude -p "PROMPT" --output-format json     # structured JSON
claude -p "PROMPT" --output-format stream-json  # streaming JSON

# With model override
claude -p "PROMPT" --model claude-opus-4-6
claude -p "PROMPT" --model claude-sonnet-4-6

# With system prompt append
claude -p "PROMPT" --append-system-prompt "You are Hephaestus..."

# With max turns (agentic mode limit)
claude -p "PROMPT" --max-turns 10

# Pipe context from stdin
cat context.md | claude -p "Implement the feature described above"

# Working directory
claude -p "PROMPT" --cwd /path/to/project

# Environment variable alternative
ANTHROPIC_API_KEY=... claude -p "PROMPT"
```

**Notes:**
- Exit 0 = success, Exit 1 = error
- JSON output format recommended for programmatic parsing
- `--max-turns` prevents infinite loops in agentic mode

---

## Gemini CLI (`gemini`)

```bash
# Basic prompt
gemini -p "PROMPT"

# With model selection
gemini -p "PROMPT" --model gemini-2.5-pro      # best quality
gemini -p "PROMPT" --model gemini-2.5-flash     # fast + cheap
gemini -p "PROMPT" --model gemini-2.0-flash-exp # experimental

# Non-interactive (no readline / TUI)
gemini --no-interactive -p "PROMPT"

# With context files
gemini -p "PROMPT" --context @file1.md @file2.ts

# Sandbox mode (safe execution)
gemini -p "PROMPT" --sandbox

# Debug / verbose
gemini -p "PROMPT" --debug

# Environment variable
GEMINI_API_KEY=... gemini -p "PROMPT"
GOOGLE_AI_API_KEY=... gemini -p "PROMPT"
```

**Notes:**
- Gemini 2.5 Pro has 2M token context window — ideal for large codebase research
- Flash models are ~10x cheaper, good for quick/librarian tasks
- `--sandbox` runs code in isolated environment (safer for untrusted operations)

---

## OpenCode with oh-my-openagent (`opencode`)

```bash
# Basic run
opencode run --prompt "PROMPT"

# Non-interactive headless mode
opencode run --headless --prompt "PROMPT"

# Agent selection (oh-my-openagent)
opencode run --prompt "PROMPT" --agent hephaestus
opencode run --prompt "PROMPT" --agent oracle
opencode run --prompt "PROMPT" --agent librarian
opencode run --prompt "PROMPT" --agent prometheus

# With specific model override
opencode run --prompt "PROMPT" --model claude-opus-4-6
opencode run --prompt "PROMPT" --model kimi-k2.5
opencode run --prompt "PROMPT" --model glm-5
opencode run --prompt "PROMPT" --model gpt-5.3-codex

# Working directory
opencode run --prompt "PROMPT" --cwd /path/to/project

# Ultrawork shortcut (triggers full orchestration)
opencode run --prompt "ultrawork: TASK_DESCRIPTION"
```

**Notes:**
- oh-my-openagent must be installed as an OpenCode plugin for `--agent` flags to work
- `--headless` is critical for subprocess use — without it, opencode opens TUI
- `kimi-k2.5` is the recommended budget alternative to claude-opus for Sisyphus/Hephaestus

---

## Codex CLI (`codex`)

```bash
# Basic usage
codex "PROMPT"

# Non-interactive / full-auto
codex "PROMPT" --approval-mode full-auto   # no confirmation prompts
codex "PROMPT" --approval-mode auto-edit   # auto file edits, confirms exec

# With model
codex "PROMPT" --model o4-mini            # default, good balance
codex "PROMPT" --model o3                 # most capable
codex "PROMPT" --model o4-mini-high       # high reasoning effort

# Quiet mode (less verbose output)
codex "PROMPT" --quiet

# With context files
codex "PROMPT" --context /path/to/file.ts

# Environment variable
OPENAI_API_KEY=... codex "PROMPT"
```

**Notes:**
- `--approval-mode full-auto` is required for non-interactive subprocess use
- Codex is optimized for deep implementation tasks (Hephaestus role)
- o4-mini-high is best for complex algorithmic problems

---

## GitHub Copilot CLI (`gh copilot`)

```bash
# Suggest shell command
gh copilot suggest "how to find all files modified in last 24h"

# Explain a command
gh copilot explain "git rebase -i HEAD~3"

# With target shell
gh copilot suggest "PROMPT" --target shell
gh copilot suggest "PROMPT" --target gh
gh copilot suggest "PROMPT" --target git

# Non-interactive
GH_COPILOT_TELEMETRY_OPT_OUT=1 gh copilot suggest "PROMPT" --target shell
```

**Notes:**
- Copilot CLI is best for shell command generation, not general coding
- Requires `gh auth login` with Copilot subscription
- Output is shell-focused, less useful for code generation tasks

---

## AmpCode (`amp`)

```bash
# Non-interactive prompt
amp run "PROMPT"
amp -p "PROMPT"

# With model
amp -p "PROMPT" --model claude-opus-4-6

# From stdin
echo "PROMPT" | amp run
```

**Notes:**
- AmpCode is a Claude Code-compatible CLI wrapper
- Accepts same `--model` flags as Claude Code

---

## Environment Variables Quick Reference

| Agent | API Key Env Var | Config File |
|-------|-----------------|-------------|
| claude | `ANTHROPIC_API_KEY` | `~/.claude/` |
| gemini | `GEMINI_API_KEY` | `~/.gemini/` |
| opencode | per-provider (ANTHROPIC/OPENAI/etc) | `~/.config/opencode/opencode.json` |
| codex | `OPENAI_API_KEY` | `~/.codex/config.json` |
| gh copilot | (uses `gh auth` token) | `~/.config/gh/` |
| amp | `ANTHROPIC_API_KEY` | `~/.amp/` |

---

## Headless Mode Verification

Before using in subprocess, test that headless mode works:

```bash
# Should print response and exit cleanly
timeout 30 claude -p "say 'hello' and nothing else" --output-format text
timeout 30 gemini --no-interactive -p "say 'hello' and nothing else"
timeout 30 codex "say 'hello' and nothing else" --approval-mode full-auto --quiet
timeout 30 opencode run --headless --prompt "say 'hello' and nothing else"
```

All should return exit code 0 and non-empty stdout.
