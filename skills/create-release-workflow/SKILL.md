---
name: create-release-workflow
description: Use when generating a GitHub Actions release workflow (.github/workflows/release.yml) for a Rust project with multi-platform binary builds (Linux/Windows/macOS, gnu/musl, x86/arm) triggered by v*.*.* tags
---

# Create Release Workflow (Rust)

## Overview

Generates a complete `.github/workflows/release.yml` for cross-platform Rust binary releases. Acts as a DevOps engineer: collects project + target info from the user, then emits the full YAML.

## When to Use

- User wants to add release automation to a Rust project
- Need multi-platform builds (Linux gnu/musl, Windows MSVC, macOS Intel/ARM)
- Release is triggered by pushing `v*.*.*` tags
- Need SHA256SUMS, tar.gz packaging, and auto-generated changelogs

**Don't use for:** non-Rust projects, library-only crates (no binary), or projects that already have a working release workflow (edit instead).

## Required Inputs

Ask the user before generating:

1. **Binary name** — `<BINARY_NAME>` (Windows uses `<BINARY_NAME>.exe`)
2. **Cargo workspace?** — yes/no
3. **Build features** — e.g. `--features tls`, or none
4. **Target platforms** (let user check from the list below)
5. **Custom RUSTFLAGS per target** — e.g. armv7 needs `-C linker=arm-linux-gnueabihf-gcc`

### Target Platform Menu

**Linux (ubuntu-latest):**
- `x86_64-unknown-linux-gnu`, `i686-unknown-linux-gnu`
- `x86_64-unknown-linux-musl`, `i686-unknown-linux-musl`
- `armv7-unknown-linux-gnueabihf`, `armv7-unknown-linux-musleabihf`
- `aarch64-unknown-linux-gnu`, `aarch64-unknown-linux-musl`

**Windows (windows-latest):**
- `x86_64-pc-windows-msvc`, `i686-pc-windows-msvc`

**macOS:**
- `x86_64-apple-darwin` → `macos-15-intel`
- `aarch64-apple-darwin` → `macos-latest`

## Build Strategy

### Optimization

| Platform | Strategy |
|----------|----------|
| Linux / macOS | Use `Cargo.toml` release profile (aggressive) |
| Windows | Override via env to avoid AV false positives: `opt-level=3`, `lto="thin"`, `strip=false`, `codegen-units=16`, `panic="unwind"`, `RUSTFLAGS="-C target-feature=+crt-static"` |

### Cross-Compilation

| Target group | Tool |
|--------------|------|
| musl (i686, armv7, aarch64) | `cross` (`cargo install cross --git https://github.com/cross-rs/cross`) |
| ARM GNU | install `gcc-arm-linux-gnueabihf`, configure `~/.cargo/config.toml` linker |
| Other Linux | native `cargo build` |

### Strip

| Target | Command |
|--------|---------|
| x86 Linux (gnu/musl) | `strip` |
| armv7 gnueabihf | `arm-linux-gnueabihf-strip` (skip if missing) |
| aarch64-linux-gnu | `aarch64-linux-gnu-strip` |
| aarch64-musl | skip (cross handles it) |
| Windows / macOS | no strip |

## Release Artifacts

- **Linux / macOS:** `<BINARY_NAME>-<platform>.tar.gz`
- **Windows:** `<BINARY_NAME>-windows-<arch>.exe` (no archive)
- Always emit `SHA256SUMS`

## Release Metadata

- **Trigger:** push tags matching `v*.*.*`
- **Body:** annotated tag message as preamble + `generate_release_notes: true`
- `draft: false`, `prerelease: false`

## Workflow Requirements

- `fail-fast: false` — platforms independent
- `permissions: contents: write, actions: write`
- Toolchain: `dtolnay/rust-toolchain@stable`
- Release action: `softprops/action-gh-release@v1`
- Ask user about extras: winget workflow_dispatch, `cargo-deny`, test runs, etc.

## Output

Emit the complete YAML file. No truncation, no placeholders left unresolved — substitute every `<...>` with the user's answers.
