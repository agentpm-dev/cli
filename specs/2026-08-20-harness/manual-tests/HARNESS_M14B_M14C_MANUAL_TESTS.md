# Harness Milestone 14b/14c Manual Tests

Manual coverage for Harness M14b and M14c: local SQLite direct Memory semantics and HarnessEngine direct `memory_read` / `memory_write` routing.

M14b exposes the local runtime primitives. M14c wires those primitives into real Harness model actions. There is not a separate public CLI for invoking M14b primitives directly, so the useful manual path is:

- create a disposable Harness workspace with a Memory Blueprint;
- run OpenAI and Anthropic headless Harness scenarios that select direct Memory actions;
- inspect JSONL trace, run reports, and the local SQLite store.

Not covered here:

- local semantic/vector Memory retrieval, which is M14d;
- custom process/host MemoryRuntime routing, which is M14e;
- Memory Hooks, which are M14f;
- optional persistence review, which is M14g;
- lifecycle operation scheduling/triggers, which are M15.

## Prerequisites

From the root of the `agentpm` repo:

```bash
cargo build -p agentpm-cli
export APM="$PWD/target/debug/agentpm"
export AGENTPM_MANUAL_PYTHON="${AGENTPM_MANUAL_PYTHON:-python3}"
```

For live model runs:

```bash
export OPENAI_API_KEY="your OpenAI key"
export ANTHROPIC_API_KEY="your Anthropic key"
```

The setup script creates OpenAI and Anthropic Harness configs. If either model name is not available in your account, edit the generated config before running that provider:

- `harness-m14bc-test/workspace/agentpm.openai.harness.json`
- `harness-m14bc-test/workspace/agentpm.anthropic.harness.json`

## Setup

Run this from the root of the `agentpm` repo. It creates `harness-m14bc-test/` with:

- one installed Memory Blueprint package under `.agentpm/`;
- one Loop and Agent configured for direct Memory read/write;
- separate OpenAI and Anthropic local state directories;
- input prompts for focused scenarios;
- helper scripts for inspecting SQLite state and traces.

```bash
cat > /tmp/setup-harness-m14bc-manual.sh <<'SH'
#!/usr/bin/env bash
set -euo pipefail

ROOT="$(pwd)"
APM="${APM:-$ROOT/target/debug/agentpm}"
BASE="${HARNESS_M14BC_TEST_BASE:-$ROOT/harness-m14bc-test}"
WORK="$BASE/workspace"
RUNS="$BASE/runs"
PYTHON_CMD="${AGENTPM_MANUAL_PYTHON:-python3}"
SCHEMA_URL="https://raw.githubusercontent.com/agentpm-dev/cli/refs/heads/main/schemas/agentpm.manifest.schema.json"

if [ ! -x "$APM" ]; then
  echo "Missing agentpm binary at $APM. Run: cargo build -p agentpm-cli" >&2
  exit 1
fi

if [ -e "$BASE" ]; then
  echo "$BASE already exists. Move or delete it before re-running setup." >&2
  exit 1
fi

mkdir -p "$WORK/.agentpm" "$WORK/inputs" "$WORK/scripts" "$RUNS"

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
  dir="$(pkg_dir loops '@zack/m14bc-loop' '0.1.0')"
  mkdir -p "$dir"
  cat > "$dir/agent.json" <<JSON
{
  "\$schema": "$SCHEMA_URL",
  "kind": "loop",
  "name": "m14bc-loop",
  "version": "0.1.0",
  "description": "M14b/M14c manual direct Memory loop.",
  "loop": {
    "entry_phase": "remember",
    "phases": [
      {
        "id": "remember",
        "objective": "Use direct Memory exactly as requested, then complete with the requested summary.",
        "access": {
          "tools": false,
          "knowledge": false,
          "memory": {
            "read": true,
            "write": true
          }
        },
        "outcomes": [
          { "id": "done", "description": "The requested Memory action sequence is complete." },
          { "id": "no-memory", "description": "No Memory action was possible." }
        ]
      }
    ],
    "transitions": [
      { "from": "remember", "on": "done", "to": "\$end" },
      { "from": "remember", "on": "no-memory", "to": "\$end" }
    ]
  }
}
JSON
  "$APM" lint "$dir/agent.json" >/dev/null
}

write_memory_package() {
  local dir
  dir="$(pkg_dir memory '@zack/m14bc-memory' '0.1.0')"
  mkdir -p "$dir/schemas"
  cat > "$dir/agent.json" <<JSON
{
  "\$schema": "$SCHEMA_URL",
  "kind": "memory",
  "name": "m14bc-memory",
  "version": "0.1.0",
  "description": "Manual M14b/M14c Memory Blueprint.",
  "memory": {
    "scopes": {
      "user": { "description": "Manual user partition." },
      "conversation": { "description": "Manual conversation partition." }
    },
    "record_types": {
      "status_summary": {
        "schema": "schemas/status-summary.schema.json",
        "version": "1.0.0",
        "description": "Conversation status summary."
      },
      "incident_summary": {
        "schema": "schemas/incident-summary.schema.json",
        "version": "1.0.0",
        "description": "Conversation incident summary."
      },
      "note": {
        "schema": "schemas/note.schema.json",
        "version": "1.0.0",
        "description": "Collection note."
      },
      "event": {
        "schema": "schemas/event.schema.json",
        "version": "1.0.0",
        "description": "Sequence event."
      },
      "append_note": {
        "schema": "schemas/append-note.schema.json",
        "version": "1.0.0",
        "description": "Append-only note."
      },
      "expiring_note": {
        "schema": "schemas/expiring-note.schema.json",
        "version": "1.0.0",
        "description": "Expiring note."
      }
    },
    "spaces": {
      "conversation_state": {
        "description": "Exactly one current conversation document.",
        "model": "document",
        "record_types": ["status_summary", "incident_summary"],
        "scope": ["user", "conversation"],
        "retrieval": { "modes": ["key"] }
      },
      "notes": {
        "description": "Small collection for CRUD, filter, full_text, capacity, and durable projection.",
        "model": "collection",
        "record_types": ["note"],
        "scope": ["user"],
        "retrieval": { "modes": ["key", "filter", "chronological", "full_text"] },
        "capacity": { "max_records": 3 }
      },
      "timeline": {
        "description": "Chronological sequence with stable ordinals.",
        "model": "sequence",
        "record_types": ["event"],
        "scope": ["user", "conversation"],
        "retrieval": { "modes": ["key", "chronological"] }
      },
      "append_log": {
        "description": "Append-only collection that should advertise create-only direct writes.",
        "model": "collection",
        "record_types": ["append_note"],
        "scope": ["user"],
        "retrieval": { "modes": ["key", "chronological"] },
        "constraints": { "append_only": true }
      },
      "archive_notes": {
        "description": "TTL archive retention check.",
        "model": "collection",
        "record_types": ["expiring_note"],
        "scope": ["user"],
        "retrieval": { "modes": ["key", "filter"] },
        "retention": { "ttl": "PT1S", "on_expire": "archive" }
      },
      "delete_notes": {
        "description": "TTL delete retention check.",
        "model": "collection",
        "record_types": ["expiring_note"],
        "scope": ["user"],
        "retrieval": { "modes": ["key", "filter"] },
        "retention": { "ttl": "PT1S", "on_expire": "delete" }
      }
    }
  }
}
JSON

  cat > "$dir/schemas/status-summary.schema.json" <<'JSON'
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "type": "object",
  "title": "Status Summary",
  "properties": {
    "summary": { "type": "string", "minLength": 1, "x-agentpm-persist": true },
    "risk_level": { "type": "string", "enum": ["low", "medium", "high"], "x-agentpm-persist": true },
    "private_hint": { "type": "string", "x-agentpm-persist": false }
  },
  "required": ["summary"],
  "additionalProperties": false
}
JSON

  cat > "$dir/schemas/incident-summary.schema.json" <<'JSON'
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "type": "object",
  "title": "Incident Summary",
  "properties": {
    "incident": { "type": "string", "minLength": 1, "x-agentpm-persist": true },
    "severity": { "type": "integer", "minimum": 1, "maximum": 5, "x-agentpm-persist": true },
    "private_hint": { "type": "string", "x-agentpm-persist": false }
  },
  "required": ["incident"],
  "additionalProperties": false
}
JSON

  cat > "$dir/schemas/note.schema.json" <<'JSON'
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "type": "object",
  "title": "Note",
  "properties": {
    "body": { "type": "string", "minLength": 1, "x-agentpm-persist": true },
    "status": { "type": "string", "enum": ["open", "closed"], "x-agentpm-persist": true },
    "labels": {
      "type": "array",
      "items": { "type": "string", "minLength": 1 },
      "x-agentpm-persist": true
    },
    "assignee": {
      "type": "object",
      "properties": {
        "team": { "type": "string", "minLength": 1, "x-agentpm-persist": true },
        "user": { "type": "string", "minLength": 1, "x-agentpm-persist": true }
      },
      "additionalProperties": false
    },
    "items": {
      "type": "array",
      "items": {
        "type": "object",
        "properties": {
          "name": { "type": "string", "minLength": 1, "x-agentpm-persist": true }
        },
        "required": ["name"],
        "additionalProperties": false
      },
      "x-agentpm-persist": true
    },
    "scratch": {
      "type": "object",
      "properties": {
        "public": { "type": "string", "x-agentpm-persist": true },
        "private": { "type": "string", "x-agentpm-persist": false }
      },
      "additionalProperties": false
    }
  },
  "required": ["body"],
  "additionalProperties": false
}
JSON

  cat > "$dir/schemas/event.schema.json" <<'JSON'
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "type": "object",
  "title": "Event",
  "properties": {
    "body": { "type": "string", "minLength": 1, "x-agentpm-persist": true },
    "kind": { "type": "string", "minLength": 1, "x-agentpm-persist": true },
    "private_note": { "type": "string", "x-agentpm-persist": false }
  },
  "required": ["body"],
  "additionalProperties": false
}
JSON

  cat > "$dir/schemas/append-note.schema.json" <<'JSON'
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "type": "object",
  "title": "Append Note",
  "properties": {
    "body": { "type": "string", "minLength": 1, "x-agentpm-persist": true }
  },
  "required": ["body"],
  "additionalProperties": false
}
JSON

  cat > "$dir/schemas/expiring-note.schema.json" <<'JSON'
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "type": "object",
  "title": "Expiring Note",
  "properties": {
    "body": { "type": "string", "minLength": 1, "x-agentpm-persist": true }
  },
  "required": ["body"],
  "additionalProperties": false
}
JSON

  "$APM" lint "$dir/agent.json" >/dev/null
  "$APM" memory build --manifest "$dir/agent.json" >/dev/null
}

write_agent() {
  cat > "$WORK/agent.json" <<JSON
{
  "\$schema": "$SCHEMA_URL",
  "kind": "agent",
  "name": "m14bc-agent",
  "version": "0.1.0",
  "description": "M14b/M14c manual Harness agent.",
  "tools": [],
  "knowledge": [],
  "memory": ["@zack/m14bc-memory@0.1.0"],
  "loop": "@zack/m14bc-loop@0.1.0",
  "bindings": {
    "global": {
      "memory": [
        {
          "package": "@zack/m14bc-memory",
          "spaces": [
            "conversation_state",
            "notes",
            "timeline",
            "append_log",
            "archive_notes",
            "delete_notes"
          ]
        }
      ]
    }
  }
}
JSON
  "$APM" lint "$WORK/agent.json" >/dev/null
}

write_lock() {
  cat > "$WORK/agent.lock" <<'JSON'
{
  "lockfile_version": 3,
  "generated": "2026-09-05T00:00:00Z",
  "packages": {
    "loop:@zack/m14bc-loop@0.1.0": {
      "kind": "loop",
      "name": "@zack/m14bc-loop",
      "version": "0.1.0",
      "integrity": "sha256-manual"
    },
    "memory:@zack/m14bc-memory@0.1.0": {
      "kind": "memory",
      "name": "@zack/m14bc-memory",
      "version": "0.1.0",
      "integrity": "sha256-manual"
    }
  },
  "roots": {
    "local:agent": {
      "name": "@zack/m14bc-agent",
      "version": "0.1.0",
      "tools": [],
      "skills": [],
      "knowledge": [],
      "memory": ["memory:@zack/m14bc-memory@0.1.0"],
      "profiles": [],
      "loop": "loop:@zack/m14bc-loop@0.1.0"
    }
  }
}
JSON
}

write_harness_config() {
  local provider="$1"
  local model="$2"
  local state_dir="$3"
  cat > "$WORK/agentpm.$provider.harness.json" <<JSON
{
  "version": 1,
  "model": {
    "provider": "$provider",
    "model": "$model"
  },
  "scopes": {
    "user": "m14bc-user",
    "conversation": "m14bc-conversation"
  },
  "runtime": {
    "state_dir": "$state_dir",
    "limits": {
      "max_steps": 4,
      "max_model_calls_per_phase": 8,
      "max_tool_calls_per_phase": 1,
      "max_actions_per_phase": 16,
      "max_structured_output_repairs": 3,
      "max_tool_call_repairs": 1
    }
  },
  "trace": {
    "level": "verbose"
  }
}
JSON
}

write_inputs() {
  cat > "$WORK/inputs/01-write-document-status.txt" <<'TXT'
Use direct Memory package @zack/m14bc-memory.

First call memory_write for space conversation_state with:
- operation: create
- record_type: status_summary
- content exactly:
  {
    "summary": "Alpha launch is green for manual M14bc",
    "risk_level": "low",
    "private_hint": "ephemeral-status-secret"
  }

Then complete with outcome done and include the returned record_id if available. Do not use external knowledge or tools.
TXT

  cat > "$WORK/inputs/02-replace-document-incident.txt" <<'TXT'
Use direct Memory package @zack/m14bc-memory.

Call memory_write for space conversation_state with:
- operation: upsert
- record_type: incident_summary
- content exactly:
  {
    "incident": "Alpha launch document was replaced by incident summary",
    "severity": 2,
    "private_hint": "ephemeral-incident-secret"
  }

This should replace the current document for the same trusted user+conversation scope, even though the record_type changes. Then complete with outcome done. Do not use external knowledge or tools.
TXT

  cat > "$WORK/inputs/03-write-notes.txt" <<'TXT'
Use direct Memory package @zack/m14bc-memory.

Create exactly three records in the notes space using memory_write operation create and record_type note:

1. {
  "body": "Alpha launch checklist Änderung marker",
  "status": "open",
  "labels": ["alpha", "bug", "release"],
  "assignee": { "team": "platform", "user": "sam" },
  "items": [{ "name": "smoke" }, { "name": "rollback" }],
  "scratch": { "public": "keep this public scratch", "private": "ephemeral-note-secret" }
}

2. {
  "body": "Beta planning note uses Straße spelling",
  "status": "open",
  "labels": ["beta"],
  "assignee": { "team": "docs", "user": "lee" },
  "items": [{ "name": "preview" }]
}

3. {
  "body": "Closed support note for manual testing",
  "status": "closed",
  "labels": ["support"],
  "assignee": { "team": "support", "user": "ren" },
  "items": [{ "name": "ticket" }]
}

After the Memory writes complete, complete with outcome done. Do not use external knowledge or tools.
TXT

  cat > "$WORK/inputs/04-read-filters.txt" <<'TXT'
Use direct Memory package @zack/m14bc-memory.

Run Memory reads against the notes space:

1. memory_read mode filter, record_type note, limit 5, filter:
   {
     "labels": "bug",
     "assignee.team": "platform",
     "items.name": "rollback",
     "status": "open"
   }

2. memory_read mode filter, record_type note, limit 5, filter:
   {
     "labels": ["alpha", "bug", "release"]
   }

3. memory_read mode filter, record_type note, limit 5, filter:
   {
     "labels": "missing-label"
   }

Then complete with outcome done and summarize the counts returned by the three reads. Do not use external knowledge or tools.
TXT

  cat > "$WORK/inputs/05-read-full-text.txt" <<'TXT'
Use direct Memory package @zack/m14bc-memory.

Run Memory reads against the notes space:

1. memory_read mode full_text, record_type note, limit 5, query: "änderung"
2. memory_read mode full_text, record_type note, limit 5, query: "strasse"

Then complete with outcome done and summarize the counts. The first should match "Änderung"; the second should not match "Straße" because full case folding is not part of Phase 7B full_text. Do not use external knowledge or tools.
TXT

  cat > "$WORK/inputs/06-sequence-create-read.txt" <<'TXT'
Use direct Memory package @zack/m14bc-memory.

First create two records in the timeline space using memory_write operation create and record_type event:

1. { "body": "first timeline event", "kind": "manual", "private_note": "ephemeral-sequence-secret-1" }
2. { "body": "second timeline event", "kind": "manual", "private_note": "ephemeral-sequence-secret-2" }

Then call memory_read for space timeline with mode chronological, record_type event, limit 5.

Then complete with outcome done and summarize the chronological records. Do not use external knowledge or tools.
TXT

  cat > "$WORK/inputs/07-capacity-overflow.txt" <<'TXT'
Use direct Memory package @zack/m14bc-memory.

The notes space already has capacity max_records=3. Try to create one more notes record using memory_write operation create and record_type note:

{
  "body": "This fourth note should exceed capacity",
  "status": "open",
  "labels": ["overflow"]
}

If Memory returns a capacity failure, report that failure and complete with outcome done. Do not use external knowledge or tools.
TXT

  cat > "$WORK/inputs/08-retention-write.txt" <<'TXT'
Use direct Memory package @zack/m14bc-memory.

Create one expiring_note in archive_notes and one expiring_note in delete_notes:

1. space archive_notes, operation create, record_type expiring_note, content:
   { "body": "archive me after ttl" }

2. space delete_notes, operation create, record_type expiring_note, content:
   { "body": "delete me after ttl" }

Then complete with outcome done. Do not use external knowledge or tools.
TXT

  cat > "$WORK/inputs/09-retention-read-after-ttl.txt" <<'TXT'
Use direct Memory package @zack/m14bc-memory.

Read both retention spaces to trigger lazy expiry:

1. memory_read space archive_notes, mode filter, record_type expiring_note, limit 5, filter:
   { "body": "archive me after ttl" }

2. memory_read space delete_notes, mode filter, record_type expiring_note, limit 5, filter:
   { "body": "delete me after ttl" }

Then complete with outcome done and summarize the counts. Do not use external knowledge or tools.
TXT

  cat > "$WORK/inputs/10-unknown-filter-path.txt" <<'TXT'
Use direct Memory package @zack/m14bc-memory.

First try a memory_read in the notes space using mode filter, record_type note, limit 5, and this filter:
{ "does_not_exist": "x" }

If Harness rejects that filter path and asks for repair, correct it by reading with this valid filter:
{ "status": "open" }

Then complete with outcome done and summarize what happened. Do not use external knowledge or tools.
TXT

  cat > "$WORK/inputs/11-read-document-after-restart.txt" <<'TXT'
Use direct Memory package @zack/m14bc-memory.

Read the current conversation_state document using memory_read mode key. Do not provide a record_id; this is a document space and should resolve the current document from trusted scope.

Then complete with outcome done and report the record_type and content fields returned. Do not use external knowledge or tools.
TXT
}

write_helpers() {
  cat > "$WORK/scripts/sqlite_memory_summary.py" <<'PY'
#!/usr/bin/env python3
import json
import sqlite3
import sys
from pathlib import Path

if len(sys.argv) != 2:
    print("usage: sqlite_memory_summary.py <state-dir-or-memory.sqlite3>", file=sys.stderr)
    raise SystemExit(2)

path = Path(sys.argv[1])
db = path / "memory.sqlite3" if path.is_dir() else path
if not db.exists():
    print(f"missing SQLite database: {db}", file=sys.stderr)
    raise SystemExit(1)

conn = sqlite3.connect(db)
conn.row_factory = sqlite3.Row
rows = conn.execute(
    """
    SELECT id, package, package_version, space, space_model, record_type,
           schema_version, scope_json, scope_hash, content_json, provenance_json,
           created_at, updated_at, expires_at, archived_at, ordinal
    FROM memory_records
    ORDER BY space ASC, COALESCE(ordinal, -1) ASC, created_at ASC, id ASC
    """
).fetchall()

records = []
for row in rows:
    item = dict(row)
    item["content"] = json.loads(item.pop("content_json"))
    item["provenance"] = json.loads(item.pop("provenance_json"))
    item["scope"] = json.loads(item["scope_json"])
    records.append(item)

print(json.dumps({"db": str(db), "count": len(records), "records": records}, indent=2, sort_keys=True))
PY
  chmod +x "$WORK/scripts/sqlite_memory_summary.py"

  cat > "$WORK/scripts/trace_summary.py" <<'PY'
#!/usr/bin/env python3
import json
import sys
from collections import Counter
from pathlib import Path

if len(sys.argv) != 2:
    print("usage: trace_summary.py <report.json-or-events.jsonl>", file=sys.stderr)
    raise SystemExit(2)

path = Path(sys.argv[1])
if path.suffix == ".json":
    report = json.loads(path.read_text())
    trace_path = Path(report["trace_path"])
else:
    trace_path = path

events = [json.loads(line) for line in trace_path.read_text().splitlines() if line.strip()]
counts = Counter(event["event_type"] for event in events)
actions = []
for event in events:
    payload = event.get("payload") or {}
    if payload.get("payload_type") == "action":
        actions.append(
            {
                "event_type": event["event_type"],
                "action_kind": payload.get("action_kind"),
                "identity": payload.get("identity"),
                "status": payload.get("status"),
                "fields": payload.get("fields", {}),
            }
        )

print(json.dumps({"trace": str(trace_path), "event_counts": dict(counts), "actions": actions}, indent=2, sort_keys=True))
PY
  chmod +x "$WORK/scripts/trace_summary.py"
}

write_loop
write_memory_package
write_agent
write_lock
write_harness_config "openai" "gpt-4o-mini" ".agentpm-state-openai"
write_harness_config "anthropic" "claude-3-5-sonnet-latest" ".agentpm-state-anthropic"
write_inputs
write_helpers

cat > "$BASE/README.txt" <<TXT
M14b/M14c manual workspace created at:
  $BASE

Useful paths:
  workspace:      $WORK
  run outputs:    $RUNS
  OpenAI state:   $WORK/.agentpm-state-openai
  Anthropic state:$WORK/.agentpm-state-anthropic
TXT

cat "$BASE/README.txt"
SH

bash /tmp/setup-harness-m14bc-manual.sh
```

Set helper variables for the remaining commands:

```bash
export HARNESS_M14BC_TEST_BASE="${HARNESS_M14BC_TEST_BASE:-$PWD/harness-m14bc-test}"
export M14BC_WORK="$HARNESS_M14BC_TEST_BASE/workspace"
export M14BC_RUNS="$HARNESS_M14BC_TEST_BASE/runs"
```

## 1. Blueprint lint/build and preflight

```bash
"$APM" lint "$M14BC_WORK/agent.json" \
  | tee "$M14BC_RUNS/lint-agent.txt"

"$APM" memory inspect "$M14BC_WORK/.agentpm/memory/zack/m14bc-memory/0.1.0" --json \
  >"$M14BC_RUNS/memory-inspect.json"

(cd "$M14BC_WORK" && "$APM" harness --config agentpm.openai.harness.json --scope user=m14bc-user --scope conversation=m14bc-conversation --verbose) \
  | tee "$M14BC_RUNS/preflight-openai.txt"

(cd "$M14BC_WORK" && "$APM" harness --config agentpm.anthropic.harness.json --scope user=m14bc-user --scope conversation=m14bc-conversation --verbose) \
  | tee "$M14BC_RUNS/preflight-anthropic.txt"
```

Expected:

- `agentpm lint` succeeds.
- `memory inspect` reports the generated contracts as current.
- both preflights reach `Status: Ready`.
- static details include Memory for `@zack/m14bc-memory`.
- no Memory surface is suppressed for missing local SQLite support.

## 2. Operator loop for each live Harness run

Use this pattern for every scenario below. Do not batch the scenarios while you are investigating M14b/M14c behavior; run one Harness command, inspect its output, then move to the next.

The setup script intentionally stops at fixture creation, lint/build, and read-only inspection helpers. From this point forward, the Harness runs are operator-driven so the report and trace for each scenario can be reviewed before continuing.

For OpenAI:

```bash
mkdir -p "$M14BC_RUNS/openai"
```

For Anthropic:

```bash
mkdir -p "$M14BC_RUNS/anthropic"
```

After each run, inspect the report and trace:

```bash
jq '{terminal_status, terminal_output, usage, memory_summaries, error_count, repair_count, trace_path}' "$REPORT"
"$AGENTPM_MANUAL_PYTHON" "$M14BC_WORK/scripts/trace_summary.py" "$REPORT" | less
```

Expected for normal successful scenarios:

- `terminal_status` is `ended`.
- `usage.memory_requests` increments when Memory actions were accepted.
- `usage.tool_calls` remains `0`; Memory actions are not Tools.
- trace contains `memory_surface_ready` plus the expected `memory_write_*` or `memory_read_*` events.
- later model requests include phase-local `ActionResult [memory_* ...]` entries.

If a live model chooses the wrong action or completes early, keep the report/trace and rerun the scenario. These are live model checks; deterministic edge-case pins are in the focused Rust tests at the end.

## 3. Document create and cross-record-type replacement

OpenAI document create:

```bash
REPORT="$M14BC_RUNS/openai/01-write-document-status.report.json"
(cd "$M14BC_WORK" && "$APM" harness \
  --config agentpm.openai.harness.json \
  --headless \
  --scope user=m14bc-user \
  --scope conversation=m14bc-conversation \
  --input-file inputs/01-write-document-status.txt \
  --report "$REPORT" \
  >"$M14BC_RUNS/openai/01-write-document-status.stdout.txt" \
  2>"$M14BC_RUNS/openai/01-write-document-status.stderr.txt")

jq '{terminal_status, terminal_output, usage, memory_summaries, trace_path}' "$REPORT"
"$AGENTPM_MANUAL_PYTHON" "$M14BC_WORK/scripts/trace_summary.py" "$REPORT" | less
```

OpenAI cross-record-type replacement:

```bash
REPORT="$M14BC_RUNS/openai/02-replace-document-incident.report.json"
(cd "$M14BC_WORK" && "$APM" harness \
  --config agentpm.openai.harness.json \
  --headless \
  --scope user=m14bc-user \
  --scope conversation=m14bc-conversation \
  --input-file inputs/02-replace-document-incident.txt \
  --report "$REPORT" \
  >"$M14BC_RUNS/openai/02-replace-document-incident.stdout.txt" \
  2>"$M14BC_RUNS/openai/02-replace-document-incident.stderr.txt")

jq '{terminal_status, terminal_output, usage, memory_summaries, trace_path}' "$REPORT"
"$AGENTPM_MANUAL_PYTHON" "$M14BC_WORK/scripts/trace_summary.py" "$REPORT" | less
```

Anthropic document create:

```bash
REPORT="$M14BC_RUNS/anthropic/01-write-document-status.report.json"
(cd "$M14BC_WORK" && "$APM" harness \
  --config agentpm.anthropic.harness.json \
  --headless \
  --scope user=m14bc-user \
  --scope conversation=m14bc-conversation \
  --input-file inputs/01-write-document-status.txt \
  --report "$REPORT" \
  >"$M14BC_RUNS/anthropic/01-write-document-status.stdout.txt" \
  2>"$M14BC_RUNS/anthropic/01-write-document-status.stderr.txt")

jq '{terminal_status, terminal_output, usage, memory_summaries, trace_path}' "$REPORT"
"$AGENTPM_MANUAL_PYTHON" "$M14BC_WORK/scripts/trace_summary.py" "$REPORT" | less
```

Anthropic cross-record-type replacement:

```bash
REPORT="$M14BC_RUNS/anthropic/02-replace-document-incident.report.json"
(cd "$M14BC_WORK" && "$APM" harness \
  --config agentpm.anthropic.harness.json \
  --headless \
  --scope user=m14bc-user \
  --scope conversation=m14bc-conversation \
  --input-file inputs/02-replace-document-incident.txt \
  --report "$REPORT" \
  >"$M14BC_RUNS/anthropic/02-replace-document-incident.stdout.txt" \
  2>"$M14BC_RUNS/anthropic/02-replace-document-incident.stderr.txt")

jq '{terminal_status, terminal_output, usage, memory_summaries, trace_path}' "$REPORT"
"$AGENTPM_MANUAL_PYTHON" "$M14BC_WORK/scripts/trace_summary.py" "$REPORT" | less
```

Inspect SQLite after both replacement runs:

```bash
"$AGENTPM_MANUAL_PYTHON" "$M14BC_WORK/scripts/sqlite_memory_summary.py" "$M14BC_WORK/.agentpm-state-openai" \
  | tee "$M14BC_RUNS/sqlite-openai-after-document.json" | less

"$AGENTPM_MANUAL_PYTHON" "$M14BC_WORK/scripts/sqlite_memory_summary.py" "$M14BC_WORK/.agentpm-state-anthropic" \
  | tee "$M14BC_RUNS/sqlite-anthropic-after-document.json" | less
```

Expected:

- each state DB has exactly one active `conversation_state` row for the trusted `user` + `conversation` scope.
- active document `record_type` is `incident_summary`, not `status_summary`.
- replacement happened through `upsert`; a repeated `create` for the same current document should be rejected rather than treated as replacement.
- the `private_hint` field is absent from durable `content`.
- `provenance.harness.kind` is `harness_direct_memory_write`.

## 4. Collection creates, durable projection, filters, and full_text

OpenAI create notes:

```bash
REPORT="$M14BC_RUNS/openai/03-write-notes.report.json"
(cd "$M14BC_WORK" && "$APM" harness \
  --config agentpm.openai.harness.json \
  --headless \
  --scope user=m14bc-user \
  --scope conversation=m14bc-conversation \
  --input-file inputs/03-write-notes.txt \
  --report "$REPORT" \
  >"$M14BC_RUNS/openai/03-write-notes.stdout.txt" \
  2>"$M14BC_RUNS/openai/03-write-notes.stderr.txt")

jq '{terminal_status, terminal_output, usage, memory_summaries, trace_path}' "$REPORT"
"$AGENTPM_MANUAL_PYTHON" "$M14BC_WORK/scripts/trace_summary.py" "$REPORT" | less
```

OpenAI filter reads:

```bash
REPORT="$M14BC_RUNS/openai/04-read-filters.report.json"
(cd "$M14BC_WORK" && "$APM" harness \
  --config agentpm.openai.harness.json \
  --headless \
  --scope user=m14bc-user \
  --scope conversation=m14bc-conversation \
  --input-file inputs/04-read-filters.txt \
  --report "$REPORT" \
  >"$M14BC_RUNS/openai/04-read-filters.stdout.txt" \
  2>"$M14BC_RUNS/openai/04-read-filters.stderr.txt")

jq '{terminal_status, terminal_output, usage, memory_summaries, trace_path}' "$REPORT"
"$AGENTPM_MANUAL_PYTHON" "$M14BC_WORK/scripts/trace_summary.py" "$REPORT" | less
```

OpenAI full_text reads:

```bash
REPORT="$M14BC_RUNS/openai/05-read-full-text.report.json"
(cd "$M14BC_WORK" && "$APM" harness \
  --config agentpm.openai.harness.json \
  --headless \
  --scope user=m14bc-user \
  --scope conversation=m14bc-conversation \
  --input-file inputs/05-read-full-text.txt \
  --report "$REPORT" \
  >"$M14BC_RUNS/openai/05-read-full-text.stdout.txt" \
  2>"$M14BC_RUNS/openai/05-read-full-text.stderr.txt")

jq '{terminal_status, terminal_output, usage, memory_summaries, trace_path}' "$REPORT"
"$AGENTPM_MANUAL_PYTHON" "$M14BC_WORK/scripts/trace_summary.py" "$REPORT" | less
```

Repeat the same three commands for Anthropic:

```bash
REPORT="$M14BC_RUNS/anthropic/03-write-notes.report.json"
(cd "$M14BC_WORK" && "$APM" harness \
  --config agentpm.anthropic.harness.json \
  --headless \
  --scope user=m14bc-user \
  --scope conversation=m14bc-conversation \
  --input-file inputs/03-write-notes.txt \
  --report "$REPORT" \
  >"$M14BC_RUNS/anthropic/03-write-notes.stdout.txt" \
  2>"$M14BC_RUNS/anthropic/03-write-notes.stderr.txt")

jq '{terminal_status, terminal_output, usage, memory_summaries, trace_path}' "$REPORT"
"$AGENTPM_MANUAL_PYTHON" "$M14BC_WORK/scripts/trace_summary.py" "$REPORT" | less

REPORT="$M14BC_RUNS/anthropic/04-read-filters.report.json"
(cd "$M14BC_WORK" && "$APM" harness \
  --config agentpm.anthropic.harness.json \
  --headless \
  --scope user=m14bc-user \
  --scope conversation=m14bc-conversation \
  --input-file inputs/04-read-filters.txt \
  --report "$REPORT" \
  >"$M14BC_RUNS/anthropic/04-read-filters.stdout.txt" \
  2>"$M14BC_RUNS/anthropic/04-read-filters.stderr.txt")

jq '{terminal_status, terminal_output, usage, memory_summaries, trace_path}' "$REPORT"
"$AGENTPM_MANUAL_PYTHON" "$M14BC_WORK/scripts/trace_summary.py" "$REPORT" | less

REPORT="$M14BC_RUNS/anthropic/05-read-full-text.report.json"
(cd "$M14BC_WORK" && "$APM" harness \
  --config agentpm.anthropic.harness.json \
  --headless \
  --scope user=m14bc-user \
  --scope conversation=m14bc-conversation \
  --input-file inputs/05-read-full-text.txt \
  --report "$REPORT" \
  >"$M14BC_RUNS/anthropic/05-read-full-text.stdout.txt" \
  2>"$M14BC_RUNS/anthropic/05-read-full-text.stderr.txt")

jq '{terminal_status, terminal_output, usage, memory_summaries, trace_path}' "$REPORT"
"$AGENTPM_MANUAL_PYTHON" "$M14BC_WORK/scripts/trace_summary.py" "$REPORT" | less
```

Inspect SQLite after note creation:

```bash
"$AGENTPM_MANUAL_PYTHON" "$M14BC_WORK/scripts/sqlite_memory_summary.py" "$M14BC_WORK/.agentpm-state-openai" \
  | tee "$M14BC_RUNS/sqlite-openai-after-notes.json" | less

"$AGENTPM_MANUAL_PYTHON" "$M14BC_WORK/scripts/sqlite_memory_summary.py" "$M14BC_WORK/.agentpm-state-anthropic" \
  | tee "$M14BC_RUNS/sqlite-anthropic-after-notes.json" | less
```

Expected:

- each DB has three active `notes` rows.
- no durable row contains `ephemeral-note-secret`.
- the Alpha note keeps `scratch.public` but removes `scratch.private`.
- filter trace results show counts `1`, `1`, `0` for:
  - conjunctive scalar/path/array-membership filter;
  - structural whole-array filter;
  - non-matching filter.
- full_text trace results show counts `1`, `0` for:
  - `änderung` matching stored `Änderung`;
  - `strasse` not matching stored `Straße`.

## 5. Sequence chronological read and stable ordinals

OpenAI:

```bash
REPORT="$M14BC_RUNS/openai/06-sequence-create-read.report.json"
(cd "$M14BC_WORK" && "$APM" harness \
  --config agentpm.openai.harness.json \
  --headless \
  --scope user=m14bc-user \
  --scope conversation=m14bc-conversation \
  --input-file inputs/06-sequence-create-read.txt \
  --report "$REPORT" \
  >"$M14BC_RUNS/openai/06-sequence-create-read.stdout.txt" \
  2>"$M14BC_RUNS/openai/06-sequence-create-read.stderr.txt")

jq '{terminal_status, terminal_output, usage, memory_summaries, trace_path}' "$REPORT"
"$AGENTPM_MANUAL_PYTHON" "$M14BC_WORK/scripts/trace_summary.py" "$REPORT" | less
```

Anthropic:

```bash
REPORT="$M14BC_RUNS/anthropic/06-sequence-create-read.report.json"
(cd "$M14BC_WORK" && "$APM" harness \
  --config agentpm.anthropic.harness.json \
  --headless \
  --scope user=m14bc-user-z \
  --scope conversation=m14bc-conversation-2 \
  --input-file inputs/06-sequence-create-read.txt \
  --report "$REPORT" \
  >"$M14BC_RUNS/anthropic/06-sequence-create-read.stdout.txt" \
  2>"$M14BC_RUNS/anthropic/06-sequence-create-read.stderr.txt")

jq '{terminal_status, terminal_output, usage, memory_summaries, trace_path}' "$REPORT"
"$AGENTPM_MANUAL_PYTHON" "$M14BC_WORK/scripts/trace_summary.py" "$REPORT" | less
```

Inspect SQLite:

```bash
"$AGENTPM_MANUAL_PYTHON" "$M14BC_WORK/scripts/sqlite_memory_summary.py" "$M14BC_WORK/.agentpm-state-openai" \
  | tee "$M14BC_RUNS/sqlite-openai-after-sequence.json" | less

"$AGENTPM_MANUAL_PYTHON" "$M14BC_WORK/scripts/sqlite_memory_summary.py" "$M14BC_WORK/.agentpm-state-anthropic" \
  | tee "$M14BC_RUNS/sqlite-anthropic-after-sequence.json" | less
```

Expected:

- `timeline` rows have ordinals `0` and `1`.
- chronological read returns `first timeline event` before `second timeline event`.
- `private_note` is absent from durable sequence content.

## 6. Capacity overflow is typed and non-destructive

OpenAI:

```bash
REPORT="$M14BC_RUNS/openai/07-capacity-overflow.report.json"
(cd "$M14BC_WORK" && "$APM" harness \
  --config agentpm.openai.harness.json \
  --headless \
  --scope user=m14bc-user \
  --scope conversation=m14bc-conversation \
  --input-file inputs/07-capacity-overflow.txt \
  --report "$REPORT" \
  >"$M14BC_RUNS/openai/07-capacity-overflow.stdout.txt" \
  2>"$M14BC_RUNS/openai/07-capacity-overflow.stderr.txt")

jq '{terminal_status, terminal_output, usage, memory_summaries, trace_path}' "$REPORT"
"$AGENTPM_MANUAL_PYTHON" "$M14BC_WORK/scripts/trace_summary.py" "$REPORT" | less
```

Anthropic:

```bash
REPORT="$M14BC_RUNS/anthropic/07-capacity-overflow.report.json"
(cd "$M14BC_WORK" && "$APM" harness \
  --config agentpm.anthropic.harness.json \
  --headless \
  --scope user=m14bc-user \
  --scope conversation=m14bc-conversation \
  --input-file inputs/07-capacity-overflow.txt \
  --report "$REPORT" \
  >"$M14BC_RUNS/anthropic/07-capacity-overflow.stdout.txt" \
  2>"$M14BC_RUNS/anthropic/07-capacity-overflow.stderr.txt")

jq '{terminal_status, terminal_output, usage, memory_summaries, trace_path}' "$REPORT"
"$AGENTPM_MANUAL_PYTHON" "$M14BC_WORK/scripts/trace_summary.py" "$REPORT" | less
```

Inspect SQLite:

```bash
"$AGENTPM_MANUAL_PYTHON" "$M14BC_WORK/scripts/sqlite_memory_summary.py" "$M14BC_WORK/.agentpm-state-openai" \
  | tee "$M14BC_RUNS/sqlite-openai-after-capacity.json" | less

"$AGENTPM_MANUAL_PYTHON" "$M14BC_WORK/scripts/sqlite_memory_summary.py" "$M14BC_WORK/.agentpm-state-anthropic" \
  | tee "$M14BC_RUNS/sqlite-anthropic-after-capacity.json" | less
```

Expected:

- Memory action result includes `ok: false`.
- structured error code is `capacity_exceeded`.
- run can still complete with outcome `done`.
- active `notes` count remains `3`.
- the rejected fourth note is not persisted.

## 7. TTL archive/delete retention

Write short-lived records:

```bash
REPORT="$M14BC_RUNS/openai/08-retention-write.report.json"
(cd "$M14BC_WORK" && "$APM" harness \
  --config agentpm.openai.harness.json \
  --headless \
  --scope user=m14bc-user \
  --scope conversation=m14bc-conversation \
  --input-file inputs/08-retention-write.txt \
  --report "$REPORT" \
  >"$M14BC_RUNS/openai/08-retention-write.stdout.txt" \
  2>"$M14BC_RUNS/openai/08-retention-write.stderr.txt")

jq '{terminal_status, terminal_output, usage, memory_summaries, trace_path}' "$REPORT"

REPORT="$M14BC_RUNS/anthropic/08-retention-write.report.json"
(cd "$M14BC_WORK" && "$APM" harness \
  --config agentpm.anthropic.harness.json \
  --headless \
  --scope user=m14bc-user \
  --scope conversation=m14bc-conversation \
  --input-file inputs/08-retention-write.txt \
  --report "$REPORT" \
  >"$M14BC_RUNS/anthropic/08-retention-write.stdout.txt" \
  2>"$M14BC_RUNS/anthropic/08-retention-write.stderr.txt")

jq '{terminal_status, terminal_output, usage, memory_summaries, trace_path}' "$REPORT"
```

Wait for the one-second TTL to expire:

```bash
sleep 2
```

Read each retention space to trigger lazy expiry:

```bash
REPORT="$M14BC_RUNS/openai/09-retention-read-after-ttl.report.json"
(cd "$M14BC_WORK" && "$APM" harness \
  --config agentpm.openai.harness.json \
  --headless \
  --scope user=m14bc-user \
  --scope conversation=m14bc-conversation \
  --input-file inputs/09-retention-read-after-ttl.txt \
  --report "$REPORT" \
  >"$M14BC_RUNS/openai/09-retention-read-after-ttl.stdout.txt" \
  2>"$M14BC_RUNS/openai/09-retention-read-after-ttl.stderr.txt")

jq '{terminal_status, terminal_output, usage, memory_summaries, trace_path}' "$REPORT"
"$AGENTPM_MANUAL_PYTHON" "$M14BC_WORK/scripts/trace_summary.py" "$REPORT" | less

REPORT="$M14BC_RUNS/anthropic/09-retention-read-after-ttl.report.json"
(cd "$M14BC_WORK" && "$APM" harness \
  --config agentpm.anthropic.harness.json \
  --headless \
  --scope user=m14bc-user \
  --scope conversation=m14bc-conversation \
  --input-file inputs/09-retention-read-after-ttl.txt \
  --report "$REPORT" \
  >"$M14BC_RUNS/anthropic/09-retention-read-after-ttl.stdout.txt" \
  2>"$M14BC_RUNS/anthropic/09-retention-read-after-ttl.stderr.txt")

jq '{terminal_status, terminal_output, usage, memory_summaries, trace_path}' "$REPORT"
"$AGENTPM_MANUAL_PYTHON" "$M14BC_WORK/scripts/trace_summary.py" "$REPORT" | less
```

Inspect SQLite:

```bash
"$AGENTPM_MANUAL_PYTHON" "$M14BC_WORK/scripts/sqlite_memory_summary.py" "$M14BC_WORK/.agentpm-state-openai" \
  | tee "$M14BC_RUNS/sqlite-openai-after-retention.json" | less

"$AGENTPM_MANUAL_PYTHON" "$M14BC_WORK/scripts/sqlite_memory_summary.py" "$M14BC_WORK/.agentpm-state-anthropic" \
  | tee "$M14BC_RUNS/sqlite-anthropic-after-retention.json" | less
```

Expected:

- `archive_notes` expired record remains stored with `archived_at` set and is excluded from normal active reads.
- `delete_notes` expired record is physically removed.
- read trace shows zero active records returned from both retention spaces after lazy expiry.
- unrelated `notes`, `conversation_state`, and `timeline` records remain intact.

## 8. Unknown filter path repair/rejection

OpenAI:

```bash
REPORT="$M14BC_RUNS/openai/10-unknown-filter-path.report.json"
(cd "$M14BC_WORK" && "$APM" harness \
  --config agentpm.openai.harness.json \
  --headless \
  --scope user=m14bc-user \
  --scope conversation=m14bc-conversation \
  --input-file inputs/10-unknown-filter-path.txt \
  --report "$REPORT" \
  >"$M14BC_RUNS/openai/10-unknown-filter-path.stdout.txt" \
  2>"$M14BC_RUNS/openai/10-unknown-filter-path.stderr.txt")

jq '{terminal_status, terminal_output, usage, memory_summaries, error_count, repair_count, trace_path}' "$REPORT"
"$AGENTPM_MANUAL_PYTHON" "$M14BC_WORK/scripts/trace_summary.py" "$REPORT" | less
```

Anthropic:

```bash
REPORT="$M14BC_RUNS/anthropic/10-unknown-filter-path.report.json"
(cd "$M14BC_WORK" && "$APM" harness \
  --config agentpm.anthropic.harness.json \
  --headless \
  --scope user=m14bc-user \
  --scope conversation=m14bc-conversation \
  --input-file inputs/10-unknown-filter-path.txt \
  --report "$REPORT" \
  >"$M14BC_RUNS/anthropic/10-unknown-filter-path.stdout.txt" \
  2>"$M14BC_RUNS/anthropic/10-unknown-filter-path.stderr.txt")

jq '{terminal_status, terminal_output, usage, memory_summaries, error_count, repair_count, trace_path}' "$REPORT"
"$AGENTPM_MANUAL_PYTHON" "$M14BC_WORK/scripts/trace_summary.py" "$REPORT" | less
```

Expected:

- if the model emits the invalid `does_not_exist` filter path, trace contains `semantic_action_rejected` and usually `model_repair_requested`.
- if repaired, the corrected `status` filter can read active notes.
- the invalid path is not silently treated as a legitimate empty result.

This check is model-dependent. If a model self-corrects before emitting the invalid action, the trace may not include a rejection; the focused Rust tests are the deterministic acceptance pin for this behavior.

## 9. Restart/readback

Run readback after the previous Harness processes have exited. This proves durable records survive process restart and are not held only in the phase transcript.

OpenAI:

```bash
REPORT="$M14BC_RUNS/openai/11-read-document-after-restart.report.json"
(cd "$M14BC_WORK" && "$APM" harness \
  --config agentpm.openai.harness.json \
  --headless \
  --scope user=m14bc-user \
  --scope conversation=m14bc-conversation \
  --input-file inputs/11-read-document-after-restart.txt \
  --report "$REPORT" \
  >"$M14BC_RUNS/openai/11-read-document-after-restart.stdout.txt" \
  2>"$M14BC_RUNS/openai/11-read-document-after-restart.stderr.txt")

jq '{terminal_status, terminal_output, usage, memory_summaries, trace_path}' "$REPORT"
"$AGENTPM_MANUAL_PYTHON" "$M14BC_WORK/scripts/trace_summary.py" "$REPORT" | less
```

Anthropic:

```bash
REPORT="$M14BC_RUNS/anthropic/11-read-document-after-restart.report.json"
(cd "$M14BC_WORK" && "$APM" harness \
  --config agentpm.anthropic.harness.json \
  --headless \
  --scope user=m14bc-user \
  --scope conversation=m14bc-conversation \
  --input-file inputs/11-read-document-after-restart.txt \
  --report "$REPORT" \
  >"$M14BC_RUNS/anthropic/11-read-document-after-restart.stdout.txt" \
  2>"$M14BC_RUNS/anthropic/11-read-document-after-restart.stderr.txt")

jq '{terminal_status, terminal_output, usage, memory_summaries, trace_path}' "$REPORT"
"$AGENTPM_MANUAL_PYTHON" "$M14BC_WORK/scripts/trace_summary.py" "$REPORT" | less
```

Expected:

- each provider reads the current `conversation_state` document from its SQLite state directory.
- returned record type is `incident_summary`.
- returned content does not include `private_hint`.

## 10. Focused automated checks to run with the manual pass

These are not a replacement for the live OpenAI/Anthropic runs above. They pin deterministic M14b/M14c edge cases that are difficult to force through live models every time.

```bash
cargo test -p agentpm-cli sqlite_direct -- --nocapture \
  | tee "$M14BC_RUNS/cargo-memory-runtime-tests.txt"

cargo test -p agentpm-cli memory_ -- --nocapture \
  | tee "$M14BC_RUNS/cargo-memory-engine-tests.txt"

cargo test -p agentpm-cli action_parameter_schemas_use_resolved_tool_and_skill_metadata -- --nocapture \
  | tee "$M14BC_RUNS/cargo-provider-memory-schema-tests.txt"

cargo test -p agentpm-cli memory_write_provider_schema_simplifies_multi_record_content_for_compatibility -- --nocapture \
  | tee -a "$M14BC_RUNS/cargo-provider-memory-schema-tests.txt"
```

Expected:

- local SQLite document/collection/sequence/capacity/retention/governance tests pass.
- HarnessEngine direct Memory routing, repair, provenance, transcript, and action-limit tests pass.
- provider-facing Memory read/write schemas are bound-space-aware.
- append-only write schemas advertise `create` only.

If a test filter does not match because Rust test names changed, run the broader focused commands instead:

```bash
cargo test -p agentpm-cli harness_runtime::memory -- --nocapture
cargo test -p agentpm-cli harness_engine -- --nocapture
cargo test -p agentpm-cli harness_runtime::provider -- --nocapture
```

## 11. Final verification commands

Run these before handing off M14b/M14c:

```bash
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test -p agentpm-cli sqlite_direct -- --nocapture
cargo test -p agentpm-cli memory_ -- --nocapture
cargo test -p agentpm-cli action_parameter_schemas_use_resolved_tool_and_skill_metadata -- --nocapture
cargo test -p agentpm-cli memory_write_provider_schema_simplifies_multi_record_content_for_compatibility -- --nocapture
```

If your local environment permits the filesystem fixture operations:

```bash
cargo test -p agentpm-cli
```

Expected:

- formatter check passes.
- clippy passes with `-D warnings`.
- focused Memory runtime, Engine, and provider schema tests pass.
- full `agentpm-cli` test suite passes, or any failure is clearly unrelated to M14b/M14c and documented.

## Cleanup

```bash
rm -rf "$HARNESS_M14BC_TEST_BASE"
rm -f /tmp/setup-harness-m14bc-manual.sh
```
