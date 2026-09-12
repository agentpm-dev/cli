# Harness Milestone 16b Manual Tests

Manual coverage for Milestone 16b: provider-native action/result turn correlation, no double representation of run input/transcript/repair feedback, persistence-review turn correlation, and repeated-action behavior measurement for the Milestone 16c baseline.

These checks use a disposable Harness workspace plus a local OpenAI/Anthropic-compatible capture server. The capture server lets you inspect the real HTTP JSON body produced by the built-in providers without spending API calls. Optional live-provider runs at the end are for measuring model behavior, not for proving the serializer.

## Coverage Target

| Requirement | Manual coverage |
|---|---|
| Built-in providers send native action/result turns | Tests 1 and 2 capture raw OpenAI and Anthropic request bodies across multiple model turns |
| Provider call IDs are preserved and correlated | Tests 1 and 2 verify assistant tool calls and tool results share the same provider call IDs |
| Prompt text does not duplicate native turn history | Tests 1 and 2 verify Section 6 transcript prose is omitted from provider prompt text when native turns are used |
| Run input appears once | Tests 1 and 2 verify run input is the first provider-native user turn, not repeated in Section 3 prompt text |
| RepairFeedback appears once | Test 3 captures an invalid tool call followed by repair and verifies repair feedback is a native user turn only |
| Persistence review uses the same turn mechanism | Test 4 captures a run-end review Memory write and verifies the next review request carries paired action/result turns |
| M16c repeated-action baseline is recorded before prompt work | Test 5 records live OpenAI/Anthropic repeated-action counts against the M16b baseline |

## Prerequisites

From the root of the `agentpm` repo:

```bash
cargo build -p agentpm-cli
export APM="$PWD/target/debug/agentpm"
export AGENTPM_MANUAL_PYTHON="${AGENTPM_MANUAL_PYTHON:-python3}"
```

The deterministic capture-server tests do not need real provider keys. They use dummy keys and local base URLs.

For optional live-provider repeated-action measurement, set real keys and unset the capture base URLs:

```bash
export OPENAI_API_KEY="your OpenAI key"
export ANTHROPIC_API_KEY="your Anthropic key"
unset OPENAI_BASE_URL
unset ANTHROPIC_BASE_URL
```

## Setup

Run this from the root of the `agentpm` repo:

```bash
cat > /tmp/setup-harness-m16b-manual.sh <<'SH'
#!/usr/bin/env bash
set -euo pipefail

ROOT="$(pwd)"
APM="${APM:-$ROOT/target/debug/agentpm}"
BASE="${HARNESS_M16B_TEST_BASE:-$ROOT/harness-m16b-test}"
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
  local package="@zack/m16b-native-turn-search-tool-with-long-readable-name"
  local manifest_name="${package#*/}"
  local dir
  dir="$(pkg_dir tools "$package" "0.1.0")"
  mkdir -p "$dir"
  cat > "$dir/agent.json" <<JSON
{
  "\$schema": "$SCHEMA_URL",
  "kind": "tool",
  "name": "$manifest_name",
  "version": "0.1.0",
  "description": "M16b Tool used to verify provider-native action/result turns.",
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
    "required": ["query"],
    "properties": {
      "query": { "type": "string" }
    }
  },
  "outputs": {
    "type": "object",
    "additionalProperties": false,
    "required": ["ok", "query", "result"],
    "properties": {
      "ok": { "type": "boolean" },
      "query": { "type": "string" },
      "result": { "type": "string" }
    }
  },
  "files": ["script.py"]
}
JSON
  cat > "$dir/script.py" <<'PY'
import json
import sys

payload = json.load(sys.stdin)
print(json.dumps({
    "ok": True,
    "query": payload["query"],
    "result": "M16b capture Tool result for " + payload["query"],
}))
PY
  "$APM" lint "$dir/agent.json" >/dev/null
}

write_memory() {
  local package="@zack/m16b-native-turn-memory-package"
  local manifest_name="${package#*/}"
  local dir
  dir="$(pkg_dir memory "$package" "0.1.0")"
  mkdir -p "$dir/schemas"
  cat > "$dir/agent.json" <<JSON
{
  "\$schema": "$SCHEMA_URL",
  "kind": "memory",
  "name": "$manifest_name",
  "version": "0.1.0",
  "description": "M16b Memory Blueprint for native action/result turn tests.",
  "memory": {
    "scopes": {
      "user": { "description": "Manual user scope." }
    },
    "record_types": {
      "note": {
        "version": "1.0.0",
        "description": "Manual note.",
        "schema": "schemas/note.schema.json"
      }
    },
    "spaces": {
      "notes_with_native_turn_history": {
        "description": "Notes used by M16b native turn tests.",
        "model": "collection",
        "record_types": ["note"],
        "scope": ["user"],
        "retrieval": { "modes": ["key", "chronological", "filter"] }
      }
    }
  }
}
JSON
  cat > "$dir/schemas/note.schema.json" <<'JSON'
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "type": "object",
  "required": ["body"],
  "properties": {
    "body": { "type": "string", "minLength": 1 },
    "tag": { "type": "string" }
  },
  "additionalProperties": false
}
JSON
  "$APM" lint "$dir/agent.json" >/dev/null
  "$APM" memory build --manifest "$dir/agent.json" >/dev/null
}

write_loop_agent_lock() {
  local loop_package="@zack/m16b-native-turn-loop"
  local loop_dir
  loop_dir="$(pkg_dir loops "$loop_package" "0.1.0")"
  mkdir -p "$loop_dir"
  cat > "$loop_dir/agent.json" <<JSON
{
  "\$schema": "$SCHEMA_URL",
  "kind": "loop",
  "name": "m16b-native-turn-loop",
  "version": "0.1.0",
  "description": "M16b loop for native provider action/result turn tests.",
  "loop": {
    "archetype": "act_finish",
    "entry_phase": "exercise",
    "limits": { "max_steps": 8 },
    "phases": [
      {
        "id": "exercise",
        "objective": "Use the requested semantic actions, then complete.",
        "access": {
          "tools": true,
          "memory": { "read": true, "write": true }
        },
        "outcomes": [
          { "id": "done", "description": "The M16b manual check is complete." }
        ]
      }
    ],
    "transitions": [
      { "from": "exercise", "on": "done", "to": "\$end" }
    ]
  }
}
JSON
  "$APM" lint "$loop_dir/agent.json" >/dev/null

  cat > "$WORK/agent.json" <<JSON
{
  "\$schema": "$SCHEMA_URL",
  "kind": "agent",
  "name": "m16b-manual-agent",
  "version": "0.1.0",
  "description": "M16b manual Harness native turn test agent.",
  "tools": ["@zack/m16b-native-turn-search-tool-with-long-readable-name@0.1.0"],
  "memory": ["@zack/m16b-native-turn-memory-package@0.1.0"],
  "loop": "@zack/m16b-native-turn-loop@0.1.0",
  "bindings": {
    "global": {
      "memory": [
        {
          "package": "@zack/m16b-native-turn-memory-package",
          "spaces": ["notes_with_native_turn_history"]
        }
      ]
    },
    "phases": {
      "exercise": {
        "tools": ["@zack/m16b-native-turn-search-tool-with-long-readable-name"]
      }
    }
  }
}
JSON
  "$APM" lint "$WORK/agent.json" >/dev/null

  cat > "$WORK/agent.lock" <<'JSON'
{
  "lockfile_version": 3,
  "generated": "2026-09-12T00:00:00Z",
  "packages": {
    "tool:@zack/m16b-native-turn-search-tool-with-long-readable-name@0.1.0": {
      "kind": "tool",
      "name": "@zack/m16b-native-turn-search-tool-with-long-readable-name",
      "version": "0.1.0",
      "integrity": "sha256-manual"
    },
    "memory:@zack/m16b-native-turn-memory-package@0.1.0": {
      "kind": "memory",
      "name": "@zack/m16b-native-turn-memory-package",
      "version": "0.1.0",
      "integrity": "sha256-manual"
    },
    "loop:@zack/m16b-native-turn-loop@0.1.0": {
      "kind": "loop",
      "name": "@zack/m16b-native-turn-loop",
      "version": "0.1.0",
      "integrity": "sha256-manual"
    }
  },
  "roots": {
    "local:agent": {
      "name": "m16b-manual-agent",
      "version": "0.1.0",
      "tools": ["tool:@zack/m16b-native-turn-search-tool-with-long-readable-name@0.1.0"],
      "skills": [],
      "knowledge": [],
      "memory": ["memory:@zack/m16b-native-turn-memory-package@0.1.0"],
      "profiles": [],
      "loop": "loop:@zack/m16b-native-turn-loop@0.1.0"
    }
  }
}
JSON
}

write_configs() {
  cat > "$WORK/agentpm.m16b.openai.harness.json" <<'JSON'
{
  "version": 1,
  "model": {
    "provider": "openai",
    "model": "gpt-4o-mini"
  },
  "scopes": {
    "user": "m16b-user-openai"
  },
  "runtime": {
    "state_dir": ".agentpm-state-m16b-openai",
    "limits": {
      "max_steps": 8,
      "max_model_calls_per_phase": 5,
      "max_tool_calls_per_phase": 4,
      "max_actions_per_phase": 12,
      "max_structured_output_repairs": 2,
      "max_tool_call_repairs": 2
    }
  },
  "trace": {
    "level": "verbose"
  }
}
JSON

  cat > "$WORK/agentpm.m16b.anthropic.harness.json" <<'JSON'
{
  "version": 1,
  "model": {
    "provider": "anthropic",
    "model": "claude-3-5-haiku-latest"
  },
  "scopes": {
    "user": "m16b-user-anthropic"
  },
  "runtime": {
    "state_dir": ".agentpm-state-m16b-anthropic",
    "limits": {
      "max_steps": 8,
      "max_model_calls_per_phase": 5,
      "max_tool_calls_per_phase": 4,
      "max_actions_per_phase": 12,
      "max_structured_output_repairs": 2,
      "max_tool_call_repairs": 2
    }
  },
  "trace": {
    "level": "verbose"
  }
}
JSON

  cat > "$WORK/agentpm.m16b.anthropic.review.harness.json" <<'JSON'
{
  "version": 1,
  "model": {
    "provider": "anthropic",
    "model": "claude-3-5-haiku-latest"
  },
  "scopes": {
    "user": "m16b-user-anthropic-review"
  },
  "runtime": {
    "state_dir": ".agentpm-state-m16b-anthropic-review",
    "limits": {
      "max_steps": 8,
      "max_model_calls_per_phase": 5,
      "max_tool_calls_per_phase": 4,
      "max_actions_per_phase": 12,
      "max_structured_output_repairs": 2,
      "max_tool_call_repairs": 2
    }
  },
  "memory": {
    "write_review": {
      "points": ["run_end"]
    }
  },
  "trace": {
    "level": "verbose"
  }
}
JSON
}

write_capture_server() {
  mkdir -p "$WORK/scripts"
  cat > "$WORK/scripts/m16b_capture_server.py" <<'PY'
#!/usr/bin/env python3
import argparse
import json
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

parser = argparse.ArgumentParser()
parser.add_argument("--host", default="127.0.0.1")
parser.add_argument("--port", type=int, required=True)
parser.add_argument("--log", required=True)
parser.add_argument("--provider", choices=["openai", "anthropic"], required=True)
parser.add_argument("--scenario", choices=["multi", "repair", "review"], required=True)
args = parser.parse_args()

log_path = Path(args.log)
log_path.parent.mkdir(parents=True, exist_ok=True)
sequence = 0

def alias_for(body, prefix):
    tools = body.get("tools") or []
    for tool in tools:
        if args.provider == "openai":
            name = ((tool.get("function") or {}).get("name") or "")
        else:
            name = tool.get("name") or ""
        if name.startswith(prefix):
            return name
    raise RuntimeError(f"missing tool alias starting with {prefix}")

def write_log(path, body):
    global sequence
    sequence += 1
    with log_path.open("a") as handle:
        handle.write(json.dumps({
            "sequence": sequence,
            "provider": args.provider,
            "scenario": args.scenario,
            "path": path,
            "body": body,
        }, sort_keys=True) + "\n")

def openai_tool_response(alias, call_id, arguments):
    return {
        "id": "chatcmpl-m16b-capture",
        "object": "chat.completion",
        "choices": [{
            "index": 0,
            "message": {
                "role": "assistant",
                "content": None,
                "tool_calls": [{
                    "id": call_id,
                    "type": "function",
                    "function": {
                        "name": alias,
                        "arguments": json.dumps(arguments),
                    },
                }],
            },
            "finish_reason": "tool_calls",
        }],
        "usage": {"prompt_tokens": 1, "completion_tokens": 1, "total_tokens": 2},
    }

def anthropic_tool_response(alias, call_id, arguments):
    return {
        "id": "msg_m16b_capture",
        "type": "message",
        "role": "assistant",
        "content": [{
            "type": "tool_use",
            "id": call_id,
            "name": alias,
            "input": arguments,
        }],
        "stop_reason": "tool_use",
        "usage": {"input_tokens": 1, "output_tokens": 1},
    }

def tool_response(body, prefix, call_id, arguments):
    alias = alias_for(body, prefix)
    if args.provider == "openai":
        return openai_tool_response(alias, call_id, arguments)
    return anthropic_tool_response(alias, call_id, arguments)

def response_for(body):
    if args.scenario == "multi":
        if sequence == 1:
            return tool_response(body, "agentpm_tool_", "call_m16b_tool_1", {
                "arguments": {"query": "m16b native correlation"}
            })
        if sequence == 2:
            return tool_response(body, "memory_write_", "call_m16b_memory_write_1", {
                "operation": "create",
                "record_type": "note",
                "content": {
                    "body": "M16b captured provider-native Memory write.",
                    "tag": "m16b",
                },
            })
        return tool_response(body, "phase_complete", "call_m16b_phase_complete_1", {
            "outcome": "done",
            "output": {"summary": "m16b multi-turn capture complete"},
        })

    if args.scenario == "repair":
        if sequence == 1:
            return tool_response(body, "agentpm_tool_", "call_m16b_invalid_tool_1", {
                "arguments": {}
            })
        return tool_response(body, "phase_complete", "call_m16b_repair_complete_1", {
            "outcome": "done",
            "output": {"summary": "m16b repair capture complete"},
        })

    if args.scenario == "review":
        if sequence == 1:
            return tool_response(body, "phase_complete", "call_m16b_phase_done_1", {
                "outcome": "done",
                "output": {"summary": "enter review"},
            })
        if sequence == 2:
            return tool_response(body, "memory_write_", "call_m16b_review_memory_write_1", {
                "operation": "create",
                "record_type": "note",
                "content": {
                    "body": "M16b captured persistence review write.",
                    "tag": "m16b-review",
                },
            })
        return tool_response(body, "persistence_review_complete", "call_m16b_review_complete_1", {})

    raise RuntimeError("unknown scenario")

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
        body = json.loads(self.rfile.read(length) or b"{}")
        write_log(self.path, body)
        try:
            response = response_for(body)
            payload = json.dumps(response).encode()
            self.send_response(200)
            self.send_header("content-type", "application/json")
            self.send_header("content-length", str(len(payload)))
            self.end_headers()
            self.wfile.write(payload)
        except Exception as exc:
            payload = json.dumps({"error": str(exc)}).encode()
            self.send_response(500)
            self.send_header("content-type", "application/json")
            self.send_header("content-length", str(len(payload)))
            self.end_headers()
            self.wfile.write(payload)

    def log_message(self, fmt, *values):
        return

server = ThreadingHTTPServer((args.host, args.port), Handler)
print(f"ready http://{args.host}:{args.port}", flush=True)
server.serve_forever()
PY
  chmod +x "$WORK/scripts/m16b_capture_server.py"
}

write_assertions() {
  cat > "$WORK/scripts/m16b_assert_capture.py" <<'PY'
#!/usr/bin/env python3
import json
import sys
from pathlib import Path

report_path = Path(sys.argv[1])
body_log_path = Path(sys.argv[2])
provider = sys.argv[3]
scenario = sys.argv[4]

def fail(message):
    raise AssertionError(message)

def load_jsonl(path):
    return [json.loads(line) for line in path.read_text().splitlines() if line.strip()]

def messages(body):
    return body.get("messages") or []

def prompt_text(body):
    if provider == "openai":
        first = messages(body)[0]
        return first.get("content") or ""
    return body.get("system") or ""

def has_openai_tool_result(body, call_id):
    return any(msg.get("role") == "tool" and msg.get("tool_call_id") == call_id for msg in messages(body))

def has_openai_tool_call(body, call_id):
    for msg in messages(body):
        for call in msg.get("tool_calls") or []:
            if call.get("id") == call_id:
                return True
    return False

def anthropic_content_items(body):
    items = []
    for msg in messages(body):
        content = msg.get("content")
        if isinstance(content, list):
            items.extend(content)
    return items

def has_anthropic_tool_result(body, call_id):
    return any(item.get("type") == "tool_result" and item.get("tool_use_id") == call_id for item in anthropic_content_items(body))

def has_anthropic_tool_call(body, call_id):
    return any(item.get("type") == "tool_use" and item.get("id") == call_id for item in anthropic_content_items(body))

def has_tool_call(body, call_id):
    return has_openai_tool_call(body, call_id) if provider == "openai" else has_anthropic_tool_call(body, call_id)

def has_tool_result(body, call_id):
    return has_openai_tool_result(body, call_id) if provider == "openai" else has_anthropic_tool_result(body, call_id)

report = json.loads(report_path.read_text())
events = load_jsonl(Path(report["trace_path"]))
bodies = [row["body"] for row in load_jsonl(body_log_path)]
if not bodies:
    fail("capture server recorded no provider bodies")

provider_requests = [
    event for event in events
    if event.get("event_type") == "model_runtime_request_prepared"
    and (((event.get("payload") or {}).get("fields") or {}).get("request_kind") == "provider_wire_request")
]
if not provider_requests:
    fail("trace has no provider_wire_request snapshots")

for event in provider_requests:
    fields = event["payload"]["fields"]
    if fields.get("turn_strategy") != "native_action_result_turns":
        fail(f"unexpected turn strategy: {fields.get('turn_strategy')}")

first_prompt = prompt_text(bodies[0])
if "EFFECTIVE CAPABILITY CATALOG" in first_prompt:
    fail("provider prompt leaked prose capability catalog")
if "CURRENT PHASE-LOCAL TRANSCRIPT" in first_prompt:
    fail("provider prompt leaked phase-local transcript section")
if "Run input:\n" in first_prompt:
    fail("provider prompt duplicated run input in text prompt")
first_messages = messages(bodies[0])
first_user_index = 1 if provider == "openai" else 0
if len(first_messages) <= first_user_index or first_messages[first_user_index].get("role") != "user":
    fail("first provider-native turn is not user input")

if scenario == "multi":
    if len(bodies) < 3:
        fail(f"multi scenario expected at least 3 provider requests, saw {len(bodies)}")
    if not has_tool_call(bodies[1], "call_m16b_tool_1"):
        fail("second body missing prior Tool call")
    if not has_tool_result(bodies[1], "call_m16b_tool_1"):
        fail("second body missing prior Tool result")
    if not has_tool_call(bodies[2], "call_m16b_memory_write_1"):
        fail("third body missing prior Memory write call")
    if not has_tool_result(bodies[2], "call_m16b_memory_write_1"):
        fail("third body missing prior Memory write result")

elif scenario == "repair":
    if len(bodies) < 2:
        fail("repair scenario expected a second provider request")
    body_text = json.dumps(bodies[1])
    if body_text.count("Repair feedback from previous turn:") != 1:
        fail("repair feedback should appear exactly once in second provider body")
    if "Repair feedback from previous turn:" in prompt_text(bodies[1]):
        fail("repair feedback was duplicated into provider prompt text")

elif scenario == "review":
    if len(bodies) < 3:
        fail("review scenario expected phase request plus at least two review requests")
    if not has_tool_call(bodies[2], "call_m16b_review_memory_write_1"):
        fail("third body missing prior review Memory write call")
    if not has_tool_result(bodies[2], "call_m16b_review_memory_write_1"):
        fail("third body missing prior review Memory write result")

print(f"ok: {provider} {scenario} provider bodies preserve M16b native turn shape")
print(f"captured bodies: {body_log_path}")
PY
  chmod +x "$WORK/scripts/m16b_assert_capture.py"

  cat > "$WORK/scripts/m16b_measure_repeats.py" <<'PY'
#!/usr/bin/env python3
import json
import sys
from collections import Counter
from pathlib import Path

report_path = Path(sys.argv[1]).resolve()
report = json.loads(report_path.read_text())
trace_path = Path(report["trace_path"]).resolve()
events = [json.loads(line) for line in trace_path.read_text().splitlines() if line.strip()]
actions = []
families = []
for event in events:
    if event.get("event_type") != "semantic_action_proposed":
        continue
    payload = event.get("payload") or {}
    fields = payload.get("fields") or {}
    kind = payload.get("action_kind")
    identity = payload.get("identity")
    actions.append((
        kind,
        identity,
        json.dumps(fields, sort_keys=True),
    ))
    families.append((kind, identity))

counts = Counter(actions)
family_counts = Counter(families)
usage = report.get("usage") or {}
print(f"report: {report_path}")
print(f"trace: {trace_path}")
print(f"terminal status: {report.get('terminal_status')}")
print(f"report accepted_semantic_actions: {usage.get('accepted_semantic_actions')}")
print("accepted semantic actions:")
for (kind, identity, fields), count in counts.items():
    repeat = " repeated" if count > 1 else ""
    print(f"  {count}x {kind} {identity}{repeat}")
print(f"total accepted actions: {sum(counts.values())}")
print(f"repeated exact action+identity+fields entries: {sum(1 for count in counts.values() if count > 1)}")
print("repeated action kind+identity entries, ignoring argument changes:")
for (kind, identity), count in family_counts.items():
    if count > 1:
        print(f"  {count}x {kind} {identity}")

provider_requests = [
    event for event in events
    if event.get("event_type") == "model_runtime_request_prepared"
    and (((event.get("payload") or {}).get("fields") or {}).get("request_kind") == "provider_wire_request")
]
if provider_requests:
    fields = provider_requests[-1]["payload"]["fields"]
    print("final provider request ordered_turns:")
    for turn in fields.get("ordered_turns", []):
        print("  " + json.dumps(turn, sort_keys=True))
PY
  chmod +x "$WORK/scripts/m16b_measure_repeats.py"
}

write_tool
write_memory
write_loop_agent_lock
write_configs
write_capture_server
write_assertions

cat > "$BASE/env.sh" <<EOF
export HARNESS_M16B_TEST_BASE="$BASE"
export M16B_WORK="$WORK"
export M16B_RUNS="$RUNS"
export APM="$APM"
export AGENTPM_MANUAL_PYTHON="$PYTHON_CMD"
EOF

echo "M16b manual workspace ready:"
echo "  $WORK"
echo "Run: source \"$BASE/env.sh\""
SH

bash /tmp/setup-harness-m16b-manual.sh
source "${HARNESS_M16B_TEST_BASE:-$PWD/harness-m16b-test}/env.sh"
```

## Test 0: Focused Automated Smoke

Run the tests that pin the M16b request-shape behavior before manual provider-body inspection:

```bash
mkdir -p "$M16B_RUNS/rust"

cargo test -p agentpm-cli \
  multi_turn_phase_carries_provider_native_call_ids_into_ordered_turns \
  -- --nocapture \
  | tee "$M16B_RUNS/rust/multi-turn-native-ids.txt"

cargo test -p agentpm-cli \
  built_in_provider_bodies_preserve_native_action_result_correlation \
  -- --nocapture \
  | tee "$M16B_RUNS/rust/provider-body-correlation.txt"

cargo test -p agentpm-cli \
  built_in_provider_body_degrades_orphaned_action_result_to_text \
  -- --nocapture \
  | tee "$M16B_RUNS/rust/orphan-result-degrades.txt"

cargo test -p agentpm-cli \
  native_provider_request_preserves_before_model_request_hook_context \
  -- --nocapture \
  | tee "$M16B_RUNS/rust/hook-context-native-turns.txt"

cargo test -p agentpm-cli \
  memory_write_review_carries_provider_native_call_ids_into_ordered_turns \
  -- --nocapture \
  | tee "$M16B_RUNS/rust/review-native-turns.txt"
```

Expected:

- all focused tests pass;
- the automated baseline covers ordinary phase turns, provider body serialization, orphan-result degradation, hook context, and persistence-review turns.

## Test 1: OpenAI Capture, Multiple Calls In One Phase

This uses the built-in OpenAI transport pointed at the local capture server.

```bash
mkdir -p "$M16B_RUNS/openai-multi"
rm -f "$M16B_RUNS/openai-multi/bodies.jsonl"
rm -rf "$M16B_WORK/.agentpm-state-m16b-openai"

"$AGENTPM_MANUAL_PYTHON" "$M16B_WORK/scripts/m16b_capture_server.py" \
  --provider openai \
  --scenario multi \
  --port 18080 \
  --log "$M16B_RUNS/openai-multi/bodies.jsonl" \
  >"$M16B_RUNS/openai-multi/server.stdout.txt" \
  2>"$M16B_RUNS/openai-multi/server.stderr.txt" &
SERVER_PID=$!
trap 'kill "$SERVER_PID" 2>/dev/null || true' EXIT
until curl -fsS http://127.0.0.1:18080/health >/dev/null 2>&1; do sleep 0.1; done

export OPENAI_API_KEY="m16b-capture-key"
export OPENAI_BASE_URL="http://127.0.0.1:18080/v1/chat/completions"

REPORT="$M16B_RUNS/openai-multi/report.json"
(cd "$M16B_WORK" && "$APM" harness \
  --config agentpm.m16b.openai.harness.json \
  --headless \
  --scope user=m16b-user-openai-multi \
  --input "Run the M16b OpenAI capture scenario: use the search tool, write Memory, then complete." \
  --report "$REPORT" \
  >"$M16B_RUNS/openai-multi/stdout.txt" \
  2>"$M16B_RUNS/openai-multi/stderr.txt")

kill "$SERVER_PID" 2>/dev/null || true
trap - EXIT

"$AGENTPM_MANUAL_PYTHON" "$M16B_WORK/scripts/m16b_assert_capture.py" \
  "$REPORT" \
  "$M16B_RUNS/openai-multi/bodies.jsonl" \
  openai \
  multi \
  | tee "$M16B_RUNS/openai-multi/assertion.txt"
```

Expected:

- run ends `ended`;
- body 1 has a `system` message with Harness control/context and a separate first `user` message for run input;
- body 1 includes OpenAI `tools`, not a prose `EFFECTIVE CAPABILITY CATALOG`;
- body 2 includes an assistant `tool_calls[0].id == call_m16b_tool_1` and a later `role: tool` result with `tool_call_id == call_m16b_tool_1`;
- body 3 includes the same correlation for `call_m16b_memory_write_1`;
- Section 6 transcript prose is absent from provider prompt text.

Useful body inspection:

```bash
"$AGENTPM_MANUAL_PYTHON" - "$M16B_RUNS/openai-multi/bodies.jsonl" <<'PY'
import json, sys
from pathlib import Path
for row in [json.loads(line) for line in Path(sys.argv[1]).read_text().splitlines() if line.strip()]:
    body = row["body"]
    print(f"\n--- OpenAI body {row['sequence']} ---")
    print(json.dumps({
        "tools": [tool["function"]["name"] for tool in body.get("tools", [])],
        "messages": body.get("messages", []),
    }, indent=2))
PY
```

## Test 2: Anthropic Capture, Multiple Calls In One Phase

This uses the built-in Anthropic transport pointed at the local capture server.

```bash
mkdir -p "$M16B_RUNS/anthropic-multi"
rm -f "$M16B_RUNS/anthropic-multi/bodies.jsonl"
rm -rf "$M16B_WORK/.agentpm-state-m16b-anthropic"

"$AGENTPM_MANUAL_PYTHON" "$M16B_WORK/scripts/m16b_capture_server.py" \
  --provider anthropic \
  --scenario multi \
  --port 18081 \
  --log "$M16B_RUNS/anthropic-multi/bodies.jsonl" \
  >"$M16B_RUNS/anthropic-multi/server.stdout.txt" \
  2>"$M16B_RUNS/anthropic-multi/server.stderr.txt" &
SERVER_PID=$!
trap 'kill "$SERVER_PID" 2>/dev/null || true' EXIT
until curl -fsS http://127.0.0.1:18081/health >/dev/null 2>&1; do sleep 0.1; done

export ANTHROPIC_API_KEY="m16b-capture-key"
export ANTHROPIC_BASE_URL="http://127.0.0.1:18081/v1/messages"

REPORT="$M16B_RUNS/anthropic-multi/report.json"
(cd "$M16B_WORK" && "$APM" harness \
  --config agentpm.m16b.anthropic.harness.json \
  --headless \
  --scope user=m16b-user-anthropic-multi \
  --input "Run the M16b Anthropic capture scenario: use the search tool, write Memory, then complete." \
  --report "$REPORT" \
  >"$M16B_RUNS/anthropic-multi/stdout.txt" \
  2>"$M16B_RUNS/anthropic-multi/stderr.txt")

kill "$SERVER_PID" 2>/dev/null || true
trap - EXIT

"$AGENTPM_MANUAL_PYTHON" "$M16B_WORK/scripts/m16b_assert_capture.py" \
  "$REPORT" \
  "$M16B_RUNS/anthropic-multi/bodies.jsonl" \
  anthropic \
  multi \
  | tee "$M16B_RUNS/anthropic-multi/assertion.txt"
```

Expected:

- run ends `ended`;
- provider body uses Anthropic `system` plus `messages`;
- body 2 includes a prior `tool_use` with `id == call_m16b_tool_1` and a `tool_result` with `tool_use_id == call_m16b_tool_1`;
- body 3 includes the same correlation for `call_m16b_memory_write_1`;
- provider prompt text does not duplicate the capability catalog, run input, or phase-local transcript.

Useful body inspection:

```bash
"$AGENTPM_MANUAL_PYTHON" - "$M16B_RUNS/anthropic-multi/bodies.jsonl" <<'PY'
import json, sys
from pathlib import Path
for row in [json.loads(line) for line in Path(sys.argv[1]).read_text().splitlines() if line.strip()]:
    body = row["body"]
    print(f"\n--- Anthropic body {row['sequence']} ---")
    print(json.dumps({
        "tools": [tool["name"] for tool in body.get("tools", [])],
        "system": body.get("system"),
        "messages": body.get("messages", []),
    }, indent=2))
PY
```

## Test 3: OpenAI Capture, Repair Feedback Appears Once

This test intentionally returns an invalid Tool call first. Harness should request repair, and the second provider body should include repair feedback exactly once as a provider-native user turn.

```bash
mkdir -p "$M16B_RUNS/openai-repair"
rm -f "$M16B_RUNS/openai-repair/bodies.jsonl"
rm -rf "$M16B_WORK/.agentpm-state-m16b-openai"

"$AGENTPM_MANUAL_PYTHON" "$M16B_WORK/scripts/m16b_capture_server.py" \
  --provider openai \
  --scenario repair \
  --port 18082 \
  --log "$M16B_RUNS/openai-repair/bodies.jsonl" \
  >"$M16B_RUNS/openai-repair/server.stdout.txt" \
  2>"$M16B_RUNS/openai-repair/server.stderr.txt" &
SERVER_PID=$!
trap 'kill "$SERVER_PID" 2>/dev/null || true' EXIT
until curl -fsS http://127.0.0.1:18082/health >/dev/null 2>&1; do sleep 0.1; done

export OPENAI_API_KEY="m16b-capture-key"
export OPENAI_BASE_URL="http://127.0.0.1:18082/v1/chat/completions"

REPORT="$M16B_RUNS/openai-repair/report.json"
(cd "$M16B_WORK" && "$APM" harness \
  --config agentpm.m16b.openai.harness.json \
  --headless \
  --scope user=m16b-user-openai-repair \
  --input "Run the M16b repair capture scenario." \
  --report "$REPORT" \
  >"$M16B_RUNS/openai-repair/stdout.txt" \
  2>"$M16B_RUNS/openai-repair/stderr.txt")

kill "$SERVER_PID" 2>/dev/null || true
trap - EXIT

"$AGENTPM_MANUAL_PYTHON" "$M16B_WORK/scripts/m16b_assert_capture.py" \
  "$REPORT" \
  "$M16B_RUNS/openai-repair/bodies.jsonl" \
  openai \
  repair \
  | tee "$M16B_RUNS/openai-repair/assertion.txt"
```

Expected:

- trace contains `semantic_action_rejected` followed by `model_repair_requested`;
- second OpenAI request body contains exactly one `Repair feedback from previous turn:` string;
- that repair feedback is in a `role: user` message, not duplicated into the system prompt.

## Test 4: Anthropic Capture, Persistence Review Native Turns

This test captures the run-end persistence-review path. The first provider call completes the phase, the second review call writes Memory, and the third review call should carry the review Memory write as a paired `tool_use`/`tool_result` before completing review.

```bash
mkdir -p "$M16B_RUNS/anthropic-review"
rm -f "$M16B_RUNS/anthropic-review/bodies.jsonl"
rm -rf "$M16B_WORK/.agentpm-state-m16b-anthropic-review"

"$AGENTPM_MANUAL_PYTHON" "$M16B_WORK/scripts/m16b_capture_server.py" \
  --provider anthropic \
  --scenario review \
  --port 18083 \
  --log "$M16B_RUNS/anthropic-review/bodies.jsonl" \
  >"$M16B_RUNS/anthropic-review/server.stdout.txt" \
  2>"$M16B_RUNS/anthropic-review/server.stderr.txt" &
SERVER_PID=$!
trap 'kill "$SERVER_PID" 2>/dev/null || true' EXIT
until curl -fsS http://127.0.0.1:18083/health >/dev/null 2>&1; do sleep 0.1; done

export ANTHROPIC_API_KEY="m16b-capture-key"
export ANTHROPIC_BASE_URL="http://127.0.0.1:18083/v1/messages"

REPORT="$M16B_RUNS/anthropic-review/report.json"
(cd "$M16B_WORK" && "$APM" harness \
  --config agentpm.m16b.anthropic.review.harness.json \
  --headless \
  --scope user=m16b-user-anthropic-review \
  --input "Complete with outcome done. Let persistence review decide whether to write Memory." \
  --report "$REPORT" \
  >"$M16B_RUNS/anthropic-review/stdout.txt" \
  2>"$M16B_RUNS/anthropic-review/stderr.txt")

kill "$SERVER_PID" 2>/dev/null || true
trap - EXIT

"$AGENTPM_MANUAL_PYTHON" "$M16B_WORK/scripts/m16b_assert_capture.py" \
  "$REPORT" \
  "$M16B_RUNS/anthropic-review/bodies.jsonl" \
  anthropic \
  review \
  | tee "$M16B_RUNS/anthropic-review/assertion.txt"
```

Expected:

- run ends `ended`;
- trace contains `memory_write_review_started` and `memory_write_review_completed`;
- third Anthropic body contains `tool_use.id == call_m16b_review_memory_write_1`;
- third Anthropic body contains `tool_result.tool_use_id == call_m16b_review_memory_write_1`;
- persistence review remains constrained to Memory read/write plus `persistence_review_complete`.

## Test 5: Optional Live Provider Repeated-Action Baseline For M16c

Run this only after the capture tests pass. This records what the real model does with the M16b native-turn baseline before Milestone 16c prompt-side salience work.

Use fresh scopes each run so old Memory does not affect behavior.

### OpenAI

```bash
mkdir -p "$M16B_RUNS/live-openai"
unset OPENAI_BASE_URL
test -n "${OPENAI_API_KEY:-}"

REPORT="$M16B_RUNS/live-openai/repeated-action-baseline.report.json"
(cd "$M16B_WORK" && "$APM" harness \
  --config agentpm.m16b.openai.harness.json \
  --headless \
  --scope user="m16b-live-openai-$(date +%s)" \
  --input "Use the M16b search tool once with query 'launch readiness', then write one Memory note summarizing the result, then complete with outcome done." \
  --report "$REPORT" \
  >"$M16B_RUNS/live-openai/stdout.txt" \
  2>"$M16B_RUNS/live-openai/stderr.txt")

"$AGENTPM_MANUAL_PYTHON" "$M16B_WORK/scripts/m16b_measure_repeats.py" "$REPORT" \
  | tee "$M16B_RUNS/live-openai/repeat-summary.txt"
```

### Anthropic

```bash
mkdir -p "$M16B_RUNS/live-anthropic"
unset ANTHROPIC_BASE_URL
test -n "${ANTHROPIC_API_KEY:-}"

REPORT="$M16B_RUNS/live-anthropic/repeated-action-baseline.report.json"
(cd "$M16B_WORK" && "$APM" harness \
  --config agentpm.m16b.anthropic.harness.json \
  --headless \
  --scope user="m16b-live-anthropic-$(date +%s)" \
  --input "Use the M16b search tool once with query 'launch readiness', then write one Memory note summarizing the result, then complete with outcome done." \
  --report "$REPORT" \
  >"$M16B_RUNS/live-anthropic/stdout.txt" \
  2>"$M16B_RUNS/live-anthropic/stderr.txt")

"$AGENTPM_MANUAL_PYTHON" "$M16B_WORK/scripts/m16b_measure_repeats.py" "$REPORT" \
  | tee "$M16B_RUNS/live-anthropic/repeat-summary.txt"
```

Expected:

- This is a measurement, not a pass/fail gate.
- Record whether exact action+identity+fields entries repeat.
- Also inspect repeated action kind+identity entries that changed arguments, since those are legitimate but still useful evidence for whether the model recognized prior work.
- Inspect the second and later `model_runtime_request_prepared` events to confirm `ordered_turns` include prior action/result turns.
- Use the observed repeats to decide how much of Milestone 16c prompt-side salience is still needed.

Useful trace inspection:

```bash
"$AGENTPM_MANUAL_PYTHON" - "$REPORT" <<'PY'
import json, sys
from pathlib import Path
report = json.loads(Path(sys.argv[1]).read_text())
events = [json.loads(line) for line in Path(report["trace_path"]).read_text().splitlines() if line.strip()]
for event in events:
    if event.get("event_type") != "model_runtime_request_prepared":
        continue
    fields = event["payload"]["fields"]
    print(f"\nrequest_kind={fields.get('request_kind')} turn_strategy={fields.get('turn_strategy')} model={fields.get('model')} provider={fields.get('provider')}")
    for turn in fields.get("ordered_turns", []):
        print(" ", json.dumps(turn, sort_keys=True))
PY
```

## Cleanup

The setup uses only a generated fixture under `harness-m16b-test`.

```bash
rm -rf "$HARNESS_M16B_TEST_BASE"
unset OPENAI_BASE_URL
unset ANTHROPIC_BASE_URL
```

## Regression Tests

After manual testing, rerun the focused automated checks before handoff:

```bash
cargo test -p agentpm-cli harness_runtime::model::tests
cargo test -p agentpm-cli harness_runtime::provider::tests
cargo test -p agentpm-cli memory_write_review
cargo fmt --all -- --check
git diff --check
```

## Additional M16c Baseline Measurements

These are optional follow-up measurements to run after the M16b manual tests above. They do not replace or modify Tests 1-5. They are designed to gather better evidence for Milestone 16c step 1, especially where a procedural prompt can hide repeated-action behavior.

Run these before the Cleanup section, or rerun Setup first if you already removed `harness-m16b-test`.

Use fresh scopes each run so old Memory does not affect behavior.

The repeat helper reports exact repeats plus repeated action kind+identity entries with changed arguments. Treat changed-argument repeats as allowed behavior, but inspect them because they can still show that the model did not recognize earlier work.

### Test 6: Objective Live Baseline

This repeats Test 5 with an objective-style prompt. It deliberately avoids saying "once," avoids a fixed action sequence, and avoids telling the model which terminal action to call.

#### OpenAI

```bash
mkdir -p "$M16B_RUNS/live-openai-objective"
unset OPENAI_BASE_URL
test -n "${OPENAI_API_KEY:-}"

REPORT="$M16B_RUNS/live-openai-objective/repeated-action-baseline.report.json"
(cd "$M16B_WORK" && "$APM" harness \
  --config agentpm.m16b.openai.harness.json \
  --headless \
  --scope user="m16b-objective-openai-$(date +%s)" \
  --input "Find out whether the launch is ready, and record anything worth remembering." \
  --report "$REPORT" \
  >"$M16B_RUNS/live-openai-objective/stdout.txt" \
  2>"$M16B_RUNS/live-openai-objective/stderr.txt")

"$AGENTPM_MANUAL_PYTHON" "$M16B_WORK/scripts/m16b_measure_repeats.py" "$REPORT" \
  | tee "$M16B_RUNS/live-openai-objective/repeat-summary.txt"
```

#### Anthropic

```bash
mkdir -p "$M16B_RUNS/live-anthropic-objective"
unset ANTHROPIC_BASE_URL
test -n "${ANTHROPIC_API_KEY:-}"

REPORT="$M16B_RUNS/live-anthropic-objective/repeated-action-baseline.report.json"
(cd "$M16B_WORK" && "$APM" harness \
  --config agentpm.m16b.anthropic.harness.json \
  --headless \
  --scope user="m16b-objective-anthropic-$(date +%s)" \
  --input "Find out whether the launch is ready, and record anything worth remembering." \
  --report "$REPORT" \
  >"$M16B_RUNS/live-anthropic-objective/stdout.txt" \
  2>"$M16B_RUNS/live-anthropic-objective/stderr.txt")

"$AGENTPM_MANUAL_PYTHON" "$M16B_WORK/scripts/m16b_measure_repeats.py" "$REPORT" \
  | tee "$M16B_RUNS/live-anthropic-objective/repeat-summary.txt"
```

Record:

- whether exact action+identity+fields entries repeat;
- whether the same Tool/Memory action repeats with changed arguments;
- whether the model progresses from Tool result to Memory write or PhaseCompletion without repeating the Tool;
- whether the model terminates cleanly without explicit completion wording in the user input.

### Test 7: Zero-Result Read Baseline

This measures whether a successful empty Memory read is treated as a no-match result instead of causing an identical blind read loop. It uses a fresh scope where no records should exist.

#### OpenAI

```bash
mkdir -p "$M16B_RUNS/live-openai-zero-read"
unset OPENAI_BASE_URL
test -n "${OPENAI_API_KEY:-}"

REPORT="$M16B_RUNS/live-openai-zero-read/repeated-action-baseline.report.json"
(cd "$M16B_WORK" && "$APM" harness \
  --config agentpm.m16b.openai.harness.json \
  --headless \
  --scope user="m16b-zero-openai-$(date +%s)" \
  --input "Determine whether any launch readiness notes are already remembered for this user." \
  --report "$REPORT" \
  >"$M16B_RUNS/live-openai-zero-read/stdout.txt" \
  2>"$M16B_RUNS/live-openai-zero-read/stderr.txt")

"$AGENTPM_MANUAL_PYTHON" "$M16B_WORK/scripts/m16b_measure_repeats.py" "$REPORT" \
  | tee "$M16B_RUNS/live-openai-zero-read/repeat-summary.txt"
```

#### Anthropic

```bash
mkdir -p "$M16B_RUNS/live-anthropic-zero-read"
unset ANTHROPIC_BASE_URL
test -n "${ANTHROPIC_API_KEY:-}"

REPORT="$M16B_RUNS/live-anthropic-zero-read/repeated-action-baseline.report.json"
(cd "$M16B_WORK" && "$APM" harness \
  --config agentpm.m16b.anthropic.harness.json \
  --headless \
  --scope user="m16b-zero-anthropic-$(date +%s)" \
  --input "Determine whether any launch readiness notes are already remembered for this user." \
  --report "$REPORT" \
  >"$M16B_RUNS/live-anthropic-zero-read/stdout.txt" \
  2>"$M16B_RUNS/live-anthropic-zero-read/stderr.txt")

"$AGENTPM_MANUAL_PYTHON" "$M16B_WORK/scripts/m16b_measure_repeats.py" "$REPORT" \
  | tee "$M16B_RUNS/live-anthropic-zero-read/repeat-summary.txt"
```

Record:

- whether any MemoryRead returns `count: 0`;
- whether an identical MemoryRead repeats after the empty success result;
- whether a near-identical MemoryRead repeats with changed arguments;
- whether the model chooses a different useful action or completion after the empty result.

### Test 8: Similar-Surface And Multi-Phase Baseline

This creates an isolated copy of the M16b manual workspace, then adds a second similar Memory space and a second phase. It does not modify the original `M16B_WORK` used by Tests 1-7.

```bash
export M16C_BASELINE_WORK="$HARNESS_M16B_TEST_BASE/m16c-baseline-workspace"
rm -rf "$M16C_BASELINE_WORK"
cp -R "$M16B_WORK" "$M16C_BASELINE_WORK"

"$AGENTPM_MANUAL_PYTHON" - "$M16C_BASELINE_WORK" <<'PY'
import json
import sys
from pathlib import Path

work = Path(sys.argv[1])

memory_manifest = work / ".agentpm/memory/zack/m16b-native-turn-memory-package/0.1.0/agent.json"
memory = json.loads(memory_manifest.read_text())
memory["memory"]["spaces"]["launch_readiness_notes_with_similar_target_name"] = {
    "description": "Similar note collection used to measure target selection under M16c baseline prompts.",
    "model": "collection",
    "record_types": ["note"],
    "scope": ["user"],
    "retrieval": {"modes": ["key", "chronological", "filter"]},
}
memory_manifest.write_text(json.dumps(memory, indent=2) + "\n")

loop_manifest = work / ".agentpm/loops/zack/m16b-native-turn-loop/0.1.0/agent.json"
loop = json.loads(loop_manifest.read_text())
loop["loop"]["limits"]["max_steps"] = 12
exercise = loop["loop"]["phases"][0]
exercise["outcomes"].append({
    "id": "followup",
    "description": "Continue into a second phase for M16c baseline measurement.",
})
loop["loop"]["phases"].append({
    "id": "followup",
    "objective": "Use the requested semantic actions in this second phase, then complete.",
    "access": {
        "tools": True,
        "memory": {"read": True, "write": True},
    },
    "outcomes": [
        {"id": "done", "description": "The second phase is complete."},
    ],
})
loop["loop"]["transitions"].append({
    "from": "exercise",
    "on": "followup",
    "to": "followup",
})
loop["loop"]["transitions"].append({
    "from": "followup",
    "on": "done",
    "to": "$end",
})
loop_manifest.write_text(json.dumps(loop, indent=2) + "\n")

agent_manifest = work / "agent.json"
agent = json.loads(agent_manifest.read_text())
agent["bindings"]["global"]["memory"][0]["spaces"].append("launch_readiness_notes_with_similar_target_name")
agent["bindings"]["phases"]["followup"] = {
    "tools": ["@zack/m16b-native-turn-search-tool-with-long-readable-name"],
}
agent_manifest.write_text(json.dumps(agent, indent=2) + "\n")

for config_name, state_dir in [
    ("agentpm.m16b.openai.harness.json", ".agentpm-state-m16c-openai"),
    ("agentpm.m16b.anthropic.harness.json", ".agentpm-state-m16c-anthropic"),
]:
    config_path = work / config_name
    config = json.loads(config_path.read_text())
    config["runtime"]["state_dir"] = state_dir
    config_path.write_text(json.dumps(config, indent=2) + "\n")
PY

(cd "$M16C_BASELINE_WORK" && "$APM" lint agent.json >/dev/null)
(cd "$M16C_BASELINE_WORK" && "$APM" memory build \
  --manifest .agentpm/memory/zack/m16b-native-turn-memory-package/0.1.0/agent.json >/dev/null)
```

#### OpenAI

```bash
mkdir -p "$M16B_RUNS/live-openai-similar-multiphase"
unset OPENAI_BASE_URL
test -n "${OPENAI_API_KEY:-}"

REPORT="$M16B_RUNS/live-openai-similar-multiphase/repeated-action-baseline.report.json"
(cd "$M16C_BASELINE_WORK" && "$APM" harness \
  --config agentpm.m16b.openai.harness.json \
  --headless \
  --scope user="m16c-similar-openai-$(date +%s)" \
  --input "Find out whether the launch is ready and record anything worth remembering. Use a followup phase if another pass is needed to verify the result." \
  --report "$REPORT" \
  >"$M16B_RUNS/live-openai-similar-multiphase/stdout.txt" \
  2>"$M16B_RUNS/live-openai-similar-multiphase/stderr.txt")

"$AGENTPM_MANUAL_PYTHON" "$M16B_WORK/scripts/m16b_measure_repeats.py" "$REPORT" \
  | tee "$M16B_RUNS/live-openai-similar-multiphase/repeat-summary.txt"
```

#### Anthropic

```bash
mkdir -p "$M16B_RUNS/live-anthropic-similar-multiphase"
unset ANTHROPIC_BASE_URL
test -n "${ANTHROPIC_API_KEY:-}"

REPORT="$M16B_RUNS/live-anthropic-similar-multiphase/repeated-action-baseline.report.json"
(cd "$M16C_BASELINE_WORK" && "$APM" harness \
  --config agentpm.m16b.anthropic.harness.json \
  --headless \
  --scope user="m16c-similar-anthropic-$(date +%s)" \
  --input "Find out whether the launch is ready and record anything worth remembering. Use a followup phase if another pass is needed to verify the result." \
  --report "$REPORT" \
  >"$M16B_RUNS/live-anthropic-similar-multiphase/stdout.txt" \
  2>"$M16B_RUNS/live-anthropic-similar-multiphase/stderr.txt")

"$AGENTPM_MANUAL_PYTHON" "$M16B_WORK/scripts/m16b_measure_repeats.py" "$REPORT" \
  | tee "$M16B_RUNS/live-anthropic-similar-multiphase/repeat-summary.txt"
```

Record:

- whether the model selects the original `notes_with_native_turn_history` space or the similar `launch_readiness_notes_with_similar_target_name` space;
- whether it repeats the same action in the same phase;
- whether it repeats the same action target with changed arguments;
- whether it enters `followup`;
- if it enters `followup`, whether it repeats the same search/read/write because the phase-local transcript reset and prior PhaseResults became the main cross-phase signal.
