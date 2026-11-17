# M3B manual inputs

These 17 visible inputs are copied into local per-case `agent.json` files by
[the manual runbook](../../CLI_M3B_MANUAL_TESTS.md). They are test inputs, not
publishable example packages or additions to the immutable APDS corpus.

- `valid-agent` is the minimal positive control.
- Selector cases omit `standard`, set an unsupported version or use `null`.
- Declaration/semantic cases expose the exact defect listed in the guide's
  rule/path table. Several reuse the pinned corpus inputs with an editor hint.
- `legacy-blank-description` proves inferred compatibility still rejects other
  manifest defects.
- `knowledge-missing-file` deliberately lacks its declared file in the supplied
  complete package directory. `needs-graph` requires a Loop graph the test does
  not supply.
- `malformed-json.fixture.txt` deliberately contains broken JSON. The `.txt`
  suffix keeps a JSON formatter from trying to repair this negative input.

Only the copied files under gitignored `cli-m3b-test/<run-date>/` are used for the
interactive tests. Setup does not run tests. The guide explains each command and
actual/expected result separately; its final evidence check reads those saved
results rather than silently running additional CLI commands.
