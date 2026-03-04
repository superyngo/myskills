# Copilot Instructions

This repository is a collection of AI assistant skills for GitHub Copilot CLI and Claude.

## Repository Architecture

Each skill lives in `skills/<skill-name>/` and follows this structure:

```
skills/<skill-name>/
  SKILL.md           # Skill definition: YAML frontmatter + instruction prose
  scripts/           # Python helper scripts (standard library only)
  references/        # Markdown modules included by the skill
```

**`SKILL.md` frontmatter fields:**
- `name` — skill identifier (matches directory name)
- `description` — shown in skill picker; used for auto-invocation matching
- `argument-hint` — optional, describes positional args (e.g. `[version]`)
- `allowed-tools` — comma-separated list of tools the skill may use

The `dev-prompt.skill` file at the repo root is a ZIP-packaged bundle of `skills/dev-prompt/` — it is a distributable artifact, not the source.

## Key Conventions

### Skill scripts
- Python scripts use **standard library only** (no third-party dependencies).
- Scripts are invoked by the skill's Claude instructions with `Bash` tool, e.g. `python3 scripts/detect_language.py`.
- `compose_prompt.py` always includes `references/base.md` first, then appends the language-specific module.

### `dev-prompt` skill modules
- `references/base.md` — universal principles, always included.
- Language modules (`python.md`, `rust.md`, `javascript.md`, `scripting.md`) are appended based on detection output.
- Language detection returns JSON: `{"detected_language": "python"|null, "confidence": "high"|"medium"|"low"}`.

### `push-update` skill
- Runs code quality checks before committing (auto-detects project type via `Cargo.toml`, `package.json`, `pyproject.toml`, `go.mod`).
- Follows [Conventional Commits](https://www.conventionalcommits.org/) to suggest semver bump.
- Skips creating GitHub Releases locally if `.github/workflows/release.yml` exists.

### Planning docs
- Implementation plans go in `docs/plans/` with filename format `YYYY-MM-DD-<topic>.md`.

## Adding a New Skill

1. Create `skills/<skill-name>/SKILL.md` with YAML frontmatter.
2. Add helper scripts to `skills/<skill-name>/scripts/` (Python, stdlib only).
3. Add reference markdown to `skills/<skill-name>/references/` if needed.
4. Optionally package into a `.skill` ZIP bundle at the repo root.
