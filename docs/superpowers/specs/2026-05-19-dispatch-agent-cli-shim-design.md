# dispatch-agent skill — CLI shim refactor

Date: 2026-05-19
Status: Approved (brainstorming, v6 — minor fills from round-4 review; ready for writing-plans)

## Goal

Refactor the `dispatch-agent` skill from a Python-script-bundling skill into a thin shell over the `dispatch-agent` CLI (https://github.com/superyngo/dispatch-agent). The skill is responsible for:

1. Detecting whether the `dispatch-agent` binary is on `PATH`.
2. Guiding installation when missing.
3. Orchestrating two subcommands that need pre-CLI work (`init`, `dispatch` without prompt).
4. Routing everything else to the matching CLI subcommand and loading the matching reference file for narrative.
5. Surfacing CLI errors.

SKILL.md contains only detection, install guidance, and routing. All subcommand-specific knowledge lives in `references/`. The two orchestration cases (`init` JSON assembly and `dispatch` prompt collection) are explicit, narrow exceptions documented below.

## Verified CLI behaviour (basis for design)

Tested at refactor time. The skill must respect these:

- `dispatch-agent init` reads a JSON payload from stdin. No stdin → `invalid JSON: EOF`.
- `dispatch-agent dispatch` with no `-p` / `-f` does NOT error — it enters interactive agent mode (will hang inside Bash tool).
- `dispatch-agent config` with no ACTION fails with `Device not configured (os error 6)` (TTY required).
- `dispatch-agent config edit` requires a TTY (same failure mode).
- No `--version` flag exists.
- `--config <PATH>` accepted at both top level and subcommand level; spec standardises on **subcommand level**.
- `dispatch-agent detect` outputs JSON keyed by agent name; values `{ path, version, callable, verified }`.
- `init` **overwrites any existing config without warning**.
- `init`'s output location is determined by the JSON `save_location` field, **not** by `--config <PATH>`. `--config` on `init` only redirects which file the CLI later reads for unrelated state; it does NOT redirect the generated config destination.
- `init` does not validate that `agents[].cli` is callable, or even that the named CLI exists. Any string is accepted.
- `model: "default"` in the input JSON is resolved by the CLI to a concrete model name in the generated TOML (e.g. `claude` → `claude-sonnet-4-5`, `gemini` → `gemini-2.0-flash`).
- `dispatch --dry-run` with no `-p` / `-f` does NOT error and does NOT hang. It prints the resolved command template with a literal `<prompt>` placeholder. Forwarding it as-is is safe.

## Non-goals

- Pinning or runtime-checking CLI version (no `--version` to check). Documented as "tested against upstream commit/date X" only.
- Caching detection results.
- Handling sudo / UAC elevation inside the skill.
- Auto-migrating state left by the previous Python implementation.
- Classifying CLI exit codes into retry / config / crash categories (CLI exit-code semantics not documented upstream; future work).

## File structure

```
skills/dispatch-agent/
  SKILL.md                          # rewritten
  references/
    install-guide.md                # NEW — install one-liners per OS
    dispatch-guide.md               # REWRITTEN — dispatch flags + examples
    detect-guide.md                 # NEW — detect JSON schema
    init-guide.md                   # REWRITTEN — init JSON schema + orchestration notes
    config-guide.md                 # NEW — config schema + show/list/path + edit guidance
```

Removed entirely (git history preserves them): `scripts/`, `scripts.bak/`, `data/`, `tests/`.

## SKILL.md flow

```
1. Detect CLI
   command -v dispatch-agent >/dev/null 2>&1

2. If missing:
   - Load references/install-guide.md.
   - AskUserQuestion options: Install (user) / Install (system) / Show only / Cancel.
   - Execute selected one-liner via Bash.
   - After install: refresh shell cache, then re-detect:
       hash -r 2>/dev/null || rehash 2>/dev/null || true
       command -v dispatch-agent
   - On success: run `dispatch-agent detect` and show user which agent CLIs are ready.
   - On failure / declined elevation: print the system-install command verbatim
     and stop. The user runs it manually and re-invokes the skill.

3. Route by first non-flag argv token. Every route loads its reference file
   first. `--config PATH` (when present in argv) is passed through verbatim
   at the subcommand level — the skill does not re-place it.

   - `init`             -> ORCHESTRATED (see "init orchestration" below)
   - `detect`           -> load detect-guide.md   -> dispatch-agent detect
   - `config edit`      -> INTERCEPTED (see "config interception" below)
   - `config` (no arg)  -> INTERCEPTED (same as config edit)
   - `config <other>`   -> load config-guide.md   -> dispatch-agent config <sub>
   - `-p` / `-f` present
                        -> load dispatch-guide.md -> dispatch-agent dispatch <args>
   - no subcommand, no `-p`/`-f`, no `--dry-run`
                        -> PROMPT-COLLECTION (see "dispatch prompt collection" below)
   - `--dry-run` without prompt
                        -> forward as-is (safe: CLI prints command template
                           with literal `<prompt>` placeholder; no error,
                           no hang)
   - `--help`           -> run `dispatch-agent --help` first, then load
                           dispatch-guide.md for skill-level notes.

4. On CLI non-zero exit: print stderr, load the route's reference file for
   troubleshooting context. No exit-code classification.

5. Removed from SKILL.md entirely:
   - Recursion Guard (CLI handles DISPATCH_AGENT_DEPTH).
   - Find Config (CLI handles discovery).
   - Any `python3` / `scripts/` invocation.
```

### init orchestration

CLI `init` consumes a JSON payload from stdin. The skill assembles it from `detect` output + user confirmation, then pipes:

```
1. Load references/init-guide.md.
2. Run `dispatch-agent detect` -> parse JSON.
2.5. If `detect` returns zero `callable == true` agents:
     Inform the user no agent CLI is installed, point them at install
     docs for the agent of their choice, and abort init without writing
     anything.
3. Build a default payload from agents where `callable == true`:
     {
       "save_location": "user",
       "agents": [
         { "id": "<cli>-default", "cli": "<cli>", "model": "default",
           "args": [], "tier": "primary", "env": [] }
         ...
       ],
       "tier_order": ["primary"]
     }
   - Default id format: `<cli>-default` (e.g. `claude-default`).
   - Non-callable agents are excluded from the default; the confirmation
     step lists them as "available but not detected" so the user can
     opt in manually.
4. AskUserQuestion to confirm/override:
     - save_location: user vs project
     - which detected agents to include (callable agents pre-selected;
       non-callable agents shown but unchecked)
     - **permission-bypass flags** (off by default, explicit risk note):
         "Append non-interactive permission-bypass flags to each agent's
          args? These flags (e.g. --dangerously-skip-permissions) let the
          agent run tools without prompting — required for unattended
          dispatch but they skip safety checks."
         Options: Off (safe default) / On (I understand the risk)
       On: append the matching flag to each agent's `args[]` per the table
       in `references/config-guide.md` (claude/codex/copilot/etc.).
     - (tier_order kept simple: single "primary" tier by default)
5. Overwrite check (DATA-LOSS PREVENTION; must run AFTER save_location is known):
     - Compute target path from save_location:
         user    -> ~/.config/dispatch-agent.toml
         project -> <git-root>/.config/dispatch-agent.toml
       (Use `git rev-parse --show-toplevel`; fall back to cwd if not a repo.)
     - If that file exists:
         AskUserQuestion: "Config exists at <path>.
                          Overwrite / Backup first / Cancel"
       - Backup: copy to `<path>.bak.<UTC-timestamp>` (format: `YYYYMMDDTHHMMSSZ`,
         e.g. `20260519T143000Z`) before continuing.
       - Cancel: abort init flow.
6. Pipe the assembled JSON via stdin:
     printf '%s' "<json>" | dispatch-agent init
   `--config PATH` (if present in argv) is stripped before forwarding init —
   it does not redirect init output (verified). `save_location` is the only
   control. The skill warns the user once if `--config` was supplied with
   `init`.
7. On success: run `dispatch-agent config show` and surface the resulting file.
   If permission-bypass flags were appended, also run
   `dispatch-agent dispatch --dry-run` to verify the command shape.
   Note: `model: "default"` will appear in the output as a concrete model
   name (CLI resolves it). This is expected.
```

Init seeds a minimal config. By default `args` is empty and `env` is empty; users may opt into permission-bypass flags in step 4. For env injection (`file` / `env` / `source` types) or additional args, edit the config after init via the config-interception path.

The full JSON schema is documented in `references/init-guide.md`; SKILL.md only needs to know the high-level steps.

### dispatch prompt collection

When argv routes to dispatch but no `-p` / `-f` is supplied (and `--dry-run` is not set), the CLI would enter interactive mode and hang. The skill prevents this:

```
1. Load references/dispatch-guide.md.
2. AskUserQuestion: collect prompt text from user.
3. If returned string is empty / whitespace-only:
     AskUserQuestion again with an explicit "prompt is required" hint.
     If still empty, abort with a clear message instead of forwarding.
4. Forward as `dispatch-agent dispatch -p "<collected>" <rest-of-argv>`.
```

`-f <file>` users supply the file path, so this case only triggers when neither flag is present. `--dry-run` without prompt is NOT routed here — it forwards as-is (the CLI prints a template with a `<prompt>` placeholder, see "Verified CLI behaviour").

### config interception

`config edit` and bare `config` both require a TTY and fail inside the Bash tool. Both are intercepted:

```
1. Load references/config-guide.md.
2. Run `dispatch-agent config path` and capture stdout.
3. Tell the user: "Edit this file in your terminal: $EDITOR <path>, or use
   the Read/Edit tools."
4. Do NOT forward to the CLI.
```

OS detection for install: `uname -s` for macOS/Linux, `$OS == Windows_NT` (or absence of `uname`) for Windows.

## YAML frontmatter

```yaml
argument-hint: "[init | detect | config <show|list|path|edit> | -p <prompt> | -f <file>] [--timeout N] [--tier ID] [--agent ID] [--config PATH] [--dry-run] [--verbose]"
allowed-tools: Bash, Read, Write, AskUserQuestion
```

## Reference files — content responsibilities

### references/install-guide.md (new)

- Repo link, README link.
- Windows PowerShell user / system install + uninstall one-liners.
- Linux/macOS bash user / system install + uninstall one-liners.
- Note: "Tested against upstream commit/date <hash or YYYY-MM-DD> at refactor time. CLI has no `--version` flag; verify by running `dispatch-agent --help`."
- One sentence on skill behaviour when CLI is missing.

### references/dispatch-guide.md (rewrite)

Section-by-section audit:

- Flag reference (`-p`, `-f`, `--timeout`, `--tier`, `--agent`, `--config`, `--dry-run`, `--verbose`): keep, verify against current CLI help.
- Config schema / tier semantics / env injection: **migrate to `config-guide.md`**.
- `cli-templates.toml`, `rr-state.json`, `data/` paths: **drop**.
- Examples: rewrite as `dispatch-agent dispatch ...`.

### references/detect-guide.md (new)

- Purpose: probe which agent CLIs are installed and callable.
- JSON output schema: `{ <agent>: { path, version, callable, verified } }`.
- Interpretation: `callable=true` means the binary ran; `verified=true` means version probe succeeded.
- When to invoke: post-install confirmation, debugging "agent not found" errors, pre-init payload assembly.

### references/init-guide.md (rewrite)

- JSON stdin schema (authoritative, used by SKILL.md's init orchestration):

  ```
  {
    "save_location": "user" | "project",
    "agents": [
      {
        "id": "<one or more of: ASCII letter, digit, underscore, hyphen>",
        "cli": "<agent key; should match a detect() output key, but CLI does NOT validate>",
        "model": "<model name, or \"default\" — CLI resolves \"default\" to a concrete model>",
        "args": ["<extra cli args>"],
        "tier": "<must appear in tier_order>",
        "env": [
          { "type": "file",   "name": "...", "path": "..." },
          { "type": "env",    "name": "...", "var": "..."  },
          { "type": "source", "path": "..." }
        ]
      }
    ],
    "tier_order": ["primary", "..."]
  }
  ```

- Default-model resolution table (informational, observed at refactor time):
    - `claude` → `claude-sonnet-4-5`
    - `gemini` → `gemini-2.0-flash`
    - …(record each CLI's `"default"` resolution observed in practice; values come from upstream CLI and may change)
- Default agent-id convention used by skill's init orchestration: `<cli>-default` (e.g. `claude-default`). If multiple agents share a cli, suffix to disambiguate (`<cli>-<suffix>`).
- Init seeds minimal entries: empty `args`, empty `env`. To add permission-bypass flags or env injection (`file` / `env` / `source` types), edit the config after init via the config-interception path.
- `init` overwrites existing config without warning — the skill's orchestration guards against this with an overwrite-check step (see SKILL.md "init orchestration"). Users running `dispatch-agent init` directly do NOT get this guard.
- `--config <PATH>` does not redirect where `init` writes; `save_location` is the only control.
- Minimal example payload.
- Verify steps: `dispatch-agent config show`, `dispatch-agent detect`.
- Pointer to `config-guide.md` for tuning after generation.

### references/config-guide.md (new)

- Config TOML schema (migrated from current dispatch-guide.md).
- Tier resolution & round-robin semantics.
- Env injection rules.
- `show` / `list` / `path` outputs.
- `edit` and bare `config`: skill intercepts; user edits via `$EDITOR <path>` or Read/Edit tools.
- **Permission-bypass flag table** (authoritative source used by init orchestration):
    - `claude` → `--dangerously-skip-permissions`
    - `codex` → `--dangerously-bypass-approvals-and-sandbox`
    - `copilot` → `--allow-all`
    - `gemini` → (no standalone flag; see `gemini-npx` note below)
    - `gemini-npx` → `--skip-trust` is **already baked into the CLI template's `extra_args`**; users must NOT add it to `args[]` manually, and init orchestration must NOT append it.
    - `opencode` → no known permission-bypass flag at refactor time; init orchestration skips this CLI when "On" is chosen and logs a note.
  Note: these flags are explicitly dangerous and skip safety checks. Documented but never default-on in init.

## Migration notes (informational)

- Old `data/cli-templates.toml` is obsolete: CLI ships templates internally.
- Old `rr-state.json` is now CLI-managed; no manual move.
- Existing TOML configs from the Python version should remain schema-compatible; skill performs no detection or conversion. Users with issues should re-run `init` and migrate manually.

## Acceptance criteria

- `rg "python3|scripts/|scripts\.bak|/data/" skills/dispatch-agent/` returns no matches.
- `skills/dispatch-agent/` contains only `SKILL.md` and `references/` (5 reference files).
- CLI absent → skill loads `install-guide.md`, asks user, runs one-liner, refreshes shell cache, re-detects, runs `dispatch-agent detect`.
- Install failure → skill prints manual command and exits without claiming success.
- `-p "hi" --dry-run` forwards verbatim to `dispatch-agent dispatch -p "hi" --dry-run`.
- No `-p` / `-f` / `--dry-run` → skill collects prompt via AskUserQuestion before forwarding.
- `init` → skill runs `detect`, assembles JSON from `callable==true` agents (non-callable shown as opt-in), asks about permission-bypass flags (off by default), **then checks for existing config and offers Overwrite/Backup/Cancel using the now-known save_location**, pipes to `dispatch-agent init`. Strips `--config` from forwarded init invocation; warns user if it was supplied.
- Backup file naming: `<path>.bak.<UTC-timestamp>` with `YYYYMMDDTHHMMSSZ` format.
- No `-p` / `-f` / `--dry-run` → skill collects prompt; empty/whitespace input re-prompts then aborts rather than forwarding.
- `detect`, `config show`, `config list`, `config path` → forward after loading reference file.
- `config edit` and bare `config` → intercepted; skill prints config path + edit guidance, does not forward.
- `--help` → runs CLI help first, then loads `dispatch-guide.md`.
- `--config PATH` in argv → passed through verbatim at subcommand level; skill does not re-place it.
- SKILL.md has no Recursion Guard or Find Config section.
- YAML `argument-hint` includes `detect` and matches CLI surface.

## Risks

- **CLI flag / behaviour drift**: install-guide.md notes the tested commit/date; no runtime check.
- **`init` JSON schema drift**: documented schema in init-guide.md becomes stale if upstream changes. Manual sync required.
- **Direct CLI `init` bypasses overwrite guard**: users who run `dispatch-agent init` outside the skill silently lose their config. Documented in `init-guide.md`; skill cannot prevent.
- **`init` accepts unknown `cli` values**: spec mitigates by filtering on `callable==true` in the skill's default payload, but advanced users supplying their own JSON can still write garbage. Out of scope.
- **`config edit` / bare `config` divergence from CLI**: skill prints instructions instead of opening editor. Documented behaviour.
- **PATH cache after install**: handled by `hash -r || rehash || true` fallback chain.

## Out of scope

- Pinning or runtime-checking CLI version.
- Auto-migration of old state.
- Fixing upstream CLI quirks (recursion-guard exit code, TTY-requiring subcommands, missing `--version`).
- New dispatch features.
- Multi-tier init defaults (skill seeds a single `"primary"` tier; users add tiers manually via `config edit`).
