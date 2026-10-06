# Tasks

## Milestone 1: Registry Discovery Correctness

- [ ] Fix Explore page-size mismatch:
  - [ ] establish one authoritative page size;
  - [ ] send the effective limit to the backend;
  - [ ] derive displayed page counts from the same contract.
- [ ] Fix cursor history so Page 2 can return to Page 1.
- [ ] Remove/raise the `HISTORY_MAX = 10` limitation so sequential Previous navigation can return through the full result set.
- [ ] Add regression coverage for:
  - [ ] 0 results;
  - [ ] less than one page;
  - [ ] exactly one page;
  - [ ] page-size + 1;
  - [ ] 2 pages;
  - [ ] 3+ pages;
  - [ ] >10 pages;
  - [ ] 1 → 2 → 3 → 2 → 1.
- [ ] Verify mixed namespace/package cursor pagination remains stable.
- [ ] Fix private-result inconsistency across Relevance/Newest versus Trending/Most Downloaded.
- [ ] Test and harden strict/relaxed relevance behavior so totals/page counts do not become misleading mid-pagination.
- [ ] Replace `Loading tools...` with:
  - [ ] `Searching AgentPM…` for mixed searches;
  - [ ] kind-specific loading text for filtered searches.
- [ ] Add explicit no-results UI:
  - [ ] preserve query;
  - [ ] clear filters;
  - [ ] search all kinds where applicable.
- [ ] Add visible recoverable search-error state.
- [ ] Replace unstable array-index React keys on search result cards with stable entity IDs where applicable.
- [ ] Verify query/kind/sort/filter changes reset pagination.
- [ ] Verify browser Back/Forward restores Explore state.

## Milestone 2: Faceted Filters and Search Foundations

- [ ] Move Kind into the Filters area and remove the disconnected kind-tab treatment if the final UX confirms this direction.
- [ ] Replace `Filters: Coming soon` with working filters.
- [ ] Implement URL-addressable filter state.
- [ ] Implement universal filters:
  - [ ] Kind;
  - [ ] Namespace typeahead;
  - [ ] License;
  - [ ] Signed;
  - [ ] Updated/published recency.
- [ ] Implement Tool-specific Runtime filter.
- [ ] Implement Agent Package Includes filter:
  - [ ] Skills;
  - [ ] Knowledge;
  - [ ] Memory;
  - [ ] Profiles;
  - [ ] Loop;
  - [ ] MCP.
- [ ] Implement Template filters:
  - [ ] Stack;
  - [ ] Execution Surface.
- [ ] Implement Skill Runtime Compatibility filter.
- [ ] Implement Knowledge Mode filter.
- [ ] Implement Memory Supports Retrieval filter.
- [ ] Leave Profile and Loop without kind-specific v1 filters.
- [ ] Implement OR-within/AND-across filter-group semantics.
- [ ] Implement AND semantics for selected Agent Package Includes values.
- [ ] Apply filters server-side before ranking/sorting/pagination.
- [ ] Ensure relaxed relevance never admits results outside hard filter constraints.
- [ ] Build the faceted-query layer so new Stage 2 objective filters can be added without ad hoc branching.
- [ ] Add `agentpm-harness` to Template execution-surface schema.
- [ ] Update lint/schema tests for `agentpm-harness`.
- [ ] Update relevant example Templates/manifests to declare `agentpm-harness`.

## Milestone 3: Search Relevance Expansion

- [ ] Extend the search materialized/indexed document with selected lower-weight semantic fields.
- [ ] Preserve strongest weighting for:
  - [ ] name;
  - [ ] namespace;
  - [ ] top-level description.
- [ ] Add lower-weight Agent Package semantic fields.
- [ ] Add very-low-weight dependency/component package identity matches.
- [ ] Add Template:
  - [ ] display name;
  - [ ] use case;
  - [ ] stack;
  - [ ] execution surfaces.
- [ ] Add Skill descriptive compatibility/runtime metadata.
- [ ] Add Knowledge:
  - [ ] content type;
  - [ ] language;
  - [ ] mode;
  - [ ] document descriptions.
- [ ] Add Memory:
  - [ ] space descriptions;
  - [ ] record type descriptions;
  - [ ] operation descriptions;
  - [ ] retrieval modes.
- [ ] Add Profile:
  - [ ] role;
  - [ ] expertise;
  - [ ] objectives;
  - [ ] audience description.
- [ ] Add Loop:
  - [ ] archetype;
  - [ ] phase objectives;
  - [ ] outcome descriptions.
- [ ] Keep trigram fuzzy matching focused on names/namespaces.
- [ ] Do not index full READMEs or arbitrary manifest bodies.
- [ ] Create a fixed search relevance fixture set.
- [ ] Include fixture cases for:
  - [ ] exact package name;
  - [ ] exact namespace;
  - [ ] typo;
  - [ ] direct name vs dependency-only match;
  - [ ] metadata-only match;
  - [ ] multi-word query;
  - [ ] filtered query;
  - [ ] no-result query.
- [ ] Assert direct package identity matches outrank dependency-only matches.
- [ ] Document weighting rationale in code/tests.

## Milestone 4: Stars, Trending, and Popularity Signals

- [ ] Add star persistence model:
  - [ ] user;
  - [ ] artifact identity;
  - [ ] `created_ts`;
  - [ ] unique constraint per user/identity.
- [ ] Add star/unstar API.
- [ ] Enforce authorization/visibility for private artifacts.
- [ ] Add aggregate star count to relevant registry/search DTOs.
- [ ] Add passive star counts to Explore and namespace result cards.
- [ ] Add detail-page interactive star rail to the shared package/component header.
- [ ] Implement starred/unstarred visual states with subtle transition.
- [ ] Remove Tool-only `Score & rating / Coming Soon` UI.
- [ ] Remove/hide Tool Evaluations tab while it has no real content.
- [ ] Replace weekly `0 / -100%` style popularity presentation with:
  - [ ] total installs;
  - [ ] stars;
  - [ ] another agreed durable signal.
- [ ] Refactor trending signal computation so it is computed for the full eligible result set.
- [ ] Separate top-N presentation shortlist from trend signal computation.
- [ ] Remove top-12-per-kind truncation from the canonical Explore trending source.
- [ ] Implement Trending ranking:
  - [ ] installs last 7d;
  - [ ] total stars;
  - [ ] all-time installs;
  - [ ] latest publish;
  - [ ] stable tie-breaker.
- [ ] Add star timestamps even if recent-star scoring is not used yet.
- [ ] Add tests for low-activity tie behavior.
- [ ] Verify private stars never affect public discovery/trending.

## Milestone 5: Namespace Curation and Scoped Discovery

- [ ] Implement namespace pinned artifacts.
- [ ] Allow Owner/Admin management only.
- [ ] Limit pins to namespace-owned visible artifacts.
- [ ] Enforce pin maximum (default target: 6 unless changed).
- [ ] Add explicit pin ordering.
- [ ] Hide empty pin section from normal visitors.
- [ ] Show empty-state management prompt to Owner/Admin.
- [ ] Render pinned items for visitors when pins exist.
- [ ] Replace namespace `0/week` popularity signals with shared improved result-card signals.
- [ ] Add namespace-scoped text search.
- [ ] Reuse shared faceted filters within namespace package list.
- [ ] Reuse shared sorts and pagination.
- [ ] Enforce namespace as hard search constraint.
- [ ] Remove/simplify current mechanical type summary where redundant with real filtering.
- [ ] Verify namespace discovery and global Explore produce consistent result behavior for equivalent constraints.

## Milestone 6: Package Detail Shared-Shell Hardening

- [ ] Implement shared star rail across all artifact kinds.
- [ ] Replace weak weekly change signal in shared sidebar/shell.
- [ ] Preserve identity-level stars while version selector changes.
- [ ] Preserve version-specific Security/integrity data.
- [ ] Remove Tool Score & Rating placeholder.
- [ ] Hide Tool Evaluations placeholder.
- [ ] Verify all existing kind-specific tabs still work:
  - [ ] Agent Package Bindings/Examples;
  - [ ] Memory Lifecycle/Record Contracts;
  - [ ] Loop Phases;
  - [ ] Security;
  - [ ] README;
  - [ ] Knowledge inspection metadata;
  - [ ] Template bootstrap metadata;
  - [ ] Profile detail metadata;
  - [ ] Skill detail metadata.
- [ ] Verify no shared-shell change conflates identity-level and version-level data.

## Milestone 7: Registry Technical SEO

- [ ] Audit and implement unique `<title>` and meta description for:
  - [ ] landing;
  - [ ] pricing;
  - [ ] package/version pages;
  - [ ] namespace pages;
  - [ ] docs pages.
- [ ] Add/verify canonical tags.
- [ ] Make immutable package-version pages canonical to themselves.
- [ ] Define and implement docs-version canonical behavior.
- [ ] Add/verify OpenGraph metadata.
- [ ] Add/verify Twitter metadata.
- [ ] Add/verify share image behavior.
- [ ] Add sitemap coverage for intended public pages.
- [ ] Audit robots directives.
- [ ] Set arbitrary Explore query/filter result pages to `noindex,follow`.
- [ ] Prevent private/authenticated surfaces from indexing.
- [ ] Prevent 404/error pages from indexing.
- [ ] Check heading hierarchy and basic image alt/accessibility metadata.
- [ ] Add regression tests where practical.
- [ ] Leave category-facing metadata copy refinement to Stage 2.

## Milestone 8: CLI Scaffolding and Lint Quality

- [ ] Make `agentpm init --kind tool` scaffold lint-valid output.
- [ ] Prefer a minimal runnable Tool stub rather than an intentionally invalid placeholder.
- [ ] Verify all eight `init` kinds lint clean immediately.
- [ ] Refactor human lint rendering:
  - [ ] semantic/domain errors first;
  - [ ] suppress redundant parent oneOf/anyOf errors when covered;
  - [ ] deduplicate `/properties/<kind>` and `/dependentSchemas/<kind>` duplicates;
  - [ ] truncate/bound instance echoes;
  - [ ] print expected discriminator values for closed unions where practical.
- [ ] Add test coverage across known oneOf/anyOf cliffs:
  - [ ] packageRef;
  - [ ] memoryTrigger;
  - [ ] memoryOperation;
  - [ ] loopTransitionTarget;
  - [ ] loopToolFailurePolicy;
  - [ ] knowledgeMetadata;
  - [ ] agentMemoryBinding;
  - [ ] entrypoint.command.
- [ ] Keep JSON/NDJSON lint output contracts stable unless explicitly versioned.
- [ ] Make publish validation use the same human lint renderer, including contextual header.
- [ ] Add ordering tests ensuring actionable semantic messages appear before generic schema fallback.

## Milestone 9: CLI Error and Success Consistency

- [ ] Centralize network/transport error formatting.
- [ ] Apply friendly connection errors to:
  - [ ] install;
  - [ ] export;
  - [ ] new;
  - [ ] whoami;
  - [ ] other matching registry-backed commands.
- [ ] Fix `whoami` to exit non-zero on network failure.
- [ ] Normalize Knowledge/Memory inspect header structure.
- [ ] Normalize freshness/status label.
- [ ] Normalize paths before printing.
- [ ] Reduce unnecessary absolute path verbosity in normal human output.
- [ ] Preserve richer paths in verbose/machine output where useful.
- [ ] Improve Memory missing-sidecar wording.
- [ ] Make `new` success output include created target/path and useful next step.
- [ ] Move `serve --mcp` requirement into clap argument validation.
- [ ] Improve `keys export <bad-id>` to concise actionable message.
- [ ] Preserve `1` runtime / `2` clap usage exit-code convention.
- [ ] Add regression tests for all corrected exit codes/messages.

## Milestone 10: Analytics, Billing Funnel, Privacy, and Feedback

- [ ] Create PostHog project/config integration.
- [ ] Disable/avoid broad autocapture if it would violate the explicit event/property contract.
- [ ] Implement initial web/registry events.
- [ ] Implement server-authoritative events for:
  - [ ] completed installs;
  - [ ] publish completion;
  - [ ] account creation where available;
  - [ ] stars;
  - [ ] subscription lifecycle.
- [ ] Add `pricing_viewed` and `checkout_started`.
- [ ] Map Lemon Squeezy webhook events to:
  - [ ] subscription started;
  - [ ] subscription updated;
  - [ ] subscription cancelled;
  - [ ] payment failed where supported.
- [ ] Keep Lemon Squeezy as billing source of truth.
- [ ] Add minimal CLI telemetry:
  - [ ] first-use milestone;
  - [ ] Harness started;
  - [ ] Harness completed.
- [ ] Generate/store anonymous install ID.
- [ ] Add strict telemetry property allowlist.
- [ ] Add `AGENTPM_TELEMETRY=0` support.
- [ ] Add persistent config opt-out if existing config architecture supports it.
- [ ] Ensure telemetry never sends:
  - [ ] prompts;
  - [ ] Tool I/O;
  - [ ] Harness conversation content;
  - [ ] paths;
  - [ ] env vars;
  - [ ] secrets;
  - [ ] package contents;
  - [ ] private package identities.
- [ ] Update Privacy Policy.
- [ ] Create Early Product Funnel PostHog dashboard.
- [ ] Add AgentPM-branded feedback entry in shared site/docs/registry chrome.
- [ ] Back feedback storage/reporting with PostHog survey capability.
- [ ] Capture:
  - [ ] what user was trying to do;
  - [ ] what got in the way/surprised them;
  - [ ] optional email;
  - [ ] follow-up consent.
- [ ] Add GitHub Issue branch/link for reproducible bugs.
- [ ] Do not enable session replay in Stage 1.

## Milestone 11: Python Tool Dependency Contract

- [ ] Extend Tool runtime schema with optional Python `dependencies` when `runtime.type == "python"`.
- [ ] Reject `dependencies` for Node Tools.
- [ ] Add Python requirement syntax validation.
- [ ] Add duplicate/conflict validation.
- [ ] Update docs/examples away from default `_vendor` guidance for new-format Python Tools.
- [ ] Preserve `_vendor` compatibility for legacy Tools.
- [ ] Warn when a new-format Tool both declares dependencies and vendors likely dependency content.
- [ ] Choose/integrate AgentPM-managed `uv` execution strategy.
- [ ] Do not require user projects to use uv.
- [ ] Do not make external requirements/pyproject/Poetry/uv lock files authoritative in Stage 1.
- [ ] Resolve exact dependency versions through AgentPM.
- [ ] Extend `agent.lock` to v4 with optional Python resolved-dependency state.
- [ ] Preserve lockfile v3 reads.
- [ ] Add clear unsupported-new-lockfile handling for older CLI paths where relevant.
- [ ] Ensure publish fails if declared dependencies and resolved lock state disagree.
- [ ] Ensure published Tool artifact carries Tool-specific resolved dependency metadata independently of publisher workspace state.

## Milestone 12: Python Tool Artifact Compatibility

- [ ] Define platform/architecture target tuple representation.
- [ ] Define portable `any` representation.
- [ ] Implement conservative payload classification.
- [ ] Detect common native payload indicators:
  - [ ] `.so`;
  - [ ] `.dylib`;
  - [ ] `.pyd`;
  - [ ] native executables;
  - [ ] platform-specific vendored wheel contents.
- [ ] Ensure declared Python dependencies do not force Tool payload to platform-specific.
- [ ] Default unknown native/binary payload to conservative current target.
- [ ] Add compatibility metadata to build/publish artifact descriptor.
- [ ] Preserve legacy Tool behavior with compatibility unknown/unspecified.
- [ ] Add tests for pure Python vs native payload classification.

## Milestone 13: Multi-Artifact Release and S3 Model

- [ ] Define release manifest schema/version.
- [ ] Define per-artifact metadata:
  - [ ] target;
  - [ ] object key;
  - [ ] SHA-256;
  - [ ] size;
  - [ ] build/package format metadata.
- [ ] Define registry DB representation for one version → many artifacts.
- [ ] Define S3 key layout for release metadata and per-target artifacts.
- [ ] Preserve legacy single-artifact S3 layout/read path.
- [ ] Ensure each target artifact is independently self-describing.
- [ ] Include logical manifest and Tool-specific dependency-lock metadata in target artifacts.
- [ ] Validate all artifacts in one release share:
  - [ ] kind;
  - [ ] name;
  - [ ] version;
  - [ ] manifest digest;
  - [ ] dependency-lock digest.
- [ ] Reject duplicate/conflicting targets.
- [ ] Add staging/unpublished upload state.
- [ ] Implement cleanup strategy for failed/orphaned staged objects.
- [ ] Implement atomic release finalize.
- [ ] Prevent partially uploaded versions from becoming visible/installable.
- [ ] Enforce immutable artifact inventory after finalize.
- [ ] Enforce new version requirement for adding targets.

## Milestone 14: Release Integrity and Provenance Upgrade

- [ ] Define canonical release manifest content.
- [ ] Define release-level SHA-256 digest.
- [ ] Define canonical serialization contract.
- [ ] Implement canonicalizer in Rust.
- [ ] Implement canonicalizer in Python/server.
- [ ] Add shared cross-language canonicalization test vectors.
- [ ] Introduce new author-signature statement version that binds release digest/manifest.
- [ ] Introduce new registry-attestation statement version that binds release digest/manifest.
- [ ] Preserve legacy signature/attestation verification.
- [ ] Keep per-artifact SHA-256 verification.
- [ ] Evolve lock entry integrity semantics for new releases:
  - [ ] release-level integrity;
  - [ ] selected-artifact target;
  - [ ] selected-artifact integrity.
- [ ] Add actual client-side author-signature verification.
- [ ] Add actual client-side registry-attestation verification.
- [ ] Add `--require-signature`-style install enforcement.
- [ ] Preserve namespace signing-mode behavior.
- [ ] Define signer-revocation handling for historical signatures.
- [ ] Preferably independently hash uploaded S3 object bytes during/around finalize.
- [ ] Ensure actual stored-byte digest matches declared digest and release manifest reference.
- [ ] Add tests for tampering:
  - [ ] artifact bytes changed;
  - [ ] artifact list changed;
  - [ ] target metadata changed;
  - [ ] manifest digest changed;
  - [ ] dependency lock changed;
  - [ ] signature statement changed.

## Milestone 15: Target-Aware Installation

- [ ] Detect current OS/architecture.
- [ ] Read new release artifact inventory.
- [ ] Select exact compatible artifact first.
- [ ] Fall back to `any` where compatible.
- [ ] Verify release-level integrity.
- [ ] Verify selected artifact SHA-256.
- [ ] Resolve/install locked Python dependency versions for target environment.
- [ ] Preserve legacy install path for legacy releases.
- [ ] Add actionable no-compatible-artifact error:
  - [ ] requested target;
  - [ ] available targets.
- [ ] Implement interactive recovery only for supported alternatives.
- [ ] Do not silently install incompatible artifact.
- [ ] Do not silently change a locked exact version.
- [ ] For unversioned/non-pinned requests, optionally offer compatible newer version if product flow can support it safely.
- [ ] In headless mode, fail deterministically with structured error.
- [ ] Do not add Rosetta/emulation handling in this milestone.

## Milestone 16: Headless Signing and GitHub Actions Publishing

- [ ] Add secure noninteractive signing mechanism.
- [ ] Preserve encrypted-at-rest local-key model where possible.
- [ ] Document CI secret handling.
- [ ] Ensure headless publish can satisfy namespace signing mode.
- [ ] Add/reuse CLI artifact-build command/path suitable for CI.
- [ ] Build initial GitHub Actions example/action for Python Tools.
- [ ] Support initial runner matrix:
  - [ ] macOS arm64;
  - [ ] macOS x86_64;
  - [ ] Linux x86_64.
- [ ] Add Linux arm64 if practical without blocking initial feature.
- [ ] Matrix jobs build target artifacts only.
- [ ] Final job downloads all build artifacts.
- [ ] Final job performs one atomic AgentPM publish.
- [ ] Ensure final publish rejects artifacts built from mismatched manifest/dependency state.
- [ ] Keep workflow provider-neutral at protocol/CLI layer.
- [ ] Verify local single-target publish remains supported.
- [ ] Add end-to-end CI test/example package if feasible.

## Milestone 17: Documentation and Migration Hardening

- [ ] Update Python Tool authoring docs:
  - [ ] declare dependencies in `agent.json`;
  - [ ] AgentPM-managed resolution;
  - [ ] portable pure-Python behavior;
  - [ ] native target behavior;
  - [ ] local single-target publish;
  - [ ] CI multi-target publish.
- [ ] Mark `_vendor` approach as legacy/manual and explain portability caveats.
- [ ] Document legacy package compatibility.
- [ ] Document artifact immutability/new-version requirement for added targets.
- [ ] Document install target selection and failure behavior.
- [ ] Document integrity/signature model at a developer-appropriate level.
- [ ] Document telemetry/privacy contract and opt-out.
- [ ] Update CLI help text where new commands/options are introduced.
- [ ] Update registry/docs examples to use `agentpm-harness` Template execution surface where relevant.
- [ ] Ensure Stage 2 terminology work remains separate.

