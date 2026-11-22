# M6A: Harness Definition, Support and Local Readiness

Claude review passed with no remaining findings. The operator confirmed all
hosted CI passed on **2026-11-21**. The user supplied the passing 26-result audit
with four TUI observations the same day, completing M6A. The earlier builder
recovery is disclosed in [verification](../m6a-verification.md).

Use **one macOS/zsh terminal**, starting in the CLI checkout. Everything you
inspect and generate stays under gitignored **`agentpm/cli-m6a-test/<run>/`**.
Run one block at a time and open the same directory in your editor. No `/tmp`
navigation or second API terminal is needed.

This milestone checks the **local Harness**, so there is **no Registry, Docker
DB, S3, publication, signing or paid model service** in this run. The CLI uses
an explicit non-listening loopback Registry address. The wrapper removes OpenAI
and Anthropic keys **only in its child environment**, leaving your shell intact.
Python 3.11+ and the reviewed debug CLI are the only runtime prerequisites.

The sources start from real CLI scaffolds, are linted, and Knowledge/Memory are
built by the CLI. The helper then creates **synthetic installed-state fixtures**:
visible `.agentpm` folders and `agent.lock` files with clearly labeled placeholder
integrities. This lets unsupported, missing and inconsistent states exist
locally without publishing bad packages. These are not real installed releases
or integrity/signature evidence. Do not use manual lock/install edits in normal
consumer apps; real installation was verified in M5A/M5B.

The local model fixture speaks the actual process service protocol, requests
implicit phase completion and returns `{"text":"PASS M6A fixture"}`. It has no
HTTP or LLM client. It proves preflight, service startup and Engine control flow,
not model quality or full APDS execution certification. Exact-version runtime
binding fixes and typed SDK support exposure remain M6B.

| Section | What you inspect and prove                                                 |
| ------- | -------------------------------------------------------------------------- |
| 1       | One owned run, real scaffold/build/lint preparation and visible inputs     |
| 2       | Valid no-Loop definition versus Harness not runnable                       |
| 3       | Configured model versus absent selection or required credentials           |
| 4       | Unsupported Agent, Loop and Skill-inherited Tool standards                 |
| 5       | Actual structural, relationship, context and provenance defects            |
| 6       | Legacy inference with visible nonfatal APDS warnings                       |
| 7       | Valid Memory/Knowledge definitions with Runner/backend limitations         |
| 8       | Actual local headless completion, handoff, abort and approval stop         |
| 9       | Existing machine protocol: inspect, run or reject, then shut down          |
| 10      | Captured non-TTY invocation cannot assume interactive bootstrap            |
| 11      | Actual TUI rows, model selection and a completed local Run                 |
| 12      | Recheck all 26 command results, unchanged inputs and four TUI observations |

`m6a_expect` saves both streams and prints **ACTUAL exit / EXPECTED exit**. It
also checks the actual definition/support/readiness, expected diagnostic
rule/path/severity, unchanged input bytes and local model traffic. An unrelated
failure is not a passing negative case. Stop on a mismatch; inspect the output.

## 1. Build and prepare once

Start at the **CLI repository root**. Adjust only the first `cd` if your checkout
is elsewhere. Keep this terminal open and do not rebuild midway through a run.

```bash
cd /Users/zackhine/projects/agentpm-project/agentpm
cargo build --locked -p agentpm-cli
export M6A_REPO="$PWD"
export M6A_APM="$M6A_REPO/target/debug/agentpm"
export M6A_PYTHON="$(command -v python3)"
export M6A_FIXTURE="$M6A_REPO/specs/2026-10-09-category-hardening/manual-tests/fixtures/m6a/fixture.py"
export M6A_ROOT="$M6A_REPO/cli-m6a-test/$(date -u +%Y%m%dT%H%M%SZ)-$$"
umask 077
"$M6A_PYTHON" --version
"$M6A_PYTHON" -B "$M6A_FIXTURE" prepare "$M6A_ROOT" --cli "$M6A_APM"
source "$M6A_ROOT/paths.sh"
cd "$M6A_ROOT"
pwd
git -C "$M6A_REPO" check-ignore "$M6A_ROOT/session.json"
cat session.json
ls cases
cat runtime/model_service.py
```

Expected: **21 visible cases and 26 command expectations**, CLI 0.1.47, and a
printed ignored session path. Preparation ran real `init`, `lint`, Knowledge
build and Memory build commands. Their actual outputs are in
`evidence/preparation.json`, including expected strict-lint failures for the
deliberately invalid/undeclared inputs. Preparation did not run an Agent.

`templates/` contains the scaffold baselines; `cases/` contains the test inputs;
`runtime/model_service.py` is the local model; `protocol/requests.jsonl` is the
visible machine request sequence; `expectations.json` lists exact commands and
exits. `session.json` pins the binary/helper and source hashes.

If the terminal is closed, open **your operator timestamp directory**, then run
`source ./paths.sh`. Do not source a different run or an automated `builder-*`
directory. If setup is repeated, choose a new `M6A_ROOT`; existing runs are never
overwritten. No credentials are stored in `paths.sh`.

## 2. Valid definition without a Loop

**Input:** the Agent has no Loop declaration or locked Loop. This is not an
invalid manifest. Open the manifest and lock before running preflight:

```bash
cd "$M6A_ROOT/cases/no-loop"
m6a_fixture describe no-loop
cat agent.json
cat agent.lock
m6a_expect no-loop-json 1 m6a harness --json
```

Expected: **APDS `conformant`**, Harness **`not_runnable`**, local runtime
**`not_evaluated`**, and fatal `agent_missing_loop`. The explanation says a Loop
is required **by Harness, not by APDS**. `runner_evaluated` is false. The nonzero
exit means this Harness cannot start it; it does not change the definition result.

## 3. Configured runtime versus missing model/provider data

**Ready input:** the same kind of valid Agent now selects a two-phase Loop and
the configured local process model. JSON preflight starts no model process.

```bash
cd "$M6A_ROOT/cases/ready"
m6a_fixture describe ready
cat agent.json
cat .agentpm/loops/manual/workflow/1.0.0/agent.json
cat agentpm.harness.json
m6a_expect ready-json 0 m6a harness --json
```

Expected: **`conformant / supported / ready`**, checked levels standalone and
resolved, and support list exactly `agentpm / 1.0.0`. Ready means the static local
checks passed; it is not a successful remote-provider probe.

**Missing selection:** the process implementation is configured, but `model` is
absent. JSON and headless preflight must both fail without prompting or calling
the model. The definition remains conformant.

```bash
cd "$M6A_ROOT/cases/missing-model"
m6a_fixture describe missing-model
cat agentpm.harness.json
m6a_expect missing-model-json 1 m6a harness --json
m6a_expect missing-model-headless 1 m6a harness --headless --input 'M6A local smoke'
```

Expected: **`conformant / supported / not_ready`**, fatal
`model_not_configured`, actionable `model.provider` / `model.model` guidance.
Headless stdout is empty; diagnostics go to stderr. No process-model requests.

**Missing credentials:** OpenAI is selected, but the wrapper deliberately removes
`OPENAI_API_KEY` from its child environment. Do not enter a real key.

```bash
cd "$M6A_ROOT/cases/missing-key"
m6a_fixture describe missing-key
cat agentpm.harness.json
m6a_expect missing-key-json 1 m6a harness --json
```

Expected: definition conformant, Harness supported, runtime not ready;
`model_provider_not_ready` names the required key without printing its value.

## 4. Unsupported standard on the root and resolved Components

Each case explicitly declares **agentpm / 9.0.0** in a different place. The
selected graph must not fall back to 1.0.0 or parse an unknown execution contract.

```bash
cd "$M6A_ROOT/cases/unsupported-agent"
m6a_fixture describe unsupported-agent
cat agent.json
m6a_expect unsupported-agent-json 1 m6a harness --json

cd "$M6A_ROOT/cases/unsupported-loop"
m6a_fixture describe unsupported-loop
cat .agentpm/loops/manual/workflow/1.0.0/agent.json
m6a_expect unsupported-loop-json 1 m6a harness --json

cd "$M6A_ROOT/cases/unsupported-tool"
m6a_fixture describe unsupported-tool
cat agent.json
cat .agentpm/skills/manual/skill/1.0.0/agent.json
cat .agentpm/tools/manual/future/1.0.0/agent.json
m6a_expect unsupported-tool-json 1 m6a harness --json
```

Expected each time: APDS **`unsupported`**, Harness **`unsupported`**, local
readiness unevaluated; fatal `unsupported_standard` identifies the offending
relative file, 9.0.0 and supported 1.0.0. The inherited Tool deliberately uses a
future shape: rejection must happen before applying today's Tool parser.

## 5. Separate real definition defects from unavailable context

Run one case at a time. `describe` states the defect and precise expectation;
`cat` shows the actual root input before the CLI checks it.

| Case                  | Deliberate issue                                                 | Expected diagnostic                             |
| --------------------- | ---------------------------------------------------------------- | ----------------------------------------------- |
| `malformed-standard`  | `standard: null` is an invalid explicit declaration              | `apds_invalid_standard`; never inferred legacy  |
| `invalid-description` | Declared Agent description is whitespace                         | fatal `APDS-ID-002`, `agent.json#/description`  |
| `invalid-phase`       | Binding phase `ghost` is absent from the selected Loop           | fatal `APDS-BIND-002`, `/bindings/phases/ghost` |
| `missing-loop`        | Lock selects a Loop whose installed manifest is absent           | `apds_context_unavailable`; APDS **incomplete** |
| `provenance`          | Explicit declaration contradicts lock's `legacy_inferred` origin | `apds_provenance_mismatch`                      |

```bash
for m6a_case in malformed-standard invalid-description invalid-phase missing-loop provenance; do
  cd "$M6A_ROOT/cases/$m6a_case"
  m6a_fixture describe "$m6a_case" || break
  cat agent.json
  cat agent.lock
  m6a_expect "$m6a_case-json" 1 m6a harness --json || break
done
```

Expected: local readiness **not evaluated**. Four definitions/graphs are
nonconformant; unavailable installed context is **incomplete**, not a success or
proof the missing artifact itself is invalid. No model process starts. A source
path, rule and severity must match the table, not merely a nonzero exit.

## 6. Legacy qualification must not hide defects

These cases remove a declaration and truthfully mark the corresponding lock
origin `legacy_inferred`. They are not republished or silently migrated.

```bash
for m6a_case in legacy legacy-description legacy-loop-description legacy-phase; do
  cd "$M6A_ROOT/cases/$m6a_case"
  m6a_fixture describe "$m6a_case" || break
  cat agent.json
  if [[ "$m6a_case" == legacy-loop-description ]]; then
    cat .agentpm/loops/manual/workflow/1.0.0/agent.json
  fi
  m6a_expect "$m6a_case-json" 0 m6a harness --json || break
done
```

Expected in all four: definition **`legacy_inferred`**, not conformant; no fatal
APDS finding; unchanged sources. The missing declaration appears as unevaluated
`APDS-ID-001`, with a qualification note, rather than an error for its absence.

- `legacy`: missing declaration only; otherwise ready.
- `legacy-description`: warning `legacy_apds_issue` includes **APDS-ID-002** at
  `agent.json#/description`; `ready_with_warnings`.
- `legacy-loop-description`: the same visible warning on the legacy **Loop**,
  while the Agent still explicitly declares its standard.
- `legacy-phase`: warning includes **APDS-BIND-002** at
  `agent.json#/bindings/phases/ghost`; the resolved finding remains visible.

The three defect cases remain runnable under historical compatibility; their
warnings cannot be used as conformance certification. Existing Harness-specific
warnings may also describe the bad phase. Declared counterparts in section 5
fail; the legacy exception does not make them pass strict authoring lint.

## 7. Backend limitations are not invalid APDS

**Memory:** the CLI-built Blueprint binds a space with semantic-only retrieval
that the selected local SQLite capability descriptor cannot realize.

```bash
cd "$M6A_ROOT/cases/memory-limited"
m6a_fixture describe memory-limited
cat agent.json
cat .agentpm/memory/manual/memory/1.0.0/agent.json
m6a_expect memory-limited-json 0 m6a harness --json
```

Expected: **`conformant / limited / limited`**, suppressed
`unrealizable_memory_space`. The contract is valid; that capability is unavailable
to this local runtime. Optional suppression does not fail the whole preflight.

**Knowledge:** built context content is mapped to a host implementation, which
cannot run on this standalone surface.

```bash
cd "$M6A_ROOT/cases/knowledge-limited"
m6a_fixture describe knowledge-limited
cat agentpm.harness.json
cat .agentpm/knowledge/manual/knowledge/1.0.0/agent.json
m6a_expect knowledge-limited-json 0 m6a harness --json
```

Expected: definition conformant; Harness/local readiness limited, with
`host_implementation_unavailable` and backend diagnostics. No false
`apds_nonconformant` claim and no backend or model process starts.

## 8. Run the local model and preserve Loop stops/approval

These commands really execute the local process model. Inspect its two phase
requests and report afterward. Explicit report paths are outside the source
fixtures but remain inside this run's visible `evidence/` directory.

```bash
cd "$M6A_ROOT/cases/ready"
m6a_expect ready-headless 0 m6a harness --headless --input 'M6A local smoke' --report "$M6A_ROOT/evidence/ready-headless.report.json"
cat runtime-requests.jsonl
cat "$M6A_ROOT/evidence/ready-headless.report.json"
```

Expected stdout: `{"text":"PASS M6A fixture"}`. Report: **`ended`**, two completed
phases, implicit outcome **`complete`** on each, final transition **`$end`**.
This is real Engine/process-protocol execution with a deterministic fixture.

```bash
cd "$M6A_ROOT/cases/handoff"
cat .agentpm/loops/manual/workflow/1.0.0/agent.json
m6a_expect handoff-headless 0 m6a harness --headless --input 'M6A local smoke' --report "$M6A_ROOT/evidence/handoff-headless.report.json"

cd "$M6A_ROOT/cases/abort"
cat .agentpm/loops/manual/workflow/1.0.0/agent.json
m6a_expect abort-headless 1 m6a harness --headless --input 'M6A local smoke' --report "$M6A_ROOT/evidence/abort-headless.report.json"

cd "$M6A_ROOT/cases/approval"
cat .agentpm/loops/manual/workflow/1.0.0/agent.json
m6a_expect approval-headless 1 m6a harness --headless --input 'M6A local smoke' --report "$M6A_ROOT/evidence/approval-headless.report.json"
```

Expected terminal statuses: **`handed_off`**, **`aborted`**, **`approval_required`**.
Handoff exits 0 and prints the fixture output; abort exits 1 with empty stdout.
Approval stops before the first phase: **zero generate requests**, no automatic
approval and no phase execution. The helper reads the actual report and process
traffic to distinguish these from unrelated failures.

## 9. Machine initialization/preflight and run gating

Read the request sequence first:

```bash
cat "$M6A_ROOT/protocol/requests.jsonl"
cd "$M6A_ROOT/cases/ready"
m6a_expect ready-machine 0 m6a_machine

cd "$M6A_ROOT/cases/missing-model"
m6a_expect missing-model-machine 0 m6a_machine
```

`m6a_machine` is a small transport client, not a validator. It launches the real
CLI with `harness --machine`, forwards every stdout frame, and sends each visible
request only after the preceding response. Do not pipe all four requests at
once: `shutdown` arriving during an active Run is correctly rejected.

Expected:

- Every stdout line is an `agentpm-harness-machine` frame, protocol version **1**.
- `initialize.payload.preflight.supported_standards` and
  `preflight.payload.report.supported_standards` both advertise **agentpm / 1.0.0**.
  APDS version and transport protocol version are different dimensions.
- Ready case: `start_run` returns **ended**, the fixture output and a report.
- Missing-model case: initialization and inspection still succeed, but
  `start_run` returns **`preflight_not_ready`**, with no model process start.
- Both Sessions shut down normally and exit 0. That exit acknowledges a clean
  protocol Session; it does not turn the failed preflight into a ready result.

## 10. No terminal means no assumed TUI bootstrap

This intentionally captures the default TUI invocation to files, making its
stdout/stderr non-TTY. It must fail rather than assume an interactive prompt.

```bash
cd "$M6A_ROOT/cases/missing-model"
m6a_expect no-terminal 1 m6a harness
```

Expected: interactive-terminal guidance and no model requests. This is distinct
from the headless missing-model diagnostic in section 3.

## 11. Inspect the real TUI and its model-selection flow

Use a real terminal window. **Do not wrap these four `m6a harness` commands in
`m6a_expect`, redirect them or pipe them to `tee`.** Capture would remove their
terminal and test section 10 again. The `begin-tui` / `confirm-tui` commands
instead record your observations and check source hashes and newly written Run
reports. Confirm only after observing the listed behavior.

**Ready:** three separate rows, no dense combined status paragraph.

```bash
cd "$M6A_ROOT/cases/ready"
m6a_fixture begin-tui ready "$PWD"
m6a harness
```

In the TUI:

1. Press **Esc once** to leave the composer, then **1** to inspect Workspace.
   Expect **APDS definition: Conformant**,
   **Harness support: Supported**, **Local runtime: Ready**, with a short
   supported-standard summary. Use PageUp/PageDown if the rail paginates.
2. Press **2**, then **Enter** to focus the composer. Type `M6A interactive
smoke` and press Enter. Wait for an **ended** Run.
3. Press Esc to leave the composer, then **O** to open the full output viewer.
   Inspect **PASS M6A fixture** (scroll if necessary). Esc closes the viewer;
   **q** exits from the Run panel. Wait for completion before quitting.

```bash
m6a_fixture confirm-tui ready
```

**Bootstrap:** no selected model in the file, but an actual TTY can supply one.

```bash
cd "$M6A_ROOT/cases/missing-model"
cat agentpm.harness.json
m6a_fixture begin-tui bootstrap "$PWD"
m6a harness
```

Expect **Local runtime: Needs model**. Press **Esc once**, then **1**. Use
**PageUp** to reach the last Workspace page containing **Resolve**, then press
**P**. The model input appears there, below the readiness categories. Remove the
prefilled `openai/` with Backspace and enter **`manual-model/fixture`**. Press
Enter. Expect readiness to update. The refreshed screen starts in the composer;
press **Esc once**, then **2**, Enter, submit `M6A interactive
smoke`, wait for an ended Run, leave the composer with Esc and inspect the full
output with O. Close the viewer with Esc, then quit with q.
This selects the already-configured local process provider, not a paid service.

```bash
m6a_fixture confirm-tui bootstrap
cat agentpm.harness.json
```

Expected: one new successful Run; the source config still has **no `model`**.
Interactive selection is an override, not a silent source rewrite.

**Legacy warning:** view only; do not submit a Run.

```bash
cd "$M6A_ROOT/cases/legacy-description"
m6a_fixture begin-tui legacy "$PWD"
m6a harness
```

Press **Esc once**, then **1**. Expect warning-colored **Legacy** for APDS, separate Harness/runtime
rows, and a visible legacy warning in diagnostics. Press **D** while Workspace is
focused to expand it: **APDS-ID-002** and `/description` must be visible. The
definition must not be labeled conformant. Quit from the panel with q.

```bash
m6a_fixture confirm-tui legacy
```

**No Loop:** view only; no attempt to start a Run.

```bash
cd "$M6A_ROOT/cases/no-loop"
m6a_fixture begin-tui no-loop "$PWD"
m6a harness
```

Press **1**. Expect **Conformant**, **Not runnable**, **Not checked** on the three
rows. Expand diagnostics with D and check that the Loop requirement belongs to
Harness, not APDS. Quit with q.

```bash
m6a_fixture confirm-tui no-loop
```

## 12. Audit saved evidence and report acceptance

```bash
cd "$M6A_ROOT"
m6a_fixture audit
cat evidence/summary.json
```

Expected: **status pass**, **26 checked command results**, and TUI observations
recorded. The audit re-parses actual stdout, stderr, reports and captured model
traffic and checks original source hashes. It rejects missing results and wrong
diagnostics; it does not just count matching exits.

`operator_signoff: false` and `hosted_ci_complete: false` are intentional: the
helper does not certify external CI or grant final acceptance. CI is already
operator-confirmed in tasks; send this final summary and any TUI concerns back
to Codex to record your acceptance. Do not manually change the flags.

Keep the ignored run until the milestone is closed. Your normal projects,
published examples, CLI configuration and Registry are not changed. If anything
unexpected happens, send the case name plus that label's `.stdout.txt`,
`.stderr.txt` and `.result.json` from `evidence/`.
