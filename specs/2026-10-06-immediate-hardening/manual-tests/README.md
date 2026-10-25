# Immediate Hardening Manual Tests

Run commands from the `agentpm` repository root unless a runbook says otherwise.
Fixture setup helpers live in `scripts/` or the runbook's `fixtures/` directory.
Generated workspaces/results are disposable; each guide states their location.

- [CLI scaffolding and lint quality (Milestone 8)](CLI_M8_MANUAL_TESTS.md)
- [CLI messaging and exit codes (Milestone 9)](CLI_M9_MANUAL_TESTS.md): individual
  CLI commands; setup creates empty directories only. Uses your local backend
  for real PAT revocation and an optional mock for controlled HTTP failures.
- [PostHog setup and verification (Milestone 10A)](ANALYTICS_M10A_SETUP.md):
  operator-guided project, survey, event, billing, and dashboard checks.
- [CLI telemetry and PostHog handoff (Milestone 10B)](CLI_M10B_MANUAL_TESTS.md):
  individual commands with isolated installation state, deterministic local
  fixtures, opt-out/exit checks, and the two deferred dashboard insights.
- [Python dependency resolution (Milestone 11A)](CLI_M11A_MANUAL_TESTS.md):
  individually run lint, real dev-mirror acquisition, portable resolution,
  cache/refresh, override/failure and reserved-namespace checks. Explicit fixture
  copies and isolated temporary HOME; no setup runner, publication or S3 writes.
- [Python locks, publication and consumer metadata (Milestone 11B)](CLI_M11B_MANUAL_TESTS.md):
  individual preparation, committed-lock reuse, stale/removal and compatibility
  checks, followed by one explicit dev-only publish and normal/frozen consumer
  installs. Includes local Registry dependency/migration prerequisites and
  isolated per-process user state; production is not modified.
- [Managed Python Tool environments (Milestone 11C)](CLI_M11C_MANUAL_TESTS.md):
  develop and publish a dev-only `validate-order`, inspect native dependencies
  in managed environments, exercise normal/frozen installs, repair, interpreter
  changes and import-path isolation, then call the real Tool through Harness
  using a local scripted model. Includes an unpublished legacy vendoring probe;
  production and paid model services are not used.
- [Python Tool payload compatibility (Milestone 12)](CLI_M12_MANUAL_TESTS.md):
  portable/native classification, format errors, incidental collection and
  explicit overrides, plus a local capture of publish descriptors/uploaded
  archives. One terminal, isolated state, no backend/PAT/S3/publication; rechecks
  disposable copies of both prepared examples without refreshing dependencies.
