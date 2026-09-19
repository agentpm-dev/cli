# Harness Milestone 17 Manual Tests

Manual coverage for Milestone 17: outward Agent-authored MCP exports, `agentpm serve --mcp --machine`, Harness-managed MCP export lifecycle, MCP-safe Tool exposure, shared-runner invocation, call activity reporting, restart policy visibility, and cleanup.

These tests use a disposable local Harness workspace. They do not require OpenAI, Anthropic, Redis, PostgreSQL, or a live external MCP server.

## Coverage Target

| Requirement | Manual coverage |
|---|---|
| `agentpm serve --mcp --machine` reports structured readiness and actual ephemeral endpoint | Tests 1 and 2 |
| Human `serve --mcp` remains usable | Test 3 |
| MCP JSON-RPC `initialize`, `tools/list`, and `tools/call` work over `/mcp` | Tests 1, 2, and 3 |
| MCP Tool calls use the shared Tool runner and preserve runner errors | Tests 2 and 5 |
| Tool-call machine events include canonical identity and MCP-safe name | Tests 2 and 6 |
| Harness starts one managed export per Agent `bindings.mcp` surface | Tests 4 and 6 |
| Exports use loopback plus ephemeral ports and honor `mcp.exports.host` | Tests 4 and 6 |
| Exported surfaces expose exactly the selected top-level Tools | Tests 4, 6, and 7 |
| MCP-only exported Tools do not need to be phase capabilities | Test 8 |
| Runtime-incompatible Tools are suppressed from a non-empty surface with diagnostics | Test 7 |
| Empty ready subset is marked unavailable rather than opening every Tool | Test 9 |
| Restart policy is configurable and visible; restart is no-replay and endpoint may change | Tests 10 and 11 |
| Headless post-run reports reconcile surface state without restarting then stopping | Test 12 |
| Report summaries retain both surface state and per-status call activity | Tests 6 and 12 |
| Managed Session shutdown stops owned MCP export subprocesses | Tests 6 and 13 |
| Focused automated regression coverage remains green | Test 14 |

## Prerequisites

Run from the root of the `agentpm` repo:

```bash
cargo build -p agentpm-cli
export APM="$PWD/target/debug/agentpm"
export AGENTPM_MANUAL_PYTHON="${AGENTPM_MANUAL_PYTHON:-python3}"
```

If your default `python3` is older than 3.10 on macOS, use the Homebrew Python that CI-friendly tests prefer:

```bash
export AGENTPM_MANUAL_PYTHON=/opt/homebrew/bin/python3.13
```

## Setup

Run this from the root of the `agentpm` repo:

```bash
cat > /tmp/setup-harness-m17-manual.sh <<'SH'
#!/usr/bin/env bash
set -euo pipefail

ROOT="$(pwd)"
APM="${APM:-$ROOT/target/debug/agentpm}"
BASE="${HARNESS_M17_TEST_BASE:-$ROOT/harness-m17-test}"
WORK="$BASE/workspace"
RUNS="$BASE/runs"
PYTHON_CMD="${AGENTPM_MANUAL_PYTHON:-python3}"
SCHEMA_URL="https://raw.githubusercontent.com/agentpm-dev/cli/refs/heads/main/schemas/agentpm.manifest.schema.json"

if [ ! -x "$APM" ]; then
  echo "Missing agentpm binary at $APM. Run: cargo build -p agentpm-cli" >&2
  exit 1
fi

rm -rf "$BASE"
mkdir -p "$WORK" "$RUNS"

pkg_dir() {
  local plural="$1"
  local package="$2"
  local version="$3"
  local without_at="${package#@}"
  local namespace="${without_at%%/*}"
  local name="${without_at#*/}"
  printf '%s/.agentpm/%s/%s/%s/%s' "$WORK" "$plural" "$namespace" "$name" "$version"
}

write_tool() {
  local package="$1"
  local version="$2"
  local description="$3"
  local runtime_type="$4"
  local timeout_ms="$5"
  local required_env="${6:-}"
  local mode="${7:-echo}"
  local manifest_name="${package#*/}"
  local runtime_version="3.10"
  local entrypoint_command="$PYTHON_CMD"
  local script_file="script.py"
  if [ "$runtime_type" = "node" ]; then
    runtime_version="999"
    entrypoint_command="node"
    script_file="script.js"
  fi
  local dir
  dir="$(pkg_dir tools "$package" "$version")"
  mkdir -p "$dir"
  cat > "$dir/agent.json" <<JSON
{
  "\$schema": "$SCHEMA_URL",
  "kind": "tool",
  "name": "$manifest_name",
  "version": "$version",
  "description": "$description",
  "entrypoint": {
    "command": "$entrypoint_command",
    "args": ["$script_file"],
    "cwd": ".",
    "timeout_ms": $timeout_ms,
    "env": {}
  },
  "runtime": {
    "type": "$runtime_type",
    "version": "$runtime_version"
  },
  "environment": {
    "vars": {
JSON
  if [ -n "$required_env" ]; then
    cat >> "$dir/agent.json" <<JSON
      "$required_env": {
        "description": "Required manual-test token for $package.",
        "required": true
      }
JSON
  fi
  cat >> "$dir/agent.json" <<'JSON'
    }
  },
  "inputs": {
    "type": "object",
    "additionalProperties": false,
    "properties": {
      "message": { "type": "string" },
      "query": { "type": "string" },
      "sleep_ms": { "type": "integer", "minimum": 0 }
    }
  },
  "outputs": {
    "type": "object",
    "additionalProperties": true,
    "properties": {
      "ok": { "type": "boolean" },
      "tool": { "type": "string" },
      "input": { "type": "object" },
      "env": { "type": "string" }
    },
    "required": ["ok", "tool", "input"]
  },
  "files": ["$script_file"]
}
JSON
  if [ "$runtime_type" = "node" ]; then
    cat > "$dir/$script_file" <<JS
const fs = require("fs");
const payload = JSON.parse(fs.readFileSync(0, "utf8"));
console.log(JSON.stringify({ ok: true, tool: "$package", input: payload }));
JS
  else
    cat > "$dir/$script_file" <<PY
#!/usr/bin/env python3
import json
import os
import sys
import time

payload = json.load(sys.stdin)
mode = "$mode"
if mode == "sleep":
    time.sleep(float(payload.get("sleep_ms", 2000)) / 1000.0)
if mode == "domain_error":
    print(json.dumps({"ok": False, "tool": "$package", "input": payload, "reason": "manual domain failure"}))
    sys.exit(0)
out = {"ok": True, "tool": "$package", "input": payload}
if "$required_env":
    out["env"] = os.environ.get("$required_env")
print(json.dumps(out))
PY
  fi
  chmod +x "$dir/$script_file"
  "$APM" lint "$dir/agent.json" >/dev/null
}

write_skill_with_transitive_tool() {
  local package="@zack/m17-skill-with-transitive-tool"
  local dir
  dir="$(pkg_dir skills "$package" "0.1.0")"
  mkdir -p "$dir"
  cat > "$dir/agent.json" <<JSON
{
  "\$schema": "$SCHEMA_URL",
  "kind": "skill",
  "name": "m17-skill-with-transitive-tool",
  "version": "0.1.0",
  "description": "M17 skill that depends on a Tool not listed as a top-level Agent Tool.",
  "skill": {
    "entrypoint": "SKILL.md"
  },
  "tools": [
    {
      "name": "@zack/m17-hidden-skill-tool",
      "version": "0.1.0"
    }
  ]
}
JSON
  cat > "$dir/SKILL.md" <<'MD'
# M17 Skill

This skill exists only to make `@zack/m17-hidden-skill-tool` installed as a Skill-transitive Tool.
MD
  "$APM" lint "$dir/agent.json" >/dev/null
}

write_loop() {
  local package="@zack/m17-mcp-export-loop"
  local dir
  dir="$(pkg_dir loops "$package" "0.1.0")"
  mkdir -p "$dir"
  cat > "$dir/agent.json" <<JSON
{
  "\$schema": "$SCHEMA_URL",
  "kind": "loop",
  "name": "m17-mcp-export-loop",
  "version": "0.1.0",
  "description": "M17 manual loop for MCP export checks.",
  "loop": {
    "archetype": "act_finish",
    "entry_phase": "finish",
    "limits": { "max_steps": 3 },
    "phases": [
      {
        "id": "finish",
        "objective": "Complete deterministically.",
        "access": {
          "tools": false,
          "knowledge": false,
          "memory": { "read": false, "write": false }
        },
        "outcomes": [
          { "id": "done", "description": "The M17 manual run is complete." }
        ]
      }
    ],
    "transitions": [
      { "from": "finish", "on": "done", "to": "\$end" }
    ]
  }
}
JSON
  "$APM" lint "$dir/agent.json" >/dev/null
}

write_agent_and_lock() {
  cat > "$WORK/agent.json" <<JSON
{
  "\$schema": "$SCHEMA_URL",
  "kind": "agent",
  "name": "m17-mcp-export-agent",
  "version": "0.1.0",
  "description": "M17 manual Harness Agent with outward MCP export bindings.",
  "tools": [
    "@zack/m17-echo-tool@0.1.0",
    "@zack/m17-env-tool@0.1.0",
    "@zack/m17-slow-tool@0.1.0",
    "@zack/m17-node-incompatible-tool@0.1.0"
  ],
  "skills": ["@zack/m17-skill-with-transitive-tool@0.1.0"],
  "loop": "@zack/m17-mcp-export-loop@0.1.0",
  "bindings": {
    "mcp": [
      {
        "id": "public-tools",
        "tools": ["@zack/m17-echo-tool", "@zack/m17-env-tool", "@zack/m17-node-incompatible-tool"]
      },
      {
        "id": "ops-tools",
        "tools": ["@zack/m17-slow-tool"]
      }
    ]
  }
}
JSON
  "$APM" lint "$WORK/agent.json" >/dev/null

  cat > "$WORK/agent.lock" <<'JSON'
{
  "lockfile_version": 3,
  "generated": "2026-09-15T00:00:00Z",
  "packages": {
    "tool:@zack/m17-echo-tool@0.1.0": {
      "kind": "tool",
      "name": "@zack/m17-echo-tool",
      "version": "0.1.0",
      "integrity": "sha256-manual"
    },
    "tool:@zack/m17-env-tool@0.1.0": {
      "kind": "tool",
      "name": "@zack/m17-env-tool",
      "version": "0.1.0",
      "integrity": "sha256-manual"
    },
    "tool:@zack/m17-slow-tool@0.1.0": {
      "kind": "tool",
      "name": "@zack/m17-slow-tool",
      "version": "0.1.0",
      "integrity": "sha256-manual"
    },
    "tool:@zack/m17-node-incompatible-tool@0.1.0": {
      "kind": "tool",
      "name": "@zack/m17-node-incompatible-tool",
      "version": "0.1.0",
      "integrity": "sha256-manual"
    },
    "tool:@zack/m17-hidden-skill-tool@0.1.0": {
      "kind": "tool",
      "name": "@zack/m17-hidden-skill-tool",
      "version": "0.1.0",
      "integrity": "sha256-manual"
    },
    "skill:@zack/m17-skill-with-transitive-tool@0.1.0": {
      "kind": "skill",
      "name": "@zack/m17-skill-with-transitive-tool",
      "version": "0.1.0",
      "integrity": "sha256-manual"
    },
    "loop:@zack/m17-mcp-export-loop@0.1.0": {
      "kind": "loop",
      "name": "@zack/m17-mcp-export-loop",
      "version": "0.1.0",
      "integrity": "sha256-manual"
    }
  },
  "roots": {
    "local:agent": {
      "name": "m17-mcp-export-agent",
      "version": "0.1.0",
      "tools": [
        "tool:@zack/m17-echo-tool@0.1.0",
        "tool:@zack/m17-env-tool@0.1.0",
        "tool:@zack/m17-slow-tool@0.1.0",
        "tool:@zack/m17-node-incompatible-tool@0.1.0"
      ],
      "skills": ["skill:@zack/m17-skill-with-transitive-tool@0.1.0"],
      "knowledge": [],
      "memory": [],
      "profiles": [],
      "loop": "loop:@zack/m17-mcp-export-loop@0.1.0"
    }
  }
}
JSON
}

write_model_and_configs() {
  mkdir -p "$WORK/runtime" "$WORK/scripts"
  cat > "$WORK/runtime/m17_model.py" <<'PY'
#!/usr/bin/env python3
import json
import sys

def emit(msg, kind, result=None, error=None):
    out = {
        "protocol": "agentpm-service",
        "version": 1,
        "kind": kind,
        "id": msg.get("id"),
        "service": "model",
    }
    if result is not None:
        out["result"] = result
    if error is not None:
        out["error"] = error
    print(json.dumps(out), flush=True)

for line in sys.stdin:
    msg = json.loads(line)
    kind = msg.get("kind")
    if kind == "initialize":
        emit(msg, "initialized", {
            "capabilities": {
                "semantic_actions": True,
                "structured_actions": True,
                "native_action_result_turns": True
            }
        })
    elif kind == "request":
        emit(msg, "response", {
            "assistant_content": None,
            "actions": [
                {
                    "id": "complete",
                    "action": {
                        "type": "phase_completion",
                        "outcome": "done",
                        "output": { "summary": "m17 complete" }
                    }
                }
            ],
            "finish_reason": "tool_calls",
            "usage": {"input_tokens": 1, "output_tokens": 1, "total_tokens": 2}
        })
    elif kind == "shutdown":
        emit(msg, "shutdown")
        break
    else:
        emit(msg, "error", error={"code": "unsupported", "message": kind})
PY
  chmod +x "$WORK/runtime/m17_model.py"

  cat > "$WORK/agentpm.harness.json" <<JSON
{
  "version": 1,
  "model": {
    "provider": "m17-model",
    "model": "deterministic"
  },
  "providers": {
    "models": {
      "m17-model": {
        "implementation": {
          "type": "process",
          "command": "$PYTHON_CMD",
          "args": ["runtime/m17_model.py"],
          "cwd": ".",
          "startup_timeout_ms": 5000,
          "request_timeout_ms": 30000,
          "restart": {
            "max_attempts": 0,
            "backoff_ms": 0
          }
        }
      }
    }
  },
  "mcp": {
    "exports": {
      "enabled": true,
      "host": "127.0.0.1",
      "restart": {
        "max_attempts": 1,
        "backoff_ms": 50
      }
    }
  },
  "runtime": {
    "state_dir": ".agentpm-state-m17",
    "limits": {
      "max_steps": 5,
      "max_model_calls_per_phase": 3,
      "max_tool_calls_per_phase": 3
    }
  },
  "trace": {
    "enabled": true,
    "level": "verbose",
    "content": "full"
  }
}
JSON

  cat > "$WORK/agentpm.no-restart.harness.json" <<JSON
{
  "version": 1,
  "model": {
    "provider": "m17-model",
    "model": "deterministic"
  },
  "providers": {
    "models": {
      "m17-model": {
        "implementation": {
          "type": "process",
          "command": "$PYTHON_CMD",
          "args": ["runtime/m17_model.py"],
          "cwd": ".",
          "startup_timeout_ms": 5000,
          "request_timeout_ms": 30000
        }
      }
    }
  },
  "mcp": {
    "exports": {
      "enabled": true,
      "host": "127.0.0.1",
      "restart": {
        "max_attempts": 0,
        "backoff_ms": 0
      }
    }
  },
  "runtime": {
    "state_dir": ".agentpm-state-m17-no-restart",
    "limits": {
      "max_steps": 5,
      "max_model_calls_per_phase": 3,
      "max_tool_calls_per_phase": 3
    }
  },
  "trace": {
    "enabled": true,
    "level": "verbose",
    "content": "full"
  }
}
JSON

  cat > "$WORK/agentpm.exports-disabled.harness.json" <<JSON
{
  "version": 1,
  "model": {
    "provider": "m17-model",
    "model": "deterministic"
  },
  "providers": {
    "models": {
      "m17-model": {
        "implementation": {
          "type": "process",
          "command": "$PYTHON_CMD",
          "args": ["runtime/m17_model.py"],
          "cwd": "."
        }
      }
    }
  },
  "mcp": {
    "exports": {
      "enabled": false,
      "host": "127.0.0.1"
    }
  },
  "runtime": {
    "state_dir": ".agentpm-state-m17-disabled"
  },
  "trace": {
    "enabled": true,
    "level": "verbose",
    "content": "full"
  }
}
JSON
}

write_helpers() {
  cat > "$WORK/scripts/mcp_client.py" <<'PY'
#!/usr/bin/env python3
import argparse
import json
import urllib.request

def rpc(endpoint, method, params=None, ident=1):
    payload = {"jsonrpc": "2.0", "id": ident, "method": method}
    if params is not None:
        payload["params"] = params
    data = json.dumps(payload).encode()
    req = urllib.request.Request(endpoint, data=data, headers={"content-type": "application/json"})
    with urllib.request.urlopen(req, timeout=10) as response:
        return json.loads(response.read().decode())

parser = argparse.ArgumentParser()
parser.add_argument("endpoint")
parser.add_argument("method", choices=["initialize", "tools/list", "tools/call"])
parser.add_argument("--name")
parser.add_argument("--arguments", default="{}")
args = parser.parse_args()

params = {}
if args.method == "tools/call":
    params = {"name": args.name, "arguments": json.loads(args.arguments)}
elif args.method == "initialize":
    params = {}
elif args.method == "tools/list":
    params = {}
print(json.dumps(rpc(args.endpoint, args.method, params), indent=2, sort_keys=True))
PY
  chmod +x "$WORK/scripts/mcp_client.py"

  cat > "$WORK/scripts/machine_session.py" <<'PY'
#!/usr/bin/env python3
import argparse
import json
import os
import select
import signal
import subprocess
import sys
import time
import urllib.request
from pathlib import Path

ROOT = Path.cwd()
APM = os.environ.get("APM", str(ROOT / "../../target/debug/agentpm"))
PYTHON = os.environ.get("AGENTPM_MANUAL_PYTHON", "python3")

def send(proc, method, payload=None, ident=None):
    ident = ident or method
    frame = {
        "protocol": "agentpm-harness-machine",
        "version": 1,
        "kind": "request",
        "id": ident,
        "method": method,
        "payload": payload or {},
    }
    proc.stdin.write(json.dumps(frame) + "\n")
    proc.stdin.flush()

def read_until(proc, predicate, timeout=20):
    deadline = time.time() + timeout
    while time.time() < deadline:
        ready, _, _ = select.select([proc.stdout], [], [], 0.1)
        if not ready:
            continue
        line = proc.stdout.readline()
        if not line:
            break
        frame = json.loads(line)
        print(json.dumps(frame), flush=True)
        if predicate(frame):
            return frame
    raise SystemExit("timed out waiting for machine frame")

def drain_available(proc, duration=1.0):
    frames = []
    deadline = time.time() + duration
    while time.time() < deadline:
        ready, _, _ = select.select([proc.stdout], [], [], 0.1)
        if not ready:
            continue
        line = proc.stdout.readline()
        if not line:
            break
        frame = json.loads(line)
        print(json.dumps(frame), flush=True)
        frames.append(frame)
    return frames

def rpc(endpoint, method, params=None, ident=1):
    payload = {"jsonrpc": "2.0", "id": ident, "method": method}
    if params is not None:
        payload["params"] = params
    req = urllib.request.Request(endpoint, data=json.dumps(payload).encode(), headers={"content-type": "application/json"})
    with urllib.request.urlopen(req, timeout=10) as response:
        return json.loads(response.read().decode())

parser = argparse.ArgumentParser()
parser.add_argument("--config", default="agentpm.harness.json")
parser.add_argument("--state-dir", default=None)
parser.add_argument("--out", required=True)
parser.add_argument("--call", action="store_true")
parser.add_argument("--start-run", action="store_true")
parser.add_argument("--shutdown", action="store_true")
parser.add_argument("--kill-first-surface", action="store_true")
args = parser.parse_args()

cmd = [APM, "harness", "--config", args.config, "--machine"]
if args.state_dir:
    cmd += ["--state-dir", args.state_dir]
proc = subprocess.Popen(cmd, cwd=ROOT, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, bufsize=1)
frames = []

try:
    send(proc, "initialize", {}, "init-1")
    init = read_until(proc, lambda frame: frame.get("kind") == "response" and frame.get("id") == "init-1")
    frames.append(init)
    exports = init["payload"].get("mcp_exports", [])
    Path(args.out).parent.mkdir(parents=True, exist_ok=True)
    Path(args.out).write_text(json.dumps({"initialize": init, "mcp_exports": exports}, indent=2, sort_keys=True))
    print(f"MCP_EXPORTS={json.dumps(exports)}", file=sys.stderr)

    if args.call:
        for surface in exports:
            if not surface.get("tools"):
                continue
            endpoint = surface["endpoint"]
            listed = rpc(endpoint, "tools/list", {})
            print(json.dumps({"surface": surface["id"], "tools_list": listed}), flush=True)
            tool_name = listed["result"]["tools"][0]["name"]
            called = rpc(endpoint, "tools/call", {"name": tool_name, "arguments": {"message": f"hello from {surface['id']}", "sleep_ms": 10}})
            print(json.dumps({"surface": surface["id"], "tool_call": called}), flush=True)
            frames.extend(drain_available(proc, duration=2.0))

    if args.kill_first_surface and exports:
        # Kill the first child by extracting the port from the endpoint and using lsof if available.
        endpoint = exports[0]["endpoint"]
        port = endpoint.rsplit(":", 1)[1].split("/", 1)[0]
        subprocess.run(
            ["bash", "-lc", f"pids=$(lsof -ti tcp:{port} || true); if [ -n \"$pids\" ]; then kill $pids; fi"],
            check=False,
        )
        time.sleep(0.5)
        send(proc, "preflight", {}, "preflight-after-kill")
        refresh = read_until(proc, lambda frame: frame.get("kind") == "response" and frame.get("id") == "preflight-after-kill")
        frames.append(refresh)

    if args.start_run:
        send(proc, "start_run", {"input": "Complete the M17 manual run."}, "run-1")
        run = read_until(proc, lambda frame: frame.get("kind") == "response" and frame.get("id") == "run-1", timeout=30)
        frames.append(run)

    if args.shutdown:
        send(proc, "shutdown", {}, "shutdown-1")
        frames.append(read_until(proc, lambda frame: frame.get("kind") == "response" and frame.get("id") == "shutdown-1"))
finally:
    if proc.poll() is None:
        proc.terminate()
        try:
            proc.wait(timeout=5)
        except subprocess.TimeoutExpired:
            proc.kill()
    stderr = proc.stderr.read()
    if stderr:
        print(stderr, file=sys.stderr)
PY
  chmod +x "$WORK/scripts/machine_session.py"

  cat > "$WORK/scripts/inspect_m17.py" <<'PY'
#!/usr/bin/env python3
import argparse
import json
from pathlib import Path

parser = argparse.ArgumentParser()
parser.add_argument("path")
args = parser.parse_args()
path = Path(args.path)
if path.is_dir():
    events = sorted(path.glob("runs/*/events.jsonl"))
    reports = sorted(path.glob("runs/*/report.json"))
    print("events:", [str(p) for p in events])
    print("reports:", [str(p) for p in reports])
    for report_path in reports:
        report = json.loads(report_path.read_text())
        print(json.dumps({
            "report": str(report_path),
            "terminal_status": report.get("terminal_status"),
            "mcp_summaries": report.get("mcp_summaries", []),
        }, indent=2, sort_keys=True))
else:
    rows = [json.loads(line) for line in path.read_text().splitlines() if line.strip()]
    counts = {}
    for row in rows:
        counts[row.get("event_type")] = counts.get(row.get("event_type"), 0) + 1
    print(json.dumps(counts, indent=2, sort_keys=True))
    for row in rows:
        if row.get("event_type", "").startswith("mcp_") or row.get("event_type", "").startswith("service_"):
            print(json.dumps(row, sort_keys=True))
PY
  chmod +x "$WORK/scripts/inspect_m17.py"
}

write_tool "@zack/m17-echo-tool" "0.1.0" "M17 echo Tool exported through MCP." "python" 5000 "" "echo"
write_tool "@zack/m17-env-tool" "0.1.0" "M17 env Tool exported through MCP." "python" 5000 "M17_REQUIRED_TOKEN" "echo"
write_tool "@zack/m17-slow-tool" "0.1.0" "M17 slow Tool for cancellation and timeout checks." "python" 5000 "" "sleep"
write_tool "@zack/m17-node-incompatible-tool" "0.1.0" "M17 runtime-incompatible Tool that should be suppressed from managed exports." "node" 5000 "" "echo"
write_tool "@zack/m17-hidden-skill-tool" "0.1.0" "M17 Skill-transitive Tool that must not be exported unless top-level." "python" 5000 "" "echo"
write_skill_with_transitive_tool
write_loop
write_agent_and_lock
write_model_and_configs
write_helpers

cat > "$BASE/env.sh" <<ENV
export APM="$APM"
export M17_BASE="$BASE"
export M17_WORK="$WORK"
export M17_RUNS="$RUNS"
export AGENTPM_MANUAL_PYTHON="$PYTHON_CMD"
ENV

echo "M17 manual workspace ready:"
echo "  source $BASE/env.sh"
echo "  workspace: $WORK"
echo "  runs:      $RUNS"
SH

chmod +x /tmp/setup-harness-m17-manual.sh
/tmp/setup-harness-m17-manual.sh
source harness-m17-test/env.sh
```

## Test 1: direct machine `serve --mcp` readiness uses an ephemeral endpoint

```bash
source harness-m17-test/env.sh
mkdir -p "$M17_RUNS/direct-serve"

(cd "$M17_WORK" && "$APM" serve --mcp --machine --port 0 --tool @zack/m17-echo-tool \
  >"$M17_RUNS/direct-serve/stdout.jsonl" \
  2>"$M17_RUNS/direct-serve/stderr.txt") &
SERVE_PID=$!
trap 'kill "$SERVE_PID" 2>/dev/null || true' EXIT

python3 - <<'PY'
import json, os, time
from pathlib import Path
path = Path(os.environ["M17_RUNS"]) / "direct-serve/stdout.jsonl"
deadline = time.time() + 10
while time.time() < deadline:
    if path.exists():
        rows = [json.loads(line) for line in path.read_text().splitlines() if line.strip()]
        ready = [row for row in rows if row.get("event") == "ready"]
        if ready:
            print(json.dumps(ready[-1], indent=2, sort_keys=True))
            assert ready[-1]["fields"]["port"] != 0
            assert ready[-1]["fields"]["endpoint"].endswith("/mcp")
            assert ready[-1]["fields"]["tools"][0]["mcp_name"] == "zack__m17_echo_tool"
            Path(os.environ["M17_RUNS"], "direct-serve/endpoint.txt").write_text(ready[-1]["fields"]["endpoint"])
            raise SystemExit(0)
    time.sleep(0.1)
raise SystemExit("ready event not found")
PY
```

Expected:

- stdout is JSONL machine protocol only;
- first `ready` event includes `endpoint`, non-zero `port`, and `zack__m17_echo_tool`;
- stderr does not need to be parsed for the endpoint.

Cleanup:

```bash
kill "$SERVE_PID" 2>/dev/null || true
trap - EXIT
```

## Test 2: direct MCP `tools/list` and `tools/call` emit call events

```bash
source harness-m17-test/env.sh
mkdir -p "$M17_RUNS/direct-call"

(cd "$M17_WORK" && "$APM" serve --mcp --machine --port 0 --tool @zack/m17-echo-tool \
  >"$M17_RUNS/direct-call/stdout.jsonl" \
  2>"$M17_RUNS/direct-call/stderr.txt") &
SERVE_PID=$!
trap 'kill "$SERVE_PID" 2>/dev/null || true' EXIT

python3 - <<'PY'
import json, os, time
from pathlib import Path
path = Path(os.environ["M17_RUNS"]) / "direct-call/stdout.jsonl"
endpoint_path = Path(os.environ["M17_RUNS"]) / "direct-call/endpoint.txt"
deadline = time.time() + 10
while time.time() < deadline:
    if path.exists():
        rows = [json.loads(line) for line in path.read_text().splitlines() if line.strip()]
        ready = [row for row in rows if row.get("event") == "ready"]
        if ready:
            endpoint_path.write_text(ready[-1]["fields"]["endpoint"])
            raise SystemExit(0)
    time.sleep(0.1)
raise SystemExit("ready event not found")
PY

ENDPOINT="$(cat "$M17_RUNS/direct-call/endpoint.txt")"
(cd "$M17_WORK" && "$AGENTPM_MANUAL_PYTHON" scripts/mcp_client.py "$ENDPOINT" initialize) \
  >"$M17_RUNS/direct-call/initialize.json"
(cd "$M17_WORK" && "$AGENTPM_MANUAL_PYTHON" scripts/mcp_client.py "$ENDPOINT" tools/list) \
  >"$M17_RUNS/direct-call/tools-list.json"
(cd "$M17_WORK" && "$AGENTPM_MANUAL_PYTHON" scripts/mcp_client.py "$ENDPOINT" tools/call \
  --name zack__m17_echo_tool \
  --arguments '{"message":"m17 direct call"}') \
  >"$M17_RUNS/direct-call/tool-call.json"

python3 - <<'PY'
import json, os
from pathlib import Path
base = Path(os.environ["M17_RUNS"]) / "direct-call"
body = json.loads((base / "tool-call.json").read_text())
assert body["result"]["structuredContent"]["ok"] is True, body
rows = [json.loads(line) for line in (base / "stdout.jsonl").read_text().splitlines() if line.strip()]
events = [row["event"] for row in rows]
assert "tool_call_started" in events, events
assert "tool_call_completed" in events, events
for row in rows:
    if row.get("event") == "tool_call_completed":
        assert row["fields"]["identity"] == "@zack/m17-echo-tool"
        assert row["fields"]["mcp_name"] == "zack__m17_echo_tool"
print("direct MCP call OK")
PY
```

Expected:

- `tools/list` exposes exactly the selected echo Tool;
- `tools/call` returns `structuredContent.ok: true`;
- machine stdout includes `tool_call_started` and `tool_call_completed` with canonical identity and MCP-safe name.

Cleanup:

```bash
kill "$SERVE_PID" 2>/dev/null || true
trap - EXIT
```

## Test 3: human `serve --mcp` still works

```bash
source harness-m17-test/env.sh
mkdir -p "$M17_RUNS/human-serve"

(cd "$M17_WORK" && "$APM" serve --mcp --port 0 --tool @zack/m17-echo-tool \
  >"$M17_RUNS/human-serve/stdout.txt" \
  2>"$M17_RUNS/human-serve/stderr.txt") &
SERVE_PID=$!
trap 'kill "$SERVE_PID" 2>/dev/null || true' EXIT

"$AGENTPM_MANUAL_PYTHON" - <<'PY'
import os, re, time
from pathlib import Path
stderr = Path(os.environ["M17_RUNS"]) / "human-serve/stderr.txt"
deadline = time.time() + 10
while time.time() < deadline:
    text = stderr.read_text() if stderr.exists() else ""
    match = re.search(r"http://127\.0\.0\.1:\d+", text)
    if match:
        endpoint = match.group(0) + "/mcp"
        Path(os.environ["M17_RUNS"], "human-serve/endpoint.txt").write_text(endpoint)
        print(endpoint)
        raise SystemExit(0)
    time.sleep(0.1)
raise SystemExit("human serve endpoint not found in stderr")
PY

ENDPOINT="$(cat "$M17_RUNS/human-serve/endpoint.txt")"
(cd "$M17_WORK" && "$AGENTPM_MANUAL_PYTHON" scripts/mcp_client.py "$ENDPOINT" tools/list) \
  >"$M17_RUNS/human-serve/tools-list.json"
cat "$M17_RUNS/human-serve/tools-list.json"
```

Expected:

- human mode prints a listening URL to stderr as before;
- `tools/list` succeeds;
- stdout is not the machine JSONL stream.

Cleanup:

```bash
kill "$SERVE_PID" 2>/dev/null || true
trap - EXIT
```

## Test 4: Harness preflight reports MCP export configuration

```bash
source harness-m17-test/env.sh
mkdir -p "$M17_RUNS/preflight"

(cd "$M17_WORK" && "$APM" harness --config agentpm.harness.json --verbose) \
  >"$M17_RUNS/preflight/stdout.txt" \
  2>"$M17_RUNS/preflight/stderr.txt"

sed -n '/MCP exports:/,/Diagnostics:/p' "$M17_RUNS/preflight/stdout.txt"
```

Expected:

- preflight shows `MCP exports:`;
- enabled is `true`;
- host is `127.0.0.1`;
- restart shows `max_attempts=1, backoff_ms=50`;
- surfaces include `public-tools` and `ops-tools`;
- this is static preflight, so it does not show live ports.

## Test 5: missing Tool env fails at shared-runner invocation, not readiness

```bash
source harness-m17-test/env.sh
mkdir -p "$M17_RUNS/env-failure"

(cd "$M17_WORK" && "$APM" serve --mcp --machine --port 0 --tool @zack/m17-env-tool \
  >"$M17_RUNS/env-failure/stdout.jsonl" \
  2>"$M17_RUNS/env-failure/stderr.txt") &
SERVE_PID=$!
trap 'kill "$SERVE_PID" 2>/dev/null || true' EXIT

python3 - <<'PY'
import json, os, time
from pathlib import Path
path = Path(os.environ["M17_RUNS"]) / "env-failure/stdout.jsonl"
deadline = time.time() + 10
while time.time() < deadline:
    rows = [json.loads(line) for line in path.read_text().splitlines() if line.strip()] if path.exists() else []
    ready = [row for row in rows if row.get("event") == "ready"]
    if ready:
        Path(os.environ["M17_RUNS"], "env-failure/endpoint.txt").write_text(ready[-1]["fields"]["endpoint"])
        raise SystemExit(0)
    time.sleep(0.1)
raise SystemExit("ready event not found")
PY

ENDPOINT="$(cat "$M17_RUNS/env-failure/endpoint.txt")"
(cd "$M17_WORK" && env -u M17_REQUIRED_TOKEN "$AGENTPM_MANUAL_PYTHON" scripts/mcp_client.py "$ENDPOINT" tools/call \
  --name zack__m17_env_tool \
  --arguments '{"message":"requires env"}') \
  >"$M17_RUNS/env-failure/tool-call.json" || true

python3 - <<'PY'
import json, os
from pathlib import Path
base = Path(os.environ["M17_RUNS"]) / "env-failure"
body = json.loads((base / "tool-call.json").read_text())
assert body["error"]["code"] == -32001, body
assert "missing required environment variables" in body["error"]["message"], body
rows = [json.loads(line) for line in (base / "stdout.jsonl").read_text().splitlines() if line.strip()]
assert any(row.get("event") == "tool_call_started" for row in rows)
assert any(row.get("event") == "tool_call_failed" for row in rows)
print("missing env fails at invocation as expected")
PY
```

Expected:

- server starts and reports ready;
- `tools/call` fails with a runtime error because `M17_REQUIRED_TOKEN` is absent;
- machine stdout includes started and failed call events.

Cleanup:

```bash
kill "$SERVE_PID" 2>/dev/null || true
trap - EXIT
```

## Test 6: Harness machine Session starts managed exports and forwards call activity

```bash
source harness-m17-test/env.sh
mkdir -p "$M17_RUNS/machine-session"

(cd "$M17_WORK" && "$AGENTPM_MANUAL_PYTHON" scripts/machine_session.py \
  --config agentpm.harness.json \
  --out "$M17_RUNS/machine-session/session.json" \
  --call \
  --start-run \
  --shutdown) \
  >"$M17_RUNS/machine-session/stdout.jsonl" \
  2>"$M17_RUNS/machine-session/stderr.txt"

python3 - <<'PY'
import json, os
from pathlib import Path
base = Path(os.environ["M17_RUNS"]) / "machine-session"
session = json.loads((base / "session.json").read_text())
exports = session["mcp_exports"]
assert len(exports) == 2, exports
assert {export["id"] for export in exports} == {"public-tools", "ops-tools"}
for export in exports:
    assert export["host"] == "127.0.0.1"
    assert export["port"] != 0
    assert export["endpoint"].endswith("/mcp")
rows = [json.loads(line) for line in (base / "stdout.jsonl").read_text().splitlines() if line.strip()]
events = [row for row in rows if row.get("method") == "mcp_export_event"]
assert any(row["payload"]["event"]["event"] == "tool_call_completed" for row in events), events
run_responses = [row for row in rows if row.get("kind") == "response" and row.get("id") == "run-1"]
assert run_responses, rows
summaries = run_responses[-1]["payload"]["report"]["mcp_summaries"]
assert any(s["operation_kind"] == "mcp_export" and s["status"] == "ready" for s in summaries), summaries
assert any(s["operation_kind"] == "mcp_tool_call" and s["status"] == "completed" for s in summaries), summaries
assert any(row.get("id") == "shutdown-1" and row.get("kind") == "response" for row in rows)
print("machine managed exports OK")
PY
```

Expected:

- `initialize` returns two live `mcp_exports`;
- both endpoints are loopback, ephemeral, and end in `/mcp`;
- external MCP calls complete while no Harness Run is active;
- call activity is forwarded through Harness machine events;
- `start_run` report includes both `mcp_export` surface state and `mcp_tool_call` activity summaries;
- shutdown stops the managed surfaces.

## Test 7: ready subset suppresses runtime-incompatible Tool

```bash
source harness-m17-test/env.sh
mkdir -p "$M17_RUNS/ready-subset"

(cd "$M17_WORK" && "$AGENTPM_MANUAL_PYTHON" scripts/machine_session.py \
  --config agentpm.harness.json \
  --out "$M17_RUNS/ready-subset/session.json" \
  --shutdown) \
  >"$M17_RUNS/ready-subset/stdout.jsonl" \
  2>"$M17_RUNS/ready-subset/stderr.txt"

python3 - <<'PY'
import json, os
from pathlib import Path
session = json.loads((Path(os.environ["M17_RUNS"]) / "ready-subset/session.json").read_text())
public = [export for export in session["mcp_exports"] if export["id"] == "public-tools"][0]
tool_ids = public["tools"]
assert "@zack/m17-echo-tool" in tool_ids, tool_ids
assert "@zack/m17-env-tool" in tool_ids, tool_ids
assert "@zack/m17-node-incompatible-tool" not in tool_ids, tool_ids
print(tool_ids)
PY
```

Expected:

- `public-tools` remains available because it has ready Tools;
- runtime-incompatible `@zack/m17-node-incompatible-tool` is omitted from the live surface.

## Test 8: MCP-only exported Tool is not a phase capability

The loop phase has no Tool access, but the Agent exports Tools through MCP.

```bash
source harness-m17-test/env.sh
mkdir -p "$M17_RUNS/mcp-only"

(cd "$M17_WORK" && "$APM" harness \
  --config agentpm.harness.json \
  --headless \
  --input "Complete the M17 manual run." \
  --report "$M17_RUNS/mcp-only/report.json") \
  >"$M17_RUNS/mcp-only/stdout.txt" \
  2>"$M17_RUNS/mcp-only/stderr.txt"

python3 - <<'PY'
import json, os
from pathlib import Path
report = json.loads((Path(os.environ["M17_RUNS"]) / "mcp-only/report.json").read_text())
assert report["terminal_status"] == "ended", report
assert not report["tool_summaries"], report["tool_summaries"]
assert any(s["operation_kind"] == "mcp_export" for s in report["mcp_summaries"]), report["mcp_summaries"]
print(json.dumps(report["mcp_summaries"], indent=2, sort_keys=True))
PY
```

Expected:

- Run completes without phase Tool capabilities;
- report still contains MCP export summaries because exports are Session-owned, not phase-bound actions.

## Test 9: exports disabled starts no surfaces

```bash
source harness-m17-test/env.sh
mkdir -p "$M17_RUNS/exports-disabled"

(cd "$M17_WORK" && "$APM" harness --config agentpm.exports-disabled.harness.json --verbose) \
  >"$M17_RUNS/exports-disabled/preflight.txt" \
  2>"$M17_RUNS/exports-disabled/preflight.err"

sed -n '/MCP exports:/,/Diagnostics:/p' "$M17_RUNS/exports-disabled/preflight.txt"

(cd "$M17_WORK" && "$APM" harness \
  --config agentpm.exports-disabled.harness.json \
  --headless \
  --input "Complete the M17 manual run." \
  --report "$M17_RUNS/exports-disabled/report.json") \
  >"$M17_RUNS/exports-disabled/stdout.txt" \
  2>"$M17_RUNS/exports-disabled/stderr.txt"

python3 - <<'PY'
import json, os
from pathlib import Path
report = json.loads((Path(os.environ["M17_RUNS"]) / "exports-disabled/report.json").read_text())
assert report["terminal_status"] == "ended"
assert report["mcp_summaries"] == [], report["mcp_summaries"]
print("exports disabled OK")
PY
```

Expected:

- preflight shows `enabled: false`;
- headless report contains no MCP summaries.

## Test 10: restart policy is visible and disabling restart marks failed

```bash
source harness-m17-test/env.sh
mkdir -p "$M17_RUNS/no-restart"

(cd "$M17_WORK" && "$AGENTPM_MANUAL_PYTHON" scripts/machine_session.py \
  --config agentpm.no-restart.harness.json \
  --out "$M17_RUNS/no-restart/session.json" \
  --kill-first-surface \
  --shutdown) \
  >"$M17_RUNS/no-restart/stdout.jsonl" \
  2>"$M17_RUNS/no-restart/stderr.txt"

python3 - <<'PY'
import json, os
from pathlib import Path
rows = [json.loads(line) for line in (Path(os.environ["M17_RUNS"]) / "no-restart/stdout.jsonl").read_text().splitlines() if line.strip()]
refresh = [row for row in rows if row.get("id") == "preflight-after-kill" and row.get("kind") == "response"]
assert refresh, rows
exports = refresh[-1]["payload"]["mcp_exports"]
assert any(export["state"] == "failed" for export in exports), exports
assert not any(row.get("method") == "mcp_export_event" and row["payload"]["event"].get("event") == "ready" for row in rows)
print(json.dumps(exports, indent=2, sort_keys=True))
PY
```

Expected:

- config `max_attempts: 0` disables managed restart;
- after the surface process is killed and machine `preflight` refreshes, the surface state becomes `failed`;
- no replacement endpoint is published.

## Test 11: restart can publish a new endpoint

```bash
source harness-m17-test/env.sh
mkdir -p "$M17_RUNS/restart"

(cd "$M17_WORK" && "$AGENTPM_MANUAL_PYTHON" scripts/machine_session.py \
  --config agentpm.harness.json \
  --out "$M17_RUNS/restart/session.json" \
  --kill-first-surface \
  --shutdown) \
  >"$M17_RUNS/restart/stdout.jsonl" \
  2>"$M17_RUNS/restart/stderr.txt"

python3 - <<'PY'
import json, os
from pathlib import Path
base = Path(os.environ["M17_RUNS"]) / "restart"
session = json.loads((base / "session.json").read_text())
old = {export["id"]: export["endpoint"] for export in session["mcp_exports"]}
rows = [json.loads(line) for line in (base / "stdout.jsonl").read_text().splitlines() if line.strip()]
refresh = [row for row in rows if row.get("id") == "preflight-after-kill" and row.get("kind") == "response"]
assert refresh, rows
new = {export["id"]: export["endpoint"] for export in refresh[-1]["payload"]["mcp_exports"]}
print("old:", old)
print("new:", new)
assert any(export["state"] == "ready" for export in refresh[-1]["payload"]["mcp_exports"]), refresh[-1]
assert old.keys() == new.keys()
print("A restarted surface may or may not reuse a port on a given OS run; clients must rediscover either way.")
PY
```

Expected:

- with `max_attempts: 1`, refresh after a killed surface attempts restart;
- refreshed machine payload reports current live endpoints;
- endpoint may change because the managed child binds `--port 0`;
- this is a rediscovery contract, not a stable URL contract.

## Test 12: headless final report reconciles failed surface without post-run restart

This check is easiest to prove with the focused automated regression plus one headless report smoke. The behavior is also covered by `mcp_export_report_refresh_marks_failed_without_restart_attempt`.

```bash
source harness-m17-test/env.sh
mkdir -p "$M17_RUNS/headless-report"

cargo test -p agentpm-cli commands::harness::tests::mcp_export_report_refresh_marks_failed_without_restart_attempt -- --nocapture \
  >"$M17_RUNS/headless-report/focused-test.txt" \
  2>"$M17_RUNS/headless-report/focused-test.err"

(cd "$M17_WORK" && "$APM" harness \
  --config agentpm.harness.json \
  --headless \
  --input "Complete the M17 manual run." \
  --report "$M17_RUNS/headless-report/report.json") \
  >"$M17_RUNS/headless-report/stdout.txt" \
  2>"$M17_RUNS/headless-report/stderr.txt"

python3 - <<'PY'
import json, os
from pathlib import Path
report = json.loads((Path(os.environ["M17_RUNS"]) / "headless-report/report.json").read_text())
assert report["terminal_status"] == "ended"
assert any(summary["operation_kind"] == "mcp_export" for summary in report["mcp_summaries"]), report["mcp_summaries"]
print(json.dumps(report["mcp_summaries"], indent=2, sort_keys=True))
PY
```

Expected:

- focused regression passes;
- headless report includes surface summaries;
- no manual claim is made that headless provides continuous in-run MCP restart/rediscovery.

## Test 13: managed shutdown leaves no listening MCP child for captured endpoints

Use the output from Test 6.

```bash
source harness-m17-test/env.sh

python3 - <<'PY'
import json, os, socket
from pathlib import Path
session = json.loads((Path(os.environ["M17_RUNS"]) / "machine-session/session.json").read_text())
for export in session["mcp_exports"]:
    host = export["host"]
    port = int(export["port"])
    sock = socket.socket()
    sock.settimeout(0.5)
    try:
        sock.connect((host, port))
        raise AssertionError(f"endpoint still accepts connections after shutdown: {export}")
    except OSError:
        pass
    finally:
        sock.close()
print("managed MCP export children stopped")
PY
```

Expected:

- after the machine Session shutdown, previously captured endpoints no longer accept connections.

## Test 14: focused automated M17 regression suite

Run these before handoff:

```bash
cargo test -p agentpm-cli commands::serve::tests:: -- --nocapture
cargo test -p agentpm-cli commands::harness::tests::mcp_export -- --nocapture
cargo test -p agentpm-cli commands::harness::tests::machine_run_report_preserves_mcp_export_surface_and_activity_summaries -- --nocapture
cargo test -p agentpm-cli commands::harness::tests::human_preflight_reports_mcp_export_surfaces -- --nocapture
cargo test -p agentpm-cli runner::tests::interpreter_resolution_uses_run_options_env_overrides -- --nocapture
```

Expected:

- all focused tests pass;
- the interpreter override regression stays green because MCP serve calls flow through the shared runner.

## Cleanup

The setup writes only inside `harness-m17-test/` plus `/tmp/setup-harness-m17-manual.sh`.

```bash
rm -f /tmp/setup-harness-m17-manual.sh
rm -rf harness-m17-test
```
