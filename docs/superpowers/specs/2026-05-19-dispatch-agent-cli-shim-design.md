# dispatch-agent skill — CLI shim refactor

Date: 2026-05-19
Status: Approved (brainstorming, v2 after review)

## Goal

Refactor the `dispatch-agent` skill from a Python-script-bundling skill into a thin shell over the `dispatch-agent` CLI (https://github.com/superyngo/dispatch-agent). The skill is responsible only for:

1. Detecting whether the `dispatch-agent` binary is on `PATH`.
2. Guiding installation when missing.
3. Routing argv to the matching CLI subcommand, loading the matching reference file for narrative.
4. Surfacing CLI errors.

All recursion-guarding, config discovery, agent detection, dispatch, and round-robin state is delegated to the CLI. SKILL.md contains no business logic — all subcommand-specific knowledge lives in `references/`.

## Non-goals

- Pinning or runtime-checking CLI version (a minimum tested version is documented, not enforced).
- Caching detection results.
- Handling sudo / UAC elevation inside the skill (fallback to manual instructions).
- Auto-migrating any state left by the previous Python implementation.
- Re-implementing any CLI logic.

## File structure

```
skills/dispatch-agent/
  SKILL.md                          # rewritten — detection + routing only
  references/
    install-guide.md                # NEW — install one-liners per OS, min tested version
    dispatch-guide.md               # REWRITTEN — dispatch subcommand + flag reference
    detect-guide.md                 # NEW — detect JSON schema + interpretation
    init-guide.md                   # REWRITTEN — pre-flight, run, verify (CLI init is non-interactive)
    config-guide.md                 # NEW — config schema + show/list/path + edit guidance
```

Removed entirely (not archived — git history preserves them):

- `scripts/` (already git-deleted)
- `scripts.bak/`
- `data/`
- `tests/`

## SKILL.md flow

```
1. Detect CLI
   command -v dispatch-agent >/dev/null 2>&1

2. If missing:
   - Load references/install-guide.md.
   - AskUserQuestion options:
       - Install (user)
       - Install (system, requires Admin/sudo)
       - Show instructions only
       - Cancel
   - Execute selected one-liner via Bash (irm|iex on Windows, curl|bash on *nix).
   - After install:
       - Run `hash -r` (bash) or equivalent to refresh PATH cache.
       - Re-check `command -v dispatch-agent`.
       - On success: run `dispatch-agent detect` and show user which agent CLIs are ready.
   - On non-zero exit / privilege failure: print the system-install command
     verbatim and stop. The user runs it manually and re-invokes the skill.

3. Route by first argv token (CLI present). Every route loads its reference
   file first, then forwards. `--config PATH` (when supplied) is appended
   verbatim to every forwarded command.

   - `init`             -> load references/init-guide.md      -> dispatch-agent init [--config ...]
   - `detect`           -> load references/detect-guide.md    -> dispatch-agent detect
   - `config edit`      -> load references/config-guide.md
                           -> dispatch-agent config path     (capture stdout)
                           -> Tell user to edit that path in their terminal
                              ($EDITOR <path>) or with Read/Edit tools.
                           Do NOT forward `config edit` to the CLI — it requires
                           a TTY and fails with "Device not configured" inside
                           the Bash tool.
   - `config <other>`   -> load references/config-guide.md    -> dispatch-agent config <sub>
   - `-p` / `-f` / etc. -> load references/dispatch-guide.md  -> dispatch-agent dispatch <args>
   - `--help`           -> run `dispatch-agent --help` first, then load
                           references/dispatch-guide.md for skill-level notes
                           (install, init, etc.).

4. On CLI non-zero exit: show stderr, load the route's reference file for
   troubleshooting context.

5. Removed from SKILL.md entirely:
   - Recursion Guard (CLI handles DISPATCH_AGENT_DEPTH internally).
   - Find Config (CLI handles discovery).
   - Any `python3` or `scripts/` invocation.
```

OS detection: `uname -s` for macOS/Linux, `$OS == Windows_NT` (or absence of `uname`) for Windows.

The skill does not pre-empt missing `-p` / `-f`; the CLI's own error message is surfaced.

## YAML frontmatter

`argument-hint` is updated to reflect actual CLI surface:

```
[init | detect | config <show|list|path|edit> | -p <prompt> | -f <file>]
[--timeout N] [--tier ID] [--agent ID] [--config PATH] [--dry-run] [--verbose]
```

`allowed-tools` stays `Bash, Read, Write, AskUserQuestion`.

## Reference files — content responsibilities

### references/install-guide.md

- Repo link, README link.
- Windows PowerShell user / system install + uninstall one-liners.
- Linux/macOS bash user / system install + uninstall one-liners.
- Minimum tested version line: `Tested against dispatch-agent >= <current installed version at refactor time>`. Not enforced.
- One sentence describing skill behaviour when the CLI is missing.

Single source of truth for install commands. SKILL.md must not duplicate them.

### references/dispatch-guide.md (rewrite, not just edit)

Section-by-section audit. Keep / migrate / drop each existing section:

- Flag reference (`-p`, `-f`, `--timeout`, `--tier`, `--agent`, `--dry-run`, `--verbose`): keep, verify against current CLI.
- Config schema / tier semantics / env injection: **migrate to `config-guide.md`**.
- `cli-templates.toml`, `rr-state.json`, `data/` paths: **drop**. CLI owns these.
- Examples: keep, but rewrite to use `dispatch-agent dispatch ...` not `python3 scripts/dispatch.py ...`.

### references/detect-guide.md (new)

- Purpose of `detect`: probe which agent CLIs are installed and callable.
- JSON output schema: `{ <agent>: { path, version, callable, verified } }`.
- How to read the fields (callable=runs, verified=version probe succeeded).
- When to invoke: post-install confirmation, debugging "agent not found" routing errors.

### references/init-guide.md (rewrite)

CLI `init` is non-interactive (only `--config <PATH>`). Old 6-step Q&A flow is removed. New content:

- Pre-flight: deciding project-level vs user-level config path.
- Execute: `dispatch-agent init [--config <path>]`.
- Verify: `dispatch-agent config show` to inspect generated file; `dispatch-agent detect` to confirm agent availability.
- Pointer to `config-guide.md` for tuning the generated file.

### references/config-guide.md (new)

- Config schema (migrated from current dispatch-guide.md).
- Tier resolution & round-robin semantics.
- Env injection rules.
- `show` / `list` / `path` subcommand outputs.
- `edit` guidance: skill intercepts; user edits via `$EDITOR <path>` or Read/Edit tools.

## Migration notes (informational)

- Old `data/cli-templates.toml` is obsolete: CLI binary ships templates internally.
- Old `rr-state.json` location is now CLI-managed; no manual move needed.
- Existing user/project TOML configs from the Python version should remain compatible (same schema), but the skill performs no detection or conversion — users with issues should re-run `dispatch-agent init` and migrate manually.

## Acceptance criteria

- `rg "python3|scripts/|scripts\.bak|/data/" skills/dispatch-agent/` returns no matches.
- `skills/dispatch-agent/` contains only `SKILL.md` and `references/` (5 reference files).
- With CLI absent: skill loads `install-guide.md`, asks user, runs selected one-liner, `hash -r`, re-detects, then runs `dispatch-agent detect` to confirm.
- On install failure: skill prints the manual command and exits without claiming success.
- With CLI present: `-p "hi" --dry-run` forwards to `dispatch-agent dispatch -p "hi" --dry-run`.
- `init`, `detect`, `config show`, `config list`, `config path` all forward to matching CLI subcommand after loading their reference file.
- `config edit` does **not** forward; skill prints config path + edit instructions.
- `--help` runs `dispatch-agent --help` then surfaces `dispatch-guide.md`.
- `--config PATH` is appended to every forwarded command when supplied.
- SKILL.md contains no Recursion Guard or Find Config section.
- YAML frontmatter `argument-hint` lists `detect` and matches CLI surface.

## Risks

- **CLI flag drift**: if upstream changes flags, skill breaks silently. Minimum-tested-version note is a hint, not enforcement.
- **`config edit` divergence**: users invoking `config edit` get instructions instead of an editor. Documented behaviour.
- **PATH cache after install**: handled by `hash -r` + re-detection.
- **Non-interactive PowerShell**: `irm | iex` may fail; fallback to printed manual command covers this.

## Out of scope

- Pinning or runtime-checking CLI version.
- Auto-migration of old state.
- Fixing upstream CLI quirks (e.g. recursion-guard exit code 0; tracked separately).
- New dispatch features.
