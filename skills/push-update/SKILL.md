---
name: push-update
description: Use when ready to commit and push changes to remote, or when preparing to release a new version with changelog and tag management
argument-hint: [version]
allowed-tools: Read, Write, Edit, Bash, Grep, AskUserQuestion, mcp__github__create_pull_request, mcp__github__list_tags, mcp__github__get_latest_release
---

# 推送更新流程

執行推送更新到遠端的完整流程。

---

## 步驟 1：確認與遠端同步無衝突

在進行任何操作前，先確保本地與遠端同步。

### 1.1 獲取遠端最新資訊

```bash
git fetch origin
```

### 1.2 檢查同步狀態

```bash
git status
```

檢查以下情況：
- 「Your branch is behind」→ 本地落後
- 「Your branch has diverged」→ 分支分歧
- 「Your branch is ahead」→ 可以推送

### 1.3 如果有差異，顯示詳情

**顯示遠端領先的提交：**
```bash
git log --oneline HEAD..origin/main
```

**顯示變更統計：**
```bash
git diff HEAD...origin/main --stat
```

**如需要，顯示詳細差異：**
```bash
git diff HEAD...origin/main
```

### 1.4 讓用戶決定處理方式

詢問用戶：
- **Rebase（推薦）**：`git pull --rebase origin main`
- **Merge**：`git pull origin main`
- **取消流程**

如果同步過程中有衝突，停止流程並顯示衝突訊息。

---

## 步驟 2：執行代碼品質審查

**重點**：在提交之前執行，確保只提交高品質代碼。

### 2.1 自動檢測專案類型

按以下順序檢測：

| 檔案 | 專案類型 |
|------|----------|
| `Cargo.toml` | Rust |
| `package.json` | Node.js |
| `pyproject.toml` 或 `setup.py` | Python |
| `go.mod` | Go |

### 2.2 Rust 專案檢查

```bash
# 1. 格式化（自動修復）
cargo fmt

# 2. Clippy 檢查
cargo clippy -- -D warnings
# 如有問題，嘗試：cargo clippy --fix --allow-dirty

# 3. 編譯檢查
cargo check

# 4. 測試
cargo test
```

### 2.3 Node.js 專案檢查

```bash
# 1. 格式化（如有 prettier）
npx prettier --write . 2>/dev/null || true

# 2. ESLint 檢查（自動修復）
npx eslint --fix . 2>/dev/null || true

# 3. 類型檢查（如有 TypeScript）
npx tsc --noEmit 2>/dev/null || true

# 4. 測試
npm test 2>/dev/null || true
```

### 2.4 Python 專案檢查

```bash
# 1. 格式化
black . 2>/dev/null || true
isort . 2>/dev/null || true

# 2. Linting（優先使用 ruff）
ruff check --fix . 2>/dev/null || flake8 . 2>/dev/null || true

# 3. 類型檢查（如有設定）
mypy . 2>/dev/null || true

# 4. 測試
pytest 2>/dev/null || true
```

### 2.5 Go 專案檢查

```bash
# 1. 格式化
gofmt -w .

# 2. Linting
golangci-lint run 2>/dev/null || true

# 3. 測試
go test ./...
```

### 2.6 檢查結果處理

- 所有檢查通過：顯示「✓ 代碼品質檢查通過」
- 如有自動修復的更改，這些更改會在步驟 3 一併提交
- 如有無法自動修復的錯誤，詢問用戶是否繼續

---

## 步驟 3：提交所有更新

### 3.1 顯示所有變更

```bash
git status
git diff --stat
```

### 3.2 讓用戶確認

- 顯示變更摘要
- 建議 commit message（根據變更內容）
- 讓用戶確認或修改 commit message

### 3.3 執行提交

```bash
git add -A
git commit -m "<message>"
```

---

## 步驟 4：詢問用戶是否發佈新版

### 4.1 分析提交歷史

```bash
# 獲取最新的 tag
LAST_TAG=$(git describe --tags --abbrev=0 2>/dev/null || echo "")

# 如果有上次 tag，顯示自那之後的提交
if [ -n "$LAST_TAG" ]; then
  git log $LAST_TAG..HEAD --pretty=format:"%s"
else
  # 沒有 tag，顯示所有提交
  git log --pretty=format:"%s"
fi
```

### 4.2 根據 Conventional Commits 建議版號

分析提交訊息：
- 包含 `BREAKING CHANGE` 或 `!:` → **major** 升級 (X.0.0)
- 包含 `feat:` → **minor** 升級 (x.Y.0)
- 僅包含 `fix:`, `chore:`, `docs:`, `refactor:` 等 → **patch** 升級 (x.y.Z)

### 4.3 顯示建議

```
上次版本：v1.2.3
變更摘要：
- 3 個新功能 (feat)
- 2 個修復 (fix)
- 1 個文件更新 (docs)

建議新版本：v1.3.0 (minor 升級，因為有新功能)
```

### 4.4 讓用戶選擇

詢問用戶：

```
是否要發佈新版本？
[1] 是，使用建議版本 v1.3.0
[2] 是，自訂版本號
[3] 否，只推送不發版
```

如果用戶選擇 [3]（不發版）：
- 直接推送到遠端
- 流程結束

---

## 步驟 5：版本發布流程

如果用戶選擇發布新版本，依序執行：

### 5.1 統一版號格式

- 支援輸入 `v1.2.3` 或 `1.2.3`
- 內部統一使用帶 `v` 前綴格式

### 5.2 更新專案設定檔版號

根據檢測到的專案類型：

**Rust (Cargo.toml)**：
```toml
version = "x.y.z"  # 不帶 v 前綴
```

**Node.js (package.json)**：
```json
"version": "x.y.z"  // 不帶 v 前綴
```

**Python (pyproject.toml)**：
```toml
version = "x.y.z"  # 不帶 v 前綴
```

### 5.3 整理 CHANGELOG.md

將 `## [Unreleased]` 區塊轉為新版本區塊：

```markdown
## [vX.Y.Z] - YYYY-MM-DD

### Added
- 新功能列表（從 feat: 提取）

### Changed
- 變更列表（從 refactor:, chore: 提取）

### Fixed
- 修復列表（從 fix: 提取）

### Docs
- 文件更新（從 docs: 提取）
```

保留空的 `## [Unreleased]` 區塊供未來使用。

### 5.4 更新 README.md

- 更新版本徽章（如有）
- 更新版本號引用（如有）

### 5.5 提交文件更改

```bash
git add -A
git commit -m "chore: release vX.Y.Z"
```

### 5.6 建立 Git Tag

```bash
git tag -a vX.Y.Z -m "Release vX.Y.Z

<release notes content>"
```

### 5.7 推送到遠端

```bash
git push origin main
git push origin --tags
```

### 5.8 檢查遠端 Release Action

推送 tag 後，檢查專案是否有自動創建 Release 的 GitHub Actions：

```bash
# 檢查是否存在 release workflow
if [ -f .github/workflows/release.yml ]; then
  echo "✓ 偵測到遠端 release workflow，GitHub Actions 將自動創建 Release"
  echo "→ 跳過本地創建 Release 步驟"
  SKIP_LOCAL_RELEASE=true
else
  echo "✓ 未偵測到自動 release workflow"
  SKIP_LOCAL_RELEASE=false
fi
```

如果 `SKIP_LOCAL_RELEASE=true`：
- 顯示訊息告知用戶 Release 將由 GitHub Actions 自動創建
- 提供 Actions 頁面連結讓用戶追蹤進度：`https://github.com/<owner>/<repo>/actions`
- 跳過步驟 5.9

如果 `SKIP_LOCAL_RELEASE=false`：
- 繼續執行步驟 5.9 本地創建 Release

### 5.9 建立 GitHub Release（僅當無自動 workflow 時）

**此步驟僅在專案沒有自動 release workflow 時執行。**

**工具優先順序**：

1. **GitHub MCP Server**（優先）
   - 使用 MCP 工具建立 Release

2. **gh CLI**（備選）
   ```bash
   gh release create vX.Y.Z \
     --title "Release vX.Y.Z" \
     --notes "<release notes>"
   ```

3. **提示用戶手動創建**（最後選項）
   ```
   請手動到 GitHub 建立 Release：
   https://github.com/<owner>/<repo>/releases/new?tag=vX.Y.Z
   ```

使用收集的 release notes 作為描述，標記為最新版本。

### 5.10 顯示結果

**如果有自動 release workflow：**
```
✓ 專案設定檔版號已更新
✓ CHANGELOG.md 已更新
✓ README.md 已更新（如適用）
✓ 已建立 tag vX.Y.Z
✓ 已推送到遠端

→ GitHub Actions 將自動創建 Release
→ 追蹤進度：https://github.com/user/repo/actions
→ 完成後查看：https://github.com/user/repo/releases/tag/vX.Y.Z
```

**如果無自動 workflow（本地創建）：**
```
✓ 專案設定檔版號已更新
✓ CHANGELOG.md 已更新
✓ README.md 已更新（如適用）
✓ 已建立 tag vX.Y.Z
✓ 已推送到遠端
✓ 已建立 GitHub Release

發布連結：https://github.com/user/repo/releases/tag/vX.Y.Z
```

---

## 錯誤處理

- 如果 git 操作失敗，顯示錯誤訊息並停止流程
- 如果沒有自動 release workflow 且 GitHub MCP 不可用，自動降級到 gh CLI
- 如果 gh CLI 不可用，降級到提示用戶手動創建
- 如果檔案更新失敗，詢問用戶是否繼續

---

## 使用範例

```bash
# 快速推送（自動檢查、提交、詢問是否發版）
/push-update

# 直接指定版本號發布
/push-update v1.3.0

# 發布補丁版本
/push-update v1.2.1
```

---

## 補充說明

### 版本號格式

建議使用語義化版本 (Semantic Versioning)：
- **主版本號** (X.0.0)：不相容的 API 修改
- **次版本號** (x.Y.0)：向下相容的功能性新增
- **修訂號** (x.y.Z)：向下相容的問題修正

### Conventional Commits 類型對應

| 類型 | 說明 | 版本影響 |
|------|------|----------|
| `feat:` | 新功能 | minor |
| `fix:` | 修復 | patch |
| `docs:` | 文件 | patch |
| `style:` | 格式 | patch |
| `refactor:` | 重構 | patch |
| `perf:` | 效能 | patch |
| `test:` | 測試 | patch |
| `chore:` | 雜項 | patch |
| `BREAKING CHANGE` | 破壞性更改 | major |

### 分支保護

如果 main 分支有保護規則，可能無法直接推送。

### 權限要求

確保有推送權限和建立 Release 的權限。

### 自動 Release Workflow 檢測

- 推送 tag 後會自動檢查 `.github/workflows/release.yml` 是否存在
- 如果存在，表示專案有自動創建 Release 的 GitHub Actions workflow
- 此時會跳過本地創建 Release 的步驟，避免重複
- 常見的自動 release workflow 觸發條件：
  ```yaml
  on:
    push:
      tags:
        - "v*.*.*"
  ```
- 如果您的專案使用不同的檔名（如 `ci.yml`、`build.yml`），可能需要手動調整檢測邏輯

---

## 未來擴展

### dry-run 模式

可選參數 `--dry-run`，預覽所有操作但不實際執行。

### Monorepo 支援

未來可擴展支援同時更新多個子專案的版號。
