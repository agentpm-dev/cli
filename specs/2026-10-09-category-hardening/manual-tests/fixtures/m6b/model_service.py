"""Scripted process model: request one Tool, then complete work and review."""

import json
from pathlib import Path
import sys


def turn(number):
    action = (
        {"type": "agent_pm_tool", "tool": "@fixture/echo", "arguments": {}}
        if number == 1
        else {"type": "phase_completion", "output": {"text": "PASS M6B fixture"}}
    )
    return {
        "assistant_content": None,
        "actions": [{"id": f"step-{number}", "action": action}],
        "finish_reason": "tool_calls",
    }


def main():
    capture = Path(sys.argv[1])
    calls = 0
    for line in sys.stdin:
        frame = json.loads(line)
        assert frame["protocol"] == "agentpm-service" and frame["version"] == 1
        assert frame["service"] == "model"
        if frame["kind"] == "initialize":
            result = {
                "registry_id": frame["payload"]["registry_id"],
                "model": frame["payload"]["model"],
                "ready": True,
                "capabilities": {
                    "semantic_actions": True,
                    "structured_output": True,
                    "multimodal_input": False,
                    "usage_reporting": False,
                },
            }
            kind = "initialized"
        else:
            assert frame["kind"] == "request" and frame["method"] == "generate"
            with capture.open("a", encoding="utf-8") as stream:
                stream.write(json.dumps(frame["payload"]["request"]) + "\n")
            calls += 1
            result, kind = turn(calls), "response"
        print(
            json.dumps(
                dict(
                    protocol="agentpm-service",
                    version=1,
                    kind=kind,
                    id=frame["id"],
                    service="model",
                    result=result,
                )
            ),
            flush=True,
        )


if __name__ == "__main__":
    main()
