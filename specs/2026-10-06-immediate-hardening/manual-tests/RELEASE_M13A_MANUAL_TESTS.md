# Milestone 13A: Release and Artifact Data Foundation

Run **one block at a time in one new macOS/zsh terminal**, starting in the
`agentpm` repository. Keep that terminal open: later commands use its variables
and functions. Each section says which repository to enter. No second terminal
is needed for the commands; keep your existing local backend/web processes
running for the final browser check.

At the end, you should have inspected one portable release document, one
four-target release document, the migrated database shape, and a logical version
with four artifact rows. Existing packages must still open and search normally.

M13A exposes internal primitives, **not a new publish command**. This guide uses
checked-in contract examples, focused tests of those primitives, and a disposable
PostgreSQL database. Example digests and sizes are fixture values, not claims
about real uploaded artifacts. The S3 client used below is a mock.

| Section | What you do                                                                      |
| ------- | -------------------------------------------------------------------------------- |
| 1       | Set paths and check local prerequisites.                                         |
| 2       | Inspect canonical release documents and deliberate validation failures.          |
| 3       | Exercise the Rust artifact helper and Registry compatibility checks.             |
| 4       | Apply the real migrations to a disposable database and inspect legacy data.      |
| 5       | Record and inspect a four-artifact version; check immutability and DB backstops. |
| 6       | Check existing package pages/search against the running local services.          |
| 7       | Remove the disposable database and record operator acceptance.                   |

No PAT, signing passphrase, AWS credentials, live S3 write, real publication,
production database access, paid model call or PostHog setup is required.
Normal CI, all four updated Resolver Contract runs, and independent review are
already operator-confirmed passed. Hosted parity against CLI `main`, deployment
rehearsal and environment-specific S3 permissions/retention remain **Release
Band 9** gates after M13B; they are not steps in this walkthrough.

## 1. Prepare Paths and Check Prerequisites

Start from the **`agentpm` repository root**. Docker Desktop must be running.
Use the sibling Registry's existing Python 3.11+ venv with its test dependencies.

```bash
export M13A_REPO="$PWD"
export M13A_API="$M13A_REPO/../agentpm-api"
export M13A_PYTHON="$M13A_API/.venv/bin/python"
export M13A_ROOT="$(mktemp -d "${TMPDIR:-/tmp}/agentpm-m13a.XXXXXX")"
export M13A_CONTAINER="agentpm-m13a-manual-$(date +%s)-$$"
mkdir -p "$M13A_ROOT/documents" "$M13A_ROOT/migrations"
m13a_python() {
  env PYTHONPATH="$M13A_API" PYTHONDONTWRITEBYTECODE=1 \
    REGISTRY_PRIVATE_KEY_B64=AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA= \
    AWS_EC2_METADATA_DISABLED=true "$M13A_PYTHON" -B "$@"
}
m13a_python - <<'PY'
import sys
import jsonschema, psycopg, pytest, rfc8785, sqlalchemy
assert sys.version_info >= (3, 11)
print('Python:', sys.version.split()[0])
print('Registry test dependencies are available.')
PY
git -C "$M13A_REPO" rev-parse HEAD
git -C "$M13A_API" rev-parse HEAD
docker info --format '{{.ServerVersion}}'
printf 'Keep this directory until acceptance is recorded: %s\n' "$M13A_ROOT"
```

Expected: the imports succeed and Docker responds. The dummy signing key applies
only to these test processes; your saved credentials/env files are unchanged.
Commit IDs identify checkout bases; note any uncommitted changes separately.

If an import is missing, install the existing test requirements, then rerun the
dependency check:

```bash
uv pip install --python "$M13A_PYTHON" -r "$M13A_API/requirements-test.txt"
```

## 2. Inspect the Release Documents and Validation Rules

Stay in the **`agentpm` repository**. First compare the two working checkouts;
this command does not consult GitHub `main`.

```bash
cd "$M13A_REPO"
m13a_python scripts/check_release_contract.py --registry-root "$M13A_API"
m13a_python - <<'PY'
import copy, json, os
from pathlib import Path
from app.tools.domain.tools.releases import ReleaseManifest, manifest_digest
from app.tools.domain.tools.release_storage import validate_release_inputs

repo = Path(os.environ['M13A_REPO'])
root = Path(os.environ['M13A_ROOT'])
vectors = json.loads((repo / 'crates/agentpm-sdk/contracts/package-release-v1.fixtures.json').read_text())['vectors']
for vector in vectors:
    release = validate_release_inputs(vector['release'], vector['manifest'], vector['pythonResolution'])
    assert release.canonical_bytes() == vector['canonical'].encode()
    assert release.digest() == vector['digest']
    assert manifest_digest(vector['manifest']) == release.manifestDigest
    reordered = copy.deepcopy(vector['release'])
    reordered['artifacts'].reverse()
    assert ReleaseManifest.model_validate(reordered).digest() == release.digest()
    document = json.loads(release.canonical_bytes())
    assert ('pythonResolutionDigest' in document) == (vector['pythonResolution'] is not None)
    (root / 'documents' / f"{vector['name']}.canonical.json").write_bytes(release.canonical_bytes())
    (root / 'documents' / f"{vector['name']}.readable.json").write_text(json.dumps(document, indent=2) + '\n')
    print(vector['name'], release.name, release.version)
    print('  Targets:', ', '.join(a.target for a in release.artifacts))
    print('  Release digest:', release.digest())
    print('  Python resolution bound:', 'pythonResolutionDigest' in document)

duplicate = copy.deepcopy(vectors[1]['release'])
duplicate['artifacts'][1] = copy.deepcopy(duplicate['artifacts'][0])
try:
    ReleaseManifest.model_validate(duplicate)
except ValueError as error:
    assert 'duplicate release target' in str(error)
    print('Expected rejection: duplicate release target.')
else:
    raise AssertionError('Duplicate target was accepted')

changed = {**vectors[1]['manifest'], 'description': 'changed without updating release'}
try:
    validate_release_inputs(vectors[1]['release'], changed, vectors[1]['pythonResolution'])
except ValueError as error:
    assert 'manifest digest mismatch' in str(error)
    print('Expected rejection: changed manifest does not match its digest.')
else:
    raise AssertionError('Changed manifest was accepted')
PY
cat "$M13A_ROOT/documents/four-targets.readable.json"
```

Expected:

- Cross-repo schema/vector comparison succeeds.
- `portable` has one `any` artifact and omits `pythonResolutionDigest`.
- `four-targets` has four artifacts: ARM Mac, Intel Mac, Windows x86_64 and
  Linux x86_64. They belong to **one** `@fixture/example@1.2.0` release.
- Each artifact has its own digest/size; the logical release has a separate
  digest. Reversing input artifact order does not change that release digest.
- Both deliberate invalid cases report their expected rejection.
- The document contains no upload IDs, temporary keys, scan state or paths from
  your machine. The `.readable.json` file is for inspection; cryptographic bytes
  are in `.canonical.json`, not the indented display copy.

## 3. Exercise the Internal Artifact and Compatibility Primitives

There is no public command for producing/publishing this new release format yet.
Use the existing focused tests to exercise the actual Rust helper and Registry
validation rather than inventing a temporary user-facing workflow.

```bash
cd "$M13A_REPO"
cargo test -p agentpm-sdk release:: -- --nocapture
cargo test -p agentpm-cli --bin agentpm commands::publish::release_artifact -- --nocapture
cd "$M13A_API"
m13a_python -m pytest -v tests/test_release_contract.py tests/test_migration_order.py
```

Expected: **4 Rust SDK tests**, **1 Rust artifact-helper test**, and **30 Python
cases** pass. A Rust command reporting zero matching tests is not a pass for
this step. These are targeted reruns of existing checks, not a replacement for
the already-passed native CI matrix.

The artifact test checks that the new archive has root `agent.json` and
`agentpm-artifact.json`, while the original legacy archive stays unchanged.
Attempting to reuse an archive that already contains the reserved descriptor
fails without replacing either valid archive. Registry cases also cover the
optional `agentpm-python.json`, mismatched metadata, immutable document retries,
and rejection of new-format install/security reads.

That rejection is expected in M13A: **409 / `unsupported_release_format`** on
install paths, before creating an install session or download URL. Existing
packages are not converted to this format. No new public upload/finalize route
is exercised here.

## 4. Migrate a Disposable Database and Inspect Legacy Preservation

Stay in the **`agentpm-api` repository**. The commands below start a separate
container on a dynamically allocated localhost port. They do not use your
application's database URL, existing Docker database or Pyway history.

```bash
cd "$M13A_API"
docker run -d --rm --name "$M13A_CONTAINER" \
  -e POSTGRES_DB=agentpm_m13a -e POSTGRES_PASSWORD=m13a-fixture-only \
  -p 127.0.0.1::5432 pgvector/pgvector:pg16
for m13a_attempt in {1..30}; do
  if docker exec "$M13A_CONTAINER" pg_isready -U postgres -d agentpm_m13a >/dev/null 2>&1; then break; fi
  sleep 1
done
docker exec "$M13A_CONTAINER" pg_isready -U postgres -d agentpm_m13a
export M13A_DB_PORT="$(docker port "$M13A_CONTAINER" 5432/tcp | sed 's/.*://')"
export AGENTPM_M13A_TEST_DATABASE_URL="postgresql+psycopg://postgres:m13a-fixture-only@127.0.0.1:$M13A_DB_PORT/agentpm_m13a"
m13a_sql() {
  docker exec -i -e PGOPTIONS='-c search_path=m13a_manual' "$M13A_CONTAINER" \
    psql -X -v ON_ERROR_STOP=1 -P pager=off -U postgres -d agentpm_m13a
}
m13a_python - <<'PY'
import json, os, runpy, shutil, zlib
from pathlib import Path
from sqlalchemy import create_engine, text

api = Path(os.environ['M13A_API'])
root = Path(os.environ['M13A_ROOT'])
engine = create_engine(os.environ['AGENTPM_M13A_TEST_DATABASE_URL'])
assert engine.url.host == '127.0.0.1' and engine.url.database == 'agentpm_m13a'
# Reuse the existing representative pre-13A SQL fixture; do not generate ORM DDL.
baseline = runpy.run_path(str(api / 'tests/test_release_migration.py'))['BASELINE']
with engine.begin() as connection:
    connection.exec_driver_sql('CREATE SCHEMA m13a_manual')
    connection.exec_driver_sql('SET LOCAL search_path=m13a_manual')
    connection.exec_driver_sql(baseline)
    legacy = dict(connection.execute(text('SELECT * FROM tool_versions')).mappings().one())
(root / 'legacy-before.json').write_text(json.dumps(legacy, default=str))

names = [
    'V1.98__Add_Package_Release_And_Artifact_Model.sql',
    'V1.99__Harden_Package_Release_Inventory.sql',
    'V1.100__Allow_Expired_Release_Artifact_Key_Reuse.sql',
    'V1.101__Validate_Release_Inventory_On_Artifact_Mutations.sql',
]
for name in names:
    source = api / 'app/migrations' / name
    print(name, 'CRC32:', f'{zlib.crc32(source.read_bytes()):08X}')
    shutil.copyfile(source, root / 'migrations' / name)
config = {
    'database_migration_dir': str(root / 'migrations'),
    'database_table': 'pyway', 'database_type': 'postgres',
    'database_host': engine.url.host, 'database_port': engine.url.port,
    'database_name': engine.url.database, 'database_username': engine.url.username,
    'database_password': engine.url.password,
}
(root / 'pyway.json').write_text(json.dumps(config))
engine.dispose()
print('Prepared a legacy row and four real migration files in the disposable database.')
PY
PGOPTIONS='-c search_path=m13a_manual' m13a_python "$M13A_API/run_migrations.py" --config "$M13A_ROOT/pyway.json" migrate
PGOPTIONS='-c search_path=m13a_manual' m13a_python "$M13A_API/run_migrations.py" --config "$M13A_ROOT/pyway.json" migrate
m13a_sql <<'SQL'
SELECT * FROM pyway ORDER BY installed_rank;
SELECT table_name FROM information_schema.tables
WHERE table_schema='m13a_manual' ORDER BY table_name;
SELECT tool_version_id, version, release_format, sha_256, size_bytes, s3_key
FROM tool_versions ORDER BY tool_version_id;
SQL
```

Expected: migrations apply in numeric order **98 → 99 → 100 → 101**; the second
run has nothing new to apply and succeeds. There are four Pyway history rows
and all three new tables. The existing `0.1.0` row remains legacy: null
`release_format`, its original SHA, size `42`, and `tools/...` key.

The first three printed CRC32 values should remain `EAF4B273`, `8FB2F27D` and
`3484543A`. Do not edit old migration files or database checksum records to
resolve a mismatch. This is a representative upgrade of the affected tables,
not a rehearsal of every historical migration or a production deployment.

## 5. Inspect One Version with Four Artifacts and Check the Backstops

Stay in **`agentpm-api`**. Call the internal persistence helper using the
disposable database and a **mock** S3 client. No payload is uploaded or verified;
this demonstrates metadata persistence, not successful release finalization.

```bash
m13a_python - <<'PY'
import json, os, uuid
from pathlib import Path
from unittest.mock import Mock
from sqlalchemy import create_engine, text
from sqlalchemy.orm import Session
from app.tools.domain.tools.entities import PackageVersion
from app.tools.domain.tools.release_storage import persist_release_metadata

root = Path(os.environ['M13A_ROOT'])
api = Path(os.environ['M13A_API'])
vector = json.loads((api / 'app/tools/domain/tools/contracts/package-release-v1.fixtures.json').read_text())['vectors'][1]
engine = create_engine(os.environ['AGENTPM_M13A_TEST_DATABASE_URL'], connect_args={'options': '-c search_path=m13a_manual'})
assert engine.url.host == '127.0.0.1' and engine.url.database == 'agentpm_m13a'
before = json.loads((root / 'legacy-before.json').read_text())
with engine.connect() as connection:
    after = dict(connection.execute(text('SELECT * FROM tool_versions WHERE version=\'0.1.0\'')).mappings().one())
after = json.loads(json.dumps(after, default=str))
assert {key: after[key] for key in before} == before
print('Every original legacy column is unchanged.')

storage = Mock()
with Session(engine) as session:
    version = PackageVersion(
        package_id=uuid.UUID('00000000-0000-0000-0000-000000000001'),
        version=vector['release']['version'], manifest=vector['manifest'],
        description='disposable manual fixture', python_resolution=vector['pythonResolution'],
    )
    persist_release_metadata(session, version, vector['release'], 'fixture', storage, 'mock-only')
    session.commit()
    assert version.release_sha_256 == vector['digest'][7:]
request = storage.put_object.call_args.kwargs
assert request['IfNoneMatch'] == '*'
assert request['Body'] == vector['canonical'].encode()
(root / 'documents/recorded-release.json').write_bytes(request['Body'])
print('Mock object key:', request['Key'])
print('Release digest:', vector['digest'])
print('Stored the mock document locally; no S3 request occurred.')
engine.dispose()
PY
m13a_sql <<'SQL'
SELECT tool_version_id, version, status, release_format, release_sha_256,
       sha_256, size_bytes, s3_key
FROM tool_versions ORDER BY tool_version_id;
SELECT package_version_id, target, sha256, size_bytes, s3_key
FROM package_version_artifacts ORDER BY target;
SELECT jsonb_pretty(release_manifest) FROM tool_versions WHERE version='1.2.0';
SQL
cmp "$M13A_ROOT/documents/four-targets.canonical.json" "$M13A_ROOT/documents/recorded-release.json"
```

Expected: two logical version rows total (legacy `0.1.0`, new fixture `1.2.0`),
not one version per target. Only the new row has release metadata and null legacy
SHA/size/key fields. It has four child artifacts, each with a full digest and
target-specific `<S3_PREFIX>/fixture/example/1.2.0/artifacts/...` key
(default `tools/fixture/example/1.2.0/artifacts/...` with the later prefix fix). `cmp` prints
nothing and exits successfully. The fixture's `published` status comes from
the internal metadata primitive; it is not a real Registry publication.

Try changing a finalized artifact:

```bash
if m13a_sql <<'SQL'
BEGIN;
UPDATE package_version_artifacts SET size_bytes=size_bytes+1;
COMMIT;
SQL
then M13A_EXIT=0; else M13A_EXIT=$?; fi
printf 'Mutation exit: %s (expected nonzero: immutable artifact)\n' "$M13A_EXIT"
m13a_sql <<'SQL'
SELECT target, size_bytes FROM package_version_artifacts ORDER BY target;
SQL
m13a_python -m pytest -v tests/test_release_migration.py
```

Expected: the UPDATE reports an immutability error and the connection rolls
back. All four sizes remain `12345`. The **13 PostgreSQL cases** pass, with no
skips. They use additional random schemas in this same disposable database and
remove those schemas afterward; the `m13a_manual` schema remains for inspection.

Those cases also exercise committed-pending INSERT/UPDATE/DELETE mismatches,
deferred commit rejection, valid temporary mismatches, both sides of artifact
reparenting, duplicate targets, expired-reservation final-key reuse, legacy
preservation and the real Pyway subprocess upgrade.

The backstop proves **manifest/artifact consistency**, not upload completeness.
There is no reservation-to-version FK/trigger proving all bytes were uploaded.
M13B still owns upload verification, expiry enforcement, cleanup and recovery
if a create-only `release.json` survives a rolled-back DB transaction. An exact
document can be retried; different bytes cannot overwrite that immutable key.

## 6. Confirm Existing Local Package Pages and Search Still Work

Your regular **local backend and web** should be running. These are read-only
checks against the development Registry, separate from the disposable database.
Use `127.0.0.1:5000`; `localhost:5000` can reach AirPlay's IPv6 listener.

```bash
m13a_python - <<'PY'
import json
from urllib.parse import quote
from urllib.request import ProxyHandler, build_opener

base = 'http://127.0.0.1:5000'
opener = build_opener(ProxyHandler({}))
def read(path):
    with opener.open(base + path, timeout=15) as response:
        assert response.status == 200
        print('200', path)
        return json.load(response)

def find_package(value):
    if not isinstance(value, (dict, list)):
        return None
    if isinstance(value, dict) and value.get('tool_id') and value.get('version'):
        return value
    for item in (value.values() if isinstance(value, dict) else value):
        result = find_package(item)
        if result:
            return result
    return None

trending = read('/stats/trending')
read('/search?q=summarize')
package = find_package(trending)
assert package is not None, 'Choose an existing dev package manually if trending is empty.'
path = '/tools/' + quote(str(package['tool_id']), safe='') + '/' + quote(package['version'], safe='')
detail = read(path)
print('Existing package:', detail['name'], detail['version'])
print('Now check package navigation and search in http://localhost:3000.')
PY
```

In the browser, open `http://localhost:3000` and:

1. Click an existing package in Trending; its detail page must open.
2. Search for an existing development package; results must load and open.
3. Open its README and Security information; legacy metadata must still render.

No new-format fixture from section 5 should appear on this website: it exists
only in the separate disposable database. There is no requirement to publish
a real new-format version or make the site install one during M13A.

## 7. Stop the Disposable Database and Record Acceptance

After you are finished inspecting it:

```bash
docker stop "$M13A_CONTAINER"
unset AGENTPM_M13A_TEST_DATABASE_URL M13A_DB_PORT
printf 'Saved documents and Pyway fixture config: %s\n' "$M13A_ROOT"
```

The container was created with `--rm`, so stopping it removes this test database.
The temporary directory remains for your inspection; it contains only fixture
documents/config and a fixture database password. No real credentials were copied.

Report which sections passed and any unexpected output. Keep M13A's manual
acceptance checkbox open until you confirm the walkthrough. Record separately
that Release Band 9 still needs hosted CLI-main parity and deployment/storage
readiness; do not check those off based on this guide.
