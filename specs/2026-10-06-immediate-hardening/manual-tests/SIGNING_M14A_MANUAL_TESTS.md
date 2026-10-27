# Milestone 14A: Canonical Signing Foundation

Run **one block at a time in one new macOS/zsh terminal**, starting in the
`agentpm` repository. Keep this terminal open; every section uses its variables.
Only the last section needs your existing local development Registry running.
Restart it on the reviewed code before the signed publish. No new server or
second test terminal is needed.

You will inspect the exact bytes both languages agree on, see that harmless
JSON reordering preserves a release digest, see meaningful changes alter it,
and check that existing signing still works through a real development publish.

| Section | What you verify                                                                            |
| ------- | ------------------------------------------------------------------------------------------ |
| 1       | Build the reviewed CLI and prepare a disposable workspace.                                 |
| 2       | Inspect shared canonical bytes, Unicode, timestamps and Python resolution ordering.        |
| 3       | Inspect one/four-artifact release digests and deliberate mutations.                        |
| 4       | Verify fixed legacy signatures and the actual Rust/Python implementations.                 |
| 5       | Publish one signed legacy Tool to dev, inspect its accepted signature, install and run it. |
| 6       | Record acceptance and finish the session.                                                  |

Sections 1–4 require no Registry, Docker, PAT, AWS credentials or network service
apart from ordinary Cargo dependency fetching if your build cache is cold.
Section 5 intentionally creates **one development package version and dev S3
artifact**. It uses a dev PAT and your encrypted local signing key. It needs no
Python resolver download, dependency resolution, paid model call or production
write. Keep `RELEASE_PUBLISH_ENABLED=false` in production; no flag change is
needed for this legacy publish.

Normal CI, PostgreSQL CI, all Resolver Contract target jobs and independent
review are already operator-confirmed passed. Registry parity against public
CLI `main` remains a **Release Band 10** gate after the CLI merge. The local
comparison below checks these two working checkouts and can run now.

## 1. Prepare Paths and Build the Reviewed CLI

Start in the **`agentpm` repository root**. Use the sibling Registry's existing
Python 3.11+ venv with its test requirements installed.

```bash
umask 077
export M14A_REPO="$PWD"
export M14A_API="$M14A_REPO/../agentpm-api"
export M14A_PYTHON="$M14A_API/.venv/bin/python"
export M14A_APM="$M14A_REPO/target/debug/agentpm"
export M14A_SCHEMA="$M14A_REPO/schemas/agentpm.manifest.schema.json"
export M14A_FIXTURES="$M14A_REPO/specs/2026-10-06-immediate-hardening/manual-tests/fixtures/m14a"
export M14A_ROOT="$(mktemp -d "${TMPDIR:-/tmp}/agentpm-m14a.XXXXXX")"
export M14A_BASE='http://127.0.0.1:5000'
mkdir -p "$M14A_ROOT/evidence" "$M14A_ROOT/author" "$M14A_ROOT/consumer"
unset M14A_TOKEN
m14a_python() {
  env PYTHONPATH="$M14A_API" PYTHONDONTWRITEBYTECODE=1 \
    REGISTRY_PRIVATE_KEY_B64=AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA= \
    AWS_EC2_METADATA_DISABLED=true "$M14A_PYTHON" -B "$@"
}
m14a() {
  if [[ "$M14A_BASE" != 'http://127.0.0.1:5000' ]]; then
    printf 'This guide targets the local dev Registry at http://127.0.0.1:5000.\n' >&2
    return 1
  fi
  env AGENTPM_TELEMETRY=0 AGENTPM_TOKEN="${M14A_TOKEN:-apm_m14a.fixture}" \
    "$M14A_APM" --base-url "$M14A_BASE" "$@"
}
m14a_python - <<'PY'
import sys
import nacl.signing, pytest, rfc8785
assert sys.version_info >= (3, 11)
print('Python:', sys.version.split()[0])
print('Required test imports are available.')
PY
cargo build --locked -p agentpm-cli
m14a --version
git -C "$M14A_REPO" rev-parse HEAD
git -C "$M14A_API" rev-parse HEAD
m14a_python "$M14A_REPO/scripts/check_release_contract.py" --registry-root "$M14A_API"
printf 'Evidence/workspaces: %s\n' "$M14A_ROOT"
```

Expected: build/imports succeed and release/signing snapshots agree. The CLI is
the **local debug build**, even if its version number matches your installed CLI.
Commit IDs identify checkout bases; note any uncommitted changes separately.

The Python wrapper's dummy signing key affects only fixture/test imports. It
does not configure your running Registry. Neither wrapper changes `HOME`: the
real signed smoke test needs your existing encrypted local keystore. Your saved
CLI token is not overwritten; the dev token is supplied per invocation.

If an import is missing, install the existing requirements and repeat the check:

```bash
uv pip install --python "$M14A_PYTHON" -r "$M14A_API/requirements-test.txt"
```

## 2. Inspect Canonical Bytes and Normalization

Stay in the **`agentpm` repository**. The inputs and expected bytes are already
checked in under `crates/agentpm-sdk/contracts/`; this does not regenerate them.

```bash
m14a_python - <<'PY'
import hashlib, json, os
from pathlib import Path
from app.tools.domain.tools.canonical import canonical_bytes, contract_bytes, normalize_timestamp
from app.tools.domain.tools.python_resolution import resolution_canonical_bytes, resolution_digest

root = Path(os.environ['M14A_ROOT']) / 'evidence'
contracts = Path(os.environ['M14A_REPO']) / 'crates/agentpm-sdk/contracts'
vectors = json.loads((contracts / 'signing-canonicalization-v1.fixtures.json').read_text())
for vector in vectors['jcs']:
    value = json.loads(vector['inputJson'])
    data = canonical_bytes(value)
    assert data == vector['canonical'].encode()
    assert hashlib.sha256(data).hexdigest() == vector['sha256']
    assert canonical_bytes(json.loads(json.dumps(value, indent=4))) == data
    (root / (vector['name'] + '.canonical.json')).write_bytes(data)
    print('PASS:', vector['name'], 'SHA-256:', vector['sha256'])
    if vector['name'] == 'utf16-key-order':
        print('  Ordered keys:', [repr(key) for key in json.loads(data)])
for vector in vectors['timestamps']:
    actual = normalize_timestamp(vector['input'])
    assert actual == vector['normalized']
    print('Timestamp:', vector['input'], '->', actual)
for value in vectors['invalidTimestamps']:
    try:
        normalize_timestamp(value)
    except ValueError:
        print('Expected timestamp rejection:', value)
    else:
        raise AssertionError('Invalid timestamp accepted: ' + value)
for value in vectors['invalidContracts']:
    try:
        contract_bytes(json.loads(value))
    except ValueError:
        print('Expected contract-number rejection:', value)
    else:
        raise AssertionError('Invalid contract number accepted: ' + value)
vector = vectors['pythonResolution']
assert resolution_canonical_bytes(vector['input']) == vector['canonical'].encode()
assert resolution_digest(vector['input']) == resolution_digest(vector['normalized']) == vector['sha256']
print('Normalized Python resolution:', vector['canonical'])
print('Resolution SHA-256:', vector['sha256'])
PY
```

Expected: every line passes, deliberately invalid values are rejected, timestamps
end in `Z`, and Python requirements/packages/hashes have deterministic order.
The Unicode case includes an emoji key and a control-character key: these are
the cases the previous Rust library mishandled. Non-ASCII values remain intact;
canonicalization does not rewrite composed/decomposed Unicode into one spelling.

The saved `.canonical.json` files contain the exact bytes being hashed. Adding
display indentation would create different file bytes. The shared numeric
example exercises ordinary JCS; the separate contract checks reject floating
point values in release/signature/key-set contracts. Historical timestamps are
not passed through the new timestamp normalizer.

## 3. Inspect Whole-Release Integrity and Deliberate Changes

```bash
m14a_python - <<'PY'
import copy, hashlib, json, os
from pathlib import Path
from app.tools.domain.tools.releases import ReleaseManifest

contracts = Path(os.environ['M14A_REPO']) / 'crates/agentpm-sdk/contracts'
root = Path(os.environ['M14A_ROOT']) / 'evidence'
vectors = json.loads((contracts / 'package-release-v1.fixtures.json').read_text())['vectors']
for vector in vectors:
    release = ReleaseManifest.model_validate(vector['release'])
    data = release.canonical_bytes()
    assert data == vector['canonical'].encode()
    assert release.digest() == vector['digest']
    assert release.digest_hex() == hashlib.sha256(data).hexdigest()
    assert len(release.digest_hex()) == 64
    reordered = copy.deepcopy(vector['release'])
    reordered['artifacts'].reverse()
    assert ReleaseManifest.model_validate(reordered).digest() == release.digest()
    document = json.loads(data)
    assert ('pythonResolutionDigest' in document) == (vector['pythonResolution'] is not None)
    (root / (vector['name'] + '.release.canonical.json')).write_bytes(data)
    (root / (vector['name'] + '.release.readable.json')).write_text(json.dumps(document, indent=2) + '\n')
    print(vector['name'], 'targets:', [item.target for item in release.artifacts])
    print('  Release integrity for agent.lock:', release.digest_hex())
    changes = [('name', '@other/example'), ('version', '2.0.0'), ('manifestDigest', 'sha256:' + 'e' * 64), ('pythonResolutionDigest', 'sha256:' + 'f' * 64)]
    for field, replacement in changes:
        changed = copy.deepcopy(vector['release'])
        changed[field] = replacement
        assert ReleaseManifest.model_validate(changed).digest() != release.digest()
        print('  Changed digest as expected:', field)
    changed = copy.deepcopy(vector['release'])
    changed['artifacts'][0]['size'] += 1
    assert ReleaseManifest.model_validate(changed).digest() != release.digest()
    print('  Changed digest as expected: artifact size')
    if len(release.artifacts) > 1:
        changed = copy.deepcopy(vector['release'])
        changed['artifacts'].pop()
        assert ReleaseManifest.model_validate(changed).digest() != release.digest()
        print('  Changed digest as expected: removed target')
PY
cat "$M14A_ROOT/evidence/four-targets.release.readable.json"
```

Expected: `portable` has one `any` artifact; `four-targets` has ARM Mac, Intel
Mac, Windows x86_64 and Linux x86_64 artifacts within **one logical version**.
Reordering the artifact input preserves its release digest. The deliberate
content changes alter it. The lock spelling is bare lowercase hex; the release
document uses `sha256:<hex>`. No selected local artifact is added to the lock.

These artifact hashes/sizes are contract fixtures, not real uploaded objects.
You have not published anything in this section. `release.json` is generated
release metadata; it does not rewrite the authored `agent.json` key order.

## 4. Verify Legacy Bytes and Both Language Implementations

First inspect the fixed legacy author v1 and Registry attestation v2 examples.
These fixtures use public deterministic test-key material, not your real key.

```bash
m14a_python - <<'PY'
import base64, json, os
from pathlib import Path
from nacl.exceptions import BadSignatureError
from nacl.signing import VerifyKey
from app.tools.domain.tools.legacy_signing import legacy_statement_bytes

path = Path(os.environ['M14A_REPO']) / 'crates/agentpm-sdk/contracts/signing-canonicalization-v1.fixtures.json'
for vector in json.loads(path.read_text())['legacy']:
    data = legacy_statement_bytes(vector['statement'])
    assert data == vector['pythonCanonical'].encode()
    key = VerifyKey(base64.b64decode(vector['publicKeyB64']))
    signature = base64.b64decode(vector['pythonSignatureB64'])
    key.verify(data, signature)
    try:
        key.verify(data + b' ', signature)
    except BadSignatureError:
        print('PASS:', vector['name'], 'valid original; changed bytes rejected')
    else:
        raise AssertionError('Changed signed bytes accepted')
    print('  Type:', vector['statement']['type'])
    print('  Historical timestamp:', vector['statement'].get('createdAt', vector['statement'].get('storedAt')))
PY
cd "$M14A_REPO"
cargo test --locked -p agentpm-sdk canonical:: -- --nocapture
cargo test --locked -p agentpm-sdk signing:: -- --nocapture
cargo test --locked -p agentpm-sdk release:: -- --nocapture
cargo test --locked -p agentpm-cli --bin agentpm keys::signing::tests -- --nocapture
cargo test --locked -p agentpm-cli --bin agentpm release_integrity_roundtrips_as_portable_hex_without_selected_artifact_state -- --nocapture
cd "$M14A_API"
m14a_python -m pytest -v tests/test_signing_canonicalization.py tests/test_release_contract.py
cd "$M14A_REPO"
```

Expected: each Rust command's unit-test suite has matching passing tests (the
SDK commands may also print a separate integration suite with zero matches).
Rust proves the typed
author v1 statement produces the same bytes and deterministic Ed25519 signature
as the old inline statement. Both languages compare against the shared expected
bytes/digests, including the Unicode ordering fix. The lock regression preserves
release hex through plan → lock → frozen plan without selected target fields.

Legacy author v1/Registry v2 stay on their historical serializers. In particular,
Python's old ASCII escaping remains unchanged; new JCS must not replace it.
These fixtures prove compatibility locally, not the contents of a production row.

## 5. Real Signed Publish, Security Inspection and Consumer Run — Dev Only

Now use your **local Registry on port 5000**, running the reviewed backend code
with its normal development DB/bucket/signing key. Confirm it uses
`agentpm-dev-tools`. Keep production configuration untouched. No release-publish
flag change is needed, even if you enabled that flag for earlier dev testing.

Use an existing **public dev namespace** you may publish to. Choose a local key
whose public key is already registered and active in that namespace.

```bash
export M14A_NAMESPACE='zack'
m14a keys list
read 'M14A_KEY_ID?Local key ID registered in the dev namespace: '
export M14A_KEY_ID
read -s 'M14A_TOKEN?Development Registry PAT: '
printf '\n'
export M14A_TOKEN
m14a whoami
m14a keys export --key-id "$M14A_KEY_ID" --out "$M14A_ROOT/evidence/author-public-key.txt"
```

Expected: the correct dev account and an exported **public** key. Use the existing
key's passphrase at the publish prompt below; do not put it in an environment
variable or paste it into chat. If the key is not registered in dev, register
its exported public key through the existing namespace signer UI or this
standard command before continuing:

```bash
m14a namespace add-signer --namespace "$M14A_NAMESPACE" \
  --label 'M14A dev signing key' --pubkey "$M14A_ROOT/evidence/author-public-key.txt"
```

Only run that command when this public key is absent from the dev namespace;
an existing active registration needs no duplicate. Registration in production
does not register the key in dev. A successful publish alone does not prove a signature was accepted
under optional signing: the security inspection below is required.

Prepare the checked-in no-dependency Tool. Its explicit `files` list excludes
generated evidence and archives. The timestamp gives this session a new package
name; the Tool uses an ordinary stable `0.1.0` version.

```bash
cp "$M14A_FIXTURES/agent.fixture.json" "$M14A_ROOT/author/agent.json"
cp "$M14A_FIXTURES/tool.py" "$M14A_FIXTURES/README.md" "$M14A_ROOT/author/"
export M14A_NAME="m14a-signing-$(date -u +%Y%m%d%H%M%S)-$$"
export M14A_REF="@$M14A_NAMESPACE/$M14A_NAME@0.1.0"
m14a_python - <<'PY'
import json, os
from pathlib import Path
path = Path(os.environ['M14A_ROOT']) / 'author/agent.json'
manifest = json.loads(path.read_text())
manifest['name'] = os.environ['M14A_NAME']
path.write_text(json.dumps(manifest, indent=2) + '\n')
PY
cd "$M14A_ROOT/author"
m14a lint --schema "$M14A_SCHEMA"
m14a publish --dry-run --schema "$M14A_SCHEMA" --namespace "$M14A_NAMESPACE"
printf 'Next command publishes only to dev: %s via %s\n' "$M14A_REF" "$M14A_BASE"
```

Run this as its own command so the signing prompt has your terminal:

```bash
m14a publish --sign --key-id "$M14A_KEY_ID" --namespace "$M14A_NAMESPACE" --schema "$M14A_SCHEMA"
```

Expected: a passphrase prompt followed by `Published` and a package-page URL.
This is a legacy single-archive release; the `Tool payload: any` classification
line does not turn it into the new release format. No Python preparation or
`agent.lock` is required because the fixture declares no dependencies.

Copy the **UUID from the published package URL**, not the numeric `id:` receipt
line. The receipt currently uses the public website hostname even for a dev
publish; use it only to obtain the UUID here. The public website will not have
this dev package. The next block reads the existing public dev security API and verifies
the accepted author signature against the archive actually published.
The security read route is `/tools/<uuid>/<version>/security`; the `/v1` prefix
is used by publish/install endpoints, not this public detail endpoint.

```bash
read 'M14A_TOOL_ID?Package UUID from the published url: '
export M14A_TOOL_ID
m14a_python - <<'PY'
import base64, hashlib, json, os, uuid
from pathlib import Path
from urllib.request import HTTPRedirectHandler, build_opener
from nacl.signing import VerifyKey
from app.tools.domain.tools.legacy_signing import legacy_statement_bytes

class NoRedirect(HTTPRedirectHandler):
    def redirect_request(self, req, fp, code, msg, headers, newurl):
        return None

base = os.environ['M14A_BASE']
assert base == 'http://127.0.0.1:5000', 'Only the local dev Registry is allowed'
tool_id = str(uuid.UUID(os.environ['M14A_TOOL_ID'].strip()))
url = f'{base}/tools/{tool_id}/0.1.0/security'
with build_opener(NoRedirect()).open(url, timeout=30) as response:
    security = json.load(response)
root = Path(os.environ['M14A_ROOT'])
archives = list((root / 'author/target/agentpm').glob(f'{os.environ["M14A_NAME"]}-0.1.0-*.tar.gz'))
assert len(archives) == 1
digest = 'sha256:' + hashlib.sha256(archives[0].read_bytes()).hexdigest()
public_key = (root / 'evidence/author-public-key.txt').read_text().strip()
matched = [item for item in security['author_signatures'] if item['public_key_b64'] == public_key]
assert matched, 'No signature from the selected local key was accepted'
for item in matched:
    statement = item['statement_json']
    assert statement['type'] == 'agentpm.package.signature.v1'
    assert statement['kind'] == 'tool'
    assert statement['name'] == os.environ['M14A_NAME']
    assert statement['version'] == '0.1.0'
    assert statement['artifactDigest'] == digest
    VerifyKey(base64.b64decode(public_key)).verify(legacy_statement_bytes(statement), base64.b64decode(item['signature_b64']))
assert security['registry_attested'] is True
(root / 'evidence/dev-security.json').write_text(json.dumps(security, indent=2) + '\n')
print('PASS: accepted legacy author v1 signature verifies against the published archive.')
print('PASS: legacy Registry attestation is present.')
print('Scan:', security['scan_status'], security.get('scan_malware_status'))
PY
cd "$M14A_ROOT/consumer"
m14a install "$M14A_REF" --require-attestation
m14a run "$M14A_REF" --machine --input '{}'
```

Expected: security returns HTTP 200 with the verified author signature and a
present Registry attestation. Install succeeds and machine output reports
`status: success` with `output: {"ok": true}`. There are no declared dependencies
to provision. If GuardDuty is still scanning, wait for the normal dev scan to
finish and retry install; do not change scan/database rows to make it pass.

The current security API/`--require-attestation` exposes attestation **presence**;
it does not deliver full attestation cryptographic verification. That remains
later work. The fixed v2 fixtures in section 4 verify the historical serializer
cryptographically; the real dev check here verifies the actual accepted author
signature and preserves today's attestation/install behavior.

If the security API reports no accepted signature, check dev namespace key
registration first. The old optional-signing path can publish successfully
without storing a signature from an unregistered key. Adding a signer later
does not attach a signature to that immutable published version. After key
registration, repeat the preparation block to choose a fresh package name,
then lint/dry-run/sign/publish again. The archive inspection selects the current
package name, so archives from a previous attempt can stay in the scratch folder.
Use the new publication's UUID for security inspection. If it returns 409 `unsupported_release_format`, you selected
a prior new-format fixture instead of this session's legacy Tool. Do not treat
either as a passing compatibility check.

## 6. Record Acceptance

Keep `$M14A_ROOT` until acceptance is recorded. Record both checkout revisions,
any uncommitted changes, dev package UUID/ref, and the outcomes below. The saved
security file contains public signature metadata, not your PAT or private key.

- [x] Local release/signing snapshots agree.
- [x] Shared Unicode/numeric/key-order fixtures match expected canonical bytes.
- [x] New timestamps and Python arrays normalize; invalid cases fail as expected.
- [x] Release input reordering preserves its digest; meaningful mutations change it.
- [x] Fixed legacy signatures and focused Rust/Python checks pass.
- [x] One real dev publish accepts the selected author v1 signature and it verifies against the archive.
- [x] Registry attestation remains present; normal install and machine run succeed.
- [x] Production release-publish flag remains disabled; no production package was published.

The operator confirmed all manual tests passed after the dev signer registration
and fresh signed publication. These checks record that confirmation; runbook
rehearsal alone did not close acceptance. Hosted Registry parity against public CLI `main` stays
unchecked under Release Band 10 until that coordinated merge/check is done.

Finish by clearing the task's PAT and wrapper functions, or close this terminal:

```bash
cd "$M14A_REPO"
unset M14A_TOKEN
unfunction m14a m14a_python
```

The dev package and its ordinary scan/install history remain as test evidence.
No package deletion, signer revocation, policy change or S3 cleanup is needed.
