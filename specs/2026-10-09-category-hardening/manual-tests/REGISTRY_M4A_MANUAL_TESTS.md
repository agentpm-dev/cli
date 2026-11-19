# M4A: APDS at the Real Local Registry Publication Boundary

Run **one block at a time**, starting in the `agentpm` checkout. The operator
confirmed Claude review and **all hosted CI passed**. The operator confirmed all
36 manual cases passed on **2026-11-18**, completing M4A. These instructions
remain available for repeat verification and do not activate production enforcement.

All source files, archives, requests and evidence live in the gitignored folder
**`agentpm/registry-m4a-test/<run-date>/`**. Open that folder in your editor.
The helper is checked in at [fixtures/m4a](fixtures/m4a/README.md); generated
`agent.json` files are outside the immutable standard corpus.

**These are real publications.** HTTP requests go to your actual local API at
**`http://127.0.0.1:5000`**, through its real authentication and application code,
and write to your normal **local Docker PostgreSQL database**. Presigned uploads
go directly to your development **`agentpm-dev-tools` S3 bucket**. Small real dev
packages, pending reservations, signatures, attestations and scan records remain.
No mock Registry, disposable database or in-memory S3 substitutes for these
operator steps. The helper's DB connection is read-only; writes happen through
your actual API. There are no model calls or consumer Tool execution.

| Section | What it establishes                                                         |
| ------- | --------------------------------------------------------------------------- |
| 1       | Visible local sources, honest Knowledge/Memory builds, real signed archives |
| 2       | Actual local API/DB, real PAT, one disposable dev signer                    |
| 3       | Gate off: declared-but-unvalidated differs from undeclared history          |
| 4       | Gate on: a raw client cannot publish a missing declaration                  |
| 5       | APDS-valid bytes cannot bypass required/invalid author signatures           |
| 6       | All eight kinds publish through the raw API                                 |
| 7       | Exact selector, identity, JSON, semantic and packaged-file failures         |
| 8       | Actual selected Loop, invalid phase and historical dependency inference     |
| 9       | Every target checked; a bad Windows archive rejects the whole release       |
| 10      | All eight kinds publish through the ordinary reviewed CLI too               |
| 11      | Saved evidence, local settings restoration and operator sign-off            |

Every negative check names the **defect, expected HTTP/code and rule/path**.
HTTP 401/403, expired URLs, HTTP 429, storage errors and HTTP 500 are setup or
infrastructure failures, not passes. Stop on a mismatch. The helper fails if the
expected rejection is replaced by an unrelated error.

## 1. Create one visible test workspace

Use your normal macOS/zsh terminal, starting at the **CLI repository root**.
The API's Python 3.13 venv needs the existing `requirements-test.txt` packages.
Keep this terminal open as **Terminal A**.

```bash
cd /Users/zackhine/projects/agentpm-project/agentpm
cargo build --locked -p agentpm-cli
umask 077
export M4A_REPO="$PWD"
export M4A_API="$M4A_REPO/../agentpm-api"
export M4A_PYTHON="$M4A_API/.venv/bin/python"
export M4A_APM="$M4A_REPO/target/debug/agentpm"
export M4A_FIXTURE="$M4A_REPO/specs/2026-10-09-category-hardening/manual-tests/fixtures/m4a/fixture.py"
export M4A_ROOT="$M4A_REPO/registry-m4a-test/$(date -u +%Y%m%dT%H%M%SZ)"
export M4A_BASE='http://127.0.0.1:5000'
export M4A_NAMESPACE='zack'
export M4A_DATABASE_URL='postgresql://postgres:postgres@127.0.0.1:5433/postgres'
mkdir -p "$M4A_REPO/registry-m4a-test"
mkdir "$M4A_ROOT"

m4a_fixture() {
  local m4a_action="$1"
  shift
  env PYTHONPATH="$M4A_API" PYTHONDONTWRITEBYTECODE=1 \
    "$M4A_PYTHON" -B "$M4A_FIXTURE" "$m4a_action" "$M4A_ROOT" "$@"
}
m4a() {
  if [[ "$M4A_BASE" != 'http://127.0.0.1:5000' ]]; then
    printf 'Stop: this runbook only permits the local API.\n' >&2
    return 1
  fi
  env AGENTPM_TELEMETRY=0 AGENTPM_TOKEN="${M4A_TOKEN:-apm_m4a.local}" \
    "$M4A_APM" --base-url "$M4A_BASE" "$@"
}
m4a_expect() {
  local m4a_label="$1" m4a_expected="$2" m4a_actual
  shift 2
  pwd > "$M4A_ROOT/evidence/$m4a_label.cwd.txt"
  printf '%q ' "$@" > "$M4A_ROOT/evidence/$m4a_label.command.txt"
  printf '\n' >> "$M4A_ROOT/evidence/$m4a_label.command.txt"
  if "$@" > "$M4A_ROOT/evidence/$m4a_label.stdout.txt" \
           2> "$M4A_ROOT/evidence/$m4a_label.stderr.txt"; then
    m4a_actual=0
  else
    m4a_actual=$?
  fi
  cat "$M4A_ROOT/evidence/$m4a_label.stdout.txt"
  cat "$M4A_ROOT/evidence/$m4a_label.stderr.txt" >&2
  printf '{"exit_code":%s,"expected_exit_code":%s}\n' \
    "$m4a_actual" "$m4a_expected" > "$M4A_ROOT/evidence/$m4a_label.result.json"
  printf 'ACTUAL exit %s | EXPECTED exit %s | %s\n' \
    "$m4a_actual" "$m4a_expected" "$m4a_label"
  [[ "$m4a_actual" == "$m4a_expected" ]]
}

m4a_fixture prepare --namespace "$M4A_NAMESPACE"
"$M4A_PYTHON" - <<'PY'
import hashlib, json, os, shlex, subprocess, sys
from pathlib import Path
root = Path(os.environ['M4A_ROOT'])
(root / 'evidence').mkdir()
record = {'python': sys.version, 'workspace': str(root)}
for name in ['M4A_REPO', 'M4A_API']:
    repo = os.environ[name]
    record[name] = {'path': repo,
        'commit': subprocess.check_output(['git', '-C', repo, 'rev-parse', 'HEAD'], text=True).strip(),
        'worktree': subprocess.check_output(['git', '-C', repo, 'status', '--short'], text=True)}
record['cli_sha256'] = hashlib.sha256(Path(os.environ['M4A_APM']).read_bytes()).hexdigest()
(root / 'evidence/setup.json').write_text(json.dumps(record, indent=2) + '\n')
names = ['M4A_REPO', 'M4A_API', 'M4A_ROOT', 'M4A_FIXTURE']
(root / 'paths.sh').write_text(''.join('export '+name+'='+shlex.quote(os.environ[name])+'\n' for name in names))
print('Open this folder:', root)
print('Optional helper-launcher terminal: source ' + shlex.quote(str(root / 'paths.sh')))
PY
cd "$M4A_ROOT"
pwd
ls cases
git -C "$M4A_REPO" check-ignore "$M4A_ROOT/evidence/setup.json"
m4a_expect version 0 m4a --version
```

Expected: **36 named case directories** under this one workspace, each with
`source/agent.json` and a readable `case.json` explaining its intent. No network
request or publication has occurred. `check-ignore` prints the evidence path.
Any `builder-*` directories alongside your timestamp are builder replay artifacts;
they are not your manual run. Keep using the timestamp printed above.

The DB URL uses the public Compose credentials and **port 5433**. The running
`FLASK_ENV=local` configuration explicitly uses database **`postgres`**, even
though Compose initially creates `agentpm`. Do not choose a different database
merely because the connection succeeds. If your usual local API uses a different
configuration, reconcile that before starting; a missing namespace/table is not
permission to create replacement tables or users.

Build the authored content using the CLI, then package the raw-client inputs:

```bash
m4a_expect build-knowledge 0 m4a knowledge build --manifest cases/knowledge/source/agent.json
m4a_expect build-memory 0 m4a memory build --manifest cases/memory/source/agent.json
m4a_expect build-knowledge-missing 0 m4a knowledge build --manifest cases/knowledge-missing-file/source/agent.json
m4a_expect build-cli-knowledge 0 m4a knowledge build --manifest cases/cli-knowledge/source/agent.json
m4a_expect build-cli-memory 0 m4a memory build --manifest cases/cli-memory/source/agent.json
m4a_fixture pack-all
cat cases/knowledge/source/agent.json
cat cases/memory/source/memory/build.json
m4a_fixture describe matrix
```

Expected: builds derive real hashes, sizes and Memory contracts. `pack-all`
creates **28 raw-client requests and archive sets locally**, plus good/unsigned/
bad-signature finalize bodies. It does not lint the deliberately invalid inputs
away, call an API, upload or publish. The eight `cli-*` sources are reserved for
ordinary CLI publication later. The API Tool source is Node; the CLI Tool is
Python and should produce an `any` release artifact.

`knowledge-missing-file` retains a valid source document so its normal content
build is honest; packaging deliberately **omits that document from the tar**.
The two matrix cases use real tar bytes and honest per-target descriptors. They
are server inventory checks, not proof of native compilation on two platforms.

## 2. Start your real local API, with enforcement initially off

Run the API yourself using your **usual IntelliJ run configuration**, Python
interpreter and existing local environment values. There is no requirement to
start it through the fixture helper or copy your secrets into a terminal.

In that **local** run configuration, confirm:

- `S3_BUCKET=agentpm-dev-tools`, your normal development AWS credentials, and
  your existing local Registry signing key.
- Your normal local Docker database. For the checked-in `FLASK_ENV=local`
  configuration this is `localhost:5433/postgres`, matching `M4A_DATABASE_URL`.
- **`RELEASE_PUBLISH_ENABLED=true`**.
- **`APDS_PUBLISH_ENFORCED=false`** for this first phase.

Restart the API from IntelliJ after setting those flags. Migrations through
**V1.107** must succeed and the API must listen at `http://127.0.0.1:5000`.
Leave it running while the test commands use **Terminal A**.

IntelliJ environment variables belong to its launched process; other terminals
do not inherit them. The earlier `serve` wrapper is optional and is unnecessary
for this guide. You do not need `server-off.json` or `server-on.json`: the audit
uses the actual off-phase database evidence and on-phase API rejection instead.
**Do not substitute a dummy Registry key or copy production settings.**

The normal API retains its publish rate limits. A **429 is throttling, not a
validation pass**: wait for the indicated reset/`Retry-After`, then retry the
same stage without rebuilding its archive. Rapidly running many cases can reach
both minute and hour limits. The optional helper launcher can disable throttling
in its own local process, but it needs the same backend environment as IntelliJ;
its details are in the [helper guide](fixtures/m4a/README.md).

Back in **Terminal A**:

```bash
read -s 'M4A_TOKEN?Your real LOCAL development PAT: '
printf '\n'
export M4A_TOKEN
m4a_expect whoami 0 m4a whoami
m4a_fixture preflight

m4a namespace add-signer --namespace "$M4A_NAMESPACE" \
  --label 'M4A disposable manual signer' --pubkey "$M4A_ROOT/author-public-key.txt"
export M4A_DISPOSABLE_SIGNER_ID="$(m4a_fixture signer-id)"
printf 'Disposable dev signer: %s\n' "$M4A_DISPOSABLE_SIGNER_ID"
```

Use an existing public dev namespace you can publish to and manage. The PAT needs
publication and namespace-management access. `preflight` makes a **read-only**
connection to your actual local DB and checks the namespace and new columns.
Registration creates **one real dev signer** using this run's generated seed;
the private seed stays under the ignored workspace with mode 0600. Namespace
signing policy and your usual author keys are unchanged.

Saved init responses contain short-lived presigned URLs; `author.seed` is private.
Do not share the entire workspace or paste those files publicly. HTTP evidence
does not save the PAT or Authorization headers.

## 3. Off means unvalidated, not undeclared

There are three independent real publications here. Inspect the source, then
run each API stage explicitly:

```bash
m4a_fixture describe off-declared
m4a_fixture init off-declared
m4a_fixture upload off-declared
m4a_fixture finalize off-declared
m4a_fixture inspect off-declared --state declared-only

m4a_fixture describe off-undeclared
m4a_fixture init off-undeclared
m4a_fixture upload off-undeclared
m4a_fixture finalize off-undeclared
m4a_fixture inspect off-undeclared --state undeclared

m4a_fixture describe off-loop
m4a_fixture init off-loop
m4a_fixture upload off-loop
m4a_fixture finalize off-loop
m4a_fixture inspect off-loop --state undeclared
```

Expected for each API stage: init **HTTP 200**, S3 PUT **200**, finalize **200**.
Inspection reads real version/signature/attestation/scan rows and makes a real
detail GET, expected **200**. The off-state distinctions must be:

| Case             | Persisted ID/version/origin      | `apds_validation` |
| ---------------- | -------------------------------- | ----------------- |
| `off-declared`   | `agentpm` / `1.0.0` / `declared` | null              |
| `off-undeclared` | all null                         | null              |
| `off-loop`       | all null                         | null              |

None is falsely APDS-verified. The undeclared Loop is a new dev fixture used to
exercise historical-dependency compatibility later; it is not a backfill or an
edit to a pre-existing release. The returned website URL may use the configured
public website host; **the saved HTTP request origin and local DB rows establish
where publication occurred**. This guide never follows that URL to publish.

## 4. Enable the local gate and prove the raw-client rejection

In your **IntelliJ local API run configuration**, change
**`APDS_PUBLISH_ENFORCED=true`**, keep `RELEASE_PUBLISH_ENABLED=true`, and restart
that API process. Keep its existing database, credentials and signing settings.

In **Terminal A**:

```bash
m4a_fixture describe missing-standard
m4a_fixture init missing-standard
m4a_fixture upload missing-standard
m4a_fixture finalize missing-standard
m4a_fixture inspect missing-standard --state rejected
```

**Issue:** init's manifest declares the standard, but the actual tar manifest
omits it. The archives have correct SHA-256 and size; no CLI lint command controls
this API request.

Expected: init/PUT **200**, finalize **422 `apds_nonconformant`**, with
**`APDS-ID-001` at `/standard`**. The helper checks the exact code/rule/path.
Inspection finds the original pending reservation, **zero versions**, no linked
publication rows, and detail GET **404**. An identity-only package reservation
may exist; that is not a published or discoverable version.

If finalize returns 200, **stop**: enforcement is not active in the API process
you reached. Exporting a variable in Terminal A cannot reconfigure the IntelliJ API process.

## 5. Valid APDS does not override signature requirements

**Input:** `cases/agent/source/agent.json` is a valid dependency-free Agent
Package. It has no Loop and is not a claim of Runner readiness.

```bash
m4a_fixture describe agent
m4a_fixture init agent
m4a_fixture upload agent
m4a_fixture finalize agent --variant unsigned --expect 422 --code signature_required
m4a_fixture inspect agent --state rejected
m4a_fixture finalize agent --variant bad-signature --expect 422 --code invalid_signature
m4a_fixture inspect agent --state rejected
m4a_fixture finalize agent
m4a_fixture inspect agent --state conformant
m4a_fixture finalize agent
m4a_fixture inspect agent --state conformant
```

Expected: missing required author signature fails **422 `signature_required`**;
the explicitly corrupted signature fails **422 `invalid_signature`**. These are
signing errors, not APDS errors. Both attempts leave the version unpublished.
The saved good signature then publishes **200**. Repeating successful finalize
returns the same version ID, package ID and URL; the legacy receipt's human
message may change to `already published`.

The final inspection verifies declared identity, `status: conformant`, all three
scope results, `runner_evaluated: false`, one persisted author signature,
one Registry attestation and one scan row. Scan execution may still be queued;
APDS conformance is separate from malware results.

## 6. Other package kinds through the raw API

These are independent positive controls. Open their sources before each block.
The helper's `describe` command also lists the actual tar members.

| Case        | Actual content                                        |
| ----------- | ----------------------------------------------------- |
| `tool`      | Node `main.js`, no declared runtime dependencies      |
| `skill`     | `SKILL.md` procedural instructions                    |
| `knowledge` | Context document plus CLI-derived sizes/hashes        |
| `memory`    | Record schema plus CLI-built contracts and metadata   |
| `profile`   | Authored behavioral guidance and README               |
| `loop`      | Assess/execute/review graph with declared transitions |
| `template`  | `template/README.md` and a generation variable        |

```bash
m4a_fixture describe tool
m4a_fixture init tool
m4a_fixture upload tool
m4a_fixture finalize tool
m4a_fixture inspect tool
```

```bash
m4a_fixture describe skill
m4a_fixture init skill
m4a_fixture upload skill
m4a_fixture finalize skill
m4a_fixture inspect skill
```

```bash
m4a_fixture describe knowledge
m4a_fixture init knowledge
m4a_fixture upload knowledge
m4a_fixture finalize knowledge
m4a_fixture inspect knowledge
```

```bash
m4a_fixture describe memory
m4a_fixture init memory
m4a_fixture upload memory
m4a_fixture finalize memory
m4a_fixture inspect memory
```

```bash
m4a_fixture describe profile
m4a_fixture init profile
m4a_fixture upload profile
m4a_fixture finalize profile
m4a_fixture inspect profile
```

```bash
m4a_fixture describe loop
m4a_fixture init loop
m4a_fixture upload loop
m4a_fixture finalize loop
m4a_fixture inspect loop
```

```bash
m4a_fixture describe template
m4a_fixture init template
m4a_fixture upload template
m4a_fixture finalize template
m4a_fixture inspect template
```

Expected for every block: init/PUT/finalize **200**, exactly one published
version, declared APDS 1.0.0, **conformant** evidence, preserved author signature,
Registry attestation, scan record and detail GET **200**. No component executes.

## 7. Known defects through a custom publishing client

Each case reserves syntactically valid init metadata and supplies a real archive.
All have honest archive digest/size. **Do not run `agentpm publish` on these
negative sources**: its intended preflight would reject them before the Registry.

| Case                     | Deliberate issue                                   | Expected finalize            | Rule/field                                   |
| ------------------------ | -------------------------------------------------- | ---------------------------- | -------------------------------------------- |
| `unsupported-standard`   | Tar declares `9.0.0`                               | 422 `apds_unsupported`       | ID-001 `/standard/version`                   |
| `malformed-standard`     | Tar declares `standard: null`                      | 422 `apds_nonconformant`     | ID-001 `/standard`                           |
| `blank-description`      | Tar description is whitespace                      | 422 `apds_nonconformant`     | ID-002 `/description`                        |
| `wrong-name`             | Tar local name differs from reserved identity      | 422 `apds_identity_mismatch` | `/name`                                      |
| `wrong-kind`             | Tar is a valid Profile but reservation is an Agent | 422 `apds_identity_mismatch` | `/kind`                                      |
| `wrong-version`          | Tar version differs from reservation               | 422 `apds_identity_mismatch` | `/version`                                   |
| `standard-mismatch`      | Tar declares 1.0.0, init declares 9.0.0            | 422 `apds_manifest_mismatch` | `/standard`                                  |
| `malformed-json`         | Tar's root JSON is truncated                       | 422 `invalid_artifact`       | Artifact JSON failure, no invented APDS rule |
| `loop-invalid-entry`     | `entry_phase` is not a declared phase              | 422 `apds_nonconformant`     | LOOP-001 `/loop/entry_phase`                 |
| `knowledge-missing-file` | Declared context file omitted from tar             | 422 `apds_nonconformant`     | KNOW-002 `/knowledge/documents/0/path`       |

Rule IDs above have the full **`APDS-`** prefix in actual output.

For **each of the ten table rows**, run this block, changing **only the first
line** to that exact case label. Run one row at a time, not an unattended loop:

```bash
export M4A_CASE='unsupported-standard'
m4a_fixture describe "$M4A_CASE"
cat "cases/$M4A_CASE/request.json"
m4a_fixture init "$M4A_CASE"
m4a_fixture upload "$M4A_CASE"
m4a_fixture finalize "$M4A_CASE"
m4a_fixture inspect "$M4A_CASE" --state rejected
```

`describe` shows the case explanation, actual source, reserved manifest and tar
member list. The finalize helper prints the **actual and expected HTTP/code**
and verifies the saved rule/path, so a generic nonzero command cannot count as a
pass. Every inspection must find no published version/linked publication rows,
a pending reservation in your Docker DB, and API detail **404**. An immutable
storage object left by an interrupted attempt is not a discoverable version.

For the Knowledge case, compare the real built source file with its absence in
the printed archive members. The defect is in what was uploaded, even though
the local source and init metadata looked plausible.

## 8. Resolved composition uses actual selected archives

The required dependencies already exist: section 6's declared `loop` and section
3's undeclared `off-loop`. Both define `assess`, `execute`, `review`. Open the
Agent's `loop` reference and its `bindings.phases` before each request.

```bash
m4a_fixture describe graph-valid
m4a_fixture init graph-valid
m4a_fixture upload graph-valid
m4a_fixture finalize graph-valid
m4a_fixture inspect graph-valid --state conformant
```

Expected **200**: the referenced selected Loop actually contains `review`.

```bash
m4a_fixture describe graph-invalid
m4a_fixture init graph-invalid
m4a_fixture upload graph-invalid
m4a_fixture finalize graph-invalid
m4a_fixture inspect graph-invalid --state rejected
```

Expected **422 `apds_nonconformant`**, **`APDS-BIND-002`** at
`/bindings/phases/nonexistent`. Merely having accessible dependency metadata is
insufficient; the real selected Loop archive does not contain that phase.

```bash
m4a_fixture describe graph-legacy
m4a_fixture init graph-legacy
m4a_fixture upload graph-legacy
m4a_fixture finalize graph-legacy
m4a_fixture inspect graph-legacy --state incomplete
m4a_fixture inspect off-loop --state undeclared
```

Expected **200**, with a declared root but **incomplete** resolved evidence:
`APDS-ID-001` remains unevaluated and the note describes legacy-inferred dependency
interpretation. This is the deliberately allowed historical-declaration exception,
not a fully verified graph. Neither the original Loop archive nor its standard
metadata may be rewritten or retroactively marked declared/conformant.

### 8.1 Required dependency context is unavailable

**Issue:** this Agent names a unique Loop that was never published in your dev
namespace. Its standalone declarations are valid; required selected context is
unavailable to the Registry.

```bash
m4a_fixture describe graph-unavailable
m4a_fixture init graph-unavailable
m4a_fixture upload graph-unavailable
m4a_fixture finalize graph-unavailable
m4a_fixture inspect graph-unavailable --state rejected
```

Expected **422 `invalid_dependency_access`**, preserving the existing redacted
dependency-access error. There must be no published version or false conformant
result. This is an absent-dependency check, not proof of private-namespace ACL
coverage; those regressions remain in the automated suite.

## 9. Multi-artifact Python Tool release

### 9.1 Two valid target archives, one release

```bash
m4a_fixture describe matrix
m4a_fixture init matrix
m4a_fixture upload matrix --target aarch64-apple-darwin
m4a_fixture upload matrix --target x86_64-pc-windows-msvc
m4a_fixture finalize matrix
m4a_fixture inspect matrix --state conformant
m4a_fixture finalize matrix
m4a_fixture inspect matrix --state conformant
```

Expected **200**: one version, **two** artifact rows, one logical author signature,
one Registry attestation and **two** artifact scan rows. Evidence names both
checked targets. A completed retry retains the same identity, release digest and
successful URL. There is no `any` artifact because this fixture explicitly lists
only the two concrete descriptors; no native-platform compatibility is claimed.

### 9.2 The later Windows archive is defective

```bash
m4a_fixture describe matrix-missing-file
m4a_fixture init matrix-missing-file
m4a_fixture upload matrix-missing-file --target aarch64-apple-darwin
m4a_fixture upload matrix-missing-file --target x86_64-pc-windows-msvc
m4a_fixture finalize matrix-missing-file
m4a_fixture inspect matrix-missing-file --state rejected
```

Expected **422 `apds_nonconformant`**, **`APDS-ID-004` at `/files/0`**, with
`target: x86_64-pc-windows-msvc`. The Windows archive honestly hashes to the
reserved descriptor but omits `main.py`. The earlier target is valid.
There must be **no published version, artifact rows, author signature, attestation
or scan publication rows** for the rejected version. An earlier target's immutable
final object may remain as existing retry state; it does not make half a release
public. The reservation remains pending and detail GET is **404**.

## 10. Ordinary CLI publication for all eight kinds

These separate `cli-*` package names have not been reserved or published by the
raw API helper. This closes the normal user path in addition to bypass testing.
Use an **existing CLI author key registered in the dev namespace**. This is
separate from the helper's disposable raw-API seed.

```bash
m4a keys list
read 'M4A_CLI_KEY_ID?Existing CLI author key ID registered in your dev namespace: '
export M4A_CLI_KEY_ID
read -s 'AGENTPM_KEY_PASSPHRASE?Passphrase for that existing CLI key: '
printf '\n'
export AGENTPM_KEY_PASSPHRASE
```

If that key is not registered in the local namespace, export its **public** key
and register it there first; do not paste a private key into a signer form:

```bash
m4a keys export --key-id "$M4A_CLI_KEY_ID" --out "$M4A_ROOT/cli-author-public-key.txt"
# Only if not already registered in this local namespace:
# m4a namespace add-signer --namespace "$M4A_NAMESPACE" \
#   --label 'Existing CLI author key' --pubkey "$M4A_ROOT/cli-author-public-key.txt"
```

For **each row below**, change only the first line and run the block once:

| `M4A_CASE`      | Expected ordinary publication              |
| --------------- | ------------------------------------------ |
| `cli-agent`     | Agent Package                              |
| `cli-tool`      | Python Tool, one new-format `any` artifact |
| `cli-skill`     | Skill                                      |
| `cli-knowledge` | Built context Knowledge                    |
| `cli-memory`    | Built Memory Blueprint                     |
| `cli-profile`   | Instruction Profile                        |
| `cli-loop`      | Loop                                       |
| `cli-template`  | Template                                   |

```bash
export M4A_CASE='cli-agent'
m4a_fixture describe "$M4A_CASE"
cd "$M4A_ROOT/cases/$M4A_CASE/source"
m4a_expect "$M4A_CASE" 0 m4a publish --sign --key-id "$M4A_CLI_KEY_ID" --namespace "$M4A_NAMESPACE"
cd "$M4A_ROOT"
m4a_fixture inspect "$M4A_CASE" --state conformant
```

Expected: CLI exits **0**, prints the successful Registry package link and the
correct kind/version, and the real DB/API inspection agrees with declared,
conformant evidence. Python Tool publication has one artifact and one scan;
the other seven keep their existing single-archive storage path. None of these
commands is `--dry-run`: every block publishes to the real local Registry.

## 11. Audit evidence and restore local operation

```bash
cd "$M4A_ROOT"
m4a_fixture audit
cat evidence/acceptance-summary.json
m4a namespace revoke-signer --namespace "$M4A_NAMESPACE" --signer-id "$M4A_DISPOSABLE_SIGNER_ID"
unset M4A_TOKEN AGENTPM_KEY_PASSPHRASE
```

Expected audit: **36 saved case inspections** agree, all eight real CLI publish
commands exited 0, the raw negative codes/paths match, and both local server
phases are evidenced by actual publication results. Helper-launcher settings
files are not required. Audit reads saved evidence; it does not silently issue
additional publications or mark the milestone complete on your behalf.

Restore your IntelliJ/local API settings to their values before this run and
restart it. Until rollout is approved, **`APDS_PUBLISH_ENFORCED=false`** remains
the normal setting. Restore the release flag to its previous local value too.
If you used the optional helper launcher, stop that process first; its temporary
rate-limiter change ends with it. No production setting was changed.

Keep the ignored workspace for review. Published versions and historical signing
records are not deleted or rewritten. Revoking the disposable signer retains its
historical signatures. Pending invalid reservations and staging objects retain
their normal expiration/cleanup lifecycle; do not run SQL DELETE statements.

Report the run folder and whether all 36 checks passed. **Do not share the seed,
PAT, passphrase or presigned URL files.** These tests establish functional M4A
publication behavior, not maximum-size capacity. M4B's T-PUB-10 dependency-heavy
latency/disk/concurrency measurement and activation decision remain outstanding.
