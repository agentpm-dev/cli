# Milestone 14C: Development Trust Bootstrap and Consumer Verification

Use **one new macOS/zsh terminal**, beginning in the `agentpm` repository.
Run one block at a time and keep that terminal open. Backend configuration changes
belong to your **local IntelliJ run configuration**, not Render.

Review passed without findings. CLI CI, all four Resolver Contracts, Registry
PostgreSQL and Web CI passed on the PR branches. Registry contract parity against
public CLI `main` remains a Release Band 11 gate after the coordinated merge.

| Section | Result |
| --- | --- |
| 1 | Disposable source/evidence, checked dependencies and a closed key endpoint. |
| 2 | A real encrypted dev root, backup and public pins in both repositories. |
| 3 | Verified backfill of the existing dev attestation key and an offline-signed public inventory. |
| 4 | Running dev endpoint and real Rust key delivery/cache verification. |
| 5 | An already-published legacy v2 attestation verified by the actual CLI. |
| 6 | Existing new-format proof, tamper rejection and historical signature status. |
| 7 | A newly signed legacy publish/install and Security page smoke test. |
| 8 | Controlled cache/policy regressions, retained bootstrap state and acceptance. |

This is **persistent development trust setup**, not just a disposable fixture.
Keep the root, encrypted backup, public pin and served public key-set file after
the test. The scratch source, consumer workspaces and test copies are disposable.
No command here sets up production, changes a namespace policy, retires a real
Registry key, uploads a root to S3, merges/pushes code or enables production
publishing. Do not paste a private key or passphrase into chat.

The checked-in [helper](fixtures/m14c/fixture.py) creates local fixtures, pins only
the public loopback root and inspects the dev database read-only. **Only its
explicit `seed` action writes DB rows**, using the reviewed key-backfill primitive;
it prompts for the existing local backend attestation key. It creates a minimal
Flask context without starting the application, migrations, ingestors or scheduler.
It never handles the root private file. The offline root tool handles that file
with hidden passphrase prompts. [consumer.rs](fixtures/m14c/consumer.rs) calls the
real Rust SDK verifier; it does not implement a second verifier or installer.

## 1. Prepare the Session and Confirm Prerequisites

```bash
umask 077
export M14C_REPO="$PWD"
export M14C_API="$M14C_REPO/../agentpm-api"
export M14C_PYTHON="$M14C_API/.venv/bin/python"
export M14C_APM="$M14C_REPO/target/debug/agentpm"
export M14C_SCHEMA="$M14C_REPO/schemas/agentpm.manifest.schema.json"
export M14C_FIXTURES="$M14C_REPO/specs/2026-10-06-immediate-hardening/manual-tests/fixtures/m14c"
export M14C_ROOT="$(mktemp -d "${TMPDIR:-/tmp}/agentpm-m14c.XXXXXX")"
export M14C_BASE='http://127.0.0.1:5000'
export M14C_NAMESPACE='zack'
export M14C_DATABASE_URL='postgresql://postgres:postgres@127.0.0.1:5433/postgres'
export M14C_OFFLINE_DIR="$HOME/.agentpm-offline/dev-registry"
export M14C_PUBLIC_DIR="$HOME/.agentpm-registry-public/dev"
unset M14C_TOKEN
m14c_python() {
  env PYTHONPATH="$M14C_API" PYTHONDONTWRITEBYTECODE=1 \
    REGISTRY_PRIVATE_KEY_B64=AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA= \
    AWS_EC2_METADATA_DISABLED=true "$M14C_PYTHON" -B "$@"
}
m14c_fixture() {
  local m14c_action="$1"
  shift
  m14c_python "$M14C_FIXTURES/fixture.py" "$m14c_action" "$M14C_ROOT" "$@"
}
m14c() {
  env AGENTPM_TELEMETRY=0 AGENTPM_TOKEN="${M14C_TOKEN:-apm_m14c.fixture}" \
    "$M14C_APM" --base-url "$M14C_BASE" "$@"
}
m14c_consumer() {
  env CARGO_TARGET_DIR="$M14C_REPO/target" \
    cargo run --offline --quiet --manifest-path "$M14C_ROOT/runner/Cargo.toml" -- "$@"
}
m14c_python - <<'PY'
import sys
import nacl.signing, psycopg, pytest, rfc8785, sqlalchemy
assert sys.version_info >= (3, 11)
print('Python and existing dependencies are available.')
PY
m14c_fixture prepare
printf 'Scratch/evidence: %s\nPersistent encrypted root: %s\nPersistent public files: %s\n' \
  "$M14C_ROOT" "$M14C_OFFLINE_DIR" "$M14C_PUBLIC_DIR"
git -C "$M14C_REPO" rev-parse HEAD
git -C "$M14C_API" rev-parse HEAD
```

The default database URL above is the **public local Docker Compose connection**
used by `FLASK_ENV=local`. If IntelliJ uses another database, change the URL to
match before continuing. The helper refuses remote DB hosts. Do not substitute
the disposable CI database or create missing tables in another database.

In IntelliJ confirm the reviewed code, migrations through **V1.105**, dev DB,
`S3_BUCKET=agentpm-dev-tools`, the **existing** `REGISTRY_KEY_ID` and
`REGISTRY_PRIVATE_KEY_B64`. Keep the local release flag at its current value;
this guide uses existing new-format releases and ordinary legacy publishing.
The Python wrapper's dummy key only permits module imports; it never changes the
running backend key or gets used for the actual backfill.

Before first bootstrap, leave `REGISTRY_SIGNED_KEYSET_PATH` unset in IntelliJ,
restart the local backend if needed, and run:

```bash
m14c_fixture keyset --expect 503
m14c_fixture catalogue
```

Expected: fail-closed **503**, followed by existing published dev Tools, UUIDs,
versions and attestation types. Save one **legacy / v2** and one **new-format /
v3** choice for sections 5–6. A previous M14A/M14B package is suitable.
If you already completed bootstrap on a previous attempt, do not remove a valid
key set just to force a 503; keep the valid setup and continue with section 4.

## 2. Generate, Back Up and Pin the Development Root

Use a long, unique passphrase in your password manager. The root private file is
encrypted and created with mode 0600; the tool refuses overwrites. Generate/sign
on your operator machine with networking disconnected if available. These offline
commands do not contact the Registry, database or AWS.

```bash
mkdir -p "$M14C_OFFLINE_DIR" "$M14C_PUBLIC_DIR"
chmod 700 "$M14C_OFFLINE_DIR" "$M14C_PUBLIC_DIR"
"$M14C_PYTHON" -B "$M14C_API/scripts/registry_root.py" generate-root \
  --private-key "$M14C_OFFLINE_DIR/root.encrypted.json" \
  --public-out "$M14C_PUBLIC_DIR/root-public.json" \
  --root-key-id agentpm-dev-root-1 --registry "$M14C_BASE"
```

Enter/repeat the passphrase locally. **Do not rerun generation if the files
already exist.** Resume using those established files. Keep an encrypted backup
on separate operator-controlled storage and the passphrase separately. Merely
leaving both files in a local directory is not an offline backup; confirm custody
and backup before recording this acceptance item.

Reconnect for subsequent Registry operations. Pin **only the public file**:

```bash
m14c_fixture pin --public "$M14C_PUBLIC_DIR/root-public.json" --api "$M14C_API"
git -C "$M14C_REPO" diff -- crates/agentpm-sdk/contracts/registry-trust-roots-v1.json
git -C "$M14C_API" diff -- app/tools/domain/tools/contracts/registry-trust-roots-v1.json
m14c_python "$M14C_REPO/scripts/check_release_contract.py" --registry-root "$M14C_API"
```

Expected: identical public dev pins for **exactly `http://127.0.0.1:5000`** in
both repos. Do not switch later commands to `localhost`, HTTPS or another port;
origins are part of the trust identity. The helper preserves other origins and
refuses replacing an established dev root. Review/commit these public files with
the branch changes later; no secret file belongs in either repo.

The release-only `check_registry_bootstrap.py --release` rejects **any dev pin**,
even after an official production root is added. Keep the dev pin for this guide
and M15 local testing. Before the Band 11 CLI release, remove dev/non-production
entries from **both audited manifest copies** and retain only the official
production root/ring. Review that source change rather than filtering at build
time. Keep the dev encrypted root, backup and public files; you can reapply its
public pin in local development copies later. Production bootstrap remains a
separate Band 11 gate; this guide does not remove or rotate your established pin.

## 3. Backfill the Existing Attestation Key and Sign the Public Inventory

Read the key ID from your **local IntelliJ run configuration**; its default may
be `apm-prod-1` even in dev. Retain that existing ID and key. This is backfill,
not key rotation, and does not rewrite package attestations.

```bash
read 'M14C_KEY_ID?Existing local backend REGISTRY_KEY_ID: '
export M14C_KEY_ID
export M14C_INVENTORY="$M14C_PUBLIC_DIR/inventory-$(date -u +%Y%m%dT%H%M%SZ).json"
m14c_fixture seed --key-id "$M14C_KEY_ID" --out "$M14C_INVENTORY"
```

The hidden prompt asks for the local backend's **existing attestation private
key**, not the new root and not its passphrase. It is held only in that process;
the output contains public keys only. The command verifies all saved v2/v3
attestations for the configured ID before registering its public half, refuses
incompatible key-ID reuse and exports the complete inventory. If another
historical key ID is missing, stop and reconcile that history; do not generate
a replacement key or delete old attestations.

Older dev rows may use `agentpm.registry.attestation.v1` or
`agentpm.package.attestation.v1`. The backfill also verifies those original
formats without rewriting them. If a previous attempt stopped with `unknown
historical attestation type`, use the updated code and retry the same `seed`
command; a failed attempt rolls back and does not create its output inventory.
Do not regenerate the root, delete historical rows or change their type strings.

On the offline operator machine, sign the public inventory. Use a new output
filename for each issuance; keep old signed documents as evidence:

```bash
export M14C_SIGNED="$M14C_PUBLIC_DIR/keyset-$(date -u +%Y%m%dT%H%M%SZ).json"
"$M14C_PYTHON" -B "$M14C_API/scripts/registry_root.py" sign-keyset \
  --private-key "$M14C_OFFLINE_DIR/root.encrypted.json" \
  --inventory "$M14C_INVENTORY" --out "$M14C_SIGNED" \
  --registry "$M14C_BASE" --days 30
printf 'Local backend public key-set path: %s\n' "$M14C_SIGNED"
```

Unlock with the root passphrase. Copy only the **signed public document** back
if signing on another machine. Keep the root disconnected/secure afterward.
The signed list must be renewed **before its 30-day expiry**; use the same root,
export the complete inventory again, sign a fresh document and update the served
path. A later expiry failure calls for renewal, not a new root.

## 4. Serve and Authenticate the Real Development Key Set

Set these **local IntelliJ** values, using the complete path printed above:

```text
REGISTRY_CANONICAL_ORIGIN=http://127.0.0.1:5000
REGISTRY_SIGNED_KEYSET_PATH=<complete value of M14C_SIGNED>
```

Restart the local backend. Its root private key is not configured anywhere.
Back in the **same command terminal**:

```bash
m14c_fixture keyset --out "$M14C_ROOT/evidence/dev-keyset.json"
cd "$M14C_REPO"
cargo build --locked -p agentpm-cli
m14c --version
m14c_consumer keys "$M14C_BASE" "$M14C_ROOT/key-cache"
```

Expected: endpoint **200**, valid root signature/origin/date, existing key ID
listed, and a PASS from the real Rust key-fetch/cache implementation. Rebuild
**after pinning**: the CLI embeds the public manifest at compile time. A stale
installed Homebrew binary does not pick up this dev pin.

If 503 persists, check the local backend's logs for root/bootstrap configuration:
exact origin, complete inventory, existing current key, expiry and file path.
Do not use the root secret as `REGISTRY_PRIVATE_KEY_B64`.

## 5. Verify an Already-Published Legacy Attestation

Choose the UUID and exact version of a **legacy v2** row from `catalogue`,
published before section 3. This is the old-catalogue regression, so do not
substitute a newly published test package. The proof export reads DB rows only:

```bash
read 'M14C_LEGACY_ID?Existing legacy Tool UUID from catalogue: '
read 'M14C_LEGACY_VERSION?Its exact version: '
m14c_fixture proof --tool-id "$M14C_LEGACY_ID" --version "$M14C_LEGACY_VERSION" \
  --out "$M14C_ROOT/evidence/legacy-proof.json"
m14c_consumer verify "$M14C_BASE" "$M14C_ROOT/evidence/legacy-proof.json" \
  "$M14C_ROOT/evidence/dev-keyset.json"
```

Expected: `agentpm.registry.attestation.v2` and a Rust PASS, without republishing
or altering the historical row. Next verify through the actual CLI HTTP install
path. Enter your dev PAT locally and use the qualified ref printed by `catalogue`:

```bash
read -s 'M14C_TOKEN?Development PAT: '
printf '\n'
export M14C_TOKEN
read 'M14C_LEGACY_REF?Same exact qualified ref, e.g. @zack/example@0.1.0: '
mkdir "$M14C_ROOT/consumer/legacy"
cd "$M14C_ROOT/consumer/legacy"
m14c install "$M14C_LEGACY_REF" --require-attestation
```

Expected: installed successfully with cryptographic attestation verification.
This install writes only in the scratch consumer (plus ordinary user caches and
the dev Registry's install session/counters); it does not use your example app.
If the selected legacy Tool contains incompatible old vendored native code,
installation/provenance is still the check here; choose a no-dependency test Tool
for execution. Requiring a signature is checked next with a known signed Tool.

## 6. New-Format Proof, Tampering and Historical Signers

Pick a previously finalized **v3 new-format** Tool from M14B, ideally the four-
artifact matrix whose disposable author was later revoked. It does not need to
be republished. New-format install stays gated until M15; here we verify the
existing release through the same actual Rust verifier and inspect its Web view.

```bash
read 'M14C_RELEASE_ID?Existing new-format Tool UUID from catalogue: '
read 'M14C_RELEASE_VERSION?Its exact version: '
m14c_fixture proof --tool-id "$M14C_RELEASE_ID" --version "$M14C_RELEASE_VERSION" \
  --out "$M14C_ROOT/evidence/release-proof.json"
m14c_consumer verify "$M14C_BASE" "$M14C_ROOT/evidence/release-proof.json" \
  "$M14C_ROOT/evidence/dev-keyset.json" --require-signature
m14c_fixture tamper --source "$M14C_ROOT/evidence/release-proof.json"
```

Expected: v3 and PASS for the complete release, full manifest/resolution binding
and accepted author signature, even if that author is currently revoked. If the
chosen release has no author signatures, select a signed M14B release instead.
If none exists, prepare/finalize only the needed signed portable/matrix case via
the M14B runbook using the dev backend; do not claim fixture vectors are live proof.

Run each rejection separately (the shell helper returns failure if a bad input
unexpectedly succeeds):

```bash
m14c_reject() {
  [[ -f "$1" && -f "$2" ]] || { printf 'Missing proof/key-set file\n' >&2; return 1; }
  local m14c_result
  if m14c_consumer verify "$M14C_BASE" "$1" "$2"; then
    printf 'FAIL: modified proof was accepted\n' >&2
    return 1
  else
    m14c_result=$?
    if [[ "$m14c_result" == 1 ]]; then
      printf 'PASS: modified proof rejected\n'
    else
      printf 'FAIL: fixture/build failed with exit %s\n' "$m14c_result" >&2
      return 1
    fi
  fi
}
m14c_reject "$M14C_ROOT/evidence/tampered-unknown-key.json" "$M14C_ROOT/evidence/dev-keyset.json"
m14c_reject "$M14C_ROOT/evidence/tampered-bad-attestation.json" "$M14C_ROOT/evidence/dev-keyset.json"
m14c_reject "$M14C_ROOT/evidence/tampered-manifest.json" "$M14C_ROOT/evidence/dev-keyset.json"
m14c_reject "$M14C_ROOT/evidence/tampered-target.json" "$M14C_ROOT/evidence/dev-keyset.json"
m14c_reject "$M14C_ROOT/evidence/tampered-required-unsigned.json" "$M14C_ROOT/evidence/dev-keyset.json"
m14c_reject "$M14C_ROOT/evidence/release-proof.json" "$M14C_ROOT/evidence/tampered-keyset.json"
m14c_consumer verify "$M14C_BASE" "$M14C_ROOT/evidence/release-proof.json" \
  "$M14C_ROOT/evidence/dev-keyset.json" --require-signature
```

Expected: six rejected copies; original still passes. Errors should name unknown
keys, signature/binding/root failures or required author signatures. A missing
file or compilation failure is **not** a valid tamper result; resolve that before
accepting the PASS line. These commands do not mutate the DB, live key set or cache.

Start/keep your normal local Web server and open:

```bash
printf 'New-format Security: http://localhost:3000/tools/%s/v%s/security\n' \
  "$M14C_RELEASE_ID" "$M14C_RELEASE_VERSION"
printf 'Legacy Security: http://localhost:3000/tools/%s/v%s/security\n' \
  "$M14C_LEGACY_ID" "$M14C_LEGACY_VERSION"
```

Verify full release SHA-256, actual Registry key ID, available artifacts with full
digests/size/scan state and historical author/current revocation separation. The
browser displays recorded evidence; it does not claim client-side verification.
Legacy still shows the old artifact digest and its signatures/license. Check a
narrow/mobile viewport for digest wrapping and artifact cards.

## 7. Signed Legacy Publish and Strict CLI Install

Use your **normal local encrypted CLI author key**, already registered as an
active signer in the dev namespace. Do not confuse this author key with the
Registry root or attestation key. If registration is needed, use the existing
`keys export` / `namespace add-signer` steps from M14A first.

```bash
read 'M14C_AUTHOR_KEY_ID?Your active CLI author key ID: '
cd "$M14C_ROOT/author"
m14c lint --schema "$M14C_SCHEMA"
m14c publish --sign --key-id "$M14C_AUTHOR_KEY_ID" --namespace "$M14C_NAMESPACE" \
  --schema "$M14C_SCHEMA"
export M14C_SIGNED_REF="@$M14C_NAMESPACE/$(m14c_python -c 'import json,os; from pathlib import Path; print(json.loads((Path(os.environ["M14C_ROOT"])/"session.json").read_text())["name"])')@0.1.0"
mkdir "$M14C_ROOT/consumer/signed"
cd "$M14C_ROOT/consumer/signed"
m14c install "$M14C_SIGNED_REF" --require-signature --require-attestation
m14c run "$M14C_SIGNED_REF" --input '{}'
```

Expected: accepted author signature, successful strict install and `{"ok":true}`.
Publishing is explicit here and creates one real dev legacy package and its
small dev S3 object. `files` enumerates only Tool source and README; none of the
bootstrap material enters the archive. No new-format CLI publication is enabled.

## 8. Controlled Regressions, Retention and Acceptance

These checked-in automated fixtures supplement the real dev checks. They bind
only temporary local mock servers and use generated test keys; they never change
the running Registry or operator root. The cache regression covers a bad 200
response, offline valid-cache reuse and expired/tampered cache rejection.

```bash
cd "$M14C_REPO"
cargo test --locked -p agentpm-sdk --test registry_key_cache
cargo test --locked -p agentpm-sdk --test install_provenance
cargo test --locked -p agentpm-sdk provenance::
cargo test --locked -p agentpm-cli --bin agentpm commands::install::provenance_tests
m14c_python "$M14C_REPO/scripts/check_release_contract.py" --registry-root "$M14C_API"
```

The ignored PostgreSQL handoff test is covered by the actual old-row consumer
check above; do not point it at the differently shaped `legacy-proof.json`.
Do not retire/change the active real Registry key to simulate rotation. Public
fixture tests cover retired-key acceptance and rejection after retirement.

Keep the root encrypted file, encrypted backup, passphrase custody, reviewed
public pins, complete public inventory and served signed public document. Keep
the local backend origin/path configured and note its renewal date. Preserve
scratch evidence until acceptance is recorded. No automatic cleanup deletes
trust material, package rows, signatures or S3 objects.

Tell Codex the sections passed and any failures. **Dev bootstrap/manual completion
does not mark production root generation/backfill/delivery or CLI-main contract
parity complete.** Those remain Band 11 gates. Do not create a CLI release tag
yet: the official production pin remains absent. Successful dev acceptance
unblocks M15's real target-aware installation work.
