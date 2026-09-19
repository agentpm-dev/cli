# Harness Milestone 18 Manual Tests

Manual coverage for Milestone 18: externally imported MCP servers, explicit import scope, Tool filtering, canonical/provider alias mapping, Hook/retry/repair behavior through the shared Tool pipeline, sanitized readiness diagnostics, and one loopback scenario where one Harness session exports an MCP surface and another Harness run imports it.

These tests use disposable local workspaces. They do not require OpenAI, Anthropic, Redis, PostgreSQL, or a live external MCP service.

## Coverage Target

| Requirement | Manual coverage |
|---|---|
| Stdio MCP imports initialize, list Tools, filter selected Tools, and dispatch `tools/call` | Tests 1 and 2 |
| HTTP MCP imports initialize with env-backed headers and dispatch `tools/call` | Test 3 |
| Resolved env/header secrets are not emitted in preflight, trace, report, or readiness reasons | Tests 1, 3, and 8 |
| Phase-scoped imports only enter matching phases and honor `access.tools=false` | Test 4 |
| Duplicate Tool names across different MCP servers remain distinct by canonical identity and provider alias metadata | Test 5 |
| Provider-facing MCP schema reduction is diagnosed; canonical MCP schema remains authoritative for validation/repair | Tests 6, 10 |
| MCP transport failure enters the shared Tool retry path without replaying the in-flight failed call | Test 7 |
| Failed HTTP import readiness uses sanitized endpoints | Test 8 |
| Harness-owned MCP export can be imported by another Harness workspace | Test 9 |
| Imported MCP readiness appears in report and traces, including available/suppressed/unavailable Tools | Tests 2, 3, 5, 6, 8, and 9 |
| Focused automated M18 regression coverage remains green | Test 10 |

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
cat > /tmp/setup-harness-m18-manual.sh <<'SH'
#!/usr/bin/env bash
set -euo pipefail

ROOT="$(pwd)"
APM="${APM:-$ROOT/target/debug/agentpm}"
BASE="${HARNESS_M18_TEST_BASE:-$ROOT/harness-m18-test}"
IMPORT_WORK="$BASE/import-workspace"
EXPORT_WORK="$BASE/export-workspace"
RUNS="$BASE/runs"
PYTHON_CMD="${AGENTPM_MANUAL_PYTHON:-python3}"
SCHEMA_URL="https://raw.githubusercontent.com/agentpm-dev/cli/refs/heads/main/schemas/agentpm.manifest.schema.json"

if [ ! -x "$APM" ]; then
  echo "Missing agentpm binary at $APM. Run: cargo build -p agentpm-cli" >&2
  exit 1
fi

rm -rf "$BASE"
mkdir -p "$IMPORT_WORK/scripts" "$IMPORT_WORK/runtime" "$EXPORT_WORK/scripts" "$EXPORT_WORK/runtime" "$RUNS"

pkg_dir() {
  local workspace="$1"
  local plural="$2"
  local package="$3"
  local version="$4"
  local without_at="${package#@}"
  local namespace="${without_at%%/*}"
  local name="${without_at#*/}"
  printf '%s/.agentpm/%s/%s/%s/%s' "$workspace" "$plural" "$namespace" "$name" "$version"
}

write_tool() {
  local workspace="$1"
  local package="$2"
  local version="$3"
  local manifest_name="${package#*/}"
  local dir
  dir="$(pkg_dir "$workspace" tools "$package" "$version")"
  mkdir -p "$dir"
  cat > "$dir/agent.json" <<JSON
{
  "\$schema": "$SCHEMA_URL",
  "kind": "tool",
  "name": "$manifest_name",
  "version": "$version",
  "description": "M18 manual export Tool.",
  "entrypoint": {
    "command": "$PYTHON_CMD",
    "args": ["script.py"],
    "cwd": ".",
    "timeout_ms": 5000,
    "env": {}
  },
  "runtime": {
    "type": "python",
    "version": "3"
  },
  "inputs": {
    "type": "object",
    "additionalProperties": false,
    "properties": {
      "message": { "type": "string" },
      "query": { "type": "string" }
    }
  },
  "outputs": {
    "type": "object",
    "additionalProperties": true,
    "required": ["ok", "tool", "input"],
    "properties": {
      "ok": { "type": "boolean" },
      "tool": { "type": "string" },
      "input": { "type": "object" }
    }
  },
  "files": ["script.py"]
}
JSON
  cat > "$dir/script.py" <<PY
import json
import sys
payload = json.load(sys.stdin)
print(json.dumps({"ok": True, "tool": "$package", "input": payload}))
PY
  "$APM" lint "$dir/agent.json" >/dev/null
}

write_loop() {
  local workspace="$1"
  local package="$2"
  local dir
  dir="$(pkg_dir "$workspace" loops "$package" "0.1.0")"
  mkdir -p "$dir"
  cat > "$dir/agent.json" <<JSON
{
  "\$schema": "$SCHEMA_URL",
  "kind": "loop",
  "name": "${package#*/}",
  "version": "0.1.0",
  "description": "M18 manual MCP import/export loop.",
  "loop": {
    "archetype": "act_finish",
    "entry_phase": "research",
    "limits": {
      "max_steps": 5
    },
    "error_policy": {
      "phase_failure": {
        "action": "abort"
      },
      "tool_failure": {
        "action": "retry",
        "max_retries": 1,
        "on_exhausted": "fail_phase"
      }
    },
    "phases": [
      {
        "id": "research",
        "objective": "Use imported MCP Tools when requested, then route or complete.",
        "access": {
          "tools": true,
          "knowledge": false,
          "memory": { "read": false, "write": false }
        },
        "outcomes": [
          { "id": "done", "description": "Manual run complete." },
          { "id": "no-tools", "description": "Move to the Tool-disabled phase." }
        ]
      },
      {
        "id": "no-tools",
        "objective": "Verify imported MCP Tools are prohibited when tools are disabled.",
        "access": {
          "tools": false,
          "knowledge": false,
          "memory": { "read": false, "write": false }
        },
        "outcomes": [
          { "id": "done", "description": "Manual no-tools run complete." }
        ]
      }
    ],
    "transitions": [
      { "from": "research", "on": "done", "to": "\$end" },
      { "from": "research", "on": "no-tools", "to": "no-tools" },
      { "from": "no-tools", "on": "done", "to": "\$end" }
    ]
  }
}
JSON
  "$APM" lint "$dir/agent.json" >/dev/null
}

write_agent_and_lock() {
  local workspace="$1"
  local agent_name="$2"
  local loop_pkg="$3"
  local include_export_tool="${4:-false}"
  cat > "$workspace/agent.json" <<JSON
{
  "\$schema": "$SCHEMA_URL",
  "kind": "agent",
  "name": "$agent_name",
  "version": "0.1.0",
  "description": "M18 manual Harness Agent.",
  "tools": [
JSON
  if [ "$include_export_tool" = "true" ]; then
    cat >> "$workspace/agent.json" <<'JSON'
    "@zack/m18-export-echo-tool@0.1.0"
JSON
  fi
  cat >> "$workspace/agent.json" <<JSON
  ],
  "loop": "$loop_pkg@0.1.0"
JSON
  if [ "$include_export_tool" = "true" ]; then
    cat >> "$workspace/agent.json" <<'JSON'
  ,
  "bindings": {
    "mcp": [
      {
        "id": "export-public",
        "tools": ["@zack/m18-export-echo-tool"]
      }
    ]
  }
JSON
  fi
  cat >> "$workspace/agent.json" <<'JSON'
}
JSON
  "$APM" lint "$workspace/agent.json" >/dev/null

  local loop_key="loop:${loop_pkg}@0.1.0"
  cat > "$workspace/agent.lock" <<JSON
{
  "lockfile_version": 3,
  "generated": "2026-09-17T00:00:00Z",
  "packages": {
    "$loop_key": {
      "kind": "loop",
      "name": "$loop_pkg",
      "version": "0.1.0",
      "integrity": "sha256-manual"
    }
JSON
  if [ "$include_export_tool" = "true" ]; then
    cat >> "$workspace/agent.lock" <<'JSON'
    ,
    "tool:@zack/m18-export-echo-tool@0.1.0": {
      "kind": "tool",
      "name": "@zack/m18-export-echo-tool",
      "version": "0.1.0",
      "integrity": "sha256-manual"
    }
JSON
  fi
  cat >> "$workspace/agent.lock" <<JSON
  },
  "roots": {
    "local:agent": {
      "name": "$agent_name",
      "version": "0.1.0",
      "tools": [
JSON
  if [ "$include_export_tool" = "true" ]; then
    cat >> "$workspace/agent.lock" <<'JSON'
        "tool:@zack/m18-export-echo-tool@0.1.0"
JSON
  fi
  cat >> "$workspace/agent.lock" <<JSON
      ],
      "skills": [],
      "knowledge": [],
      "memory": [],
      "profiles": [],
      "loop": "$loop_key"
    }
  }
}
JSON
}

write_mcp_server() {
  cat > "$IMPORT_WORK/scripts/mcp_server.py" <<'PY'
#!/usr/bin/env python3
import argparse
import json
import os
import sys
from http.server import BaseHTTPRequestHandler, HTTPServer
from pathlib import Path

def tool_schema(name):
    if name == "composed":
        return {
            "$ref": "#/$defs/LookupArgs",
            "$defs": {
                "LookupArgs": {
                    "type": "object",
                    "additionalProperties": False,
                    "properties": {"query": {"type": "string", "minLength": 1}},
                    "required": ["query"],
                }
            },
        }
    return {
        "type": "object",
        "additionalProperties": False,
        "properties": {
            "query": {"type": "string"},
            "message": {"type": "string"},
        },
    }

def advertised_tools(names):
    return [
        {
            "name": name,
            "description": f"M18 manual MCP Tool {name}.",
            "inputSchema": tool_schema(name),
        }
        for name in names
    ]

def handle_rpc(request, args):
    method = request.get("method")
    params = request.get("params", {})
    if method == "initialize":
        result = {"protocolVersion": "2025-06-18"}
    elif method == "tools/list":
        result = {"tools": advertised_tools(args.tools)}
    elif method == "tools/call":
        if args.fail_first:
            marker = Path(args.fail_first)
            if not marker.exists():
                marker.write_text("failed once")
                sys.exit(1)
        arguments = params.get("arguments", {})
        result = {
            "content": [{"type": "text", "text": "ok"}],
            "structuredContent": {
                "server": args.server_id,
                "tool": params.get("name"),
                "arguments": arguments,
                "token_seen": bool(os.environ.get("M18_LOCAL_MCP_TOKEN")),
            },
            "isError": False,
        }
    else:
        result = {}
    return {"jsonrpc": "2.0", "id": request.get("id"), "result": result}

def run_stdio(args):
    for line in sys.stdin:
        request = json.loads(line)
        print(json.dumps(handle_rpc(request, args)), flush=True)

def run_http(args):
    class Handler(BaseHTTPRequestHandler):
        def do_GET(self):
            if self.path == "/health":
                self.send_response(200)
                self.end_headers()
                self.wfile.write(b"ok")
                return
            self.send_response(404)
            self.end_headers()

        def do_POST(self):
            length = int(self.headers.get("content-length", "0"))
            body = self.rfile.read(length)
            request = json.loads(body)
            auth_ok = self.headers.get("authorization") == "Bearer m18-secret"
            with open(args.log, "a", encoding="utf-8") as fh:
                fh.write(json.dumps({"method": request.get("method"), "authorization_ok": auth_ok}) + "\n")
            response = handle_rpc(request, args)
            payload = json.dumps(response).encode()
            self.send_response(200)
            self.send_header("content-type", "application/json")
            self.send_header("content-length", str(len(payload)))
            self.end_headers()
            self.wfile.write(payload)

        def log_message(self, *_):
            return

    server = HTTPServer(("127.0.0.1", args.port), Handler)
    endpoint = f"http://127.0.0.1:{server.server_port}/mcp"
    Path(args.endpoint_file).write_text(endpoint)
    print(endpoint, flush=True)
    server.serve_forever()

parser = argparse.ArgumentParser()
parser.add_argument("--transport", choices=["stdio", "http"], required=True)
parser.add_argument("--server-id", default="manual")
parser.add_argument("--tools", default="lookup", help="Comma-separated Tool names to advertise.")
parser.add_argument("--fail-first")
parser.add_argument("--port", type=int, default=0)
parser.add_argument("--endpoint-file", default="http-endpoint.txt")
parser.add_argument("--log", default="http-server.jsonl")
args = parser.parse_args()
args.tools = [part for part in args.tools.split(",") if part]

if args.transport == "stdio":
    run_stdio(args)
else:
    run_http(args)
PY
  chmod +x "$IMPORT_WORK/scripts/mcp_server.py"
}

write_model() {
  local workspace="$1"
  cat > "$workspace/runtime/m18_model.py" <<'PY'
#!/usr/bin/env python3
import json
import re
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

def prompt_of(msg):
    req = msg.get("request") or msg.get("payload", {}).get("request", {})
    prompt = req.get("prompt", "")
    if isinstance(prompt, dict):
        return prompt.get("text", "") or json.dumps(prompt)
    return prompt or req.get("prompt_text", "") or json.dumps(req)

def input_of(prompt):
    match = re.search(r"Run input:\n(.+?)(?:\n\n|$)", prompt, re.S)
    return match.group(1).strip() if match else prompt

def user_input_of(msg, prompt):
    req = msg.get("request") or msg.get("payload", {}).get("request", {})
    for turn in req.get("ordered_turns", []):
        if turn.get("kind") == "user_input" and isinstance(turn.get("content"), str):
            return turn["content"].strip()
    return input_of(prompt)

def has_repair_feedback(msg, prompt):
    if "RepairFeedback:" in prompt:
        return True
    serialized = json.dumps(msg, separators=(",", ":"))
    return '"kind":"repair_feedback"' in serialized

def complete(outcome="done", output=None):
    action = {"type": "phase_completion", "outcome": outcome}
    if output is not None:
        action["output"] = output
    return {"id": "complete", "action": action}

def mcp(server, tool, arguments, action_id="mcp"):
    return {
        "id": action_id,
        "action": {
            "type": "external_mcp_tool",
            "server": server,
            "tool": tool,
            "arguments": arguments,
        },
    }

scenario_steps = {}

for line in sys.stdin:
    msg = json.loads(line)
    kind = msg.get("kind")
    if kind == "initialize":
        emit(msg, "initialized", {
            "capabilities": {
                "semantic_actions": True,
                "structured_actions": True,
                "native_action_result_turns": True,
            }
        })
        continue
    if kind == "shutdown":
        emit(msg, "shutdown")
        break
    if kind != "request":
        emit(msg, "error", error={"code": "unsupported", "message": kind})
        continue

    prompt = prompt_of(msg)
    run_input = user_input_of(msg, prompt).lower()
    scenario_text = run_input
    has_action_result = "ActionResult" in prompt or "semantic_action_result" in prompt
    has_repair = has_repair_feedback(msg, prompt)
    current_phase = "no-tools" if "Current phase: no-tools" in prompt else "research"
    if "duplicate" in scenario_text:
        scenario_key = "duplicate"
    else:
        scenario_key = f"{current_phase}:{run_input}"
    step = scenario_steps.get(scenario_key, 0)

    def advance():
        scenario_steps[scenario_key] = step + 1

    if has_repair:
        actions = [complete("done", {"summary": "completed after repair"})]
    elif "no-tools phase" in run_input and current_phase == "research":
        actions = [complete("no-tools")]
    elif "no-tools phase" in run_input and current_phase == "no-tools" and step == 0:
        advance()
        actions = [mcp("local-search", "lookup", {"query": "blocked phase"})]
    elif "duplicate" in scenario_text and step == 0:
        advance()
        actions = [mcp("github", "search", {"query": "repo readiness"}, "github")]
    elif "duplicate" in scenario_text and step == 1:
        advance()
        actions = [mcp("linear", "search", {"query": "issue readiness"}, "linear")]
    elif "schema" in scenario_text and step == 0:
        advance()
        actions = [mcp("composed-search", "composed", {"unused": True})]
    elif "restart" in scenario_text and step == 0:
        advance()
        actions = [mcp("flaky-search", "lookup", {"query": "retry once"})]
    elif "http" in scenario_text and step == 0:
        advance()
        actions = [mcp("http-search", "lookup", {"query": "http readiness"})]
    elif "loopback" in scenario_text and step == 0:
        advance()
        actions = [mcp("loopback-export", "zack__m18_export_echo_tool", {"message": "loopback import"})]
    elif step == 0:
        advance()
        actions = [mcp("local-search", "lookup", {"query": "stdio readiness"})]
    else:
        actions = [complete("done", {"summary": "m18 manual complete"})]

    emit(msg, "response", {
        "assistant_content": None,
        "actions": actions,
        "finish_reason": "tool_calls",
        "usage": {"input_tokens": 1, "output_tokens": 1, "total_tokens": 2},
    })
PY
  chmod +x "$workspace/runtime/m18_model.py"
}

write_import_configs() {
  cat > "$IMPORT_WORK/agentpm.stdio.harness.json" <<JSON
{
  "version": 1,
  "model": { "provider": "m18-model", "model": "deterministic" },
  "providers": {
    "models": {
      "m18-model": {
        "implementation": {
          "type": "process",
          "command": "$PYTHON_CMD",
          "args": ["runtime/m18_model.py"],
          "cwd": ".",
          "startup_timeout_ms": 5000,
          "request_timeout_ms": 30000
        }
      }
    }
  },
  "mcp": {
    "imports": {
      "local-search": {
        "transport": "stdio",
        "command": "$PYTHON_CMD",
        "args": ["scripts/mcp_server.py", "--transport", "stdio", "--server-id", "local-search", "--tools", "lookup,ignored"],
        "cwd": ".",
        "env": ["M18_LOCAL_MCP_TOKEN"],
        "scope": { "mode": "phases", "phases": ["research"] },
        "tools": ["lookup", "missing"],
        "startup_timeout_ms": 5000,
        "request_timeout_ms": 5000,
        "restart": { "max_attempts": 0, "backoff_ms": 0 }
      }
    }
  },
  "runtime": {
    "state_dir": ".agentpm-state-m18-stdio",
    "limits": {
      "max_steps": 5,
      "max_model_calls_per_phase": 6,
      "max_tool_calls_per_phase": 4,
      "max_actions_per_phase": 8,
      "max_tool_call_repairs": 2
    }
  },
  "trace": { "enabled": true, "level": "verbose", "content": "full" }
}
JSON

  cat > "$IMPORT_WORK/agentpm.duplicates.harness.json" <<JSON
{
  "version": 1,
  "model": { "provider": "m18-model", "model": "deterministic" },
  "providers": {
    "models": {
      "m18-model": {
        "implementation": {
          "type": "process",
          "command": "$PYTHON_CMD",
          "args": ["runtime/m18_model.py"],
          "cwd": ".",
          "startup_timeout_ms": 5000,
          "request_timeout_ms": 30000
        }
      }
    }
  },
  "mcp": {
    "imports": {
      "github": {
        "transport": "stdio",
        "command": "$PYTHON_CMD",
        "args": ["scripts/mcp_server.py", "--transport", "stdio", "--server-id", "github", "--tools", "search"],
        "cwd": ".",
        "scope": { "mode": "global" },
        "tools": ["search"],
        "startup_timeout_ms": 5000,
        "request_timeout_ms": 5000
      },
      "linear": {
        "transport": "stdio",
        "command": "$PYTHON_CMD",
        "args": ["scripts/mcp_server.py", "--transport", "stdio", "--server-id", "linear", "--tools", "search"],
        "cwd": ".",
        "scope": { "mode": "global" },
        "tools": ["search"],
        "startup_timeout_ms": 5000,
        "request_timeout_ms": 5000
      }
    }
  },
  "runtime": { "state_dir": ".agentpm-state-m18-duplicates" },
  "trace": { "enabled": true, "level": "verbose", "content": "full" }
}
JSON

  cat > "$IMPORT_WORK/agentpm.schema.harness.json" <<JSON
{
  "version": 1,
  "model": { "provider": "m18-model", "model": "deterministic" },
  "providers": {
    "models": {
      "m18-model": {
        "implementation": {
          "type": "process",
          "command": "$PYTHON_CMD",
          "args": ["runtime/m18_model.py"],
          "cwd": ".",
          "startup_timeout_ms": 5000,
          "request_timeout_ms": 30000
        }
      }
    }
  },
  "mcp": {
    "imports": {
      "composed-search": {
        "transport": "stdio",
        "command": "$PYTHON_CMD",
        "args": ["scripts/mcp_server.py", "--transport", "stdio", "--server-id", "composed-search", "--tools", "composed"],
        "cwd": ".",
        "scope": { "mode": "global" },
        "tools": ["composed"],
        "startup_timeout_ms": 5000,
        "request_timeout_ms": 5000
      }
    }
  },
  "runtime": { "state_dir": ".agentpm-state-m18-schema" },
  "trace": { "enabled": true, "level": "verbose", "content": "full" }
}
JSON

  cat > "$IMPORT_WORK/agentpm.restart.harness.json" <<JSON
{
  "version": 1,
  "model": { "provider": "m18-model", "model": "deterministic" },
  "providers": {
    "models": {
      "m18-model": {
        "implementation": {
          "type": "process",
          "command": "$PYTHON_CMD",
          "args": ["runtime/m18_model.py"],
          "cwd": ".",
          "startup_timeout_ms": 5000,
          "request_timeout_ms": 30000
        }
      }
    }
  },
  "mcp": {
    "imports": {
      "flaky-search": {
        "transport": "stdio",
        "command": "$PYTHON_CMD",
        "args": ["scripts/mcp_server.py", "--transport", "stdio", "--server-id", "flaky-search", "--tools", "lookup", "--fail-first", "runtime/flaky-marker"],
        "cwd": ".",
        "scope": { "mode": "global" },
        "tools": ["lookup"],
        "startup_timeout_ms": 5000,
        "request_timeout_ms": 5000,
        "restart": { "max_attempts": 1, "backoff_ms": 0 }
      }
    }
  },
  "runtime": { "state_dir": ".agentpm-state-m18-restart" },
  "trace": { "enabled": true, "level": "verbose", "content": "full" }
}
JSON

  cat > "$IMPORT_WORK/agentpm.bad-http.harness.json" <<JSON
{
  "version": 1,
  "model": { "provider": "m18-model", "model": "deterministic" },
  "providers": {
    "models": {
      "m18-model": {
        "implementation": {
          "type": "process",
          "command": "$PYTHON_CMD",
          "args": ["runtime/m18_model.py"],
          "cwd": ".",
          "startup_timeout_ms": 5000,
          "request_timeout_ms": 30000
        }
      }
    }
  },
  "mcp": {
    "imports": {
      "bad-http": {
        "transport": "http",
        "url": "http://user:secret@127.0.0.1:1/mcp?token=secret-token#frag",
        "scope": { "mode": "global" },
        "tools": ["lookup"]
      }
    }
  },
  "runtime": { "state_dir": ".agentpm-state-m18-bad-http" },
  "trace": { "enabled": true, "level": "verbose", "content": "full" }
}
JSON

  cat > "$EXPORT_WORK/agentpm.export.harness.json" <<JSON
{
  "version": 1,
  "model": { "provider": "m18-model", "model": "deterministic" },
  "providers": {
    "models": {
      "m18-model": {
        "implementation": {
          "type": "process",
          "command": "$PYTHON_CMD",
          "args": ["runtime/m18_model.py"],
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
      "restart": { "max_attempts": 0, "backoff_ms": 0 }
    }
  },
  "runtime": { "state_dir": ".agentpm-state-m18-export" },
  "trace": { "enabled": true, "level": "verbose", "content": "full" }
}
JSON
}

write_helpers() {
  cat > "$IMPORT_WORK/scripts/write_http_config.py" <<'PY'
import json
import os
from pathlib import Path

work = Path(os.environ["M18_WORK"])
endpoint = Path(os.environ["M18_RUNS"], "http-server/endpoint.txt").read_text().strip()
config = {
    "version": 1,
    "model": {"provider": "m18-model", "model": "deterministic"},
    "providers": {
        "models": {
            "m18-model": {
                "implementation": {
                    "type": "process",
                    "command": os.environ["AGENTPM_MANUAL_PYTHON"],
                    "args": ["runtime/m18_model.py"],
                    "cwd": ".",
                    "startup_timeout_ms": 5000,
                    "request_timeout_ms": 30000,
                }
            }
        }
    },
    "mcp": {
        "imports": {
            "http-search": {
                "transport": "http",
                "url": endpoint,
                "headers": {"Authorization": {"env": "M18_HTTP_AUTHORIZATION"}},
                "scope": {"mode": "global"},
                "tools": ["lookup"],
            }
        }
    },
    "runtime": {"state_dir": ".agentpm-state-m18-http"},
    "trace": {"enabled": True, "level": "verbose", "content": "full"},
}
(work / "agentpm.http.harness.json").write_text(json.dumps(config, indent=2, sort_keys=True))
PY

  cat > "$IMPORT_WORK/scripts/write_loopback_config.py" <<'PY'
import json
import os
from pathlib import Path

work = Path(os.environ["M18_WORK"])
endpoint = Path(os.environ["M18_RUNS"], "loopback-export/endpoint.txt").read_text().strip()
config = {
    "version": 1,
    "model": {"provider": "m18-model", "model": "deterministic"},
    "providers": {
        "models": {
            "m18-model": {
                "implementation": {
                    "type": "process",
                    "command": os.environ["AGENTPM_MANUAL_PYTHON"],
                    "args": ["runtime/m18_model.py"],
                    "cwd": ".",
                    "startup_timeout_ms": 5000,
                    "request_timeout_ms": 30000,
                }
            }
        }
    },
    "mcp": {
        "imports": {
            "loopback-export": {
                "transport": "http",
                "url": endpoint,
                "scope": {"mode": "global"},
                "tools": ["zack__m18_export_echo_tool"],
            }
        }
    },
    "runtime": {"state_dir": ".agentpm-state-m18-loopback"},
    "trace": {"enabled": True, "level": "verbose", "content": "full"},
}
(work / "agentpm.loopback.harness.json").write_text(json.dumps(config, indent=2, sort_keys=True))
PY

  cat > "$IMPORT_WORK/scripts/harness_export_session.py" <<'PY'
#!/usr/bin/env python3
import argparse
import json
import os
import signal
import subprocess
import sys
import time
from pathlib import Path

parser = argparse.ArgumentParser()
parser.add_argument("--agentpm", required=True)
parser.add_argument("--workspace", required=True)
parser.add_argument("--config", default="agentpm.export.harness.json")
parser.add_argument("--endpoint-file", required=True)
parser.add_argument("--stdout-file", required=True)
parser.add_argument("--stderr-file", required=True)
args = parser.parse_args()

stdout_path = Path(args.stdout_file)
stderr_path = Path(args.stderr_file)
endpoint_path = Path(args.endpoint_file)
stdout = stdout_path.open("w", encoding="utf-8")
stderr = stderr_path.open("w", encoding="utf-8")
proc = subprocess.Popen(
    [args.agentpm, "harness", "--config", args.config, "--machine"],
    cwd=args.workspace,
    stdin=subprocess.PIPE,
    stdout=subprocess.PIPE,
    stderr=stderr,
    text=True,
    bufsize=1,
)

def stop(*_):
    try:
        if proc.stdin:
            proc.stdin.write(json.dumps({"protocol": "agentpm-harness-machine", "version": 1, "id": "shutdown-1", "kind": "request", "method": "shutdown", "payload": {}}) + "\n")
            proc.stdin.flush()
    except Exception:
        pass
    try:
        proc.terminate()
    except Exception:
        pass

signal.signal(signal.SIGTERM, stop)
signal.signal(signal.SIGINT, stop)

init = {"protocol": "agentpm-harness-machine", "version": 1, "id": "init-1", "kind": "request", "method": "initialize", "payload": {}}
proc.stdin.write(json.dumps(init) + "\n")
proc.stdin.flush()
deadline = time.time() + 15
while time.time() < deadline:
    line = proc.stdout.readline()
    if not line:
        break
    stdout.write(line)
    stdout.flush()
    row = json.loads(line)
    if row.get("id") == "init-1" and row.get("kind") == "response":
        exports = row["payload"]["mcp_exports"]
        public = next(export for export in exports if export["id"] == "export-public")
        endpoint_path.write_text(public["endpoint"])
        break
else:
    raise SystemExit("timed out waiting for Harness MCP export endpoint")

while proc.poll() is None:
    line = proc.stdout.readline()
    if line:
        stdout.write(line)
        stdout.flush()
    else:
        time.sleep(0.1)
PY
  chmod +x "$IMPORT_WORK/scripts/harness_export_session.py"

  cat > "$IMPORT_WORK/scripts/find_report.py" <<'PY'
import json
import sys
from pathlib import Path

state_dir = Path(sys.argv[1])
reports = sorted(state_dir.glob("runs/*/report.json"), key=lambda path: path.stat().st_mtime)
if not reports:
    raise SystemExit(f"no report under {state_dir}")
print(reports[-1])
PY
}

write_loop "$IMPORT_WORK" "@zack/m18-mcp-import-loop"
write_agent_and_lock "$IMPORT_WORK" "m18-mcp-import-agent" "@zack/m18-mcp-import-loop" false
write_model "$IMPORT_WORK"
write_mcp_server
write_import_configs
write_helpers

write_tool "$EXPORT_WORK" "@zack/m18-export-echo-tool" "0.1.0"
write_loop "$EXPORT_WORK" "@zack/m18-mcp-export-loop"
write_agent_and_lock "$EXPORT_WORK" "m18-mcp-export-agent" "@zack/m18-mcp-export-loop" true
write_model "$EXPORT_WORK"

cat > "$BASE/env.sh" <<ENV
export M18_BASE="$BASE"
export M18_WORK="$IMPORT_WORK"
export M18_EXPORT_WORK="$EXPORT_WORK"
export M18_RUNS="$RUNS"
export APM="$APM"
export AGENTPM_MANUAL_PYTHON="$PYTHON_CMD"
ENV

echo "M18 manual workspace ready:"
echo "  base:        $BASE"
echo "  import work: $IMPORT_WORK"
echo "  export work: $EXPORT_WORK"
echo "  runs:        $RUNS"
SH

chmod +x /tmp/setup-harness-m18-manual.sh
/tmp/setup-harness-m18-manual.sh
source harness-m18-test/env.sh
```

## Test 1: stdio import appears in preflight without leaking env secrets

```bash
mkdir -p "$M18_RUNS/preflight-stdio"
(cd "$M18_WORK" && M18_LOCAL_MCP_TOKEN="m18-secret-token" "$APM" harness --config agentpm.stdio.harness.json --verbose) \
  >"$M18_RUNS/preflight-stdio/stdout.txt" \
  2>"$M18_RUNS/preflight-stdio/stderr.txt"

python3 - <<'PY'
import os
from pathlib import Path
text = (Path(os.environ["M18_RUNS"]) / "preflight-stdio/stdout.txt").read_text()
assert "MCP imports:" in text, text
assert "local-search" in text, text
assert "M18_LOCAL_MCP_TOKEN" in text, text
assert "m18-secret-token" not in text, text
print("preflight stdio import is visible and secret value is not printed")
PY
```

Expected:

- Preflight lists `local-search`.
- Preflight names the required env key.
- The secret value does not appear.

## Test 2: stdio import dispatches and filtered/missing Tools are reported

```bash
mkdir -p "$M18_RUNS/stdio-run"
(cd "$M18_WORK" && M18_LOCAL_MCP_TOKEN="m18-secret-token" "$APM" harness \
    --config agentpm.stdio.harness.json \
    --headless \
    --input "Use the stdio imported MCP lookup Tool, then complete." \
    --report "$M18_RUNS/stdio-run/report.json") \
  >"$M18_RUNS/stdio-run/stdout.txt" \
  2>"$M18_RUNS/stdio-run/stderr.txt"

python3 - <<'PY'
import json, os
from pathlib import Path
run_dir = Path(os.environ["M18_RUNS"]) / "stdio-run"
report = json.loads((run_dir / "report.json").read_text())
events = [json.loads(line) for line in Path(report["trace_path"]).read_text().splitlines()]
connected = [e for e in events if e["event_type"] == "mcp_import_connected"]
failed = [e for e in events if e["event_type"] == "mcp_import_failed"]
completed = [e for e in events if e["event_type"] == "mcp_tool_completed"]
assert any(e["payload"]["fields"].get("identity") == "mcp:local-search/lookup" for e in connected), connected
assert any(e["payload"]["fields"].get("tool") == "missing" for e in failed), failed
assert any(e["payload"]["identity"] == "mcp:local-search/lookup" for e in completed), completed
assert any(s["operation_kind"] == "mcp_import" and s["identity"] == "mcp:local-search/lookup" for s in report["mcp_summaries"]), report["mcp_summaries"]
assert "m18-secret-token" not in Path(report["trace_path"]).read_text()
print(json.dumps({"mcp_summaries": report["mcp_summaries"], "terminal_status": report["terminal_status"]}, indent=2))
PY
```

Expected:

- `mcp:local-search/lookup` is connected and called.
- Configured missing Tool `missing` is unavailable/suppressed before model selection.
- The report includes an `mcp_import` summary.
- Secret values do not appear in trace content.

## Test 3: HTTP import uses env-backed headers and dispatches

Start a disposable HTTP MCP server:

```bash
mkdir -p "$M18_RUNS/http-server" "$M18_RUNS/http-run"
"$AGENTPM_MANUAL_PYTHON" "$M18_WORK/scripts/mcp_server.py" \
  --transport http \
  --server-id http-search \
  --tools lookup \
  --port 0 \
  --endpoint-file "$M18_RUNS/http-server/endpoint.txt" \
  --log "$M18_RUNS/http-server/requests.jsonl" \
  >"$M18_RUNS/http-server/stdout.txt" \
  2>"$M18_RUNS/http-server/stderr.txt" &
HTTP_PID=$!
trap 'kill "$HTTP_PID" 2>/dev/null || true' EXIT
until [ -s "$M18_RUNS/http-server/endpoint.txt" ]; do sleep 0.1; done

(cd "$M18_WORK" && "$AGENTPM_MANUAL_PYTHON" scripts/write_http_config.py)
```

Run the Harness import:

```bash
(cd "$M18_WORK" && M18_HTTP_AUTHORIZATION="Bearer m18-secret" "$APM" harness \
    --config agentpm.http.harness.json \
    --headless \
    --input "Use the http imported MCP lookup Tool, then complete." \
    --report "$M18_RUNS/http-run/report.json") \
  >"$M18_RUNS/http-run/stdout.txt" \
  2>"$M18_RUNS/http-run/stderr.txt"

python3 - <<'PY'
import json, os
from pathlib import Path
runs = Path(os.environ["M18_RUNS"])
server_rows = [json.loads(line) for line in (runs / "http-server/requests.jsonl").read_text().splitlines()]
assert any(row["method"] == "tools/call" and row["authorization_ok"] for row in server_rows), server_rows
report = json.loads((runs / "http-run/report.json").read_text())
trace = Path(report["trace_path"]).read_text()
assert "m18-secret" not in trace
events = [json.loads(line) for line in trace.splitlines()]
assert any(e["event_type"] == "mcp_import_connected" and e["payload"]["fields"].get("server") == "http-search" for e in events), events
assert any(e["event_type"] == "mcp_tool_completed" and e["payload"]["identity"] == "mcp:http-search/lookup" for e in events), events
print(json.dumps(server_rows, indent=2))
PY
```

Expected:

- The HTTP server records `authorization_ok: true`.
- Trace/report do not contain the resolved secret header value.
- `mcp:http-search/lookup` is connected and completed.

## Test 4: phase-scoped import is blocked in a Tool-disabled phase

```bash
mkdir -p "$M18_RUNS/no-tools"
(cd "$M18_WORK" && M18_LOCAL_MCP_TOKEN="m18-secret-token" "$APM" harness \
    --config agentpm.stdio.harness.json \
    --headless \
    --input "Route to the no-tools phase, try the imported MCP Tool there, then complete." \
    --report "$M18_RUNS/no-tools/report.json") \
  >"$M18_RUNS/no-tools/stdout.txt" \
  2>"$M18_RUNS/no-tools/stderr.txt"

python3 - <<'PY'
import json, os
from pathlib import Path
report = json.loads((Path(os.environ["M18_RUNS"]) / "no-tools/report.json").read_text())
events = [json.loads(line) for line in Path(report["trace_path"]).read_text().splitlines()]
rejections = [e for e in events if e["event_type"] == "semantic_action_rejected"]
assert any(e["payload"]["status"] == "prohibited_by_loop_access" and e["payload"]["identity"] == "mcp:local-search/lookup" for e in rejections), rejections
assert report["terminal_status"] == "ended", report["terminal_status"]
print(json.dumps({"terminal_status": report["terminal_status"], "repair_count": report["repair_count"]}, indent=2))
PY
```

Expected:

- The imported MCP Tool is usable in `research` but rejected in `no-tools`.
- The rejection is a Loop access rejection, not a transport failure.
- The run repairs and completes.

## Test 5: duplicate Tool names across servers keep distinct identities

```bash
mkdir -p "$M18_RUNS/duplicates"
(cd "$M18_WORK" && "$APM" harness \
  --config agentpm.duplicates.harness.json \
  --headless \
  --input "Use duplicate imported MCP search Tools from both servers, then complete." \
  --report "$M18_RUNS/duplicates/report.json") \
  >"$M18_RUNS/duplicates/stdout.txt" \
  2>"$M18_RUNS/duplicates/stderr.txt"

python3 - <<'PY'
import json, os
from pathlib import Path
report = json.loads((Path(os.environ["M18_RUNS"]) / "duplicates/report.json").read_text())
events = [json.loads(line) for line in Path(report["trace_path"]).read_text().splitlines()]
completed = [e["payload"]["identity"] for e in events if e["event_type"] == "mcp_tool_completed"]
assert "mcp:github/search" in completed, completed
assert "mcp:linear/search" in completed, completed
prepared = [e for e in events if e["event_type"] == "model_runtime_request_prepared"]
aliases = prepared[0]["payload"]["fields"].get("action_aliases", [])
mcp_aliases = [a for a in aliases if a["action_kind"] == "external_mcp_tool"]
assert len(mcp_aliases) == 2, mcp_aliases
assert {a["identity"] for a in mcp_aliases} == {"mcp:github/search", "mcp:linear/search"}, mcp_aliases
print(json.dumps({"completed": completed, "aliases": mcp_aliases}, indent=2, sort_keys=True))
PY
```

Expected:

- Both Tools are named `search` at the MCP level.
- Harness calls both as distinct canonical identities.
- Provider aliases are distinct and map back to their canonical identities.

## Test 6: reduced provider schema still repairs against canonical MCP schema

```bash
mkdir -p "$M18_RUNS/schema"
(cd "$M18_WORK" && "$APM" harness \
  --config agentpm.schema.harness.json \
  --headless \
  --input "Use the schema-reduced imported MCP Tool with invalid arguments, then repair and complete." \
  --report "$M18_RUNS/schema/report.json") \
  >"$M18_RUNS/schema/stdout.txt" \
  2>"$M18_RUNS/schema/stderr.txt"

python3 - <<'PY'
import json, os
from pathlib import Path
report = json.loads((Path(os.environ["M18_RUNS"]) / "schema/report.json").read_text())
events = [json.loads(line) for line in Path(report["trace_path"]).read_text().splitlines()]
prepared = [e for e in events if e["event_type"] == "model_runtime_request_prepared"]
diagnostics = []
for event in prepared:
    diagnostics.extend(event["payload"]["fields"].get("diagnostics", []))
if diagnostics:
    assert any("MCP Tool `mcp:composed-search/composed`" in d and "canonical MCP schema" in d for d in diagnostics), diagnostics
else:
    print("process-model trace has no provider schema diagnostics; Test 10 covers the built-in provider snapshot")
repairs = [e for e in events if e["event_type"] == "model_repair_requested"]
assert any("canonical MCP input schema" in e["payload"]["message"] for e in repairs), repairs
assert report["terminal_status"] == "ended", report["terminal_status"]
print(json.dumps({"diagnostics": diagnostics, "repair_count": report["repair_count"]}, indent=2))
PY
```

Expected:

- The invalid call is rejected against the canonical MCP schema.
- Repair feedback names the canonical MCP schema and the run completes.
- Built-in provider request diagnostics for schema reduction are covered by Test 10.

## Test 7: stdio restart does not replay the failed in-flight call and shared Tool retry succeeds

```bash
mkdir -p "$M18_RUNS/restart"
rm -f "$M18_WORK/runtime/flaky-marker"
(cd "$M18_WORK" && "$APM" harness \
  --config agentpm.restart.harness.json \
  --headless \
  --input "Use the restart imported MCP Tool, then complete." \
  --report "$M18_RUNS/restart/report.json") \
  >"$M18_RUNS/restart/stdout.txt" \
  2>"$M18_RUNS/restart/stderr.txt"

python3 - <<'PY'
import json, os
from pathlib import Path
report = json.loads((Path(os.environ["M18_RUNS"]) / "restart/report.json").read_text())
events = [json.loads(line) for line in Path(report["trace_path"]).read_text().splitlines()]
assert report["retry_count"] == 1, report
assert report["usage"]["tool_retries"] == 1, report["usage"]
assert any(e["event_type"] == "mcp_tool_failed" and e["payload"]["identity"] == "mcp:flaky-search/lookup" for e in events), events
assert any(e["event_type"] == "mcp_tool_completed" and e["payload"]["identity"] == "mcp:flaky-search/lookup" for e in events), events
print(json.dumps({"retry_count": report["retry_count"], "tool_retries": report["usage"]["tool_retries"]}, indent=2))
PY
```

Expected:

- First MCP call fails because the stdio server exits.
- Harness restarts the server for future calls only.
- The shared logical Tool retry makes a new call and succeeds.

## Test 8: failed HTTP import readiness redacts endpoint secrets

```bash
mkdir -p "$M18_RUNS/bad-http"
(cd "$M18_WORK" && "$APM" harness \
  --config agentpm.bad-http.harness.json \
  --headless \
  --input "Try the bad-http imported MCP Tool, then repair and complete." \
  --report "$M18_RUNS/bad-http/report.json") \
  >"$M18_RUNS/bad-http/stdout.txt" \
  2>"$M18_RUNS/bad-http/stderr.txt"

python3 - <<'PY'
import json, os
from pathlib import Path
report = json.loads((Path(os.environ["M18_RUNS"]) / "bad-http/report.json").read_text())
trace = Path(report["trace_path"]).read_text()
assert "secret-token" not in trace
assert "user:secret" not in trace
assert "frag" not in trace
events = [json.loads(line) for line in trace.splitlines()]
failed = [e for e in events if e["event_type"] == "mcp_import_failed"]
assert failed, events
reason = failed[0]["payload"]["fields"].get("reason", "")
assert "http://127.0.0.1:1/mcp" in reason, reason
assert "secret" not in reason and "token" not in reason and "frag" not in reason, reason
print(reason)
PY
```

Expected:

- Failed import emits a sanitized endpoint only.
- Userinfo, query-string token, and fragment are absent from trace/report.

## Test 9: Harness export imported by another Harness workspace

Start a Harness machine session in the export workspace. It will start the Agent-authored MCP export and write the endpoint to `runs/loopback-export/endpoint.txt`.

```bash
mkdir -p "$M18_RUNS/loopback-export" "$M18_RUNS/loopback-import"
"$AGENTPM_MANUAL_PYTHON" "$M18_WORK/scripts/harness_export_session.py" \
  --agentpm "$APM" \
  --workspace "$M18_EXPORT_WORK" \
  --config agentpm.export.harness.json \
  --endpoint-file "$M18_RUNS/loopback-export/endpoint.txt" \
  --stdout-file "$M18_RUNS/loopback-export/stdout.jsonl" \
  --stderr-file "$M18_RUNS/loopback-export/stderr.txt" &
EXPORT_PID=$!
trap 'kill "$EXPORT_PID" 2>/dev/null || true' EXIT
until [ -s "$M18_RUNS/loopback-export/endpoint.txt" ]; do sleep 0.1; done

(cd "$M18_WORK" && "$AGENTPM_MANUAL_PYTHON" scripts/write_loopback_config.py)
```

Run the importing Harness workspace:

```bash
(cd "$M18_WORK" && "$APM" harness \
  --config agentpm.loopback.harness.json \
  --headless \
  --input "Use the loopback imported MCP Tool from the Harness export, then complete." \
  --report "$M18_RUNS/loopback-import/report.json") \
  >"$M18_RUNS/loopback-import/stdout.txt" \
  2>"$M18_RUNS/loopback-import/stderr.txt"

python3 - <<'PY'
import json, os
from pathlib import Path
runs = Path(os.environ["M18_RUNS"])
endpoint = (runs / "loopback-export/endpoint.txt").read_text().strip()
report = json.loads((runs / "loopback-import/report.json").read_text())
events = [json.loads(line) for line in Path(report["trace_path"]).read_text().splitlines()]
assert endpoint.endswith("/mcp"), endpoint
assert any(e["event_type"] == "mcp_import_connected" and e["payload"]["fields"].get("server") == "loopback-export" for e in events), events
assert any(e["event_type"] == "mcp_tool_completed" and e["payload"]["identity"] == "mcp:loopback-export/zack__m18_export_echo_tool" for e in events), events
export_rows = [json.loads(line) for line in (runs / "loopback-export/stdout.jsonl").read_text().splitlines()]
assert any(row.get("method") == "mcp_export_event" for row in export_rows), export_rows
print(json.dumps({"endpoint": endpoint, "import_terminal_status": report["terminal_status"]}, indent=2))
PY

kill "$EXPORT_PID" 2>/dev/null || true
```

Expected:

- One Harness session exports `@zack/m18-export-echo-tool` over MCP.
- A second Harness run imports that endpoint as `loopback-export`.
- The imported Tool completes as `mcp:loopback-export/zack__m18_export_echo_tool`.
- The exporting Harness machine stream includes MCP export activity.

## Test 10: focused automated M18 regressions remain green

```bash
cargo test -p agentpm-cli --no-default-features mcp -- --nocapture
cargo test -p agentpm-cli --no-default-features external_mcp -- --nocapture
cargo test -p agentpm-cli --no-default-features fallback_diagnostics -- --nocapture
cargo clippy -p agentpm-cli --no-default-features --all-targets -- -D warnings
```

Expected:

- All focused MCP import/export tests pass.
- External MCP alias/schema/validation tests pass.
- Provider schema fallback diagnostics pass.
- Clippy stays clean.

## Cleanup

```bash
rm -rf harness-m18-test
```
