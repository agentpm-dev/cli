# Release Band 9: Storage and Deployment Readiness

M13A/M13B implementation, review, native CI, manual testing and CLI-main contract
parity have passed. Production publication/deployment and storage readiness are
tracked separately in `tasks.md`. Keep `RELEASE_PUBLISH_ENABLED=false` in Render.

Operator acceptance is now complete for this band, including production
deployment, migration/legacy-flow smoke, the storage probe and cost review.
The commands below remain the reproducible evidence procedure. Hosted finalize
capacity remains an unchecked Band 11 check before enabling the new flow;
the tiny object probe does not establish large-release/concurrent-load capacity.

## 1. Load Production Credentials in a New Terminal

Start in a new **macOS/zsh terminal**. This does not depend on the old manual-test
terminal or its exported variables. Use the production AWS access key/secret
from your existing production configuration. The prompts are hidden; credential
values are not typed into shell commands, saved by this guide or included in
the inspection report.

For the access-key/secret pair without a session token:

```bash
cd /Users/zackhine/projects/agentpm-project/agentpm
read -s 'AWS_ACCESS_KEY_ID?Production AWS access key ID: '
printf '\n'
read -s 'AWS_SECRET_ACCESS_KEY?Production AWS secret access key: '
printf '\n'
export AWS_ACCESS_KEY_ID AWS_SECRET_ACCESS_KEY
unset AWS_SESSION_TOKEN
```

If you use **temporary** credentials instead, also load their session token:

```bash
read -s 'AWS_SESSION_TOKEN?Production AWS session token: '
printf '\n'
export AWS_SESSION_TOKEN
```

Credentials exported here belong to this terminal's processes. They do not
change the running backend, Render or another terminal. Do not paste credential
values into chat. No additional application key or PAT is needed for inspection.

## 2. Collect Read-Only Production Configuration

The helper uses the API's existing boto3 installation. It reads caller identity,
bucket location, lifecycle rules, bucket policy, versioning, encryption, public
access settings and notifications, then lists/details matching GuardDuty Malware
Protection plans in the bucket's region. It makes **no object upload, delete,
tag update, bucket/policy edit, database change or application request**.

```bash
export BAND9_STORAGE_REPORT="${TMPDIR:-/tmp}/agentpm-band9-prod-$(date +%Y%m%d-%H%M%S)-$$.json"
AWS_EC2_METADATA_DISABLED=true ../agentpm-api/.venv/bin/python -B \
  specs/2026-10-06-immediate-hardening/manual-tests/fixtures/m13b/inspect_storage.py \
  --bucket agentpm-tools \
  --region us-east-1 \
  --output "$BAND9_STORAGE_REPORT"
```

The report contains configuration and ARNs, **not credentials**. The local file
is private and refuses to overwrite earlier evidence. Share the output for
review. `AccessDenied`/similar results are marked **unverified**, not passed or
proof the running Registry cannot publish: object access and configuration-read
permissions are different. An absent lifecycle configuration/policy is likewise
reported for explicit review. If these credentials cannot read settings, inspect
those settings through your authorized AWS console account instead.

Review these items together:

- The bucket/region and caller ARN are the intended production resources and
  identity. A caller ARN alone does not establish that Render uses that principal.
- Enabled lifecycle rules preserve current final objects under
  `tools/<namespace>/<package>/<version>/`. Check overlapping/all-bucket rules,
  not just the named `tools/` rule. Noncurrent-version expiry differs from current
  object expiration. Staging `uploads/` has a deliberately temporary lifetime.
- The bucket policy does not block the required final-object conditional PUT/
  GET or reservation-owned staging cleanup; reading it alone does not prove
  effective IAM, permission-boundary, account-policy or KMS access.
- The matching GuardDuty plan is `ACTIVE`, covers `tools/` (or the entire bucket),
  and has result tagging enabled. Inspect its reported role and status reasons.
  Configuration alone does not prove delivery of new scans/tags.

References: [GuardDuty protection-plan fields](https://docs.aws.amazon.com/guardduty/latest/APIReference/API_GetMalwareProtectionPlan.html)
and [S3 lifecycle filters/actions](https://docs.aws.amazon.com/AmazonS3/latest/userguide/intro-lifecycle-rules.html).

## 3. Checks After Deployment

Configuration inspection can run before deployment. The remaining live checks
use the shipped Registry revision and its actual principal after CLI **0.1.38**
and Registry deployment; Web is not required for storage checks. Record migration
startup through V1.104, the disabled release gate, legacy publish/install smoke
results, the deployed status card after Web deployment, and environment-specific
permissions/retention evidence. Any later production probe must use agreed tiny
benign objects under the actual key layout and verify real GuardDuty tagging;
this inspection command does not create probes or enable the new-format API.

The operator accepted cost for the gated Band 9 rollout. Hosted finalize
disk/request/concurrency capacity remains required before Band 11 activation.
The configuration report alone does not establish live delivery or capacity;
the completed object probe supplies the separate production delivery evidence.

## 4. Tiny Live Production Object Probe

CLI, Registry and Web deployment are operator-confirmed complete. Keep
`RELEASE_PUBLISH_ENABLED=false`. This probe calls S3 directly using the reviewed
production principal; it creates **no Registry package/version or scan rows**.

The explicit `upload` command creates three benign objects in `agentpm-tools`:
one staging archive under `uploads/<probe-uuid>/`, plus a real target archive and
canonical `release.json` under the unique
`tools/zack/band9-storage-probe-<uuid>/0.0.0/` layout. The final archive and
document together are bounded to 16 KiB; the archive is also written once as
staging. The two protected final objects trigger normal GuardDuty scans.
They remain as small evidence objects; only this probe's staging key is deleted.
There is no malware sample, manual tag writing or production configuration change.

In the same terminal with production credentials loaded, prepare and lint locally:

```bash
cd /Users/zackhine/projects/agentpm-project/agentpm
export BAND9_REPO="$PWD"
export BAND9_PROBE_ROOT="$(mktemp -d "${TMPDIR:-/tmp}/agentpm-band9-probe.XXXXXX")"
band9_probe() {
  AWS_EC2_METADATA_DISABLED=true "$BAND9_REPO/../agentpm-api/.venv/bin/python" -B \
    "$BAND9_REPO/specs/2026-10-06-immediate-hardening/manual-tests/fixtures/m13b/probe_storage.py" \
    "$1" "$BAND9_PROBE_ROOT"
}
band9_probe prepare
agentpm --version
cd "$BAND9_PROBE_ROOT/source"
agentpm lint --schema "$BAND9_REPO/schemas/agentpm.manifest.schema.json"
cd "$BAND9_REPO"
cat "$BAND9_PROBE_ROOT/release.json"
```

Expected: installed CLI reports **0.1.38**, lint succeeds, and preparation prints
the exact three keys and tiny byte counts. No AWS request has run during prepare.
The helper checks its expected caller is
`arn:aws:iam::510092505185:user/agentpm-reader-writer` before live operations;
different credentials fail before object mutation.

Run the small production writes explicitly:

```bash
band9_probe upload
```

Expected: conditional uploads succeed, SHA-256 PUT responses and final GET/HEAD
readback match, and encryption reports `AES256` (SSE-S3). Repeating the archive's
create-only PUT must produce 412. If a prior attempt stopped partway through,
the same prepared probe can recover by verifying its exact existing bytes;
it never replaces a different object. Successful upload is recorded locally.

Read real scan-result tags:

```bash
band9_probe check
```

Expected eventually: **both** archive and document show `NO_THREATS_FOUND` and
the helper prints its clean-tag PASS. `PENDING`/`SCAN_IN_PROGRESS` is not a pass;
wait a few minutes and rerun `band9_probe check` in this same terminal. Unexpected
scan outcomes or tag-read errors stop the check. Never set tags yourself.

After both tags are clean, verify staging deletion and final retention:

```bash
band9_probe cleanup
cat "$BAND9_PROBE_ROOT/state.json"
```

Expected: exactly the UUID-derived staging key is deleted and now returns 404;
the two final objects still match their bytes/checksum/encryption. Local state
reads `phase: complete` with both clean tag results. This verifies production
object permissions and actual scan/tag delivery for the reviewed principal,
not Registry scan-row/poller orchestration or a new-format publication. Deployment
smoke checks and capacity/cost sizing remain separately tracked.

Keep `BAND9_PROBE_ROOT` until the result is recorded. If the terminal closes,
reload credentials using section 1, restore that root path, and redefine the
function; use `check`/`cleanup` rather than preparing a second probe.

After all inspection and probe work in this terminal is finished:

```bash
unset AWS_ACCESS_KEY_ID AWS_SECRET_ACCESS_KEY AWS_SESSION_TOKEN
```
