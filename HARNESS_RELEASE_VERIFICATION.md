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

## 10. Provider Runtime And Schema Matrix

These checks cover representative built-in provider request shapes, custom
process-provider dispatch, custom-provider failure without silent fallback, and
the focused schema/host-provider contract tests that stand in for unavailable
live providers.

The deterministic capture server from Setup must be running. It accepts the
OpenAI, Anthropic, and Ollama request shapes on the same port configured by
`env.sh`.

Run the built-in provider matrix:

```bash
OPENAI_PROVIDER_REPORT="$HARNESS_VERIFY_OUT/provider-openai-report.json"
(
  cd "$HARNESS_VERIFY_PROVIDER_WORK"
  "$APM" harness \
    --config "$HARNESS_VERIFY_PROVIDER_OPENAI_CONFIG" \
    --headless \
    --scope "$HARNESS_VERIFY_SCOPE_KEY=$HARNESS_VERIFY_SCOPE_VALUE" \
    --input "$HARNESS_VERIFY_INPUT" \
    --report "$OPENAI_PROVIDER_REPORT" \
    >"$HARNESS_VERIFY_OUT/provider-openai-stdout.txt" \
    2>"$HARNESS_VERIFY_OUT/provider-openai-stderr.txt"
)
OPENAI_PROVIDER_TRACE="$("$AGENTPM_MANUAL_PYTHON" "$HARNESS_VERIFY_RUNNERS/extract_trace.py" "$OPENAI_PROVIDER_REPORT")"

ANTHROPIC_PROVIDER_REPORT="$HARNESS_VERIFY_OUT/provider-anthropic-report.json"
(
  cd "$HARNESS_VERIFY_PROVIDER_WORK"
  "$APM" harness \
    --config "$HARNESS_VERIFY_PROVIDER_ANTHROPIC_CONFIG" \
    --headless \
    --scope "$HARNESS_VERIFY_SCOPE_KEY=$HARNESS_VERIFY_SCOPE_VALUE" \
    --input "$HARNESS_VERIFY_INPUT" \
    --report "$ANTHROPIC_PROVIDER_REPORT" \
    >"$HARNESS_VERIFY_OUT/provider-anthropic-stdout.txt" \
    2>"$HARNESS_VERIFY_OUT/provider-anthropic-stderr.txt"
)
ANTHROPIC_PROVIDER_TRACE="$("$AGENTPM_MANUAL_PYTHON" "$HARNESS_VERIFY_RUNNERS/extract_trace.py" "$ANTHROPIC_PROVIDER_REPORT")"

OLLAMA_PROVIDER_REPORT="$HARNESS_VERIFY_OUT/provider-ollama-report.json"
(
  cd "$HARNESS_VERIFY_PROVIDER_WORK"
  "$APM" harness \
    --config "$HARNESS_VERIFY_PROVIDER_OLLAMA_CONFIG" \
    --headless \
    --scope "$HARNESS_VERIFY_SCOPE_KEY=$HARNESS_VERIFY_SCOPE_VALUE" \
    --input "$HARNESS_VERIFY_INPUT" \
    --report "$OLLAMA_PROVIDER_REPORT" \
    >"$HARNESS_VERIFY_OUT/provider-ollama-stdout.txt" \
    2>"$HARNESS_VERIFY_OUT/provider-ollama-stderr.txt"
)
OLLAMA_PROVIDER_TRACE="$("$AGENTPM_MANUAL_PYTHON" "$HARNESS_VERIFY_RUNNERS/extract_trace.py" "$OLLAMA_PROVIDER_REPORT")"
```

Run the custom process-provider success and failure cases:

```bash
PROCESS_PROVIDER_REPORT="$HARNESS_VERIFY_OUT/provider-process-report.json"
(
  cd "$HARNESS_VERIFY_PROVIDER_WORK"
  "$APM" harness \
    --config "$HARNESS_VERIFY_PROVIDER_PROCESS_CONFIG" \
    --headless \
    --scope "$HARNESS_VERIFY_SCOPE_KEY=$HARNESS_VERIFY_SCOPE_VALUE" \
    --input "$HARNESS_VERIFY_INPUT" \
    --report "$PROCESS_PROVIDER_REPORT" \
    >"$HARNESS_VERIFY_OUT/provider-process-stdout.txt" \
    2>"$HARNESS_VERIFY_OUT/provider-process-stderr.txt"
)
PROCESS_PROVIDER_TRACE="$("$AGENTPM_MANUAL_PYTHON" "$HARNESS_VERIFY_RUNNERS/extract_trace.py" "$PROCESS_PROVIDER_REPORT")"

PROCESS_FAILURE_REPORT="$HARNESS_VERIFY_OUT/provider-process-failure-report.json"
set +e
(
  cd "$HARNESS_VERIFY_PROVIDER_WORK"
  "$APM" harness \
    --config "$HARNESS_VERIFY_PROVIDER_PROCESS_FAILURE_CONFIG" \
    --headless \
    --scope "$HARNESS_VERIFY_SCOPE_KEY=$HARNESS_VERIFY_SCOPE_VALUE" \
    --input "$HARNESS_VERIFY_INPUT" \
    --report "$PROCESS_FAILURE_REPORT" \
    >"$HARNESS_VERIFY_OUT/provider-process-failure-stdout.txt" \
    2>"$HARNESS_VERIFY_OUT/provider-process-failure-stderr.txt"
)
PROCESS_FAILURE_EXIT=$?
if [ "$PROCESS_FAILURE_EXIT" -eq 0 ]; then
  echo "expected process provider failure, got exit 0" >&2
fi
if [ ! -s "$PROCESS_FAILURE_REPORT" ]; then
  echo "missing process failure report: $PROCESS_FAILURE_REPORT" >&2
else
  PROCESS_FAILURE_TRACE="$("$AGENTPM_MANUAL_PYTHON" "$HARNESS_VERIFY_RUNNERS/extract_trace.py" "$PROCESS_FAILURE_REPORT")"
fi

PROCESS_REQUEST_TIMEOUT_REPORT="$HARNESS_VERIFY_OUT/provider-process-request-timeout-report.json"
set +e
(
  cd "$HARNESS_VERIFY_PROVIDER_WORK"
  "$APM" harness \
    --config "$HARNESS_VERIFY_PROVIDER_PROCESS_REQUEST_TIMEOUT_CONFIG" \
    --headless \
    --scope "$HARNESS_VERIFY_SCOPE_KEY=$HARNESS_VERIFY_SCOPE_VALUE" \
    --input "$HARNESS_VERIFY_INPUT" \
    --report "$PROCESS_REQUEST_TIMEOUT_REPORT" \
    >"$HARNESS_VERIFY_OUT/provider-process-request-timeout-stdout.txt" \
    2>"$HARNESS_VERIFY_OUT/provider-process-request-timeout-stderr.txt"
)
PROCESS_REQUEST_TIMEOUT_EXIT=$?
if [ "$PROCESS_REQUEST_TIMEOUT_EXIT" -eq 0 ]; then
  echo "expected process provider request timeout, got exit 0" >&2
fi
if [ ! -s "$PROCESS_REQUEST_TIMEOUT_REPORT" ]; then
  echo "missing process request-timeout report: $PROCESS_REQUEST_TIMEOUT_REPORT" >&2
else
  PROCESS_REQUEST_TIMEOUT_TRACE="$("$AGENTPM_MANUAL_PYTHON" "$HARNESS_VERIFY_RUNNERS/extract_trace.py" "$PROCESS_REQUEST_TIMEOUT_REPORT")"
fi

PROCESS_MALFORMED_REPORT="$HARNESS_VERIFY_OUT/provider-process-malformed-report.json"
set +e
(
  cd "$HARNESS_VERIFY_PROVIDER_WORK"
  "$APM" harness \
    --config "$HARNESS_VERIFY_PROVIDER_PROCESS_MALFORMED_CONFIG" \
    --headless \
    --scope "$HARNESS_VERIFY_SCOPE_KEY=$HARNESS_VERIFY_SCOPE_VALUE" \
    --input "$HARNESS_VERIFY_INPUT" \
    --report "$PROCESS_MALFORMED_REPORT" \
    >"$HARNESS_VERIFY_OUT/provider-process-malformed-stdout.txt" \
    2>"$HARNESS_VERIFY_OUT/provider-process-malformed-stderr.txt"
)
PROCESS_MALFORMED_EXIT=$?
if [ "$PROCESS_MALFORMED_EXIT" -eq 0 ]; then
  echo "expected malformed process provider response failure, got exit 0" >&2
fi
if [ ! -s "$PROCESS_MALFORMED_REPORT" ]; then
  echo "missing malformed process report: $PROCESS_MALFORMED_REPORT" >&2
else
  PROCESS_MALFORMED_TRACE="$("$AGENTPM_MANUAL_PYTHON" "$HARNESS_VERIFY_RUNNERS/extract_trace.py" "$PROCESS_MALFORMED_REPORT")"
fi

set +e
(
  cd "$HARNESS_VERIFY_PROVIDER_WORK"
  "$APM" harness \
    --config "$HARNESS_VERIFY_PROVIDER_PROCESS_BAD_COMMAND_CONFIG" \
    --headless \
    --scope "$HARNESS_VERIFY_SCOPE_KEY=$HARNESS_VERIFY_SCOPE_VALUE" \
    --input "$HARNESS_VERIFY_INPUT" \
    --report "$HARNESS_VERIFY_OUT/provider-process-bad-command-report.json" \
    >"$HARNESS_VERIFY_OUT/provider-process-bad-command-stdout.txt" \
    2>"$HARNESS_VERIFY_OUT/provider-process-bad-command-stderr.txt"
)
PROCESS_BAD_COMMAND_EXIT=$?
if [ "$PROCESS_BAD_COMMAND_EXIT" -eq 0 ]; then
  echo "expected bad process command failure, got exit 0" >&2
fi

set +e
(
  cd "$HARNESS_VERIFY_PROVIDER_WORK"
  "$APM" harness \
    --config "$HARNESS_VERIFY_PROVIDER_PROCESS_STARTUP_TIMEOUT_CONFIG" \
    --headless \
    --scope "$HARNESS_VERIFY_SCOPE_KEY=$HARNESS_VERIFY_SCOPE_VALUE" \
    --input "$HARNESS_VERIFY_INPUT" \
    --report "$HARNESS_VERIFY_OUT/provider-process-startup-timeout-report.json" \
    >"$HARNESS_VERIFY_OUT/provider-process-startup-timeout-stdout.txt" \
    2>"$HARNESS_VERIFY_OUT/provider-process-startup-timeout-stderr.txt"
)
PROCESS_STARTUP_TIMEOUT_EXIT=$?
if [ "$PROCESS_STARTUP_TIMEOUT_EXIT" -eq 0 ]; then
  echo "expected process provider startup timeout, got exit 0" >&2
fi
set -e
```

Validate the generated artifacts and captured provider request shapes:

```bash
"$AGENTPM_MANUAL_PYTHON" "$HARNESS_VERIFY_RUNNERS/check_provider_matrix.py" \
  --provider-log "$HARNESS_VERIFY_OUT/provider-bodies.jsonl" \
  --case openai:ended:"$OPENAI_PROVIDER_REPORT":"$OPENAI_PROVIDER_TRACE" \
  --case anthropic:ended:"$ANTHROPIC_PROVIDER_REPORT":"$ANTHROPIC_PROVIDER_TRACE" \
  --case ollama:ended:"$OLLAMA_PROVIDER_REPORT":"$OLLAMA_PROVIDER_TRACE" \
  --case process:ended:"$PROCESS_PROVIDER_REPORT":"$PROCESS_PROVIDER_TRACE" \
  --case process-failure:failed:"$PROCESS_FAILURE_REPORT":"$PROCESS_FAILURE_TRACE" \
  --case process-request-timeout:failed:"$PROCESS_REQUEST_TIMEOUT_REPORT":"$PROCESS_REQUEST_TIMEOUT_TRACE" \
  --case process-malformed:failed:"$PROCESS_MALFORMED_REPORT":"$PROCESS_MALFORMED_TRACE" \
  --activation-failure process-bad-command:"$HARNESS_VERIFY_OUT/provider-process-bad-command-stderr.txt" \
  --activation-failure process-startup-timeout:"$HARNESS_VERIFY_OUT/provider-process-startup-timeout-stderr.txt" \
  | tee "$HARNESS_VERIFY_OUT/provider-runtime-schema-check.json"
```

Expected:

- exit code `0`
- `"status": "passed"`
- OpenAI, Anthropic, and Ollama provider request bodies all advertise
  `phase_complete` plus one executable Tool
- provider-facing schemas contain no unsupported top-level composition in the
  advertised Tool definitions
- OpenAI and Anthropic show required tool choice for explicit-completion
  phases, while Ollama records the intentional auto/fallback behavior
- the custom process provider reaches `ended`
- custom process-provider generate failure, request timeout, and malformed
  response failures reach `failed` through the process runtime without producing
  a built-in-provider request body
- custom process-provider bad command and startup-timeout activation failures
  fail before a Run can start, with stderr evidence and without built-in-provider
  fallback

Run the focused schema and host-provider contract tests and keep the logs:
If you have not already defined `run_compat` from Section 9, define it first.

```bash
run_compat "Provider Tool/Skill schemas" "$HARNESS_VERIFY_OUT/provider-schema-tool-skill.txt" \
  cargo test -p agentpm-cli action_parameter_schemas_use_resolved_tool_and_skill_metadata
run_compat "Provider MCP schema simplification" "$HARNESS_VERIFY_OUT/provider-schema-mcp.txt" \
  cargo test -p agentpm-cli external_mcp_provider_schema_strips_unsupported_composition_without_changing_runtime_schema
run_compat "Provider Knowledge schemas" "$HARNESS_VERIFY_OUT/provider-schema-knowledge.txt" \
  cargo test -p agentpm-cli knowledge_request_provider_schema_omits_top_level_any_of_for_openai_tools
run_compat "Provider Memory write schemas" "$HARNESS_VERIFY_OUT/provider-schema-memory-write.txt" \
  cargo test -p agentpm-cli memory_write_provider_actions_advertise_flat_shape_schemas
run_compat "Provider Memory read schemas" "$HARNESS_VERIFY_OUT/provider-schema-memory-read.txt" \
  cargo test -p agentpm-cli memory_read_provider_actions_advertise_flat_shape_schemas
run_compat "Process model semantic contract" "$HARNESS_VERIFY_OUT/provider-process-contract.txt" \
  cargo test -p agentpm-cli process_model_runtime_uses_agentpm_service_semantic_contract
run_compat "Host model semantic contract" "$HARNESS_VERIFY_OUT/provider-host-contract.txt" \
  cargo test -p agentpm-cli host_model_runtime_uses_machine_host_service_contract
run_compat "Host model capability advertisement" "$HARNESS_VERIFY_OUT/provider-host-capabilities.txt" \
  cargo test -p agentpm-cli host_model_runtime_uses_registered_capability_advertisement
```

Record live-provider skips explicitly if OpenAI, Anthropic, or Ollama cannot be
run against real credentials/runtime in the release environment. The generated
fixture is deterministic mocked coverage; it does not replace optional live
provider smoke evidence where credentials and a local Ollama runtime are
available.

## 11. SDK Host-Service Parity

These checks cover Node/Python parity for host model, embedding, Knowledge,
Hook, approval, cancellation, Memory provider/control, reports, and usage.

The generated `env.sh` exports the `AGENTPM_HARNESS_*` variables used by both
SDKs' real CLI integration tests. If you regenerated the fixture in a new
terminal, source it again before running this section:

```bash
source harness-release-verify-test/env.sh
```

Build the Node SDK once so the generated runner can import the package entry:
If you have not already defined `run_compat` from Section 9, define it first.

```bash
run_compat "Node SDK build" "$HARNESS_VERIFY_OUT/sdk-node-build.txt" \
  bash -lc 'cd ../agentpm-sdk-node && pnpm build'
```

Run the real Node/Python SDK parity runners. These use the same generated
workspace and register the same host model, embedding provider, Knowledge
runtime, Hooks, and approval controller:

```bash
export NODE_SDK_PARITY_REPORT="$HARNESS_VERIFY_OUT/sdk-node-parity-report.json"
export NODE_SDK_PARITY_SUMMARY="$HARNESS_VERIFY_OUT/sdk-node-parity-summary.json"
node "$HARNESS_VERIFY_RUNNERS/node-sdk-parity-runner.mjs" \
  >"$HARNESS_VERIFY_OUT/sdk-node-parity-stdout.txt" \
  2>"$HARNESS_VERIFY_OUT/sdk-node-parity-stderr.txt"

export PYTHON_SDK_PARITY_REPORT="$HARNESS_VERIFY_OUT/sdk-python-parity-report.json"
export PYTHON_SDK_PARITY_SUMMARY="$HARNESS_VERIFY_OUT/sdk-python-parity-summary.json"
"$AGENTPM_MANUAL_PYTHON" "$HARNESS_VERIFY_RUNNERS/python_sdk_parity_runner.py" \
  >"$HARNESS_VERIFY_OUT/sdk-python-parity-stdout.txt" \
  2>"$HARNESS_VERIFY_OUT/sdk-python-parity-stderr.txt"
```

Run the focused SDK contract tests. These include fake-machine protocol tests
for cancellation and external Memory-operation control, typed Hook capability
advertisement, typed embedding/Knowledge providers, Knowledge and Memory
process-provider serving, and each SDK's real CLI integration test against the
generated host-service workspace:

```bash
run_compat "Node SDK Harness parity" "$HARNESS_VERIFY_OUT/sdk-node-harness-parity.txt" \
  bash -lc 'cd ../agentpm-sdk-node && pnpm vitest run test/harness.spec.ts -t "routes host model, Hook, and approval requests through typed callbacks|advertises role-specific host service capabilities|registers typed embedding and Knowledge providers and dispatches host requests|maps cancellation and external Memory-operation control through machine requests|runs a real agentpm harness process with host model, embedding, Knowledge, Hook, approval, and report|serveKnowledgeRuntimeProcess|serveMemoryRuntimeProcess"'

run_compat "Python SDK Harness parity" "$HARNESS_VERIFY_OUT/sdk-python-harness-parity.txt" \
  bash -lc 'cd ../agentpm-sdk-python && uv run pytest -q tests/test_harness.py -k "routes_model_hook_and_approval_callbacks or advertises_typed_hook_helpers or registers_typed_embedding_and_knowledge_providers or cancellation_and_memory_operation_errors or real_agentpm_harness_process_when_fixture_env_is_set or serve_knowledge_runtime_process or serve_memory_runtime_process"'
```

Validate the parity artifacts:

```bash
"$AGENTPM_MANUAL_PYTHON" "$HARNESS_VERIFY_RUNNERS/check_sdk_parity.py" \
  --case node:"$NODE_SDK_PARITY_SUMMARY":"$NODE_SDK_PARITY_REPORT" \
  --case python:"$PYTHON_SDK_PARITY_SUMMARY":"$PYTHON_SDK_PARITY_REPORT" \
  --node-log "$HARNESS_VERIFY_OUT/sdk-node-harness-parity.txt" \
  --python-log "$HARNESS_VERIFY_OUT/sdk-python-harness-parity.txt" \
  | tee "$HARNESS_VERIFY_OUT/sdk-parity-check.json"
```

Expected:

- exit code `0`
- `"status": "passed"`
- Node and Python both reach terminal status `ended`
- both reports show the same phase path: `inspect -> answer`, then
  `respond -> complete`
- both reports include at least two Knowledge actions, non-empty usage, and a
  trace path
- both summaries show model, Hook, Knowledge, embedding, and approval callbacks
- focused SDK logs are non-empty and passing

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
  provider-openai-report.json
  provider-anthropic-report.json
  provider-ollama-report.json
  provider-process-report.json
  provider-process-failure-report.json
  provider-process-request-timeout-report.json
  provider-process-malformed-report.json
  provider-process-bad-command-stderr.txt
  provider-process-startup-timeout-stderr.txt
  provider-runtime-schema-check.json
  provider-schema-*.txt
  provider-process-contract.txt
  provider-host-*.txt
  sdk-node-build.txt
  sdk-node-parity-report.json
  sdk-node-parity-summary.json
  sdk-node-harness-parity.txt
  sdk-python-parity-report.json
  sdk-python-parity-summary.json
  sdk-python-harness-parity.txt
  sdk-parity-check.json
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
