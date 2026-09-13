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
family_fields = {}
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
    family_fields.setdefault((kind, identity), Counter())[json.dumps(fields, sort_keys=True)] += 1

counts = Counter(actions)
family_counts = Counter(families)
usage = report.get("usage") or {}
print(f"report: {report_path}")
print(f"trace: {trace_path}")
print(f"terminal status: {report.get('terminal_status')}")
print(f"report accepted_semantic_actions including phase_completion: {usage.get('accepted_semantic_actions')}")
print("accepted non-completion semantic actions:")
for (kind, identity, fields), count in counts.items():
    repeat = " repeated" if count > 1 else ""
    print(f"  {count}x {kind} {identity}{repeat}")
print(f"total accepted non-completion actions: {sum(counts.values())}")
print(f"repeated exact non-completion action+identity+fields entries: {sum(1 for count in counts.values() if count > 1)}")
print("repeated non-completion action kind+identity entries, ignoring argument changes:")
changed_argument_repeat_count = 0
for (kind, identity), count in family_counts.items():
    if count > 1:
        print(f"  {count}x {kind} {identity}")
        variants = family_fields.get((kind, identity), Counter())
        if len(variants) > 1:
            changed_argument_repeat_count += 1
            print("    changed argument variants:")
            for fields, variant_count in variants.items():
                print(f"      {variant_count}x {fields}")
if changed_argument_repeat_count == 0:
    print("  none with changed arguments")

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

## Additional M16c Split-Read Manual Tests

These tests continue from the generated M16b workspace and start at Test 9. They exercise the Milestone 16c MemoryRead split by provider-facing argument shape:

- document key read has no `record_id`;
- collection key read requires `record_id`;
- chronological read has no key/query/filter arguments;
- filter read requires `filter` and has no key/query arguments;
- full_text read requires `query` and has no key/filter arguments;
- legacy bundled `mode` arguments are rejected and repair feedback names the valid alternatives;
- semantic read remains absent in this manual fixture because no embedding provider is configured.

Run Setup again first if you removed `harness-m16b-test`.

### Test 9: Create The M16c Split-Read Workspace

This creates an isolated workspace from the M16b fixture and adds a document space plus a collection space with `key`, `chronological`, `filter`, and `full_text` retrieval.

```bash
export M16C_SPLIT_WORK="$HARNESS_M16B_TEST_BASE/m16c-split-read-workspace"
rm -rf "$M16C_SPLIT_WORK"
cp -R "$M16B_WORK" "$M16C_SPLIT_WORK"

"$AGENTPM_MANUAL_PYTHON" - "$M16C_SPLIT_WORK" <<'PY'
import json
import sys
from pathlib import Path

work = Path(sys.argv[1])

memory_manifest = work / ".agentpm/memory/zack/m16b-native-turn-memory-package/0.1.0/agent.json"
memory = json.loads(memory_manifest.read_text())
memory["memory"]["spaces"] = {
    "current_note": {
        "description": "Current launch readiness note document for M16c split-read testing.",
        "model": "document",
        "record_types": ["note"],
        "scope": ["user"],
        "retrieval": {"modes": ["key"]},
    },
    "launch_readiness_notes": {
        "description": "Launch readiness note collection for M16c split-read testing.",
        "model": "collection",
        "record_types": ["note"],
        "scope": ["user"],
        "retrieval": {"modes": ["key", "chronological", "filter", "full_text", "semantic"]},
    },
}
memory_manifest.write_text(json.dumps(memory, indent=2) + "\n")

loop_manifest = work / ".agentpm/loops/zack/m16b-native-turn-loop/0.1.0/agent.json"
loop = json.loads(loop_manifest.read_text())
loop["loop"]["limits"]["max_steps"] = 16
loop["loop"]["phases"][0]["objective"] = "Use the requested M16c split-read Memory surfaces, then complete."
loop_manifest.write_text(json.dumps(loop, indent=2) + "\n")

agent_manifest = work / "agent.json"
agent = json.loads(agent_manifest.read_text())
agent["bindings"]["global"]["memory"][0]["spaces"] = ["current_note", "launch_readiness_notes"]
agent_manifest.write_text(json.dumps(agent, indent=2) + "\n")

for config_name, state_dir in [
    ("agentpm.m16b.openai.harness.json", ".agentpm-state-m16c-split-openai"),
    ("agentpm.m16b.anthropic.harness.json", ".agentpm-state-m16c-split-anthropic"),
]:
    config_path = work / config_name
    config = json.loads(config_path.read_text())
    config["runtime"]["state_dir"] = state_dir
    config["runtime"]["limits"]["max_steps"] = 16
    config["runtime"]["limits"]["max_model_calls_per_phase"] = 12
    config["runtime"]["limits"]["max_tool_calls_per_phase"] = 12
    config["runtime"]["limits"]["max_actions_per_phase"] = 20
    config["runtime"]["limits"]["max_tool_call_repairs"] = 3
    config_path.write_text(json.dumps(config, indent=2) + "\n")
PY

(cd "$M16C_SPLIT_WORK" && "$APM" lint agent.json >/dev/null)
(cd "$M16C_SPLIT_WORK" && "$APM" memory build \
  --manifest .agentpm/memory/zack/m16b-native-turn-memory-package/0.1.0/agent.json >/dev/null)
```

Add the M16c capture and assertion helpers:

```bash
cat > "$M16C_SPLIT_WORK/scripts/m16c_split_capture_server.py" <<'PY'
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
parser.add_argument("--scenario", choices=["schema", "split_sequence", "repair"], required=True)
args = parser.parse_args()

log_path = Path(args.log)
log_path.parent.mkdir(parents=True, exist_ok=True)
sequence = 0

def tools(body):
    if args.provider == "openai":
        return [tool.get("function") or {} for tool in body.get("tools", [])]
    return body.get("tools", [])

def alias_for(body, *needles):
    for tool in tools(body):
        name = tool.get("name") or ""
        if all(needle in name for needle in needles):
            return name
    raise RuntimeError(f"missing tool alias containing {needles}")

def first_record_id(body):
    def maybe_parse(value):
        if isinstance(value, str):
            try:
                return json.loads(value)
            except Exception:
                return None
        return value if isinstance(value, dict) else None

    if args.provider == "openai":
        for message in body.get("messages", []):
            if message.get("role") != "tool":
                continue
            parsed = maybe_parse(message.get("content"))
            if not parsed:
                continue
            if parsed.get("record_id"):
                return parsed["record_id"]
            record = parsed.get("record") or {}
            if record.get("id"):
                return record["id"]
    else:
        for message in body.get("messages", []):
            content = message.get("content")
            if not isinstance(content, list):
                continue
            for item in content:
                if item.get("type") != "tool_result":
                    continue
                parsed = maybe_parse(item.get("content"))
                if not parsed:
                    continue
                if parsed.get("record_id"):
                    return parsed["record_id"]
                record = parsed.get("record") or {}
                if record.get("id"):
                    return record["id"]
    raise RuntimeError("missing prior Memory write record_id")

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
        "id": "chatcmpl-m16c-capture",
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
        "id": "msg_m16c_capture",
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

def tool_response(body, alias, call_id, arguments):
    if args.provider == "openai":
        return openai_tool_response(alias, call_id, arguments)
    return anthropic_tool_response(alias, call_id, arguments)

def response_for(body):
    if args.scenario == "schema":
        return tool_response(
            body,
            alias_for(body, "phase_complete"),
            "call_m16c_schema_done",
            {"outcome": "done", "output": {"summary": "schema captured"}},
        )

    if args.scenario == "split_sequence":
        if sequence == 1:
            return tool_response(
                body,
                alias_for(body, "memory_write", "launch_readiness_notes"),
                "call_m16c_write_collection",
                {
                    "operation": "create",
                    "record_type": "note",
                    "content": {
                        "body": "Launch readiness is green for M16c split-read testing.",
                        "tag": "launch-readiness",
                    },
                },
            )
        if sequence == 2:
            return tool_response(
                body,
                alias_for(body, "memory_read", "launch_readiness_notes", "key_record"),
                "call_m16c_key_record",
                {"record_id": first_record_id(body), "record_type": "note"},
            )
        if sequence == 3:
            return tool_response(
                body,
                alias_for(body, "memory_write", "current_note"),
                "call_m16c_write_document",
                {
                    "operation": "create",
                    "record_type": "note",
                    "content": {
                        "body": "Current launch readiness note for M16c split-read testing.",
                        "tag": "current",
                    },
                },
            )
        if sequence == 4:
            return tool_response(
                body,
                alias_for(body, "memory_read", "current_note", "key_document"),
                "call_m16c_key_document",
                {"record_type": "note"},
            )
        if sequence == 5:
            return tool_response(
                body,
                alias_for(body, "memory_read", "launch_readiness_notes", "chronological"),
                "call_m16c_chronological",
                {"record_type": "note", "limit": 10},
            )
        if sequence == 6:
            return tool_response(
                body,
                alias_for(body, "memory_read", "launch_readiness_notes", "filter"),
                "call_m16c_filter",
                {"record_type": "note", "filter": {"tag": "launch-readiness"}, "limit": 10},
            )
        if sequence == 7:
            return tool_response(
                body,
                alias_for(body, "memory_read", "launch_readiness_notes", "full_text"),
                "call_m16c_full_text",
                {"record_type": "note", "query": "green", "limit": 10},
            )
        return tool_response(
            body,
            alias_for(body, "phase_complete"),
            "call_m16c_split_done",
            {"outcome": "done", "output": {"summary": "split sequence complete"}},
        )

    if args.scenario == "repair":
        if sequence == 1:
            return tool_response(
                body,
                alias_for(body, "memory_read", "launch_readiness_notes", "key_record"),
                "call_m16c_bad_key_record",
                {"mode": "key", "query": "launch readiness"},
            )
        if sequence == 2:
            return tool_response(
                body,
                alias_for(body, "memory_read", "launch_readiness_notes", "chronological"),
                "call_m16c_repaired_chronological",
                {"record_type": "note", "limit": 10},
            )
        return tool_response(
            body,
            alias_for(body, "phase_complete"),
            "call_m16c_repair_done",
            {"outcome": "done", "output": {"summary": "repair complete"}},
        )

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
chmod +x "$M16C_SPLIT_WORK/scripts/m16c_split_capture_server.py"

cat > "$M16C_SPLIT_WORK/scripts/m16c_assert_split_read.py" <<'PY'
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

def body_tools(body):
    if provider == "openai":
        return [
            {
                "name": (tool.get("function") or {}).get("name"),
                "description": (tool.get("function") or {}).get("description") or "",
                "schema": (tool.get("function") or {}).get("parameters") or {},
            }
            for tool in body.get("tools", [])
        ]
    return [
        {
            "name": tool.get("name"),
            "description": tool.get("description") or "",
            "schema": tool.get("input_schema") or {},
        }
        for tool in body.get("tools", [])
    ]

def find_tool(tools, *needles):
    for tool in tools:
        name = tool.get("name") or ""
        if all(needle in name for needle in needles):
            return tool
    fail(f"missing tool containing {needles}")

def assert_absent(schema, *keys):
    properties = schema.get("properties") or {}
    for key in keys:
        if key in properties:
            fail(f"unexpected `{key}` in schema {schema}")

def assert_required(schema, *keys):
    required = set(schema.get("required") or [])
    for key in keys:
        if key not in required:
            fail(f"expected required `{key}` in schema {schema}")

def assert_not_required(schema, *keys):
    required = set(schema.get("required") or [])
    for key in keys:
        if key in required:
            fail(f"did not expect required `{key}` in schema {schema}")

report = json.loads(report_path.read_text())
events = load_jsonl(Path(report["trace_path"]))
bodies = [row["body"] for row in load_jsonl(body_log_path)]
if not bodies:
    fail("capture server recorded no provider bodies")

first_tools = body_tools(bodies[0])
read_tools = [tool for tool in first_tools if (tool.get("name") or "").startswith("memory_read_")]
if not read_tools:
    fail("no provider-facing memory_read tools captured")

for tool in read_tools:
    assert_absent(tool["schema"], "mode")
    if "Modes:" in tool["description"]:
        fail(f"old canonical mode prose leaked into split-read description: {tool['name']}")
    if "For collection spaces, key requires record_id" in tool["description"]:
        fail(f"old key-mode prose leaked into split-read description: {tool['name']}")

key_document = find_tool(first_tools, "memory_read", "current_note", "key_document")
assert_absent(key_document["schema"], "record_id", "query", "filter", "mode")
assert_not_required(key_document["schema"], "record_id", "query", "filter", "mode")

key_record = find_tool(first_tools, "memory_read", "launch_readiness_notes", "key_record")
assert_required(key_record["schema"], "record_id")
assert_absent(key_record["schema"], "query", "filter", "mode")

chronological = find_tool(first_tools, "memory_read", "launch_readiness_notes", "chronological")
assert_absent(chronological["schema"], "record_id", "query", "filter", "mode")
assert_not_required(chronological["schema"], "record_id", "query", "filter", "mode")

filter_read = find_tool(first_tools, "memory_read", "launch_readiness_notes", "filter")
assert_required(filter_read["schema"], "filter")
assert_absent(filter_read["schema"], "record_id", "query", "mode")

full_text = find_tool(first_tools, "memory_read", "launch_readiness_notes", "full_text")
assert_required(full_text["schema"], "query")
assert_absent(full_text["schema"], "record_id", "filter", "mode")

if any("semantic" in (tool.get("name") or "") for tool in read_tools):
    fail("semantic read alias should be absent without a configured embedding provider")

if scenario == "schema":
    print(f"ok: {provider} split-read schemas are flat and shape-specific")
    sys.exit(0)

if scenario == "split_sequence":
    rejected = [event for event in events if event.get("event_type") == "semantic_action_rejected"]
    if rejected:
        fail(f"split sequence should not reject semantic actions: {rejected}")
    completed_reads = [
        ((event.get("payload") or {}).get("fields") or {})
        for event in events
        if event.get("event_type") == "memory_read_completed"
    ]
    seen = {(fields.get("space"), fields.get("mode")) for fields in completed_reads}
    expected = {
        ("launch_readiness_notes", "key"),
        ("current_note", "key"),
        ("launch_readiness_notes", "chronological"),
        ("launch_readiness_notes", "filter"),
        ("launch_readiness_notes", "full_text"),
    }
    missing = expected - seen
    if missing:
        fail(f"missing completed MemoryRead modes: {missing}; saw {seen}")
    print(f"ok: {provider} split-read sequence dispatched every shape")
    sys.exit(0)

if scenario == "repair":
    rejected = [event for event in events if event.get("event_type") == "semantic_action_rejected"]
    if len(rejected) != 1:
        fail(f"expected exactly one rejected read before repair, saw {len(rejected)}")
    error_text = json.dumps(rejected[0])
    if "Authorized Memory read shapes" not in error_text:
        fail("repair feedback did not name authorized Memory read shapes")
    completed_reads = [
        event for event in events
        if event.get("event_type") == "memory_read_completed"
    ]
    if len(completed_reads) != 1:
        fail(f"expected exactly one dispatched read after repair, saw {len(completed_reads)}")
    fields = (completed_reads[0].get("payload") or {}).get("fields") or {}
    if fields.get("mode") != "chronological":
        fail(f"expected repaired chronological read, saw {fields}")
    if (report.get("usage") or {}).get("memory_requests") != 1:
        fail(f"invalid rejected read should not count as a memory request: {report.get('usage')}")
    print(f"ok: {provider} invalid legacy read args repaired before dispatch")
    sys.exit(0)

fail(f"unknown scenario {scenario}")
PY
chmod +x "$M16C_SPLIT_WORK/scripts/m16c_assert_split_read.py"
```

Expected:

- lint and memory build pass;
- `M16C_SPLIT_WORK` points to the isolated split-read workspace;
- generated configs use fresh `.agentpm-state-m16c-split-*` state directories;
- no real provider calls happen yet.

### Test 10: OpenAI Capture, Split-Read Tool Schemas

This captures the first OpenAI request body and verifies each MemoryRead shape has a flat, provider-native schema.

```bash
mkdir -p "$M16B_RUNS/m16c-openai-schema"
rm -f "$M16B_RUNS/m16c-openai-schema/bodies.jsonl"
rm -rf "$M16C_SPLIT_WORK/.agentpm-state-m16c-split-openai"

"$AGENTPM_MANUAL_PYTHON" "$M16C_SPLIT_WORK/scripts/m16c_split_capture_server.py" \
  --provider openai \
  --scenario schema \
  --port 18084 \
  --log "$M16B_RUNS/m16c-openai-schema/bodies.jsonl" \
  >"$M16B_RUNS/m16c-openai-schema/server.stdout.txt" \
  2>"$M16B_RUNS/m16c-openai-schema/server.stderr.txt" &
SERVER_PID=$!
trap 'kill "$SERVER_PID" 2>/dev/null || true' EXIT
until curl -fsS http://127.0.0.1:18084/health >/dev/null 2>&1; do sleep 0.1; done

export OPENAI_API_KEY="m16c-capture-key"
export OPENAI_BASE_URL="http://127.0.0.1:18084/v1/chat/completions"

REPORT="$M16B_RUNS/m16c-openai-schema/report.json"
(cd "$M16C_SPLIT_WORK" && "$APM" harness \
  --config agentpm.m16b.openai.harness.json \
  --headless \
  --scope user=m16c-openai-schema \
  --input "Capture the M16c split-read OpenAI tool schemas, then complete." \
  --report "$REPORT" \
  >"$M16B_RUNS/m16c-openai-schema/stdout.txt" \
  2>"$M16B_RUNS/m16c-openai-schema/stderr.txt")

kill "$SERVER_PID" 2>/dev/null || true
trap - EXIT

"$AGENTPM_MANUAL_PYTHON" "$M16C_SPLIT_WORK/scripts/m16c_assert_split_read.py" \
  "$REPORT" \
  "$M16B_RUNS/m16c-openai-schema/bodies.jsonl" \
  openai \
  schema \
  | tee "$M16B_RUNS/m16c-openai-schema/assertion.txt"
```

Expected:

- captured OpenAI `tools[*].function.name` include `memory_read_*_key_document_*`, `memory_read_*_key_record_*`, `memory_read_*_chronological_*`, `memory_read_*_filter_*`, and `memory_read_*_full_text_*`;
- no MemoryRead schema exposes `mode`;
- `key_record` requires `record_id`;
- `key_document` does not expose `record_id`;
- `filter` requires `filter` and does not expose `query`;
- `full_text` requires `query` and does not expose `filter`;
- no `semantic` MemoryRead alias appears because this fixture has no embedding provider.

Useful body inspection:

```bash
"$AGENTPM_MANUAL_PYTHON" - "$M16B_RUNS/m16c-openai-schema/bodies.jsonl" <<'PY'
import json, sys
from pathlib import Path
body = json.loads(Path(sys.argv[1]).read_text().splitlines()[0])["body"]
for tool in body.get("tools", []):
    fn = tool["function"]
    if fn["name"].startswith("memory_read_"):
        print("\n" + fn["name"])
        print(fn["description"])
        print(json.dumps(fn["parameters"], indent=2))
PY
```

### Test 11: Anthropic Capture, Split-Read Tool Schemas

This is the same schema check through the Anthropic serializer.

```bash
mkdir -p "$M16B_RUNS/m16c-anthropic-schema"
rm -f "$M16B_RUNS/m16c-anthropic-schema/bodies.jsonl"
rm -rf "$M16C_SPLIT_WORK/.agentpm-state-m16c-split-anthropic"

"$AGENTPM_MANUAL_PYTHON" "$M16C_SPLIT_WORK/scripts/m16c_split_capture_server.py" \
  --provider anthropic \
  --scenario schema \
  --port 18085 \
  --log "$M16B_RUNS/m16c-anthropic-schema/bodies.jsonl" \
  >"$M16B_RUNS/m16c-anthropic-schema/server.stdout.txt" \
  2>"$M16B_RUNS/m16c-anthropic-schema/server.stderr.txt" &
SERVER_PID=$!
trap 'kill "$SERVER_PID" 2>/dev/null || true' EXIT
until curl -fsS http://127.0.0.1:18085/health >/dev/null 2>&1; do sleep 0.1; done

export ANTHROPIC_API_KEY="m16c-capture-key"
export ANTHROPIC_BASE_URL="http://127.0.0.1:18085/v1/messages"

REPORT="$M16B_RUNS/m16c-anthropic-schema/report.json"
(cd "$M16C_SPLIT_WORK" && "$APM" harness \
  --config agentpm.m16b.anthropic.harness.json \
  --headless \
  --scope user=m16c-anthropic-schema \
  --input "Capture the M16c split-read Anthropic tool schemas, then complete." \
  --report "$REPORT" \
  >"$M16B_RUNS/m16c-anthropic-schema/stdout.txt" \
  2>"$M16B_RUNS/m16c-anthropic-schema/stderr.txt")

kill "$SERVER_PID" 2>/dev/null || true
trap - EXIT

"$AGENTPM_MANUAL_PYTHON" "$M16C_SPLIT_WORK/scripts/m16c_assert_split_read.py" \
  "$REPORT" \
  "$M16B_RUNS/m16c-anthropic-schema/bodies.jsonl" \
  anthropic \
  schema \
  | tee "$M16B_RUNS/m16c-anthropic-schema/assertion.txt"
```

Expected:

- captured Anthropic `tools[*].name` expose the same split MemoryRead aliases as OpenAI;
- captured Anthropic `tools[*].input_schema` has the same flat schema properties;
- no old bundled `mode` enum is present.

Useful body inspection:

```bash
"$AGENTPM_MANUAL_PYTHON" - "$M16B_RUNS/m16c-anthropic-schema/bodies.jsonl" <<'PY'
import json, sys
from pathlib import Path
body = json.loads(Path(sys.argv[1]).read_text().splitlines()[0])["body"]
for tool in body.get("tools", []):
    if tool["name"].startswith("memory_read_"):
        print("\n" + tool["name"])
        print(tool["description"])
        print(json.dumps(tool["input_schema"], indent=2))
PY
```

### Test 12: OpenAI Capture, Every Split-Read Shape Dispatches

This deterministic capture run writes one collection record, reads it by collection key, writes one document record, reads the document by key, then exercises chronological, filter, and full_text reads. It verifies that valid split-shape calls dispatch without repair.

```bash
mkdir -p "$M16B_RUNS/m16c-openai-split-sequence"
rm -f "$M16B_RUNS/m16c-openai-split-sequence/bodies.jsonl"
rm -rf "$M16C_SPLIT_WORK/.agentpm-state-m16c-split-openai"

"$AGENTPM_MANUAL_PYTHON" "$M16C_SPLIT_WORK/scripts/m16c_split_capture_server.py" \
  --provider openai \
  --scenario split_sequence \
  --port 18086 \
  --log "$M16B_RUNS/m16c-openai-split-sequence/bodies.jsonl" \
  >"$M16B_RUNS/m16c-openai-split-sequence/server.stdout.txt" \
  2>"$M16B_RUNS/m16c-openai-split-sequence/server.stderr.txt" &
SERVER_PID=$!
trap 'kill "$SERVER_PID" 2>/dev/null || true' EXIT
until curl -fsS http://127.0.0.1:18086/health >/dev/null 2>&1; do sleep 0.1; done

export OPENAI_API_KEY="m16c-capture-key"
export OPENAI_BASE_URL="http://127.0.0.1:18086/v1/chat/completions"

REPORT="$M16B_RUNS/m16c-openai-split-sequence/report.json"
(cd "$M16C_SPLIT_WORK" && "$APM" harness \
  --config agentpm.m16b.openai.harness.json \
  --headless \
  --scope user=m16c-openai-split-sequence \
  --input "Run the M16c split-read sequence." \
  --report "$REPORT" \
  >"$M16B_RUNS/m16c-openai-split-sequence/stdout.txt" \
  2>"$M16B_RUNS/m16c-openai-split-sequence/stderr.txt")

kill "$SERVER_PID" 2>/dev/null || true
trap - EXIT

"$AGENTPM_MANUAL_PYTHON" "$M16C_SPLIT_WORK/scripts/m16c_assert_split_read.py" \
  "$REPORT" \
  "$M16B_RUNS/m16c-openai-split-sequence/bodies.jsonl" \
  openai \
  split_sequence \
  | tee "$M16B_RUNS/m16c-openai-split-sequence/assertion.txt"
```

Expected:

- run ends `ended`;
- trace has no `semantic_action_rejected`;
- trace has completed Memory reads for:
  - `current_note` with mode `key`;
  - `launch_readiness_notes` with mode `key`;
  - `launch_readiness_notes` with mode `chronological`;
  - `launch_readiness_notes` with mode `filter`;
  - `launch_readiness_notes` with mode `full_text`;
- `usage.memory_requests` counts only dispatched Memory reads/writes, not provider tool-schema inspection.

Useful trace inspection:

```bash
"$AGENTPM_MANUAL_PYTHON" - "$REPORT" <<'PY'
import json, sys
from pathlib import Path
report = json.loads(Path(sys.argv[1]).read_text())
events = [json.loads(line) for line in Path(report["trace_path"]).read_text().splitlines() if line.strip()]
for event in events:
    if event.get("event_type") in {"semantic_action_proposed", "memory_read_started", "memory_read_completed"}:
        payload = event.get("payload") or {}
        print(event["event_type"], payload.get("identity"), json.dumps(payload.get("fields") or {}, sort_keys=True))
PY
```

### Test 13: OpenAI Capture, Legacy Read Arguments Repair Before Dispatch

This intentionally sends an invalid collection key-read shape with legacy bundled arguments. Harness should reject it before Memory dispatch, ask for repair, accept a chronological read, and count only the repaired Memory read as a Memory request.

```bash
mkdir -p "$M16B_RUNS/m16c-openai-repair"
rm -f "$M16B_RUNS/m16c-openai-repair/bodies.jsonl"
rm -rf "$M16C_SPLIT_WORK/.agentpm-state-m16c-split-openai"

"$AGENTPM_MANUAL_PYTHON" "$M16C_SPLIT_WORK/scripts/m16c_split_capture_server.py" \
  --provider openai \
  --scenario repair \
  --port 18087 \
  --log "$M16B_RUNS/m16c-openai-repair/bodies.jsonl" \
  >"$M16B_RUNS/m16c-openai-repair/server.stdout.txt" \
  2>"$M16B_RUNS/m16c-openai-repair/server.stderr.txt" &
SERVER_PID=$!
trap 'kill "$SERVER_PID" 2>/dev/null || true' EXIT
until curl -fsS http://127.0.0.1:18087/health >/dev/null 2>&1; do sleep 0.1; done

export OPENAI_API_KEY="m16c-capture-key"
export OPENAI_BASE_URL="http://127.0.0.1:18087/v1/chat/completions"

REPORT="$M16B_RUNS/m16c-openai-repair/report.json"
(cd "$M16C_SPLIT_WORK" && "$APM" harness \
  --config agentpm.m16b.openai.harness.json \
  --headless \
  --scope user=m16c-openai-repair \
  --input "Run the M16c split-read repair scenario." \
  --report "$REPORT" \
  >"$M16B_RUNS/m16c-openai-repair/stdout.txt" \
  2>"$M16B_RUNS/m16c-openai-repair/stderr.txt")

kill "$SERVER_PID" 2>/dev/null || true
trap - EXIT

"$AGENTPM_MANUAL_PYTHON" "$M16C_SPLIT_WORK/scripts/m16c_assert_split_read.py" \
  "$REPORT" \
  "$M16B_RUNS/m16c-openai-repair/bodies.jsonl" \
  openai \
  repair \
  | tee "$M16B_RUNS/m16c-openai-repair/assertion.txt"
```

Expected:

- trace has exactly one `semantic_action_rejected`;
- rejection text contains `Authorized Memory read shapes`;
- no `memory_read_started` event exists for the invalid read;
- the repaired chronological read dispatches and completes;
- `usage.memory_requests == 1`.

Useful repair inspection:

```bash
"$AGENTPM_MANUAL_PYTHON" - "$REPORT" <<'PY'
import json, sys
from pathlib import Path
report = json.loads(Path(sys.argv[1]).read_text())
events = [json.loads(line) for line in Path(report["trace_path"]).read_text().splitlines() if line.strip()]
for event in events:
    if event.get("event_type") in {"semantic_action_rejected", "model_repair_requested", "memory_read_started", "memory_read_completed"}:
        print(json.dumps(event, indent=2))
PY
```

### Test 14: Anthropic Capture, Every Split-Read Shape Dispatches

This repeats Test 12 through Anthropic so both built-in provider serializers are covered beyond schema inspection.

```bash
mkdir -p "$M16B_RUNS/m16c-anthropic-split-sequence"
rm -f "$M16B_RUNS/m16c-anthropic-split-sequence/bodies.jsonl"
rm -rf "$M16C_SPLIT_WORK/.agentpm-state-m16c-split-anthropic"

"$AGENTPM_MANUAL_PYTHON" "$M16C_SPLIT_WORK/scripts/m16c_split_capture_server.py" \
  --provider anthropic \
  --scenario split_sequence \
  --port 18088 \
  --log "$M16B_RUNS/m16c-anthropic-split-sequence/bodies.jsonl" \
  >"$M16B_RUNS/m16c-anthropic-split-sequence/server.stdout.txt" \
  2>"$M16B_RUNS/m16c-anthropic-split-sequence/server.stderr.txt" &
SERVER_PID=$!
trap 'kill "$SERVER_PID" 2>/dev/null || true' EXIT
until curl -fsS http://127.0.0.1:18088/health >/dev/null 2>&1; do sleep 0.1; done

export ANTHROPIC_API_KEY="m16c-capture-key"
export ANTHROPIC_BASE_URL="http://127.0.0.1:18088/v1/messages"

REPORT="$M16B_RUNS/m16c-anthropic-split-sequence/report.json"
(cd "$M16C_SPLIT_WORK" && "$APM" harness \
  --config agentpm.m16b.anthropic.harness.json \
  --headless \
  --scope user=m16c-anthropic-split-sequence \
  --input "Run the M16c split-read sequence." \
  --report "$REPORT" \
  >"$M16B_RUNS/m16c-anthropic-split-sequence/stdout.txt" \
  2>"$M16B_RUNS/m16c-anthropic-split-sequence/stderr.txt")

kill "$SERVER_PID" 2>/dev/null || true
trap - EXIT

"$AGENTPM_MANUAL_PYTHON" "$M16C_SPLIT_WORK/scripts/m16c_assert_split_read.py" \
  "$REPORT" \
  "$M16B_RUNS/m16c-anthropic-split-sequence/bodies.jsonl" \
  anthropic \
  split_sequence \
  | tee "$M16B_RUNS/m16c-anthropic-split-sequence/assertion.txt"
```

Expected:

- same as Test 12;
- Anthropic `tool_use` and `tool_result` turns remain correlated across the longer MemoryRead sequence.

### Test 15: Anthropic Capture, Legacy Read Arguments Repair Before Dispatch

This repeats Test 13 through Anthropic.

```bash
mkdir -p "$M16B_RUNS/m16c-anthropic-repair"
rm -f "$M16B_RUNS/m16c-anthropic-repair/bodies.jsonl"
rm -rf "$M16C_SPLIT_WORK/.agentpm-state-m16c-split-anthropic"

"$AGENTPM_MANUAL_PYTHON" "$M16C_SPLIT_WORK/scripts/m16c_split_capture_server.py" \
  --provider anthropic \
  --scenario repair \
  --port 18089 \
  --log "$M16B_RUNS/m16c-anthropic-repair/bodies.jsonl" \
  >"$M16B_RUNS/m16c-anthropic-repair/server.stdout.txt" \
  2>"$M16B_RUNS/m16c-anthropic-repair/server.stderr.txt" &
SERVER_PID=$!
trap 'kill "$SERVER_PID" 2>/dev/null || true' EXIT
until curl -fsS http://127.0.0.1:18089/health >/dev/null 2>&1; do sleep 0.1; done

export ANTHROPIC_API_KEY="m16c-capture-key"
export ANTHROPIC_BASE_URL="http://127.0.0.1:18089/v1/messages"

REPORT="$M16B_RUNS/m16c-anthropic-repair/report.json"
(cd "$M16C_SPLIT_WORK" && "$APM" harness \
  --config agentpm.m16b.anthropic.harness.json \
  --headless \
  --scope user=m16c-anthropic-repair \
  --input "Run the M16c split-read repair scenario." \
  --report "$REPORT" \
  >"$M16B_RUNS/m16c-anthropic-repair/stdout.txt" \
  2>"$M16B_RUNS/m16c-anthropic-repair/stderr.txt")

kill "$SERVER_PID" 2>/dev/null || true
trap - EXIT

"$AGENTPM_MANUAL_PYTHON" "$M16C_SPLIT_WORK/scripts/m16c_assert_split_read.py" \
  "$REPORT" \
  "$M16B_RUNS/m16c-anthropic-repair/bodies.jsonl" \
  anthropic \
  repair \
  | tee "$M16B_RUNS/m16c-anthropic-repair/assertion.txt"
```

Expected:

- same as Test 13;
- Anthropic receives repair feedback as provider-native turn content, and the invalid read is never dispatched.

### Test 16: Optional Live OpenAI, Zero-Result Read With Split Aliases

This revisits the OpenAI failure mode that motivated M16c: an objective prompt over an empty collection should now expose separate list/filter/full_text read actions instead of one bundled `mode` enum.

```bash
mkdir -p "$M16B_RUNS/m16c-live-openai-zero-read"
rm -rf "$M16C_SPLIT_WORK/.agentpm-state-m16c-split-openai"
unset OPENAI_BASE_URL
test -n "${OPENAI_API_KEY:-}"

REPORT="$M16B_RUNS/m16c-live-openai-zero-read/report.json"
(cd "$M16C_SPLIT_WORK" && "$APM" harness \
  --config agentpm.m16b.openai.harness.json \
  --headless \
  --scope user="m16c-live-openai-zero-$(date +%s)" \
  --input "Determine whether any launch readiness notes are already remembered for this user." \
  --report "$REPORT" \
  >"$M16B_RUNS/m16c-live-openai-zero-read/stdout.txt" \
  2>"$M16B_RUNS/m16c-live-openai-zero-read/stderr.txt")

"$AGENTPM_MANUAL_PYTHON" "$M16B_WORK/scripts/m16b_measure_repeats.py" "$REPORT" \
  | tee "$M16B_RUNS/m16c-live-openai-zero-read/repeat-summary.txt"
```

Record:

- whether the run ends or fails;
- whether any `semantic_action_rejected` events remain;
- which split MemoryRead alias the model chose;
- whether a `memory_read_completed` result with `count: 0` is treated as successful/no-match;
- whether exact or changed-argument MemoryRead repeats occur.

### Test 17: Optional Live Anthropic, Zero-Result Read With Split Aliases

This is the same live zero-result read measurement through Anthropic.

```bash
mkdir -p "$M16B_RUNS/m16c-live-anthropic-zero-read"
rm -rf "$M16C_SPLIT_WORK/.agentpm-state-m16c-split-anthropic"
unset ANTHROPIC_BASE_URL
test -n "${ANTHROPIC_API_KEY:-}"

REPORT="$M16B_RUNS/m16c-live-anthropic-zero-read/report.json"
(cd "$M16C_SPLIT_WORK" && "$APM" harness \
  --config agentpm.m16b.anthropic.harness.json \
  --headless \
  --scope user="m16c-live-anthropic-zero-$(date +%s)" \
  --input "Determine whether any launch readiness notes are already remembered for this user." \
  --report "$REPORT" \
  >"$M16B_RUNS/m16c-live-anthropic-zero-read/stdout.txt" \
  2>"$M16B_RUNS/m16c-live-anthropic-zero-read/stderr.txt")

"$AGENTPM_MANUAL_PYTHON" "$M16B_WORK/scripts/m16b_measure_repeats.py" "$REPORT" \
  | tee "$M16B_RUNS/m16c-live-anthropic-zero-read/repeat-summary.txt"
```

Record:

- same observations as Test 16;
- compare chosen MemoryRead shape and repair behavior with OpenAI.

### Test 18: Optional Live OpenAI, Objective Prompt With Similar Read Choices

This checks whether the split aliases help the model choose between document key read, collection list read, filter read, full_text read, Tool use, Memory write, and PhaseCompletion without explicit sequencing.

```bash
mkdir -p "$M16B_RUNS/m16c-live-openai-objective"
rm -rf "$M16C_SPLIT_WORK/.agentpm-state-m16c-split-openai"
unset OPENAI_BASE_URL
test -n "${OPENAI_API_KEY:-}"

REPORT="$M16B_RUNS/m16c-live-openai-objective/report.json"
(cd "$M16C_SPLIT_WORK" && "$APM" harness \
  --config agentpm.m16b.openai.harness.json \
  --headless \
  --scope user="m16c-live-openai-objective-$(date +%s)" \
  --input "Find out whether the launch is ready, and record anything worth remembering." \
  --report "$REPORT" \
  >"$M16B_RUNS/m16c-live-openai-objective/stdout.txt" \
  2>"$M16B_RUNS/m16c-live-openai-objective/stderr.txt")

"$AGENTPM_MANUAL_PYTHON" "$M16B_WORK/scripts/m16b_measure_repeats.py" "$REPORT" \
  | tee "$M16B_RUNS/m16c-live-openai-objective/repeat-summary.txt"
```

Record:

- whether the model uses a MemoryRead shape before writing;
- whether it uses the Tool, Memory, both, or neither;
- whether it repeats a semantically similar action with changed arguments;
- whether completion happens cleanly without explicit completion wording.

### Test 19: Optional Live Anthropic, Objective Prompt With Similar Read Choices

This is the same objective prompt through Anthropic.

```bash
mkdir -p "$M16B_RUNS/m16c-live-anthropic-objective"
rm -rf "$M16C_SPLIT_WORK/.agentpm-state-m16c-split-anthropic"
unset ANTHROPIC_BASE_URL
test -n "${ANTHROPIC_API_KEY:-}"

REPORT="$M16B_RUNS/m16c-live-anthropic-objective/report.json"
(cd "$M16C_SPLIT_WORK" && "$APM" harness \
  --config agentpm.m16b.anthropic.harness.json \
  --headless \
  --scope user="m16c-live-anthropic-objective-$(date +%s)" \
  --input "Find out whether the launch is ready, and record anything worth remembering." \
  --report "$REPORT" \
  >"$M16B_RUNS/m16c-live-anthropic-objective/stdout.txt" \
  2>"$M16B_RUNS/m16c-live-anthropic-objective/stderr.txt")

"$AGENTPM_MANUAL_PYTHON" "$M16B_WORK/scripts/m16b_measure_repeats.py" "$REPORT" \
  | tee "$M16B_RUNS/m16c-live-anthropic-objective/repeat-summary.txt"
```

Record:

- same observations as Test 18;
- compare chosen read/write/tool path with OpenAI.

### Test 20: Create The M16c Similar-Surface Multi-Phase Workspace

This replaces the split-read fixture's Memory spaces with two long, similar collection spaces and adds a second phase. It is the M16c version of the Test 8 failure shape: similar collection surfaces plus a phase boundary, with split MemoryRead aliases active.

If you deleted `harness-m16b-test` and want the shortest path back to Test 20, do this first:

1. Run the top-level prerequisites and Setup block to recreate `M16B_WORK`.
2. Run Test 9's workspace setup block and the helper block that writes `m16c_split_capture_server.py` and `m16c_assert_split_read.py`. You do not need to run Tests 10-19.
3. Run Test 20 below, then continue with Tests 21-23.

```bash
export M16C_SPLIT_MULTIPHASE_WORK="$HARNESS_M16B_TEST_BASE/m16c-split-read-multiphase-workspace"
rm -rf "$M16C_SPLIT_MULTIPHASE_WORK"
cp -R "$M16C_SPLIT_WORK" "$M16C_SPLIT_MULTIPHASE_WORK"

"$AGENTPM_MANUAL_PYTHON" - "$M16C_SPLIT_MULTIPHASE_WORK" <<'PY'
import json
import sys
from pathlib import Path

work = Path(sys.argv[1])

memory_manifest = work / ".agentpm/memory/zack/m16b-native-turn-memory-package/0.1.0/agent.json"
memory = json.loads(memory_manifest.read_text())
memory["memory"]["spaces"] = {
    "launch_readiness_notes_with_native_turn_history_for_current_release": {
        "description": "Launch readiness note collection with a long similar name for M16c multi-phase target-selection testing.",
        "model": "collection",
        "record_types": ["note"],
        "scope": ["user"],
        "retrieval": {"modes": ["key", "chronological", "filter"]},
    },
    "launch_readiness_notes_with_native_turn_history_for_followup_review": {
        "description": "Launch readiness note collection with a long similar name for M16c multi-phase target-selection testing.",
        "model": "collection",
        "record_types": ["note"],
        "scope": ["user"],
        "retrieval": {"modes": ["key", "chronological", "filter"]},
    },
}
memory_manifest.write_text(json.dumps(memory, indent=2) + "\n")

loop_manifest = work / ".agentpm/loops/zack/m16b-native-turn-loop/0.1.0/agent.json"
loop = json.loads(loop_manifest.read_text())
loop["loop"]["limits"]["max_steps"] = 20
exercise = loop["loop"]["phases"][0]
exercise["objective"] = "Use the requested M16c split-read Memory and Tool surfaces, then choose whether another phase is needed."
if not any(outcome.get("id") == "followup" for outcome in exercise["outcomes"]):
    exercise["outcomes"].append({
        "id": "followup",
        "description": "Continue into a second phase for similar-surface multi-phase measurement.",
    })
if not any(phase.get("id") == "followup" for phase in loop["loop"]["phases"]):
    loop["loop"]["phases"].append({
        "id": "followup",
        "objective": "Use the requested M16c split-read Memory and Tool surfaces in the second phase, then complete.",
        "access": {
            "tools": True,
            "memory": {"read": True, "write": True},
        },
        "outcomes": [
            {"id": "done", "description": "The second phase is complete."},
        ],
    })
if not any(t.get("from") == "exercise" and t.get("on") == "followup" for t in loop["loop"]["transitions"]):
    loop["loop"]["transitions"].append({
        "from": "exercise",
        "on": "followup",
        "to": "followup",
    })
if not any(t.get("from") == "followup" and t.get("on") == "done" for t in loop["loop"]["transitions"]):
    loop["loop"]["transitions"].append({
        "from": "followup",
        "on": "done",
        "to": "$end",
    })
loop_manifest.write_text(json.dumps(loop, indent=2) + "\n")

agent_manifest = work / "agent.json"
agent = json.loads(agent_manifest.read_text())
agent["bindings"]["global"]["memory"][0]["spaces"] = [
    "launch_readiness_notes_with_native_turn_history_for_current_release",
    "launch_readiness_notes_with_native_turn_history_for_followup_review",
]
agent["bindings"]["phases"]["followup"] = {
    "tools": ["@zack/m16b-native-turn-search-tool-with-long-readable-name"],
}
agent_manifest.write_text(json.dumps(agent, indent=2) + "\n")

for config_name, state_dir in [
    ("agentpm.m16b.openai.harness.json", ".agentpm-state-m16c-split-multiphase-openai"),
    ("agentpm.m16b.anthropic.harness.json", ".agentpm-state-m16c-split-multiphase-anthropic"),
]:
    config_path = work / config_name
    config = json.loads(config_path.read_text())
    config["runtime"]["state_dir"] = state_dir
    config["runtime"]["limits"]["max_steps"] = 20
    config["runtime"]["limits"]["max_model_calls_per_phase"] = 12
    config["runtime"]["limits"]["max_tool_calls_per_phase"] = 12
    config["runtime"]["limits"]["max_actions_per_phase"] = 24
    config["runtime"]["limits"]["max_tool_call_repairs"] = 3
    config["runtime"]["limits"]["max_structured_output_repairs"] = 3
    config_path.write_text(json.dumps(config, indent=2) + "\n")
PY

(cd "$M16C_SPLIT_MULTIPHASE_WORK" && "$APM" lint agent.json >/dev/null)
(cd "$M16C_SPLIT_MULTIPHASE_WORK" && "$APM" memory build \
  --manifest .agentpm/memory/zack/m16b-native-turn-memory-package/0.1.0/agent.json >/dev/null)
```

Expected:

- lint and memory build pass;
- `M16C_SPLIT_MULTIPHASE_WORK` points to the isolated multi-phase workspace;
- the workspace has two long, similar collection spaces:
  - `launch_readiness_notes_with_native_turn_history_for_current_release`;
  - `launch_readiness_notes_with_native_turn_history_for_followup_review`;
- both collections declare only `key`, `chronological`, and `filter`;
- both phases expose the same Tool and Memory surfaces;
- OpenAI/Anthropic configs use fresh `.agentpm-state-m16c-split-multiphase-*` state directories.

### Test 21: Optional Live OpenAI And Anthropic, Similar Surfaces Plus Multi-Phase

Run this after Test 20. This is the strongest M16c live measurement because it revisits the M16b failure shape with the split MemoryRead aliases in place.

#### OpenAI

```bash
mkdir -p "$M16B_RUNS/m16c-live-openai-similar-multiphase"
rm -rf "$M16C_SPLIT_MULTIPHASE_WORK/.agentpm-state-m16c-split-multiphase-openai"
unset OPENAI_BASE_URL
test -n "${OPENAI_API_KEY:-}"

REPORT="$M16B_RUNS/m16c-live-openai-similar-multiphase/report.json"
(cd "$M16C_SPLIT_MULTIPHASE_WORK" && "$APM" harness \
  --config agentpm.m16b.openai.harness.json \
  --headless \
  --scope user="m16c-live-openai-similar-multiphase-$(date +%s)" \
  --input "Find out whether the launch is ready and record anything worth remembering. Use a followup phase if another pass is needed to verify the result." \
  --report "$REPORT" \
  >"$M16B_RUNS/m16c-live-openai-similar-multiphase/stdout.txt" \
  2>"$M16B_RUNS/m16c-live-openai-similar-multiphase/stderr.txt")

"$AGENTPM_MANUAL_PYTHON" "$M16B_WORK/scripts/m16b_measure_repeats.py" "$REPORT" \
  | tee "$M16B_RUNS/m16c-live-openai-similar-multiphase/repeat-summary.txt"
```

#### Anthropic

```bash
mkdir -p "$M16B_RUNS/m16c-live-anthropic-similar-multiphase"
rm -rf "$M16C_SPLIT_MULTIPHASE_WORK/.agentpm-state-m16c-split-multiphase-anthropic"
unset ANTHROPIC_BASE_URL
test -n "${ANTHROPIC_API_KEY:-}"

REPORT="$M16B_RUNS/m16c-live-anthropic-similar-multiphase/report.json"
(cd "$M16C_SPLIT_MULTIPHASE_WORK" && "$APM" harness \
  --config agentpm.m16b.anthropic.harness.json \
  --headless \
  --scope user="m16c-live-anthropic-similar-multiphase-$(date +%s)" \
  --input "Find out whether the launch is ready and record anything worth remembering. Use a followup phase if another pass is needed to verify the result." \
  --report "$REPORT" \
  >"$M16B_RUNS/m16c-live-anthropic-similar-multiphase/stdout.txt" \
  2>"$M16B_RUNS/m16c-live-anthropic-similar-multiphase/stderr.txt")

"$AGENTPM_MANUAL_PYTHON" "$M16B_WORK/scripts/m16b_measure_repeats.py" "$REPORT" \
  | tee "$M16B_RUNS/m16c-live-anthropic-similar-multiphase/repeat-summary.txt"
```

Record:

- whether either provider fails or exhausts repair;
- whether OpenAI still proposes an invalid collection key read;
- whether any `semantic_action_rejected` events appear;
- whether the model selects the `current_release` collection, the `followup_review` collection, the Tool, or some combination;
- whether the visible alias prefixes are still readable enough despite the two long similar collection names;
- whether the model enters `followup`;
- if it enters `followup`, whether actions repeat across the phase boundary;
- exact repeats and changed-argument repeats from `repeat-summary.txt`;
- whether completion repair is still needed.

### Test 22: Capture OpenAI M16c MemoryWrite And Filter Schemas

Run this after Test 20. This deterministic capture checks the first OpenAI provider body for the M16c provider-facing Memory schemas:

- `memory_write_*_create_or_upsert_note_*`
- `memory_write_*_update_note_*`
- `memory_write_*_delete_or_archive_*`
- `memory_read_*_filter_note_*`

It also prints the captured function names so you can inspect how alias truncation looks with the two long similar spaces.

```bash
cat > "$M16C_SPLIT_MULTIPHASE_WORK/scripts/m16c_assert_write_filter_schema.py" <<'PY'
#!/usr/bin/env python3
import json
import sys
from pathlib import Path

body_log_path = Path(sys.argv[1])
provider = sys.argv[2]

SPACES = [
    "launch_readiness_notes_with_native_turn_history_for_current_release",
    "launch_readiness_notes_with_native_turn_history_for_followup_review",
]

def fail(message):
    raise AssertionError(message)

def load_jsonl(path):
    return [json.loads(line) for line in path.read_text().splitlines() if line.strip()]

def body_tools(body):
    if provider == "openai":
        return [
            {
                "name": (tool.get("function") or {}).get("name") or "",
                "description": (tool.get("function") or {}).get("description") or "",
                "schema": (tool.get("function") or {}).get("parameters") or {},
            }
            for tool in body.get("tools", [])
        ]
    return [
        {
            "name": tool.get("name") or "",
            "description": tool.get("description") or "",
            "schema": tool.get("input_schema") or {},
        }
        for tool in body.get("tools", [])
    ]

def find_tool(tools, space, kind, shape_text):
    matches = [
        tool for tool in tools
        if tool["name"].startswith(kind)
        and f"space `{space}`" in tool["description"]
        and shape_text in tool["description"]
    ]
    if len(matches) != 1:
        names = [tool["name"] for tool in matches]
        fail(f"expected one {kind} tool for {space} / {shape_text}, saw {len(matches)}: {names}")
    return matches[0]

def require_props(schema, *keys):
    properties = schema.get("properties") or {}
    for key in keys:
        if key not in properties:
            fail(f"missing property `{key}` in schema {schema}")
    return properties

def forbid_props(schema, *keys):
    properties = schema.get("properties") or {}
    for key in keys:
        if key in properties:
            fail(f"unexpected property `{key}` in schema {schema}")

def require_required(schema, *keys):
    required = set(schema.get("required") or [])
    for key in keys:
        if key not in required:
            fail(f"missing required `{key}` in schema {schema}")

bodies = [row["body"] for row in load_jsonl(body_log_path)]
if not bodies:
    fail("capture server recorded no provider bodies")

tools = body_tools(bodies[0])
printed = []

for space in SPACES:
    create = find_tool(tools, space, "memory_write_", "Fixed write shape: create/upsert")
    if "create_or_upsert_note" not in create["name"]:
        fail(f"create/upsert alias should name the write shape: {create['name']}")
    create_props = require_props(create["schema"], "operation", "content", "record_type")
    require_required(create["schema"], "operation", "content")
    forbid_props(create["schema"], "record_id")
    if create_props["operation"].get("enum") != ["create", "upsert"]:
        fail(f"create/upsert operation enum is wrong: {create_props['operation']}")
    if create_props["record_type"].get("const") != "note":
        fail(f"create/upsert record_type should be fixed to note: {create_props['record_type']}")
    printed.append(create["name"])

    update = find_tool(tools, space, "memory_write_", "Fixed write shape: update")
    if "update_note" not in update["name"]:
        fail(f"update alias should name the write shape: {update['name']}")
    update_props = require_props(update["schema"], "operation", "record_id", "content", "record_type")
    require_required(update["schema"], "operation", "record_id", "content")
    if update_props["operation"].get("const") != "update":
        fail(f"update operation should be fixed: {update_props['operation']}")
    if update_props["record_type"].get("const") != "note":
        fail(f"update record_type should be fixed to note: {update_props['record_type']}")
    printed.append(update["name"])

    delete = find_tool(tools, space, "memory_write_", "Fixed write shape: delete/archive")
    if "delete_or_archive" not in delete["name"]:
        fail(f"delete/archive alias should name the write shape: {delete['name']}")
    delete_props = require_props(delete["schema"], "operation", "record_id", "record_type")
    require_required(delete["schema"], "operation", "record_id", "record_type")
    forbid_props(delete["schema"], "content")
    if delete_props["operation"].get("enum") != ["delete", "archive"]:
        fail(f"delete/archive operation enum is wrong: {delete_props['operation']}")
    printed.append(delete["name"])

    filter_read = find_tool(tools, space, "memory_read_", "Fixed read shape: filter read")
    if "filter_note" not in filter_read["name"]:
        fail(f"filter alias should name the read shape: {filter_read['name']}")
    filter_props = require_props(filter_read["schema"], "filter", "limit", "record_type")
    require_required(filter_read["schema"], "filter")
    forbid_props(filter_read["schema"], "mode", "record_id", "query")
    filter_schema = filter_props["filter"]
    if filter_schema.get("additionalProperties") is not False:
        fail(f"filter schema should be closed: {filter_schema}")
    if filter_schema.get("minProperties") != 1:
        fail(f"filter schema should require at least one declared path: {filter_schema}")
    declared = sorted((filter_schema.get("properties") or {}).keys())
    if declared != ["body", "tag"]:
        fail(f"filter schema should expose exactly body/tag, saw {declared}")
    printed.append(filter_read["name"])

print(f"ok: {provider} MemoryWrite and filter provider schemas are shape-specific")
print("captured aliases:")
for name in printed:
    print(f"- {name}")
print(f"captured bodies: {body_log_path}")
PY
chmod +x "$M16C_SPLIT_MULTIPHASE_WORK/scripts/m16c_assert_write_filter_schema.py"

mkdir -p "$M16B_RUNS/m16c-openai-write-filter-schema"
rm -rf "$M16C_SPLIT_MULTIPHASE_WORK/.agentpm-state-m16c-split-multiphase-openai"

"$AGENTPM_MANUAL_PYTHON" "$M16C_SPLIT_MULTIPHASE_WORK/scripts/m16c_split_capture_server.py" \
  --provider openai \
  --scenario schema \
  --port 18088 \
  --log "$M16B_RUNS/m16c-openai-write-filter-schema/bodies.jsonl" \
  >"$M16B_RUNS/m16c-openai-write-filter-schema/server.stdout.txt" \
  2>"$M16B_RUNS/m16c-openai-write-filter-schema/server.stderr.txt" &
SERVER_PID=$!
trap 'kill "$SERVER_PID" 2>/dev/null || true' EXIT
until curl -fsS http://127.0.0.1:18088/health >/dev/null; do sleep 0.1; done

export OPENAI_API_KEY="m16c-capture-key"
export OPENAI_BASE_URL="http://127.0.0.1:18088/v1"

REPORT="$M16B_RUNS/m16c-openai-write-filter-schema/report.json"
(cd "$M16C_SPLIT_MULTIPHASE_WORK" && "$APM" harness \
  --config agentpm.m16b.openai.harness.json \
  --headless \
  --scope user="m16c-openai-write-filter-schema" \
  --input "Capture the M16c OpenAI Memory schema surface, then complete with outcome done." \
  --report "$REPORT" \
  >"$M16B_RUNS/m16c-openai-write-filter-schema/stdout.txt" \
  2>"$M16B_RUNS/m16c-openai-write-filter-schema/stderr.txt")

kill "$SERVER_PID" 2>/dev/null || true
trap - EXIT

"$AGENTPM_MANUAL_PYTHON" "$M16C_SPLIT_MULTIPHASE_WORK/scripts/m16c_assert_write_filter_schema.py" \
  "$M16B_RUNS/m16c-openai-write-filter-schema/bodies.jsonl" \
  openai
```

Expected:

- `bodies.jsonl` contains an OpenAI `/chat/completions` body with the split MemoryWrite function schemas;
- both similar collection spaces have separate `create_or_upsert`, `update`, `delete_or_archive`, and `filter` functions;
- filter schemas are closed to `body` and `tag`, with `minProperties: 1`;
- write schemas have flat, shape-specific parameters and do not expose incompatible fields.

### Test 23: Capture Anthropic M16c MemoryWrite And Filter Schemas

Run this after Test 22. It repeats the same schema capture through the Anthropic provider body shape.

```bash
mkdir -p "$M16B_RUNS/m16c-anthropic-write-filter-schema"
rm -rf "$M16C_SPLIT_MULTIPHASE_WORK/.agentpm-state-m16c-split-multiphase-anthropic"

"$AGENTPM_MANUAL_PYTHON" "$M16C_SPLIT_MULTIPHASE_WORK/scripts/m16c_split_capture_server.py" \
  --provider anthropic \
  --scenario schema \
  --port 18089 \
  --log "$M16B_RUNS/m16c-anthropic-write-filter-schema/bodies.jsonl" \
  >"$M16B_RUNS/m16c-anthropic-write-filter-schema/server.stdout.txt" \
  2>"$M16B_RUNS/m16c-anthropic-write-filter-schema/server.stderr.txt" &
SERVER_PID=$!
trap 'kill "$SERVER_PID" 2>/dev/null || true' EXIT
until curl -fsS http://127.0.0.1:18089/health >/dev/null; do sleep 0.1; done

export ANTHROPIC_API_KEY="m16c-capture-key"
export ANTHROPIC_BASE_URL="http://127.0.0.1:18089"

REPORT="$M16B_RUNS/m16c-anthropic-write-filter-schema/report.json"
(cd "$M16C_SPLIT_MULTIPHASE_WORK" && "$APM" harness \
  --config agentpm.m16b.anthropic.harness.json \
  --headless \
  --scope user="m16c-anthropic-write-filter-schema" \
  --input "Capture the M16c Anthropic Memory schema surface, then complete with outcome done." \
  --report "$REPORT" \
  >"$M16B_RUNS/m16c-anthropic-write-filter-schema/stdout.txt" \
  2>"$M16B_RUNS/m16c-anthropic-write-filter-schema/stderr.txt")

kill "$SERVER_PID" 2>/dev/null || true
trap - EXIT

"$AGENTPM_MANUAL_PYTHON" "$M16C_SPLIT_MULTIPHASE_WORK/scripts/m16c_assert_write_filter_schema.py" \
  "$M16B_RUNS/m16c-anthropic-write-filter-schema/bodies.jsonl" \
  anthropic
```

Expected:

- `bodies.jsonl` contains an Anthropic `/v1/messages` body with the same split MemoryWrite and filter schemas;
- `tools[*].name` exposes the readable provider aliases;
- `tools[*].input_schema` matches the OpenAI `function.parameters` shape for MemoryWrite and filter reads.
