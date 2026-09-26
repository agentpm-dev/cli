# Harness Release Verification

Use this guide to produce the cross-surface evidence consumed by `scripts/harness_release_verify.py`.

The script does not run Harness for you. It compares already-produced `report.json` and `events.jsonl` artifacts from the same representative scenario across execution surfaces.

## Goal

Run the same representative Agent/Loop through:

- one-shot headless CLI
- Node SDK machine client
- Python SDK machine client
- interactive TUI

Then compare stable HarnessEngine semantics:

- terminal status
- phase summaries
- checkpoint summaries
- action summaries
- MCP summaries
- Memory summaries
- diagnostics
- trace event type counts

## Setup

Run from the root of the `agentpm` repo:

```bash
cargo build -p agentpm-cli
scripts/setup-harness-release-verification.sh
source harness-release-verify-test/env.sh
python3 -B scripts/harness_release_verify.py --self-test
```

This creates a disposable deterministic fixture under `harness-release-verify-test/`.
Re-running the setup script deletes and recreates that directory.

In a second terminal, start the local OpenAI-compatible capture server:

```bash
source harness-release-verify-test/env.sh
"$AGENTPM_MANUAL_PYTHON" "$HARNESS_VERIFY_ROOT/fake_openai_server.py" \
  --port 18130 \
  --log "$HARNESS_VERIFY_OUT/provider-bodies.jsonl"
```

Keep that server running while executing the four surfaces below.

The setup script exports these defaults:

```bash
echo "$HARNESS_VERIFY_WORK"
echo "$HARNESS_VERIFY_CONFIG"
echo "$HARNESS_VERIFY_OUT"
```

Use the same Agent selector, config, scope, and input for every surface. If a surface needs extra host services, keep the semantic scenario equivalent and record that difference in your release notes.

## 1. Headless CLI

```bash
HEADLESS_REPORT="$HARNESS_VERIFY_OUT/headless-report.json"

(
  cd "$HARNESS_VERIFY_WORK"
  "$APM" harness \
    --config "$HARNESS_VERIFY_CONFIG" \
    --headless \
    --scope "$HARNESS_VERIFY_SCOPE_KEY=$HARNESS_VERIFY_SCOPE_VALUE" \
    --input "$HARNESS_VERIFY_INPUT" \
    --report "$HEADLESS_REPORT" \
    >"$HARNESS_VERIFY_OUT/headless-stdout.txt" \
    2>"$HARNESS_VERIFY_OUT/headless-stderr.txt"
)

HEADLESS_TRACE="$("$AGENTPM_MANUAL_PYTHON" "$HARNESS_VERIFY_RUNNERS/extract_trace.py" "$HEADLESS_REPORT")"
```

Check:

```bash
test -s "$HEADLESS_REPORT"
test -s "$HEADLESS_TRACE"
```

## 2. Node SDK Machine Client

Build the Node SDK if `dist/` is not current:

```bash
(cd ../agentpm-sdk-node && pnpm build)
```

Run the generated Node SDK client:

```bash
export NODE_REPORT="$HARNESS_VERIFY_OUT/node-sdk-report.json"
node "$HARNESS_VERIFY_RUNNERS/node-runner.mjs" \
  >"$HARNESS_VERIFY_OUT/node-sdk-stdout.txt" \
  2>"$HARNESS_VERIFY_OUT/node-sdk-stderr.txt"

NODE_TRACE="$("$AGENTPM_MANUAL_PYTHON" "$HARNESS_VERIFY_RUNNERS/extract_trace.py" "$NODE_REPORT")"
test -s "$NODE_REPORT"
test -s "$NODE_TRACE"
```

If your client writes the report to another path, set `NODE_REPORT` to that path before the final comparison.

## 3. Python SDK Machine Client

Run the generated Python SDK client:

```bash
export PYTHON_REPORT="$HARNESS_VERIFY_OUT/python-sdk-report.json"
"$AGENTPM_MANUAL_PYTHON" "$HARNESS_VERIFY_RUNNERS/python_runner.py" \
  >"$HARNESS_VERIFY_OUT/python-sdk-stdout.txt" \
  2>"$HARNESS_VERIFY_OUT/python-sdk-stderr.txt"

PYTHON_TRACE="$("$AGENTPM_MANUAL_PYTHON" "$HARNESS_VERIFY_RUNNERS/extract_trace.py" "$PYTHON_REPORT")"
test -s "$PYTHON_REPORT"
test -s "$PYTHON_TRACE"
```

If your client writes the report to another path, set `PYTHON_REPORT` to that path before the final comparison.

## 4. Interactive TUI

Run the same scenario manually in the TUI:

```bash
(
  cd "$HARNESS_VERIFY_WORK"
  "$APM" harness \
    --config "$HARNESS_VERIFY_CONFIG" \
    --scope "$HARNESS_VERIFY_SCOPE_KEY=$HARNESS_VERIFY_SCOPE_VALUE"
)
```

In the TUI:

1. Enter the same prompt as `HARNESS_VERIFY_INPUT`.
2. Wait for the Run to reach a terminal state.
3. Open the Reports tab.
4. Record the displayed `report:` and `trace:` paths.
5. Quit the TUI.

Then export those paths:

```bash
export TUI_REPORT="<path shown in Reports tab>"
export TUI_TRACE="<path shown in Reports tab, or report.trace_path>"

test -s "$TUI_REPORT"
test -s "$TUI_TRACE"
```

If you only captured `TUI_REPORT`, derive the trace path:

```bash
TUI_TRACE="$(python3 - "$TUI_REPORT" <<'PY'
import json, sys
from pathlib import Path
print(json.loads(Path(sys.argv[1]).read_text())["trace_path"])
PY
)"
```

## 5. Run The Equivalence Gate

```bash
python3 -B scripts/harness_release_verify.py \
  --case headless=headless:"$HEADLESS_REPORT":"$HEADLESS_TRACE" \
  --case node-sdk=node-sdk:"$NODE_REPORT":"$NODE_TRACE" \
  --case python-sdk=python-sdk:"$PYTHON_REPORT":"$PYTHON_TRACE" \
  --case tui=tui:"$TUI_REPORT":"$TUI_TRACE" \
  --evidence "$HARNESS_VERIFY_OUT/harness-surface-equivalence.json" \
  | tee "$HARNESS_VERIFY_OUT/harness-surface-equivalence.stdout.json"
```

Expected:

- exit code `0`
- `"status": "passed"`
- `"mismatches": []`
- each case has non-empty `phase_summaries`
- each case has non-empty `trace_event_counts`

Do not pass `--allow-empty` for release verification evidence.

## 6. Headless Surface Matrix

These checks cover direct text, stdin, input-file, stdout/stderr separation,
report/trace writing, deterministic shutdown, and the headless
`approval_required` terminal path.

The direct-text case is the headless artifact from step 1. Add the stdin and
input-file cases:

```bash
STDIN_REPORT="$HARNESS_VERIFY_OUT/headless-stdin-report.json"
printf '%s\n' "$HARNESS_VERIFY_INPUT" | (
  cd "$HARNESS_VERIFY_WORK"
  "$APM" harness \
    --config "$HARNESS_VERIFY_CONFIG" \
    --headless \
    --scope "$HARNESS_VERIFY_SCOPE_KEY=$HARNESS_VERIFY_SCOPE_VALUE" \
    --report "$STDIN_REPORT" \
    >"$HARNESS_VERIFY_OUT/headless-stdin-stdout.txt" \
    2>"$HARNESS_VERIFY_OUT/headless-stdin-stderr.txt"
)
STDIN_TRACE="$("$AGENTPM_MANUAL_PYTHON" "$HARNESS_VERIFY_RUNNERS/extract_trace.py" "$STDIN_REPORT")"

INPUT_FILE="$HARNESS_VERIFY_OUT/headless-input.txt"
INPUT_FILE_REPORT="$HARNESS_VERIFY_OUT/headless-input-file-report.json"
printf '%s\n' "$HARNESS_VERIFY_INPUT" >"$INPUT_FILE"
(
  cd "$HARNESS_VERIFY_WORK"
  "$APM" harness \
    --config "$HARNESS_VERIFY_CONFIG" \
    --headless \
    --scope "$HARNESS_VERIFY_SCOPE_KEY=$HARNESS_VERIFY_SCOPE_VALUE" \
    --input-file "$INPUT_FILE" \
    --report "$INPUT_FILE_REPORT" \
    >"$HARNESS_VERIFY_OUT/headless-input-file-stdout.txt" \
    2>"$HARNESS_VERIFY_OUT/headless-input-file-stderr.txt"
)
INPUT_FILE_TRACE="$("$AGENTPM_MANUAL_PYTHON" "$HARNESS_VERIFY_RUNNERS/extract_trace.py" "$INPUT_FILE_REPORT")"

test -s "$STDIN_REPORT"
test -s "$STDIN_TRACE"
test -s "$INPUT_FILE_REPORT"
test -s "$INPUT_FILE_TRACE"
```

Check stdout/stderr and terminal status:

```bash
cmp -s "$HARNESS_VERIFY_OUT/headless-stdout.txt" "$HARNESS_VERIFY_OUT/headless-stdin-stdout.txt"
cmp -s "$HARNESS_VERIFY_OUT/headless-stdout.txt" "$HARNESS_VERIFY_OUT/headless-input-file-stdout.txt"
test -s "$HARNESS_VERIFY_OUT/headless-stderr.txt"
cmp -s "$HARNESS_VERIFY_OUT/headless-stderr.txt" "$HARNESS_VERIFY_OUT/headless-stdin-stderr.txt"
cmp -s "$HARNESS_VERIFY_OUT/headless-stderr.txt" "$HARNESS_VERIFY_OUT/headless-input-file-stderr.txt"

"$AGENTPM_MANUAL_PYTHON" - "$HEADLESS_REPORT" "$STDIN_REPORT" "$INPUT_FILE_REPORT" <<'PY'
import json
import sys
from pathlib import Path

for path in sys.argv[1:]:
    report = json.loads(Path(path).read_text())
    assert report["terminal_status"] == "ended", path
    assert report.get("trace_path"), path
    assert report.get("phase_summaries"), path
print("headless success matrix passed")
PY
```

Run the approval-required headless case. This command is expected to exit
non-zero because plain headless cannot wait for interactive approval, but it
must still write a report/trace with terminal status `approval_required`.

```bash
APPROVAL_REPORT="$HARNESS_VERIFY_OUT/headless-approval-required-report.json"
set +e
(
  cd "$HARNESS_VERIFY_APPROVAL_WORK"
  "$APM" harness \
    --config "$HARNESS_VERIFY_APPROVAL_CONFIG" \
    --headless \
    --scope "$HARNESS_VERIFY_SCOPE_KEY=$HARNESS_VERIFY_SCOPE_VALUE" \
    --input "$HARNESS_VERIFY_INPUT" \
    --report "$APPROVAL_REPORT" \
    >"$HARNESS_VERIFY_OUT/headless-approval-required-stdout.txt" \
    2>"$HARNESS_VERIFY_OUT/headless-approval-required-stderr.txt"
)
APPROVAL_EXIT=$?
set -e
APPROVAL_TRACE="$("$AGENTPM_MANUAL_PYTHON" "$HARNESS_VERIFY_RUNNERS/extract_trace.py" "$APPROVAL_REPORT")"

test "$APPROVAL_EXIT" -ne 0
test ! -s "$HARNESS_VERIFY_OUT/headless-approval-required-stdout.txt"
test -s "$HARNESS_VERIFY_OUT/headless-approval-required-stderr.txt"
test -s "$APPROVAL_REPORT"
test -s "$APPROVAL_TRACE"

"$AGENTPM_MANUAL_PYTHON" - "$APPROVAL_REPORT" <<'PY'
import json
import sys
from pathlib import Path

report = json.loads(Path(sys.argv[1]).read_text())
assert report["terminal_status"] == "approval_required", report["terminal_status"]
assert report.get("trace_path")
print("headless approval_required path passed")
PY
```

## 7. Repeated Run State

This check keeps one Node SDK Harness Session alive across two Runs, edits
Consumer Context between the Runs, and confirms each Run has its own report and
trace while the machine client stays alive for both.

Build the Node SDK if `dist/` is not current:

```bash
(cd ../agentpm-sdk-node && pnpm build)
```

Run the generated repeated-run client:

```bash
export NODE_REPEAT_REPORT_ONE="$HARNESS_VERIFY_OUT/node-repeat-run-1-report.json"
export NODE_REPEAT_REPORT_TWO="$HARNESS_VERIFY_OUT/node-repeat-run-2-report.json"
export NODE_REPEAT_SUMMARY="$HARNESS_VERIFY_OUT/node-repeat-runs-summary.json"

node "$HARNESS_VERIFY_RUNNERS/node-repeated-runner.mjs" \
  >"$HARNESS_VERIFY_OUT/node-repeat-stdout.txt" \
  2>"$HARNESS_VERIFY_OUT/node-repeat-stderr.txt"

NODE_REPEAT_TRACE_ONE="$("$AGENTPM_MANUAL_PYTHON" "$HARNESS_VERIFY_RUNNERS/extract_trace.py" "$NODE_REPEAT_REPORT_ONE")"
NODE_REPEAT_TRACE_TWO="$("$AGENTPM_MANUAL_PYTHON" "$HARNESS_VERIFY_RUNNERS/extract_trace.py" "$NODE_REPEAT_REPORT_TWO")"

test -s "$NODE_REPEAT_REPORT_ONE"
test -s "$NODE_REPEAT_REPORT_TWO"
test -s "$NODE_REPEAT_TRACE_ONE"
test -s "$NODE_REPEAT_TRACE_TWO"
test -s "$NODE_REPEAT_SUMMARY"
```

Check that the Runs are distinct, terminal, and structurally reset:

```bash
"$AGENTPM_MANUAL_PYTHON" - "$NODE_REPEAT_REPORT_ONE" "$NODE_REPEAT_REPORT_TWO" <<'PY'
import json
import sys
from pathlib import Path

first = json.loads(Path(sys.argv[1]).read_text())
second = json.loads(Path(sys.argv[2]).read_text())
assert first["session_id"] == second["session_id"]
assert first["run_id"] != second["run_id"]
assert first["terminal_status"] == "ended"
assert second["terminal_status"] == "ended"
assert len(first.get("phase_summaries") or []) == len(second.get("phase_summaries") or []) == 2
assert len(first.get("action_summaries") or []) == len(second.get("action_summaries") or []) == 1
assert first.get("trace_path") != second.get("trace_path")
print("repeated SDK Run reset evidence passed")
PY
```

Check Consumer Context reload evidence in the provider capture log. The first
marker should appear in an earlier provider request than the second marker.

```bash
"$AGENTPM_MANUAL_PYTHON" - "$HARNESS_VERIFY_OUT/provider-bodies.jsonl" <<'PY'
import json
import sys
from pathlib import Path

first = []
second = []
for line in Path(sys.argv[1]).read_text().splitlines():
    if not line.strip():
        continue
    row = json.loads(line)
    text = json.dumps(row.get("body", {}))
    if "first-run-context" in text:
        first.append(row["sequence"])
    if "second-run-context" in text:
        second.append(row["sequence"])

assert first, "first context marker not observed"
assert second, "second context marker not observed"
assert min(first) < min(second), (first, second)
print("Consumer Context reload evidence passed")
PY
```

For TUI repeated-run evidence, run the TUI from step 4, submit two prompts in
one TUI Session, edit `context.md` between Runs, and confirm the Reports tab
shows distinct Run report/trace paths for each terminal Run. Keep terminal
captures or notes with the release evidence.

## 8. Terminal Paths And Redaction

These checks prove representative terminal paths still write syntactically
valid report/trace artifacts, and that planted secret markers do not appear in
reports, traces, captured provider request bodies, or headless stdout under
`trace.content = full`, `redacted`, or `none`.

The setup fixture creates a separate workspace for this so the main
cross-surface equivalence evidence remains stable:

```bash
echo "$HARNESS_VERIFY_REDACTION_WORK"
echo "$HARNESS_VERIFY_REDACTION_FULL_CONFIG"
echo "$HARNESS_VERIFY_REDACTION_REDACTED_CONFIG"
echo "$HARNESS_VERIFY_REDACTION_NONE_CONFIG"
echo "$HARNESS_VERIFY_LIMIT_CONFIG"
echo "$HARNESS_VERIFY_FAILURE_CONFIG"
```

Run the three trace-content success cases:

```bash
FULL_REPORT="$HARNESS_VERIFY_OUT/terminal-redaction-full-report.json"
(
  cd "$HARNESS_VERIFY_REDACTION_WORK"
  "$APM" harness \
    --config "$HARNESS_VERIFY_REDACTION_FULL_CONFIG" \
    --headless \
    --scope "$HARNESS_VERIFY_SCOPE_KEY=$HARNESS_VERIFY_SCOPE_VALUE" \
    --input "$HARNESS_VERIFY_INPUT" \
    --report "$FULL_REPORT" \
    >"$HARNESS_VERIFY_OUT/terminal-redaction-full-stdout.txt" \
    2>"$HARNESS_VERIFY_OUT/terminal-redaction-full-stderr.txt"
)
FULL_TRACE="$("$AGENTPM_MANUAL_PYTHON" "$HARNESS_VERIFY_RUNNERS/extract_trace.py" "$FULL_REPORT")"

REDACTED_REPORT="$HARNESS_VERIFY_OUT/terminal-redaction-redacted-report.json"
(
  cd "$HARNESS_VERIFY_REDACTION_WORK"
  "$APM" harness \
    --config "$HARNESS_VERIFY_REDACTION_REDACTED_CONFIG" \
    --headless \
    --scope "$HARNESS_VERIFY_SCOPE_KEY=$HARNESS_VERIFY_SCOPE_VALUE" \
    --input "$HARNESS_VERIFY_INPUT" \
    --report "$REDACTED_REPORT" \
    >"$HARNESS_VERIFY_OUT/terminal-redaction-redacted-stdout.txt" \
    2>"$HARNESS_VERIFY_OUT/terminal-redaction-redacted-stderr.txt"
)
REDACTED_TRACE="$("$AGENTPM_MANUAL_PYTHON" "$HARNESS_VERIFY_RUNNERS/extract_trace.py" "$REDACTED_REPORT")"

NONE_REPORT="$HARNESS_VERIFY_OUT/terminal-redaction-none-report.json"
(
  cd "$HARNESS_VERIFY_REDACTION_WORK"
  "$APM" harness \
    --config "$HARNESS_VERIFY_REDACTION_NONE_CONFIG" \
    --headless \
    --scope "$HARNESS_VERIFY_SCOPE_KEY=$HARNESS_VERIFY_SCOPE_VALUE" \
    --input "$HARNESS_VERIFY_INPUT" \
    --report "$NONE_REPORT" \
    >"$HARNESS_VERIFY_OUT/terminal-redaction-none-stdout.txt" \
    2>"$HARNESS_VERIFY_OUT/terminal-redaction-none-stderr.txt"
)
NONE_TRACE="$("$AGENTPM_MANUAL_PYTHON" "$HARNESS_VERIFY_RUNNERS/extract_trace.py" "$NONE_REPORT")"
```

Run the `limit_reached` and `failed` terminal paths. These commands are
expected to exit non-zero after writing their report/trace artifacts.

```bash
LIMIT_REPORT="$HARNESS_VERIFY_OUT/terminal-limit-report.json"
set +e
(
  cd "$HARNESS_VERIFY_REDACTION_WORK"
  "$APM" harness \
    --config "$HARNESS_VERIFY_LIMIT_CONFIG" \
    --headless \
    --scope "$HARNESS_VERIFY_SCOPE_KEY=$HARNESS_VERIFY_SCOPE_VALUE" \
    --input "$HARNESS_VERIFY_INPUT" \
    --report "$LIMIT_REPORT" \
    >"$HARNESS_VERIFY_OUT/terminal-limit-stdout.txt" \
    2>"$HARNESS_VERIFY_OUT/terminal-limit-stderr.txt"
)
LIMIT_EXIT=$?
set -e
test "$LIMIT_EXIT" -ne 0
LIMIT_TRACE="$("$AGENTPM_MANUAL_PYTHON" "$HARNESS_VERIFY_RUNNERS/extract_trace.py" "$LIMIT_REPORT")"

FAILURE_REPORT="$HARNESS_VERIFY_OUT/terminal-failure-report.json"
set +e
(
  cd "$HARNESS_VERIFY_REDACTION_WORK"
  "$APM" harness \
    --config "$HARNESS_VERIFY_FAILURE_CONFIG" \
    --headless \
    --scope "$HARNESS_VERIFY_SCOPE_KEY=$HARNESS_VERIFY_SCOPE_VALUE" \
    --input "$HARNESS_VERIFY_INPUT" \
    --report "$FAILURE_REPORT" \
    >"$HARNESS_VERIFY_OUT/terminal-failure-stdout.txt" \
    2>"$HARNESS_VERIFY_OUT/terminal-failure-stderr.txt"
)
FAILURE_EXIT=$?
set -e
test "$FAILURE_EXIT" -ne 0
FAILURE_TRACE="$("$AGENTPM_MANUAL_PYTHON" "$HARNESS_VERIFY_RUNNERS/extract_trace.py" "$FAILURE_REPORT")"
```

Validate the generated artifacts and secret redaction. If you did not run the
approval-required case in Section 6, remove the final `approval` case line.

```bash
"$AGENTPM_MANUAL_PYTHON" "$HARNESS_VERIFY_RUNNERS/check_terminal_artifacts.py" \
  --secret "$HARNESS_VERIFY_SECRET_MARKER" \
  --secret "$HARNESS_VERIFY_CAMEL_SECRET_MARKER" \
  --secret "$HARNESS_VERIFY_PRIVATE_KEY_SECRET_MARKER" \
  --provider-log "$HARNESS_VERIFY_OUT/provider-bodies.jsonl" \
  --stdout "$HARNESS_VERIFY_OUT/terminal-redaction-full-stdout.txt" \
  --stdout "$HARNESS_VERIFY_OUT/terminal-redaction-redacted-stdout.txt" \
  --stdout "$HARNESS_VERIFY_OUT/terminal-redaction-none-stdout.txt" \
  --case full:ended:"$FULL_REPORT":"$FULL_TRACE" \
  --case redacted:ended:"$REDACTED_REPORT":"$REDACTED_TRACE" \
  --case none:ended:"$NONE_REPORT":"$NONE_TRACE" \
  --case limit:limit_reached:"$LIMIT_REPORT":"$LIMIT_TRACE" \
  --case failure:failed:"$FAILURE_REPORT":"$FAILURE_TRACE" \
  --case approval:approval_required:"$APPROVAL_REPORT":"$APPROVAL_TRACE" \
  | tee "$HARNESS_VERIFY_OUT/terminal-artifacts-redaction-check.json"
```

Expected:

- exit code `0`
- `"status": "passed"`
- every case has a non-empty trace
- every case records `trace_path`
- the planted secret marker values are absent from every report, trace,
  captured provider request body, and headless stdout file

Cancellation and handoff are surface-specific interactive paths. Retain the TUI
or SDK cancellation evidence with the release notes when those paths are
exercised; this generated headless matrix does not attempt to simulate a human
cancelling an active Run.

## 9. Compatibility Sweep

These commands provide the evidence for package-kind compatibility and existing
publish/install/new/build/query/registry/API/web behavior. Run the applicable
commands and retain stdout/stderr under `harness-release-verify-test/runs/` or
in the release notes. If a command is skipped because credentials or external
services are unavailable, record the skip reason.

Define a quiet runner first. It writes the full command output to the named log
file and only prints the tail when a command fails:

```bash
run_compat() {
  local label="$1"
  local log="$2"
  shift 2
  echo "running $label..."
  if "$@" >"$log" 2>&1; then
    echo "passed $label -> $log"
  else
    local exit_code=$?
    echo "failed $label -> $log"
    tail -80 "$log"
    return "$exit_code"
  fi
}
```

CLI compatibility:

```bash
run_compat "CLI new" "$HARNESS_VERIFY_OUT/compat-cli-new.txt" \
  cargo test -p agentpm-cli commands::new::tests
run_compat "CLI install" "$HARNESS_VERIFY_OUT/compat-cli-install.txt" \
  cargo test -p agentpm-cli commands::install::tests
run_compat "CLI publish" "$HARNESS_VERIFY_OUT/compat-cli-publish.txt" \
  cargo test -p agentpm-cli commands::publish::tests
run_compat "CLI manifest" "$HARNESS_VERIFY_OUT/compat-cli-manifest.txt" \
  cargo test -p agentpm-cli manifest::tests
run_compat "CLI run" "$HARNESS_VERIFY_OUT/compat-cli-run.txt" \
  cargo test -p agentpm-cli commands::run::tests
run_compat "CLI export" "$HARNESS_VERIFY_OUT/compat-cli-export.txt" \
  cargo test -p agentpm-cli commands::export::tests
```

SDK metadata-loader compatibility:

```bash
run_compat "Node SDK" "$HARNESS_VERIFY_OUT/compat-node-sdk.txt" \
  bash -lc 'cd ../agentpm-sdk-node && pnpm test'
run_compat "Python SDK" "$HARNESS_VERIFY_OUT/compat-python-sdk.txt" \
  bash -lc 'cd ../agentpm-sdk-python && uv run pytest -q'
```

Registry/API and web compatibility:

```bash
run_compat "API" "$HARNESS_VERIFY_OUT/compat-api.txt" \
  bash -lc 'cd ../agentpm-api && REGISTRY_PRIVATE_KEY_B64=AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA= uv run python -m pytest -q'
run_compat "Web tests" "$HARNESS_VERIFY_OUT/compat-web-test.txt" \
  bash -lc 'cd ../agentpm-web && pnpm test'
run_compat "Web typecheck" "$HARNESS_VERIFY_OUT/compat-web-typecheck.txt" \
  bash -lc 'cd ../agentpm-web && pnpm typecheck'
```

This sweep is intentionally broad. The expected release result is that all
commands pass, except for explicitly recorded environment skips. The only
intentional compatibility exceptions for this release band remain the Loop
checkpoint relaxation and Memory transform `output_mode` addition.

## What To Keep

Retain this directory with the release verification notes:

```text
harness-release-verify-test/runs/
  headless-report.json
  headless-stdout.txt
  headless-stderr.txt
  node-sdk-report.json
  python-sdk-report.json
  headless-stdin-report.json
  headless-input-file-report.json
  headless-approval-required-report.json
  terminal-redaction-full-report.json
  terminal-redaction-redacted-report.json
  terminal-redaction-none-report.json
  terminal-limit-report.json
  terminal-failure-report.json
  terminal-artifacts-redaction-check.json
  compat-cli-*.txt
  compat-node-sdk.txt
  compat-python-sdk.txt
  compat-api.txt
  compat-web-*.txt
  node-repeat-run-1-report.json
  node-repeat-run-2-report.json
  node-repeat-runs-summary.json
  harness-surface-equivalence.json
  harness-surface-equivalence.stdout.json
```

Also retain or reference the four `events.jsonl` files named by the reports.

## Troubleshooting

`case_count` mismatch:

- Fewer than two cases were provided. Release evidence should include all four required surfaces.

`empty_semantics` mismatch:

- The report or trace exists but did not contain phase summaries or trace event counts. Re-run that surface and confirm the scenario actually executed.

`terminal_status` mismatch:

- One surface ended differently. Check stdout/stderr, diagnostics, and provider/host availability.

`phase_summaries` mismatch:

- The surfaces traversed different Loop outcomes or transitions. Confirm the same Agent, config, scope, provider behavior, and input were used.

`action_summaries`, `mcp_summaries`, or `memory_summaries` mismatch:

- The surfaces used different runtime capability availability or host/provider wiring. Check SDK host registration, process providers, MCP imports/exports, and Memory state paths.

`trace_event_counts` mismatch:

- The surfaces may have produced the same report summary but different event sequencing or observability. Inspect the corresponding `events.jsonl` files before treating the run as equivalent.
