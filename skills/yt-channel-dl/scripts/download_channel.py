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
