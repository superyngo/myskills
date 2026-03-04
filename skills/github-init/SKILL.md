---
name: github-init
description: Use when initialising a new GitHub repository or Gist for the current directory. Handles git init, skeleton file generation (README, CHANGELOG, LICENSE, .gitignore, release workflow), remote creation, and initial push.
argument-hint: [repo|gist]
allowed-tools: Read, Write, Edit, Bash, Grep, Glob, AskUserQuestion
---

# GitHub 初始化流程

為當前目錄建立 GitHub 遠端倉庫或 Gist，並生成標準項目骨架文件。

---

## 步驟 0：確認模式

根據參數決定流程：
- 參數為 `gist` → 直接跳至步驟 5（Gist 流程）
- 參數為 `repo` 或無參數 → 執行 Repo 流程
- 若不明確，詢問用戶：「建立 GitHub Repo 還是 Gist？」

---

## 步驟 1：檢查 Git 狀態

```bash
git remote -v 2>/dev/null
git status 2>/dev/null
```

根據結果：

- **無 git 倉庫**：執行 `git init`，然後繼續
- **有 git 倉庫，無遠端**：繼續步驟 2
- **有 git 倉庫，已有遠端**：告知用戶遠端已存在，詢問是否只補建缺少的骨架文件，然後結束

---

## 步驟 2：檢測項目類型

按順序檢查以下標記文件：

| 標記文件 | 項目類型 |
|---|---|
| `Cargo.toml` | Rust |
| `package.json` | Node.js |
| `pyproject.toml` / `setup.py` | Python |
| `go.mod` | Go |
| 無 | Generic |

同時檢測是否為**二進制項目**：
- Rust：存在 `src/main.rs`
- Go：`main` package
- Node.js：`package.json` 中有 `bin` 字段

---

## 步驟 3：生成骨架文件

詢問用戶確認要生成哪些文件（根據項目類型預先勾選，跳過已存在的文件）：

### 必選文件（所有 Repo）

**README.md**（若不存在）：

```markdown
# <project-name>

<description>

## Installation (Windows)

```powershell
$env:APP_NAME="<project-name>"; $env:REPO="superyngo/<project-name>"; irm https://gist.githubusercontent.com/superyngo/a6b786af38b8b4c2ce15a70ae5387bd7/raw/gpinstall.ps1 | iex
```

## Usage

...

## License

MIT
```

> 僅當項目為二進制項目時才包含 Installation 段落。描述從用戶在步驟 4 提供的 repo description 中獲取。

**CHANGELOG.md**（若不存在）：

```markdown
# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]
```

**LICENSE**（若不存在）— MIT，作者：`wen`，年份：當前年份：

```
MIT License

Copyright (c) <YEAR> wen

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

**.gitignore**（若不存在）— 根據項目類型：

- **Rust**：
  ```
  /target/
  **/*.rs.bk
  ```
- **Node.js**：
  ```
  node_modules/
  dist/
  .env
  ```
- **Python**：
  ```
  __pycache__/
  *.pyc
  .venv/
  dist/
  *.egg-info/
  ```
- **Go**：
  ```
  *.exe
  *.exe~
  *.test
  vendor/
  ```
- **Generic**：空或基本 OS 文件忽略

### 二進制項目額外文件

如果項目產生二進制程序，生成 `.github/workflows/release.yml`（從 Wenget 的 release.yml 改編，替換所有 `wenget` 為 `<project-name>`，並移除「Update bucket binary」最後步驟）：

```yaml
name: Release Build

on:
  push:
    tags:
      - "v*.*.*"
  workflow_dispatch:
    inputs:
      version:
        description: 'Release version (e.g., v0.3.0)'
        required: true
        type: string

permissions:
  contents: write
  actions: write

env:
  CARGO_TERM_COLOR: always

jobs:
  build:
    name: Build ${{ matrix.target }}
    runs-on: ${{ matrix.os }}
    continue-on-error: false
    strategy:
      fail-fast: false
      matrix:
        include:
          # Linux builds
          - os: ubuntu-latest
            target: x86_64-unknown-linux-gnu
            artifact_name: <project-name>
            asset_name: <project-name>-linux-x86_64
          - os: ubuntu-latest
            target: i686-unknown-linux-gnu
            artifact_name: <project-name>
            asset_name: <project-name>-linux-i686
          - os: ubuntu-latest
            target: x86_64-unknown-linux-musl
            artifact_name: <project-name>
            asset_name: <project-name>-linux-x86_64-musl
          - os: ubuntu-latest
            target: armv7-unknown-linux-gnueabihf
            artifact_name: <project-name>
            asset_name: <project-name>-linux-armv7
          - os: ubuntu-latest
            target: aarch64-unknown-linux-gnu
            artifact_name: <project-name>
            asset_name: <project-name>-linux-aarch64
          - os: ubuntu-latest
            target: aarch64-unknown-linux-musl
            artifact_name: <project-name>
            asset_name: <project-name>-linux-aarch64-musl
            cflags: "-U_FORTIFY_SOURCE"
            cc: "aarch64-linux-gnu-gcc"
          - os: ubuntu-latest
            target: i686-unknown-linux-musl
            artifact_name: <project-name>
            asset_name: <project-name>-linux-i686-musl
          - os: ubuntu-latest
            target: armv7-unknown-linux-musleabihf
            artifact_name: <project-name>
            asset_name: <project-name>-linux-armv7-musl
          # Windows builds
          - os: windows-latest
            target: x86_64-pc-windows-msvc
            artifact_name: <project-name>.exe
            asset_name: <project-name>-windows-x86_64.exe
            rustflags: "-C target-feature=+crt-static"
            opt_level: "3"
            lto: "thin"
            strip: "false"
            codegen_units: "16"
            panic: "unwind"
          - os: windows-latest
            target: i686-pc-windows-msvc
            artifact_name: <project-name>.exe
            asset_name: <project-name>-windows-i686.exe
            rustflags: "-C target-feature=+crt-static"
            opt_level: "3"
            lto: "thin"
            strip: "false"
            codegen_units: "16"
            panic: "unwind"
          - os: windows-latest
            target: aarch64-pc-windows-msvc
            artifact_name: <project-name>.exe
            asset_name: <project-name>-windows-aarch64.exe
            rustflags: "-C target-feature=+crt-static"
            opt_level: "3"
            lto: "thin"
            strip: "false"
            codegen_units: "16"
            panic: "unwind"
          # macOS builds
          - os: macos-15-intel
            target: x86_64-apple-darwin
            artifact_name: <project-name>
            asset_name: <project-name>-macos-x86_64
          - os: macos-latest
            target: aarch64-apple-darwin
            artifact_name: <project-name>
            asset_name: <project-name>-macos-aarch64

    steps:
      - name: Checkout code
        uses: actions/checkout@v4

      - name: Setup Rust
        uses: dtolnay/rust-toolchain@stable
        with:
          targets: ${{ matrix.target }}

      - name: Install cross (for musl targets)
        if: matrix.target == 'x86_64-unknown-linux-musl' || matrix.target == 'aarch64-unknown-linux-musl' || matrix.target == 'i686-unknown-linux-musl' || matrix.target == 'armv7-unknown-linux-musleabihf'
        run: cargo install cross --git https://github.com/cross-rs/cross

      - name: Install 32-bit libraries (Linux i686 only)
        if: matrix.target == 'i686-unknown-linux-gnu'
        run: |
          sudo dpkg --add-architecture i386
          sudo apt-get update
          sudo apt-get install -y gcc-multilib g++-multilib

      - name: Install ARM cross-compilation tools (ARM targets only)
        if: matrix.target == 'armv7-unknown-linux-gnueabihf'
        run: |
          sudo apt-get update
          sudo apt-get install -y gcc-arm-linux-gnueabihf g++-arm-linux-gnueabihf

      - name: Install ARM64 cross-compilation tools (ARM64 targets only)
        if: matrix.target == 'aarch64-unknown-linux-gnu'
        run: |
          sudo apt-get update
          sudo apt-get install -y gcc-aarch64-linux-gnu g++-aarch64-linux-gnu

      - name: Configure cargo for ARM cross-compilation
        if: matrix.target == 'armv7-unknown-linux-gnueabihf' || matrix.target == 'aarch64-unknown-linux-gnu'
        run: |
          mkdir -p ~/.cargo
          if [ "${{ matrix.target }}" = "armv7-unknown-linux-gnueabihf" ]; then
            echo '[target.armv7-unknown-linux-gnueabihf]' >> ~/.cargo/config.toml
            echo 'linker = "arm-linux-gnueabihf-gcc"' >> ~/.cargo/config.toml
          elif [ "${{ matrix.target }}" = "aarch64-unknown-linux-gnu" ]; then
            echo '[target.aarch64-unknown-linux-gnu]' >> ~/.cargo/config.toml
            echo 'linker = "aarch64-linux-gnu-gcc"' >> ~/.cargo/config.toml
          fi

      - name: Build (Windows)
        if: matrix.os == 'windows-latest'
        run: cargo build --release --target ${{ matrix.target }}
        env:
          RUSTFLAGS: ${{ matrix.rustflags }}
          CARGO_PROFILE_RELEASE_OPT_LEVEL: ${{ matrix.opt_level }}
          CARGO_PROFILE_RELEASE_LTO: ${{ matrix.lto }}
          CARGO_PROFILE_RELEASE_STRIP: ${{ matrix.strip }}
          CARGO_PROFILE_RELEASE_CODEGEN_UNITS: ${{ matrix.codegen_units }}
          CARGO_PROFILE_RELEASE_PANIC: ${{ matrix.panic }}

      - name: Build with cross (musl targets)
        if: matrix.target == 'x86_64-unknown-linux-musl' || matrix.target == 'aarch64-unknown-linux-musl' || matrix.target == 'i686-unknown-linux-musl' || matrix.target == 'armv7-unknown-linux-musleabihf'
        run: cross build --release --target ${{ matrix.target }}
        env:
          RUSTFLAGS: ${{ matrix.rustflags }}
          CFLAGS: ${{ matrix.cflags }}
          CC: ${{ matrix.cc }}

      - name: Build (Linux and macOS)
        if: matrix.os != 'windows-latest' && matrix.target != 'x86_64-unknown-linux-musl' && matrix.target != 'aarch64-unknown-linux-musl' && matrix.target != 'i686-unknown-linux-musl' && matrix.target != 'armv7-unknown-linux-musleabihf'
        run: cargo build --release --target ${{ matrix.target }}
        env:
          RUSTFLAGS: ${{ matrix.rustflags }}
          CFLAGS: ${{ matrix.cflags }}
          CC: ${{ matrix.cc }}

      - name: Strip binary (Linux and macOS - x86)
        if: matrix.os != 'windows-latest' && matrix.target != 'armv7-unknown-linux-gnueabihf' && matrix.target != 'aarch64-unknown-linux-gnu' && matrix.target != 'aarch64-unknown-linux-musl' && matrix.target != 'i686-unknown-linux-musl' && matrix.target != 'armv7-unknown-linux-musleabihf'
        run: strip target/${{ matrix.target }}/release/${{ matrix.artifact_name }}

      - name: Strip binary (ARM32)
        if: matrix.target == 'armv7-unknown-linux-gnueabihf'
        run: arm-linux-gnueabihf-strip target/${{ matrix.target }}/release/${{ matrix.artifact_name }}

      - name: Strip binary (ARM64)
        if: matrix.target == 'aarch64-unknown-linux-gnu'
        run: aarch64-linux-gnu-strip target/${{ matrix.target }}/release/${{ matrix.artifact_name }}

      - name: Strip binary (cross-compiled musl)
        if: matrix.target == 'x86_64-unknown-linux-musl' || matrix.target == 'aarch64-unknown-linux-musl' || matrix.target == 'i686-unknown-linux-musl' || matrix.target == 'armv7-unknown-linux-musleabihf'
        run: echo "Skipping strip for cross-compiled ${{ matrix.target }} (already stripped by cross)"

      - name: Create tarball (Linux and macOS)
        if: matrix.os != 'windows-latest'
        run: |
          cd target/${{ matrix.target }}/release
          tar czf ${{ matrix.asset_name }}.tar.gz ${{ matrix.artifact_name }}
          mv ${{ matrix.asset_name }}.tar.gz ../../../

      - name: Upload artifacts (Linux and macOS)
        if: matrix.os != 'windows-latest'
        uses: actions/upload-artifact@v4
        with:
          name: ${{ matrix.asset_name }}
          path: ${{ matrix.asset_name }}.tar.gz

      - name: Upload artifacts (Windows)
        if: matrix.os == 'windows-latest'
        uses: actions/upload-artifact@v4
        with:
          name: ${{ matrix.asset_name }}
          path: target/${{ matrix.target }}/release/${{ matrix.artifact_name }}

  release:
    name: Create Release
    needs: build
    runs-on: ubuntu-latest
    steps:
      - name: Checkout code
        uses: actions/checkout@v4
        with:
          fetch-depth: 0
          ref: ${{ github.event.inputs.version || github.ref }}

      - name: Download all artifacts
        uses: actions/download-artifact@v4
        with:
          path: artifacts

      - name: Display structure
        run: |
          echo "Current directory structure:"
          ls -la
          echo "Artifacts directory:"
          ls -la artifacts/
          echo "Looking for artifacts:"
          find artifacts -type f \( -name "*.tar.gz" -o -name "*.zip" -o -name "*.exe" \)

      - name: Prepare release files
        run: |
          mkdir -p release_files
          find artifacts -type f -name "*.tar.gz" -exec cp {} release_files/ \;
          for dir in artifacts/*; do
            if [ -d "$dir" ] && [[ "$dir" == *"windows"* ]]; then
              asset_name=$(basename "$dir")
              exe_file="$dir/<project-name>.exe"
              if [ -f "$exe_file" ]; then
                cp "$exe_file" "release_files/$asset_name"
              fi
            fi
          done
          echo "Files in release_files:"
          ls -la release_files/

      - name: Generate checksums
        run: |
          cd release_files
          if [ -n "$(ls -A)" ]; then
            sha256sum * > SHA256SUMS
            echo "Checksums generated:"
            cat SHA256SUMS
          else
            echo "No files found in release_files directory!"
            exit 1
          fi

      - name: Get tag message
        id: tag_message
        run: |
          if [ -n "${{ github.event.inputs.version }}" ]; then
            TAG_NAME="${{ github.event.inputs.version }}"
          else
            TAG_NAME=${GITHUB_REF#refs/tags/}
          fi
          echo "tag_name=$TAG_NAME" >> $GITHUB_OUTPUT
          TAG_MESSAGE=$(git tag -l --format='%(contents)' "$TAG_NAME")
          if [ -z "$TAG_MESSAGE" ]; then
            TAG_MESSAGE="Release $TAG_NAME"
          fi
          echo "message<<EOF" >> $GITHUB_OUTPUT
          echo "$TAG_MESSAGE" >> $GITHUB_OUTPUT
          echo "EOF" >> $GITHUB_OUTPUT

      - name: Create Release
        uses: softprops/action-gh-release@v1
        with:
          tag_name: ${{ steps.tag_message.outputs.tag_name }}
          files: release_files/*
          draft: false
          prerelease: false
          body: |
            ${{ steps.tag_message.outputs.message }}

            ---

            ## 📦 Downloads

            Please download the appropriate version for your system from below.

            ## 🔒 File Verification

            Use the SHA256SUMS file to verify the integrity of downloaded files.

            ---

            ## 📝 Auto-generated Changelog
          generate_release_notes: true
        env:
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
```

> **重要**：生成此文件時，將所有 `<project-name>` 替換為實際的項目名稱（小寫）。

### 可選文件（詢問用戶）

**PRIVACY.md**（若用戶選擇）：

```markdown
# Privacy Policy

This application does not collect, store, or transmit any personal data.

Last updated: <YEAR>-<MONTH>-<DAY>
```

---

## 步驟 4：建立 GitHub Repo 並推送

在生成骨架文件前，先詢問用戶：

1. **Repo 描述**（用於 `gh repo create` 的 `--description` 和 README.md）
2. **可見性**：Public（預設）還是 Private？

```bash
PROJECT_NAME=$(basename "$PWD")

gh repo create "$PROJECT_NAME" \
  --public \
  --description "<user-provided description>" \
  --source=. \
  --remote=origin \
  --push
```

- 若用戶選擇 Private，將 `--public` 替換為 `--private`
- 推送後顯示 Repo URL

**執行順序**：

1. 詢問用戶描述和可見性
2. 生成骨架文件（使用描述填入 README.md）
3. 執行初始 commit（`git add -A && git commit -m "chore: initial commit"`）
4. 建立 GitHub Repo 並推送

---

## 步驟 5：Gist 流程（參數為 gist 時）

```bash
# 顯示當前目錄文件供用戶選擇
ls -la

# 詢問用戶：
# 1. 要上傳哪些文件（預設：所有非隱藏文件）
# 2. Gist 描述
# 3. Public 還是 Secret？（預設：Public）

gh gist create <files> --desc "<description>" --public
# 或
gh gist create <files> --desc "<description>"  # secret 時省略 --public
```

推送後顯示 Gist URL。

---

## 步驟 6：完成摘要

```
✓ Git 倉庫已初始化
✓ 骨架文件已生成：README.md, CHANGELOG.md, LICENSE, .gitignore [, .github/workflows/release.yml]
✓ GitHub Repo 已建立：https://github.com/superyngo/<project-name>
✓ 初始提交已推送到 main

後續步驟：
- 編輯 README.md 填入完整的項目描述
- 準備好發布版本時使用 /git-release
```

---

## 錯誤處理

- `gh` CLI 未安裝 → 提示用戶安裝：`brew install gh` 或訪問 https://cli.github.com
- `gh` 未認證 → 提示用戶運行 `gh auth login`
- Repo 名稱已存在 → 告知用戶並詢問是否使用不同名稱或設置現有 Repo 為遠端
- git 操作失敗 → 顯示錯誤訊息並停止流程

---

## 使用範例

```bash
# 為當前目錄建立 GitHub Repo（自動檢測類型）
/github-init

# 明確指定建立 Repo
/github-init repo

# 將當前目錄文件上傳為 Gist
/github-init gist
```
