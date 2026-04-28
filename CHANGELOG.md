# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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
