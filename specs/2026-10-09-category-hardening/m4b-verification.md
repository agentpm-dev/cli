# M4B: publication compatibility and rollout

**M4B implementation, independent review and PR CI complete: 2026-11-18.** Scope: M4B only. Claude's migration/security
design approval is granted on the local evidence, as supplied by the user on
2026-11-18. The user subsequently confirmed full Claude review complete with no
new findings on 2026-11-18, including closure of the guard coverage correction.
The user confirmed all PR CI passed on 2026-11-18; the CLI runs were also
independently inspected as recorded below. Production history inspection, 138
inferred metadata backfills and unchanged-reference verification were
operator-confirmed on 2026-11-19. Measured capacity and the separate activation
approval remain outstanding. This record does not enable production enforcement
or close the release band.

Production-only execution is explicitly sequenced in [Release Band 1's ordered
production rollout](tasks.md#ordered-production-rollout): review/CI, authoring CLI
release, Registry/MDX deployment with enforcement off, production inspection and
reviewed backfill, capacity approval, coordinated activation and controlled smoke
checks. The new audit command must be deployed in Render before it can inspect
production there. These gates precede activation and release-band closure; they
do not prevent the initial deployment with enforcement off.

## D05: finalize-time policy, without grandfathering pending uploads

Selected policy: `explicit-standard-at-finalize-no-grace-v1`.

The existing server-controlled `APDS_PUBLISH_ENFORCED` flag governs every pending
finalization on both protocols, regardless of init date, package identity age or
client-provided timestamps/flags. A compliant pending upload can finish within
its existing expiry. A noncompliant upload must be corrected locally, rebuilt
and reserved again. There is no bounded or indefinite pre-cutover grace bypass.
Completed publications retain their original receipts, bytes and evidence.

New validation evidence records the policy ID. Multi-target finalization retains
the policy in its existing verification/locked-commit intent comparison: a
policy change during object inspection forces a retry, rather than committing
an unchecked result under the new setting. No new request field can select
policy. Existing authorization/signing/scan/integrity gates remain separate.

Activation must pause publishing and drain active finalize requests while all
workers redeploy with the gate true. The operational boundary is the completed
fleet transition; a rolling deployment with mixed flag values is not a valid
cutover. Record deployment/server UTC time/configuration evidence. This also
handles the single-archive transaction's already-running request safely without
inventing a client-clock or init-time exemption. Ordinary rollback retains the
true flag; any emergency disabling needs a paused/drained, recorded and reviewed
exception with a new inventory.

The default remains false until the explicit operator gates pass. This preserves
the reviewed deployment sequence; it is not a claim that enforcement is active.

## D19: observed declarations versus inferred historical compatibility

The new explicit operator command is:

```sh
python -m flask --app run registry audit-apds-history --out history-before.json
python -m flask --app run registry audit-apds-history \
  --inspect-artifacts --out history-inspected.json
# Only after inspection/review; never run automatically at startup:
python -m flask --app run registry audit-apds-history \
  --inspect-artifacts --apply --out history-applied.json
```

Run from the Registry root in its configured environment. Repeat `--version-id`
for bounded batches. All-version inventory includes yanked history and pending
sessions on both protocols. Single-archive uploads lack persisted creation time;
the report records null, not a client timestamp or guessed cutoff.

The audit fingerprints immutable version columns and all artifact, author and
attestation rows. Inspection verifies stored size/SHA for every target, compares
actual embedded identity/target manifests and release-manifest binding, then
observes the declaration. Absent declarations become `legacy_inferred`, supported
effective interpretation `agentpm/1.0.0`, with no validation evidence. Well-formed
declarations retain their actual ID/version as `declared`, including an unsupported
version; inspection is not certification. Malformed or inconsistent history stops
for review. Null metadata is never itself evidence that a declaration was absent.

Apply re-inspects actual archives; it cannot import a caller-edited report. It
returns the database connection before object I/O, locks and freshly compares
version/reference fingerprints before applying, and changes only previously
absent APDS metadata. A late conflict rolls back the entire batch. Existing
validated evidence cannot be downgraded. Repeating a successful backfill changes
zero rows. No objects, manifest bytes, digest/version fields, signatures,
attestations or scan rows are rewritten or re-signed.

`PackageVersion.apds_validation` now uses `JSONB(none_as_null=True)`: absence of
evidence must be SQL NULL for V1.107's existing `legacy_inferred` constraint,
not the distinct JSON `null` value. No DDL or frozen APDS schema/bundle changed.
Previously stored declared rows with JSON-null evidence still read as absent
evidence; this work does not rewrite their history.

## Real local history inventory

A **read-only** transaction queried the real local Docker application database
on 2026-11-18 (`SET TRANSACTION READ ONLY`, then rollback). This was not the
disposable test schema and not production. The private inventory is saved under
the CLI's ignored `target/m4b/history-local.json` with mode 0600:

- 113 actual version rows; 93 with no recorded APDS metadata, 20 declared.
- 76 versions have author-signature references; 89 have Registry attestations.
- 16 pending session rows, including rows that may await expiry cleanup.

For example, actual historical `@zack/summarize@0.1.7` (version row 25) references
34,822 bytes with stored SHA-256
`7c59585cdb13978d33ca9150b21ee9cbbc9f267106a239d655304c8d40ed15bd`.
Its captured author-row fingerprint is
`760d93ac0338dafda854891f013abdc2e13d1418baa7f8ff1786da3986d3f133`
and attestation-row fingerprint is
`1fa375cd267a61e4e28a967bba4649455c180b80e559ce5cea3c188bacc2bbc4`.
These are database references, not a new signature verification claim.

No S3 history inspection or metadata apply ran against this database. Its serving
API configuration was not inspected; the saved report explicitly marks that
unknown. In particular, **93 unrecorded rows must not be relabeled wholesale as
legacy**. Production inventory/inspection and any reviewed backfill remain
operator steps in the rollout guide.

## Files and user-facing behavior

All product changes are in `agentpm-api`:

| File                                                  | Purpose                                                                                                           |
| ----------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------- |
| `app/tools/application/apds_history.py`               | Read-only inventory, verified archive observation, conservative atomic metadata backfill                          |
| `app/registry_routes.py`                              | Explicit `registry audit-apds-history` command; inspection required for apply; exclusive 0600 output              |
| `app/tools/application/publish_apds.py`               | Recorded policy ID and concrete minimum-publisher guidance on root selector failures                              |
| `app/config.py`, `app/tools/domain/tools/entities.py` | Cutover policy comment and correct SQL-null persistence                                                           |
| `tests/test_apds_rollout.py`                          | Real PostgreSQL cutover, history, completed retry, install/successor and operator-command coverage                |
| `tests/test_apds_publish_integration.py`              | Reuse the existing reservation fixture under an existing package identity                                         |
| `.github/workflows/release-contract.yml`              | Run the rollout tests in the existing PostgreSQL release-contract job                                             |
| `docs/v0.1/cli/publish.mdx`                           | Minimum 0.1.36 tolerance floor, actual upgrade commands, rebuild/re-init guidance, historical install distinction |
| `app/apds/ROLLOUT.md`, `app/apds/README.md`           | Operator inventory, no-grace policy, deployment/rollback and capacity measurement procedure                       |

Missing/malformed/unsupported root selectors retain the existing APDS error code,
scope, rule and source pointers. Their message now names **CLI >=0.1.36** as the
declaration-tolerance floor and recommends the latest CLI for current authoring
checks. It directs the publisher to lint, rebuild and reserve again. A custom
client is still allowed if its actual artifact conforms; a client version alone
never authorizes publication. Existing success URL behavior is unchanged.

No Rust command, SDK, lockfile, resolve DTO, web component or normative APDS file
changed. Public MDX contains no internal milestone/stage language. Shared
package-page update guidance remains with M9B/M10A, as agreed.

## Verification

The final full-suite command from the Registry root is:

```sh
REGISTRY_PRIVATE_KEY_B64=AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA= \
AWS_EC2_METADATA_DISABLED=true \
AGENTPM_APDS_TEST_CLI=/Users/zackhine/projects/agentpm-project/agentpm/target/debug/agentpm \
AGENTPM_M13A_TEST_DATABASE_URL=postgresql+psycopg://postgres:postgres@127.0.0.1:5433/agentpm_m13a \
.venv/bin/python -B -m pytest -q -rs
```

The database URL points at the existing disposable local test database. Tests
apply real checked-in Pyway migrations to random schemas, exercise actual service
transactions/routes, and use in-memory archive storage with real tar bytes.
The synthetic all-zero Registry signing key is test-only. This command does not
publish to a live Registry or change the application database.

Final result: **1,071 passed, 7 skipped, 347 subtests passed**, exit 0 in 24.60 s.
The seven pre-existing optional search/PAT/sitemap tests require a separate
`AGENTPM_SEARCH_TEST_DATABASE_URI`; they are not rollout coverage. Existing
`Query.get()` deprecation warnings remain. Log: `/tmp/agentpm-m4b-history-consistency-full.log`.

Focused publication/rollout baseline before the final additional atomicity test:
110 passed, exit 0 (`/tmp/agentpm-m4b-focused.log`). The final full suite includes
that extra late-conflict/verified-evidence test and six review regression cases:
**21 new rollout tests** total.

These additional checks passed with exit 0:

- Registry: `.venv/bin/python -B scripts/check_apds.py --out /tmp/agentpm-m4b-apds-python-results.json` — all 129 Python cases and pinned inventory agree.
- CLI root: `../agentpm-api/.venv/bin/python -B scripts/check_apds_schema.py` — generated schema and bundle checksums agree, without rewriting them.
- MDX: compile `docs/v0.1/cli/publish.mdx` using the existing web `@mdx-js/mdx` compiler — success; no web source changed.
- Cached repo-hook equivalents on all seven touched Python files: Black 23.9.1, isort 5.12 with Black profile, Flake8 6.1/Bugbear/comprehensions, mypy 1.5.1, Pylint 3.1, Bandit 1.7.5 and pyupgrade 3.11.1. Pylint: 10.00/10; Bandit: zero issues. The two new modules receive the same per-file W503 exception already used by the APDS modules for Black's operator wrapping; no global suppression was added.
- Mypy uses `--follow-imports=silent` on the touched files, as in the previous milestone, to avoid unrelated baseline transitive search errors; it is not a claim that a full-tree mypy run is clean.
- Prettier for changed/new feature Markdown, MDX and workflow YAML; repo EOF/whitespace/conflict hooks and `git diff --check`. Existing long specification tables retain their local formatting, avoiding an unrelated whole-document reformat.

Rust/SDK/web test suites were not rerun: no implementation in those repositories
changed. The full Registry suite used the real built CLI for its opt-in native
comparison, and the separate CLI bundle checksum check passed. Hosted PR CI
passed as recorded below. Claude's design approval and the separate pending
activation approval are recorded below. The user confirmed full post-fix Claude
review complete with no new findings on 2026-11-18.

### Hosted PR CI confirmation

On 2026-11-18 the user confirmed all CI passed. The actual CLI PR runs for commit
`670a996c612d29b771e923b0ee376a4352a3ee5b` were independently inspected via GitHub:

- [CI run 330](https://github.com/agentpm-dev/cli/actions/runs/44435195713): build passed, including formatting, Clippy, workspace tests, APDS drift/corpus and positive CLI fixture checks. `platform-targets` and `Native runtime (${{ matrix.target }})` were skipped as configured for pull requests.
- [Resolver Contract run 51](https://github.com/agentpm-dev/cli/actions/runs/44435195791): target selection and **all four** native targets passed: macOS arm64, macOS x86_64, Linux x86_64 and Windows x86_64. Every target completed both payload/target compatibility tests and native managed Python environment verification. The user's pasted check list showed only the two macOS rows; GitHub's complete job list includes the successful Linux/Windows rows too.
- Registry hosted CI success is user-confirmed; its run was not independently queried in this inspection.

In `.github/workflows/ci.yml`, `platform-targets` explicitly requires
`github.event_name == 'push'`, and `platform-tests` depends on it. These skips on
a `pull_request` event are intentional. The unexpanded target label is the skipped
matrix job's name template, not a failed target. The separate Resolver Contract
workflow supplies actual four-platform native coverage on this PR. After merge
to main, confirm the push-triggered CI target/native matrix passes before tagging
the CLI release; those later runs are not claimed as already executed.

### Review correction: stored metadata contradicting immutable artifacts

The stored-versus-artifact declaration guard worked but lacked a regression that
would detect its removal. `test_history_rejects_stored_declaration_that_contradicts_artifact`
now exercises six real PostgreSQL cases: mismatched standard ID, version and
declared/inferred origin, for both declared and undeclared historical archives.
Each starts with an honestly published, correctly hashed archive, introduces
contradictory but SQL-valid metadata, and requires the exact stored-declaration
error for that version. Inventory/reference fingerprints, existing metadata and
stored bytes remain unchanged after rejection. No product behavior was changed.

Using the same disposable database settings as above,
`.venv/bin/python -B -m pytest -q tests/test_apds_rollout.py -k stored_declaration`
passes all **6 cases**, exit 0 (`/tmp/agentpm-m4b-history-consistency.log`).
An isolated process then replaced only the loaded guard condition with false,
without editing any product file: all **6 cases failed with `DID NOT RAISE ValueError`**,
pytest exit 1 as expected (`/tmp/agentpm-m4b-history-consistency-mutant.log`). The
subsequent full suite used the original unmodified guard and passed with the
updated totals above. All applicable configured Python hook equivalents passed
for the changed test file. The user confirmed full Claude review complete with
no new findings on 2026-11-18 after this correction. Production activation
approval remains a later gate.

### Claude migration/security design approval

On 2026-11-18 the user supplied Claude's explicit approval of the migration and
security design on the local evidence: backfill only fills absent metadata and
cannot certify history, immutable artifacts/signatures stay untouched, batch
writes are atomic/idempotent, release finalization compares policy fingerprints,
pending sessions have no age/client-clock grace, and historical exact installs
retain their original integrity alongside compliant successors. Claude also
reported independent mutation checks of backfill restrictions and policy
fingerprint behavior; those are reviewer-supplied evidence, separate from the
builder runs above.

The M4B checklist's **migration/security matrix approval is granted**. Claude
explicitly distinguished that design approval from **Release Band 1 step 7's
activation approval**, which cannot be granted until production inspection and
any reviewed backfill (steps 4–5) and the production-equivalent capacity envelope
(step 6) are available. Step 7 and activation step 8 remain unchecked. The one
actionable test-coverage finding in the supplied review is addressed by the six
guard regression/mutation cases recorded above. The user subsequently confirmed
full Claude review complete with no new findings on 2026-11-18. That completes
the independent code/design review; it does not pre-grant the later production
activation approval.

## Security/compatibility matrix and acceptance

| Requirement                                                  | Evidence / remaining gate                                                                                                                                                                                                                                                                   |
| ------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| T-PUB-04: old version versus new version under same identity | Undeclared successor rejected, fresh declared reservation accepted; old exact resolve/install-init still returns original integrity and verification material; original bytes/references unchanged                                                                                          |
| T-PUB-05: pending boundary                                   | Both protocols, with/without declaration; no timestamp/flag exemption; compliant pending upload passes; failed upload remains pending without version/artifact/signing/scan publication; completed undeclared receipts remain unchanged                                                     |
| T-PUB-09: concrete upgrade guidance and success URL          | Existing all-eight-kind receipt tests plus new selector-failure floor assertions; MDX parsed/formatted; web guidance intentionally remains M9B/M10A                                                                                                                                         |
| Migration/backfill                                           | Actual PostgreSQL constraints, every-target inspection including non-first corruption, real signed multi-target author/attestation preservation, malformed declarations, stale metadata, late conflict rollback, idempotence, verified-evidence retention and actual Flask operator command |
| T-PUB-01–03/06–08 regressions                                | Existing M4A archive, authorization, integrity, private graph, signature and atomicity tests retained in full Registry suite                                                                                                                                                                |
| T-PUB-10: capacity before activation                         | **Not yet measured on a deployment matching production**; detailed workload/measurement/mitigation procedure prepared; existing functional results are not capacity evidence                                                                                                                |
| S2-AC-05 / D05 / D19                                         | Implementation and local verification recorded here; full Claude review and PR CI passed; production inventory/backfill verified for 138 inferred versions; measured envelope and separate activation approval/controlled rollout still required                                            |

Before activation, capture the most dependency-heavy real Agent Package and
Template, representative large artifacts, repeat publications and planned
concurrency. Measure root/dependency GETs and compressed/expanded bytes, elapsed
finalize time, worker RSS, peak temporary disk and cleanup against actual
worker/proxy timeouts and headroom, for both protocols and both gate settings.
Use an isolated deployment matching the planned production resources, not a
maximum-load test on the live Registry. No undocumented cache, aggregate artifact
budget, larger compute tier or timeout is assumed. Claude has approved the
migration/security design on local evidence; the separate activation approval
still requires production findings and capacity measurements.

**M4B is complete** as the implementation/review/PR-CI milestone. Per the user-agreed sequencing, production inspection/backfill and unchanged-reference verification were operator-confirmed on 2026-11-19; measured capacity and activation approval/execution remain Release Band 1 gates. The user confirmed all three preparation PRs and CLI main/post-merge CI passed on 2026-11-18. They have not been completed or waived, and Release Band 1 remains open.

## Release Band 1 interruption follow-up — 2026-11-19

Claude independently approved the original measured envelope and production-history evidence, but withheld activation approval pending configuration confirmation and orphan reclamation. The follow-up uses locked, private, own-UID extraction directories and Gunicorn startup/fork/reap hooks. It includes the legacy named verified snapshot in the managed lifetime. Older/untracked paths, audit reports, symlinks and live locks are not deleted.

Real local tests now kill a worker with SIGKILL during root and dependency extraction, with archive bytes already written and Python cleanup unavailable. The production master reclaimed two/four managed directories (including the snapshot) and both requests returned to a zero-byte temp baseline. The post-mitigation representative run recorded 102 expected outcomes and 333.90 MiB peak cgroup memory under 512 MiB. A separate normal-config phase confirmed live Redis-backed rate-limit counters via loopback health; no production API was called.

Full Registry regression: **1,083 passed, 7 unrelated opt-in skips, 347 subtests passed**. Focused publication/rollout/cleanup/probe regression: **81 passed**. Python format/static/security/type checks pass. Evidence and provenance are detailed in [release readiness](release-band-1-readiness.md#independent-capacity-review-and-interruption-follow-up--2026-11-19) and the [single-command runbook](manual-tests/REGISTRY_CAPACITY.md).

This follow-up still requires independent review and hosted CI, then deployment with enforcement off. The operator must retain the production settings/filesystem/live-Redis probe output and obtain separate activation approval. The confirmed HTTP `/health` and no-extra-proxy configuration are recorded with official Render policy references; sustained single-worker health starvation remains an explicit review risk. No production setting, artifact, DB record or S3 object was changed by the builder.

### Independent cleanup review and guard coverage

The operator supplied Claude's acceptance of the cleanup implementation and revised measured envelope on 2026-11-19. Two nonblocking guard-coverage gaps were closed with test-only changes: private 0700 legacy names across all three prefixes, and unlink after open before the post-flock link-count check. Both pass normally and fail with their corresponding guard disabled in memory (three name cases and one inode case). The native Linux unlink/zero-link behavior was verified in a network-disabled local container. The focused cleanup/probe suite now passes 16 cases; no runtime code or production setting changed in this follow-up. Final CI/deploy/probe/activation gates remain open.

Final guard-coverage verification: **1,087 passed, 7 unrelated opt-in skips, 347 subtests passed** (`/tmp/agentpm-band1-guard-coverage-final.log`). Focused cleanup/probe: **16 passed**. Test-file formatting, imports, lint, Pylint (10/10), Bandit, mypy and pyupgrade passed. No capacity workload or production operation was rerun for this test/documentation-only follow-up.
