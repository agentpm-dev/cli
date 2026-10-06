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

## Python dependency contract

- Confirm Python dependency declarations are optional and Python-only.
- Confirm Node Tools reject Python dependency fields.
- Confirm dependency syntax is linted.
- Confirm AgentPM owns resolution and does not require the user's project to be a uv project.
- Confirm external requirements/pyproject/Poetry/uv lock files did not become competing authoritative sources.
- Confirm `agent.lock` remains the user-facing resolved-state file.
- Confirm new publish artifacts carry Tool-specific resolved dependency state.
- Confirm stale lock/manifest mismatch blocks publish.
- Confirm legacy `_vendor` packages remain valid.
- Confirm new-format vendoring produces at least a portability warning where intended.

## Payload compatibility

- Confirm pure Python Tool payload can be classified portable.
- Confirm native payload detection is conservative.
- Confirm declared dependencies do not incorrectly make Tool payload target-specific.
- Confirm unknown binary payload is not falsely labeled `any`.
- Confirm legacy packages do not receive invented compatibility claims.

## Multi-artifact release model

- Confirm one version can reference multiple immutable target artifacts.
- Confirm all artifacts share identical logical manifest/dependency-lock state.
- Confirm target-specific metadata is the only allowed divergence.
- Confirm duplicate targets are rejected.
- Confirm S3 storage avoids one giant all-platform tarball.
- Confirm release inventory is immutable after finalize.
- Confirm adding architecture requires new version.
- Confirm failed uploads cannot create visible partial releases.
- Confirm staged/orphan cleanup exists.

## Integrity and signing

- Confirm release manifest binds:
  - identity;
  - version;
  - manifest digest;
  - dependency-lock digest;
  - complete target artifact inventory;
  - artifact digests.
- Confirm release digest uses formally specified canonical serialization.
- Confirm Rust/Python canonicalization fixtures match.
- Confirm Unicode behavior is deterministic.
- Confirm artifact ordering is deterministic.
- Confirm author signature binds release-level integrity.
- Confirm registry attestation binds release-level integrity.
- Confirm legacy signing remains supported.
- Confirm selected artifact SHA-256 is still recomputed at install.
- Confirm client actually verifies provenance cryptographically rather than trusting API booleans.
- Confirm `--require-signature` or equivalent enforcement behaves correctly.
- Confirm signer revocation semantics are explicit and tested.
- Confirm server-side stored-byte verification is implemented if included by the spec; if not, confirm it is explicitly deferred rather than accidentally omitted.

## Target-aware install

- Confirm exact compatible artifact selection.
- Confirm `any` fallback is only used for truly portable artifacts.
- Confirm incompatible artifact is never silently installed.
- Confirm exact lockfile-pinned version is never silently changed.
- Confirm interactive recovery requires explicit user action.
- Confirm noninteractive mode is deterministic.
- Confirm no-compatible-artifact error lists requested and available targets.
- Confirm Python dependencies are resolved for the target environment.
- Confirm legacy install path remains intact.

## Headless signing and CI

- Confirm signing can run without TTY.
- Confirm CI secrets are not printed.
- Confirm namespace required-signing policy works in CI.
- Confirm matrix jobs only build artifacts.
- Confirm one final job performs the atomic publish.
- Confirm final publish rejects mismatched build inputs.
- Confirm the GitHub Action does not hide capabilities unavailable to other CI providers.
- Confirm local single-target publish still works.

## Regressions

- Check install, publish, lint, run, new, export, namespace, keys, knowledge, memory, serve, login-adjacent behavior for unintended changes.
- Check package detail pages for all kinds.
- Check private namespaces.
- Check old Python Tools with vendored dependencies.
- Check old single-artifact packages and signatures.
- Check old lockfiles.
- Check existing signing mode `off | optional | required`.
- Check existing malware scan/yank flow.
- Check existing package integrity verification and cache re-verification.
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
- Reuse existing publish init/finalize concepts where they fit rather than building a separate unrelated release pipeline.
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
- lockfile migration;
- `any` portability misclassification;
- atomic publish semantics;
- S3/DB divergence;
- canonical serialization;
- release-level signing;
- client-side provenance verification;
- legacy package compatibility;
- CI secret handling.

Do not approve based only on happy-path demos. The central Stage 1 promise is that AgentPM feels dependable before early traffic increases.
