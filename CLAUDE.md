# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Repository Purpose

A collection of AI assistant skills for Claude and GitHub Copilot CLI. Each skill is a self-contained directory under `skills/` that defines a specialized AI behavior.

## Skill Structure

```
skills/<skill-name>/
  SKILL.md           # YAML frontmatter + instruction prose
  scripts/           # Python helper scripts (stdlib only — no third-party deps)
  references/        # Markdown modules included by the skill
```

**`SKILL.md` frontmatter fields:** `name`, `description`, `argument-hint` (optional), `allowed-tools`

## Key Conventions

- **Python scripts**: Standard library only. Invoked via `Bash` tool as `python3 scripts/<script>.py`.
- **Planning docs**: Go in `docs/plans/` with filename `YYYY-MM-DD-<topic>.md`.
- **Distributable bundles**: `.skill` files at repo root are ZIP-packaged bundles of a skill directory — not source.

## `dev-prompt` Skill

- `references/base.md` — universal principles, always included first.
- `compose_prompt.py` concatenates `base.md` + a language-specific module (`python.md`, `rust.md`, `javascript.md`, `scripting.md`).
- `detect_language.py` returns JSON: `{"detected_language": "python"|null, "confidence": "high"|"medium"|"low"}`.

## `git-release` Skill (formerly `push-update`)

- Auto-detects project type via `Cargo.toml`, `package.json`, `pyproject.toml`, `go.mod`.
- Runs quality checks (fmt, lint, test) before committing.
- Follows [Conventional Commits](https://www.conventionalcommits.org/) to suggest semver bump.
- Skips GitHub Releases if `.github/workflows/release.yml` exists.

## `github-init` Skill

- Initialises a GitHub Repo or Gist for the current directory.
- Handles `git init`, skeleton file generation (README, CHANGELOG, LICENSE, .gitignore), and optionally `.github/workflows/release.yml` for binary projects.
- Uses `gh repo create` / `gh gist create` for remote setup.
- `release.yml` template adapted from Wenget's release workflow (all `wenget` references replaced with project name; "Update bucket binary" step removed).
- Install snippet in README uses the generic `gpinstall.ps1` Gist.

## Adding a New Skill

1. Create `skills/<skill-name>/SKILL.md` with YAML frontmatter.
2. Add Python stdlib-only helper scripts to `skills/<skill-name>/scripts/`.
3. Add reference markdown to `skills/<skill-name>/references/` if needed.
4. Optionally package into a `.skill` ZIP bundle at the repo root.
