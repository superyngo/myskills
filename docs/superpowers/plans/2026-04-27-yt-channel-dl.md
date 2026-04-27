# yt-channel-dl Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a `yt-channel-dl` skill that downloads all videos from a YouTube channel as audio files (MP3 by default), with resume support and humanized crawl behavior.

**Architecture:** A SKILL.md defines the Claude operating procedure; two Python scripts handle environment detection (`detect_python.py`) and the actual download logic (`download_channel.py`). The downloader uses yt-dlp as a library for channel extraction and audio postprocessing, with per-video random delays, User-Agent rotation, and a JSON state file for resume tracking.

**Tech Stack:** Python 3 (stdlib), yt-dlp library, ffmpeg (optional, for MP3 conversion)

**Spec:** `docs/superpowers/specs/2026-04-27-yt-channel-dl-design.md`

---

## File Map

| File | Role |
|---|---|
| `skills/yt-channel-dl/SKILL.md` | Frontmatter + Claude operating procedure |
| `skills/yt-channel-dl/scripts/detect_python.py` | Detect uv/python3/python runner + ffmpeg availability |
| `skills/yt-channel-dl/scripts/download_channel.py` | Channel fetch, humanized download loop, MP3 extraction, summary |
| `skills/yt-channel-dl/tests/test_detect_python.py` | Integration tests for detect_python.py |
| `skills/yt-channel-dl/tests/test_download_channel.py` | Unit tests for pure utility functions |

---

## Task 1: Directory skeleton + SKILL.md frontmatter

**Files:**
- Create: `skills/yt-channel-dl/SKILL.md`
- Create: `skills/yt-channel-dl/scripts/.gitkeep`
- Create: `skills/yt-channel-dl/tests/.gitkeep`

- [ ] **Step 1: Create directory structure**

```bash
mkdir -p skills/yt-channel-dl/scripts
mkdir -p skills/yt-channel-dl/tests
touch skills/yt-channel-dl/scripts/.gitkeep
touch skills/yt-channel-dl/tests/.gitkeep
```

- [ ] **Step 2: Create `skills/yt-channel-dl/SKILL.md` with frontmatter only**

```markdown
---
name: yt-channel-dl
description: 從 YouTube 頻道 URL 下載所有影片音頻為 MP3（或其他格式），支援斷點續傳與擬人化爬取
argument-hint: "[channel_url] [output_dir] [--format mp3|aac|m4a|flac]"
allowed-tools: Bash, AskUserQuestion
---

# YouTube Channel Audio Downloader

_Instructions coming in Task 5._
```

- [ ] **Step 3: Commit**

```bash
git add skills/yt-channel-dl/
git commit -m "feat: scaffold yt-channel-dl skill directory"
```

---

## Task 2: `detect_python.py` — environment detection

**Files:**
- Create: `skills/yt-channel-dl/scripts/detect_python.py`
- Create: `skills/yt-channel-dl/tests/test_detect_python.py`

- [ ] **Step 1: Write the failing test**

Create `skills/yt-channel-dl/tests/test_detect_python.py`:

```python
#!/usr/bin/env python3
"""Integration tests for detect_python.py — runs the script as subprocess."""
import json
import os
import subprocess
import sys
import unittest

SCRIPT = os.path.join(os.path.dirname(__file__), "..", "scripts", "detect_python.py")


class TestDetectPython(unittest.TestCase):
    def _run_script(self):
        result = subprocess.run(
            [sys.executable, SCRIPT],
            capture_output=True,
            text=True,
        )
        return result

    def test_exits_zero(self):
        result = self._run_script()
        self.assertEqual(result.returncode, 0, msg=result.stderr)

    def test_outputs_valid_json(self):
        result = self._run_script()
        data = json.loads(result.stdout)
        self.assertIsInstance(data, dict)

    def test_required_keys_present(self):
        result = self._run_script()
        data = json.loads(result.stdout)
        for key in ("runner", "uv_available", "has_yt_dlp", "has_ffmpeg", "ffmpeg_path"):
            self.assertIn(key, data, msg=f"Missing key: {key}")

    def test_runner_is_valid_value(self):
        result = self._run_script()
        data = json.loads(result.stdout)
        self.assertIn(data["runner"], ["uv", "python3", "python", None])

    def test_uv_available_is_bool(self):
        result = self._run_script()
        data = json.loads(result.stdout)
        self.assertIsInstance(data["uv_available"], bool)

    def test_has_ffmpeg_is_bool(self):
        result = self._run_script()
        data = json.loads(result.stdout)
        self.assertIsInstance(data["has_ffmpeg"], bool)


if __name__ == "__main__":
    unittest.main()
```

- [ ] **Step 2: Run test to confirm it fails**

```bash
cd skills/yt-channel-dl
python3 -m pytest tests/test_detect_python.py -v 2>/dev/null || python3 -m unittest tests/test_detect_python -v
```

Expected: `ModuleNotFoundError` or `FileNotFoundError` (script doesn't exist yet).

- [ ] **Step 3: Write `scripts/detect_python.py`**

```python
#!/usr/bin/env python3
"""Detect available Python runner and tool availability for yt-channel-dl.

Outputs JSON to stdout:
  {"runner": "uv"|"python3"|"python"|null, "uv_available": bool,
   "has_yt_dlp": bool, "has_ffmpeg": bool, "ffmpeg_path": str|null}
"""
import json
import shutil
import subprocess
import sys


def check_uv() -> bool:
    return shutil.which("uv") is not None


def check_yt_dlp(python_cmd: str) -> bool:
    try:
        result = subprocess.run(
            [python_cmd, "-c", "import yt_dlp"],
            capture_output=True,
            timeout=5,
        )
        return result.returncode == 0
    except (FileNotFoundError, subprocess.TimeoutExpired):
        return False


def check_ffmpeg() -> tuple:
    path = shutil.which("ffmpeg")
    return (path is not None), path


def main():
    uv_available = check_uv()
    ffmpeg_found, ffmpeg_path = check_ffmpeg()

    if uv_available:
        result = {
            "runner": "uv",
            "uv_available": True,
            "has_yt_dlp": True,  # uv installs yt-dlp on demand via --with
            "has_ffmpeg": ffmpeg_found,
            "ffmpeg_path": ffmpeg_path,
        }
    elif check_yt_dlp("python3"):
        result = {
            "runner": "python3",
            "uv_available": False,
            "has_yt_dlp": True,
            "has_ffmpeg": ffmpeg_found,
            "ffmpeg_path": ffmpeg_path,
        }
    elif check_yt_dlp("python"):
        result = {
            "runner": "python",
            "uv_available": False,
            "has_yt_dlp": True,
            "has_ffmpeg": ffmpeg_found,
            "ffmpeg_path": ffmpeg_path,
        }
    else:
        result = {
            "runner": None,
            "uv_available": False,
            "has_yt_dlp": False,
            "has_ffmpeg": ffmpeg_found,
            "ffmpeg_path": ffmpeg_path,
        }
        print(
            "ERROR: yt-dlp not found. Install with:\n"
            "  pip install yt-dlp\n"
            "  or: uv tool install yt-dlp",
            file=sys.stderr,
        )

    print(json.dumps(result, indent=2))


if __name__ == "__main__":
    main()
```

- [ ] **Step 4: Run tests to confirm they pass**

```bash
cd skills/yt-channel-dl
python3 -m unittest tests/test_detect_python -v
```

Expected output (all 6 tests PASS):
```
test_exits_zero ... ok
test_has_ffmpeg_is_bool ... ok
test_outputs_valid_json ... ok
test_required_keys_present ... ok
test_runner_is_valid_value ... ok
test_uv_available_is_bool ... ok
----------------------------------------------------------------------
Ran 6 tests in X.XXXs
OK
```

- [ ] **Step 5: Commit**

```bash
git add skills/yt-channel-dl/scripts/detect_python.py skills/yt-channel-dl/tests/test_detect_python.py
git commit -m "feat(yt-channel-dl): add detect_python.py with environment detection"
```

---

## Task 3: `download_channel.py` — pure utility functions

**Files:**
- Create: `skills/yt-channel-dl/scripts/download_channel.py` (utility functions only, no main yet)
- Create: `skills/yt-channel-dl/tests/test_download_channel.py`

- [ ] **Step 1: Write failing tests**

Create `skills/yt-channel-dl/tests/test_download_channel.py`:

```python
#!/usr/bin/env python3
"""Unit tests for pure utility functions in download_channel.py."""
import json
import os
import sys
import tempfile
import unittest

sys.path.insert(0, os.path.join(os.path.dirname(__file__), "..", "scripts"))

from download_channel import (
    USER_AGENTS,
    build_ydl_opts,
    filter_pending,
    load_state,
    parse_rate_limit,
    save_state,
    scan_existing_ids,
)


class TestParseRateLimit(unittest.TestCase):
    def test_kilobytes(self):
        self.assertEqual(parse_rate_limit("500K"), 512000)

    def test_megabytes(self):
        self.assertEqual(parse_rate_limit("2M"), 2097152)

    def test_none_returns_none(self):
        self.assertIsNone(parse_rate_limit(None))

    def test_raw_bytes(self):
        self.assertEqual(parse_rate_limit("1024"), 1024)

    def test_lowercase(self):
        self.assertEqual(parse_rate_limit("1k"), 1024)


class TestStateFile(unittest.TestCase):
    def test_load_returns_empty_state_when_missing(self):
        with tempfile.TemporaryDirectory() as d:
            state = load_state(d)
            self.assertEqual(state, {"downloaded_ids": []})

    def test_load_returns_empty_on_corrupt_json(self):
        with tempfile.TemporaryDirectory() as d:
            with open(os.path.join(d, ".yt-channel-dl.json"), "w") as f:
                f.write("not json")
            state = load_state(d)
            self.assertEqual(state, {"downloaded_ids": []})

    def test_round_trip(self):
        with tempfile.TemporaryDirectory() as d:
            save_state(d, {"downloaded_ids": ["abc123", "def456"]})
            state = load_state(d)
            self.assertEqual(sorted(state["downloaded_ids"]), ["abc123", "def456"])


class TestScanExistingIds(unittest.TestCase):
    def test_finds_11char_bracketed_ids(self):
        with tempfile.TemporaryDirectory() as d:
            open(os.path.join(d, "Song Title [dQw4w9WgXcQ].mp3"), "w").close()
            open(os.path.join(d, "Another Video [abc1234DEFG].m4a"), "w").close()
            ids = scan_existing_ids(d)
            self.assertIn("dQw4w9WgXcQ", ids)
            self.assertIn("abc1234DEFG", ids)

    def test_ignores_state_file(self):
        with tempfile.TemporaryDirectory() as d:
            with open(os.path.join(d, ".yt-channel-dl.json"), "w") as f:
                json.dump({"downloaded_ids": []}, f)
            ids = scan_existing_ids(d)
            self.assertEqual(ids, set())

    def test_empty_dir(self):
        with tempfile.TemporaryDirectory() as d:
            self.assertEqual(scan_existing_ids(d), set())

    def test_ignores_files_without_bracket_id(self):
        with tempfile.TemporaryDirectory() as d:
            open(os.path.join(d, "no_id_here.mp3"), "w").close()
            ids = scan_existing_ids(d)
            self.assertEqual(ids, set())


class TestFilterPending(unittest.TestCase):
    def test_filters_already_downloaded(self):
        entries = [
            {"id": "abc123", "title": "Video 1"},
            {"id": "def456", "title": "Video 2"},
            {"id": "ghi789", "title": "Video 3"},
        ]
        pending = filter_pending(entries, {"abc123", "ghi789"})
        self.assertEqual(len(pending), 1)
        self.assertEqual(pending[0]["id"], "def456")

    def test_all_new_returns_all(self):
        entries = [{"id": "abc123", "title": "Video 1"}]
        self.assertEqual(filter_pending(entries, set()), entries)

    def test_all_existing_returns_empty(self):
        entries = [{"id": "abc123", "title": "Video 1"}]
        self.assertEqual(filter_pending(entries, {"abc123"}), [])

    def test_empty_entries(self):
        self.assertEqual(filter_pending([], {"abc123"}), [])


class TestBuildYdlOpts(unittest.TestCase):
    def test_mp3_with_ffmpeg_adds_postprocessor(self):
        opts = build_ydl_opts(
            output_dir="/tmp/out",
            fmt="mp3",
            ffmpeg_path="/usr/bin/ffmpeg",
            rate_limit=512000,
            user_agent=USER_AGENTS[0],
        )
        self.assertEqual(opts["format"], "bestaudio/best")
        self.assertIn("postprocessors", opts)
        pp = opts["postprocessors"][0]
        self.assertEqual(pp["key"], "FFmpegExtractAudio")
        self.assertEqual(pp["preferredcodec"], "mp3")
        self.assertEqual(opts["ffmpeg_location"], "/usr/bin/ffmpeg")
        self.assertEqual(opts["ratelimit"], 512000)

    def test_no_ffmpeg_no_postprocessor(self):
        opts = build_ydl_opts(
            output_dir="/tmp/out",
            fmt="mp3",
            ffmpeg_path=None,
            rate_limit=None,
            user_agent=USER_AGENTS[0],
        )
        self.assertNotIn("postprocessors", opts)
        self.assertNotIn("ratelimit", opts)

    def test_outtmpl_uses_output_dir(self):
        opts = build_ydl_opts(
            output_dir="/my/music",
            fmt="m4a",
            ffmpeg_path=None,
            rate_limit=None,
            user_agent=USER_AGENTS[0],
        )
        self.assertTrue(opts["outtmpl"].startswith("/my/music"))

    def test_user_agent_in_headers(self):
        ua = USER_AGENTS[2]
        opts = build_ydl_opts("/tmp", "mp3", None, None, ua)
        self.assertEqual(opts["http_headers"]["User-Agent"], ua)

    def test_retries_set(self):
        opts = build_ydl_opts("/tmp", "mp3", None, None, USER_AGENTS[0])
        self.assertEqual(opts["retries"], 3)

    def test_flac_with_ffmpeg(self):
        opts = build_ydl_opts("/tmp", "flac", "/usr/bin/ffmpeg", None, USER_AGENTS[0])
        self.assertEqual(opts["postprocessors"][0]["preferredcodec"], "flac")


if __name__ == "__main__":
    unittest.main()
```

- [ ] **Step 2: Run tests — confirm they fail**

```bash
cd skills/yt-channel-dl
python3 -m unittest tests/test_download_channel -v 2>&1 | head -5
```

Expected: `ImportError: cannot import name 'parse_rate_limit' from 'scripts.download_channel'` (or similar).

- [ ] **Step 3: Write `scripts/download_channel.py` — utility functions only**

```python
#!/usr/bin/env python3
"""Download all audio from a YouTube channel.

Usage:
    python3 download_channel.py <channel_url> <output_dir>
        [--format mp3|aac|m4a|flac]
        [--rate-limit 500K]
        [--min-sleep 2] [--max-sleep 8]
        [--burst 10] [--burst-sleep 45]
        [--ffmpeg-path /path/to/ffmpeg]
"""
import argparse
import json
import os
import random
import re
import sys
import time
from pathlib import Path

try:
    import yt_dlp
except ImportError:
    print(
        "ERROR: yt-dlp not installed. Run detect_python.py for install instructions.",
        file=sys.stderr,
    )
    sys.exit(1)

STATE_FILE = ".yt-channel-dl.json"

USER_AGENTS = [
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36",
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36",
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:125.0) Gecko/20100101 Firefox/125.0",
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 14_4_1) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.4.1 Safari/605.1.15",
    "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36",
]


def parse_rate_limit(s):
    """Convert '500K' or '2M' to bytes/s integer. Returns None if s is None."""
    if s is None:
        return None
    s = s.strip().upper()
    if s.endswith("K"):
        return int(float(s[:-1]) * 1024)
    if s.endswith("M"):
        return int(float(s[:-1]) * 1024 * 1024)
    return int(s)


def load_state(output_dir):
    """Load downloaded ID list from state file. Returns empty state if missing/corrupt."""
    path = Path(output_dir) / STATE_FILE
    try:
        with open(path) as f:
            return json.load(f)
    except (FileNotFoundError, json.JSONDecodeError):
        return {"downloaded_ids": []}


def save_state(output_dir, state):
    """Write state dict to JSON file in output_dir."""
    path = Path(output_dir) / STATE_FILE
    with open(path, "w") as f:
        json.dump(state, f, indent=2)


def scan_existing_ids(output_dir):
    """Scan output_dir filenames for [video_id] patterns (11-char YouTube IDs)."""
    pattern = re.compile(r"\[([A-Za-z0-9_\-]{11})\]")
    ids = set()
    for fname in os.listdir(output_dir):
        if fname == STATE_FILE:
            continue
        m = pattern.search(fname)
        if m:
            ids.add(m.group(1))
    return ids


def filter_pending(entries, downloaded_ids):
    """Return entries whose 'id' is not in downloaded_ids."""
    return [e for e in entries if e.get("id") not in downloaded_ids]


def build_ydl_opts(output_dir, fmt, ffmpeg_path, rate_limit, user_agent):
    """Build yt-dlp YoutubeDL options dict for a single video download."""
    opts = {
        "format": "bestaudio/best",
        "outtmpl": str(Path(output_dir) / "%(title)s [%(id)s].%(ext)s"),
        "http_headers": {"User-Agent": user_agent},
        "retries": 3,
        "quiet": True,
        "no_warnings": True,
    }
    if rate_limit:
        opts["ratelimit"] = rate_limit
    if ffmpeg_path:
        opts["postprocessors"] = [
            {
                "key": "FFmpegExtractAudio",
                "preferredcodec": fmt,
                "preferredquality": "192",
            }
        ]
        opts["ffmpeg_location"] = ffmpeg_path
    return opts
```

> **Note:** Do NOT add `main()` or `fetch_video_list()` yet — that comes in Task 4. The file ends after `build_ydl_opts`.

- [ ] **Step 4: Run tests — confirm they pass**

```bash
cd skills/yt-channel-dl
python3 -m unittest tests/test_download_channel -v
```

Expected: All 20 tests PASS. Example output:
```
test_all_existing_returns_empty ... ok
test_all_new_returns_all ... ok
test_empty_dir ... ok
...
----------------------------------------------------------------------
Ran 20 tests in 0.XXXs
OK
```

- [ ] **Step 5: Commit**

```bash
git add skills/yt-channel-dl/scripts/download_channel.py skills/yt-channel-dl/tests/test_download_channel.py
git commit -m "feat(yt-channel-dl): add download_channel.py utility functions with tests"
```

---

## Task 4: `download_channel.py` — fetch, progress hook, download loop, main

**Files:**
- Modify: `skills/yt-channel-dl/scripts/download_channel.py` (append remaining functions + main)

- [ ] **Step 1: Append `fetch_video_list()` and `make_progress_hook()` to `download_channel.py`**

Add these functions after `build_ydl_opts` in `scripts/download_channel.py`:

```python
def fetch_video_list(channel_url):
    """Fetch all video entries from channel URL without downloading.

    Returns list of dicts: [{"id": str, "title": str, "url": str}, ...]
    """
    ydl_opts = {
        "extract_flat": "in_playlist",
        "quiet": True,
        "no_warnings": True,
    }
    with yt_dlp.YoutubeDL(ydl_opts) as ydl:
        info = ydl.extract_info(channel_url, download=False)

    entries = info.get("entries", []) if info else []

    # Flatten one nesting level (channel → playlist → videos)
    flat = []
    for e in entries:
        if e and e.get("_type") == "playlist":
            flat.extend(e.get("entries") or [])
        elif e:
            flat.append(e)

    result = []
    for e in flat:
        if not e or not e.get("id"):
            continue
        result.append(
            {
                "id": e["id"],
                "title": e.get("title") or e["id"],
                "url": e.get("url") or f"https://www.youtube.com/watch?v={e['id']}",
            }
        )
    return result


def make_progress_hook(title):
    """Return a yt-dlp progress hook that prints a compact progress bar to stdout."""
    last_pct = [-1]

    def hook(d):
        if d["status"] == "downloading":
            total = d.get("total_bytes") or d.get("total_bytes_estimate") or 0
            downloaded_bytes = d.get("downloaded_bytes") or 0
            speed = d.get("speed") or 0
            pct = int(downloaded_bytes / total * 100) if total else 0
            if pct != last_pct[0]:
                last_pct[0] = pct
                bar_len = 25
                filled = int(bar_len * pct / 100)
                bar = "█" * filled + "░" * (bar_len - filled)
                speed_str = f"{speed / 1024:.0f}KB/s" if speed else "?KB/s"
                line = f"\r  [{bar}] {pct:3d}% {speed_str}  {title[:35]}"
                sys.stdout.write(line)
                sys.stdout.flush()
        elif d["status"] == "finished":
            sys.stdout.write("\n")
            sys.stdout.flush()

    return hook


def print_summary(total, skipped, downloaded, failed_list):
    """Print final download summary table."""
    line = "━" * 37
    print(f"\n{line}")
    print(f"✓ Downloaded:                {downloaded}")
    print(f"⏭  Skipped (already exists): {skipped}")
    print(f"✗ Failed:                    {len(failed_list)}")
    print(line)
    if failed_list:
        print("\nFailed videos:")
        for item in failed_list:
            print(f'  - "{item["title"]}" [{item["id"]}] — {item["error"]}')
```

- [ ] **Step 2: Append `main()` to `download_channel.py`**

Add this after `print_summary`:

```python
def main():
    import shutil

    parser = argparse.ArgumentParser(
        description="Download all audio from a YouTube channel."
    )
    parser.add_argument("channel_url", help="YouTube channel URL")
    parser.add_argument("output_dir", help="Directory to save audio files")
    parser.add_argument(
        "--format",
        default="mp3",
        choices=["mp3", "aac", "m4a", "flac"],
        dest="fmt",
    )
    parser.add_argument("--workers", type=int, default=1)
    parser.add_argument("--rate-limit", default="500K")
    parser.add_argument("--min-sleep", type=float, default=2.0)
    parser.add_argument("--max-sleep", type=float, default=8.0)
    parser.add_argument("--burst", type=int, default=10)
    parser.add_argument("--burst-sleep", type=float, default=45.0)
    parser.add_argument("--ffmpeg-path", default=None)
    args = parser.parse_args()

    ffmpeg_path = args.ffmpeg_path or shutil.which("ffmpeg")
    fmt = args.fmt

    if fmt == "mp3" and not ffmpeg_path:
        print(
            "WARNING: ffmpeg not found — MP3 requires ffmpeg. Falling back to m4a.",
            file=sys.stderr,
        )
        fmt = "m4a"

    Path(args.output_dir).mkdir(parents=True, exist_ok=True)
    rate_limit = parse_rate_limit(args.rate_limit)

    # Step 1: Fetch video list
    print(f"Fetching video list from: {args.channel_url}")
    try:
        entries = fetch_video_list(args.channel_url)
    except Exception as e:
        print(f"ERROR: Failed to fetch video list: {e}", file=sys.stderr)
        sys.exit(1)

    print(f"Found {len(entries)} videos.")
    time.sleep(random.uniform(1, 3))

    # Step 2: Filter already downloaded
    state = load_state(args.output_dir)
    downloaded_ids = set(state["downloaded_ids"]) | scan_existing_ids(args.output_dir)
    pending = filter_pending(entries, downloaded_ids)
    skipped = len(entries) - len(pending)
    print(f"Skipping {skipped} already downloaded. {len(pending)} to go.")

    if not pending:
        print_summary(len(entries), skipped, 0, [])
        return

    # Step 3: Download loop
    downloaded_count = 0
    failed_list = []

    for i, entry in enumerate(pending, 1):
        # Burst rest every N videos (not before the first download)
        if i > 1 and (i - 1) % args.burst == 0:
            rest = random.uniform(args.burst_sleep * 0.8, args.burst_sleep * 1.2)
            print(f"\n  [Burst rest: sleeping {rest:.0f}s]")
            time.sleep(rest)

        # Per-video humanized delay
        time.sleep(random.uniform(args.min_sleep, args.max_sleep))

        ua = random.choice(USER_AGENTS)
        title = entry["title"]
        print(f"\n[{i}/{len(pending)}] {title}")

        opts = build_ydl_opts(args.output_dir, fmt, ffmpeg_path, rate_limit, ua)
        opts["progress_hooks"] = [make_progress_hook(title)]

        try:
            with yt_dlp.YoutubeDL(opts) as ydl:
                ydl.download([entry["url"]])
            downloaded_count += 1
            state["downloaded_ids"].append(entry["id"])
            save_state(args.output_dir, state)
        except yt_dlp.utils.DownloadError as e:
            err_msg = str(e).split("\n")[0][:100]
            print(f"  FAILED: {err_msg}", file=sys.stderr)
            failed_list.append({"id": entry["id"], "title": title, "error": err_msg})

    print_summary(len(entries), skipped, downloaded_count, failed_list)


if __name__ == "__main__":
    main()
```

- [ ] **Step 3: Verify `--help` works**

```bash
cd skills/yt-channel-dl
python3 scripts/download_channel.py --help
```

Expected output (no errors, shows all arguments):
```
usage: download_channel.py [-h] [--format {mp3,aac,m4a,flac}] [--workers WORKERS]
                            [--rate-limit RATE_LIMIT] [--min-sleep MIN_SLEEP]
                            [--max-sleep MAX_SLEEP] [--burst BURST]
                            [--burst-sleep BURST_SLEEP] [--ffmpeg-path FFMPEG_PATH]
                            channel_url output_dir
```

- [ ] **Step 4: Re-run existing unit tests to confirm nothing broke**

```bash
cd skills/yt-channel-dl
python3 -m unittest tests/test_download_channel -v
```

Expected: All 20 tests still PASS.

- [ ] **Step 5: Commit**

```bash
git add skills/yt-channel-dl/scripts/download_channel.py
git commit -m "feat(yt-channel-dl): add fetch, download loop, progress hook, and main()"
```

---

## Task 5: Complete `SKILL.md` operating procedure

**Files:**
- Modify: `skills/yt-channel-dl/SKILL.md`

- [ ] **Step 1: Replace placeholder content in `SKILL.md` with full operating procedure**

Replace the entire content of `skills/yt-channel-dl/SKILL.md` with:

```markdown
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

**若 `has_ffmpeg == false` 且使用者要求 `mp3` 格式：**
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
  --rate-limit 500K
```

**runner = `"python3"` 或 `"python"`：**
```bash
python3 skills/yt-channel-dl/scripts/download_channel.py \
  "<channel_url>" "<output_dir>" \
  --format <fmt> \
  --rate-limit 500K
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
```

- [ ] **Step 2: Verify SKILL.md parses correctly (frontmatter check)**

```bash
head -8 skills/yt-channel-dl/SKILL.md
```

Expected output:
```
---
name: yt-channel-dl
description: 從 YouTube 頻道 URL 下載所有影片音頻為 MP3（或其他格式），支援斷點續傳與擬人化爬取
argument-hint: "[channel_url] [output_dir] [--format mp3|aac|m4a|flac]"
allowed-tools: Bash, AskUserQuestion
---
```

- [ ] **Step 3: Commit**

```bash
git add skills/yt-channel-dl/SKILL.md
git commit -m "feat(yt-channel-dl): complete SKILL.md operating procedure"
```

---

## Task 6: Integration smoke test

**Files:** None created. Verify the full skill works end-to-end.

- [ ] **Step 1: Run environment detection and confirm JSON output**

```bash
python3 skills/yt-channel-dl/scripts/detect_python.py
```

Expected: Valid JSON printed to stdout with all 5 keys present.

- [ ] **Step 2: Run `--help` on download script**

```bash
python3 skills/yt-channel-dl/scripts/download_channel.py --help
```

Expected: Help text printed, exit code 0.

- [ ] **Step 3: Run all unit tests**

```bash
cd skills/yt-channel-dl && python3 -m unittest discover tests -v && cd ../..
```

Expected: All tests PASS, no failures.

- [ ] **Step 4: Verify directory structure is complete**

```bash
find skills/yt-channel-dl -type f | sort
```

Expected:
```
skills/yt-channel-dl/SKILL.md
skills/yt-channel-dl/scripts/detect_python.py
skills/yt-channel-dl/scripts/download_channel.py
skills/yt-channel-dl/tests/test_detect_python.py
skills/yt-channel-dl/tests/test_download_channel.py
```

- [ ] **Step 5: Final commit**

```bash
git add -A
git commit -m "feat(yt-channel-dl): complete skill with tests - ready for use"
```

---

## Post-Implementation Notes

- **yt-dlp channel URL formats supported:** `@handle`, `/channel/UCxxx`, `/c/name`, `/user/name`
- **Resume:** Re-run the same command to skip already-downloaded files automatically
- **ffmpeg not required** for M4A/AAC (native yt-dlp output); only needed for MP3/FLAC conversion
- **Rate limiting / bans:** Increase `--min-sleep` and `--max-sleep` if encountering HTTP 429 errors
