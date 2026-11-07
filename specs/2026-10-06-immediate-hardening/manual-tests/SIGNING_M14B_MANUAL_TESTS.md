# Milestone 14B: Release Signatures and Stored-Byte Integrity

Run **one block at a time in one new macOS/zsh terminal**, starting in the
`agentpm` repository. Keep that terminal open. Your local Registry may keep
running in IntelliJ; configuration changes below belong to that local run
configuration, not Render.

Review, all CLI CI jobs and the Registry's **Release uploads and migrations**
job have passed. Passing Registry **Release Contract Parity** against public
CLI `main` remains a **Release Band 11** gate after the coordinated merge;
it does not block this walkthrough.

| Section | What you verify |
| --- | --- |
| 1 | Prepare source, real archives, disposable signing keys and saved requests. |
| 2 | Confirm local/dev settings, PAT, read-only DB access and Registry public key. |
| 3 | Reject bad explicit signatures, then accept one signed portable release. |
| 4 | Bind one signature to four targets; reject authentic cross-release replay and corrupted bytes. |
| 5 | Revoke only a disposable signer; preserve historical signatures and reject new use. |
| 6 | Publish a dependency-bearing release with a different active signer. |
| 7 | Check ordinary CLI legacy signing, install and run. |
| 8 | Exercise configuration/persistence/policy failures in disposable PostgreSQL. |
| 9 | Restore the local gate, revoke the second disposable signer and record acceptance. |

The checked-in [`fixture.py`](fixtures/m14b/fixture.py) reuses M13B's validated
archive packaging and dev-only transport. Each `init`, `upload` or `finalize`
command performs just that action. Preparation/signing are local operations.
The helper rejects Registry requests outside loopback and PUTs outside HTTPS
**`agentpm-dev-tools`**. Its database connection is loopback-only and explicitly
read-only. It never edits DB rows, namespace policy, backend config or scan tags.

This creates small real development releases, S3 objects, scan rows and **two
temporary dev signer registrations**. Their private seeds stay in the private
scratch directory for this fixture only. It does not use them as a new CLI
keystore or headless signing feature. Your usual encrypted CLI key is used only
for the separate legacy smoke test. Production stays disabled and untouched.

New-format publishing still has no public CLI command, Security tab or installer
support. The helper exercises the gated API and verifies saved signing rows
directly. Four target descriptors test inventory handling, **not native builds
on four computers**. Full consumer trust/verification and the new Security UI
remain M14C; target-aware installation remains M15.

## 1. Build and Prepare the Local Fixtures

Start in the **`agentpm` repository root**. The API venv needs its existing test
dependencies and Python 3.11+. No AWS credentials are needed in this command
terminal: uploads later use URLs signed by your local Registry.

```bash
umask 077
export M14B_REPO="$PWD"
export M14B_API="$M14B_REPO/../agentpm-api"
export M14B_PYTHON="$M14B_API/.venv/bin/python"
export M14B_APM="$M14B_REPO/target/debug/agentpm"
export M14B_SCHEMA="$M14B_REPO/schemas/agentpm.manifest.schema.json"
export M14B_FIXTURES="$M14B_REPO/specs/2026-10-06-immediate-hardening/manual-tests/fixtures/m14b"
export M14B_ROOT="$(mktemp -d "${TMPDIR:-/tmp}/agentpm-m14b.XXXXXX")"
export M14B_BASE='http://127.0.0.1:5000'
export M14B_NAMESPACE='zack'
unset M14B_TOKEN M14B_DATABASE_URL
m14b_python() {
  env PYTHONPATH="$M14B_API" PYTHONDONTWRITEBYTECODE=1 \
    REGISTRY_PRIVATE_KEY_B64=AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA= \
    AWS_EC2_METADATA_DISABLED=true "$M14B_PYTHON" -B "$@"
}
m14b() {
  if [[ "$M14B_BASE" != 'http://127.0.0.1:5000' ]]; then
    printf 'This walkthrough uses only http://127.0.0.1:5000.\n' >&2
    return 1
  fi
  env AGENTPM_TELEMETRY=0 AGENTPM_TOKEN="${M14B_TOKEN:-apm_m14b.fixture}" \
    "$M14B_APM" --base-url "$M14B_BASE" "$@"
}
m14b_fixture() {
  local m14b_action="$1"
  shift
  m14b_python "$M14B_FIXTURES/fixture.py" "$m14b_action" "$M14B_ROOT" "$@"
}
m14b_python - <<'PY'
import sys
import nacl.signing, psycopg, pytest, rfc8785
assert sys.version_info >= (3, 11)
print('Python and existing test dependencies are available.')
PY
cargo build --locked -p agentpm-cli
m14b --version
m14b_python "$M14B_REPO/scripts/check_release_contract.py" --registry-root "$M14B_API"
m14b_fixture prepare --namespace "$M14B_NAMESPACE"
git -C "$M14B_REPO" rev-parse HEAD
git -C "$M14B_API" rev-parse HEAD
printf 'Keep this terminal; scratch/evidence directory: %s\n' "$M14B_ROOT"
```

Choose another development namespace **before** running `prepare` if needed.
Use a public namespace you can manage and publish to; retain its existing signing
mode. The controlled tests compare `off`, `optional` and `required` without changing
your namespace's policy. The Python wrapper's dummy key is only for imports/controlled tests;
it does not set the key used by the running backend. Neither wrapper changes
your usual HOME, saved login, encrypted keystore or AWS setup.

The generated cases share one unique package name: `portable` is `0.1.0`,
`matrix` is `0.1.1`, and `dependencies` is `0.1.2`. `legacy` is a separate
package at `0.1.3`. Each source directory has its own `agent.json`, `tool.py`
and README; the explicit `files` list does not include generated requests,
signature evidence or archives.

Build the actual archives with the CLI, then add release descriptors and sign
the requests locally. The `package` action is test scaffolding, not a new step
in the ordinary publishing workflow.

Prepare and sign **portable first**:

```bash
cd "$M14B_ROOT/portable"
m14b lint --schema "$M14B_SCHEMA"
m14b publish --dry-run --schema "$M14B_SCHEMA"
m14b_fixture package portable
m14b_fixture sign portable
```

Then build/package **matrix separately** before signing it. A portable dry run
does not create the matrix request:

```bash
cd "$M14B_ROOT/matrix"
m14b lint --schema "$M14B_SCHEMA"
m14b publish --dry-run --schema "$M14B_SCHEMA"
m14b_fixture package matrix
m14b_fixture sign matrix
```

Next prepare the dependency-bearing case:

```bash
cd "$M14B_ROOT/dependencies"
m14b lint --schema "$M14B_SCHEMA"
m14b publish --dry-run --refresh-python --schema "$M14B_SCHEMA"
m14b_fixture package dependencies
m14b_fixture sign dependencies
```

Finally prepare the legacy case:

```bash
cd "$M14B_ROOT/legacy"
m14b lint --schema "$M14B_SCHEMA"
m14b publish --dry-run --schema "$M14B_SCHEMA"
cd "$M14B_REPO"
cat "$M14B_ROOT/matrix/finalize-primary.json"
```

Expected: real dependency resolution succeeds against your dev resolver mirror;
each `package` reports its targets and whole-release digest. The matrix has
**four artifacts in one version**. The saved author v2 statement includes a
qualified name, `releaseDigest` and normalized `createdAt` ending in `Z`.
The signed timestamp is saved once; rerunning `sign` preserves the exact envelope.
Do not rebuild a case after `init` or replace its saved signature between retries.

No Registry reservation, S3 PUT or publication has occurred yet. The real local
backend must already be running to serve the dependency resolver mirror.

## 2. Confirm Local/Dev Settings and Register the First Disposable Signer

In **IntelliJ's local backend run configuration**, confirm:

- Development DB and **`S3_BUCKET=agentpm-dev-tools`**, `S3_PREFIX=tools`.
- Normal dev AWS credentials, scheduler and GuardDuty configuration.
- Reviewed M14B code and migrations through **V1.104**. M14B adds no migration.
- Existing valid `REGISTRY_KEY_ID` and `REGISTRY_PRIVATE_KEY_B64`.
- Set **only this local backend** to `RELEASE_PUBLISH_ENABLED=true`, then restart.

Leave `RELEASE_PUBLISH_ENABLED=false` in Render/production. Exporting this flag
in the command terminal would not update an already-running IntelliJ backend.
Do not replace its Registry signing key with the wrapper's dummy fixture key.

Back in the **same command terminal**, enter the dev PAT and the connection URL
for that backend's local development PostgreSQL database. The URL is normally
`postgresql://USER:PASSWORD@127.0.0.1:PORT/DATABASE`; the SQLAlchemy
`postgresql+psycopg://` spelling is accepted too. Use the real local DB here,
not the disposable DB from section 8. The helper refuses remote DB hosts and
sets every inspection connection read-only.

For the default **`FLASK_ENV=local`** profile and checked-in Docker Compose,
use `postgresql://postgres:postgres@127.0.0.1:5433/postgres`. The application
connection is set in `agentpm-api/app/config.py`; do not omit port **5433** or
choose another database just because PostgreSQL accepts the connection.
If your backend uses another profile/override, match its actual application
connection instead. A missing signer table in the inspection connection is not
a reason to create tables or repeat registration in a different database.

```bash
read -s 'M14B_TOKEN?Development publish PAT: '
printf '\n'
export M14B_TOKEN
m14b whoami
read -s 'M14B_DATABASE_URL?Local DEVELOPMENT PostgreSQL connection URL: '
printf '\n'
export M14B_DATABASE_URL
m14b_fixture registry-public-key
```

The final command prompts for the **value of your local backend's existing
`REGISTRY_PRIVATE_KEY_B64`**, with hidden input. Copy it from the local run
configuration. It derives and saves only `registry-public-key.txt`; the private
value is not saved, printed or sent anywhere. This lets the diagnostic verify
real persisted Registry signatures without a consumer key endpoint, which is
not implemented until M14C. This is not a trust-root bootstrap or key rotation.

Register **only the primary** disposable public key now. Leave the secondary
unregistered until section 6 so the negative test is meaningful.

```bash
m14b namespace add-signer --namespace "$M14B_NAMESPACE" \
  --label 'M14B disposable primary' --pubkey "$M14B_ROOT/primary-public-key.txt"
export M14B_PRIMARY_ID="$(m14b_fixture signer-id --key primary)"
printf 'Disposable primary signer: %s\n' "$M14B_PRIMARY_ID"
```

Expected: registration succeeds and `signer-id` prints one UUID. If lookup fails,
check that the CLI and DB URL refer to the same dev Registry and namespace.
These commands do not register or change your ordinary production signing key.

## 3. Reject Explicit Signing Failures, Then Publish One Portable Release

`m14b_fixture` commands work from **any directory in this terminal**; they use
`M14B_ROOT`. Each rejection uses a prebuilt request file and states its expected
HTTP code. Run each command separately so a failure cannot be hidden by later output.

```bash
m14b_fixture init portable
m14b_fixture finalize portable --variant algorithm --expect 422 --code unsupported_signature_algorithm
m14b_fixture finalize portable --variant type --expect 422 --code unsupported_signature_type
m14b_fixture finalize portable --variant identity --expect 422 --code signature_identity_mismatch
m14b_fixture finalize portable --variant digest --expect 422 --code signature_identity_mismatch
m14b_fixture finalize portable --variant bad-signature --expect 422 --code invalid_signature
m14b_fixture finalize portable --variant unregistered --expect 422 --code unregistered_signer
m14b_fixture finalize portable --variant missing --expect 422 --code signature_required
m14b_fixture inspect portable --pending
```

Expected: all seven requests fail with their distinct actionable diagnostics,
including when namespace mode allows unsigned publication. None silently becomes an
unsigned success. Read-only inspection finds a pending reservation and no version
or version-linked signing rows. `missing` sets `require_signature=true`; namespace
mode does not override an explicit signing requirement.

Now use the unchanged valid envelope:

```bash
m14b_fixture finalize portable --expect 409 --code artifact_missing
m14b_fixture upload portable
m14b_fixture finalize portable
cp "$M14B_ROOT/portable/receipt.json" "$M14B_ROOT/portable/first-receipt.json"
m14b_fixture finalize portable
cmp "$M14B_ROOT/portable/first-receipt.json" "$M14B_ROOT/portable/receipt.json"
m14b_fixture inspect portable
```

Expected: a valid signature alone does not replace stored-byte checks. After
upload, finalize returns `author_signatures_accepted: 1`; its retry has the same
receipt (`cmp` is silent). Inspection verifies the actual saved author v2 and
Registry v3 signatures, their whole-release binding and the artifact inventory,
then saves public `evidence.json`. Both rows attach to the logical version.

If verification says `BadSignatureError` only for Registry v3, check that section
2 derived the public key from the **running local backend's** key. Do not change
stored statements or substitute the dummy import key to make the check pass.

## 4. Four Targets, Authentic Replay and Corrupted Stored Bytes

```bash
m14b_fixture init matrix
m14b_fixture finalize matrix --variant replay --expect 422 --code signature_identity_mismatch
m14b_fixture inspect matrix --pending
m14b_fixture upload matrix --target aarch64-apple-darwin
m14b_fixture upload matrix --target x86_64-apple-darwin
m14b_fixture upload matrix --target x86_64-pc-windows-msvc --corrupt
m14b_fixture upload matrix --target x86_64-unknown-linux-gnu
m14b_fixture finalize matrix --expect 422 --code invalid_artifact
m14b_fixture inspect matrix --pending
```

The replay envelope is **honestly signed**, not a random/corrupted signature:
`replay-source-request.json` describes a valid one-target subset of this same
release. Name, version, manifest and the shared artifact bytes are unchanged,
but that release's inventory/digest differs. It must not authorize the four-target
release. The controlled cases in section 8 also cover Registry v3 replay.

The corrupted PUT preserves the declared length and publisher checksum metadata.
Its valid author signature must not let mismatching stored bytes finalize.
There may be immutable final objects from earlier verified siblings; without a
published DB version, those are retry state, not partial publication.

Recover using the original correct archive; no new signature is necessary:

```bash
m14b_fixture renew matrix --target x86_64-pc-windows-msvc
m14b_fixture upload matrix --target x86_64-pc-windows-msvc
m14b_fixture finalize matrix
m14b_fixture finalize matrix
m14b_fixture inspect matrix
```

Expected: **one** author v2 row and **one** Registry v3 row bind **four** artifact
rows. The original unchanged envelope succeeds after the correct bytes arrive.
If any PUT URL expires during a pause, use `renew <case> --target <target>` and
retry that upload, without rebuilding its archive or changing its release.

## 5. Ordinary Revocation Preserves Historical Evidence

Revoke **only** the UUID looked up for this walkthrough's disposable primary key:

```bash
m14b namespace revoke-signer --namespace "$M14B_NAMESPACE" --signer-id "$M14B_PRIMARY_ID"
m14b_fixture finalize portable
m14b_fixture inspect portable --revoked
m14b_fixture inspect matrix --revoked
m14b_fixture init dependencies
m14b_fixture finalize dependencies --expect 422 --code inactive_signer
m14b_fixture inspect dependencies --pending
```

Expected: the exact completed portable retry still succeeds. Previously accepted
author and Registry signatures still verify, while the author now has
`is_active=false` and a non-null `revoked_ts`. A newly attempted publication using
that signer is rejected. No historical signed statement is rewritten and no
retroactive compromise policy is being tested.

The current web schema does not retain the new revocation timestamp yet; that
update is tracked under M14C. New-format Security views still return 409 until
then. Read-only inspection, not a changed badge/UI, is the expected surface here.

## 6. Finish the Dependency-Bearing Release With an Active Signer

Now register the secondary key used by the earlier unregistered-signer test:

```bash
m14b namespace add-signer --namespace "$M14B_NAMESPACE" \
  --label 'M14B disposable secondary' --pubkey "$M14B_ROOT/secondary-public-key.txt"
export M14B_SECONDARY_ID="$(m14b_fixture signer-id --key secondary)"
m14b_fixture sign dependencies --key secondary
m14b_fixture upload dependencies
m14b_fixture finalize dependencies --variant secondary
m14b_fixture finalize dependencies --variant secondary
m14b_fixture inspect dependencies --key secondary
```

Expected: one `any` artifact, one accepted active author signature and one Registry
attestation. The logical release binds the real `pythonResolutionDigest`; installed
distributions are not embedded in the payload. The new signature is from a different
active key for an unpublished reservation, not a retroactive signature attachment.

## 7. Ordinary CLI Signing and Legacy Consumption

Use an existing encrypted local key whose public half is already registered and
active in your **development** namespace. This is your normal CLI key, **not**
either raw fixture seed. If needed, register its exported public half with the
ordinary `namespace add-signer` command first; production registration is separate.

```bash
m14b keys list
read 'M14B_LEGACY_KEY_ID?Existing local key ID registered in dev: '
export M14B_LEGACY_KEY_ID
m14b keys export --key-id "$M14B_LEGACY_KEY_ID" --out "$M14B_ROOT/legacy-public-key.txt"
cd "$M14B_ROOT/legacy"
m14b publish --sign --key-id "$M14B_LEGACY_KEY_ID" --namespace "$M14B_NAMESPACE" --schema "$M14B_SCHEMA"
```

Run the publish command by itself so the passphrase prompt uses your terminal.
Expected: `Published`, with no silent unsigned fallback. Copy the package **UUID
from its URL**, not the numeric receipt ID. The printed website hostname is not
the local website, so use the UUID with this local diagnostic:

```bash
read 'M14B_LEGACY_TOOL_ID?Legacy package UUID from the published URL: '
export M14B_LEGACY_TOOL_ID
m14b_fixture legacy-check --tool-id "$M14B_LEGACY_TOOL_ID" --public-key "$M14B_ROOT/legacy-public-key.txt"
mkdir -p "$M14B_ROOT/consumer"
cd "$M14B_ROOT/consumer"
export M14B_LEGACY_REF="@$M14B_NAMESPACE/$(m14b_python -c 'import json,os; from pathlib import Path; print(json.loads((Path(os.environ["M14B_ROOT"])/"legacy/agent.json").read_text())["name"])')@0.1.3"
m14b install "$M14B_LEGACY_REF" --require-attestation
m14b run "$M14B_LEGACY_REF" --machine --input '{}'
cd "$M14B_REPO"
```

Expected: the accepted historical author v1 signature verifies against the actual
published archive, and its display fingerprint matches the CLI's 16 characters.
Registry v2 attestation remains present. Normal install and machine run succeed
with `output: {"ok": true}`. If GuardDuty is still scanning, wait and retry
install normally; do not edit scan rows or tags. Existing `--require-attestation`
still checks presence, not independently trusted cryptography; that changes in 14C.

## 8. Controlled Failures Without Editing the Running Registry

The hosted PostgreSQL job passed already. These focused checks make the failure
boundaries reproducible locally without removing the running backend's key,
injecting DB errors into its real tables or changing namespace policy.

Docker must be running. This creates only the named disposable test container
with a dedicated database on port **18146**, separate from `M14B_DATABASE_URL`.
If that port is occupied, choose a free port in **both** commands using it.

```bash
export M14B_TEST_CONTAINER="agentpm-m14b-$$"
docker run --rm --name "$M14B_TEST_CONTAINER" \
  -e POSTGRES_PASSWORD=signing-fixture-only -e POSTGRES_DB=agentpm_m13a \
  -p 127.0.0.1:18146:5432 -d postgres:16-alpine
docker exec "$M14B_TEST_CONTAINER" pg_isready -U postgres -d agentpm_m13a
```

Repeat `pg_isready` if the new database is still starting. Once it reports
`accepting connections`:

```bash
cd "$M14B_API"
env AGENTPM_M13A_TEST_DATABASE_URL='postgresql+psycopg://postgres:signing-fixture-only@127.0.0.1:18146/agentpm_m13a' \
  PYTHONPATH="$M14B_API:$M14B_API/tests" PYTHONDONTWRITEBYTECODE=1 \
  REGISTRY_PRIVATE_KEY_B64=AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA= \
  "$M14B_PYTHON" -B -m pytest -v --tb=short \
  tests/test_release_signing.py tests/test_signing_statements.py tests/test_legacy_signing_publish.py \
  "$M14B_FIXTURES/test_fixture.py"
docker stop "$M14B_TEST_CONTAINER"
cd "$M14B_REPO"
cargo test --locked -p agentpm-sdk --test signing_upload -- --nocapture
```

Expected: **73 Python cases pass**. Coverage includes missing/invalid Registry
configuration, attestation insert failure with atomic rollback and retry,
authorization/configuration changes during storage, all signing modes with an
effective threshold of one despite `min_author_signatures=3`, authentic cross-release
author/Registry replay, and explicit rejected legacy signatures before copies or
inserts, plus the helper's transport intent, key handling, read-only database
guard and actual four-artifact signing-row inspection before/after revocation.
The Rust transport's **two tests** check required acceptance confirmation
and signed finalize transport, including an older response that omits acceptance.

These are controlled tests, not injected production failures. The test DB applies
actual checked-in migrations and signing DDL; the helper never generates schema
DDL. If a test fails, keep its output and stop the named container when finished.

## 9. Restore and Record Acceptance

Revoke the **secondary disposable signer only**. Keep your normal legacy CLI key
registered and active. The earlier primary is already revoked.

```bash
m14b namespace revoke-signer --namespace "$M14B_NAMESPACE" --signer-id "$M14B_SECONDARY_ID"
```

In **IntelliJ's local backend config**, restore `RELEASE_PUBLISH_ENABLED=false`
and restart. No Registry signing key or namespace policy was changed by this guide.
Production/Render must still have its release flag disabled.

Record the case refs/UUIDs, checkout revisions and any uncommitted changes along
with the saved public signing evidence under `$M14B_ROOT`. Keep the scratch folder
private: it also contains disposable private seeds and short-lived signed URLs.
Do not paste private seed files, PATs, DB URLs or signed PUT URLs into review notes.

- [x] Shared contract snapshots agree locally; actual archives/manifests pass CLI lint/dry-run.
- [x] Explicit bad signatures produce distinct errors and no unsigned publication.
- [x] Portable release stores exactly one author v2 and one Registry v3; both verify.
- [x] Four-target release rejects authentic replay and corrupt stored bytes, then recovers.
- [x] Four artifacts still have only one release-level author/Registry signature pair.
- [x] Ordinary revocation preserves historical crypto/exact retry and blocks new use.
- [x] Dependency-bearing `any` release binds its recorded resolution and succeeds with an active signer.
- [x] Real legacy CLI signing verifies; install and run remain compatible.
- [x] Controlled PostgreSQL failures/rollback/policy tests and Rust receipt checks pass.
- [x] Both disposable dev signers are revoked; local and production release gates remain disabled.

The operator confirmed all manual outcomes passed; these boxes record that
acceptance, separately from runbook rehearsal. Hosted contract parity against CLI `main`
remains separately unchecked under Release Band 11.

Finish by clearing the task's credentials/wrappers, or close this terminal:

```bash
cd "$M14B_REPO"
unset M14B_TOKEN M14B_DATABASE_URL
unfunction m14b m14b_python m14b_fixture
```

Development releases, ordinary scan history and revoked signer rows remain as
evidence. No package deletion, direct DB mutation or immutable S3 cleanup is needed.
