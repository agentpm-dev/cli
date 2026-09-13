# Harness Milestone 14g Manual Tests

Manual coverage for Milestone 14g: optional Harness Memory persistence review after accepted phase completions.

This pass extends the M14b/M14c Memory fixture. M14g is an optional review sub-loop over the existing ModelRuntime, Memory semantic actions, Hooks, MemoryRuntime, prompt assembly, events, and accounting surfaces. The manual tests therefore focus on proving that review is inserted only at configured completion boundaries, uses the normal Memory pipeline, exposes only Memory plus `PersistenceReviewComplete`, and does not change the already-valid phase completion.

## Coverage target

| Requirement | Manual coverage |
|---|---|
| Omitted `memory.write_review` preserves existing behavior | disabled baseline run has no review lifecycle events |
| `memory.write_review.points` validates shape and uniqueness | invalid empty, duplicate, and unknown point configs fail before a run |
| `run_end` supersedes `phase_end` at terminal boundaries | config with both points emits one `run_end` review only |
| Review uses canonical ModelRequest/provider path | trace `model_runtime_request_prepared` shows canonical request, no prompt catalog injection |
| Review catalog is Memory-only plus `PersistenceReviewComplete` | trace action aliases include Memory read/write and review complete, not tools/Knowledge/Skill/phase completion |
| Pending phase completion is preserved | report still ends with the original authored outcome after review |
| Review Memory writes use normal Memory dispatch | trace contains normal `memory_write_*` events and no tool dispatch |
| Review completion is not recursive | focused Rust test asserts exactly one review for one pending completion |
| Later phases do not see review transcript | focused Rust test asserts next prompt excludes review ActionResult/control content |
| Process MemoryRuntime parity | live fake process MemoryRuntime receives review write through custom runtime path |
| Review failure/limit exhaustion does not fail completed phase | focused Rust test asserts pending completion survives review limit failure |

Provider behavior can vary. If a live model does not select the requested review Memory action or review completion action, keep the report/trace and rerun that scenario before treating it as a Harness regression. The deterministic Rust checks at the end are the pass/fail source for hard-to-force control-flow edges.

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

Then apply the M14g persistence-review extension:

```bash
cat > /tmp/setup-harness-m14g-manual.sh <<'SH'
#!/usr/bin/env bash
set -euo pipefail

ROOT="$(pwd)"
APM="${APM:-$ROOT/target/debug/agentpm}"
BASE="${HARNESS_M14BC_TEST_BASE:-$ROOT/harness-m14bc-test}"
WORK="$BASE/workspace"
RUNS="$BASE/runs-m14g"
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

mkdir -p "$WORK/runtime" "$WORK/inputs" "$WORK/scripts" "$RUNS"/{config,disabled,run-end,custom-process,rust}

export M14G_SETUP_WORK="$WORK"
export M14G_SETUP_RUNS="$RUNS"
export M14G_SETUP_MODEL="${AGENTPM_MANUAL_OPENAI_MODEL:-gpt-4o-mini}"
export M14G_SETUP_PYTHON="$PYTHON_CMD"

"$PYTHON_CMD" <<'PY'
import json
import os
import shlex
from copy import deepcopy
from pathlib import Path

work = Path(os.environ["M14G_SETUP_WORK"])
model = os.environ["M14G_SETUP_MODEL"]
python_cmd = os.environ["M14G_SETUP_PYTHON"]
base = json.loads((work / "agentpm.openai.harness.json").read_text())

def common_config(state_dir, points=None):
    data = deepcopy(base)
    data.setdefault("runtime", {})["state_dir"] = state_dir
    limits = data["runtime"].setdefault("limits", {})
    limits.setdefault("max_model_calls_per_phase", 8)
    limits.setdefault("max_actions_per_phase", 16)
    limits.setdefault("max_structured_output_repairs", 3)
    data.setdefault("trace", {})["enabled"] = True
    data["trace"]["level"] = "verbose"
    data["trace"]["content"] = "full"
    data.setdefault("model", {})["model"] = model
    if points is not None:
        data.setdefault("memory", {})["write_review"] = {"points": points}
    return data

def add_process_memory(data):
    memory = data.setdefault("memory", {})
    memory.setdefault("runtimes", {})["m14g-process-memory"] = {
        "implementation": {
            "type": "process",
            "command": "runtime/m14g_process_memory.sh",
            "args": [],
            "cwd": ".",
            "env": [],
            "startup_timeout_ms": 10000,
            "request_timeout_ms": 30000,
            "restart": {"max_attempts": 0, "backoff_ms": 0},
        }
    }
    memory.setdefault("packages", {})["@zack/m14bc-memory"] = {
        "runtime": "m14g-process-memory"
    }

def write_config(filename, data):
    (work / filename).write_text(json.dumps(data, indent=2) + "\n")

write_config("agentpm.openai.m14g.disabled.harness.json", common_config(".agentpm-state-m14g-disabled"))
write_config("agentpm.openai.m14g.run-end.harness.json", common_config(".agentpm-state-m14g-run-end", ["phase_end", "run_end"]))
write_config("agentpm.openai.m14g.run-end-only.harness.json", common_config(".agentpm-state-m14g-run-end-only", ["run_end"]))

process_cfg = common_config(".agentpm-state-m14g-custom-process", ["run_end"])
add_process_memory(process_cfg)
write_config("agentpm.openai.m14g.custom-process.harness.json", process_cfg)

invalid_empty = common_config(".agentpm-state-m14g-invalid-empty")
invalid_empty.setdefault("memory", {})["write_review"] = {"points": []}
write_config("agentpm.openai.m14g.invalid-empty.harness.json", invalid_empty)

invalid_duplicate = common_config(".agentpm-state-m14g-invalid-duplicate")
invalid_duplicate.setdefault("memory", {})["write_review"] = {"points": ["run_end", "run_end"]}
write_config("agentpm.openai.m14g.invalid-duplicate.harness.json", invalid_duplicate)

invalid_unknown = common_config(".agentpm-state-m14g-invalid-unknown")
invalid_unknown.setdefault("memory", {})["write_review"] = {"points": ["before_sleep"]}
write_config("agentpm.openai.m14g.invalid-unknown.harness.json", invalid_unknown)

(work / "runtime" / "m14g_process_memory_service.py").write_text(r'''#!/usr/bin/env python3
import json
import os
import sys
from pathlib import Path

LOG_PATH = Path(os.environ.get("M14G_MEMORY_LOG", "runtime/m14g-process-memory-calls.jsonl"))
REGISTRY_ID = os.environ.get("M14G_MEMORY_REGISTRY_ID", "m14g-process-memory")

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

for line in sys.stdin:
    msg = json.loads(line)
    write_log({"kind": msg.get("kind"), "method": msg.get("method"), "payload": msg.get("payload")})

    if msg.get("kind") == "initialize":
        emit(msg, "initialized", {
            "ready": True,
            "registry_id": REGISTRY_ID,
            "protocol_version": 1,
            "capabilities": {
                "space_models": ["document", "collection", "sequence"],
                "retrieval_modes": ["key", "filter", "chronological", "full_text"],
                "retention_actions": ["archive", "delete"],
                "constraints": ["append_only"],
                "capacity": True,
                "durable_trigger_state": False,
                "atomic_batches": False,
                "packages": [
                    { "package": "@zack/m14bc-memory", "version": "0.1.0", "ready": True }
                ]
            }
        })
        continue

    if msg.get("kind") == "request" and msg.get("method") == "write":
        req = msg.get("payload", {}).get("request", {})
        emit(msg, "response", {
            "ok": True,
            "package": req.get("package"),
            "package_version": req.get("package_version"),
            "space": req.get("space"),
            "operation": req.get("operation"),
            "record_id": "m14g-process-review-1"
        })
        continue

    if msg.get("kind") == "request" and msg.get("method") == "read":
        req = msg.get("payload", {}).get("request", {})
        emit(msg, "response", {
            "ok": True,
            "package": req.get("package"),
            "package_version": req.get("package_version"),
            "space": req.get("space"),
            "mode": req.get("mode"),
            "records": [],
            "count": 0
        })
        continue

    emit(msg, "error", error={"code": "unsupported_method", "message": "unsupported MemoryRuntime request"})
''')
(work / "runtime" / "m14g_process_memory_service.py").chmod(0o755)

(work / "runtime" / "m14g_process_memory.sh").write_text(
    "#!/usr/bin/env bash\n"
    "set -euo pipefail\n"
    "export M14G_MEMORY_REGISTRY_ID=m14g-process-memory\n"
    "export M14G_MEMORY_LOG=runtime/m14g-process-memory-calls.jsonl\n"
    f"exec {shlex.quote(python_cmd)} runtime/m14g_process_memory_service.py\n"
)
(work / "runtime" / "m14g_process_memory.sh").chmod(0o755)

(work / "inputs" / "m14g-review-write.txt").write_text('''First complete the ordinary phase with outcome done and output {"summary":"m14g ordinary completion"}. Do not call Memory during the ordinary phase.

The information learned in this Run that should persist is:

{
  "body": "M14g persistence review note",
  "status": "open",
  "labels": ["m14g", "review"]
}

If Harness starts a persistence review after the phase completion, create exactly one note record in @zack/m14bc-memory/notes using memory_write operation create and record_type note with the JSON above. After that Memory write completes, call persistence_review_complete. Do not perform external research or use tools.
''')

(work / "inputs" / "m14g-review-complete-only.txt").write_text('''Complete the ordinary phase with outcome done and output {"summary":"m14g no persistence needed"}. Do not call Memory, tools, or Knowledge.

If Harness starts a persistence review, call persistence_review_complete without Memory work.
''')

(work / "scripts" / "m14g_assert_report.py").write_text(r'''#!/usr/bin/env python3
import json
import sys
from pathlib import Path

report_path = Path(sys.argv[1])
checks = sys.argv[2:]
report = json.loads(report_path.read_text())

trace_path_value = report.get("trace_path")
events = []
if trace_path_value:
    trace_path = Path(trace_path_value)
    if not trace_path.is_absolute():
        trace_path = report_path.parent / trace_path
    if trace_path.exists():
        with trace_path.open() as handle:
            events = [json.loads(line) for line in handle if line.strip()]

def event_type(event):
    return event.get("event_type")

def payload(event):
    return event.get("payload") or {}

def fields(event):
    return payload(event).get("fields") or {}

def events_of(name):
    return [event for event in events if event_type(event) == name]

def fail(message):
    raise SystemExit(message)

def require(condition, message):
    if not condition:
        fail(message)

def review_model_requests():
    out = []
    for event in events_of("model_runtime_request_prepared"):
        event_fields = fields(event)
        prompt = event_fields.get("prompt", "")
        aliases = event_fields.get("action_aliases", [])
        if "bounded Memory write review" in prompt or any(
            alias.get("action_kind") == "persistence_review_complete" for alias in aliases
        ):
            out.append(event_fields)
    return out

for check in checks:
    if check == "ended":
        require(report.get("terminal_status") == "ended", f"expected ended terminal status, got {report.get('terminal_status')}")
    elif check == "outcome-done":
        outcomes = [p.get("outcome") for p in report.get("phase_summaries", [])]
        require("done" in outcomes, f"expected done phase outcome, got {outcomes}")
    elif check == "no-review-events":
        require(not any(event_type(event).startswith("memory_write_review_") for event in events), "unexpected persistence-review events")
    elif check == "review-started":
        require(events_of("memory_write_review_started"), "missing memory_write_review_started")
    elif check == "review-completed":
        require(events_of("memory_write_review_completed"), "missing memory_write_review_completed")
    elif check == "review-terminal":
        require(
            events_of("memory_write_review_completed") or events_of("memory_write_review_failed") or events_of("memory_write_review_skipped"),
            "missing terminal review event",
        )
    elif check == "run-end-only":
        started = events_of("memory_write_review_started")
        require(len(started) == 1, f"expected exactly one review start, got {len(started)}")
        require(fields(started[0]).get("point") == "run_end", f"expected run_end review, got {fields(started[0]).get('point')}")
    elif check == "review-request-shape":
        requests = review_model_requests()
        require(requests, "missing review model request")
        request = requests[0]
        require(request.get("request_kind") in (None, "provider_wire_request", "canonical_model_request"), "unexpected request kind")
        require(request.get("capability_catalog_in_prompt") is False, "capability catalog should not be prose-injected into provider prompt")
    elif check == "review-catalog-memory-only":
        requests = review_model_requests()
        require(requests, "missing review model request")
        kinds = {alias.get("action_kind") for alias in requests[0].get("action_aliases", [])}
        require("persistence_review_complete" in kinds, f"missing persistence_review_complete alias: {kinds}")
        require("memory_write" in kinds, f"missing memory_write alias: {kinds}")
        forbidden = {"phase_completion", "agentpm_tool", "external_mcp_tool", "knowledge_request", "skill_resource_read"}
        require(not (kinds & forbidden), f"review catalog exposed forbidden actions: {kinds & forbidden}")
    elif check == "memory-write-completed":
        require(events_of("memory_write_completed"), "missing memory_write_completed")
    elif check == "normal-memory-pipeline":
        require(events_of("memory_write_started"), "missing normal memory_write_started event")
        require(not events_of("tool_call_started"), "review Memory write went through tool dispatch")
    elif check == "review-summary":
        require(report.get("memory_write_review_summaries"), "missing memory_write_review_summaries")
    elif check == "process-record-id":
        text = json.dumps(report)
        trace_text = "\n".join(json.dumps(event) for event in events)
        require("m14g-process-review-1" in text or "m14g-process-review-1" in trace_text, "missing process MemoryRuntime record id")
    else:
        fail(f"unknown check: {check}")

print(f"ok: {report_path} {' '.join(checks)}")
''')
(work / "scripts" / "m14g_assert_report.py").chmod(0o755)

print(f"M14g manual workspace: {work}")
print(f"M14g manual runs: {os.environ['M14G_SETUP_RUNS']}")
PY
SH

chmod +x /tmp/setup-harness-m14g-manual.sh
/tmp/setup-harness-m14g-manual.sh

export M14G_WORK="$M14BC_WORK"
export M14G_RUNS="$HARNESS_M14BC_TEST_BASE/runs-m14g"
```

## Test 1: config validation for `memory.write_review.points`

```bash
for case in invalid-empty invalid-duplicate invalid-unknown; do
  REPORT="$M14G_RUNS/config/${case}-report.json"
  if (cd "$M14G_WORK" && "$APM" harness \
    --config "agentpm.openai.m14g.${case}.harness.json" \
    --headless \
    --scope user="m14g-${case}-user" \
    --scope conversation="m14g-${case}-conversation" \
    --input-file inputs/m14g-review-complete-only.txt \
    --report "$REPORT"); then
      echo "Expected $case to fail validation" >&2
      exit 1
  fi
done
```

Expected:

- empty `points` is rejected;
- duplicate `run_end` is rejected;
- unknown point `before_sleep` is rejected;
- no Harness run report is required for these config-validation failures.

## Test 2: omitted review preserves existing behavior

```bash
REPORT="$M14G_RUNS/disabled/disabled-report.json"
(cd "$M14G_WORK" && "$APM" harness \
  --config agentpm.openai.m14g.disabled.harness.json \
  --headless \
  --scope user=m14g-disabled-user \
  --scope conversation=m14g-disabled-conversation \
  --input-file inputs/m14g-review-complete-only.txt \
  --report "$REPORT")

"$AGENTPM_MANUAL_PYTHON" "$M14G_WORK/scripts/m14g_assert_report.py" \
  "$REPORT" \
  ended outcome-done no-review-events
```

Expected:

- run completes normally with outcome `done`;
- no `memory_write_review_*` lifecycle events appear;
- model-call count reflects ordinary phase execution only.

## Test 3: `run_end` supersedes `phase_end` at terminal boundary

```bash
export M14G_RUN_END_CASE="$(date +%s)"
REPORT="$M14G_RUNS/run-end/run-end-supersedes-phase-end-report.json"
(cd "$M14G_WORK" && "$APM" harness \
  --config agentpm.openai.m14g.run-end.harness.json \
  --headless \
  --scope user="m14g-run-end-user-$M14G_RUN_END_CASE" \
  --scope conversation="m14g-run-end-conversation-$M14G_RUN_END_CASE" \
  --input-file inputs/m14g-review-write.txt \
  --report "$REPORT") || true

"$AGENTPM_MANUAL_PYTHON" "$M14G_WORK/scripts/m14g_assert_report.py" \
  "$REPORT" \
  ended outcome-done run-end-only review-request-shape review-catalog-memory-only review-terminal review-summary
```

Expected:

- exactly one persistence review starts;
- its `point` is `run_end`, not `phase_end`;
- report still has the original `done` phase outcome;
- trace review request uses the canonical ModelRuntime path;
- `capability_catalog_in_prompt` is `false`;
- review action aliases expose Memory read/write plus `persistence_review_complete`, not ordinary `phase_completion`, tools, Knowledge, MCP, or Skill resource reads.

If the live model writes the requested note, additionally run:

```bash
"$AGENTPM_MANUAL_PYTHON" "$M14G_WORK/scripts/m14g_assert_report.py" \
  "$REPORT" \
  memory-write-completed normal-memory-pipeline
```

Expected:

- review Memory write emits normal `memory_write_started` and `memory_write_completed`;
- no tool dispatch is used for the review Memory action.

## Test 4: process MemoryRuntime parity

```bash
rm -f "$M14G_WORK/runtime/m14g-process-memory-calls.jsonl"

export M14G_PROCESS_CASE="$(date +%s)"
REPORT="$M14G_RUNS/custom-process/custom-process-report.json"
(cd "$M14G_WORK" && "$APM" harness \
  --config agentpm.openai.m14g.custom-process.harness.json \
  --headless \
  --scope user="m14g-process-user-$M14G_PROCESS_CASE" \
  --scope conversation="m14g-process-conversation-$M14G_PROCESS_CASE" \
  --input-file inputs/m14g-review-write.txt \
  --report "$REPORT") || true

"$AGENTPM_MANUAL_PYTHON" "$M14G_WORK/scripts/m14g_assert_report.py" \
  "$REPORT" \
  ended outcome-done run-end-only review-request-shape review-catalog-memory-only memory-write-completed normal-memory-pipeline process-record-id

"$AGENTPM_MANUAL_PYTHON" - "$M14G_WORK/runtime/m14g-process-memory-calls.jsonl" <<'PY'
import json
import sys
from pathlib import Path

path = Path(sys.argv[1])
rows = [json.loads(line) for line in path.read_text().splitlines() if line.strip()]
assert any(row.get("kind") == "initialize" for row in rows), "missing process initialize"
writes = [row for row in rows if row.get("method") == "write"]
assert writes, "missing process MemoryRuntime write"
request = writes[0]["payload"]["request"]
assert request["package"] == "@zack/m14bc-memory"
assert request["space"] == "notes"
assert request["operation"] == "create"
assert request["content"]["body"] == "M14g persistence review note"
print("ok: process MemoryRuntime received review write")
PY
```

Expected:

- configured process MemoryRuntime starts and handshakes;
- review write routes through the custom process MemoryRuntime;
- local fake tool dispatcher is not used;
- process log shows a `write` request for `@zack/m14bc-memory/notes`.

## Test 5: deterministic non-recursion and hidden later-phase context

These are hard to force reliably with a live model because they depend on exact review-turn sequencing and a multi-phase fixture. Verify the focused engine regressions directly:

```bash
cargo test -p agentpm-cli \
  memory_write_review_completion_does_not_reenter_review \
  -- --nocapture \
  | tee "$M14G_RUNS/rust/no-recursive-review.txt"

cargo test -p agentpm-cli \
  memory_write_review_phase_end_runs_before_transition_without_leaking_transcript \
  -- --nocapture \
  | tee "$M14G_RUNS/rust/phase-end-hidden-context.txt"
```

Expected:

- exactly one review sub-loop is started for one pending completion;
- review `PersistenceReviewComplete` does not recursively start another review;
- a later phase receives prior `PhaseResult` state but not review `ActionResult`, review control action text, or review-only assistant content.

## Test 6: deterministic review failure and limit handling

```bash
cargo test -p agentpm-cli \
  memory_write_review_limit_exhaustion_preserves_pending_completion \
  -- --nocapture \
  | tee "$M14G_RUNS/rust/review-limit-preserves-completion.txt"

cargo test -p agentpm-cli \
  memory_write_review_failure_preserves_pending_completion \
  -- --nocapture \
  | tee "$M14G_RUNS/rust/review-failure-preserves-completion.txt"

cargo test -p agentpm-cli \
  memory_write_review_committed_writes_survive_later_review_failure \
  -- --nocapture \
  | tee "$M14G_RUNS/rust/committed-write-survives-review-failure.txt"
```

Expected:

- review-specific limit exhaustion emits review failure, not Harness runtime failure;
- unrecoverable review model failure preserves the accepted phase completion;
- Memory writes committed before later review failure remain committed.

## Test 7: focused automated M14g regression checks

```bash
cargo fmt --all --check
cargo test -p agentpm-cli memory_write_review -- --nocapture \
  | tee "$M14G_RUNS/rust/memory-write-review.txt"
cargo test -p agentpm-cli persistence_review_complete -- --nocapture \
  | tee "$M14G_RUNS/rust/persistence-review-complete.txt"
cargo test -p agentpm-cli harness_engine::tests -- --nocapture \
  | tee "$M14G_RUNS/rust/harness-engine.txt"
```

Expected:

- M14g config validation, prompt shape, review catalog filtering, review events, failure handling, process/host MemoryRuntime parity, and no-recursion checks pass;
- `PersistenceReviewComplete` normalizes through provider actions during review but is rejected in ordinary phase validation before dispatch;
- formatter check passes.

## Manual pass acceptance

Manual pass is acceptable when:

- invalid `memory.write_review.points` configs fail validation;
- omitted review produces no review lifecycle events;
- `run_end` supersedes `phase_end` at terminal boundaries;
- review prompts use canonical request inspection with no prose capability catalog injected into provider prompts;
- review exposes only authorized Memory actions and `PersistenceReviewComplete`;
- review Memory writes use normal Memory events/runtime paths;
- process MemoryRuntime receives review writes without fake dispatcher fallback;
- focused Rust checks pass for no recursion, later-phase hidden context, and review failure/limit preservation.

## Cleanup

The setup writes only inside the existing M14b/M14c manual fixture plus `/tmp/setup-harness-m14g-manual.sh`.

```bash
rm -f /tmp/setup-harness-m14g-manual.sh
```

To remove all generated M14g manual state:

```bash
rm -rf "$M14G_RUNS" \
  "$M14G_WORK/.agentpm-state-m14g-disabled" \
  "$M14G_WORK/.agentpm-state-m14g-run-end" \
  "$M14G_WORK/.agentpm-state-m14g-run-end-only" \
  "$M14G_WORK/.agentpm-state-m14g-custom-process"
```
