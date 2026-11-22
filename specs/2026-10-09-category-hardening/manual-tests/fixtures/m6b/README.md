# M6B manual fixture helpers

Follow [the operator runbook](../../CLI_M6B_MANUAL_TESTS.md). Run it one block at
a time from the visible, gitignored `agentpm/cli-m6b-test/<run>/` workspace.

- `fixture.py` prepares ten cases from the CLI-owned shared SDK fixture, records
  real scaffold/build/lint outputs, generates shell helpers, and checks each
  command's semantic result, exact versions, Tool markers and input hashes.
  It refuses existing runs and mismatched commands/directories; `audit` rechecks
  all 30 saved results and both TUI Run observations.
- `model_service.py` implements the actual process service protocol. It records
  raw model requests and scripts one Tool action plus two implicit completions.
  It has no HTTP or LLM client.
- `sdk_node.mjs` imports the built checkout SDK ESM entrypoint; `sdk_python.py`
  imports the Python checkout source. They exercise real CLI machine sessions,
  keep initialization/preflight envelopes and typed supported standards, host
  the scripted model where requested, and retain raw requests for independent
  helper assertions. SDK stdout is retained in full; the terminal shows a compact
  summary so version/report results remain readable.
- `test_fixture.py` checks shell syntax and replays the actual runbook command
  blocks, two real PTY Runs and the final audit. Guard tests reject wrong runtime
  versions, absent diagnostics, stale TUI proof, changed inputs and altered
  expectations. It creates its own `builder-<UUID>` directory and removes only
  that exact directory. It never sweeps existing operator runs.

Builder reproduction from the CLI root, after building the CLI and Node SDK:

```bash
../agentpm-api/.venv/bin/python -B -m pytest -q \
  specs/2026-10-09-category-hardening/manual-tests/fixtures/m6b/test_fixture.py
```

The shell replay log is `agentpm/target/m6b-manual-builder.log` (ignored).
Builder checks grant neither operator acceptance nor hosted CI approval.
All installed state is synthetic: placeholder integrity strings do not prove
publication, signature verification or real installation. Real install behavior
was tested separately in M5A/M5B. No server or paid model is required here.
