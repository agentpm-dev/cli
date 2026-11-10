"""Shared offline CLI transport fixture; never executes a Tool."""

import json
import os
import sys
import time
from pathlib import Path

if sys.argv[1:] == ["--version"]:
    print(os.environ.get("MOCK_CLI_VERSION", "agentpm 0.1.43"))
    sys.exit(0)
request = json.load(sys.stdin)
if path := os.environ.get("MOCK_CLI_CAPTURE"):
    Path(path).write_text(
        json.dumps({"argv": sys.argv[1:], "cwd": os.getcwd(), "request": request})
    )
spec = sys.argv[2]
name, version = spec.rsplit("@", 1)
case = request["input"].get("case", "success")
if case == "hang":
    time.sleep(30)
if case == "huge":
    sys.stdout.write("x" * (13 * 1024 * 1024))
    sys.exit(0)
if case == "malformed":
    print("stdout noise")
    sys.exit(0)
if case == "incomplete":
    print('{"schema_version":1')
    sys.exit(0)
if case == "wrong-schema":
    print(json.dumps({"schema_version": 2}))
    sys.exit(0)
if case in ("error", "timeout"):
    print(
        json.dumps(
            {
                "schema_version": 1,
                "status": "error",
                "error": {
                    "category": "timeout" if case == "timeout" else "runtime",
                    "message": "run agentpm install again "
                    + os.environ.get("MOCK_SECRET_TOKEN", ""),
                },
            }
        )
    )
    print("diagnostic " + os.environ.get("MOCK_SECRET_TOKEN", ""), file=sys.stderr)
    sys.exit(1)
print(
    json.dumps(
        {
            "schema_version": 1,
            "status": "success",
            "tool": {
                "package": [name] if case == "wrong-tool-type" else name,
                "version": "9.9.9" if case == "wrong-tool" else version,
            },
            "output": (
                {"ok": False, "error": "Tool business error"}
                if case == "tool-error"
                else request["input"]
            ),
        }
    )
)
if case == "nonzero":
    sys.exit(2)
