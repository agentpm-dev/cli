# Milestone 11C: Managed Python Tool Environments

Run **one block at a time**, checking its expected result before continuing.
Use macOS/zsh and a **new terminal**, starting in the `agentpm` repository.
Keep that terminal for the whole guide. Only section 9 uses a second terminal.

We will develop the real `validate-order` example, publish one temporary version
to your **dev Registry**, install it into fresh consumers, and execute its native
Pydantic dependency through both direct run and Harness. No paid model is used.

| Sections | What changes |
| --- | --- |
| 1–2 | Disposable files and caches; downloads from the dev uv mirror and PyPI. |
| 3 | **One dev-only Tool publication**, creating dev Registry rows and S3 objects. |
| 4–8 | Consumer installs/download bookkeeping and disposable managed environments. |
| 9 | A local scripted model and unpublished fixture Loop; real Harness Tool dispatch. |
| 10 | A locally staged legacy Tool; no Registry publication. |
| 11 | Record results; leave the disposable directory for inspection. |

The four Resolver Contract jobs are already confirmed green. They cover native
target replay, wrong hashes, unavailable/no-compatible wheels, source-build
rejection, no package-set expansion, and creation without `ensurepip`. This guide
adds the live dev Registry/S3 and operator walkthrough; it does not recreate a
hostile package index or upgrade your OS/Python installation.

**Do not publish `validate-order` to production.** `summarize-text` publication
and its later `devwork-copilot` rollout remain Release Band 8 work.

## Before Starting

- The local backend must run the accepted 11B Registry code and be configured for
  **`agentpm-dev-tools`**. Restart it through IntelliJ if needed. There are no new
  API migrations or resolver mirror uploads for 11C.
- Use an existing **public dev namespace** you own, with signing optional/off for
  this unsigned example. Have a dev PAT with `packages:publish` ready. We will
  paste it into the CLI's hidden prompt, not into a shell command.
- The sibling `agentpm-examples` checkout must contain `tools-python/validate-order`.
- Use Python 3.11+ and `uv` for author development. Consumers use AgentPM's pinned
  uv; a system uv is not needed for consumer installation/execution.

## 1. Build and Set Up the Disposable Area

From the **`agentpm` repo root**:

```bash
cargo build -p agentpm-cli
export M11C_REPO="$PWD"
export M11C_APM="$M11C_REPO/target/debug/agentpm"
export M11C_SCHEMA="$M11C_REPO/schemas/agentpm.manifest.schema.json"
export M11C_PYTHON="$M11C_REPO/../agentpm-api/.venv/bin/python"
export M11C_EXAMPLE="$M11C_REPO/../agentpm-examples/tools-python/validate-order"
export M11C_FIXTURES="$M11C_REPO/specs/2026-10-06-immediate-hardening/manual-tests/fixtures/m11c"
export M11C_REGISTRY='http://127.0.0.1:5000'
export M11C_OFFLINE='http://127.0.0.1:1'
export M11C_ROOT="$(mktemp -d "${TMPDIR:-/tmp}/agentpm-m11c.XXXXXX")"
export M11C_STATE="$M11C_ROOT/warm-state"
export M11C_VERSION="0.1.0-m11c.$(date -u +%Y%m%d%H%M%S)"
export M11C_MODEL_PORT=18160
mkdir "$M11C_STATE"
unset AGENTPM_TOKEN AGENTPM_UV_PATH M11C_RUN_PYTHON M11C_ENDPOINT
```

The unique prerelease version avoids colliding with an immutable earlier dev
publication. Only a copied manifest is changed; the examples checkout stays intact.
Port 1 must be unused; choose another closed loopback port if necessary.

Define this launcher in the **same terminal**:

```bash
m11c() {
  env HOME="$M11C_STATE" USERPROFILE="$M11C_STATE" \
    XDG_CONFIG_HOME="$M11C_STATE/config" XDG_DATA_HOME="$M11C_STATE/data" \
    XDG_CACHE_HOME="$M11C_STATE/cache" APPDATA="$M11C_STATE/data" \
    AGENTPM_TELEMETRY=0 AGENTPM_PYTHON="${M11C_RUN_PYTHON:-$M11C_PYTHON}" \
    PATH="$(dirname "$M11C_APM"):$PATH" \
    "$M11C_APM" --base-url "${M11C_ENDPOINT:-$M11C_REGISTRY}" "$@"
}
"$M11C_PYTHON" -c 'import sys; assert sys.version_info >= (3,11); print(sys.version)'
m11c --version
curl --fail --show-error "$M11C_REGISTRY/health"
printf '\nDisposable workspace: %s\nDev version: %s\n' "$M11C_ROOT" "$M11C_VERSION"
```

Expected: Python 3.11+, the rebuilt CLI and a healthy local backend. The CLI may
still display `0.1.36`; the path above selects the new build, not your Brew binary.
The launcher isolates each CLI process's user state; your terminal's real HOME
and normal credentials are unchanged. `M11C_PYTHON` is the baseline interpreter;
section 7 temporarily selects another through `M11C_RUN_PYTHON`.

Check the local-only endpoint and record the executable you are testing:

```bash
"$M11C_PYTHON" - <<'PY'
import hashlib, os
from pathlib import Path
from urllib.parse import urlsplit
assert urlsplit(os.environ['M11C_REGISTRY']).hostname in ('127.0.0.1', 'localhost')
assert Path(os.environ['M11C_EXAMPLE'], 'tool.py').is_file()
print('CLI SHA-256:', hashlib.sha256(Path(os.environ['M11C_APM']).read_bytes()).hexdigest())
PY
```

A healthy endpoint does not prove the bucket selection: confirm the dev bucket
in your backend configuration before section 3.

## 2. Develop and Prepare the Real Example

```bash
mkdir "$M11C_ROOT/author"
cp "$M11C_EXAMPLE/agent.json" "$M11C_EXAMPLE/tool.py" "$M11C_EXAMPLE/README.md" "$M11C_EXAMPLE/pyproject.toml" "$M11C_ROOT/author/"
cp -R "$M11C_EXAMPLE/tests" "$M11C_ROOT/author/tests"
cd "$M11C_ROOT/author"
"$M11C_PYTHON" - <<'PY'
import json, os
from pathlib import Path
p = Path('agent.json')
value = json.loads(p.read_text())
value['version'] = os.environ['M11C_VERSION']
p.write_text(json.dumps(value, indent=2) + '\n')
PY
uv sync
uv run python -m unittest discover -s tests -p 'test_*.py'
printf '%s\n' '{"quantity":2,"unit_price":4.5}' | uv run python tool.py
m11c lint --strict --schema "$M11C_SCHEMA"
```

Expected: **3 development tests pass**, local execution returns `{"total":9.0}`
(whitespace can differ), and lint succeeds. This is real Pydantic validation,
including its native extension, inside the **author's `.venv`**.

Prepare the author lock and archive. This block publishes nothing:

```bash
m11c publish --dry-run --refresh-python --token apm_m11c.fixture --schema "$M11C_SCHEMA"
"$M11C_PYTHON" - <<'PY'
import json, os, re, tarfile
from pathlib import Path
root = Path(os.environ['M11C_ROOT'])
lock = json.loads(Path('agent.lock').read_text())
assert lock['lockfile_version'] == 4 and lock['packages'] == {}
local = lock['roots']['local:tool']
assert local['name'] == 'validate-order' and local['version'] == os.environ['M11C_VERSION']
resolution = local['python_resolution']
assert resolution['requirements'] == ['pydantic==2.11.7']
assert any(p['name'] == 'pydantic-core' for p in resolution['packages'])
for package in resolution['packages']:
    assert package['hashes'] and package['hashes'] == sorted(set(package['hashes']))
    assert all(re.fullmatch(r'sha256:[0-9a-f]{64}', h) for h in package['hashes'])
    print(package['name'], package['version'], 'hashes:', len(package['hashes']))
archives = list(Path('target/agentpm').glob('*.tar.gz'))
assert len(archives) == 1
with tarfile.open(archives[0]) as archive:
    names = archive.getnames()
    assert 'agent.lock' not in names and not any('.venv' in n or '_vendor' in n for n in names)
    metadata = json.load(archive.extractfile('agentpm-python.json'))
assert metadata['python_resolution'] == resolution
(root / 'expected-metadata.json').write_text(json.dumps(metadata, indent=2) + '\n')
print('Resolution digest:', metadata['python_resolution_digest'])
print('Archive:', archives[0])
PY
```

Expected: exact versions/full hashes, native `pydantic-core`, and Tool metadata
matching the author resolution. Count/transitive versions can vary with PyPI.
`roots.local:tool.tools: []` is normal: these are Python distributions, not
AgentPM Tool dependencies. No consumer environment is created in the author root.
Review this disposable lock; the real example's reviewed lock remains untouched.

## 3. Publish This Version to Dev Only

This is the **only publication** in the walkthrough. Authenticate into the
isolated user state, using your existing public dev namespace:

```bash
cd "$M11C_ROOT/author"
printf 'Public dev namespace handle (without @): '
read -r M11C_NAMESPACE
export M11C_NAMESPACE
export M11C_REF="@$M11C_NAMESPACE/validate-order@$M11C_VERSION"
m11c login --paste
m11c whoami
printf 'Dev Registry: %s\nDev Tool: %s\n' "$M11C_REGISTRY" "$M11C_REF"
```

Expected: your dev account with publish access. Paste the dev PAT only at the
hidden login prompt. Stop if the endpoint/account/bucket is wrong.

```bash
AGENTPM_UV_PATH="$M11C_ROOT/does-not-exist" m11c publish --namespace "$M11C_NAMESPACE" --schema "$M11C_SCHEMA"
```

Expected: successful dev publication despite the nonexistent uv override:
publication uses the reviewed lock without resolving again. This creates dev
Registry rows and dev S3 objects. Keep the printed package reference for the
remaining sections. The override above is temporary; do not export it globally.

If viewing this prerelease in the Registry reports `packaging.version.InvalidVersion`,
restart the local backend with the SemVer detail/latest lookup fix. This is a
Registry parser mismatch, not an invalid AgentPM version. Keep the existing
publication and reference; no rename, database edit or republish is needed.
This test publishes a prerelease: its explicit version page remains viewable,
while `/latest` shows no stable release until a non-yanked stable version exists.

## 4. Install, Inspect and Run as a Consumer

```bash
mkdir "$M11C_ROOT/consumer"
cd "$M11C_ROOT/consumer"
printf '.agentpm/\n.agentpm-state/\n' > .gitignore
"$M11C_PYTHON" - <<'PY'
import json, os
from pathlib import Path
Path('agent.json').write_text(json.dumps({
    '$schema': 'https://raw.githubusercontent.com/agentpm-dev/cli/main/schemas/agentpm.manifest.schema.json',
    'kind': 'agent', 'name': 'm11c-consumer', 'version': '0.1.0',
    'description': 'Disposable consumer for managed Python Tool checks.',
    'tools': [{'name': '@' + os.environ['M11C_NAMESPACE'] + '/validate-order', 'version': os.environ['M11C_VERSION']}]
}, indent=2) + '\n')
PY
m11c install
```

Expected: install includes **Preparing Python environments** and succeeds only
after provisioning. No consumer `uv sync`, pip install or venv activation is
needed. Extracted Tool code and runtime environment occupy separate directories.

Locate and inspect the environment; this prints its actual path:

```bash
export M11C_ENV="$("$M11C_PYTHON" - <<'PY'
from pathlib import Path
paths = list(Path('.agentpm/runtimes/python').rglob('ready.json'))
assert len(paths) == 1, paths
print(paths[0].parent.resolve())
PY
)"
export M11C_ENV_PYTHON="$M11C_ENV/venv/bin/python"
printf 'Managed environment: %s\nManaged Python: %s\n' "$M11C_ENV" "$M11C_ENV_PYTHON"
"$M11C_PYTHON" - <<'PY'
import json, os, subprocess
from pathlib import Path
expected = json.loads(Path(os.environ['M11C_ROOT'], 'expected-metadata.json').read_text())
lock = json.loads(Path('agent.lock').read_text())
key = 'tool:@' + os.environ['M11C_NAMESPACE'] + '/validate-order@' + os.environ['M11C_VERSION']
assert lock['packages'][key]['python_resolution'] == expected['python_resolution']
environment = Path(os.environ['M11C_ENV'])
ready = json.loads((environment / 'ready.json').read_text())
assert ready['fingerprint']['resolution_digest'] == expected['python_resolution_digest']
assert not {'platform_release', 'platform_version', 'python_full_version'} & ready['fingerprint']['interpreter'].keys()
script = '''import importlib.metadata,json,re,sys
import pydantic_core
import pydantic_core._pydantic_core as native
assert pydantic_core.from_json(b'{"native":true}') == {"native":True}
packages=sorted([[re.sub(r"[-_.]+","-",d.metadata["Name"]).lower(),d.version] for d in importlib.metadata.distributions()])
print(json.dumps({"prefix":sys.prefix,"native":native.__file__,"packages":packages}))'''
actual = json.loads(subprocess.check_output([os.environ['M11C_ENV_PYTHON'], '-I', '-c', script], text=True))
assert Path(actual['prefix']).resolve() == (environment / 'venv').resolve()
assert actual['packages'] == ready['packages']
recorded = {(p['name'], p['version']) for p in expected['python_resolution']['packages']}
assert all(tuple(p) in recorded for p in actual['packages'])
print(json.dumps(actual, indent=2))
PY
m11c run "$M11C_REF" --input '{"quantity":2,"unit_price":4.5}'
m11c run "$M11C_REF" --machine --input '{"quantity":2,"unit_price":4.5}'
```

Expected: the compiled native module is inside the managed venv, its package set
matches the readiness record and recorded versions, direct output is total 9,
and machine output has `status: success` with `output.total: 9`.

Run uses no resolver or Registry. Reuse an already-ready install as well:

```bash
M11C_ENDPOINT="$M11C_OFFLINE" AGENTPM_UV_PATH="$M11C_ROOT/does-not-exist" m11c run "$M11C_REF" --input '{"quantity":2,"unit_price":4.5}'
cp "$M11C_ENV/ready.json" "$M11C_ROOT/ready-before-reuse.json"
cp agent.lock "$M11C_ROOT/consumer-lock.json"
m11c install
cmp "$M11C_ROOT/ready-before-reuse.json" "$M11C_ENV/ready.json"
cmp "$M11C_ROOT/consumer-lock.json" agent.lock
```

Expected: run still returns total 9; both `cmp` commands are silent. The second
install reuses the ready environment. It still uses normal Registry/S3 install
bookkeeping; this is not a claim that the entire install is network-free.

## 5. Fresh Frozen Consumer with an Empty User Cache

```bash
mkdir "$M11C_ROOT/frozen" "$M11C_ROOT/cold-state"
cp "$M11C_ROOT/consumer/agent.json" "$M11C_ROOT/consumer/agent.lock" "$M11C_ROOT/consumer/.gitignore" "$M11C_ROOT/frozen/"
cd "$M11C_ROOT/frozen"
export M11C_STATE="$M11C_ROOT/cold-state"
m11c install --frozen
cmp "$M11C_ROOT/consumer-lock.json" agent.lock
m11c run "$M11C_REF" --input '{"quantity":2,"unit_price":4.5}'
export M11C_STATE="$M11C_ROOT/warm-state"
cd "$M11C_ROOT/consumer"
```

Expected: the fresh consumer obtains verified uv/wheels, provisions its own
environment and returns total 9. The copied v4 lock is byte-identical. It does
not need the author's `.venv`, Python-resolution cache, or publish credentials.
This is same-machine replay with fresh state; the passing native jobs supply the
other-target replay evidence.

## 6. Interrupted State and Repair

Remove only the completion record in this disposable environment:

```bash
cd "$M11C_ROOT/consumer"
"$M11C_PYTHON" - <<'PY'
import os
from pathlib import Path
root = Path(os.environ['M11C_ROOT']).resolve()
environment = Path(os.environ['M11C_ENV']).resolve()
assert environment.is_relative_to(root / 'consumer' / '.agentpm')
(environment / 'ready.json').unlink()
PY
if m11c run "$M11C_REF" --input '{"quantity":2,"unit_price":4.5}'; then M11C_EXIT=0; else M11C_EXIT=$?; fi
printf 'Exit: %s (expected 1: missing completion record)\n' "$M11C_EXIT"
m11c install
m11c run "$M11C_REF" --input '{"quantity":2,"unit_price":4.5}'
```

Expected: run refuses the incomplete environment with reinstall guidance.
Install repairs it **in the same directory**, writes a new completion record,
and run works again. No global Python packages are changed.

Now simulate an unexpected installed distribution, using metadata only:

```bash
"$M11C_ENV_PYTHON" -I - <<'PY'
from pathlib import Path
import sys, sysconfig
site = Path(sysconfig.get_path('purelib')).resolve()
assert site.is_relative_to(Path(sys.prefix).resolve())
extra = site / 'm11c_unrecorded-1.0.0.dist-info'
extra.mkdir()
(extra / 'METADATA').write_text('Metadata-Version: 2.3\nName: m11c-unrecorded\nVersion: 1.0.0\n')
(extra / 'RECORD').write_text('m11c_unrecorded-1.0.0.dist-info/METADATA,,\nm11c_unrecorded-1.0.0.dist-info/RECORD,,\n')
PY
if m11c run "$M11C_REF" --input '{"quantity":2,"unit_price":4.5}'; then M11C_EXIT=0; else M11C_EXIT=$?; fi
printf 'Exit: %s (expected 1: package inventory mismatch)\n' "$M11C_EXIT"
m11c install --refresh
"$M11C_ENV_PYTHON" -I - <<'PY'
import importlib.metadata
assert not any(d.metadata['Name'] == 'm11c-unrecorded' for d in importlib.metadata.distributions())
print('Unrecorded distribution removed')
PY
m11c run "$M11C_REF" --input '{"quantity":2,"unit_price":4.5}'
cmp "$M11C_ROOT/consumer-lock.json" agent.lock
```

Expected: run rejects the changed inventory, refresh converges to the exact
recorded set, and the portable lock stays unchanged. `--refresh` reinstalls
recorded wheels; it does not re-resolve dependencies.

## 7. Change the Selected Python, Then Return to the Original

Find the **managed uv** acquired into the isolated warm user cache:

```bash
export M11C_UV="$("$M11C_PYTHON" - <<'PY'
import json, os
from pathlib import Path
version = json.loads(Path(os.environ['M11C_REPO'], 'platforms/uv-pins.json').read_text())['version']
paths = [p for p in Path(os.environ['M11C_STATE']).rglob('uv') if p.is_file() and version in p.parts]
assert len(paths) == 1, paths
print(paths[0].resolve())
PY
)"
"$M11C_UV" --version
"$M11C_UV" --no-config venv --no-python-downloads --no-managed-python --python "$M11C_PYTHON" --cache-dir "$M11C_ROOT/alternate-cache" "$M11C_ROOT/alternate-python"
export M11C_RUN_PYTHON="$M11C_ROOT/alternate-python/bin/python"
if m11c run "$M11C_REF" --input '{"quantity":2,"unit_price":4.5}'; then M11C_EXIT=0; else M11C_EXIT=$?; fi
printf 'Exit: %s (expected 1: different selected executable)\n' "$M11C_EXIT"
m11c install
m11c run "$M11C_REF" --input '{"quantity":2,"unit_price":4.5}'
unset M11C_RUN_PYTHON
m11c run "$M11C_REF" --input '{"quantity":2,"unit_price":4.5}'
find .agentpm/runtimes/python -name ready.json -print
cmp "$M11C_ROOT/consumer-lock.json" agent.lock
```

Expected: the alternate selection initially fails, explicit install prepares its
own environment, then both selections work. There are two readiness records,
and the portable lock does not change. This tests a deliberate executable
selection change without downloading or upgrading Python. Routine kernel/patch
identity stability is covered by the native/unit tests; do not update your OS
for this guide. Old environments for distinct identities are retained.

## 8. Ambient Import Paths Cannot Shadow Locked Packages

```bash
mkdir "$M11C_ROOT/shadow"
printf 'raise RuntimeError("M11C ambient package shadow was imported")\n' > "$M11C_ROOT/shadow/pydantic.py"
if PYTHONPATH="$M11C_ROOT/shadow" "$M11C_PYTHON" -c 'import pydantic'; then M11C_EXIT=0; else M11C_EXIT=$?; fi
printf 'Control exit: %s (expected 1: ordinary Python sees the shadow)\n' "$M11C_EXIT"
PYTHONPATH="$M11C_ROOT/shadow" m11c run "$M11C_REF" --input '{"quantity":2,"unit_price":4.5}'
PYTHONPATH="$M11C_ROOT/shadow" m11c run "$M11C_REF" --machine --input '{"quantity":2,"unit_price":4.5}'
```

Expected: the control fails with the deliberate shadow message. Both managed
runs succeed with total 9. These environment assignments affect only the command
on that line. Managed execution clears `PYTHONHOME`/`PYTHONPATH` after merging
environment values; it still permits normal Tool-local package imports.

## 9. Harness Calls the Installed Tool Through the Managed Environment

Use the same consumer. The Tool has already come through the real Registry/S3.
The Loop below is **only an unpublished local test fixture**, staged explicitly
so we do not need a second publication. Its placeholder lock integrity is never
used for a download. Do not run install while this local-only Loop is in the
manifest; we restore the normal consumer manifest/lock at the end of the section.

```bash
cd "$M11C_ROOT/consumer"
cp agent.json "$M11C_ROOT/before-harness-agent.json"
cp agent.lock "$M11C_ROOT/before-harness.lock"
mkdir -p .agentpm/loops/fixture/m11c-order-loop/0.1.0
cp "$M11C_FIXTURES/loop.fixture.json" .agentpm/loops/fixture/m11c-order-loop/0.1.0/agent.json
cp "$M11C_FIXTURES/harness.fixture.json" agentpm.harness.json
"$M11C_PYTHON" - <<'PY'
import json, os
from pathlib import Path
tool = '@' + os.environ['M11C_NAMESPACE'] + '/validate-order'
loop = '@fixture/m11c-order-loop'
key = 'loop:' + loop + '@0.1.0'
p = Path('agent.json')
agent = json.loads(p.read_text())
agent['loop'] = loop + '@0.1.0'
agent['bindings'] = {'global': {'tools': [tool]}}
p.write_text(json.dumps(agent, indent=2) + '\n')
p = Path('agent.lock')
lock = json.loads(p.read_text())
lock['packages'][key] = {'kind':'loop', 'name':loop, 'version':'0.1.0', 'integrity':'0' * 64}
lock['roots']['local:agent']['loop'] = key
p.write_text(json.dumps(lock, indent=2) + '\n')
PY
m11c lint --strict --schema "$M11C_SCHEMA"
m11c lint --strict .agentpm/loops/fixture/m11c-order-loop/0.1.0/agent.json --schema "$M11C_SCHEMA"
```

Print a complete command for the second terminal:

```bash
"$M11C_PYTHON" - <<'PY'
import os, shlex
from pathlib import Path
command = [os.environ['M11C_PYTHON'], str(Path(os.environ['M11C_FIXTURES'], 'provider.py')), '--port', os.environ['M11C_MODEL_PORT'], '--events', str(Path(os.environ['M11C_ROOT'], 'harness-provider-events.jsonl'))]
print(shlex.join(command))
PY
```

**Copy the printed command, paste it into a second terminal, and leave it running.**
That terminal needs none of the first terminal's variables. If port 18160 is
occupied, change `M11C_MODEL_PORT` in the first terminal, print a new command,
and use that command. The fixture listens on loopback and never forwards calls.

Back in the **first terminal**:

```bash
curl --fail --show-error "http://127.0.0.1:$M11C_MODEL_PORT/health"
OPENAI_API_KEY=local-fixture-only OPENAI_BASE_URL="http://127.0.0.1:$M11C_MODEL_PORT/model" m11c harness --headless --input 'Validate two units at 4.5 each using the order Tool.' --report "$M11C_ROOT/harness-report.json" > "$M11C_ROOT/harness-output.json"
cat "$M11C_ROOT/harness-output.json"
"$M11C_PYTHON" - <<'PY'
import json, os
from pathlib import Path
root = Path(os.environ['M11C_ROOT'])
value = json.loads((root / 'harness-output.json').read_text())
assert value == {'total':9.0, 'verified_tool_result':True}, value
events = [json.loads(line) for line in (root / 'harness-provider-events.jsonl').read_text().splitlines()]
assert any(e['event'] == 'tool_requested' for e in events)
assert any(e['event'] == 'tool_result_observed' for e in events)
assert not any(e['event'] == 'fixture_error' for e in events)
report = json.loads((root / 'harness-report.json').read_text())
assert report['terminal_status'] == 'ended', report['terminal_status']
assert report['usage']['tool_calls'] >= 1
print('Harness ended after a real Tool call; provider observed total 9.0')
PY
```

Expected: preflight is Ready, Harness invokes the real installed Tool through
`agentpm run --machine`, and the local model sees its result before completing.
This is not a fixed completion that skips Tool execution. The CLI/Harness use
the same managed environment; no real API key or model service is involved.

Restore the normal consumer files, then stop the fixture with **Ctrl-C in the
second terminal**:

```bash
cp "$M11C_ROOT/before-harness-agent.json" agent.json
cp "$M11C_ROOT/before-harness.lock" agent.lock
cmp "$M11C_ROOT/consumer-lock.json" agent.lock
```

## 10. Legacy Author-Vendored Tool Still Runs

This local probe does not publish or install anything from a Registry. It has
no Python dependency declaration and deliberately uses its own `_vendor` helper.

```bash
mkdir -p "$M11C_ROOT/legacy/.agentpm/tools/fixture/m11c-legacy/0.1.0/_vendor"
cd "$M11C_ROOT/legacy"
cp "$M11C_FIXTURES/legacy-tool.fixture.json" .agentpm/tools/fixture/m11c-legacy/0.1.0/agent.json
cp "$M11C_FIXTURES/legacy_tool.py" .agentpm/tools/fixture/m11c-legacy/0.1.0/
cp "$M11C_FIXTURES/legacy_helper.py" .agentpm/tools/fixture/m11c-legacy/0.1.0/_vendor/
"$M11C_PYTHON" - <<'PY'
import json
from pathlib import Path
name = '@fixture/m11c-legacy'
key = 'tool:' + name + '@0.1.0'
Path('agent.json').write_text(json.dumps({'$schema':'https://raw.githubusercontent.com/agentpm-dev/cli/main/schemas/agentpm.manifest.schema.json','kind':'agent','name':'m11c-legacy-consumer','version':'0.1.0','description':'Disposable legacy Tool consumer.','tools':[{'name':name,'version':'0.1.0'}]}, indent=2) + '\n')
Path('agent.lock').write_text(json.dumps({'lockfile_version':4,'generated':'2026-10-24T00:00:00Z','packages':{key:{'kind':'tool','name':name,'version':'0.1.0','integrity':'0' * 64}},'roots':{'local:agent':{'name':'m11c-legacy-consumer','version':'0.1.0','tools':[key]}}}, indent=2) + '\n')
PY
m11c lint --strict .agentpm/tools/fixture/m11c-legacy/0.1.0/agent.json --schema "$M11C_SCHEMA"
M11C_ENDPOINT="$M11C_OFFLINE" AGENTPM_UV_PATH="$M11C_ROOT/does-not-exist" m11c run @fixture/m11c-legacy@0.1.0 --input '{}' > "$M11C_ROOT/legacy-output.json"
cat "$M11C_ROOT/legacy-output.json"
"$M11C_PYTHON" - <<'PY'
import json, os, subprocess
from pathlib import Path
value = json.loads(Path(os.environ['M11C_ROOT'], 'legacy-output.json').read_text())
assert value['legacy'] == 'author-vendor'
selected = subprocess.check_output([os.environ['M11C_PYTHON'], '-I', '-c', 'import sys; print(sys.prefix)'], text=True).strip()
assert Path(value['prefix']).resolve() == Path(selected).resolve()
assert not Path('.agentpm/runtimes').exists()
print('Legacy helper loaded through selected Python; no managed environment created')
PY
```

Expected: legacy author vendoring works through the selected baseline Python,
with Registry offline and a bad resolver override, without entering the managed
environment path. The placeholder integrity belongs only to this local probe.

## 11. Acceptance and What to Keep

Record the dev package reference, CLI digest, and section outcomes. Keep
`M11C_ROOT` until we have reviewed any failures; no automatic deletion or Registry
cleanup is part of these commands.

Operator-confirmed results, 2026-10-24:

- [x] Author development tests and real dependency-using invocation passed.
- [x] Dev-only publication reused the prepared lock with a missing uv override.
- [x] Fresh install created a separate managed environment and imported native code.
- [x] Direct and machine runs returned total 9; ready reinstall preserved state.
- [x] Fresh user/cache frozen install preserved the portable lock and ran.
- [x] Missing completion and unexpected inventory were rejected and repaired.
- [x] Deliberate Python selection change required install; both selections then ran.
- [x] Ambient `PYTHONPATH` shadow failed to replace the managed dependency.
- [x] Headless Harness called the actual Tool and observed its result.
- [x] Legacy `_vendor` probe kept its existing execution behavior.

These checkmarks record the operator's completed walkthrough, using dev reference
`@zack/validate-order@0.1.0-m11c.20261024200841`. Together with completed review and
all four passing Resolver Contract jobs, this closes 11C acceptance; see
`../test-plan.md` for evidence and limits. No production Tool/Agent publication,
CLI release or Milestone 12 work is included.
