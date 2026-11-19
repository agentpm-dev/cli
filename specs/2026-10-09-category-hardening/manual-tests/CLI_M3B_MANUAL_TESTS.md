# M3B: APDS-aware Authoring and Legacy Lint

Run **one block at a time in one macOS/zsh terminal**, starting in your local
`agentpm` checkout. Claude review and hosted CI have passed, as confirmed by the
operator. The operator confirmed all manual tests passed on **2026-11-17**,
completing M3B. The instructions remain available for repeat verification.

All test commands run inside **`agentpm/cli-m3b-test/<run-date>/`**, not a temporary
system folder. That directory is gitignored. You can open it in your editor and
look at the exact files being checked. The reusable fixture inputs live beside
this guide in [fixtures/m3b](fixtures/m3b/README.md). Setup copies them; tests do
not edit the checked-in fixtures or the APDS standard corpus.

These tests use your reviewed **local debug CLI**, not Homebrew's installed
version. You need the repo's Rust toolchain and Python 3.11+. There is no Registry
or S3 access, real publication, signing key, model call, Docker or paid service.
The only `publish` commands below use `--dry-run`, a dummy token and an explicit
loopback base URL. Cargo may fetch dependencies when building the binary.

| Section | What you are checking                                                           |
| ------- | ------------------------------------------------------------------------------- |
| 1       | Build the reviewed CLI; copy visible cases into a local test workspace          |
| 2       | A valid empty Agent passes standalone lint without claiming Runner readiness    |
| 3       | Missing, unsupported and malformed standard declarations are different failures |
| 4       | Legacy preview is explicit/read-only and cannot authorize publication           |
| 5       | Individual schema and semantic defects produce the expected rule and field path |
| 6       | JSON/NDJSON, malformed JSON and a missing filename produce honest results       |
| 7       | Missing package/graph context differs from a real content defect                |
| 8       | Safe editor-hint fixes and deliberate local migration                           |
| 9       | Knowledge/Memory build and inspect share strict authoring preflight             |
| 10      | Template generation declares the root and explains undeclared nested Agents     |
| 11      | Check saved evidence and record acceptance                                      |

For every check, compare **the described issue, actual output and exit code**.
A negative test passes when the CLI rejects the intended defect. An unrelated
error such as a missing executable is not a pass just because its exit is nonzero.
Stop if the displayed rule/path/status differs from the expectation.

## 1. Build and create the local test workspace

Start in the **CLI repository root**. Only adjust the first `cd` if your checkout
is somewhere else. Keep this terminal open; later blocks use these variables.

```bash
cd /Users/zackhine/projects/agentpm-project/agentpm
cargo build --locked -p agentpm-cli
export M3B_REPO="$PWD"
export M3B_APM="$M3B_REPO/target/debug/agentpm"
export M3B_PYTHON="$(command -v python3)"
export M3B_FIXTURES="$M3B_REPO/specs/2026-10-09-category-hardening/manual-tests/fixtures/m3b"
export M3B_ROOT="$M3B_REPO/cli-m3b-test/$(date -u +%Y%m%dT%H%M%SZ)"
mkdir -p "$M3B_REPO/cli-m3b-test"
mkdir "$M3B_ROOT"

m3b() {
  env AGENTPM_TELEMETRY=0 AGENTPM_TOKEN=apm_m3b.fixture NO_COLOR=1 \
    "$M3B_APM" --base-url http://127.0.0.1:9 "$@"
}

m3b_expect() {
  local m3b_label="$1" m3b_expected="$2" m3b_actual
  shift 2
  pwd > "$M3B_ROOT/evidence/$m3b_label.cwd.txt"
  printf '%q ' "$@" > "$M3B_ROOT/evidence/$m3b_label.command.txt"
  printf '\n' >> "$M3B_ROOT/evidence/$m3b_label.command.txt"
  if "$@" > "$M3B_ROOT/evidence/$m3b_label.stdout.txt" \
           2> "$M3B_ROOT/evidence/$m3b_label.stderr.txt"; then
    m3b_actual=0
  else
    m3b_actual=$?
  fi
  cat "$M3B_ROOT/evidence/$m3b_label.stdout.txt"
  cat "$M3B_ROOT/evidence/$m3b_label.stderr.txt" >&2
  printf '{"exit_code":%s,"expected_exit_code":%s}\n' \
    "$m3b_actual" "$m3b_expected" > "$M3B_ROOT/evidence/$m3b_label.result.json"
  printf 'ACTUAL exit: %s | EXPECTED exit: %s | %s\n' \
    "$m3b_actual" "$m3b_expected" "$m3b_label"
  if [[ "$m3b_actual" != "$m3b_expected" ]]; then
    printf 'FAIL: stop and inspect the output above.\n' >&2
    return 1
  fi
}

"$M3B_PYTHON" - <<'PY'
import hashlib, json, os, subprocess, sys
from pathlib import Path
assert sys.version_info >= (3, 11), 'Use Python 3.11+'
root = Path(os.environ['M3B_ROOT'])
for name in ['cases', 'projects', 'evidence']:
    (root / name).mkdir()
sources = Path(os.environ['M3B_FIXTURES'])
inputs = sorted(sources.glob('*.fixture.*'))
assert len(inputs) == 17, 'Unexpected fixture inventory; stop and check this guide'
for source in inputs:
    name = source.name.split('.fixture.')[0]
    folder = root / 'cases' / name
    folder.mkdir()
    (folder / 'agent.json').write_bytes(source.read_bytes())
repo = Path(os.environ['M3B_REPO'])
binary = Path(os.environ['M3B_APM'])
record = {
    'cli_path': str(binary),
    'cli_sha256': hashlib.sha256(binary.read_bytes()).hexdigest(),
    'commit': subprocess.check_output(['git', '-C', str(repo), 'rev-parse', 'HEAD'], text=True).strip(),
    'worktree': subprocess.check_output(['git', '-C', str(repo), 'status', '--short'], text=True),
    'python': sys.version,
    'workspace': str(root),
}
(root / 'evidence/setup.json').write_text(json.dumps(record, indent=2) + '\n')
print('Copied 17 fixtures. No test command has run yet.')
print('Open this local folder in your editor:', root)
print('Evidence:', root / 'evidence')
PY
cd "$M3B_ROOT"
pwd
ls cases
m3b_expect version 0 m3b --version
m3b_expect lint-help 0 m3b lint --help
git -C "$M3B_REPO" check-ignore "$M3B_ROOT/evidence/setup.json"
```

Expected: the printed path is inside `agentpm/cli-m3b-test/`; all 17 named case
folders are visible. `check-ignore` prints the evidence path. Help includes
`--legacy`, described as read-only compatibility for undeclared historical
manifests. Do not switch/rebuild binaries midway through a run; start a fresh
workspace if the binary changes.

`m3b` only selects the debug binary and supplies local test settings. It does not
edit a manifest or contact a real Registry. `m3b_expect NAME EXIT COMMAND...`
runs the exact command shown, displays both streams, saves them separately and
prints the **actual and expected child exit codes**. Its own return is successful
when those codes agree, including an expected CLI failure. It does not itself
decide whether the correct rule/path was reported; you inspect that below, and
the final evidence check verifies the main diagnostic expectations again.

## 2. A valid Agent Package

**Input:** `cases/valid-agent/agent.json` declares AgentPM 1.0.0 and has an empty
Tools list. It has no Loop. There is no intentional manifest defect.

```bash
cd "$M3B_ROOT/cases/valid-agent"
cat agent.json
m3b_expect valid-human 0 m3b lint --strict
m3b_expect valid-json 0 m3b lint --strict --format json
```

Expected: human output includes `APDS standalone scope: conformant; Runner
behavior not evaluated.` JSON contains `ok: true`, `issues: []`,
`apds.level: "standalone"`, `apds.status: "conformant"` and
`apds.runner_evaluated: false`. **No file should change.**

This means the definition passes standalone checks. It does not mean an empty
Agent has a runnable workflow, or that the CLI has resolved dependencies or
checked complete packaged content. This section does not invoke the Harness.

## 3. Three different standard-selector problems

Return to the test workspace root. Open each file before running its lint.

### 3.1 No declaration

**Issue:** `standard` is absent. The rest matches the valid Agent.

```bash
cd "$M3B_ROOT"
cat cases/missing-standard/agent.json
m3b_expect missing-standard 1 m3b lint cases/missing-standard/agent.json
```

Expected: **`APDS-ID-001` at `/standard`**, status `nonconformant`, and guidance
to review this local manifest and add `{"id":"agentpm","version":"1.0.0"}`.
The CLI must not add it automatically.

### 3.2 A declaration this CLI does not support

**Issue:** `standard.version` is `9.0.0`, not the supported `1.0.0`.

```bash
cat cases/unsupported-version/agent.json
m3b_expect unsupported-version 1 m3b lint cases/unsupported-version/agent.json
```

Expected: **`APDS-ID-001` at `/standard/version`**, status **`unsupported`**,
and guidance to use a supporting CLI or review the declaration. It must not
silently reinterpret the input as version 1.0.0.

### 3.3 A malformed declaration

**Issue:** `standard` is present but is `null`, rather than an object with an ID
and version.

```bash
cat cases/malformed-standard/agent.json
m3b_expect malformed-standard 1 m3b lint cases/malformed-standard/agent.json
```

Expected: **`APDS-ID-001` at `/standard`**, status **`nonconformant`**. This is
an invalid declaration, not missing metadata or an unsupported valid selector.
All three source files remain unchanged.

## 4. Explicit legacy preview is not publish authorization

Use the same missing-standard case, without editing it.

```bash
m3b_expect legacy-human 0 m3b lint --legacy cases/missing-standard/agent.json
m3b_expect legacy-json 0 m3b lint --legacy cases/missing-standard/agent.json --format json
```

Expected: human output says **legacy compatibility only**, identifies the
inferred AgentPM 1.0.0, says APDS conformance and Runner behavior were not
evaluated, and says the source is unchanged. JSON has
`compatibility: "legacy-unverified"` and **no `apds` property**. It still uses
the same engine; only the missing selector was assumed in memory.

Now check what the flag cannot hide:

```bash
cat cases/legacy-blank-description/agent.json
m3b_expect legacy-invalid 1 m3b lint --legacy cases/legacy-blank-description/agent.json
m3b_expect legacy-unsupported 1 m3b lint --legacy cases/unsupported-version/agent.json --format json
m3b_expect legacy-malformed 1 m3b lint --legacy cases/malformed-standard/agent.json --format json
m3b_expect legacy-write-conflict 2 m3b lint --legacy --fix cases/missing-standard/agent.json
```

Expected, in order:

- Blank description remains an error: **`APDS-ID-002` at `/description`**, with
  the legacy compatibility label. The flag is not a general validation bypass.
- Unsupported version stays `unsupported` with an `apds` result; no inferred
  `compatibility` label is added.
- Present-but-null declaration stays `nonconformant`, without a legacy label.
- `--legacy --fix` is a **usage error, exit 2**. It must not write anything.

Finally try authoring preflight on the undeclared source:

```bash
m3b_expect package-rejects-legacy 1 m3b package --manifest cases/missing-standard/agent.json --machine
m3b_expect publish-rejects-legacy 1 m3b publish --manifest cases/missing-standard/agent.json --dry-run --quiet
```

Both must reject **`APDS-ID-001` at `/standard`**. Package's error is a machine
envelope; publish reports its validation error on stderr. Neither command has a
legacy bypass. No archive is produced by these failed checks. Every command in
this section is read-only with respect to the case manifests.

## 5. Read each defect and its rule/path

Each case below is separate. **Run one `cat`/lint pair at a time**, then compare
the output against its row. All expect **exit 1**, `nonconformant`, the named
source file and no source edits.

| Case folder                     | The authored issue to look for                                          | Required finding                                                                                                                                      |
| ------------------------------- | ----------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------- |
| `whitespace-description`        | `description` contains only space, tab and newline                      | `APDS-ID-002` at `/description`                                                                                                                       |
| `invalid-name`                  | Local `name` is `Bad Name`                                              | `APDS-ID-002` at `/name`                                                                                                                              |
| `unknown-kind`                  | `kind` has an unsupported value                                         | `APDS-GRAPH-001` at `/kind`                                                                                                                           |
| `dependency-duplicate-identity` | Third Tools entry repeats an earlier identity                           | `APDS-GRAPH-003` at `/tools/2`                                                                                                                        |
| `binding-undeclared`            | Second globally bound Tool was not declared in the top-level Tools list | `APDS-BIND-001` at `/bindings/global/tools/1`                                                                                                         |
| `loop-entry-unknown`            | `entry_phase` is `missing`, which is not a declared phase               | `APDS-LOOP-001` at `/loop/entry_phase`; this also produces `APDS-LOOP-002` at `/loop/transitions` because the graph cannot be reached from that entry |
| `memory-scope-unknown`          | The first scope in `memory.spaces.notes.scope` is not allowed           | `APDS-MEM-001` at `/memory/spaces/notes/scope/0`                                                                                                      |
| `template-variable-duplicate`   | Second variable repeats the first variable's name                       | `APDS-TEMPLATE-001` at `/template/variables/1/name`                                                                                                   |
| `tool-runtime-mismatch`         | Runtime is Python, but the entrypoint command is Node                   | `APDS-TOOL-002` at `/runtime/type`                                                                                                                    |

```bash
cd "$M3B_ROOT"
cat cases/whitespace-description/agent.json
m3b_expect whitespace-description 1 m3b lint cases/whitespace-description/agent.json

cat cases/invalid-name/agent.json
m3b_expect invalid-name 1 m3b lint cases/invalid-name/agent.json

cat cases/unknown-kind/agent.json
m3b_expect unknown-kind 1 m3b lint cases/unknown-kind/agent.json

cat cases/dependency-duplicate-identity/agent.json
m3b_expect dependency-duplicate-identity 1 m3b lint cases/dependency-duplicate-identity/agent.json

cat cases/binding-undeclared/agent.json
m3b_expect binding-undeclared 1 m3b lint cases/binding-undeclared/agent.json

cat cases/loop-entry-unknown/agent.json
m3b_expect loop-entry-unknown 1 m3b lint cases/loop-entry-unknown/agent.json

cat cases/memory-scope-unknown/agent.json
m3b_expect memory-scope-unknown 1 m3b lint cases/memory-scope-unknown/agent.json

cat cases/template-variable-duplicate/agent.json
m3b_expect template-variable-duplicate 1 m3b lint cases/template-variable-duplicate/agent.json

cat cases/tool-runtime-mismatch/agent.json
m3b_expect tool-runtime-mismatch 1 m3b lint cases/tool-runtime-mismatch/agent.json
```

The Loop can report more than one consequence of the same authored defect.
Do not interpret an evaluated-rule list as proof that every clause of the
standard or any Runner behavior was exercised.

## 6. Machine output and broken input

**Issue:** the duplicate-Tool manifest is unchanged. This time inspect the
structured finding instead of the human renderer.

```bash
m3b_expect duplicate-json 1 m3b lint cases/dependency-duplicate-identity/agent.json --format json
m3b_expect duplicate-ndjson 1 m3b lint cases/dependency-duplicate-identity/agent.json --format ndjson
"$M3B_PYTHON" - <<'PY'
import json, os
from pathlib import Path
e = Path(os.environ['M3B_ROOT']) / 'evidence'
array = json.loads((e / 'duplicate-json.stdout.txt').read_text())
line = json.loads((e / 'duplicate-ndjson.stdout.txt').read_text())
assert len(array) == 1 and array[0] == line
finding = next(i for i in line['issues'] if i.get('rule_id') == 'APDS-GRAPH-003')
assert finding['instance_path'] == '/tools/2'
assert line['apds']['status'] == 'nonconformant'
assert line['apds']['runner_evaluated'] is False
assert 'Lint failed' in (e / 'duplicate-json.stderr.txt').read_text()
print('PASS: JSON and NDJSON contain the same finding; stdout is parseable, command failure is on stderr.')
PY
```

Two different input problems follow. Neither should be presented as a semantic
APDS finding: the CLI could not read a usable manifest at all.

```bash
cat cases/malformed-json/agent.json
m3b_expect malformed-json 1 m3b lint cases/malformed-json/agent.json --format json
m3b_expect missing-file 1 m3b lint cases/does-not-exist/agent.json --format json
```

Expected: each produces a nonempty JSON report with `ok: false`, a read/parse
error in `issues`, **no invented `rule_id`**, and no `apds` evaluation. The
missing-file message identifies the requested path. A missing file must not be
treated as a successful run with zero manifests.

## 7. Standalone validity, unavailable context and a missing required file

**Input:** this Knowledge manifest declares `docs/context.md`; that file is
deliberately absent from its case directory. It is structurally valid.

```bash
cat cases/knowledge-missing-file/agent.json
m3b_expect knowledge-standalone 0 m3b lint cases/knowledge-missing-file/agent.json --format json
m3b_expect knowledge-no-context 1 m3b lint --apds packaged cases/knowledge-missing-file/agent.json --format json
m3b_expect knowledge-complete-missing 1 m3b lint --apds packaged --package-root cases/knowledge-missing-file cases/knowledge-missing-file/agent.json --format json
```

Expected, in order:

1. `conformant` at **standalone** scope: the declaration is valid, but files were
   not requested as part of this evaluation.
2. `incomplete` at **packaged** scope, with `APDS-KNOW-002` unevaluated: no
   complete package root was supplied. An empty `issues` list is correct here;
   absent context is not proof of a defect or success.
3. `nonconformant`, **`APDS-KNOW-002` at `/knowledge/documents/0/path`**:
   you supplied the complete package root, and the required file is absent.

Now inspect a different context requirement:

```bash
cat cases/needs-graph/agent.json
m3b_expect graph-standalone 0 m3b lint cases/needs-graph/agent.json --format json
m3b_expect graph-unavailable 1 m3b lint --apds resolved cases/needs-graph/agent.json --format json
```

**Input:** the Agent references an external Loop and binds the `review` phase.
Standalone lint cannot establish which phases that Loop declares. Resolved lint
without a supplied dependency graph must report **`incomplete`**, leaving
`APDS-BIND-002` and `APDS-GRAPH-002` unevaluated. It must not fetch a Registry
graph, invent the Loop's phases, or claim the binding is verified. All five
reports retain `runner_evaluated: false`.

## 8. Editor hint repair and deliberate local migration

First prove a no-op fix does not rewrite an already valid file, and `--fix`
cannot invent a standard on an undeclared one:

```bash
m3b_expect fix-noop 0 m3b lint --fix cases/valid-agent/agent.json
cmp "$M3B_FIXTURES/valid-agent.fixture.json" cases/valid-agent/agent.json
m3b_expect fix-missing-standard 1 m3b lint --fix cases/missing-standard/agent.json
cmp "$M3B_FIXTURES/missing-standard.fixture.json" cases/missing-standard/agent.json
```

`cmp` is silent and exits 0 when the bytes match. Both files should match.

Next use a separate editable copy with a missing editor hint:

```bash
mkdir projects/schema-fix
cp cases/valid-agent/agent.json projects/schema-fix/agent.json
"$M3B_PYTHON" - <<'PY'
import json
from pathlib import Path
p = Path('projects/schema-fix/agent.json')
v = json.loads(p.read_text())
del v['$schema']
p.write_text(json.dumps(v, indent=2) + '\n')
PY
cat projects/schema-fix/agent.json
m3b_expect schema-warning 1 m3b lint --strict projects/schema-fix/agent.json --format json
m3b_expect schema-fix 0 m3b lint --fix projects/schema-fix/agent.json
cat projects/schema-fix/agent.json
m3b_expect schema-fixed 0 m3b lint --strict projects/schema-fix/agent.json
```

Expected: before repair, APDS remains `conformant` but `--strict` fails because
of the **missing `$schema` warning**. Repair adds the pinned editor URL, leaving
the explicit `standard` intact. The repair invocation may still display the
warning it just fixed; the following strict lint is clean. This is the intended
file change in this section.

Finally migrate a separate local copy deliberately:

```bash
mkdir projects/migrated-agent
cp cases/missing-standard/agent.json projects/migrated-agent/agent.json
"$M3B_PYTHON" - <<'PY'
import json
from pathlib import Path
p = Path('projects/migrated-agent/agent.json')
v = json.loads(p.read_text())
v['standard'] = {'id': 'agentpm', 'version': '1.0.0'}
p.write_text(json.dumps(v, indent=2) + '\n')
PY
cat projects/migrated-agent/agent.json
cd projects/migrated-agent
m3b_expect migrated-lint 0 m3b lint --strict
m3b_expect migrated-package 0 m3b package --machine
m3b_expect migrated-dry-run 0 m3b publish --dry-run --quiet
ls target/agentpm
cd "$M3B_ROOT"
```

Expected: ordinary lint and packaging pass **after your explicit source edit**.
The package command writes a descriptor/archive under this project's `target`;
dry-run writes its local archive without uploading or reporting a real published
Registry URL. No Python dependencies, resolver or model are needed for this
empty Agent. The original missing-standard case is still unchanged.

## 9. Local Knowledge and Memory authoring gates

Generate two real starter projects, then build and inspect them:

```bash
cd "$M3B_ROOT"
m3b_expect knowledge-init 0 m3b init --kind knowledge --out-dir projects/knowledge
m3b_expect memory-init 0 m3b init --kind memory --out-dir projects/memory
cat projects/knowledge/agent.json
cat projects/memory/agent.json
m3b_expect knowledge-build 0 m3b knowledge build --manifest projects/knowledge/agent.json
m3b_expect memory-build 0 m3b memory build --manifest projects/memory/agent.json
m3b_expect knowledge-inspect 0 m3b knowledge inspect projects/knowledge --json
m3b_expect memory-inspect 0 m3b memory inspect projects/memory --json
```

Expected: explicit supported declarations, successful local builds, and inspection
of the resulting metadata. Knowledge build creates its local content metadata;
Memory build creates `memory/build.json` and `memory/contracts/index.json`.
Inspect is read-only. These are authored local projects, not installed releases.

Make undeclared **copies** of these projects. Removing the selector is the only
intentional new defect; the valid originals stay available for inspection.

```bash
"$M3B_PYTHON" - <<'PY'
import json, shutil
from pathlib import Path
for kind in ['knowledge', 'memory']:
    dst = Path('projects') / (kind + '-undeclared')
    shutil.copytree(Path('projects') / kind, dst)
    p = dst / 'agent.json'
    v = json.loads(p.read_text())
    del v['standard']
    p.write_text(json.dumps(v, indent=2) + '\n')
    (dst / 'before-agent.txt').write_bytes(p.read_bytes())
PY
m3b_expect knowledge-build-rejects 1 m3b knowledge build --manifest projects/knowledge-undeclared/agent.json
m3b_expect knowledge-inspect-rejects 1 m3b knowledge inspect projects/knowledge-undeclared
m3b_expect memory-build-rejects 1 m3b memory build --manifest projects/memory-undeclared/agent.json
m3b_expect memory-inspect-rejects 1 m3b memory inspect projects/memory-undeclared
cmp projects/knowledge-undeclared/before-agent.txt projects/knowledge-undeclared/agent.json
cmp projects/memory-undeclared/before-agent.txt projects/memory-undeclared/agent.json
```

All four must reject **`APDS-ID-001` at `/standard`**, even though those copies
contain previously built content. No declaration should be inserted. Historical
installed package compatibility is covered separately by the automated regression
suite; these arbitrary local source directories intentionally use strict
authoring inspection.

## 10. `new` → root lint, plus nested migration guidance

Generate a local Template and give it **two undeclared nested Agent files**.
These reuse the same visible missing-standard case. The root will be synthesized
by the CLI; the nested files are Template-authored content.

```bash
m3b_expect template-init 0 m3b init --kind template --out-dir projects/template
mkdir -p projects/template/template/agents
cp cases/missing-standard/agent.json projects/template/template/agents/one.agent.json
cp cases/missing-standard/agent.json projects/template/template/agents/two.agent.json
"$M3B_PYTHON" - <<'PY'
import json
from pathlib import Path
p = Path('projects/template/template/agents/two.agent.json')
v = json.loads(p.read_text())
v['name'] = 'second-agent'
p.write_text(json.dumps(v, indent=2) + '\n')
PY
cat projects/template/template/agents/one.agent.json
cd projects/template
m3b_expect template-new 0 m3b new . generated --var project_name=generated --quiet
cd generated
cat agent.json
m3b_expect generated-root-lint 0 m3b lint --strict
cat agents/one.agent.json
m3b_expect generated-nested-lint 1 m3b lint agents/one.agent.json
cmp ../template/agents/one.agent.json agents/one.agent.json
cmp ../template/agents/two.agent.json agents/two.agent.json
cd "$M3B_ROOT"
```

Expected completion output:

- One `Created project:` line, followed by **one `Next:` line** combining root
  lint with README/project review.
- One grouped **`Local manifests missing standard`** notice, a shared declaration
  example and the two nested filenames with their explicit lint commands.
- The Template's authored entrypoint text remains present.

The generated root contains the supported `standard` and passes bare strict lint.
The nested files have not gained a declaration; explicit nested lint fails with
**`APDS-ID-001` at `/standard`**. Both `cmp` commands succeed: the rendered files
in this fixture need no variable substitutions and match the Template sources.
No installed or published bytes were rewritten.

The notice is actionable migration guidance, not a declaration that the complete
generated workspace is ready to run. A later Template publication can carry
explicit declarations in its authored nested files.

## 11. Review evidence and record the result

The following block checks captured exit codes, important machine fields, the
expected human diagnostic rule/path pairs and source preservation. It does not
rerun lint or hide a new test suite behind the manual commands.

```bash
cd "$M3B_ROOT"
"$M3B_PYTHON" - <<'PY'
import json, os
from pathlib import Path
root = Path(os.environ['M3B_ROOT'])
e = root / 'evidence'
rows = []
for p in sorted(e.glob('*.result.json')):
    result = json.loads(p.read_text())
    label = p.name.removesuffix('.result.json')
    assert result['exit_code'] == result['expected_exit_code'], label
    rows.append({'check': label, **result})
assert len(rows) == 55, f'Expected 55 captured commands, got {len(rows)}; inspect any skipped section'
expected = {
    'missing-standard': ('APDS-ID-001', '/standard'),
    'unsupported-version': ('APDS-ID-001', '/standard/version'),
    'malformed-standard': ('APDS-ID-001', '/standard'),
    'legacy-invalid': ('APDS-ID-002', '/description'),
    'package-rejects-legacy': ('APDS-ID-001', '/standard'),
    'publish-rejects-legacy': ('APDS-ID-001', '/standard'),
    'whitespace-description': ('APDS-ID-002', '/description'),
    'invalid-name': ('APDS-ID-002', '/name'),
    'unknown-kind': ('APDS-GRAPH-001', '/kind'),
    'dependency-duplicate-identity': ('APDS-GRAPH-003', '/tools/2'),
    'binding-undeclared': ('APDS-BIND-001', '/bindings/global/tools/1'),
    'loop-entry-unknown': ('APDS-LOOP-001', '/loop/entry_phase'),
    'memory-scope-unknown': ('APDS-MEM-001', '/memory/spaces/notes/scope/0'),
    'template-variable-duplicate': ('APDS-TEMPLATE-001', '/template/variables/1/name'),
    'tool-runtime-mismatch': ('APDS-TOOL-002', '/runtime/type'),
    'knowledge-build-rejects': ('APDS-ID-001', '/standard'),
    'knowledge-inspect-rejects': ('APDS-ID-001', '/standard'),
    'memory-build-rejects': ('APDS-ID-001', '/standard'),
    'memory-inspect-rejects': ('APDS-ID-001', '/standard'),
    'generated-nested-lint': ('APDS-ID-001', '/standard'),
}
for label, (rule, pointer) in expected.items():
    text = (e / f'{label}.stdout.txt').read_text() + (e / f'{label}.stderr.txt').read_text()
    assert rule in text and pointer in text, (label, rule, pointer)
    print(f'PASS {label}: actual output contains {rule} at {pointer}')
def report(label):
    values = json.loads((e / f'{label}.stdout.txt').read_text())
    assert len(values) == 1, label
    return values[0]
legacy = report('legacy-json')
assert legacy['compatibility'] == 'legacy-unverified' and 'apds' not in legacy
for label, status in [('legacy-unsupported', 'unsupported'), ('legacy-malformed', 'nonconformant')]:
    value = report(label)
    assert value['apds']['status'] == status and 'compatibility' not in value
for label, level, status in [
    ('valid-json', 'standalone', 'conformant'),
    ('knowledge-standalone', 'standalone', 'conformant'),
    ('knowledge-no-context', 'packaged', 'incomplete'),
    ('knowledge-complete-missing', 'packaged', 'nonconformant'),
    ('graph-standalone', 'standalone', 'conformant'),
    ('graph-unavailable', 'resolved', 'incomplete'),
]:
    value = report(label)['apds']
    assert (value['level'], value['status'], value['runner_evaluated']) == (level, status, False), label
    print(f'PASS {label}: actual {level}/{status}; Runner not evaluated')
assert 'APDS-KNOW-002' in report('knowledge-no-context')['apds']['unevaluated']
assert {'APDS-BIND-002', 'APDS-GRAPH-002'} <= set(report('graph-unavailable')['apds']['unevaluated'])
assert any(i.get('rule_id') == 'APDS-KNOW-002' and i['instance_path'] == '/knowledge/documents/0/path'
           for i in report('knowledge-complete-missing')['issues'])
for label in ['malformed-json', 'missing-file']:
    value = report(label)
    assert value['ok'] is False and value['issues'] and 'apds' not in value
    assert all('rule_id' not in issue for issue in value['issues'])
new_output = (e / 'template-new.stdout.txt').read_text()
assert new_output.count('Next:') == 1
assert new_output.count('Local manifests missing `standard`:') == 1
for name in ['one', 'two']:
    assert f'agentpm lint agents/{name}.agent.json' in new_output
for source in Path(os.environ['M3B_FIXTURES']).glob('*.fixture.*'):
    name = source.name.split('.fixture.')[0]
    assert (root / 'cases' / name / 'agent.json').read_bytes() == source.read_bytes(), name
assert not (root / 'target').exists(), 'Rejected preflight unexpectedly created package output at workspace root'
binary = Path(os.environ['M3B_APM'])
import hashlib
setup = json.loads((e / 'setup.json').read_text())
assert hashlib.sha256(binary.read_bytes()).hexdigest() == setup['cli_sha256'], 'Binary changed during this run'
(e / 'command-summary.json').write_text(json.dumps(rows, indent=2) + '\n')
print(f'PASS: {len(rows)} actual/expected exits match; diagnostic expectations match; all 17 original case copies are byte-identical.')
print('Keep evidence at:', e)
PY
```

Operator confirmed the following acceptance checks passed on **2026-11-17**.
Keep the evidence directory and hosted CI links with that acceptance:

- [x] The human messages made each actual defect and its field path clear.
- [x] Missing, malformed, unsupported, incomplete and conformant results matched
      the explanations, including the no-issues-but-incomplete case.
- [x] Legacy preview was clearly unverified/read-only; strict preflight rejected
      undeclared input; deliberate migration made new lint/package pass.
- [x] Fixes changed only the intended editable project; original cases remained
      byte-identical. Knowledge/Memory local authoring gates behaved as described.
- [x] Template root lint passed, and nested migration guidance was grouped,
      concrete and accurate without editing Template-provided declarations.
- [x] Final evidence check passed, with no unexpected failures or skipped sections.

This maps to T-CLI-04–06, scoped S2-AC-02/03/04 and D04. The automated suite
retains broader all-kind/scaffold, installed historical artifact, execution/export,
saved-build tampering and publication-link regressions. This local run does not
claim live Registry finalization, signing, full Runner conformance or successful
production publication.

Keep the workspace until acceptance is recorded. To repeat, rerun section 1 in a
fresh terminal to create another dated folder; do not overwrite a previous run's
evidence. You may remove your own dated folder later after retaining any evidence
you need. Nothing here changes your saved CLI credentials or production settings.
