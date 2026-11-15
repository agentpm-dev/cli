# Category Hardening Manual Tests

Use the local reviewed CLI checkout unless a guide explicitly requires a released
binary. Each runbook states its prerequisites, generated workspace and evidence
location. Run one block at a time and inspect the results before continuing.

- [Agent Package-first scaffolding (M3A)](CLI_M3A_MANUAL_TESTS.md): default Agent,
  all eight explicit kinds, both Knowledge modes, escaped descriptions, invalid
  input and existing-file protection, Python Tool execution, Node authoring,
  content builds, Template generation and safe Skill export. Local disposable
  fixtures only; no Registry, signing key, paid service or publication. Review
  and hosted CI are operator-confirmed passed; the operator confirmed all manual
  tests passed on 2026-11-15, completing M3A.
