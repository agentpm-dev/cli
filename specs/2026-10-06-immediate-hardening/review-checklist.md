# Review Checklist

## Contract surfaces

- Confirm public APIs, schemas, manifests, lockfiles, route/output shapes, S3 layout, release metadata, and signing statements changed only where intended by `spec.md`.
- Confirm legacy API/package/install contracts remain supported where explicitly required.
- Confirm `agent.lock` v3 remains readable.
- Confirm lockfile v4 changes are additive and intentional.
- Confirm legacy single-artifact signature/attestation statements remain valid.
- Confirm new release-level signature/attestation versions are clearly distinguished.
- Confirm Template execution-surface schema includes `agentpm-harness` without breaking existing values.
- Confirm docs were updated for any user-facing behavior change.

## Scope discipline

- Confirm Stage 1 did not drift into Stage 2 category/IA redesign.
- Confirm no broad Agent → Agent Package global rename was introduced as incidental cleanup.
- Confirm no AI/embedding/LLM search dependency was introduced.
- Confirm no ratings/reviews/social system was added beyond stars.
- Confirm no universal quality score/eval framework was introduced.
- Confirm GitHub Actions is a thin client of provider-neutral CLI/protocol behavior.
- Confirm local publishing remains supported.

## Registry correctness

- Verify page-size frontend/backend contract is unified.
- Verify page counts are correct.
- Verify Page 2 can return to Page 1.
- Verify >10-page backward navigation.
- Verify query/kind/sort/filter state survives navigation.
- Verify mixed namespace/package result pagination.
- Verify strict/relaxed search does not corrupt totals or cursor behavior.
- Verify private results behave consistently across supported sorts.
- Verify filter changes reset pagination.
- Verify Back/Forward restores URL state.

## Search and filters

- Confirm filters apply server-side before sorting/pagination.
- Confirm namespace filtering is typeahead-based.
- Confirm OR-within / AND-across semantics.
- Confirm Agent Package Includes uses AND semantics.
- Confirm kind-specific filters appear only where intended.
- Confirm no Profile/Loop-specific filters were added without justification.
- Confirm dependency identity search is very low weight.
- Confirm exact/direct matches still outrank dependent matches.
- Confirm full README bodies and arbitrary manifest content were not indexed.
- Confirm search fixture tests represent meaningful relevance behavior rather than brittle exact rank snapshots.

## Trending and stars

- Confirm trending signal computation covers the full eligible result set.
- Confirm top-N truncation is only presentation, not canonical trend data.
- Confirm low-activity ranking does not collapse meaningfully to alphabetical order.
- Confirm stars are identity-level.
- Confirm one star per user/artifact identity.
- Confirm private star data cannot leak private package existence.
- Confirm star timestamps are stored.
- Confirm stars are not presented as quality/health scores.
- Confirm Tool Score & Rating placeholder is removed.
- Confirm Tool Evaluations placeholder is hidden/removed until meaningful.

## Namespace behavior

- Confirm pins are Owner/Admin-managed.
- Confirm pins are namespace-owned identities only.
- Confirm pin maximum and ordering.
- Confirm empty pins are hidden from visitors.
- Confirm Owner/Admin sees useful empty-state management.
- Confirm namespace discovery reuses global Explore search/filter/sort/pagination infrastructure.
- Confirm namespace constraint cannot be escaped through filter/query params.

## Package detail shell

- Confirm star UI is shared across all kinds.
- Confirm stars remain stable while changing selected version.
- Confirm version-specific security/integrity remains version-specific.
- Confirm weekly `-100%` style signal is removed/replaced.
- Confirm useful specialized tabs and inspection surfaces were not lost.
- Confirm README content remains treated as author content.
- Confirm new-format Tool Security/detail data exposes release-level integrity/provenance and target-artifact inventory while legacy single-artifact versions still render correctly.
- Confirm this Security expansion did not become an unplanned Stage 2 Package Health/quality score.

## SEO

- Confirm important public pages have deliberate titles/descriptions.
- Confirm canonical behavior:
  - package versions;
  - docs versions;
  - landing/pricing;
  - namespaces.
- Confirm arbitrary Explore search/filter URLs are not creating indexable crawl explosion.
- Confirm private/auth/404/error surfaces are not indexed.
- Confirm sitemap excludes private/internal routes.
- Confirm OG/Twitter metadata is valid.
- Confirm technical SEO changes did not prematurely encode Stage 2 terminology decisions.

## CLI scaffolding

- Confirm every `init` kind is immediately lint-valid.
- Confirm Tool scaffold is minimal but actually valid.
- Confirm scaffold changes do not pull in unnecessary framework boilerplate.

## CLI lint quality

- Confirm semantic/domain errors are shown before generic schema noise.
- Confirm parent oneOf/anyOf suppression does not hide the only useful error.
- Confirm duplicate `/properties` / `/dependentSchemas` errors are gone where equivalent.
- Confirm huge object echoes are bounded.
- Confirm closed-union errors identify useful expected alternatives.
- Confirm JSON/NDJSON output remains stable.
- Confirm publish validation uses the same lint-rendering behavior.

## CLI error consistency

- Confirm registry transport failures use one shared friendly formatter.
- Confirm raw underlying errors remain available only where useful/debuggable.
- Confirm `whoami` exits non-zero on failure.
- Confirm inspect output is consistent across sibling commands.
- Confirm paths are normalized.
- Confirm `new` success is script/user-friendly.
- Confirm clap owns argument errors such as missing `--mcp`.
- Confirm invalid key-id messaging is concise and does not expose unnecessary home paths.

## Analytics/privacy

- Confirm PostHog instrumentation is intentionally scoped.
- Confirm broad autocapture/session replay is not enabled.
- Confirm authoritative server events are emitted from completion/truth points.
- Confirm billing lifecycle events derive from Lemon Squeezy webhook truth.
- Confirm CLI telemetry has clear default-on disclosure and opt-out.
- Confirm telemetry uses a strict allowlist.
- Confirm no prompts, Tool I/O, Harness conversations, file contents, paths, env vars, secrets, package contents, or private package identities are sent.
- Confirm Privacy Policy was updated.
- Confirm feedback UI is AgentPM-owned and PostHog-backed.
- Confirm feedback prompts focus on user goal/obstacle rather than only satisfaction scoring.

## Python dependency contract / runtime environment

- Confirm Python dependency declarations are optional and Python-only.
- Confirm Node Tools reject Python dependency fields.
- Confirm dependency syntax/conflicts are linted.
- Confirm portable resolution does not lock publisher-selected wheels/architecture.
- Confirm conditional/environment-marker dependency branches are represented deterministically where needed.
- Confirm AgentPM owns resolver acquisition/versioning and does not require the user's project to be a uv project.
- Confirm external requirements/pyproject/Poetry/uv lock files did not become competing authoritative sources.
- Confirm `agent.lock` remains the user-facing portable resolved-state file.
- Confirm new Tool resolution state survives plain full-regeneration `agentpm install`.
- Confirm managed Python environment is:
  - separate from extracted Tool payload;
  - keyed by Tool/version/target/interpreter identity;
  - populated with exact applicable locked dependencies;
  - used by `agentpm run` for new-format Tools.
- Confirm changed interpreter cannot silently reuse an incompatible environment.
- Confirm legacy `_vendor` packages and current PATH/`AGENTPM_PYTHON` execution remain valid.
- Confirm new-format vendoring produces a portability warning where intended.

## Lockfile evolution

- Confirm current v2/v3 lockfiles remain readable.
- Confirm capability-requiring locks use minimum version 4.
- Confirm version choice is implemented deliberately (for example `minimum_lock_version`) rather than blindly creating a new enum arm.
- Confirm future lockfile versions are rejected **before** normal deserialization/rewrite can drop unknown fields.
- Confirm Harness and other consumers do not accept arbitrary future versions merely because version >= 2.
- Confirm package key remains `kind:name@version`, with no target dimension.
- Confirm one logical package entry represents one release.
- Confirm `LockedPackage.integrity` means logical release integrity for new-format releases.
- Confirm selected target/artifact digest is **not** persisted in portable `agent.lock`.
- Confirm Python resolution/new per-package fields are generated by `lock_from_plan`/equivalent and cannot disappear on plain install.
- Confirm `LockedPackage` was shaped once for both Milestone 11 and Milestone 14 rather than reshaped twice, and that no target/selected-artifact/interpreter field leaked into the portable lock.
- Confirm the two `PackageKind` enumerations are either unified or guarded by a test that fails on divergence, rather than left to manual sync.

## Payload compatibility

- Confirm canonical target IDs include ABI/libc-relevant identity, not only OS/CPU.
- Confirm `any` is used only for safely portable payloads.
- Confirm native/binary detection is conservative.
- Confirm declared dependencies do not make Tool payload target-specific.
- Confirm embedded `.whl` did not become the dependency-distribution mechanism.
- Confirm missing declared `manifest.files` paths fail packaging instead of silently omitting files.
- Confirm legacy packages do not receive invented compatibility claims.

## Multi-artifact release / storage / scanning

- Confirm logical `PackageVersion` owns N target artifact rows rather than N versions.
- Confirm legacy `Upload`/single-artifact path remains readable and is not ambiguously reused for concurrent targets.
- Confirm new publish uses one release reservation with child artifact-upload state.
- Confirm all artifacts share identical logical manifest/Python-resolution state.
- Confirm target-specific payload metadata is the only intended divergence.
- Confirm duplicate targets are rejected.
- Confirm new S3 keys include canonical target and full digest.
- Confirm release metadata is stored both as normalized state and immutable release manifest if specified.
- Confirm publishing streams large artifacts instead of buffering the entire body.
- Confirm server upload-size policy is authoritative and client/server caps are no longer contradictory.
- Confirm release inventory is immutable after finalize.
- Confirm adding architecture requires a new semantic version.
- Confirm failed uploads cannot create visible partial releases.
- Confirm staged/orphan cleanup exists.
- Confirm unsupported old clients fail clearly on new release format.
- Confirm every target artifact receives malware-scan handling.
- Confirm any infected target yanks/disables the logical version.
- Confirm one clean artifact cannot make an incompletely scanned release look wholly clean.
- Confirm the release reservation lifetime is chosen deliberately and outlives any single presigned PUT window, rather than inheriting the existing 900-second presign.
- Confirm a presigned PUT can be re-issued for a pending child artifact without invalidating the release or already-uploaded siblings.
- Confirm reservation expiry surfaces as an identifiable, actionable error rather than an opaque S3 or `409` response.
- Confirm publish rate limits were audited against worst-case multi-target CI usage including presign re-issues and retries.
- Confirm registry-API timeouts are separated from artifact-transfer timeouts rather than one total-duration cap covering the S3 PUT.
- Confirm the finalize destination-exists check distinguishes genuine not-found from other S3 errors, and that `AccessDenied` no longer reads as "object absent".
- Confirm resume state is either surfaced in the client's typed response or removed from the wire, with no documented-but-ignored field, and that release-level resume is visible to the client.

## Integrity and signing

- Confirm legacy author signature v1 has a typed representation whose serialization is byte-identical to existing signatures.
- Confirm release manifest binds:
  - identity/version/release format;
  - manifest digest;
  - Python resolution digest;
  - complete target artifact inventory;
  - per-artifact digests/sizes.
- Confirm release digest uses formally specified canonical serialization.
- Confirm Rust/Python canonicalization fixtures match including Unicode.
- Confirm artifact array ordering is deterministic.
- Confirm author signature v2 binds release-level integrity.
- Confirm registry attestation v3 binds release-level integrity.
- Confirm legacy author v1 / registry attestation v2 remain supported.
- Confirm selected artifact SHA-256 is still recomputed at install.
- Confirm new finalize verifies a trustworthy stored-byte checksum, not just publisher-set S3 metadata.
- Confirm explicit `--sign` cannot silently succeed unsigned if validation fails.
- Confirm new-format registry attestation failure prevents release finalization.
- Confirm client verifies provenance cryptographically rather than trusting API booleans/counts.
- Confirm `--require-signature` requires at least one valid trusted author signature.
- Confirm `--require-attestation` means cryptographically valid trusted registry attestation.
- Confirm Stage 1 did not accidentally implement dynamic N-of-M signing; effective required threshold remains one.
- Confirm full public keys, not truncated key IDs, drive author verification.
- Confirm Rust/Python display key-ID mismatch is normalized.
- Confirm registry public-key history exists and supports historical attestation verification.
- Confirm official-registry trust does not blindly accept an arbitrary key delivered adjacent to its signature.
- Confirm ordinary signer revocation does not retroactively invalidate a historically valid signature; current revoked state remains observable.

## Target-aware install / extraction

- Confirm install detects canonical current target.
- Confirm install obtains/downloads only one selected target artifact.
- Confirm exact target is preferred over `any`.
- Confirm `any` fallback is only used for truly portable payload.
- Confirm incompatible artifact is never silently installed.
- Confirm exact lockfile-pinned version is never silently changed.
- Confirm interactive recovery requires explicit user action.
- Confirm noninteractive mode is deterministic.
- Confirm no-compatible-artifact error lists requested/available targets.
- Confirm selected target/artifact/runtime fingerprint is local `.agentpm/` state, not portable lock state.
- Confirm cache naming/identity cannot collide across kind/package/version/target/digest.
- Confirm cache hits are re-verified.
- Confirm downloaded size is checked where practical.
- Confirm unsafe symlink/hardlink archive entries no longer disappear silently.
- Confirm traversal protection remains.
- Confirm decompressed-byte and entry-count extraction limits exist.
- Confirm failed extraction/dependency provisioning cannot be reported as completed install.
- Confirm Python environment is provisioned before install completion.
- Confirm the install session stays valid through dependency provisioning, not only through artifact download.
- Confirm a presigned GET expiring mid-install produces an actionable error.
- Confirm the download client sets explicit connect and idle/progress timeouts instead of using `reqwest` defaults.
- Confirm legacy install path remains intact.

## Headless signing and CI

- Confirm headless signing uses encrypted AgentPM key material rather than preferring raw private-key secrets.
- Confirm key/passphrase can be supplied without TTY.
- Confirm CI secrets/passphrases/decrypted keys are not printed.
- Confirm AgentPM's own CI matrix was extended beyond `ubuntu-latest` so target detection, payload classification, extraction, and runtime provisioning are actually exercised on macOS and Windows.
- Confirm no `#[cfg(unix)]`-gated test is the sole coverage for behavior that also has a Windows path.
- Confirm namespace required-signing policy works in CI.
- Confirm provider-neutral package-build command outputs target/digest/manifest-resolution descriptor.
- Confirm Git tag is checked against `agent.json.version`.
- Confirm matrix jobs build/package only and use CI artifact transport.
- Confirm final fan-in job downloads all artifacts and performs one atomic publish.
- Confirm final publish rejects mismatched manifest/Python-resolution state and duplicate targets.
- Confirm a failed matrix target prevents publish.
- Confirm the GitHub workflow does not hide capabilities unavailable to other CI providers.
- Confirm local single-target publish still works.
- Confirm Stage 1 CI auth uses supported PAT/`AGENTPM_TOKEN` flow.
- Confirm OIDC trusted publishing was not accidentally pulled into Stage 1.

## Regressions

- Check install, publish, lint, run, new, export, namespace, keys, knowledge, memory, serve, login-adjacent behavior for unintended changes.
- Check package detail pages for all kinds.
- Check private namespaces.
- Check old Python Tools with vendored dependencies.
- Check old single-artifact packages and signatures.
- Check old lockfiles.
- Check unsupported future lockfile is rejected before any rewrite.
- Check new source-controlled lock is identical across target artifact selection where logical resolution is identical.
- Check existing signing mode `off | optional | required`.
- Check existing malware scan/yank flow.
- Check existing package integrity verification and cache re-verification.
- Check legacy author-signature v1 and registry-attestation v2 verification byte compatibility.
- Check new per-artifact malware scan/yank behavior does not regress the legacy scan path.
- Check current `AGENTPM_PYTHON` legacy runner behavior for Tools without declared dependencies.
- Check existing Homebrew/local CLI install expectations if bundling/managing uv changes binary packaging.

## Tests and verification

- Confirm work was verified according to `test-plan.md`.
- Confirm new behavior has automated tests where practical.
- Confirm search relevance has deterministic fixtures.
- Confirm pagination has multi-page regression tests.
- Confirm security-sensitive release/signing code has tamper tests.
- Confirm cross-language canonicalization uses shared vectors.
- Confirm multi-platform behavior was tested on real runners, not only mocked architecture strings.
- Confirm legacy compatibility was actually exercised.
- Confirm any unverifiable item is called out explicitly.

## Pattern adherence

- Check existing repo patterns before introducing new abstractions.
- Reuse existing publish init/finalize route/service patterns where they fit, but do not force the one-object legacy `Upload` model to represent a multi-artifact release.
- Keep legacy and new release-format branches explicit; avoid heuristic interpretation of scalar `PackageVersion` artifact fields.
- Reuse existing search service/query infrastructure for namespace search.
- Reuse shared package-detail shell for stars/popularity.
- Reuse existing connection-error helpers rather than duplicating wrappers.
- Keep new release/integrity types versioned and explicit.
- Justify any new dependency, binary-bundling mechanism, DB subsystem, or storage abstraction.
- Avoid GitHub-specific concepts in core registry/release schemas.

## Notes for reviewer

Pay particular attention to these high-risk areas:

- cursor pagination correctness;
- search relevance/noise;
- private visibility leaks;
- telemetry privacy;
- lockfile forward-version safety and lossless migration;
- `any` portability misclassification;
- atomic publish semantics;
- release-session / S3 / DB divergence;
- canonical serialization;
- release-level signing;
- client-side provenance verification and registry-key trust/rotation;
- legacy package compatibility;
- managed Python environment/interpreter drift;
- extraction safety/size limits;
- CI secret handling.

Do not approve based only on happy-path demos. The central Stage 1 promise is that AgentPM feels dependable before early traffic increases.
