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
- [ ] Verify Security tab continues to display the selected version’s:
  - [ ] package/artifact type;
  - [ ] digest;
  - [ ] author-signature state;
  - [ ] registry-attestation state;
  - [ ] malware status;
  - [ ] license.
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
> Scope note: introduce the new-format Python Tool dependency contract: author intent in `agent.json`, exact AgentPM-resolved state in `agent.lock`, and target-side dependency installation managed by AgentPM. This milestone preserves legacy vendored Tools and does not yet define multi-target release storage, target artifact selection, CI matrices, release-level signing, or Stage 2 compatibility UI.
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
> Scope note: establish a conservative compatibility model for the Tool payload itself, distinct from Python dependency portability, so AgentPM can distinguish portable `any` artifacts from platform/architecture-specific native payloads. This milestone does not yet implement multi-artifact releases, S3 layout changes, target-aware installation, CI publishing, or retroactively classify legacy releases.
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
> Scope note: evolve the publish/storage model from one version = one tarball to one immutable Tool release = one or more target artifacts, with explicit release metadata, staging, atomic finalize, and backwards-compatible legacy storage reads. This milestone defines storage and release assembly; it does not yet complete release-level cryptographic signing, target-aware installation, or GitHub Actions orchestration.
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
> Scope note: upgrade integrity and provenance for the new multi-artifact release model by defining canonical release integrity, versioned signing/attestation statements, cross-language canonicalization, and real client-side provenance verification while preserving legacy formats. This milestone does not change package compatibility selection logic, build CI matrices, redesign namespace signing policy, or introduce a universal trust/quality score.
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
> Scope note: make installation target-aware for new-format Python Tool releases: select compatible artifacts deterministically, verify release/artifact integrity, install locked dependencies for the consumer environment, and fail or recover safely when no compatible artifact exists. This milestone preserves the legacy install path and does not add Rosetta/emulation, silently change locked versions, or implement CI publishing.
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
> Scope note: make the new multi-target release model practical for maintainers by adding secure headless signing and a first official GitHub Actions workflow that builds target artifacts in parallel and performs one final atomic publish. The core CLI/protocol must remain CI-provider-neutral. This milestone does not build a generalized CI platform, require GitHub Actions for local publishing, or implement additional CI providers.
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
> Scope note: finish Stage 1 by documenting the new Python Tool dependency/portability/release/integrity model, migration expectations, telemetry contract, and newly introduced CLI behavior so authors can use the hardened system without founder guidance. This milestone updates Stage 1-facing docs and examples only; it does not perform the Stage 2 category-language/IA rewrite or expand the feature set beyond what earlier milestones implemented.
> Implementation notes:
> - Documentation must describe the new Python Tool model from an author’s point of view, not merely mirror internal implementation terminology.
> - Make the legacy/new distinction explicit:
>   - legacy Python Tools with vendored `_vendor` content remain valid;
>   - new-format Python Tools should declare dependencies and let AgentPM resolve/install them for the target.
> - Explain that local publish remains supported and may produce only the compatibility surface the local build can truthfully claim.
> - Explain that multi-target CI publishes one immutable AgentPM Tool version containing multiple target artifacts.
> - Make immutability explicit: adding another target to an already finalized release requires a new semantic version.
> - Document the integrity model in terms sophisticated developers care about:
>   - release-level integrity;
>   - per-artifact integrity;
>   - author signature;
>   - registry attestation;
>   - install-time verification.
> - Keep Stage 2 category language out of scope. Do not use this milestone to rewrite every occurrence of Agent/Package terminology across the product.
> - Update docs/examples that currently teach manual `_vendor` installation so the new default path is clear while the old method remains documented as legacy/manual.

- [ ] Update Python Tool authoring docs:
  - [ ] declare dependencies in `agent.json`;
  - [ ] explain that `agent.json` is author intent;
  - [ ] explain that `agent.lock` records AgentPM-resolved state;
  - [ ] explain AgentPM-managed dependency resolution;
  - [ ] explain that the user’s project does not need to use `uv`;
  - [ ] portable pure-Python behavior;
  - [ ] native target-specific behavior;
  - [ ] local single-target publish;
  - [ ] CI multi-target publish.
- [ ] Add a migration section for existing Python Tool authors:
  - [ ] existing vendored packages remain valid;
  - [ ] no forced republish;
  - [ ] compatibility may remain unknown/legacy until a new version adopts the new format;
  - [ ] how to migrate a new version away from `_vendor`.
- [ ] Mark `_vendor` approach as legacy/manual and explain:
  - [ ] why it can capture architecture-specific native dependencies;
  - [ ] when it may still be intentionally used;
  - [ ] why declared dependencies are preferred for portability.
- [ ] Document lockfile evolution:
  - [ ] existing lockfiles remain supported;
  - [ ] new Python dependency state is additive;
  - [ ] new-format release/install behavior is versioned.
- [ ] Document release/artifact model:
  - [ ] one logical Tool version may contain multiple target artifacts;
  - [ ] artifacts share manifest/dependency state;
  - [ ] artifacts differ only in target-specific payload/build metadata.
- [ ] Document artifact immutability/new-version requirement for added targets.
- [ ] Document install target selection:
  - [ ] exact target;
  - [ ] portable `any`;
  - [ ] no-compatible-artifact failure;
  - [ ] interactive recovery rules;
  - [ ] no silent locked-version substitution.
- [ ] Document integrity/signature model at a developer-appropriate level:
  - [ ] release digest;
  - [ ] artifact digest;
  - [ ] author signature;
  - [ ] registry attestation;
  - [ ] client-side verification;
  - [ ] relevant `--require-*` flags.
- [ ] Document headless/CI signing flow and secret-handling guidance.
- [ ] Document official GitHub Actions workflow:
  - [ ] matrix builds;
  - [ ] artifact collection;
  - [ ] one final atomic publish;
  - [ ] provider-neutral CLI equivalent.
- [ ] Document telemetry/privacy contract and opt-out:
  - [ ] minimal anonymous telemetry;
  - [ ] allowed high-level fields;
  - [ ] excluded sensitive/content fields;
  - [ ] `AGENTPM_TELEMETRY=0`;
  - [ ] persistent opt-out if implemented.
- [ ] Update CLI help text where new commands/options are introduced.
- [ ] Update registry/docs examples to use `agentpm-harness` Template execution surface where relevant.
- [ ] Update feedback/help documentation so users know where to submit product feedback versus reproducible GitHub bugs.
- [ ] Ensure Stage 2 terminology work remains separate and is not pulled into this milestone.
