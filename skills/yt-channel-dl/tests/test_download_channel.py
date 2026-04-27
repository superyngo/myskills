#!/usr/bin/env python3
"""Unit tests for pure utility functions in download_channel.py."""
import json
import os
import sys
import tempfile
import unittest

sys.path.insert(0, os.path.join(os.path.dirname(__file__), "..", "scripts"))

from download_channel import (
    STATE_FILE,
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
            with open(os.path.join(d, STATE_FILE), "w") as f:
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
            with open(os.path.join(d, STATE_FILE), "w") as f:
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

    def test_entry_without_id_is_excluded(self):
        entries = [{"title": "No ID entry"}]
        self.assertEqual(filter_pending(entries, set()), [])


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
        self.assertEqual(opts["outtmpl"], "/my/music/%(title)s [%(id)s].%(ext)s")

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
