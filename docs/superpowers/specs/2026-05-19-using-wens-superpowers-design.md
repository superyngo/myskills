# using-wens-superpowers — Design Spec

**Date:** 2026-05-19
**Author:** wen (superyngo@gmail.com)
**Status:** Draft

## 1. Purpose

A workflow orchestrator skill that wraps the standard superpowers development flow (brainstorm → spec → plan → subagent-driven-implement) with externally-dispatched verification rounds at three critical nodes:

1. After `brainstorming` produces a spec → iterative external spec review until clean.
2. After `writing-plans` produces a plan → single external plan-vs-spec consistency check.
3. Inside `subagent-driven-development` → per-task spec-compliance review and code review (and optionally the implementer itself) are delegated to `dispatch-agent`.

The motivation is to **conserve the main Claude session's context and token budget** by pushing review (and optionally implementation) work to third-party agent CLIs via the existing `dispatch-agent` binary, while the main session remains the authoritative coordinator.

## 2. Scope

### In scope
- A new skill at `skills/using-wens-superpowers/` consisting of:
  - `SKILL.md` (short orchestrator, no duplication of other skills' content)
  - `scripts/dispatch.sh` (thin wrapper around `dispatch-agent dispatch -f`)
  - `references/*-prompt.md` (prompt templates for each external call)
- A `.gitignore` entry for `docs/tmp/` if missing.

### Out of scope
- Installing or detecting the `dispatch-agent` CLI (assumed installed).
- Replacing or modifying the upstream superpowers skills (`brainstorming`, `writing-plans`, `subagent-driven-development`).
- Distributing this skill globally — install path is limited to this repo's `skills/`.
- A `.skill` ZIP bundle (can be added later).

## 3. Non-goals
- Not a general-purpose dispatch wrapper; it encodes one specific workflow.
- Not concerned with which third-party agent (Codex / Gemini / Claude-CLI) `dispatch-agent` ultimately routes to — that is the user's `dispatch-agent` config.

## 4. Users and triggers

Single user (wen). Triggered by typing `/using-wens-superpowers` at the start of a development task that warrants the full spec/plan/implement cycle.

The skill is **not** triggered automatically by `/using-superpowers`; the two coexist.

## 5. Workflow

The skill drives a 7-stage flow. Each stage either delegates to an existing skill via the `Skill` tool, or runs an injected `dispatch-agent` step.

### Stage 1 — Clarify requirements
`Skill(brainstorming)` performs its standard checklist up to the point of producing a design doc at `docs/superpowers/specs/YYYY-MM-DD-<topic>-design.md`.

### Stage 2 — Author design spec
Same skill, same step. Output is the spec markdown file.

### Stage 3 — External spec review loop (injected)
**Loop:**
1. Render `references/spec-review-prompt.md` with the spec's absolute path and the current round number, write it to `docs/tmp/<ts>-spec-review-r<N>.md`.
2. Invoke `scripts/dispatch.sh spec-review-r<N>` with the rendered prompt on stdin.
3. Read the resulting `.out.md`, parse the leading YAML frontmatter for `status`.
4. If `status: PASS` → exit loop.
5. If `status: ISSUES_FOUND` → main agent edits the spec to address issues, increment N, repeat.

**Termination guards:**
- No fixed upper bound. When N reaches 10 without `PASS`, the skill uses `AskUserQuestion` to ask the user whether to continue, pause, or accept the spec as-is. If "continue," reset the prompt counter (allow further rounds).
- Per-round artifacts are kept on disk (in `docs/tmp/`, gitignored) as an audit trail of the spec's evolution.

### Stage 4 — Write implementation plan
`Skill(writing-plans)` runs unchanged. Output: `docs/superpowers/plans/YYYY-MM-DD-<feature-name>.md`.

### Stage 5 — Plan consistency check (injected)
Single pass:
1. Render `references/plan-verify-prompt.md` with absolute paths to both spec and plan, write to `docs/tmp/<ts>-plan-verify.md`.
2. `scripts/dispatch.sh plan-verify` ← prompt.
3. Parse `.out.md` for `status`.
4. If `PASS` → proceed. If `ISSUES_FOUND` → main agent revises plan, repeat (no cap; same 10-round AskUserQuestion gate applies).

### Stage 6 — Subagent-driven implementation (injected)
6.1. **Mode prompt:** the skill calls `AskUserQuestion`:
- (a) **reviewers only** — implementer remains a Task-tool subagent (per upstream `subagent-driven-development`); both reviewers are dispatched.
- (b) **all dispatched** — implementer, spec-compliance reviewer, code reviewer all go through `dispatch-agent`. Skill shows a one-time risk note: "Mode (b) assumes your dispatch-agent config has `--dangerously-skip-permissions` (or equivalent) enabled so the third-party agent can write files directly."

6.2. **Per task** (iterate over all tasks in the plan):

Mode (a):
- Implementer: standard upstream Task-tool subagent (unchanged).
- Spec-compliance review: render `references/spec-compliance-review-prompt.md` → `dispatch.sh spec-compliance-task<i>` → parse status. On `ISSUES_FOUND`, re-dispatch implementer with feedback.
- Code review: render `references/code-review-prompt.md` → `dispatch.sh code-review-task<i>` → parse status. On `ISSUES_FOUND`, re-dispatch implementer.

Mode (b):
- Implementer: render `references/implement-task-prompt.md` with the task body and the spec/plan paths → `dispatch.sh implement-task<i>` → the third-party agent writes files directly.
- Reviewers: same as mode (a).

6.3. Continuous execution per upstream `subagent-driven-development` — no human check-in between tasks. Stop only on BLOCKED, all-pass, or the 10-round AskUserQuestion gate triggering on a single task's review loop.

### Stage 7 — Finalize
After all tasks pass both reviews:
1. Main agent appends a single Unreleased entry to `CHANGELOG.md` summarizing the feature.
2. Updates `README.md` / other top-level docs as warranted by the change.
3. Reports completion to the user. Does **not** auto-commit (user runs `/git-release` separately).

## 6. dispatch-agent invocation contract

### CLI assumption
`dispatch-agent` is on `PATH`. The skill does not detect or install it. If absent, the first `dispatch.sh` invocation fails with the binary's native error.

### Prompt → file convention
- Path: `docs/tmp/<UTC-timestamp>-<phase-slug>.md`
- Timestamp format: `YYYYMMDDTHHMMSSZ` (e.g., `20260519T143012Z`)
- Phase slugs (deterministic, used in filename and in `dispatch.sh` argv):
  - `spec-review-r<N>`
  - `plan-verify` (suffix `-r<N>` on retries)
  - `implement-task<i>`
  - `spec-compliance-task<i>` (suffix `-r<N>` on retries)
  - `code-review-task<i>` (suffix `-r<N>` on retries)

### Output capture
The `.out.md` sibling file receives `dispatch-agent`'s stdout verbatim. Main agent reads it with `Read`.

### Wrapper script (`scripts/dispatch.sh`)
- Signature: `dispatch.sh <phase-slug>` — reads prompt body from stdin.
- Behavior:
  1. Resolve git root (or cwd) and ensure `docs/tmp/` exists.
  2. Compute timestamp; build `PROMPT=docs/tmp/<ts>-<slug>.md`, `OUT=docs/tmp/<ts>-<slug>.out.md`.
  3. Write stdin to `$PROMPT`.
  4. Run `dispatch-agent dispatch -f "$PROMPT"`, tee stdout to `$OUT`.
  5. Print on stderr (so callers see it): `prompt=$PROMPT` / `out=$OUT`.
  6. Exit with `dispatch-agent`'s exit code.
- POSIX `sh` compatible, no third-party deps.

### Reviewer output contract
Every reviewer prompt instructs the third-party agent to begin its response with a YAML frontmatter block:

```yaml
---
status: PASS | ISSUES_FOUND
issues:
  - severity: blocker | major | minor
    location: <file:line or section name>
    description: <what's wrong>
    suggestion: <concrete fix or direction>
---
```

Free-form prose may follow. The main agent parses only the frontmatter for the loop decision. If frontmatter is missing or malformed, the skill treats the response as `ISSUES_FOUND` with a synthetic issue noting the format violation, and re-dispatches.

### Implementer output contract (mode b)
The implementer prompt instructs the third-party agent to:
- Write files directly to the workspace (relying on bypass flags).
- Emit a final YAML frontmatter:
  ```yaml
  ---
  status: COMPLETED | BLOCKED
  files_changed:
    - <path>
  notes: <free text>
  ---
  ```
- On `BLOCKED`, main agent surfaces the notes to the user via `AskUserQuestion`.

## 7. Prompt templates

Each template is a markdown file in `references/` with `{{placeholder}}` substitutions resolved at render time (simple shell `sed` is sufficient — no real template engine).

| File | Placeholders |
|---|---|
| `spec-review-prompt.md` | `{{spec_path}}`, `{{round}}` |
| `plan-verify-prompt.md` | `{{spec_path}}`, `{{plan_path}}` |
| `implement-task-prompt.md` | `{{spec_path}}`, `{{plan_path}}`, `{{task_body}}`, `{{repo_root}}` |
| `spec-compliance-review-prompt.md` | `{{spec_path}}`, `{{task_body}}`, `{{files_changed}}` |
| `code-review-prompt.md` | `{{plan_path}}`, `{{task_body}}`, `{{files_changed}}` |

Every template ends with an explicit reminder of the output contract (Section 6).

Rendering is done inline by the main agent (Read template → string-substitute → pipe to `dispatch.sh`). No separate rendering script.

## 8. File layout

```
skills/using-wens-superpowers/
  SKILL.md
  scripts/
    dispatch.sh
  references/
    spec-review-prompt.md
    plan-verify-prompt.md
    implement-task-prompt.md
    spec-compliance-review-prompt.md
    code-review-prompt.md
```

`docs/tmp/` is created lazily by `dispatch.sh` on first call.

## 9. .gitignore handling

On entry, the skill checks the project root `.gitignore` for `docs/tmp/`. If missing, it appends the line and informs the user once. Idempotent.

## 10. Risks and tradeoffs

- **Mode (b) requires bypass flags.** The user is single-operator and aware; risk is acceptable but surfaced once per session.
- **Third-party agent may ignore the output contract.** Mitigated by treating malformed output as `ISSUES_FOUND` and re-dispatching with a stricter reminder. Repeated failures will be visible (loop progress prints).
- **`docs/tmp/` accumulates.** Gitignored, so it does not bloat the repo, but it does grow on disk. Out of scope to clean up automatically — user can `rm -rf docs/tmp` between sessions.
- **The skill duplicates orchestration logic that `/using-superpowers` already loosely implies.** Acceptable because `/using-wens-superpowers` is the explicit opt-in path; the standard `/using-superpowers` is untouched.
- **Per-task review loops have no hard cap.** Same 10-round AskUserQuestion gate applies to keep humans in the loop on pathological cases.
- **Spec/plan revisions during review loops are done by the main agent, not dispatched.** This is intentional: the third-party agent emits diagnoses, the main agent (with full conversational context of the user's intent) makes the edits. Trying to dispatch the rewrite too would lose user-intent fidelity.

## 11. Success criteria

1. From a clean repo state, `/using-wens-superpowers` drives a full feature through all 7 stages without manual intervention except: brainstorming clarifications, mode selection (a/b), and 10-round gate checks.
2. Every `dispatch-agent` call produces both a `docs/tmp/*.md` prompt file and a `.out.md` output file.
3. Review loops terminate when the third-party agent emits `status: PASS`, or via the 10-round user gate.
4. `docs/tmp/` is gitignored after first run; no temp files appear in `git status`.
5. The SKILL.md file is under ~150 lines (forces real delegation, not duplication).
6. `scripts/dispatch.sh` is under ~40 lines.

## 12. Open questions

None at spec time — all decisions confirmed in the pre-spec dialog on 2026-05-19.
