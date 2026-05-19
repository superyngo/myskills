# dispatch-agent skill CLI shim refactor — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Refactor `skills/dispatch-agent/` from a Python-script-bundling skill into a thin shell over the `dispatch-agent` CLI. After this plan, the skill contains only `SKILL.md` + 5 reference files; no Python, no helper scripts.

**Architecture:** SKILL.md is a thin router. It (1) detects the CLI on PATH, (2) guides install if missing, (3) orchestrates `init` (JSON-on-stdin) and `dispatch` (prompt collection), (4) intercepts TTY-requiring `config` subcommands, (5) forwards everything else verbatim. All subcommand knowledge lives in `references/`.

**Tech Stack:** Markdown only. Skill runtime is Bash, Read, Write, AskUserQuestion (per SKILL.md frontmatter). No new dependencies.

**Spec:** `docs/superpowers/specs/2026-05-19-dispatch-agent-cli-shim-design.md` (v6).

---

## Verification model

This is a documentation refactor — there is no test suite. Each task is verified by:

- Running the exact `rg` / `ls` / `cat` / `command` shown in its "Verify" step and comparing to stated expected output.
- Each task commits independently. Commits are small enough to revert one without rolling the whole refactor.

Working directory for all tasks: `/Volumes/Home/Users/wen/.local/share/agm/source/myskills`. All paths below are relative to that root unless noted.

---

## Task 0: Snapshot and clean slate

**Files:**
- Delete: `skills/dispatch-agent/scripts.bak/`
- Delete: `skills/dispatch-agent/data/`
- Delete: `skills/dispatch-agent/tests/`
- Already git-deleted (just confirm): `skills/dispatch-agent/scripts/`

- [ ] **Step 1: Confirm current state**

Run:
```bash
git status --short skills/dispatch-agent/
ls skills/dispatch-agent/
```

Expected: `scripts.bak/`, `data/`, `tests/` exist. `scripts/` is gone (or shows as deleted in `git status`).

- [ ] **Step 2: Remove the three directories**

Run:
```bash
git rm -r skills/dispatch-agent/scripts.bak skills/dispatch-agent/data skills/dispatch-agent/tests
```

If any directory is untracked, fall back to `rm -rf` for that one. Do NOT use `rm -rf` on `scripts.bak/` if `git rm -r` succeeds — `git rm -r` already removes from working tree.

- [ ] **Step 3: Verify**

Run:
```bash
ls skills/dispatch-agent/
rg -l 'python3|scripts/|scripts\.bak|/data/' skills/dispatch-agent/ || echo "NO MATCHES"
```

Expected: only `SKILL.md` and `references/` remain. `rg` prints `NO MATCHES` (current SKILL.md and references still reference these — they will be rewritten in later tasks; the matches that remain are inside files we'll rewrite, not in directory paths). The point of this check is to confirm the directories themselves are gone.

Re-run after Task 7 with stricter expectation: `rg` returns no matches at all.

- [ ] **Step 4: Commit**

```bash
git add -A skills/dispatch-agent/
git commit -m "refactor(dispatch-agent): remove scripts.bak/, data/, tests/

Cleans up obsolete Python implementation and template files.
SKILL.md and references/ still reference them; rewritten in
subsequent commits."
```

---

## Task 1: Write `references/install-guide.md`

**Files:**
- Create: `skills/dispatch-agent/references/install-guide.md`

- [ ] **Step 1: Determine the "tested against" marker**

Run:
```bash
dispatch-agent --help 2>&1 | head -5
date -u +%Y-%m-%d
```

Note the date string; that's what goes into the "tested against" line.

- [ ] **Step 2: Write the file**

Create `skills/dispatch-agent/references/install-guide.md` with this exact content (replace `<DATE>` with the date from Step 1):

````markdown
# Installing the dispatch-agent CLI

This file is loaded by the `dispatch-agent` skill when the binary is not on `PATH`.

- **Repository:** https://github.com/superyngo/dispatch-agent
- **README:** https://raw.githubusercontent.com/superyngo/dispatch-agent/refs/heads/main/README.md
- **Tested against upstream main as of <DATE>.** The CLI has no `--version` flag; verify presence with `dispatch-agent --help`.

When the skill cannot find `dispatch-agent` on `PATH`, it asks the user to pick one of: **Install (user)**, **Install (system)**, **Show instructions only**, or **Cancel**. On a chosen install it runs the matching one-liner below via the Bash tool, refreshes the shell cache (`hash -r || rehash || true`), and re-detects. If install fails or elevation is declined, the skill prints the system command verbatim for the user to run manually and stops.

## Windows (PowerShell)

User install:

```powershell
$env:APP_NAME="dispatch-agent"; $env:REPO="superyngo/dispatch-agent"; irm https://gist.githubusercontent.com/superyngo/a6b786af38b8b4c2ce15a70ae5387bd7/raw/gpinstaller.ps1 | iex
```

User uninstall:

```powershell
$env:APP_NAME="dispatch-agent"; $env:REPO="superyngo/dispatch-agent"; $env:UNINSTALL="true"; irm https://gist.githubusercontent.com/superyngo/a6b786af38b8b4c2ce15a70ae5387bd7/raw/gpinstaller.ps1 | iex
```

System install (Administrator):

```powershell
Start-Process pwsh -Verb RunAs -ArgumentList "-NoExit","-Command","`$env:APP_NAME='dispatch-agent'; `$env:REPO='superyngo/dispatch-agent'; irm https://gist.githubusercontent.com/superyngo/a6b786af38b8b4c2ce15a70ae5387bd7/raw/gpinstaller.ps1 | iex"
```

## Linux / macOS (Bash)

User install:

```bash
curl -fsSL https://gist.githubusercontent.com/superyngo/a6b786af38b8b4c2ce15a70ae5387bd7/raw/gpinstaller.sh \
  | APP_NAME="dispatch-agent" REPO="superyngo/dispatch-agent" bash
```

User uninstall:

```bash
curl -fsSL https://gist.githubusercontent.com/superyngo/a6b786af38b8b4c2ce15a70ae5387bd7/raw/gpinstaller.sh \
  | APP_NAME="dispatch-agent" REPO="superyngo/dispatch-agent" bash -s uninstall
```

System install (root):

```bash
sudo -E bash -c 'curl -fsSL https://gist.githubusercontent.com/superyngo/a6b786af38b8b4c2ce15a70ae5387bd7/raw/gpinstaller.sh \
  | APP_NAME="dispatch-agent" REPO="superyngo/dispatch-agent" bash'
```
````

- [ ] **Step 3: Verify**

Run:
```bash
test -f skills/dispatch-agent/references/install-guide.md && echo OK
grep -c 'gpinstaller' skills/dispatch-agent/references/install-guide.md
grep -c 'Tested against upstream main' skills/dispatch-agent/references/install-guide.md
```

Expected: `OK`, `4` (four one-liners reference the gist), `1`.

- [ ] **Step 4: Commit**

```bash
git add skills/dispatch-agent/references/install-guide.md
git commit -m "docs(dispatch-agent): add install-guide.md

New reference loaded when dispatch-agent CLI is not on PATH.
Single source of truth for install/uninstall one-liners.
SKILL.md must not duplicate them."
```

---

## Task 2: Write `references/detect-guide.md`

**Files:**
- Create: `skills/dispatch-agent/references/detect-guide.md`

- [ ] **Step 1: Confirm the detect output shape**

Run:
```bash
dispatch-agent detect | head -10
```

Expected: JSON object keyed by agent name, each value `{ path, version, callable, verified }`. If you don't see this, stop — the CLI on PATH is incompatible with this plan.

- [ ] **Step 2: Write the file**

Create `skills/dispatch-agent/references/detect-guide.md`:

````markdown
# `dispatch-agent detect` — output reference

Loaded when the skill routes a `detect` invocation, after install success, and during init pre-flight to enumerate callable agents.

## Output shape

`dispatch-agent detect` prints a JSON object to stdout. Keys are agent template names; values describe what the CLI found on `PATH`.

```json
{
  "claude": {
    "path": "/usr/local/bin/claude",
    "version": "2.1.144 (Claude Code)",
    "callable": true,
    "verified": true
  },
  "gemini": {
    "path": null,
    "version": null,
    "callable": false,
    "verified": true
  }
}
```

## Field interpretation

| Field | Meaning |
|---|---|
| `path` | Resolved binary location, or `null` if not found on PATH. |
| `version` | Stdout of the agent's version probe, or `null` (probe disabled or failed). |
| `callable` | `true` if the binary was found and runs at all. |
| `verified` | `true` if the agent template's non-interactive contract is trusted upstream. `false` agents are listed but skipped at dispatch. |

## When the skill uses `detect`

- **After install:** to confirm which agent CLIs are ready.
- **Init pre-flight:** to enumerate `callable == true` agents and seed the default JSON payload. Non-callable agents are surfaced as opt-in choices so the user can still include them manually.
- **Troubleshooting:** when dispatch reports "agent not found" or unexpected tier resolution.
````

- [ ] **Step 3: Verify**

Run:
```bash
test -f skills/dispatch-agent/references/detect-guide.md && echo OK
grep -c 'callable' skills/dispatch-agent/references/detect-guide.md
```

Expected: `OK`, at least `3` (mention in JSON example, in the field table, and in the init pre-flight bullet).

- [ ] **Step 4: Commit**

```bash
git add skills/dispatch-agent/references/detect-guide.md
git commit -m "docs(dispatch-agent): add detect-guide.md

Documents the detect subcommand's JSON output schema and how
the skill consumes it (post-install confirmation, init seed,
troubleshooting)."
```

---

## Task 3: Write `references/config-guide.md`

**Files:**
- Create: `skills/dispatch-agent/references/config-guide.md`
- Reference for migration: `skills/dispatch-agent/references/dispatch-guide.md` (current — has the old config schema sections)

- [ ] **Step 1: Pull current config-related content from old dispatch-guide.md**

Run:
```bash
sed -n '/^## Config Schema/,/^## /p' skills/dispatch-agent/references/dispatch-guide.md | head -100
```

Note the schema fields, tier semantics, and env injection rules. They will be reproduced (not copy-pasted verbatim — verify against current CLI output) in the new file.

- [ ] **Step 2: Verify config subcommand outputs**

Run:
```bash
dispatch-agent config list 2>&1 | head -20
dispatch-agent config show 2>&1 | head -30
dispatch-agent config path
```

Note the current shapes. Use them when describing each subcommand below.

- [ ] **Step 3: Write the file**

Create `skills/dispatch-agent/references/config-guide.md`:

````markdown
# `dispatch-agent config` — schema, semantics, and skill behaviour

Loaded when the skill routes any `config` subcommand. `config edit` and bare `config` are intercepted by the skill (see "Editing the config" below); `show`, `list`, `path` are forwarded to the CLI verbatim.

## Where the config lives

- **User-level:** `~/.config/dispatch-agent.toml`
- **Project-level:** `<git-root>/.config/dispatch-agent.toml` (if cwd is in a git repo)

The CLI's resolution order is: `--config PATH` if supplied → project-level if in a repo → user-level. The `dispatch-agent init` JSON `save_location` field (`"user"` or `"project"`) selects which file `init` writes to. **`--config` does not redirect `init` output** — only `save_location` does.

## TOML schema

```toml
version = 1

[[tiers]]
id = "primary"

  [[tiers.agents]]
  id = "claude-default"          # [a-zA-Z0-9_-]+, unique across all tiers
  cli = "claude"                  # must match a detect() output key
  model = "claude-sonnet-4-5"     # resolved value, may be "default" in input JSON
  args = ["--dangerously-skip-permissions"]   # extra CLI args
  enabled = true                  # written by init, default true

    [[tiers.agents.env]]
    type = "file"                 # or "env", or "source"
    name = "ANTHROPIC_API_KEY"    # for "file" / "env"
    path = "~/.secrets/anthropic" # for "file" / "source"
    # var = "UPSTREAM_VAR_NAME"   # for "env"
```

### `env` block types

- `type = "file"` — load the variable named `name` from a file at `path` (one `KEY=VALUE` per line, or single-value file).
- `type = "env"` — copy the variable named `name` from the parent env var `var`.
- `type = "source"` — `source` the shell file at `path` before executing the agent (loads everything the file exports).

## Tier resolution and round-robin

Tiers are ordered. A dispatch request resolves the first tier in `tier_order` that has at least one enabled agent. Within that tier, the CLI rotates across agents in round-robin order, persisted in CLI-managed state.

`--tier ID` forces a specific tier. `--agent ID` forces a specific agent and bypasses tier logic entirely.

## Permission-bypass flags (required for unattended dispatch)

These flags let an agent run tools without prompting. They skip safety checks and are **off by default in init orchestration**. Authoritative table:

| CLI | Flag |
|---|---|
| `claude` | `--dangerously-skip-permissions` |
| `codex` | `--dangerously-bypass-approvals-and-sandbox` |
| `copilot` | `--allow-all` |
| `gemini` | (no standalone flag — use `gemini-npx`; its `--skip-trust` is already baked into the CLI template's `extra_args` and must NOT be re-added) |
| `gemini-npx` | `--skip-trust` is already in the template; do NOT add to `args[]` |
| `opencode` | no known bypass flag at refactor time |

When the user opts into bypass flags during init, the skill appends the matching flag to each agent's `args[]`. Agents with no known flag (e.g. `opencode`) are left alone and the skill logs a note.

## Subcommands

| Command | Behaviour |
|---|---|
| `dispatch-agent config show` | Forwarded. Prints the resolved TOML to stdout. |
| `dispatch-agent config list` | Forwarded. Lists agents and their callable / enabled status. |
| `dispatch-agent config path` | Forwarded. Prints the resolved config path. |
| `dispatch-agent config edit` | **Intercepted by the skill.** Requires a TTY; fails with `Device not configured (os error 6)` inside the Bash tool. |
| `dispatch-agent config` (no action) | **Intercepted by the skill.** Same TTY failure mode. |

## Editing the config

When you ask for `config edit` (or bare `config`), the skill instead:

1. Runs `dispatch-agent config path` to capture the resolved path.
2. Tells you the path and suggests one of:
   - `$EDITOR <path>` in your terminal.
   - The Read/Edit tools in this conversation.

The skill does NOT forward `config edit` to the CLI.
````

- [ ] **Step 4: Verify**

Run:
```bash
test -f skills/dispatch-agent/references/config-guide.md && echo OK
grep -c 'dangerously-skip-permissions' skills/dispatch-agent/references/config-guide.md
grep -c 'tier' skills/dispatch-agent/references/config-guide.md
grep -c 'Intercepted by the skill' skills/dispatch-agent/references/config-guide.md
```

Expected: `OK`, at least `2`, at least `3`, exactly `2`.

- [ ] **Step 5: Commit**

```bash
git add skills/dispatch-agent/references/config-guide.md
git commit -m "docs(dispatch-agent): add config-guide.md

Migrates config schema, tier semantics, and env injection rules
from dispatch-guide.md. Adds the authoritative permission-bypass
flag table used by init orchestration. Documents the skill's
interception of config edit / bare config."
```

---

## Task 4: Rewrite `references/init-guide.md`

**Files:**
- Overwrite: `skills/dispatch-agent/references/init-guide.md` (currently 150 lines of old interactive Q&A flow)

- [ ] **Step 1: Read the existing file before overwriting**

Run:
```bash
cat skills/dispatch-agent/references/init-guide.md
```

Confirm the existing content is the old 6-step Q&A flow. The rewrite replaces it entirely.

- [ ] **Step 2: Overwrite the file**

Replace `skills/dispatch-agent/references/init-guide.md` with:

````markdown
# `dispatch-agent init` — JSON stdin schema and skill orchestration

Loaded by the skill when the user invokes `init`. The CLI's `init` subcommand reads a JSON payload from stdin and writes a TOML config to either `~/.config/dispatch-agent.toml` (`save_location: "user"`) or `<git-root>/.config/dispatch-agent.toml` (`save_location: "project"`).

**The CLI overwrites existing config without warning.** The skill's orchestration adds an overwrite check (Backup / Overwrite / Cancel) before piping. Users who run `dispatch-agent init` directly, outside the skill, do NOT get this guard.

## JSON schema

```json
{
  "save_location": "user",
  "agents": [
    {
      "id": "claude-default",
      "cli": "claude",
      "model": "default",
      "args": [],
      "tier": "primary",
      "env": []
    }
  ],
  "tier_order": ["primary"]
}
```

### Field reference

| Field | Type | Notes |
|---|---|---|
| `save_location` | `"user"` \| `"project"` | Only control over output file. `--config PATH` does NOT redirect init output. |
| `agents[].id` | string | One or more of: ASCII letter, digit, underscore, hyphen. Must be unique across all tiers. |
| `agents[].cli` | string | Should match a `dispatch-agent detect` output key. **The CLI does NOT validate** — any string is accepted. |
| `agents[].model` | string | Concrete model name, or `"default"`. The CLI resolves `"default"` to a concrete model in the output TOML (see resolution table below). |
| `agents[].args` | string[] | Extra args appended to the agent invocation. Init seeds this as `[]`; users may opt into permission-bypass flags during orchestration. |
| `agents[].tier` | string | Must appear in `tier_order`. |
| `agents[].env` | object[] | Optional env injection entries; see config-guide.md for `file` / `env` / `source` types. |
| `tier_order` | string[] | Order in which tiers are tried during dispatch. |

### Default-model resolution (observed at refactor time)

These are the values the CLI substitutes when input is `"default"`. They come from upstream and may change.

| Input `cli` | Resolved `model` |
|---|---|
| `claude` | `claude-sonnet-4-5` |
| `gemini` | `gemini-2.0-flash` |
| `codex` | (record what `config show` reports after init) |
| `copilot` | (record what `config show` reports after init) |
| `opencode` | (record what `config show` reports after init) |

Implementers should fill in observed values when running init verification for the first time.

## Skill orchestration (what happens when you run `init` via the skill)

1. Load this guide.
2. Run `dispatch-agent detect`, parse the JSON.
3. **Zero-agent abort:** if no agent has `callable == true`, the skill informs you and aborts without writing anything. Install an agent CLI first.
4. Build a default JSON payload: one entry per callable agent, `id = "<cli>-default"`, `model = "default"`, `args = []`, `tier = "primary"`. Non-callable agents are listed in the confirmation step as opt-in choices.
5. Ask you to confirm:
   - `save_location`: user or project.
   - which agents to include (callable ones pre-selected).
   - **permission-bypass flags** (off by default, with an explicit risk note). On selection: append the matching flag from config-guide.md's table to each agent's `args[]`.
6. **Overwrite check** (runs after step 5 because it needs the chosen `save_location`):
   - Compute target path: `~/.config/dispatch-agent.toml` for user, `<git-root>/.config/dispatch-agent.toml` for project (`git rev-parse --show-toplevel`, fall back to cwd).
   - If the file exists: ask Overwrite / Backup first / Cancel.
   - Backup copies to `<path>.bak.<UTC-timestamp>` with format `YYYYMMDDTHHMMSSZ` (e.g. `20260519T143000Z`).
7. Pipe the JSON via stdin: `printf '%s' "<json>" | dispatch-agent init`.
   - `--config PATH` in argv is **stripped** before forwarding (it does not redirect init output). The skill warns you once if you supplied it.
8. Run `dispatch-agent config show` and surface the resulting file.
   - If you opted into bypass flags, the skill also runs `dispatch-agent dispatch --dry-run` to show the resulting command shape.

## Tuning after init

Init seeds a minimal config (no extra `args` beyond bypass flags, no `env`). To add more args or env injection:

- Run `dispatch-agent config path` to get the config location.
- Edit the file directly with `$EDITOR <path>` in your terminal, or with Read/Edit tools.
- See `config-guide.md` for the TOML schema, tier semantics, and env block types.

The skill's `config edit` route does this for you (prints the path and edit guidance).

## Multi-tier setups

Init seeds a single `"primary"` tier. Multi-tier setups (e.g. `primary` falls back to `secondary`) are not generated automatically — add additional `[[tiers]]` blocks manually in the config after init.
````

- [ ] **Step 3: Verify**

Run:
```bash
grep -c 'save_location' skills/dispatch-agent/references/init-guide.md
grep -c 'YYYYMMDDTHHMMSSZ' skills/dispatch-agent/references/init-guide.md
grep -c 'Zero-agent abort' skills/dispatch-agent/references/init-guide.md
rg 'python3|scripts/' skills/dispatch-agent/references/init-guide.md && echo "LEFTOVER" || echo "clean"
```

Expected: at least `3`, `1`, `1`, `clean`.

- [ ] **Step 4: Commit**

```bash
git add skills/dispatch-agent/references/init-guide.md
git commit -m "docs(dispatch-agent): rewrite init-guide.md for CLI shim

Replaces the old 6-step interactive flow with a JSON-stdin schema
reference and skill orchestration narrative (detect, zero-agent
abort, AskUserQuestion, overwrite check, pipe, verify). Documents
the CLI's silent overwrite behaviour."
```

---

## Task 5: Rewrite `references/dispatch-guide.md`

**Files:**
- Overwrite: `skills/dispatch-agent/references/dispatch-guide.md` (currently 244 lines, includes config schema sections that just moved to config-guide.md)

- [ ] **Step 1: Verify what to keep**

Run:
```bash
dispatch-agent dispatch --help
```

Capture the current flag surface — this is the source of truth for the flag reference.

- [ ] **Step 2: Read existing examples worth keeping**

Run:
```bash
grep -nE '^(##|###|dispatch-agent|python3)' skills/dispatch-agent/references/dispatch-guide.md
```

Identify which examples can be reused with `dispatch-agent dispatch ...` substituted for any `python3 scripts/dispatch.py ...` invocation. Config-schema sections are dropped (now in config-guide.md).

- [ ] **Step 3: Overwrite the file**

Replace `skills/dispatch-agent/references/dispatch-guide.md` with:

````markdown
# `dispatch-agent dispatch` — flag reference and examples

Loaded by the skill when it routes a dispatch invocation (the user supplied `-p` / `-f`, or the skill collected a prompt via AskUserQuestion). Also loaded for `--help` and for troubleshooting after a non-zero CLI exit.

## Flag reference

| Flag | Meaning |
|---|---|
| `-p <prompt>` | Inline prompt string. |
| `-f <file>` | Read prompt from a file. Contents become the prompt. |
| `--timeout <N>` | Per-agent timeout in seconds. `-1` (default) = no timeout. |
| `--tier <ID>` | Force a specific tier; skip the normal `tier_order` resolution. |
| `--agent <ID>` | Force a specific agent by `id`; bypass tier logic entirely. |
| `--config <PATH>` | Use a specific config file instead of the resolved default. |
| `--dry-run` | Print the resolved command without running the agent. No agent invocation happens. |
| `--verbose` | Extra logging on stderr. |

`-p` and `-f` are mutually exclusive in practice. Without either flag (and without `--dry-run`), `dispatch` enters interactive mode and will hang inside the Bash tool. The skill prevents this by collecting a prompt via AskUserQuestion before forwarding.

## How the skill routes you here

| Argv shape | Skill action |
|---|---|
| `-p "..."` or `-f path` supplied | Forward as-is. |
| Neither, and not `--dry-run` | Collect prompt via AskUserQuestion; re-prompt on empty input; abort if still empty. Then forward with `-p`. |
| `--dry-run` only | Forward as-is. CLI prints the command template with a literal `<prompt>` placeholder. Safe — no hang. |

## Examples

```bash
# Simplest: inline prompt, default tier rotation.
dispatch-agent dispatch -p "Summarise this PR diff."

# From a file.
dispatch-agent dispatch -f /tmp/prompt.md

# Force a specific agent.
dispatch-agent dispatch --agent claude-default -p "..."

# Force a tier (e.g. fallback tier).
dispatch-agent dispatch --tier secondary -p "..."

# Inspect the resolved command without running.
dispatch-agent dispatch --dry-run -p "..."

# Use a one-off config.
dispatch-agent dispatch --config /tmp/test.toml -p "..."
```

## Troubleshooting

| Symptom | Likely cause | Where to look |
|---|---|---|
| `agent not found` / unexpected agent selected | Config tier order or agent enabled flag. | `dispatch-agent config show`; see config-guide.md. |
| Agent stalls / no output | Missing permission-bypass flag for that CLI. | config-guide.md "Permission-bypass flags" table. |
| Detect lists an agent but dispatch reports it as unavailable | `verified == false` in the upstream template, or `callable == false`. | `dispatch-agent detect`; see detect-guide.md. |
| `Device not configured (os error 6)` | A `config` subcommand was forwarded that requires a TTY. | This shouldn't happen via the skill — it intercepts `config edit` and bare `config`. If you see it, you ran the CLI directly. |
| `invalid JSON: EOF` from `init` | `init` was invoked with no JSON on stdin. | Use the skill's `init` route, which pipes a JSON payload. |

See `config-guide.md` for the TOML schema, tier semantics, and env injection. See `detect-guide.md` for the JSON output of `detect`.
````

- [ ] **Step 4: Verify**

Run:
```bash
grep -c 'cli-templates\|rr-state\|data/' skills/dispatch-agent/references/dispatch-guide.md
grep -c 'python3\|scripts/' skills/dispatch-agent/references/dispatch-guide.md
grep -c '\-\-tier' skills/dispatch-agent/references/dispatch-guide.md
grep -c 'config-guide.md' skills/dispatch-agent/references/dispatch-guide.md
```

Expected: `0`, `0`, at least `3`, at least `2`.

- [ ] **Step 5: Commit**

```bash
git add skills/dispatch-agent/references/dispatch-guide.md
git commit -m "docs(dispatch-agent): rewrite dispatch-guide.md for CLI shim

Trimmed to flag reference + skill routing notes + examples +
troubleshooting. Config schema, tier semantics, env injection,
cli-templates.toml, rr-state.json references all removed
(migrated to config-guide.md or dropped as CLI-internal)."
```

---

## Task 6: Rewrite `SKILL.md`

**Files:**
- Overwrite: `skills/dispatch-agent/SKILL.md`

- [ ] **Step 1: Read current SKILL.md to confirm what changes**

Run:
```bash
cat skills/dispatch-agent/SKILL.md
```

Confirm it still references `python3 scripts/dispatch.py`, has a Recursion Guard section, has a Find Config section. All three will be removed.

- [ ] **Step 2: Overwrite SKILL.md**

Replace `skills/dispatch-agent/SKILL.md` with:

````markdown
---
name: dispatch-agent
description: Dispatch tasks to other agent CLIs with tier-based fallback
argument-hint: "[init | detect | config <show|list|path|edit> | -p <prompt> | -f <file>] [--timeout N] [--tier ID] [--agent ID] [--config PATH] [--dry-run] [--verbose]"
allowed-tools: Bash, Read, Write, AskUserQuestion
---

# dispatch-agent

Thin shell over the `dispatch-agent` CLI (https://github.com/superyngo/dispatch-agent). The skill detects the binary, guides install when missing, orchestrates two subcommands that need pre-CLI work (`init`, `dispatch` without prompt), intercepts TTY-requiring `config` subcommands, and forwards everything else verbatim. All subcommand knowledge lives in `references/`.

## 1. Detect the CLI

Run:

```bash
command -v dispatch-agent >/dev/null 2>&1
```

If it succeeds, skip to **Section 3 — Route**.

## 2. Install when missing

1. Load `references/install-guide.md` (Read the file in full — the user may need to see the repo link or the manual command).
2. Ask the user via `AskUserQuestion`:
   - **Install (user)** — non-elevated install for the current account.
   - **Install (system)** — requires Administrator (Windows) or sudo (Linux/macOS).
   - **Show instructions only** — print the one-liner, do nothing.
   - **Cancel** — abort the skill.
3. Detect OS for command selection:
   - macOS / Linux: `uname -s` returns `Darwin` or `Linux`.
   - Windows: `uname` absent, or `$OS == "Windows_NT"`.
4. Execute the matching one-liner from `install-guide.md` via Bash.
5. After install, refresh the shell cache and re-detect:

   ```bash
   hash -r 2>/dev/null || rehash 2>/dev/null || true
   command -v dispatch-agent
   ```

6. On success: run `dispatch-agent detect` and show the user which agent CLIs are ready.
7. On install failure (non-zero exit, declined elevation, network error): print the system-install command verbatim from `install-guide.md` and stop. The user runs it manually and re-invokes the skill.

The skill must not claim success unless the second `command -v` returns 0.

## 3. Route

Inspect the first non-flag argv token.

### `init` — orchestrated

The CLI's `init` reads a JSON payload from stdin. The skill assembles it:

1. Load `references/init-guide.md`.
2. Run `dispatch-agent detect`, parse the JSON.
3. **Zero-agent abort:** if no agent has `callable == true`, tell the user no agent CLI is installed, suggest installing one, and stop. Do not call `init`.
4. Build a default payload from callable agents:

   ```json
   {
     "save_location": "user",
     "agents": [
       { "id": "<cli>-default", "cli": "<cli>", "model": "default",
         "args": [], "tier": "primary", "env": [] }
     ],
     "tier_order": ["primary"]
   }
   ```

   Non-callable agents are shown to the user in the next step as opt-in only.
5. `AskUserQuestion` to confirm:
   - `save_location`: user vs project.
   - Which agents to include (callable ones pre-selected; non-callable shown unchecked).
   - **Permission-bypass flags** — off by default, with the explicit risk note: "These flags (e.g. `--dangerously-skip-permissions`) let the agent run tools without prompting — required for unattended dispatch but they skip safety checks." Options: **Off (safe default)** / **On (I understand the risk)**.
     - On: append the matching flag to each agent's `args[]` per the table in `references/config-guide.md`. Skip agents with no known bypass flag (currently `opencode`); log a note.
6. **Overwrite check** (runs AFTER step 5 because it needs the chosen `save_location`):
   - Compute target path:
     - `user` → `~/.config/dispatch-agent.toml`.
     - `project` → `$(git rev-parse --show-toplevel 2>/dev/null || pwd)/.config/dispatch-agent.toml`.
   - If the file exists, `AskUserQuestion` Overwrite / Backup first / Cancel.
     - **Backup:** `cp "<path>" "<path>.bak.$(date -u +%Y%m%dT%H%M%SZ)"` before continuing.
     - **Cancel:** stop here.
7. Pipe the JSON:

   ```bash
   printf '%s' "<json>" | dispatch-agent init
   ```

   If the user supplied `--config PATH` in argv, **strip it** before forwarding — `--config` does not redirect init output. Warn the user once that `--config` was ignored.
8. Run `dispatch-agent config show` and surface the resulting file. If bypass flags were appended, also run `dispatch-agent dispatch --dry-run` to confirm the command shape.

### `detect`

Load `references/detect-guide.md`, then run:

```bash
dispatch-agent detect
```

Pass `--config PATH` through verbatim if supplied.

### `config edit` and bare `config` — intercepted

Both require a TTY and fail with `Device not configured (os error 6)` inside the Bash tool. Do NOT forward.

1. Load `references/config-guide.md`.
2. Run `dispatch-agent config path` and capture stdout.
3. Tell the user: "Edit this file in your terminal: `$EDITOR <path>`, or with the Read/Edit tools."

### `config <show|list|path>`

Load `references/config-guide.md`, then forward:

```bash
dispatch-agent config <sub> [--config PATH]
```

### Dispatch with `-p` or `-f`

Load `references/dispatch-guide.md`, then forward verbatim:

```bash
dispatch-agent dispatch <argv>
```

### Dispatch without `-p` / `-f` and without `--dry-run` — prompt collection

The CLI would enter interactive mode and hang. Prevent this:

1. Load `references/dispatch-guide.md`.
2. `AskUserQuestion` to collect prompt text.
3. If the response is empty or whitespace-only, re-ask once with a hint. If still empty, abort with a clear message. Do NOT forward.
4. Forward as:

   ```bash
   dispatch-agent dispatch -p "<collected>" <rest-of-argv>
   ```

### Dispatch with `--dry-run` and no prompt

Forward as-is. The CLI prints the resolved command template with a literal `<prompt>` placeholder. No error, no hang.

### `--help`

1. Run `dispatch-agent --help` and surface the output.
2. Load `references/dispatch-guide.md` for skill-level notes (install, init, etc.).

## 4. CLI errors

On non-zero CLI exit: print stderr unchanged, then load the matching reference file for the route as troubleshooting context. Do not classify exit codes — upstream semantics are not documented.

## 5. `--config PATH` rules

- Passed through verbatim at subcommand level for every forwarded route.
- **Stripped** before forwarding `dispatch-agent init`, with a one-time warning to the user (init's output location is controlled only by JSON `save_location`).
````

- [ ] **Step 3: Verify SKILL.md is well-formed**

Run:
```bash
head -10 skills/dispatch-agent/SKILL.md
grep -c 'Recursion Guard\|Find Config' skills/dispatch-agent/SKILL.md
grep -c 'python3\|scripts/' skills/dispatch-agent/SKILL.md
grep -c 'detect' skills/dispatch-agent/SKILL.md
grep -c 'argument-hint:' skills/dispatch-agent/SKILL.md
```

Expected: YAML frontmatter present, `0`, `0`, at least `4`, exactly `1`. The frontmatter `argument-hint` must include `detect`.

- [ ] **Step 4: Commit**

```bash
git add skills/dispatch-agent/SKILL.md
git commit -m "refactor(dispatch-agent): rewrite SKILL.md as CLI shim

Removes Recursion Guard and Find Config sections (CLI handles
both internally). Drops all python3 / scripts/ invocations.
Adds: install orchestration with hash -r refresh, init JSON
orchestration with zero-agent abort + overwrite check, config
edit / bare config interception, dispatch prompt collection,
explicit --config passthrough rules. Frontmatter argument-hint
includes the detect subcommand."
```

---

## Task 7: Final acceptance verification

**Files:** none modified — verification only.

- [ ] **Step 1: Final directory state**

Run:
```bash
ls skills/dispatch-agent/
ls skills/dispatch-agent/references/
```

Expected:
```
SKILL.md
references/
```
and
```
config-guide.md
detect-guide.md
dispatch-guide.md
init-guide.md
install-guide.md
```

- [ ] **Step 2: No Python or legacy path references**

Run:
```bash
rg 'python3|scripts/|scripts\.bak|/data/' skills/dispatch-agent/
```

Expected: no output (exit code 1).

- [ ] **Step 3: SKILL.md frontmatter sanity**

Run:
```bash
sed -n '1,8p' skills/dispatch-agent/SKILL.md
```

Expected output contains:
- `name: dispatch-agent`
- `argument-hint:` line with `detect` and `config <show|list|path|edit>` segments
- `allowed-tools: Bash, Read, Write, AskUserQuestion`

- [ ] **Step 4: Reference files cross-link correctly**

Run:
```bash
for f in skills/dispatch-agent/references/*.md; do
  echo "== $f =="
  grep -E '\bconfig-guide\.md|\bdetect-guide\.md|\binit-guide\.md|\bdispatch-guide\.md|\binstall-guide\.md' "$f" || echo "(no cross-links)"
done
```

Expected: cross-links exist between guides that the spec says reference each other (init→config, dispatch→config+detect, config↔init via permission flags).

- [ ] **Step 5: Live-route smoke test against installed CLI**

This is a manual smoke test, not a CI gate. Confirm the skill's described behaviour by running the CLI commands the skill would issue:

```bash
command -v dispatch-agent
dispatch-agent --help | head -10
dispatch-agent detect | head -20
dispatch-agent config path
```

Each should succeed. Do NOT run `dispatch-agent init` here — it overwrites config. The orchestration is exercised by invoking the skill itself in a follow-up session.

- [ ] **Step 6: Confirm git state is clean**

Run:
```bash
git status --short skills/dispatch-agent/
```

Expected: empty (all changes committed across Tasks 0–6).

- [ ] **Step 7: No commit needed; this is a verification-only task.**

If any check failed, fix in the offending task and re-commit there. Do not paper over with a follow-up commit at this stage.

---

## Spec coverage map

| Spec requirement | Implementing task |
|---|---|
| Remove `scripts/`, `scripts.bak/`, `data/`, `tests/` | Task 0 |
| `references/install-guide.md` with one-liners + tested-against note | Task 1 |
| `references/detect-guide.md` with JSON schema | Task 2 |
| `references/config-guide.md` with schema + permission-bypass table | Task 3 |
| `references/init-guide.md` rewrite with JSON schema + orchestration | Task 4 |
| `references/dispatch-guide.md` rewrite (config sections removed) | Task 5 |
| SKILL.md detection step with `hash -r || rehash || true` | Task 6 |
| SKILL.md install AskUserQuestion + post-install `detect` | Task 6 |
| SKILL.md init orchestration: detect → zero-agent abort → payload → AskUserQuestion → overwrite check → pipe → verify | Task 6 |
| SKILL.md dispatch prompt collection + empty-prompt re-ask/abort | Task 6 |
| SKILL.md `config edit` / bare `config` interception | Task 6 |
| SKILL.md `--config PATH` passthrough + strip-on-init rule | Task 6 |
| SKILL.md `--help` runs CLI help then loads dispatch-guide.md | Task 6 |
| SKILL.md no Recursion Guard / no Find Config / no python3 | Task 6 + Task 7 verify |
| `argument-hint` includes `detect` | Task 6 + Task 7 verify |
| Acceptance: `rg 'python3|scripts/...'` empty | Task 7 |
| Acceptance: only `SKILL.md` + 5 references | Task 7 |

All spec requirements are mapped. No placeholders remain.
