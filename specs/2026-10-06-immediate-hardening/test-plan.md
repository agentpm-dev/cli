# Test Plan

## Required verification

The stage is not complete until the major user flows pass end-to-end and legacy compatibility is preserved.

Verification must cover:

- registry search/discovery;
- pagination;
- filters;
- relevance;
- stars/trending;
- namespace curation;
- package detail shared shell;
- technical SEO;
- CLI messaging/lint;
- analytics/privacy/feedback;
- Python dependency portability;
- legacy Python Tool install;
- target-aware multi-artifact publish/install;
- release integrity/signing;
- GitHub Actions or equivalent CI flow.

Where the repo already has canonical test commands, use those commands rather than inventing parallel verification.

## Automated checks

### Platform coverage for this stage

AgentPM CLI CI currently runs on `ubuntu-latest` only, while releases ship macOS and Windows binaries. This stage adds current-target detection, native-payload classification, archive extraction changes, and local runtime-environment provisioning — all of which behave differently per platform and cannot be meaningfully verified on Linux alone.

- The suites covering target detection, payload classification, extraction, and runtime-environment provisioning must execute on macOS and Windows as well as Linux.
- `#[cfg(unix)]`-gated tests must not be the sole coverage for behavior that also has a Windows path.
- If a full matrix per pull request is too slow, the platform matrix must run on merge to main and on release tags; it must not be omitted.
- Evidence for the payload-classification and target-detection cases below is incomplete if produced only from a Linux runner.

### Registry / frontend / API

Run the existing frontend and backend unit/integration suites.

Add automated cases for:

- Explore page-size agreement.
- Cursor history Page 1 ↔ Page 2.
- 3+ page sequential navigation.
- >10 page backward navigation.
- query preservation.
- kind preservation.
- sort preservation.
- filter preservation.
- private-result visibility across all supported sorts.
- mixed namespace/package search.
- strict/relaxed relevance continuity.
- no-result state.
- error state.
- namespace-scoped search reuse.
- star create/delete/count.
- private star authorization.
- `auth_optional` falls through as anonymous on an unverifiable JWT and on an unverifiable PAT, leaving `g.auth` `None`; `auth_required` still returns 401 for the same token.
- stale-session star state: a viewer whose token no longer verifies receives `starred: false` for a package they have starred, and the star control does not render a wrong state that then fails on click.
- revoked PAT installing a private package reports rejected credentials rather than a missing package.
- pinned artifact authorization/order/limit.
- trending full-result-set ranking.
- top-N presentation not truncating Explore sort.
- stable result keys/rendering where testable.
- package Security/detail DTO renders both legacy single-artifact versions and new-format release/artifact versions correctly once portability milestones land.

### Package detail shell verification

Milestone 6 uses the existing shared `StarRail` and `PackageStatsGrid` across Tool,
Agent, Skill, Knowledge, Memory, Profile, Loop, and Template pages.

Run in `agentpm-web`:

```bash
pnpm vitest run src/components/ui/StarRail.test.tsx src/components/tools/PackageDetailShell.test.tsx src/components/tools/PackageSecurity.test.tsx
pnpm test
pnpm typecheck
pnpm lint
pnpm build
```

Keep `PackageStatsGrid` server-only: the Installs and Last publish cells, `Panel`
wrappers, and `agoText` stay server-rendered. Only `StarStatCell` reads client star
context, shared with `StarRail` through the identity-keyed `PackageStarProvider`.
Verify both mobile and desktop Stars cells change during an optimistic write,
accept the server's returned count, and roll back with the rail after a failure.
Seed star state only when the identity-keyed provider mounts. A successful write
must not call `router.refresh()`, and changed initial props for the same mounted
identity must not overwrite its local star state. Verify a different identity
remounts with its own server state. This deliberately favors preserving the user's
write over reconciling concurrent changes from another tab or device; fresh server
state is loaded when the provider remounts.

Run UTC calendar-boundary tests under different process timezones:

```bash
TZ=UTC pnpm vitest run src/lib/date/ago.test.ts
TZ=America/Los_Angeles pnpm vitest run src/lib/date/ago.test.ts
```

The focused tests cover successful star/unstar writes, pending-write suppression,
401 recovery through sign-in with a return URL, server/network failure rollback,
protection from stale same-identity props, identity remounts, and all eight kinds' footer placement, popularity panels,
version switching, install/load/bootstrap/inspect/query actions, and specialized tab
links. Security tests use the real fetchers and response schemas, switch between
two versions with different integrity/provenance/scan/license values, and verify
the selected version's rendered data. The latest-publish panel must pair the
latest timestamp with the latest version, even when an older version is selected.

Run the existing API suite with `uv run pytest -q` using the repository's test
environment. The current Security contract remains the extension point for later
release/artifact data. Milestone 14C owns the new-format Security/detail UI and
release inventory verification, including legacy rendering compatibility, alongside
its public Security/read DTO changes. Milestones 13A–14B supply the prerequisite
release/artifact data and signing foundation/statements.

Browser evidence (2026-10-14): all eight kinds were rendered against the local app
at 390px and 1440px widths. All 16 checks confirm the package header fits the
viewport and the star control ends at the header's bottom border. Retained
measurements: [detail-bounds.json](assets/milestone-6/detail-bounds.json).
Representative screenshots: [Tool mobile](assets/milestone-6/tool-mobile.png) and
[Template desktop](assets/milestone-6/template-desktop.png). Session-expiry recovery
is verified with mocked Clerk and write responses; a live account sign-out/sign-in
was not performed.

### Search relevance fixtures

Create and run a deterministic fixture corpus.

Required assertions:

- exact package-name match outranks semantic/dependency-only matches;
- namespace match behaves as expected;
- known typo still returns intended package;
- metadata-only match is discoverable;
- dependency-only match appears but ranks below the directly matching dependency, verified for each declaring kind — Agent Package, Template and Skill;
- a Template is discoverable through its declared Agent Package dependency, the one edge no other kind produces;
- only direct declared dependencies match; a transitively-reachable package does not surface its ancestor;
- full README-only terms do not influence results;
- filters are hard constraints;
- no-result query stays empty;
- result ordering is deterministic for ties.

### Schema / Template metadata

Validate:

- `agentpm-harness` accepted for Template execution surface;
- invalid execution surface rejected;
- existing execution surfaces remain valid.

### CLI

Run existing Rust/CLI tests.

Add fixture/command tests for:

- all eight `agentpm init --kind` scaffolds lint clean;
- Tool scaffold contains valid minimal files/entrypoint;
- oneOf/anyOf suppression;
- a manifest carrying a permissive `standard` field lints clean for all eight kinds, while a misspelled top-level key (`standrad`, `tool`) is still rejected — proving the root relaxed by one field, not opened;
- `standard` plus one extra unknown top-level key survives `agentpm install <pkg>` write-back and `knowledge build` (the existing default write mode) with no field loss; use a local schema override to allow the extra key for Knowledge build without opening the bundled schema root;
- root kind-union dispatch: a kind-specific violation (e.g. `kind: "agent"` missing a kind-required field) reports the offending branch/field and path, not `/oneOf` with the manifest echoed;
- the diagnostic shape can carry an external rule identifier next to the path, so Stage 2's APDS rule IDs attach without re-cutting the renderer;
- duplicate schema error removal;
- bounded instance echoes;
- semantic errors ordered first;
- closed-union expected-values message;
- JSON and NDJSON lint formats remain valid;
- `publish --dry-run` lint context;
- connection failures for install/export/new/whoami;
- `whoami` exit code 1 on connection failure;
- inspect output normalization;
- no `/./` path rendering;
- `new` success output;
- clap failure for missing `--mcp`;
- concise bad key-id message.

#### Milestone 8 evidence

Verified 2026-10-15, scoped to scaffolding, packaging validation, schema tolerance,
and lint diagnostics. The transport/inspect/key-message cases above belong to
Milestone 9 and were not implemented here.

Commands and results:

- `cargo fmt --all` and `cargo fmt --all --check`: passed.
- `cargo clippy --workspace --all-targets --offline -- -D warnings`: passed.
- `cargo test --workspace --offline --quiet`: 1,077 passed, zero failed or ignored
  (1,057 CLI unit tests, six authoring integration tests, seven machine-run
  integration tests, seven Rust SDK tests). Localhost access was enabled for the
  existing mock HTTP/process fixtures; no live registry/provider was required.
- Black check on `crates/agentpm-cli/assets/templates/tool_main.py.tpl`: passed.
- API pre-commit checks for the six updated MDX pages: whitespace, EOF, and
  merge-conflict checks passed; Python-only hooks correctly skipped MDX.
  Frontmatter parsed successfully for all six pages; no routes or ordering changed.

`crates/agentpm-cli/tests/authoring.rs` drives the compiled CLI to verify all eight
fresh scaffolds pass strict lint with no issues, each accepts arbitrary `standard`
values without emitting the field, and each still rejects unknown root keys.
It asserts matching Python runtime metadata (`type: python`, minimum `version: 3`)
so published scaffolds are discoverable by runtime. It executes the Tool's JSON
echo source, preserves existing source, packages the
scaffold, and rejects missing file/directory/glob-literal entries without leaving a
partial archive. It checks actual lint/publish output equivalence and exact normal
JSON/NDJSON report and issue field sets.

The existing install mock-server test now asserts that `standard` and a future
unknown field survive a real dependency installation and manifest write-back.
The Knowledge build integration test verifies `standard` with the bundled schema,
then preserves both fields using a local schema override for the unknown key.
Knowledge build already writes by default; no new `--write` flag was added.
`serde_json` remains without `preserve_order`.

`lint_output` tests cover every listed union cliff, semantic-first ordering,
duplicate suppression, distinct structural constraints, retained sole structural
errors, Unicode/control-character bounds, and an optional external rule ID.
Custom schemas with references outside `$defs` retain their original validation
diagnostic when human-only expansion cannot resolve the reference.

Retained command output is in
[`assets/milestone-8/lint-before-after.json`](assets/milestone-8/lint-before-after.json).
It includes the complete manifests, commands, exit codes, and before/after stdout
and stderr for the invalid Loop transition, unknown Memory operation, and an
Agent missing `tools`. All exit 1. The Loop now prioritizes semantic errors without
schema object dumps; Memory names `consolidate, transform, delete` once; the Agent
points to `/tools` and `/oneOf/0/required`. Nested human-only expansion does not
change machine issue fields; root kind dispatch intentionally supplies the actual
branch/field diagnostics in all formats.

Release acceptance still open: record the actual CLI version shipping the
permissive `standard` field during Release Band 5. The installed baseline and
unreleased working-tree binary both report `0.1.34`; this evidence is not a claim
that the published `0.1.34` supports the change. No version bump was made here.

Operator walkthrough:
[`manual-tests/CLI_M8_MANUAL_TESTS.md`](manual-tests/CLI_M8_MANUAL_TESTS.md).
`scripts/cli_m8_manual.py setup` prepares gitignored `cli-m8-test/` fixtures;
its eight independent `run` sections use the real compiled CLI and require no
live registry. The install section manages an ephemeral loopback registry,
including a real artifact download/hash check and manifest write-back. Expected
CLI failures are asserted without altering the operator's shell error settings.
Each command retains stdout/stderr and its exact exit code; the summary requires
every expected command in all eight sections from the same binary digest.
The optional blog-sitemap browser check is separate from the CLI summary.
Human readability acceptance remains pending until the operator completes the
runbook's record.

Builder verification of the runbook helper (2026-10-15): `cargo build -p agentpm-cli
--offline` passed; fixture setup, all eight sections (122 CLI/source commands),
and the final summary passed. A summary requested before running any section
correctly exited 1. `python3 -B -m unittest discover -s scripts -p
test_cli_m8_manual.py -v` passed five guard tests, including both directions of
the expected-exit check and incomplete-section rejection. Both helper files
passed Black. The install smoke check used only its temporary localhost server;
no live registry or provider was contacted. These are builder smoke results, not
the operator's readability approval.

### Analytics

Use local/dev PostHog test configuration or event sink abstraction.

Verify:

- intended event emitted;
- unintended payload fields rejected/not sent;
- server events use authoritative completion points;
- billing webhook event mapping is idempotent;
- telemetry opt-out suppresses CLI events;
- anonymous install ID behavior is stable;
- private package identity does not enter telemetry payloads.

### Python dependency schema / lock / runtime environment

Add tests for:

- Python Tool with dependencies accepted.
- Node Tool with dependencies rejected.
- Invalid Python requirement syntax rejected.
- Duplicate/conflicting dependency handling.
- Canonical `PythonResolution` object uses type `agentpm.python-resolution.v1`.
- `PythonResolution` contains `python.requires`, normalized authored `requirements`, and exact `packages`.
- Package entries contain normalized name + exact version + optional normalized PEP 508 marker.
- Resolution ordering is deterministic (`requirements` and `(name, marker-or-empty, version)` package ordering).
- Resolution can represent conditional/environment-marker dependencies without publisher-target leakage.
- Resolution rejects/never records wheel filenames, wheel tags, local paths, publisher target/architecture, interpreter paths, or local environment IDs.
- Equivalent `PythonResolution` values produce identical RFC 8785/JCS bytes/digests in Rust and Python.
- Current v2/v3 lock reads remain valid.
- New capability state writes minimum lockfile version 4.
- Future unsupported lockfile version is rejected before lossy deserialization/rewrite.
- Plain `agentpm install` lock regeneration preserves Python resolution state from the resolve plan.
- Package key remains logical `kind:name@version`, not target-qualified.
- `PackageKind` variants stay in agreement across the CLI lockfile/semver types and the SDK install DTOs; a variant added to one side without the other fails a test rather than passing review.
- New lock records release integrity but no local selected-artifact target/digest.
- Stale manifest/Python resolution mismatch blocks publish.
- Published release/artifact contains Tool-specific Python resolution metadata.
- Managed Python environment:
  - created under `.agentpm/` separate from payload;
  - keyed by Tool/version/target/interpreter identity;
  - exact applicable locked versions installed;
  - new-format Tool executes through managed interpreter;
  - changed `AGENTPM_PYTHON`/interpreter identity cannot silently reuse incompatible env.
- Legacy `_vendor` Tool still uses legacy run behavior.

### Payload compatibility

Fixtures:

- pure Python/data payload → `any`;
- `.so` payload → concrete target;
- `.dylib` payload → concrete target;
- `.pyd`/`.dll`/`.exe` payload → concrete target;
- known unpacked native dependency content → concrete target;
- unknown executable/binary → conservative concrete target;
- declared dependencies alone do not make payload target-specific;
- legacy package remains compatibility unknown/legacy;
- missing declared `files` path fails packaging rather than warning + omission.

Verify canonical target mapping for at least:

- `aarch64-apple-darwin`;
- `x86_64-apple-darwin`;
- `x86_64-unknown-linux-gnu`;
- `x86_64-pc-windows-msvc` where test host/mocking permits.

### Multi-artifact release / publish storage

Automated cases:

- legacy single-artifact publish still finalizes.
- one-artifact new-format release finalizes.
- multiple-target new-format release finalizes.
- release manifest type is exactly `agentpm.package.release.v1` and validates the fixed field contract.
- `pythonResolutionDigest` is omitted when no Python resolution exists and present/bound when one exists.
- release artifact inventory is target-unique and canonically sorted before digesting.
- `PackagePublishRelease` persists in `package_publish_releases`.
- `PackagePublishArtifact` persists in `package_publish_artifacts`.
- `PackageVersionArtifact` persists in `package_version_artifacts`.
- one release reservation owns multiple child artifact uploads.
- concurrent child artifact reservations for one version do not collide with legacy one-pending-upload rule.
- duplicate target rejected.
- mismatched identity/version rejected.
- mismatched manifest digest rejected.
- mismatched Python resolution digest rejected.
- one failed/missing upload leaves release unpublished.
- finalization creates one logical version + N artifact rows atomically.
- legacy `PackageVersion.sha_256/size_bytes/s3_key` rows remain populated/readable.
- new `agentpm.package.release.v1` rows have legacy scalar artifact columns `NULL`, populate the release-level digest field, and use child artifact rows as the only authoritative target-artifact source.
- read paths branch on explicit release format, not on nullable-column heuristics.
- finalized artifact inventory is immutable.
- adding artifact to finalized version rejected.
- expired/failed staged objects can be cleaned up safely.
- legacy S3/read path remains valid.
- new S3 target object keys include canonical target + full digest.
- upload implementation streams rather than buffers complete artifact in memory (unit/integration evidence as appropriate).
- server-authoritative upload-size limit is honored.
- old client/new release produces clear unsupported-release-format error.
- malware scanning:
  - one scan scheduled per target artifact;
  - all-clean state requires appropriate artifact results;
  - infection of any artifact yanks/disables logical version.
- release reservation outlives an individual presigned PUT window.
- a presigned PUT can be re-issued for a still-pending child artifact without invalidating the release or already-uploaded siblings.
- an expired release reservation produces an actionable, identifiable error rather than an opaque S3 or `409` failure.
- worst-case registry call count for a supported multi-target release stays within the effective publish rate-limit budget, including presign re-issues and one retried matrix job.
- an S3 error other than not-found on the finalize destination-exists check fails finalize instead of being treated as "object absent" and triggering a copy; `AccessDenied` specifically is covered.
- resume state is observable by the client — a resumed reservation is distinguishable from a fresh one, or the flag is absent from the response entirely.

### Canonicalization / stored-byte integrity / provenance

Create shared canonicalization fixtures consumed by Rust and Python.

Required cases:

- typed legacy author v1 statement serializes byte-for-byte like current implementation;
- RFC 8785/JCS implementation matches published/known JCS vectors in Rust and Python;
- new canonical key order is deterministic;
- non-ASCII Unicode serializes identically cross-language;
- artifact inventory order is normalized by target before JCS;
- `PythonResolution` array ordering is normalized before JCS;
- whitespace/input map order does not alter canonical bytes;
- new timestamps are normalized to UTC RFC3339 `Z` form before canonicalization;
- release digest matches in Rust/Python.

Stored-byte verification cases:

- client-declared metadata hash that does not match stored bytes cannot finalize a new release;
- stored object size mismatch cannot finalize;
- declared digest == trusted stored digest == release manifest digest on success.

Tamper tests:

- modified target artifact bytes fail artifact integrity;
- modified release manifest fails release integrity;
- changed target/artifact list fails signed release verification;
- changed manifest digest fails;
- changed Python resolution digest fails;
- invalid author signature fails;
- wrong/unregistered author key fails;
- invalid registry attestation fails;
- attestation signed by untrusted registry key fails.

Provenance-policy cases:

- namespace `required` enforces one valid signature.
- `--require-signature` requires one valid signature even under optional/off mode.
- `--require-attestation` requires cryptographic registry attestation, not a boolean.
- explicit `--sign` with a rejected signature fails publish with actionable diagnostic.
- new-format publish fails if registry attestation cannot be created.
- historical signature remains cryptographically valid after normal signer revocation while current revoked state is surfaced separately.
- legacy signature/attestation formats still verify as before.
- normalized key-ID display does not affect verification by full public key.
- `GET /v1/registry/signing-keys` returns type `agentpm.registry.keyset.v1`.
- registry key set is RFC 8785/JCS-canonicalized and root-signed.
- CLI rejects a key set with an invalid root signature.
- CLI rejects an arbitrary attestation signing key delivered outside the verified key set.
- CLI can cache and reuse only a previously root-verified key set.
- unknown `registryKeyId` fails verification.
- retired signing key remains present in the verified key set and can verify an old attestation after rotation.
- root signing key itself is not confused with routine attestation signing keys.
- the currently configured attestation key ID (`apm-prod-1` by default) is present in the published key set.
- an **existing, already-published** version's `agentpm.registry.attestation.v2` row verifies end-to-end through the new client verification path — this is a regression test, since `--require-attestation` passes for those versions today.
- `--require-attestation` succeeds on a legacy single-artifact release whose attestation key was backfilled, and fails on one whose key ID is absent from the key set.
- `GET /v1/registry/signing-keys` fails closed when a valid root signature cannot be produced, rather than returning an unsigned or partial key set.
- the root public key pinned in the CLI matches the key the registry actually signs key sets with (bootstrap-order check, not only unit-level verification).

### Install resolution / cache / extraction

Test target behavior:

- exact target selected when available;
- `any` used when no exact target and portable artifact exists;
- exact target preferred over `any`;
- only selected artifact gets downloaded/presigned;
- no compatible artifact returns actionable failure with available targets;
- headless no-compatible path is deterministic;
- pinned/exact version never silently upgrades;
- unversioned recovery only with explicit interactive consent;
- portable lock remains unchanged across different local target selections;
- local `.agentpm/` state may differ by target/interpreter without touching `agent.lock`;
- cache entries cannot collide across kind/package/version/target/digest;
- cache hit re-verifies digest;
- downloaded size mismatch fails;
- symlink/hardlink archive entry fails explicitly if unsupported;
- traversal protection remains intact;
- decompressed-size cap enforced;
- extraction entry-count cap enforced;
- failed extraction/provisioning does not finalize install session;
- dependency provisioning failure leaves clear recoverable state;
- install session remains valid through dependency provisioning, not only through artifact download;
- a presigned GET expiring mid-install produces an actionable error rather than a bare transport failure;
- download client enforces explicit connect and idle/progress timeouts instead of `reqwest` defaults;
- legacy package follows legacy install path.

### CI / headless signing

Automated or integration cases:

- encrypted AgentPM signing key works without TTY.
- missing key file/passphrase fails cleanly without secret leakage.
- wrong passphrase fails cleanly.
- namespace required-signing policy is satisfied by headless flow.
- package-build command emits machine-readable descriptor with:
  - target;
  - artifact SHA/size;
  - manifest digest;
  - Python resolution digest.
- tag/version mismatch fails workflow before publish.
- matrix artifacts are uploaded/downloaded through CI fan-in.
- final job rejects mismatched manifest/resolution descriptors.
- final job rejects duplicate targets.
- final atomic publish succeeds.
- missing/failed matrix target prevents publish.
- local single-target publish continues to work.

## Manual checks

### Registry

- Search for a query with 3+ pages and manually navigate:
  - Next twice;
  - Previous twice;
  - browser Back/Forward.
- Change filters while on later page and confirm reset to page 1.
- Verify Agent Package Includes AND semantics.
- Verify no-results state is understandable.
- Verify loading text:
  - mixed;
  - specific kind.
- Verify star rail visual transition is subtle and persistent.
- Verify Explore card star counts.
- Verify no empty Pinned section for visitor.
- Verify Owner/Admin empty pin state.
- Verify pinned ordering controls.
- Verify namespace-scoped search looks/behaves like Explore, not a separate system.
- Verify Tool Evaluations/Score & Rating placeholder is gone.
- Verify detail pages retain all specialized tabs.
- Verify a legacy Tool Security tab still renders correctly.
- Verify a new-format Tool Security tab shows release-level integrity/provenance and target-artifact inventory without presenting stars/popularity as trust signals.

### SEO

Using rendered HTML / browser dev tools:

- inspect title/meta/canonical/robots on:
  - landing;
  - pricing;
  - package version;
  - namespace;
  - docs;
  - Explore query URL;
  - 404;
  - authenticated/private route.
- inspect OG/Twitter preview values.
- verify sitemap contains intended public URLs only.

Milestone 7 regression commands:

- In `agentpm-web`: `pnpm test`, `pnpm typecheck`, `pnpm lint`, and `pnpm build`.
  Build in a disposable copy when a development server is active; do not let two processes
  write to the same `.next` directory.
- In `agentpm-api`: `uv run pytest -q`. Set `AGENTPM_SEARCH_TEST_DATABASE_URI` to a local
  Postgres test connection to also run `tests/test_sitemap.py` and the existing search
  behavior fixtures. Sitemap fixtures use transaction-local tables and leave registry data intact.
- Run API pre-commit checks on `app/models/cache.py`, `app/stats/application/stats.py`,
  `app/stats/stats_routes.py`, and `tests/test_sitemap.py`.

Recorded evidence (2026-10-14): `assets/milestone-7/seo-evidence.json` contains 23
production-rendered route inspections without executing JavaScript, plus the sitemap inventory
(54 public/static/docs URLs and 72 public registry namespace/version URLs in the local dataset).
`assets/milestone-7/error-evidence.json` records a fault-injected HTTP 500 response with `noindex`.
The HTML audit covers all eight package kinds, public/private namespaces, private/missing
packages, docs aliases and concrete versions, search/filter URLs, sign-in and the account redirect.
Every successful primary public page has one H1 and no image missing its alt attribute.
The existing share image was checked at 1200x630. Older docs version behavior and private
payload rejection are additionally pinned by unit fixtures; no signed-in private browser session
was used for this audit.

Docs canonical decision: prefer version-specific documentation identity over consolidating
current-guide search signals on the stable `/docs/latest/...` URL. `latest` remains a usable,
shareable navigation alias, but metadata and sitemap entries identify the resolved concrete
version. Historical versions remain independently discoverable rather than pointing at guidance
for a newer release. This is an identity/consistency choice, not a claim of better search ranking.

Accepted tradeoff: when the current docs version changes, the canonical target of a `latest`
link changes too. Search signals may be distributed across version URLs over time instead of
consolidating on an evergreen URL. Milestone 17 should carry this decision forward explicitly.
If evergreen discovery becomes the priority, reconsider canonicalizing both current concrete
pages and their aliases to `/docs/latest/...`, while keeping historical pages self-canonical.
Such a change must update metadata, sitemap selection, tests, and documented policy together;
it should not happen incidentally during docs copy or navigation work.

Verification result: web tests, typecheck, lint and the isolated production build pass;
the complete API suite passes with Postgres fixtures enabled. The full API pre-commit invocation
still reports two existing imported-module mypy errors in `app/tools/application/search.py`
(lines 1486 and 1856), and the pinned Bandit hook lacks `pbr` in its isolated environment.
The four touched Python files pass mypy with imported-module checking suppressed, and pinned
Bandit passes with its missing tool dependencies supplied in a separate temporary environment.
All other applicable pre-commit hooks pass. These existing issues were not changed in Milestone 7.

Deployment note: deploy the registry's `/stats/sitemap` endpoint before or with the web update.
The sitemap index and its bounded registry partitions are generated at request time, not during
the web build. Sitemap responses have a five-minute cache window. Robots directives are not
access controls, and this work does not replace private namespace authorization.

Blog sitemap follow-up (2026-10-15, completed with Milestone 8): the pages partition
now includes individual `/blog/<slug>` URLs from the existing public blog feed,
in addition to `/blog`. URL components are encoded and the combined pages partition
retains the 50,000-entry guard. A failed or malformed blog feed fails sitemap generation
rather than publishing a successful but incomplete sitemap; a genuinely empty feed remains
valid. The blog index's existing empty-list fallback is unchanged.

Verification: all 182 web tests passed, including nine sitemap tests covering article
inclusion, encoded URLs in the emitted XML, valid empty feeds, and failed/malformed feeds.
`pnpm typecheck`, Prettier checks, and the isolated production `pnpm build` passed.
`pnpm lint` passed with six existing unused-variable warnings in untouched files.
The build required network access for the existing Google Font and was run in a disposable
copy so the live development `.next` directory was not disturbed. Applicable API docs
pre-commit checks passed for the sitemap documentation update. No registry API or CLI
behavior changed in this follow-up.

### CLI

Run representative commands in a real terminal and compare overall tone/shape:

- `agentpm init --kind tool`
- `agentpm lint`
- `agentpm publish --dry-run`
- `agentpm knowledge inspect`
- `agentpm memory inspect`
- registry-backed connection failure
- `agentpm keys export --key-id bad`
- `agentpm serve`

Confirm errors answer:
- what happened;
- where;
- why;
- what to do next.

### Analytics / feedback

- Inspect actual PostHog event payloads in dev.
- Confirm excluded fields never appear.
- Toggle telemetry off and rerun CLI milestones.
- Submit feedback form.
- Verify optional email and follow-up consent behavior.
- Verify reproducible-bug GitHub path.
- Exercise pricing → checkout click.
- Replay a Lemon Squeezy webhook in test/dev and verify authoritative event.

### Python portability

On at least:

- Apple Silicon macOS (`aarch64-apple-darwin`);
- Intel macOS (`x86_64-apple-darwin`);
- Linux x86_64 (`x86_64-unknown-linux-gnu`);

verify a new-format Python Tool whose dependency graph includes at least one package with native wheels can:

- resolve portable exact dependency state;
- package/publish under the new model;
- install on each target;
- select the correct target artifact or `any`;
- create an AgentPM-managed Python environment for that target/interpreter;
- install the correct target-compatible dependency distributions;
- run successfully through `agentpm run`.

Also verify:

- the same source-controlled `agent.lock` is usable across at least two target platforms without target-specific edits;
- changing the local selected artifact does not rewrite target information into the lock;
- changing `AGENTPM_PYTHON` to another compatible minor version does not silently reuse an incompatible native dependency environment;
- a legacy vendored Python Tool still installs/runs under legacy semantics;
- a local pure-source publish can claim `any`;
- a local native publish claims only the concrete current target;
- a multi-target CI publish produces one immutable logical version with multiple artifacts;
- adding a target requires a new Tool version.

### Integrity / provenance

Manually inspect a new release:

- canonical release manifest;
- artifact inventory;
- per-artifact digests;
- release digest;
- Python resolution digest;
- author signature v2;
- registry attestation v3;
- registry key ID/trust material;
- local selected artifact/runtime state under `.agentpm/`;
- `agent.lock` confirming no local target/artifact selection is persisted.

Attempt install with:

- valid author signature/attestation;
- author signature absent under optional policy;
- namespace required signature;
- `--require-signature`;
- cryptographic `--require-attestation`;
- signer that has since been normally revoked;
- intentionally corrupted local cache/artifact;
- tampered release manifest in a test fixture;
- no compatible target.

Confirm failure messages distinguish:

- artifact integrity failure;
- release-integrity failure;
- author-signature failure;
- registry-attestation trust/signature failure;
- compatibility failure.

### CI publish

Run or inspect a real GitHub Actions release workflow showing:

- tag matches `agent.json.version`;
- each matrix job runs package/build only;
- `actions/upload-artifact` produces per-target build outputs;
- final job uses `actions/download-artifact`;
- final job performs one AgentPM publish;
- encrypted key + noninteractive passphrase path works when signing;
- `AGENTPM_TOKEN` is not exposed in logs;
- no partial release is visible if one matrix target fails.

## Expected evidence

Codex should report back:

- all automated commands run and their status;
- test counts or relevant suite summary;
- the final search relevance fixture results;
- example corrected lint output before/after;
- example pagination API responses for Page 1/2/3;
- screenshots of:
  - Explore filters;
  - no-results state;
  - star rail;
  - namespace pins;
  - namespace-scoped search;
  - feedback UI;
- representative SEO metadata output;
- representative PostHog event payload with sensitive/excluded fields absent;
- example new `agent.lock` v4 entry showing the exact `agentpm.python-resolution.v1` shape + logical release integrity and **no** local target selection;
- example canonical `PythonResolution` JSON + JCS digest;
- example machine-local runtime/install state showing selected target/artifact/interpreter fingerprint;
- example `agentpm.package.release.v1` release manifest + JCS bytes/digest;
- example release reservation + child artifact DB/object inventory showing `package_publish_releases`, `package_publish_artifacts`, and `package_version_artifacts`;
- evidence that new-format `PackageVersion` legacy scalar artifact columns are null while legacy rows remain populated;
- example S3/object inventory including canonical target IDs and `release.json`;
- byte-compatibility evidence for typed legacy v1 author statement;
- example new release-level author-signature v2 and registry-attestation v3 statements;
- example trusted stored-byte checksum verification evidence;
- example `agentpm.registry.keyset.v1` response, root signature verification, and registry signing-key history/retirement evidence;
- cross-platform publish/install matrix results;
- legacy Python Tool compatibility result;
- GitHub Actions run link or captured job output if available;
- anything that could not be verified and why.

## Out of scope

The test plan does not require:

- Stage 2 category-copy redesign;
- Agent → Agent Package full terminology migration;
- Package Health UX;
- semantic/embedding search;
- session replay;
- recommendation engine;
- ratings/reviews;
- universal quality scoring;
- broad eval framework;
- Rosetta/emulation compatibility;
- multiple CI providers;
- OIDC/keyless signing;
- general-purpose AgentPM Developer workflows.
