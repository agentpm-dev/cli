#!/usr/bin/env python3
"""Compare Harness report/trace semantics across execution surfaces."""

from __future__ import annotations

import argparse
import json
import sys
import tempfile
from collections import Counter
from pathlib import Path
from typing import Any


SEMANTIC_KEYS = (
    "terminal_status",
    "phase_summaries",
    "checkpoint_summaries",
    "action_summaries",
    "mcp_summaries",
    "memory_summaries",
    "diagnostics",
    "trace_event_counts",
)


def read_json(path: Path) -> dict[str, Any]:
    with path.open("r", encoding="utf-8") as handle:
        value = json.load(handle)
    if not isinstance(value, dict):
        raise ValueError(f"{path} did not contain a JSON object")
    return value


def read_jsonl(path: Path) -> list[dict[str, Any]]:
    events: list[dict[str, Any]] = []
    with path.open("r", encoding="utf-8") as handle:
        for line_number, line in enumerate(handle, 1):
            if not line.strip():
                continue
            value = json.loads(line)
            if not isinstance(value, dict):
                raise ValueError(f"{path}:{line_number} did not contain a JSON object")
            events.append(value)
    return events


def pick(value: dict[str, Any], *keys: str) -> Any:
    for key in keys:
        if key in value:
            return value[key]
    return None


def scalar(value: Any) -> Any:
    if isinstance(value, (str, int, float, bool)) or value is None:
        return value
    return json.dumps(value, sort_keys=True, separators=(",", ":"))


def normalize_records(records: Any, keys: tuple[str, ...]) -> list[dict[str, Any]]:
    if not isinstance(records, list):
        return []
    normalized = []
    for record in records:
        if not isinstance(record, dict):
            continue
        normalized.append({key: scalar(record.get(key)) for key in keys if key in record})
    return sorted(normalized, key=lambda item: json.dumps(item, sort_keys=True))


def normalize_diagnostics(records: Any) -> list[dict[str, Any]]:
    if not isinstance(records, list):
        return []
    normalized = []
    for record in records:
        if not isinstance(record, dict):
            continue
        normalized.append(
            {
                "severity": scalar(record.get("severity")),
                "code": scalar(record.get("code")),
            }
        )
    return sorted(normalized, key=lambda item: json.dumps(item, sort_keys=True))


def trace_event_counts(events: list[dict[str, Any]]) -> dict[str, int]:
    counter: Counter[str] = Counter()
    for event in events:
        event_type = event.get("event_type")
        if isinstance(event_type, str):
            counter[event_type] += 1
    return dict(sorted(counter.items()))


def summarize_case(label: str, surface: str, report_path: Path, trace_path: Path) -> dict[str, Any]:
    report = read_json(report_path)
    events = read_jsonl(trace_path)
    return {
        "label": label,
        "surface": surface,
        "report_path": str(report_path),
        "trace_path": str(trace_path),
        "summary": {
            "terminal_status": scalar(
                pick(report, "terminal_status", "status", "runtime_status", "terminal")
            ),
            "phase_summaries": normalize_records(
                report.get("phase_summaries"),
                ("phase_id", "status", "outcome", "transition", "terminal_status"),
            ),
            "checkpoint_summaries": normalize_records(
                report.get("checkpoint_summaries"),
                ("id", "checkpoint_id", "status", "decision", "phase_id"),
            ),
            "action_summaries": normalize_records(
                report.get("action_summaries"),
                ("phase_id", "action_kind", "identity", "status", "outcome"),
            ),
            "mcp_summaries": normalize_records(
                report.get("mcp_summaries"),
                ("identity", "status", "operation", "source"),
            ),
            "memory_summaries": normalize_records(
                report.get("memory_summaries"),
                ("identity", "status", "operation", "scope", "source"),
            ),
            "diagnostics": normalize_diagnostics(report.get("diagnostics")),
            "trace_event_counts": trace_event_counts(events),
        },
    }


def parse_case(raw: str) -> tuple[str, str, Path, Path | None]:
    try:
        label, payload = raw.split("=", 1)
    except ValueError as exc:
        raise argparse.ArgumentTypeError("case must be LABEL=SURFACE:REPORT[:TRACE]") from exc
    parts = payload.split(":")
    if len(parts) not in (2, 3):
        raise argparse.ArgumentTypeError("case must be LABEL=SURFACE:REPORT[:TRACE]")
    surface = parts[0]
    report = Path(parts[1])
    trace = Path(parts[2]) if len(parts) == 3 and parts[2] else None
    return label, surface, report, trace


def resolve_trace(report: dict[str, Any], explicit: Path | None) -> Path:
    if explicit is not None:
        return explicit
    trace_path = report.get("trace_path")
    if not isinstance(trace_path, str) or not trace_path:
        raise ValueError("case omitted TRACE and report has no trace_path")
    return Path(trace_path)


def compare(cases: list[dict[str, Any]]) -> list[dict[str, Any]]:
    if len(cases) < 2:
        return []
    baseline = cases[0]
    mismatches: list[dict[str, Any]] = []
    for case in cases[1:]:
        for key in SEMANTIC_KEYS:
            left = baseline["summary"].get(key)
            right = case["summary"].get(key)
            if left != right:
                mismatches.append(
                    {
                        "baseline": baseline["label"],
                        "case": case["label"],
                        "field": key,
                        "baseline_value": left,
                        "case_value": right,
                    }
                )
    return mismatches


def guard_mismatches(
    cases: list[dict[str, Any]], min_cases: int, require_nonempty: bool
) -> list[dict[str, Any]]:
    mismatches: list[dict[str, Any]] = []
    if len(cases) < min_cases:
        mismatches.append(
            {
                "field": "case_count",
                "reason": "insufficient_cases",
                "minimum": min_cases,
                "actual": len(cases),
            }
        )
    if require_nonempty:
        for case in cases:
            summary = case["summary"]
            if not summary.get("phase_summaries"):
                mismatches.append(
                    {
                        "case": case["label"],
                        "field": "phase_summaries",
                        "reason": "empty_semantics",
                    }
                )
            if not summary.get("trace_event_counts"):
                mismatches.append(
                    {
                        "case": case["label"],
                        "field": "trace_event_counts",
                        "reason": "empty_semantics",
                    }
                )
    return mismatches


def write_artifacts(
    root: Path,
    name: str,
    report: dict[str, Any],
    trace: list[dict[str, Any]],
) -> tuple[Path, Path]:
    report_path = root / f"{name}.json"
    trace_path = root / f"{name}.jsonl"
    report_path.write_text(json.dumps(report), encoding="utf-8")
    trace_path.write_text(
        "\n".join(json.dumps(event) for event in trace) + ("\n" if trace else ""),
        encoding="utf-8",
    )
    return report_path, trace_path


def run_self_test() -> int:
    with tempfile.TemporaryDirectory(prefix="agentpm-harness-release-verify-") as tmp:
        root = Path(tmp)
        report = {
            "terminal_status": "ended",
            "phase_summaries": [
                {"phase_id": "inspect", "status": "completed", "outcome": "respond"},
                {"phase_id": "respond", "status": "completed", "outcome": "done"},
            ],
            "checkpoint_summaries": [],
            "action_summaries": [
                {"phase_id": "inspect", "action_kind": "tool", "identity": "@zack/search", "status": "completed"}
            ],
            "mcp_summaries": [],
            "memory_summaries": [],
            "diagnostics": [],
        }
        trace = [
            {"event_type": "run_started"},
            {"event_type": "phase_started"},
            {"event_type": "phase_result_ready"},
            {"event_type": "run_completed"},
        ]
        for name in ("headless", "machine"):
            write_artifacts(root, name, report, trace)
        argv = [
            "--case",
            f"headless=headless:{root / 'headless.json'}:{root / 'headless.jsonl'}",
            "--case",
            f"machine=machine:{root / 'machine.json'}:{root / 'machine.jsonl'}",
        ]
        if main(argv) != 0:
            return 1

        mismatched_report = {**report, "terminal_status": "failed"}
        mismatch_report, mismatch_trace = write_artifacts(
            root, "mismatch", mismatched_report, trace
        )
        if (
            main(
                [
                    "--case",
                    f"headless=headless:{root / 'headless.json'}:{root / 'headless.jsonl'}",
                    "--case",
                    f"mismatch=machine:{mismatch_report}:{mismatch_trace}",
                ]
            )
            == 0
        ):
            print("self-test failed: semantic mismatch passed", file=sys.stderr)
            return 1

        single_report, single_trace = write_artifacts(root, "single", report, trace)
        if (
            main(
                [
                    "--case",
                    f"single=headless:{single_report}:{single_trace}",
                ]
            )
            == 0
        ):
            print("self-test failed: single case passed", file=sys.stderr)
            return 1

        empty_report = {"terminal_status": "ended"}
        empty_trace: list[dict[str, Any]] = []
        for name in ("empty-headless", "empty-machine"):
            write_artifacts(root, name, empty_report, empty_trace)
        if (
            main(
                [
                    "--case",
                    f"empty-headless=headless:{root / 'empty-headless.json'}:{root / 'empty-headless.jsonl'}",
                    "--case",
                    f"empty-machine=machine:{root / 'empty-machine.json'}:{root / 'empty-machine.jsonl'}",
                ]
            )
            == 0
        ):
            print("self-test failed: empty semantics passed", file=sys.stderr)
            return 1

        return 0


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--case",
        action="append",
        type=parse_case,
        default=[],
        metavar="LABEL=SURFACE:REPORT[:TRACE]",
        help="Report/trace artifact pair for one execution surface.",
    )
    parser.add_argument(
        "--skip",
        action="append",
        default=[],
        metavar="SURFACE=REASON",
        help="Record an environment-gated skip in the evidence output.",
    )
    parser.add_argument("--evidence", type=Path, help="Write comparison evidence JSON.")
    parser.add_argument(
        "--min-cases",
        type=int,
        default=2,
        help="Minimum number of execution-surface cases required before comparison can pass.",
    )
    parser.add_argument(
        "--allow-empty",
        action="store_true",
        help="Allow cases with no phase summaries or trace events.",
    )
    parser.add_argument("--self-test", action="store_true", help="Run a built-in smoke test.")
    args = parser.parse_args(argv)

    if args.self_test:
        return run_self_test()

    if not args.case:
        parser.error("at least one --case is required")

    cases = []
    for label, surface, report_path, explicit_trace_path in args.case:
        report = read_json(report_path)
        trace_path = resolve_trace(report, explicit_trace_path)
        cases.append(summarize_case(label, surface, report_path, trace_path))

    mismatches = guard_mismatches(cases, args.min_cases, not args.allow_empty)
    mismatches.extend(compare(cases))
    evidence = {
        "schema_version": 1,
        "cases": cases,
        "skips": args.skip,
        "mismatches": mismatches,
        "status": "failed" if mismatches else "passed",
    }

    if args.evidence:
        args.evidence.parent.mkdir(parents=True, exist_ok=True)
        args.evidence.write_text(json.dumps(evidence, indent=2) + "\n", encoding="utf-8")

    print(json.dumps(evidence, indent=2))
    return 1 if mismatches else 0


if __name__ == "__main__":
    sys.exit(main())
