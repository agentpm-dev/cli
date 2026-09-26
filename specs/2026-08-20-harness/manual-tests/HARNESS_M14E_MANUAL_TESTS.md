# Harness Milestone 14e Manual Tests

Manual coverage for Milestone 14e: generic custom process/host `MemoryRuntime` activation, capability enforcement, routed direct Memory actions, durable-governance boundaries, typed backend failures, and explicit no-fallback behavior.

This manual pass extends the M14b/M14c Memory fixture because M14e is an external-runtime routing layer over the direct Memory action surface. It uses fake/conformance `MemoryRuntime` services only. PostgreSQL, Redis, Memory Hooks, SDK parity, and persistence review are intentionally out of scope for this milestone.

## Coverage target

| Requirement | Manual coverage |
|---|---|
| `memory.packages` routes mapped packages to a custom runtime | process runtime happy path read/write |
| explicit custom mapping never falls back to SQLite | suppressed/failed custom runtime with empty local SQLite state |
| custom process startup/handshake/readiness | `service_*` trace events and preflight/runtime readiness |
| `ready:false` and malformed descriptors reject exposure | dedicated fake runtime modes |
| package/version/Blueprint realization mismatch rejects exposure | dedicated fake runtime modes |
| retrieval-mode capability subset degrades direct read modes | key-only custom runtime removes `filter` / `chronological` / `full_text` |
| lifecycle-only capability gaps do not suppress direct access | descriptor omits durable trigger state / atomic batches while direct key access remains available |
| durable projection is applied before custom dispatch | runtime request log excludes `x-agentpm-persist:false` values |
| custom backend failure codes map like local SQLite | fake `not_found`, `record_type_mismatch`, and `capacity_exceeded` result modes |
| one bad runtime does not disable a healthy sibling | one mapped package suppressed while another remains available |
| host MemoryRuntime unavailable state is visible | host mapping without registration suppresses the mapped package |

Provider behavior can vary. If a live model does not select the explicitly requested Memory action, keep the report/trace and rerun that scenario before treating it as a Harness regression.

## Prerequisites

From the root of the `agentpm` repo:

```bash
cargo build -p agentpm-cli
export APM="$PWD/target/debug/agentpm"
export AGENTPM_MANUAL_PYTHON="${AGENTPM_MANUAL_PYTHON:-python3}"
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

Then apply the M14e custom runtime extension:

```bash
cat > /tmp/setup-harness-m14e-manual.sh <<'SH'
#!/usr/bin/env bash
set -euo pipefail

ROOT="$(pwd)"
APM="${APM:-$ROOT/target/debug/agentpm}"
BASE="${HARNESS_M14BC_TEST_BASE:-$ROOT/harness-m14bc-test}"
WORK="$BASE/workspace"
RUNS="$BASE/runs-m14e"
PYTHON_CMD="${AGENTPM_MANUAL_PYTHON:-python3}"
MEMORY_MANIFEST="$WORK/.agentpm/memory/zack/m14bc-memory/0.1.0/agent.json"

if [ ! -x "$APM" ]; then
  echo "Missing agentpm binary at $APM. Run: cargo build -p agentpm-cli" >&2
  exit 1
fi

if [ ! -f "$MEMORY_MANIFEST" ]; then
  echo "Missing M14b/M14c fixture. Run HARNESS_M14B_M14C_MANUAL_TESTS.md setup first." >&2
  exit 1
fi

mkdir -p "$WORK/runtime" "$WORK/inputs" "$WORK/scripts" "$RUNS"/{process,capabilities,failures,host,isolation}

cat > "$WORK/runtime/m14e_memory_runtime_service.py" <<'PY'
#!/usr/bin/env python3
import json
import os
import sys
from pathlib import Path
from datetime import datetime, timezone

LOG_PATH = Path(os.environ.get("M14E_MEMORY_LOG", "runtime/m14e-memory-runtime-calls.jsonl"))
MODE = os.environ.get("M14E_MEMORY_MODE", "ok")
REGISTRY_ID = os.environ.get("M14E_MEMORY_REGISTRY_ID", "m14e-memory-runtime")
PACKAGE = os.environ.get("M14E_MEMORY_PACKAGE", "@zack/m14bc-memory")
VERSION = os.environ.get("M14E_MEMORY_VERSION", "0.1.0")
RETRIEVAL_MODES = [
    mode.strip()
    for mode in os.environ.get("M14E_MEMORY_RETRIEVAL_MODES", "key,filter,chronological,full_text").split(",")
    if mode.strip()
]
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
    package = PACKAGE
    version = VERSION
    if MODE == "package_mismatch":
        package = "@zack/wrong-memory"
    if MODE == "version_mismatch":
        version = "9.9.9"
    space_models = ["document", "collection", "sequence"]
    if MODE == "blueprint_mismatch":
        space_models = ["document"]
    return {
        "space_models": space_models,
        "retrieval_modes": RETRIEVAL_MODES,
        "retention_actions": ["archive", "delete"],
        "constraints": ["append_only"],
        "capacity": True,
        "durable_trigger_state": False,
        "atomic_batches": False,
        "packages": [
            {
                "package": package,
                "version": version,
                "ready": MODE != "package_not_ready",
                "reason": "manual package not ready" if MODE == "package_not_ready" else None
            }
        ]
    }

def record_from_request(req, record_id):
    now = req.get("now") or datetime.now(timezone.utc).isoformat()
    return {
        "id": record_id,
        "package": req["package"],
        "package_version": req["package_version"],
        "space": req["space"],
        "space_model": req["space_model"],
        "record_type": req["record_type"],
        "schema_version": req["schema_version"],
        "scope_json": json.dumps(req.get("scope", {}), sort_keys=True, separators=(",", ":")),
        "scope_hash": "sha256:manual-custom-runtime",
        "content": req.get("content") or {},
        "created_at": now,
        "updated_at": now,
        "expires_at": None,
        "archived_at": None,
        "ordinal": None,
        "provenance": req.get("provenance", {})
    }

def failure_result(req, operation=None):
    code = MODE.replace("fail_", "")
    message = f"manual custom runtime {code}"
    base = {
        "ok": False,
        "package": req.get("package", PACKAGE),
        "package_version": req.get("package_version", VERSION),
        "space": req.get("space", "notes"),
        "error": {"code": code, "message": message}
    }
    if operation:
        base["operation"] = operation
    else:
        base["mode"] = req.get("mode", "key")
        base["records"] = []
        base["count"] = 0
    return base

for line in sys.stdin:
    msg = json.loads(line)
    write_log({"kind": msg.get("kind"), "method": msg.get("method"), "payload": msg.get("payload")})

    if msg.get("kind") == "initialize":
        if MODE == "malformed_descriptor":
            emit(msg, "initialized", {"ready": True, "registry_id": REGISTRY_ID, "protocol_version": 1})
            continue
        emit(msg, "initialized", {
            "ready": MODE != "ready_false",
            "registry_id": REGISTRY_ID,
            "protocol_version": 1,
            "capabilities": capabilities(),
            "reason": "manual ready false" if MODE == "ready_false" else None
        })
        continue

    method = msg.get("method")
    payload = msg.get("payload", {})
    req = payload.get("request", {})

    if method == "write":
        operation = req.get("operation", "create")
        if MODE.startswith("fail_"):
            emit(msg, "response", failure_result(req, operation))
            continue
        record_id = req.get("record_id") or f"custom-{len(STATE) + 1}"
        record = record_from_request(req, record_id)
        if operation in ("delete", "archive"):
            STATE.pop(record_id, None)
        else:
            STATE[record_id] = record
        emit(msg, "response", {
            "ok": True,
            "package": req["package"],
            "package_version": req["package_version"],
            "space": req["space"],
            "operation": operation,
            "record_id": record_id,
            "record": record
        })
        continue

    if method == "read":
        if MODE.startswith("fail_"):
            emit(msg, "response", failure_result(req))
            continue
        records = list(STATE.values())
        if not records:
            records = [
                {
                    "id": "custom-seeded-1",
                    "package": req["package"],
                    "package_version": req["package_version"],
                    "space": req["space"],
                    "space_model": "collection",
                    "record_type": req.get("record_type") or "note",
                    "schema_version": "1.0.0",
                    "scope_json": json.dumps(req.get("scope", {}), sort_keys=True, separators=(",", ":")),
                    "scope_hash": "sha256:manual-custom-runtime",
                    "content": {
                        "body": "Custom runtime seeded note",
                        "status": "open",
                        "labels": ["custom", "m14e"]
                    },
                    "created_at": req.get("now"),
                    "updated_at": req.get("now"),
                    "expires_at": None,
                    "archived_at": None,
                    "ordinal": None,
                    "provenance": {"manual": {"runtime": REGISTRY_ID}}
                }
            ]
        limit = req.get("limit")
        if isinstance(limit, int):
            records = records[:limit]
        emit(msg, "response", {
            "ok": True,
            "package": req["package"],
            "package_version": req["package_version"],
            "space": req["space"],
            "mode": req.get("mode", "key"),
            "records": records,
            "count": len(records)
        })
        continue

    emit(msg, "error", error={"code": "unsupported_method", "message": f"unsupported method {method}"})
PY
chmod +x "$WORK/runtime/m14e_memory_runtime_service.py"

write_process_config() {
  local target="$1"
  local state_dir="$2"
  local runtime_id="$3"
  local mode="$4"
  local retrieval_modes="${5:-key,filter,chronological,full_text}"
  local wrapper="$WORK/runtime/m14e_${runtime_id}_${mode}.sh"
  cat > "$wrapper" <<WRAPPER_SH
#!/usr/bin/env bash
set -euo pipefail
export M14E_MEMORY_MODE="$mode"
export M14E_MEMORY_REGISTRY_ID="$runtime_id"
export M14E_MEMORY_PACKAGE="@zack/m14bc-memory"
export M14E_MEMORY_VERSION="0.1.0"
export M14E_MEMORY_RETRIEVAL_MODES="$retrieval_modes"
export M14E_MEMORY_LOG="runtime/m14e-${runtime_id}-${mode}-calls.jsonl"
exec "$PYTHON_CMD" "runtime/m14e_memory_runtime_service.py"
WRAPPER_SH
  chmod +x "$wrapper"
  "$PYTHON_CMD" - "$WORK/agentpm.openai.harness.json" "$target" "$state_dir" "$runtime_id" "$(basename "$wrapper")" <<'PY'
import json
import os
import sys
from pathlib import Path

source = Path(sys.argv[1])
target = Path(sys.argv[2])
state_dir = sys.argv[3]
runtime_id = sys.argv[4]
wrapper = sys.argv[5]

data = json.loads(source.read_text())
data.setdefault("runtime", {})["state_dir"] = state_dir
data.setdefault("trace", {})["enabled"] = True
data["trace"]["level"] = "verbose"
data["trace"]["content"] = "full"
data.setdefault("model", {})["model"] = os.environ.get("AGENTPM_MANUAL_OPENAI_MODEL", data.get("model", {}).get("model", "gpt-4o-mini"))
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
        "restart": {"max_attempts": 0, "backoff_ms": 0}
    }
}
memory.setdefault("packages", {})["@zack/m14bc-memory"] = {"runtime": runtime_id}
target.write_text(json.dumps(data, indent=2) + "\n")
PY
}

write_host_config() {
  local target="$1"
  local state_dir="$2"
  "$PYTHON_CMD" - "$WORK/agentpm.openai.harness.json" "$target" "$state_dir" <<'PY'
import json
import os
import sys
from pathlib import Path

source = Path(sys.argv[1])
target = Path(sys.argv[2])
state_dir = sys.argv[3]
data = json.loads(source.read_text())
data.setdefault("runtime", {})["state_dir"] = state_dir
data.setdefault("trace", {})["enabled"] = True
data["trace"]["level"] = "verbose"
data["trace"]["content"] = "full"
data.setdefault("model", {})["model"] = os.environ.get("AGENTPM_MANUAL_OPENAI_MODEL", data.get("model", {}).get("model", "gpt-4o-mini"))
memory = data.setdefault("memory", {})
memory.setdefault("runtimes", {})["m14e-host-memory"] = {
    "implementation": {
        "type": "host",
        "request_timeout_ms": 30000
    }
}
memory.setdefault("packages", {})["@zack/m14bc-memory"] = {"runtime": "m14e-host-memory"}
target.write_text(json.dumps(data, indent=2) + "\n")
PY
}

write_process_config "$WORK/agentpm.openai.m14e.process.harness.json" ".agentpm-state-m14e-process" "m14e-process-memory" "ok"
write_process_config "$WORK/agentpm.openai.m14e.key-only.harness.json" ".agentpm-state-m14e-key-only" "m14e-key-only-memory" "ok" "key"
write_process_config "$WORK/agentpm.openai.m14e.ready-false.harness.json" ".agentpm-state-m14e-ready-false" "m14e-ready-false-memory" "ready_false"
write_process_config "$WORK/agentpm.openai.m14e.malformed.harness.json" ".agentpm-state-m14e-malformed" "m14e-malformed-memory" "malformed_descriptor"
write_process_config "$WORK/agentpm.openai.m14e.version-mismatch.harness.json" ".agentpm-state-m14e-version-mismatch" "m14e-version-mismatch-memory" "version_mismatch"
write_process_config "$WORK/agentpm.openai.m14e.blueprint-mismatch.harness.json" ".agentpm-state-m14e-blueprint-mismatch" "m14e-blueprint-mismatch-memory" "blueprint_mismatch"
write_process_config "$WORK/agentpm.openai.m14e.fail-not-found.harness.json" ".agentpm-state-m14e-fail-not-found" "m14e-fail-not-found-memory" "fail_not_found"
write_process_config "$WORK/agentpm.openai.m14e.fail-record-type.harness.json" ".agentpm-state-m14e-fail-record-type" "m14e-fail-record-type-memory" "fail_record_type_mismatch"
write_process_config "$WORK/agentpm.openai.m14e.fail-capacity.harness.json" ".agentpm-state-m14e-fail-capacity" "m14e-fail-capacity-memory" "fail_capacity_exceeded"
write_host_config "$WORK/agentpm.openai.m14e.host-unregistered.harness.json" ".agentpm-state-m14e-host-unregistered"

cat > "$WORK/inputs/m14e-write-note.txt" <<'TXT'
Use direct Memory package @zack/m14bc-memory.

Create exactly one record in the notes space using memory_write operation create and record_type note:

{
  "body": "M14e custom runtime durable note",
  "status": "open",
  "labels": ["m14e", "custom"],
  "scratch": { "public": "visible scratch", "private": "m14e-ephemeral-secret" }
}

After the Memory write completes, complete with outcome done and summarize the created record id. Do not use external knowledge or tools.
TXT

cat > "$WORK/inputs/m14e-read-notes.txt" <<'TXT'
Use direct Memory package @zack/m14bc-memory.

Run one memory_read against the notes space:
- mode: filter
- record_type: note
- limit: 3
- filter: status equals "open"

Then complete with outcome done and summarize the returned record bodies in order. Do not use external knowledge or tools.
TXT

cat > "$WORK/inputs/m14e-read-key-only.txt" <<'TXT'
Use direct Memory package @zack/m14bc-memory.

Run one memory_read against the notes space:
- mode: key
- record_id: custom-seeded-1

Then complete with outcome done and summarize whether a custom runtime record was returned. Do not use external knowledge or tools.
TXT

cat > "$WORK/inputs/m14e-read-missing-key.txt" <<'TXT'
Use direct Memory package @zack/m14bc-memory.

Run one memory_read against the notes space:
- mode: key
- record_id: missing-record-id

If Harness asks for a correction, complete with outcome done and summarize the failure. Do not use external knowledge or tools.
TXT

cat > "$WORK/scripts/m14e_assert_report.py" <<'PY'
#!/usr/bin/env python3
import json
import sys
from pathlib import Path

if len(sys.argv) < 3:
    print("usage: m14e_assert_report.py <report.json> <check>...", file=sys.stderr)
    sys.exit(2)

report = json.loads(Path(sys.argv[1]).read_text())
events_path = Path(report["trace_path"])
events = [json.loads(line) for line in events_path.read_text().splitlines() if line.strip()]

def event_types():
    return [event.get("event_type") for event in events]

def all_text():
    return json.dumps({"report": report, "events": events}, sort_keys=True)

def memory_surface(identity="@zack/m14bc-memory/notes"):
    for event in events:
        if event.get("event_type") != "memory_surface_ready":
            continue
        payload = event.get("payload", {})
        if payload.get("identity") == identity:
            return payload.get("fields", {})
    return None

for check in sys.argv[2:]:
    if check == "completed":
        assert report["terminal_status"] == "ended", report["terminal_status"]
    elif check == "memory-write-completed":
        assert "memory_write_completed" in event_types(), event_types()
    elif check == "memory-read-completed":
        assert "memory_read_completed" in event_types(), event_types()
    elif check == "service-ready":
        assert "service_ready" in event_types(), event_types()
    elif check == "service-failed-or-suppressed":
        types = event_types()
        assert any(t in types for t in ("service_failed", "service_unavailable", "memory_surface_unavailable")), types
    elif check == "runtime-process":
        fields = memory_surface()
        assert fields and fields.get("runtime", "").startswith("m14e-"), fields
    elif check == "no-filter-mode":
        fields = memory_surface()
        assert fields and "filter" not in fields.get("retrieval_modes", []), fields
    elif check == "key-mode":
        fields = memory_surface()
        assert fields and "key" in fields.get("retrieval_modes", []), fields
    elif check == "suppressed":
        text = all_text()
        assert "suppressed" in text or "unavailable" in text, text[:1000]
    elif check == "not-found-repair":
        assert report.get("repair_count", 0) >= 1 or "not_found" in all_text(), report
    elif check == "capacity-typed":
        assert "capacity_exceeded" in all_text(), all_text()[:1000]
    elif check == "record-type-typed":
        assert "record_type_mismatch" in all_text(), all_text()[:1000]
    elif check == "no-secret":
        durable_payloads = [report.get("terminal_output")]
        for event in events:
            if event.get("event_type") != "memory_write_completed":
                continue
            payload = event.get("payload", {})
            fields = payload.get("fields", {})
            durable_payloads.append(fields.get("result"))
        assert "m14e-ephemeral-secret" not in json.dumps(durable_payloads, sort_keys=True), "secret leaked into durable Memory result"
    else:
        raise AssertionError(f"unknown check {check}")
PY
chmod +x "$WORK/scripts/m14e_assert_report.py"

cat > "$WORK/scripts/m14e_assert_runtime_log.py" <<'PY'
#!/usr/bin/env python3
import json
import sys
from pathlib import Path

if len(sys.argv) < 3:
    print("usage: m14e_assert_runtime_log.py <workspace> <runtime-id-fragment> [check...]", file=sys.stderr)
    sys.exit(2)

workspace = Path(sys.argv[1])
fragment = sys.argv[2]
paths = sorted((workspace / "runtime").glob(f"m14e-*{fragment}*-calls.jsonl"))
assert paths, f"no runtime logs matching {fragment}"
entries = []
for path in paths:
    entries.extend(json.loads(line) for line in path.read_text().splitlines() if line.strip())
text = json.dumps(entries, sort_keys=True)

for check in sys.argv[3:]:
    if check == "saw-initialize":
        assert '"kind": "initialize"' in text or '"kind":"initialize"' in text, text
    elif check == "saw-read":
        assert '"method": "read"' in text or '"method":"read"' in text, text
    elif check == "saw-write":
        assert '"method": "write"' in text or '"method":"write"' in text, text
    elif check == "no-secret":
        assert "m14e-ephemeral-secret" not in text, text
    elif check == "custom-runtime":
        assert "m14e" in text and "@zack/m14bc-memory" in text, text
    else:
        raise AssertionError(f"unknown check {check}")
PY
chmod +x "$WORK/scripts/m14e_assert_runtime_log.py"

cat > "$WORK/scripts/m14e_sqlite_count.py" <<'PY'
#!/usr/bin/env python3
import sqlite3
import sys
from pathlib import Path

if len(sys.argv) != 2:
    print("usage: m14e_sqlite_count.py <state-dir>", file=sys.stderr)
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
chmod +x "$WORK/scripts/m14e_sqlite_count.py"

echo "M14e manual extension ready."
echo
echo "Workspace: $WORK"
echo "Run output: $RUNS"
SH

bash /tmp/setup-harness-m14e-manual.sh
export M14E_WORK="$HARNESS_M14BC_TEST_BASE/workspace"
export M14E_RUNS="$HARNESS_M14BC_TEST_BASE/runs-m14e"
```

## Test 1: process custom MemoryRuntime preflight is active

```bash
(cd "$M14E_WORK" && "$APM" harness --config agentpm.openai.m14e.process.harness.json --verbose) \
  | tee "$M14E_RUNS/process/preflight.txt"
```

Expected:

- preflight is `Ready`;
- static capability details include a configured memory runtime/provider entry;
- the notes Memory surface is exposed with `runtime: m14e-process-memory` in verbose trace/run events once a run starts.

## Test 2: process custom write routes to custom runtime and strips non-persistable fields

```bash
REPORT="$M14E_RUNS/process/write-report.json"
(cd "$M14E_WORK" && "$APM" harness \
  --config agentpm.openai.m14e.process.harness.json \
  --headless \
  --scope user=m14e-process-user \
  --scope conversation=m14e-process-conversation \
  --input-file inputs/m14e-write-note.txt \
  --report "$REPORT")

"$AGENTPM_MANUAL_PYTHON" "$M14E_WORK/scripts/m14e_assert_report.py" \
  "$REPORT" \
  completed memory-write-completed service-ready runtime-process no-secret

"$AGENTPM_MANUAL_PYTHON" "$M14E_WORK/scripts/m14e_assert_runtime_log.py" \
  "$M14E_WORK" process saw-initialize saw-write custom-runtime no-secret

"$AGENTPM_MANUAL_PYTHON" "$M14E_WORK/scripts/m14e_sqlite_count.py" \
  "$M14E_WORK/.agentpm-state-m14e-process"
```

Expected:

- report terminal status is `ended`;
- trace includes `service_ready` and `memory_write_completed`;
- runtime log includes `initialize` and `write`;
- durable Memory result and runtime log do not contain `m14e-ephemeral-secret`;
- with `trace.content: full`, the original user input/prompt may still contain `m14e-ephemeral-secret`; M14e is testing the persistence-governance/runtime-dispatch boundary, not final M14 redaction;
- SQLite local record count is `0` or no DB exists, proving the mapped package did not fall back to local SQLite.

## Test 3: process custom read routes to custom runtime

```bash
REPORT="$M14E_RUNS/process/read-report.json"
(cd "$M14E_WORK" && "$APM" harness \
  --config agentpm.openai.m14e.process.harness.json \
  --headless \
  --scope user=m14e-process-user \
  --scope conversation=m14e-process-conversation \
  --input-file inputs/m14e-read-notes.txt \
  --report "$REPORT")

"$AGENTPM_MANUAL_PYTHON" "$M14E_WORK/scripts/m14e_assert_report.py" \
  "$REPORT" \
  completed memory-read-completed service-ready runtime-process

"$AGENTPM_MANUAL_PYTHON" "$M14E_WORK/scripts/m14e_assert_runtime_log.py" \
  "$M14E_WORK" process saw-read custom-runtime
```

Expected:

- report terminal status is `ended`;
- trace includes `memory_read_completed`;
- runtime log includes a `read` request whose package is `@zack/m14bc-memory`.

## Test 4: retrieval-mode subset degrades direct modes without suppressing direct key access

```bash
REPORT="$M14E_RUNS/capabilities/key-only-report.json"
(cd "$M14E_WORK" && "$APM" harness \
  --config agentpm.openai.m14e.key-only.harness.json \
  --headless \
  --scope user=m14e-key-only-user \
  --scope conversation=m14e-key-only-conversation \
  --input-file inputs/m14e-read-key-only.txt \
  --report "$REPORT")

"$AGENTPM_MANUAL_PYTHON" "$M14E_WORK/scripts/m14e_assert_report.py" \
  "$REPORT" \
  completed memory-read-completed key-mode no-filter-mode
```

Expected:

- the notes surface remains available;
- retrieval modes include `key`;
- retrieval modes do not include `filter`;
- lifecycle-only gaps (`durable_trigger_state:false`, `atomic_batches:false`) do not suppress direct key access.

## Test 5: descriptor readiness and capability mismatches suppress custom Memory surfaces

Run the rejected descriptor cases:

```bash
for case in ready-false malformed version-mismatch blueprint-mismatch; do
  REPORT="$M14E_RUNS/capabilities/${case}-report.json"
  (cd "$M14E_WORK" && "$APM" harness \
    --config "agentpm.openai.m14e.${case}.harness.json" \
    --headless \
    --scope user="m14e-${case}-user" \
    --scope conversation="m14e-${case}-conversation" \
    --input-file inputs/m14e-read-notes.txt \
    --report "$REPORT") || true

  "$AGENTPM_MANUAL_PYTHON" "$M14E_WORK/scripts/m14e_assert_report.py" \
    "$REPORT" \
    suppressed service-failed-or-suppressed
done
```

Expected:

- preflight can still report `Status: Ready` because MemoryRuntime activation is runtime-scoped, not a static preflight failure;
- each run reports suppressed/unavailable Memory access instead of a successful custom read/write on the mapped surface;
- the mapped package does not use local SQLite as fallback;
- trace/report suppression reasons mention the relevant descriptor problem:
  - `ready:false`
  - malformed MemoryRuntime descriptor
  - package/version mismatch
  - Blueprint realization mismatch.

## Test 6: custom backend failures map to typed Memory failures

Run the documented custom runtime error-code cases:

```bash
REPORT="$M14E_RUNS/failures/not-found-report.json"

(cd "$M14E_WORK" && "$APM" harness \
  --config agentpm.openai.m14e.fail-not-found.harness.json \
  --headless \
  --scope user=m14e-fail-not-found-user \
  --scope conversation=m14e-fail-not-found-conversation \
  --input-file inputs/m14e-read-missing-key.txt \
  --report "$REPORT") || true
"$AGENTPM_MANUAL_PYTHON" "$M14E_WORK/scripts/m14e_assert_report.py" \
  "$REPORT" \
  not-found-repair

REPORT="$M14E_RUNS/failures/record-type-report.json"
(cd "$M14E_WORK" && "$APM" harness \
  --config agentpm.openai.m14e.fail-record-type.harness.json \
  --headless \
  --scope user=m14e-fail-record-type-user \
  --scope conversation=m14e-fail-record-type-conversation \
  --input-file inputs/m14e-write-note.txt \
  --report "$REPORT") || true

"$AGENTPM_MANUAL_PYTHON" "$M14E_WORK/scripts/m14e_assert_report.py" \
  "$REPORT" \
  record-type-typed

REPORT="$M14E_RUNS/failures/capacity-report.json"
(cd "$M14E_WORK" && "$APM" harness \
  --config agentpm.openai.m14e.fail-capacity.harness.json \
  --headless \
  --scope user=m14e-fail-capacity-user \
  --scope conversation=m14e-fail-capacity-conversation \
  --input-file inputs/m14e-write-note.txt \
  --report "$REPORT") || true

"$AGENTPM_MANUAL_PYTHON" "$M14E_WORK/scripts/m14e_assert_report.py" \
  "$REPORT" \
  capacity-typed
```

Expected:

- `not_found` behaves like local SQLite lookup failure and reaches the repair/structured-failure path;
- `record_type_mismatch` is visible as a typed Memory failure;
- `capacity_exceeded` is visible as a typed Memory failure;
- none of these failures silently dispatch to local SQLite.

## Test 7: host MemoryRuntime mapping without host registration is unavailable

```bash
REPORT="$M14E_RUNS/host/unregistered-report.json"
(cd "$M14E_WORK" && "$APM" harness \
  --config agentpm.openai.m14e.host-unregistered.harness.json \
  --headless \
  --scope user=m14e-host-user \
  --scope conversation=m14e-host-conversation \
  --input-file inputs/m14e-read-notes.txt \
  --report "$REPORT") || true

"$AGENTPM_MANUAL_PYTHON" "$M14E_WORK/scripts/m14e_assert_report.py" \
  "$REPORT" \
  suppressed service-failed-or-suppressed
```

Expected:

- mapped Memory surface is suppressed/unavailable;
- trace or report mentions missing host registration;
- local SQLite is not used as fallback.

## Test 8: one bad custom runtime does not disable a healthy sibling

This scenario is easiest to verify with focused unit coverage, because the shared M14b/M14c manual fixture installs one Memory package. Manual validation should confirm the relevant regression test remains present and passing:

```bash
cargo test -p agentpm-cli custom_memory_one_bad_runtime_does_not_disable_healthy_sibling -- --nocapture
```

Expected:

- test passes;
- healthy mapped runtime remains available while the unhealthy mapped runtime is suppressed.

## Final verification checklist

Run before handing off the M14e manual pass:

```bash
cargo fmt --all --check
cargo test -p agentpm-cli custom_memory -- --nocapture
cargo test -p agentpm-cli host_service -- --nocapture
cargo test -p agentpm-cli commands::harness::tests -- --nocapture
```

Manual pass is acceptable when:

- process custom runtime preflight/read/write scenarios pass;
- durable runtime logs exclude non-persistable/private values;
- descriptor rejection cases suppress mapped Memory surfaces without SQLite fallback;
- typed backend failure codes are visible in report/trace behavior;
- host-not-registered mapping is unavailable with service diagnostics;
- one-bad-runtime isolation unit coverage remains passing.

## Cleanup

```bash
rm -f /tmp/setup-harness-m14e-manual.sh
rm -rf "$HARNESS_M14BC_TEST_BASE"
```
