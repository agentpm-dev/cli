# Release Band 11: Production Signed-Release Smoke

Use one new **Mac/zsh terminal**, starting in the `agentpm` repo. Run one block
at a time. This uses your **installed, released CLI 0.1.40**, production API,
production namespace and **`agentpm-tools`** bucket. It creates two small real
published test versions and one temporary author-signer registration.

The production root, verified key-list endpoint, strict legacy install and
CI/parity gates passed. The operator enabled `RELEASE_PUBLISH_ENABLED=true`
and redeployed. The existing 0.5 CPU / 512 MB host is retained by explicit
operator decision. `/tmp` had 58.12 GiB free; artifact/aggregate budgets remain
1,000,000,000 bytes. Maximum-size/concurrent capacity remains **unverified and
deferred**, not a passing load test. Start command remains `gunicorn run:app`;
without environment overrides its sync worker timeout is 30 seconds.

The [helper](fixtures/band11/fixture.py) only publishes its own unique
`release-check-<random>` package, to the exact production API and bucket, with
archives below **1 MiB each**. It reuses local authoring/inspection helpers and
the reviewed signing/validation models; it does not change product code or
relax the M13B/M15 development transport guards. It never accesses a database,
root private file, backend private key, AWS credentials or service configuration.

The two versions contain real Python source and a recorded Pydantic resolution:

- `portable` **0.1.0**: one `any` artifact, proving fallback.
- `matrix` **0.1.1**: `any` plus **your current concrete target**, proving exact
  selection wins over `any`. These are controlled descriptor fixtures built
  from the same portable code; they are not native matrix builds. The native
  execution proof is Pydantic Core running on your actual Mac after installation.

This publishes a separate verification package, not `@zack/validate-order`,
`summarize-text` or `devwork-copilot`. Ordinary CLI `publish` still uses the
legacy path; its dry run below prepares archives but does not publish them.
The helper explicitly exercises the production release API until M16 supplies
the public package-build/headless publishing workflow.

## 1. Prepare, Check Trust and Register the Disposable Author

```bash
cd /Users/zackhine/projects/agentpm-project/agentpm
umask 077
export BAND11_REPO="$PWD"
export BAND11_API="$BAND11_REPO/../agentpm-api"
export BAND11_PYTHON="$BAND11_API/.venv/bin/python"
export BAND11_APM="$(command -v agentpm)"
export BAND11_SCHEMA="$BAND11_REPO/schemas/agentpm.manifest.schema.json"
export BAND11_FIXTURES="$BAND11_REPO/specs/2026-10-06-immediate-hardening/manual-tests/fixtures/band11"
export BAND11_SMOKE_ROOT="$(mktemp -d "${TMPDIR:-/tmp}/agentpm-band11-smoke.XXXXXX")"
export BAND11_NAMESPACE='zack'
export BAND11_INTERPRETER='/opt/homebrew/opt/python@3.13/bin/python3.13'
unset BAND11_TOKEN AGENTPM_UV_PATH
band11_python() {
  env PYTHONPATH="$BAND11_API" PYTHONDONTWRITEBYTECODE=1 \
    REGISTRY_PRIVATE_KEY_B64=AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA= \
    AWS_EC2_METADATA_DISABLED=true "$BAND11_PYTHON" -B "$@"
}
band11_fixture() {
  local band11_action="$1"
  shift
  band11_python "$BAND11_FIXTURES/fixture.py" "$band11_action" "$BAND11_SMOKE_ROOT" "$@"
}
band11() {
  env AGENTPM_TELEMETRY=0 AGENTPM_TOKEN="${BAND11_TOKEN:-apm_band11.fixture}" \
    AGENTPM_PYTHON="$BAND11_INTERPRETER" \
    "$BAND11_APM" --base-url https://api.agentpackagemanager.com "$@"
}
"$BAND11_APM" --version
"$BAND11_INTERPRETER" --version
band11_fixture prepare --namespace "$BAND11_NAMESPACE"
git -C "$BAND11_REPO" rev-parse HEAD
git -C "$BAND11_API" rev-parse HEAD
printf 'Private scratch/evidence: %s\n' "$BAND11_SMOKE_ROOT"
```

Expected CLI **0.1.40**. Choose a production namespace you control before
preparation if needed. Preparation creates source and a disposable author seed
only, with private local permissions. The dummy environment key in the Python
wrapper merely permits imports; it never changes or signs for the backend.

Enter your production publish PAT, verify whoami and the live root-signed list,
then register the disposable public key in your production namespace:

```bash
read -s 'BAND11_TOKEN?Production publish PAT: '
printf '\n'
export BAND11_TOKEN
band11 whoami
band11_fixture preflight
band11 namespace add-signer --namespace "$BAND11_NAMESPACE" \
  --label 'Temporary release verification' --pubkey "$BAND11_SMOKE_ROOT/primary-public-key.txt"
```

Expected: correct account, trusted production key set, successful registration.
This disposable raw seed is restricted to this test fixture; your ordinary
encrypted CLI keystore and offline Registry root are not used or changed.

## 2. Build and Sign Locally

Run **portable first**, then matrix. Each helper package step requires its own
completed dry run and refuses to rebuild after a reservation has been created.

```bash
cd "$BAND11_SMOKE_ROOT/portable"
band11 lint --strict --schema "$BAND11_SCHEMA"
band11 publish --dry-run --refresh-python --schema "$BAND11_SCHEMA"
band11_fixture package portable
```

```bash
cd "$BAND11_SMOKE_ROOT/matrix"
band11 lint --strict --schema "$BAND11_SCHEMA"
band11 publish --dry-run --refresh-python --schema "$BAND11_SCHEMA"
band11_fixture package matrix
cd "$BAND11_REPO"
```

Expected: real resolver-mirror/dependency preparation, valid source archives and
saved author v2 signatures binding each complete inventory. No publication yet.
The payload contains dependency metadata, not installed distributions. Requests,
signatures, seed files and generated archives are outside the explicit file list.

## 3. Explicit Production Publication

These commands create the real production releases and GuardDuty scans:

```bash
band11_fixture init portable
band11_fixture upload portable
band11_fixture finalize portable
band11_fixture finalize portable
band11_fixture security portable
```

```bash
band11_fixture init matrix
band11_fixture upload matrix
band11_fixture finalize matrix
band11_fixture finalize matrix
band11_fixture security matrix
```

Expected: one accepted author signature per logical version; repeated finalize
returns that same immutable release. The `security` checks root-verified key
delivery, actual saved author v2 and Registry v3 signatures, and every artifact
digest/size against the signed release. It prints production Security URLs and
saves public signature/scan evidence under `evidence/`.

If an upload URL expires, run `band11_fixture renew-upload <case>`, then repeat
that case's `upload` and `finalize`. Renewal only refreshes upload instructions;
it does not change the saved artifacts or signatures. If finalize gets a worker
timeout, retain the same files/reservation and report it; do not rebuild or
invent another release to hide a failed test.

Wait for normal GuardDuty polling, then check all targets are complete and clean:

```bash
band11_fixture security portable --clean
band11_fixture security matrix --clean
```

If scans are pending, wait and retry those two commands. Do not edit DB rows,
S3 tags or scan statuses. Inspect the printed Security pages in the deployed web:
whole-release integrity, author/Registry signing and the correct one/two-artifact
table should appear. All target scan rows must be clean, not just one.

## 4. Consume With the Released CLI

First install the portable-only release into an empty consumer:

```bash
band11_fixture consumer portable --consumer "$BAND11_SMOKE_ROOT/consumer-portable"
cd "$BAND11_SMOKE_ROOT/consumer-portable"
band11 lint --strict --schema "$BAND11_SCHEMA"
band11 install --require-attestation --require-signature
band11_fixture check portable --consumer "$PWD"
export BAND11_PORTABLE_REF="$(band11_python -c 'import json,os;from pathlib import Path;s=json.loads((Path(os.environ["BAND11_SMOKE_ROOT"])/"session.json").read_text());print("@"+s["namespace"]+"/"+s["name"]+"@0.1.0")')"
band11 run "$BAND11_PORTABLE_REF" --machine --input '{"quantity":2,"unit_price":4.5}'
```

Expected: strict CLI verification passes, `any` selected, one payload cache, native
Pydantic Core import from the managed venv, and machine output `status: success`
with `output.total: 9`. No user pip install or venv activation is needed.

Then install the same package's exact-plus-any version separately:

```bash
band11_fixture consumer matrix --consumer "$BAND11_SMOKE_ROOT/consumer-exact"
cd "$BAND11_SMOKE_ROOT/consumer-exact"
band11 lint --strict --schema "$BAND11_SCHEMA"
band11 install --machine --require-attestation --require-signature > "$BAND11_SMOKE_ROOT/evidence/exact-install.json"
cat "$BAND11_SMOKE_ROOT/evidence/exact-install.json"
band11_fixture check matrix --consumer "$PWD"
export BAND11_EXACT_REF="${BAND11_PORTABLE_REF%@*}@0.1.1"
band11 run "$BAND11_EXACT_REF" --machine --input '{"quantity":2,"unit_price":4.5}'
cp agent.lock "$BAND11_SMOKE_ROOT/evidence/exact.lock"
band11 install --frozen --require-attestation --require-signature
cmp "$BAND11_SMOKE_ROOT/evidence/exact.lock" agent.lock
```

Expected: exact host target wins over `any`; only its one payload is cached;
strict crypto, native execution and frozen install pass. The portable lock's
integrity is the **whole-release digest**, while local selection/cache state
contains the chosen target and payload digest. `cmp` is silent.

Each consumer's `m15-evidence.json` records the actual release, target, selected
digest, cache and native runtime path. The filename reflects reuse of the already
tested M15 local audit; it is not a file shipped in either production Tool.

## 5. Finish and Record Acceptance

Once both strict installs, runs, scans and UI checks pass, revoke **only** the
disposable signer belonging to these test releases:

```bash
cd "$BAND11_REPO"
export BAND11_SMOKE_SIGNER_ID="$(band11_fixture signer-id)"
printf 'Disposable smoke signer: %s\n' "$BAND11_SMOKE_SIGNER_ID"
band11 namespace revoke-signer --namespace "$BAND11_NAMESPACE" --signer-id "$BAND11_SMOKE_SIGNER_ID"
printf 'Keep private evidence until acceptance: %s\n' "$BAND11_SMOKE_ROOT"
unset BAND11_TOKEN
```

Do not revoke your normal author keys or the Registry/root keys. Accepted
historical signatures remain valid after ordinary signer revocation. This guide
does not delete or yank production versions or remove immutable S3 evidence.

Report the smoke outcome and preserve the package identity/versions, receipts,
Security evidence, native execution and local selection/lock evidence. Passing
closes the small signed-production-release acceptance gate; the documented
maximum-size/concurrency capacity exception remains explicitly deferred.
