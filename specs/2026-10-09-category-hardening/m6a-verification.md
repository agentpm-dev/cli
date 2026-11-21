# M6A: Harness standard support and preflight readiness

Builder verification: **2026-11-21**. Scope: M6A only; **D20 / S2-AC-08**.
User confirmed independent Claude review passed with no remaining findings on
**2026-11-21**, after the legacy-diagnostic and relative-path corrections.
Hosted CI and operator manual acceptance remain pending.
CLI **0.1.47 remains unreleased**. No production requests or deployment occurred.

## Implementation and source contracts

| Source                                                                  | Change and purpose                                                                                                                                                                                                        |
| ----------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `crates/agentpm-cli/src/harness_plan/standard.rs`                       | Advertise exact supported standards; inspect selected Agent/Component declarations, provenance, identities and locked relationships; reuse offline APDS evaluation.                                                       |
| `crates/agentpm-cli/src/harness_plan.rs`                                | Select a no-Loop Agent for inspection; keep definition/support/readiness separate; check model configuration, provider credentials and execution surface. Unselected manifests do not decide the selected graph's result. |
| `crates/agentpm-cli/src/manifest/apds/graph.rs`                         | Accept caller-owned in-memory interpretation documents through an internal, nonserialized field. Published graph JSON and fixture behavior are unchanged.                                                                 |
| `crates/agentpm-cli/src/harness_plan/backend.rs`                        | Reuse the existing Knowledge readiness functions for both preflight and execution snapshots. Report backend gaps without changing definition results.                                                                     |
| `crates/agentpm-cli/src/commands/harness/runtime_snapshot.rs`           | Import those shared readiness functions; their existing runtime interpretation is preserved.                                                                                                                              |
| `crates/agentpm-cli/src/commands/harness.rs`                            | Print the same capability/status summary in headless diagnostics; permit machine initialization and inspection after failed preflight while retaining the `start_run` readiness guard.                                    |
| `crates/agentpm-cli/src/commands/harness/tui/{state,render}.rs`         | Display the same facts in workspace readiness. Only the terminal-gated TUI bootstrap enables interactive model selection. Failed definition checks do not construct an execution engine.                                  |
| `crates/agentpm-cli/tests/harness_standard.rs`                          | Seventeen actual-binary integration tests, covering positive, negative, provenance, context, legacy, headless and machine cases without Registry/model API calls.                                                         |
| `crates/agentpm-cli/tests/authoring.rs`                                 | Deliberately update the prior no-Loop wording assertion and check the three structured statuses.                                                                                                                          |
| `.github/workflows/ci.yml`                                              | Include the integration suite in native-target jobs as well as the existing full workspace job.                                                                                                                           |
| `README.md`, `CHANGELOG.md`, `../agentpm-api/docs/v0.1/cli/harness.mdx` | Feature-local workflow, compatibility and result documentation; no docs route changes.                                                                                                                                    |

## D20 interface choice

**M6A choice approved in Claude review, as confirmed by the user on 2026-11-21:** extend the existing preflight report;
do not add a new Runner protocol or change its version.

```json
{
  "supported_standards": [{ "id": "agentpm", "version": "1.0.0" }],
  "apds": {
    "status": "conformant",
    "checked_levels": ["standalone", "resolved"],
    "artifacts": [],
    "runner_evaluated": false
  },
  "harness_support": "not_runnable",
  "local_readiness": "not_evaluated"
}
```

This abbreviated example omits existing report fields and artifact rows. Each
artifact row includes `artifact_id`, `file`, `standard`, `standard_origin`,
`status`, `evaluated_rules` and `unevaluated`. A resolved root's rule coverage
includes the selected closure; these are implementation reporting fields, not
new APDS requirements. `artifact_id` avoids the existing machine redactor's
secret-key field convention; redaction policy is unchanged.

The definition status vocabulary is `not_evaluated`, `conformant`,
`legacy_inferred`, `nonconformant`, `incomplete`, `unsupported`. It describes the
checks actually performed, not packaged-content or execution certification.
Historical interpretation never receives a retroactive APDS-conformant result:
existing Harness parser/readiness behavior remains applicable, and inference is
shown explicitly. Declared manifests receive strict standalone checks on their
authored values before dependency-selector normalization can hide an error.

**Review correction — legacy diagnostics:** the initial builder suppressed all
APDS findings for legacy documents. That was too broad. The only diagnostic
exception is now `APDS-ID-001` at `/standard` for a missing historical
declaration; the artifact records that rule as unevaluated. Other structural
and resolved findings are emitted as **warning** `legacy_apds_issue`, with the
rule ID and original document/source path. They do not turn the qualified
definition status into `nonconformant` or make these APDS warnings fatal.
Declared counterparts remain fatal. Existing provenance/context/runtime guards
are unchanged. This follows the Registry's narrow missing-declaration exception
for diagnostic visibility; Harness keeps historical execution compatibility by
warning, while Registry publication applies its own stricter admission policy.

New actual-CLI tests compare whitespace descriptions in legacy versus declared
root and Component manifests and verify a legacy nonexistent-phase binding's
resolved `APDS-BIND-002` warning. They assert exact severity/path, no fatal
legacy APDS diagnostics, a single deduplicated finding and unchanged source/lock
bytes. Four earlier planning tests now deliberately expect
`ready_with_warnings` from their nonconforming historical fixtures.

**Review correction — selector diagnostic paths:** unsupported/malformed standard
messages now reuse the same workspace-relative path as their `path` field and
artifact row. The existing root/Component unsupported-standard test asserts the
message prefix agrees with that relative field and contains no absolute workspace
prefix. All **17 CLI integration tests** pass on this message-only follow-up
(`/tmp/m6a-path-cli.log`), as do Clippy (`/tmp/m6a-path-clippy.log`) and formatting.
The full-workspace count below is the preceding semantic-correction run; it was
not repeated for this cosmetic change.

Harness support is `not_evaluated`, `supported`, `limited`, `unsupported` or
`not_runnable`. Local readiness is `not_evaluated`, `ready`, `limited`,
`not_ready` or `bootstrap_required`. Existing optional capability suppression
remains usable: `limited` does not make the artifact invalid or introduce an
unconditional whole-Run failure for an unavailable optional backend.

The same report is exposed by:

- CLI `harness --json`, with nonzero exit when preflight fails;
- headless human diagnostics on stderr, leaving stdout for final output;
- the TUI workspace readiness view;
- machine preflight events, `initialize.payload.preflight` and
  `preflight.payload.report`.

Machine Session identity stays **`agentpm-harness-machine`, version `1`**. APDS
version `1.0.0` is a separate capability dimension. Failed machine preflight
allows initialize/inspect/shutdown, but `start_run` returns
`preflight_not_ready`. Existing report status enums and frame fields remain.
Deserializing an older report without these fields produces empty/unevaluated
capabilities, not fabricated support.

Node `HarnessPreflightResult` already allows additional JSON fields and Python
returns the report dictionaries unchanged. M6A therefore makes the data available
through their existing machine bridge. Typed/convenience exposure and independent
cross-SDK parity remain **M6B**; no SDK source or shared fixture was changed.

## Behavior and acceptance mapping

| Requirement         | Builder evidence                                                                                                                                                                                                                                                       |
| ------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| T-RUN-01 / D20      | Actual CLI JSON, headless stderr and machine report equality; unchanged Session protocol identity; TUI workspace render assertion; older-report default test.                                                                                                          |
| T-RUN-02            | Actual `init → lint → install → harness --json` smoke: definition conformant, Harness not runnable, local readiness unevaluated; both default and explicit Agent selectors tested.                                                                                     |
| T-RUN-03            | Unknown root, Loop and Skill-inherited Tool standards reject before runtime shape parsing; unsupported lock metadata gets an explicit compatibility result. No fallback interpretation.                                                                                |
| T-RUN-04            | Noninteractive JSON/headless/machine missing model fails; absent OpenAI/Anthropic credentials are runtime configuration failures. Interactive TUI retains bootstrap and interactive model-source handling; other surfaces cannot enable that path.                     |
| Definition boundary | Blank descriptions, invalid phase bindings, malformed selectors/JSON, changed identities, malformed dependency objects, absent lock/installed context and provenance mismatches fail explicitly. Preflight preserves source/lock bytes.                                |
| Backend boundary    | Valid declared Memory with unsupported local semantic retrieval and Knowledge mapped to an unavailable host remain definition-conformant and report Runner/runtime limitations. Existing Knowledge content/vector readiness logic is shared rather than reimplemented. |
| T-RUN-07            | Full workspace regressions cover Loop outcomes/stops, approval/checkpoint/retry behavior, events/reports, Hooks/MCP, Memory operations and providers. Engine inner-loop, phase-output and handoff implementations are untouched.                                       |

Existing execution tests include multi-phase accumulation, phase reentry,
pending-approval Run guards, authored phase Tool scoping, internal Memory
operation outputs, count/capacity rearming, interval dormancy and persisted
baselines. M6B still owns the complete Runner-observation/parity work and D13
experiment; no new full-execution APDS certification claim is made here.

## Commands and results

All commands below exited **0** unless an expected-negative result is stated.

```bash
cargo fmt --all
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --no-fail-fast
python3 scripts/check_apds_schema.py
python3 scripts/check_sdk_contract.py --sdk-root ../agentpm-sdk-node --sdk-root ../agentpm-sdk-python
```

- Final workspace: **1,353 passed, 0 failed, 6 opt-in ignores**, across 21 targets.
  Saved output: `/tmp/m6a-legacy-workspace.log`.
- Focused new CLI suite: all **17** tests pass in the final workspace. Earlier
  focused runs and full Harness regressions also passed; the final workspace
  supersedes those intermediate counts.
- Clippy: `/tmp/m6a-legacy-clippy.log`; formatting and whitespace checks pass.
- User screenshot follow-up: replace the dense combined readiness slot with
  separate **APDS definition**, **Harness support** and **Local runtime** rows.
  Each has a short summary and its own truthful status/color; legacy and limited
  results are warning-colored, and not-ready runtime is red. The generic green
  `Ready` label no longer appears for this combination. **85 TUI tests pass**,
  including narrow-panel rendering and that exact status/color regression:
  `/tmp/m6a-tui-polish-final.log`. The full workspace and Clippy were rerun after
  this presentation change; machine report fields and readiness policy are
  unchanged.
- Native Python environment regressions, including the normally opt-in cases:
  **10 passed, 0 failed**, `/tmp/m6a-native.log`:

  ```bash
  env -u AGENTPM_PYTHON \
    AGENTPM_TEST_UV=/Users/zackhine/Library/Caches/com.agentpm.AgentPM/runtimes/uv/0.13.5/aarch64-apple-darwin/uv \
    AGENTPM_TEST_PYTHON=/opt/homebrew/opt/python@3.13/bin/python3.13 \
    AGENTPM_TEST_CLI=/Users/zackhine/projects/agentpm-project/agentpm/target/debug/agentpm \
    AGENTPM_TELEMETRY=0 \
    cargo test -p agentpm-cli --bin agentpm python_environment::tests -- --include-ignored --test-threads=1
  ```

  These overlap five ordinary workspace tests and run the five native opt-ins;
  the remaining workspace ignore is the separate opt-in SDK publication proof.

- Docs: from `agentpm-api`,
  `REGISTRY_PRIVATE_KEY_B64=AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA= AWS_EC2_METADATA_DISABLED=true .venv/bin/python -B -m pytest -q tests/test_docs_content.py`:
  **5 passed, 166 subtests passed**, `/tmp/m6a-legacy-docs.log`. The key is import-only
  test configuration, not a server credential.
- Prettier passes for README, changelog, this verification, product MDX and CI
  YAML; no Python source changed. `tasks.md`, `test-plan.md` and
  `review-checklist.md` already differ from Prettier at `HEAD` (independently
  checked against Git's baseline). Their existing planning-document formatting
  is retained to avoid an unrelated whole-file reformat; Rust's repo-native
  formatter and whitespace checks pass.
- Immutable APDS checksums/schema copy and both shared SDK fixture guards pass.

The owned smoke directory `/tmp/m6a-preflight-smoke-kuiek066` contains
`commands.json`, `summary.json` and a real CLI-generated consumer project.
Actual init/lint/empty install succeeded; `harness --json` exited **1**, correctly
reporting `conformant / not_runnable / not_evaluated`. Source/lock hashes stayed
unchanged. Install used a non-listening loopback base URL to verify that this
empty project required no Registry request. No model call was made.

The initial broad run found the older authoring test's exact message assertion;
it was corrected intentionally and the entire workspace rerun successfully.
The initial sandboxed Harness run could not bind three local HTTP fixtures;
the unrestricted rerun passed. These failures are not omitted from the record.

## Compatibility, remaining gates and risks

- No APDS bundle/schema, lock format, Registry route/DTO, SDK runtime, Tool
  invocation protocol or provider protocol changed. The new machine report
  fields are additive. No private package access policy changed: preflight reads
  already-installed local state and makes no Registry authorization claims.
- Missing model/provider data now fails noninteractive preflight, including
  `--json`; this is an intentional correction of optimistic readiness. Required
  credentials are checked for presence without exposing their values. Remote
  availability and process/host capability handshakes remain runtime work.
- APDS checking adds local graph/metadata I/O. Knowledge readiness uses existing
  readers; it does not create a scheduler, backend or availability service.
- Preflight validation uses selected lock identities. **Runtime name-only lookup
  fixes remain M6B**, as do SDK wrapper parity and the D13 prompt experiment.
  S2-AC-08's complete cross-version/cross-SDK acceptance and Release Band 2 are
  not claimed complete by M6A.
- Claude's independent M6A review and scoped D20 interface approval are recorded
  as passed on 2026-11-21. Hosted CI on the final revision and operator manual
  acceptance remain pending. M6B decisions and Release Band 2 are separate;
  nothing has been published.
