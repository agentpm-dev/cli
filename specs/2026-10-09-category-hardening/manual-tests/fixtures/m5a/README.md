# M5A manual-test fixtures

Use [the runbook](../../CLI_M5A_MANUAL_TESTS.md) from the CLI repository root.
Operator workspaces live in ignored `agentpm/cli-m5a-test/<run-date>/`.

The helper has two separate roles. It reuses M4A's existing guarded raw
publication, author-signing, dev-S3 upload and read-only local DB routines for
five **real local Registry** seed releases. It also serves eleven explicitly
controlled loopback responses to test client behavior when downloaded identity,
origin or integrity is wrong. The controlled server never calls the real API,
uses S3 or writes database rows.

Generated files are meant to be opened:

- `session.json` names each unique `@namespace/name@version` reference.
- `cases/*/source/` contains the five real publication inputs. The legacy case
  has no selector; Echo v1/v2, Skill and Agent explicitly declare 1.0.0. Sources
  pass actual CLI lint, using read-only legacy preview for the historical
  simulation. The legacy case is published with **local** enforcement off;
  enforcement is restored before the declared releases are published.
- `cases/*/request.json`, `any.tar.gz` and `finalize-good.json` hold exact API
  reservation data, archive bytes and author signatures. `init.json` and
  `receipt.json` are actual network results. The two Echo versions share a name;
  Skill selects v1 and Agent selects v2 plus that Skill.
- `projects/*/agent.json` are separate editable consumers. Real API installs and
  deliberately corrupted test state are restricted to these owned directories.
  Published source manifests and maintained examples are not edited by the
  local-root/cache mutation actions.
- `controlled/*/source/agent.json`, `artifact.tar.gz`, `resolve.json` and
  `init-item.json` make each adversarial difference inspectable. Most deliberately
  bad archives have honest checksums; only `wrong-digest` lies about the bytes.
- `control-server.json` records the separate loopback origin. The server's
  request log excludes Authorization headers and tokens.
- `evidence/` records source lint, raw HTTP attempts, CLI command/cwd/output/exit
  results, actual locks and byte-hash snapshots. `author.seed` and presigned URLs
  are private evidence under the ignored, private run directory.

`prepare` only writes/lints/packages. `publish CASE` explicitly performs and
records real init/upload/finalize, preserving completed receipts on rerun.
`inspect-db CASE` reuses M4A's real local read-only checks. `resolve CASE|graph`
checks the actual API metadata/digests without adding them to its response.
`check-lock PROJECT` checks the real lock and installed manifest against the
fixture archive/source. `snapshot` and `unchanged` compare actual bytes;
`check-result` requires the intended CLI message as well as its machine status
and exit. `audit` checks saved outcomes and current state, but does not rerun
installs, publish releases or grant operator/hosted-CI approval.

The runbook intentionally uses Node single-archive publications: the CLI's
bundled Registry root pin is production-specific. These local author-verified
installs must not imply a locally bootstrapped Registry attestation trust chain.
Existing native/release integrity tests and later hosted target jobs remain
separate evidence. No product trust policy is weakened for these manual tests.

Builder checks are distinct from operator acceptance:

```bash
PYTHONPATH="$PWD/../agentpm-api" \
REGISTRY_PRIVATE_KEY_B64=AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA= \
AWS_EC2_METADATA_DISABLED=true \
  ../agentpm-api/.venv/bin/python -B -m pytest -q \
  specs/2026-10-09-category-hardening/manual-tests/fixtures/m5a/test_fixture.py
```

They use a loopback fixture and the independent publisher's validator, with no
production or normal development DB/S3 publication. They do not establish that
the operator completed the real-local API runbook.
The dummy signing key above is only for isolated test imports; do not put it in
the operator's running local API configuration.
