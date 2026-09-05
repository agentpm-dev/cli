# Harness Milestone 14a Manual Tests

Manual coverage for Harness M14a: MemoryRuntime foundation, generated Memory contract validation, local SQLite substrate, and preflight/readiness inputs.

M14a deliberately does not expose model-facing direct Memory actions yet. These checks therefore focus on the surfaces that are live after M14a:

- Memory Blueprint lint/build validation for `x-agentpm-persist:false`
- generated Memory contract freshness/read-only validation
- Harness preflight readiness/suppression for selected Memory spaces
- local SQLite Memory substrate tests for schema, migrations, scope hashing, active reads, atomic batches, WAL/busy timeout, and restart persistence
- `.agentpm/` installed package immutability during Harness preflight/contract validation

Not covered here:

- model-selected `MemoryRead` / `MemoryWrite` actions, which begin in M14c
- local SQLite retention/capacity/direct CRUD semantics, which are M14b
- semantic vectors, which are M14d
- custom process/host MemoryRuntime routing, which is M14e
- Memory Hooks, which are M14f
- lifecycle operation scheduling/triggers, which are M15

## Prerequisites

From the root of the `agentpm` repo:

```bash
cargo build -p agentpm-cli
export APM="$PWD/target/debug/agentpm"
export AGENTPM_MANUAL_PYTHON="${AGENTPM_MANUAL_PYTHON:-python3}"
```

These tests do not require OpenAI, Anthropic, Pinecone, pgvector, Redis, or registry publishing.

## Setup

Run this from the root of the `agentpm` repo. It creates `harness-m14a-test/` with:

- a valid Harness workspace using a local SQLite-realizable Memory space
- an unrealizable Harness workspace using `full_text` retrieval, which M14a local SQLite must suppress
- valid and invalid standalone Memory Blueprint packages for lint/build checks
- a small snapshot helper for proving `.agentpm/` is not mutated by preflight

```bash
cat > /tmp/setup-harness-m14a-manual.sh <<'SH'
#!/usr/bin/env bash
set -euo pipefail

ROOT="$(pwd)"
APM="${APM:-$ROOT/target/debug/agentpm}"
BASE="${HARNESS_M14A_TEST_BASE:-$ROOT/harness-m14a-test}"
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

mkdir -p "$BASE/scripts" "$BASE/runs"

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

write_loop() {
  local workspace="$1"
  local dir
  dir="$(pkg_dir "$workspace" loops '@zack/m14a-loop' '0.1.0')"
  mkdir -p "$dir"
  cat > "$dir/agent.json" <<JSON
{
  "\$schema": "$SCHEMA_URL",
  "kind": "loop",
  "name": "m14a-loop",
  "version": "0.1.0",
  "description": "M14a manual preflight loop.",
  "loop": {
    "entry_phase": "inspect",
    "phases": [
      {
        "id": "inspect",
        "objective": "Inspect M14a preflight state.",
        "outcomes": [
          { "id": "complete", "description": "Complete." }
        ]
      }
    ],
    "transitions": [
      { "from": "inspect", "on": "complete", "to": "\$end" }
    ]
  }
}
JSON
  "$APM" lint "$dir/agent.json" >/dev/null
}

write_harness_config() {
  local workspace="$1"
  cat > "$workspace/agentpm.harness.json" <<JSON
{
  "version": 1,
  "model": {
    "provider": "openai",
    "model": "gpt-4o-mini"
  },
  "scopes": {
    "user": "manual-user-1"
  },
  "runtime": {
    "state_dir": ".agentpm-state"
  },
  "trace": {
    "level": "verbose"
  }
}
JSON
}

write_lock() {
  local workspace="$1"
  local memory_package="$2"
  cat > "$workspace/agent.lock" <<JSON
{
  "lockfile_version": 3,
  "generated": "2026-09-04T00:00:00Z",
  "packages": {
    "loop:@zack/m14a-loop@0.1.0": {
      "kind": "loop",
      "name": "@zack/m14a-loop",
      "version": "0.1.0",
      "integrity": "sha256-manual"
    },
    "memory:$memory_package@0.1.0": {
      "kind": "memory",
      "name": "$memory_package",
      "version": "0.1.0",
      "integrity": "sha256-manual"
    }
  },
  "roots": {
    "local:agent": {
      "name": "@zack/m14a-agent",
      "version": "0.1.0",
      "tools": [],
      "skills": [],
      "knowledge": [],
      "memory": ["memory:$memory_package@0.1.0"],
      "profiles": [],
      "loop": "loop:@zack/m14a-loop@0.1.0"
    }
  }
}
JSON
}

write_agent() {
  local workspace="$1"
  local memory_package="$2"
  cat > "$workspace/agent.json" <<JSON
{
  "\$schema": "$SCHEMA_URL",
  "kind": "agent",
  "name": "m14a-agent",
  "version": "0.1.0",
  "description": "M14a manual Harness agent.",
  "tools": [],
  "memory": ["$memory_package@0.1.0"],
  "loop": "@zack/m14a-loop@0.1.0",
  "bindings": {
    "global": {
      "memory": [
        { "package": "$memory_package", "spaces": ["session"] }
      ]
    }
  }
}
JSON
  "$APM" lint "$workspace/agent.json" >/dev/null
}

write_installed_memory() {
  local workspace="$1"
  local package="$2"
  local retrieval_mode="$3"
  local dir
  local manifest_name
  dir="$(pkg_dir "$workspace" memory "$package" '0.1.0')"
  manifest_name="${package#*/}"
  mkdir -p "$dir/schemas"
  cat > "$dir/agent.json" <<JSON
{
  "\$schema": "$SCHEMA_URL",
  "kind": "memory",
  "name": "$manifest_name",
  "version": "0.1.0",
  "description": "M14a manual Memory Blueprint.",
  "memory": {
    "scopes": {
      "user": { "description": "User scope." }
    },
    "record_types": {
      "note": {
        "schema": "schemas/note.schema.json",
        "version": "1.0.0",
        "description": "Manual note."
      }
    },
    "spaces": {
      "session": {
        "model": "collection",
        "scope": ["user"],
        "retrieval": { "modes": ["$retrieval_mode"] },
        "description": "Session memory.",
        "record_types": ["note"]
      }
    }
  }
}
JSON
  cat > "$dir/schemas/note.schema.json" <<'JSON'
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "type": "object",
  "properties": {
    "body": {
      "type": "string",
      "minLength": 1,
      "x-agentpm-persist": true
    },
    "ephemeral_hint": {
      "type": "string",
      "x-agentpm-persist": false
    }
  },
  "required": ["body"],
  "additionalProperties": false
}
JSON
  "$APM" lint "$dir/agent.json" >/dev/null
  "$APM" memory build --manifest "$dir/agent.json" >/dev/null
}

write_workspace() {
  local workspace="$1"
  local memory_package="$2"
  local retrieval_mode="$3"
  mkdir -p "$workspace"
  write_loop "$workspace"
  write_installed_memory "$workspace" "$memory_package" "$retrieval_mode"
  write_agent "$workspace" "$memory_package"
  write_lock "$workspace" "$memory_package"
  write_harness_config "$workspace"
}

write_memory_case() {
  local dir="$1"
  local package="$2"
  local schema_json="$3"
  local manifest_name
  manifest_name="${package#*/}"
  mkdir -p "$dir/schemas"
  cat > "$dir/agent.json" <<JSON
{
  "\$schema": "$SCHEMA_URL",
  "kind": "memory",
  "name": "$manifest_name",
  "version": "0.1.0",
  "description": "M14a persistence governance case.",
  "memory": {
    "scopes": {
      "user": { "description": "User scope." }
    },
    "record_types": {
      "note": {
        "schema": "schemas/note.schema.json",
        "version": "1.0.0",
        "description": "Note."
      }
    },
    "spaces": {
      "notes": {
        "model": "collection",
        "scope": ["user"],
        "retrieval": { "modes": ["key"] },
        "description": "Notes.",
        "record_types": ["note"]
      }
    }
  }
}
JSON
  printf '%s\n' "$schema_json" > "$dir/schemas/note.schema.json"
}

write_memory_case "$BASE/valid-optional-nonpersist" "@zack/m14a-valid-optional" '{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "type": "object",
  "properties": {
    "body": { "type": "string", "minLength": 1, "x-agentpm-persist": true },
    "draft_hint": { "type": "string", "x-agentpm-persist": false }
  },
  "required": ["body"],
  "additionalProperties": false
}'
"$APM" lint "$BASE/valid-optional-nonpersist/agent.json" >/dev/null

write_memory_case "$BASE/invalid-direct-required-nonpersist" "@zack/m14a-invalid-direct" '{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "type": "object",
  "properties": {
    "body": { "type": "string", "minLength": 1 },
    "secret": { "type": "string", "x-agentpm-persist": false }
  },
  "required": ["body", "secret"],
  "additionalProperties": false
}'

write_memory_case "$BASE/invalid-nested-required-nonpersist" "@zack/m14a-invalid-nested" '{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "type": "object",
  "properties": {
    "body": { "type": "string", "minLength": 1 },
    "nested": {
      "type": "object",
      "properties": {
        "ephemeral": { "type": "string", "x-agentpm-persist": false }
      },
      "required": ["ephemeral"],
      "additionalProperties": false
    }
  },
  "required": ["body", "nested"],
  "additionalProperties": false
}'

write_memory_case "$BASE/complex-composition-runtime-validated" "@zack/m14a-complex-composition" '{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "type": "object",
  "properties": {
    "body": { "type": "string", "minLength": 1 },
    "maybe": {
      "anyOf": [
        {
          "type": "object",
          "properties": {
            "ephemeral": { "type": "string", "x-agentpm-persist": false }
          },
          "required": ["ephemeral"],
          "additionalProperties": false
        },
        {
          "type": "object",
          "properties": {
            "durable": { "type": "string" }
          },
          "required": ["durable"],
          "additionalProperties": false
        }
      ]
    }
  },
  "required": ["body"],
  "additionalProperties": false
}'
"$APM" lint "$BASE/complex-composition-runtime-validated/agent.json" >/dev/null

READY="$BASE/workspace-ready"
UNREALIZABLE="$BASE/workspace-unrealizable"
write_workspace "$READY" "@zack/m14a-memory" "key"
write_workspace "$UNREALIZABLE" "@zack/m14a-unrealizable-memory" "full_text"

cat > "$BASE/scripts/snapshot_tree.py" <<'PY'
#!/usr/bin/env python3
import hashlib
import json
import sys
from pathlib import Path

root = Path(sys.argv[1]).resolve()
entries = {}
for path in sorted(root.rglob("*")):
    stat = path.stat()
    rel = path.relative_to(root).as_posix()
    item = {
        "type": "dir" if path.is_dir() else "file",
        "size": stat.st_size,
        "mtime_ns": stat.st_mtime_ns,
    }
    if path.is_file():
        item["sha256"] = hashlib.sha256(path.read_bytes()).hexdigest()
    entries[rel] = item
print(json.dumps(entries, sort_keys=True, indent=2))
PY
chmod +x "$BASE/scripts/snapshot_tree.py"

cat > "$BASE/README.txt" <<TXT
M14a manual workspace created at:
  $BASE

Useful paths:
  ready workspace:        $READY
  unrealizable workspace: $UNREALIZABLE
  run outputs:            $BASE/runs
TXT

cat "$BASE/README.txt"
SH

bash /tmp/setup-harness-m14a-manual.sh
```

Set helper variables for the remaining commands:

```bash
export HARNESS_M14A_TEST_BASE="${HARNESS_M14A_TEST_BASE:-$PWD/harness-m14a-test}"
export M14A_RUNS="$HARNESS_M14A_TEST_BASE/runs"
export M14A_READY="$HARNESS_M14A_TEST_BASE/workspace-ready"
export M14A_UNREALIZABLE="$HARNESS_M14A_TEST_BASE/workspace-unrealizable"
```

## 1. Valid optional non-persistable fields pass lint and build

```bash
"$APM" lint "$HARNESS_M14A_TEST_BASE/valid-optional-nonpersist/agent.json" \
  | tee "$M14A_RUNS/lint-valid-optional.txt"

"$APM" memory build --manifest "$HARNESS_M14A_TEST_BASE/valid-optional-nonpersist/agent.json" \
  | tee "$M14A_RUNS/build-valid-optional.txt"
```

Expected:

- lint succeeds
- build succeeds
- generated `memory/build.json` and `memory/contracts/index.json` exist under `valid-optional-nonpersist/memory/`

## 2. Direct required `x-agentpm-persist:false` conflict fails lint and build

```bash
set +e
"$APM" lint "$HARNESS_M14A_TEST_BASE/invalid-direct-required-nonpersist/agent.json" \
  >"$M14A_RUNS/lint-invalid-direct.stdout.txt" \
  2>"$M14A_RUNS/lint-invalid-direct.stderr.txt"
direct_lint_status=$?

"$APM" memory build --manifest "$HARNESS_M14A_TEST_BASE/invalid-direct-required-nonpersist/agent.json" \
  >"$M14A_RUNS/build-invalid-direct.stdout.txt" \
  2>"$M14A_RUNS/build-invalid-direct.stderr.txt"
direct_build_status=$?
set -e

echo "direct lint status: $direct_lint_status"
echo "direct build status: $direct_build_status"
```

Expected:

- both statuses are non-zero
- output mentions the required non-persistable field conflict
- build fails even if lint was skipped

## 3. Nested required `x-agentpm-persist:false` conflict fails lint and build

```bash
set +e
"$APM" lint "$HARNESS_M14A_TEST_BASE/invalid-nested-required-nonpersist/agent.json" \
  >"$M14A_RUNS/lint-invalid-nested.stdout.txt" \
  2>"$M14A_RUNS/lint-invalid-nested.stderr.txt"
nested_lint_status=$?

"$APM" memory build --manifest "$HARNESS_M14A_TEST_BASE/invalid-nested-required-nonpersist/agent.json" \
  >"$M14A_RUNS/build-invalid-nested.stdout.txt" \
  2>"$M14A_RUNS/build-invalid-nested.stderr.txt"
nested_build_status=$?
set -e

echo "nested lint status: $nested_lint_status"
echo "nested build status: $nested_build_status"
```

Expected:

- both statuses are non-zero
- output identifies the nested non-persistable required property

## 4. Complex composition is not over-rejected statically

```bash
"$APM" lint "$HARNESS_M14A_TEST_BASE/complex-composition-runtime-validated/agent.json" \
  | tee "$M14A_RUNS/lint-complex-composition.txt"

"$APM" memory build --manifest "$HARNESS_M14A_TEST_BASE/complex-composition-runtime-validated/agent.json" \
  | tee "$M14A_RUNS/build-complex-composition.txt"
```

Expected:

- lint succeeds
- build succeeds
- this confirms authoring validation does not attempt broad Draft 2020-12 satisfiability solving for `anyOf`
- runtime durable-projection validation remains the later final authority for actual record instances

## 5. Preflight exposes a realizable Memory binding without suppressing it

These commands intentionally omit `--headless` and `--machine`. That selects the current default TUI surface. In M14a the TUI runner is still a no-op after preflight, so the command resolves the Harness plan, prints/returns preflight state, and exits without starting a model Run.

```bash
(cd "$M14A_READY" && "$APM" harness --config agentpm.harness.json --scope user=manual-user-1 --verbose) \
  | tee "$M14A_RUNS/preflight-ready.txt"

(cd "$M14A_READY" && "$APM" harness --config agentpm.harness.json --scope user=manual-user-1 --json) \
  >"$M14A_RUNS/preflight-ready.json"
```

Expected:

- human preflight reaches `Status: Ready`
- no `unrealizable_memory_space` diagnostic appears
- verbose static capability details include Memory `@zack/m14a-memory` as `pending runtime activation`
- preflight reports the resolved runtime state dir as `.agentpm-state`, not `.agentpm`; M14a preflight may not physically create the directory because model-facing Memory execution is not active yet

Quick machine check:

```bash
"$AGENTPM_MANUAL_PYTHON" - "$M14A_RUNS/preflight-ready.json" <<'PY'
import json
import sys

report = json.load(open(sys.argv[1]))
diagnostics = report.get("diagnostics", [])

assert report.get("status") == "ready", report.get("status")
assert not any(d.get("code") == "unrealizable_memory_space" for d in diagnostics), diagnostics

print("ready preflight diagnostics verified")
PY

grep -F 'pending runtime activation: memory `@zack/m14a-memory`' "$M14A_RUNS/preflight-ready.txt"
```

## 6. Preflight suppresses a selected Memory space requiring unsupported local capabilities

The unrealizable workspace selects a Memory collection whose retrieval mode is `full_text`. M14a local SQLite advertises only the capabilities it actually has at this milestone, so this selected space should be suppressed rather than exposed as a late-failing action.

```bash
(cd "$M14A_UNREALIZABLE" && "$APM" harness --config agentpm.harness.json --scope user=manual-user-1 --verbose) \
  | tee "$M14A_RUNS/preflight-unrealizable.txt"

(cd "$M14A_UNREALIZABLE" && "$APM" harness --config agentpm.harness.json --scope user=manual-user-1 --json) \
  >"$M14A_RUNS/preflight-unrealizable.json"
```

Expected:

- preflight does not crash
- output/report includes `unrealizable_memory_space`
- diagnostic mentions unsupported `FullText` retrieval
- verbose static capability details include suppressed Memory `@zack/m14a-unrealizable-memory`

Quick machine check:

```bash
"$AGENTPM_MANUAL_PYTHON" - "$M14A_RUNS/preflight-unrealizable.json" <<'PY'
import json
import sys

report = json.load(open(sys.argv[1]))
diagnostics = report.get("diagnostics", [])

assert any(d.get("code") == "unrealizable_memory_space" for d in diagnostics), diagnostics
assert any("FullText" in d.get("message", "") for d in diagnostics), diagnostics

print("unrealizable memory diagnostic verified")
PY

grep -F 'suppressed: memory `@zack/m14a-unrealizable-memory`' "$M14A_RUNS/preflight-unrealizable.txt"
```

## 7. Harness preflight does not mutate installed `.agentpm` package roots

The setup step intentionally runs `agentpm memory build` inside installed Memory package roots to create generated artifacts. This test snapshots `.agentpm/` after setup, runs Harness preflight, and verifies preflight did not rewrite package files, generated contracts, or build metadata.

```bash
"$AGENTPM_MANUAL_PYTHON" "$HARNESS_M14A_TEST_BASE/scripts/snapshot_tree.py" "$M14A_READY/.agentpm" \
  >"$M14A_RUNS/agentpm-before-preflight.json"

sleep 1

(cd "$M14A_READY" && "$APM" harness --config agentpm.harness.json --scope user=manual-user-1 --verbose) \
  >"$M14A_RUNS/preflight-no-mutation.txt"

"$AGENTPM_MANUAL_PYTHON" "$HARNESS_M14A_TEST_BASE/scripts/snapshot_tree.py" "$M14A_READY/.agentpm" \
  >"$M14A_RUNS/agentpm-after-preflight.json"

diff -u "$M14A_RUNS/agentpm-before-preflight.json" "$M14A_RUNS/agentpm-after-preflight.json"
```

Expected:

- `diff` has no output and exits `0`
- `.agentpm/` remains immutable installed package state
- if any runtime state is created later, it belongs under `.agentpm-state/`

## 8. Generated Memory contract integrity failures are detected

```bash
BROKEN="$HARNESS_M14A_TEST_BASE/broken-generated-contract"
cp -R "$HARNESS_M14A_TEST_BASE/valid-optional-nonpersist" "$BROKEN"
printf '{"type":"object"}\n' > "$BROKEN/memory/contracts/notes.note.schema.json"

"$APM" memory inspect "$BROKEN" --json \
  >"$M14A_RUNS/inspect-broken-generated-contract.json"

"$AGENTPM_MANUAL_PYTHON" - "$M14A_RUNS/inspect-broken-generated-contract.json" <<'PY'
import json
import sys

report = json.load(open(sys.argv[1]))
assert report["status"] in {"stale", "invalid"}, report["status"]
assert report.get("mismatches"), report
print(f"broken generated contract detected as {report['status']}")
PY
```

Expected:

- inspect reports `stale` or `invalid`
- output includes at least one mismatch
- the command does not regenerate the broken generated contract

## 9. Local SQLite Memory substrate focused checks

These are automated Rust checks, but they are the only practical way in M14a to exercise the local SQLite runtime because model-facing direct Memory actions are not live yet.

```bash
cargo test -p agentpm-cli harness_runtime::memory -- --nocapture \
  | tee "$M14A_RUNS/cargo-memory-tests.txt"
```

Expected:

- all focused `harness_runtime::memory` tests pass
- covered behavior includes:
  - schema version and v1 tables/indexes
  - unsupported newer schema rejection
  - schema `0 -> 1` migration structure
  - WAL, busy timeout, and foreign keys
  - canonical scope JSON/hash stability
  - one active document per package/version + space + scope
  - archived rows excluded from active key reads
  - deterministic sequence ordinals
  - operation state persistence
  - atomic batch commit and rollback
  - generated contract loader integrity
  - contract cache invalidation
  - no writes into package roots during generated-contract validation

## 10. Final verification commands

Run these before handing off M14a:

```bash
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test -p agentpm-cli harness_runtime::memory -- --nocapture
cargo test -p agentpm-cli preflight_ -- --nocapture
```

If you want the broader Rust pass and your local environment permits the filesystem fixture operations:

```bash
cargo test -p agentpm-cli
```

Expected:

- formatter check passes
- clippy passes with `-D warnings`
- focused Memory tests pass
- preflight tests pass
- full `agentpm-cli` test suite passes, or any failure is clearly unrelated to M14a and documented

## Cleanup

```bash
rm -rf "$HARNESS_M14A_TEST_BASE"
rm -f /tmp/setup-harness-m14a-manual.sh
```
