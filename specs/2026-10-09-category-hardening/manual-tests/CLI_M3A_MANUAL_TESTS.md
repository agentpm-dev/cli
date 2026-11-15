# M3A: Agent Package-first Scaffolding

Run **one block at a time in one new macOS/zsh terminal**, starting in the
`agentpm` repository. Keep the terminal open: all sections share its variables.
Claude review and all hosted CI have passed, as confirmed by the operator.
The operator confirmed all manual tests passed on **2026-11-15**, completing
M3A. Keep the CI links and local evidence with that acceptance; the instructions
below remain available for repeat verification.

These checks use your reviewed local debug binary, not Homebrew's `agentpm`.
They create disposable packages and evidence under a fresh temporary directory.
There is no live Registry, S3 upload, actual publication, author key, root
bootstrap, model call or paid embedding request. `publish` is used **only with
`--dry-run`**, a dummy token and a loopback base URL. Cargo may download build
dependencies if its cache is cold.

Python 3.11+ and the repository's Rust toolchain are required. The Python checks
use only the standard library; Docker, Node and the Registry Python venv are
not required. Node authoring is checked by linting and packaging a small authored
Tool, without executing Node. `init --kind tool` still creates Python only.

| Section | What you verify                                                               |
| ------- | ----------------------------------------------------------------------------- |
| 1       | Build, identify the binary and prepare temporary evidence capture             |
| 2       | Default Agent equals explicit Agent; valid without Loop, not Harness-runnable |
| 3       | Eight explicit kinds and both Knowledge modes declare the supported standard  |
| 4       | Descriptions round-trip; invalid names/descriptions leave no output           |
| 5       | Existing source, duplicate init and secondary-file collisions are safe        |
| 6       | Python echo execution and Python/Node dry-run archives                        |
| 7       | Knowledge/Memory build metadata and authored declarations stay intact         |
| 8       | Skill export, rejection before forced replacement, and Template generation    |
| 9       | Inspect docs, retain evidence and record acceptance                           |

## 1. Build and capture the reviewed CLI

Start from the **CLI repository root**, not the workspace parent, API repo or
examples repo. This path matches the normal local checkout; adapt only the `cd`
line if your checkout is elsewhere.

```bash
cd /Users/zackhine/projects/agentpm-project/agentpm
cargo build --locked -p agentpm-cli
export M3A_REPO="$PWD"
export M3A_APM="$M3A_REPO/target/debug/agentpm"
export M3A_PYTHON="$(command -v python3)"
export M3A_ROOT="$(mktemp -d "${TMPDIR:-/tmp}/agentpm-m3a.XXXXXX")"
mkdir -p "$M3A_ROOT/evidence"

m3a() {
  env AGENTPM_TELEMETRY=0 AGENTPM_TOKEN=apm_m3a.fixture NO_COLOR=1 \
    "$M3A_APM" --base-url http://127.0.0.1:9 "$@"
}

m3a_capture() {
  local m3a_label="$1" m3a_expected="$2" m3a_actual
  shift 2
  printf '%q ' "$@" > "$M3A_ROOT/evidence/$m3a_label.command.txt"
  printf '\n' >> "$M3A_ROOT/evidence/$m3a_label.command.txt"
  if "$@" > "$M3A_ROOT/evidence/$m3a_label.stdout.txt" \
           2> "$M3A_ROOT/evidence/$m3a_label.stderr.txt"; then
    m3a_actual=0
  else
    m3a_actual=$?
  fi
  cat "$M3A_ROOT/evidence/$m3a_label.stdout.txt"
  cat "$M3A_ROOT/evidence/$m3a_label.stderr.txt" >&2
  printf '{"exit_code":%s,"expected_exit_code":%s}\n' \
    "$m3a_actual" "$m3a_expected" > "$M3A_ROOT/evidence/$m3a_label.result.json"
  if [[ "$m3a_actual" != "$m3a_expected" ]]; then
    printf 'FAIL %s: exit %s, expected %s\n' "$m3a_label" "$m3a_actual" "$m3a_expected" >&2
    return 1
  fi
  printf 'PASS %s: exit %s (expected %s)\n' "$m3a_label" "$m3a_actual" "$m3a_expected"
}

m3a_capture version 0 m3a --version
m3a_capture help 0 m3a init --help
"$M3A_PYTHON" - <<'PY'
import hashlib, json, os, subprocess, sys
from pathlib import Path
assert sys.version_info >= (3, 11), 'Use Python 3.11+ and repeat setup in a fresh terminal'
repo = Path(os.environ['M3A_REPO'])
binary = Path(os.environ['M3A_APM'])
record = {
    'cli_path': str(binary),
    'cli_sha256': hashlib.sha256(binary.read_bytes()).hexdigest(),
    'cli_commit': subprocess.check_output(['git', '-C', str(repo), 'rev-parse', 'HEAD'], text=True).strip(),
    'cli_worktree': subprocess.check_output(['git', '-C', str(repo), 'status', '--short'], text=True),
    'python': sys.version,
}
root = Path(os.environ['M3A_ROOT'])
(root / 'evidence/setup.json').write_text(json.dumps(record, indent=2) + '\n')
print(json.dumps(record, indent=2))
print('Keep this workspace/evidence path:', root)
PY
```

Expected: `--kind` describes the default Agent Package, shows `[default: agent]`
and lists all eight possible values; `--name` defaults to `my-agent`. Record the
binary digest; an installed release
can have the same version number and different behavior. Do not rebuild or
switch binaries midway through this run; start a new root if the binary changes.

The capture function prints both streams and records command, stdout, stderr and
**actual/expected child exit codes**. Its own exit is 0 when those codes agree,
including an expected CLI failure. Stop on any `FAIL` or Python assertion.
You may inspect or rerun an individual block, but rerunning init into an existing
output intentionally fails: use a fresh setup/root for a complete repeat.

Your saved credentials are not replaced. The wrapper supplies a dummy token and
explicit base URL per command; it does not change `HOME` or save CLI settings.

## 2. Create the default Agent and check the no-Loop boundary

```bash
mkdir "$M3A_ROOT/default-agent"
cd "$M3A_ROOT/default-agent"
m3a_capture default-init 0 m3a init
cat agent.json
m3a_capture default-lint 0 m3a lint --strict
m3a_capture default-apds 0 m3a lint --apds standalone --format json
m3a_capture explicit-agent-init 0 m3a init --kind agent --out-dir "$M3A_ROOT/explicit-agent"
m3a_capture default-inspect 0 "$M3A_PYTHON" - <<'PY'
import json, os
from pathlib import Path
root = Path(os.environ['M3A_ROOT'])
raw = (root / 'default-agent/agent.json').read_bytes()
value = json.loads(raw)
assert raw == (root / 'explicit-agent/agent.json').read_bytes()
assert value['kind'] == 'agent' and value['name'] == 'my-agent'
assert value['version'] == '0.1.0'
assert value['description'] == "A reusable Agent Package defining an agent's capabilities and behavior."
assert value['standard'] == {'id': 'agentpm', 'version': '1.0.0'}
assert value['$schema'] == 'https://raw.githubusercontent.com/agentpm-dev/cli/refs/tags/apds-v1.0.0/standards/agentpm/1.0.0/manifest.schema.json'
assert 'loop' not in value
for field in ['tools', 'skills', 'knowledge', 'memory', 'profiles']:
    assert not value.get(field), field
assert not (root / 'default-agent/main.py').exists()
report = json.loads((root / 'evidence/default-apds.stdout.txt').read_text())
assert report[0]['apds']['status'] == 'conformant'
print('PASS: identical default/explicit Agent; supported standard, pinned schema, no forced Components or Loop.')
PY
```

Expected: exit 0, no missing-schema warning, and immediate next-step text about
Components, lint, a Loop and a model. The APDS JSON reports `conformant` at
`standalone` scope, not complete packaged/resolved or Runner conformance.

Now supply a **test-only local-root lock context** and run the actual Harness
preflight. This fixture is not an install result: empty-dependency install and
frozen behavior belong to later work. No model is configured or called.

```bash
"$M3A_PYTHON" - <<'PY'
import json, os
from pathlib import Path
root = Path(os.environ['M3A_ROOT']) / 'default-agent'
lock = {
    'lockfile_version': 2, 'generated': '2026-11-15T00:00:00Z', 'packages': {},
    'roots': {'local:agent': {'name': 'my-agent', 'version': '0.1.0', 'tools': []}},
}
(root / 'agent.lock').write_text(json.dumps(lock, indent=2) + '\n')
PY
m3a_capture no-loop 1 m3a harness --json
m3a_capture no-loop-inspect 0 "$M3A_PYTHON" - <<'PY'
import json, os
from pathlib import Path
text = (Path(os.environ['M3A_ROOT']) / 'evidence/no-loop.stdout.txt').read_text()
json.loads(text)
assert 'agent_missing_loop' in text
assert 'valid artifacts but are not runnable by Harness' in text
print('PASS: valid Agent definition; actual Harness correctly rejects missing Loop.')
PY
```

Expected: the CLI exits **1** with the no-Loop explanation, not a schema failure.
Other readiness findings may appear too. Use `--json` alone, not `--headless --json`.

## 3. Inspect all eight explicit kinds and both Knowledge modes

```bash
mkdir "$M3A_ROOT/kinds"
for M3A_KIND in agent tool skill knowledge memory profile loop template; do
  m3a_capture "kind-$M3A_KIND-init" 0 m3a init --kind "$M3A_KIND" \
    --out-dir "$M3A_ROOT/kinds/$M3A_KIND" || break
  cd "$M3A_ROOT/kinds/$M3A_KIND"
  m3a_capture "kind-$M3A_KIND-lint" 0 m3a lint --strict || break
  m3a_capture "kind-$M3A_KIND-apds" 0 m3a lint --apds standalone --format json || break
done
m3a_capture vector-init 0 m3a init --kind knowledge --mode vector \
  --out-dir "$M3A_ROOT/kinds/knowledge-vector"
cd "$M3A_ROOT/kinds/knowledge-vector"
m3a_capture vector-lint 0 m3a lint --strict
m3a_capture vector-apds 0 m3a lint --apds standalone --format json
m3a_capture scaffolds-inspect 0 "$M3A_PYTHON" - <<'PY'
import json, os
from pathlib import Path
root = Path(os.environ['M3A_ROOT'])
for name in ['agent', 'tool', 'skill', 'knowledge', 'memory', 'profile', 'loop', 'template', 'knowledge-vector']:
    kind = 'knowledge' if name == 'knowledge-vector' else name
    path = root / 'kinds' / name
    value = json.loads((path / 'agent.json').read_text())
    assert value['kind'] == kind and value['name'] == 'my-' + kind, name
    assert value['description'].strip(), name
    assert value['standard'] == {'id': 'agentpm', 'version': '1.0.0'}, name
    assert '/refs/tags/apds-v1.0.0/' in value['$schema'], name
    label = 'vector-apds' if name == 'knowledge-vector' else 'kind-' + kind + '-apds'
    report = json.loads((root / 'evidence' / (label + '.stdout.txt')).read_text())
    assert report[0]['apds']['status'] == 'conformant', name
    mode = value['knowledge']['mode'] if kind == 'knowledge' else ''
    print(kind, value['name'], mode, value['description'])
assert (root / 'kinds/tool/main.py').is_file()
assert (root / 'kinds/skill/SKILL.md').is_file()
assert (root / 'kinds/template/template/README.md').is_file()
assert not (root / 'kinds/template/template/agent.json').exists()
assert (root / 'kinds/knowledge/knowledge/docs/context.md').is_file()
assert (root / 'kinds/knowledge-vector/knowledge/embeddings').is_dir()
assert not (root / 'kinds/knowledge-vector/knowledge/embeddings/default.f32').exists()
assert (root / 'kinds/memory/schemas/user-preference.schema.json').is_file()
print('PASS: all explicit scaffolds and expected source resources.')
PY
```

Open a few generated files, especially Tool `main.py`, Skill `SKILL.md`, Memory's
schema and the Loop manifest. The vector starter deliberately contains no real
vectors and is **not yet publishable**; its standalone pass is the correct claim.

## 4. Check escaped descriptions and reject invalid input

Use a real newline, quotes, a backslash, Unicode and a literal template token:

```bash
"$M3A_PYTHON" - <<'PY'
import os
from pathlib import Path
text = 'Quotes " : backslash \\ / Unicode 😀 café\nsecond line {{AGENT_NAME}} {{SKILL_TITLE}}'
(Path(os.environ['M3A_ROOT']) / 'description.txt').write_text(text)
PY
export M3A_DESCRIPTION="$(cat "$M3A_ROOT/description.txt")"
m3a_capture escaped-agent-init 0 m3a init --description "$M3A_DESCRIPTION" \
  --out-dir "$M3A_ROOT/escaped-agent"
m3a_capture escaped-skill-init 0 m3a init --kind skill --description "$M3A_DESCRIPTION" \
  --out-dir "$M3A_ROOT/escaped-skill"
m3a_capture escaping-inspect 0 "$M3A_PYTHON" - <<'PY'
import json, os
from pathlib import Path
root = Path(os.environ['M3A_ROOT'])
expected = (root / 'description.txt').read_text()
for name in ['escaped-agent', 'escaped-skill']:
    assert json.loads((root / name / 'agent.json').read_text())['description'] == expected
markdown = (root / 'escaped-skill/SKILL.md').read_text()
scalar = next(line.removeprefix('description: ') for line in markdown.splitlines() if line.startswith('description: '))
assert json.loads(scalar) == expected
print('PASS: JSON and quoted Skill frontmatter preserve authored text and literal tokens.')
PY
m3a_capture bad-name 1 m3a init --name '../escape' --out-dir "$M3A_ROOT/rejected-name"
m3a_capture bad-description 1 m3a init --description '   ' --out-dir "$M3A_ROOT/rejected-description"
m3a_capture rejections-inspect 0 "$M3A_PYTHON" - <<'PY'
import os
from pathlib import Path
root = Path(os.environ['M3A_ROOT'])
assert not (root / 'rejected-name').exists()
assert not (root / 'rejected-description').exists()
for label in ['bad-name', 'bad-description']:
    text = (root / 'evidence' / (label + '.stderr.txt')).read_text()
    assert 'Cannot create package' in text
print('PASS: invalid input rejected deliberately, before creating output directories.')
PY
```

Expected failures identify the invalid name/description; no malformed manifest
or partial package is created. Look at `escaped-agent/agent.json` to confirm
ordinary JSON escaping, with identity fields still near the top.

## 5. Preserve existing source and reject collisions

```bash
mkdir "$M3A_ROOT/existing-source" "$M3A_ROOT/secondary-collision"
printf '# Authored source: keep this exact content.\n' > "$M3A_ROOT/existing-source/main.py"
printf 'Existing authored Skill instructions.\n' > "$M3A_ROOT/secondary-collision/SKILL.md"
m3a_capture existing-source-init 0 m3a init --kind tool --out-dir "$M3A_ROOT/existing-source"
"$M3A_PYTHON" - <<'PY'
import os, shutil
from pathlib import Path
root = Path(os.environ['M3A_ROOT'])
shutil.copyfile(root / 'existing-source/agent.json', root / 'evidence/existing-source-agent.before.json')
PY
m3a_capture duplicate-init 1 m3a init --kind tool --out-dir "$M3A_ROOT/existing-source"
m3a_capture secondary-collision 1 m3a init --kind skill --out-dir "$M3A_ROOT/secondary-collision"
m3a_capture preservation-inspect 0 "$M3A_PYTHON" - <<'PY'
import os
from pathlib import Path
root = Path(os.environ['M3A_ROOT'])
assert (root / 'existing-source/main.py').read_bytes() == b'# Authored source: keep this exact content.\n'
assert (root / 'existing-source/agent.json').read_bytes() == (root / 'evidence/existing-source-agent.before.json').read_bytes()
assert (root / 'secondary-collision/SKILL.md').read_bytes() == b'Existing authored Skill instructions.\n'
assert not (root / 'secondary-collision/agent.json').exists()
before = (root / 'evidence/existing-source-init.stdout.txt').read_text()
assert 'Kept existing' in before
assert 'does not overwrite existing files' in (root / 'evidence/duplicate-init.stderr.txt').read_text()
print('PASS: source kept; duplicate init and secondary collision rejected without replacement.')
PY
```

Expected: the first command keeps `main.py`; the repeated init and existing
`SKILL.md` fail with exit 1. Init has no overwrite flag. This is preflight and
per-file atomic-write behavior, not a promise of a directory transaction under
disk exhaustion or concurrent writers.

## 6. Execute Python and inspect Python/Node dry-run payloads

Use the fresh Python Tool from section 3, not the intentionally preserved comment
file from section 5. The generated echo script has no third-party dependencies.

```bash
cd "$M3A_ROOT/kinds/tool"
printf '%s\n' '{"message":"hello","nested":{"count":2}}' | \
  m3a_capture tool-echo 0 "$M3A_PYTHON" main.py
m3a_capture tool-package 0 m3a publish --dry-run --strict --quiet
"$M3A_PYTHON" - <<'PY'
import json, os
from pathlib import Path
root = Path(os.environ['M3A_ROOT'])
source = root / 'kinds/tool'
target = root / 'node-tool'
target.mkdir()
value = json.loads((source / 'agent.json').read_text())
value.update(name='node-check', files=['tool.mjs'], runtime={'type': 'node', 'version': '20'})
value['entrypoint'] = {'command': 'node', 'args': ['tool.mjs']}
(target / 'agent.json').write_text(json.dumps(value, indent=2) + '\n')
(target / 'tool.mjs').write_text('console.log(JSON.stringify({ok:true}));\n')
PY
cd "$M3A_ROOT/node-tool"
m3a_capture node-lint 0 m3a lint --strict
m3a_capture node-package 0 m3a publish --dry-run --strict --quiet
m3a_capture payloads-inspect 0 "$M3A_PYTHON" - <<'PY'
import json, os, tarfile
from pathlib import Path
root = Path(os.environ['M3A_ROOT'])
echo = json.loads((root / 'evidence/tool-echo.stdout.txt').read_text())
assert echo == {'message': 'hello', 'nested': {'count': 2}}
for directory, source in [('kinds/tool', 'main.py'), ('node-tool', 'tool.mjs')]:
    package = root / directory
    archives = list((package / 'target/agentpm').glob('*.tar.gz'))
    assert len(archives) == 1, (directory, archives)
    with tarfile.open(archives[0], 'r:gz') as archive:
        members = {item.name.removeprefix('./'): item for item in archive.getmembers()}
        embedded = json.load(archive.extractfile(members['agent.json']))
        assert embedded == json.loads((package / 'agent.json').read_text())
        assert embedded['standard'] == {'id': 'agentpm', 'version': '1.0.0'}
        assert archive.extractfile(members[source]).read() == (package / source).read_bytes()
    print('Archive:', archives[0])
print('PASS: Python echo; Python and authored Node archives preserve source and declarations.')
PY
```

Expected: Python echoes the request, both archives are created locally, and no
Registry connection or upload occurs. This confirms Node authoring/packaging
compatibility, **not** a Node init template or Node execution.

## 7. Build Knowledge and Memory without losing declarations

```bash
cd "$M3A_ROOT/kinds/knowledge"
m3a_capture knowledge-build 0 m3a knowledge build
cd "$M3A_ROOT/kinds/memory"
m3a_capture memory-build 0 m3a memory build
m3a_capture metadata-inspect 0 "$M3A_PYTHON" - <<'PY'
import json, os
from pathlib import Path
root = Path(os.environ['M3A_ROOT'])
for kind in ['knowledge', 'memory']:
    value = json.loads((root / 'kinds' / kind / 'agent.json').read_text())
    assert value['standard'] == {'id': 'agentpm', 'version': '1.0.0'}
    assert '/refs/tags/apds-v1.0.0/' in value['$schema']
assert (root / 'kinds/memory/memory/build.json').is_file()
assert (root / 'kinds/memory/memory/contracts/index.json').is_file()
print('PASS: content builds retain authored standard and schema; Memory outputs generated.')
PY
```

Expected: both builds pass; context Knowledge build requires no vectors, API key
or embedding request. The vector starter from section 3 is deliberately not built.

## 8. Export a Skill safely and generate a local Template

Prepare **test-only installed metadata** inside a disposable consumer. This
copies the already-linted local Tool into the existing installed layout and
writes a fixture lock; it does not claim to test Registry install or provenance.

```bash
"$M3A_PYTHON" - <<'PY'
import json, os, shutil
from pathlib import Path
root = Path(os.environ['M3A_ROOT'])
consumer = root / 'export-consumer'
installed = consumer / '.agentpm/tools/fixture/my-tool/0.1.0'
installed.mkdir(parents=True)
for name in ['agent.json', 'main.py']:
    shutil.copyfile(root / 'kinds/tool' / name, installed / name)
lock = {
    'lockfile_version': 2, 'generated': '2026-11-15T00:00:00Z',
    'packages': {'tool:@fixture/my-tool@0.1.0': {
        'kind': 'tool', 'name': '@fixture/my-tool', 'version': '0.1.0',
        'integrity': 'test-only-not-downloaded',
    }}, 'roots': {},
}
(consumer / 'agent.lock').write_text(json.dumps(lock, indent=2) + '\n')
PY
cd "$M3A_ROOT/export-consumer"
m3a_capture export 0 m3a export --skill @fixture/my-tool@0.1.0 --manifest --output exported
cd "$M3A_ROOT/export-consumer/exported"
cat agent.json
m3a_capture export-lint 0 m3a lint --strict
m3a_capture export-apds 0 m3a lint --apds standalone --format json
m3a_capture export-inspect 0 "$M3A_PYTHON" - <<'PY'
import json, os
from pathlib import Path
root = Path(os.environ['M3A_ROOT'])
output = root / 'export-consumer/exported'
value = json.loads((output / 'agent.json').read_text())
assert value['kind'] == 'skill'
assert value['standard'] == {'id': 'agentpm', 'version': '1.0.0'}
assert '/refs/tags/apds-v1.0.0/' in value['$schema']
assert value['tools'] == [{'name': '@fixture/my-tool', 'version': '0.1.0'}]
assert (output / 'SKILL.md').is_file()
assert (output / 'scripts/run.sh').is_file()
assert (root / 'export-consumer/.agentpm/tools/fixture/my-tool/0.1.0/agent.json').read_bytes() == (root / 'kinds/tool/agent.json').read_bytes()
report = json.loads((root / 'evidence/export-apds.stdout.txt').read_text())
assert report[0]['apds']['status'] == 'conformant'
print('PASS: exported Skill declares the standard; selected source manifest stays unchanged.')
PY
```

Now demonstrate the review's critical failure ordering. A 65-character scope
passes legacy installed-reference parsing but fails the generated Skill's
APDS dependency-name check. The previous export must survive `--force`.

```bash
"$M3A_PYTHON" - <<'PY'
import base64, json, os, shutil
from pathlib import Path
root = Path(os.environ['M3A_ROOT'])
consumer = root / 'export-consumer'
output = consumer / 'exported'
(output / 'SKILL.md').write_text('Authored instructions: preserve these.\n')
(output / 'references/operator-notes.bin').write_bytes(b'keep\x00\xff\n')
before = {path.relative_to(output).as_posix(): base64.b64encode(path.read_bytes()).decode()
          for path in output.rglob('*') if path.is_file()}
(root / 'evidence/export-before.json').write_text(json.dumps(before, indent=2) + '\n')
package = '@' + 'a' * 65 + '/my-tool'
installed = consumer / '.agentpm/tools' / ('a' * 65) / 'my-tool/0.1.0'
installed.mkdir(parents=True)
for name in ['agent.json', 'main.py']:
    shutil.copyfile(root / 'kinds/tool' / name, installed / name)
lock = {
    'lockfile_version': 1, 'generated': '2026-11-15T00:00:00Z',
    'dependencies': {package: {'version': '0.1.0', 'integrity': 'test-only-not-downloaded'}},
}
(consumer / 'agent.lock').write_text(json.dumps(lock, indent=2) + '\n')
(root / 'invalid-export-ref.txt').write_text(package + '@0.1.0')
PY
export M3A_INVALID_EXPORT_REF="$(cat "$M3A_ROOT/invalid-export-ref.txt")"
cd "$M3A_ROOT/export-consumer"
m3a_capture export-force-reject 1 m3a export --skill "$M3A_INVALID_EXPORT_REF" \
  --manifest --force --output exported
m3a_capture export-force-preserved 0 "$M3A_PYTHON" - <<'PY'
import base64, json, os
from pathlib import Path
root = Path(os.environ['M3A_ROOT'])
output = root / 'export-consumer/exported'
before = json.loads((root / 'evidence/export-before.json').read_text())
after = {path.relative_to(output).as_posix(): base64.b64encode(path.read_bytes()).decode()
         for path in output.rglob('*') if path.is_file()}
assert after == before, 'The previous export was deleted, replaced or partially changed'
message = (root / 'evidence/export-force-reject.stderr.txt').read_text()
assert 'Cannot create package' in message and '/tools/0' in message and 'APDS-ID-003' in message
print('PASS: generated manifest rejected before forced replacement; every existing file survives.')
PY
```

Expected: exit **1** at `/tools/0` (`APDS-ID-003`), not a connection/parsing error.
The schema reports the union-level path rather than `/tools/0/name`.

Finish with a local Template. Its source comes from section 3; add an authored
Agent manifest to its files tree, then exercise the actual `new` command.

```bash
"$M3A_PYTHON" - <<'PY'
import os, shutil
from pathlib import Path
root = Path(os.environ['M3A_ROOT'])
target = root / 'kinds/template/template/agents'
target.mkdir()
shutil.copyfile(root / 'explicit-agent/agent.json', target / 'starter.agent.json')
PY
cd "$M3A_ROOT/kinds/template"
m3a_capture template-new 0 m3a new . "$M3A_ROOT/template-consumer" --var project_name=generated --quiet
m3a_capture template-lint 0 m3a lint --apds standalone --format json "$M3A_ROOT/template-consumer/agents/starter.agent.json"
m3a_capture template-inspect 0 "$M3A_PYTHON" - <<'PY'
import json, os
from pathlib import Path
root = Path(os.environ['M3A_ROOT'])
assert json.loads((root / 'explicit-agent/agent.json').read_text()) == json.loads((root / 'template-consumer/agents/starter.agent.json').read_text())
report = json.loads((root / 'evidence/template-lint.stdout.txt').read_text())
assert report[0]['apds']['status'] == 'conformant'
print('PASS: Template new preserves the authored Agent declaration and generated Agent lints.')
PY
```

Expected: no Registry dependencies to fetch; the copied Agent keeps its declaration.
The explicit APDS scope accepts the `starter.agent.json` filename and produces
an actual validation report. Ordinary lint currently discovers `agent.json`
basenames; a zero-exit "No agent.json found" message is not validation evidence.
`new`'s synthesized consumer root/workspace metadata is a separate compatibility
surface. This test does not claim those generated records acquired `standard`.

## 9. Inspect docs and record acceptance

Open these **working-checkout** docs (or their local rendered pages), not an older
production deployment:

- `agentpm-api/docs/v0.1/cli/init.mdx`: default Agent, explicit Tool migration,
  supported declaration, pinned schema and no-Loop readiness.
- `agentpm-api/docs/v0.1/agent.json/introduction.mdx`: validated selector, editor
  hint and legacy ordinary-lint boundary; no opaque/reserved framing.
- `agentpm-api/docs/v0.1/publish-a-tool/init.mdx` and `agent.json.mdx`: displayed
  starting manifests agree with section 3's Python Tool, apart from the supplied
  name and description, and use the pinned schema plus `standard`.

The old source-doc copies in the published `agentpm-docs` Knowledge corpus are
deliberately outside this run. M17B owns regeneration/new publication and M17C
owns its final freshness checks.

Save a command-results summary and confirm you used one binary throughout:

```bash
"$M3A_PYTHON" - <<'PY'
import hashlib, json, os
from pathlib import Path
root = Path(os.environ['M3A_ROOT'])
evidence = root / 'evidence'
setup = json.loads((evidence / 'setup.json').read_text())
assert hashlib.sha256(Path(os.environ['M3A_APM']).read_bytes()).hexdigest() == setup['cli_sha256'], 'CLI changed; repeat with a fresh root'
required = ['version', 'help', 'default-init', 'default-lint', 'default-apds', 'explicit-agent-init',
            'default-inspect', 'no-loop', 'no-loop-inspect', 'vector-init', 'vector-lint', 'vector-apds',
            'scaffolds-inspect', 'escaped-agent-init', 'escaped-skill-init', 'escaping-inspect',
            'bad-name', 'bad-description', 'rejections-inspect', 'existing-source-init',
            'duplicate-init', 'secondary-collision', 'preservation-inspect', 'tool-echo',
            'tool-package', 'node-lint', 'node-package', 'payloads-inspect', 'knowledge-build',
            'memory-build', 'metadata-inspect', 'export', 'export-lint', 'export-apds',
            'export-inspect', 'export-force-reject', 'export-force-preserved',
            'template-new', 'template-lint', 'template-inspect']
required += ['kind-' + kind + '-' + action
             for kind in ['agent', 'tool', 'skill', 'knowledge', 'memory', 'profile', 'loop', 'template']
             for action in ['init', 'lint', 'apds']]
results = {}
for name in required:
    record = json.loads((evidence / (name + '.result.json')).read_text())
    assert record['exit_code'] == record['expected_exit_code'], (name, record)
    results[name] = record
(evidence / 'command-summary.json').write_text(json.dumps({
    'cli_sha256': setup['cli_sha256'], 'command_count': len(results),
    'status': 'all recorded command expectations passed', 'results': results,
}, indent=2) + '\n')
print('PASS:', len(results), 'recorded command expectations. Evidence:', evidence)
print('Operator still records CI links, docs inspection and final acceptance separately.')
PY
```

Record in the handoff:

- CLI/Registry docs CI links and reviewed commit(s), including any worktree changes.
- Temporary root and CLI SHA-256 from `evidence/setup.json`.
- Whether all sections passed, including the expected failures and doc inspection.
- Any unexpected output, skipped section or deviation, with its captured log path.

All command expectations passing is **not** a substitute for the operator's CI and
documentation checks. Only after those checks and this run pass should M3A be
marked complete. The public APDS tag and coordinated release remain separate
release-band gates.

Keep the temporary root until acceptance is recorded; there is nothing to clean
up in a Registry or bucket. Closing this test terminal drops the functions and
environment variables. Delete only the printed disposable root later if desired.
