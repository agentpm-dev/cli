"""Deterministic local model fixture; no HTTP, credentials, Tools or LLM calls."""

import json
from pathlib import Path
import sys


def main():
    capture = Path(sys.argv[1])
    for line in sys.stdin:
        request = json.loads(line)
        assert request["protocol"] == "agentpm-service" and request["version"] == 1
        assert request["service"] == "model"
        with capture.open("a", encoding="utf-8") as stream:
            stream.write(json.dumps(request, ensure_ascii=True) + "\n")
        if request["kind"] == "initialize":
            result = {
                "registry_id": request["payload"]["registry_id"],
                "model": request["payload"]["model"],
                "ready": True,
                "capabilities": {
                    "semantic_actions": True,
                    "structured_output": True,
                    "multimodal_input": False,
                    "context_window_tokens": 32768,
                    "usage_reporting": False,
                },
            }
            kind = "initialized"
        else:
            assert request["kind"] == "request" and request["method"] == "generate"
            phase = request["payload"]["request"]["phase_id"]
            result = {
                "assistant_content": "Local M6A fixture completed " + phase,
                "actions": [
                    {
                        "id": "finish-" + phase,
                        "action": {
                            "type": "phase_completion",
                            "outcome": None,
                            "output": {"text": "PASS M6A fixture"},
                        },
                    }
                ],
                "finish_reason": "stop",
            }
            kind = "response"
        print(
            json.dumps(
                {
                    "protocol": "agentpm-service",
                    "version": 1,
                    "kind": kind,
                    "id": request["id"],
                    "service": "model",
                    "result": result,
                }
            ),
            flush=True,
        )


if __name__ == "__main__":
    main()
