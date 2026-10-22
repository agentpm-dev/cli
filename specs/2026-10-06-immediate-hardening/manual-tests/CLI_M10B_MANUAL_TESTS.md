# Milestone 10B: CLI Telemetry and PostHog Handoff

Run from the `agentpm` repo root. These are individual commands, not a test
runner. No real model, account, PAT, package publication, or paid API is needed.
Fixtures are copied explicitly; they do not change `agentpm init` output.
Use Development only until the policy and CLI are released in Band 7.

## 1. Build and Prepare an Isolated Installation

```bash
cargo build -p agentpm-cli
export M10B_APM="$PWD/target/debug/agentpm"
export M10B_FIXTURES="$PWD/specs/2026-10-06-immediate-hardening/manual-tests/fixtures/m10b"
export M10B_ROOT="$(mktemp -d "${TMPDIR:-/tmp}/agentpm-m10b.XXXXXX")"
mkdir -p "$M10B_ROOT/workspace/.agentpm/loops/fixture/telemetry-loop/0.1.0" "$M10B_ROOT/home"
cp "$M10B_FIXTURES/agent.fixture.json" "$M10B_ROOT/workspace/agent.json"
cp "$M10B_FIXTURES/lock.fixture.json" "$M10B_ROOT/workspace/agent.lock"
cp "$M10B_FIXTURES/harness.fixture.json" "$M10B_ROOT/workspace/agentpm.harness.json"
cp "$M10B_FIXTURES/loop.fixture.json" "$M10B_ROOT/workspace/.agentpm/loops/fixture/telemetry-loop/0.1.0/agent.json"
export M10B_REAL_HOME="$HOME"
export HOME="$M10B_ROOT/home"
export USERPROFILE="$HOME"
export XDG_CONFIG_HOME="$HOME/config"
export XDG_DATA_HOME="$HOME/data"
export APPDATA="$HOME/data"
export OPENAI_API_KEY='local-fixture-only'
export OPENAI_BASE_URL='http://127.0.0.1:18140/model'
cd "$M10B_ROOT/workspace"
"$M10B_APM" --help
AGENTPM_TELEMETRY=0 "$M10B_APM" lint --strict
AGENTPM_TELEMETRY=0 "$M10B_APM" lint --strict .agentpm/loops/fixture/telemetry-loop/0.1.0/agent.json
```

Expected: help discloses default collection and both opt-outs; both manifests
lint clean. No telemetry state is created by help or opted-out lint.
The real HOME and credentials are not used for these CLI commands. Run these
exports in a disposable terminal and close it when finished, rather than
continuing normal work with the isolated HOME.

## 2. Inspect Local Capture Before Using PostHog

In a second terminal, run the fixture using the actual path printed by
`printf '%s\n' "$M10B_ROOT"` in the first terminal:

```bash
python3 /absolute/path/to/agentpm/specs/2026-10-06-immediate-hardening/manual-tests/fixtures/m10b/provider.py --events /the/printed/M10B_ROOT/events.jsonl
```

The fixture listens only on loopback port 18140 and prints each received
telemetry payload. `/model` returns a fixed result without logging prompts.
It does not forward anything to PostHog. If that port is occupied, choose a
different `--port` and adjust `OPENAI_BASE_URL` and the base URL below together.

In the first terminal, run each command individually:

```bash
export AGENTPM_TELEMETRY=1
"$M10B_APM" --base-url http://127.0.0.1:18140 harness --headless --input 'M10B PRIVATE_PROMPT @private/example /private/path'
"$M10B_APM" --base-url http://127.0.0.1:18140 harness --headless --input 'Second local run'
cat "$M10B_ROOT/events.jsonl"
find "$HOME" -name 'telemetry-*' -type f -print
```

Expected: each command exits 0 with `{"answer":"fixture-ok"}` on stdout and
normal preflight on stderr. There is one `cli_first_used` attempt for this
installation, two `harness_started` events, and no more than two completions.
Headless completion delivery can be lost at immediate exit; never add a flush
or a sleep to the CLI to make the chart look complete. All received events
share a random `install_id`; they contain version/platform and, for Harness,
`runtime_mode: headless` plus an outcome on completion. None contains the
private prompt/package/path, credentials, run ID, session ID or actual output.

For guaranteed observation before process exit, keep a TUI session open:

```bash
"$M10B_APM" --base-url http://127.0.0.1:18140 harness
```

Submit two prompts separately, leaving the TUI open after each completes.
Expected: two start/completion pairs with `runtime_mode: tui`, not one pair for
the session. Completed outcomes are `ended`; no extra first-use event.

For the machine surface, start the command and paste each JSON line separately,
waiting for its response before the next. This keeps the session open so queued
completions can be observed; these are real protocol commands, not fake events.

```bash
"$M10B_APM" --base-url http://127.0.0.1:18140 harness --machine
```

```json
{"protocol":"agentpm-harness-machine","version":1,"kind":"request","id":"init","method":"initialize","payload":{}}
```

```json
{"protocol":"agentpm-harness-machine","version":1,"kind":"request","id":"run","method":"start_run","payload":{"input":"Machine fixture run"}}
```

```json
{"protocol":"agentpm-harness-machine","version":1,"kind":"request","id":"stop","method":"shutdown","payload":{}}
```

Expected: valid protocol stdout and a `machine` run pair, with the same allowed
properties as TUI/headless. Initialization/shutdown alone are not Harness runs.

## 3. Environment and Persistent Opt-Out

```bash
wc -l "$M10B_ROOT/events.jsonl"
AGENTPM_TELEMETRY=0 "$M10B_APM" --base-url http://127.0.0.1:18140 harness --headless --input 'Environment opt-out'
wc -l "$M10B_ROOT/events.jsonl"
```

Expected: identical line count, output still fixture-ok, exit 0. If an earlier
in-flight event finishes, wait for it before recording the starting count.

Find the isolated `telemetry-id` path from section 2 and open `config.toml`
in that same directory, creating it if needed. Add only:

```toml
[telemetry]
enabled = false
```

```bash
"$M10B_APM" --base-url http://127.0.0.1:18140 harness --headless --input 'Persistent opt-out'
wc -l "$M10B_ROOT/events.jsonl"
```

Expected: still no new events, even with `AGENTPM_TELEMETRY=1`. This environment
value does not override a persistent false setting. For a mid-session check,
set `enabled = true`, open the TUI, submit one prompt, then edit it back to
false while the TUI remains open. Submit another prompt: no later events.
Restore `enabled = true` only in this isolated config for remaining tests.

## 4. Exit Safety and Kill

Stop the fixture with Ctrl-C and restart it with a long analytics delay:

```bash
python3 /absolute/path/to/provider.py --events /the/printed/M10B_ROOT/events.jsonl --analytics-delay 5
```

```bash
time "$M10B_APM" --base-url http://127.0.0.1:18140 harness --headless --input 'Stalled analytics'
printf 'Exit: %s\n' "$?"
```

Expected: fixture-ok, exit 0, no five-second shutdown wait or telemetry error.
The transport has a one-second timeout, no retries, and no exit-time join.

For a kill, restart the fixture with `--model-delay 30` and no analytics delay.
Start one headless command in the background, record its PID, wait until the
fixture prints its `harness_started`, then kill it:

```bash
"$M10B_APM" --base-url http://127.0.0.1:18140 harness --headless --input 'Kill fixture' &
export M10B_PID=$!
kill -KILL "$M10B_PID"
wait "$M10B_PID"
```

Expected: nonzero killed-process status and a start without completion. It
does not retry on the next command. Automated tests additionally cover panic
unwinding, handled errors, all terminal outcomes and unchanged failure output.

## 5. Development PostHog Capture (Together)

Restart the fixture normally (no delay overrides). Confirm the local Registry
has reloaded the new `/analytics/cli` route and uses **AgentPM Development**, not
Production. No additional keys are needed in the CLI or web. Choose a **fresh**
isolated HOME (repeat section 1 in a new terminal) if testing adoption: an
installation already claimed in local-capture mode does not emit first-use
again just because its Registry URL changes.

```bash
"$M10B_APM" --base-url http://127.0.0.1:5000 harness --headless --input 'Development telemetry verification'
"$M10B_APM" --base-url http://127.0.0.1:5000 harness
```

Submit two TUI prompts and leave it open until Activity shows completions.
Verify `cli_first_used`, `harness_started`, `harness_completed` have
`source: cli`, `environment: development`, `contract_version: 1`, and
`distinct_id: cli_<random UUID>`. Confirm no account identify event or IP,
URL, user-agent, prompt, package or path enrichment. Recheck the project's
**Discard client IP data** setting. Capture may appear asynchronously.

## 6. Extend Existing Early Product Funnel Dashboards (Together)

Do not create a second dashboard, change retention, or share it publicly.

1. Development: add **CLI Adoption (Unique Installations)**, Trends,
   `cli_first_used`, unique users/distinct IDs. Filter `source = cli` and
   `environment = development`. It measures once-per-installation attempts,
   not all CLI commands, accounts, downloads, or package installs.
2. Add **Harness Activity (Per Run)**, Trends, total event count for both
   `harness_started` and `harness_completed`. Break down by `runtime_mode`.
   Description: "Per-run, not per-session. Missing completions are an upper
   bound on failures, not a failure rate: kills, panic, sleep, failed/dropped
   sends and opt-out contribute. Missing telemetry is not zero usage."
3. Check counts against the real Development activity above, including a
   repeated TUI session and an opted-out run. Delivery is intentionally lossy;
   charts are not an exact execution ledger or a person-level funnel.
4. Copy these two insights to the existing Production dashboard and change
   the saved environment filter to `production`. Do not send synthetic events
   there. Confirm both dashboards remain private.
5. After Band 7 release and real Production capture, replace the M10A pending
   card with: "CLI adoption is once per anonymous installation. Harness activity
   is per run, by surface. CLI telemetry is default-on and optional; missing or
   opted-out telemetry is not zero usage. Registry install/publish completions
   are not local execution. CLI installations are not website accounts."

## Release Gate / Still Pending

Deploy the API endpoint, docs and Privacy Policy first; verify the public policy
contains this disclosure **before distributing the telemetry-enabled CLI**.
Then prepare/publish the CLI through the normal Band 7 release flow and verify
real Production payloads and dashboard filters. This implementation does not
bump versions, publish, deploy, or mark live/chart acceptance complete.
