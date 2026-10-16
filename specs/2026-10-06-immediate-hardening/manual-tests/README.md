# Immediate Hardening Manual Tests

Run commands from the `agentpm` repository root unless a runbook says otherwise.
Fixture setup helpers live in `scripts/`; their generated workspaces and results
are disposable and gitignored.

- [CLI scaffolding and lint quality (Milestone 8)](CLI_M8_MANUAL_TESTS.md)
- [CLI messaging and exit codes (Milestone 9)](CLI_M9_MANUAL_TESTS.md): individual
  CLI commands; setup creates empty directories only. Uses your local backend
  for real PAT revocation and an optional mock for controlled HTTP failures.
