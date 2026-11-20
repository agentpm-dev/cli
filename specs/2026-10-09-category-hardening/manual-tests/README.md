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
