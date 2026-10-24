# Milestone 11B: Python Locks, Publication and Consumer Metadata

Run one block at a time and inspect the expected result before continuing.
This guide is for macOS with zsh, the workspace layout used in M11A, and the
local/dev Registry at `http://127.0.0.1:5000`. It does not use a test runner.

What we are proving: prepare a Tool's Python resolution, review and commit it,
publish that exact resolution from a fresh checkout with an empty user cache,
then retain the same resolution in normal and frozen consumer installs.

| Sections | What changes |
| --- | --- |
| 1–6 | Disposable local files, caches and a local Git repository only. Resolution downloads the mirrored uv if needed and accesses PyPI. |
| 7 | Validate a real dev PAT; two deliberately invalid publish-init requests must be rejected without reservations or S3 writes. |
| 8 | Publish **one new dev Tool version**, creating Registry rows and dev S3 objects. |
| 9–10 | Download that Tool into disposable consumers; normal install-session/analytics bookkeeping may occur. |
| 11 | Local compatibility checks with the Registry address deliberately offline. |

Use a **new terminal**. Keep your normal local Registry and website running in
their existing terminals. No production publish, uv re-ingestion, release or
version bump is needed for this walkthrough. Do not reuse the historical M11A
instructions that expect publication to be blocked or resolution records under
the project's `target/` directory; both behaviors changed in 11B.

## Before Starting: Update the Local Registry

The local Registry must run the M11B API changes, including the review fixes:

- Install the updated API requirements using its normal environment workflow
  (`uv pip install --python .venv/bin/python -r requirements.txt`, from
  `agentpm-api`, if the venv needs updating).
- Restart the local backend through your usual IntelliJ configuration. Normal
  startup applies Pyway migration `V1.97__Persist_Python_Resolution.sql`.
- Confirm startup succeeds and the S3 bucket is **`agentpm-dev-tools`**. A healthy
  endpoint alone does not establish which bucket is configured.
- Use an existing **public dev namespace** you can publish into, with author
  signing optional/off for this unsigned fixture. Use the local website to
  obtain a dev PAT with `packages:publish` and the required namespace access.
  Keep the PAT private; section 7 uses the CLI's hidden paste prompt.

In your dev database console, this read-only query should return **two rows**,
both `jsonb`. A missing row means stop and fix startup/migration first; do not
manually alter the tables or run this migration against production here.

```sql
SELECT table_name, column_name, data_type
FROM information_schema.columns
WHERE table_schema = 'public'
  AND table_name IN ('uploads', 'tool_versions')
  AND column_name = 'python_resolution'
ORDER BY table_name;
```

## 1. Build and Create an Empty Test Area

Start from the **`agentpm` repository root**. Build using your normal environment:

```bash
cargo build -p agentpm-cli
export M11B_REPO="$PWD"
export M11B_APM="$M11B_REPO/target/debug/agentpm"
export M11B_PYTHON="$M11B_REPO/../agentpm-api/.venv/bin/python"
export M11B_SCHEMA="$M11B_REPO/schemas/agentpm.manifest.schema.json"
export M11B_FIXTURES="$M11B_REPO/specs/2026-10-06-immediate-hardening/manual-tests/fixtures/m11b"
export M11B_REGISTRY='http://127.0.0.1:5000'
export M11B_OFFLINE='http://127.0.0.1:1'
export M11B_ROOT="$(mktemp -d "${TMPDIR:-/tmp}/agentpm-m11b.XXXXXX")"
export M11B_WARM_STATE="$M11B_ROOT/warm-state"
export M11B_STATE="$M11B_WARM_STATE"
export M11B_NAME="m11b-python-check-$(date -u +%Y%m%d%H%M%S)"
mkdir "$M11B_STATE"
```

Define this small launcher in the same terminal. It only gives each CLI process
isolated user state; it does not set up fixtures or run checks automatically.
Your shell's actual HOME and normal login remain unchanged. Later sections can
select another empty user directory by changing **`M11B_STATE`**.

```bash
m11b() {
  env HOME="$M11B_STATE" USERPROFILE="$M11B_STATE" \
    XDG_CONFIG_HOME="$M11B_STATE/config" XDG_DATA_HOME="$M11B_STATE/data" \
    XDG_CACHE_HOME="$M11B_STATE/cache" APPDATA="$M11B_STATE/data" \
    PATH="$(dirname "$M11B_PYTHON"):$PATH" AGENTPM_TELEMETRY=0 \
    "$M11B_APM" "$@"
}
unset AGENTPM_TOKEN AGENTPM_UV_PATH
printf 'Keep this workspace: %s\nPackage name: %s\n' "$M11B_ROOT" "$M11B_NAME"
"$M11B_PYTHON" --version
m11b --version
"$M11B_PYTHON" -c 'import hashlib,os,pathlib; print("CLI SHA-256:", hashlib.sha256(pathlib.Path(os.environ["M11B_APM"]).read_bytes()).hexdigest())'
"$M11B_PYTHON" -c 'import dep_logic; print("Registry dependency available: dep_logic")'
curl --fail --show-error "$M11B_REGISTRY/health"
```

Expected: Python 3.11+, the rebuilt CLI, the new Registry dependency installed,
and a healthy local backend. Record the CLI digest: the displayed version may
still equal the installed release while the executable contains newer code.
Port 1 must be unused; otherwise select another closed loopback port for
`M11B_OFFLINE`. All lint/publish commands use the checked-out schema explicitly.

## 2. Missing Lock and Live Refresh Fail Before Acquisition

Create the author project and give it the unique package name now:

```bash
mkdir "$M11B_ROOT/author"
cd "$M11B_ROOT/author"
cp "$M11B_FIXTURES/tool.py" "$M11B_FIXTURES/README.md" .
cp "$M11B_FIXTURES/python-tool.fixture.json" agent.json
"$M11B_PYTHON" - <<'PY'
import json, os
from pathlib import Path
p = Path('agent.json')
value = json.loads(p.read_text())
value['name'] = os.environ['M11B_NAME']
p.write_text(json.dumps(value, indent=2) + '\n')
PY
m11b lint --strict --schema "$M11B_SCHEMA"
```

Expected: lint exits 0. There is no `agent.lock` yet. The fake token below is
only for local credential-presence checks; it cannot authenticate a live publish.

```bash
if AGENTPM_UV_PATH="$M11B_ROOT/does-not-exist" m11b --base-url "$M11B_OFFLINE" publish --token apm_m11b.fixture --schema "$M11B_SCHEMA"; then M11B_EXIT=0; else M11B_EXIT=$?; fi
printf 'CLI exit: %s (expected 1: missing/stale committed resolution)\n' "$M11B_EXIT"
```

Expected: `Python resolution in agent.lock is missing or stale`, with dry-run
preparation guidance. It must not report a bad uv override or a connection
failure. No resolver is acquired and no upload starts.

```bash
if AGENTPM_UV_PATH="$M11B_ROOT/does-not-exist" m11b --base-url "$M11B_OFFLINE" publish --refresh-python --token apm_m11b.fixture --schema "$M11B_SCHEMA"; then M11B_EXIT=0; else M11B_EXIT=$?; fi
printf 'CLI exit: %s (expected 1: refresh requires dry-run)\n' "$M11B_EXIT"
"$M11B_PYTHON" -c 'from pathlib import Path; assert not Path("agent.lock").exists(); assert not Path("target").exists(); print("No lock or package output created")'
```

Expected: `--refresh-python requires --dry-run`, before credentials or any
resolution work. Use `M11B_EXIT`, not zsh's read-only `status` variable.

## 3. Prepare, Inspect and Commit the Author Lock

This downloads verified uv from the dev mirror on first use and resolves real
Python dependencies. It creates a local archive but publishes nothing:

```bash
m11b --base-url "$M11B_REGISTRY" publish --dry-run --refresh-python --token apm_m11b.fixture --schema "$M11B_SCHEMA"
"$M11B_PYTHON" - <<'PY'
import json, re
from pathlib import Path
lock = json.loads(Path('agent.lock').read_text())
assert lock['lockfile_version'] == 4
assert lock['packages'] == {}
assert 'local:agent' not in lock['roots']
root = lock['roots']['local:tool']
resolution = root['python_resolution']
assert root['name'] == json.loads(Path('agent.json').read_text())['name']
assert re.fullmatch(r'[0-9a-f]{64}', root['python_input_fingerprint'])
assert resolution['requirements'] == ['click==8.1.8', 'pydantic==2.11.7']
print('Lock version:', lock['lockfile_version'])
print('Author root:', root['name'], root['version'])
print('Input fingerprint:', root['python_input_fingerprint'])
print('Python constraint:', resolution['python']['requires'])
for package in resolution['packages']:
    hashes = package['hashes']
    assert hashes and hashes == sorted(set(hashes))
    assert all(re.fullmatch(r'sha256:[0-9a-f]{64}', h) for h in hashes)
    print(package['name'], package['version'], 'hashes:', len(hashes), 'marker:', package.get('marker', 'all'))
assert not Path('target/agentpm/python-resolutions').exists()
print('Lock bytes:', Path('agent.lock').stat().st_size)
PY
find "$M11B_STATE" -type f -path '*/python-resolutions/*.json' -print
```

Expected: a checkmarked resolution summary and a dry-run archive. The author's
resolution/fingerprint are on `roots.local:tool`; `packages` stays empty. There
are exact versions, full sorted hashes and Windows-only Colorama despite running
on a Mac. Transitive versions/counts can change with PyPI. Disposable preparation
records now live in the isolated **user cache**, not the project.

Exercise lock exclusion even when the author explicitly lists it in `files`:

```bash
"$M11B_PYTHON" - <<'PY'
import json
from pathlib import Path
p = Path('agent.json')
value = json.loads(p.read_text())
value['files'] = list(dict.fromkeys([*value['files'], 'agent.lock']))
p.write_text(json.dumps(value, indent=2) + '\n')
PY
AGENTPM_UV_PATH="$M11B_ROOT/does-not-exist" m11b --base-url "$M11B_OFFLINE" publish --dry-run --token apm_m11b.fixture --schema "$M11B_SCHEMA"
"$M11B_PYTHON" - <<'PY'
import hashlib, json, os, tarfile
from pathlib import Path
archives = list(Path('target/agentpm').glob('*.tar.gz'))
assert len(archives) == 1, archives
with tarfile.open(archives[0], 'r:gz') as archive:
    names = archive.getnames()
    assert 'agent.lock' not in names
    assert names.count('agentpm-python.json') == 1
    metadata = json.load(archive.extractfile('agentpm-python.json'))
resolution = json.loads(Path('agent.lock').read_text())['roots']['local:tool']['python_resolution']
assert metadata['python_resolution'] == resolution
# This frozen resolution has ASCII object keys and string-only leaves; sorted
# compact UTF-8 JSON gives its canonical bytes (not a general JSON JCS encoder).
canonical = json.dumps(resolution, sort_keys=True, separators=(',', ':'), ensure_ascii=False).encode()
digest = hashlib.sha256(canonical).hexdigest()
assert metadata['python_resolution_digest'] == digest
Path(os.environ['M11B_ROOT'], 'expected-metadata.json').write_text(json.dumps(metadata, indent=2) + '\n')
print('Archive:', archives[0])
print('Tool-only metadata digest:', digest)
print('Metadata bytes:', len(json.dumps(metadata).encode()))
print('Root lock excluded:', 'agent.lock' not in names)
PY
```

Expected: success without uv or Registry access; matching lock state is reused.
The archive's metadata has the same resolution/digest, and the root lock is
absent. **Open `agent.lock` in your editor and review it before the next block.**
Then record that reviewed state in a local commit:

```bash
git init -q
git add agent.json agent.lock tool.py README.md
git -c user.name='AgentPM Manual Test' -c user.email='manual-test@example.invalid' -c commit.gpgsign=false commit -m 'Prepare reviewed Python resolution'
```

Expected: a local commit of the four reviewed files. This Git repository is
disposable and is never pushed. The CLI checks lock freshness/content; it does
not inspect Git history to enforce that you committed it.

## 4. Fresh Checkout and Empty User Cache Reuse the Lock

Use a local clone to reproduce a clean CI checkout, with no project `target/`
and a completely different user directory. Do not delete your normal caches:

```bash
git clone --quiet "$M11B_ROOT/author" "$M11B_ROOT/checkout"
cd "$M11B_ROOT/checkout"
export M11B_STATE="$M11B_ROOT/cold-state"
mkdir "$M11B_STATE"
AGENTPM_UV_PATH="$M11B_ROOT/does-not-exist" m11b --base-url "$M11B_OFFLINE" publish --dry-run --token apm_m11b.fixture --schema "$M11B_SCHEMA"
cmp "$M11B_ROOT/author/agent.lock" agent.lock
git diff --exit-code -- agent.json agent.lock
find "$M11B_STATE" -type f -print
```

Expected: the archive is prepared with the **same digest**, no uv execution,
resolver download or PyPI request. `cmp`/`git diff` exit 0; `find` prints no
files. Empty configuration directories are harmless. This is the committed-lock
reuse check, rather than reuse of the old disposable author cache.

Even a valid lock must not permit live refresh:

```bash
if AGENTPM_UV_PATH="$M11B_ROOT/does-not-exist" m11b --base-url "$M11B_OFFLINE" publish --refresh-python --token apm_m11b.fixture --schema "$M11B_SCHEMA"; then M11B_EXIT=0; else M11B_EXIT=$?; fi
printf 'CLI exit: %s (expected 1: refresh requires dry-run)\n' "$M11B_EXIT"
cmp "$M11B_ROOT/author/agent.lock" agent.lock
if AGENTPM_UV_PATH="$M11B_ROOT/does-not-exist" m11b --base-url "$M11B_OFFLINE" publish --dry-run --refresh-python --token apm_m11b.fixture --schema "$M11B_SCHEMA"; then M11B_EXIT=0; else M11B_EXIT=$?; fi
printf 'CLI exit: %s (expected 1: explicit refresh needs an existing uv)\n' "$M11B_EXIT"
cmp "$M11B_ROOT/author/agent.lock" agent.lock
```

Expected: the first failure names the flag rule; the second names the invalid
override file. Neither rewrites the valid lock. Keep this checkout unchanged for
the actual publish in section 8.

## 5. Install Preserves Author State; Removed Dependencies Clear It

Use another disposable copy so these mutations cannot affect the reviewed
checkout. A Tool project with no AgentPM package dependencies can now install
successfully without a Registry request:

```bash
mkdir "$M11B_ROOT/cleanup"
cp "$M11B_ROOT/author/agent.json" "$M11B_ROOT/author/agent.lock" "$M11B_ROOT/author/tool.py" "$M11B_ROOT/author/README.md" "$M11B_ROOT/cleanup/"
cd "$M11B_ROOT/cleanup"
m11b --base-url "$M11B_OFFLINE" install
cmp "$M11B_ROOT/author/agent.lock" agent.lock
m11b --base-url "$M11B_OFFLINE" install
cmp "$M11B_ROOT/author/agent.lock" agent.lock
```

Expected: both installs succeed, preserving the lock's exact bytes, timestamp,
resolution and fingerprint. No synthetic self-entry appears in `packages`.

Now remove the Python declaration and regenerate normally:

```bash
"$M11B_PYTHON" - <<'PY'
import json
from pathlib import Path
p = Path('agent.json')
value = json.loads(p.read_text())
del value['runtime']['dependencies']
p.write_text(json.dumps(value, indent=2) + '\n')
PY
m11b --base-url "$M11B_OFFLINE" install
"$M11B_PYTHON" - <<'PY'
import json
from pathlib import Path
lock = json.loads(Path('agent.lock').read_text())
root = lock['roots']['local:tool']
assert 'python_resolution' not in root
assert 'python_input_fingerprint' not in root
assert lock['lockfile_version'] == 4
assert lock['packages'] == {}
print('Removed author Python state; retained v4 and Tool identity')
PY
```

Expected: dead resolution state disappears; the existing v4 lock is not
downgraded. This changes only the `cleanup` copy.

## 6. Stale State Is Preserved but Cannot Pass Publication

Create a separate case and change Click from 8.1.8 to 8.1.7:

```bash
mkdir "$M11B_ROOT/stale"
cp "$M11B_ROOT/author/agent.json" "$M11B_ROOT/author/agent.lock" "$M11B_ROOT/author/tool.py" "$M11B_ROOT/author/README.md" "$M11B_ROOT/stale/"
cd "$M11B_ROOT/stale"
"$M11B_PYTHON" - <<'PY'
import json
from pathlib import Path
p = Path('agent.json')
value = json.loads(p.read_text())
value['runtime']['dependencies'][0] = 'click==8.1.7'
p.write_text(json.dumps(value, indent=2) + '\n')
PY
m11b --base-url "$M11B_OFFLINE" install
cmp "$M11B_ROOT/author/agent.lock" agent.lock
if AGENTPM_UV_PATH="$M11B_ROOT/does-not-exist" m11b --base-url "$M11B_OFFLINE" publish --token apm_m11b.fixture --schema "$M11B_SCHEMA"; then M11B_EXIT=0; else M11B_EXIT=$?; fi
printf 'CLI exit: %s (expected 1: stale resolution before acquisition)\n' "$M11B_EXIT"
cmp "$M11B_ROOT/author/agent.lock" agent.lock
```

Expected: install carries the author state forward, but it is still stale.
Publication fails with missing/stale guidance, not an override or connection
error; preservation cannot bypass freshness validation.

Refresh this copy using the original warm user directory, which already has uv:

```bash
export M11B_STATE="$M11B_WARM_STATE"
m11b --base-url "$M11B_OFFLINE" publish --dry-run --refresh-python --token apm_m11b.fixture --schema "$M11B_SCHEMA"
"$M11B_PYTHON" - <<'PY'
import json, os
from pathlib import Path
old = json.loads(Path(os.environ['M11B_ROOT'], 'author/agent.lock').read_text())['roots']['local:tool']
new = json.loads(Path('agent.lock').read_text())['roots']['local:tool']
assert old['python_input_fingerprint'] != new['python_input_fingerprint']
assert 'click==8.1.7' in new['python_resolution']['requirements']
assert any(p['name'] == 'click' and p['version'] == '8.1.7' for p in new['python_resolution']['packages'])
print('Refreshed fingerprint and Click requirement/version')
PY
```

Expected: successful refresh with Registry offline; PyPI access can still occur.
Do not publish this stale-test copy. Section 8 uses the unchanged checkout.

## 7. Authenticate and Check Registry Rejection of Invalid State

**From here onward, use only the local/dev Registry and dev namespace.** Return
to the reviewed checkout and the cold user directory; neither contains uv:

```bash
cd "$M11B_ROOT/checkout"
export M11B_STATE="$M11B_ROOT/cold-state"
printf 'Existing public dev namespace handle (without @): '
read -r M11B_NAMESPACE
export M11B_NAMESPACE
m11b --base-url "$M11B_REGISTRY" login --paste
m11b --base-url "$M11B_REGISTRY" whoami
export M11B_REF="@$M11B_NAMESPACE/$M11B_NAME"
printf 'Will publish only to dev: %s@0.1.0 via %s\n' "$M11B_REF" "$M11B_REGISTRY"
```

Paste your **dev** PAT at the hidden CLI prompt. Expected: the correct account
and `packages:publish` scope. Stop if the Registry/bucket/namespace is wrong.
The login is saved only inside `M11B_STATE`; do not print the credential file.

These two negative requests validate server enforcement independently of the
CLI. They read the isolated saved token without displaying it and must both
return HTTP 400 before creating a reservation or presigned URL:

```bash
"$M11B_PYTHON" - <<'PY'
import json, os
from pathlib import Path
from urllib.error import HTTPError
from urllib.parse import urlsplit
from urllib.request import Request, urlopen
registry = os.environ['M11B_REGISTRY']
assert urlsplit(registry).hostname in ('127.0.0.1', 'localhost'), 'Use the local Registry'
tokens = list(Path(os.environ['M11B_STATE']).rglob('token.json'))
assert len(tokens) == 1
token = json.loads(tokens[0].read_text())['access_token']
expected = json.loads(Path(os.environ['M11B_ROOT'], 'expected-metadata.json').read_text())
base = {'manifest': json.loads(Path('agent.json').read_text()), 'namespace_handle': os.environ['M11B_NAMESPACE'], 'sha256': '0' * 64, 'size': 1}
for label, extra in [
    ('missing resolution', {}),
    ('mismatched digest', {'python_resolution': expected['python_resolution'], 'python_resolution_digest': '0' * 64}),
]:
    request = Request(registry + '/v1/tools/publish/init', data=json.dumps({**base, **extra}).encode(), headers={'Content-Type': 'application/json', 'Authorization': 'Bearer ' + token}, method='POST')
    try:
        with urlopen(request, timeout=30) as response:
            status = response.status
            body = json.load(response)
    except HTTPError as error:
        status = error.code
        body = json.load(error)
    print(label, 'HTTP', status, body.get('code'))
    assert status == 400 and body.get('code') == 'invalid_python_resolution', 'STOP: unexpected server result'
PY
```

Expected: two `400 invalid_python_resolution` results. A 401/403 is an auth or
scope problem, not a passing validation result. A 429 means wait for the
publish-init rate window before retrying; these requests consume its budget.
If either unexpectedly succeeds, stop: it may have created a pending reservation.
Do not PUT any object or continue to publication until investigated.

## 8. Publish the Committed Resolution Once to Dev

This is the one intentional package publish. It writes dev Registry/S3 state.
Do not run it against production. Keep the invalid uv override: successful
publication must use the committed lock, not a newly computed resolution.

```bash
cd "$M11B_ROOT/checkout"
git diff --exit-code -- agent.json agent.lock
AGENTPM_UV_PATH="$M11B_ROOT/does-not-exist" m11b --base-url "$M11B_REGISTRY" publish --namespace "$M11B_NAMESPACE" --schema "$M11B_SCHEMA"
cmp "$M11B_ROOT/author/agent.lock" agent.lock
find "$M11B_STATE" -type f -path '*/python-resolutions/*.json' -print
find "$M11B_STATE" -type f -path '*/runtimes/uv/*' -print
```

Expected: publish init, upload and finalize succeed for `$M11B_REF@0.1.0`.
The resolution digest is unchanged, `cmp` exits 0, and both `find` commands
print nothing. Login credentials are allowed in this directory; resolver and
author caches are not required. The old blanket M11A publication block must
not appear. Registry logs should show successful publish init/finalize.

If publication fails, stop and retain its error/logs rather than changing the
lock or retrying with refresh. A duplicate-version error on a repeat run is not
a regression: releases are immutable. For a full rerun, start a new test root
and unique name from section 1.

In the dev database console, verify stored state with this read-only query.
Replace the namespace and package-name placeholders with the values printed
above; there should be one completed upload and one published version:

```sql
SELECT 'upload' AS record, n.handle, t.name, u.version, u.status,
       u.python_resolution->>'type' AS resolution_type,
       jsonb_array_length(u.python_resolution->'packages') AS distributions
FROM uploads u JOIN tools t ON t.tool_id = u.tool_id
JOIN namespaces n ON n.namespace_id = t.namespace_id
WHERE n.handle = '<your dev namespace>' AND t.name = '<M11B_NAME>'
UNION ALL
SELECT 'version', n.handle, t.name, v.version, v.status,
       v.python_resolution->>'type',
       jsonb_array_length(v.python_resolution->'packages')
FROM tool_versions v JOIN tools t ON t.tool_id = v.tool_id
JOIN namespaces n ON n.namespace_id = t.namespace_id
WHERE n.handle = '<your dev namespace>' AND t.name = '<M11B_NAME>';
```

Expected: non-null `agentpm.python-resolution.v1` on both records, with the same
distribution count as the prepared lock. The earlier invalid requests should
not have left extra upload reservations. Pending scan status is separate from
resolution persistence; if an install is blocked by policy, inspect the normal
dev scan workflow before treating it as a lock failure.

## 9. Consumer Install, Regeneration and Fresh Frozen Install

Create a real consumer Agent and install the exact published Tool:

```bash
mkdir "$M11B_ROOT/consumer"
cd "$M11B_ROOT/consumer"
m11b init --kind agent --name m11b-consumer
m11b --base-url "$M11B_REGISTRY" install "$M11B_REF@0.1.0"
"$M11B_PYTHON" - <<'PY'
import json, os
from pathlib import Path
lock = json.loads(Path('agent.lock').read_text())
expected = json.loads(Path(os.environ['M11B_ROOT'], 'expected-metadata.json').read_text())
key = 'tool:' + os.environ['M11B_REF'] + '@0.1.0'
assert lock['lockfile_version'] == 4
assert lock['packages'][key]['python_resolution'] == expected['python_resolution']
assert 'local:tool' not in lock.get('roots', {})
print('Consumer package key:', key)
print('Consumer resolution equals the published author resolution')
metadata_files = list(Path('.agentpm/tools').rglob('agentpm-python.json'))
assert len(metadata_files) == 1
assert json.loads(metadata_files[0].read_text()) == expected
print('Downloaded metadata:', metadata_files[0])
PY
m11b --base-url "$M11B_REGISTRY" install
cp agent.lock "$M11B_ROOT/consumer-regenerated.lock"
m11b --base-url "$M11B_REGISTRY" install
cmp "$M11B_ROOT/consumer-regenerated.lock" agent.lock
```

Expected: installs succeed. The exact resolution is on the **installed package**
entry; the consumer is still a local Agent. Normal regeneration retains all
resolution fields. The repeated unchanged install does not rewrite the lock's
bytes or `generated` timestamp. Compare the saved regenerated lock to the
author metadata again in the fresh frozen check below.

Copy only the consumer manifest and lock into a fresh directory, with no
downloaded artifacts or `.agentpm` state:

```bash
mkdir "$M11B_ROOT/frozen-consumer"
cp "$M11B_ROOT/consumer/agent.json" "$M11B_ROOT/consumer/agent.lock" "$M11B_ROOT/frozen-consumer/"
cd "$M11B_ROOT/frozen-consumer"
m11b --base-url "$M11B_REGISTRY" install --frozen
cmp "$M11B_ROOT/consumer-regenerated.lock" agent.lock
"$M11B_PYTHON" - <<'PY'
import json, os
from pathlib import Path
expected = json.loads(Path(os.environ['M11B_ROOT'], 'expected-metadata.json').read_text())
lock = json.loads(Path('agent.lock').read_text())
key = 'tool:' + os.environ['M11B_REF'] + '@0.1.0'
assert lock['packages'][key]['python_resolution'] == expected['python_resolution']
metadata_files = list(Path('.agentpm/tools').rglob('agentpm-python.json'))
assert len(metadata_files) == 1
assert json.loads(metadata_files[0].read_text()) == expected
print('Fresh frozen install retained the same resolution and artifact metadata')
PY
```

Expected: frozen uses the locked plan and obtains download metadata without
performing a new version resolve. The frozen lock stays byte-identical and the
downloaded Tool carries the same metadata as the normal install. Backend logs
should show install-init/finalize; this frozen command does not call
`/v1/tools/install/resolve`.

Do not use `agentpm run` as a dependency-import acceptance check here. M11B
carries metadata; 11C will provision and validate the Python environment.

## 10. An Old Consumer Lock Must Require Regeneration

Make an explicitly synthetic v3 copy that lacks Python state. Do not edit the
successful consumer's real v4 lock:

```bash
mkdir "$M11B_ROOT/legacy-consumer"
cp "$M11B_ROOT/consumer/agent.json" "$M11B_ROOT/consumer/agent.lock" "$M11B_ROOT/legacy-consumer/"
cd "$M11B_ROOT/legacy-consumer"
"$M11B_PYTHON" - <<'PY'
import json
from pathlib import Path
p = Path('agent.lock')
lock = json.loads(p.read_text())
lock['lockfile_version'] = 3
for package in lock['packages'].values():
    package.pop('python_resolution', None)
p.write_text(json.dumps(lock, indent=2) + '\n')
PY
cp agent.lock "$M11B_ROOT/legacy-consumer-before.lock"
if m11b --base-url "$M11B_REGISTRY" install --frozen; then M11B_EXIT=0; else M11B_EXIT=$?; fi
printf 'CLI exit: %s (expected 1: regenerate without frozen)\n' "$M11B_EXIT"
cmp "$M11B_ROOT/legacy-consumer-before.lock" agent.lock
"$M11B_PYTHON" -c 'from pathlib import Path; assert not Path(".agentpm/tools").exists(); print("No artifact extraction before rejection")'
```

Expected: actionable guidance to run `agentpm install` without `--frozen`, with
no lock rewrite or artifact download. The error may name a resolution mismatch
with the locked plan: Registry install-init supplies state the old lock lacks.
Recover through normal resolution:

```bash
m11b --base-url "$M11B_REGISTRY" install
"$M11B_PYTHON" - <<'PY'
import json, os
from pathlib import Path
lock = json.loads(Path('agent.lock').read_text())
expected = json.loads(Path(os.environ['M11B_ROOT'], 'expected-metadata.json').read_text())
assert lock['lockfile_version'] == 4
assert lock['packages']['tool:' + os.environ['M11B_REF'] + '@0.1.0']['python_resolution'] == expected['python_resolution']
print('Normal regeneration recovered complete v4 state')
PY
```

## 11. Local Compatibility and Unrelated-Lock Checks

These commands use a closed Registry address and do not publish anything.
Start an empty Agent to check v2/v3 retention without requiring published test
packages from historical snapshots:

```bash
mkdir "$M11B_ROOT/legacy-empty"
cd "$M11B_ROOT/legacy-empty"
m11b init --kind agent --name m11b-legacy-empty
m11b --base-url "$M11B_OFFLINE" install
cp agent.lock "$M11B_ROOT/empty-v2.lock"
m11b --base-url "$M11B_OFFLINE" install
cmp "$M11B_ROOT/empty-v2.lock" agent.lock
"$M11B_PYTHON" - <<'PY'
import json
from pathlib import Path
p = Path('agent.lock')
lock = json.loads(p.read_text())
assert lock['lockfile_version'] == 2
lock['lockfile_version'] = 3
p.write_text(json.dumps(lock, indent=2) + '\n')
PY
cp agent.lock "$M11B_ROOT/empty-v3.lock"
m11b --base-url "$M11B_OFFLINE" install
cmp "$M11B_ROOT/empty-v3.lock" agent.lock
```

Expected: unchanged v2 and v3 locks remain byte-identical; no forced v4 upgrade
or v3-to-v2 downgrade. These are manual synthetic checks, not a substitute for
the real pre-11B snapshot tests that already passed in CI.

The Agent can dry-run publish even with a malformed unrelated lock:

```bash
printf 'not valid JSON\n' > agent.lock
m11b --base-url "$M11B_OFFLINE" publish --dry-run --token apm_m11b.fixture --schema "$M11B_SCHEMA"
cat agent.lock
```

Expected: success and the malformed lock unchanged. Repeat for a Skill with an
unsupported future lock:

```bash
mkdir "$M11B_ROOT/skill-unrelated"
cd "$M11B_ROOT/skill-unrelated"
m11b init --kind skill --name m11b-skill-unrelated
printf '{"lockfile_version":99,"packages":{},"roots":{}}\n' > agent.lock
m11b --base-url "$M11B_OFFLINE" publish --dry-run --token apm_m11b.fixture --schema "$M11B_SCHEMA"
cat agent.lock
```

Expected: success; non-Tool publication does not deserialize the unrelated lock.
Now verify the Tool path does reject a future lock before mutation:

```bash
mkdir "$M11B_ROOT/future-tool"
cp "$M11B_ROOT/author/agent.json" "$M11B_ROOT/author/agent.lock" "$M11B_ROOT/author/tool.py" "$M11B_ROOT/author/README.md" "$M11B_ROOT/future-tool/"
cd "$M11B_ROOT/future-tool"
"$M11B_PYTHON" - <<'PY'
import json
from pathlib import Path
p = Path('agent.lock')
lock = json.loads(p.read_text())
lock['lockfile_version'] = 99
lock['future_state'] = {'must': 'survive'}
p.write_text(json.dumps(lock, indent=2) + '\n')
PY
cp agent.lock "$M11B_ROOT/future-before.lock"
if m11b --base-url "$M11B_OFFLINE" install; then M11B_EXIT=0; else M11B_EXIT=$?; fi
printf 'CLI exit: %s (expected 1: unsupported lock version 99)\n' "$M11B_EXIT"
cmp "$M11B_ROOT/future-before.lock" agent.lock
if m11b --base-url "$M11B_OFFLINE" publish --token apm_m11b.fixture --schema "$M11B_SCHEMA"; then M11B_EXIT=0; else M11B_EXIT=$?; fi
printf 'CLI exit: %s (expected 1: unsupported lock version 99)\n' "$M11B_EXIT"
cmp "$M11B_ROOT/future-before.lock" agent.lock
```

Expected: both failures say the lock version is unsupported and advise upgrading
AgentPM, while preserving the file. Current readers also reject unknown
structural fields within v4; this is fail-closed behavior, not transparent
support for future additive v4 fields. The Stage 2 extension decision remains
recorded in the specs.

## Completion Record and Boundaries

The operator confirmed all manual tests passed on **2026-10-24** using the local
development walkthrough. The checked items below record that confirmation;
verify them again when repeating the guide.

Record the CLI digest, workspace path, dev namespace/package/version, prepared
resolution digest, and the outcome of each section. Do not copy PATs or signed
S3 URLs into acceptance notes. The guide's existence is not acceptance evidence.

- [x] Local backend dependencies and migration verified; dev bucket confirmed.
- [x] Missing/stale locks and live refresh fail before acquisition/upload.
- [x] Preparation writes exact portable Python state on the local Tool root.
- [x] Archive metadata matches the lock/digest and excludes the root lock.
- [x] Reviewed local commit works from a clean checkout and empty user cache.
- [x] Plain install preserves live author state and drops removed declarations.
- [x] Preserved stale state remains unpublishable; explicit preparation refreshes it.
- [x] Registry rejects missing resolution and mismatched digest independently.
- [x] One real dev publication persists resolution in reservation and version rows.
- [x] Normal, regenerated and fresh frozen consumer installs retain identical resolution metadata.
- [x] A legacy consumer lock requires regeneration and recovers complete v4 state.
- [x] Local v2/v3 byte stability, non-Tool lock independence and future-version rejection pass.

During this run, repeating the original section 3 command appended `agent.lock`
three times to `files` before publication. The runbook now deduplicates that
list. Those repeated declarations did not require republishing: the root lock
is excluded from the Tool archive, and the operator reported the subsequent
consumer metadata checks passed. The published fixture was left unchanged.

Automated tests cover the realistic OpenAI/Pydantic scale fixture, canonical digest
vectors, tar metadata tampering/aliases and the 8 MiB aggregate compact resolve
response budget. The 512 KiB realistic-lock assertion is a test-growth budget,
not an enforced lock or Registry limit. No giant manual uploads are required to
repeat those automated cases.

Python environment provisioning, third-party imports on a clean machine and
resolve-on-one/install-and-execute-on-another proof belong to **11C**. Full
multi-artifact release assembly remains M12/M14. Production deployment or a CLI
release is a separate release-band decision, not performed by this guide.

Keep the temporary root for inspection. No cleanup command deletes local files,
Registry packages, S3 objects or your real caches. Closing the disposable terminal
discards the launcher and its task variables. The dev test package remains a real
published fixture; normal namespace/package management can handle it later.
