#!/usr/bin/env python3
"""Capture repeated generic Kitsune pane reads for offline screen fixtures."""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
import time
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path
from typing import Any


DEFAULT_OUT_DIR = Path(".local/agent-screen-captures")
DEFAULT_PANE = "harness-test"
AGENT_RETIREMENT_MESSAGE = "feature_retired: Legacy agent capture targeting is retired; use --pane with a pane id or label. Kitsune no longer provides built-in agent lifecycle, readiness, or status."
STATE_CHOICES = {
    "i": "idle",
    "idle": "idle",
    "w": "working",
    "working": "working",
    "b": "blocked",
    "blocked": "blocked",
    "d": "done",
    "done": "done",
    "u": "unknown",
    "unknown": "unknown",
    "c": "custom",
    "custom": "custom",
}


@dataclass
class CommandResult:
    code: int
    stdout: bytes
    stderr: bytes


@dataclass
class PaneMatch:
    pane_id: str
    label: str | None
    title: str | None
    raw: dict[str, Any] | None


def main() -> int:
    args = parse_args()
    if args.agent is not None:
        print(AGENT_RETIREMENT_MESSAGE, file=sys.stderr)
        return 1
    run_dir = args.out / datetime.now().strftime("%Y%m%d-%H%M%S")
    run_dir.mkdir(parents=True, exist_ok=True)

    print(f"writing captures under {run_dir}")
    print("state shortcuts: i=idle, w=working, b=blocked, d=done, u=unknown, c=custom, q=quit")

    capture_index = 1
    while True:
        try:
            pane = resolve_pane(args.kitsune, args.pane)
        except RuntimeError as error:
            print(error, file=sys.stderr)
            return 1
        if pane is None:
            print(f"pane '{args.pane}' was not found by `kitsune pane list`")
            print("pass a pane id with --pane, or rename the target pane to harness-test")
            return 1

        print(f"\npane found: {pane.pane_id}{format_pane_context(pane)}")
        state = prompt_state()
        if state is None:
            break

        name = prompt_name(pane, state)
        if name is None:
            break

        capture_dir = run_dir / f"{capture_index:03d}-{slugify(name)}"
        capture_dir.mkdir(parents=True, exist_ok=False)
        print(f"capturing {args.samples} sample(s) into {capture_dir}")

        started_at = iso_now()
        failures = capture_case(args, pane, state, name, capture_dir)
        write_metadata(
            capture_dir,
            args=args,
            pane=pane,
            state=state,
            name=name,
            started_at=started_at,
            finished_at=iso_now(),
            failures=failures,
        )

        if failures:
            print(f"saved with {len(failures)} command failure(s); see metadata.toml")
        else:
            print("saved")

        capture_index += 1
        if args.once:
            break

    print("done")
    return 0


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Interactively capture generic Kitsune pane screen reads for offline fixture work."
    )
    parser.add_argument(
        "--pane",
        default=DEFAULT_PANE,
        help=f"pane label/title/id to capture when --agent is not set (default: {DEFAULT_PANE})",
    )
    parser.add_argument(
        "--agent",
        help="retired agent target spelling; fails locally with feature_retired (use --pane)",
    )
    parser.add_argument(
        "--out",
        type=Path,
        default=DEFAULT_OUT_DIR,
        help=f"output directory (default: {DEFAULT_OUT_DIR})",
    )
    parser.add_argument(
        "--samples",
        type=positive_int,
        default=5,
        help="samples to capture per state (default: 5)",
    )
    parser.add_argument(
        "--interval",
        type=non_negative_float,
        default=1.0,
        help="seconds between samples (default: 1.0)",
    )
    parser.add_argument(
        "--lines",
        type=positive_int,
        default=120,
        help="recent-buffer lines to save per sample (default: 120)",
    )
    parser.add_argument(
        "--kitsune",
        default="kitsune",
        help="Kitsune CLI binary to call (default: kitsune)",
    )
    parser.add_argument(
        "--once",
        action="store_true",
        help="capture one state and exit",
    )
    return parser.parse_args()


def positive_int(value: str) -> int:
    parsed = int(value)
    if parsed <= 0:
        raise argparse.ArgumentTypeError("must be greater than zero")
    return parsed


def non_negative_float(value: str) -> float:
    parsed = float(value)
    if parsed < 0:
        raise argparse.ArgumentTypeError("must be zero or greater")
    return parsed


def resolve_pane(kitsune: str, pane_ref: str) -> PaneMatch | None:
    result = run_command([kitsune, "pane", "list"])
    if result.code != 0:
        raise RuntimeError(f"kitsune pane list failed with exit {result.code}")

    try:
        response = json.loads(result.stdout.decode("utf-8"))
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise RuntimeError("kitsune pane list returned invalid JSON") from error

    if not isinstance(response, dict) or not isinstance(response.get("result"), dict):
        raise RuntimeError("kitsune pane list returned no pane result")
    panes = response["result"].get("panes")
    if not isinstance(panes, list):
        raise RuntimeError("kitsune pane list returned no pane list")

    exact_matches = []
    loose_matches = []
    for pane in panes:
        if not isinstance(pane, dict):
            continue
        pane_id = string_value(pane.get("pane_id"))
        if pane_id is None:
            continue
        fields = pane_match_fields(pane)
        if any(field == pane_ref for field in fields):
            exact_matches.append(pane)
        elif any(field and pane_ref in field for field in fields):
            loose_matches.append(pane)

    matches = exact_matches or loose_matches
    if len(matches) == 1:
        return pane_from_dict(matches[0])
    if len(matches) > 1:
        print(f"pane ref '{pane_ref}' matched multiple panes:")
        for pane in matches:
            print(f"  {pane.get('pane_id')}{format_pane_context(pane_from_dict(pane))}")
        return None

    return fallback_pane_id(pane_ref)


def fallback_pane_id(pane_ref: str) -> PaneMatch | None:
    if re.fullmatch(r"p_(?:[A-Za-z0-9]+_)?\d+", pane_ref) or re.fullmatch(
        r"[A-Za-z0-9_]+-\d+", pane_ref
    ):
        return PaneMatch(
            pane_id=normalize_pane_id(pane_ref),
            label=None,
            title=None,
            raw=None,
        )
    return None


def normalize_pane_id(pane_ref: str) -> str:
    return pane_ref


def pane_match_fields(pane: dict[str, Any]) -> list[str]:
    fields = [
        string_value(pane.get("pane_id")),
        string_value(pane.get("terminal_id")),
        string_value(pane.get("label")),
        string_value(pane.get("title")),
    ]
    return [field for field in fields if field]


def pane_from_dict(pane: dict[str, Any]) -> PaneMatch:
    return PaneMatch(
        pane_id=string_value(pane.get("pane_id")) or "",
        label=string_value(pane.get("label")),
        title=string_value(pane.get("title")),
        raw={key: value for key, value in pane.items() if key not in {
            "agent", "agent_status", "agent_session", "display_agent", "name",
            "launch_pending", "interactive_ready",
        }},
    )


def string_value(value: Any) -> str | None:
    return value if isinstance(value, str) and value else None


def format_pane_context(pane: PaneMatch) -> str:
    details = []
    if pane.label:
        details.append(f"label={pane.label}")
    if pane.title:
        details.append(f"title={pane.title}")
    if not details:
        return ""
    return " (" + ", ".join(details) + ")"


def prompt_state() -> str | None:
    while True:
        raw = input("state [idle/working/blocked/done/unknown/custom/q]: ").strip()
        if raw.lower() in {"q", "quit", "exit"}:
            return None
        if not raw:
            continue
        state = STATE_CHOICES.get(raw.lower())
        if state == "custom":
            custom = input("custom state label: ").strip()
            if custom:
                return custom
            continue
        if state:
            return state
        return raw


def prompt_name(pane: PaneMatch, state: str) -> str | None:
    default_parts = [
        pane.label or "pane",
        state,
        datetime.now().strftime("%H%M%S"),
    ]
    default_name = "-".join(slugify(part) for part in default_parts if part)
    raw = input(f"capture name [{default_name}]: ").strip()
    if raw.lower() in {"q", "quit", "exit"}:
        return None
    return raw or default_name


def capture_case(
    args: argparse.Namespace,
    pane: PaneMatch,
    state: str,
    name: str,
    capture_dir: Path,
) -> list[str]:
    failures: list[str] = []
    for index in range(1, args.samples + 1):
        prefix = f"sample-{index:03d}"
        print(f"  sample {index}/{args.samples}")

        # Detection reads the generic bottom of the live buffer, independent of the
        # viewport; it does not infer a native agent or its status.
        commands = [
            (
                f"{prefix}.detection.txt",
                [args.kitsune, "pane", "read", pane.pane_id, "--source", "detection", "--format", "text"],
            ),
            (
                f"{prefix}.detection.ansi",
                [args.kitsune, "pane", "read", pane.pane_id, "--source", "detection", "--format", "ansi"],
            ),
            (
                f"{prefix}.recent.txt",
                [
                    args.kitsune,
                    "pane",
                    "read",
                    pane.pane_id,
                    "--source",
                    "recent",
                    "--lines",
                    str(args.lines),
                    "--format",
                    "text",
                ],
            ),
            (
                f"{prefix}.recent.ansi",
                [
                    args.kitsune,
                    "pane",
                    "read",
                    pane.pane_id,
                    "--source",
                    "recent",
                    "--lines",
                    str(args.lines),
                    "--format",
                    "ansi",
                ],
            ),
        ]

        for filename, command in commands:
            result = run_command(command)
            output_path = capture_dir / filename
            if result.code == 0:
                output_path.write_bytes(result.stdout)
            else:
                failures.append(f"{filename}: exit {result.code}: {' '.join(command)}")
                output_path.with_suffix(output_path.suffix + ".stderr").write_bytes(result.stderr)
                output_path.write_bytes(result.stdout)

        sample_meta = {
            "captured_at": iso_now(),
            "sample": index,
            "state": state,
            "name": name,
        }
        (capture_dir / f"{prefix}.json").write_text(
            json.dumps(sample_meta, indent=2, sort_keys=True) + "\n",
            encoding="utf-8",
        )

        if index < args.samples:
            time.sleep(args.interval)
    return failures


def run_command(command: list[str]) -> CommandResult:
    try:
        completed = subprocess.run(command, capture_output=True, check=False)
    except FileNotFoundError as err:
        return CommandResult(code=127, stdout=b"", stderr=str(err).encode("utf-8"))
    return CommandResult(
        code=completed.returncode,
        stdout=completed.stdout,
        stderr=completed.stderr,
    )


def write_metadata(
    capture_dir: Path,
    *,
    args: argparse.Namespace,
    pane: PaneMatch,
    state: str,
    name: str,
    started_at: str,
    finished_at: str,
    failures: list[str],
) -> None:
    lines = [
        f"name = {toml_string(name)}",
        f"state = {toml_string(state)}",
        f"started_at = {toml_string(started_at)}",
        f"finished_at = {toml_string(finished_at)}",
        f"pane_ref = {toml_string(args.pane)}",
        f"agent_ref = {toml_optional_string(args.agent)}",
        f"pane_id = {toml_string(pane.pane_id)}",
        f"samples = {args.samples}",
        f"interval_seconds = {args.interval}",
        f"recent_lines = {args.lines}",
        f"kitsune = {toml_string(args.kitsune)}",
        f"pane_label = {toml_optional_string(pane.label)}",
        'agent_name = ""',
        'pane_agent = ""',
        'display_agent = ""',
        f"pane_title = {toml_optional_string(pane.title)}",
        "",
        "[commands]",
        'detection_text = "kitsune pane read <pane> --source detection --format text"',
        'detection_ansi = "kitsune pane read <pane> --source detection --format ansi"',
        'recent_text = "kitsune pane read <pane> --source recent --lines <n> --format text"',
        'recent_ansi = "kitsune pane read <pane> --source recent --lines <n> --format ansi"',
    ]
    if failures:
        lines.append("")
        lines.append("failures = [")
        for failure in failures:
            lines.append(f"  {toml_string(failure)},")
        lines.append("]")

    (capture_dir / "metadata.toml").write_text("\n".join(lines) + "\n", encoding="utf-8")
    if pane.raw is not None:
        (capture_dir / "pane.json").write_text(
            json.dumps(pane.raw, indent=2, sort_keys=True) + "\n",
            encoding="utf-8",
        )


def toml_optional_string(value: str | None) -> str:
    if value is None:
        return '""'
    return toml_string(value)


def toml_string(value: str) -> str:
    return json.dumps(value)


def slugify(value: str) -> str:
    slug = re.sub(r"[^A-Za-z0-9._-]+", "-", value.strip().lower())
    slug = re.sub(r"-+", "-", slug).strip("-")
    return slug or "capture"


def iso_now() -> str:
    return datetime.now(timezone.utc).isoformat(timespec="seconds")


if __name__ == "__main__":
    sys.exit(main())
