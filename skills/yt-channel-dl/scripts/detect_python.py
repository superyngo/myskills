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
