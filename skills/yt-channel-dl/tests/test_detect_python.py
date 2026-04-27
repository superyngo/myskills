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

    def test_has_yt_dlp_is_bool(self):
        result = self._run_script()
        data = json.loads(result.stdout)
        self.assertIsInstance(data["has_yt_dlp"], bool)

    def test_has_ffmpeg_is_bool(self):
        result = self._run_script()
        data = json.loads(result.stdout)
        self.assertIsInstance(data["has_ffmpeg"], bool)

    def test_ffmpeg_path_consistent_with_has_ffmpeg(self):
        result = self._run_script()
        data = json.loads(result.stdout)
        if data["has_ffmpeg"]:
            self.assertIsInstance(data["ffmpeg_path"], str)
            self.assertTrue(len(data["ffmpeg_path"]) > 0)
        else:
            self.assertIsNone(data["ffmpeg_path"])


if __name__ == "__main__":
    unittest.main()
