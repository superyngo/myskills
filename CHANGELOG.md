# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### 2026-05-10 — feat(rust): implement rr_state.rs (load_rr_state, store_rr_state)

- `skills/dispatch-agent/rust/src/rr_state.rs`: Implements round-robin state persistence with file-based locking (fs2 sidecar lock), JSON serialization via IndexMap, and graceful error handling (NotFound → empty, PermissionDenied/parse errors → warn stderr). Includes roundtrip, NotFound, and concurrent load+store tests.

### 2026-05-10 — feat(dispatch-agent): Rust crate scaffold and cli-templates.toml rewrite

- `skills/dispatch-agent/rust/`: New Rust crate for the dispatch-agent binary rewrite (PR 1, layer a). Implements `types.rs`, `fsutil.rs`, `config.rs`, `templates.rs` with full unit tests. Python scripts remain the active entry point; Rust source is dark in production (see docs/plans/2026-05-10-dispatch-agent-rust-rewrite.md for rollout plan).
- `skills/dispatch-agent/data/cli-templates.toml`: Rewritten as a fully-commented field reference document.

### 2026-05-08 — feat(dispatch-agent): add type=source env entries for shell env file sourcing

- Added `type=source` as a valid env entry type in dispatch-agent config
- At dispatch time, source files are loaded via `bash -c "set -a; source <file>; set +a; exec ..."` — no Python-side parsing needed
- Updated `init-guide.md` to include the new "Source env file (type=source)" option
- Updated `init.py` TOML serialization to omit `name` field for source entries
- Updated `--show-config` display to label source entries correctly

### 2026-04-28 — feat(yt-channel-dl): background execution with status file

- Added `--status-file` to `download_channel.py` — writes compact JSON progress at each video boundary
- SKILL.md step 3 now runs the download script in background via `nohup`, redirecting output to a log file
- Agent polls a tiny status.json instead of streaming all stdout, drastically reducing token consumption
- Users can monitor progress via `tail -f download.log` in another terminal

### 2026-04-28 — feat(yt-channel-dl): add playlist URL support

- SKILL.md now accepts YouTube playlist URLs (e.g. `?list=PLxxxxxx`) alongside channel URLs
- Updated prompts, parameter descriptions, and script metadata to reflect channel + playlist support
- Summary output changed from "Total in channel" to generic "Total"

### 2026-03-04 — feat: add github-init skill

Add `github-init` skill for initialising a new GitHub repository or Gist from the current directory. Handles git init, skeleton file generation (README, CHANGELOG, LICENSE, .gitignore, release workflow), remote creation via `gh repo create`, and initial push.
