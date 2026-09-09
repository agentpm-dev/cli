# Harness Milestone 14h Manual Tests

Manual coverage for Milestone 14h: clearer Memory repair feedback and user-visible persistence-review failure warnings.

This pass extends the M14b/M14c Memory fixture and uses a deterministic process `ModelRuntime`. That keeps the checks focused on Harness behavior instead of depending on a live model to choose the exact wrong and repaired actions.

## Coverage Target

| Requirement | Manual coverage |
|---|---|
| Wrong Memory space/record-type feedback suggests authorized alternatives | Test 1 forces `note` into `conversation_state` and checks repair feedback names `@zack/m14bc-memory/notes` |
| Corrected retry uses the normal model/validation/Memory path | Test 1 then returns a corrected `notes` write and checks normal `memory_write_*` events |
| Repair stays within the existing structured-output repair budget | Test 1 completes after one repair; Test 2 exhausts the default repair budget |
| Review failure is nonfatal and nonmutating | Test 2 repeats the invalid action, records a failed review summary, and leaves Memory writes at 0/0 |
| Human warning is visible when intended Memory may not have been written | Test 2 captures stderr and checks the warning appears before the authored abort error |
| Process ModelRuntime receives the normal canonical review request | Tests assert `model_runtime_request_prepared` has `runtime_kind=process`, `request_kind=canonical_model_request`, and review Memory aliases |

## Prerequisites

From the root of the `agentpm` repo:

```bash
cargo build -p agentpm-cli
export APM="$PWD/target/debug/agentpm"
export AGENTPM_MANUAL_PYTHON="${AGENTPM_MANUAL_PYTHON:-python3}"
```

First create the M14b/M14c fixture using the setup block in `HARNESS_M14B_M14C_MANUAL_TESTS.md`.

Expected base workspace:

```bash
export HARNESS_M14BC_TEST_BASE="${HARNESS_M14BC_TEST_BASE:-$PWD/harness-m14bc-test}"
export M14BC_WORK="$HARNESS_M14BC_TEST_BASE/workspace"
```

## Setup

```bash
cat > /tmp/setup-harness-m14h-manual.sh <<'SH'
#!/usr/bin/env bash
set -euo pipefail

ROOT="$(pwd)"
APM="${APM:-$ROOT/target/debug/agentpm}"
BASE="${HARNESS_M14BC_TEST_BASE:-$ROOT/harness-m14bc-test}"
SOURCE_WORK="$BASE/workspace"
M14H_BASE="$ROOT/harness-m14h-test"
WORK="$M14H_BASE/workspace"
ABORT_WORK="$M14H_BASE/workspace-abort"
RUNS="$M14H_BASE/runs"
PYTHON_CMD="${AGENTPM_MANUAL_PYTHON:-python3}"

if [ ! -x "$APM" ]; then
  echo "Missing agentpm binary at $APM. Run: cargo build -p agentpm-cli" >&2
  exit 1
fi

if [ ! -f "$SOURCE_WORK/agent.json" ] || [ ! -f "$SOURCE_WORK/.agentpm/memory/zack/m14bc-memory/0.1.0/agent.json" ]; then
  echo "Missing M14b/M14c fixture. Run HARNESS_M14B_M14C_MANUAL_TESTS.md setup first." >&2
  exit 1
fi

rm -rf "$M14H_BASE"
mkdir -p "$M14H_BASE" "$RUNS"/{repair,abort}
cp -R "$SOURCE_WORK" "$WORK"
cp -R "$SOURCE_WORK" "$ABORT_WORK"

export M14H_SETUP_WORK="$WORK"
export M14H_SETUP_ABORT_WORK="$ABORT_WORK"
export M14H_SETUP_PYTHON="$PYTHON_CMD"

"$PYTHON_CMD" <<'PY'
import json
import os
import shlex
from copy import deepcopy
from pathlib import Path

work = Path(os.environ["M14H_SETUP_WORK"])
abort_work = Path(os.environ["M14H_SETUP_ABORT_WORK"])
python_cmd = os.environ["M14H_SETUP_PYTHON"]

def load_base_config(root):
    return json.loads((root / "agentpm.openai.harness.json").read_text())

def write_process_model(root, scenario):
    (root / "runtime").mkdir(exist_ok=True)
    (root / "inputs").mkdir(exist_ok=True)
    (root / "scripts").mkdir(exist_ok=True)

    (root / "runtime" / "m14h_process_model.py").write_text(r'''#!/usr/bin/env python3
import json
import os
import sys
from pathlib import Path

SCENARIO = os.environ.get("M14H_MODEL_SCENARIO", "repair")
LOG_PATH = Path(os.environ.get("M14H_MODEL_LOG", "runtime/m14h-process-model-calls.jsonl"))
turn = 0

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
        "service": "model",
    }
    if result is not None:
        out["result"] = result
    if error is not None:
        out["error"] = error
    print(json.dumps(out), flush=True)

def phase_done():
    return {
        "assistant_content": None,
        "actions": [{
            "id": "complete-done",
            "action": {
                "type": "phase_completion",
                "outcome": "done",
                "output": {"summary": "m14h ordinary completion"}
            }
        }],
        "finish_reason": "tool_calls"
    }

def review_write(space, body):
    return {
        "assistant_content": None,
        "actions": [{
            "id": "review-write",
            "action": {
                "type": "memory_write",
                "package": "@zack/m14bc-memory",
                "space": space,
                "operation": "create",
                "record_type": "note",
                "content": {
                    "body": body,
                    "status": "open",
                    "labels": ["m14h", "manual"]
                }
            }
        }],
        "finish_reason": "tool_calls"
    }

def review_complete():
    return {
        "assistant_content": "review complete",
        "actions": [{
            "id": "review-complete",
            "action": {"type": "persistence_review_complete"}
        }],
        "finish_reason": "tool_calls"
    }

for line in sys.stdin:
    msg = json.loads(line)
    write_log({"kind": msg.get("kind"), "method": msg.get("method"), "payload": msg.get("payload")})

    if msg.get("kind") == "initialize":
        emit(msg, "initialized", {
            "ready": True,
            "registry_id": "m14h-scripted-model",
            "model": "m14h-scripted",
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
        turn += 1
        if turn == 1:
            emit(msg, "response", phase_done())
        elif SCENARIO == "repair":
            if turn == 2:
                emit(msg, "response", review_write("conversation_state", "m14h wrong target"))
            elif turn == 3:
                emit(msg, "response", review_write("notes", "m14h repaired note"))
            else:
                emit(msg, "response", review_complete())
        else:
            emit(msg, "response", review_write("conversation_state", f"m14h repeated wrong target {turn}"))
        continue

    emit(msg, "error", error={"code": "unsupported_method", "message": "unsupported model request"})
''')
    (root / "runtime" / "m14h_process_model.py").chmod(0o755)

    (root / "runtime" / "m14h_process_model.sh").write_text(
        "#!/usr/bin/env bash\n"
        "set -euo pipefail\n"
        f"export M14H_MODEL_SCENARIO={shlex.quote(scenario)}\n"
        "export M14H_MODEL_LOG=runtime/m14h-process-model-calls.jsonl\n"
        "exec " + shlex.quote(python_cmd) + " runtime/m14h_process_model.py\n"
    )
    (root / "runtime" / "m14h_process_model.sh").chmod(0o755)

def write_config(root, state_dir, scenario):
    data = load_base_config(root)
    data["model"] = {
        "provider": "m14h-scripted-model",
        "model": "m14h-scripted",
    }
    data.setdefault("providers", {}).setdefault("models", {})["m14h-scripted-model"] = {
        "implementation": {
            "type": "process",
            "command": "runtime/m14h_process_model.sh",
            "args": [],
            "cwd": ".",
            "env": [],
            "startup_timeout_ms": 10000,
            "request_timeout_ms": 30000,
            "restart": {"max_attempts": 0, "backoff_ms": 0}
        }
    }
    limits = data.setdefault("runtime", {}).setdefault("limits", {})
    limits["max_model_calls_per_phase"] = 8
    limits["max_actions_per_phase"] = 16
    limits["max_structured_output_repairs"] = 3
    data["runtime"]["state_dir"] = state_dir
    data.setdefault("memory", {})["write_review"] = {"points": ["run_end"]}
    data.setdefault("trace", {})["enabled"] = True
    data["trace"]["level"] = "verbose"
    data["trace"]["content"] = "full"
    (root / "agentpm.m14h.harness.json").write_text(json.dumps(data, indent=2) + "\n")

def make_abort_loop(root):
    loop_path = root / ".agentpm" / "loops" / "zack" / "m14bc-loop" / "0.1.0" / "agent.json"
    data = json.loads(loop_path.read_text())
    data["loop"]["transitions"] = [
        {"from": "remember", "on": "done", "to": "$abort"},
        {"from": "remember", "on": "no-memory", "to": "$end"},
    ]
    loop_path.write_text(json.dumps(data, indent=2) + "\n")

write_process_model(work, "repair")
write_process_model(abort_work, "fail")

for root in (work, abort_work):
    (root / "inputs" / "m14h-review.txt").write_text("""Complete the ordinary phase with outcome done and output {\"summary\":\"m14h ordinary completion\"}. Do not call Memory during the ordinary phase.

If Harness starts a persistence review, create one note in @zack/m14bc-memory/notes using memory_write operation create and record_type note, then call persistence_review_complete. Do not use tools, Knowledge, or external research.
""")

write_config(work, ".agentpm-state-m14h-repair", "repair")
make_abort_loop(abort_work)
write_config(abort_work, ".agentpm-state-m14h-abort-failure", "fail")

assert_script = r'''#!/usr/bin/env python3
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

def events_of(name):
    return [event for event in events if event.get("event_type") == name]

def fields(event):
    return (event.get("payload") or {}).get("fields") or {}

def action_payload(event):
    payload = event.get("payload") or {}
    return payload if payload.get("payload_type") == "action" else {}

def require(condition, message):
    if not condition:
        raise SystemExit(message)

expected_feedback = "Memory record type `note` is not declared for selected Memory space `conversation_state` in package `@zack/m14bc-memory`. Authorized alternative Memory write action(s) for record type `note`: `@zack/m14bc-memory/notes`."

for check in checks:
    if check == "ended":
        require(report["terminal_status"] == "ended", f"expected ended, got {report['terminal_status']}")
    elif check == "aborted":
        require(report["terminal_status"] == "aborted", f"expected aborted, got {report['terminal_status']}")
    elif check == "one-repair":
        require(report["repair_count"] == 1, f"expected one repair, got {report['repair_count']}")
    elif check == "review-completed":
        summaries = report["memory_write_review_summaries"]
        require(len(summaries) == 1, f"expected one review summary, got {len(summaries)}")
        require(summaries[0]["status"] == "completed", f"expected completed review, got {summaries[0]}")
        require(summaries[0]["memory_writes_completed"] == 1, f"expected one completed write, got {summaries[0]}")
    elif check == "review-failed-nonmutating":
        summaries = report["memory_write_review_summaries"]
        require(len(summaries) == 1, f"expected one review summary, got {len(summaries)}")
        summary = summaries[0]
        require(summary["status"] == "failed", f"expected failed review, got {summary}")
        require(summary["reason"] == "structured_output_repair_limit", f"unexpected failure reason {summary}")
        require(summary["memory_writes_attempted"] == 0, f"expected zero attempted writes, got {summary}")
        require(summary["memory_writes_completed"] == 0, f"expected zero completed writes, got {summary}")
    elif check == "feedback-suggested-notes":
        rejected = events_of("semantic_action_rejected")
        require(any(expected_feedback == fields(event).get("error") for event in rejected), "missing improved mismatch feedback")
        prompts = [fields(event).get("prompt", "") for event in events_of("model_runtime_request_prepared")]
        require(any(expected_feedback in prompt for prompt in prompts), "repair prompt did not include improved feedback")
    elif check == "corrected-notes-write":
        completed = events_of("memory_write_completed")
        require(any(
            action_payload(event).get("identity") == "@zack/m14bc-memory/notes"
            and fields(event).get("space") == "notes"
            and (((fields(event).get("result") or {}).get("record") or {}).get("content") or {}).get("body") == "m14h repaired note"
            for event in completed
        ), "missing corrected notes memory_write_completed")
    elif check == "normal-memory-pipeline":
        require(events_of("memory_write_started"), "missing memory_write_started")
        require(events_of("memory_write_completed"), "missing memory_write_completed")
        require(not events_of("tool_call_started"), "Memory review went through tool dispatch")
    elif check == "process-canonical-review-request":
        requests = [fields(event) for event in events_of("model_runtime_request_prepared")]
        require(requests, "missing model_runtime_request_prepared")
        review_requests = [
            request for request in requests
            if any(alias.get("action_kind") == "persistence_review_complete" for alias in request.get("action_aliases", []))
        ]
        require(review_requests, "missing review model request")
        request = review_requests[0]
        require(request.get("runtime_kind") == "process", f"expected process runtime request, got {request.get('runtime_kind')}")
        require(request.get("request_kind") == "canonical_model_request", f"expected canonical model request, got {request.get('request_kind')}")
        kinds = {alias.get("action_kind") for alias in request.get("action_aliases", [])}
        require("memory_write" in kinds, f"missing memory_write alias: {kinds}")
        require("persistence_review_complete" in kinds, f"missing persistence_review_complete alias: {kinds}")
        forbidden = {"phase_completion", "agentpm_tool", "external_mcp_tool", "knowledge_request", "skill_resource_read"}
        require(not (kinds & forbidden), f"review request exposed forbidden aliases: {kinds & forbidden}")
    elif check == "review-failed-event":
        require(events_of("memory_write_review_failed"), "missing memory_write_review_failed")
    else:
        raise SystemExit(f"unknown check: {check}")

print(f"ok: {report_path} {' '.join(checks)}")
'''

for root in (work, abort_work):
    script = root / "scripts" / "m14h_assert_report.py"
    script.write_text(assert_script)
    script.chmod(0o755)

print(f"M14h repair workspace: {work}")
print(f"M14h abort workspace: {abort_work}")
print(f"M14h runs: {work.parent / 'runs'}")
PY
SH

chmod +x /tmp/setup-harness-m14h-manual.sh
/tmp/setup-harness-m14h-manual.sh

export M14H_BASE="$PWD/harness-m14h-test"
export M14H_WORK="$M14H_BASE/workspace"
export M14H_ABORT_WORK="$M14H_BASE/workspace-abort"
export M14H_RUNS="$M14H_BASE/runs"
```

## Test 1: Wrong Space Repairs To Correct Notes Write

```bash
rm -f "$M14H_WORK/runtime/m14h-process-model-calls.jsonl"

REPORT="$M14H_RUNS/repair/repair-converges-report.json"
STDOUT="$M14H_RUNS/repair/repair-converges-stdout.txt"
STDERR="$M14H_RUNS/repair/repair-converges-stderr.txt"

(cd "$M14H_WORK" && "$APM" harness \
  --config agentpm.m14h.harness.json \
  --headless \
  --scope user="m14h-repair-user" \
  --scope conversation="m14h-repair-conversation" \
  --input-file inputs/m14h-review.txt \
  --report "$REPORT" \
  >"$STDOUT" 2>"$STDERR")

"$AGENTPM_MANUAL_PYTHON" "$M14H_WORK/scripts/m14h_assert_report.py" \
  "$REPORT" \
  ended one-repair review-completed feedback-suggested-notes corrected-notes-write normal-memory-pipeline process-canonical-review-request

test ! -s "$STDERR"
cat "$STDOUT"
```

Expected:

- run exits 0 and terminal status is `ended`;
- exactly one structured-output repair is counted;
- rejection feedback says `note` is invalid for `conversation_state` and suggests `@zack/m14bc-memory/notes`;
- the next review turn writes `m14h repaired note` to `notes`;
- review model requests use the process runtime's canonical `ModelRequest` and expose Memory plus `persistence_review_complete` aliases;
- review completes and stdout contains the original ordinary phase output.

## Test 2: Failed Review Warning Surfaces On Authored Abort

```bash
rm -f "$M14H_ABORT_WORK/runtime/m14h-process-model-calls.jsonl"

REPORT="$M14H_RUNS/abort/abort-review-failed-report.json"
STDOUT="$M14H_RUNS/abort/abort-review-failed-stdout.txt"
STDERR="$M14H_RUNS/abort/abort-review-failed-stderr.txt"

set +e
(cd "$M14H_ABORT_WORK" && "$APM" harness \
  --config agentpm.m14h.harness.json \
  --headless \
  --scope user="m14h-abort-user" \
  --scope conversation="m14h-abort-conversation" \
  --input-file inputs/m14h-review.txt \
  --report "$REPORT" \
  >"$STDOUT" 2>"$STDERR")
STATUS=$?
set -e

test "$STATUS" -ne 0

"$AGENTPM_MANUAL_PYTHON" "$M14H_ABORT_WORK/scripts/m14h_assert_report.py" \
  "$REPORT" \
  aborted review-failed-nonmutating feedback-suggested-notes review-failed-event process-canonical-review-request

grep -F "Warning: Memory write review at run_end failed: structured_output_repair_limit. Memory writes attempted/completed: 0/0. Intended Memory may not have been written." "$STDERR"
grep -F "terminal status Aborted" "$STDERR"
```

Expected:

- run exits nonzero because the authored `done` outcome transitions to `$abort`;
- `run_end` review still ran before the abort terminal was returned;
- repeated invalid review actions exhaust structured-output repair;
- no Memory write is attempted or completed;
- stderr contains the M14h warning and the normal aborted-terminal error.

## Cleanup

```bash
rm -f /tmp/setup-harness-m14h-manual.sh
rm -rf "$M14H_BASE"
```
