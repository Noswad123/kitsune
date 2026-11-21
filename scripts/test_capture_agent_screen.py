from __future__ import annotations

import argparse
import json
import subprocess
import sys
import tempfile
import tomllib
import unittest
from pathlib import Path
from unittest.mock import patch

from scripts import capture_agent_screen as capture


class CaptureAgentScreenTests(unittest.TestCase):
    def test_agent_target_fails_before_cli_or_output_creation(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            marker = root / "invoked"
            canary = root / "kitsune"
            canary.write_text(
                f"#!{sys.executable}\nfrom pathlib import Path\nPath({str(marker)!r}).touch()\n",
                encoding="utf-8",
            )
            canary.chmod(0o755)
            output_dir = root / "captures"
            for target in ["private-target", ""]:
                output = subprocess.run(
                    [sys.executable, str(Path(capture.__file__)), "--agent", target,
                     "--kitsune", str(canary), "--out", str(output_dir)],
                    capture_output=True, check=False, timeout=5,
                )
                self.assertEqual(output.returncode, 1, output)
                self.assertEqual(output.stderr.decode().strip(), capture.AGENT_RETIREMENT_MESSAGE)
                self.assertEqual(output.stdout, b"")
                self.assertFalse(marker.exists())
                self.assertFalse(output_dir.exists())

    def test_generic_pane_capture_preserves_sources_without_agent_lookup(self):
        calls = []
        outputs = {
            ("detection", "text"): b"bottom of live buffer\n",
            # The generic detection selector currently returns plaintext for both formats.
            ("detection", "ansi"): b"bottom of live buffer\n",
            ("recent", "text"): b"recent buffer history\n",
            ("recent", "ansi"): b"\x1b[32mrecent buffer history\x1b[0m\n",
            ("visible", "text"): b"scrolled viewport\n",
            ("visible", "ansi"): b"\x1b[31mscrolled viewport\x1b[0m\n",
        }
        pane = {"pane_id": "w1:p1", "terminal_id": "term_1", "label": "capture-pane",
                "title": "generic title", "agent": "private-agent", "name": "private-name",
                "display_agent": "private-display", "agent_status": "idle",
                "interactive_ready": True, "agent_session": {"value": "private-session"}}

        def command(argv):
            calls.append(argv)
            if argv[1:] == ["pane", "list"]:
                return capture.CommandResult(0, json.dumps({"result": {"panes": [pane]}}).encode(), b"")
            self.assertEqual(argv[1:4], ["pane", "read", "w1:p1"])
            source = argv[argv.index("--source") + 1]
            read_format = argv[argv.index("--format") + 1]
            return capture.CommandResult(0, outputs[(source, read_format)], b"")

        with tempfile.TemporaryDirectory() as directory:
            args = argparse.Namespace(out=Path(directory), pane="capture-pane", agent=None,
                                      kitsune="canary", samples=1, interval=0, lines=12, once=True)
            with patch.object(capture, "parse_args", return_value=args), \
                 patch.object(capture, "run_command", side_effect=command), \
                 patch.object(capture, "prompt_state", return_value="unknown"), \
                 patch.object(capture, "prompt_name", return_value="screen"):
                self.assertEqual(capture.main(), 0)
            self.assertEqual(calls, [
                ["canary", "pane", "list"],
                ["canary", "pane", "read", "w1:p1", "--source", "detection", "--format", "text"],
                ["canary", "pane", "read", "w1:p1", "--source", "detection", "--format", "ansi"],
                ["canary", "pane", "read", "w1:p1", "--source", "recent", "--lines", "12", "--format", "text"],
                ["canary", "pane", "read", "w1:p1", "--source", "recent", "--lines", "12", "--format", "ansi"],
            ])
            case = next(Path(directory).glob("*/001-screen"))
            metadata = tomllib.loads((case / "metadata.toml").read_text())
            for field in ["agent_ref", "agent_name", "pane_agent", "display_agent"]:
                self.assertEqual(metadata[field], "")
            self.assertEqual(metadata["pane_id"], "w1:p1")
            self.assertEqual(metadata["state"], "unknown")
            self.assertEqual(metadata["commands"], {
                "detection_text": "kitsune pane read <pane> --source detection --format text",
                "detection_ansi": "kitsune pane read <pane> --source detection --format ansi",
                "recent_text": "kitsune pane read <pane> --source recent --lines <n> --format text",
                "recent_ansi": "kitsune pane read <pane> --source recent --lines <n> --format ansi",
            })
            for source in ["detection", "recent"]:
                for read_format, suffix in [("text", "txt"), ("ansi", "ansi")]:
                    with self.subTest(source=source, read_format=read_format):
                        self.assertEqual(
                            (case / f"sample-001.{source}.{suffix}").read_bytes(),
                            outputs[(source, read_format)],
                        )
            self.assertFalse((case / "sample-001.explain.json").exists())
            self.assertFalse((case / "agent.json").exists())
            self.assertNotIn("private-", (case / "pane.json").read_text())

    def test_agent_fields_are_not_pane_lookup_aliases(self):
        fields = capture.pane_match_fields({"pane_id": "w1:p1", "agent": "pi", "display_agent": "agent"})
        self.assertEqual(fields, ["w1:p1"])

    def test_generic_fetch_failures_are_not_hidden_by_fallbacks(self):
        for result in [capture.CommandResult(1, b"", b"failure"),
                       capture.CommandResult(0, b"not-json", b""),
                       capture.CommandResult(0, b'{"error":{}}', b""),
                       capture.CommandResult(0, b'{"result":{"panes":{}}}', b"")]:
            with self.subTest(result=result), patch.object(capture, "run_command", return_value=result) as command:
                with self.assertRaises(RuntimeError):
                    capture.resolve_pane("canary", "p_1")
                command.assert_called_once_with(["canary", "pane", "list"])

    def test_parser_help_and_invalid_syntax_do_not_invoke_cli(self):
        for argv, code in [(["capture", "--help"], 0), (["capture", "--agent"], 2),
                           (["capture", "--pane", "w1:p1", "--samples", "0"], 2)]:
            with self.subTest(argv=argv), patch.object(sys, "argv", argv), \
                 patch.object(capture, "run_command") as command:
                with self.assertRaises(SystemExit) as error:
                    capture.main()
                self.assertEqual(error.exception.code, code)
                command.assert_not_called()

    def test_missing_cli_preserves_command_result_contract(self):
        with patch.object(capture.subprocess, "run", side_effect=FileNotFoundError("missing")):
            result = capture.run_command(["missing", "pane", "list"])
        self.assertEqual(result.code, 127)
        self.assertEqual(result.stdout, b"")
        self.assertIn(b"missing", result.stderr)


if __name__ == "__main__":
    unittest.main()
