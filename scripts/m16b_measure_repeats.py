#!/usr/bin/env python3
"""Summarize Harness repeat behavior from an events.jsonl trace.

The exact-repeat metric is phase-scoped by default so intentional repetition
across phase boundaries does not inflate the within-phase repeat count. A
separate cross-phase counter is reported for Milestone 20A/20A.1 measurement.
"""

from __future__ import annotations

import argparse
import json
from collections import Counter, defaultdict
from pathlib import Path
from typing import Any


def stable_json(value: Any) -> str:
    return json.dumps(value if value is not None else {}, sort_keys=True, separators=(",", ":"))


def action_key(event: dict[str, Any]) -> tuple[str, str, str] | None:
    payload = event.get("payload") or {}
    if payload.get("payload_type") != "action":
        return None
    action_kind = payload.get("action_kind")
    identity = payload.get("identity")
    if not action_kind or not identity:
        return None
    fields = payload.get("fields") or {}
    return (str(action_kind), str(identity), stable_json(fields))


def load_events(path: Path) -> list[dict[str, Any]]:
    events: list[dict[str, Any]] = []
    with path.open("r", encoding="utf-8") as handle:
        for line_number, line in enumerate(handle, start=1):
            line = line.strip()
            if not line:
                continue
            try:
                events.append(json.loads(line))
            except json.JSONDecodeError as exc:
                raise SystemExit(f"{path}:{line_number}: invalid JSON: {exc}") from exc
    return events


def summarize(events: list[dict[str, Any]]) -> dict[str, Any]:
    by_phase: dict[str, Counter[tuple[str, str, str]]] = defaultdict(Counter)
    seen_phase_for_action: dict[tuple[str, str, str], set[str]] = defaultdict(set)
    phase_result_null_outputs = 0
    budget_prompt_count = 0
    budget_marker_count = 0
    output_stub_marker_count = 0

    for event in events:
        event_type = event.get("event_type")
        phase_execution_id = event.get("phase_execution_id") or "no-phase"
        if event_type == "semantic_action_proposed":
            key = action_key(event)
            if key is not None:
                by_phase[str(phase_execution_id)][key] += 1
                seen_phase_for_action[key].add(str(phase_execution_id))
        elif event_type == "phase_result_ready":
            payload = event.get("payload") or {}
            if payload.get("output") is None:
                phase_result_null_outputs += 1
        elif event_type == "prompt_prepared":
            fields = (event.get("payload") or {}).get("fields") or {}
            prompt = fields.get("prompt") or ""
            if "[cross-phase detail budget:" in prompt:
                budget_prompt_count += 1
            if "[cross-phase output detail marker:" in prompt:
                budget_marker_count += 1
            output_stub_marker_count += prompt.count("output: [omitted:")

    within_phase_exact_repeats = 0
    within_phase_repeat_details: list[dict[str, Any]] = []
    for phase_execution_id, counter in sorted(by_phase.items()):
        for key, count in counter.items():
            if count <= 1:
                continue
            repeats = count - 1
            within_phase_exact_repeats += repeats
            action_kind, identity, fields = key
            within_phase_repeat_details.append(
                {
                    "phase_execution_id": phase_execution_id,
                    "action_kind": action_kind,
                    "identity": identity,
                    "fields": json.loads(fields),
                    "count": count,
                    "repeats": repeats,
                }
            )

    cross_phase_exact_repeats = 0
    cross_phase_repeat_details: list[dict[str, Any]] = []
    for key, phases in sorted(seen_phase_for_action.items(), key=lambda item: item[0]):
        if len(phases) <= 1:
            continue
        repeats = len(phases) - 1
        cross_phase_exact_repeats += repeats
        action_kind, identity, fields = key
        cross_phase_repeat_details.append(
            {
                "action_kind": action_kind,
                "identity": identity,
                "fields": json.loads(fields),
                "phase_count": len(phases),
                "repeats": repeats,
            }
        )

    return {
        "events": len(events),
        "within_phase_exact_repeats": within_phase_exact_repeats,
        "cross_phase_exact_repeats": cross_phase_exact_repeats,
        "phase_result_null_outputs": phase_result_null_outputs,
        "section4_budget_prompts": budget_prompt_count,
        "section4_budget_marker_prompts": budget_marker_count,
        "section4_output_stub_markers": output_stub_marker_count,
        "within_phase_repeat_details": within_phase_repeat_details,
        "cross_phase_repeat_details": cross_phase_repeat_details,
    }


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("events_jsonl", type=Path)
    parser.add_argument("--json", action="store_true", help="emit machine-readable JSON")
    args = parser.parse_args()

    summary = summarize(load_events(args.events_jsonl))
    if args.json:
        print(json.dumps(summary, indent=2, sort_keys=True))
        return

    print(f"events: {summary['events']}")
    print(f"within_phase_exact_repeats: {summary['within_phase_exact_repeats']}")
    print(f"cross_phase_exact_repeats: {summary['cross_phase_exact_repeats']}")
    print(f"phase_result_null_outputs: {summary['phase_result_null_outputs']}")
    print(f"section4_budget_prompts: {summary['section4_budget_prompts']}")
    print(f"section4_budget_marker_prompts: {summary['section4_budget_marker_prompts']}")
    print(f"section4_output_stub_markers: {summary['section4_output_stub_markers']}")


if __name__ == "__main__":
    main()
