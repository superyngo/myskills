---
name: yt-channel-dl
description: 從 YouTube 頻道 URL 下載所有影片音頻為 MP3（或其他格式），支援斷點續傳與擬人化爬取
argument-hint: "[channel_url] [output_dir] [--format mp3|aac|m4a|flac]"
allowed-tools: Bash, AskUserQuestion
---

# YouTube Channel Audio Downloader

從 YouTube 頻道下載所有影片音頻，輸出為 MP3（預設）、AAC、M4A 或 FLAC。  
支援斷點續傳（自動跳過已下載）與擬人化爬取（隨機延遲 + User-Agent 輪換）。

---

## 步驟 1：收集參數

從第一個 argument 取得 `channel_url`。若未提供，詢問：

> 請提供 YouTube 頻道 URL  
> 例如：`https://www.youtube.com/@ChannelName` 或 `https://www.youtube.com/channel/UCxxxxxx`

從第二個 argument 取得 `output_dir`。若未提供，詢問：

> 請提供音頻檔案儲存路徑（例如：`~/Music/ChannelName`）

從 `--format` argument 取得格式，預設 `mp3`。

---

## 步驟 2：偵測執行環境

```bash
python3 skills/yt-channel-dl/scripts/detect_python.py
```

解析 JSON 輸出，依情況處理：

**若 `runner == null`（找不到 yt-dlp）：**
```
找不到 yt-dlp，請先安裝：
  pip install yt-dlp
  或：uv tool install yt-dlp
安裝後重新執行此 skill。
```
→ 停止流程。

**若 `js_runtime == null`（找不到 Node/Bun/Deno）：**
```
yt-dlp 需要 JavaScript runtime 才能解析 YouTube。
請安裝以下任一：
  1. Node.js   — macOS: brew install node | 其他: https://nodejs.org
  2. Bun       — macOS: brew install bun  | 其他: curl -fsSL https://bun.sh/install | bash
  3. Deno      — macOS: brew install deno | 其他: curl -fsSL https://deno.land/install.sh | sh
```
詢問用戶選擇並執行安裝指令，安裝後重新執行偵測。

**若 `has_ffmpeg == false` 且 format 為 `mp3`、`aac` 或 `flac`：**
```
警告：未偵測到 ffmpeg，無法轉換為 MP3。
選項：
  1. 改用 m4a（原生音頻，品質相同，不需轉換）
  2. 安裝 ffmpeg 後重新執行
     macOS:   brew install ffmpeg
     Ubuntu:  sudo apt install ffmpeg
     Windows: winget install ffmpeg
```
詢問用戶選擇，若選 1 則將 format 改為 `m4a`，若選 2 則停止等待安裝。

---

## 步驟 3：執行下載

根據偵測結果的 `runner` 值組裝指令：

**runner = `"uv"`：**
```bash
uv run --with yt-dlp python3 skills/yt-channel-dl/scripts/download_channel.py \
  "<channel_url>" "<output_dir>" \
  --format <fmt> \
  --rate-limit 500K \
  [--ffmpeg-path "<ffmpeg_path>" # 若 ffmpeg_path 不為 null]
```

**runner = `"python3"` 或 `"python"`：**
```bash
python3 skills/yt-channel-dl/scripts/download_channel.py \
  "<channel_url>" "<output_dir>" \
  --format <fmt> \
  --rate-limit 500K \
  [--ffmpeg-path "<ffmpeg_path>" # 若 ffmpeg_path 不為 null]
```

（若 `runner` 是 `"python"`，將 `python3` 替換為 `python`）

執行後讓輸出串流到終端，等待完成。

---

## 步驟 4：顯示結果

下載完成後，summary 已由腳本直接列印。若有失敗項目，告知用戶：

> 有 N 個影片下載失敗（詳見上方清單）。  
> 重新執行相同指令可自動跳過已下載的部分，僅重試失敗的影片。

---

## 參數說明

| 參數 | 預設值 | 說明 |
|---|---|---|
| `channel_url` | — | YouTube 頻道/playlist URL |
| `output_dir` | — | 輸出目錄（自動建立） |
| `--format` | `mp3` | 音頻格式：mp3 / aac / m4a / flac |
| `--rate-limit` | `500K` | 下載限速（例：`1M`、`500K`） |
| `--min-sleep` | `2` | 每影片最短延遲秒數 |
| `--max-sleep` | `8` | 每影片最長延遲秒數 |
| `--burst` | `10` | 每 N 部影片後休息 |
| `--burst-sleep` | `45` | 休息秒數（±20% 隨機） |

---

## 擬人化機制說明

- **每部影片前**隨機等待 2–8 秒
- **每 10 部影片後**額外休息約 45 秒
- **User-Agent**每次下載隨機輪換（5 個主流瀏覽器）
- **下載限速**預設 500KB/s
- **自動重試**失敗最多 3 次（yt-dlp 內建指數退避）
