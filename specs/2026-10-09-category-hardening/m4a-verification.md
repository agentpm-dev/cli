# M4A — Registry APDS publication validation

Implemented 2026-11-17. Local automated verification passes. User confirmed full
independent Claude review passed with no remaining findings on 2026-11-17. Hosted
CI and operator publication acceptance remain pending; production enforcement is
not activated and the milestone exit gate is not yet complete.

## Scope and source inventory

The Registry now authorizes APDS publication from verified archive snapshots in
both existing finalize paths. It uses the independent Python engine from M2C;
there is no new validator, schema change, normative bundle change, SDK change,
lock change, Runner change or production backfill.

| Surface                       | Existing contract and M4A change                                                                                                                                                                                                                                                                                                                                                                        |
| ----------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Single-archive init/upload    | `app/tools/application/cli.py`: init reserves package/version/manifest, digest, size and staging/final keys in `uploads`; the client PUTs to staging. Init remains preliminary.                                                                                                                                                                                                                         |
| Single-archive finalize       | `CliService.publish_finalize` retains namespace/PAT/dependency access, upload expiry and completed-retry behavior. Before a new version is written, the active gate independently hashes the uploaded bytes, reads embedded `agent.json`, compares the reservation and authorized identity, and evaluates applicable scopes. Kind-specific readers and final storage use that exact snapshot.           |
| Multi-artifact init/upload    | `release_publish.py` reserves the release manifest and per-target staging records in `package_publish_releases` / `package_publish_artifacts`; existing digest, manifest and target checks remain authoritative.                                                                                                                                                                                        |
| Multi-artifact finalize       | Every verified target snapshot is APDS-checked before its final storage; all targets must pass before release metadata, signatures, attestation and scan rows commit. The existing storage-free locked commit phase is preserved.                                                                                                                                                                       |
| Shared archive/storage checks | `publish_apds.py` reuses secure tar inspection, adds bounded extraction with Python's `data` filter, and rejects ambiguous JSON/member paths. The existing streamed size/SHA verifier and checksum-confirmed conditional PUT contract are shared through `release_storage.py`, rather than duplicated.                                                                                                  |
| Version evidence              | `tool_versions` / `PackageVersion` gain nullable standard ID/version/origin and scoped validation JSON via actual Pyway migration V1.107. `ToolSecurity` and `ToolsService` expose optional evidence separately from scan, signature and attestation fields.                                                                                                                                            |
| Existing sessions and history | Disposable fixtures include an undeclared published legacy row, legacy pending/completed uploads, pending/finalized multi-target reservations, failed attempts and retries. Original manifest/digest/storage fields remain unchanged when the additive migration runs. No production database or real pending-session inventory was queried; M4B owns that operator inventory and the in-flight cutoff. |

Implementation files are under `agentpm-api/app/tools/application/`,
`app/tools/domain/tools/`, `app/tools/common/dtos.py`, `app/tools/publish_routes.py`
and `app/config.py`. The migration is
`app/migrations/V1.107__Persist_Version_APDS_Publication_Evidence.sql`.
Feature-local user guidance is in `docs/v0.1/cli/publish.mdx`; developer policy and
commands are in `app/apds/README.md`. The PostgreSQL release workflow includes the
new tests without depending on a new public CLI contract commit.

## D05: authoritative staged validation, separately activated

Selected: a server-owned `APDS_PUBLISH_ENFORCED` switch, default **false**, with no
client override. M4A implements and tests the active gate. M4B owns coordinated
activation, old-client guidance and the policy for sessions straddling cutover.
This is deployment sequencing, not a publisher-selectable compatibility mode.
Every new version reaching either active finalize path is checked, including
versions under an existing package identity. Completed retries return their
already committed receipt; they do not reinterpret historical bytes.

The archive must have an explicit supported root declaration. Its complete JSON
manifest must match reserved metadata, including `standard`, and its local
kind/name/version must match the authorized Registry package and version. The
existing namespace mapping, release digest and per-target association checks are
retained. JSON booleans and numbers are compared distinctly. JSON Schema integer
bounds on the single-archive path do not acquire release/JCS safe-integer limits.

Standalone checks run first, packaged checks receive the complete safe extraction,
and resolved checks receive actual selected manifests and owner-specific edges.
The graph loader reuses the existing semver and authorization resolver, verifies
selected stored archive bytes and checks their packaged content. Old database
request metadata cannot substitute an invented Loop phase for the actual archive.
Repeated selections are reused only within the request.

Known violations, unsupported required representations and unavailable required
context block publication explicitly. Inaccessible dependencies keep the existing
redacted access error. Storage failures remain retryable storage failures. There
is no success marked fully verified when a required graph cannot be inspected.

One compatibility exception allows a selected historical dependency with no
`standard`. A separate in-memory analysis view applies the supported contract so
that missing metadata cannot suppress local semantic checks. Its original source
is never rewritten or attributed a declaration. All other known violations still
reject. The resolved result is **incomplete**, with `APDS-ID-001` unevaluated and
an explicit legacy-inferred note. This permits new Agent Packages to reference
otherwise valid historical Components without falsely certifying those releases.

Inspection limits are Registry policy: existing compressed upload limits, plus
`MAX_APDS_UNPACKED_BYTES` (default 1,000,000,000 expanded bytes per artifact), 256
selected graph nodes and 16 MiB of selected manifest JSON. Bounds fail explicitly;
they are not APDS requirements. Maximum-size/concurrent capacity has not been
benchmarked and must be considered before production activation.

Errors return scope/rule/source JSON Pointer evidence where available; reservation
mismatches report their source field. Temporary inspection paths are sanitized.
APDS exceptions explicitly roll back the legacy transaction, whose decorator
otherwise commits returned errors. A real competing unique version insert returns
409 and cannot reuse the other version's signatures or APDS evidence. Multi-target
rejection leaves no discoverable version. Existing immutable final objects from a
partial storage attempt remain safe retry state; staging/expiry cleanup retains
its established lifecycle.

## Capacity evidence required before M4B activation

Review clarification: enabling enforcement does **not** reduce the single-archive
root path to one remote read. Its preliminary staging inspection remains. The
inactive path then observes the final archive; the active path instead downloads
staging into a verified local snapshot. Both have two baseline root GETs.
Knowledge/Memory inspections can add remote reads while inactive; when active,
`SnapshotStorage` serves those inspections locally. Multi-target publication
already verifies a local snapshot per target while inactive and has a different
baseline; measure it separately.

| Work in single-archive finalization  | Enforcement off                        | Enforcement on                                                   |
| ------------------------------------ | -------------------------------------- | ---------------------------------------------------------------- |
| Baseline root GETs                   | Staging inspection + final observation | Staging inspection + verified snapshot download                  |
| Root full extraction                 | None                                   | Complete safe extraction to temporary disk                       |
| Selected dependency archive download | None for APDS validation               | Complete verified download for each request-cache miss           |
| Selected dependency full extraction  | None for APDS validation               | Complete safe extraction and packaged checks for each cache miss |

The active gate adds work proportional to selected artifact bytes and expansion,
not just graph-node count or manifest text. There is no request-wide cumulative
artifact-byte or extraction-work budget. The 256-node/16-MiB metadata bounds and
individual compressed/expanded limits are not evidence that large graphs fit the
deployment's latency or resource envelope. The cache is request-local and keyed
by `(kind, identity, selector)`: repeated identical selections deduplicate, while
different selectors selecting the same version may still download it again.
Publishing another version starts a new cache; no cross-request artifact cache
exists.

Dependency downloads/extractions are sequential, and each dependency's temporary
archive and extracted tree are removed before the next is inspected. Consequently,
256 large dependencies describe cumulative network/CPU/extraction work, not 256
simultaneously retained extracted trees. Peak temporary disk includes the retained
root snapshot and extracted root plus the current dependency snapshot/extraction
and inspection metadata. Concurrent finalize requests multiply that demand.

M4B's unchecked capacity task and T-PUB-10 require a measured realistic worst case
using the most dependency-heavy Agent Package and Template, representative large
artifacts, repeated publications and planned request concurrency. Record actual
sizes, GET counts, cumulative bytes/work, peak memory/temp disk, elapsed time,
failure cleanup and the deployed worker/proxy timeout and disk headroom. Establish
an acceptable operating envelope or verify mitigation **before activation**.
No such load test was run in M4A; the green functional suite does not satisfy this
gate. Cross-request caching or another mitigation is not implemented by this note.

## D19: declaration provenance and scoped evidence

New nullable columns: `standard_id`, `standard_version`, `standard_origin`,
`apds_validation`. The SQL constraint rejects partial identity and a
`legacy_inferred` row with invented validation evidence. Existing rows remain all
null; this milestone performs no historical metadata backfill.

Review finding 1 closed: declaration observation runs even with enforcement off.
A well-formed selector from the archive records its actual ID/version and
`standard_origin: declared`, with `apds_validation` null. Unsupported declarations
can be observed without being certified or silently substituted. Missing or
malformed selectors acquire no invented identity. The single-archive legacy copy
path observes the **final stored archive**, not request metadata or a potentially
replaced staging object. This adds one streaming final-archive inspection while
enforcement is off. Multi-target publication reuses its already verified archive
snapshots and existing manifest/release association checks. Active-gate metadata
comes from the actual validated snapshot's standalone evidence.

PostgreSQL regressions cover declared/undeclared publications and completed retries
on both paths, forged init declarations, semantic-invalid input remaining ungated
while enforcement is off, and staging replacement before copy. The replacement
case records the actual final declaration rather than the prior observation.
Malformed selector unit cases reject fabricated metadata. No migration or
constraint change is needed: declared identity with null evidence is already
permitted, while inferred identity with evidence remains forbidden.

An actively validated new root records `agentpm`, `1.0.0`, `declared`, with separate
standalone/packaged/resolved results and `runner_evaluated: false`. The overall
status can remain incomplete for the historical-dependency exception; a declared
root does not turn its historical dependency into a declared or verified release.
Multi-target evidence records the checked target inventory. Optional security DTO
fields expose the version evidence without merging it into trust, malware,
authorization or execution-readiness signals. Resolve/lock/UI integration remains
with its owning later milestones.

## S2-AC-05 and test-plan mapping

| Requirement             | Evidence and boundary                                                                                                                                                                                                                                                                                                                                                                                                  |
| ----------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| T-PUB-01                | Actual tar bytes for all eight kinds pass standalone, packaged and resolved checks. All eight pass the actual single-archive application service against disposable PostgreSQL, preserving the package URL and completed retry. Python multi-target publication also passes. Normal CLI-to-running-Registry publication for all kinds remains operator acceptance after review; these are not live-publication claims. |
| T-PUB-02–03             | Raw finalize/HTTP cases reject missing/malformed/unsupported declarations, invalid semantics, forged metadata and valid-hash mismatches. Strict JSON and unsafe archive regressions exercise actual bytes. Rejections create no version/signature/attestation or scan publication rows.                                                                                                                                |
| T-PUB-06                | Multi-target success persists one version; a later Windows target missing its declared file rejects the entire release. Existing inventory/signing/transaction/scan tests also pass.                                                                                                                                                                                                                                   |
| T-PUB-07                | Active-gate tests retain required author signature rejection, namespace denial, Registry attestation and scan queue creation. Forged S3 metadata cannot replace actual byte hashing; staged replacement after inspection cannot replace the verified final payload. Existing malware rollup, signing and ACL regressions pass. This does not claim live malware-provider execution.                                    |
| T-PUB-08                | Actual selected Loop bytes override forged historical request metadata. Unknown phases fail, historical missing selectors remain incomplete, missing historical Knowledge files and invalid historical Loop semantics still fail, and inaccessible context is redacted and blocks publication.                                                                                                                         |
| Migration / idempotency | Real PostgreSQL constraints, competing version insertion, successful repeat finalization, pre-existing immutable object verification, additive legacy-row preservation and an actual Pyway V1.107 upgrade all pass.                                                                                                                                                                                                    |
| M4B boundary            | T-PUB-04/05/09 production history/backfill, client floor, cutover/grace/restart policy and activation are not completed here. No production changes or version bumps were made.                                                                                                                                                                                                                                        |

## Commands and actual results

Commands run from `agentpm-api` unless noted. All final commands below exited 0.
Tests use synthetic keys, in-memory storage and random schemas in the disposable
local `agentpm_m13a` database; the application database was not changed.

```sh
AGENTPM_M13A_TEST_DATABASE_URL=postgresql+psycopg://postgres:postgres@127.0.0.1:5433/agentpm_m13a \
REGISTRY_PRIVATE_KEY_B64=AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA= \
AGENTPM_APDS_TEST_CLI=/Users/zackhine/projects/agentpm-project/agentpm/target/debug/agentpm \
AWS_EC2_METADATA_DISABLED=true \
  .venv/bin/python -B -m pytest -q -rs
```

Result: **1,050 passed, 7 skipped, 347 subtests passed**. The seven skips are the
existing opt-in revoked-PAT, search relevance and sitemap tests requiring
`AGENTPM_SEARCH_TEST_DATABASE_URI`; they are unrelated to APDS publication.
There are 47 existing SQLAlchemy `Query.get()` deprecation warnings from the
legacy finalizer exercised by the new tests. The new publication files contain
48 archive/semantic tests and 48 PostgreSQL/HTTP integration tests.

Earlier focused runs covered APDS/signing/Python-resolution regressions (228
passed, 1 optional native comparison skipped before supplying the CLI path),
release/migration regressions (60 passed), and feature docs (45 passed, 166
subtests). Final full-suite results above include the final expanded behavior. The review
correction also passed a focused publication/signing run (142 tests, 24 subtests)
and Python-resolution/archive regressions (76 tests). The first expanded run found
an old storage fixture reusing one closed response body; it now returns a fresh
body per GET, matching S3 ownership. The final full suite is green.

```sh
.venv/bin/python -B scripts/check_apds.py \
  --cli-root /tmp/agentpm-m4a-apds-source --out target/apds-python-results.json
.venv/bin/python -B app/apds/contracts/agentpm/1.0.0/conformance/run.py \
  --results target/apds-python-results.json

# From the CLI repository:
../agentpm-api/.venv/bin/python -B scripts/check_apds_schema.py
```

The source check uses an isolated detached checkout at the existing pinned commit
`e308bfefb880f2a3a96cc08f4f87d61d3c55a76a`; no pin/bundle changes were made. An
initial attempt against the working CLI checkout correctly failed the immutable
source guard because its HEAD is newer. The corrected source check passes all
129 Python corpus cases and the pinned checksum inventory. The portable consumer
and current CLI schema/bundle checksum checks pass. The full suite also enables
the actual current CLI comparison test via `AGENTPM_APDS_TEST_CLI`.

The configured Python hooks were run using their cached pinned executables on all
16 changed/new Python files: Black 23.9.1, isort 5.12.0 (`--profile black`),
Flake8 6.1.0 (`--max-line-length=120`), mypy 1.5.1
(`--follow-imports=silent`, narrow changed-file check), Pylint 3.1.0
(`--rcfile=.pylintrc`), Bandit 1.7.5 (`--skip B101`) and pyupgrade 3.11.1
(`--py39-plus`). Pylint uses a writable temporary `PYLINTHOME`. No dependencies
were added. The narrow mypy check avoids unrelated existing imported search-module
errors; no new type error is suppressed. Black binary-operator formatting is
excluded from Flake8 W503 only for the three new publication files, following the
existing APDS convention. Trailing whitespace, EOF and conflict-marker hooks,
Prettier for changed Markdown/MDX/YAML, and `git diff --check` pass.

## Remaining acceptance and rollout risks

- User confirmed independent Claude review passed with no remaining findings on
  2026-11-17. Hosted Registry CI remains pending.
- The normal-CLI, running-Registry all-kind publication smoke in T-PUB-01 remains
  operator acceptance; no manual runbook or production publication was done during
  implementation. Prepare it after review, following the established local-folder
  workflow.
- M4B must inventory production history and pending sessions, finalize compatibility
  and client guidance, and explicitly activate enforcement. Apply V1.107 before
  serving updated ORM queries even while the switch is off.
- Large expanded archives and dependency inspection cost temporary disk, I/O and
  memory. Limits are enforced, but maximum-size/concurrent load was not measured.
  The M4B activation checklist and T-PUB-10 now explicitly require this evidence;
  baseline root GET counts alone do not establish comparable enforcement cost.
- Historical archives with real content or semantic defects can prevent a new
  composition referencing them. Their original release is not rewritten; publish
  a corrected Component version or address compatibility deliberately in M4B.

No production setting, signing root, immutable release, public standard tag or
schema was changed. This is a builder handoff, not a release or conformance claim
for Runner behavior.
