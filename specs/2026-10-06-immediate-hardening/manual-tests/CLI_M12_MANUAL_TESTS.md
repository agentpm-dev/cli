# Milestone 12: Python Tool Payload Compatibility

Run **one block at a time** in a **new macOS/zsh terminal**, starting in the
`agentpm` repository. Keep that same terminal for the entire guide. Each section
states which disposable case to enter; **no second terminal is needed**.

This checks what goes into the archive and whether its payload is classified as
portable `any` or as your Mac's concrete target. It also checks the warnings and
errors an author sees. Dependency portability remains a separate concern.

The updated normal CI and all four Resolver Contract jobs are operator-confirmed
green for the initial reviewed implementation, including Windows after its
fixture correction. The subsequently authorized Node packaging correction needs
a fresh CI/native rerun after it is pushed. This walkthrough supplies operator
acceptance; it does not replace those native-platform tests.

| Sections | What changes |
| --- | --- |
| 1 | Build the CLI; create disposable inputs and isolated CLI state. |
| 2–7 | Create local archives, inspect their members, and exercise expected failures. |
| 8 | Run the full publish HTTP flow against a loopback capture fixture. |
| 9 | Dry-run disposable copies of both prepared real examples using their existing locks. |
| 10 | Check authored contracts, stop the fixture, and record results. |

**No backend, dev PAT, S3 access, real Registry publication, paid model call or
PostHog setup is needed.** The capture fixture stores requests and archives only
under the disposable directory. `Published` in its CLI receipt means the mock
completed the HTTP exchange; it does not create a real package version.

The fixture uses inert native/bytecode samples to test classification; do not
execute those samples. The prepared real examples are packaged but not run or
published here. Production `summarize-text`/`devwork-copilot` rollout remains
separate Release Band 8 work.

## 1. Build and Prepare the Disposable Cases

Start from the **`agentpm` repo root**. The sibling examples repo must contain
the reviewed `validate-order` and `summarize-text` manifests and `agent.lock`
files. Use the existing API venv's Python 3.11+; no Python packages are installed.

```bash
cargo build -p agentpm-cli
export M12_REPO="$PWD"
export M12_APM="$M12_REPO/target/debug/agentpm"
export M12_SCHEMA="$M12_REPO/schemas/agentpm.manifest.schema.json"
export M12_PYTHON="$M12_REPO/../agentpm-api/.venv/bin/python"
export M12_FIXTURES="$M12_REPO/specs/2026-10-06-immediate-hardening/manual-tests/fixtures/m12"
export M12_ROOT="$(mktemp -d "${TMPDIR:-/tmp}/agentpm-m12.XXXXXX")"
export M12_STATE="$M12_ROOT/state"
export M12_OFFLINE='http://127.0.0.1:1'
mkdir -p "$M12_STATE/config" "$M12_STATE/data" "$M12_STATE/cache" "$M12_STATE/AppData/Roaming" "$M12_STATE/AppData/Local"
unset M12_ENDPOINT M12_CAPTURE M12_CAPTURE_PID
"$M12_PYTHON" -c 'import sys; assert sys.version_info >= (3,11); print(sys.version)'
"$M12_PYTHON" "$M12_FIXTURES/prepare.py" "$M12_ROOT" "$M12_REPO/../agentpm-examples"
```

Expected: disposable cases are created; the helper does not execute AgentPM,
resolve dependencies, or publish anything. Port 1 must be closed; choose another
closed loopback port for `M12_OFFLINE` if necessary.

The dependency-free `files: ["."]` cases start without an optional author lock.
Otherwise an incidental `agent.lock` would itself be an unrecognized payload
format and narrow the archive, obscuring the collection-only check. The real
dependency-bearing examples retain their locks and existing author-lock handling.

Define these launch/inspection helpers in this **same terminal**:

```bash
m12() {
  env HOME="$M12_STATE" USERPROFILE="$M12_STATE" \
    XDG_CONFIG_HOME="$M12_STATE/config" XDG_DATA_HOME="$M12_STATE/data" \
    XDG_CACHE_HOME="$M12_STATE/cache" APPDATA="$M12_STATE/AppData/Roaming" \
    LOCALAPPDATA="$M12_STATE/AppData/Local" AGENTPM_TELEMETRY=0 \
    AGENTPM_UV_PATH="$M12_ROOT/must-not-execute" \
    "$M12_APM" --base-url "${M12_ENDPOINT:-$M12_OFFLINE}" "$@"
}
m12_publish() {
  m12 publish --token apm_m12.fixture --schema "$M12_SCHEMA" "$@"
}
m12_members() {
  "$M12_PYTHON" - <<'PY'
from pathlib import Path
import tarfile
paths = list(Path('target/agentpm').glob('*.tar.gz'))
assert len(paths) == 1, paths
with tarfile.open(paths[0]) as archive:
    for member in archive.getmembers():
        print(oct(member.mode), member.name, ('-> ' + member.linkname) if member.issym() else '')
PY
}
m12 --version
"$M12_PYTHON" - <<'PY'
import hashlib, os, platform
from pathlib import Path
assert platform.system() == 'Darwin', 'This command guide uses macOS/zsh.'
print('Expected concrete target:', {'arm64':'aarch64-apple-darwin', 'x86_64':'x86_64-apple-darwin'}[platform.machine()])
print('CLI SHA-256:', hashlib.sha256(Path(os.environ['M12_APM']).read_bytes()).hexdigest())
print('Disposable directory:', os.environ['M12_ROOT'])
PY
```

The CLI version may still be `0.1.36`; the absolute executable path selects the
new build. Your terminal's real HOME and credentials are unchanged. The dummy
token is accepted by the mock only. A deliberately nonexistent uv override
ensures these checks cannot accidentally acquire or execute a resolver. Matching
prepared locks must be reused in section 9.

## 2. Recognized Source, Data and Assets Stay Portable

```bash
cd "$M12_ROOT/cases/portable"
m12 lint --strict --schema "$M12_SCHEMA"
m12_publish --dry-run
m12_members
```

Expected:

- Lint and packaging succeed, with `Tool payload: any (format: tar.gz)` and no
  narrowing warnings.
- Members include Python source, mixed-case JSON/PNG, a Jinja template,
  `requirements.in`, `MANIFEST.in`, and `sample.egg-info/PKG-INFO`.
- Executable `tool.py` is still portable. The `.in` files are payload data;
  they do not become dependency declarations or build steps.
- No descriptor sidecar is added to the archive. `agent.json` is present;
  the small fixture's existing `agent.lock` is unchanged.

## 3. Directory Collection Omits Incidental State

```bash
cd "$M12_ROOT/cases/collection"
m12_publish --dry-run --quiet --strict > "$M12_ROOT/logs/collection.stdout" 2> "$M12_ROOT/logs/collection.stderr"
cat "$M12_ROOT/logs/collection.stderr"
m12_members
"$M12_PYTHON" - <<'PY'
import os, tarfile
from pathlib import Path
with tarfile.open(next(Path('target/agentpm').glob('*.tar.gz'))) as archive:
    names = set(archive.getnames())
assert {'agent.json', 'tool.py', 'keep.json', '.env.example', 'mypkg/source.py'} <= names
excluded = {'.git', '.venv', 'venv', 'site-packages', 'dist-packages', 'dev-python', 'mypkg/__pycache__'}
assert not any(n == p or n.startswith(p + '/') for n in names for p in excluded)
assert not ({'.env', '.env.local', '.env.sample', '.env.template', '.env.dist', '.env.example.local', '.DS_Store', 'Thumbs.db', 'desktop.ini'} & names)
root = Path(os.environ['M12_ROOT'])
stderr = (root / 'logs/collection.stderr').read_text()
assert 'Tool payload: any' in stderr and 'Info:' in stderr and 'Warning:' not in stderr
assert 'Info:' not in (root / 'logs/collection.stdout').read_text()
assert Path('.env').is_file() and Path('.venv/vendor/example.whl').is_file()
assert Path('dev-python/PYVENV.CFG').is_file()
print('Only archive selection changed; workspace files still exist.')
PY
```

Expected: `any` with **information** messages naming omitted paths/reasons,
including the custom environment recognized through uppercase `PYVENV.CFG`.
The wheel and invalid PNG inside excluded environments do not cause errors:
those trees are pruned before inspecting descendants. Messages remain visible
under quiet/strict/captured output. No guessed count of unseen files is reported.

Only exact `.env.example` is an incidental exception. `.env.sample`,
`.env.template`, `.env.dist`, and `.env.example.local` remain omitted. These
files contain fixture placeholders, not credentials.

## 4. Explicit Requests Win and Remain Visible

```bash
cd "$M12_ROOT/cases/overrides"
m12_publish --dry-run
m12_members
cd "$M12_ROOT/cases/overrides-reversed"
m12_publish --dry-run
"$M12_PYTHON" - <<'PY'
import os, tarfile
from pathlib import Path
root = Path(os.environ['M12_ROOT']) / 'cases'
def members(case):
    with tarfile.open(next((root / case / 'target/agentpm').glob('*.tar.gz'))) as archive:
        return archive.getnames()
first, reversed_order = members('overrides'), members('overrides-reversed')
assert first == reversed_order
assert len(first) == len(set(first))
assert {'.env', '.git/config.json', 'mypkg/__pycache__/cached.pyc', 'mypkg/__pycache__/.env.nested', 'mypkg/__pycache__/.git/config.json'} <= set(first)
assert '.env.local' not in first
print('Explicit children/subtrees survived; request order did not change member selection.')
PY
```

Expected: override **information** for intentionally included files/subtrees,
plus a native/compiled-content **warning** for `cached.pyc`; the payload is your
Mac's concrete target. Explicitly requesting the cache directory preserves its
whole subtree, including nested dotenv/Git content. Requesting just
`.git/config.json` does not include the rest of `.git`.

The two manifests deliberately have different `files` ordering. Compare member
selection, not archive bytes or manifest digests.

Explicit inclusion cannot bypass archive restrictions:

```bash
cd "$M12_ROOT/cases/explicit-wheel"
if m12_publish --dry-run; then M12_EXIT=0; else M12_EXIT=$?; fi
printf 'CLI exit: %s (expected 1: explicitly included wheel)\n' "$M12_EXIT"
test "$M12_EXIT" -eq 1
cd "$M12_ROOT/cases/missing"
if m12_publish --dry-run; then M12_EXIT=0; else M12_EXIT=$?; fi
printf 'CLI exit: %s (expected 1: missing explicit .env.missing)\n' "$M12_EXIT"
test "$M12_EXIT" -eq 1
```

Expected: the first names an embedded archive under `.venv`; the second names
the missing literal `manifest.files` path. Neither creates a usable archive.

## 5. Native, Unknown, Executable and Linked Members Narrow Honestly

Each block below starts in its own named case.

```bash
cd "$M12_ROOT/cases/native"
m12_publish --dry-run
m12_members
```

Expected: your Mac's concrete target, with native/compiled-content warnings for
versioned `.so`, `.so.` before a final `.json` suffix, and ELF bytes disguised
as `.py`. These inert samples are intentionally never executed.

```bash
cd "$M12_ROOT/cases/unknown"
m12_publish --dry-run --quiet --strict > "$M12_ROOT/logs/unknown.stdout" 2> "$M12_ROOT/logs/unknown.stderr"
cat "$M12_ROOT/logs/unknown.stderr"
"$M12_PYTHON" - <<'PY'
import os
from pathlib import Path
root = Path(os.environ['M12_ROOT'])
stderr = (root / 'logs/unknown.stderr').read_text()
paths = ['assets/a.ts', 'assets/image.tiff', 'assets/settings.pth', 'assets/z.rtf']
positions = [stderr.index('Warning: ' + p + ':') for p in paths]
assert positions == sorted(positions)
assert all(stderr.count('Warning: ' + p + ':') == 1 for p in paths)
assert 'Warning:' not in (root / 'logs/unknown.stdout').read_text()
print('Unknown-format warnings remain ordered and visible on stderr; strict did not fail.')
PY
```

Expected: success with unknown-format warnings and a concrete target. These
messages must not claim that `.ts` proves a Node requirement or that `.rtf` is
native code. No publish `--machine` option exists in this milestone.

```bash
cd "$M12_ROOT/cases/permissions"
m12_publish --dry-run
m12_members
chmod 644 data.csv
m12_publish --dry-run
m12_members
```

Expected: first, executable `data.csv` narrows with an executable-member warning;
after removing that bit, the payload becomes `any`. Executable `tool.py` retains
its exception. Unix preserves executable bits; Windows assigns ordinary text
`0644`, so this first result is intentionally publisher-platform-dependent.
M16's future mixed-claim gate is documented, not implemented by this guide.

```bash
cd "$M12_ROOT/cases/symlink"
m12_publish --dry-run
m12_members
```

Expected: a concrete target with an unsupported-member-type warning. The archive
contains `data-link.json` **as a link**, not the external target's contents.

## 6. Preservation Boundaries and Other Runtimes

```bash
cd "$M12_ROOT/cases/boundaries"
m12_publish --dry-run
m12_members
```

Expected: `_vendor/helper.py`, `.egg-info`, `.dist-info`, Makefile, Dockerfile,
CODEOWNERS, `.gitignore`, `.editorconfig` and `.dockerignore` remain present.
`.dist-info` intentionally produces installed-state narrowing; it is not a
collection exclusion. Unrecognized project filenames produce their own warnings.
`keep.json` is present despite the ignore-file contents. This packager does not
interpret Git or Docker ignore rules.

Check that a required entrypoint under an excluded directory is preserved alone:

```bash
cd "$M12_ROOT/cases/required-entrypoint"
m12_publish --dry-run 2> "$M12_ROOT/logs/required-entrypoint.stderr"
cat "$M12_ROOT/logs/required-entrypoint.stderr"
m12_members
"$M12_PYTHON" - <<'PY'
import os, platform, tarfile
from pathlib import Path
with tarfile.open(next(Path('target/agentpm').glob('*.tar.gz'))) as archive:
    names = archive.getnames()
assert '__pycache__/tool.py' in names
assert '__pycache__/other.pyc' not in names
stderr = (Path(os.environ['M12_ROOT']) / 'logs/required-entrypoint.stderr').read_text()
target = {'arm64':'aarch64-apple-darwin', 'x86_64':'x86_64-apple-darwin'}[platform.machine()]
assert f'Tool payload: {target} (format: tar.gz)' in stderr
assert 'Warning: __pycache__/tool.py: contains installed Python environment state' in stderr
assert 'Info: __pycache__:' in stderr and 'Info: __pycache__/tool.py:' in stderr
print('Required source survived with installed-state narrowing; unrelated cache content did not.')
PY
```

Expected: your Mac's **concrete target**, with an installed-state warning for
`__pycache__/tool.py` and omission/override information. Preserving required
source does not exempt its installed-state path from classification or
implicitly re-enable the entire excluded cache tree. The `.py` executable-mode
exception does not bypass installed-state checks.

The new rules apply only to Python Tools:

```bash
cd "$M12_REPO"
cargo build -p agentpm-cli
cd "$M12_ROOT/cases/node"
m12_publish --dry-run
m12_publish --dry-run
m12_members
"$M12_PYTHON" - <<'PY'
import tarfile
from pathlib import Path
with tarfile.open(next(Path('target/agentpm').glob('*.tar.gz'))) as archive:
    names = archive.getnames()
assert len(names) == len(set(names)), names
assert names.count('agent.json') == 1 and names.count('tool.js') == 1
assert not any(n.startswith('./') or n.startswith('target/agentpm/') for n in names), names
assert {'.env', '.git/config.json', '.venv/data.txt'} <= set(names)
print('Node selection stayed intact; names are unique and generated outputs are absent.')
PY
```

Expected: success without `Tool payload:` classification or new collection
information; the named Node fixture files remain packaged. Every member appears
once, with no generated archive/temp file under `target/agentpm/`. Both packaging
attempts succeed, including when an earlier archive already exists.

The rebuild picks up the separately authorized Node path-normalization fix. If
you already ran this step with the buggy binary, rerun this block in your current
terminal; keep the same `M12_ROOT` and helpers. The old archive is replaced safely;
no fixture reset or manual archive deletion is needed. This correction keeps
Node's existing incidental-file selection and adds no Python compatibility rules.

## 7. A Format Mismatch Fails Without Replacing a Good Archive

First create a valid PNG archive, then replace only the asset contents:

```bash
cd "$M12_ROOT/cases/mismatch"
m12_publish --dry-run
cp target/agentpm/*.tar.gz "$M12_ROOT/good-mismatch.tar.gz"
cp "$M12_ROOT/inputs/wrong-format.gif" icon.png
if m12_publish --dry-run --quiet; then M12_EXIT=0; else M12_EXIT=$?; fi
printf 'CLI exit: %s (expected 1: PNG signature mismatch)\n' "$M12_EXIT"
test "$M12_EXIT" -eq 1
cmp "$M12_ROOT/good-mismatch.tar.gz" target/agentpm/*.tar.gz
test ! -e target/agentpm/m12-mismatch-0.1.0-python311.tar.gz.tmp
```

Expected: an error names `icon.png`, expected PNG recognition, and the remedy:
rename the file to its actual format or replace it with valid PNG data, updating
references if renamed. `cmp` prints nothing: the earlier valid archive survived.
The `.tmp` file is absent. Quiet mode does not hide the error.

This same check applies to a later version of a previously published Tool.
Existing published bytes are not reclassified on read/install. Renaming to an
unrecognized actual format can clear this error while still narrowing the next
archive; it does not guarantee `any`. Native CI covers that rename regression.
Leave this invalid `icon.png` in place for the live failure check next.

## 8. Inspect Live Publish Requests Without a Registry Publication

Stay in this terminal. Start the loopback capture fixture in the background:

```bash
"$M12_PYTHON" "$M12_FIXTURES/capture.py" "$M12_ROOT/capture" > "$M12_ROOT/logs/capture.log" 2>&1 &
export M12_CAPTURE_PID=$!
"$M12_PYTHON" - <<'PY'
import json, os, time
from pathlib import Path
path = Path(os.environ['M12_ROOT']) / 'capture/server.json'
for _ in range(50):
    try:
        value = json.loads(path.read_text())
        print('Capture ready:', value['base_url'])
        break
    except (FileNotFoundError, json.JSONDecodeError):
        time.sleep(0.1)
else:
    raise SystemExit('Capture did not start; inspect logs/capture.log before continuing.')
PY
export M12_CAPTURE="$("$M12_PYTHON" -c 'import json,os; from pathlib import Path; print(json.loads((Path(os.environ["M12_ROOT"])/"capture/server.json").read_text())["base_url"])')"
curl --fail --show-error "$M12_CAPTURE/health"
```

Expected: a dynamically selected `http://127.0.0.1:<port>` and a fixture health
response. No fixed port to edit and no second-terminal commands to reconstruct.

Run three successful live HTTP exchanges, each from its specified case:

```bash
cd "$M12_ROOT/cases/portable"
M12_ENDPOINT="$M12_CAPTURE" m12_publish --quiet --strict
cd "$M12_ROOT/cases/collection"
M12_ENDPOINT="$M12_CAPTURE" m12_publish --quiet --strict
cd "$M12_ROOT/cases/native"
M12_ENDPOINT="$M12_CAPTURE" m12_publish --quiet --strict
"$M12_PYTHON" - <<'PY'
import json, os
from pathlib import Path
capture = Path(os.environ['M12_ROOT']) / 'capture'
directories = sorted(capture.glob('upload-*'))
assert len(directories) == 3
for index, directory in enumerate(directories):
    value = json.loads((directory / 'metadata.json').read_text())
    artifact = value['artifact']
    diagnostics = artifact['diagnostics']
    members = {m['name'] for m in json.loads((directory / 'members.json').read_text())}
    assert (directory / 'finalize.json').is_file()
    assert artifact['format'] == 'tar.gz'
    assert value['client']['os'] == 'macos'
    assert 'artifact' not in value['manifest']
    if index == 0:
        assert artifact['target'] == 'any' and diagnostics == []
    elif index == 1:
        assert artifact['target'] == 'any' and diagnostics
        assert all(d['severity'] == 'info' for d in diagnostics)
        assert all(d.get('rule') for d in diagnostics)
        assert '.env.example' in members and '.env' not in members
        assert not any(n.startswith('.venv/') for n in members)
    else:
        assert artifact['target'] != 'any'
        assert diagnostics and all(d['code'] == 'native_content' for d in diagnostics)
    print(value['manifest']['name'], artifact['target'], 'diagnostics:', len(diagnostics))
    for diagnostic in diagnostics:
        print(' ', diagnostic['severity'], diagnostic['code'], diagnostic['path'], diagnostic.get('rule', ''))
events = [json.loads(line) for line in (capture / 'events.jsonl').read_text().splitlines()]
assert len(events) == 9, events
print('Three init/upload/finalize exchanges captured; publisher telemetry stayed separate.')
PY
```

Expected: `any`, `any` with collection information, and a concrete target with
native warnings. Inspect the saved `metadata.json`, `members.json`, and
`object.tar.gz` under `capture/upload-1` through `upload-3` if desired. These
capture-only JSON files are not new authored files or Tool archive members.

Now prove a live format failure reaches **none** of those HTTP stages:

```bash
cd "$M12_ROOT/cases/mismatch"
cp "$M12_ROOT/capture/events.jsonl" "$M12_ROOT/events-before-error.jsonl"
if M12_ENDPOINT="$M12_CAPTURE" m12_publish --quiet; then M12_EXIT=0; else M12_EXIT=$?; fi
printf 'CLI exit: %s (expected 1: error before publish-init)\n' "$M12_EXIT"
test "$M12_EXIT" -eq 1
cmp "$M12_ROOT/events-before-error.jsonl" "$M12_ROOT/capture/events.jsonl"
cmp "$M12_ROOT/good-mismatch.tar.gz" target/agentpm/*.tar.gz
```

Expected: the same PNG error; both comparisons print nothing. The prior valid
archive survives and no init/upload/finalize request was made.

## 9. Recheck the Two Prepared Dependency-Bearing Examples

These are disposable copies made in section 1, not your working examples repo.
Their real, reviewed Python resolutions must already exist in `agent.lock`.

```bash
cd "$M12_ROOT/cases/example-validate-order"
m12 lint --strict --schema "$M12_SCHEMA"
m12_publish --dry-run
m12_members
cd "$M12_ROOT/cases/example-summarize-text"
m12 lint --strict --schema "$M12_SCHEMA"
m12_publish --dry-run
m12_members
```

Expected for both:

- Lint and packaging pass with `Tool payload: any (format: tar.gz)` and no
  compatibility warnings.
- Python preparation reuses the matching lock. The nonexistent uv override
  causes no error; no resolver acquisition or PyPI access is needed.
- `agentpm-python.json` is included, while the author `agent.lock` and development
  environment are not packaged. Declared dependencies do not narrow the payload.
- `summarize-text` remains prepared version **0.1.9**. Neither example is
  published or executed, so no `OPENAI_API_KEY` is needed.

If this fails with a stale/missing resolution, stop and inspect the prepared
example inputs. Do not work around it by refreshing dependencies during this
classification walkthrough.

## 10. Confirm Authored State and Record Acceptance

```bash
"$M12_PYTHON" - <<'PY'
import hashlib, json, os, tarfile
from pathlib import Path
root = Path(os.environ['M12_ROOT'])
for relative, expected in json.loads((root / 'authored-before.json').read_text()).items():
    path = root / relative
    if expected is None:
        assert not path.exists(), relative
    else:
        assert hashlib.sha256(path.read_bytes()).hexdigest() == expected, relative
for name in ['validate-order', 'summarize-text']:
    case = root / 'cases' / ('example-' + name)
    source = Path(os.environ['M12_REPO']).parent / 'agentpm-examples/tools-python' / name
    for member in ['agent.json', 'agent.lock']:
        assert (case / member).read_bytes() == (source / member).read_bytes()
    with tarfile.open(next((case / 'target/agentpm').glob('*.tar.gz'))) as archive:
        names = archive.getnames()
    assert 'agentpm-python.json' in names and 'agent.lock' not in names
    assert not any(n.startswith('.venv/') for n in names)
assert (root / 'outside.json').read_text() == '{"outside":true}\n'
print('All authored manifests/locks and the outside symlink target stayed unchanged.')
print('Results retained at:', root)
PY
kill "$M12_CAPTURE_PID"
wait "$M12_CAPTURE_PID" 2>/dev/null || true
unset M12_ENDPOINT M12_CAPTURE_PID
cd "$M12_REPO"
```

Expected: all assertions pass. The generated files remain available for inspection;
no cleanup command deletes your working files. Source assets were changed only
where the walkthrough explicitly requested the disposable PNG/permission edits.

Operator-confirmed complete: all manual tests below passed after the Node
correction. Retain this checklist as the acceptance record:

- [x] Portable source/assets and both prepared dependency-bearing examples report `any`.
- [x] Native, unknown, executable, symlink and installed-state cases have accurate warnings/targets.
- [x] Incidental exclusions, required entrypoint preservation, explicit overrides and request-order checks pass.
- [x] Node keeps its existing incidental-file selection, with unique members and no generated output after the correction.
- [x] Expected missing-file, embedded-wheel and format-mismatch failures are clear; prior good archives survive.
- [x] Quiet/strict/captured stderr retains information/warnings/errors with the intended severities.
- [x] Live mock descriptors and uploaded members agree; invalid live packaging sends no request.
- [x] Authored manifests/locks stay unchanged; no real publication or example rollout was performed.

Operator acceptance is recorded in `test-plan.md`. Fresh normal CI/native jobs
after the Node correction remain a separate M12 gate; Release Band 8 production
publication/install/run verification also remains separate.
