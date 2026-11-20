# M5A: APDS Identity Through Real Local Resolve, Install and Lockfiles

Claude review has passed. At the operator's request, **manual testing comes
before hosted CI for this milestone**. After these checks, push the reviewed
changes together and run hosted CLI/Registry/SDK CI before closing M5A. The
operator supplied a passing **38-result** audit on **2026-11-20**; hosted CI
remains pending. These instructions remain available for repeat verification.

Start in the **CLI repository root** and run one block at a time. Open the
printed workspace in your editor. All generated sources, archives, installed
packages, locks and evidence stay under gitignored
**`agentpm/cli-m5a-test/<run-date>/`**. Builder replay folders, if present, are
separate from your timestamped operator run.

There are two explicitly different test surfaces:

- **Real local Registry:** your normal IntelliJ-launched API at
  `http://127.0.0.1:5000`, your local Docker application database and development
  `agentpm-dev-tools` S3 bucket. Five small signed publications remain in this
  local Registry. These are new disposable test identities, not changes to
  production or maintained examples. The helper's DB connection is read-only;
  publication writes go through your real HTTP API.
- **Controlled download faults:** a separate loopback-only fixture server serves
  deliberately inconsistent responses/archives to the actual CLI. These inputs
  cannot normally be published through the enforcing Registry. They are never
  uploaded to S3 or written to your real Registry database.

No model calls or paid services. Node **20+**, Python **3.11+**, the existing
API test virtualenv, local API/Docker DB and development S3 are required. The
five publications use the existing single-archive protocol, including the two
Node Tool versions. Their lock integrity is the archive SHA-256. Native Python
and multi-artifact release/target verification remain covered by the recorded
builder tests and later hosted native CI; these manual commands do not claim
to reproduce that separate matrix. Author signatures are verified on real
installs. Do not add `--require-attestation`: the released trust-root inventory
pins production, not a new local Registry root.

| Section | What you are checking                                                              |
| ------- | ---------------------------------------------------------------------------------- |
| 1       | Build once; create visible sources and a private local test workspace              |
| 2       | Seed one missing-selector historical simulation while **local** enforcement is off |
| 3       | Restore local enforcement and publish four declared releases                       |
| 4       | Actual resolve responses and installed locks retain explicit/inferred origin       |
| 5       | Ordinary regeneration and supported simple frozen replay preserve bytes            |
| 6       | Local Agent/Skill/Tool roots refresh from authored source                          |
| 7       | Exact Tool versions coexist and produce visibly different outputs                  |
| 8       | Deliberately bad download metadata/bytes are rejected without replacement          |
| 9       | Verified cache repairs tampering, including an older response without metadata     |
| 10      | Older CLI, future lock, unknown field and partial origin fail without rewriting    |
| 11      | Audit evidence; operator acceptance first, hosted CI afterward                     |

A negative test passes only when **the actual error matches the stated defect**.
401/403, expired URLs, 429, missing executables and unrelated 500 errors are not
substitutes for the expected result. Stop on a mismatch.

## 1. Build and prepare one local workspace

Use your normal macOS/zsh terminal, **Terminal A**. Adjust only the first `cd` if
your checkout moved. Keep this terminal open.

```bash
cd /Users/zackhine/projects/agentpm-project/agentpm
cargo build --locked -p agentpm-cli
umask 077
export M5A_REPO="$PWD"
export M5A_API="$M5A_REPO/../agentpm-api"
export M5A_PYTHON="$M5A_API/.venv/bin/python"
export M5A_APM="$M5A_REPO/target/debug/agentpm"
export M5A_OLD_APM="$(command -v agentpm)"
export M5A_FIXTURE="$M5A_REPO/specs/2026-10-09-category-hardening/manual-tests/fixtures/m5a/fixture.py"
export M5A_ROOT="$M5A_REPO/cli-m5a-test/$(date -u +%Y%m%dT%H%M%SZ)-$$"
export M5A_BASE='http://127.0.0.1:5000'
export M5A_NAMESPACE='zack'
export M5A_DATABASE_URL='postgresql://postgres:postgres@127.0.0.1:5433/postgres'

m5a_fixture() {
  local m5a_action="$1"
  shift
  "$M5A_PYTHON" -B "$M5A_FIXTURE" "$m5a_action" "$M5A_ROOT" "$@"
}
m5a_ref() {
  "$M5A_PYTHON" -c 'import json,sys; print(json.load(open(sys.argv[1]))["refs"][sys.argv[2]])' \
    "$M5A_ROOT/session.json" "$1"
}
m5a() {
  [[ "$M5A_BASE" == 'http://127.0.0.1:5000' ]] || return 1
  env AGENTPM_TOKEN="${M5A_TOKEN:?Set your LOCAL PAT}" AGENTPM_TELEMETRY=0 NO_COLOR=1 \
    "$M5A_APM" --base-url "$M5A_BASE" "$@"
}
m5a_fault() {
  local m5a_case="$1" m5a_origin
  shift
  m5a_origin="$("$M5A_PYTHON" -c 'import json,sys; from urllib.parse import urlsplit; v=json.load(open(sys.argv[1])); u=urlsplit(v["base_url"]); assert u.scheme=="http" and u.hostname=="127.0.0.1" and u.port; print(v["base_url"])' "$M5A_ROOT/control-server.json")" || return 1
  env AGENTPM_TOKEN=apm_m5a.fixture AGENTPM_TELEMETRY=0 NO_COLOR=1 \
    "$M5A_APM" --base-url "$m5a_origin/$m5a_case" "$@"
}
m5a_expect() {
  local m5a_label="$1" m5a_expected="$2" m5a_actual
  shift 2
  pwd > "$M5A_ROOT/evidence/$m5a_label.cwd.txt"
  printf '%q ' "$@" > "$M5A_ROOT/evidence/$m5a_label.command.txt"
  printf '\n' >> "$M5A_ROOT/evidence/$m5a_label.command.txt"
  if "$@" > "$M5A_ROOT/evidence/$m5a_label.stdout.txt" \
           2> "$M5A_ROOT/evidence/$m5a_label.stderr.txt"; then
    m5a_actual=0
  else
    m5a_actual=$?
  fi
  cat "$M5A_ROOT/evidence/$m5a_label.stdout.txt"
  cat "$M5A_ROOT/evidence/$m5a_label.stderr.txt" >&2
  printf '{"exit_code":%s,"expected_exit_code":%s}\n' "$m5a_actual" "$m5a_expected" \
    > "$M5A_ROOT/evidence/$m5a_label.result.json"
  printf 'ACTUAL exit %s | EXPECTED exit %s | %s\n' "$m5a_actual" "$m5a_expected" "$m5a_label"
  [[ "$m5a_actual" == "$m5a_expected" ]]
}

"$M5A_PYTHON" --version
node --version
m5a_fixture prepare --namespace "$M5A_NAMESPACE" --cli "$M5A_APM"
"$M5A_PYTHON" - <<'PY'
import os, shlex
from pathlib import Path
root = Path(os.environ['M5A_ROOT'])
names = ['M5A_REPO','M5A_ROOT','M5A_PYTHON','M5A_FIXTURE','M5A_APM']
(root/'paths.sh').write_text(''.join('export '+n+'='+shlex.quote(os.environ[n])+'\n' for n in names))
print('Copy this command into Terminal B:')
print('source ' + shlex.quote(str(root/'paths.sh')))
PY
cd "$M5A_ROOT"
pwd
ls cases projects controlled
git -C "$M5A_REPO" check-ignore "$M5A_ROOT/author.seed"
m5a_expect version 0 "$M5A_APM" --version
"$M5A_OLD_APM" --version
```

Expected: reviewed CLI **0.1.47**, five named publication cases, eight consumer
projects and eleven controlled response cases. The installed older CLI should
be **0.1.46** for section 10. If it is already newer, stop and choose a retained
released 0.1.46 executable for `M5A_OLD_APM`; do not downgrade your normal install.
`check-ignore` prints the private seed path. Preparation has linted the declared
sources and run `lint --legacy` on the historical simulation, then created local
archives/signatures. **No package has been published or installed yet.**

`cases/*/source/agent.json` are publication sources; `projects/*/agent.json` are
editable consumer sources. `controlled/*/source`, `resolve.json` and
`init-item.json` display the independent download-fault inputs. `author.seed`
and presigned URLs are private test evidence. The PAT is read into Terminal A
below and is not written to `paths.sh` or command logs.

`m5a_expect` captures the exact visible command, working directory, both output
streams and expected/actual child exit codes. It does not turn any nonzero exit
into a semantic pass; the later `check-result` verifies the actual message too.
Do not rebuild or switch the reviewed binary midway through the run.

## 2. Seed the legacy simulation against your real local API

Start/restart your normal API from **IntelliJ using its existing local run
configuration**. Keep its actual local development AWS/Registry signing settings.
You do not need to export those IntelliJ secrets into Terminal A.

Confirm the API uses your local Docker application database and
`S3_BUCKET=agentpm-dev-tools`. For the checked-in `FLASK_ENV=local` configuration,
the database is `localhost:5433/postgres`, matching `M5A_DATABASE_URL` above.
If your local setup differs, reconcile that DSN before continuing. No test code
creates replacement application tables, accounts or namespaces.

For **this local seed phase only**, set `APDS_PUBLISH_ENFORCED=false` and restart
the local API. Keep `RELEASE_PUBLISH_ENABLED=true`. **Do not change production.**
The normal enforcing publisher cannot create a missing-selector historical
release, which is why this one explicitly labeled simulation is seeded first.

In Terminal A:

Use an existing **public development namespace** you can publish/manage and a
local PAT with the required publish, namespace-management and install access.

```bash
read -s 'M5A_TOKEN?Your real LOCAL development PAT: '
printf '\n'
export M5A_TOKEN
m5a_expect whoami 0 m5a whoami
m5a_fixture preflight
m5a namespace add-signer --namespace "$M5A_NAMESPACE" \
  --label 'M5A disposable manual signer' --pubkey "$M5A_ROOT/author-public-key.txt"

cat cases/legacy/source/agent.json
m5a_fixture describe legacy
m5a_expect publish-legacy 0 m5a_fixture publish legacy
m5a_fixture inspect-db legacy
m5a_fixture resolve legacy
```

Expected: one trusted **local** disposable author signer is registered. The raw
publication prints init → development S3 upload → finalize, all successful, and
saves the exact bodies/reservation/receipt. The source and downloaded archive
have **no `standard`**. Read-only DB inspection finds all four stored APDS columns
NULL, while the **actual new resolve response** reports effective
`agentpm/1.0.0`, `standard_origin: "legacy_inferred"` and the archive digest.
Resolve must not backfill or certify the row.

This is a local historical simulation, not a claim that this newly seeded version
was published before enforcement. Nothing edits an old immutable archive.

A 422 missing-standard rejection here means local enforcement is still on; stop
and correct the **local** run configuration. A 429 means rate limiting: wait for
the reset/`Retry-After`, then rerun the same `publish` command without preparing
or repacking the case. The existing transport's idempotent init/finalize handles
resumption. An already completed case keeps its receipt instead of republishing.

## 3. Restore enforcement and publish the declared test graph

Set **`APDS_PUBLISH_ENFORCED=true` in the local IntelliJ configuration**, restart
the API and leave that normal enforcing configuration running for the rest of
the run. No separate server wrapper or replacement DB is needed.

Publish these four cases **in this order**, one at a time:

```bash
m5a_fixture describe echo-v1
m5a_expect publish-echo-v1 0 m5a_fixture publish echo-v1
m5a_fixture inspect-db echo-v1

m5a_fixture describe echo-v2
m5a_expect publish-echo-v2 0 m5a_fixture publish echo-v2
m5a_fixture inspect-db echo-v2

m5a_fixture describe skill
m5a_expect publish-skill 0 m5a_fixture publish skill
m5a_fixture inspect-db skill

m5a_fixture describe agent
m5a_expect publish-agent 0 m5a_fixture publish agent
m5a_fixture inspect-db agent
```

Expected: the two Echo versions have the **same package name** and versions
`1.0.0`/`2.0.0`. The Skill references v1; the Agent references v2 and that Skill.
All four embedded manifests explicitly declare the standard. Actual local DB
records have origin **`declared`** and real conformance evidence from the
enforcing publisher, with Runner behavior unevaluated. No Harness execution is
promised. The author signatures are real and use only this run's local signer.

The helper reuses M4A's guarded HTTP/signature/storage routines. It is publishing
fixtures for install testing, not bypassing validation of the declared cases.
The local API performs its normal finalization checks against actual S3 bytes.
In `case.json`, `mode: "legacy"` names the **single-archive HTTP protocol**, not
APDS provenance. `expected_standard_origin` is the separate declared/inferred
distinction. A declared Node Tool can use that older transport format.

## 4. See real resolve metadata and install it

**Input:** a local Agent consumer directly selects declared Echo v2 and the legacy
Tool. It also selects the Skill, which selects Echo v1. This produces **four exact
installed identities**, including two versions of one Tool name.

```bash
m5a_fixture resolve graph
cd "$M5A_ROOT/projects/graph"
cat agent.json
m5a_expect install-graph 0 m5a install --require-signature --machine
cat agent.lock
m5a_fixture check-lock graph
```

Expected: success and a v4 lock with four packages. The two Echo entries and
Skill are `declared`; the legacy Tool is `legacy_inferred`. Their digests match
the real publication archives exactly. The local Agent root is also `declared`.
Installed manifests match those immutable source bytes, including the legacy
manifest's missing declaration. There is no chosen target, interpreter path or
`apds_validation` certificate in the portable package entries.

`check-lock` compares actual installed manifests, exact kind/name/version keys,
the expected origins and real archive hashes. It prints the actual lock and
fails on a discrepancy rather than decorating it.

## 5. Ordinary regeneration and simple frozen replay

**Input:** `simple` has two direct Tool dependencies and no transitive Skill
closure. That keeps this frozen check within M5A's supported path; M5B owns the
broader clean graph reconstruction tests.

```bash
cd "$M5A_ROOT/projects/simple"
cat agent.json
m5a_expect install-simple 0 m5a install --require-signature --machine
m5a_fixture check-lock simple
m5a_fixture snapshot simple regenerate-simple
m5a_expect regenerate-simple 0 m5a install --require-signature --machine
m5a_fixture unchanged simple regenerate-simple

m5a_fixture snapshot simple frozen-simple
mv .agentpm installed-before-frozen
m5a_expect frozen-simple 0 m5a install --frozen --require-signature --machine
m5a_fixture unchanged simple frozen-simple
m5a_fixture check-lock simple
```

Expected: both standard/origin pairs survive ordinary regeneration. Repeating
the same install preserves manifest, lock and installed file bytes. After moving
the old install aside, frozen replay reconstructs both exact Tools and preserves
**every lock byte**. You can compare `.agentpm/` with `installed-before-frozen/`.
The helper's byte snapshot ignores mutable cache/log directories and compares
the manifest, lock and all installed package files.

## 6. Local roots use authored source, including legacy inference

First inspect the Skill and Tool author roots:

```bash
cd "$M5A_ROOT/projects/local-skill"
cat agent.json
m5a_expect install-skill 0 m5a install --require-signature --machine
m5a_fixture check-lock local-skill

cd "$M5A_ROOT/projects/local-tool"
cat agent.json
m5a_expect install-local-tool 0 m5a install --machine
m5a_fixture check-lock local-tool
```

Expected: `local:skill` and `local:tool` carry their actual authored name/version,
standard and `declared` origin. The Skill has one installed Tool. The Node Tool
author project has an empty package map and a represented local Tool root; this
is not the separate empty-Agent install claim.

Now deliberately edit **only the consumer source**, not a published package:

```bash
cd "$M5A_ROOT/projects/simple"
m5a_fixture mutate simple rename-local
cat agent.json
m5a_expect refresh-local 0 m5a install --require-signature --machine
m5a_fixture check-lock simple

m5a_fixture mutate simple legacy-local
cat agent.json
m5a_fixture snapshot simple frozen-origin
m5a_expect frozen-origin 1 m5a install --frozen --require-signature --machine
m5a_fixture check-result frozen-origin
m5a_fixture unchanged simple frozen-origin

m5a_expect infer-local 0 m5a install --require-signature --machine
m5a_fixture check-lock simple
m5a_fixture mutate simple declared-local
m5a_expect restore-local 0 m5a install --require-signature --machine
m5a_fixture check-lock simple
```

Expected: normal regeneration refreshes the local root to `renamed-consumer`
version `2.0.0`. Removing the local selector conflicts with frozen pinned origin:
the CLI says **`Local APDS declaration changed`**, and changes nothing. A normal
install then records that local root as **`legacy_inferred`**, with no new flag
and no source rewrite. Restoring the authored declaration and reinstalling makes
the root `declared` again. Installed dependency origins stay unchanged throughout.

## 7. Exact versions and an installed Agent root

```bash
cd "$M5A_ROOT/projects/graph"
m5a_expect run-v1 0 m5a run "$(m5a_ref echo-v1)" --machine --input '{"message":"first version"}'
m5a_expect run-v2 0 m5a run "$(m5a_ref echo-v2)" --machine --input '{"message":"second version"}'

cd "$M5A_ROOT/projects/direct-agent"
cat agent.json
m5a_expect install-agent 0 m5a install "$(m5a_ref agent)" --require-signature --machine
m5a_fixture check-lock direct-agent
```

Expected: the Tool results visibly contain versions `1.0.0` and `2.0.0`,
respectively, with the input echoed back. The direct Agent install records that
Agent, its Skill and both exact Tool versions, with declared origins on their
package entries and registry-root relationships. Installing its definition is
not a claim that it is runnable by the Harness. This section does not test a
direct Agent frozen replay.

## 8. Controlled bad downloads: see the defect and the actual failure

Keep your real API running. Open **Terminal B**, source the `paths.sh` printed
in section 1, and start the separate fixture server:

Replace the first line below with that exact printed `source` command; do not
type the placeholder path literally.

```bash
source /absolute/path/from-section-1/paths.sh
"$M5A_PYTHON" -B "$M5A_FIXTURE" serve "$M5A_ROOT"
```

Expected: it binds **127.0.0.1 on a different, automatically chosen port** and
says `controlled fixture, not real Registry`. It does not need your PAT, API
secrets or Docker DB. Leave this terminal running. Terminal A's `m5a_fault`
reads that local address and supplies a dummy fixture token instead of your PAT.

Back in **Terminal A**, establish a valid disposable installed Tool to protect:

```bash
cd "$M5A_ROOT/projects/repair"
m5a_fixture describe good
m5a_expect control-good 0 m5a_fault good install --machine
m5a_fixture check-lock repair
m5a_fixture snapshot repair failures
```

For each case below, `describe` prints the actual embedded manifest and the
independent resolve response. Each failure must leave this previous manifest,
lock and installed Tool **byte-identical**. Only test-owned cache entries may be
created/removed while validating downloads. No failed install may finalize.

### 8.1 Unsupported standard in resolve

The response advertises `agentpm/9.0.0`. The CLI should stop **before init or download**.

```bash
m5a_fixture describe unsupported-resolve
m5a_expect unsupported-resolve 1 m5a_fault unsupported-resolve install --machine
m5a_fixture check-result unsupported-resolve
m5a_fixture unchanged repair failures
```

### 8.2 Supported response, unsupported embedded standard

Resolve says 1.0.0; the hash-correct archive actually declares 9.0.0.

```bash
m5a_fixture describe unsupported-embedded
m5a_expect unsupported-embedded 1 m5a_fault unsupported-embedded install --machine
m5a_fixture check-result unsupported-embedded
m5a_fixture unchanged repair failures
```

Both cases report **`Unsupported APDS standard`**. Neither silently becomes legacy.

### 8.3 Malformed embedded standard

The hash-correct archive contains `"standard": null`, not an absent legacy selector.

```bash
m5a_fixture describe malformed-embedded
m5a_expect malformed-embedded 1 m5a_fault malformed-embedded install --machine
m5a_fixture check-result malformed-embedded
m5a_fixture unchanged repair failures
```

Expected: **`Malformed APDS standard declaration`**.

### 8.4 Kind, name and version mismatches

Run one case at a time: the archive is a Profile instead of a Tool; then it has
another local name; then another version. Their archive checksums are honest.

```bash
m5a_fixture describe wrong-kind
m5a_expect wrong-kind 1 m5a_fault wrong-kind install --machine
m5a_fixture check-result wrong-kind
m5a_fixture unchanged repair failures

m5a_fixture describe wrong-name
m5a_expect wrong-name 1 m5a_fault wrong-name install --machine
m5a_fixture check-result wrong-name
m5a_fixture unchanged repair failures

m5a_fixture describe wrong-version
m5a_expect wrong-version 1 m5a_fault wrong-version install --machine
m5a_fixture check-result wrong-version
m5a_fixture unchanged repair failures
```

Expected each time: **`Embedded package kind/name/version differs`**. A valid
checksum does not authorize a different identity.

### 8.5 False provenance and response drift

`false-origin` labels a declared artifact as inferred. `init-drift` resolves it as
declared but changes origin at init, so that second case must fail before download.

```bash
m5a_fixture describe false-origin
m5a_expect false-origin 1 m5a_fault false-origin install --machine
m5a_fixture check-result false-origin
m5a_fixture unchanged repair failures

m5a_fixture describe init-drift
m5a_expect init-drift 1 m5a_fault init-drift install --machine
m5a_fixture check-result init-drift
m5a_fixture unchanged repair failures
```

Expected: **`APDS standard/origin differs from the embedded package manifest`**,
then **`APDS standard/origin differs from the lock/resolve plan`**.

### 8.6 Wrong archive checksum

The response advertises 64 zeroes while serving the original valid archive bytes.

```bash
m5a_fixture describe wrong-digest
m5a_expect wrong-digest 1 m5a_fault wrong-digest install --machine
m5a_fixture check-result wrong-digest
m5a_fixture unchanged repair failures
```

Expected: **`sha256 mismatch`**, with the prior install and lock intact.

## 9. Cache repair and an older response without metadata

First corrupt only the cache file in the dedicated one-Tool test project:

```bash
cd "$M5A_ROOT/projects/repair"
m5a_fixture mutate repair cache
m5a_expect repair-cache 0 m5a_fault good install --machine
m5a_fixture check-lock repair
m5a_fixture unchanged repair failures
```

Expected: the corrupt cache is not trusted. The valid archive is fetched and
verified again; package/manifest/lock bytes match the original snapshot.

Now use a response that omits `standard` **and** `standard_origin`, like an older
Registry. Its archive still explicitly declares 1.0.0:

```bash
cd "$M5A_ROOT/projects/older-response"
m5a_fixture describe older-response
m5a_expect older-response 0 m5a_fault older-response install --machine
m5a_fixture check-lock older-response
m5a_fixture snapshot older-response selector-repair

m5a_fixture mutate older-response installed-selector
cat .agentpm/tools/fixture/controlled/1.0.0/agent.json
m5a_expect repair-selector 0 m5a_fault older-response install --machine
m5a_fixture check-lock older-response
m5a_fixture unchanged older-response selector-repair
```

Expected: initial install derives **`declared` from the actual archive**, not from
invented response metadata. Removing the installed selector is an intentional
test-only fault; normal install repairs it from verified cache, preserving the
original declared origin. The changed installed file must not become inferred
lock truth. Do not apply these fault-injection edits to real consumer apps.

## 10. Lock reader compatibility and unchanged failure state

Create an ordinary extended-v4 fixture lock, then use the **older released CLI**:

```bash
cd "$M5A_ROOT/projects/lock-guards"
m5a_expect guards-good 0 m5a_fault good install --machine
m5a_fixture check-lock lock-guards
m5a_fixture snapshot lock-guards older-cli
m5a_expect older-cli 1 env AGENTPM_TOKEN=apm_m5a.fixture AGENTPM_TELEMETRY=0 \
  "$M5A_OLD_APM" --base-url http://127.0.0.1:9 install --frozen --machine
m5a_fixture check-result older-cli
m5a_fixture unchanged lock-guards older-cli
```

Expected from released 0.1.46: its real **`unknown field`** diagnostic, before
network traffic, and unchanged bytes. It may name `standard` or `standard_origin`.
That safe but less helpful refusal is why all CLIs need upgrading before shared
locks are regenerated. The SDK v4 field tolerance is separately covered by the
published 0.1.17 probes in the [verification record](../m5a-verification.md).

Now each mutation starts from the saved valid lock, not from the previous defect:

```bash
m5a_fixture mutate lock-guards future-lock
cat agent.lock
m5a_fixture snapshot lock-guards future-lock
m5a_expect future-lock 1 m5a_fault good install --frozen --machine
m5a_fixture check-result future-lock
m5a_fixture unchanged lock-guards future-lock

m5a_fixture mutate lock-guards unknown-field
cat agent.lock
m5a_fixture snapshot lock-guards unknown-field
m5a_expect unknown-field 1 m5a_fault good install --frozen --machine
m5a_fixture check-result unknown-field
m5a_fixture unchanged lock-guards unknown-field

m5a_fixture mutate lock-guards partial-origin
cat agent.lock
m5a_fixture snapshot lock-guards partial-origin
m5a_expect partial-origin 1 m5a_fault good install --frozen --machine
m5a_fixture check-result partial-origin
m5a_fixture unchanged lock-guards partial-origin

m5a_fixture mutate lock-guards restore-lock
m5a_fixture check-lock lock-guards
```

Expected respectively: **`Unsupported agent.lock version 5`**, **`unknown field`**
and **`must be recorded together`**. Each invalid lock remains unchanged rather
than being downgraded or rewritten away. Installed bytes remain intact.

## 11. Audit and operator acceptance, then hosted CI

Keep your real local API running for the final read-only DB/API checks:

```bash
cd "$M5A_ROOT"
m5a_fixture audit
cat evidence/summary.json
```

Expected: all saved expected/actual CLI exits and intended error messages match,
all eight consumer lock/install states pass, the five real local publications
have the expected stored declaration state, and the reviewed binary hash has not
changed. `summary.json` deliberately leaves `operator_signoff: false` and
`hosted_ci_complete: false`: helper checks cannot grant either approval.

If every displayed input/result agrees, report that manual testing passed.
Then push all reviewed changes together and verify hosted Registry, CLI and SDK
CI, including the native target jobs, **before marking M5A complete**. For SDK
pre-merge CI, select the matching CLI ref through the existing `cli_ref` input or
`AGENTPM_CLI_REF` variable until the synchronized fixture is on CLI `main`.

Stop Terminal B's controlled server with **Ctrl-C**. Your real local API should
remain on its normal **enforcement-enabled** configuration. Retain the workspace
as private evidence; the five dev releases and the disposable local signer remain
in your local Registry. This guide does not delete them, change namespace policy,
publish production packages, release CLI 0.1.47 or waive the later reader-first
release ordering gate.
