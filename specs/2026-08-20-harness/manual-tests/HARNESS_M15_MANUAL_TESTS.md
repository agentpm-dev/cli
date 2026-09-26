# Harness Milestone 15 Manual Tests

Manual coverage for Milestone 15: Memory lifecycle operations, durable trigger state, source handling, capacity relief, interval triggers, and external invocation controls.

This guide creates a disposable workspace with a deterministic process `ModelRuntime`. That keeps the checks focused on Harness behavior instead of depending on a live model to choose the exact Memory actions or lifecycle output content.

## Coverage Target

| Requirement | Manual coverage |
|---|---|
| Manifest validation for lifecycle shape | Test 0 checks invalid `retain_until_expiration` and interval shorthand are rejected by `agentpm lint` |
| Record-count transform | Test 1 writes a source note and verifies `summarize_notes` produces a summary |
| `replace_input` transform | Test 2 verifies source identity is retained and content is updated in place |
| Consolidate and `delete_after_success` | Test 3 writes three notes, verifies one summary, mutated sources, and no active notes |
| Capacity relief before overflow | Test 4 fills a capped space, then verifies capacity relief runs before the overflow write |
| `retain_until_expiration` | Test 5 verifies sources have `expires_at`, stay active until TTL, then disappear after a read triggers lazy expiry |
| Interval baseline and restart state | Test 6 creates baseline state, waits, then verifies a later run fires the interval delete operation |
| External operation control ingress | Test 7 runs focused machine-control tests for queueing, busy, cancellation, and Engine yield-point routing |
| Failure backoff | Test 8 runs focused lifecycle backoff tests for no immediate retry storms and retry after cooldown |

## Prerequisites

From the root of the `agentpm` repo:

```bash
cargo build -p agentpm-cli
export APM="$PWD/target/debug/agentpm"
export AGENTPM_MANUAL_PYTHON="${AGENTPM_MANUAL_PYTHON:-python3}"
```

## Setup

Run this from the root of the `agentpm` repo. It creates `harness-m15-test/` with:

- one installed Memory Blueprint package under `.agentpm/`;
- one Loop and Agent configured for direct Memory plus selectable lifecycle operations;
- deterministic process-model configs for each scenario;
- invalid manifests for lint checks;
- helper scripts for report and SQLite assertions.

```bash
cat > /tmp/setup-harness-m15-manual.sh <<'SH'
#!/usr/bin/env bash
set -euo pipefail

ROOT="$(pwd)"
APM="${APM:-$ROOT/target/debug/agentpm}"
BASE="${HARNESS_M15_TEST_BASE:-$ROOT/harness-m15-test}"
WORK="$BASE/workspace"
RUNS="$BASE/runs"
PYTHON_CMD="${AGENTPM_MANUAL_PYTHON:-python3}"
SCHEMA_URL="https://raw.githubusercontent.com/agentpm-dev/cli/refs/heads/main/schemas/agentpm.manifest.schema.json"

if [ ! -x "$APM" ]; then
  echo "Missing agentpm binary at $APM. Run: cargo build -p agentpm-cli" >&2
  exit 1
fi

rm -rf "$BASE"
mkdir -p "$WORK/.agentpm" "$WORK/inputs" "$WORK/runtime" "$WORK/scripts" "$WORK/invalid" "$RUNS"

pkg_dir() {
  local plural="$1"
  local package="$2"
  local version="$3"
  local without_at="${package#@}"
  local namespace="${without_at%%/*}"
  local name="${without_at#*/}"
  printf '%s/.agentpm/%s/%s/%s/%s' "$WORK" "$plural" "$namespace" "$name" "$version"
}

write_loop() {
  local dir
  dir="$(pkg_dir loops '@zack/m15-loop' '0.1.0')"
  mkdir -p "$dir"
  cat > "$dir/agent.json" <<JSON
{
  "\$schema": "$SCHEMA_URL",
  "kind": "loop",
  "name": "m15-loop",
  "version": "0.1.0",
  "description": "M15 manual lifecycle loop.",
  "loop": {
    "entry_phase": "remember",
    "phases": [
      {
        "id": "remember",
        "objective": "Use direct Memory exactly as requested, then complete.",
        "access": {
          "tools": false,
          "knowledge": false,
          "memory": { "read": true, "write": true }
        },
        "outcomes": [
          { "id": "done", "description": "The requested Memory behavior is complete." }
        ]
      }
    ],
    "transitions": [
      { "from": "remember", "on": "done", "to": "\$end" }
    ]
  }
}
JSON
  "$APM" lint "$dir/agent.json" >/dev/null
}

write_memory_package() {
  local dir
  dir="$(pkg_dir memory '@zack/m15-memory' '0.1.0')"
  mkdir -p "$dir/schemas"
  cat > "$dir/agent.json" <<JSON
{
  "\$schema": "$SCHEMA_URL",
  "kind": "memory",
  "name": "m15-memory",
  "version": "0.1.0",
  "description": "M15 manual Memory Blueprint.",
  "memory": {
    "scopes": {
      "user": { "description": "Manual user partition." }
    },
    "record_types": {
      "note": {
        "version": "1.0.0",
        "description": "Note.",
        "schema": "schemas/note.schema.json"
      },
      "summary": {
        "version": "1.0.0",
        "description": "Summary.",
        "schema": "schemas/summary.schema.json"
      },
      "capacity_note": {
        "version": "1.0.0",
        "description": "Capacity note.",
        "schema": "schemas/capacity-note.schema.json"
      },
      "expiring_note": {
        "version": "1.0.0",
        "description": "Expiring note.",
        "schema": "schemas/expiring-note.schema.json"
      },
      "interval_note": {
        "version": "1.0.0",
        "description": "Interval note.",
        "schema": "schemas/interval-note.schema.json"
      }
    },
    "spaces": {
      "notes": {
        "description": "Direct note sources.",
        "model": "collection",
        "record_types": ["note"],
        "scope": ["user"],
        "retrieval": { "modes": ["key", "chronological", "filter"] },
        "retention": { "ttl": "P1D", "on_expire": "delete" }
      },
      "summaries": {
        "description": "Lifecycle summaries.",
        "model": "collection",
        "record_types": ["summary"],
        "scope": ["user"],
        "retrieval": { "modes": ["key", "chronological", "filter"] }
      },
      "capacity_notes": {
        "description": "Capped notes.",
        "model": "collection",
        "record_types": ["capacity_note"],
        "scope": ["user"],
        "retrieval": { "modes": ["key", "chronological", "filter"] },
        "capacity": { "max_records": 2 }
      },
      "expiring_notes": {
        "description": "Short-lived notes.",
        "model": "collection",
        "record_types": ["expiring_note"],
        "scope": ["user"],
        "retrieval": { "modes": ["key", "chronological", "filter"] },
        "retention": { "ttl": "PT1S", "on_expire": "delete" }
      },
      "interval_notes": {
        "description": "Interval trigger notes.",
        "model": "collection",
        "record_types": ["interval_note"],
        "scope": ["user"],
        "retrieval": { "modes": ["key", "chronological", "filter"] }
      }
    },
    "operations": {
      "summarize_notes": {
        "type": "transform",
        "description": "Summarize each note source.",
        "trigger": { "type": "record_count", "space": "notes", "threshold": 1 },
        "inputs": [{ "space": "notes", "record_type": "note" }],
        "output": { "space": "summaries", "record_type": "summary" },
        "source_handling": "retain",
        "output_mode": "create",
        "preserve_provenance": true
      },
      "refresh_note": {
        "type": "transform",
        "description": "Refresh notes in place.",
        "trigger": { "type": "record_count", "space": "notes", "threshold": 1 },
        "inputs": [{ "space": "notes", "record_type": "note" }],
        "output": { "space": "notes", "record_type": "note" },
        "source_handling": "retain",
        "output_mode": "replace_input",
        "preserve_provenance": true
      },
      "consolidate_notes": {
        "type": "consolidate",
        "description": "Consolidate three notes into one summary and delete sources.",
        "trigger": { "type": "record_count", "space": "notes", "threshold": 3 },
        "inputs": [{ "space": "notes", "record_type": "note" }],
        "output": { "space": "summaries", "record_type": "summary" },
        "source_handling": "delete_after_success",
        "preserve_provenance": true
      },
      "summarize_expiring_notes": {
        "type": "transform",
        "description": "Summarize expiring notes and let sources expire naturally.",
        "trigger": { "type": "record_count", "space": "expiring_notes", "threshold": 1 },
        "inputs": [{ "space": "expiring_notes", "record_type": "expiring_note" }],
        "output": { "space": "summaries", "record_type": "summary" },
        "source_handling": "retain_until_expiration",
        "output_mode": "create",
        "preserve_provenance": true
      },
      "prune_capacity_notes": {
        "type": "delete",
        "description": "Delete capped notes to relieve capacity.",
        "trigger": { "type": "capacity", "space": "capacity_notes" },
        "targets": [{ "space": "capacity_notes" }],
        "cascade_derived_records": false
      },
      "delete_interval_notes": {
        "type": "delete",
        "description": "Delete interval notes after the interval elapses.",
        "trigger": { "type": "interval", "every": "PT1S" },
        "targets": [{ "space": "interval_notes" }],
        "cascade_derived_records": false
      },
      "external_delete_notes": {
        "type": "delete",
        "description": "Delete notes on external request.",
        "trigger": { "type": "external" },
        "targets": [{ "space": "notes" }],
        "cascade_derived_records": false
      }
    }
  }
}
JSON

  for schema in note summary capacity-note expiring-note interval-note; do
    local prop="body"
    if [ "$schema" = "summary" ]; then prop="summary"; fi
    cat > "$dir/schemas/$schema.schema.json" <<JSON
{
  "\$schema": "https://json-schema.org/draft/2020-12/schema",
  "type": "object",
  "properties": {
    "$prop": { "type": "string", "minLength": 1, "x-agentpm-persist": true }
  },
  "required": ["$prop"],
  "additionalProperties": false
}
JSON
  done

  "$APM" lint "$dir/agent.json" >/dev/null
  "$APM" memory build --manifest "$dir/agent.json" >/dev/null
}

write_agent() {
  cat > "$WORK/agent.json" <<JSON
{
  "\$schema": "$SCHEMA_URL",
  "kind": "agent",
  "name": "m15-agent",
  "version": "0.1.0",
  "description": "M15 manual Harness agent.",
  "tools": [],
  "knowledge": [],
  "memory": ["@zack/m15-memory@0.1.0"],
  "loop": "@zack/m15-loop@0.1.0",
  "bindings": {
    "global": {
      "memory": [
        {
          "package": "@zack/m15-memory",
          "spaces": ["notes", "summaries", "capacity_notes", "expiring_notes", "interval_notes"]
        }
      ]
    }
  }
}
JSON
}

write_lock() {
  cat > "$WORK/agent.lock" <<'JSON'
{
  "lockfile_version": 3,
  "generated": "2026-09-10T00:00:00Z",
  "packages": {
    "loop:@zack/m15-loop@0.1.0": {
      "kind": "loop",
      "name": "@zack/m15-loop",
      "version": "0.1.0",
      "integrity": "sha256-manual"
    },
    "memory:@zack/m15-memory@0.1.0": {
      "kind": "memory",
      "name": "@zack/m15-memory",
      "version": "0.1.0",
      "integrity": "sha256-manual"
    }
  },
  "roots": {
    "local:agent": {
      "name": "m15-agent",
      "version": "0.1.0",
      "tools": [],
      "skills": [],
      "knowledge": [],
      "memory": ["memory:@zack/m15-memory@0.1.0"],
      "profiles": [],
      "loop": "loop:@zack/m15-loop@0.1.0"
    }
  }
}
JSON
}

write_model() {
  cat > "$WORK/runtime/m15_process_model.py" <<'PY'
#!/usr/bin/env python3
import json
import os
import re
import sys
from pathlib import Path

PACKAGE = "@zack/m15-memory"
SCENARIO = os.environ.get("M15_SCENARIO", "transform")
LOG_PATH = Path(os.environ.get("M15_MODEL_LOG", "runtime/m15-process-model-calls.jsonl"))
ordinary_turns = 0

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

def log(entry):
    LOG_PATH.parent.mkdir(parents=True, exist_ok=True)
    with LOG_PATH.open("a") as handle:
        handle.write(json.dumps(entry, sort_keys=True) + "\n")

def write_action(id_, space, record_type, content):
    return {
        "id": id_,
        "action": {
            "type": "memory_write",
            "package": PACKAGE,
            "space": space,
            "operation": "create",
            "record_type": record_type,
            "content": content,
        },
    }

def read_action(id_, space, record_type):
    return {
        "id": id_,
        "action": {
            "type": "memory_read",
            "package": PACKAGE,
            "space": space,
            "mode": "chronological",
            "record_type": record_type,
            "limit": 10,
        },
    }

def complete():
    return {
        "id": "complete-done",
        "action": {
            "type": "phase_completion",
            "outcome": "done",
            "output": {"summary": f"m15 {SCENARIO} complete"},
        },
    }

def operation_name(text):
    match = re.search(r"Memory lifecycle operation `([^`]+)`", text)
    return match.group(1) if match else "unknown"

def lifecycle_turn(text):
    op = operation_name(text)
    if op == "refresh_note":
        return {
            "assistant_content": json.dumps({"body": "m15 refreshed note content"}),
            "actions": [],
            "finish_reason": "stop",
        }
    return {
        "assistant_content": json.dumps({"summary": f"m15 lifecycle output for {op}"}),
        "actions": [],
        "finish_reason": "stop",
    }

def first_ordinary_turn():
    if SCENARIO == "transform":
        actions = [write_action("write-transform-note", "notes", "note", {"body": "m15 transform source"})]
    elif SCENARIO == "replace":
        actions = [write_action("write-replace-note", "notes", "note", {"body": "m15 stale note content"})]
    elif SCENARIO == "consolidate":
        actions = [
            write_action("write-consolidate-a", "notes", "note", {"body": "m15 consolidate source a"}),
            write_action("write-consolidate-b", "notes", "note", {"body": "m15 consolidate source b"}),
            write_action("write-consolidate-c", "notes", "note", {"body": "m15 consolidate source c"}),
        ]
    elif SCENARIO == "capacity_seed":
        actions = [
            write_action("seed-capacity-a", "capacity_notes", "capacity_note", {"body": "m15 capacity seed a"}),
            write_action("seed-capacity-b", "capacity_notes", "capacity_note", {"body": "m15 capacity seed b"}),
        ]
    elif SCENARIO == "capacity_overflow":
        actions = [write_action("overflow-capacity", "capacity_notes", "capacity_note", {"body": "m15 capacity overflow survivor"})]
    elif SCENARIO == "retain_expiring":
        actions = [write_action("write-expiring-note", "expiring_notes", "expiring_note", {"body": "m15 expiring source"})]
    elif SCENARIO == "read_expiring":
        actions = [read_action("read-expiring-notes", "expiring_notes", "expiring_note")]
    elif SCENARIO == "interval_seed":
        actions = [write_action("write-interval-seed", "interval_notes", "interval_note", {"body": "m15 interval seed"})]
    elif SCENARIO == "interval_fire":
        actions = [write_action("write-interval-fire", "interval_notes", "interval_note", {"body": "m15 interval fire"})]
    else:
        actions = []
    return {"assistant_content": None, "actions": actions, "finish_reason": "tool_calls"}

for line in sys.stdin:
    msg = json.loads(line)
    log({"kind": msg.get("kind"), "method": msg.get("method"), "payload": msg.get("payload")})

    if msg.get("kind") == "initialize":
        emit(msg, "initialized", {
            "ready": True,
            "registry_id": f"m15-scripted-{SCENARIO}",
            "model": "m15-scripted",
            "capabilities": {
                "semantic_actions": True,
                "structured_output": True,
                "multimodal_input": False,
                "context_window_tokens": 128000,
                "usage_reporting": True
            }
        })
        continue

    if msg.get("kind") == "request" and msg.get("method") == "generate":
        payload_text = json.dumps(msg.get("payload", {}), sort_keys=True)
        if "Memory lifecycle operation `" in payload_text:
            emit(msg, "response", lifecycle_turn(payload_text))
            continue

        ordinary_turns += 1
        if ordinary_turns == 1:
            emit(msg, "response", first_ordinary_turn())
        else:
            emit(msg, "response", {"assistant_content": None, "actions": [complete()], "finish_reason": "tool_calls"})
        continue

    emit(msg, "error", error={"code": "unsupported_method", "message": "unsupported model request"})
PY
  chmod +x "$WORK/runtime/m15_process_model.py"

  for scenario in transform replace consolidate capacity_seed capacity_overflow retain_expiring read_expiring interval_seed interval_fire; do
    cat > "$WORK/runtime/m15_model_$scenario.sh" <<SH2
#!/usr/bin/env bash
set -euo pipefail
export M15_SCENARIO="$scenario"
export M15_MODEL_LOG="runtime/m15-process-model-$scenario.jsonl"
exec "$PYTHON_CMD" runtime/m15_process_model.py
SH2
    chmod +x "$WORK/runtime/m15_model_$scenario.sh"
  done
}

write_config() {
  local scenario="$1"
  local state_dir="$2"
  cat > "$WORK/agentpm.m15.$scenario.harness.json" <<JSON
{
  "version": 1,
  "model": {
    "provider": "m15-scripted-$scenario",
    "model": "m15-scripted"
  },
  "providers": {
    "models": {
      "m15-scripted-$scenario": {
        "implementation": {
          "type": "process",
          "command": "runtime/m15_model_$scenario.sh",
          "args": [],
          "cwd": ".",
          "env": [],
          "startup_timeout_ms": 10000,
          "request_timeout_ms": 30000,
          "restart": { "max_attempts": 0, "backoff_ms": 0 }
        }
      }
    }
  },
  "scopes": {
    "user": "m15-user"
  },
  "runtime": {
    "state_dir": "$state_dir",
    "limits": {
      "max_steps": 4,
      "max_model_calls_per_phase": 12,
      "max_actions_per_phase": 32,
      "max_memory_operation_repairs": 2,
      "max_structured_output_repairs": 3
    }
  },
  "trace": {
    "enabled": true,
    "level": "verbose",
    "content": "full"
  }
}
JSON
}

write_inputs() {
  for scenario in transform replace consolidate capacity_seed capacity_overflow retain_expiring read_expiring interval_seed interval_fire; do
    cat > "$WORK/inputs/$scenario.txt" <<TXT
Run the M15 manual scenario: $scenario.
Use only direct Memory actions requested by the deterministic process model, then complete with outcome done.
TXT
  done
}

write_helpers() {
  cat > "$WORK/scripts/select_ops.py" <<'PY'
#!/usr/bin/env python3
import json
import sys
from pathlib import Path

agent = Path("agent.json")
data = json.loads(agent.read_text())
binding = data["bindings"]["global"]["memory"][0]
if len(sys.argv) > 1:
    binding["operations"] = sys.argv[1:]
else:
    binding.pop("operations", None)
agent.write_text(json.dumps(data, indent=2) + "\n")
print("selected operations:", ", ".join(binding.get("operations", [])) or "(none)")
PY
  chmod +x "$WORK/scripts/select_ops.py"

  cat > "$WORK/scripts/m15_assert_report.py" <<'PY'
#!/usr/bin/env python3
import json
import sys
from pathlib import Path

report_path = Path(sys.argv[1])
checks = sys.argv[2:]
report = json.loads(report_path.read_text())
trace_path = Path(report["trace_path"])
if not trace_path.is_absolute():
    trace_path = report_path.parent / trace_path
events = [json.loads(line) for line in trace_path.read_text().splitlines() if line.strip()]

def fields(event):
    return (event.get("payload") or {}).get("fields") or {}

def require(condition, message):
    if not condition:
        raise SystemExit(message)

def events_of(name):
    return [event for event in events if event.get("event_type") == name]

def operation_events(name, operation):
    return [event for event in events_of(name) if fields(event).get("operation") == operation]

def summaries():
    return (
        report.get("operation_summaries")
        or report.get("memory_operation_summaries")
        or report.get("memory_summaries")
        or []
    )

for check in checks:
    if check == "ended":
        require(report.get("terminal_status") == "ended", f"expected ended, got {report.get('terminal_status')}")
    elif check == "no-tools":
        require((report.get("usage") or {}).get("tool_calls", 0) == 0, "expected no tool calls")
    elif check.startswith("operation-completed:"):
        op = check.split(":", 1)[1]
        require(operation_events("memory_operation_completed", op), f"missing memory_operation_completed for {op}")
    elif check.startswith("operation-output:"):
        op = check.split(":", 1)[1]
        require(operation_events("memory_operation_output", op), f"missing memory_operation_output for {op}")
    elif check.startswith("operation-source:"):
        op = check.split(":", 1)[1]
        require(operation_events("memory_operation_source", op), f"missing memory_operation_source for {op}")
    elif check.startswith("trigger-evaluated:"):
        op = check.split(":", 1)[1]
        require(operation_events("memory_trigger_evaluated", op), f"missing memory_trigger_evaluated for {op}")
    elif check.startswith("operation-summary:"):
        op = check.split(":", 1)[1]
        identity = f"@zack/m15-memory/operations/{op}"
        require(
            any(
                summary.get("identity") == identity
                and summary.get("operation_kind") == "memory_operation"
                and summary.get("status") == "completed"
                for summary in summaries()
            ),
            f"missing completed report summary for {identity}",
        )
    elif check == "lifecycle-model-content-only":
        prompts = [fields(event) for event in events_of("prompt_prepared")]
        lifecycle_prompts = [prompt for prompt in prompts if "Memory lifecycle operation `" in prompt.get("prompt", "")]
        require(lifecycle_prompts, "missing lifecycle prompt_prepared event")
        for prompt in lifecycle_prompts:
            require(prompt.get("memory_operation"), "lifecycle prompt_prepared should name memory_operation")
            require(prompt.get("action_descriptors") == 0, "lifecycle prompt_prepared should have no action descriptors")
            require("return exactly one JSON content object" in prompt.get("prompt", ""), "lifecycle prompt should request one JSON content object")
            require("4. OUTPUT CONTENT SCHEMA" in prompt.get("prompt", ""), "lifecycle prompt should include output content schema")
        requests = [fields(event) for event in events_of("model_runtime_request_prepared")]
        lifecycle = [request for request in requests if "Memory lifecycle operation `" in request.get("prompt", "")]
        require(lifecycle, "missing lifecycle model request")
        for request in lifecycle:
            require(not request.get("action_aliases"), "lifecycle model request should not expose semantic action aliases")
            require("return exactly one JSON content object" in request.get("prompt", ""), "lifecycle prompt should request one JSON content object")
            require("4. OUTPUT CONTENT SCHEMA" in request.get("prompt", ""), "lifecycle prompt should include output content schema")
    else:
        raise SystemExit(f"unknown check: {check}")

print(f"ok: {report_path} {' '.join(checks)}")
PY
  chmod +x "$WORK/scripts/m15_assert_report.py"

  cat > "$WORK/scripts/m15_assert_sqlite.py" <<'PY'
#!/usr/bin/env python3
import json
import sqlite3
import sys
from pathlib import Path

state = Path(sys.argv[1])
checks = sys.argv[2:]
db = state / "memory.sqlite3" if state.is_dir() else state
if not db.exists():
    raise SystemExit(f"missing SQLite database: {db}")
conn = sqlite3.connect(db)
conn.row_factory = sqlite3.Row

def require(condition, message):
    if not condition:
        raise SystemExit(message)

def active_rows(space):
    return conn.execute(
        """
        SELECT id, space, record_type, content_json, provenance_json, expires_at, archived_at, ordinal
        FROM memory_records
        WHERE space = ? AND archived_at IS NULL
        ORDER BY created_at ASC, id ASC
        """,
        (space,),
    ).fetchall()

def all_rows(space):
    return conn.execute(
        """
        SELECT id, space, record_type, content_json, provenance_json, expires_at, archived_at, ordinal
        FROM memory_records
        WHERE space = ?
        ORDER BY created_at ASC, id ASC
        """,
        (space,),
    ).fetchall()

for check in checks:
    if check.startswith("active-count:"):
        _, space, count = check.split(":", 2)
        rows = active_rows(space)
        require(len(rows) == int(count), f"expected {count} active rows in {space}, got {len(rows)}")
    elif check.startswith("all-count:"):
        _, space, count = check.split(":", 2)
        rows = all_rows(space)
        require(len(rows) == int(count), f"expected {count} total rows in {space}, got {len(rows)}")
    elif check.startswith("content-contains:"):
        _, space, text = check.split(":", 2)
        require(any(text in row["content_json"] for row in all_rows(space)), f"missing content containing {text!r} in {space}")
    elif check.startswith("expires-present:"):
        space = check.split(":", 1)[1]
        require(any(row["expires_at"] for row in all_rows(space)), f"missing expires_at in {space}")
    elif check.startswith("operation-state:"):
        op = check.split(":", 1)[1]
        rows = conn.execute("SELECT * FROM memory_operation_state WHERE operation = ?", (op,)).fetchall()
        require(rows, f"missing operation state for {op}")
    else:
        raise SystemExit(f"unknown check: {check}")

print(f"ok: {db} {' '.join(checks)}")
PY
  chmod +x "$WORK/scripts/m15_assert_sqlite.py"

  cat > "$WORK/scripts/sqlite_memory_summary.py" <<'PY'
#!/usr/bin/env python3
import json
import sqlite3
import sys
from pathlib import Path

path = Path(sys.argv[1])
db = path / "memory.sqlite3" if path.is_dir() else path
conn = sqlite3.connect(db)
conn.row_factory = sqlite3.Row
records = []
for row in conn.execute("SELECT * FROM memory_records ORDER BY space, created_at, id"):
    item = dict(row)
    item["content"] = json.loads(item.pop("content_json"))
    item["provenance"] = json.loads(item.pop("provenance_json"))
    item["scope"] = json.loads(item["scope_json"])
    records.append(item)
states = [dict(row) for row in conn.execute("SELECT * FROM memory_operation_state ORDER BY operation")]
print(json.dumps({"db": str(db), "records": records, "operation_state": states}, indent=2, sort_keys=True))
PY
  chmod +x "$WORK/scripts/sqlite_memory_summary.py"
}

write_invalid_manifests() {
  local valid
  valid="$(pkg_dir memory '@zack/m15-memory' '0.1.0')/agent.json"
  mkdir -p "$WORK/invalid/retain-without-retention" "$WORK/invalid/interval-shorthand"
  cp "$valid" "$WORK/invalid/retain-without-retention/agent.json"
  "$PYTHON_CMD" - "$WORK/invalid/retain-without-retention/agent.json" <<'PY'
import json
import sys
from pathlib import Path
path = Path(sys.argv[1])
data = json.loads(path.read_text())
data["memory"]["spaces"]["notes"].pop("retention", None)
data["memory"]["operations"] = {
    "bad_retain_until_expiration": {
        "type": "transform",
        "description": "Invalid retain_until_expiration source.",
        "trigger": {"type": "external"},
        "inputs": [{"space": "notes", "record_type": "note"}],
        "output": {"space": "summaries", "record_type": "summary"},
        "source_handling": "retain_until_expiration",
        "output_mode": "create",
        "preserve_provenance": True,
    }
}
path.write_text(json.dumps(data, indent=2) + "\n")
PY

  cp "$valid" "$WORK/invalid/interval-shorthand/agent.json"
  "$PYTHON_CMD" - "$WORK/invalid/interval-shorthand/agent.json" <<'PY'
import json
import sys
from pathlib import Path
path = Path(sys.argv[1])
data = json.loads(path.read_text())
data["memory"]["operations"]["delete_interval_notes"]["trigger"]["every"] = "5m"
path.write_text(json.dumps(data, indent=2) + "\n")
PY
}

write_loop
write_memory_package
write_agent
write_lock
write_model
write_inputs
write_helpers
write_invalid_manifests

for scenario in transform replace consolidate capacity_seed capacity_overflow retain_expiring read_expiring interval_seed interval_fire; do
  case "$scenario" in
    capacity_seed|capacity_overflow)
      state_dir=".agentpm-state-capacity"
      ;;
    retain_expiring|read_expiring)
      state_dir=".agentpm-state-retain-expiring"
      ;;
    interval_seed|interval_fire)
      state_dir=".agentpm-state-interval"
      ;;
    *)
      state_dir=".agentpm-state-$scenario"
      ;;
  esac
  write_config "$scenario" "$state_dir"
done

(cd "$WORK" && "$APM" lint agent.json >/dev/null)

cat > "$BASE/README.txt" <<TXT
M15 manual workspace created at:
  $BASE

Useful paths:
  workspace:   $WORK
  run outputs: $RUNS
TXT

cat > "$BASE/env.sh" <<TXT
export HARNESS_M15_TEST_BASE="$BASE"
export M15_WORK="$WORK"
export M15_RUNS="$RUNS"
export APM="$APM"
export AGENTPM_MANUAL_PYTHON="$PYTHON_CMD"
TXT

cat "$BASE/README.txt"
echo
echo "To reuse this workspace in a new shell, run:"
echo "  source \"$BASE/env.sh\""
SH

chmod +x /tmp/setup-harness-m15-manual.sh
/tmp/setup-harness-m15-manual.sh

export HARNESS_M15_TEST_BASE="${HARNESS_M15_TEST_BASE:-$PWD/harness-m15-test}"
export M15_WORK="$HARNESS_M15_TEST_BASE/workspace"
export M15_RUNS="$HARNESS_M15_TEST_BASE/runs"
```

If you open a new terminal or your shell loses these exports, restore them with:

```bash
source "$PWD/harness-m15-test/env.sh"
```

## Test 0: Manifest Lint Pins

```bash
set +e
"$APM" lint "$M15_WORK/invalid/retain-without-retention" >"$M15_RUNS/00-retain-lint.stdout.txt" 2>"$M15_RUNS/00-retain-lint.stderr.txt"
RETAIN_STATUS=$?
"$APM" lint "$M15_WORK/invalid/interval-shorthand" >"$M15_RUNS/00-interval-lint.stdout.txt" 2>"$M15_RUNS/00-interval-lint.stderr.txt"
INTERVAL_STATUS=$?
set -e

test "$RETAIN_STATUS" -ne 0
test "$INTERVAL_STATUS" -ne 0
grep -F "retain_until_expiration" "$M15_RUNS/00-retain-lint.stdout.txt" "$M15_RUNS/00-retain-lint.stderr.txt"
grep -F "supported positive ISO 8601 duration subset" "$M15_RUNS/00-interval-lint.stdout.txt" "$M15_RUNS/00-interval-lint.stderr.txt"
```

Expected: both invalid manifests fail lint before any Harness run starts.

## Test 1: Record-Count Transform

```bash
mkdir -p "$M15_RUNS/transform"
(cd "$M15_WORK" && "$AGENTPM_MANUAL_PYTHON" scripts/select_ops.py summarize_notes)

REPORT="$M15_RUNS/transform/transform.report.json"
(cd "$M15_WORK" && "$APM" harness \
  --config agentpm.m15.transform.harness.json \
  --headless \
  --scope user=m15-user \
  --input-file inputs/transform.txt \
  --report "$REPORT" \
  >"$M15_RUNS/transform/stdout.txt" \
  2>"$M15_RUNS/transform/stderr.txt")

"$AGENTPM_MANUAL_PYTHON" "$M15_WORK/scripts/m15_assert_report.py" "$REPORT" \
  ended no-tools trigger-evaluated:summarize_notes operation-completed:summarize_notes operation-output:summarize_notes operation-summary:summarize_notes lifecycle-model-content-only

"$AGENTPM_MANUAL_PYTHON" "$M15_WORK/scripts/m15_assert_sqlite.py" "$M15_WORK/.agentpm-state-transform" \
  active-count:notes:1 active-count:summaries:1 content-contains:summaries:summarize_notes operation-state:summarize_notes
```

Expected: one note remains active, one summary is created, the lifecycle model request asks for exactly one JSON content object, the output content schema is present, and the operation summary is completed.

## Test 2: Replace-Input Transform

```bash
mkdir -p "$M15_RUNS/replace"
(cd "$M15_WORK" && "$AGENTPM_MANUAL_PYTHON" scripts/select_ops.py refresh_note)

REPORT="$M15_RUNS/replace/replace.report.json"
(cd "$M15_WORK" && "$APM" harness \
  --config agentpm.m15.replace.harness.json \
  --headless \
  --scope user=m15-user \
  --input-file inputs/replace.txt \
  --report "$REPORT" \
  >"$M15_RUNS/replace/stdout.txt" \
  2>"$M15_RUNS/replace/stderr.txt")

"$AGENTPM_MANUAL_PYTHON" "$M15_WORK/scripts/m15_assert_report.py" "$REPORT" \
  ended no-tools operation-completed:refresh_note operation-output:refresh_note operation-summary:refresh_note lifecycle-model-content-only

"$AGENTPM_MANUAL_PYTHON" "$M15_WORK/scripts/m15_assert_sqlite.py" "$M15_WORK/.agentpm-state-replace" \
  active-count:notes:1 active-count:summaries:0 content-contains:notes:refreshed operation-state:refresh_note
```

Expected: the note is updated in place, no summary is created, and the output is still a Harness-owned Memory record.

## Test 3: Consolidate And Delete Sources

```bash
mkdir -p "$M15_RUNS/consolidate"
(cd "$M15_WORK" && "$AGENTPM_MANUAL_PYTHON" scripts/select_ops.py consolidate_notes)

REPORT="$M15_RUNS/consolidate/consolidate.report.json"
(cd "$M15_WORK" && "$APM" harness \
  --config agentpm.m15.consolidate.harness.json \
  --headless \
  --scope user=m15-user \
  --input-file inputs/consolidate.txt \
  --report "$REPORT" \
  >"$M15_RUNS/consolidate/stdout.txt" \
  2>"$M15_RUNS/consolidate/stderr.txt")

"$AGENTPM_MANUAL_PYTHON" "$M15_WORK/scripts/m15_assert_report.py" "$REPORT" \
  ended no-tools operation-completed:consolidate_notes operation-output:consolidate_notes operation-source:consolidate_notes operation-summary:consolidate_notes lifecycle-model-content-only

"$AGENTPM_MANUAL_PYTHON" "$M15_WORK/scripts/m15_assert_sqlite.py" "$M15_WORK/.agentpm-state-consolidate" \
  active-count:notes:0 active-count:summaries:1 content-contains:summaries:consolidate_notes operation-state:consolidate_notes
```

Expected: three source notes are deleted in the same lifecycle commit that writes one summary.

## Test 4: Capacity Relief Before Overflow

```bash
mkdir -p "$M15_RUNS/capacity"
(cd "$M15_WORK" && "$AGENTPM_MANUAL_PYTHON" scripts/select_ops.py)

SEED_REPORT="$M15_RUNS/capacity/seed.report.json"
(cd "$M15_WORK" && "$APM" harness \
  --config agentpm.m15.capacity_seed.harness.json \
  --headless \
  --scope user=m15-user \
  --input-file inputs/capacity_seed.txt \
  --report "$SEED_REPORT" \
  >"$M15_RUNS/capacity/seed.stdout.txt" \
  2>"$M15_RUNS/capacity/seed.stderr.txt")

"$AGENTPM_MANUAL_PYTHON" "$M15_WORK/scripts/m15_assert_report.py" "$SEED_REPORT" ended no-tools
"$AGENTPM_MANUAL_PYTHON" "$M15_WORK/scripts/m15_assert_sqlite.py" "$M15_WORK/.agentpm-state-capacity" active-count:capacity_notes:2

(cd "$M15_WORK" && "$AGENTPM_MANUAL_PYTHON" scripts/select_ops.py prune_capacity_notes)

OVERFLOW_REPORT="$M15_RUNS/capacity/overflow.report.json"
(cd "$M15_WORK" && "$APM" harness \
  --config agentpm.m15.capacity_overflow.harness.json \
  --headless \
  --scope user=m15-user \
  --input-file inputs/capacity_overflow.txt \
  --report "$OVERFLOW_REPORT" \
  >"$M15_RUNS/capacity/overflow.stdout.txt" \
  2>"$M15_RUNS/capacity/overflow.stderr.txt")

"$AGENTPM_MANUAL_PYTHON" "$M15_WORK/scripts/m15_assert_report.py" "$OVERFLOW_REPORT" \
  ended no-tools operation-completed:prune_capacity_notes operation-source:prune_capacity_notes operation-summary:prune_capacity_notes

"$AGENTPM_MANUAL_PYTHON" "$M15_WORK/scripts/m15_assert_sqlite.py" "$M15_WORK/.agentpm-state-capacity" \
  active-count:capacity_notes:1 content-contains:capacity_notes:survivor operation-state:prune_capacity_notes
```

Expected: the two seeded rows are removed by capacity relief and the would-overflow write succeeds as the one active survivor.

## Test 5: Retain Until Expiration

```bash
mkdir -p "$M15_RUNS/retain-expiring"
(cd "$M15_WORK" && "$AGENTPM_MANUAL_PYTHON" scripts/select_ops.py summarize_expiring_notes)

REPORT="$M15_RUNS/retain-expiring/create.report.json"
(cd "$M15_WORK" && "$APM" harness \
  --config agentpm.m15.retain_expiring.harness.json \
  --headless \
  --scope user=m15-user \
  --input-file inputs/retain_expiring.txt \
  --report "$REPORT" \
  >"$M15_RUNS/retain-expiring/create.stdout.txt" \
  2>"$M15_RUNS/retain-expiring/create.stderr.txt")

"$AGENTPM_MANUAL_PYTHON" "$M15_WORK/scripts/m15_assert_report.py" "$REPORT" \
  ended no-tools operation-completed:summarize_expiring_notes operation-output:summarize_expiring_notes operation-summary:summarize_expiring_notes lifecycle-model-content-only

"$AGENTPM_MANUAL_PYTHON" "$M15_WORK/scripts/m15_assert_sqlite.py" "$M15_WORK/.agentpm-state-retain-expiring" \
  active-count:expiring_notes:1 active-count:summaries:1 expires-present:expiring_notes operation-state:summarize_expiring_notes

sleep 2

(cd "$M15_WORK" && "$AGENTPM_MANUAL_PYTHON" scripts/select_ops.py)
READ_REPORT="$M15_RUNS/retain-expiring/read-after-ttl.report.json"
(cd "$M15_WORK" && "$APM" harness \
  --config agentpm.m15.read_expiring.harness.json \
  --headless \
  --scope user=m15-user \
  --input-file inputs/read_expiring.txt \
  --report "$READ_REPORT" \
  >"$M15_RUNS/retain-expiring/read.stdout.txt" \
  2>"$M15_RUNS/retain-expiring/read.stderr.txt")

"$AGENTPM_MANUAL_PYTHON" "$M15_WORK/scripts/m15_assert_report.py" "$READ_REPORT" ended no-tools
"$AGENTPM_MANUAL_PYTHON" "$M15_WORK/scripts/m15_assert_sqlite.py" "$M15_WORK/.agentpm-state-retain-expiring" active-count:expiring_notes:0
```

Expected: `retain_until_expiration` does not delete the source immediately, but the source has `expires_at` and is gone from active reads after TTL.

## Test 6: Interval Baseline And Restart

```bash
mkdir -p "$M15_RUNS/interval"
(cd "$M15_WORK" && "$AGENTPM_MANUAL_PYTHON" scripts/select_ops.py delete_interval_notes)

SEED_REPORT="$M15_RUNS/interval/seed.report.json"
(cd "$M15_WORK" && "$APM" harness \
  --config agentpm.m15.interval_seed.harness.json \
  --headless \
  --scope user=m15-user \
  --input-file inputs/interval_seed.txt \
  --report "$SEED_REPORT" \
  >"$M15_RUNS/interval/seed.stdout.txt" \
  2>"$M15_RUNS/interval/seed.stderr.txt")

"$AGENTPM_MANUAL_PYTHON" "$M15_WORK/scripts/m15_assert_report.py" "$SEED_REPORT" ended no-tools trigger-evaluated:delete_interval_notes
"$AGENTPM_MANUAL_PYTHON" "$M15_WORK/scripts/m15_assert_sqlite.py" "$M15_WORK/.agentpm-state-interval" active-count:interval_notes:1 operation-state:delete_interval_notes

sleep 2

FIRE_REPORT="$M15_RUNS/interval/fire.report.json"
(cd "$M15_WORK" && "$APM" harness \
  --config agentpm.m15.interval_fire.harness.json \
  --headless \
  --scope user=m15-user \
  --input-file inputs/interval_fire.txt \
  --report "$FIRE_REPORT" \
  >"$M15_RUNS/interval/fire.stdout.txt" \
  2>"$M15_RUNS/interval/fire.stderr.txt")

"$AGENTPM_MANUAL_PYTHON" "$M15_WORK/scripts/m15_assert_report.py" "$FIRE_REPORT" \
  ended no-tools operation-completed:delete_interval_notes operation-source:delete_interval_notes operation-summary:delete_interval_notes

"$AGENTPM_MANUAL_PYTHON" "$M15_WORK/scripts/m15_assert_sqlite.py" "$M15_WORK/.agentpm-state-interval" \
  active-count:interval_notes:1 content-contains:interval_notes:fire operation-state:delete_interval_notes
```

Expected: the first run establishes interval state without deleting the seed row; the second run starts a new Harness process/session, observes the elapsed interval at phase start, deletes the old interval note, and then keeps the new note written during the run.

## Test 7: External Operation Control Ingress

The CLI/TUI surface for interactive external Memory controls is not the M15 manual target. Pin the Engine/machine ingress with the focused tests that exercise queueing, busy errors, cancellation, scope mismatch, and Engine yield-point execution:

```bash
cargo test -p agentpm-cli machine_memory_operation_control -- --test-threads=1
cargo test -p agentpm-cli machine_bridge_queues_memory_operation_while_active_without_blocking_host_service_response -- --test-threads=1
cargo test -p agentpm-cli machine_bridge_cancel_run_flushes_pending_memory_operation_control -- --test-threads=1
```

Expected: all three commands pass.

## Test 8: Failure Backoff

Run the focused tests for lifecycle failure cooldown behavior:

```bash
cargo test -p agentpm-cli memory_lifecycle_failure_backoff -- --test-threads=1
```

Expected: both backoff tests pass, proving a deterministic failure does not retry on the next write and does retry after the cooldown expires.

## Inspection Helpers

Dump state for any scenario:

```bash
"$AGENTPM_MANUAL_PYTHON" "$M15_WORK/scripts/sqlite_memory_summary.py" "$M15_WORK/.agentpm-state-transform" | less
```

Inspect a report:

```bash
jq '{terminal_status, terminal_output, usage, operation_summaries, memory_summaries, error_count, repair_count, trace_path}' "$REPORT"
```

## Cleanup

```bash
rm -f /tmp/setup-harness-m15-manual.sh
rm -rf "$HARNESS_M15_TEST_BASE"
```
