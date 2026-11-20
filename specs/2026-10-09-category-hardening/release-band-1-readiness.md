# Release Band 1: release preparation

Prepared **2026-11-18**. This completes the builder's release-preparation step;
the user confirmed on 2026-11-18 that all three preparation PRs passed CI and CLI main/post-merge CI passed. Publication and production inspection/backfill subsequently completed as recorded below.
The measured capacity envelope and activation are approved. The operator confirmed
corrected hosted CI passed and the Registry was redeployed with enforcement enabled.
Production smoke checks remain open in
[tasks.md](tasks.md#ordered-production-rollout).

## Prepared versions and files

| Surface                             | Prepared state                                                      | Files / deployment condition                                                                                                                                                                                  |
| ----------------------------------- | ------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| CLI                                 | **0.1.45**, previously 0.1.44                                       | `crates/agentpm-cli/Cargo.toml`, regenerated `Cargo.lock`; new public `CHANGELOG.md` with migration notes                                                                                                     |
| Web CLI status                      | **v0.1.45**, planned date **2026-11-18**                            | `agentpm-web/src/app/status/page.tsx`; deploy after the CLI is published/installable, preferably alongside the matching Registry docs. Correct the date if the release occurs on another local calendar date. |
| Registry docs                       | Explain 0.1.45 authoring checks versus 0.1.36 declaration tolerance | `agentpm-api/docs/v0.1/cli/publish.mdx`; deploy with Registry enforcement off initially                                                                                                                       |
| Node/Python SDKs and Rust SDK crate | **0.1.17**, unchanged                                               | No SDK release or new runtime minimum is introduced here                                                                                                                                                      |
| APDS                                | **agentpm / 1.0.0**, unchanged frozen bytes                         | Publish the separate immutable APDS tag and verify the raw schema before publishing CLI 0.1.45                                                                                                                |

The CLI has no existing root changelog; the new entry documents this release
without inventing historical entries. It is marked **Unreleased**. Mark the
entry released with the actual date when publishing the CLI. Its migration notes
cover the explicit selector, nonblank descriptions, rebuilding authored content,
read-only legacy lint and preservation of historical installed releases. Public
copy contains no internal milestone/stage language.

The lockfile diff changes only the CLI package's own version; no dependency
version was upgraded. The website change updates only its existing CLI status
marker. APDS package-page presentation remains later work.

## Version/tag availability and frozen source

A read-only GitHub API check on 2026-11-18 confirmed:

- [Latest published CLI](https://github.com/agentpm-dev/cli/releases/tag/v0.1.44): `v0.1.44`, published `2026-11-11T02:24:25Z`.
- `refs/tags/v0.1.45` did not exist (HTTP 404); 0.1.45 is available for this release.
- `refs/tags/apds-v1.0.0` did not exist (HTTP 404); this was the pre-publication state; the subsequent user-confirmed publication is recorded below.

The reviewed frozen bundle source is
`e308bfefb880f2a3a96cc08f4f87d61d3c55a76a`. The current canonical directory is
byte-identical to that commit, and the Registry snapshot still pins it with
inventory SHA-256
`b8580681aec3b113cf8a65061cee9907b07f07aa71c9361822aa8c874310fed6`.
That reviewed source is the candidate target for the APDS tag; a later tag target
must contain exactly the same reviewed bundle. No tag was created in preparation.

Before CLI publication, verify this exact editor URL resolves to the frozen
schema after the operator publishes the APDS tag:

```text
https://raw.githubusercontent.com/agentpm-dev/cli/refs/tags/apds-v1.0.0/standards/agentpm/1.0.0/manifest.schema.json
```

## Reconciled band prerequisites

| Prerequisite                                                        | Evidence                                                                                                                                                                                                                                                                                                               |
| ------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Tolerance shipped before declaration emission                       | [M1A.1 record](m1a1-verification.md): CLI 0.1.36 publication/installation operator-confirmed 2026-10-22; closed-root/schema and preservation coverage reviewed. Tolerance commit `e0143afc0dd92dbe1c05d3fe27bb19c8325ed02f` remains an ancestor of the prepared source.                                                |
| Required earlier schema additions precede freeze                    | [M1B prerequisite record](m1b-verification.md#freeze-prerequisite-evidence): commits `8c68bef66bf0f3bfd6dfceb5f6f5d11c6c80543a` and `2f6908ea676fcbe6ffd92c5d4a3f5790b11189af` remain ancestors; their fields are already in the frozen bundle.                                                                        |
| Canonical definition, resolved discrepancies and independent parity | [M2C frozen-candidate/parity record](m2c-verification.md) and [durable report](m2c-parity-report.json): no open tested mismatch. Fresh 0.1.45 Rust and independent Python executions agree on all 129 corpus cases; schema/bundle checksums and Registry release/signing snapshots agree.                              |
| Newcomer/readability and contract review                            | [M1B review](m1b-verification.md) and [M1C review](m1c-verification.md) record completed independent README/semantics review, with later corpus reconciliation frozen in M2C. No normative bytes changed afterward.                                                                                                    |
| New authored kinds                                                  | Fresh 0.1.45 default init produces an Agent Package; each of the eight scaffold kinds declares the supported standard and passes ordinary lint. Saved builder results: ignored `target/band1/scaffold-results.json`.                                                                                                   |
| Publication bypass/migration and rollout design                     | [M4A real-local evidence](m4a-verification.md) plus [M4B tests/review](m4b-verification.md): custom-client negatives, in-flight/completed sessions, immutable historical release/successor, private graph/signing/integrity gates and backfill mutation regressions. Production smoke evidence remains a release gate. |

## Local verification for this preparation

All commands below passed, exit 0, from their corresponding repository roots:

- CLI: `cargo check --offline -p agentpm-cli` regenerated only its package version in `Cargo.lock`.
- CLI: `cargo fmt --all -- --check`; `cargo clippy --offline --locked --workspace --all-targets -- -D warnings`.
- CLI: `cargo build --offline --locked -p agentpm-cli`; `AGENTPM_TELEMETRY=0 target/debug/agentpm --version` prints **agentpm 0.1.45**.
- CLI: `AGENTPM_APDS_RESULTS=target/band1/apds-rust-results.json cargo test --offline --locked -p agentpm-cli --test apds_validation shared_corpus_matches_actual_rust_validation -- --nocapture` — **129 actual Rust cases**, one integration test passed.
- Registry: `.venv/bin/python -B scripts/check_apds.py --out ../agentpm/target/band1/apds-python-results.json --rust-results ../agentpm/target/band1/apds-rust-results.json` — **129 Python cases, actual Rust/Python results agree case by case**. Synthetic test Registry key and disabled AWS metadata lookup were supplied as in M4B.
- CLI: `../agentpm-api/.venv/bin/python -B scripts/check_apds_schema.py`; `../agentpm-api/.venv/bin/python -B scripts/check_release_contract.py --registry-root ../agentpm-api`; `python3 scripts/check_registry_bootstrap.py --release` — frozen schema/bundle, cross-repo release/signing snapshots and official-root release policy all pass.
- CLI: read-only ancestry checks for the tolerance, required earlier schema and frozen-source commits; `git diff --exit-code e308bfefb880f2a3a96cc08f4f87d61d3c55a76a HEAD -- standards/agentpm/1.0.0` — no bundle drift.
- CLI: eight real `init` → ordinary `lint --format json` probes with the rebuilt binary, including bare/default Agent init — all conformant, no Runner readiness claim.
- Registry: `.venv/bin/python -B -m pytest -q tests/test_docs_content.py` — **5 passed, 166 subtests passed**; updated publish MDX also compiles through the existing web MDX compiler.
- Web: `pnpm typecheck` and `pnpm lint` — success; lint retains six existing unused-variable warnings outside the changed status page, with zero errors.
- Formatting: existing Prettier configuration for the status page, MDX and new Markdown; Markdown/Python-hook whitespace/EOF/conflict equivalents where applicable; `git diff --check` on each changed repository.

Logs are in `/tmp/agentpm-band1-{cargo-check,clippy,build,rust-apds,python-apds,parity,docs,web-typecheck,web-lint}.log`.
Fresh corpus arrays and scaffold results are in ignored `agentpm/target/band1/`.
No new tests were added for the version/status literals. Full runtime regression
suites were not repeated solely for these metadata changes; their prior reviewed
results remain in the milestone records. The user subsequently confirmed all
three preparation PRs and CLI main/post-merge CI passed on 2026-11-18.

## APDS publication handoff

On 2026-11-18, a read-only GitHub API inspection found public main at
`914cb00f0e1b0a1c3a31997cb66ee192fbc91e68`. Its complete APDS directory entries,
including the recursive conformance tree identity, match the reviewed frozen
source exactly. The APDS tag still returned 404. Local schema/bundle checks pass.
The following publication commands were supplied to the operator for the frozen source; the user subsequently confirmed the public schema verification below:

```sh
cd /Users/zackhine/projects/agentpm-project/agentpm
git fetch origin main
git tag -a apds-v1.0.0 e308bfefb880f2a3a96cc08f4f87d61d3c55a76a \
  -m "AgentPM APDS 1.0.0"
git push origin refs/tags/apds-v1.0.0
```

Then verify the actual public schema:

```sh
curl --fail --show-error --location \
  https://raw.githubusercontent.com/agentpm-dev/cli/refs/tags/apds-v1.0.0/standards/agentpm/1.0.0/manifest.schema.json \
  --output /tmp/agentpm-apds-1.0.0.schema.json
shasum -a 256 /tmp/agentpm-apds-1.0.0.schema.json
```

Expected SHA-256:
`f123150ff42cea4b29edb5076d7d70697269d20ad85118b4a07fe5af5cf2ab04`.
On 2026-11-18 the user supplied the successful public `curl` result (64,889 bytes)
and `shasum -a 256` output, exactly matching that expected hash. **APDS 1.0.0
publication and public-schema availability are confirmed.** Treat the standard
files and tag as immutable from this point. This tag does not match the CLI
binary release workflow's `v*` trigger; The user subsequently confirmed CLI 0.1.45 released and installed, Web and
Registry deployed with enforcement off, and additional CI passed on 2026-11-18.
Production history inspection is the next step.

## Original preparation handoff

1. Push the CLI version/lock/changelog and preparation records, Registry MDX, and
   Web status changes to their PRs. Wait for fresh hosted checks for these changes;
   the earlier M4B PR runs covered the pre-bump source.
2. Continue the [ordered rollout](tasks.md#ordered-production-rollout): CLI merge
   and push-only CI, immutable APDS tag/URL verification, then CLI publication.
   Keep the Web status change queued until 0.1.45 is actually available.
3. Deploy the Registry with `APDS_PUBLISH_ENFORCED=false`, then execute the
   remaining production inventory/backfill, isolated capacity, approval and
   coordinated activation/smoke gates. Neither release capacity nor production
   activation is certified by these preparation checks.

## Deployment handoff to production inspection

The user confirmed on 2026-11-18: CLI 0.1.45 released and installation verified,
Web and Registry released, additional CI successful, and Registry
`APDS_PUBLISH_ENFORCED=false`. Release Band 1 steps 1–3 are complete by operator
confirmation. No production audit/backfill, capacity approval or enforcement
activation is claimed yet.

Open the **production Registry's Render shell** and work from the directory
containing `run.py`. Use a fresh private report directory so the command's
exclusive-output guard cannot overwrite previous evidence:

```sh
cd /opt/render/project/src
umask 077
export APDS_AUDIT_DIR="$(mktemp -d /tmp/agentpm-apds-audit.XXXXXX)"
printf 'Audit reports: %s\n' "$APDS_AUDIT_DIR"

python -m flask --app run registry audit-apds-history \
  --out "$APDS_AUDIT_DIR/history-before.json"

python -m flask --app run registry audit-apds-history \
  --inspect-artifacts --out "$APDS_AUDIT_DIR/history-inspected.json"
```

These commands perform inventory/inspection only. They do not apply metadata
backfill or rewrite artifacts/signatures. The first reads database references;
the second also downloads and hashes every selected target archive, observes
its embedded manifest and checks consistency against stored identity/metadata.
Both should report **0 backfilled**. Normal application-startup logs may precede
the audit output. A failed inspection stops for review; retain the original
inventory and the error rather than treating a partial output as successful.

After both commands succeed, print a compact summary:

```sh
python - <<'PYCODE'
import json
import os
from collections import Counter
from pathlib import Path

folder = Path(os.environ["APDS_AUDIT_DIR"])
report = json.loads((folder / "history-inspected.json").read_text())
rows = report["versions"]
needed = [row for row in rows if row["needs_backfill"]]
print("Enforcement enabled:", report["enforcement"])
print("Versions inspected:", len(rows))
print("Pending sessions:", len(report["pending_sessions"]))
print("Versions needing metadata backfill:", len(needed))
print("Proposed origins:", dict(Counter(row["proposed_metadata"]["standard_origin"] for row in needed)))
print("Actually backfilled:", report["backfilled_versions"])
print("Reports:", folder)
PYCODE
```

Expected enforcement is **false**, actual backfills **0**. The inspection can
legitimately find missing metadata; that is a proposal for the next reviewed
step, not an automatic update. Review the summary/errors before supplying any
`--apply` command. Reports can contain private package identities/storage keys;
retain them privately outside the service before any restart/redeploy discards
ephemeral `/tmp` files. Keep the same shell/exported report path for follow-up.

## Production inspection result and metadata backfill handoff

On **2026-11-19** the user supplied this production inspection summary:

- Enforcement enabled: **false**.
- Versions inspected: **138**; pending sessions: **0**.
- Versions needing metadata backfill: **138**.
- Proposed origins: **138 `legacy_inferred`**; actually backfilled: **0**.
- Private service-side report directory: `/tmp/agentpm-apds-audit.NugfKM`.

This completes Release Band 1 step 4 on operator-supplied evidence. The successful
inspection observed absent APDS declarations in the actual archives. "Legacy"
here describes the APDS declaration, independently of the release/artifact or
signature format. The proposed effective `agentpm/1.0.0` interpretation is
inferred, not certified conformance. The user subsequently confirmed the metadata apply completed on 2026-11-19:
`APDS history: 138 versions; 138 backfilled`, with output in
`/tmp/agentpm-apds-audit.NugfKM/history-applied.json`. The user subsequently supplied the successful fresh read-only inspection and
immutable-reference comparison on 2026-11-19, recorded below; step 5 is complete.

From the **same production Registry Render shell**, apply the metadata backfill
and then run a fresh read-only inspection. Both output names must be unused;
retain prior reports. If either command fails, stop and review the exact error.

```sh
export APDS_AUDIT_DIR=/tmp/agentpm-apds-audit.NugfKM

python -m flask --app run registry audit-apds-history \
  --inspect-artifacts --apply --out "$APDS_AUDIT_DIR/history-applied.json"

python -m flask --app run registry audit-apds-history \
  --inspect-artifacts --out "$APDS_AUDIT_DIR/history-after.json"
```

The first command re-inspects actual archives and atomically fills only missing
APDS metadata after a fresh locked fingerprint comparison. It does not import
or trust a caller-edited report. The expected applied count is 138 for the
inspected history; report any different count for review. The second command
must apply zero changes and should find zero remaining metadata backfills.
Keep enforcement false until capacity measurement and the separate activation
approval are complete.

After both commands succeed, compare immutable references and print results:

```sh
python - <<'PYCODE'
import json
import os
from collections import Counter
from pathlib import Path

folder = Path(os.environ["APDS_AUDIT_DIR"])
def read(name):
    return json.loads((folder / name).read_text())

before = {r["version_id"]: r for r in read("history-before.json")["versions"]}
applied = read("history-applied.json")
report = read("history-after.json")
after = {r["version_id"]: r for r in report["versions"]}
fields = (
    "immutable_columns_sha256", "artifact_rows_sha256",
    "author_rows_sha256", "attestation_rows_sha256",
)
missing = sorted(before.keys() - after.keys())
changed = sorted(i for i in before.keys() & after.keys()
                 if any(before[i][key] != after[i][key] for key in fields))
needed = sum(r["needs_backfill"] for r in after.values())
print("Enforcement enabled:", report["enforcement"])
print("Versions backfilled:", applied["backfilled_versions"])
print("Still needing backfill:", needed)
print("Stored origins:", dict(Counter(r["metadata"]["standard_origin"] for r in after.values())))
print("Original versions missing:", len(missing))
print("Immutable reference changes:", len(changed))
print("New versions since baseline:", len(after.keys() - before.keys()))
assert report["enforcement"] is False, "Enforcement unexpectedly enabled"
assert not missing and not changed, "Historical references differ; review reports"
assert needed == 0, "Some versions still need metadata backfill"
expected = {"standard_id": "agentpm", "standard_version": "1.0.0",
            "standard_origin": "legacy_inferred", "apds_validation": None}
assert all(after[i]["metadata"] == expected for i in before), "Unexpected historical APDS metadata"
print("PASS: original history preserved; inferred metadata populated without certification.")
PYCODE
```

Expected original-version and immutable-reference differences are zero, with
138 historical rows now `legacy_inferred` and no APDS validation evidence. New
versions appearing since the initial baseline are reported separately rather
than silently treated as part of the original history. Share the compact
results for review, retain the private reports outside the ephemeral service,
and close step 5 only after successful apply/post-verification is confirmed.
That operator confirmation was supplied on 2026-11-19, as recorded below.

## Production history verification complete — 2026-11-19

The user supplied the successful post-backfill verification output:

```text
Enforcement enabled: False
Still needing backfill: 0
Stored origins: {'legacy_inferred': 138}
Original versions missing: 0
Immutable reference changes: 0
PASS: history preserved; inferred metadata populated without certification.
```

Combined with the confirmed 138 applied backfills, this completes Release Band 1
steps 4–5 on operator-provided evidence. The verifier also asserted each original
row's effective `agentpm/1.0.0` identity, `legacy_inferred` origin and absent
validation evidence. Its comparisons cover immutable version columns, artifact
rows, author-signature rows and Registry-attestation rows. This preserves
historical references; it does not retroactively certify APDS conformance.

Private before/inspection/applied/after reports remain in
`/tmp/agentpm-apds-audit.NugfKM` on the service. Retain them privately outside the
ephemeral service before a redeploy/restart can discard them. The user subsequently transferred the four full reports by checksum-framed
copy/paste; they were safely unpacked and independently rechecked locally, as
recorded below. No direct production connection was used by the builder.

The user reconfirmed the unchanged production baseline for the next capacity
step: **0.5 CPU / 512 MB RAM**, start command **`gunicorn run:app`**. Capture
effective workers, timeout/proxy limits and disk headroom for the measured
production-equivalent setup. Capacity measurements, Claude's separate activation
approval, coordinated enforcement activation and production smoke tests remain
pending. Enforcement stays false.

## Retaining the four production reports locally

The user prefers copy/paste while the Render shell is still open. A private
local destination is prepared at
`agentpm/release-band1-evidence/production-audit/paste-backup.txt`; its entire
parent evidence directory is gitignored. The user saved the copy/paste backup on 2026-11-19; its checksum and all four
report contents have now been verified and unpacked locally, as recorded below.

Run this export in the **existing production Registry Render shell**. It reads
exactly the four report files, preserves their original JSON bytes in an in-memory
archive, compresses them to reduce terminal output, and prints a copy/paste
block with a transport checksum. It does not change the database or S3.

```sh
python - <<'PYCODE'
import base64
import hashlib
import io
import lzma
import tarfile
import textwrap
from pathlib import Path

folder = Path("/tmp/agentpm-apds-audit.NugfKM")
names = ("history-before.json", "history-inspected.json",
         "history-applied.json", "history-after.json")
buffer = io.BytesIO()
with tarfile.open(fileobj=buffer, mode="w") as archive:
    for name in names:
        data = (folder / name).read_bytes()
        member = tarfile.TarInfo(name)
        member.size = len(data)
        member.mode = 0o600
        archive.addfile(member, io.BytesIO(data))
payload = lzma.compress(buffer.getvalue(), preset=3)
print("APDS_BACKUP_SHA256:", hashlib.sha256(payload).hexdigest())
print("APDS_BACKUP_BEGIN")
print("\n".join(textwrap.wrap(base64.b64encode(payload).decode(), 76)))
print("APDS_BACKUP_END")
PYCODE
```

Copy the entire output from `APDS_BACKUP_SHA256` through `APDS_BACKUP_END` into
the local input file and save it. Tell the builder once saved; it can then check
the checksum and unpack only the four expected regular JSON members into the
private local directory. Never execute copied archive content or publish the
reports. The compressed block is transport encoding, not encryption. If terminal
output is truncated, checksum/decoding must fail and export should be repeated
in chunks before considering the reports retained.

The archive transport was round-trip checked locally using a representative
113-version audit report, without printing report contents: four archived report
copies compressed to 29,076 bytes / 511 encoded lines. Actual production export
size is not yet observed; no successful backup is claimed from that local check.

## Capacity execution approach — operator adjustment, 2026-11-19

The operator approved a local Docker harness instead of provisioning another
Render environment. See the [single-command runbook](manual-tests/REGISTRY_CAPACITY.md).
The actual Registry starts through Gunicorn with Pyway migrations and real
HTTP authentication/publication/signatures, constrained to 0.5 CPU / 512 MiB RAM
with swap disabled. Disposable PostgreSQL, Redis and Moto S3 services are on an
internal network with no host ports; no production configuration or credentials
are copied. A test-only Gunicorn hook disables rate pacing for load generation
and measures actual storage response streams, APDS extraction, cgroup memory/CPU,
process RSS and named/unnamed temporary files at 200 ms intervals.

The workloads include mirrored dependency/binding graphs from the largest
locally complete Agent and Template examples, a larger synthetic fan-out/diamond
graph, large compressed/expanded payloads, repeated publication, both protocols,
two concurrent clients, APDS rejection and an injected storage timeout. All
new source manifests are CLI-linted; actual Knowledge/Memory content is rebuilt.
Existing published packages and examples source files are unchanged.

The quick profile passed 68 actual finalize attempts (66 successful publications,
one exact APDS rejection and one controlled timeout/worker replacement), with
no sampler errors and no sequential temporary-file residue. It is tooling
verification, not capacity acceptance. Full evidence is ignored under
`release-band1-evidence/capacity/20261120T020346Z-67020d/`.

The local architecture/CPU, disk, emulator/network latency and absence of the
Render proxy remain explicit limitations. Confirm effective production worker
and timeout settings and review those differences with the measurements. The
script cannot grant activation approval or certify every supported upload size.
Claude's separate activation review and the remaining controlled production
rollout are still required. Production enforcement remains false.

## Production report backup retained and independently checked — 2026-11-19

The user saved the exported backup in the prepared private local file. The
builder verified the compressed archive SHA-256 against the export header:

```text
51ea92d1aa795cada2c4f094aded9f8cb5c1d756b02b50a61c02622ca918e420
```

The decoded archive was bounded and inspected before writing. Only the four
expected regular report members were accepted; arbitrary paths, duplicate
members, links and conflicting existing files were not allowed. Original JSON
bytes were retained, with per-file SHA-256 values in the private
`backup-checksums.json`. All four full reports contain **138 versions**.

The builder then independently compared the actual copied reports: **138
backfills applied**, **0** remaining backfills, **0** missing original versions,
**0** changes in immutable version/artifact/author-signature/Registry-attestation
references, all original metadata `agentpm/1.0.0` / `legacy_inferred` with no
validation evidence, enforcement **false**. This verifies the earlier compact
operator output against the full supplied files, not against a fresh production
query. The private `verification-summary.json` records these results.

Retained local directory:
`agentpm/release-band1-evidence/production-audit/`. It contains the four JSON
reports, the encoded copy/paste input, `audit-backup.tar.xz`, checksum inventory
and verification summary. Files have mode 0600 and the directory is private and
gitignored. Local retention is complete; an additional private off-device backup
can be made using the verified archive. Capacity testing is still unperformed
and its runnable workload/measurement harness still needs preparation.

## Local capacity measurement completed — 2026-11-19

The representative single-command run completed in approximately **197 seconds**
using the actual Registry application at **0.5 CPU / 512 MiB**, no swap,
Gunicorn **23.0.0**, **one sync worker**, **one thread**, **30-second timeout**,
on Linux **aarch64**. The script independently checked Docker's applied resource
limits, expected HTTP outcomes and actual database publication/evidence state.

Evidence is retained privately at
`release-band1-evidence/capacity/20261120T021448Z-51e075/`.
The final report was re-analyzed from unchanged raw events/requests with the
latest guard code; `analysis-source.json` records the reporter hash. The source
snapshot/checksums distinguish the measured container from later reporter-only
and test/style changes. Earlier directories are builder debugging/pilot runs;
the directory above is the release-review result.

- **100 real finalize attempts:** 98 successful signed publications, one exact
  APDS rejection and one intentionally induced 500 from the Gunicorn watchdog.
- Both single-archive and two-target Python publication paths passed off/on.
- Mirrored largest complete local graphs were `ops-console` (11 unique nodes)
  and `support-assistant-workspace` (17). The inventory records all selections
  and explicit test-artifact adaptations.
- The synthetic Agent inspected 23 unique dependencies with 25 total GETs when
  enforced; its repeated versions took **0.36–0.52 s**. The Template added the
  Agent dependency for 26 GETs, taking **0.38 s**. Root reads were two in both
  enforcement settings; off-mode dependency reads were zero for these graphs.
- With enforcement on, the mirrored real Agent/Template roots took approximately
  **0.24 / 0.36 s**. Two concurrent clients completed in **0.87 / 1.48 s**,
  including queueing behind the single sync worker.
- The incompressible payload was **32 MiB**; each multi-target archive contained
  that payload (about 64 MiB total compressed release). The highly compressible
  case expanded to **128 MiB**, taking **4.69 s** with enforcement and **7.04 s**
  without. These individual observations are not statistical evidence that
  enforcement improves latency; ordering, cache state and local variance differ.
- Aggregate cgroup memory high-water mark was **306.04 MiB** (including startup,
  cache and measurement overhead), about **206 MiB below the test ceiling**.
  No cgroup OOM kills or unplanned worker replacements occurred.
- **Zero sampler errors and zero sequential temporary-file residue.** All
  recorded requests had cleanup observations; both concurrent groups returned
  to an idle baseline. The controlled timeout replaced exactly one worker and
  created no published version. Docker teardown completed successfully.
- Ten evidence/safety regression tests pass, including missing workload,
  incorrect HTTP outcome, OOM, unplanned restart, missing sampler, residual temp
  files, legacy shortened final-key classification and non-approval checks.
  Black, isort, Flake8, Pylint (10/10), Bandit and harness-scoped mypy pass.

**Interpretation:** the sampled workloads fit the locally measured envelope.
This is not approval of all 1 GB uploads/256-node graphs, a load percentile/SLA,
or a reproduction of Render CPU speed, architecture, disk, real S3/network or
proxy behavior. Rate pacing was disabled only inside the test hook. Confirm
production worker/timeout/disk configuration with the read-only runbook probe;
review these residual differences and any required limits/mitigation with Claude.
**Claude subsequently approved this measured envelope (step 6), with mandatory
orphan reclamation before activation. Step 7 activation approval has not been granted.** Production enforcement remains false. No production
publication/configuration changes occurred during these runs.

## Independent capacity review and interruption follow-up — 2026-11-19

The operator supplied Claude's independent review: all original counts, applied
limits, raw instrumentation and source provenance checked out. The production
history audit was independently verified across all 138 versions as well.
**Measured-envelope approval is granted (step 6); activation approval is not.**
The approved scope is the tested protocols/settings, approximately 26 graph nodes,
64 MiB compressed/128 MiB expanded and at most two concurrent publishers.
Supported maxima, sustained queueing and exact Render performance remain unproven.

Claude required orphan reclamation because the first timeout happened before
extraction. The follow-up implements a locked-directory ownership protocol for
root extraction, graph metadata and dependency extraction, plus the legacy
publication flow's named verified snapshot. Normal/error returns clean them up;
Gunicorn startup/fork/worker-reap hooks reclaim only dead managed directories.
Live locks, other UIDs, symlinks, non-private paths and older/untracked names are
skipped. Historical audit directories are outside the cleanup namespace.

The first stronger local probe confirmed the review gap and found that the named
snapshot also survived a hard kill. The snapshot was then enclosed in the same
managed lifetime; no broad deletion of arbitrary `/tmp/tmp*` files was added.

**Post-mitigation representative evidence:**
`release-band1-evidence/capacity/20261120T025207Z-7ef6ce/`.
The actual production Gunicorn hooks were delegated to by the measurement config.
The run completed **102 expected outcomes**: 98 successful publications, one APDS
rejection, one pre-extraction timeout/500, and two intentional disconnects from
SIGKILL during root and dependency extraction. The test-only hook ignores SIGABRT
for those two cases so the watchdog escalates; Python finally blocks cannot
recover their files. The real master logged reclamation of **2 and 4 managed
directories**, respectively. All three timeout rows went from **0 to 0 temporary
bytes**, with no OOM, unplanned replacement or sampler error. Gunicorn's generic
“Perhaps out of memory?” SIGKILL log hint is expected here; cgroup OOM counters
remain zero and the controlled-delay events identify both induced kills.

Peak cgroup memory was **333.90 MiB** of 512 MiB; this supersedes the original
306.04 MiB sample for review of the modified runtime. Slowest ordinary success
was **6.60 seconds** (off lane); enforced expanded-payload success was **4.87 s**.
The run took approximately **274 seconds**. These are individual local samples,
not a causal performance comparison or SLA. The normal-config diagnostic phase
then confirmed HTTP 200 and **two fresh Redis counters** from its loopback health
request. That phase uses normal rate limiting, independently of the load hook.
The local temporary filesystem was **overlay**. No production endpoint was used.

The new operational command is
`python scripts/inspect_publication_capacity.py` from the Render directory
containing `run.py`, **after the reviewed mitigation is deployed with enforcement
off**. It reports Gunicorn settings, temp filesystem/memory accounting and live
Redis use without application startup or publication. Its only request is a
loopback health check, which creates ordinary expiring limiter counters. Local
execution is not a substitute for the operator's production output.

The operator confirmed the production health URL is
`https://api.agentpackagemanager.com/health` and there is **no additional proxy**.
The runbook cites Render's documented response limit and health-check timing.
A single sync worker can still starve HTTP health checks during prolonged work:
that availability risk remains for activation review. Linux cgroup memory includes
filesystem cache, so disk-backed temp data must not be treated as wholly absent
from the peak or blindly added a second time; compare mount type and file/shmem
accounting in the production probe.

**Still required:** Registry PR/hosted CI for the cleanup change, deployment with enforcement off, production
probe/configuration confirmation and Claude's separate activation approval. The
ordered pause/drain/activation/smoke steps remain unchanged. No production
configuration, package, database or S3 changes were made by the builder.

Final follow-up verification: **1,083 passed, 7 unrelated opt-in skips, 347 subtests passed** in the full Registry suite (`/tmp/agentpm-band1-orphan-final.log`). The focused publication/rollout/cleanup/probe run passed **81 tests**. Black/isort/Flake8/Pylint (10/10), Bandit, mypy and pyupgrade pass for the touched API Python files; the ten harness evidence tests and static checks also pass.

`post-measurement-source.json` records subsequent documentation/lint-comment and operational-probe metadata differences from the saved source snapshot. The publication CLI change is AST-identical to its measured form; the ownership, sweeping and validation behavior was unchanged after the successful 102-case run. The extra probe inventory/preload fields are covered by the final tests. This provenance note preserves the original source/raw evidence instead of presenting it as a different worktree.

## Cleanup review accepted; isolated guard coverage — 2026-11-19

Claude independently reviewed and accepted the cleanup mechanism, named snapshot lifetime and actual extraction SIGKILL evidence, and re-approved the revised **333.90 MiB** measured envelope. Final activation approval is still withheld pending exact-revision Registry CI, deployment with enforcement off and the actual Render operational probe. The existing health URL/no-extra-proxy confirmations remain recorded; no production action was taken by the builder.

Two low-severity test gaps were closed without changing runtime code. All three prefixes now have an unlocked, own-UID, **0700**, eight-hex-suffix legacy directory with preserved content, so only the managed-name restriction can protect it. The post-open race test removes an already-open directory before flock, verifies the owner's context never yields and verifies descriptor closure. macOS/APFS retains a removed open directory's link count; the portable test models Linux's zero-link report there, and a separate network-disabled Linux container independently verified real unlink/fstat behavior and the production rejection.

In-memory mutation checks confirm specificity: loosening the name pattern fails all three legacy cases; removing the link-count guard fails the new after-open case. No production source file was rewritten to run the mutations. Logs: `/tmp/agentpm-band1-name-guard-mutation.log` and `/tmp/agentpm-band1-nlink-guard-mutation.log`. The focused cleanup/probe suite now has **16 passing cases**. No capacity rerun was necessary for test/documentation-only changes.

The default expanded-byte ceiling remains **1,000,000,000 bytes**; no operator setting was changed. A conservative initial operator setting could be **167,772,160 bytes (160 MiB) per artifact**, leaving room above the measured 128 MiB payload plus manifest/other members. This would narrow new-publication policy and needs an explicit operator decision. It is not a watchdog guarantee: the legacy single-archive path performs initial archive validation and snapshot transfer before the APDS extraction budget is checked. Keep the measured envelope and queueing risk explicit whichever limit is chosen.

Memory interpretation: reclaimable disk-backed page cache is favorable, but neither overlay nor `shmem=0` guarantees a request cannot trigger OOM. Cgroup `memory.max` can invoke OOM when memory cannot be reduced sufficiently; dirty cache and other allocations still matter. The reported peak includes cache. Do not describe temporary data as unaccounted or immune to the memory ceiling. See the [kernel memory-controller documentation](https://www.kernel.org/doc/html/latest/admin-guide/cgroup-v2.html#memory).

Final guard-coverage verification: **1,087 passed, 7 unrelated opt-in skips, 347 subtests passed** (`/tmp/agentpm-band1-guard-coverage-final.log`). Focused cleanup/probe: **16 passed**. Test-file formatting, imports, lint, Pylint (10/10), Bandit, mypy and pyupgrade passed. No capacity workload or production operation was rerun for this test/documentation-only follow-up.

## Follow-up CI and enforcement-off deployment confirmed — 2026-11-19

The operator confirmed CI passed and the Registry cleanup deployment completed with enforcement still off. This is operator-supplied evidence, not an independent Render connection by the builder. Production operational probe output and final activation approval remain outstanding.

The operator subsequently closed the Render shell. No audit/backfill replay is needed: metadata changes are persisted in the database, and the four original audit reports were transferred, checksum-verified and retained privately under `release-band1-evidence/production-audit/`. Original service `/tmp` paths are not relied on after deployment. Reopen the existing production Registry's Render shell, change to the directory containing `run.py` (previously `/opt/render/project/src`), then run `python scripts/inspect_publication_capacity.py`. Keep enforcement off and retain the output for activation review.

## Production operational probe received — 2026-11-19

The operator supplied the production JSON and it was retained privately at
`release-band1-evidence/production-probe/20261120T032152Z/operator-probe.json`.
The builder did not connect to the deployed API, Redis, DB or storage. Confirmed
settings: Gunicorn 23.0.0, one sync worker/thread, timeout/graceful timeout 30 s,
512 MiB cgroup ceiling, `/tmp` on **ext4**, **48.69 GiB free**, zero managed or
legacy-prefixed inspection names, enforcement false, and live HTTP 200 with two
fresh Redis rate-limit counters. `shmem` was zero; this is a snapshot, not a
production peak-load measurement or OOM guarantee.

One configuration difference remains: **production `preload_app=true`**, while
the archived local configuration evaluates to **false**. The latter was verified
by a configuration-only invocation of the measured image with networking
disabled. The operator has not manually added `GUNICORN_CMD_ARGS`; Render's
[documented native Python default](https://render.com/docs/environment-variables)
automatically supplies `--preload --access-logfile - --bind=0.0.0.0:10000`.
This explains the observed preload setting without a dashboard variable.

Preloading runs `create_app` before worker fork, including ingestion/database use
and scheduler startup. Existing hooks reclaim temps but do not reset inherited
SQLAlchemy pools or reinitialize inherited scheduler state. This is a material
lifecycle difference from the measured setup; [SQLAlchemy documents the risk of
sharing pooled connections across fork](https://docs.sqlalchemy.org/en/20/core/pooling.html#using-connection-pools-with-multiprocessing-or-os-fork).
It is not evidence of a particular historical failure's cause.

Recommended operator alignment: explicitly set Render `GUNICORN_CMD_ARGS` to
`--access-logfile - --bind=0.0.0.0:10000`, preserving the documented listener/logging
flags while removing preload. Keep enforcement off, redeploy, rerun the existing
operational probe, and verify `preload_app=false` with all prior checks still
passing. This is a recommendation, not an applied production change. Retain the
new output and obtain Claude's activation approval. No source-code startup
refactor, additional capacity claim or enforcement activation was performed.

## Aligned production operational probe received — 2026-11-19

The operator supplied the follow-up JSON after the preload override. It reports
**preload_app=false**, Gunicorn 23.0.0, one sync worker/thread, timeout/graceful
timeout 30 seconds, 512 MiB memory limit, ext4 `/tmp` with **52.73 GiB free**,
zero managed/unrecognized inspection names, `shmem=0`, enforcement false, and
live HTTP 200 with two fresh Redis counters. This closes the previously observed
preload configuration mismatch with the measured startup behavior.

The original user-provided JSON is retained privately at
`release-band1-evidence/production-probe/20261120T032719Z/operator-probe.json`, with a scoped assessment alongside. The prior
preload-true output is retained separately. This is operator-supplied evidence:
the probe parses the shell's configuration and performs a live loopback health/
Redis check, not live process-argv inspection or a production capacity stress run.
The builder did not contact or modify production.

CI and enforcement-off deployment were already operator-confirmed. Claude then
approved activation contingent only on successful hosted Registry CI for the
exact deployed revision, with no further review required once that condition is
met. The two guard-coverage follow-ups are already closed and mutation-tested.

## Final CI coverage correction and operator cutover — 2026-11-19

The builder independently confirmed successful hosted runs on Registry revision
`72bb09016632a80a3ca621eda9125bbfa0597796`:
[Release Contract Parity](https://github.com/agentpm-dev/registry/actions/runs/44666387182)
and [APDS conformance parity](https://github.com/agentpm-dev/registry/actions/runs/44666387178).
However, the release job's explicit pytest list omitted
`tests/test_inspection_temporary.py` and
`tests/test_publication_capacity_probe.py`. Those tests had passed locally, but
the successful hosted runs did not execute them. Both files are now included in
`.github/workflows/release-contract.yml`. The exact updated job selection passes
locally against disposable PostgreSQL: **449 passed, 24 subtests passed** (69
existing SQLAlchemy warnings).

The operator subsequently confirmed a green hosted run of that workflow
correction and successful Registry redeployment with enforcement enabled. This
satisfies Claude's final CI condition and closes step 7. This is a CI selection
correction; application code and the reviewed capacity envelope are unchanged.
Another review is not required by that approval.

The operator confirms there are no current users or active publishes. For this
cutover, setting **`APDS_PUBLISH_ENFORCED=true`** in Render and redeploying is
sufficient after the CI condition is met; no separate publishing-pause mechanism
is needed. Retain the deployment ID, commit and server UTC activation time,
confirm the new deployment is serving with enforcement enabled, then perform the
step 9 production smoke checks. The operator confirmed activation with
`APDS_PUBLISH_ENFORCED=true`, closing step 8. The deployment ID, exact deployed
commit and UTC activation time have not yet been supplied; retain them with the
smoke evidence. No production smoke checks have been reported or performed by
the builder. The builder did not contact or modify production.

## Post-activation production smoke checklist

These are small real production publications and verification checks under an
operator-controlled namespace, using the released CLI and an authorized signing
key. They do not repeat the capacity experiment. Successful smoke versions are
real immutable releases; choose their package names and versions deliberately.
Keep request receipts, HTTP status/error bodies, install output and release URLs
in ignored `release-band1-evidence/`, without tokens, private keys or presigned
URLs. Record the activation deployment ID, commit and UTC time there too.

| Check                | Expected result / retained evidence                                                                                                                                                                                                                                                                                                                                                               |
| -------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Historical release   | Resolve/download/install an exact pre-activation signed release with `--require-signature --require-attestation`; compare its integrity/signature references with the retained history audit. Installation succeeds and historical bytes/references are unchanged.                                                                                                                                |
| Compliant successor  | Publish a tiny compliant new version of an existing legacy package identity, with explicit `agentpm / 1.0.0` and normal signing. Exercise both single-archive and release-upload protocols across the smoke set. Both succeed with normal published URLs; clean installs verify signatures and attestations.                                                                                      |
| Raw-client rejection | For each protocol, reserve/upload a tiny artifact missing `standard`, then finalize directly through the API. Expect HTTP 422, `apds_nonconformant`, `APDS-ID-001`, `/standard` and concrete CLI >= 0.1.36 upgrade guidance. The rejected version is not discoverable; a successful init is not a successful publication. The current CLI's local rejection alone does not prove the server gate. |
| Completed retry      | Repeat each successful finalize with its original receipt/request. It returns the original committed release without another version or changed integrity/signature evidence. Re-running `agentpm publish` is not the same test.                                                                                                                                                                  |
| Pending upload       | Separate init/upload from finalize, then finalize the still-unexpired compliant session successfully. Rejected staged bytes require correction, rebuild and a fresh reservation. Retain the session/finalize result; do not edit the bytes bound to an existing reservation.                                                                                                                      |
| Operational check    | Confirm enforcement remains enabled, normal health/install traffic works, and Render logs show no unexpected storage errors, worker timeouts or failed cleanup during these small requests. Rejected staging data remains under normal expiry cleanup; do not delete published history to clean up a smoke test. Ordinary rollback keeps enforcement enabled.                                     |

The pre-activation inventory had zero pending sessions, and none spanning the
actual cutover have been reported. If no such session exists, record that fact;
do not disable enforcement to manufacture one. The before/after-cutoff policy is
covered by the reviewed local PostgreSQL tests. Post-activation split-stage
checks exercise current production finalize behavior without claiming a
historical cutover observation.

The M4A manual helper is deliberately local-only; it must not be repointed at
production. Raw production requests need a separate narrowly scoped sequence
with the intended identities and expected outcomes visible before execution.
This checklist records the required evidence, not completed smoke results.

## Production smoke blocker: inherited setgid on temporary directories

The operator's first new Python release (`summarize-text@0.1.11`) failed at
`2026-11-20T03:51:43Z`: release finalize reached APDS snapshot evaluation but
raised HTTP 500 in `_open_directory` before extraction/evaluation/publication
commit. This is a Registry filesystem guard defect, not an APDS rejection of
the example. The release reservation remains retry state; this failure does
not consume a published version.

The operator's Render probe confirmed real/effective UID 1000, `/tmp` owned by
UID 0 / GID 1000 at `03777`, and both `TemporaryDirectory` and `mkdir(0700)`
children owned by UID/GID 1000 at `02700`. A network-isolated Linux reproduction
confirmed the original exact-`0700` guard rejects that inherited setgid bit and
leaves an unlocked private managed directory behind. The earlier operational
probe recorded filesystem type but not these permission bits.

The correction permits `0700` and `2700` by masking only `stat.S_ISGID` during
the access-mode check. It retains exact owning UID, no group/other access,
symlink refusal, managed-name restrictions and inode-lock/reclaimed-inode
checks. No permission is broadened, and service `/tmp` should not be chmodded.
The new worker's existing startup sweep can reclaim eligible unlocked managed
`2700` paths. No schema, APDS bytes, signing contract, capacity limits or example
version changes are needed for this server fix.

Validation: full local Registry suite **1,091 passed, 11 skipped, 347 subtests
passed**, with 69 existing SQLAlchemy warnings. Three newly added inheritance
cases are skipped on APFS, where the same parent does not propagate setgid;
separate network-isolated Linux execution as UID 1000 runs the complete cleanup
file, including real `03777` parent / `02700` child inheritance, live-lock skip,
orphan reclamation and five group/other-access rejection cases. Python format,
lint, type and security checks also pass. The Linux cleanup suite passed **19 tests with no skips** as UID 1000. The
actual APDS snapshot evaluator returned conformant under the same inherited
mode and left no paths behind. Removing only the setgid mask makes all three
new inherited-mode cases fail. Private logs, source hashes and the operator
permission probe are retained under
`release-band1-evidence/smoke/20261120T034804Z/setgid-fix/`.

The operator subsequently confirmed the correction was all good and the
unchanged `0.1.11` publish passed. The saved local
`release-band1-evidence/smoke/20261120T034804Z/publish-summarize-text.log` confirms
17 resolved Python distributions, a portable `any` source artifact and successful
publication as version ID **139**, with the
[normal published URL](https://www.agentpackagemanager.com/tools/b36a9384-ed1b-4379-80ce-736f520dcd5c/v0.1.11/overview).
The production blocker is resolved; step 4 Agent publication and step 5 verified
new-version installs are next. Smoke gate 9 remains open for those and the
remaining direct API/reference checks. This update verifies saved local output
and operator confirmation, not an independent hosted-CI/Render inspection. The
builder has not contacted or changed production.

## Step 4 archive omission and CLI 0.1.46 patch preparation

The operator's `devwork-copilot@0.1.9` publication was rejected with HTTP 422,
`apds_nonconformant`, `APDS-ID-004 /readme`. The builder inspected the actual
archive: it contained only `agent.json`, although its unchanged manifest
explicitly declared `README.md`. The Registry correctly rejected missing
content; CLI page-metadata extraction had printed a README success line without
putting that file in the archive. Standalone lint and archive creation alone
were insufficient checks in the initial smoke preparation.

Audit identified the same missing inclusion in Agent, Template and non-Python
Tool packaging. Python Tools and the five other Component packagers already
have declared-file inclusion paths. The patch adds required, safely resolved
README/license files to the three missing paths, with normalized de-duplication,
existing archive-entry limits, and atomic failure preserving previous output.
It does not weaken the Registry validator or rewrite the published APDS bundle.

The historical dependency cache also exposed
`github-issues@0.1.1` missing its declared README. Correcting only the root Agent
would leave that dependency-content failure. Prepared publication order:

1. Release/install-confirm **CLI 0.1.46** after independent review and required
   hosted CI. Crate version, Cargo-generated lock and unreleased changelog are
   updated; SDK versions stay **0.1.17**. The lock changes only the CLI version.
2. Publish **github-issues 0.1.2**, with a supported declaration and corrected
   Node archive. Its private Node project version is aligned.
3. Publish **issue-triage-playbook 0.1.1**, declaring the standard and selecting
   the new Tool version.
4. Retry **devwork-copilot 0.1.9**, now selecting the corrected Skill. Its direct
   summarize-text dependency remains **0.1.10**. The already-published
   summarize-text **0.1.11** source is unchanged by this patch.
5. Continue clean-consumer verification and the outstanding direct API/reference
   checks. Existing consumer apps/locks/installed trees have not been edited.

Matching Registry MDX, CLI README and example publishing guidance are updated.
The Web CLI marker is prepared as **v0.1.46**, planned local date **2026-11-19**;
queue that deployment until the CLI is actually published/installable and correct
its date if release occurs later. Keep enforcement enabled. Changed Agent bytes
need a fresh reservation: an unexpired rejected reservation can temporarily
return 409; allow normal expiry rather than modifying its bound bytes or DB row.

Validation completed on the 0.1.46 tree:

- `cargo test --workspace --no-fail-fast`: **1,291 passed, 0 failed, 6 ignored**.
  Four new Rust tests exercise all three affected packagers, actual archive
  extraction and packaged APDS evaluation, missing-file atomicity, normalized
  duplicate selection and outside-root symlinks.
- `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all
--check`, and the immutable schema/bundle check pass.
- The rebuilt binary reports **agentpm 0.1.46**. All three actual example
  sources lint and dry-run package; GitHub Tool build and four mocked tests pass.
- Registry `validate_snapshot` and its actual dependency-content evaluator accept
  the three rebuilt archives using exact prepared versions and the historical
  install cache. New Tool/Skill scopes are conformant; Agent standalone/packaged
  scopes are conformant and its seven-dependency resolved scope explicitly
  remains incomplete only for historical inferred declarations. The resolver
  callback uses those exact local selections; this does not claim a live
  production authorization/storage/selection test.
- Web typecheck and focused ESLint pass. Relevant formatters and whitespace
  checks pass; no tests were added for the version/status literals.

The original rejected archive/log, corrected archive-validation results, full
CLI test/clippy logs and source hashes are retained privately under
`release-band1-evidence/smoke/20261120T034804Z/agent-packaging-fix/`.
The [runbook](manual-tests/RELEASE_BAND1_SMOKE.md#4-publish-the-compliant-agent-package)
now gives the CLI upgrade, dependency publication order, archive-member check
and exact installed dependency assertions. Review, hosted CI, CLI patch release
and those production publications are still pending; smoke gate 9 remains open.
No production request or mutation was performed by the builder.

## Python declared display-file review correction

Claude's finding was confirmed on the initial 0.1.46 candidate: the Python Tool
collector added declared README/license paths only when optional discovery and
bounded text reads succeeded. Missing or oversized explicit declarations could
therefore produce an apparently successful archive that omitted required content.
The initial Node-only fixture did not establish Python error handling.

The candidate now resolves explicitly declared paths as required package files,
rejects missing/unsafe/unreadable or invalid-UTF-8 content, and hard-fails above
its existing **512 KiB README / 128 KiB license** display-content limits. It does
not raise those CLI limits or silently omit an explicit declaration. A file at
the exact limit is accepted. The Python collector must retain the declared file
(or it must already be the mandatory root manifest); generated-output exclusions
cannot silently remove it. Optional discovery of an undeclared README keeps its
previous behavior, including omission when that discovered file is oversized.

The shared inclusion, missing-file atomicity, duplicate selection and external
symlink tests now exercise Python as well as Agent, Template and Node Tool
paths. Five additional unit tests cover cap overflow/exact boundaries, invalid
UTF-8, oversized optional discovery, generated-output exclusion and already
embedded manifest selection. A real-binary integration test exercises both
`package --machine` and `publish --dry-run` for missing/oversized/invalid-UTF-8
README and license declarations, asserting nonzero status, field-specific errors
and unchanged prior saved archives/descriptors. That test is included in the
existing native `ci_publishing` matrix selection.

Final verification: **1,297 passed, 0 failed, 6 ignored** across the workspace;
clippy with warnings denied and rustfmt pass. Public CLI README, changelog and
Registry MDX describe the explicit declaration behavior and Python limits.
Private logs/source hashes are retained at
`release-band1-evidence/smoke/20261120T034804Z/agent-packaging-fix/python-declared-display/`.
CLI **0.1.46** is still an unpublished candidate; no further version bump,
SDK/Web change or production operation was performed for this correction.
Independent review, hosted CI, patch release and the remaining smoke steps stay
open.
