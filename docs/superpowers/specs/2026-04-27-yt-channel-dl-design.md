# yt-channel-dl Skill Design

**Date:** 2026-04-27  
**Status:** Approved

---

## Problem Statement

Download all videos from a YouTube channel as audio files (MP3 by default, other formats optional). Needs resume support, humanized crawl behavior to avoid rate-limiting/blocking, and automatic detection of Python invocation method (uv vs python3).

---

## Skill Structure

```
skills/yt-channel-dl/
  SKILL.md
  scripts/
    detect_python.py
    download_channel.py
```

### `SKILL.md` Frontmatter

```yaml
name: yt-channel-dl
description: 從 YouTube 頻道 URL 下載所有影片音頻為 MP3（或其他格式），支援斷點續傳與擬人化爬取
argument-hint: [channel_url] [output_dir] [--format mp3|aac|m4a|flac]
allowed-tools: Bash, AskUserQuestion
```

---

## Components

### `scripts/detect_python.py`

Detects the available Python runner and tool availability. Outputs JSON:

```json
{
  "runner": "uv|python3|python",
  "uv_available": true,
  "has_yt_dlp": true,
  "has_ffmpeg": true,
  "ffmpeg_path": "/usr/bin/ffmpeg"
}
```

**Detection order:**
1. Check `uv` via `shutil.which("uv")` → if found, runner = `"uv"` (uses `uv run --with yt-dlp`)
2. Check `python3 -c "import yt_dlp"` → if exits 0, runner = `"python3"`
3. Check `python -c "import yt_dlp"` → if exits 0, runner = `"python"`
4. If none found: output `{"runner": null, "has_yt_dlp": false}` and print install instructions to stderr

`has_ffmpeg` checked via `shutil.which("ffmpeg")`.

---

### `scripts/download_channel.py`

**CLI signature:**
```
python3 download_channel.py <channel_url> <output_dir>
    [--format mp3|aac|m4a|flac]   default: mp3
    [--workers 1]                  concurrent downloads (default: 1, serial)
    [--rate-limit 500K]            yt-dlp download speed cap
    [--min-sleep 2] [--max-sleep 8]  per-video delay range (seconds)
    [--burst 10] [--burst-sleep 45]  rest N seconds after every N videos
```

#### Step 1 — Fetch video list (flat extraction)

Use `yt_dlp.YoutubeDL` with `extract_flat="in_playlist"` and `quiet=True` to fetch all entries without downloading. This returns video IDs and titles only.

For paginated channels, yt-dlp handles pagination internally. A random `sleep(1–3s)` is applied after the extraction call before proceeding.

#### Step 2 — Filter already downloaded

Scan `output_dir` for files matching the pattern `*[<video_id>]*`. Any video whose ID already appears in a filename is skipped. A `.yt-channel-dl.json` state file in `output_dir` caches the downloaded ID set for faster lookups on large libraries. Format: `{"downloaded_ids": ["abc123", "def456", ...]}`. The script reads this on startup, merges with filesystem scan results, and appends to it after each successful download.

#### Step 3 — Download loop

For each remaining video:

1. `sleep(random.uniform(min_sleep, max_sleep))` — per-video humanization
2. Every `burst` videos: additional `sleep(random.uniform(burst_sleep * 0.8, burst_sleep * 1.2))`
3. Randomly select a User-Agent from a pool of 5 common desktop browser UA strings
4. Invoke yt-dlp `YoutubeDL.download([url])` with options:
   - `format`: `bestaudio/best`
   - `outtmpl`: `<output_dir>/%(title)s [%(id)s].%(ext)s`
   - `http_headers`: `{"User-Agent": <selected_ua>}`
   - `ratelimit`: parsed from `--rate-limit` arg
   - `retries`: 3 (exponential backoff handled by yt-dlp)
   - If ffmpeg available: `postprocessors` → `FFmpegExtractAudio` with `preferredcodec` = format
   - If no ffmpeg and format = `mp3`: warn and fall back to `m4a`/`bestaudio`
5. Progress hook writes to tqdm progress bar (title, % complete, speed, ETA)
6. On success: append video ID to state file
7. On failure after retries: log to `failed_list` (video ID + title + error message)

#### Step 4 — Summary output

```
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
✓ Downloaded: 47
⏭  Skipped (already exists): 12
✗ Failed: 2
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
Failed videos:
  - "Video Title 1" [abc123xyz] — HTTP Error 429
  - "Video Title 2" [def456uvw] — Video unavailable
```

---

### `SKILL.md` Claude Operating Procedure

1. **Collect arguments** — if `channel_url` not provided as argument, ask user. Ask for `output_dir` (no default). Ask for format (default mp3). Workers default to 1.
2. **Run detection** — `python3 scripts/detect_python.py`, parse JSON.
3. **Handle missing dependencies:**
   - `runner == null`: tell user to install yt-dlp (`pip install yt-dlp` or `uv tool install yt-dlp`) and re-run.
   - `has_ffmpeg == false` and format == `mp3`: warn user ffmpeg is needed for MP3 conversion. Offer to continue with `m4a` instead, or ask user to install ffmpeg.
4. **Build runner command:**
   - `uv`: `uv run --with yt-dlp python3 scripts/download_channel.py ...`
   - `python3`/`python`: `python3 scripts/download_channel.py ...` (yt-dlp already importable)
5. **Execute** and stream output to terminal.
6. **On completion**, display the final summary.

---

## Humanization Details

| Technique | Implementation |
|---|---|
| Per-video random delay | `sleep(uniform(min_sleep, max_sleep))` — default 2–8 s |
| Burst rest | After every 10 videos: `sleep(uniform(36, 54))` |
| User-Agent rotation | Pool of 5 modern desktop browser UA strings, random choice per download |
| Download rate cap | yt-dlp `ratelimit` option (default 500K/s) |
| Auto-retry | yt-dlp built-in retries=3 with back-off |

---

## Error Handling

- Missing `channel_url`: ask user before proceeding.
- Invalid URL (yt-dlp extraction fails): surface error message and exit gracefully.
- `output_dir` does not exist: create it automatically.
- Individual video failure: log to failed list, continue with remaining videos.
- Rate-limited (HTTP 429): yt-dlp retry + exponential backoff; if still failing, add to failed list.
- No ffmpeg + MP3 requested: warn and offer m4a fallback instead of aborting.

---

## Non-Goals

- No video file retention (audio-only output).
- No playlist support beyond the channel level (channel URL covers all uploads).
- No GUI.
- No concurrent downloads in initial version (`--workers` accepted but defaults to 1).
