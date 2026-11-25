# Category Hardening Manual Tests

Use the local reviewed CLI checkout unless a guide explicitly requires a released
binary. Each runbook states its prerequisites, generated workspace and evidence
location. Run one block at a time and inspect the results before continuing.
The capacity harness below is the exception: one command automates its complete
isolated workload and retains a report.

- [Agent Package-first scaffolding (M3A)](CLI_M3A_MANUAL_TESTS.md): default Agent,
  all eight explicit kinds, both Knowledge modes, escaped descriptions, invalid
  input and existing-file protection, Python Tool execution, Node authoring,
  content builds, Template generation and safe Skill export. Local disposable
  fixtures only; no Registry, signing key, paid service or publication. Review
  and hosted CI are operator-confirmed passed; the operator confirmed all manual
  tests passed on 2026-11-15, completing M3A.
- [APDS-aware authoring and legacy lint (M3B)](CLI_M3B_MANUAL_TESTS.md): visible
  local-project cases with an explanation of every defect, expected rule/path,
  scope, exit and file change. Includes strict preflight, read-only legacy
  preview, safe fixes, contextual results, Knowledge/Memory gates and Template
  next steps. Workspaces/evidence stay in gitignored `agentpm/cli-m3b-test/`.
  Review and hosted CI are operator-confirmed passed; the operator confirmed all
  manual tests passed on 2026-11-17, completing M3B.
- [Real local Registry APDS publication (M4A)](REGISTRY_M4A_MANUAL_TESTS.md):
  36 visible cases under gitignored `agentpm/registry-m4a-test/`, explicit raw
  API stages plus ordinary CLI publication for all eight kinds. These use the
  actual local API, Docker application DB and development S3. Covers off/on
  declaration evidence, exact server rejections, signing separation, selected
  dependency archives and multi-target atomicity. Review and all hosted CI are
  operator-confirmed passed; the operator confirmed all 36 manual cases passed on
  2026-11-18, completing M4A. Production activation/capacity gates remain M4B.
- [APDS resolve/install/lock identity (M5A)](CLI_M5A_MANUAL_TESTS.md): five real
  signed dev publications through the usual IntelliJ API/local Docker DB/dev S3,
  visible v4 locks and installed bytes, origin preservation, simple frozen replay,
  exact-version execution and local-root refresh. Separate loopback cases test
  bad downloaded identity/origin/checksums, cache repair and strict lock readers.
  Everything stays in ignored `agentpm/cli-m5a-test/`. Claude review passed; the
  operator requested manual acceptance before pushing/running hosted CI. The
  guide/helper are builder-replayed; the user supplied a passing 38-result
  operator-run audit on 2026-11-20, then confirmed all hosted CI passed the same
  day, completing M5A. Release Band 2 rollout gates remain separate.
- [Frozen graph replay and empty Agents (M5B)](CLI_M5B_MANUAL_TESTS.md): reuse
  four immutable M5A fixtures, publish six declared fixtures through the real
  local API, and replay complete direct/local/multi-root graphs. Separate
  controlled cases expose wrong kinds, missing transitive pins, stale edges,
  digest/origin faults, cache/refresh repair and v2/v3 lock compatibility. Sources
  and 51 captured command expectations stay in ignored `agentpm/cli-m5b-test/`.
  Five builder tests pass, including actual runbook shell replay on an isolated
  fixture; all 28 shell blocks parse. Claude review and hosted CI passed as
  operator-confirmed on 2026-11-20. The operator subsequently supplied the passing
  51-result real-local manual audit, completing M5B. Release Band 2 remains open.
- [Harness definition, support and local readiness (M6A)](CLI_M6A_MANUAL_TESTS.md):
  21 visible local cases, 26 captured command expectations and four TUI checks
  under ignored `agentpm/cli-m6a-test/`. Inspect strict and legacy definition
  diagnostics, missing context/model/credentials, optional backend limitations,
  actual local completion/handoff/abort/approval and machine protocol results.
  Uses a deterministic process model; no API, database, publication or paid
  service is required. Four builder checks pass, including real shell-block and
  PTY replay; all 23 shell blocks parse. Claude review and hosted CI are
  operator-confirmed passed on 2026-11-21. The user supplied the passing 26-result
  audit and four TUI observations the same day, completing M6A. Saved evidence was
  independently rechecked; the earlier builder recovery is disclosed in verification.
- [Exact Component versions and SDK reports (M6B)](CLI_M6B_MANUAL_TESTS.md):
  ten visible local cases, 30 captured command expectations and two TUI Runs
  under ignored `agentpm/cli-m6b-test/`. Checks six-kind exact selection,
  direct/Skill-inherited Tool execution, phase scope, Memory participation,
  ambiguity rejection and the Skill-only workaround, CLI/Node/Python report
  parity and the existing description experiment. No Registry or paid model.
  Four builder checks pass, including actual shell/PTY replay and final audit;
  all 13 shell blocks parse. Claude review is operator-confirmed passed.
  All hosted CI is operator-confirmed passed on 2026-11-21 after the Windows
  corrections and macOS rerun. The user supplied the passing 30-result audit and
  both TUI observations the same day; saved evidence/hashes were independently
  rechecked, completing M6B. Release Band 2 remains separate.
- [Release Band 2 verification](../release-band2-verification.md): published CLI
  0.1.47 and SDK 0.1.18 artifact/hash checks, live Registry/web responses, actual
  signed multi-kind installation and clean frozen replay, wrong kind/digest/range
  rejection, and isolated published-SDK exact-version/report checks. Combined with
  the operator-confirmed CI, ref cleanup and M5A–M6B acceptance, Release Band 2 is
  complete. Raw evidence stays under ignored `release-band2-evidence/`.
- [Automated local Registry capacity measurement](REGISTRY_CAPACITY.md): a single
  command runs the isolated Docker stack, both enforcement settings, actual
  example graphs, larger payloads, concurrency and controlled failures. Evidence
  stays in gitignored `release-band1-evidence/capacity/`; activation still needs
  separate review of measurements and production limitations.

- [Production smoke checks using maintained examples](RELEASE_BAND1_SMOKE.md):
  historical signed installs, new declared summarize-text/devwork-copilot
  versions through both protocols, and clean-consumer verification. Real
  production publications; the guide separately lists the remaining direct API
  and historical-reference checks needed before release closure.

- [Versioned APDS website reference and package links (M8A)](WEB_M8A_MANUAL_TESTS.md):
  real local web/API reads over eleven existing M4A/M5A publications, all eight
  kinds, two versions of the same Tool and an undeclared legacy release. An
  ignored `agentpm/web-m8a-test/` run contains a browser index, copied inputs,
  response evidence and explicit observations. Includes the real private
  `zack-internal` namespace; no publications or visibility changes. Independent
  implementation review and hosted CI are user-confirmed passed on 2026-11-25;
  the user supplied the passing 11-fixture, 60-request, seven-observation audit
  the same day. Saved evidence was independently rechecked, completing M8A.
  Coordinated production release remains separate.
