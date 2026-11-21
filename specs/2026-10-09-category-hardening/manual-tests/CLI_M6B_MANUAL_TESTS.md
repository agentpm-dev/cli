# M6B: Exact Component Versions and SDK Reports

Claude review passed. Hosted CI on matching CLI/SDK revisions and your manual
acceptance remain pending. These instructions use the reviewed local checkouts;
they do not require publishing a new CLI or SDK first.

Use **one macOS/zsh terminal**, starting in the CLI repo. All generated inputs,
logs and results stay in **`agentpm/cli-m6b-test/<run>/`**, which is gitignored.
Run one block at a time and open that directory in your editor.

There is **no Registry, API, database, S3, signing or paid model** in this guide.
The child environment isolates CLI configuration, disables telemetry and selects
a non-listening loopback Registry address. Your shell's credentials are unchanged.
The model is a short local script that requests one real Node Tool invocation,
then completes the Loop's `work` and `review` phases. It proves Engine behavior
with known requests, not LLM quality or full APDS execution conformance.

The fixtures reuse the reviewed CLI-owned SDK fixture factory. Knowledge and
Memory go through actual CLI scaffolding/builds; all manifests go through actual
`agentpm lint`. The helper creates **synthetic installed state** with placeholder
integrities, allowing us to inspect multiple versions and intentional ambiguity
without publishing bad packages. These are not real installations, signatures or
digest evidence. In a consumer app, change declarations and run `agentpm install`;
do not manually edit installed packages or locks as this fixture setup does.

| Section | What you prove                                                              |
| ------- | --------------------------------------------------------------------------- |
| 1       | Prepare one owned run, pin the CLI/SDK/helper bytes, inspect visible inputs |
| 2       | Two Agents select different versions of all six Component kinds             |
| 3       | Real headless execution uses the selected direct or Skill-inherited Tool    |
| 4       | Both SDKs host the model and expose the same exact runtime versions         |
| 5       | Phase-only Skill inheritance and separately bound Memory operations         |
| 6       | Ambiguous active versions block execution without APDS-invalid labeling     |
| 7       | Removing the direct Tool edge resolves ambiguity through Skill inheritance  |
| 8       | CLI/Node/Python report parity for five compatibility/readiness states       |
| 9       | Reproduce the description prompt experiment and its limited conclusion      |
| 10      | Observe both Agents in the TUI and complete a Run with each                 |
| 11      | Audit all 30 command results and both TUI observations                      |

`m6b_expect` checks the exact command and case directory, prints the actual
command/output and **ACTUAL exit / EXPECTED exit**, then inspects the result.
It verifies semantic fields, diagnostics, runtime snapshots, actual Tool markers
and unchanged inputs. An unrelated failure cannot pass a negative test. Both
streams and raw requests are retained under `evidence/` and the case directory.
Stop if it reports a mismatch. Results are never silently overwritten.

## 1. Build and prepare once

Prerequisites: the workspace's existing Rust toolchain, Node 20+, pnpm,
Python 3.11+, and the Python SDK's already provisioned `.venv` dependencies.
These are checkout SDKs with M6B changes, not the published 0.1.17 packages.
The Node probe imports the actual built **ESM `dist/index.js`** entrypoint;
the Python probe imports this checkout's `src/agentpm`.

Start at the **CLI repository root**. Adjust the first path only if necessary:

```bash
cd /Users/zackhine/projects/agentpm-project/agentpm
cargo build --locked -p agentpm-cli
pnpm --dir ../agentpm-sdk-node build
export M6B_REPO="$PWD"
export M6B_APM="$M6B_REPO/target/debug/agentpm"
export M6B_PYTHON="$(command -v python3)"
export M6B_FIXTURE="$M6B_REPO/specs/2026-10-09-category-hardening/manual-tests/fixtures/m6b/fixture.py"
export M6B_ROOT="$M6B_REPO/cli-m6b-test/$(date -u +%Y%m%dT%H%M%SZ)-$$"
umask 077
"$M6B_PYTHON" -B "$M6B_FIXTURE" prepare "$M6B_ROOT" --cli "$M6B_APM"
source "$M6B_ROOT/paths.sh"
cd "$M6B_ROOT"
pwd
git -C "$M6B_REPO" check-ignore "$M6B_ROOT/session.json"
cat session.json
ls cases
cat runtime/model_service.py
```

Expected: **10 visible cases and 30 command expectations**. Setup itself starts
no Agent Runs. `evidence/preparation.json` records real scaffold/build/lint
commands and their outputs, including expected missing/unsupported declaration
failures. `expectations.json` lists the exact commands/exits for this run.

Do not rebuild or edit the CLI, SDKs, helper or prepared inputs midway through
the run: their hashes are pinned. Choose a fresh run if they change. Existing
timestamp directories are never overwritten or cleaned by the helper.

If you close the terminal, open **your operator timestamp directory** and run
`source ./paths.sh`. That is the generated `paths.sh` next to `session.json`;
do not choose an automated `builder-*` directory or another operator run.

## 2. Inspect the two-Agent workspace

Both versions are installed side by side. Agent **`consumer-1`** declares Tool
1.0.0 directly and through Skill 1.0.0. Agent **`consumer-2`** has no direct Tool
declaration or binding: Skill 2.0.0 supplies Tool 2.0.0. Each Agent also selects
its own Knowledge, Memory, Profile and Loop version. Both Agent artifact versions
are 1.0.0; the Component versions distinguish the graphs.

```bash
cd "$M6B_ROOT/cases/standalone"
m6b_fixture describe standalone
cat agents/agent-1.agent.json
cat agents/agent-2.agent.json
cat agent.lock
cat .agentpm/skills/fixture/skill/2.0.0/agent.json
cat .agentpm/loops/fixture/workflow/2.0.0/agent.json
m6b_expect json-1 0 m6b harness consumer-1 --json
m6b_expect json-2 0 m6b harness consumer-2 --json
```

Expected each time: **APDS `conformant` / Harness `supported` / local `ready`**,
support list `agentpm / 1.0.0`, and `runner_evaluated: false`. Inspect the
`apds.artifacts` keys: each report selects its own exact Component graph. Static
preflight alone is not execution proof; the following sections execute it.

## 3. Execute both Agents headlessly

The local model makes three generate requests: request the Tool in `work`,
complete `work`, then complete `review`. The Loop omits outcomes, so the implicit
`complete` transition must work. In `review`, `access.tools: false` hides the Tool
even though its Skill remains globally bound. No Skill script runs automatically.

```bash
cd "$M6B_ROOT/cases/standalone"
cat agentpm.harness.json
cat .agentpm/tools/fixture/echo/1.0.0/main.cjs
cat .agentpm/tools/fixture/echo/2.0.0/main.cjs
m6b_expect headless-1 0 m6b harness consumer-1 --headless --input 'Check exact versions.' --report "$M6B_ROOT/evidence/headless-1.report.json"
m6b_expect headless-2 0 m6b harness consumer-2 --headless --input 'Check exact versions.' --report "$M6B_ROOT/evidence/headless-2.report.json"
cat tool-runs.jsonl
cat "$M6B_ROOT/evidence/headless-2.report.json"
```

Expected: both Runs end with `{"text":"PASS M6B fixture"}`. The actual Tool
markers are **1.0.0 then 2.0.0**, once each. The helper checks all six exact
Component versions in every model request, not just the final output.
`model-requests.jsonl` contains those raw requests: inspect `runtime` and
`effective_phase`. The latter has one active Tool in `work` and none in `review`.
The external Memory operation `prune` is bound, but not invoked; its target is
not directly exposed as an Agent Memory space.

## 4. Execute the same graphs through both SDKs

This case uses the same authored graphs, with the model configured as **host**
instead of process. Each SDK registers the deterministic callback, initializes
the real CLI machine session, reads preflight and supported standards, performs
one Run and shuts down. A standalone invocation of this host-only case would
need its host registration; use the SDK commands below.

```bash
cd "$M6B_ROOT/cases/sdk-host"
cat agentpm.harness.json
cat "$M6B_ROOT/runtime/sdk_node.mjs"
cat "$M6B_ROOT/runtime/sdk_python.py"
m6b_expect node-1 0 m6b_node run consumer-1
m6b_expect node-2 0 m6b_node run consumer-2
m6b_expect python-1 0 m6b_python run consumer-1
m6b_expect python-2 0 m6b_python run consumer-2
cat tool-runs.jsonl
```

Expected Tool markers: **1, 2, 1, 2** (as full `1.0.0` / `2.0.0` versions).
Each SDK result saves `initialized`, the existing **`preflight.report` envelope**,
typed `supported`, the ended Run/report, and three raw model requests. Protocol
version remains 1. All exact versions and review-phase Tool suppression must
match the standalone behavior. The SDK commands' exit 0 means the probe passed;
their saved JSON exposes the actual underlying report and Run status.

## 5. Phase scope and Memory operation authority

**Input:** `scoped` selects Agent 1's graph, but binds its Skill **only in work**
and Memory `prune` **only in review**. There is no direct Tool edge/binding.
Review allows Tools by default: the Tool's absence now follows from Skill scope,
independently of the Tool prohibition tested above. Review explicitly prohibits
direct Memory reads/writes. The separately bound operation still participates
on its declared unbound target; participation does not automatically invoke it.

```bash
cd "$M6B_ROOT/cases/scoped"
cat agents/agent-1.agent.json
cat .agentpm/loops/fixture/workflow/1.0.0/agent.json
cat .agentpm/memory/fixture/memory/1.0.0/agent.json
m6b_expect scoped-1 0 m6b harness consumer-1 --headless --input 'Check exact versions.' --report "$M6B_ROOT/evidence/scoped-1.report.json"
cat tool-runs.jsonl
```

Expected: Tool 1.0.0 executes once in work; review has no active Skill or Tool.
Work has no active Memory operation; review has `prune` at 1.0.0. Neither phase
has a directly exposed Memory space, and the operation is not automatically
called. This is inspection of participation and scope, not an external-delete
authorization test. The latter and stateful trigger behavior remain covered by
the milestone's automated Engine regressions.

## 6. Ambiguity is a Harness limitation, not invalid APDS

**Defect:** Agent 1 directly selects Tool 1.0.0, but its Skill selects Tool 2.0.0.
Both are active under the same versionless binding identity `@fixture/echo`.
The definitions are valid; this Harness cannot choose between those versions.

```bash
cd "$M6B_ROOT/cases/ambiguous"
cat agents/agent-1.agent.json
cat .agentpm/skills/fixture/skill/1.0.0/agent.json
m6b_expect ambiguous-json 1 m6b harness consumer-1 --json
m6b_expect ambiguous-headless 1 m6b harness consumer-1 --headless --input 'Must not execute.'
m6b_expect ambiguous-node 0 m6b_node blocked consumer-1
m6b_expect ambiguous-python 0 m6b_python blocked consumer-1
```

Expected: **APDS `conformant` / Harness `limited` / local `not_ready`**, with
fatal `ambiguous_locked_component` naming both versions. Headless exits 1.
SDK initialization, preflight and shutdown still work; Run start returns
**`preflight_not_ready`**. The probe exits 0 because that specific rejection is
the expected result. No model requests or Tool executions occur.

## 7. Inspect the Skill-only workaround

**Input:** the Skill still selects Tool 2.0.0, but this prepared variant removes
Agent 1's direct Tool declaration and binding. Only one Tool version is reachable
in its selected graph. This illustrates the workaround we discussed; normal
authors would edit their manifest and regenerate installed state with the CLI.

```bash
cd "$M6B_ROOT/cases/skill-only"
cat agents/agent-1.agent.json
cat .agentpm/skills/fixture/skill/1.0.0/agent.json
cat agent.lock
m6b_expect skill-only-1 0 m6b harness consumer-1 --headless --input 'Check exact versions.' --report "$M6B_ROOT/evidence/skill-only-1.report.json"
cat tool-runs.jsonl
```

Expected: one Tool execution at **2.0.0**, inherited through Skill **1.0.0**.
Knowledge, Memory, Profile and Loop remain 1.0.0. The helper checks this mixed
version expectation explicitly; it does not assume every Component must share
one version number. Review still suppresses Tools through Loop policy.

## 8. Compare CLI and both SDKs for the same five reports

These small report-only cases use no real model endpoint. Do not try an actual
Run for `ready` or `legacy`: configured local readiness is all we inspect here.

| Case            | Intentional condition                                 | APDS / Harness / local result             |
| --------------- | ----------------------------------------------------- | ----------------------------------------- |
| `ready`         | Supported definition and configured model             | conformant / supported / ready            |
| `no-loop`       | Valid Agent with no Loop                              | conformant / not_runnable / not_evaluated |
| `missing-model` | No model selection                                    | conformant / supported / not_ready        |
| `legacy`        | Missing standard declaration, qualified compatibility | legacy_inferred / supported / ready       |
| `unsupported`   | Explicit agentpm / 9.0.0                              | unsupported / unsupported / not_evaluated |

```bash
for m6b_case in ready no-loop missing-model legacy unsupported; do
  cd "$M6B_ROOT/cases/$m6b_case" || break
  m6b_fixture describe "$m6b_case" || break
  cat agent.json
  cat agentpm.harness.json
  if [[ "$m6b_case" == ready || "$m6b_case" == legacy ]]; then
    m6b_exit=0
    m6b_mode=inspect
  else
    m6b_exit=1
    m6b_mode=blocked
  fi
  m6b_expect "report-$m6b_case" "$m6b_exit" m6b harness --json || break
  m6b_expect "node-$m6b_case" 0 m6b_node "$m6b_mode" '' || break
  m6b_expect "python-$m6b_case" 0 m6b_python "$m6b_mode" '' || break
done
```

Expected: exact report equality between standalone CLI, SDK initialization and
the SDK preflight envelope, including diagnostics and supported standards.
Both SDK helper methods return `[{"id":"agentpm","version":"1.0.0"}]`.
For each blocked case both SDKs also verify Run start is refused, then shut down
normally. Legacy never becomes verified conformance; no-Loop/model readiness
failures never become APDS-invalid results.

## 9. Reproduce the description experiment

This invokes only the existing focused Rust experiment. The output path is
**inside this ignored run**, not a new tracked crate-local `target` directory.

```bash
cd "$M6B_ROOT/cases/standalone"
m6b_expect description-ab 0 m6b_description
cat "$M6B_ROOT/evidence/description-ab.json"
```

Expected: four rows, baseline/injection for one and three phases, with equal
ended outputs and larger prompts after injection. Provider tokens/cost are
`null`, paid requests are zero. The scripted model cannot establish real LLM
quality or fidelity. The recorded decision remains **defer injection; keep
current prompts**. There is no APDS or production prompt change.

## 10. Observe both Agents in the actual TUI

Run these **directly in the terminal, without `tee` or capture**. The launcher
preserves terminal colors. Start each observation immediately before its TUI;
send exactly one message, wait for the Run to end, then quit before confirming.

```bash
cd "$M6B_ROOT/cases/standalone"
m6b_fixture begin-tui consumer-1
m6b harness consumer-1
```

Check the workspace panel: **APDS definition Conformant / Harness support
Supported / Local runtime Ready**, selected Agent **consumer-1**. The Run starts
with Composer focus: type `Check exact versions.` and press Enter. Wait for
Ended and `PASS M6B fixture`. Press Esc to leave the Composer; `O` opens the full
output viewer, Esc closes it. With panel focus, `q` quits. If needed, Esc then
`1` focuses workspace; PgUp/PgDn (Mac Fn-Up/Fn-Down) page the workspace rows.

Only after observing that result:

```bash
m6b_fixture confirm-tui consumer-1
m6b_fixture begin-tui consumer-2
m6b harness consumer-2
```

Repeat with **consumer-2**, the same three readiness rows and one completed Run.
Quit, then record the observation:

```bash
m6b_fixture confirm-tui consumer-2
cat tool-runs.jsonl
```

Expected Tool markers in this case are now **1, 2, 1, 2**: two earlier headless
Runs plus two TUI Runs. Each confirmation requires a fresh ended report, three
new model requests with the selected exact versions, and one matching actual
Tool execution. Merely opening/quitting the TUI cannot pass. These confirmations
record your visual observation; they do not replace you checking the display.

## 11. Audit and finish

```bash
cd "$M6B_ROOT"
m6b_fixture audit
cat evidence/summary.json
```

Expected: **`status: pass`, `checked_command_results: 30`,
`tui_observations_recorded: true`**. The helper rechecks saved results, raw
requests, diagnostics, outputs and source hashes. `operator_signoff: false` and
`hosted_ci_complete: false` are intentional: it does not grant those approvals
for you. Send the summary and your observations back; hosted CI confirmation on
matching revisions is still required before marking M6B complete.

Keep this run directory until acceptance is recorded. There is no server to
stop, production state to restore or cleanup command to run.
