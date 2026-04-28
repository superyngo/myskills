#!/usr/bin/env python3
"""Download all audio from a YouTube channel or playlist.

Usage:
    python3 download_channel.py <url> <output_dir>
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
    yt_dlp = None

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
    """Write state dict to JSON file in output_dir. Logs warning on failure."""
    path = Path(output_dir) / STATE_FILE
    try:
        with open(path, "w") as f:
            json.dump(state, f, indent=2)
    except OSError as exc:
        print(f"WARNING: could not save state to {path}: {exc}", file=sys.stderr)


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


def write_status(status_file, data):
    """Write compact status dict to JSON file atomically. No-op if status_file is None."""
    if not status_file:
        return
    tmp = status_file + ".tmp"
    try:
        with open(tmp, "w") as f:
            json.dump(data, f)
        os.replace(tmp, status_file)
    except OSError:
        pass


def filter_pending(entries, downloaded_ids):
    """Return entries whose 'id' is not in downloaded_ids. Excludes entries without an id."""
    return [e for e in entries if e.get("id") and e["id"] not in downloaded_ids]


def build_ydl_opts(output_dir, fmt, ffmpeg_path, rate_limit, user_agent):
    """Build yt-dlp YoutubeDL options dict for a single video download."""
    opts = {
        "format": "bestaudio[ext=m4a]/bestaudio/best" if (not ffmpeg_path and fmt == "m4a") else "bestaudio/best",
        "outtmpl": str(Path(output_dir) / "%(title)s [%(id)s].%(ext)s"),
        "http_headers": {"User-Agent": user_agent},
        "retries": 3,
        "quiet": True,
        "no_warnings": True,
        "remote_components": ["ejs:github"],
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


def fetch_video_list(channel_url):
    """Fetch all video entries from channel URL without downloading.

    Returns list of dicts: [{"id": str, "title": str, "url": str}, ...]
    """
    ydl_opts = {
        "extract_flat": "in_playlist",
        "quiet": True,
        "no_warnings": True,
        "remote_components": ["ejs:github"],
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
    print(f"  Total:                      {total}")
    print(f"✓ Downloaded:                {downloaded}")
    print(f"⏭  Skipped (already exists): {skipped}")
    print(f"✗ Failed:                    {len(failed_list)}")
    print(line)
    if failed_list:
        print("\nFailed videos:")
        for item in failed_list:
            print(f'  - "{item["title"]}" [{item["id"]}] — {item["error"]}')


def main():
    import shutil

    if yt_dlp is None:
        print(
            "ERROR: yt-dlp not installed. Run detect_python.py for install instructions.",
            file=sys.stderr,
        )
        sys.exit(1)

    parser = argparse.ArgumentParser(
        description="Download all audio from a YouTube channel or playlist."
    )
    parser.add_argument("channel_url", help="YouTube channel or playlist URL")
    parser.add_argument("output_dir", help="Directory to save audio files")
    parser.add_argument(
        "--format",
        default="mp3",
        choices=["mp3", "aac", "m4a", "flac"],
        dest="fmt",
    )
    parser.add_argument("--rate-limit", default="500K")
    parser.add_argument("--min-sleep", type=float, default=2.0)
    parser.add_argument("--max-sleep", type=float, default=8.0)
    parser.add_argument("--burst", type=int, default=10)
    parser.add_argument("--burst-sleep", type=float, default=45.0)
    parser.add_argument("--ffmpeg-path", default=None)
    parser.add_argument("--status-file", default=None)
    args = parser.parse_args()

    if args.burst < 1:
        parser.error("--burst must be >= 1")

    ffmpeg_path = args.ffmpeg_path or shutil.which("ffmpeg")
    fmt = args.fmt

    if fmt in ("mp3", "aac", "flac") and not ffmpeg_path:
        print(
            f"WARNING: ffmpeg not found — {fmt.upper()} conversion requires ffmpeg. "
            "Falling back to native m4a stream (no re-encoding).",
            file=sys.stderr,
        )
        fmt = "m4a"

    Path(args.output_dir).mkdir(parents=True, exist_ok=True)
    rate_limit = parse_rate_limit(args.rate_limit)
    started_at = time.strftime("%Y-%m-%dT%H:%M:%S")
    sf = args.status_file

    # Step 1: Fetch video list
    write_status(sf, {"phase": "fetching", "started_at": started_at})
    print(f"Fetching video list from: {args.channel_url}")
    try:
        entries = fetch_video_list(args.channel_url)
    except Exception as e:
        write_status(sf, {"phase": "error", "error": str(e), "started_at": started_at})
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
        write_status(sf, {
            "phase": "done", "total": len(entries),
            "downloaded": 0, "skipped": skipped, "failed": 0,
            "failed_list": [], "started_at": started_at,
            "finished_at": time.strftime("%Y-%m-%dT%H:%M:%S"),
        })
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

        write_status(sf, {
            "phase": "downloading", "total": len(entries),
            "completed": downloaded_count, "skipped": skipped,
            "failed": len(failed_list), "current": title[:60],
            "current_index": f"{i}/{len(pending)}",
            "pending_count": len(pending), "started_at": started_at,
        })

        opts = build_ydl_opts(args.output_dir, fmt, ffmpeg_path, rate_limit, ua)
        opts["progress_hooks"] = [make_progress_hook(title)]

        try:
            with yt_dlp.YoutubeDL(opts) as ydl:
                ydl.download([entry["url"]])
            downloaded_count += 1
            state["downloaded_ids"].append(entry["id"])
            state["downloaded_ids"] = list(dict.fromkeys(state["downloaded_ids"]))
            save_state(args.output_dir, state)
        except yt_dlp.utils.DownloadError as e:
            err_msg = str(e).split("\n")[0][:100]
            print(f"  FAILED: {err_msg}", file=sys.stderr)
            failed_list.append({"id": entry["id"], "title": title, "error": err_msg})

    write_status(sf, {
        "phase": "done", "total": len(entries),
        "downloaded": downloaded_count, "skipped": skipped,
        "failed": len(failed_list), "failed_list": failed_list,
        "started_at": started_at,
        "finished_at": time.strftime("%Y-%m-%dT%H:%M:%S"),
    })
    print_summary(len(entries), skipped, downloaded_count, failed_list)


if __name__ == "__main__":
    main()
