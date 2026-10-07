# Tasks

## Milestone 1: Registry Discovery Correctness
> Scope note: establish a correct, predictable Explore/search navigation baseline before adding new discovery capabilities. This milestone fixes page-size/cursor/history/state defects, visibility inconsistencies, and basic loading/empty/error behavior. It does not add new filters, expand indexed search fields, change ranking semantics, add stars/trending logic, redesign namespace discovery, or perform Stage 2 category/terminology work.
> Implementation notes:
> - The current pagination bug has two confirmed root causes:
>   1. the frontend renders `Pagination limit={20}` but `getSearchResults()` forwards no `limit`, so the API uses the backend default of 30;
>   2. `_make_next_bundle()` only pushes a previous seek into history when it is truthy, so the initial Page-1 `seek = None` is never represented and Page 2 cannot produce a backend `prev_cursor` to Page 1.
> - The current parent results component renders pagination only when `next_cursor || prev_cursor` is truthy. Once Page 2 reaches backend EOF and also lacks a valid `prev_cursor`, the entire paginator disappears.
> - `HISTORY_MAX = 10` is incompatible with the UX promise of normal sequential Previous/Next pagination if users can navigate beyond ten pages.
> - Keep the existing mixed `all` search architecture (independent package + namespace streams merged into one page) unless a concrete correctness issue requires a rewrite; it is complex because the problem is genuinely multi-stream.
> - Relevance currently performs a strict FTS pass and may switch to a relaxed trigram pass when results are sparse. Pagination/totals must remain stable enough that users do not see impossible page counts or lose continuation state.
> - Search state should remain URL-driven. Do not introduce hidden client-only pagination/filter state that breaks refresh, deep links, or browser navigation.

- [ ] Fix Explore page-size mismatch:
  - [ ] choose one authoritative page size for Explore;
  - [ ] ensure the frontend sends that limit explicitly to `/search`;
  - [ ] derive displayed page counts from the same effective limit;
  - [ ] avoid duplicating the page-size constant in multiple layers where possible.
- [ ] Fix cursor history so Page 2 can return to Page 1:
  - [ ] represent the initial/start-of-results state explicitly in cursor history or otherwise make Page 1 a valid previous destination;
  - [ ] do not rely only on the current frontend special case that strips the cursor to return to Page 1.
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
- [ ] Verify the API only emits `next_cursor` when at least one stream can truly advance and only emits `prev_cursor` when a real previous page exists.
- [ ] Verify mixed namespace/package cursor pagination remains stable:
  - [ ] package-only contribution on a page;
  - [ ] namespace-only contribution on a page;
  - [ ] both streams contributing;
  - [ ] one stream reaching EOF before the other.
- [ ] Fix private-result inconsistency across Relevance/Newest versus Trending/Most Downloaded:
  - [ ] apply the same authorized-private visibility rule to every supported sort;
  - [ ] preserve anonymous/public behavior.
- [ ] Test and harden strict/relaxed relevance behavior so totals/page counts do not become misleading mid-pagination:
  - [ ] avoid switching candidate-set semantics in a way that invalidates an existing cursor;
  - [ ] if strict/relaxed mode becomes part of the cursor contract, encode it explicitly rather than inferring it independently on each page.
- [ ] Replace `Loading tools...` with:
  - [ ] `Searching AgentPM…` for mixed searches;
  - [ ] kind-specific loading text for filtered searches.
- [ ] Add explicit no-results UI:
  - [ ] preserve the user’s current query text;
  - [ ] show `Clear filters` when filters are active;
  - [ ] show `Search all kinds` when a kind constraint is active;
  - [ ] do not introduce the future AgentPM Developer prompt CTA in Stage 1.
- [ ] Add visible recoverable search-error state:
  - [ ] distinguish transport/server failure from a legitimate zero-result query;
  - [ ] provide a retry/recovery affordance where appropriate.
- [ ] Replace unstable array-index React keys on search result cards with stable entity IDs where applicable.
- [ ] Verify query/kind/sort/filter changes reset pagination to page 1 rather than attempting to reuse a stale cursor.
- [ ] Verify browser Back/Forward restores:
  - [ ] query;
  - [ ] kind;
  - [ ] sort;
  - [ ] filters;
  - [ ] cursor/page position.

## Milestone 2: Faceted Filters and Search Foundations
> Scope note: replace the unfinished filter surface with a reusable server-side faceted-search foundation and add the first objective universal and kind-specific filters. This milestone also adds `agentpm-harness` as a Template execution surface because it is needed by the new Template filter model. It does not expand full-text relevance fields, add AI/semantic search, implement stars/trending, redesign package cards, or introduce Stage 2 Package Health filters.
> Implementation notes:
> - Treat text search and filters as one backend query. Text determines match/relevance; filters determine eligibility. Do not fetch a page of text results and then filter them client-side.
> - All active filters must be URL-addressable so searches are shareable, refresh-safe, Back/Forward-safe, measurable, and reusable later by AgentPM Developer.
> - Default filter semantics:
>   - OR within a normal multi-select group;
>   - AND across different groups;
>   - Agent Package `Includes` is intentionally stricter: multiple selected component types mean the Agent Package must include all selected types.
> - The current package-kind tabs under the search bar feel disconnected from the filter model. Move Kind into the filter surface unless implementation review finds a strong UX reason to retain the tabs.
> - Namespace filtering must scale to many namespaces. Use typeahead/search-backed selection, not a giant static dropdown.
> - Do not expose a field merely because it exists in `agent.json`; filters should correspond to natural discovery questions for the best-fit initial user.
> - Stage 2 may add objective Package Health facets later. Structure the backend/filter representation so adding those does not require another query-system rewrite.
> - `agentpm-harness` should become a real Template execution-surface enum rather than overloading `agentpm-run` when Harness is the intended execution path.

- [ ] Move Kind into the Filters area and remove the disconnected kind-tab treatment if the final UX confirms this direction.
- [ ] Replace `Filters: Coming soon` with working filters.
- [ ] Implement a reusable filter-query representation shared by frontend and backend rather than one-off query parameters handled independently in multiple code paths.
- [ ] Implement URL-addressable filter state:
  - [ ] preserve multiple selected values;
  - [ ] ensure deterministic serialization/order where practical;
  - [ ] clear stale cursor whenever filters change.
- [ ] Implement universal filters:
  - [ ] Kind;
  - [ ] Namespace typeahead:
    - [ ] query namespaces incrementally;
    - [ ] allow selecting at least one namespace;
    - [ ] support multiple namespaces if the shared filter model already supports it cleanly;
  - [ ] License;
  - [ ] Signed;
  - [ ] Updated/published recency.
- [ ] Define initial Updated/published recency buckets in one place; recommended starting options:
  - [ ] past 30 days;
  - [ ] past 90 days;
  - [ ] past year.
- [ ] Implement Tool-specific Runtime filter:
  - [ ] Python;
  - [ ] Node.
- [ ] Implement Agent Package Includes filter:
  - [ ] Skills;
  - [ ] Knowledge;
  - [ ] Memory;
  - [ ] Profiles;
  - [ ] Loop;
  - [ ] MCP.
- [ ] Define `Includes` from the authored/resolved Agent composition model rather than from incidental README text.
- [ ] Implement Template filters:
  - [ ] Stack;
  - [ ] Execution Surface.
- [ ] Implement Skill Runtime Compatibility filter using the closed runtime compatibility enum only.
- [ ] Implement Knowledge Mode filter:
  - [ ] context;
  - [ ] vector.
- [ ] Implement Memory Supports Retrieval filter:
  - [ ] key;
  - [ ] filter;
  - [ ] chronological;
  - [ ] full text;
  - [ ] semantic.
- [ ] Leave Profile and Loop without kind-specific v1 filters.
- [ ] Implement OR-within/AND-across filter-group semantics.
- [ ] Implement AND semantics for selected Agent Package Includes values.
- [ ] Apply filters server-side before:
  - [ ] relevance ranking;
  - [ ] non-relevance sorting;
  - [ ] pagination.
- [ ] Ensure strict and relaxed relevance passes use the same hard filter predicate and never broaden outside the user-selected filter set.
- [ ] Ensure authorized-private visibility is applied before filters/sort and remains consistent across all sorts.
- [ ] Build the faceted-query layer so new Stage 2 objective filters can be added without ad hoc branching.
- [ ] Add `agentpm-harness` to Template execution-surface schema.
- [ ] Update lint/schema tests for:
  - [ ] accepting `agentpm-harness`;
  - [ ] continuing to accept existing execution surfaces;
  - [ ] rejecting unknown execution surfaces.
- [ ] Update relevant example Templates/manifests to declare `agentpm-harness` when Harness is genuinely the intended execution surface.

## Milestone 3: Search Relevance Expansion
> Scope note: increase deterministic search recall by indexing selected user-meaningful semantic metadata at deliberately lower weights while preserving direct identity matches as the strongest signals. This milestone includes a fixed relevance fixture set to guard against noisy regressions. It does not index full READMEs/arbitrary manifest text, add embeddings or LLM reranking, change the faceted-filter contract, or build recommendations.
> Implementation notes:
> - Keep the current PostgreSQL FTS + trigram architecture. This milestone is an evolution of the existing weighted materialized search index, not a search-engine replacement.
> - Preserve the current intent that identity-level matches dominate:
>   - package/component name strongest;
>   - namespace strong;
>   - top-level description lower;
>   - richer kind-specific semantic metadata lower still;
>   - dependency/component identities very low weight.
> - A search for a dependency such as `github-tool` should be able to surface an Agent Package that depends on it, but the actual `github-tool` result must clearly outrank that dependent Agent Package.
> - Keep typo/fuzzy trigram behavior focused on package names and namespace handles. Do not fuzzy-match every long descriptive field.
> - Avoid indexing full README bodies: authored prose contains many incidental terms and is likely to create noisy matches.
> - Prefer indexing human-meaningful fields over implementation plumbing such as paths, hashes, raw schemas, generated metadata, and internal IDs.
> - The relevance fixture set should assert important relationships/ordering properties, not freeze every exact numeric rank forever.

- [ ] Extend the search materialized/indexed document with selected lower-weight semantic fields.
- [ ] Preserve strongest weighting for:
  - [ ] name;
  - [ ] namespace;
  - [ ] top-level description.
- [ ] Define explicit additional weighted search vector(s) or equivalent structure so lower-weight semantic fields are not accidentally promoted to the same relevance class as top-level description.
- [ ] Add lower-weight Agent Package semantic fields:
  - [ ] authored Examples prompt titles/text where available;
  - [ ] useful composition/binding labels that help discovery;
  - [ ] avoid indexing internal phase/binding IDs unless they carry real user meaning.
- [ ] Add very-low-weight dependency/component package identity matches:
  - [ ] Tools;
  - [ ] Skills;
  - [ ] Knowledge;
  - [ ] Memory;
  - [ ] Profiles;
  - [ ] Loop;
  - [ ] MCP-bound Tool package identities where available.
- [ ] Add Template:
  - [ ] display name;
  - [ ] use case;
  - [ ] stack;
  - [ ] execution surfaces.
- [ ] Add Skill descriptive compatibility/runtime metadata:
  - [ ] controlled runtime labels;
  - [ ] other descriptive fields only where they are sufficiently normalized to be useful.
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
- [ ] Do not index:
  - [ ] full READMEs;
  - [ ] arbitrary manifest bodies;
  - [ ] schema blobs;
  - [ ] hashes;
  - [ ] generated build metadata;
  - [ ] package-relative file paths as general discovery text.
- [ ] Create a fixed search relevance fixture set using representative artifacts across multiple kinds.
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
- [ ] Assert lower-weight semantic metadata can make an otherwise-unfindable relevant artifact discoverable without displacing obvious identity matches.
- [ ] Assert result ordering is deterministic under ties using stable tie-breakers.
- [ ] Document weighting rationale in code/tests so future changes do not accidentally flatten the intended hierarchy.

## Milestone 4: Stars, Trending, and Popularity Signals
> Scope note: add lightweight ecosystem popularity/engagement primitives and make Trending useful in a low-activity registry. This milestone introduces identity-level stars, replaces weak weekly-change presentation, and separates full-result-set trend computation from top-N presentation. It does not add ratings, reviews, comments, social feeds, universal quality scoring, or Stage 2 Package Health semantics.
> Implementation notes:
> - A star belongs to the package/component identity, not a specific version.
> - Users may star their own artifacts.
> - Private artifacts may be starred only by users authorized to see them, and private star activity must never leak the existence of that artifact into public discovery.
> - Store `created_ts` even if Stage 1 only uses total stars. This preserves the option to introduce recent-star signals later without a schema migration.
> - The current `trending_tools` view only keeps the top 12 per kind. That truncation may remain useful for homepage shortlists, but it must not be the canonical source for Explore’s Trending sort.
> - The current score is effectively `installs_last_7d` with alphabetical fallback because ratings are hardcoded to zero. Replace that behavior with an explainable low-activity fallback.
> - Recommended Trending ordering is lexicographic and deterministic:
>   1. installs last 7 days;
>   2. total stars;
>   3. all-time installs;
>   4. latest publish timestamp;
>   5. stable ID/slug.
> - Do not treat stars as quality, trust, or Package Health.
> - The preferred detail-page star interaction is a full-width footer rail integrated into the main identity card, with a subtle starred-state transition and no gamification.
> - M4 owns the star data/API/trending primitive and shared star UI component. Milestone 6 applies/verifies that primitive across every detail-page kind.

- [ ] Add star persistence model:
  - [ ] user/account identity;
  - [ ] artifact identity;
  - [ ] `created_ts`;
  - [ ] unique constraint per user/identity.
- [ ] Add star/unstar API:
  - [ ] idempotent enough for normal optimistic UI behavior;
  - [ ] correct authorization for public/private artifacts.
- [ ] Enforce authorization/visibility for private artifacts.
- [ ] Add aggregate star count to relevant registry/search DTOs and materialized/search data needed for sorting.
- [ ] Add current-user `starred` state to detail/API responses where needed for interactive UI.
- [ ] Add passive star counts to Explore and namespace result cards.
- [ ] Add reusable detail-page interactive star rail component:
  - [ ] unstarred state;
  - [ ] starred state;
  - [ ] immediate count update;
  - [ ] subtle border/background/accent transition;
  - [ ] no confetti/bounce/social gamification.
- [ ] Remove Tool-only `Score & rating / Coming Soon` UI.
- [ ] Remove/hide Tool Evaluations tab while it has no real content.
- [ ] Replace weekly `0 / -100%` style popularity presentation with durable signals:
  - [ ] total installs;
  - [ ] stars;
  - [ ] do not show a misleading percentage delta when the base period is zero.
- [ ] Refactor trending signal computation so it is computed for the full eligible result set.
- [ ] Separate top-N presentation shortlist from trend signal computation.
- [ ] Remove top-12-per-kind truncation from the canonical Explore trending source.
- [ ] Preserve or recreate a top-N-per-kind query/view only for homepage/curated presentation where useful.
- [ ] Implement Trending ranking:
  - [ ] installs last 7d;
  - [ ] total stars;
  - [ ] all-time installs;
  - [ ] latest publish;
  - [ ] stable tie-breaker.
- [ ] Keep the Trending implementation understandable in code; avoid introducing a black-box weighted score unless required by existing query structure.
- [ ] Add star timestamps even if recent-star scoring is not used yet.
- [ ] Add tests for:
  - [ ] all-zero recent activity;
  - [ ] equal recent installs but different stars;
  - [ ] equal stars but different all-time installs;
  - [ ] full tie resolved by freshness/stable ID.
- [ ] Verify private stars never affect public discovery/trending.

## Milestone 5: Namespace Curation and Scoped Discovery
> Scope note: turn namespace pages into curated and scalable discovery surfaces by adding owner/admin pins and reusing the shared Explore search/filter/sort/pagination foundation under a hard namespace constraint. This milestone does not create a separate namespace search implementation, redesign namespace taxonomy, add Stage 2 category hierarchy, or broaden namespace permissions beyond what pin management requires.
> Implementation notes:
> - Pins are explicit namespace-owner/admin curation, not an algorithmic recommendation surface.
> - Pin the package/component identity, not one version.
> - Only artifacts owned by the namespace are eligible for pins.
> - Recommended maximum is 6 unless an existing layout constraint suggests a different small fixed number.
> - Visitor behavior when there are zero pins: render nothing.
> - Owner/Admin behavior when there are zero pins: show a management-only empty state such as “Pin packages to highlight them here.”
> - Namespace discovery should call the same underlying search/filter service as `/explore` with an enforced namespace constraint; do not fork ranking/filter/pagination logic.
> - The namespace page can use a more compact UI than Explore, but the query semantics and result-card behavior should remain shared.

- [ ] Implement namespace pinned artifacts.
- [ ] Add persistence for pin identity and explicit order.
- [ ] Allow Owner/Admin management only.
- [ ] Limit pins to namespace-owned visible artifacts.
- [ ] Enforce pin maximum (default target: 6 unless changed).
- [ ] Add explicit pin ordering:
  - [ ] drag/drop if it fits current UI patterns cleanly; or
  - [ ] deterministic move-up/down/order controls.
- [ ] Ensure deleting/unpublishing/yanking an artifact does not leave a broken visible pin:
  - [ ] either remove invalid pins automatically; or
  - [ ] suppress them and expose cleanup to Owner/Admin.
- [ ] Hide empty pin section from normal visitors.
- [ ] Show empty-state management prompt to Owner/Admin.
- [ ] Render pinned items for visitors when pins exist.
- [ ] Reuse shared result/card primitives for pinned items where practical rather than creating a visually unrelated card system.
- [ ] Replace namespace `0/week` popularity signals with shared improved result-card signals from Milestone 4.
- [ ] Add namespace-scoped text search.
- [ ] Reuse shared faceted filters within namespace package list:
  - [ ] omit Namespace selector because the namespace is already fixed;
  - [ ] retain Kind and applicable universal/kind-specific filters.
- [ ] Reuse shared sorts and pagination.
- [ ] Enforce namespace as a server-side hard search constraint that cannot be overridden by query params.
- [ ] Keep namespace filter/search state URL-addressable.
- [ ] Remove/simplify current mechanical type summary where redundant with real filtering.
- [ ] Verify namespace discovery and global Explore produce consistent result behavior for equivalent constraints.

## Milestone 6: Package Detail Shared-Shell Hardening
> Scope note: apply shared package/component detail-page hardening consistently across all kinds while preserving the useful specialized inspection tabs already present. This milestone adds the shared star treatment, removes unfinished placeholder UI, and improves popularity signals. It does not redesign the detail-page information architecture, migrate Agent/Agent Package terminology, change authored README content, or implement Stage 2 Package Health presentation.
> Implementation notes:
> - Every kind currently uses essentially the same identity/version shell. Make shared changes once in that shell rather than patching each kind independently.
> - Keep identity-level and selected-version data conceptually separate:
>   - identity-level: star state/count, long-lived popularity, name/namespace;
>   - version-level: selected version, publish timestamp, SHA-256, signatures, registry attestation, malware result, version-specific manifest metadata.
> - Changing the selected version must not make the package appear newly starred/unstarred because the star belongs to identity.
> - Security remains version-specific and should not be reinterpreted as an identity-level badge in Stage 1.
> - Do not remove useful specialized views while cleaning up the shared shell.
> - README content is author-published. Do not rewrite or normalize README wording as part of this milestone.

- [ ] Implement the Milestone-4 shared star rail across all artifact kinds.
- [ ] Place the star rail consistently at the bottom of the primary identity card unless the existing shared layout requires a small adaptation.
- [ ] Replace weak weekly change signal in shared sidebar/shell with the durable popularity treatment established in Milestone 4.
- [ ] Preserve identity-level stars while version selector changes.
- [ ] Preserve version-specific Security/integrity data.
- [ ] Remove Tool Score & Rating placeholder.
- [ ] Hide Tool Evaluations placeholder/tab until actual evaluation content exists.
- [ ] Ensure removing placeholder sections does not leave empty cards, spacing gaps, or dead navigation states.
- [ ] Verify install/use action areas remain kind-appropriate:
  - [ ] Tool install/run/load behavior;
  - [ ] Agent Package install/load behavior unchanged in Stage 1;
  - [ ] Template bootstrap flow;
  - [ ] Knowledge inspect/query flow;
  - [ ] Memory/Profile/Loop/Skill component actions.
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
- [ ] Keep the shared Security/detail contract extensible for the new release model implemented later in Stage 1; when Milestones 13–14 land, update the same surface rather than introducing a separate new-format-only page.
- [ ] Verify Security tab continues to display the selected version’s:
  - [ ] package/artifact type;
  - [ ] digest;
  - [ ] author-signature state;
  - [ ] registry-attestation state;
  - [ ] malware status;
  - [ ] license.
- [ ] After Milestones 13–14, ensure new-format Tool versions additionally expose, without Package Health scoring:
  - [ ] release-level digest;
  - [ ] author-signature/registry-attestation state;
  - [ ] available target-artifact inventory and per-artifact digest/scan state where useful;
  - [ ] while legacy versions continue rendering their existing single-artifact Security data.
- [ ] Verify no shared-shell change conflates identity-level and version-level data.
- [ ] Do not add the Stage-2 `Run with Harness` Agent Package action in this milestone; record that as Stage 2 work only.

## Milestone 7: Registry Technical SEO
> Scope note: establish the technical SEO/indexing baseline required before intentionally generating traffic: metadata uniqueness, canonical behavior, social previews, sitemap/robots boundaries, and crawl control. This milestone does not rewrite category-facing copy, perform keyword/content marketing strategy, add broad structured-data/schema.org work, or make Stage 2 information-architecture decisions.
> Implementation notes:
> - This milestone is about technical correctness and crawl/index boundaries, not final category messaging.
> - Keep package-version pages indexable and self-canonical because published versions are immutable, meaningful artifacts that may be linked directly.
> - Avoid an infinite crawl surface from `/explore?q=...&filters...`; arbitrary query/filter combinations should be `noindex,follow` and canonically point to `/explore` unless a future deliberate landing page is created.
> - Public namespace pages are meaningful indexable ecosystem pages.
> - Authenticated/private/account/settings surfaces must not be indexable.
> - Docs use a `/docs/latest/...` model; define one deliberate canonical strategy so the same article is not unintentionally indexed under multiple version aliases.
> - Do not use this milestone to force the new Stage-2 category vocabulary into titles/descriptions prematurely.

- [ ] Audit and implement unique `<title>` and meta description for:
  - [ ] landing;
  - [ ] pricing;
  - [ ] package/version pages;
  - [ ] namespace pages;
  - [ ] docs pages.
- [ ] Ensure dynamic package metadata derives from public package/version data server-side and does not require client hydration to become crawlable.
- [ ] Ensure namespace metadata derives from public namespace data server-side.
- [ ] Add/verify canonical tags.
- [ ] Make immutable package-version pages canonical to themselves.
- [ ] Define and implement docs-version canonical behavior:
  - [ ] decide whether `/latest/` is canonical or points to a concrete docs version;
  - [ ] ensure old versions do not duplicate-current content unintentionally.
- [ ] Add/verify OpenGraph metadata:
  - [ ] title;
  - [ ] description;
  - [ ] canonical URL;
  - [ ] image where available.
- [ ] Add/verify Twitter metadata.
- [ ] Add/verify share image behavior for landing, package, namespace, pricing, and docs surfaces where practical.
- [ ] Add sitemap coverage for intended public pages:
  - [ ] landing;
  - [ ] pricing;
  - [ ] docs;
  - [ ] public namespaces;
  - [ ] public package/version pages.
- [ ] Audit robots directives.
- [ ] Set arbitrary Explore query/filter result pages to `noindex,follow`.
- [ ] Canonicalize arbitrary Explore query/filter URLs to `/explore` unless a deliberate indexable facet is introduced later.
- [ ] Prevent private/authenticated/account/settings surfaces from indexing.
- [ ] Prevent 404/error pages from indexing.
- [ ] Check heading hierarchy:
  - [ ] one meaningful H1 per page where appropriate;
  - [ ] logical H2/H3 structure.
- [ ] Check basic image alt/accessibility metadata on primary public pages.
- [ ] Add regression tests for metadata/canonical/robots behavior where practical.
- [ ] Leave category-facing metadata copy refinement to Stage 2.

## Milestone 8: CLI Scaffolding and Lint Quality
> Scope note: fix the highest-impact CLI creation and linting rough edges so a newly scaffolded artifact is valid and lint output prioritizes actionable domain errors over schema noise. This milestone preserves machine-readable lint contracts and focuses on validation/rendering quality. It does not redesign all CLI output, add new runtime behavior, change AgentPM package semantics, or perform Stage 2 terminology migration.
> Implementation notes:
> - Current confirmed first-run defect: `agentpm init --kind tool` scaffolds `files: []`, `entrypoint.command: ""`, and `entrypoint.args: []`, then immediately fails `agentpm lint`; the other seven kinds lint clean.
> - Prefer generating a minimal valid/runnable Tool stub over deliberately generating an invalid placeholder plus warning.
> - AgentPM semantic validators already produce the best lint messages. Preserve those and remove generic schema noise that obscures them.
> - Current `oneOf`/`anyOf` failure cliffs can serialize the entire containing object and can be duplicated through both `/properties/<kind>` and `/dependentSchemas/<kind>`.
> - Suppress a generic parent oneOf/anyOf message when a more specific semantic/domain error already exists at or beneath the same instance path.
> - Deduplicate only genuinely equivalent errors; do not hide the only available structural error.
> - Human rendering may improve significantly while JSON/NDJSON remains contract-stable.

- [ ] Make `agentpm init --kind tool` scaffold lint-valid output.
- [ ] Prefer a minimal runnable Tool stub:
  - [ ] valid `files`;
  - [ ] valid interpreter command;
  - [ ] valid non-empty args;
  - [ ] minimal source file needed by the entrypoint.
- [ ] Keep the Tool scaffold small; do not turn `init` into a large framework/template generator.
- [ ] Harden Tool package validation so a `manifest.files` entry that does not exist is a fatal packaging/publish error rather than a warning followed by silent omission.
- [ ] Correct stale Tool packaging comments/docs that describe `files` entries as globs when the implementation treats them as literal file/directory paths, unless true glob support is deliberately added.
- [ ] Verify all eight `init` kinds lint clean immediately.
- [ ] Refactor human lint rendering:
  - [ ] semantic/domain errors first;
  - [ ] suppress redundant parent oneOf/anyOf errors when covered by a more specific descendant/domain error;
  - [ ] deduplicate `/properties/<kind>` and `/dependentSchemas/<kind>` duplicates when they describe the same underlying failure;
  - [ ] truncate/bound instance echoes so large nested objects are never dumped inline;
  - [ ] print the failing pointer/path even when value echo is suppressed;
  - [ ] print expected discriminator values for closed unions where practical.
- [ ] For `packageRef` failures, prefer an actionable message describing accepted forms:
  - [ ] `"@ns/name@1.2.3"` string;
  - [ ] `{name, version?}` object.
- [ ] For closed Memory operation kinds, prefer expected values such as:
  - [ ] `consolidate`;
  - [ ] `transform`;
  - [ ] `delete`.
- [ ] Add test coverage across known oneOf/anyOf cliffs:
  - [ ] packageRef;
  - [ ] memoryTrigger;
  - [ ] memoryOperation;
  - [ ] loopTransitionTarget;
  - [ ] loopToolFailurePolicy;
  - [ ] knowledgeMetadata;
  - [ ] agentMemoryBinding;
  - [ ] entrypoint.command.
- [ ] Add a regression fixture reproducing the existing Loop case where one invalid transition currently emits giant schema messages before good semantic errors.
- [ ] Add a regression fixture reproducing the Memory invalid-operation-kind case where today no useful semantic error exists.
- [ ] Keep JSON/NDJSON lint output contracts stable unless explicitly versioned.
- [ ] Make publish validation use the same human lint renderer, including a contextual manifest header instead of beginning with unattributed `[ERROR]`.
- [ ] Add ordering tests ensuring actionable semantic messages appear before generic schema fallback.

## Milestone 9: CLI Error and Success Consistency
> Scope note: make non-Harness CLI success/failure behavior feel like one coherent product by centralizing transport errors, correcting exit codes, normalizing sibling inspect output, and cleaning up low-level path/filesystem leaks. This milestone does not alter Harness output, introduce new package/runtime capabilities, change machine-readable contracts beyond explicitly specified fixes, or absorb the Python portability work from later milestones.
> Implementation notes:
> - `whoami` already demonstrates a friendlier connection message, while `install`, `export`, and `new` currently expose raw reqwest URLs. Reuse one shared formatter rather than cloning command-specific wrappers.
> - Current exit-code convention is otherwise coherent:
>   - runtime error = 1;
>   - clap/usage error = 2.
>   `whoami` returning 0 after a connection failure is the confirmed exception.
> - `knowledge inspect` and `memory inspect` should share one structural summary pattern while retaining kind-specific detail.
> - Human output should normally avoid leaking long absolute paths unless the path is itself the important result; richer detail can stay available in verbose/machine modes.
> - Normalize user-supplied `.` paths before display so `/./` never appears.
> - Error/success wording should answer: what happened, where, why, and what next.

- [ ] Centralize network/transport error formatting in a reusable CLI helper.
- [ ] Apply friendly connection errors to:
  - [ ] install;
  - [ ] export;
  - [ ] new;
  - [ ] whoami;
  - [ ] other matching registry-backed commands discovered during implementation.
- [ ] Preserve low-level transport detail behind verbose/debug output if existing CLI patterns support it.
- [ ] Avoid surfacing irrelevant internal endpoint names as the primary error for higher-level commands such as `export --skill`.
- [ ] Fix `whoami` to exit non-zero on network failure.
- [ ] Add exit-code regression tests:
  - [ ] runtime error → 1;
  - [ ] clap/usage error → 2;
  - [ ] successful command → 0.
- [ ] Normalize Knowledge/Memory inspect header structure, for example:
  - [ ] `<Kind>: name@version`;
  - [ ] `Target: ...`;
  - [ ] `Status: fresh|stale|...`;
  - [ ] normalized manifest path/name;
  - [ ] then kind-specific details.
- [ ] Normalize freshness/status label across sibling inspect commands.
- [ ] Normalize paths before printing.
- [ ] Reduce unnecessary absolute path verbosity in normal human output.
- [ ] Preserve richer paths in verbose/machine output where useful.
- [ ] Improve Memory missing-sidecar wording from `is invalid: reading <path>` to an explicit “references X, which could not be read” style message.
- [ ] Make `new` success output include:
  - [ ] created target/path;
  - [ ] useful next step;
  - [ ] retain the existing README review guidance where useful.
- [ ] Move `serve --mcp` requirement into clap argument validation:
  - [ ] use clap-required/constraint behavior rather than runtime `currently requires --mcp`;
  - [ ] preserve exit code 2 for missing required usage.
- [ ] Improve `keys export <bad-id>` to:
  - [ ] say no local key with that ID exists;
  - [ ] suggest `agentpm keys list`;
  - [ ] avoid duplicate filesystem exception lines;
  - [ ] avoid unnecessary home-directory disclosure in normal output.
- [ ] Preserve `1` runtime / `2` clap usage exit-code convention.
- [ ] Add regression tests for all corrected messages and exit codes.

## Milestone 10: Analytics, Billing Funnel, Privacy, and Feedback
> Scope note: stop flying blind before early traffic by adding a deliberately small analytics/funnel vocabulary, authoritative server-side events where possible, privacy-conscious local CLI telemetry, billing conversion events, and a lightweight feedback path. This milestone does not enable session replay, experimentation, NPS, sophisticated attribution, broad behavioral profiling, or replace Lemon Squeezy as billing source of truth.
> Implementation notes:
> - Prefer server-side authoritative events whenever AgentPM already knows the real outcome. Example: `package_install_completed` should come from successful install-session completion, not merely an Install-button click.
> - Browser/CLI events should represent intent or local-only behavior only where the server cannot know the truth.
> - The initial product funnel is intentionally small:
>   landing → docs/explore/package → account → CLI/install → Harness → publish,
>   with a separate pricing → checkout → subscription branch.
> - `checkout_started` is intent; Lemon Squeezy webhook events are billing truth.
> - CLI telemetry is minimal, anonymous, default-on with clear disclosure, and easy to disable.
> - Use a strict event/property allowlist. Do not pass broad application objects to PostHog and rely on redaction later.
> - Never collect prompts, Tool I/O, Harness conversation content, file contents, paths, env vars, secrets, package contents, or private package identities.
> - PostHog may provide the survey backend/reporting, but the visible feedback UI should look and feel like AgentPM.
> - Do not enable session replay in Stage 1.

- [ ] Create PostHog project/config integration for web/backend/CLI surfaces as needed.
- [ ] Decide one shared event naming/property convention and document it before instrumenting broadly.
- [ ] Disable/avoid broad autocapture if it would violate the explicit event/property contract.
- [ ] Implement initial web/registry events:
  - [ ] `landing_viewed`;
  - [ ] `docs_viewed`;
  - [ ] `explore_searched`;
  - [ ] `package_viewed`;
  - [ ] `namespace_viewed`;
  - [ ] `signup_started`;
  - [ ] `pricing_viewed`;
  - [ ] `checkout_started`;
  - [ ] `feedback_submitted`.
- [ ] Implement server-authoritative events for:
  - [ ] `account_created` where the backend has an authoritative completion point;
  - [ ] `package_install_completed`;
  - [ ] `publish_completed`;
  - [ ] `package_starred` / unstar equivalent if needed for analysis;
  - [ ] subscription lifecycle.
- [ ] Add `pricing_viewed` at actual pricing-page exposure and `checkout_started` at Lemon Squeezy checkout handoff.
- [ ] Map Lemon Squeezy webhook events to:
  - [ ] subscription started;
  - [ ] subscription updated;
  - [ ] subscription cancelled;
  - [ ] payment failed where supported.
- [ ] Make webhook-derived analytics idempotent so retries do not double-count subscription lifecycle events.
- [ ] Keep Lemon Squeezy as billing source of truth; PostHog receives product events, not authoritative billing state.
- [ ] Add minimal CLI telemetry:
  - [ ] `cli_first_used`;
  - [ ] `harness_started`;
  - [ ] `harness_completed`.
- [ ] Decide whether `cli_first_used` is once per anonymous installation or once per CLI version; prefer once per installation unless product analysis requires version-level adoption.
- [ ] Generate/store anonymous install ID:
  - [ ] non-secret random identifier;
  - [ ] stable across CLI runs;
  - [ ] not derived from machine serial, username, path, or other identifying system data.
- [ ] Add strict telemetry property allowlist.
- [ ] Allowed CLI properties should be limited to fields such as:
  - [ ] event;
  - [ ] CLI version;
  - [ ] OS family;
  - [ ] CPU architecture;
  - [ ] anonymous install ID;
  - [ ] small runtime-mode enum where needed.
- [ ] Add `AGENTPM_TELEMETRY=0` support.
- [ ] Add persistent config opt-out if existing config architecture supports it cleanly.
- [ ] Ensure telemetry never sends:
  - [ ] prompts;
  - [ ] Tool I/O;
  - [ ] Harness conversation content;
  - [ ] Tool args/results;
  - [ ] paths;
  - [ ] env vars;
  - [ ] secrets;
  - [ ] package contents;
  - [ ] private package identities.
- [ ] Update Privacy Policy to disclose:
  - [ ] web/product analytics;
  - [ ] minimal CLI telemetry;
  - [ ] categories of data collected;
  - [ ] explicit excluded data categories;
  - [ ] anonymous install identifier;
  - [ ] opt-out mechanism;
  - [ ] PostHog as relevant service/provider;
  - [ ] feedback form data including optional email.
- [ ] Create one Early Product Funnel PostHog dashboard rather than many dashboards.
- [ ] Dashboard should answer at minimum:
  - [ ] are people arriving?;
  - [ ] do they reach docs/explore/package pages?;
  - [ ] do they install/use the CLI?;
  - [ ] do installs complete?;
  - [ ] do Harness runs start/complete?;
  - [ ] do users publish?;
  - [ ] do users start checkout/subscribe?;
  - [ ] are users giving feedback/stars?
- [ ] Add AgentPM-branded Feedback entry in shared site/docs/registry chrome.
- [ ] Back feedback storage/reporting with PostHog survey capability.
- [ ] Initial feedback form should capture:
  - [ ] what user was trying to do;
  - [ ] what got in the way or surprised them;
  - [ ] optional email;
  - [ ] follow-up consent.
- [ ] Do not make satisfaction score/NPS the primary Stage 1 feedback mechanism.
- [ ] Add GitHub Issue branch/link for reproducible bugs.
- [ ] Do not enable session replay in Stage 1.

## Milestone 11: Python Tool Dependency Contract
> Scope note: introduce the new-format Python Tool dependency contract and AgentPM-owned local Python environment: author intent in `agent.json`, portable exact resolution state in `agent.lock`, eager dependency provisioning during install, and deterministic execution through an AgentPM-managed environment. This milestone preserves legacy vendored Tools and does not yet define multi-target release storage, target artifact selection, release-level cryptographic signing, or GitHub Actions orchestration.
>
> Implementation notes:
> - Current AgentPM has no venv, no pip/uv integration, and no Python dependency location. `agentpm run` resolves `python`/`python3` from PATH (or `AGENTPM_PYTHON`) and invokes it directly.
> - Existing `_vendor` support is entirely author code (`sys.path.insert`) and must keep working for legacy Tools.
> - `runtime.version` is currently enforced as a minimum, not an exact interpreter pin; retain that contract.
> - The new portable resolution must not capture only the publisher's selected wheel/architecture. Exact versions may need environment markers/conditional branches.
> - Plain `agentpm install` fully regenerates `agent.lock` from the server resolve plan. New Python resolution state must therefore flow through the resolve DTO/plan or it will be erased.
> - Current lock parsing is forward-unsafe: `lockfile_version` is not consulted before untagged deserialization, unknown fields are ignored, and a future lock can be rewritten lossily. Fix this before writing v4 data.
> - Keep one logical locked package entry per `kind:name@version`; target selection is local machine state, not source-controlled lock state.

- [ ] Extend Tool runtime schema with optional Python `dependencies` when `runtime.type == "python"`.
- [ ] Extend all relevant typed runtime representations, including runner-side `RuntimeDecl`, so the new field is understood consistently.
- [ ] Reject `dependencies` for Node Tools.
- [ ] Add Python requirement syntax validation.
- [ ] Add deterministic duplicate/conflict validation.
- [ ] Define a typed/versioned `PythonResolution` model suitable for:
  - [ ] exact distribution names/versions;
  - [ ] environment markers/conditional transitive dependencies where needed;
  - [ ] deterministic serialization/digesting;
  - [ ] portability across supported consumer targets.
- [ ] Do not lock publisher-selected wheel filenames or publisher architecture in `PythonResolution`.
- [ ] Integrate AgentPM-managed `uv`:
  - [ ] pin/version-check the resolver AgentPM uses;
  - [ ] choose a managed acquisition strategy (bundled or downloaded/cached) rather than requiring the user's project to manage uv;
  - [ ] surface a clear error if the managed resolver cannot be acquired/run.
- [ ] Do not make `requirements.txt`, `pyproject.toml`, Poetry, or uv project lockfiles authoritative in Stage 1.
- [ ] Resolve declared dependencies to exact portable state before publishing:
  - [ ] update/write AgentPM lock state;
  - [ ] fail publish if manifest declarations and resolved state disagree/stale.
- [ ] Extend `LockedPackage` with optional Python resolution state.
- [ ] Evolve lockfile-version selection:
  - [ ] replace `requires_v3_lock(...)` with a `minimum_lock_version(...)`-style helper;
  - [ ] return minimum version 4 when Python resolution/new release fields are present;
  - [ ] retain versions 2/3 for locks that do not need v4 capabilities unless a broader migration is intentionally chosen.
- [ ] Add a **forward-version guard before normal lock deserialization/mutation**:
  - [ ] reject `lockfile_version` newer than the CLI supports;
  - [ ] prevent older CLIs from silently dropping future fields on rewrite;
  - [ ] audit `Lock::empty_v2()` and Harness `>=2` checks for correct max-version behavior.
- [ ] Preserve current V1/V2-shape and on-disk v2/v3 reads.
- [ ] Keep package keys target-independent (`kind:name@version`).
- [ ] Update registry resolve/install DTOs and `ResolvePlan` so Python resolution state reaches `lock_from_plan` and survives a plain full-regeneration install.
- [ ] Ensure published Tool release metadata/artifact contains the Tool-specific Python resolution independently of the publisher's broader workspace lock.
- [ ] Add an AgentPM-managed per-Tool Python environment under `.agentpm/`, keyed by:
  - [ ] Tool identity/version;
  - [ ] consumer target;
  - [ ] resolved interpreter major/minor or equivalent ABI identity.
- [ ] During install, resolve the interpreter using current `AGENTPM_PYTHON`/PATH family and minimum-version rules.
- [ ] Create/reuse a managed venv-style environment with that interpreter and install the exact applicable locked dependencies through AgentPM-managed uv.
- [ ] Update `agentpm run` so new-format dependency-bearing Python Tools execute with the managed environment's Python interpreter.
- [ ] Validate the local runtime-environment fingerprint at run time; if interpreter/target changed, deterministically rebuild or fail with an actionable refresh/reinstall instruction.
- [ ] Preserve legacy Tool execution through current PATH/`AGENTPM_PYTHON` semantics.
- [ ] Preserve existing author-written `_vendor` imports alongside managed environments.
- [ ] Warn when a new-format Tool both declares dependencies and vendors likely dependency trees.
- [ ] Add tests covering:
  - [ ] pure-Python dependency;
  - [ ] dependency with native wheel;
  - [ ] conditional/marker dependency;
  - [ ] interpreter minimum/version changes;
  - [ ] future lockfile rejection;
  - [ ] plain install lock regeneration retaining Python resolution.

## Milestone 12: Python Tool Artifact Compatibility
> Scope note: establish a conservative compatibility model for the Tool payload itself, distinct from Python dependency portability, so AgentPM can distinguish portable `any` artifacts from target-specific native payloads. This milestone defines the canonical target vocabulary and package-time classification metadata. It does not yet implement multi-artifact registry storage, target-aware install selection, CI fan-in publishing, or retroactively classify legacy releases.
>
> Implementation notes:
> - `client.os` / `client.arch` in today's publish descriptor describe the publishing machine only; they are not compatibility metadata and must not be reinterpreted as such.
> - Prefer AgentPM target IDs compatible with the Rust target-triple vocabulary already used by AgentPM's release workflows. The target must distinguish relevant ABI/libc, not only OS + CPU.
> - `any` is reserved for payloads AgentPM can safely treat as portable; when uncertain, choose the current concrete target.
> - Declared Python dependencies are installed separately for the consumer and do not make the Tool payload target-specific.
> - `.whl` is currently blocked as an embedded archive. Keep it blocked for normal Tool payloads; target-side dependency installation removes the need to ship wheels inside the Tool tarball.

- [ ] Define canonical AgentPM target identifier representation, including:
  - [ ] `any`;
  - [ ] `aarch64-apple-darwin`;
  - [ ] `x86_64-apple-darwin`;
  - [ ] `x86_64-unknown-linux-gnu`;
  - [ ] `x86_64-pc-windows-msvc`;
  - [ ] room for future supported targets/ABI variants.
- [ ] Add deterministic current-target detection/mapping rather than treating raw `std::env::consts::{OS,ARCH}` as the compatibility ID.
- [ ] Define portable `any` semantics explicitly.
- [ ] Implement conservative payload classification after final file collection:
  - [ ] `.so`;
  - [ ] `.dylib`;
  - [ ] `.pyd`;
  - [ ] `.dll`;
  - [ ] `.exe`;
  - [ ] executable/native binary signatures where practical;
  - [ ] known platform-specific unpacked vendored binary content.
- [ ] Treat unknown executable/binary payload conservatively as current-target specific.
- [ ] Ensure declared Python dependencies do not force Tool payload to target-specific.
- [ ] Keep `.whl` blocked as an embedded Tool payload unless a non-dependency use case is separately justified.
- [ ] Add target + artifact-format metadata to the package-build descriptor.
- [ ] Preserve legacy Tool behavior with compatibility unknown/legacy rather than inventing `any`.
- [ ] Ensure local publish of a pure source Tool can produce `any`.
- [ ] Ensure local publish of a native payload claims only the detected current target.
- [ ] Add tests for pure Python/data payload versus native payload classification on supported host families.

## Milestone 13: Multi-Artifact Release, Upload, S3, and Scanning Model
> Scope note: evolve publishing/storage from one version = one tarball into one immutable Tool release = one or more target artifacts. Introduce release reservations with child uploads, separate per-target S3 objects, normalized artifact rows, atomic release finalization, streaming uploads, and per-artifact malware scanning while preserving the legacy one-object publish path. This milestone defines release assembly/storage; Milestone 14 adds the release cryptographic contract and Milestone 15 consumes it during installation.
>
> Implementation notes:
> - The existing `Upload` row has one `tmp_key`, one `final_key`, one digest and one pending row per `(package_id, version)`. A second target upload currently collides with `409 publish already in progress`; do not stretch this row into pretending it is a multi-artifact release.
> - Keep the existing `/v1/tools/publish/init|finalize` legacy behavior readable. New-format requests may reuse/version these endpoints, but need a release-level reservation plus child artifact-upload representation.
> - `PackageVersion` remains the logical immutable version. Add child artifact rows rather than N package versions.
> - Existing `PackageVersion.sha_256/size_bytes/s3_key` are legacy single-artifact columns. Add an explicit release-format discriminator so read paths never guess semantics.
> - Current publish buffers an entire artifact in memory. New package/release upload should stream.
> - Current client and server upload caps disagree (3 GB client vs ~1 GB server). Treat server policy as authoritative.
> - Current S3 final key uses only a 12-hex digest prefix. New immutable artifact object identity should use full digest plus target.
> - Current malware scan model is asynchronous after publish. Preserve that timing, but create a scan per concrete target artifact and yank the logical release if any artifact is infected.

- [ ] Define release manifest schema/version and typed client/server representations.
- [ ] Define new release-level publish reservation model (for example `PublishRelease`) containing:
  - [ ] package/version identity;
  - [ ] manifest + manifest digest;
  - [ ] Python resolution + resolution digest where present;
  - [ ] publisher;
  - [ ] pending/finalized/expired status;
  - [ ] expiration/resume metadata.
- [ ] Define child artifact-upload reservation model containing:
  - [ ] release ID;
  - [ ] target;
  - [ ] expected SHA-256;
  - [ ] expected size;
  - [ ] tmp/final S3 keys;
  - [ ] upload status.
- [ ] Define `PackageVersionArtifact` (or equivalent) under logical `PackageVersion`:
  - [ ] target;
  - [ ] SHA-256;
  - [ ] size;
  - [ ] S3 key;
  - [ ] content type/artifact format;
  - [ ] immutable association to the release.
- [ ] Add an explicit release/artifact-format discriminator on `PackageVersion` so legacy vs new rows branch intentionally.
- [ ] Preserve legacy scalar version columns/read path for existing one-artifact releases; do not reinterpret old rows.
- [ ] Ensure unsupported old clients receive a clear "requires newer AgentPM" response for new-format releases rather than a release-manifest URL masquerading as a tarball.
- [ ] Define new-format S3 layout using separate immutable objects, conceptually:
  - [ ] `packages/<ns>/<name>/<version>/release.json`;
  - [ ] `packages/<ns>/<name>/<version>/artifacts/<target>/<full-sha256>.tar.gz`;
  - [ ] staging `uploads/<release-id>/<artifact-id>.tar.gz`.
- [ ] Keep legacy S3 key layout readable.
- [ ] Persist the canonical release metadata both in normalized DB state and immutable `release.json`.
- [ ] Make each artifact independently self-describing:
  - [ ] include logical `agent.json`;
  - [ ] include Tool-specific Python resolution metadata/descriptor;
  - [ ] include target/build descriptor as needed.
- [ ] Validate all artifacts in one release share:
  - [ ] kind/name/version;
  - [ ] manifest digest;
  - [ ] Python resolution digest.
- [ ] Reject duplicate/conflicting targets.
- [ ] Implement release init/resume that can return multiple per-artifact presigned PUT instructions without one-pending-upload collisions.
- [ ] Implement per-artifact retries/resume while keeping one logical release reservation.
- [ ] Stream artifact PUT bodies instead of `tokio::fs::read` buffering the entire file.
- [ ] Reconcile size limits:
  - [ ] server returns authoritative per-artifact max;
  - [ ] client enforces the server policy;
  - [ ] remove contradictory independent 3 GB vs 1 GB expectations.
- [ ] Implement server-side cleanup/expiration for orphaned staged release artifacts.
- [ ] Implement one atomic release finalize:
  - [ ] all intended artifacts present;
  - [ ] all metadata consistent;
  - [ ] target set unique;
  - [ ] release manifest constructed/persisted;
  - [ ] DB release/artifact rows committed together;
  - [ ] release becomes installable only after finalize succeeds.
- [ ] Enforce immutable artifact inventory after finalize.
- [ ] Enforce new semantic version requirement for adding targets.
- [ ] Adapt malware scanning:
  - [ ] enqueue one scan per `PackageVersionArtifact`;
  - [ ] associate scan row/result with concrete artifact (artifact FK preferred);
  - [ ] preserve queued/running/unknown semantics;
  - [ ] if any artifact is infected, yank/disable the logical version;
  - [ ] never display the version as wholly clean unless all required artifact scan states justify it.
- [ ] Do not wire existing unused SBOM/Grype columns as a side effect unless deliberately scoped.

## Milestone 14: Release Integrity and Provenance Upgrade
> Scope note: upgrade the new release model from "multiple stored objects" to a cryptographically bound, independently verifiable release. Introduce canonical release integrity, typed/versioned signing statements, trustworthy stored-byte checksums, install-facing signature material, registry-key history/trust, and clear historical revocation semantics while preserving every legacy signing format. This milestone intentionally keeps the effective namespace threshold at one signature; dynamic N-of-M policy and AgentPM OIDC publishing remain deferred.
>
> Implementation notes:
> - There is no Rust signature statement type today; v1 is an inline `json!` object. Introduce a typed v1 representation first and prove byte compatibility before adding v2.
> - Legacy author signatures are `agentpm.package.signature.v1`; legacy registry attestations are already `agentpm.registry.attestation.v2`. New versions therefore advance independently (expected author v2, registry v3).
> - New-format signatures attach naturally to `PackageVersion` (the logical release), not each target artifact.
> - Current server finalize compares publisher-declared SHA against publisher-set S3 metadata. New release finalize needs a trustworthy stored-byte digest.
> - Current install receives only counts/booleans. New verification requires the actual statement/signature/public-key material.
> - Rust and Python display the same author key with different truncated key IDs (16 vs 12 chars). Match cryptographically by full public key and normalize key ID only for display.
> - Registry attestations cannot be verified today because the registry public key/history is not exposed. Add an explicit trust/key-history contract.
> - A normal signer revocation after publication should not retroactively make a cryptographically valid historical release unsigned.

- [ ] Introduce typed Rust representation for legacy `agentpm.package.signature.v1`:
  - [ ] preserve camelCase wire fields exactly;
  - [ ] replace inline `json!` construction;
  - [ ] add byte-for-byte fixture proving serialized bytes match current behavior.
- [ ] Introduce corresponding typed/server validation helpers rather than open-coded dict-key checks where practical.
- [ ] Define canonical release manifest content binding:
  - [ ] kind;
  - [ ] namespace/name;
  - [ ] version;
  - [ ] release format version;
  - [ ] manifest digest;
  - [ ] Python resolution digest when present;
  - [ ] sorted complete artifact inventory;
  - [ ] target/digest/size/content type per artifact.
- [ ] Define release-level SHA-256 as hash of canonical release-manifest bytes.
- [ ] Keep `LockedPackage.integrity` as a bare 64-char lowercase hex value for the release digest.
- [ ] **Do not** add selected artifact target/integrity to portable `agent.lock`.
- [ ] Define new canonical JSON contract for new statement versions:
  - [ ] prefer RFC 8785 JCS or equivalently explicit documented profile;
  - [ ] deterministic Unicode;
  - [ ] deterministic object keys;
  - [ ] artifact array sorted by canonical target;
  - [ ] deterministic timestamps/numbers.
- [ ] Implement canonicalizer in Rust.
- [ ] Implement canonicalizer in Python/server.
- [ ] Add shared cross-language test vectors including:
  - [ ] ASCII;
  - [ ] non-ASCII Unicode;
  - [ ] key-order differences;
  - [ ] artifact-order normalization.
- [ ] Preserve historical serialization/verification logic for legacy author v1 and registry attestation v2.
- [ ] Introduce new release-level author signature type, expected `agentpm.package.signature.v2`, binding release digest/identity.
- [ ] Introduce new release-level registry attestation type, expected `agentpm.registry.attestation.v3`, binding release digest plus registry provenance facts.
- [ ] Keep signatures/attestations attached to logical `PackageVersion`; do not create one author signature per target.
- [ ] Improve publish signature diagnostics:
  - [ ] distinguish unsupported algo/type;
  - [ ] identity/digest mismatch;
  - [ ] unregistered/inactive signer at publish;
  - [ ] invalid Ed25519 signature;
  - [ ] if user supplied `--sign`, rejection must fail publish rather than silently downgrade to unsigned.
- [ ] Preserve namespace signing modes `off|optional|required`.
- [ ] Preserve effective Stage 1 threshold: `required` means at least one valid trusted author signature.
- [ ] Explicitly defer dynamic `min_author_signatures`/N-of-M enforcement.
- [ ] Normalize key-ID display algorithm/length across Rust/Python.
- [ ] Use full `public_key_b64` for signer matching/verification, not truncated key ID.
- [ ] Define historical signer semantics:
  - [ ] signature accepted while signer was authorized remains cryptographically/historically valid after normal revocation;
  - [ ] expose current `is_active`/`revoked_ts` separately;
  - [ ] do not invent retroactive compromise revocation in Stage 1.
- [ ] Add trustworthy stored-byte verification before new release finalization:
  - [ ] use S3/storage checksum if it provides trusted SHA-256 in the presigned flow; otherwise independently stream/hash the staged object;
  - [ ] require declared digest == trusted stored digest == release-manifest digest reference;
  - [ ] continue independently checking stored size.
- [ ] Make new-format registry attestation fail closed:
  - [ ] if registry signing key/config is unavailable or attestation persistence fails, leave release unpublished;
  - [ ] legacy behavior may remain compatible for legacy releases.
- [ ] Add registry public-key history/read model:
  - [ ] registry key ID;
  - [ ] full public key;
  - [ ] algorithm;
  - [ ] activation/retirement metadata;
  - [ ] historical keys retained for old attestations.
- [ ] Define official-registry client trust anchor:
  - [ ] do not trust an arbitrary key merely because it arrived beside the signature;
  - [ ] use built-in/pinned key material, a root-signed key set, or equivalent explicit trust configuration.
- [ ] Evolve the public Security/read DTO for new-format versions so the web UI can inspect release-level integrity/provenance and target artifacts without using truncated digests as verification inputs; keep legacy responses compatible.
- [ ] Extend install/init verification payload to include:
  - [ ] canonical release manifest/release digest;
  - [ ] author signature + statement + full public key + signer status;
  - [ ] namespace signing mode/effective threshold;
  - [ ] registry attestation statement/signature/key ID;
  - [ ] trusted registry-key lookup/reference material.
- [ ] Do not depend on the current Security DTO's truncated display digest for install verification.
- [ ] Implement client-side release-digest verification.
- [ ] Implement client-side author-signature verification.
- [ ] Implement client-side registry-attestation verification.
- [ ] Add `--require-signature`:
  - [ ] enforce ≥1 valid trusted author signature regardless of optional/off namespace mode.
- [ ] Change `--require-attestation` semantics to cryptographically verified attestation rather than server boolean.
- [ ] When namespace mode is `required`, client verification also enforces the effective one-signature policy.
- [ ] Add tamper tests for:
  - [ ] artifact bytes;
  - [ ] artifact target/list;
  - [ ] manifest digest;
  - [ ] Python resolution digest;
  - [ ] release digest;
  - [ ] author statement/signature;
  - [ ] registry statement/signature;
  - [ ] wrong/untrusted registry key.

## Milestone 15: Target-Aware Installation and Runtime Provisioning
> Scope note: consume the new release model safely on the client: choose one compatible target artifact, verify the release and payload, cache/extract it without collisions, provision the AgentPM-managed Python environment, and fail deterministically when compatibility or archive safety checks fail. Preserve the legacy install path and do not add Rosetta/emulation or silently change locked versions.
>
> Implementation notes:
> - Current install `PackageArtifact` contains one URL/digest and no target. Extend the contract for new release metadata without requiring clients to download/presign every target.
> - Detect consumer target before install init and send it to the server; server may return the selected artifact URL plus the full signed release inventory for client verification.
> - Keep `agent.lock` platform-neutral. Selected target/artifact and runtime-environment fingerprint belong in `.agentpm/` local state.
> - Current cache filename omits kind, digest, and target; fix it before multiple target payloads exist.
> - Current extraction silently skips symlink/hardlink entries and has no decompressed-size cap. Harden this while changing the install path.
> - Dependency provisioning adds a new install failure stage; do not finalize the install session as successful until the local runtime environment is ready.

- [ ] Detect canonical current target using Milestone 12 target mapping.
- [ ] Resolve the Tool's Python interpreter using existing family/minimum-version/`AGENTPM_PYTHON` rules before provisioning dependencies.
- [ ] Extend install-init request with target/runtime context needed for selection.
- [ ] Read/verify new release metadata:
  - [ ] logical release digest;
  - [ ] canonical artifact inventory;
  - [ ] Python resolution;
  - [ ] provenance material from Milestone 14.
- [ ] Have install-init return/presign only the selected concrete artifact for the consumer while still exposing available target metadata for diagnostics.
- [ ] Select exact target first.
- [ ] Fall back to `any` only when a portable artifact is present.
- [ ] Verify release-level integrity before trusting artifact metadata.
- [ ] Verify selected artifact appears exactly in the verified release manifest.
- [ ] Stream-download selected artifact.
- [ ] Validate received size against expected artifact size where practical.
- [ ] Recompute selected artifact SHA-256 and compare to the verified release manifest.
- [ ] Redesign cache identity to prevent collisions across:
  - [ ] kind;
  - [ ] namespace/name;
  - [ ] version;
  - [ ] target;
  - [ ] artifact digest.
- [ ] Preserve cache re-verification before reuse.
- [ ] Keep extracted Tool directory version-terminal if no correctness issue requires a target segment; only one target payload is active per local install.
- [ ] Store selected target/artifact digest/runtime-environment fingerprint in machine-local `.agentpm/` state if persistence is needed.
- [ ] Harden archive extraction:
  - [ ] reject unsupported symlink/hardlink entries explicitly instead of silently skipping them;
  - [ ] retain traversal/absolute-path protection;
  - [ ] enforce decompressed total-byte cap;
  - [ ] enforce entry-count cap on extraction;
  - [ ] ensure failed extraction cannot leave a seemingly complete install directory.
- [ ] Provision dependencies after payload extraction:
  - [ ] create/reuse Milestone-11 managed Python environment;
  - [ ] install exact applicable locked dependencies with AgentPM-managed uv;
  - [ ] verify environment fingerprint.
- [ ] Only finalize the install session as successful after payload + dependency environment are ready.
- [ ] Preserve legacy install/download/extract path for legacy releases.
- [ ] Add actionable no-compatible-artifact error:
  - [ ] requested target;
  - [ ] available targets;
  - [ ] selected Tool version.
- [ ] Implement interactive recovery only for supported alternatives:
  - [ ] a compatible newer version may be offered only for an unpinned/non-exact request;
  - [ ] require explicit confirmation.
- [ ] Do not silently install incompatible artifact.
- [ ] Do not silently change a locked/exact version.
- [ ] In headless mode, fail deterministically with structured error.
- [ ] Do not add Rosetta/emulation handling in this milestone.

## Milestone 16: Headless Signing and GitHub Actions Publishing
> Scope note: make multi-target releases practical for maintainers by adding secure noninteractive signing plus a first official GitHub Actions workflow that packages target artifacts in parallel and performs one final atomic AgentPM publish. Follow existing AgentPM workflow conventions, but keep the package-build/publish protocol CI-provider-neutral. Stage 1 uses PAT auth for AgentPM registry CI; OIDC trusted publishing remains future work.
>
> Implementation notes:
> - Current `--sign` always prompts for a passphrase and therefore cannot run headless.
> - Preserve `StoredKeyV1` encrypted key material; do not make raw Ed25519 private-key secrets the preferred CI interface.
> - Existing AgentPM workflows establish tag/version guards, pinned actions/toolchains, `uv`, target matrices, and artifact upload. They do **not** establish atomic fan-in publishing; this milestone must add that.
> - `publish --dry-run` is not a sufficient matrix build contract because it produces human output and skips signing/network behavior. Extract or add a provider-neutral package-build CLI surface.
> - Initial registry authentication can use the existing `AGENTPM_TOKEN`/PAT mechanism.
> - Matrix target vocabulary should match the canonical AgentPM targets from Milestone 12.

- [ ] Add secure noninteractive key loading:
  - [ ] preserve encrypted `StoredKeyV1`;
  - [ ] support explicit encrypted key file/input suitable for ephemeral CI (`--key-file` or equivalent);
  - [ ] support passphrase from `AGENTPM_KEY_PASSPHRASE` or equivalent secret source;
  - [ ] preserve interactive prompt when no noninteractive secret is provided;
  - [ ] never print passphrase/decrypted key.
- [ ] Ensure headless publish can satisfy namespace `required` signing policy.
- [ ] Keep `--key-id` behavior for local keystore users.
- [ ] Normalize CI signing errors so missing key/passphrase/invalid decryption is actionable and non-secret.
- [ ] Add/reuse a provider-neutral artifact-build command/path; prefer explicit `agentpm package` if no existing command can expose a stable machine-readable contract.
- [ ] Package-build output must include:
  - [ ] artifact path;
  - [ ] canonical target;
  - [ ] artifact SHA-256;
  - [ ] artifact size;
  - [ ] manifest digest;
  - [ ] Python resolution digest;
  - [ ] machine-readable descriptor for fan-in validation.
- [ ] Keep local `agentpm publish` convenient by internally reusing the same package-build code for current-target/`any` publishing.
- [ ] Build official GitHub Actions example/workflow for Python Tools.
- [ ] Follow established AgentPM conventions:
  - [ ] tag-triggered on `v*`;
  - [ ] fail if tag version != `agent.json.version`;
  - [ ] `actions/checkout@v4`;
  - [ ] pinned toolchain/actions;
  - [ ] minimal job permissions;
  - [ ] `astral-sh/setup-uv@v6` where Python tooling is needed;
  - [ ] `set -euo pipefail` for multiline bash.
- [ ] Support initial required runner/target matrix:
  - [ ] `aarch64-apple-darwin`;
  - [ ] `x86_64-apple-darwin`;
  - [ ] `x86_64-unknown-linux-gnu`.
- [ ] Add Linux arm64 and/or Windows only if practical without blocking the initial proof.
- [ ] Matrix jobs:
  - [ ] checkout identical commit;
  - [ ] run `agentpm package`/equivalent;
  - [ ] upload artifact + descriptor with `actions/upload-artifact@v4`;
  - [ ] do **not** independently publish to AgentPM.
- [ ] Final fan-in job:
  - [ ] `needs` all matrix build jobs;
  - [ ] download all build outputs with `actions/download-artifact`;
  - [ ] verify name/version/manifest digest/Python resolution digest match;
  - [ ] reject duplicate targets;
  - [ ] authenticate with `AGENTPM_TOKEN`;
  - [ ] load encrypted signing key/passphrase secrets if signing;
  - [ ] perform one atomic release publish/finalize.
- [ ] Ensure a failed/missing target build prevents final publish rather than creating a partial release.
- [ ] Keep workflow/provider concepts out of core release schema; GitLab/other CI should be able to call the same CLI commands.
- [ ] Verify local single-target publish remains supported.
- [ ] Add end-to-end CI example package/run if feasible.
- [ ] Explicitly document AgentPM OIDC trusted publishing as future work, not Stage 1.

## Milestone 17: Documentation and Migration Hardening
> Scope note: finish Stage 1 by documenting the hardened discovery/CLI/analytics surfaces and the new Python Tool dependency, runtime-environment, portability, release, integrity, and CI models so authors can use the system without founder guidance. This milestone updates Stage 1-facing docs/examples only; it does not perform the Stage 2 category-language/IA rewrite or expand the feature set beyond earlier milestones.
>
> Implementation notes:
> - Clearly separate legacy Python Tools from new-format dependency-bearing Tools.
> - The portable lock records logical release integrity and portable Python resolution; local target/artifact/runtime-environment state lives under `.agentpm/`.
> - New-format dependencies are installed into an AgentPM-managed environment, not into the Tool tarball or user's global Python.
> - Use canonical target IDs in docs/examples (`aarch64-apple-darwin`, etc.), not the earlier shorthand `macos-arm64`.
> - Explain that one version is an immutable logical release containing one or more target artifacts; adding a target means a new semantic version.
> - Document cryptographic meaning precisely: artifact digest, release digest, author signature, registry attestation, and what install actually verifies.
> - Keep Stage 2 taxonomy/copy rewrite out of this milestone.

- [ ] Update Python Tool authoring docs:
  - [ ] declare dependencies in `agent.json`;
  - [ ] `agent.json` = author intent;
  - [ ] `agent.lock` = AgentPM-resolved portable state;
  - [ ] AgentPM-managed uv/resolution;
  - [ ] users do not need a uv project;
  - [ ] AgentPM-managed per-Tool Python environment;
  - [ ] `AGENTPM_PYTHON` interpreter behavior/minimum runtime version;
  - [ ] portable source payload versus native target-specific payload;
  - [ ] local publish;
  - [ ] CI multi-target publish.
- [ ] Add migration section for existing Python Tool authors:
  - [ ] existing `_vendor` Tools remain valid;
  - [ ] no forced republish;
  - [ ] historical compatibility remains unknown/legacy;
  - [ ] how to migrate a new version to declared dependencies.
- [ ] Mark `_vendor` as legacy/manual for dependency management and explain architecture pitfalls.
- [ ] Correct current docs/comments that imply Tool `files` entries are globs if the implementation remains literal file/directory paths.
- [ ] Document missing declared `files` path as a publish error.
- [ ] Document lockfile evolution:
  - [ ] existing v2/v3 locks remain supported;
  - [ ] v4 capability state;
  - [ ] future unsupported lock versions fail rather than being rewritten lossily;
  - [ ] no target-qualified package keys;
  - [ ] local selected artifact is not stored in the portable lock.
- [ ] Document release/artifact model:
  - [ ] logical Tool version/release;
  - [ ] target artifact inventory;
  - [ ] canonical target identifiers + `any`;
  - [ ] immutable release inventory.
- [ ] Document S3/release concepts only to the degree useful for advanced publishers; avoid forcing internal storage details into basic author docs.
- [ ] Document install behavior:
  - [ ] exact target then `any`;
  - [ ] managed dependency environment creation;
  - [ ] local environment refresh behavior;
  - [ ] no-compatible-target error;
  - [ ] no silent locked-version substitution.
- [ ] Document integrity/provenance:
  - [ ] release digest;
  - [ ] artifact digest;
  - [ ] author signature;
  - [ ] registry attestation;
  - [ ] actual client-side verification;
  - [ ] `--require-signature`;
  - [ ] cryptographic `--require-attestation`;
  - [ ] normal historical signer revocation semantics.
- [ ] Document headless signing:
  - [ ] encrypted key file/material;
  - [ ] `AGENTPM_KEY_PASSPHRASE` or final chosen secret source;
  - [ ] do not recommend raw private-key secrets.
- [ ] Document official GitHub Actions workflow:
  - [ ] tag ↔ `agent.json.version` guard;
  - [ ] target matrix;
  - [ ] `actions/upload-artifact` / `actions/download-artifact` fan-in;
  - [ ] one atomic publish;
  - [ ] `AGENTPM_TOKEN`;
  - [ ] provider-neutral CLI equivalent.
- [ ] Document OIDC/trusted AgentPM publishing as a future direction rather than a Stage 1 feature.
- [ ] Document telemetry/privacy contract and opt-out.
- [ ] Update CLI help text for new package/signing/install options.
- [ ] Update registry/docs examples to use `agentpm-harness` Template execution surface where relevant.
- [ ] Update feedback/help docs for product feedback versus reproducible GitHub bugs.
- [ ] Ensure Stage 2 terminology work remains separate.
