# M6A manual fixtures

Follow [the operator runbook](../../CLI_M6A_MANUAL_TESTS.md). Everything is created
under an ignored direct child of `agentpm/cli-m6a-test/`.

- `fixture.py` creates visible sources from actual CLI scaffolds, builds and
  lints them, pins inputs and checks actual command results. Its generated
  installed layout and locks are synthetic test state, not Registry evidence.
- `model_service.py` is a deterministic local model process speaking the actual
  `agentpm-service` v1 contract. It only returns phase completion; it has no HTTP
  client or LLM dependency. Captured requests prove what the real Engine asked.
- `shell.sh` is copied into the owned run. `m6a` invokes the reviewed CLI,
  `m6a_expect` captures/checks results, and `m6a_machine` sequences the visible
  protocol requests without sending shutdown during an active Run.
- `test_fixture.py` is builder verification. It replays the operator's command
  blocks, exercises the local protocol and tests the evidence guard. Its
  temporary builder directories are removed; it does not grant user acceptance.

Preserve operator timestamp directories and their evidence. Builder cleanup
applies only to the exact `builder-*` directory created by that test invocation.

The factory requires Python 3.11+ and the built CLI. Builder checks additionally
use the workspace's existing pytest environment. No API credentials, Registry,
Docker, database, S3, signatures or real models are required.
