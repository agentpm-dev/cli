# M5B manual-test fixtures

Start with [the runbook](../../CLI_M5B_MANUAL_TESTS.md) in the CLI repository.
It assumes the retained M5A seed run, the actual local IntelliJ API/Docker DB/dev
S3, and the reviewed CLI. Operator evidence stays under ignored
`agentpm/cli-m5b-test/<timestamp>-<pid>/`.

`prepare` reads/copies four immutable M5A fixture releases without changing their
sources, archives, signing key or database rows. It creates six new declared
sources and signs their archives: Knowledge, Memory, Profile, Loop, composed
Agent and empty Agent. Knowledge/Memory build products come from the real CLI;
all new manifests and local consumers pass actual CLI lint. Preparation makes
no publication request and grants no acceptance. No enforcement toggle is needed.

- `cases/`: six new publication sources, actual CLI build outputs, request bodies,
  archives and signature statements. Later `init.json` and `receipt.json` are real
  local API results. The transport's `mode: legacy` means single-archive HTTP,
  not inferred APDS origin; all six new sources declare the standard.
- `reused/`: copies of the old Echo v1/v2, Skill and legacy Tool sources/archives/
  signature evidence. The original M5A run is left untouched.
- `catalog.json` and `session.json`: explicit identities, original archive hashes,
  declaration provenance and the old/new reference mapping.
- `projects/`: distinct direct, local, workspace, empty, guard and fault consumers.
  Direct-package consumers start without local manifests. Deliberate mutations
  and cleaning are restricted to these owned projects. Normal installed state is
  always created by the CLI; only documented fault actions edit it deliberately.
- `controlled/`: visible init-item JSON plus same-size corrupt archive copies.
  The separate loopback server reads these inputs. It cannot publish to the real
  Registry or write S3/DB rows. Its signed fixture proof is not independent
  evidence of namespace authorization or Registry attestation trust.
- `evidence/`: actual command/cwd/stdout/stderr/expected-vs-actual exit records,
  before/after source/lock/payload hashes, request offsets and raw request slices,
  source lint/build logs and inspection results.
- `author.seed`: the new disposable private author seed; `author-public-key.txt`
  is registered in the real local namespace. The PAT is never written to the
  generated paths or shell command logs. Keep the ignored workspace private.

`preflight` reads the real local DB namespace and calls actual resolve for the
four reused identities, comparing original digests and origins. It catches a
reset DB or missing prerequisite before any new publication.
`publish`/`inspect-db` reuse M4A's existing fixed-loopback, development-bucket,
real HTTP/signing and read-only SQL guards. They never create synthetic
application tables or backfill historical rows.

`check-lock` compares the exact expected set, kind/version/digest/provenance and
all installed payload files against the immutable fixture sources. `snapshot`
and `unchanged` include authored source files and workspace metadata, lock bytes
and installed payload hashes. `traffic` checks recorded request slices for
absence of resolve/finalize, early rejection, or exact archive GET counts.
`check-result` requires the intended machine error, not merely a nonzero exit.
`audit` refuses missing evidence, checks all documented outcomes and current
state, inspects all six real local publications and checks the CLI binary hash.
It leaves operator and hosted-CI approval flags false.

`functions.sh` is copied from the committed `shell.sh`. Generated `paths.sh`
contains the actual owned paths and sources the functions, so either terminal can
resume by sourcing one exact path. Computed run paths cannot be overridden by a
stale `M5B_ROOT` from another run. `clean` removes only a named owned project's
`.agentpm/`; source and lock are retained. `mutate ... restore` restores the
fixture source/lock backup without changing the Registry's immutable archives.

Builder verification from the CLI root:

```bash
PYTHONPATH="$PWD/../agentpm-api" \
REGISTRY_PRIVATE_KEY_B64=AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA= \
AWS_EC2_METADATA_DISABLED=true \
  ../agentpm-api/.venv/bin/python -B -m pytest -q \
  specs/2026-10-09-category-hardening/manual-tests/fixtures/m5b/test_fixture.py
```

The placeholder key is for isolated test imports only. Tests use temporary owned
workspaces and loopback fixture HTTP, verify signatures and the actual Registry
publisher validator, exercise correct/incorrect replay and run the guide's real
shell blocks. They do not publish into the operator's DB/S3 or fabricate missing
publication receipts/operator acceptance. Their private builder directories are
removed afterward. Native/release tests and hosted CI remain separately recorded
implementation evidence.
