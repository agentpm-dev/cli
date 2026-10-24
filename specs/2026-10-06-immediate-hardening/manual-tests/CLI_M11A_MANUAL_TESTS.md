# Milestone 11A: Python Dependency Resolution

Run each command yourself and inspect its output before continuing. There is no
test runner or reset helper. Setup creates only a disposable root and empty
HOME; each section explicitly copies its own fixtures when they are needed.
Nothing here changes `agentpm init` output.

Use a **new disposable terminal** on your Mac. Keep your existing local Registry
and website running in their normal terminals. The Registry must include M11A
and the latest `runtimes` namespace guard; restart it if necessary. Its S3 config
must use the dev bucket, not production. This guide assumes
`http://127.0.0.1:5000`; change that address during setup if needed.

The uv ingestion/native-workflow checks and dev mirror probe were already
completed separately. These tests exercise the actual CLI against that dev
mirror and the public Python index. No AWS keys or real AgentPM PAT are needed.
The fake token below satisfies the CLI's local credential-presence check only.
No package is published, no S3 object is written, and the namespace checks must
be rejected rather than create a namespace.

M11A does **not** install Tool dependencies, write Python state into `agent.lock`,
or make dependency imports work through `agentpm run`. Those are 11B/11C tests.

## 1. Build and Prepare Empty, Isolated State

Start from the `agentpm` repository root. Build before replacing HOME:

```bash
cargo build -p agentpm-cli
export M11A_REPO="$PWD"
export M11A_APM="$M11A_REPO/target/debug/agentpm"
export M11A_PYTHON="$M11A_REPO/../agentpm-api/.venv/bin/python"
export M11A_SCHEMA="$M11A_REPO/schemas/agentpm.manifest.schema.json"
export M11A_FIXTURES="$M11A_REPO/specs/2026-10-06-immediate-hardening/manual-tests/fixtures/m11a"
export M11A_REGISTRY='http://127.0.0.1:5000'
export M11A_OFFLINE='http://127.0.0.1:1'
export M11A_ROOT="$(mktemp -d "${TMPDIR:-/tmp}/agentpm-m11a.XXXXXX")"
mkdir "$M11A_ROOT/home"
export M11A_UV_VERSION="$("$M11A_PYTHON" -c 'import json,sys; print(json.load(open(sys.argv[1]))["version"])' "$M11A_REPO/platforms/uv-pins.json")"
```

Now isolate CLI state. The API venv supplies an existing Python 3.11+ interpreter
for resolution; it is not modified or used as a managed Tool environment.

```bash
export HOME="$M11A_ROOT/home"
export USERPROFILE="$HOME"
export XDG_CONFIG_HOME="$HOME/config"
export XDG_DATA_HOME="$HOME/data"
export XDG_CACHE_HOME="$HOME/cache"
export APPDATA="$HOME/data"
export PATH="$(dirname "$M11A_PYTHON"):$PATH"
export AGENTPM_TELEMETRY=0
unset AGENTPM_UV_PATH AGENTPM_TOKEN AWS_ACCESS_KEY_ID AWS_SECRET_ACCESS_KEY AWS_SESSION_TOKEN
unset VIRTUAL_ENV CONDA_PREFIX
printf 'Keep this workspace path: %s\n' "$M11A_ROOT"
"$M11A_PYTHON" --version
"$M11A_APM" --version
"$M11A_PYTHON" -c 'import hashlib,os,pathlib; print("CLI SHA-256:", hashlib.sha256(pathlib.Path(os.environ["M11A_APM"]).read_bytes()).hexdigest())'
curl --fail --show-error "$M11A_REGISTRY/health"
find "$M11A_ROOT" -name agent.json -o -name agent.lock
```

Expected: Python 3.11+, the just-built CLI, a healthy local Registry, and **no
manifests or lockfiles** yet. Record the CLI digest; its version alone may match
the installed release even though the code is different. The expected uv pin is
currently `0.13.5`. Port 1 must be unused; otherwise choose another closed
loopback port for `M11A_OFFLINE`. Do not stop the real backend for failure tests.

All lint/publish commands explicitly use the checked-out schema. The public
schema on GitHub may not yet contain M11A; that must not affect these tests.

For a negative test, the `if` statement captures its exit code without terminating
a shell with `errexit` enabled. Use `M11A_EXIT`, not zsh's read-only `status`.

## 2. Dependency Validation

Create this case now, then inspect and lint it:

```bash
mkdir "$M11A_ROOT/lint"
cd "$M11A_ROOT/lint"
cp "$M11A_FIXTURES/tool.py" "$M11A_FIXTURES/tool.mjs" "$M11A_FIXTURES/README.md" .
cp "$M11A_FIXTURES/python-deps.fixture.json" agent.json
cat agent.json
"$M11A_APM" lint --strict --schema "$M11A_SCHEMA"
```

Expected: exit 0. `runtime.version` is the minimum `3.11`; dependencies are
Python distribution requirements, not AgentPM package references.

Try each invalid declaration separately. Each must exit **1**, identify
`/runtime/dependencies`, and explain the particular error. None should create
a resolver cache or attempt resolution.

```bash
cp "$M11A_FIXTURES/invalid-requirement.fixture.json" agent.json
if "$M11A_APM" lint --strict --schema "$M11A_SCHEMA"; then M11A_EXIT=0; else M11A_EXIT=$?; fi
printf 'CLI exit: %s (expected 1: invalid requirement syntax)\n' "$M11A_EXIT"
```

```bash
cp "$M11A_FIXTURES/duplicate-requirement.fixture.json" agent.json
if "$M11A_APM" lint --strict --schema "$M11A_SCHEMA"; then M11A_EXIT=0; else M11A_EXIT=$?; fi
printf 'CLI exit: %s (expected 1: overlapping duplicate name)\n' "$M11A_EXIT"
```

```bash
cp "$M11A_FIXTURES/conflicting-requirement.fixture.json" agent.json
if "$M11A_APM" lint --strict --schema "$M11A_SCHEMA"; then M11A_EXIT=0; else M11A_EXIT=$?; fi
printf 'CLI exit: %s (expected 1: contradictory constraints)\n' "$M11A_EXIT"
```

```bash
cp "$M11A_FIXTURES/local-path-requirement.fixture.json" agent.json
if "$M11A_APM" lint --strict --schema "$M11A_SCHEMA"; then M11A_EXIT=0; else M11A_EXIT=$?; fi
printf 'CLI exit: %s (expected 1: local/URL dependency source)\n' "$M11A_EXIT"
```

```bash
cp "$M11A_FIXTURES/node-deps.fixture.json" agent.json
if "$M11A_APM" lint --strict --schema "$M11A_SCHEMA"; then M11A_EXIT=0; else M11A_EXIT=$?; fi
printf 'CLI exit: %s (expected 1: dependencies are Python-only, even when empty)\n' "$M11A_EXIT"
```

Disjoint marker branches for the same name are allowed:

```bash
cp "$M11A_FIXTURES/python-markers.fixture.json" agent.json
"$M11A_APM" lint --strict --schema "$M11A_SCHEMA"
```

Expected: exit 0. Windows and non-Windows requirements do not overlap.

## 3. No Dependencies Means No Resolver Acquisition

```bash
mkdir "$M11A_ROOT/legacy"
cd "$M11A_ROOT/legacy"
cp "$M11A_FIXTURES/tool.py" "$M11A_FIXTURES/README.md" .
cp "$M11A_FIXTURES/python-no-deps.fixture.json" agent.json
AGENTPM_UV_PATH="$M11A_ROOT/does-not-exist" "$M11A_APM" --base-url "$M11A_OFFLINE" publish --dry-run --token apm_m11a.fixture --schema "$M11A_SCHEMA"
```

Expected: exit 0 and a local dry-run archive, with no Python-resolution message.
An invalid override and closed Registry cannot affect this dependency-free path.
Repeat for an explicitly empty dependency list:

```bash
cp "$M11A_FIXTURES/python-empty-deps.fixture.json" agent.json
AGENTPM_UV_PATH="$M11A_ROOT/does-not-exist" "$M11A_APM" --base-url "$M11A_OFFLINE" publish --dry-run --token apm_m11a.fixture --schema "$M11A_SCHEMA"
find "$HOME" -type f -path '*/runtimes/uv/*' -print
```

Expected: exit 0 again; `find` prints nothing. No per-project resolution record,
`agent.lock`, or managed `.agentpm` environment should appear in this case.

## 4. Cold Publish Block and Connection Failure

Create the real dependency-bearing fixture now:

```bash
mkdir "$M11A_ROOT/project"
cd "$M11A_ROOT/project"
cp "$M11A_FIXTURES/tool.py" "$M11A_FIXTURES/README.md" .
cp "$M11A_FIXTURES/python-deps.fixture.json" agent.json
if AGENTPM_UV_PATH="$M11A_ROOT/does-not-exist" "$M11A_APM" --base-url "$M11A_OFFLINE" publish --refresh-python --token apm_m11a.fixture --schema "$M11A_SCHEMA"; then M11A_EXIT=0; else M11A_EXIT=$?; fi
printf 'CLI exit: %s (expected 1: Milestone 11B publication gate)\n' "$M11A_EXIT"
find "$HOME" -type f -path '*/runtimes/uv/*' -print
find . -name agent.lock -o -path '*/python-resolutions/*.json'
```

Expected: the error names **Milestone 11B** and says no resolver acquisition or
upload was attempted. It must not be an override, connection or resolution error.
Both `find` commands print nothing. Even `--refresh-python` must not cause a
download or expensive resolution before this unconditional block.

Now allow the dry-run path but point it at the closed address:

```bash
if "$M11A_APM" --base-url "$M11A_OFFLINE" publish --dry-run --token apm_m11a.fixture --schema "$M11A_SCHEMA"; then M11A_EXIT=0; else M11A_EXIT=$?; fi
printf 'CLI exit: %s (expected 1: network/proxy failure)\n' "$M11A_EXIT"
```

Expected: a distinct `network/proxy failure` reaching Registry storage, not
`Milestone 11B`, `not mirrored`, or a digest mismatch. No managed binary or
resolution record is left behind.

## 5. Real First Download and Portable Resolution

Put a deliberately wrong uv on this disposable terminal's PATH. This is a
negative fixture, not the real resolver used for the successful test:

```bash
mkdir "$M11A_ROOT/path-bin"
cp "$M11A_FIXTURES/wrong-version-uv.sh" "$M11A_ROOT/path-bin/uv"
chmod +x "$M11A_ROOT/path-bin/uv"
export PATH="$M11A_ROOT/path-bin:$PATH"
command -v uv
uv --version
```

Expected: the path is inside `M11A_ROOT` and the version is `0.0.0`. Now run the
actual CLI against the local Registry:

```bash
"$M11A_APM" --base-url "$M11A_REGISTRY" publish --dry-run --token apm_m11a.fixture --schema "$M11A_SCHEMA"
export M11A_UV="$(find "$HOME" -type f -path "*/runtimes/uv/$M11A_UV_VERSION/*/uv" -print)"
printf 'Managed resolver: %s\n' "$M11A_UV"
"$M11A_UV" --version
```

Expected: exit 0, a checkmark-prefixed `Python dependencies resolved: ... distributions, sha256:...`,
and `Dry-run: artifact created at ...`. The managed binary reports uv `0.13.5`,
ignoring PATH's `0.0.0`. Your backend log should show a successful resolver GET
for your compiled target. The executable is under the isolated HOME, outside
the Tool directory. There should be exactly one matching cached uv file.

Verify the downloaded executable against the checked-in pin yourself:

```bash
"$M11A_PYTHON" - <<'PY'
import hashlib, json, os
from pathlib import Path
p = Path(os.environ["M11A_UV"])
pins = json.loads((Path(os.environ["M11A_REPO"]) / "platforms/uv-pins.json").read_text())
digest = hashlib.sha256(p.read_bytes()).hexdigest()
print("Target:", p.parent.name)
print("Downloaded SHA-256:", digest)
print("Matches compiled pin:", digest == pins["artifacts"][p.parent.name]["sha256"])
PY
```

Expected: your supported target and `Matches compiled pin: True`.

Find and read the author-side record:

```bash
export M11A_RECORD="$(find "$PWD/target/agentpm/python-resolutions" -type f -name '*.json' -print)"
"$M11A_PYTHON" -m json.tool "$M11A_RECORD"
cp "$M11A_RECORD" "$M11A_ROOT/first-resolution.json"
```

There must be exactly one record at this point. Its `resolution` must contain
type `agentpm.python-resolution.v1`, `python.requires: >=3.11`, sorted normalized
roots and exact packages. Use this visible inspection command for hash counts
and conditional packages:

```bash
"$M11A_PYTHON" - <<'PY'
import json, os, re
from pathlib import Path
record = json.loads(Path(os.environ["M11A_RECORD"]).read_text())
value = record["resolution"]
print("Input fingerprint:", record["input_fingerprint"])
print("Resolution digest:", record["resolution_digest"])
print("Python constraint:", value["python"]["requires"])
print("Requirements:", value["requirements"])
for package in value["packages"]:
    print(package["name"], package["version"], "hashes:", len(package["hashes"]), "marker:", package.get("marker", "all"))
print("Full lowercase hashes:", all(p["hashes"] and all(re.fullmatch(r"sha256:[0-9a-f]{64}", h) for h in p["hashes"]) for p in value["packages"]))
print("Sorted hashes:", all(p["hashes"] == sorted(set(p["hashes"])) for p in value["packages"]))
PY
```

Expected: Click `8.1.8`, Pydantic `2.11.7`, pydantic-core `2.33.2` with multiple
distribution hashes, and Colorama conditional on Windows even though you resolved
on a Mac. Both hash checks must print `True`. Other transitive versions/counts
can change with the public index. The Python constraint remains a minimum, not
the exact interpreter version on your machine. There must be no wheel filenames,
URLs, publisher target, interpreter path or environment ID in the resolution.
The separate native-workflow reports already check all four platforms' wheel
coverage; this graph-free object is not an independent transitive-closure proof.

```bash
find . -name agent.lock -o -name .agentpm -o -name .venv
```

Expected: no output. The JSON above is disposable author-side preparation, not
an AgentPM lockfile or an installed dependency environment.

## 6. Author-Resolution Reuse, Refresh, and Changed Inputs

This tests a different cache from the executable cache: matching author-side
records live under each Tool's `target/agentpm/python-resolutions/`.

```bash
AGENTPM_UV_PATH="$M11A_ROOT/does-not-exist" "$M11A_APM" --base-url "$M11A_OFFLINE" publish --dry-run --token apm_m11a.fixture --schema "$M11A_SCHEMA"
cmp "$M11A_ROOT/first-resolution.json" "$M11A_RECORD"
```

Expected: success despite the invalid override and offline Registry. No resolver
is acquired/executed for this matching record; `cmp` prints nothing and exits 0.
The displayed resolution digest is unchanged.

Force refresh; the same missing override must now matter:

```bash
if AGENTPM_UV_PATH="$M11A_ROOT/does-not-exist" "$M11A_APM" --base-url "$M11A_OFFLINE" publish --dry-run --refresh-python --token apm_m11a.fixture --schema "$M11A_SCHEMA"; then M11A_EXIT=0; else M11A_EXIT=$?; fi
printf 'CLI exit: %s (expected 1: override must name an existing file)\n' "$M11A_EXIT"
cmp "$M11A_ROOT/first-resolution.json" "$M11A_RECORD"
```

Expected: the explicit override-path error, with the old valid record unchanged.
Now refresh using the real managed binary:

```bash
"$M11A_APM" --base-url "$M11A_OFFLINE" publish --dry-run --refresh-python --token apm_m11a.fixture --schema "$M11A_SCHEMA"
```

Expected: success without contacting the Registry, because uv itself is cached.
Fresh resolution can still access the Python index; an offline **Registry** is
not a claim that the entire machine is offline. A refresh is allowed to produce
the same resolution digest when the index returns the same dependency set.

Change an authored requirement without requesting refresh:

```bash
cp "$M11A_FIXTURES/python-changed.fixture.json" agent.json
if AGENTPM_UV_PATH="$M11A_ROOT/does-not-exist" "$M11A_APM" --base-url "$M11A_OFFLINE" publish --dry-run --token apm_m11a.fixture --schema "$M11A_SCHEMA"; then M11A_EXIT=0; else M11A_EXIT=$?; fi
printf 'CLI exit: %s (expected 1: changed roots require a new resolution)\n' "$M11A_EXIT"
"$M11A_APM" --base-url "$M11A_OFFLINE" publish --dry-run --token apm_m11a.fixture --schema "$M11A_SCHEMA"
find target/agentpm/python-resolutions -type f -name '*.json' -print
```

Expected: the first command cannot reuse the old record and rejects the missing
override. The second succeeds using cached uv. There are now two input-fingerprint
records: their roots select different Click versions (`8.1.8` and `8.1.7`). The
old `M11A_RECORD` still names only the original record. Restore author intent:

```bash
cp "$M11A_FIXTURES/python-deps.fixture.json" agent.json
```

## 7. A Second Project Reuses the Executable Cache

```bash
mkdir "$M11A_ROOT/second-project"
cd "$M11A_ROOT/second-project"
cp "$M11A_FIXTURES/tool.py" "$M11A_FIXTURES/README.md" .
cp "$M11A_FIXTURES/python-deps.fixture.json" agent.json
"$M11A_APM" --base-url "$M11A_OFFLINE" publish --dry-run --token apm_m11a.fixture --schema "$M11A_SCHEMA"
```

Expected: success even though this project has no prior resolution record and
the Registry address is closed. One per-user cached executable serves both
projects; each has its own author-side record. PATH's wrong uv is still ignored.
Python index access may occur; this specifically proves no new resolver download.

## 8. Disjoint Marker Branches Resolve Portably

```bash
mkdir "$M11A_ROOT/markers"
cd "$M11A_ROOT/markers"
cp "$M11A_FIXTURES/tool.py" "$M11A_FIXTURES/README.md" .
cp "$M11A_FIXTURES/python-markers.fixture.json" agent.json
"$M11A_APM" --base-url "$M11A_OFFLINE" publish --dry-run --token apm_m11a.fixture --schema "$M11A_SCHEMA"
find target/agentpm/python-resolutions -type f -name '*.json' -exec "$M11A_PYTHON" -m json.tool {} \;
```

Expected: success, with Click `8.1.8` for Windows and `8.1.7` for non-Windows,
each carrying the appropriate marker; Colorama is Windows-only. Both Click
branches must be retained on the Mac, not reduced to the publisher's branch.
Equivalent normalized marker text is fine: a non-Windows branch can be expressed
as Linux-or-Darwin rather than literally `sys_platform != 'win32'`.

## 9. Overrides, Corrupt Cache, and Execution Errors

Return to the original project. Each command here uses `--refresh-python` so a
matching author-side record cannot hide acquisition/override behavior.

```bash
cd "$M11A_ROOT/project"
cp "$M11A_UV" "$M11A_ROOT/provisioned-uv"
mv "$M11A_UV" "$M11A_UV.saved"
AGENTPM_UV_PATH="$M11A_ROOT/provisioned-uv" "$M11A_APM" --base-url "$M11A_OFFLINE" publish --dry-run --refresh-python --token apm_m11a.fixture --schema "$M11A_SCHEMA"
mv "$M11A_UV.saved" "$M11A_UV"
```

Expected: success with the exact pinned override, even while the managed cache
entry is absent. This avoids resolver acquisition, not Python index access.
If the command fails unexpectedly, **restore `M11A_UV.saved` before continuing**.

Reject the controlled wrong-version fixture:

```bash
if AGENTPM_UV_PATH="$M11A_ROOT/path-bin/uv" "$M11A_APM" --base-url "$M11A_OFFLINE" publish --dry-run --refresh-python --token apm_m11a.fixture --schema "$M11A_SCHEMA"; then M11A_EXIT=0; else M11A_EXIT=$?; fi
printf 'CLI exit: %s (expected 1: exact uv version mismatch)\n' "$M11A_EXIT"
```

Expected: names required uv `0.13.5`, not a warning or PATH fallback. No lock/export
command should run through that fixture.

Corrupt only the **isolated managed cache**, then restore it immediately:

```bash
cp "$M11A_UV" "$M11A_ROOT/uv-before-corruption"
printf '\nM11A intentional corruption\n' >> "$M11A_UV"
if "$M11A_APM" --base-url "$M11A_OFFLINE" publish --dry-run --refresh-python --token apm_m11a.fixture --schema "$M11A_SCHEMA"; then M11A_EXIT=0; else M11A_EXIT=$?; fi
printf 'CLI exit: %s (expected 1: cached resolver digest mismatch)\n' "$M11A_EXIT"
cp "$M11A_ROOT/uv-before-corruption" "$M11A_UV"
cmp "$M11A_ROOT/uv-before-corruption" "$M11A_UV"
```

Expected: digest mismatch, no execution or silent re-download, and a clean restore
(`cmp` exits 0). Do not modify the checked-in pin table or either S3 object.

Finally distinguish an engine execution failure from those acquisition errors:

```bash
cp "$M11A_FIXTURES/failing-uv.sh" "$M11A_ROOT/failing-uv"
chmod +x "$M11A_ROOT/failing-uv"
cp "$M11A_RECORD" "$M11A_ROOT/before-engine-error.json"
if AGENTPM_UV_PATH="$M11A_ROOT/failing-uv" "$M11A_APM" --base-url "$M11A_OFFLINE" publish --dry-run --refresh-python --token apm_m11a.fixture --schema "$M11A_SCHEMA"; then M11A_EXIT=0; else M11A_EXIT=$?; fi
printf 'CLI exit: %s (expected 1: resolver executed but failed)\n' "$M11A_EXIT"
cmp "$M11A_ROOT/before-engine-error.json" "$M11A_RECORD"
"$M11A_APM" --base-url "$M11A_OFFLINE" publish --dry-run --refresh-python --token apm_m11a.fixture --schema "$M11A_SCHEMA"
```

Expected: `Python resolver executed but failed` plus the fixture's deliberate
failure message, not a transport/version/digest error. No valid record is
overwritten on failure. The last real-resolver command succeeds again.

## 10. Reserved Namespace Through the Local Website

This is the only section that submits namespace-creation requests. Use the
**local/dev website only**, signed in normally; no JWT needs to be copied into a
terminal. Your previous dev/prod existing-namespace audit was a separate step.

1. Open the existing namespace-creation dialog. Select a public user namespace
   and enter `runtimes`.
2. Submit and inspect the request in browser Network tools. Expect HTTP **400**
   and `Handle is reserved`, with no new namespace.
3. Repeat with a mixed-case value such as `RunTimes`. It must also be rejected.
4. Try the public org option with `runtimes`; expect the same server-side rejection.

If a namespace is unexpectedly created, stop and report it; do not publish into
it or proceed as if the test passed. The backend may not have reloaded the guard.
Existing `agentpm` rejection behavior is unchanged. Path-style input bypasses,
whitespace variants, private modes and allowed similar handles are covered by
the automated API tests; don't create extra namespaces just to duplicate them.

## Completion Record and Boundaries

Operator confirmed the walkthrough passed on 2026-10-23 using the local Registry
and isolated macOS Apple Silicon test workspace. The checked items below record
that confirmation; when repeating the guide, verify each result again.

- [x] Valid declarations/markers lint; syntax, duplicate, conflict, local-path and Node cases fail correctly.
- [x] No-dependency and empty-dependency dry runs succeed without acquiring uv.
- [x] Cold live publish fails at the 11B gate before acquisition; no state/upload.
- [x] Cold offline acquisition has its own network/proxy error.
- [x] Real dev Registry/S3 acquisition succeeds; raw binary matches its pin; PATH uv is ignored.
- [x] Portable exact resolution retains native hashes and Windows-only dependencies, without publisher fields.
- [x] Matching author-side state is reused; refresh and changed roots actually require resolution.
- [x] A second project reuses the managed executable with the Registry offline.
- [x] Disjoint marker branches resolve to both exact Click versions.
- [x] Exact override succeeds; wrong version, corrupted cache and engine failure have distinct errors; recovery succeeds.
- [x] Local namespace creation rejects `runtimes` without creating a row.

Record the workspace path, CLI digest, results and any failures in your notes or
the milestone test-plan evidence. Do not mark the milestone complete merely
because this guide exists. Production route delivery still needs the Registry
deployment and separate production mirror probe. Cross-machine installation,
managed environments and dependency-bearing publication are not claimed here.
Unsupported-target, tampered-download, HTTP/proxy and canonical-digest cases also
retain their automated coverage; this guide does not fabricate another platform.

Close the disposable terminal when finished. Your normal HOME/PATH/credentials
in other terminals are unchanged. Keep the temporary workspace for inspection;
there is no deletion/reset command. To start over, open a **fresh terminal** and
repeat Section 1 for a new empty root, rather than deleting your real cache.
