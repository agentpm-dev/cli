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

## What To Keep

Retain this directory with the release verification notes:

```text
harness-release-verify-test/runs/
  headless-report.json
  headless-stdout.txt
  headless-stderr.txt
  node-sdk-report.json
  python-sdk-report.json
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
