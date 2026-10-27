# Milestone 13B: Release Uploads, Finalize, Cleanup and Scanning

Run **one block at a time in one new macOS/zsh terminal**, starting in the
`agentpm` repository. Keep that terminal open. Your existing local Registry can
continue running in IntelliJ; this guide explicitly says when to change its env
and restart it. You do not need another command terminal.

This walkthrough has two kinds of checks:

- **Real development publication:** local Registry, its development database,
  and **`agentpm-dev-tools`**. Small fixture archives are uploaded and finalized,
  scanned by the real GuardDuty setup, and inspected with read-only SQL.
- **Controlled failure checks:** a separate disposable PostgreSQL container and
  mocked storage, using the checked-in tests. These exercise expiry, orphan
  recovery, concurrent changes, failure rollback and infection without aging or
  editing real development rows or uploading malware.

Normal CI, all four Resolver Contracts, the Registry PostgreSQL job and independent
review are already operator-confirmed passed. Release Contract Parity is expected
to fail against pre-merge CLI `main`; its post-merge run remains a Release Band 9
gate. This guide does not require merging or deploying anything.

| Section | What you verify                                                                         |
| ------- | --------------------------------------------------------------------------------------- |
| 1       | Build and prepare tiny real archives, including a real Python resolution.               |
| 2       | Confirm local/dev configuration and the default-off API gate.                           |
| 3       | Finalize one portable artifact; inspect immutable bytes and receipt reuse.              |
| 4       | Resume a four-artifact release, reject missing/corrupt uploads, renew and recover.      |
| 5       | Finalize a dependency-bearing `any` artifact; reject size excess and inventory changes. |
| 6       | Publish/install a legacy Tool and confirm new-format consumption stays gated.           |
| 7       | Inspect real GuardDuty tags, scan rows and scheduled staging cleanup.                   |
| 8       | Exercise destructive failure scenarios only in disposable PostgreSQL.                   |
| 9       | Restore the local feature gate and record acceptance.                                   |

The new-format flow has **no public CLI publish command yet**. The checked-in
[`fixture.py`](fixtures/m13b/fixture.py) helper makes each Registry request or S3
PUT only when you run its corresponding command. It does not enable the backend,
modify database rows, set scan tags or publish everything automatically. It
rejects non-loopback Registry URLs and storage PUTs outside the dev bucket.

The four-target fixture contains portable source plus a target descriptor in
each archive. This verifies upload/inventory handling, **not four native builds**
or cross-platform execution. Dependencies are actually resolved by the CLI;
their digests are not invented. No production publication, signing, paid model
call or PostHog setup is involved. Dev S3 writes/reads and GuardDuty scans do occur.

## 1. Prepare Paths, Build and Create the Archives

Start in the **`agentpm` repository root**. The API venv must have the existing
test dependencies and Python 3.11+. Your local Registry should already be able
to serve the dev resolver mirror for the dependency dry run.

```bash
umask 077
export M13B_REPO="$PWD"
export M13B_API="$M13B_REPO/../agentpm-api"
export M13B_PYTHON="$M13B_API/.venv/bin/python"
export M13B_APM="$M13B_REPO/target/debug/agentpm"
export M13B_SCHEMA="$M13B_REPO/schemas/agentpm.manifest.schema.json"
export M13B_FIXTURE="$M13B_REPO/specs/2026-10-06-immediate-hardening/manual-tests/fixtures/m13b/fixture.py"
export M13B_ROOT="$(mktemp -d "${TMPDIR:-/tmp}/agentpm-m13b.XXXXXX")"
export M13B_STATE="$M13B_ROOT/state"
export M13B_BASE='http://127.0.0.1:5000'
export M13B_NAMESPACE='zack'
unset M13B_TOKEN
mkdir -p "$M13B_STATE/config" "$M13B_STATE/data" "$M13B_STATE/cache"
mkdir -p "$M13B_STATE/AppData/Roaming" "$M13B_STATE/AppData/Local"
m13b_python() {
  env PYTHONPATH="$M13B_API" PYTHONDONTWRITEBYTECODE=1 \
    REGISTRY_PRIVATE_KEY_B64=AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA= \
    AWS_EC2_METADATA_DISABLED=true "$M13B_PYTHON" -B "$@"
}
m13b() {
  env HOME="$M13B_STATE" USERPROFILE="$M13B_STATE" \
    XDG_CONFIG_HOME="$M13B_STATE/config" XDG_DATA_HOME="$M13B_STATE/data" \
    XDG_CACHE_HOME="$M13B_STATE/cache" APPDATA="$M13B_STATE/AppData/Roaming" \
    LOCALAPPDATA="$M13B_STATE/AppData/Local" AGENTPM_TELEMETRY=0 \
    AGENTPM_TOKEN="${M13B_TOKEN:-apm_m13b.fixture}" \
    "$M13B_APM" --base-url "$M13B_BASE" "$@"
}
m13b_fixture() {
  local m13b_action="$1"
  shift
  m13b_python "$M13B_FIXTURE" "$m13b_action" "$M13B_ROOT" "$@"
}
m13b_python - <<'PY'
import sys
import boto3, jsonschema, psycopg, pytest, rfc8785, sqlalchemy
assert sys.version_info >= (3, 11)
print('Python and existing test dependencies are available.')
PY
cargo build -p agentpm-cli
m13b --version
m13b_python scripts/check_release_contract.py --registry-root "$M13B_API"
m13b_fixture prepare --namespace "$M13B_NAMESPACE"
git -C "$M13B_REPO" rev-parse HEAD
git -C "$M13B_API" rev-parse HEAD
printf 'Keep this terminal and directory: %s\n' "$M13B_ROOT"
```

Use another namespace if needed **before** preparation. It must be a development
namespace you may publish to, with optional signing; the guarded route deliberately
rejects required-signature namespaces until release signing is implemented.
Do not weaken a production namespace policy for this test.

The wrapper's dummy signing key is only for importing API contract/test modules;
it changes neither your running Registry nor its saved signing configuration.
The isolated CLI HOME also applies only to `m13b` calls. AWS inspection below
uses your normal shell's credential chain, not this isolated CLI state.

Create the ordinary dry-run archives, then add the new-format descriptors using
the helper. The CLI still performs manifest lint/schema validation.

```bash
cd "$M13B_ROOT/portable"
m13b lint --schema "$M13B_SCHEMA"
m13b publish --dry-run --schema "$M13B_SCHEMA"
m13b_fixture package portable

cd "$M13B_ROOT/matrix"
m13b lint --schema "$M13B_SCHEMA"
m13b publish --dry-run --schema "$M13B_SCHEMA"
m13b_fixture package matrix

cd "$M13B_ROOT/dependencies"
m13b lint --schema "$M13B_SCHEMA"
m13b publish --dry-run --refresh-python --schema "$M13B_SCHEMA"
m13b_fixture package dependencies

cd "$M13B_ROOT/legacy"
m13b lint --schema "$M13B_SCHEMA"
m13b publish --dry-run --schema "$M13B_SCHEMA"
```

Expected: portable is `0.1.0`/`any`; matrix is `0.1.1`/four targets; dependencies
is `0.1.2`/`any`, with a real resolution bound. They share one uniquely named
package, with **one logical version per case**. The separate legacy package is
`0.1.3`. No upload has occurred. The helper prints release digests and writes
each `request.json`, canonical release document and target archives under its
case directory. Oversize test requests declare large sizes but create no large
files. Do not rebuild a case after reserving it; start a fresh root if necessary.

## 2. Check the Local Registry and Its Default-Off Gate

In **IntelliJ's local backend run configuration**, confirm:

- The database is your **development** Registry database, not production.
- `S3_BUCKET=agentpm-dev-tools`, `S3_PREFIX=tools`, and the normal dev AWS region
  and credentials are configured.
- Migrations through **V1.104** have applied successfully. Leave old migration
  checksums alone if startup reports a mismatch.
- `MAX_UPLOAD_BYTES=1000000000` and `MAX_RELEASE_UPLOAD_BYTES=1000000000` are at
  their defaults for these specific limit expectations.
- `RELEASE_PUBLISH_ENABLED=false` initially. Restart after changing IntelliJ env.

The dev bucket's GuardDuty coverage is already operator-confirmed configured like
production. Section 7 checks whether these newly created objects actually get
tagged and the poller observes them. Keep the scheduler running normally.

Back in your **same command terminal**, enter a **development publish PAT**:

```bash
read -s 'M13B_TOKEN?Development publish PAT: '
printf '\n'
export M13B_TOKEN
m13b whoami
m13b_fixture init portable --expect 404 --code release_publish_disabled
```

Expected: `whoami` identifies your dev account; the disabled request returns
`404 / release_publish_disabled` and creates no reservation. If it returns 200,
the local flag is still enabled; stop here and correct the backend configuration.
The token is not written to fixture files or printed by the helper. Init files
later contain short-lived signed PUT URLs; keep the disposable directory private.

Now set **only the local backend's** `RELEASE_PUBLISH_ENABLED=true` in IntelliJ
and restart it. Setting it in this command terminal would not change that
already-running backend. Production/Render must remain disabled.

For the later read-only S3 inspection, make your **dev AWS credentials** available
in this terminal using your existing AWS profile or credential setup. Credentials
saved only in IntelliJ are not inherited here. Confirm identity without printing keys:

```bash
m13b_python - <<'PY'
import boto3
print(boto3.client('sts', region_name='us-east-1').get_caller_identity()['Arn'])
PY
```

## 3. One Portable Artifact and an Idempotent Receipt

Any current directory is fine for `m13b_fixture` commands; they use `M13B_ROOT`.

```bash
m13b_fixture init portable
m13b_fixture finalize portable --expect 409 --code artifact_missing
m13b_fixture upload portable
m13b_fixture finalize portable
cp "$M13B_ROOT/portable/receipt.json" "$M13B_ROOT/portable/first-receipt.json"
m13b_fixture finalize portable
cmp "$M13B_ROOT/portable/first-receipt.json" "$M13B_ROOT/portable/receipt.json"
m13b_fixture storage portable
m13b_fixture sql portable
```

Expected: init has `resumed: false`, a 24-hour reservation, a PUT expiry at most
15 minutes away, and both 1 GB caps. Finalize with nothing uploaded fails. After
the valid PUT, finalize returns 200 and repeating it gives the identical receipt;
`cmp` is silent. The storage check reads the actual final artifact, verifies its
size/hash and S3 SHA-256 checksum, and compares stored `release.json` with the
canonical local bytes.

The `sql` command **prints read-only queries**; it does not run them. Copy those
queries into your **development database SQL console**. They are scoped to this
case's exact reservation UUID. Expected: one finalized reservation, one version
with `release_format=agentpm.package.release.v1`, null legacy SHA/size/key fields,
one artifact and one linked scan. The scan can already be running or complete.
SBOM/Grype/pip-audit/OSV columns remain null. Its artifact parent and scan version
IDs must match. Canonical/final objects are under
`tools/<namespace>/<unique-name>/0.1.0/`, not bucket-root `packages/`.

## 4. Four Artifacts, Partial Upload, Corruption and Renewal

```bash
m13b_fixture init matrix
cp "$M13B_ROOT/matrix/init.json" "$M13B_ROOT/matrix/first-init.json"
m13b_fixture init matrix
m13b_fixture init matrix --request changed-inventory.json --expect 409 --code publish_in_progress
m13b_fixture upload matrix --target aarch64-apple-darwin
m13b_fixture finalize matrix --expect 409 --code artifact_missing
m13b_fixture sql matrix
```

Expected: repeating init reports `resumed: true` and preserves all four artifact
IDs plus the one release ID; the helper asserts that. Changed inventory cannot
replace the pending release. Only one uploaded sibling is insufficient to publish.
The printed SQL should show **no version or artifact-scan rows yet** for `0.1.1`.
An earlier validated final object may exist as retry state after a failed finalize;
its existence alone is not publication.

This is the interrupted-matrix case: one job uploaded, others did not. It does
not pretend that a tiny PUT was interrupted in the middle of its TCP stream;
the controlled Rust HTTP fixture in section 8 checks actual transport retry/reopen.

```bash
m13b_fixture upload matrix --target x86_64-apple-darwin
m13b_fixture upload matrix --target x86_64-pc-windows-msvc --corrupt
m13b_fixture upload matrix --target x86_64-unknown-linux-gnu
m13b_fixture finalize matrix --expect 422 --code invalid_artifact
m13b_fixture sql matrix
m13b_fixture renew matrix --target x86_64-pc-windows-msvc
m13b_fixture upload matrix --target x86_64-pc-windows-msvc
m13b_fixture finalize matrix
m13b_fixture finalize matrix
m13b_fixture storage matrix
m13b_fixture sql matrix
```

Expected: corrupt bytes keep the declared size/metadata but fail independent
stored-byte hashing; no partial version survives. Renewal keeps the same Windows
artifact ID and release; it does not invalidate the other siblings. Re-uploading
the correct Windows archive allows one successful logical version with **four**
artifacts and **four** correctly associated scan rows. Retry uses exact existing
final objects safely; repeated finalize returns the same receipt without extra rows.
Final storage checks pass for all four targets and the canonical document.

If an ordinary PUT URL expires while you pause, run `renew` for that target,
then rerun its `upload`. Do not start a different release or rebuild its archives.
There is no need to wait 15 minutes to exercise renewal.

If this sequence previously reached `409 / release_conflict` with the message
`Release changed concurrently; retry`, the pre-V1.104 database may still have
V1.55's unconditional `uniq_tool_version_scan_one` index. That index incorrectly
rejects the second artifact scan. Restart the local backend, confirm V1.104
applies, and rerun `m13b_fixture finalize matrix`, then `storage matrix` and
`sql matrix` using the same terminal/reservation. No new init or upload is needed
while the reservation is valid. Existing final objects are verified and reused;
do not remove immutable objects or edit migration checksums. The new backend log
includes constraint names if a different integrity conflict remains.

## 5. Dependency-Bearing `any`, Size Guidance and Immutable Inventory

```bash
m13b_fixture init dependencies
m13b_fixture upload dependencies
m13b_fixture finalize dependencies
m13b_fixture storage dependencies
m13b_fixture sql dependencies

m13b_fixture init portable --request oversized.json --expect 413 --code too_large
m13b_fixture init matrix --request excess-total.json --expect 413 --code release_too_large
m13b_fixture init matrix --request changed-inventory.json --expect 409 --code version_exists
```

Expected: dependency-bearing `0.1.2` has one `any` artifact and a non-null Python
resolution. The archive includes its real generated `agentpm-python.json` and
the added target descriptor, not installed distributions. It is a new-format
release even though it has only one portable artifact.

The single-file limit produces `too_large`; four declared 300 MB artifacts
produce `release_too_large`, retaining the numeric total/cap and suggesting
`agent.json`/`runtime.dependencies` instead of bundling dependencies. These errors
occur before creating new reservations or transferring large files. Both 1 GB
defaults are deliberate; individual checking gives the clearer single-file error.
The changed finalized inventory returns `version_exists` with an instruction to
use a new semantic version; existing final keys and receipts remain unchanged.

## 6. Legacy Publication Still Works; New-Format Consumption Stays Gated

Publish the separate tiny legacy fixture through the **normal CLI**:

```bash
cd "$M13B_ROOT/legacy"
m13b publish --namespace "$M13B_NAMESPACE" --schema "$M13B_SCHEMA"
export M13B_NAME="$(m13b_python -c 'import json,os; from pathlib import Path; print(json.loads((Path(os.environ["M13B_ROOT"])/"session.json").read_text())["name"])')"
mkdir -p "$M13B_ROOT/consumer-legacy" "$M13B_ROOT/consumer-new"
cd "$M13B_ROOT/consumer-legacy"
m13b install "@$M13B_NAMESPACE/$M13B_NAME-legacy@0.1.3"
m13b run "@$M13B_NAMESPACE/$M13B_NAME-legacy@0.1.3" --input '{}'

cd "$M13B_ROOT/consumer-new"
if m13b install "@$M13B_NAMESPACE/$M13B_NAME@0.1.0"; then M13B_EXIT=0; else M13B_EXIT=$?; fi
printf 'New-format install exit: %s (expected nonzero: unsupported release format)\n' "$M13B_EXIT"
test "$M13B_EXIT" -ne 0
test ! -e agent.lock
m13b_fixture security-gate portable
```

Expected: the normal CLI publishes the legacy format, installs it and runs it,
returning `{"ok":true}`. The new-format install fails clearly before writing a
consumer lock; its Security endpoint returns 409 because release verification/
target-aware consumption are later work. A public new-format Security page is
**not** an acceptance requirement here, and the gate must not be bypassed.

Check an **existing legacy package** in your local website as well: detail,
README, search and Security should still load normally. No external SDK changes
are needed for this walkthrough.

## 7. Real GuardDuty Tags, Scan Rows and Staging Cleanup

These commands read **only `agentpm-dev-tools`**. Start checking tags after the
publications above; continue section 8 while AWS scanning/the scheduler run.

```bash
m13b_fixture tags portable
m13b_fixture tags matrix
m13b_fixture tags dependencies
m13b_fixture sql matrix
```

Expected eventually: `NO_THREATS_FOUND` on **every** final artifact. The protected
prefix also covers `release.json`; inspect its tag too, although there is no
artifact scan row for that metadata document. `not tagged yet` is pending, not a
pass or proof of permanent failure. Do not add tags yourself. The helper prints
each key so AWS inspection can target the exact object.

Use the printed SQL again in your dev DB console. The poller runs every five
minutes and has a two-minute initial grace, so tag arrival and DB completion are
not simultaneous; allow roughly two polling intervals before investigating.
All four matrix scans should become `status=complete`/`malware_status=clean`.
The shared rollup's mixed/missing/infected states are exercised in section 8;
do not edit real scan rows or use live malware to manufacture those cases.

If a tag remains absent, inspect actual dev protection-prefix coverage, storage/
tagging permissions and scanner health. If tags exist but DB rows remain running,
inspect the scheduled worker and its credentials. Missing-tag timeout is 30
minutes with an explicit warning; its fast deterministic check is in section 8.
Do not mark real scan acceptance passed on the basis of fixture tests alone.

After a five-minute cleanup interval, check staging removal and final preservation:

```bash
m13b_fixture staging-cleaned portable
m13b_fixture staging-cleaned matrix
m13b_fixture staging-cleaned dependencies
```

Expected: all reservation-owned `uploads/<release-id>/<artifact-id>.tar.gz` keys
are absent, while final artifacts and `release.json` still pass byte/checksum
verification. A still-existing staging key means wait/check the cleanup job;
AccessDenied is an error, not treated as absence. Finalized reservation history
is retained. Do not delete final objects or resolver binaries to tidy this test.

This records live **dev** evidence only. Production coverage, retention/permissions,
instance concurrency/request capacity and scan cost sizing remain separate
Release Band 9 gates.

## 8. Controlled Failures in a Disposable Database

Docker Desktop must be running. Stay in the same terminal; this database is
separate from both your development Registry and production. The test variable
retains the existing `M13A` name because the fixture is shared, not because this
is testing an older implementation.

```bash
cd "$M13B_API"
export M13B_CONTAINER="agentpm-m13b-manual-$(date +%s)-$$"
docker run -d --rm --name "$M13B_CONTAINER" \
  -e POSTGRES_DB=agentpm_m13a -e POSTGRES_PASSWORD=m13b-fixture-only \
  -p 127.0.0.1::5432 pgvector/pgvector:pg16
for m13b_attempt in {1..30}; do
  if docker exec "$M13B_CONTAINER" pg_isready -U postgres -d agentpm_m13a >/dev/null 2>&1; then break; fi
  sleep 1
done
docker exec "$M13B_CONTAINER" pg_isready -U postgres -d agentpm_m13a
export M13B_DB_PORT="$(docker port "$M13B_CONTAINER" 5432/tcp | sed 's/.*://')"
export AGENTPM_M13A_TEST_DATABASE_URL="postgresql+psycopg://postgres:m13b-fixture-only@127.0.0.1:$M13B_DB_PORT/agentpm_m13a"
m13b_python -m pytest -v \
  tests/test_release_publish.py \
  tests/test_release_migration.py \
  tests/test_migration_order.py \
  tests/test_malware_rollup.py
```

Expected: **80 cases pass with no skips**. These use actual forward Pyway SQL
in randomly named disposable schemas, not ORM schema generation. They inspect:

- Missing/corrupt uploads and no surviving published rows on failure.
- Expiry rejection, idempotent cleanup/retry, safe history pruning, and preserving
  final objects even when another reservation reuses their deterministic keys.
- Exact orphan-document recovery and rejection of different document bytes.
- Aggregate limits, one snapshot at a time, no active DB transaction/row lock
  during storage calls, reauthorization and expiry/inventory/budget changes
  between verification and commit, and concurrent identical finalize.
- Required S3 checksum responses, existing-object hash verification, and
  correct/different conditional-write race winners.
- Scan/artifact parent constraints, legacy null associations, all-clean coverage,
  missing/queued/running/failed siblings, infected-artifact version yanking, and
  a missing-tag timeout with a useful warning emitted once.
- The real historical scan table/index migrations, V1.104's scoped uniqueness,
  and retrying the same four-artifact reservation after the old index caused
  an HTTP conflict and complete database rollback.
- Default-off and scope checks. No SBOM/vulnerability data is fabricated.

These are fixture-backed regressions, not claims about real AWS failure injection.
For streaming HTTP retry, reopening the same file and server-limit rejection:

```bash
cd "$M13B_REPO"
cargo test -p agentpm-sdk --test release_upload -- --nocapture
```

Expected: **2 tests pass**. The sparse oversized-file case rejects before transfer;
it does not upload gigabytes. The HTTP fixture also checks Registry authorization
is absent from storage requests. Native CI already ran these fixtures on all
four targets; this local rerun is an inspectable controlled check.

Stop the disposable database after inspection:

```bash
docker stop "$M13B_CONTAINER"
unset AGENTPM_M13A_TEST_DATABASE_URL M13B_DB_PORT
```

## 9. Restore the Gate and Record Acceptance

Finish any pending real tag/DB/cleanup checks from section 7. Then set the local
IntelliJ backend's **`RELEASE_PUBLISH_ENABLED=false`** again and restart it.
Verify from the same terminal before clearing the PAT:

```bash
m13b_fixture init portable --expect 404 --code release_publish_disabled
unset M13B_TOKEN
printf 'Retained manual evidence and short-lived PUT instructions: %s\n' "$M13B_ROOT"
```

Keep your case documents, receipts, relevant SQL results, scan tags and the two
checkout commit IDs until results are recorded. Report sections passed and any
unexpected errors; no checkbox closes until you confirm acceptance. Small real
dev versions/final objects remain as immutable test history. No production gate,
cross-repo main-ref parity, signing/verification feature or later milestone is
marked complete by this guide.
