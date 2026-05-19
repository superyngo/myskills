# dispatch-agent skill — CLI shim refactor

Date: 2026-05-19
Status: Approved (brainstorming)

## Goal

Refactor the `dispatch-agent` skill from a Python-script-bundling skill into a thin shell over the `dispatch-agent` CLI (https://github.com/superyngo/dispatch-agent). The skill becomes responsible for:

1. Detecting whether the `dispatch-agent` binary is on `PATH`.
2. Guiding installation when missing.
3. Translating user-facing arguments into CLI subcommands.

All recursion-guarding, config discovery, agent detection, and dispatch logic is delegated to the CLI itself.

## Non-goals

- Locking the skill to a specific CLI version.
- Handling sudo / UAC elevation inside the skill (fallback to manual instructions).
- Re-implementing any logic that the CLI already provides.

## File structure (after refactor)

```
skills/dispatch-agent/
  SKILL.md                          # rewritten
  references/
    dispatch-guide.md               # python3 / scripts/ refs removed
    init-guide.md                   # python3 / scripts/ refs removed
    install-guide.md                # new — install instructions per OS
  archived/                         # preserved, not loaded by the skill
    scripts.bak/
    data/
    tests/
```

`scripts/` (already git-deleted), `data/`, and `tests/` are moved under `archived/`. Nothing in `archived/` is referenced by `SKILL.md` or any runtime path.

## SKILL.md flow

```
1. Detect CLI
   command -v dispatch-agent >/dev/null 2>&1

2. If missing:
   - Load references/install-guide.md.
   - Use AskUserQuestion to present options:
       - Install (user)
       - Install (system, requires Admin/sudo)
       - Show instructions only
       - Cancel
   - On Install: run the matching one-liner via Bash
       - Windows: irm <gist> | iex
       - macOS/Linux: curl -fsSL <gist> | APP_NAME=... REPO=... bash
   - On non-zero exit / privilege failure: print the system-install command
     and exit. The user runs it manually, then re-invokes the skill.
   - On success: re-run `command -v` to confirm before continuing.

3. Route (CLI present):
   - argv contains `init`              -> dispatch-agent init
                                          (optionally load references/init-guide.md
                                          for narrative)
   - argv contains `config <sub>`      -> dispatch-agent config <sub>
   - argv contains `-p` / `-f` / other -> dispatch-agent dispatch <args>
   - `--help` or error                 -> load references/dispatch-guide.md

4. Removed entirely from SKILL.md:
   - Recursion Guard block (CLI handles DISPATCH_AGENT_DEPTH internally)
   - Find Config block (CLI handles config discovery)
   - Any `python3 scripts/...` invocation
```

OS detection uses `uname -s` (macOS/Linux) and falls back to `$OS=Windows_NT` for Windows shells.

The skill does not pre-empt missing `-p` / `-f`; the CLI's own error is surfaced to the user.

## references/install-guide.md

Single file containing:

- Repo link: https://github.com/superyngo/dispatch-agent
- README link: https://raw.githubusercontent.com/superyngo/dispatch-agent/refs/heads/main/README.md
- Windows PowerShell user/system install + uninstall one-liners.
- Linux/macOS bash user/system install + uninstall one-liners.
- A short note describing the skill's behaviour when the CLI is missing.

The skill reads this file when the CLI is absent. It MUST be the single source of truth for install commands; SKILL.md does not duplicate them inline.

## Acceptance criteria

- `rg "python3|scripts/" skills/dispatch-agent/ -g '!archived/**'` returns no matches.
- With CLI absent, running the skill loads `install-guide.md`, asks the user, executes the chosen one-liner, then re-detects. On failure it prints the manual command and exits cleanly.
- With CLI present, `-p "hi" --dry-run` is forwarded to `dispatch-agent dispatch -p "hi" --dry-run` verbatim.
- `init`, `config show`, `config list`, `config path`, `config edit` all forward to the matching CLI subcommand.
- `archived/scripts.bak/`, `archived/data/`, `archived/tests/` all exist; nothing in `archived/` is referenced by any active code path.
- SKILL.md no longer contains a Recursion Guard or Find Config section.

## Risks

- **CLI flag drift**: if the upstream CLI changes flags, the skill silently breaks. Mitigation deferred — not pinning a version this pass.
- **Non-interactive PowerShell**: `irm | iex` may fail in some shells; the fallback to printed manual instructions covers this.
- **Skill cannot elevate privileges**: handled via the "Show instructions only" path and the failure fallback.

## Out of scope

- Pinning or detecting CLI version.
- Caching the detection result.
- Adding new dispatch features.
- Modifying upstream `dispatch-agent` CLI behaviour (e.g. its exit code when recursion guard trips — observed to be `0` with an error message; tracked separately, not fixed here).
