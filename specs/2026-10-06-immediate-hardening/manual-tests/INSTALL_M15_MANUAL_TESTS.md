# Milestone 15: Verified Target-Aware Installation

Use **one new macOS/zsh terminal**, starting in the `agentpm` repository. Run
one block at a time and keep it open. Only section 8 uses a second terminal;
its exact command is printed for you. IntelliJ keeps running the local backend.

Review and hosted CLI/native/Registry PostgreSQL jobs passed. Registry Release
Contract Parity against CLI `main` and the production trust bootstrap remain
**Release Band 11** gates. This guide verifies the complete **development**
chain using the real root/key set already established in M14C. Do not generate
another root, replace the backend signing key or remove the dev pin.

| Section | What you verify |
| --- | --- |
| 1 | Existing dev trust, current CLI, scratch source and disposable author key. |
| 2 | CLI-built archives and signed development releases, with actual Python dependencies. |
| 3 | Exact-before-`any`, fallback, full inventory, one URL and same-session renewal. |
| 4 | Actual CLI verifies/installs each selected payload and executes native dependencies. |
| 5 | A fresh frozen consumer preserves the portable lock. |
| 6 | Cache/state tampering is detected or repaired without changing the lock. |
| 7 | Provisioning failure cannot finalize; incompatible exact installs fail as JSON. |
| 8 | Harness invokes the installed dependency-bearing Tool through the real runner. |
| 9 | Legacy vendored Tool remains installable/runnable; bounded failure tests. |
| 10 | Cleanup, retained evidence and acceptance. |

The [helper](fixtures/m15/fixture.py) reuses the reviewed M13B transport and M14B
signing implementation. Preparation, packaging and signing are local; explicit
`init`, `upload` and `finalize` commands publish to your **loopback Registry and
`agentpm-dev-tools` only**. `upload` uploads all targets in that case unless
`--target` is supplied. `upload-renew` only refreshes PUT URLs; it does not upload.
No AWS keys are needed in this terminal. The backend uses its existing dev keys.
Database inspections are read-only and loopback-only. No scan tags/rows are edited.

These fixtures intentionally label identical portable source for several targets
to test inventory/selection. They are **not native builds for four computers**.
The native proof here is Pydantic Core actually running on your current host.
New public package-build/matrix publishing commands remain later work; helper
packaging/signing is test scaffolding, not the ordinary author workflow.

## 1. Confirm Local Settings and Prepare

In IntelliJ confirm the reviewed Registry code, migrations through **V1.105**,
development database, `S3_BUCKET=agentpm-dev-tools`, `S3_PREFIX=tools`, existing
attestation key, and the valid **`REGISTRY_SIGNED_KEYSET_PATH` from M14C**.
Set **only this local backend** to `RELEASE_PUBLISH_ENABLED=true` if necessary,
then restart it. Record its previous local value so it can be restored later.
Production remains `false`. The signed key set must be unexpired and include
the backend's current attestation key; if not, renew the public document using
M14C's existing root/inventory procedure before proceeding.

Start the command terminal in the `agentpm` repository:

```bash
umask 077
export M15_REPO="$PWD"
export M15_API="$M15_REPO/../agentpm-api"
export M15_PYTHON="$M15_API/.venv/bin/python"
export M15_APM="$M15_REPO/target/debug/agentpm"
export M15_SCHEMA="$M15_REPO/schemas/agentpm.manifest.schema.json"
export M15_FIXTURES="$M15_REPO/specs/2026-10-06-immediate-hardening/manual-tests/fixtures/m15"
export M15_PREVIOUS="$M15_REPO/specs/2026-10-06-immediate-hardening/manual-tests/fixtures/m11c"
export M15_ROOT="$(mktemp -d "${TMPDIR:-/tmp}/agentpm-m15.XXXXXX")"
export M15_BASE='http://127.0.0.1:5000'
export M15_NAMESPACE='zack'
export M15_DATABASE_URL='postgresql://postgres:postgres@127.0.0.1:5433/postgres'
export M15_INTERPRETER='/opt/homebrew/opt/python@3.13/bin/python3.13'
export M15_MODEL_PORT='18160'
unset M15_TOKEN AGENTPM_UV_PATH
m15_python() {
  env PYTHONPATH="$M15_API" PYTHONDONTWRITEBYTECODE=1 \
    REGISTRY_PRIVATE_KEY_B64=AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA= \
    AWS_EC2_METADATA_DISABLED=true "$M15_PYTHON" -B "$@"
}
m15_fixture() {
  local m15_action="$1"
  shift
  m15_python "$M15_FIXTURES/fixture.py" "$m15_action" "$M15_ROOT" "$@"
}
m15() {
  if [[ "$M15_BASE" != 'http://127.0.0.1:5000' ]]; then
    printf 'Keep the exact pinned dev origin http://127.0.0.1:5000.\n' >&2
    return 1
  fi
  env AGENTPM_TELEMETRY=0 AGENTPM_TOKEN="${M15_TOKEN:-apm_m15.fixture}" \
    AGENTPM_PYTHON="$M15_INTERPRETER" "$M15_APM" --base-url "$M15_BASE" "$@"
}
m15_cold() {
  env HOME="$M15_ROOT/cold-home" XDG_CACHE_HOME="$M15_ROOT/cold-home/.cache" \
    XDG_CONFIG_HOME="$M15_ROOT/cold-home/.config" \
    AGENTPM_TELEMETRY=0 AGENTPM_TOKEN="$M15_TOKEN" \
    AGENTPM_PYTHON="$M15_INTERPRETER" "$M15_APM" --base-url "$M15_BASE" "$@"
}
m15_python - <<'PY'
import sys
import nacl.signing, psycopg, pytest, rfc8785
assert sys.version_info >= (3, 11)
print('Existing Python dependencies available.')
PY
"$M15_INTERPRETER" --version
cargo build --locked -p agentpm-cli
m15 --version
m15_python "$M15_REPO/scripts/check_release_contract.py" --registry-root "$M15_API"
m15_fixture prepare --namespace "$M15_NAMESPACE"
git -C "$M15_REPO" rev-parse HEAD
git -C "$M15_API" rev-parse HEAD
printf 'Keep this terminal. Scratch/evidence: %s\n' "$M15_ROOT"
```

Use another supported native Python 3.11+ path if needed. The API venv and chosen
Tool interpreter have separate roles. The DB URL above is the checked-in local
Docker Compose connection; match IntelliJ if overridden. Do not omit port 5433,
create missing tables in another database or use production credentials.

The Python wrapper's dummy key permits imports only. It neither changes the
running backend nor signs Registry attestations. Your normal HOME and encrypted
CLI keystore are preserved; `m15_cold` overrides HOME only for that subprocess.

Enter your **dev** PAT, then check the served trust chain and register just this
walkthrough's disposable author key:

```bash
read -s 'M15_TOKEN?Development publish PAT: '
printf '\n'
export M15_TOKEN
m15 whoami
m15_fixture preflight
m15 namespace add-signer --namespace "$M15_NAMESPACE" \
  --label 'M15 disposable install check' --pubkey "$M15_ROOT/primary-public-key.txt"
export M15_SIGNER_ID="$(m15_fixture signer-id)"
printf 'Disposable signer UUID: %s\n' "$M15_SIGNER_ID"
```

Expected: the served key set verifies against the existing exact-origin dev pin,
and one disposable signer UUID prints. No root private key or Registry private
key is requested. Keep this signer active until all installs pass.

## 2. Prepare and Publish the Three Signed Releases

All three versions use the same unique package name: `portable` **0.1.0** has
one `any` artifact; `matrix` **0.1.1** has `any` plus all four concrete targets;
`incompatible` **0.1.2** has one foreign target and no `any`. The latter is never
executed. Explicit consumer versions below prevent an accidental latest install.

Run preparation **one case at a time**, in this order. `package` needs that
case's completed CLI dry run and cannot rebuild an already reserved release.

```bash
cd "$M15_ROOT/portable"
m15 lint --strict --schema "$M15_SCHEMA"
m15 publish --dry-run --refresh-python --schema "$M15_SCHEMA"
m15_fixture package portable
```

```bash
cd "$M15_ROOT/matrix"
m15 lint --strict --schema "$M15_SCHEMA"
m15 publish --dry-run --refresh-python --schema "$M15_SCHEMA"
m15_fixture package matrix
```

```bash
cd "$M15_ROOT/incompatible"
m15 lint --strict --schema "$M15_SCHEMA"
m15 publish --dry-run --refresh-python --schema "$M15_SCHEMA"
m15_fixture package incompatible
cd "$M15_REPO"
```

Expected: real resolution via your dev resolver mirror and PyPI, CLI packaging,
then validated per-target archives and a saved author v2 envelope for each
whole release. Dependencies are metadata, not vendored distributions. Requests,
signatures and generated archives are outside the manifest's explicit file list.
Do not rebuild/change an inventory after `init`.

The next commands create real dev Registry rows, immutable dev S3 objects and
GuardDuty scans. Each operation is explicit; no helper enables publishing.

```bash
m15_fixture init portable
m15_fixture upload portable
m15_fixture finalize portable
m15_fixture init matrix
m15_fixture upload matrix
m15_fixture finalize matrix
m15_fixture init incompatible
m15_fixture upload incompatible
m15_fixture finalize incompatible
```

Expected: one accepted author signature per release, plus a Registry v3
attestation from the already trusted backend key. If a PUT expires, run
`m15_fixture upload-renew <case>`, then repeat that case's `upload` and `finalize`.
The immutable release/signature stays unchanged. Let real scanning complete if
installation is temporarily gated; retry normally, never change scan rows/tags.

## 3. Inspect Selection and Renewal Without Downloading

These commands call actual install-init, create **pending inspection sessions**,
and save their private scratch responses. They do not download/finalize anything.
They check the response shape and inventory; the actual CLI trust-chain test is
section 4. Saved responses contain expiring URLs, so keep the scratch directory
private and do not paste them wholesale into chat.

```bash
m15_fixture probe portable
m15_fixture probe matrix
m15_fixture renew matrix
m15_fixture sessions matrix
```

Expected: portable selects `any`; matrix selects **your exact target even though
`any` also exists**. Each response has exactly one payload URL/descriptor and the
entire one/five-artifact inventory. GET expiry is 600 seconds; session lifetime
is two hours. Renewal changes only URLs, preserving session ID and deadline.
The helper never prints signed URLs or sends the dev PAT to S3.

The same portable release can be selected for a different consumer target,
without pretending your machine can run it:

```bash
m15_fixture probe portable --target x86_64-pc-windows-msvc
```

Expected: `any`, unchanged logical release integrity. This is a real server
selection check, **not Windows execution evidence**. Full cross-host execution
remains native CI/actual host evidence.

## 4. Actual CLI Install, Verification and Native Execution

Create the portable consumer first:

```bash
m15_fixture consumer portable --consumer "$M15_ROOT/consumer-portable"
cd "$M15_ROOT/consumer-portable"
m15 lint --strict --schema "$M15_SCHEMA"
m15 install --require-attestation --require-signature
m15_fixture check portable --consumer "$PWD"
```

The install must complete through the real CLI's root/key-set/Registry v3/author
v2/release verification and selected artifact hash/size checks. Strict flags do
not substitute for a public key supplied by the fixture. The helper then checks
the actual local selection, cache, logical lock integrity and compiled Pydantic
Core import path inside the managed venv; it saves `m15-evidence.json`.

```bash
export M15_PORTABLE_REF="$(m15_python -c 'import json,os;from pathlib import Path;r=Path(os.environ["M15_ROOT"]);s=json.loads((r/"session.json").read_text());print("@"+s["namespace"]+"/"+s["name"]+"@0.1.0")')"
m15 run "$M15_PORTABLE_REF" --input '{"quantity":2,"unit_price":4.5}'
m15 run "$M15_PORTABLE_REF" --machine --input '{"quantity":2,"unit_price":4.5}'
```

Expected: `{"total":9.0}`; machine envelope `status: success`, `output.total: 9`.
No consumer pip install, `uv sync`, venv activation or resolution refresh occurs.

Now install the version containing both `any` and exact-target artifacts into
a separate, empty consumer and save the machine result:

```bash
m15_fixture consumer matrix --consumer "$M15_ROOT/consumer-exact"
cd "$M15_ROOT/consumer-exact"
m15 lint --strict --schema "$M15_SCHEMA"
m15 install --machine --require-attestation --require-signature > "$M15_ROOT/evidence/exact-install.json"
cat "$M15_ROOT/evidence/exact-install.json"
m15_fixture check matrix --consumer "$PWD"
export M15_EXACT_REF="${M15_PORTABLE_REF%@*}@0.1.1"
m15 run "$M15_EXACT_REF" --machine --input '{"quantity":2,"unit_price":4.5}'
cp agent.lock "$M15_ROOT/evidence/exact.lock"
```

Expected: one JSON success on machine stdout, exact target in both descriptor
and local selection state, exactly one payload cache file, and successful native
execution. `agent.lock` contains the **whole-release digest**, not the chosen
archive's digest/target. Section 3 independently showed only that one URL was
returned. Other targets are not downloaded into this workspace.

## 5. Fresh Frozen Consumer, Same Portable Lock

```bash
mkdir "$M15_ROOT/consumer-frozen" "$M15_ROOT/cold-home"
cp "$M15_ROOT/consumer-exact/agent.json" "$M15_ROOT/consumer-exact/agent.lock" "$M15_ROOT/consumer-frozen/"
cd "$M15_ROOT/consumer-frozen"
m15_cold install --frozen --require-attestation --require-signature
cmp "$M15_ROOT/evidence/exact.lock" agent.lock
m15_fixture check matrix --consumer "$PWD"
m15_cold run "$M15_EXACT_REF" --machine --input '{"quantity":2,"unit_price":4.5}'
```

Expected: empty project/runtime and isolated user resolver/key caches still
produce a valid install; the same pinned logical version and lock bytes survive.
The CLI fetches/verifies the public key set and managed resolver through normal
paths. This is same-host cold installation, not another architecture.

## 6. Cache Reverification and Local Target-State Repair

Return to the **exact consumer**, not the frozen copy:

```bash
cd "$M15_ROOT/consumer-exact"
m15_python - <<'PY'
from pathlib import Path
import json
evidence = json.loads(Path('m15-evidence.json').read_text())
cache = Path(evidence['cache'])
data = cache.read_bytes()
cache.write_bytes(bytes([data[0] ^ 1]) + data[1:])
print('Corrupted only this disposable consumer payload cache:', cache)
PY
m15 install --frozen --require-attestation --require-signature
m15_fixture check matrix --consumer "$PWD"
cmp "$M15_ROOT/evidence/exact.lock" agent.lock
```

Expected: corruption is not accepted as a cache hit. Normal install obtains good
bytes again; verified hash, native environment and lock remain correct.

Change only this disposable consumer's local target marker:

```bash
m15_python - <<'PY'
from pathlib import Path
import json
state = next(Path('.agentpm/tools').rglob('.0.1.1.selection.json'))
value = json.loads(state.read_text())
value['target'] = 'x86_64-pc-windows-msvc' if value['target'] != 'x86_64-pc-windows-msvc' else 'aarch64-apple-darwin'
state.write_text(json.dumps(value))
print('Changed local target marker:', state)
PY
if m15 run "$M15_EXACT_REF" --machine --input '{"quantity":2,"unit_price":4.5}'; then M15_EXIT=0; else M15_EXIT=$?; fi
printf 'Exit: %s (expected nonzero: reinstall on this machine)\n' "$M15_EXIT"
m15 install --frozen --require-attestation --require-signature
m15_fixture check matrix --consumer "$PWD"
cmp "$M15_ROOT/evidence/exact.lock" agent.lock
```

Expected: execution rejects the changed target/state; reinstall repairs the local
selection from verified cached bytes without silently upgrading or changing the
portable lock. This simulates copied machine state, not actual emulation.

## 7. Failed Provisioning Cannot Finalize; Exact Incompatibility Cannot Upgrade

Use the **portable** version in a new consumer, with a second empty process HOME
so no already-acquired resolver hides the deliberate bad override. Snapshot its
existing sessions first; pending inspection sessions from section 3 are normal.

```bash
m15_fixture consumer portable --consumer "$M15_ROOT/consumer-repair"
mkdir "$M15_ROOT/failed-home"
m15_fixture sessions portable --out "$M15_ROOT/evidence/before-failure-sessions.json"
cd "$M15_ROOT/consumer-repair"
if env HOME="$M15_ROOT/failed-home" XDG_CACHE_HOME="$M15_ROOT/failed-home/.cache" \
  XDG_CONFIG_HOME="$M15_ROOT/failed-home/.config" AGENTPM_TELEMETRY=0 \
  AGENTPM_TOKEN="$M15_TOKEN" AGENTPM_PYTHON="$M15_INTERPRETER" \
  AGENTPM_UV_PATH="$M15_ROOT/does-not-exist" \
  "$M15_APM" --base-url "$M15_BASE" install --machine --require-attestation --require-signature \
  > "$M15_ROOT/evidence/provisioning-failure.json"; then M15_EXIT=0; else M15_EXIT=$?; fi
printf 'Exit: %s (expected nonzero: invalid resolver override)\n' "$M15_EXIT"
cat "$M15_ROOT/evidence/provisioning-failure.json"
m15_fixture sessions portable --baseline "$M15_ROOT/evidence/before-failure-sessions.json"
test ! -f agent.lock && printf 'PASS: no successful lock was written.\n'
```

Expected: payload can already be extracted, but provisioning fails explicitly,
no ready environment/new lock is accepted, and the new DB session stays pending.
The read-only comparison asserts **no newly created session completed**. Do not
interpret a leftover extracted payload as a completed managed installation.

Recover with the normal interpreter and real resolver:

```bash
m15 install --require-attestation --require-signature
m15_fixture check portable --consumer "$PWD"
m15 run "$M15_PORTABLE_REF" --machine --input '{"quantity":2,"unit_price":4.5}'
m15_fixture sessions portable
```

Expected: install repairs/provisions and creates a new completed session; the
old failed attempt may remain pending until expiry. No manual DB cleanup.

Now test the explicitly incompatible **0.1.2**, in another fresh directory:

```bash
m15_fixture consumer incompatible --consumer "$M15_ROOT/consumer-incompatible"
cd "$M15_ROOT/consumer-incompatible"
if m15 install --machine --require-attestation --require-signature > "$M15_ROOT/evidence/incompatible.json"; then M15_EXIT=0; else M15_EXIT=$?; fi
printf 'Exit: %s (expected nonzero: no compatible artifact)\n' "$M15_EXIT"
cat "$M15_ROOT/evidence/incompatible.json"
m15_python - <<'PY'
import json, os
from pathlib import Path
root = Path(os.environ['M15_ROOT'])
value = json.loads((root/'evidence/incompatible.json').read_text())
session = json.loads((root/'session.json').read_text())
assert value['status'] == 'error'
assert value['error']['category'] == 'no_compatible_artifact'
details = value['error']['details']
assert details['version'] == '0.1.2' and details['target'] == session['target']
assert details['available_targets'] and session['target'] not in details['available_targets']
assert not Path('agent.lock').exists() and not Path('.agentpm/tools').exists()
print('PASS: exact incompatible request failed before download/lock, with target diagnostics.')
PY
```

There must be no prompt, downgrade to 0.1.1, `any` invention or emulation. The
interactive-only newer-version offer is exercised by the focused tests in
section 9; this real exact request deliberately cannot offer a replacement.

## 8. Harness Executes the Installed New-Format Tool

Use the successful **exact consumer**. Stage an unpublished local Loop solely
for this model-free Harness check, then restore the manifest/lock afterward.
Do not run install while this local-only loop is present.

```bash
cd "$M15_ROOT/consumer-exact"
cp agent.json "$M15_ROOT/evidence/before-harness-agent.json"
cp agent.lock "$M15_ROOT/evidence/before-harness.lock"
mkdir -p .agentpm/loops/fixture/m11c-order-loop/0.1.0
cp "$M15_PREVIOUS/loop.fixture.json" .agentpm/loops/fixture/m11c-order-loop/0.1.0/agent.json
cp "$M15_PREVIOUS/harness.fixture.json" agentpm.harness.json
m15_python - <<'PY'
import json, os
from pathlib import Path
root = Path(os.environ['M15_ROOT'])
session = json.loads((root/'session.json').read_text())
tool = '@'+session['namespace']+'/'+session['name']
loop = '@fixture/m11c-order-loop'
key = 'loop:'+loop+'@0.1.0'
manifest = json.loads(Path('agent.json').read_text())
manifest['loop'] = loop+'@0.1.0'
manifest['bindings'] = {'global': {'tools': [tool]}}
Path('agent.json').write_text(json.dumps(manifest, indent=2)+'\n')
lock = json.loads(Path('agent.lock').read_text())
lock['packages'][key] = {'kind':'loop','name':loop,'version':'0.1.0','integrity':'0'*64}
lock['roots']['local:agent']['loop'] = key
Path('agent.lock').write_text(json.dumps(lock, indent=2)+'\n')
PY
m15 lint --strict --schema "$M15_SCHEMA"
m15 lint --strict .agentpm/loops/fixture/m11c-order-loop/0.1.0/agent.json --schema "$M15_SCHEMA"
m15_fixture provider-command
```

**Copy the complete command printed by `provider-command`, open a second
terminal, paste that printed command there, and leave it running.** It contains
absolute paths and needs no first-terminal variables or setup. It starts only
a local scripted model on port 18160. If occupied, change `M15_MODEL_PORT` in
the first terminal and print the command again.

Return to the **first terminal**, still in `consumer-exact`:

```bash
curl --fail --show-error "http://127.0.0.1:$M15_MODEL_PORT/health"
OPENAI_API_KEY=local-fixture-only OPENAI_BASE_URL="http://127.0.0.1:$M15_MODEL_PORT/model" \
  m15 harness --headless --input 'Validate two units at 4.5 each using the order Tool.' \
  --report "$M15_ROOT/evidence/harness-report.json" > "$M15_ROOT/evidence/harness-output.json"
cat "$M15_ROOT/evidence/harness-output.json"
m15_python - <<'PY'
import json, os
from pathlib import Path
evidence = Path(os.environ['M15_ROOT'])/'evidence'
assert json.loads((evidence/'harness-output.json').read_text()) == {'total':9.0,'verified_tool_result':True}
events = [json.loads(line) for line in (evidence/'harness-events.jsonl').read_text().splitlines()]
assert any(e['event']=='tool_requested' for e in events)
assert any(e['event']=='tool_result_observed' for e in events)
assert not any(e['event']=='fixture_error' for e in events)
report = json.loads((evidence/'harness-report.json').read_text())
assert report['terminal_status']=='ended' and report['usage']['tool_calls'] >= 1
print('PASS: Harness observed the real new-format Tool result through its managed runtime.')
PY
cp "$M15_ROOT/evidence/before-harness-agent.json" agent.json
cp "$M15_ROOT/evidence/before-harness.lock" agent.lock
cmp "$M15_ROOT/evidence/exact.lock" agent.lock
m15_fixture check matrix --consumer "$PWD"
```

Stop the provider with **Ctrl-C in the second terminal**. No paid model API or
real OpenAI key is used; the model reports success only after seeing the actual
Tool result. The `m11c` fixture names are deliberately reused test infrastructure.

## 9. Legacy Vendoring and Controlled Boundary Checks

The `legacy` fixture has no declared dependencies, ships a tiny `_vendor/helper.py`
and uses the ordinary legacy CLI publish path. Use an existing encrypted author
key whose public half is **active in your dev namespace**. It is distinct from
the raw disposable fixture seed above; the fixture did not import that seed into
your CLI keystore.

```bash
m15 keys list
read 'M15_LEGACY_KEY_ID?Existing encrypted CLI key ID registered in dev: '
export M15_LEGACY_KEY_ID
cd "$M15_ROOT/legacy"
m15 lint --strict --schema "$M15_SCHEMA"
m15 publish --sign --key-id "$M15_LEGACY_KEY_ID" --namespace "$M15_NAMESPACE" --schema "$M15_SCHEMA"
```

Run publish by itself so its passphrase prompt reads the terminal. If needed,
export this existing key's public half and register it in dev before publishing,
as in M14C section 7. Do not change production signer policy or paste a private key.

```bash
m15_fixture consumer legacy --consumer "$M15_ROOT/consumer-legacy"
cd "$M15_ROOT/consumer-legacy"
m15 install --require-attestation --require-signature
export M15_LEGACY_REF="${M15_PORTABLE_REF%@*}-legacy@0.1.0"
m15 run "$M15_LEGACY_REF" --machine --input '{}'
m15_python - <<'PY'
from pathlib import Path
assert list(Path('.agentpm/tools').rglob('_vendor/helper.py'))
assert not list(Path('.agentpm/tools').rglob('agentpm-artifact.json'))
assert not list(Path('.agentpm/tools').rglob('.*.selection.json'))
assert not list(Path('.agentpm/runtimes/python').rglob('ready.json'))
print('PASS: legacy vendoring ran without inventing new-format selection or a managed environment.')
PY
```

Expected: successful total 9 from vendored code, trusted historical author v1 /
Registry v2 verification and unchanged legacy runtime behavior.

The following deterministic checks cover dangerous/costly boundaries without
editing real signed rows, expiring sessions by SQL, corrupting S3 or creating a
multi-gigabyte bomb. Run them from the **repo**, not a consumer directory:

```bash
cd "$M15_REPO"
cargo test --locked -p agentpm-cli --bin agentpm io::download::
cargo test --locked -p agentpm-cli --bin agentpm commands::install::compatibility::
cargo test --locked -p agentpm-cli --test target_install
cargo test --locked -p agentpm-sdk --test install_provenance
```

Expected: all pass. The download tests use real local HTTP plus small injected
caps/timeouts to prove expired-GET automatic renewal, corrupt/size/truncated/idle
failure, unsafe links/paths/entry/decompression bounds and previous-byte
preservation. SDK tests reject changed/duplicate renewal metadata. Compatibility
tests verify candidate range/exact/locked/machine restrictions; they do not claim
an actual operator accepted a recovery prompt. An exact incompatible production
version must always fail rather than offering an unapproved change.

## 10. Finish and Keep the Evidence

After all installs (including strict legacy) pass, revoke **only this guide's
disposable signer UUID** and restore the local release flag to its previous
value in IntelliJ if changed. Existing accepted signatures remain historical
evidence; ordinary revocation does not retroactively invalidate them.

```bash
cd "$M15_REPO"
m15 namespace revoke-signer --namespace "$M15_NAMESPACE" --signer-id "$M15_SIGNER_ID"
printf 'Private scratch/evidence to retain until acceptance: %s\n' "$M15_ROOT"
unset M15_TOKEN
```

Keep the public dev root/key-set inventory and encrypted offline root/backup
from M14C. Do not delete or regenerate them. Final dev artifacts and inspection
sessions remain real test history; no automatic bucket/DB deletion is performed.
Scratch contains the disposable author seed and expired signed URLs, so archive
it privately or remove it only after evidence is no longer needed.

Record the two repository revisions, selected target/artifact and release
digests from each consumer's `m15-evidence.json`, strict install/run results,
frozen lock comparisons, failure/repair outcome and Harness report. Passing this
guide closes M15's **dev manual acceptance** only. Production root/pin/key-list
bootstrap, authoritative CLI-main parity, hosted finalize capacity and the real
production-chain end-to-end smoke remain **unchecked Band 11 gates**.
