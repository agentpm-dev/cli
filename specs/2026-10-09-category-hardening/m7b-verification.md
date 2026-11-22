# M7B: Registry Package Health evidence

Builder verification: **2026-11-22**. Scope: M7B only, D12/D17/D19,
T-HEALTH-01–05 at the Registry/data boundary, and the backend portion of
S2-AC-10. Implementation and local verification are complete. The user confirmed
**independent Claude review passed with no findings on 2026-11-22**.
**Hosted CI and the separately assigned operator key-set renewal remain pending.**
No production requests, deployment, root/signing-key rotation or publication
against the user's local Registry occurred.

## Delivered behavior

`GET /v1/tools/{package_id}/{version}/health` exposes one shared selected-release
envelope for all eight kinds. The historical route name remains shared across
kinds. `latest` resolves to a concrete returned version. Anonymous reads require
a public namespace; authenticated private reads use existing namespace access.
Missing/inaccessible packages or versions return 404; database unavailability
returns a sanitized 503 `health_unavailable` response.

The response contains identity, selected version/date, observation time, scoped
signals, target artifacts and kind-specific facts. It has no quality score,
popularity calculations or top-level safety/readiness verdict. Unknown evidence
does not become passing, and irrelevant Tool target signals are not applicable.

| Source changed in `agentpm-api`                                                               | Purpose                                                                                                                                                                                                                                                                                       |
| --------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `app/tools/common/health.py`                                                                  | Shared typed evidence/signal/artifact/envelope models; verified outcomes require observer, named check, trust context, actual check time and an attributed label; expired trust is rejected.                                                                                                  |
| `app/tools/application/package_health.py`                                                     | Selected-version declaration reconciliation, recorded APDS levels, full digest/inventory binding, provenance checks, scan coverage/times, actual selected publication date, unavailable data and safe summary projection.                                                                     |
| `app/tools/application/health_provenance.py`                                                  | Reuse Stage 1 canonical statement models, binding/serialization, Ed25519 decoding and authenticated key-set delivery for Registry-attributed author/attestation checks. Historical normal revocation does not rewrite accepted history.                                                       |
| `app/tools/application/health_composition.py`                                                 | Reuse the authorized install resolver and APDS dependency declarations for exact current graph metadata; include Template Loop dependencies; bound/deduplicate inspection and withhold incomplete private graphs. Project kind-specific declarations without backend/Runner execution claims. |
| `app/tools/application/tools.py`, `app/tools/tool_routes.py`                                  | Thin authorized service/route; authorization before evidence reads; fresh responses with `Cache-Control: private, no-store`. Existing Security endpoint/DTO behavior remains intact.                                                                                                          |
| `app/tools/application/publish_apds.py`                                                       | Record actual UTC evaluation completion time in future Registry publication reports. No old rows/reports or immutable artifact bytes are changed.                                                                                                                                             |
| `app/tools/domain/tools/malware.py`                                                           | Extract the existing pure artifact-scan aggregate so Health can summarize one captured inventory/result snapshot without duplicate queries or inconsistent scan snapshots. Existing aggregate semantics remain unchanged.                                                                     |
| `app/tools/domain/tools/registry_trust.py`                                                    | Optional keyword-only clock input to existing key-set verification, retaining default behavior and enabling exact expiry-boundary tests; typed validity errors distinguish actual expiry from retrieval/encoding failures.                                                                    |
| `tests/health_fixtures.py`, `tests/test_package_health*.py`, `tests/test_apds_publication.py` | Synthetic rows with actual crypto, real Flask HTTP, actual signed publication/SQL reads, privacy/version/expiry/metadata faults and actual new validation timestamps.                                                                                                                         |
| `.flake8`                                                                                     | Apply the established narrow Black binary-operator exception to the new Health modules/tests; other checks remain enabled.                                                                                                                                                                    |
| `docs/v0.1/security/package-health.mdx`                                                       | Feature-local API usage, source/status/observer meanings, privacy, cache and verification limitations. No comprehensive docs rewrite.                                                                                                                                                         |

## D12, D17 and D19

**D12:** choose an additive `/health` endpoint and a shared `schema_version: 1`
envelope. Reusing raw Security fields directly would confuse presence booleans
with verification and expose raw proof/account/scan-note material in the new
summary. Embedding the summary in Security would inherit a five-minute cache
that cannot safely stand in for current trust/visibility. Existing Security
proof/detail transport remains unchanged; Health reuses underlying services and
verification contracts rather than its display hashes or presence flags.

**D17:** use shared signals plus a compact kind-specific `extensions` projection,
not eight APIs/tables. Tool targets/runtime and frozen Python resolution metadata
are distinct from provisioning. Agents expose current authorized composition,
with Runner support/local readiness not evaluated. Skills expose declared
entrypoint/references/scripts without auto-execution. Knowledge exposes mode,
document/corpus/index/hash declarations without embedding/provider claims.
Memory exposes declared contract counts and existing build/index availability,
not backend execution. Profile guidance is advisory; Loop graph counts do not
prove execution; Template scaffold/stack/surface declarations do not imply
Harness suitability. Future UI/tab placement remains M10B.

**D19:** preserve `declared` and `legacy_inferred`; reconcile version columns
with the selected manifest using the existing install adapter. Unsupported,
inconsistent, incomplete and absent validation data stay qualified. Recorded
APDS levels/rules are projected without raw issue paths/messages/notes and never
claim Runner execution. Reports with inconsistent status/context cannot supply a
pass. A historical report without an evaluation timestamp retains `null`;
future actual validations record completion time. Old timezone-less timestamps
are not assigned invented offsets. No schema, manifest, lock or SDK wire-origin
change occurs.

## Verification executed

All commands below ran with exit 0. API tests use Python 3.13 and an isolated
temporary PostgreSQL container/database, created from the existing local image;
they do not read application `.env` or contact production/S3. Actual signed
publication tests use the existing in-memory storage fixture. Canonical APDS
manifests seed unit fixtures; synthetic stored reports/scans are explicitly not
claims of live scanning or newly executed packaged checks.

From `agentpm-api` (the URL is the actual ephemeral test instance used):

```sh
PYTHONPATH=.:tests \
REGISTRY_PRIVATE_KEY_B64=AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA= \
AWS_EC2_METADATA_DISABLED=true \
AGENTPM_M13A_TEST_DATABASE_URL=postgresql+psycopg://postgres:fixture-password@127.0.0.1:58939/agentpm_m13a \
.venv/bin/python -m pytest -q -rs --tb=short --maxfail=3
```

**1,200 passed, 350 subtests passed, 11 skipped** (29.95 seconds). The skips are one opt-in additional CLI parity check, three unsupported setgid inheritance cases on this filesystem, and seven separately configured search/PAT/sitemap SQL fixtures. The 69 SQLAlchemy `Query.get()` warnings are from existing publication/rollout paths. The import-time all-zero test
key and disposable database password above are fixture material, not production
secrets. The temporary database is removed after verification. A fresh rerun
must supply its own disposable localhost `agentpm_m13a` URL; the recorded port
is not a retained service.

The focused Health/source/scanner/publication/docs run passed **164 tests and
169 subtests** before the final timezone-offset preservation case. All changed
Python files pass the configured hook equivalents: Black, isort (Black profile),
flake8 (120 columns), mypy (`--follow-imports=skip`, scoped to changed files),
pylint (`--rcfile=.pylintrc --persistent=n`), bandit (`--skip B101`), pyupgrade
(`--py39-plus`), trailing-whitespace, EOF and merge-conflict checks. New Markdown
and MDX use the installed workspace Prettier. `git diff --check` passes.

From `agentpm`:

```sh
cargo test -p agentpm-sdk provenance::tests -- --nocapture
python3 scripts/check_apds_schema.py
```

**6 provenance tests passed; 1 existing exported-PostgreSQL-proof test is
deliberately ignored.** Executed tests cover actual root/origin/time/signature
verification, historical formats and binding/policy mutations. Strict consumer
verification behavior is unchanged. APDS schema copy and published bundle
checksums agree. No Rust/SDK/frontend runtime code changed.

## Acceptance and negative evidence

| Required case                                                      | Executed evidence                                                                                                                                                                                                                                                                                                                                                                                      |
| ------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Recent and historical release for every kind; source applicability | `test_each_kind_recent_and_legacy_evidence_is_scoped_and_qualified` (16 cases); full hashes, actual selected dates, untimed historic reports, inferred origin, absent proofs and Tool-target N/A.                                                                                                                                                                                                      |
| Signature/attestation actually checked and observer retained       | Eight historical-kind crypto cases plus a real new multi-target release; signed statements checked with Stage 1 contracts. Bare verified/missing observer/check/trust/time/expired context are rejected by the typed signal.                                                                                                                                                                           |
| Bad/unsupported/forged source data                                 | Unsupported selectors, provenance drift, malformed/contradictory APDS reports, forged Runner/status/Health fields, absent evaluated rules, bad signatures, wrong identity/inventory/namespace/key and invalid key sets cannot override authority.                                                                                                                                                      |
| Expired trust and stale caches                                     | Actual Flask requests before/at/after signed-key-set expiry; even a previously cached verified body is not replayed. Attestation becomes unknown with an expired-trust reason, unrelated evidence remains intact, and renewed authenticated material permits a new attributed check.                                                                                                                   |
| Scan coverage/progress/results and real timestamps                 | Full/partial multi-target coverage; clean-but-running; failed/unknown/infected/suspicious; actual stored artifact times; old APDS time absent; new actual validation time between measured start/end on all eight kinds; publication/evaluation/observation times remain distinct.                                                                                                                     |
| Private/dependency facts                                           | Authorization before version lookup; anonymous private denial/authorized reader isolation; public Skill with inaccessible inherited Tool returns no partial graph, hidden names/versions/notes/accounts; graph resolution deduplicates diamonds and stops at selection limits.                                                                                                                         |
| Real PostgreSQL, real publication and HTTP                         | `test_actual_signed_publication_health_is_read_only_scoped_and_crypto_checked`: actual signed multi-artifact publish, authenticated key inventory, canonical release/author/attestation checks and frozen Python resolution digest. Reads leave rows/statements/storage bytes unchanged. Real route/ORM returns the selected version, then 404 immediately after namespace visibility becomes private. |
| Version switch and metrics                                         | Old/new selected HTTP responses change APDS origin/date while Health excludes popularity; existing identity metrics calculations/routes are unchanged and covered by the full regression suite.                                                                                                                                                                                                        |
| Existing detail/contract compatibility                             | Security route/DTO preserved; no shared schema/manifest/signature/lock/SDK protocol changes. Docs metadata, links and content checks pass.                                                                                                                                                                                                                                                             |

## Rollout, limitations and remaining gates

There is no SQL migration: new timestamp metadata uses the existing nullable
validation JSON. Merge/review the Registry implementation/tests/docs and CLI
spec records through their normal CI. Deploy Registry after review/CI through
the planned release band; it can precede the Health UI because the endpoint is
additive. No enforcement/configuration flag needs changing. Comprehensive docs,
UI/badges, browser screenshots and SDK API additions are outside this milestone.

Health reads metadata and performs cryptographic checks locally in the Registry;
it does not download/re-hash archives, execute packages or verify dependency
bytes. Current graph resolution is contextual, not a historical lock. Inspection
is capped at 256 distinct selections/nodes, 4,096 edges and 16 MiB of metadata;
an unavailable/private/bounded graph is withheld as incomplete. There is no
cross-request Health cache, so large graphs cost fresh database reads. Public
Registry checks are explicitly attributed and do not replace independent local
verification against the applicable trust roots.

Historical missing times, formal deprecation, vulnerability/SBOM reports and
scanner version/report URLs remain genuinely unavailable rather than being
fabricated. APDS validation still does not retain a full evaluated dependency
snapshot or validator build revision; its recorded levels are not new proof of
current graph/Runner conformance. No clean scan, signature, license, date or
compatibility hint becomes a safety claim.

**Review:** the user confirmed Claude's independent review passed with no findings
on 2026-11-22, closing the D12/D17 representation/evidence-language review item.

**Still unmet:** hosted CI and Zack's dated operator task to renew/deploy and verify the
production signed key set before the retained `2026-12-07T22:31:48Z` expiry.
This implementation has not renewed or inspected that production set. The
deadline must not wait for M7B or a later release. S2-AC-10's public rendering
portion and M10B screenshots remain with M10B; the Registry evidence slice is
implemented and locally verified.
