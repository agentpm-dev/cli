# Harness Milestone 14f Manual Tests

Manual coverage for Milestone 14f: Memory hooks before read/write/operation dispatch, hook patch/rejection behavior, fail-closed boundaries, MemoryRuntime wire-boundary projection, and SDK hook contract exposure.

This pass extends the M14b/M14c Memory fixture. M14f is a governance layer over the direct Memory action surface, so the manual tests use the same package and add fake Hook and MemoryRuntime services. The SDK checks are focused contract smoke tests; they do not require a live Harness run through a host bridge.

## Coverage target

| Requirement | Manual coverage |
|---|---|
| `before_memory_write` can patch record content before mutation | process Hook patches a local SQLite write, then readback verifies patched content |
| `before_memory_read` can patch query/filter/limit/mode before dispatch | process Hook patches a local read and readback verifies the patched result |
| Hook rejections fail closed before Memory dispatch | read Hook rejection run emits `hook_rejected` and no `memory_read_started` |
| queued nonfatal Hook failures drain before terminal rejection | two read Hooks: first nonfatal failure, second rejection; ordering asserted |
| read-side Hook patches are revalidated before dispatch | process Hook returns an undeclared read mode; Harness emits `hook_failed` and does not dispatch Memory |
| Hook-modified Memory content is projected before custom runtime wire dispatch | write Hook adds a private scratch field; custom MemoryRuntime request log must not contain it |
| SDKs expose typed Memory Hook contracts | Node and Python SDK focused harness tests include typed Memory Hook registration and decisions |
| M14f automated regression coverage remains green | focused Rust engine/runtime Hook suites |

Provider behavior can vary. If a live model does not select the explicitly requested Memory action, keep the report/trace and rerun that scenario before treating it as a Harness regression.

## Prerequisites

From the root of the `agentpm` repo:

```bash
cargo build -p agentpm-cli
export APM="$PWD/target/debug/agentpm"
export AGENTPM_MANUAL_PYTHON="${AGENTPM_MANUAL_PYTHON:-python3}"
export AGENTPM_PROJECT_ROOT="${AGENTPM_PROJECT_ROOT:-$(dirname "$PWD")}"
export OPENAI_API_KEY="your OpenAI key"
```

If the default OpenAI model is unavailable in your account, set an override before setup:

```bash
export AGENTPM_MANUAL_OPENAI_MODEL="${AGENTPM_MANUAL_OPENAI_MODEL:-gpt-4o-mini}"
```

## Setup

First create the M14b/M14c fixture using the setup block in `HARNESS_M14B_M14C_MANUAL_TESTS.md`.

Expected workspace:

```bash
export HARNESS_M14BC_TEST_BASE="${HARNESS_M14BC_TEST_BASE:-$PWD/harness-m14bc-test}"
export M14BC_WORK="$HARNESS_M14BC_TEST_BASE/workspace"
export M14BC_RUNS="$HARNESS_M14BC_TEST_BASE/runs"
```

Then apply the M14f Hook/runtime extension:

```bash
cat > /tmp/setup-harness-m14f-manual.sh <<'SH'
#!/usr/bin/env bash
set -euo pipefail

ROOT="$(pwd)"
APM="${APM:-$ROOT/target/debug/agentpm}"
BASE="${HARNESS_M14BC_TEST_BASE:-$ROOT/harness-m14bc-test}"
WORK="$BASE/workspace"
RUNS="$BASE/runs-m14f"
PYTHON_CMD="${AGENTPM_MANUAL_PYTHON:-python3}"
MEMORY_MANIFEST="$WORK/.agentpm/memory/zack/m14bc-memory/0.1.0/agent.json"
BASE_CONFIG="$WORK/agentpm.openai.harness.json"

if [ ! -x "$APM" ]; then
  echo "Missing agentpm binary at $APM. Run: cargo build -p agentpm-cli" >&2
  exit 1
fi

if [ ! -f "$MEMORY_MANIFEST" ] || [ ! -f "$BASE_CONFIG" ]; then
  echo "Missing M14b/M14c fixture. Run HARNESS_M14B_M14C_MANUAL_TESTS.md setup first." >&2
  exit 1
fi

mkdir -p "$WORK/runtime" "$WORK/inputs" "$WORK/scripts" "$RUNS"/{hooks,custom-runtime,sdk,rust}

export M14F_SETUP_WORK="$WORK"
export M14F_SETUP_RUNS="$RUNS"
export M14F_SETUP_MODEL="${AGENTPM_MANUAL_OPENAI_MODEL:-gpt-4o-mini}"

"$PYTHON_CMD" <<'PY'
import json
import os
from copy import deepcopy
from pathlib import Path

work = Path(os.environ["M14F_SETUP_WORK"])
model = os.environ["M14F_SETUP_MODEL"]
base_config_path = work / "agentpm.openai.harness.json"
base = json.loads(base_config_path.read_text())

(work / "runtime" / "m14f_hook_service.py").write_text(r'''#!/usr/bin/env python3
import json
import os
import sys
from pathlib import Path

MODE = os.environ.get("M14F_HOOK_MODE", "continue")
REGISTRY_ID = os.environ.get("M14F_HOOK_REGISTRY_ID", "m14f-hooks")
LOG_PATH = Path(os.environ.get("M14F_HOOK_LOG", "runtime/m14f-hook-calls.jsonl"))

def write_log(entry):
    LOG_PATH.parent.mkdir(parents=True, exist_ok=True)
    with LOG_PATH.open("a") as handle:
        handle.write(json.dumps(entry, sort_keys=True) + "\n")

def emit(msg, kind, result=None, error=None):
    out = {
        "protocol": "agentpm-service",
        "version": 1,
        "kind": kind,
        "id": msg.get("id"),
        "service": msg.get("service", "hook"),
    }
    if result is not None:
        out["result"] = result
    if error is not None:
        out["error"] = error
    print(json.dumps(out), flush=True)

for line in sys.stdin:
    msg = json.loads(line)
    write_log({"kind": msg.get("kind"), "method": msg.get("method"), "payload": msg.get("payload")})

    if msg.get("kind") == "initialize":
        hooks = msg.get("payload", {}).get("hooks", [])
        emit(msg, "initialized", {
            "registry_id": REGISTRY_ID,
            "ready": True,
            "hooks": hooks,
        })
        continue

    method = msg.get("method")

    payload = msg.get("payload", {})
    hook_input = payload.get("input", {})
    space = hook_input.get("space")
    record_type = hook_input.get("record_type")

    if MODE == "write_patch" and method == "before_memory_write":
        if space != "notes" or record_type != "note":
            emit(msg, "response", {"decision": "continue"})
            continue
        emit(msg, "response", {
            "decision": "continue",
            "patch": {
                "content": {
                    "body": "M14f hooked process note",
                    "status": "open",
                    "labels": ["m14f", "hooked"],
                    "scratch": {
                        "public": "m14f hook public scratch",
                        "private": "m14f-ephemeral-secret"
                    }
                }
            }
        })
        continue

    if MODE == "custom_write_patch" and method == "before_memory_write":
        if space != "notes" or record_type != "note":
            emit(msg, "response", {"decision": "continue"})
            continue
        emit(msg, "response", {
            "decision": "continue",
            "patch": {
                "content": {
                    "body": "M14f hooked custom runtime note",
                    "status": "open",
                    "labels": ["m14f", "custom-runtime"],
                    "scratch": {
                        "public": "m14f custom public scratch",
                        "private": "m14f-ephemeral-secret"
                    }
                }
            }
        })
        continue

    if MODE == "read_patch" and method == "before_memory_read":
        emit(msg, "response", {
            "decision": "continue",
            "patch": {
                "mode": "filter",
                "filter": {"body": "M14f hooked process note"},
                "limit": 1
            }
        })
        continue

    if MODE == "read_bad_mode" and method == "before_memory_read":
        emit(msg, "response", {
            "decision": "continue",
            "patch": {
                "mode": "not_a_declared_mode",
                "limit": 1
            }
        })
        continue

    if MODE == "read_failure" and method == "before_memory_read":
        emit(msg, "response", {"decision": "unsupported_manual_failure"})
        continue

    if MODE == "read_reject" and method == "before_memory_read":
        emit(msg, "response", {
            "decision": "reject",
            "reason": "m14f policy denied memory read"
        })
        continue

    emit(msg, "response", {"decision": "continue"})
''')
(work / "runtime" / "m14f_hook_service.py").chmod(0o755)

(work / "runtime" / "m14f_memory_runtime_service.py").write_text(r'''#!/usr/bin/env python3
import json
import os
import sys
from datetime import datetime, timezone
from pathlib import Path

REGISTRY_ID = os.environ.get("M14F_MEMORY_REGISTRY_ID", "m14f-custom-memory")
LOG_PATH = Path(os.environ.get("M14F_MEMORY_LOG", "runtime/m14f-custom-memory-calls.jsonl"))
STATE = {}

def write_log(entry):
    LOG_PATH.parent.mkdir(parents=True, exist_ok=True)
    with LOG_PATH.open("a") as handle:
        handle.write(json.dumps(entry, sort_keys=True) + "\n")

def emit(msg, kind, result=None, error=None):
    out = {
        "protocol": "agentpm-service",
        "version": 1,
        "kind": kind,
        "id": msg.get("id"),
        "service": "memory",
    }
    if result is not None:
        out["result"] = result
    if error is not None:
        out["error"] = error
    print(json.dumps(out), flush=True)

def capabilities():
    return {
        "space_models": ["document", "collection", "sequence"],
        "retrieval_modes": ["key", "filter", "chronological", "full_text", "semantic"],
        "retention_actions": ["archive", "delete"],
        "constraints": ["append_only"],
        "capacity": True,
        "durable_trigger_state": True,
        "atomic_batches": False,
        "packages": [
            {
                "package": "@zack/m14bc-memory",
                "version": "0.1.0",
                "ready": True
            }
        ]
    }

def record_from_request(req, record_id):
    now = datetime.now(timezone.utc).isoformat()
    return {
        "id": record_id,
        "package": req["package"],
        "package_version": req["package_version"],
        "space": req["space"],
        "space_model": req["space_model"],
        "record_type": req["record_type"],
        "schema_version": req["schema_version"],
        "scope_json": json.dumps(req.get("scope", {}), sort_keys=True, separators=(",", ":")),
        "scope_hash": "sha256:m14f-custom-runtime",
        "content": req.get("content") or {},
        "created_at": now,
        "updated_at": now,
        "expires_at": None,
        "archived_at": None,
        "ordinal": None,
        "provenance": req.get("provenance", {})
    }

for line in sys.stdin:
    msg = json.loads(line)
    write_log({"kind": msg.get("kind"), "method": msg.get("method"), "payload": msg.get("payload")})

    if msg.get("kind") == "initialize":
        emit(msg, "initialized", {
            "ready": True,
            "registry_id": REGISTRY_ID,
            "protocol_version": 1,
            "capabilities": capabilities(),
        })
        continue

    method = msg.get("method")
    payload = msg.get("payload", {})
    req = payload.get("request", {})

    if method == "write":
        record_id = req.get("record_id") or f"m14f-custom-{len(STATE) + 1}"
        record = record_from_request(req, record_id)
        STATE[record_id] = record
        emit(msg, "response", {
            "ok": True,
            "package": req["package"],
            "package_version": req["package_version"],
            "space": req["space"],
            "operation": req.get("operation", "create"),
            "record_id": record_id,
            "record": record,
        })
        continue

    if method == "read":
        records = list(STATE.values())
        emit(msg, "response", {
            "ok": True,
            "package": req["package"],
            "package_version": req["package_version"],
            "space": req["space"],
            "mode": req.get("mode", "key"),
            "records": records[: req.get("limit", len(records))],
            "count": len(records),
        })
        continue

    emit(msg, "error", error={"code": "unsupported_method", "message": f"unsupported method {method}"})
''')
(work / "runtime" / "m14f_memory_runtime_service.py").chmod(0o755)

def write_wrapper(name, exports, target):
    lines = ["#!/usr/bin/env bash", "set -euo pipefail"]
    for key, value in exports.items():
        lines.append(f'export {key}="{value}"')
    lines.append(f'exec "{os.environ.get("AGENTPM_MANUAL_PYTHON", "python3")}" "{target}"')
    path = work / "runtime" / name
    path.write_text("\n".join(lines) + "\n")
    path.chmod(0o755)
    return name

def hook_wrapper(mode, registry_id):
    return write_wrapper(
        f"m14f_{registry_id}_{mode}.sh",
        {
            "M14F_HOOK_MODE": mode,
            "M14F_HOOK_REGISTRY_ID": registry_id,
            "M14F_HOOK_LOG": f"runtime/m14f-{registry_id}-{mode}-hook-calls.jsonl",
        },
        "runtime/m14f_hook_service.py",
    )

def memory_wrapper(registry_id):
    return write_wrapper(
        f"m14f_{registry_id}.sh",
        {
            "M14F_MEMORY_REGISTRY_ID": registry_id,
            "M14F_MEMORY_LOG": f"runtime/m14f-{registry_id}-memory-calls.jsonl",
        },
        "runtime/m14f_memory_runtime_service.py",
    )

def common_config(state_dir):
    data = deepcopy(base)
    data.setdefault("runtime", {})["state_dir"] = state_dir
    data.setdefault("trace", {})["enabled"] = True
    data["trace"]["level"] = "verbose"
    data["trace"]["content"] = "full"
    data.setdefault("model", {})["model"] = model
    data["hooks"] = {"implementations": {}, "bindings": []}
    return data

def add_hook(data, impl_id, wrapper, hook, failure_policy="closed"):
    data["hooks"]["implementations"][impl_id] = {
        "implementation": {
            "type": "process",
            "command": "runtime/" + wrapper,
            "args": [],
            "cwd": ".",
            "env": [],
            "startup_timeout_ms": 10000,
            "request_timeout_ms": 30000,
            "restart": {"max_attempts": 0, "backoff_ms": 0},
        }
    }
    data["hooks"]["bindings"].append({
        "hook": hook,
        "implementation": impl_id,
        "failure_policy": failure_policy,
    })

def add_custom_memory(data, runtime_id, wrapper):
    memory = data.setdefault("memory", {})
    memory.setdefault("runtimes", {})[runtime_id] = {
        "implementation": {
            "type": "process",
            "command": "runtime/" + wrapper,
            "args": [],
            "cwd": ".",
            "env": [],
            "startup_timeout_ms": 10000,
            "request_timeout_ms": 30000,
            "restart": {"max_attempts": 0, "backoff_ms": 0},
        }
    }
    memory.setdefault("packages", {})["@zack/m14bc-memory"] = {"runtime": runtime_id}

def write_config(filename, data):
    (work / filename).write_text(json.dumps(data, indent=2) + "\n")

cfg = common_config(".agentpm-state-m14f-write-hook")
add_hook(cfg, "m14f-write-hook", hook_wrapper("write_patch", "m14f-write-hook"), "before_memory_write")
write_config("agentpm.openai.m14f.write-hook.harness.json", cfg)

cfg = common_config(".agentpm-state-m14f-write-hook")
add_hook(cfg, "m14f-read-hook", hook_wrapper("read_patch", "m14f-read-hook"), "before_memory_read")
write_config("agentpm.openai.m14f.read-hook.harness.json", cfg)

cfg = common_config(".agentpm-state-m14f-read-reject")
add_hook(cfg, "m14f-read-failure", hook_wrapper("read_failure", "m14f-read-failure"), "before_memory_read", "continue")
add_hook(cfg, "m14f-read-reject", hook_wrapper("read_reject", "m14f-read-reject"), "before_memory_read", "closed")
write_config("agentpm.openai.m14f.read-reject.harness.json", cfg)

cfg = common_config(".agentpm-state-m14f-read-bad-mode")
add_hook(cfg, "m14f-read-bad-mode", hook_wrapper("read_bad_mode", "m14f-read-bad-mode"), "before_memory_read")
write_config("agentpm.openai.m14f.read-bad-mode.harness.json", cfg)

cfg = common_config(".agentpm-state-m14f-custom-runtime")
add_hook(cfg, "m14f-custom-write-hook", hook_wrapper("custom_write_patch", "m14f-custom-write-hook"), "before_memory_write")
add_custom_memory(cfg, "m14f-custom-memory", memory_wrapper("m14f-custom-memory"))
write_config("agentpm.openai.m14f.custom-runtime-hook.harness.json", cfg)

(work / "inputs" / "m14f-write-note.txt").write_text('''Use direct Memory package @zack/m14bc-memory.

Create exactly one record in the notes space using memory_write operation create and record_type note:

{
  "body": "M14f original note",
  "status": "open",
  "labels": ["m14f", "original"],
  "scratch": { "public": "m14f original public scratch", "private": "m14f-ephemeral-secret" }
}

After exactly one Memory write completes, complete with outcome done and summarize the created record id. Do not call memory_write more than once. Do not use external knowledge or tools.
''')

(work / "inputs" / "m14f-read-notes.txt").write_text('''Use direct Memory package @zack/m14bc-memory.

Run one memory_read against the notes space:
- mode: filter
- record_type: note
- limit: 3
- filter: status equals "open"

Then complete with outcome done and summarize the returned record bodies in order. Do not use external knowledge or tools.
''')
PY

cat > "$WORK/scripts/m14f_assert_report.py" <<'PY'
#!/usr/bin/env python3
import json
import sqlite3
import sys
from pathlib import Path

if len(sys.argv) < 3:
    print("usage: m14f_assert_report.py <report.json> <check>...", file=sys.stderr)
    sys.exit(2)

report = json.loads(Path(sys.argv[1]).read_text())
events_path = Path(report["trace_path"])
events = [json.loads(line) for line in events_path.read_text().splitlines() if line.strip()]
types = [event.get("event_type") for event in events]

def index_of(event_type):
    try:
        return types.index(event_type)
    except ValueError:
        return -1

def all_text():
    return json.dumps({"report": report, "events": events}, sort_keys=True)

def durable_text():
    payloads = [report.get("terminal_output")]
    for event in events:
        if event.get("event_type") in ("memory_write_completed", "memory_read_completed"):
            payloads.append(event.get("payload", {}).get("fields", {}).get("result"))
    return json.dumps(payloads, sort_keys=True)

def hook_event(hook, event_type):
    for event in events:
        if event.get("event_type") != event_type:
            continue
        fields = event.get("payload", {}).get("fields", {})
        if fields.get("hook") == hook:
            return fields
    return None

for check in sys.argv[2:]:
    if check == "completed":
        assert report["terminal_status"] == "ended", report["terminal_status"]
    elif check == "failed":
        assert report["terminal_status"] == "failed", report["terminal_status"]
    elif check == "memory-write-completed":
        assert "memory_write_completed" in types, types
    elif check == "memory-read-completed":
        assert "memory_read_completed" in types, types
    elif check == "no-memory-read-started":
        assert "memory_read_started" not in types, types
    elif check == "hook-write-before-memory":
        assert index_of("hook_started") >= 0 and index_of("memory_write_started") >= 0, types
        assert index_of("hook_started") < index_of("memory_write_started"), types
    elif check == "hook-read-before-memory":
        assert index_of("hook_started") >= 0 and index_of("memory_read_started") >= 0, types
        assert index_of("hook_started") < index_of("memory_read_started"), types
    elif check == "hook-completed":
        assert "hook_completed" in types, types
    elif check == "hook-rejected":
        assert "hook_rejected" in types, types
    elif check == "hook-failed":
        assert "hook_failed" in types, types
    elif check == "nonfatal-before-reject":
        failed = index_of("hook_failed")
        rejected = index_of("hook_rejected")
        assert failed >= 0 and rejected >= 0 and failed < rejected, types
        fields = hook_event("before_memory_read", "hook_failed")
        assert fields and fields.get("nonfatal") is True, fields
    elif check == "patched-process-body":
        assert "M14f hooked process note" in durable_text(), durable_text()
    elif check == "patched-custom-body":
        assert "M14f hooked custom runtime note" in durable_text(), durable_text()
    elif check == "no-durable-secret":
        assert "m14f-ephemeral-secret" not in durable_text(), "secret leaked into durable Memory result"
    elif check == "custom-runtime":
        assert "m14f-custom-memory" in all_text(), all_text()[:1000]
    else:
        raise AssertionError(f"unknown check {check}")
PY
chmod +x "$WORK/scripts/m14f_assert_report.py"

cat > "$WORK/scripts/m14f_assert_runtime_log.py" <<'PY'
#!/usr/bin/env python3
import json
import sys
from pathlib import Path

if len(sys.argv) < 4:
    print("usage: m14f_assert_runtime_log.py <workspace> <fragment> <check>...", file=sys.stderr)
    sys.exit(2)

workspace = Path(sys.argv[1])
fragment = sys.argv[2]
paths = sorted((workspace / "runtime").glob(f"m14f-*{fragment}*.jsonl"))
assert paths, f"no runtime logs matching {fragment}"
entries = []
for path in paths:
    entries.extend(json.loads(line) for line in path.read_text().splitlines() if line.strip())
text = json.dumps(entries, sort_keys=True)

for check in sys.argv[3:]:
    if check == "saw-initialize":
        assert '"kind": "initialize"' in text or '"kind":"initialize"' in text, text
    elif check == "saw-before-memory-write":
        assert "before_memory_write" in text, text
    elif check == "saw-before-memory-read":
        assert "before_memory_read" in text, text
    elif check == "saw-memory-write":
        assert '"method": "write"' in text or '"method":"write"' in text, text
    elif check == "saw-memory-read":
        assert '"method": "read"' in text or '"method":"read"' in text, text
    elif check == "patched-custom-body":
        assert "M14f hooked custom runtime note" in text, text
    elif check == "no-secret":
        assert "m14f-ephemeral-secret" not in text, text
    else:
        raise AssertionError(f"unknown check {check}")
PY
chmod +x "$WORK/scripts/m14f_assert_runtime_log.py"

cat > "$WORK/scripts/m14f_sqlite_count.py" <<'PY'
#!/usr/bin/env python3
import sqlite3
import sys
from pathlib import Path

if len(sys.argv) != 2:
    print("usage: m14f_sqlite_count.py <state-dir>", file=sys.stderr)
    sys.exit(2)

db = Path(sys.argv[1]) / "memory.sqlite3"
if not db.exists():
    print("0")
    sys.exit(0)

conn = sqlite3.connect(db)
try:
    print(conn.execute("select count(*) from memory_records").fetchone()[0])
except sqlite3.Error:
    print("0")
PY
chmod +x "$WORK/scripts/m14f_sqlite_count.py"

echo "M14f manual extension ready."
echo
echo "Workspace: $WORK"
echo "Run output: $RUNS"
SH

bash /tmp/setup-harness-m14f-manual.sh
export M14F_WORK="$HARNESS_M14BC_TEST_BASE/workspace"
export M14F_RUNS="$HARNESS_M14BC_TEST_BASE/runs-m14f"
```

## Test 1: `before_memory_write` patch applies before local mutation

Use a fresh case id for each Test 1 attempt, then keep that same value for Test 2. Reusing an old `notes` scope can hit the M14b capacity limit after three successful writes.

```bash
export M14F_HOOK_CASE="$(date +%s)"
REPORT="$M14F_RUNS/hooks/write-hook-report.json"
(cd "$M14F_WORK" && "$APM" harness \
  --config agentpm.openai.m14f.write-hook.harness.json \
  --headless \
  --scope user="m14f-hook-user-$M14F_HOOK_CASE" \
  --scope conversation="m14f-hook-conversation-$M14F_HOOK_CASE" \
  --input-file inputs/m14f-write-note.txt \
  --report "$REPORT") || true

"$AGENTPM_MANUAL_PYTHON" "$M14F_WORK/scripts/m14f_assert_report.py" \
  "$REPORT" \
  memory-write-completed hook-completed hook-write-before-memory patched-process-body no-durable-secret

"$AGENTPM_MANUAL_PYTHON" "$M14F_WORK/scripts/m14f_assert_runtime_log.py" \
  "$M14F_WORK" write-hook saw-initialize saw-before-memory-write
```

Expected:

- trace contains at least one successful `memory_write_completed`;
- trace contains `hook_started`, `hook_completed`, then `memory_write_started`;
- durable Memory result contains `M14f hooked process note`;
- durable Memory result does not contain `m14f-ephemeral-secret`.

The run may end as `ended`, `failed`, or `limit_reached` if the live model repeats the write instead of selecting phase completion. That is model nondeterminism in the manual scenario. For this test, the pass/fail signal is the first successful Hook-patched write before any later model wander.

If the assertion fails with only `memory_write_failed` events, the selected scope is already full. Rerun Test 1 from the first line so `M14F_HOOK_CASE` gets a new value.

The Hook log may contain the secret because `before_memory_write` sees the pre-projection request. The no-secret boundary for M14f is the durable/local Memory result and any custom runtime wire request.

## Test 2: `before_memory_read` patch applies before local dispatch

Run this after Test 1 without changing `M14F_HOOK_CASE`, so the patched record exists in the same Memory scope.

```bash
REPORT="$M14F_RUNS/hooks/read-hook-report.json"
(cd "$M14F_WORK" && "$APM" harness \
  --config agentpm.openai.m14f.read-hook.harness.json \
  --headless \
  --scope user="m14f-hook-user-$M14F_HOOK_CASE" \
  --scope conversation="m14f-hook-conversation-$M14F_HOOK_CASE" \
  --input-file inputs/m14f-read-notes.txt \
  --report "$REPORT")

"$AGENTPM_MANUAL_PYTHON" "$M14F_WORK/scripts/m14f_assert_report.py" \
  "$REPORT" \
  completed memory-read-completed hook-completed hook-read-before-memory patched-process-body

"$AGENTPM_MANUAL_PYTHON" "$M14F_WORK/scripts/m14f_assert_runtime_log.py" \
  "$M14F_WORK" read-hook saw-initialize saw-before-memory-read
```

Expected:

- trace contains `hook_started`, `hook_completed`, then `memory_read_started`;
- returned bodies include `M14f hooked process note`;
- the read Hook patch is applied to the active `notes` space without changing package/space/scope/envelope authority fields.

## Test 3: read Hook rejection fails closed and drains queued nonfatal failures first

```bash
REPORT="$M14F_RUNS/hooks/read-reject-report.json"
(cd "$M14F_WORK" && "$APM" harness \
  --config agentpm.openai.m14f.read-reject.harness.json \
  --headless \
  --scope user=m14f-read-reject-user \
  --scope conversation=m14f-read-reject-conversation \
  --input-file inputs/m14f-read-notes.txt \
  --report "$REPORT") || true

"$AGENTPM_MANUAL_PYTHON" "$M14F_WORK/scripts/m14f_assert_report.py" \
  "$REPORT" \
  failed hook-failed hook-rejected nonfatal-before-reject no-memory-read-started
```

Expected:

- report terminal status is `failed`;
- trace contains a nonfatal `hook_failed` for `before_memory_read`;
- trace then contains `hook_rejected` for `before_memory_read`;
- no `memory_read_started` event appears.

## Test 4: read Hook invalid patch is revalidated before dispatch

```bash
REPORT="$M14F_RUNS/hooks/read-bad-mode-report.json"
(cd "$M14F_WORK" && "$APM" harness \
  --config agentpm.openai.m14f.read-bad-mode.harness.json \
  --headless \
  --scope user=m14f-read-bad-mode-user \
  --scope conversation=m14f-read-bad-mode-conversation \
  --input-file inputs/m14f-read-notes.txt \
  --report "$REPORT") || true

"$AGENTPM_MANUAL_PYTHON" "$M14F_WORK/scripts/m14f_assert_report.py" \
  "$REPORT" \
  failed hook-failed no-memory-read-started
```

Expected:

- report terminal status is `failed`;
- trace contains `hook_failed`;
- no Memory read is dispatched after the Hook returns undeclared mode `not_a_declared_mode`.

## Test 5: Hook patch plus custom MemoryRuntime projects content before the wire

```bash
  export M14F_CUSTOM_CASE="$(date +%s)"
  REPORT="$M14F_RUNS/custom-runtime/write-hook-custom-runtime-report.json"
  (cd "$M14F_WORK" && "$APM" harness \
    --config agentpm.openai.m14f.custom-runtime-hook.harness.json \
    --headless \
    --scope user="m14f-custom-runtime-user-$M14F_CUSTOM_CASE" \
    --scope conversation="m14f-custom-runtime-conversation-$M14F_CUSTOM_CASE" \
    --input-file inputs/m14f-write-note.txt \
    --report "$REPORT") || true

"$AGENTPM_MANUAL_PYTHON" "$M14F_WORK/scripts/m14f_assert_report.py" \
  "$REPORT" \
  memory-write-completed hook-completed hook-write-before-memory custom-runtime patched-custom-body no-durable-secret

"$AGENTPM_MANUAL_PYTHON" "$M14F_WORK/scripts/m14f_assert_runtime_log.py" \
  "$M14F_WORK" custom-memory saw-initialize saw-memory-write patched-custom-body no-secret

"$AGENTPM_MANUAL_PYTHON" "$M14F_WORK/scripts/m14f_sqlite_count.py" \
  "$M14F_WORK/.agentpm-state-m14f-custom-runtime"
```

Expected:

- trace shows Hook execution before custom MemoryRuntime write dispatch;
- custom MemoryRuntime log contains `M14f hooked custom runtime note`;
- custom MemoryRuntime log does not contain `m14f-ephemeral-secret`;
- SQLite local record count is `0` or no DB exists, proving the mapped package did not fall back to local SQLite.

The run may end as `ended`, `failed`, or `limit_reached` if the live model repeats the write or fails to select phase completion after the successful custom MemoryRuntime dispatch. For this test, the pass/fail signal is the first Hook-patched custom runtime write and the no-secret wire-boundary assertion.

## Test 6: SDK focused Memory Hook contract checks

Node SDK:

```bash
(cd "$AGENTPM_PROJECT_ROOT/agentpm-sdk-node" && pnpm exec vitest run test/harness.spec.ts) \
  | tee "$M14F_RUNS/sdk/node-harness.txt"
```

Python SDK:

```bash
(cd "$AGENTPM_PROJECT_ROOT/agentpm-sdk-python" && uv run pytest tests/test_harness.py) \
  | tee "$M14F_RUNS/sdk/python-harness.txt"
```

Expected:

- both SDK test commands pass;
- the SDK Harness tests include typed Memory Hook registration for:
  - `before_memory_read`
  - `before_memory_write`
  - `before_memory_operation`
- the registered service capabilities advertise those hook IDs.

These are SDK contract smoke tests, not live host-bridge Harness runs. If a later milestone adds manual host-bridge coverage, keep that separate from this M14f pass.

## Test 7: focused automated M14f regression checks

```bash
cargo test -p agentpm-cli harness_engine::tests::memory -- --nocapture \
  | tee "$M14F_RUNS/rust/harness-engine-memory.txt"

cargo test -p agentpm-cli harness_runtime::hook -- --nocapture \
  | tee "$M14F_RUNS/rust/harness-runtime-hook.txt"

cargo fmt --all --check
```

Expected:

- Memory engine tests pass, including read/write Hook patch/rejection/revalidation and custom-runtime projection cases;
- Hook runtime tests pass, including typed Hook decision decoding and patch authority boundaries;
- formatter check passes.

## Cleanup

The setup writes only inside the existing M14b/M14c manual fixture plus `/tmp/setup-harness-m14f-manual.sh`.

```bash
rm -f /tmp/setup-harness-m14f-manual.sh
```

To remove all generated M14f manual state:

```bash
rm -rf "$M14F_RUNS" \
  "$M14F_WORK/.agentpm-state-m14f-write-hook" \
  "$M14F_WORK/.agentpm-state-m14f-read-reject" \
  "$M14F_WORK/.agentpm-state-m14f-read-bad-mode" \
  "$M14F_WORK/.agentpm-state-m14f-custom-runtime"
```
