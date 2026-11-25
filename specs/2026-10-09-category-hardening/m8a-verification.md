# M8A implementation and verification

Builder: Codex. Date: 2026-11-25. User confirmed independent Claude review complete with no new findings on 2026-11-25. Hosted CI remains pending.

## Scope and D07

The website now serves `/standards/agentpm/1.0.0` from an explicit supported-version
catalog. `agentpm-web/src/content/standards/agentpm-1.0.0.json` contains the
published README, semantics and schema as exact UTF-8 strings. The web build runs
`scripts/check-apds-reference.mjs` before Next.js, comparing those strings with
the published SHA-256 values. It needs neither GitHub access nor a sibling CLI
checkout for its standard content. The existing Google font build fetch remains
separate from this contract source.

| File                   | Published SHA-256                                                  |
| ---------------------- | ------------------------------------------------------------------ |
| `README.md`            | `7607e88683644d70dc3d4848ebb423283c5b5a9802ec404fb296deb0c20febd7` |
| `semantics.md`         | `c81daa2e5194e98d964e046dac872227ad7c34b191bba30cad79df365f48c4cc` |
| `manifest.schema.json` | `f123150ff42cea4b29edb5076d7d70697269d20ad85118b4a07fe5af5cf2ab04` |

The existing React Markdown/GFM and heading-ID packages render the frozen text.
The README is the introduction; the eleven-chapter index exposes individual rule
anchors before the complete semantics. Heading levels adapt the second document
to the page outline without changing its text. Relative source/fixture links use
the immutable `apds-v1.0.0` tag. The schema link is its exact canonical `$id`.
No moving `main`, author-controlled schema URL, remote MDX evaluation or runtime
standard download is involved. Unknown identities/versions return `notFound`
with a not-supported explanation and nonindexable metadata, never another
version's content.

The route uses the **current global shell**, Panel and existing prose/color
conventions. M8B.1 explicitly owns its later shell adoption: first content-route
migration after shared primitives, before M9/M10 package-page adoption. This
milestone does not create a new global layout, change kind colors or implement
the category explainer.

Stage 1 SEO integration uses `publicMetadata`, the existing canonical origin,
Open Graph/Twitter values and the pages sitemap. Robots remains unchanged and
allows discovery. Only catalogued concrete standard routes enter the sitemap;
unknown versions have no canonical fallback. The full response/static rendering
behavior still follows the existing root shell; build-pinned content does not
require forcing authentication-dependent root layout code static.

## Selected-release links and evidence

All eight detail layouts use one `PackageDefinition` panel. Its raw-manifest
link uses the **concrete returned package version**, including when the incoming
page route selected `latest`. A supported declaration links to its exact APDS
reference. Missing declarations receive qualified legacy copy and an explicitly
qualified compatibility-reference link. Malformed or unknown declarations never
inherit a supported declaration link. `$schema` cannot select the reference.

`/packages/{package_id}/{exact_version}/agent.json` validates UUID/exact SemVer,
reads the existing authenticated/anonymous Registry detail surface through
`fetchFlaskPublicRead`, and returns every stored manifest field. It refuses
`latest` aliases, hides missing/inaccessible records as 404 and sanitizes upstream
failure/malformed/wrong-version responses as 503. All responses are JSON with
`private, no-store`, `noindex, nofollow` and `nosniff` headers. No new Registry
authorization implementation or API response contract is introduced.

This is a JSON serialization of the selected **stored manifest**, not a claim to
have fetched the archive or reproduced its JSON whitespace. The distinction is
also documented publicly in `agentpm-api/docs/v0.1/agent.json/apds-reference.mdx`.

The existing Security endpoint now adds typed `apds_evidence` signals from
`package_health.apds_signals`, the same identity reconciliation, report consistency
checks and worst-scope projection used by Health. The existing raw `standard`,
`standard_origin` and `apds_validation` fields remain for compatibility but do not
supply the web's evaluation labels. The web validates the projected transport
shape and displays its reconciled identity, scope/status, unevaluated-rule counts
and actual timestamp. Unknown/malformed evidence stays unavailable. An older API
or cached response without `apds_evidence` cannot fall back to raw JSONB.

Inferred legacy interpretation cannot become a conformance result, even if
contradictory evaluation data is supplied. Declared version alone is not
conformance; recorded definition checks are distinct from signatures, scans,
Runner support and runtime readiness. No bare Verified badge or new cryptographic
verification claim is introduced.

The broader Health UI, composition views and reusable card redesign remain with
their assigned milestones. The Registry Security response gains only the additive
validated projection; its existing fields, access controls and queries remain.
No schema, CLI, SDK, database, historical artifact, signature or production
configuration changed.

## Required checks and outcomes

Commands run from `agentpm-web`, unless another directory is specified.

| Check                                                                                                                                                                                                                                                                             | Actual outcome                                                                                                                            |
| --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------- |
| `pnpm exec vitest run src/app/standards src/app/packages src/components/tools/PackageDefinition.test.tsx src/components/tools/PackageConformance.test.tsx src/components/tools/PackageDetailShell.test.tsx src/components/tools/PackageSecurity.test.tsx src/app/sitemap.test.ts` | Exit 0; 117 tests across seven files passed.                                                                                              |
| `pnpm test`                                                                                                                                                                                                                                                                       | Exit 0; 334 tests across 64 files passed after the review corrections and isolated status-allowlist regression.                           |
| `pnpm typecheck`                                                                                                                                                                                                                                                                  | Exit 0.                                                                                                                                   |
| `pnpm lint`                                                                                                                                                                                                                                                                       | Exit 0; six pre-existing unused-variable warnings in unrelated files.                                                                     |
| `pnpm standards:check`                                                                                                                                                                                                                                                            | Exit 0; all three frozen byte hashes match.                                                                                               |
| `node scripts/check-apds-reference.mjs --source ../agentpm/standards/agentpm/1.0.0`                                                                                                                                                                                               | Exit 0; trusted local source and committed copy match the published hashes.                                                               |
| `pnpm build` with hosted-CI placeholder environment                                                                                                                                                                                                                               | Exit 0 in an isolated test-owned source copy; standard route and generic raw-manifest route compiled successfully. See build notes below. |
| Repo-native Prettier on changed web files and new MDX/Markdown                                                                                                                                                                                                                    | Run before handoff; frozen strings remain checksum-identical. Existing planning-document layout is retained with narrow edits.            |
| API: `PYTHONPATH=.:tests REGISTRY_PRIVATE_KEY_B64=AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA= AWS_EC2_METADATA_DISABLED=true .venv/bin/python -m pytest -q tests/test_docs_content.py`                                                                                           | Exit 0; 5 tests plus 173 subtests passed. The key is an all-zero test fixture, not a production secret.                                   |
| CLI: `python3 scripts/check_apds_schema.py`                                                                                                                                                                                                                                       | Exit 0; canonical bundle and generated schema agree.                                                                                      |
| `git diff --check`                                                                                                                                                                                                                                                                | Exit 0 in changed repositories.                                                                                                           |

The tests cover:

- **T-DETAIL-04:** separate selected-release manifest/reference links across all
  eight kinds; version switching; concrete returned version; historical missing,
  malformed and unsupported declarations; recorded scoped evidence and missing
  timestamps; intact raw JSON and access/error/cache controls.
- **T-DETAIL-06:** exact published checksums; deliberate normative-source mutation
  rejected by the actual deployment checker; exact canonical schema/tagged
  semantics/fixture links; all chapter/rule anchors resolve; keyboard focus and
  native disclosure controls; unknown-route behavior/metadata; no fetch for
  malicious schema hints or unsupported standard identities.
- Existing detail actions, stars, latest statistics, specialized tabs, Security,
  docs and sitemap tests remain green.

### Build and manual-verification limits

The first sandboxed build could not resolve `fonts.googleapis.com`. The authorized
network retry compiled but encountered generated-route manifest interference: a
user-owned `next dev` process was using the same `.next` directory. It was left
running. A fresh test-owned source copy, with existing dependencies symlinked and
CI placeholder configuration instead of `.env` files, passed the production
build. The source comparison before final whitespace formatting covered 333
TypeScript/TSX/JSON/build-input files with zero mismatches.

An additional temporary isolated-server loopback smoke initially failed in its
Next proxy configuration before rendering a page; those attempts are not counted
as passes. Each probe stopped its own server and did not stop the user-owned
server. The already-running **configured local web server** was then checked
with read-only Node `fetch` requests (`node --input-type=module`, origin
`http://localhost:3000`, 20-second per-request timeout):

| Request                              | Observed result                                                                              |
| ------------------------------------ | -------------------------------------------------------------------------------------------- |
| `/standards/agentpm/1.0.0`           | HTTP 200; canonical URL is `https://agentpackagemanager.com/standards/agentpm/1.0.0`.        |
| `/standards/agentpm/9.0.0`           | HTTP 404; not-supported content, noindex and no fallback canonical.                          |
| `/packages/invalid/1.0.0/agent.json` | HTTP 404; `private, no-store` and noindex. No Registry request is needed for the invalid ID. |

The production build and configured local HTTP smoke both passed. No production
Registry request was made. Manual browser/operator sign-off still needs visual
narrow-width/keyboard inspection and real private-package access. The tests
exercise the shared access helper and inaccessible responses, but those mocks are
not claimed as a live private-namespace test.

## Review correction: one authoritative APDS report decision

The reviewer demonstrated that a shallow client schema accepted raw JSONB that
Health rejects. This was a valid correctness/trust-label finding. The fix shares
`apds_signals` between Health and Security and exposes its typed safe projection
as `apds_evidence`; the web never interprets raw `apds_validation` as evidence.
This also carries Health's aggregate worst-scope status instead of reporting the
raw top-level/resolved status when another scope failed.

New API regressions compare the actual Security service/DTO serialization against
Health for 17 individually inconsistent inputs (status mismatch, conformant with
issues/unevaluated rules, missing/extra levels, wrong policy, top/level Runner
claims, level identity, declared/supported standard, malformed/empty rule IDs,
invalid status and stored/unsupported identity). Five further cases preserve
valid, legacy, absent, mixed-scope and incomplete behavior. They also retain the
raw compatibility field and establish that projected evidence omits raw issues.
The web tests the reviewer's combined contradictory report, old APIs with no
projection, rejected projections, reconciled legacy identity and mixed-scope
failure overriding a raw conformant result. Existing minimal service-test rows
were updated to include the manifest/report attributes now read by the shared
projector; production model attributes already exist.

Review-fix validation on 2026-11-25:

- API targeted: `PYTHONPATH=.:tests REGISTRY_PRIVATE_KEY_B64=AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA= AWS_EC2_METADATA_DISABLED=true .venv/bin/python -m pytest -q tests/test_package_health_security.py tests/test_package_health_routes.py tests/test_package_health.py tests/test_health_evidence_sources.py tests/test_release_contract.py tests/test_tools_helpers.py tests/test_docs_content.py` — exit 0, **192 passed plus 184 subtests**, no skips.
- API full: the same environment with `.venv/bin/python -m pytest -q` — exit 0, **1,028 passed, 354 subtests passed, 205 environment-dependent skips**. No isolated PostgreSQL fixture was provisioned for this correction; do not claim those skipped integration cases passed.
- Web: `pnpm test` — exit 0, **315 passed**. `pnpm typecheck`, `pnpm lint` and Prettier pass; lint retains the six existing unrelated warnings.
- Python: repo-pinned Black 23.9.1, isort with the Black profile, flake8, mypy (`--follow-imports=skip` over seven changed files), pylint (10/10), bandit excluding B101, pyupgrade, whitespace/EOF/conflict checks pass. A cached newer formatter was initially selected; the final files were checked/formatted with 23.9.1 and unrelated wrapping removed.
- The production build was rerun in the existing test-owned isolated copy with the updated web sources and CI-placeholder environment; exit 0.

Deploy the Registry correction with Web. Any old/cached Security payload without
the new projection displays unavailable conformance until it is refreshed, rather
than making an unchecked claim. No database migration, new evidence write,
additional graph/cryptographic inspection or change to validation policy occurs.
User confirmed independent review of the corrections complete with no new findings on 2026-11-25; hosted CI remains pending.

## Review correction: extensible projected level names

The review correctly identified Zod 4's exhaustive enum-keyed record behavior.
The web's projected `levels` map now accepts nonempty string keys with validated
entry shapes, rather than requiring the exact standalone/resolved/packaged set.
It renders every projected named scope, including additional future levels. The
explicit nonempty-map guard is now reachable and tested. Malformed entries and
empty maps remain unavailable, and raw reports still cannot supply a fallback.
The Registry remains the sole authority for applicable levels and consistency;
its current three-level report policy has not been relaxed or duplicated here.

Ten regression cases cover sparse maps (including exactly two of three with
`resolved` absent), an added fourth scope, a failed future
scope retaining the Registry's aggregate status, empty maps, missing/malformed
entry fields, unknown statuses and empty level names. Targeted command
`pnpm exec vitest run src/components/tools/PackageConformance.test.tsx src/components/tools/PackageSecurity.test.tsx`
initially passed **46 tests**. After adding the exact two-of-three case,
`pnpm test` passed **325 tests across 64 files**;
`pnpm typecheck`, `pnpm lint` and Prettier checks passed. The same six unrelated
lint warnings remain. This correction changes two web files only, plus the
milestone evidence notes; it adds no API policy or dependency change. The prior
production-build result belongs to the preceding correction; this narrow parser
change was verified with tests, typecheck, lint and formatting without claiming
another production build.

## Review correction: structural Security route helper

`packageSecurityPath` now lives beside `packageOverviewPath` in the existing SEO
route utility. Both share the package/version prefix, including URL encoding and
optional `v` normalization, and append their own explicit tab segment.
`PackageDefinition` calls the Security helper directly instead of replacing the
Overview suffix with a regular expression.

Eight direct route tests cover all kinds, encoded package IDs and prefixed/plain
prerelease-plus-build versions. The existing all-kind component test also checks
that switching releases updates the visible Security link. Final commands
`pnpm test`, `pnpm typecheck`, `pnpm lint` and Prettier pass: **333 tests across
64 files**, with the same six unrelated lint warnings. This is a path-construction
refactor only; no route contract, navigation destination or API behavior changes.

## Review correction: directly gate the status allowlist

The reviewer correctly identified that the prior raw-report fallback test paired
an `unknown` status with an empty value, so it could not prove the independent
status guard. That test was renamed to describe its actual raw-fallback scope.
A separate regression now supplies `status: "unknown"` with the same valid,
populated projected report used by the passing case. All identity, reference,
entry-shape and nonempty-level checks succeed; the allowlist alone must suppress
the conformance claim and show unavailable.

Mutation verification ran in the existing test-owned isolated source copy:
replacing `['passed', 'failed', 'incomplete'].includes(...)` with `true` made
`unknown status with a populated valid report still renders unavailable` fail
(exit 1) because the unavailable text disappeared. The copied component was
restored in `finally`; the live workspace implementation was never mutated.
`pnpm test` then passed **334 tests across 64 files** with the real guard;
`pnpm typecheck`, targeted ESLint and Prettier passed. No production code, API
policy or user-facing behavior changed for this coverage correction.

## Acceptance mapping and outstanding gates

| Requirement | Scoped result                                                                                                                                                                                  |
| ----------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| D07         | Trusted committed bundle with deployment checksum gate; current-shell delivery and M8B.1 adoption assigned. Independent decision review passed, user-confirmed 2026-11-25.                     |
| S2-AC-12    | Builder tests establish selected-release raw manifest vs immutable standard links and honest declaration/inference/recorded-result separation. Real-browser/operator sign-off remains pending. |
| S2-AC-17    | Versioned reference and Stage 1 canonical/robots/sitemap integration implemented and tested. Broader category explainer, pricing and redesign work remain with later milestones.               |

User confirmed independent Claude review complete with no new findings on
2026-11-25, including the review corrections and isolated status-allowlist
regression. The review gate is satisfied.

Outstanding: hosted Web/CLI/API CI appropriate to the changed repositories,
configured manual browser smoke and coordinated release. M8A is reviewed and
ready for those checks, **not yet marked complete**. No other milestone is
completed by this work.
