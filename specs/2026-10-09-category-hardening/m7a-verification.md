# M7A: Universal Health evidence inventory

Builder verification: **2026-11-22**. Scope: M7A only, D12/D19,
T-HEALTH-01/03 and the evidence-design portion of S2-AC-10. Implementation and
local checks are complete. The user confirmed **independent Claude review passed
with no open findings on 2026-11-22**, including the trust-label challenge.
**Hosted CI remains pending.** No production requests, database writes,
package publication or deployment occurred.

## Deliverables and decisions

- [Evidence inventory and source matrix](m7a-health-evidence-inventory.md): exact
  stored fields/services, identity/release/artifact/local scope, allowed claims,
  missing-data behavior, timestamp/report-link/privacy rules, eight-kind
  applicability and concrete handoff gaps for M7B.
- Registry `tests/test_health_evidence_sources.py`: 22 probes against the existing
  Security service, malware aggregate and popularity query. Synthetic stored rows
  and mocked sessions exercise real assembly/query code; they are not live DB,
  scanner, storage or cryptographic-verification results.
- Updated M7A task state, test mapping, reviewer focus and blog handoff. M7B/M10B
  checklist follow-ups bind the timestamp and observer-attribution requirements
  to their later implementation; the original spec and runtime code are unchanged.

**D12:** choose shared conceptual evidence records with kind-specific facts and
reuse existing Security/provenance/scanner sources. Reject eight divergent
calculations, presence-as-verification and a universal score. The inventory
records the choice; M7B implements the DTO and M10B the UI.

**D19:** preserve `declared` / `legacy_inferred` and native validation levels,
incomplete/unsupported states and `runner_evaluated: false`. Do not introduce
an `explicit` transport spelling or retrospectively certify legacy artifacts.

## Actual checks

Commands below ran from the named repository, all with **exit 0**. The Registry
prefix supplies a public all-zero test key for import-time configuration and
disables AWS metadata discovery. It is not production signing material.

Registry:

```sh
PYTHONPATH=. \
REGISTRY_PRIVATE_KEY_B64=AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA= \
AWS_EC2_METADATA_DISABLED=true \
.venv/bin/python -m pytest -q \
  tests/test_health_evidence_sources.py \
  tests/test_tools_helpers.py \
  tests/test_malware_rollup.py \
  tests/test_install_apds.py \
  tests/test_release_contract.py \
  tests/test_apds_publication.py \
  tests/test_registry_trust.py::test_shared_keyset_canonical_bytes_and_root_authentication
```

**169 passed, 11 subtests passed**, including all 22 new probes; rerun after
Python formatting. These cover selected-version sources and access denial,
APDS absent/inferred/unsupported/incomplete cases, archive/publication checks,
digest separation, scan progress/coverage and root-authenticated key delivery.

CLI:

```sh
cargo test -p agentpm-sdk provenance::tests -- --nocapture
python3 scripts/check_apds_schema.py
```

**6 provenance tests passed; 1 deliberately ignored** because it requires a
separately exported disposable-PostgreSQL backfill proof. The six executed
tests perform actual cryptography: authenticated new/legacy releases, historical
Tool statements, root/origin/signature/time validity, mutated release bindings
and policy requiring crypto rather than counts/booleans. The schema check
reports: `APDS schema copy and bundle checksums agree.` No Rust code changed.

Web:

```sh
pnpm exec vitest run src/components/tools/PackageSecurity.test.tsx
```

**18 passed**: current Security displays for all eight kinds, version switching,
revoked signer history and missing/multi-target data. This is regression
evidence for existing UI; no new Health badges or browser/manual acceptance is
claimed.

Python hook equivalents ran using the installed cached pre-commit environments
for the new test file: Black; isort with `--profile black`; flake8 with
`--max-line-length=120`; mypy with `--follow-imports=skip` (scoped to the new
test, retaining repo `mypy.ini`); pylint with `--rcfile=.pylintrc`; bandit with
`--skip B101`; pyupgrade with `--py39-plus`; trailing-whitespace,
end-of-file and merge-conflict checks. All passed; pylint rated the file 10/10.
New Markdown artifacts were formatted with the workspace's installed Prettier.
Planning-file edits preserve their existing formatting. `git diff --check`
passes in both changed repositories.

## Negative cases and authority mapping

| Case / test                                                                                                                                                                   | Actual observation                                                                                                                                                                     | Health design consequence                                                                                                         |
| ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------- |
| `test_security_sources_follow_selected_version_for_every_kind` (eight kinds, two selected rows each)                                                                          | Service retains selected version predicates, target digests, license, inferred origin, incomplete APDS and absent scan/attestation                                                     | Source facts follow the selected release; incomplete/inferred/absent do not become passing                                        |
| `test_security_exposes_presence_material_without_claiming_fresh_verification`                                                                                                 | Deliberately invalid synthetic signature/attestation rows are returned as material; `registry_attested` is true solely from row presence                                               | That boolean/row count cannot supply verified Health; use the real verifier or not-evaluated state                                |
| `test_legacy_security_digest_is_display_only_and_declaration_is_not_validation`                                                                                               | Short legacy display hash, no logical release digest, NULL APDS fields                                                                                                                 | Fetch authoritative full hash; no fabricated APDS or logical-release proof                                                        |
| `test_unavailable_security_never_queries_evidence` (private/missing version)                                                                                                  | 404 before evidence SQL; private denial also prevents version lookup                                                                                                                   | Keep authorization before new aggregation; this does not prove future private dependency projection safe                          |
| `test_scan_coverage_progress_and_result_are_independent` (nine cases)                                                                                                         | Missing target/legacy-only row/empty inventory remain unknown; running+clean retains running; complete+unknown stays unknown; failed/infected/suspicious retained                      | Require complete coverage and completion as well as clean; no scan is not a clean scan; aggregate currently supplies no timestamp |
| `test_popularity_queries_package_identity_and_retains_existing_day_bucket_sum`                                                                                                | Existing day-bucket sum and star count use package identity without version joins                                                                                                      | Reuse existing metrics outside Health; no ranking/scoring changes                                                                 |
| `test_install_apds.py` / `test_apds_publication.py`                                                                                                                           | Identity/origin conflicts and unsupported declarations rejected; gate-off observations do not fabricate validator evidence                                                             | Declared support, validation and inference are distinct                                                                           |
| Rust `policy_requires_crypto_instead_of_counts_or_booleans`, `mutations_of_every_release_binding_fail_verification`, `keyset_root_origin_signature_and_validity_are_required` | Real verifier rejects insufficient/invalid/mismatched proofs                                                                                                                           | No signature/attestation verified claim without the complete trust and binding check                                              |
| Source inspection: selected date, compatibility, license, optional scanner columns                                                                                            | Selected publication date exists; `latest_published_ts` refers to latest; hints/license are declarations; optional vulnerability/SBOM columns have no writer in the inspected pipeline | Old date is advisory, compatibility is not enforcement, license is not legal assurance, unused columns are unavailable evidence   |

For **T-HEALTH-03**, the required cases are explicitly constrained in the
inventory: unverified signature → presence/not evaluated; unknown scan → unknown;
absent attestation → absent/not evaluated; compatibility hint → advisory; old
release → dated/advisory; inferred APDS → legacy interpretation. None permits
“safe,” “fully verified” or a global score. No implementation of the future
Health state projection is claimed by these source probes.

## Compatibility, migration and remaining gates

**Claude finding addressed — observer attribution:** the M7A vocabulary now
requires the observer and named check in every public verified label, including
compact badges and accessible names. A Registry-reported result cannot imply
independent local verification; instructions to verify locally are not a
completed check. Missing observer/trust context cannot yield verified status.
M7B payload and M10B rendering checklists, T-HEALTH-03 and the reviewer checklist
now bind this rule. This follow-up changes planning/audit documents only;
payload/rendering tests are explicit future gates, not claimed as executed here.

No runtime files, DTOs, routes, schemas, published APDS bytes, locks, SDK payloads,
existing UI labels, Stage 1 metrics, migrations or configuration changed. No
product interaction changed, so no API user docs/release-version bump is needed.
Merge the Registry tests and CLI audit/task records through their ordinary CI;
there is no M7A deployment or cutover. Re-read source changes before building M7B
if later release revisions differ from the pinned inventory baseline.

No live DB migration, scanner run, browser session, full workspace suite or new
production audit was performed: this slice adds source evidence/tests and a
design artifact only. Previously retained M14C/Band 2 deployment observations
establish prerequisite availability, not current verification of the catalog.
The opt-in PostgreSQL backfill proof remains an explicitly unrun existing test.

**Risks for the later implementation:** presence booleans are not fresh crypto
verdicts; APDS has no retained validator timestamp/revision/full graph snapshot;
Security omits scan timestamps and full legacy digests; recency uses the wrong
scope if latest is substituted for selected; formal deprecation and optional
vulnerability reports lack authoritative data; raw proof/notes/dependency
material requires a safe authorized Health projection. These are documented
boundaries rather than new behavior silently added in M7A.

**Acceptance:** the M7A source inventory, scope/status/freshness contract and
D12/D19 builder decisions are delivered. S2-AC-10 is only partially evidenced:
its public response/rendering requirements belong to M7B/M10B. T-HEALTH-02/04/05
remain implementation gates there. Claude's independent trust-label review is
complete with no open findings, as confirmed by the user on 2026-11-22. Hosted CI
for these new changes is not yet run; that remains the milestone's open gate.
