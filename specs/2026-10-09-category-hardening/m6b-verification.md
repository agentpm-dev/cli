# M6B: Exact Component versions and SDK visibility

Builder verification: **2026-11-21**. Scope: **M6B only**, D13 / D20,
S2-AC-08 / S2-AC-09, T-RUN-01,05–09. Implementation and local verification are
complete. User confirmed independent Claude review passed with no remaining
findings on **2026-11-21**, including the guard assertion follow-up. Hosted CI
and operator manual acceptance remain pending. No production Registry requests, publication or deployment occurred.

## Changes and decision D20

| Source                                                                         | Purpose                                                                                                                                                                                                                      |
| ------------------------------------------------------------------------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `crates/agentpm-cli/src/harness_plan.rs`                                       | Restrict the runtime package graph to exact identities visited by selected-Agent APDS inspection, including Skill-inherited Tools. Reject ambiguous active identities before execution; never select a version by map order. |
| `crates/agentpm-cli/src/commands/harness/tests/exact_versions.rs`              | Two-Agent/two-version regression for all six Component kinds, actual built Knowledge/Memory, inherited-only Tools, and explicit ambiguity classification.                                                                    |
| `crates/agentpm-cli/src/harness_engine/tests/{core,memory}.rs`                 | Additive global/phase Tool and Skill applicability, Profile applicability and lack of permission-granting power, uninvoked external operations and out-of-phase operation rejection.                                         |
| `crates/agentpm-cli/src/harness_engine/tests/description_ab.rs`                | D13 comparison confined to a test model adapter; no production prompt change.                                                                                                                                                |
| `crates/agentpm-sdk/contracts/sdk-v1/harness_fixture.py`                       | Shared local fixture factory using actual CLI scaffold/build/lint commands, synthetic installed locks and version-identifiable Node Tools. SDK snapshots are byte-identical.                                                 |
| `../agentpm-sdk-node/src/harness.ts`                                           | Export support/report types and `supportedStandards()` through the existing client/host bridge.                                                                                                                              |
| `../agentpm-sdk-python/src/agentpm/{harness,__init__}.py`                      | Export `HarnessSupportedStandard` and equivalent `supported_standards()` through the existing client.                                                                                                                        |
| Both SDKs' `harness-standard` tests                                            | Actual CLI/SDK report parity for ready, no Loop, no model, legacy and unsupported standard; real execution with exact versions and Loop denial. Old/empty/malformed advertisements and unknown fields are covered.           |
| CLI/SDK changelogs, SDK READMEs and `../agentpm-api/docs/v0.1/cli/harness.mdx` | Feature-local usage, source authority, result envelopes and compatibility limits. No docs route changes or comprehensive rewrite.                                                                                            |

**Selected interface:** retain `agentpm-harness-machine`, version **1**. APDS
identity is independent of protocol identity. `initialize` retains
`payload.preflight`; `preflight` retains its existing `{report, mcp_exports}`
envelope. Both SDK convenience methods read that envelope or an older flat
response. Existing methods and unknown fields are preserved. An absent
advertisement returns Node `undefined` / Python `None`; an explicitly empty list
stays empty. Malformed advertisements or report envelopes raise `invalid_standard_advertisement` in both SDKs.
The SDK does not hard-code or infer a support list.

A new protocol or flattened replacement response would create unnecessary
compatibility costs. Tests against the actual binary exposed the nested
preflight envelope that the earlier flat mocked responses did not cover; both
shapes now have coverage. Existing callers still receive the original payload.
The user confirmed Claude's completed review with no remaining findings on
2026-11-21, recording D20 signoff on the selected exact-resolution/interface choice.

## Exact-version boundary and limitations

Before this change, APDS inspection followed the selected graph, but runtime
snapshot construction still received every workspace package. Name-only lookup
could read another Agent's earlier version, while the Tool dispatcher's map
could independently retain the later version. Filtering the shared runtime
input corrects all consumers without changing the orchestration engine,
provider protocol, action identifiers or scope model.

The checked selected graph now supplies Tool, Skill, Knowledge, Memory space and
operation, Profile and Loop metadata. Other Agents' installed versions are not
runtime candidates. Actual SDK-hosted Runs execute version-marked Node Tool
files for both Agents; model requests independently assert each Component
version, including Memory operation package versions. One Agent has a Tool
both directly and through a Skill; the other has only the inherited path.
Review-phase Tool denial remains effective. Bound external operations remain
available without directly exposing their target spaces; Skill scripts do not
run automatically.

**Known Runner limitation:** if the selected graph contains multiple versions of
an actively named Component, the current versionless action/snapshot interface
cannot safely distinguish them. Preflight emits fatal
`ambiguous_locked_component`, with APDS definition still `conformant`, Harness
`limited` and local runtime `not_ready`. It never collapses distinct versions or
claims the artifact is APDS-invalid. Supporting simultaneous version-specific
routing would require changes beyond this milestone's existing-interface
boundary; failing explicitly is preferable to silently choosing a version.
Multi-Agent workspaces with one selected version per identity are supported.

The user accepted this documented limitation on 2026-11-21 and confirmed that
version-qualified bindings or aliases should remain outside the current work.
The user subsequently reported that a future solution is being planned in
Stage 3. That future design is not assessed or implemented by M6B; the clear
current ambiguity diagnostic and safeguard remain in place.

**Review correction — uniqueness guard coverage:** the ambiguity regression now
asserts that both Tool and Memory candidate collections are nonempty and every
matching candidate is `Suppressed`, independently of the fatal Run guard.
Reverting `package_by_name` to `Some(package)` makes the Tool candidate
`Available` and fails the new assertion. The mutation was tested and the exact
original production source restored. Both focused exact-version tests then
pass, as do Rust formatting and whitespace checks. Logs:
`/tmp/m6b-uniqueness-mutation.log` and `/tmp/m6b-uniqueness-final.log`.
This follow-up changes test assertions only; the full-workspace results below
are the preceding implementation run and were not repeated for this test-only
correction. User subsequently confirmed independent review passed with no remaining findings; hosted CI remains open.

Definition checks remain read-only. Standard identity/provenance, lock v4,
private-package authorization, target selection, signature/integrity and native
Python provisioning mechanisms are unchanged. No published APDS file changed.

## Runtime semantic evidence

The full Rust suite exercises the following published semantic scenarios through
actual Engine or capability evaluation, not just manifest acceptance. The
published corpus and its Runner observations remain immutable. This is scoped
implementation evidence, not a blanket APDS execution certification.

| Cases   | Executed evidence                                                                                                                                                                                                                                       |
| ------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| A–C     | New `m6b_global_and_phase_skill_inheritance_stays_additive_and_scoped`; existing `phase_scoped_tool_candidates_are_active_only_for_matching_phase` and `loop_access_suppresses_skill_inherited_tools_but_not_skill_resources`; real SDK versioned Runs. |
| D, H, N | Existing actual CLI `harness_standard` tests for phase/reference defects, valid no-Loop and unavailable context; N's packaged-content assertions remain validator checks.                                                                               |
| E, K    | New capability test checks additive global/phase Profiles as sets, without a prompt-order requirement, and verifies a Tool-use hint grants no access under Loop denial. Existing Profile prompt/compatibility regressions remain.                       |
| F       | `memory_lifecycle_external_invocation_runs_only_external_participating_operation`: real authorized deletion with direct read/write forbidden, plus `memory_lifecycle_record_count_transform_writes_internal_output_space`.                              |
| G, M    | New no-invocation/no-scheduling and out-of-phase rejection tests; existing external invocation scope/availability/nonexternal-trigger guards.                                                                                                           |
| I, J    | New Rust all-kind exact snapshot/ambiguity tests and both SDK actual Tool executions, including repeated same-version direct/inherited paths.                                                                                                           |
| L       | Existing `memory_descriptors_union_global_and_phase_scoped_bindings`, direct-access and effective-phase tests.                                                                                                                                          |
| O       | Existing implicit outcome/transition and repair tests; actual SDK fixtures omit outcomes and complete both phases implicitly.                                                                                                                           |
| P       | Existing Knowledge/Memory capability, readiness and backend failure tests; M6A definition/readiness classifications and SDK report comparisons remain intact.                                                                                           |
| Q, R    | `memory_lifecycle_count_triggers_start_armed_on_initial_boundary_observation` and `memory_lifecycle_count_triggers_do_not_refire_while_count_is_unchanged`, covering count and capacity.                                                                |
| S       | Record-count delete rearm and capacity automatic-delete rearm tests.                                                                                                                                                                                    |
| T       | Interval dormancy, phase-start eligibility, no read-trigger evaluation and persistent baseline across session restart tests.                                                                                                                            |

Existing checkpoints, approvals, retries, `$end` / `$abort` / `$handoff`, phase
isolation, Tool errors, traces/events and internal Memory transformation tests
remain unchanged and pass. Skill resources remain on-demand authored content;
the shared fixture includes a script with an observable marker and asserts it
never executes automatically.

## Decision D13

See [the reproducible description comparison](m6b-description-decision.md).
**Selected: defer injection; retain current prompt behavior.** One- and
three-phase actual Engine runs compare unchanged versus test-only injected
prompts. Additional content duplicates phase intent; deterministic outputs are
identical. Provider token/cost and real-model quality improvement remain
unmeasured, explicitly disclosed. No production prompt setting or normative
APDS requirement is introduced.

## Verification

Commands run from each relevant repository root; detailed local logs are under
`/tmp/m6b-*`. Generated description evidence stays under ignored CLI `target/`.

- Rust: `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo test --workspace --no-fail-fast`: **1,359 passed, 0 failed, 6 opt-in
  ignores**, 21 targets. `/tmp/m6b-workspace-unrestricted-final.log` and
  `/tmp/m6b-clippy-final.log`.
- The sandbox prevents loopback HTTP fixtures from binding. The unrestricted
  workspace run is authoritative. Early new-fixture schema/phase-name mistakes
  were corrected before that final passing run; earlier red counts are not
  represented as final evidence.
- Native Python environment regressions, including all five normally opt-in cases:
  **10 passed, 0 failed**, `/tmp/m6b-native-unrestricted.log`. Run with the pinned
  uv 0.13.5, explicit Python 3.13 host and reviewed CLI, using
  `cargo test -p agentpm-cli --bin agentpm python_environment::tests --
--include-ignored --test-threads=1`. The sandboxed attempt was blocked by
  loopback/cache/network permissions; the unrestricted rerun passed.
- Node: `AGENTPM_TEST_CLI=<reviewed binary> pnpm test`: **119 passed, 2 optional
  skips**; `pnpm typecheck`, `pnpm lint`, `pnpm build`, `pnpm format` pass.
  Both ESM/CJS and declaration builds retain their packaging contract.
- Python: `AGENTPM_TEST_CLI=<reviewed binary> .venv/bin/python -m pytest`:
  **120 passed, 2 optional skips**, existing semver warnings; `mypy src/agentpm`,
  `ruff check .` and `black --check .` pass. No optional adapter or package model
  was rewritten. The skipped SDK tests are existing opt-in native/host fixtures;
  all twelve new tests in each SDK pass, including six actual-CLI cases and six transport cases.
- Shared fixtures: `python3 scripts/check_sdk_contract.py --sdk-root
../agentpm-sdk-node --sdk-root ../agentpm-sdk-python` passes. APDS schema and
  immutable bundle checksum checks pass. New fixtures are explicitly synthetic
  installed state, not signed publication/integrity evidence.
- API feature-local docs: import-only fixture configuration with
  `REGISTRY_PRIVATE_KEY_B64=AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=` and
  `AWS_EC2_METADATA_DISABLED=true`, then `.venv/bin/python -B -m pytest -q
tests/test_docs_content.py`: **5 passed, 166 subtests passed**.
- Built Node ESM/CJS exports expose the new method, and Python public exports
  expose the method and `HarnessSupportedStandard`; direct import probes pass.
- Markdown formatting and `git diff --check` pass. Existing planning documents
  retain their prior layout rather than undergoing an unrelated whole-file
  reformat.

## Remaining gates and release order

- Independent Claude review of implementation, D13 evidence/decision and D20
  exact-version selection is user-confirmed passed on 2026-11-21. Hosted
  CLI/native-target and both SDK CI on matching revisions remain pending.
- Operator manual acceptance follows review/CI; no manual runbook or acceptance
  has been claimed for this milestone yet.
- CLI **0.1.47 remains prepared and unreleased**. SDK package versions remain
  **0.1.17**, with new API changes recorded under Unreleased. Prepare/publish new
  SDK versions as part of Release Band 2; existing 0.1.17 clients retain raw
  payload access and do not need an upgrade merely to read extended v4 locks.
- Match the CLI-owned fixture revision when running SDK CI. After the matching
  CLI merges, restore/remove temporary `AGENTPM_CLI_REF` selections and verify
  both SDK workflows against `main`, as already required by Release Band 2.
- Ship the compatible CLI before relying on its behavior in distributed locks
  and new SDK guidance. No new lock format, protocol version, Registry deployment
  or automatic example republication is part of M6B.
