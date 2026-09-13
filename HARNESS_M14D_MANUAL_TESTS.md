# Harness Milestone 14d Manual Tests

Manual coverage for Milestone 14d: built-in local SQLite semantic Memory retrieval and vector lifecycle.

This manual pass extends the M14b/M14c Memory fixture because M14d is an additive semantic/vector layer over the same direct Memory runtime. It uses a deterministic local process `EmbeddingProvider` so OpenAI and Anthropic are only responsible for selecting the requested Harness semantic actions; vector behavior is asserted directly against the SQLite store and trace/report output.

## Coverage target

| Requirement | Manual coverage |
|---|---|
| `memory.local.semantic` gates `semantic` readiness | preflight/config checks |
| OpenAI semantic Memory write/read | live provider run |
| Anthropic semantic Memory write/read | live provider run |
| write-time vector generation | SQLite `memory_vectors` assertions after writes |
| missing-vector lazy backfill | write without semantic config, read with semantic config |
| stale hash regeneration | deliberate vector tamper, semantic read, refreshed hash |
| little-endian `f32` vector blobs | SQLite blob decode/assertion helper |
| durable-projection-only embedding input | embedder request log excludes `x-agentpm-persist:false` values |
| embedding usage/events | report usage and trace event assertions |
| no sqlite-vec dependency | successful exact cosine retrieval using only stored blobs and Rust ranking |

Provider behavior can vary. If a model does not select the explicitly requested Memory action, keep the report/trace and rerun that scenario before treating it as a Harness regression.

## Prerequisites

From the root of the `agentpm` repo:

```bash
cargo build -p agentpm-cli
export APM="$PWD/target/debug/agentpm"
export AGENTPM_MANUAL_PYTHON="${AGENTPM_MANUAL_PYTHON:-python3}"
export OPENAI_API_KEY="your OpenAI key"
export ANTHROPIC_API_KEY="your Anthropic key"
```

If either provider model is unavailable in your account, set overrides before setup:

```bash
export AGENTPM_MANUAL_OPENAI_MODEL="${AGENTPM_MANUAL_OPENAI_MODEL:-gpt-4o-mini}"
export AGENTPM_MANUAL_ANTHROPIC_MODEL="${AGENTPM_MANUAL_ANTHROPIC_MODEL:-claude-haiku-4-5-20251001}"
```

## Setup

First create the M14b/M14c fixture using the setup block in `HARNESS_M14B_M14C_MANUAL_TESTS.md`.

Expected workspace:

```bash
export HARNESS_M14BC_TEST_BASE="${HARNESS_M14BC_TEST_BASE:-$PWD/harness-m14bc-test}"
export M14BC_WORK="$HARNESS_M14BC_TEST_BASE/workspace"
export M14BC_RUNS="$HARNESS_M14BC_TEST_BASE/runs"
```

Then apply the M14d semantic extension:

```bash
cat > /tmp/setup-harness-m14d-manual.sh <<'SH'
#!/usr/bin/env bash
set -euo pipefail

ROOT="$(pwd)"
APM="${APM:-$ROOT/target/debug/agentpm}"
BASE="${HARNESS_M14BC_TEST_BASE:-$ROOT/harness-m14bc-test}"
WORK="$BASE/workspace"
RUNS="$BASE/runs-m14d"
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

mkdir -p "$WORK/runtime" "$WORK/inputs" "$WORK/scripts" "$RUNS"/{openai,anthropic,backfill}

"$PYTHON_CMD" - "$MEMORY_MANIFEST" <<'PY'
import json
import sys
from pathlib import Path

path = Path(sys.argv[1])
data = json.loads(path.read_text())
notes = data["memory"]["spaces"]["notes"]
modes = notes.setdefault("retrieval", {}).setdefault("modes", [])
if "semantic" not in modes:
    modes.append("semantic")
path.write_text(json.dumps(data, indent=2) + "\n")
PY

"$APM" lint "$MEMORY_MANIFEST" >/dev/null
"$APM" memory build --manifest "$MEMORY_MANIFEST" >/dev/null

cat > "$WORK/runtime/m14d_memory_embedding_service.py" <<'PY'
#!/usr/bin/env python3
import json
import math
import sys
from pathlib import Path

LOG_PATH = Path("runtime/m14d-memory-embedder-calls.jsonl")

def emit(msg, kind, result=None, error=None):
    out = {
        "protocol": "agentpm-service",
        "version": 1,
        "kind": kind,
        "id": msg.get("id"),
        "service": msg.get("service", "embedding"),
    }
    if result is not None:
        out["result"] = result
    if error is not None:
        out["error"] = error
    print(json.dumps(out), flush=True)

def normalize(vector):
    norm = math.sqrt(sum(v * v for v in vector))
    return [v / norm for v in vector]

def vector_for(text):
    text = str(text or "").lower()
    if "beta" in text and "alpha" not in text:
        return normalize([0.0, 1.0, 0.0])
    if "support" in text or "closed" in text:
        return normalize([0.0, 0.0, 1.0])
    if "mixed" in text:
        return normalize([0.70710678, 0.70710678, 0.0])
    return normalize([1.0, 0.0, 0.0])

for line in sys.stdin:
    msg = json.loads(line)
    if msg.get("kind") == "initialize":
        emit(msg, "initialized", {
            "ready": True,
            "registry_id": msg.get("payload", {}).get("registry_id", "m14d-memory-embedder"),
            "embedding_spaces": [
                {
                    "id": "m14d-memory-embedder",
                    "provider": "m14d-memory-embedder",
                    "model": "toy-3d",
                    "dimensions": 3,
                    "metric": "cosine",
                    "normalized": True
                }
            ]
        })
        continue
    if msg.get("method") != "embed":
        emit(msg, "error", error={"code": "unsupported_method", "message": "unsupported embedding method"})
        continue
    payload = msg.get("payload", {})
    text = payload.get("text")
    LOG_PATH.parent.mkdir(parents=True, exist_ok=True)
    with LOG_PATH.open("a") as handle:
        handle.write(json.dumps({"text": text, "provider": payload.get("provider"), "model": payload.get("model")}) + "\n")
    emit(msg, "response", {
        "vector": vector_for(text),
        "provider": payload.get("provider"),
        "model": payload.get("model"),
        "dimensions": payload.get("dimensions"),
        "normalized": payload.get("normalized")
    })
PY
chmod +x "$WORK/runtime/m14d_memory_embedding_service.py"

write_semantic_config() {
  local source="$1"
  local target="$2"
  local state_dir="$3"
  "$PYTHON_CMD" - "$source" "$target" "$state_dir" "$PYTHON_CMD" <<'PY'
import json
import os
import sys
from pathlib import Path

source = Path(sys.argv[1])
target = Path(sys.argv[2])
state_dir = sys.argv[3]
python_cmd = sys.argv[4]
data = json.loads(source.read_text())
if data.get("model", {}).get("provider") == "openai":
    data["model"]["model"] = os.environ.get("AGENTPM_MANUAL_OPENAI_MODEL", data["model"]["model"])
elif data.get("model", {}).get("provider") == "anthropic":
    data["model"]["model"] = os.environ.get("AGENTPM_MANUAL_ANTHROPIC_MODEL", data["model"]["model"])
data.setdefault("providers", {}).setdefault("embeddings", {})["m14d-memory-embedder"] = {
    "implementation": {
        "type": "process",
        "command": python_cmd,
        "args": ["runtime/m14d_memory_embedding_service.py"],
        "cwd": ".",
        "startup_timeout_ms": 10000,
        "request_timeout_ms": 30000,
        "restart": {"max_attempts": 0, "backoff_ms": 0}
    }
}
data.setdefault("memory", {}).setdefault("local", {})["semantic"] = {
    "embedding_provider": "m14d-memory-embedder",
    "model": "toy-3d",
    "dimensions": 3
}
data.setdefault("runtime", {})["state_dir"] = state_dir
data.setdefault("trace", {})["enabled"] = True
data["trace"]["level"] = "verbose"
data["trace"]["content"] = "full"
target.write_text(json.dumps(data, indent=2) + "\n")
PY
}

write_no_semantic_config() {
  local source="$1"
  local target="$2"
  local state_dir="$3"
  "$PYTHON_CMD" - "$source" "$target" "$state_dir" <<'PY'
import json
import os
import sys
from pathlib import Path

source = Path(sys.argv[1])
target = Path(sys.argv[2])
state_dir = sys.argv[3]
data = json.loads(source.read_text())
if data.get("model", {}).get("provider") == "openai":
    data["model"]["model"] = os.environ.get("AGENTPM_MANUAL_OPENAI_MODEL", data["model"]["model"])
elif data.get("model", {}).get("provider") == "anthropic":
    data["model"]["model"] = os.environ.get("AGENTPM_MANUAL_ANTHROPIC_MODEL", data["model"]["model"])
data.setdefault("runtime", {})["state_dir"] = state_dir
data.pop("memory", None)
data.setdefault("providers", {}).pop("embeddings", None)
if not data.get("providers"):
    data.pop("providers", None)
data.setdefault("trace", {})["enabled"] = True
data["trace"]["level"] = "verbose"
data["trace"]["content"] = "full"
target.write_text(json.dumps(data, indent=2) + "\n")
PY
}

write_semantic_config "$WORK/agentpm.openai.harness.json" "$WORK/agentpm.openai.m14d.harness.json" ".agentpm-state-m14d-openai"
write_semantic_config "$WORK/agentpm.anthropic.harness.json" "$WORK/agentpm.anthropic.m14d.harness.json" ".agentpm-state-m14d-anthropic"
write_no_semantic_config "$WORK/agentpm.openai.harness.json" "$WORK/agentpm.openai.m14d-no-semantic.harness.json" ".agentpm-state-m14d-backfill"
write_semantic_config "$WORK/agentpm.openai.harness.json" "$WORK/agentpm.openai.m14d-backfill.harness.json" ".agentpm-state-m14d-backfill"

cat > "$WORK/inputs/m14d-write-semantic-notes.txt" <<'TXT'
Use direct Memory package @zack/m14bc-memory.

Create exactly three records in the notes space using memory_write operation create and record_type note:

1. {
  "body": "Alpha semantic durable note",
  "status": "open",
  "labels": ["alpha", "semantic"],
  "scratch": { "public": "semantic public scratch", "private": "ephemeral-semantic-secret" }
}

2. {
  "body": "Beta semantic durable note",
  "status": "open",
  "labels": ["beta", "semantic"]
}

3. {
  "body": "Closed support semantic note",
  "status": "closed",
  "labels": ["support"]
}

After the Memory writes complete, complete with outcome done and summarize the created record count. Do not use external knowledge or tools.
TXT

cat > "$WORK/inputs/m14d-read-semantic-alpha.txt" <<'TXT'
Use direct Memory package @zack/m14bc-memory.

Run one memory_read against the notes space:
- mode: semantic
- record_type: note
- limit: 3
- query: "alpha semantic"

Then complete with outcome done and summarize the returned record bodies in order. Do not use external knowledge or tools.
TXT

cat > "$WORK/inputs/m14d-read-semantic-alpha-filtered.txt" <<'TXT'
Use direct Memory package @zack/m14bc-memory.

Run one memory_read against the notes space:
- mode: semantic
- record_type: note
- limit: 3
- query: "alpha semantic"
- filter:
  { "status": "open" }

Then complete with outcome done and summarize the returned record bodies in order. Do not use external knowledge or tools.
TXT

cat > "$WORK/scripts/m14d_vector_summary.py" <<'PY'
#!/usr/bin/env python3
import json
import sqlite3
import struct
import sys
from pathlib import Path

if len(sys.argv) != 2:
    print("usage: m14d_vector_summary.py <state-dir-or-memory.sqlite3>", file=sys.stderr)
    raise SystemExit(2)

path = Path(sys.argv[1])
db = path / "memory.sqlite3" if path.is_dir() else path
if not db.exists():
    print(json.dumps({"db": str(db), "count": 0, "vectors": []}, indent=2, sort_keys=True))
    raise SystemExit(0)
conn = sqlite3.connect(db)
conn.row_factory = sqlite3.Row
try:
    rows = conn.execute(
        """
        SELECT v.record_id, v.space, v.record_type, v.embedding_provider, v.embedding_model,
               v.dimensions, v.content_hash, v.vector, r.content_json
        FROM memory_vectors v
        JOIN memory_records r
          ON r.package = v.package
         AND r.package_version = v.package_version
         AND r.space = v.space
         AND r.scope_hash = v.scope_hash
         AND r.id = v.record_id
        ORDER BY v.space, v.record_id
        """
    ).fetchall()
except sqlite3.OperationalError as error:
    if "no such table: memory_vectors" not in str(error):
        raise
    print(json.dumps({"db": str(db), "count": 0, "vectors": []}, indent=2, sort_keys=True))
    raise SystemExit(0)

vectors = []
for row in rows:
    blob = row["vector"]
    dimensions = row["dimensions"]
    assert len(blob) == dimensions * 4, (row["record_id"], len(blob), dimensions)
    values = list(struct.unpack("<" + "f" * dimensions, blob))
    vectors.append({
        "record_id": row["record_id"],
        "space": row["space"],
        "record_type": row["record_type"],
        "embedding_provider": row["embedding_provider"],
        "embedding_model": row["embedding_model"],
        "dimensions": dimensions,
        "content_hash": row["content_hash"],
        "vector": values,
        "body": json.loads(row["content_json"]).get("body")
    })

print(json.dumps({"db": str(db), "count": len(vectors), "vectors": vectors}, indent=2, sort_keys=True))
PY
chmod +x "$WORK/scripts/m14d_vector_summary.py"

cat > "$WORK/scripts/m14d_assert_report.py" <<'PY'
#!/usr/bin/env python3
import json
import sys
from pathlib import Path

if len(sys.argv) < 3:
    print("usage: m14d_assert_report.py <report.json> <event-type>...", file=sys.stderr)
    raise SystemExit(2)

report_path = Path(sys.argv[1])
expected_events = set(sys.argv[2:])
report = json.loads(report_path.read_text())
assert report.get("terminal_status") == "ended", report.get("terminal_status")
trace_path = Path(report["trace_path"])
events = [json.loads(line) for line in trace_path.read_text().splitlines() if line.strip()]
types = {event["event_type"] for event in events}
missing = expected_events - types
assert not missing, {"missing": sorted(missing), "types": sorted(types)}
usage = report.get("usage") or {}
assert usage.get("memory_requests", 0) >= 1, usage
print(json.dumps({
    "ok": True,
    "report": str(report_path),
    "usage": usage,
    "events": sorted(expected_events)
}, indent=2))
PY
chmod +x "$WORK/scripts/m14d_assert_report.py"

cat > "$WORK/scripts/m14d_assert_embedder_log.py" <<'PY'
#!/usr/bin/env python3
import json
import sys
from pathlib import Path

if len(sys.argv) != 2:
    print("usage: m14d_assert_embedder_log.py <workspace>", file=sys.stderr)
    raise SystemExit(2)

log = Path(sys.argv[1]) / "runtime" / "m14d-memory-embedder-calls.jsonl"
rows = [json.loads(line) for line in log.read_text().splitlines() if line.strip()]
texts = [row.get("text") or "" for row in rows]
joined = "\n".join(texts)
assert "ephemeral-semantic-secret" not in joined, joined
assert any("Alpha semantic durable note" in text for text in texts), texts
assert any("alpha semantic" in text for text in texts), texts
print(json.dumps({"ok": True, "calls": len(rows)}, indent=2))
PY
chmod +x "$WORK/scripts/m14d_assert_embedder_log.py"

cat > "$WORK/scripts/m14d_tamper_one_vector.py" <<'PY'
#!/usr/bin/env python3
import sqlite3
import struct
import sys
from pathlib import Path

if len(sys.argv) != 3:
    print("usage: m14d_tamper_one_vector.py <state-dir-or-memory.sqlite3> <body-substring>", file=sys.stderr)
    raise SystemExit(2)

path = Path(sys.argv[1])
needle = sys.argv[2]
db = path / "memory.sqlite3" if path.is_dir() else path
conn = sqlite3.connect(db)
row = conn.execute(
    """
    SELECT v.record_id
    FROM memory_vectors v
    JOIN memory_records r
      ON r.package = v.package
     AND r.package_version = v.package_version
     AND r.space = v.space
     AND r.scope_hash = v.scope_hash
     AND r.id = v.record_id
    WHERE r.content_json LIKE ?
    ORDER BY v.record_id
    LIMIT 1
    """,
    (f"%{needle}%",),
).fetchone()
assert row, f"no vector row matched {needle!r}"
bad_blob = struct.pack("<fff", 0.0, 0.0, 1.0)
conn.execute(
    "UPDATE memory_vectors SET content_hash = 'sha256:manual-stale', vector = ? WHERE record_id = ?",
    (bad_blob, row[0]),
)
conn.commit()
print(row[0])
PY
chmod +x "$WORK/scripts/m14d_tamper_one_vector.py"

cat > "$BASE/README-M14D.txt" <<TXT
M14d manual extension ready.

Workspace:
  $WORK

M14d run output:
  $RUNS

Configs:
  $WORK/agentpm.openai.m14d.harness.json
  $WORK/agentpm.anthropic.m14d.harness.json
  $WORK/agentpm.openai.m14d-no-semantic.harness.json
  $WORK/agentpm.openai.m14d-backfill.harness.json
TXT

cat "$BASE/README-M14D.txt"
SH

bash /tmp/setup-harness-m14d-manual.sh
```

Set helper variables for the tests:

```bash
export HARNESS_M14BC_TEST_BASE="${HARNESS_M14BC_TEST_BASE:-$PWD/harness-m14bc-test}"
export M14D_WORK="$HARNESS_M14BC_TEST_BASE/workspace"
export M14D_RUNS="$HARNESS_M14BC_TEST_BASE/runs-m14d"
```

## Test 1: semantic readiness and degraded readiness

Semantic config preflight:

```bash
(cd "$M14D_WORK" && "$APM" harness --config agentpm.openai.m14d.harness.json --verbose) \
  | tee "$M14D_RUNS/preflight-openai-semantic.txt"

(cd "$M14D_WORK" && "$APM" harness --config agentpm.anthropic.m14d.harness.json --verbose) \
  | tee "$M14D_RUNS/preflight-anthropic-semantic.txt"
```

No-semantic config preflight:

```bash
(cd "$M14D_WORK" && "$APM" harness --config agentpm.openai.m14d-no-semantic.harness.json --verbose) \
  | tee "$M14D_RUNS/preflight-openai-no-semantic.txt"
```

Expected:

- all three preflights report `Status: Ready`;
- semantic configs show the configured EmbeddingProvider as pending runtime activation;
- Memory remains visible as a pending runtime activation capability;
- preflight does not currently print per-space retrieval modes, so it does not directly prove whether `semantic` is exposed on `notes`.

Confirm the actual `semantic` retrieval mode in the first headless run trace instead. After Test 2 or Test 3, inspect `memory_surface_ready` events:

```bash
TRACE="$("$AGENTPM_MANUAL_PYTHON" - "$REPORT" <<'PY'
import json
import sys
from pathlib import Path
print(json.loads(Path(sys.argv[1]).read_text())["trace_path"])
PY
)"

"$AGENTPM_MANUAL_PYTHON" - "$TRACE" <<'PY'
import json
import sys
from pathlib import Path

events = [json.loads(line) for line in Path(sys.argv[1]).read_text().splitlines() if line.strip()]
surfaces = [
    event["payload"]["fields"]
    for event in events
    if event.get("event_type") == "memory_surface_ready"
]
notes = [surface for surface in surfaces if surface.get("space") == "notes"]
assert notes, surfaces
assert "semantic" in notes[0].get("retrieval_modes", []), notes[0]
print(json.dumps(notes[0], indent=2, sort_keys=True))
PY
```

For the no-semantic config, the useful confirmation is a headless run trace showing the `notes` surface without `semantic`, or an explicit semantic-read attempt being unavailable/rejected. The focused Rust readiness tests in Test 8 are the deterministic pin for that degraded-mode behavior.

## Test 2: OpenAI write-time vector generation

```bash
REPORT="$M14D_RUNS/openai/01-write-semantic-notes.report.json"
(cd "$M14D_WORK" && "$APM" harness \
  --config agentpm.openai.m14d.harness.json \
  --headless \
  --scope user=m14d-openai-user \
  --scope conversation=m14d-openai-conversation \
  --input-file inputs/m14d-write-semantic-notes.txt \
  --report "$REPORT" \
  >"$M14D_RUNS/openai/01-write-semantic-notes.stdout.txt" \
  2>"$M14D_RUNS/openai/01-write-semantic-notes.stderr.txt")

jq '{terminal_status, terminal_output, usage, memory_summaries, trace_path}' "$REPORT"
"$AGENTPM_MANUAL_PYTHON" "$M14D_WORK/scripts/m14d_assert_report.py" \
  "$REPORT" memory_write_started memory_write_completed embedding_request_started embedding_request_completed

"$AGENTPM_MANUAL_PYTHON" "$M14D_WORK/scripts/m14d_vector_summary.py" "$M14D_WORK/.agentpm-state-m14d-openai" \
  | tee "$M14D_RUNS/openai/01-vector-summary.json"

"$AGENTPM_MANUAL_PYTHON" "$M14D_WORK/scripts/m14d_assert_embedder_log.py" "$M14D_WORK"
```

Expected:

- run ends successfully;
- three `notes` writes are accepted;
- report usage includes `memory_requests >= 3` and `embedding_requests >= 3`;
- `memory_vectors` has one 3-dimensional vector row per created note;
- vector blobs decode as little-endian `f32`;
- embedder log does not contain `ephemeral-semantic-secret`.

## Test 3: OpenAI semantic read uses cached vectors and exact cosine ranking

```bash
REPORT="$M14D_RUNS/openai/02-read-semantic-alpha.report.json"
(cd "$M14D_WORK" && "$APM" harness \
  --config agentpm.openai.m14d.harness.json \
  --headless \
  --scope user=m14d-openai-user \
  --scope conversation=m14d-openai-conversation \
  --input-file inputs/m14d-read-semantic-alpha.txt \
  --report "$REPORT" \
  >"$M14D_RUNS/openai/02-read-semantic-alpha.stdout.txt" \
  2>"$M14D_RUNS/openai/02-read-semantic-alpha.stderr.txt")

jq '{terminal_status, terminal_output, usage, memory_summaries, trace_path}' "$REPORT"
"$AGENTPM_MANUAL_PYTHON" "$M14D_WORK/scripts/m14d_assert_report.py" \
  "$REPORT" memory_read_started memory_read_completed embedding_request_started embedding_request_completed

"$AGENTPM_MANUAL_PYTHON" "$M14D_WORK/scripts/trace_summary.py" "$REPORT" | less
```

Expected:

- semantic read returns the Alpha note before Beta/support records;
- report usage includes `memory_requests >= 1` and `embedding_requests >= 1`;
- if Test 2 generated all vectors, the semantic read should need only the query embedding, not three record backfills.

## Test 4: Anthropic write-time vector generation

```bash
REPORT="$M14D_RUNS/anthropic/01-write-semantic-notes.report.json"
(cd "$M14D_WORK" && "$APM" harness \
  --config agentpm.anthropic.m14d.harness.json \
  --headless \
  --scope user=m14d-anthropic-user \
  --scope conversation=m14d-anthropic-conversation \
  --input-file inputs/m14d-write-semantic-notes.txt \
  --report "$REPORT" \
  >"$M14D_RUNS/anthropic/01-write-semantic-notes.stdout.txt" \
  2>"$M14D_RUNS/anthropic/01-write-semantic-notes.stderr.txt")

jq '{terminal_status, terminal_output, usage, memory_summaries, trace_path}' "$REPORT"
"$AGENTPM_MANUAL_PYTHON" "$M14D_WORK/scripts/m14d_assert_report.py" \
  "$REPORT" memory_write_started memory_write_completed embedding_request_started embedding_request_completed

"$AGENTPM_MANUAL_PYTHON" "$M14D_WORK/scripts/m14d_vector_summary.py" "$M14D_WORK/.agentpm-state-m14d-anthropic" \
  | tee "$M14D_RUNS/anthropic/01-vector-summary.json"
```

Expected:

- run ends successfully;
- Anthropic executes direct `memory_write` actions;
- semantic write path generates vectors and embedding usage/events just like OpenAI.

## Test 5: Anthropic semantic read and filter restriction

```bash
REPORT="$M14D_RUNS/anthropic/02-read-semantic-alpha-filtered.report.json"
(cd "$M14D_WORK" && "$APM" harness \
  --config agentpm.anthropic.m14d.harness.json \
  --headless \
  --scope user=m14d-anthropic-user \
  --scope conversation=m14d-anthropic-conversation \
  --input-file inputs/m14d-read-semantic-alpha-filtered.txt \
  --report "$REPORT" \
  >"$M14D_RUNS/anthropic/02-read-semantic-alpha-filtered.stdout.txt" \
  2>"$M14D_RUNS/anthropic/02-read-semantic-alpha-filtered.stderr.txt")

jq '{terminal_status, terminal_output, usage, memory_summaries, trace_path}' "$REPORT"
"$AGENTPM_MANUAL_PYTHON" "$M14D_WORK/scripts/m14d_assert_report.py" \
  "$REPORT" memory_read_started memory_read_completed embedding_request_started embedding_request_completed

"$AGENTPM_MANUAL_PYTHON" "$M14D_WORK/scripts/trace_summary.py" "$REPORT" | less
```

Expected:

- semantic read is restricted to active `note` records for the requested trusted scope;
- filter `{ "status": "open" }` excludes the closed support note before ranking;
- Alpha ranks before Beta for query `alpha semantic`.

## Test 6: missing-vector lazy backfill

Create records without `memory.local.semantic`, so writes do not generate vector rows:

```bash
REPORT="$M14D_RUNS/backfill/01-write-without-semantic.report.json"
(cd "$M14D_WORK" && "$APM" harness \
  --config agentpm.openai.m14d-no-semantic.harness.json \
  --headless \
  --scope user=m14d-backfill-user \
  --scope conversation=m14d-backfill-conversation \
  --input-file inputs/m14d-write-semantic-notes.txt \
  --report "$REPORT" \
  >"$M14D_RUNS/backfill/01-write-without-semantic.stdout.txt" \
  2>"$M14D_RUNS/backfill/01-write-without-semantic.stderr.txt")

jq '{terminal_status, terminal_output, usage, memory_summaries, trace_path}' "$REPORT"
"$AGENTPM_MANUAL_PYTHON" "$M14D_WORK/scripts/m14d_vector_summary.py" "$M14D_WORK/.agentpm-state-m14d-backfill" \
  | tee "$M14D_RUNS/backfill/01-vector-summary-before.json"
```

Expected:

- Memory writes succeed;
- vector count is `0`;
- write report has `embedding_requests == 0` or no embedding usage from Memory writes.

Read the same scope with semantic config enabled:

```bash
REPORT="$M14D_RUNS/backfill/02-read-backfills-missing-vectors.report.json"
(cd "$M14D_WORK" && "$APM" harness \
  --config agentpm.openai.m14d-backfill.harness.json \
  --headless \
  --scope user=m14d-backfill-user \
  --scope conversation=m14d-backfill-conversation \
  --input-file inputs/m14d-read-semantic-alpha.txt \
  --report "$REPORT" \
  >"$M14D_RUNS/backfill/02-read-backfills-missing-vectors.stdout.txt" \
  2>"$M14D_RUNS/backfill/02-read-backfills-missing-vectors.stderr.txt")

jq '{terminal_status, terminal_output, usage, memory_summaries, trace_path}' "$REPORT"
"$AGENTPM_MANUAL_PYTHON" "$M14D_WORK/scripts/m14d_assert_report.py" \
  "$REPORT" memory_read_started memory_read_completed embedding_request_started embedding_request_completed

"$AGENTPM_MANUAL_PYTHON" "$M14D_WORK/scripts/m14d_vector_summary.py" "$M14D_WORK/.agentpm-state-m14d-backfill" \
  | tee "$M14D_RUNS/backfill/02-vector-summary-after.json"
```

Expected:

- semantic read succeeds and ranks Alpha first;
- vector count changes from `0` to `3`;
- report/trace show embedding usage for query plus record backfill.

## Test 7: stale hash regeneration

Tamper one OpenAI vector row, then run another semantic read:

```bash
"$AGENTPM_MANUAL_PYTHON" "$M14D_WORK/scripts/m14d_tamper_one_vector.py" \
  "$M14D_WORK/.agentpm-state-m14d-openai" "Alpha semantic durable note" \
  | tee "$M14D_RUNS/openai/tampered-record-id.txt"

"$AGENTPM_MANUAL_PYTHON" "$M14D_WORK/scripts/m14d_vector_summary.py" "$M14D_WORK/.agentpm-state-m14d-openai" \
  | tee "$M14D_RUNS/openai/03-vector-summary-tampered.json"

REPORT="$M14D_RUNS/openai/03-read-regenerates-stale-vector.report.json"
(cd "$M14D_WORK" && "$APM" harness \
  --config agentpm.openai.m14d.harness.json \
  --headless \
  --scope user=m14d-openai-user \
  --scope conversation=m14d-openai-conversation \
  --input-file inputs/m14d-read-semantic-alpha.txt \
  --report "$REPORT" \
  >"$M14D_RUNS/openai/03-read-regenerates-stale-vector.stdout.txt" \
  2>"$M14D_RUNS/openai/03-read-regenerates-stale-vector.stderr.txt")

jq '{terminal_status, terminal_output, usage, memory_summaries, trace_path}' "$REPORT"
"$AGENTPM_MANUAL_PYTHON" "$M14D_WORK/scripts/m14d_vector_summary.py" "$M14D_WORK/.agentpm-state-m14d-openai" \
  | tee "$M14D_RUNS/openai/03-vector-summary-regenerated.json"
```

Expected:

- tampered row initially shows `content_hash: sha256:manual-stale`;
- after semantic read, the same row has a real `sha256:` durable content hash;
- Alpha still ranks first;
- trace/report show an extra embedding request for stale vector regeneration.

## Test 8: focused automated checks to run with the manual pass

These deterministic tests pin edge cases that are difficult to force through live providers every time:

```bash
cargo test -p agentpm-cli sqlite_semantic -- --nocapture \
  | tee "$M14D_RUNS/cargo-sqlite-semantic.txt"

cargo test -p agentpm-cli memory_semantic_retrieval -- --nocapture \
  | tee "$M14D_RUNS/cargo-memory-semantic-readiness.txt"

cargo test -p agentpm-cli semantic_memory_read_reports_embedding_usage_and_events -- --nocapture \
  | tee "$M14D_RUNS/cargo-memory-semantic-engine-events.txt"

cargo test -p agentpm-cli embedding_provider_failures_emit_embedding_failed_event -- --nocapture \
  | tee "$M14D_RUNS/cargo-memory-embedding-failure-events.txt"
```

Expected:

- local SQLite semantic read/write/vector lifecycle tests pass;
- semantic readiness is advertised only when embedding provider config is usable;
- embedding usage/events are counted for Memory semantic reads/writes;
- embedding provider failures map to typed embedding failure events.

## Final verification commands

Run before handing off the M14d manual pass:

```bash
cargo fmt --all --check
cargo clippy -p agentpm-cli --all-targets --all-features -- -D warnings
cargo test -p agentpm-cli sqlite_semantic -- --nocapture
cargo test -p agentpm-cli memory_semantic_retrieval -- --nocapture
cargo test -p agentpm-cli semantic_memory_read_reports_embedding_usage_and_events -- --nocapture
cargo test -p agentpm-cli embedding_provider_failures_emit_embedding_failed_event -- --nocapture
```

If the local environment permits the full filesystem fixture suite:

```bash
cargo test -p agentpm-cli -- --test-threads=1
```

Expected:

- formatter check passes;
- clippy passes with `-D warnings`;
- focused M14d runtime/readiness/Engine event tests pass;
- full `agentpm-cli` test suite passes, or any unrelated failure is documented.

## Cleanup

```bash
rm -rf "$HARNESS_M14BC_TEST_BASE"
rm -f /tmp/setup-harness-m14d-manual.sh
```
