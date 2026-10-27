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

- [x] Fix Explore page-size mismatch:
  - [x] choose one authoritative page size for Explore;
  - [x] ensure the frontend sends that limit explicitly to `/search`;
  - [x] derive displayed page counts from the same effective limit;
  - [x] avoid duplicating the page-size constant in multiple layers where possible.
- [x] Fix cursor history so Page 2 can return to Page 1:
  - [x] represent the initial/start-of-results state explicitly in cursor history or otherwise make Page 1 a valid previous destination;
  - [x] do not rely only on the current frontend special case that strips the cursor to return to Page 1.
- [x] Remove/raise the `HISTORY_MAX = 10` limitation so sequential Previous navigation can return through the full result set.
- [x] Add regression coverage for:
  - [x] 0 results;
  - [x] less than one page;
  - [x] exactly one page;
  - [x] page-size + 1;
  - [x] 2 pages;
  - [x] 3+ pages;
  - [x] >10 pages;
  - [x] 1 → 2 → 3 → 2 → 1.
- [x] Verify the API only emits `next_cursor` when at least one stream can truly advance and only emits `prev_cursor` when a real previous page exists.
- [x] Verify mixed namespace/package cursor pagination remains stable:
  - [x] package-only contribution on a page;
  - [x] namespace-only contribution on a page;
  - [x] both streams contributing;
  - [x] one stream reaching EOF before the other.
- [x] Fix private-result inconsistency across Relevance/Newest versus Trending/Most Downloaded:
  - [x] apply the same authorized-private visibility rule to every supported sort;
  - [x] preserve anonymous/public behavior.
- [x] Test and harden strict/relaxed relevance behavior so totals/page counts do not become misleading mid-pagination:
  - [x] avoid switching candidate-set semantics in a way that invalidates an existing cursor;
  - [x] if strict/relaxed mode becomes part of the cursor contract, encode it explicitly rather than inferring it independently on each page.
- [x] Replace `Loading tools...` with:
  - [x] `Searching AgentPM…` for mixed searches;
  - [x] kind-specific loading text for filtered searches.
- [x] Add explicit no-results UI:
  - [x] preserve the user’s current query text;
  - [x] show `Clear filters` when filters are active;
  - [x] show `Search all kinds` when a kind constraint is active;
  - [x] do not introduce the future AgentPM Developer prompt CTA in Stage 1.
- [x] Add visible recoverable search-error state:
  - [x] distinguish transport/server failure from a legitimate zero-result query;
  - [x] provide a retry/recovery affordance where appropriate.
- [x] Replace unstable array-index React keys on search result cards with stable entity IDs where applicable.
- [x] Verify query/kind/sort/filter changes reset pagination to page 1 rather than attempting to reuse a stale cursor.
- [x] Verify browser Back/Forward restores:
  - [x] query;
  - [x] kind;
  - [x] sort;
  - [x] filters;
  - [x] cursor/page position.

## Release Band 1: Explore Correctness Baseline
Covered milestones: 1.
This gives us a correct, predictable Explore surface: sane page-size/cursor behavior, consistent visibility rules, working history/state restoration, and real loading/empty/error states. It adds no new discovery capability, which is exactly why it can ship first and alone — every later discovery band exercises this state machine, and shipping the fixes early means the rest is built on a baseline that is already proven in production.

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
> - Do not expose a field merely because it exists in `agent.json`; filters should correspond to natural discovery questions for the best-fit initial user.
> - Stage 2 may add objective Package Health facets later. Structure the backend/filter representation so adding those does not require another query-system rewrite.
> - `agentpm-harness` should become a real Template execution-surface enum rather than overloading `agentpm-run` when Harness is the intended execution path.
> - This milestone lands the **schema and lint** change only. Updating the published example Templates to declare it is **Milestone 3.1**, deliberately deferred to the end of this release band: publishing a Template that declares `agentpm-harness` requires a CLI whose bundled schema already accepts it, so the CLI has to be released first.

- [x] Move Kind into the Filters area and remove the disconnected kind-tab treatment if the final UX confirms this direction.
- [x] Replace `Filters: Coming soon` with working filters.
- [x] Implement a reusable filter-query representation shared by frontend and backend rather than one-off query parameters handled independently in multiple code paths.
- [x] Implement URL-addressable filter state:
  - [x] preserve multiple selected values;
  - [x] ensure deterministic serialization/order where practical;
  - [x] clear stale cursor whenever filters change.
- [x] Implement universal filters:
  - [x] Kind;
  - [x] License;
  - [x] Signed;
  - [x] Updated/published recency.
- [x] Define initial Updated/published recency buckets in one place; recommended starting options:
  - [x] past 30 days;
  - [x] past 90 days;
  - [x] past year.
- [x] Implement Tool-specific Runtime filter:
  - [x] Python;
  - [x] Node.
- [x] Implement Agent Package Includes filter:
  - [x] Skills;
  - [x] Knowledge;
  - [x] Memory;
  - [x] Profiles;
  - [x] Loop;
  - [x] MCP.
- [x] Define `Includes` from the authored/resolved Agent composition model rather than from incidental README text.
- [x] Implement Template filters:
  - [x] Stack;
  - [x] Execution Surface.
- [x] Implement Skill Runtime Compatibility filter using the closed runtime compatibility enum only.
- [x] Implement Knowledge Mode filter:
  - [x] context;
  - [x] vector.
- [x] Implement Memory Supports Retrieval filter:
  - [x] key;
  - [x] filter;
  - [x] chronological;
  - [x] full text;
  - [x] semantic.
- [x] Leave Profile and Loop without kind-specific v1 filters.
- [x] Implement OR-within/AND-across filter-group semantics.
- [x] Implement AND semantics for selected Agent Package Includes values.
- [x] Apply filters server-side before:
  - [x] relevance ranking;
  - [x] non-relevance sorting;
  - [x] pagination.
- [x] Ensure strict and relaxed relevance passes use the same hard filter predicate and never broaden outside the user-selected filter set.
- [x] Ensure authorized-private visibility is applied before filters/sort and remains consistent across all sorts.
- [x] Build the faceted-query layer so new Stage 2 objective filters can be added without ad hoc branching.
- [x] Add `agentpm-harness` to Template execution-surface schema. **Cross-stage note:** `$defs/templateMetadata.execution_surfaces` is a closed enum and Stage 2's APDS v1.0.0 structural freeze (Stage 2 M1B) is gated on this landing. Flag the merge to the Stage 2 track.
- [x] Update lint/schema tests for:
  - [x] accepting `agentpm-harness`;
  - [x] continuing to accept existing execution surfaces;
  - [x] rejecting unknown execution surfaces.

## Milestone 3: Search Relevance Expansion
> Scope note: increase deterministic search recall by indexing selected user-meaningful semantic metadata at deliberately lower weights while preserving direct identity matches as the strongest signals. Dependency-identity indexing covers every kind that declares dependencies — Agent Packages, Templates and Skills — using direct declarations only. This milestone includes a fixed relevance fixture set to guard against noisy regressions. It does not index full READMEs/arbitrary manifest text, add embeddings or LLM reranking, change the faceted-filter contract, or build recommendations.
> Implementation notes:
> - Keep the current PostgreSQL FTS + trigram architecture. This milestone is an evolution of the existing weighted materialized search index, not a search-engine replacement.
> - Preserve the current intent that identity-level matches dominate:
>   - package/component name strongest;
>   - namespace strong;
>   - top-level description lower;
>   - richer kind-specific semantic metadata lower still;
>   - dependency/component identities very low weight.
> - A search for a dependency such as `github-tool` should be able to surface an artifact that depends on it, but the actual `github-tool` result must clearly outrank the dependent artifact. This applies to **Agent Packages, Templates and Skills** — all three declare dependencies, and indexing only the Agent edge would make a Skill built around a Tool invisible to a search for that Tool.
> - Index **direct** declared dependencies only. Transitive resolution is out of scope here and would blur the weight hierarchy this milestone exists to protect.
> - Indexing dependency identities for recall is **not** reverse-dependency discovery as a product feature ("Used by these Agent Packages"), which stays deferred. This is search recall at the lowest weight class.
> - Keep typo/fuzzy trigram behavior focused on package names and namespace handles. Do not fuzzy-match every long descriptive field.
> - Avoid indexing full README bodies: authored prose contains many incidental terms and is likely to create noisy matches.
> - Prefer indexing human-meaningful fields over implementation plumbing such as paths, hashes, raw schemas, generated metadata, and internal IDs.
> - The relevance fixture set should assert important relationships/ordering properties, not freeze every exact numeric rank forever.

- [x] Extend the search materialized/indexed document with selected lower-weight semantic fields.
- [x] Preserve strongest weighting for:
  - [x] name;
  - [x] namespace;
  - [x] top-level description.
- [x] Define explicit additional weighted search vector(s) or equivalent structure so lower-weight semantic fields are not accidentally promoted to the same relevance class as top-level description.
- [x] Add lower-weight Agent Package semantic fields:
  - [x] authored Examples prompt titles/text where available;
  - [x] useful composition/binding labels that help discovery;
  - [x] avoid indexing internal phase/binding IDs unless they carry real user meaning.
- [x] Add very-low-weight **direct** dependency/component package identity matches for every kind that declares dependencies. Index declared direct dependencies only — do not resolve and index the transitive closure, which would inflate the document and blur the weighting hierarchy:
  - [x] **Agent Package** (`kind: agent`) declared identities:
    - [x] Tools;
    - [x] Skills;
    - [x] Knowledge;
    - [x] Memory;
    - [x] Profiles;
    - [x] Loop;
    - [x] MCP-bound Tool package identities where available.
  - [x] **Template** (`template.dependencies`, a `templateDependencyGroup`) declared identities:
    - [x] Tools and Agent Packages — both are schema-required, and the Agent Package edge is unique to Templates;
    - [x] Skills, Knowledge, Memory and Profiles where declared.
  - [x] **Skill** declared Tool identities — note these live at the **manifest top level** (`tools`), not under `skill`, which is easy to miss. Already populated in the live corpus: `issue-triage-playbook`, `slack-status-update`, `research-brief-playbook`.
  - [x] Use one shared very-low-weight class for all three so no kind's dependency edges outrank another's.
- [x] Add Template:
  - [x] display name;
  - [x] use case;
  - [x] stack;
  - [x] execution surfaces.
- [x] Add Skill descriptive compatibility/runtime metadata:
  - [x] controlled runtime labels;
  - [x] other descriptive fields only where they are sufficiently normalized to be useful.
- [x] Add Knowledge:
  - [x] content type;
  - [x] language;
  - [x] mode;
  - [x] document descriptions.
- [x] Add Memory:
  - [x] space descriptions;
  - [x] record type descriptions;
  - [x] operation descriptions;
  - [x] retrieval modes.
- [x] Add Profile:
  - [x] role;
  - [x] expertise;
  - [x] objectives;
  - [x] audience description.
- [x] Add Loop:
  - [x] archetype;
  - [x] phase objectives;
  - [x] outcome descriptions.
- [x] Keep trigram fuzzy matching focused on names/namespaces.
- [x] Do not index:
  - [x] full READMEs;
  - [x] arbitrary manifest bodies;
  - [x] schema blobs;
  - [x] hashes;
  - [x] generated build metadata;
  - [x] package-relative file paths as general discovery text.
- [x] Create a fixed search relevance fixture set using representative artifacts across multiple kinds.
- [x] Include fixture cases for:
  - [x] exact package name;
  - [x] exact namespace;
  - [x] typo;
  - [x] direct name vs dependency-only match;
  - [x] dependency-only match for each declaring kind — Agent Package, Template and Skill — so no kind's edges are silently unindexed;
  - [x] Template discovered through its declared **Agent Package** dependency, the one edge no other kind produces;
  - [x] metadata-only match;
  - [x] multi-word query;
  - [x] filtered query;
  - [x] no-result query.
- [x] Assert direct package identity matches outrank dependency-only matches, for each of Agent Package, Template and Skill dependents.
- [x] Assert a dependency-only match does not outrank another kind's dependency-only match purely because of the dependent's kind.
- [x] Assert lower-weight semantic metadata can make an otherwise-unfindable relevant artifact discoverable without displacing obvious identity matches.
- [x] Assert result ordering is deterministic under ties using stable tie-breakers.
- [x] Document weighting rationale in code/tests so future changes do not accidentally flatten the intended hierarchy.

## Milestone 3.1: Release the CLI, Then Update Example Templates
> Scope note: close out the release band by publishing the CLI that carries the Milestone 2 schema change, then using that CLI to republish the example Templates whose real execution path is Harness. Owns the CLI release and the `agentpm-examples` Template updates. It does not change the schema, add execution surfaces, author new Templates, or alter Template functionality beyond the declared execution surface.
> Implementation notes:
> - **Order matters and is the reason this milestone exists.** `agentpm publish` validates against the CLI's bundled manifest schema. Any CLI built before Milestone 2 rejects `agentpm-harness` as an invalid enum value, so the Templates cannot be republished until the new CLI is released and installed locally. Release first, verify the installed version accepts the surface, then update Templates.
> - **Consumers are not at risk and no tolerance release is needed.** `TemplateMetadata.execution_surfaces` deserializes as `Vec<String>` with `#[serde(default)]`, not a typed enum, and registry-fetched Templates are not schema-validated client-side — only `agentpm new ./local-path` validates a Template's own manifest. An older CLI installing a Template that declares `agentpm-harness` reads the value and ignores it.
> - Published versions are immutable. Each updated Template is a **new version**; the existing versions keep declaring `agentpm-run` and must remain installable unchanged.
> - `execution_surfaces` is an array, so a Template that is genuinely both SDK-hosted and Harness-executed should declare both rather than swapping one for the other.
> - Judge each Template by what it actually does, not by its name. The signal is a shipped `agentpm.harness.json` **plus** entrypoints that invoke `agentpm harness`. A Template that merely hosts Harness through an SDK is correctly declared as that SDK surface already.

- [x] Release the CLI version carrying the Milestone 2 manifest-schema change and record the released version number.
- [x] Install that CLI locally and confirm it lints a Template manifest declaring `agentpm-harness` and still rejects an unknown surface.
- [x] Audit every Template in `agentpm-examples` against the actual execution path before editing. Current state at the time of writing:
  - [x] straight `agentpm-run` → `agentpm-harness`: `harness-minimal-agent`, `harness-provider-lab`, `harness-mcp-bridge` — each ships `agentpm.harness.json` and runs `agentpm harness` from its entrypoints;
  - [x] decide one way or the other for `harness-embedding-provider` — declares `node-sdk`, ships harness config, and its entrypoint runs `agentpm harness`, so `["node-sdk", "agentpm-harness"]` is the likely answer;
  - [x] leave `harness-sdk-host-node` and `harness-sdk-host-python` unchanged unless review disagrees — the SDK hosts Harness there, so the existing SDK surface is the honest declaration;
  - [x] leave `cli-automation-worker`, `mcp-tool-server`, `research-assistant-node`, `support-assistant-workspace` and `triage-worker-python` unchanged — no Harness execution path.
- [x] Bump each changed Template as a new version.
- [x] Republish each changed Template as a new version.
- [x] Verify prior published versions remain byte-identical and installable, with digests recorded before and after.
- [x] For each republished Template, run `agentpm new` from a clean directory and confirm the scaffold still generates, resolves, installs and lints clean.
- [x] Confirm each republished Template's advertised execution surface actually works end to end, rather than only passing lint.
- [x] Record which Templates were deliberately **not** changed and why, so the remaining `agentpm-run` declarations read as intentional.

## Release Band 2: Faceted Filters and Search Relevance
Covered milestones: 2-3.1.
This gives us a reusable server-side faceted-search foundation, the first objective universal and kind-specific filters, `agentpm-harness` as a Template execution surface, and deliberately low-weighted indexing of selected user-meaningful semantic metadata with direct identity matches preserved as the strongest signal, guarded by a fixed relevance fixture set. Indexing full READMEs or arbitrary manifest text, embeddings, and LLM reranking all stay out of scope. Filters and recall ship together because a filter set over a thin index produces confidently wrong empty states; neither half is convincing without the other. The band closes with Milestone 3.1: release the CLI carrying the new `agentpm-harness` execution surface, then use that CLI to republish the example Templates whose real execution path is Harness. That ordering is forced — a Template declaring the new surface cannot be published by a CLI whose bundled schema predates it.

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
> - **Auth semantics changed here, and it reaches further than stars.** `auth_optional` previously returned 401 whenever a bearer token failed verification, so a stale Clerk session or a revoked PAT broke *public* reads. It now falls through to the handler as anonymous (`g.auth` stays `None`; `auth_required` is unchanged and still 401s). That is the correct meaning of optional auth, but it changes behavior across all 13 `auth_optional` endpoints — tool and namespace detail GETs, `/search`, and `/install/resolve` + `/install/init` — in two ways worth tracking:
>   1. **private resource + bad token now reads as 404 rather than 401**, so "your credentials were rejected" is no longer distinguishable from "it does not exist" (follow-up in Milestone 9);
>   2. **public resource + bad token now returns silently degraded data rather than an error** — most visibly `starred: false` on a package the viewer has actually starred, and `/namespaces/{id}/members` omitting emails for an org admin (follow-up in Milestone 6).

- [x] Add star persistence model:
  - [x] user/account identity;
  - [x] artifact identity;
  - [x] `created_ts`;
  - [x] unique constraint per user/identity.
- [x] Add star/unstar API:
  - [x] idempotent enough for normal optimistic UI behavior;
  - [x] correct authorization for public/private artifacts.
- [x] Enforce authorization/visibility for private artifacts.
- [x] Add aggregate star count to relevant registry/search DTOs and materialized/search data needed for sorting.
- [x] Add current-user `starred` state to detail/API responses where needed for interactive UI.
- [x] Add passive star counts to Explore and namespace result cards.
- [x] Add reusable detail-page interactive star rail component:
  - [x] unstarred state;
  - [x] starred state;
  - [x] immediate count update;
  - [x] subtle border/background/accent transition;
  - [x] no confetti/bounce/social gamification.
- [x] Remove Tool-only `Score & rating / Coming Soon` UI.
- [x] Remove/hide Tool Evaluations tab while it has no real content.
- [x] Replace weekly `0 / -100%` style popularity presentation with durable signals:
  - [x] total installs;
  - [x] stars;
  - [x] do not show a misleading percentage delta when the base period is zero.
- [x] Refactor trending signal computation so it is computed for the full eligible result set.
- [x] Separate top-N presentation shortlist from trend signal computation.
- [x] Remove top-12-per-kind truncation from the canonical Explore trending source.
- [x] Preserve or recreate a top-N-per-kind query/view only for homepage/curated presentation where useful.
- [x] Implement Trending ranking:
  - [x] installs last 7d;
  - [x] total stars;
  - [x] all-time installs;
  - [x] latest publish;
  - [x] stable tie-breaker.
- [x] Keep the Trending implementation understandable in code; avoid introducing a black-box weighted score unless required by existing query structure.
- [x] Add star timestamps even if recent-star scoring is not used yet.
- [x] Add tests for:
  - [x] all-zero recent activity;
  - [x] equal recent installs but different stars;
  - [x] equal stars but different all-time installs;
  - [x] full tie resolved by freshness/stable ID.
- [x] Verify private stars never affect public discovery/trending.

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

- [x] Implement namespace pinned artifacts.
- [x] Add persistence for pin identity and explicit order.
- [x] Allow Owner/Admin management only.
- [x] Limit pins to namespace-owned visible artifacts.
- [x] Enforce pin maximum (default target: 6 unless changed).
- [x] Add explicit pin ordering:
  - [ ] drag/drop if it fits current UI patterns cleanly; or
  - [x] deterministic move-up/down/order controls.
- [x] Ensure deleting/unpublishing/yanking an artifact does not leave a broken visible pin:
  - [ ] either remove invalid pins automatically; or
  - [x] suppress them and expose cleanup to Owner/Admin.
- [x] Hide empty pin section from normal visitors.
- [x] Show empty-state management prompt to Owner/Admin.
- [x] Render pinned items for visitors when pins exist.
- [x] Reuse shared result/card primitives for pinned items where practical rather than creating a visually unrelated card system.
- [x] Replace namespace `0/week` popularity signals with shared improved result-card signals from Milestone 4.
- [x] Add namespace-scoped text search.
- [x] Reuse shared faceted filters within namespace package list:
  - [x] omit Namespace selector because the namespace is already fixed;
  - [x] retain Kind and applicable universal/kind-specific filters.
- [x] Only show recent activity if there is recent activity
- [x] Reuse shared sorts and pagination.
- [x] Enforce namespace as a server-side hard search constraint that cannot be overridden by query params.
- [x] Keep namespace filter/search state URL-addressable.
- [x] Remove/simplify current mechanical type summary where redundant with real filtering.
- [x] Verify namespace discovery and global Explore produce consistent result behavior for equivalent constraints.

## Release Band 3: Popularity Signals and Namespace Discovery
Covered milestones: 4-5.
This gives us identity-level stars with visibility-safe aggregates, a Trending signal computed over the full eligible result set with deterministic ordering in place of the current installs-only score, and curated namespace pages with owner/admin pins. Stars are grouped with namespace discovery because both land in the shared Explore search/filter/sort/pagination foundation — star counts become a Trending ranking input and appear on namespace result cards. Stars are explicitly not quality, trust, or Package Health, and no weighted quality score is introduced here. The shared star UI primitive defined in this band is applied across every detail-page kind in the next one.

## Milestone 6: Package Detail Shared-Shell Hardening
> Scope note: apply shared package/component detail-page hardening consistently across all kinds while preserving the useful specialized inspection tabs already present. This milestone adds the shared star treatment, removes unfinished placeholder UI, and improves popularity signals. It does not redesign the detail-page information architecture, migrate Agent/Agent Package terminology, change authored README content, or implement Stage 2 Package Health presentation.
> Implementation notes:
> - Every kind currently uses essentially the same identity/version shell. Make shared changes once in that shell rather than patching each kind independently.
> - Keep identity-level and selected-version data conceptually separate:
>   - identity/package-level: star state/count, long-lived popularity, name/namespace, and the Last publish summary pairing the latest published timestamp with `latest_version` regardless of selected version;
>   - selected-version-level: selected version, SHA-256, signatures, registry attestation, malware result, version-specific manifest metadata, and any explicitly selected-version publish timestamp.
> - Changing the selected version must not make the package appear newly starred/unstarred because the star belongs to identity.
> - Security remains version-specific and should not be reinterpreted as an identity-level badge in Stage 1.
> - Do not remove useful specialized views while cleaning up the shared shell.
> - README content is author-published. Do not rewrite or normalize README wording as part of this milestone.

- [x] Implement the Milestone-4 shared star rail across all artifact kinds.
- [x] Place the star rail consistently at the bottom of the primary identity card unless the existing shared layout requires a small adaptation.
- [x] Replace weak weekly change signal in shared sidebar/shell with the durable popularity treatment established in Milestone 4.
- [x] Preserve identity-level stars while version selector changes.
- [x] Synchronize the star rail and mobile/desktop Stars cells through shared identity-keyed state, including optimistic writes, authoritative response counts, and rollback. Keep `PackageStatsGrid`, its `Panel` wrappers, Installs, Last publish, and `agoText` server-rendered; only the Stars cell reads client context.
- [x] Handle the stale-session star state introduced by Milestone 4's `auth_optional` change. A viewer whose token no longer verifies is served the anonymous payload, so `starred` returns `false` for a package they have in fact starred — the rail renders unstarred, and clicking it hits the `auth_required` star endpoint and 401s. Decide and implement the behavior rather than leaving the mismatch: surface the signed-out state in the rail, or have the failed write prompt re-authentication. Do not ship a control that shows the wrong state and then errors on use.
- [x] Preserve version-specific Security/integrity data.
- [x] Remove Tool Score & Rating placeholder.
- [x] Hide Tool Evaluations placeholder/tab until actual evaluation content exists.
- [x] Ensure removing placeholder sections does not leave empty cards, spacing gaps, or dead navigation states.
- [x] Verify install/use action areas remain kind-appropriate:
  - [x] Tool install/run/load behavior;
  - [x] Agent Package install/load behavior unchanged in Stage 1;
  - [x] Template bootstrap flow;
  - [x] Knowledge inspect/query flow;
  - [x] Memory/Profile/Loop/Skill component actions.
- [x] Verify all existing kind-specific tabs still work:
  - [x] Agent Package Bindings/Examples;
  - [x] Memory Lifecycle/Record Contracts;
  - [x] Loop Phases;
  - [x] Security;
  - [x] README;
  - [x] Knowledge inspection metadata;
  - [x] Template bootstrap metadata;
  - [x] Profile detail metadata;
  - [x] Skill detail metadata.
- [x] Keep the shared Security/detail contract extensible for the new release model implemented later in Stage 1; when Milestones 13A–14C land, update the same surface rather than introducing a separate new-format-only page.
- [x] Verify Security tab continues to display the selected version’s:
  - [x] package/artifact type;
  - [x] digest;
  - [x] author-signature state;
  - [x] registry-attestation state;
  - [x] malware status;
  - [x] license.
- [x] Verify no shared-shell change conflates identity-level and version-level data.
- [x] Do not add the Stage-2 `Run with Harness` Agent Package action in this milestone; record that as Stage 2 work only.

> Milestone 6 verification (2026-10-14): audited all eight package kinds. The shared footer rail, durable installs/stars panels, and Tool placeholder removal were already present from Milestone 4. Added pending-write protection, rollback/error feedback, rejected-session sign-out/sign-in recovery with a return URL, and shared star state after writes. The latest-publish timestamp is now labeled with `latest_version`, independently of the selected version. `PackageDetailShell.test.tsx` covers all eight layouts, version changes, actions, and specialized tab links; `PackageSecurity.test.tsx` exercises the real fetch/schema/render path for selected-version type, digest, author signature, attestation, malware, and license across all eight kinds. Existing specialized overview, Bindings, Phases, Lifecycle, Record Contracts, Manual, and README suites also pass. Full web suite: 132 passed; API suite: 352 passed, 65 subtests passed, 5 optional PostgreSQL cases skipped. Web typecheck, lint (six existing warnings), and production build pass. The new-format Security UI and verification checklist now lives in Milestone 14C alongside the public Security/read DTO evolution; the current shared detail contract and version-specific Security routes remain the extension point. `Run with Harness` remains Stage 2 work.

> Milestone 6 follow-up verification (2026-10-14): split the client `StarStatCell` from the server-only stats grid and connected all eight detail layouts through `PackageStarProvider`. The provider seeds state only on its identity-keyed mount; successful writes update context without a route refresh, and stale same-identity RSC props cannot overwrite the user's result. Concurrent changes from another tab/device are picked up on remount rather than by same-identity prop reconciliation. Tests exercise both mobile and desktop Stars cells during optimistic writes, authoritative count updates, and failure rollback, including Clerk sign-out failure preserving the sign-in message. Corrected `agoText` to use UTC accessors for day/month/year boundaries; all nine date cases pass under both `TZ=UTC` and `TZ=America/Los_Angeles`. The full frontend suite now has 142 passing tests. Typecheck and lint pass (the six existing warnings remain). A production build run in an isolated temporary copy passes; its client-reference manifest includes the provider, rail, and Stars cell while excluding the stats grid and date formatter.

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

- [x] Audit and implement unique `<title>` and meta description for:
  - [x] landing;
  - [x] pricing;
  - [x] package/version pages;
  - [x] namespace pages;
  - [x] docs pages.
- [x] Ensure dynamic package metadata derives from public package/version data server-side and does not require client hydration to become crawlable.
- [x] Ensure namespace metadata derives from public namespace data server-side.
- [x] Add/verify canonical tags.
- [x] Make immutable package-version pages canonical to themselves.
- [x] Define and implement docs-version canonical behavior:
  - [x] decide whether `/latest/` is canonical or points to a concrete docs version;
  - [x] ensure old versions do not duplicate-current content unintentionally.
- [x] Add/verify OpenGraph metadata:
  - [x] title;
  - [x] description;
  - [x] canonical URL;
  - [x] image where available.
- [x] Add/verify Twitter metadata.
- [x] Add/verify share image behavior for landing, package, namespace, pricing, and docs surfaces where practical.
- [x] Add sitemap coverage for intended public pages:
  - [x] landing;
  - [x] pricing;
  - [x] blog index and individual blog posts (follow-up completed with Milestone 8);
  - [x] docs;
  - [x] public namespaces;
  - [x] public package/version pages.
- [x] Audit robots directives.
- [x] Set arbitrary Explore query/filter result pages to `noindex,follow`.
- [x] Canonicalize arbitrary Explore query/filter URLs to `/explore` unless a deliberate indexable facet is introduced later.
- [x] Prevent private/authenticated/account/settings surfaces from indexing.
- [x] Prevent 404/error pages from indexing.
- [x] Check heading hierarchy:
  - [x] one meaningful H1 per page where appropriate;
  - [x] logical H2/H3 structure.
- [x] Check basic image alt/accessibility metadata on primary public pages.
- [x] Add regression tests for metadata/canonical/robots behavior where practical.
- [x] Leave category-facing metadata copy refinement to Stage 2.

Implementation and verification evidence (2026-10-14): see the SEO section of `test-plan.md`
and `assets/milestone-7/seo-evidence.json` / `assets/milestone-7/error-evidence.json`.
Registry and web changes only; no CLI/SDK release or Stage 2 copy work. Existing API
pre-commit limitations are recorded in the test plan rather than treated as passing checks.

## Release Band 4: Package Detail and Technical SEO
Covered milestones: 6-7.
This gives us a hardened shared package-detail shell across kinds plus the registry technical SEO/indexing baseline: unique server-rendered metadata, canonical behavior, OpenGraph/social previews, sitemap and robots boundaries, and crawl control over arbitrary Explore query/filter URLs. Broad structured-data/schema.org work stays out of scope here and waits on Stage 2 category semantics. SEO is deliberately last in the web sequence — canonicals and sitemaps are only worth emitting once the detail routes and discovery URLs underneath them have stopped moving. At the end of this band the public registry is technically ready for inbound traffic; Band 6 adds measurement before traffic is intentionally increased.

## Milestone 8: CLI Scaffolding and Lint Quality
> Scope note: fix the highest-impact CLI creation and linting rough edges so a newly scaffolded artifact is valid and lint output prioritizes actionable domain errors over schema noise. This milestone preserves machine-readable lint contracts and focuses on validation/rendering quality. It does not redesign all CLI output, add new runtime behavior, change AgentPM package semantics, or perform Stage 2 terminology migration. It **does** own the root kind-union diagnostic restructure that Stage 2's APDS rule-ID/JSON-path diagnostics depend on.
> Implementation notes:
> - Current confirmed first-run defect: `agentpm init --kind tool` scaffolds `files: []`, `entrypoint.command: ""`, and `entrypoint.args: []`, then immediately fails `agentpm lint`; the other seven kinds lint clean.
> - Prefer generating a minimal valid/runnable Tool stub over deliberately generating an invalid placeholder plus warning.
> - AgentPM semantic validators already produce the best lint messages. Preserve those and remove generic schema noise that obscures them.
> - Current `oneOf`/`anyOf` failure cliffs can serialize the entire containing object and can be duplicated through both `/properties/<kind>` and `/dependentSchemas/<kind>`.
> - The worst cliff is the **root kind union itself**. Kind selection is an eight-branch top-level `oneOf`, so any kind-specific violation reports as `{entire manifest} is not valid under any of the schemas listed in the 'oneOf' keyword` at path `/oneOf` — no failing branch, no failing field. Verified on CLI 0.1.33 with an Agent manifest missing `tools`. Because `kind` is always present and is the discriminator, the branch is knowable: either restructure root dispatch to `if`/`then` per kind, or keep `oneOf` and map the failure to the branch matching the declared `kind` before rendering.
> - **This milestone owns that restructure, and Stage 2 depends on it.** Stage 2's APDS work requires every diagnostic to carry a stable rule ID *and* a manifest JSON path; a verdict anchored at `/oneOf` cannot satisfy that. Fixing root dispatch here keeps the diagnostic layer in one stage instead of having Stage 2 re-cut the same code.
> - Suppress a generic parent oneOf/anyOf message when a more specific semantic/domain error already exists at or beneath the same instance path.
> - Deduplicate only genuinely equivalent errors; do not hide the only available structural error.
> - Human rendering may improve significantly while JSON/NDJSON remains contract-stable.

- [x] Make `agentpm init --kind tool` scaffold lint-valid output.
- [x] Prefer a minimal runnable Tool stub:
  - [x] valid `files`;
  - [x] valid interpreter command;
  - [x] matching Python runtime metadata so published scaffolds match the Runtime facet;
  - [x] valid non-empty args;
  - [x] minimal source file needed by the entrypoint.
- [x] Keep the Tool scaffold small; do not turn `init` into a large framework/template generator.
- [x] Harden Tool package validation so a `manifest.files` entry that does not exist is a fatal packaging/publish error rather than a warning followed by silent omission.
- [x] Correct stale Tool packaging comments/docs that describe `files` entries as globs when the implementation treats them as literal file/directory paths, unless true glob support is deliberately added.
- [x] Verify all eight `init` kinds lint clean immediately.
- [x] **Cross-stage, ship with this milestone's CLI release:** add `standard` to the manifest schema's root `properties` with a permissive shape so the field is *accepted and ignored*. Do not emit it, validate it, or require it — Stage 2 M3A owns emission. Rationale: the root is `additionalProperties: false`, so once Stage 2 publishes packages declaring `standard`, every CLI released before this change hard-fails `agentpm lint` (exit 1) and fails `agentpm new` inside `validate_generated_manifests_blocking`. Relax the root by **this one field only**; a misspelled top-level key must still be rejected. See Stage 2 `spec.md` §2 "Old-client forward compatibility" and Stage 2 M1A.1.
- [x] Verify the `standard` field survives every manifest-rewriting path: `agentpm install <pkg>` write-back and `knowledge build` (which writes by default; there is no `--write` flag) must preserve it. Regression tests use `standard` plus one extra unknown key; Knowledge build uses a local schema override for the extra key so the bundled root stays closed. Keep `serde_json` pinned without `preserve_order`; alphabetized rewrites are expected and lose nothing.
- [x] Record the released CLI version carrying this tolerance change — **0.1.36**, published/installed by operator confirmation on **2026-10-22** and verified by a live Production `cli_first_used` payload carrying `cli_version: 0.1.36`. Stage 2 M4B must name **0.1.36** as its minimum CLI floor. This satisfies M3A's publication prerequisite only, not its other dependencies or acceptance criteria.
- [x] Refactor human lint rendering:
  - [x] semantic/domain errors first;
  - [x] suppress redundant parent oneOf/anyOf errors when covered by a more specific descendant/domain error;
  - [x] deduplicate `/properties/<kind>` and `/dependentSchemas/<kind>` duplicates when they describe the same underlying failure;
  - [x] truncate/bound instance echoes so large nested objects are never dumped inline;
  - [x] print the failing pointer/path even when value echo is suppressed;
  - [x] print expected discriminator values for closed unions where practical;
  - [x] resolve the **root kind union** to the branch named by the declared `kind` so a kind-specific violation reports the offending field and path instead of `/oneOf` with the whole manifest echoed.
- [x] For `packageRef` failures, prefer an actionable message describing accepted forms:
  - [x] `"@ns/name@1.2.3"` string;
  - [x] `{name, version?}` object.
- [x] For closed Memory operation kinds, prefer expected values such as:
  - [x] `consolidate`;
  - [x] `transform`;
  - [x] `delete`.
- [x] Add test coverage across known oneOf/anyOf cliffs:
  - [x] root kind dispatch (for example `kind: "agent"` missing a kind-required field) asserting a branch-specific path/message rather than `/oneOf`;
  - [x] packageRef;
  - [x] memoryTrigger;
  - [x] memoryOperation;
  - [x] loopTransitionTarget;
  - [x] loopToolFailurePolicy;
  - [x] knowledgeMetadata;
  - [x] agentMemoryBinding;
  - [x] entrypoint.command.
- [x] Add a regression fixture reproducing the existing Loop case where one invalid transition currently emits giant schema messages before good semantic errors.
- [x] Add a regression fixture reproducing the Memory invalid-operation-kind case where today no useful semantic error exists.
- [x] Keep JSON/NDJSON lint output contracts stable unless explicitly versioned.
- [x] Make publish validation use the same human lint renderer, including a contextual manifest header instead of beginning with unattributed `[ERROR]`.
- [x] Add ordering tests ensuring actionable semantic messages appear before generic schema fallback.
- [x] Confirm the diagnostic shape produced here can carry an external rule identifier alongside the path, so Stage 2's APDS rule IDs attach without re-cutting the renderer. Do not implement APDS rules in this milestone — only avoid foreclosing them.
- [x] Add a reproducible operator runbook and disposable fixtures: `specs/2026-10-06-immediate-hardening/manual-tests/CLI_M8_MANUAL_TESTS.md` and `scripts/cli_m8_manual.py`.
- [ ] Complete the operator walkthrough and record readability/unexpected-output notes in the runbook. Builder smoke checks do not replace this manual acceptance.

Implementation and verification completed 2026-10-15; see `specs/2026-10-06-immediate-hardening/test-plan.md`, "Milestone 8 evidence". The schema tolerance ships in **0.1.36**, published/installed by operator confirmation on **2026-10-22**, superseding the original **0.1.35** preparation target. Production first-use capture confirms the installed binary reports **0.1.36**; this is operator release evidence, not an independent GitHub/Homebrew publication audit.

## Milestone 9: CLI Error and Success Consistency
> Scope note: make non-Harness CLI success/failure behavior feel like one coherent product by centralizing transport errors, correcting exit codes, normalizing sibling inspect output, and cleaning up low-level path/filesystem leaks. This milestone does not alter Harness output, introduce new package/runtime capabilities, change machine-readable contracts beyond explicitly specified fixes, or absorb the Python portability work from later milestones.
> Implementation notes:
> - `whoami` already demonstrates a friendlier connection message, while `install`, `export`, and `new` currently expose raw reqwest URLs. Reuse one shared formatter rather than cloning command-specific wrappers.
> - **Rejected credentials now look like a missing package on the install path.** Milestone 4 changed `auth_optional` to fall through anonymously instead of returning 401, and `/install/resolve` + `/install/init` use that decorator while the SDK attaches the PAT (`crates/agentpm-sdk/src/client.rs`, `bearer_auth`). A revoked or expired PAT installing a private package therefore resolves as anonymous, the package is invisible, and the user is told it does not exist. The status code alone can no longer carry that distinction, so the fix belongs in the request/response contract rather than in message wording.
> - Current exit-code convention is otherwise coherent:
>   - runtime error = 1;
>   - clap/usage error = 2.
>   `whoami` returning 0 after a connection failure is the confirmed exception.
> - `knowledge inspect` and `memory inspect` should share one structural summary pattern while retaining kind-specific detail.
> - Human output should normally avoid leaking long absolute paths unless the path is itself the important result; richer detail can stay available in verbose/machine modes.
> - Normalize user-supplied `.` paths before display so `/./` never appears.
> - Error/success wording should answer: what happened, where, why, and what next.

- [x] Centralize network/transport error formatting in a reusable CLI helper.
- [x] Apply friendly connection errors to:
  - [x] install;
  - [x] export;
  - [x] new;
  - [x] whoami;
  - [x] other matching registry-backed commands discovered during implementation.
- [x] Preserve low-level transport detail behind verbose/debug output if existing CLI patterns support it.
- [x] Avoid surfacing irrelevant internal endpoint names as the primary error for higher-level commands such as `export --skill`.
- [x] Fix `whoami` to exit non-zero on network failure.
- [x] Distinguish "credentials rejected" from "not found" on `install`. Since `auth_optional` no longer returns 401, decide how the server signals that a supplied token was ignored — an explicit response field, a dedicated header, or moving the install endpoints to `auth_required` — and have the CLI say so plainly instead of reporting a missing package. Verify against a revoked PAT plus a private package, which is the real-world case (token rotation or revocation).
- [x] Add exit-code regression tests:
  - [x] runtime error → 1;
  - [x] clap/usage error → 2;
  - [x] successful command → 0.
- [x] Normalize Knowledge/Memory inspect header structure, for example:
  - [x] `<Kind>: name@version`;
  - [x] `Target: ...`;
  - [x] `Status: fresh|stale|...`;
  - [x] normalized manifest path/name;
  - [x] then kind-specific details.
- [x] Normalize freshness/status label across sibling inspect commands.
- [x] Normalize paths before printing.
- [x] Reduce unnecessary absolute path verbosity in normal human output.
- [x] Preserve richer paths in verbose/machine output where useful.
- [x] Improve Memory missing-sidecar wording from `is invalid: reading <path>` to an explicit “references X, which could not be read” style message.
- [x] Make `new` success output include:
  - [x] created target/path;
  - [x] useful next step;
  - [x] retain the existing README review guidance where useful.
- [x] Move `serve --mcp` requirement into clap argument validation:
  - [x] use clap-required/constraint behavior rather than runtime `currently requires --mcp`;
  - [x] preserve exit code 2 for missing required usage.
- [x] Improve `keys export <bad-id>` to:
  - [x] say no local key with that ID exists;
  - [x] suggest `agentpm keys list`;
  - [x] avoid duplicate filesystem exception lines;
  - [x] avoid unnecessary home-directory disclosure in normal output.
- [x] Preserve `1` runtime / `2` clap usage exit-code convention.
- [x] Add regression tests for all corrected messages and exit codes.
- [x] Complete the operator walkthrough in
  [CLI messaging and exit codes manual verification](manual-tests/CLI_M9_MANUAL_TESTS.md)
  before closing Release Band 5; record any blocked real-registry cases.

Implementation and automated verification completed 2026-10-16; see "Milestone 9 evidence" in `specs/2026-10-06-immediate-hardening/test-plan.md`. Install resolve/init use `auth_optional(reject_invalid=True)`: missing credentials remain anonymous, while supplied invalid/revoked credentials return 401 before package lookup. Default optional authentication on public reads is unchanged. CLI transport formatting is applied at the non-Harness command boundary, with underlying reqwest errors retained by the Rust client. No schema, Harness, Node/Python SDK, or version-marker changes were made. Existing API pre-commit environment/baseline failures are recorded separately in the test plan, not counted as passing checks.

## Release Band 5: CLI Authoring and Messaging Quality
Covered milestones: 8-9.
This gives us trustworthy scaffolding, lint output that explains what is actually wrong (including the `oneOf`/`anyOf` cases that currently produce unusable messages), and consistent error/success wording and exit codes across non-Harness commands. Ship it before the dependency/release work because that work adds new failure modes to the same messaging surface. Deploy the registry's opt-in strict install-token handling before or with the CLI: the new client recognizes its existing 401 contract, but cannot distinguish rejected credentials from anonymous not-found responses sent by an older registry.

**🚦 Published CLI 0.1.36 is the APDS minimum floor (operator-confirmed 2026-10-22).** Milestone 8 added the permissive root `standard` field so clients carrying that change tolerate manifests that declare it. Operator publication/installation confirmation and a live Production first-use payload now satisfy the cross-stage publication prerequisite. Stage 2 M3A's other dependencies and acceptance criteria remain unchanged:

- [x] Original preparation: CLI **0.1.35** (`v0.1.35`) and matching website status date **2026-10-16**. This historical preparation target is superseded by **0.1.36** / **2026-10-22** under Release Band 7; it is not the APDS minimum floor or evidence that the tolerance change shipped.
- [x] Record CLI **0.1.36** as the published tolerance-release version carrying M8. Operator confirmed publication/installation on **2026-10-22**; the installed Homebrew-path binary reports **0.1.36** and a live Production first-use event carries that version. M8's publication task is now complete.
- [x] Hand **0.1.36** to the Stage 2 track as the published tolerance-release floor: recorded in Stage 2 `spec.md` and M1A.1/M3A/M4B in `tasks.md`. **Stage 2 M4B** must name this version in its outdated-client error. **M3A's publication prerequisite is satisfied; no other Stage 2 prerequisite is waived.**

Release handoff updated **2026-10-22** to use **0.1.36**, superseding the
**2026-10-16** target of **0.1.35**. Operator confirmed backend/web deployment
before CLI publication, then published/installed **0.1.36** and verified
Production first-use capture. M8 and Stage 2 M1A.1's publication records are
complete on that evidence. Remaining Band 7 privacy/Harness/dashboard checks
do not imply that all Stage 2 work is accepted or implemented.

Publishing a package that declares `standard` before this CLI is released makes `agentpm lint` hard-fail (exit 1) and `agentpm new` fail inside `validate_generated_manifests_blocking` for every user on an older build — and published packages are immutable, so it cannot be walked back.

## Milestone 10A: Product Analytics, Billing Funnel, and Feedback
> Scope note: stop flying blind before early traffic by adding a deliberately small web/registry analytics vocabulary, authoritative server-side events where possible, billing conversion events, one funnel dashboard, and a lightweight feedback path. Local CLI telemetry and its privacy contract are split into Milestone 10B. This milestone does not enable session replay, experimentation, NPS, sophisticated attribution, broad behavioral profiling, or replace Lemon Squeezy as billing source of truth.
> Implementation notes:
> - Prefer server-side authoritative events whenever AgentPM already knows the real outcome. Example: `package_install_completed` should come from successful install-session completion, not merely an Install-button click.
> - Browser/CLI events should represent intent or local-only behavior only where the server cannot know the truth.
> - The initial product funnel is intentionally small:
>   landing → docs/explore/package → account → CLI/install → Harness → publish,
>   with a separate pricing → checkout → subscription branch.
> - `checkout_started` is intent; Lemon Squeezy webhook events are billing truth.
> - The event naming/property convention decided here is also binding on Milestone 10B's CLI telemetry.
> - PostHog may provide the survey backend/reporting, but the visible feedback UI should look and feel like AgentPM.
> - Do not enable session replay in Stage 1.

- [x] Create PostHog project/config integration for web/backend/CLI surfaces as needed. Development project/token configured by operator; CLI emission remains Milestone 10B. Live delivery and production setup remain below.
- [x] Decide one shared event naming/property convention and document it before instrumenting broadly. See `analytics-contract.md`.
- [x] Disable/avoid broad autocapture if it would violate the explicit event/property contract.
- [x] Implement initial web/registry events:
  - [x] `landing_viewed`;
  - [x] `docs_viewed`;
  - [x] `explore_searched`; explicit nonempty submissions from Explore and the desktop/mobile global header; no capture on results-page arrival/refresh. The post-release mobile omission is fixed and locally verified; its deployment/recheck is tracked in the manual guide.
  - [x] `package_viewed`;
  - [x] `namespace_viewed`;
  - [x] `signup_started`;
  - [x] `pricing_viewed`;
  - [x] `checkout_started`;
  - [x] `feedback_submitted`.
- [x] Implement server-authoritative events for:
  - [x] `account_created` where the backend has an authoritative completion point;
  - [x] `package_install_completed`;
  - [x] `publish_completed`;
  - [x] `package_starred` / unstar equivalent if needed for analysis;
  - [x] subscription lifecycle.
- [x] Add `pricing_viewed` at actual pricing-page exposure and `checkout_started` at Lemon Squeezy checkout handoff.
- [x] Map Lemon Squeezy webhook events to:
  - [x] subscription started;
  - [x] subscription updated;
  - [x] subscription cancelled;
  - [x] payment failed where supported.
- [x] Make webhook-derived analytics idempotent so retries do not double-count subscription lifecycle events.
- [x] Keep Lemon Squeezy as billing source of truth; PostHog receives product events, not authoritative billing state.
- [x] Create one Early Product Funnel PostHog dashboard rather than many dashboards. All Development panels verified; operator also copied the dashboard and insights to Production with production environment filters, and confirmed live charts update, including real star/unstar actions in Engagement.
- [x] Dashboard answers the M10A questions below; actual local CLI/Harness measurement remains explicitly deferred to Milestone 10B:
  - [x] are people arriving?; Arrivals panel verified with `landing_viewed` unique users / total count, last 7 days, and `environment = development`.
  - [x] do they reach docs/explore/package pages?; Discovery panel verified with all four discovery events as total counts, last 7 days, and `environment = development`.
  - [x] do signup starts connect to account creation?; Saved Signup funnel verified with one person completing both steps (100%), Sequential order, and a 7-day conversion window.
  - [x] do they install/use the CLI?; Completed by Milestone 10B: operator confirmed live Development and Production CLI adoption charts on 2026-10-22.
  - [x] do installs complete?; Real local-registry install captured and Registry use panel saved with completion-event total counts.
  - [x] do Harness runs start/complete?; Completed by Milestone 10B: operator confirmed live Development and Production per-run Harness activity charts on 2026-10-22.
  - [x] do users publish?; Real local-only publish completed and Registry use panel saved with `publish_completed` total count.
  - [x] do users start checkout/subscribe?; Operator confirmed test checkout and conversion events, then saved Billing intent and conversion with the three expected steps, last 7 days, and `environment = development`. Screenshot shows one person reaching subscription creation at 100% conversion.
  - [x] are users giving feedback/stars?; Engagement panel verified with `package_starred`, `package_unstarred`, and `feedback_submitted` total counts, last 7 days, and `environment = development`.
- [x] Add the M10A CLI/Harness availability text card: local telemetry pending Milestone 10B; missing data is not zero usage; registry completions are not CLI/Harness execution counts. Operator confirmed it was added.
- [x] Billing lifecycle panel saved with `subscription_updated`, `subscription_cancelled`, and `subscription_payment_failed`; operator screenshot shows total counts of 4, 1, and 1 respectively on October 17 UTC.
- [x] Add AgentPM-branded Feedback entry in shared site/docs/registry chrome.
- [x] Back feedback storage/reporting with PostHog survey capability. Running API survey with real question IDs; operator confirmed stored answers and separate allowlisted `feedback_submitted` events for both blank email / follow-up No and supplied email / follow-up Yes.
- [x] Initial feedback form should capture:
  - [x] what user was trying to do;
  - [x] what got in the way or surprised them;
  - [x] optional email;
  - [x] follow-up consent.
- [x] Do not make satisfaction score/NPS the primary Stage 1 feedback mechanism.
- [x] Add GitHub Issue branch/link for reproducible bugs.
- [x] Do not enable session replay in Stage 1.
- [x] Complete live operator acceptance in `manual-tests/ANALYTICS_M10A_SETUP.md`: inspect real payloads and signup linkage; verify survey storage and test-mode billing replay; review production project/survey configuration and privacy/access/retention settings. Operator also confirmed production capture, feedback storage, IP suppression, updating dashboards, and star/unstar events. Final ordinary-event payload spot-check found no unexpected properties.

Implementation/offline verification and live operator evidence are recorded in
`test-plan.md`. Milestone 10A implementation and scoped live operator acceptance
are complete. The unresolved customer-portal redirect and invoice-event
`plan: unknown` attribution limitation remain documented residual risks, not
fixed or verified by the analytics checks. CLI/Harness measurement was deferred
to Milestone 10B and is now verified in both projects; operator also confirmed
the availability card's post-release update on 2026-10-22.

## Release Band 6: Product Analytics, Billing Funnel, and Feedback
Covered milestones: 10A.
This gives us the small deliberate analytics vocabulary, authoritative server-side events, billing conversion events, one Early Product Funnel dashboard, and a lightweight feedback path. The user-visible instrumentation in this band ships on the web/API surfaces; shared PostHog configuration and event conventions may prepare the CLI integration, but no CLI telemetry is emitted until Band 7. Sequencing it immediately after the registry is technically ready for inbound traffic is the point: instrumentation has to be live before that traffic is intentionally increased.

## Milestone 10B: CLI Telemetry and Privacy Contract
> Scope note: add minimal, anonymous, default-on-with-disclosure CLI telemetry and the privacy contract that governs it, including the opt-out mechanisms and the Privacy Policy update. Split from Milestone 10A because this is the only analytics work that ships inside the Rust binary, is the only part with a legal/disclosure surface, and must not go out half-finished — a partially implemented allowlist or a missing opt-out is a privacy problem, whereas incomplete web analytics is merely incomplete data.
> Implementation notes:
> - Depends on Milestone 10A for the PostHog project/config integration and the shared event naming/property convention; do not invent a second convention here.
> - Telemetry is minimal, anonymous, default-on with clear disclosure, and easy to disable.
> - Use a strict event/property allowlist. Do not pass broad application objects to PostHog and rely on redaction later.
> - Never collect prompts, Tool I/O, Harness conversation content, file contents, paths, env vars, secrets, package contents, or private package identities.
> - The Privacy Policy update is part of this milestone, not a follow-up: shipping collection before disclosure is not acceptable.
> - **`harness_started`/`harness_completed` are per Harness *run*, not per session.** The engine allocates both (`allocate_harness_session_id` and `allocate_harness_run_id`, with `start_run()` separate from session creation), and a TUI session hosts many runs while a headless invocation is typically one. Per-session counting would make TUI and headless incomparable — twenty TUI prompts would register once, twenty headless invocations twenty times. `cli_first_used` already answers adoption; this pair answers activity, so it is emitted once per run with the surface carried as the runtime-mode enum.
> - **A missing `harness_completed` is an upper bound on failures, not a failure rate.** SIGKILL, a panic, machine sleep, a failed send, or opting out mid-session all produce a started event with no completion. Say so on the dashboard card rather than presenting the delta as a clean failure count — reuse the M10A framing that missing data is not evidence of zero usage.
> - **Telemetry must never delay or alter process exit.** `harness_completed` fires on the exit path, so delivery is fire-and-forget with a hard timeout: it must not block exit, must not change the exit code, and a failed send must never surface as a Harness failure. This matters most for headless/CI runs.

- [x] Add minimal CLI telemetry:
  - [x] `cli_first_used`;
  - [x] `harness_started`;
  - [x] `harness_completed`.
- [x] Extend the existing Early Product Funnel dashboard in Development and Production with the two questions deferred from M10A; do not create a separate dashboard:
  - [x] Add a CLI adoption chart using `cli_first_used` and unique anonymous installations, consistent with the first-use semantics chosen below. Do not label it as a count of all CLI commands or package installs.
  - [x] Add a Harness activity chart showing `harness_started` and `harness_completed` event counts. Label it as **per-run** activity and break it down by surface, so TUI sessions and headless invocations are not silently summed as if equivalent. Note on the chart that a missing completion is an upper bound on failures, not a failure rate.
  - [x] Save the correct environment filters in each project's new insights and verify their counts with real CLI/Harness test activity in Development, then live capture after the Release Band 7 production rollout. Do not inject fake production events to populate charts.
  - [x] Update the M10A availability card after local telemetry is released and verified; keep the distinction between registry completions and local execution, and explain that missing/opted-out telemetry is not evidence of zero usage.

Development progress (operator-confirmed 2026-10-22): both new insights are
saved in Early Product Funnel with `environment = development` and `source = cli`.
CLI adoption shows one unique installation; Harness totals show one headless
start/completion pair and two TUI pairs, broken down by `runtime_mode`, with the
missing-completion caveat. Live events share a stable `cli_` distinct ID.
Operator also confirmed both copied insights appear on the existing Production
dashboard. Production screenshots verify saved `environment = production` and
`source = cli` filters on both insights, correct aggregations, and the Harness
surface breakdown/caveat. Operator confirmed Production dashboard public sharing
remains disabled. After publishing CLI 0.1.36, operator confirmed real Production
first-use capture and a TUI start/completion pair from an actual Agent in
`agentpm-examples`, with terminal outcome `handed_off`. Operator also confirmed
both Production charts reflect adoption and the runs. Operator confirmed the
availability-card update on 2026-10-22; the dashboard checklist is complete.

- [x] Decide whether `cli_first_used` is once per anonymous installation or once per CLI version; prefer once per installation unless product analysis requires version-level adoption. **Chosen: once per installation, best-effort attempt.**
- [x] Record the `harness_started`/`harness_completed` contract before instrumenting, since redefining an event's meaning later invalidates the chart's history:
  - [x] confirm the agreed **per-run** granularity (not per session) and that the surface — `tui`, `headless`, `machine` — is the runtime-mode enum;
  - [x] decide whether `harness_completed` fires on failure as well as success. **Chosen: success and handled failure, with the closed terminal-outcome enum recorded in `analytics-contract.md` before instrumentation.** The Loop's terminal targets (`$end`, `$abort`, `$handoff`) and max-steps exhaustion are structural facts, not conversation content;
  - [x] if `harness_completed` instead fires only on success, state plainly that `started − completed` conflates failures, cancellations and crashes. **Not applicable: not success-only; missing completion is still not a failure rate.**
- [x] Generate/store anonymous install ID:
  - [x] non-secret random identifier;
  - [x] stable across CLI runs;
  - [x] not derived from machine serial, username, path, or other identifying system data.
- [x] Add strict telemetry property allowlist.
- [x] Allowed CLI properties should be limited to fields such as:
  - [x] event;
  - [x] CLI version;
  - [x] OS family;
  - [x] CPU architecture;
  - [x] anonymous install ID;
  - [x] runtime-mode enum: the Harness surface (`tui`, `headless`, `machine`);
  - [x] terminal-outcome enum on `harness_completed`, **only if** the decision above adopts it.
- [x] Add `AGENTPM_TELEMETRY=0` support.
- [x] Add persistent config opt-out if existing config architecture supports it cleanly.
- [x] Ensure telemetry never sends:
  - [x] prompts;
  - [x] Tool I/O;
  - [x] Harness conversation content;
  - [x] Tool args/results;
  - [x] paths;
  - [x] env vars;
  - [x] secrets;
  - [x] package contents;
  - [x] private package identities.
- [x] Update Privacy Policy to disclose:
  - [x] web/product analytics;
  - [x] minimal CLI telemetry;
  - [x] categories of data collected;
  - [x] explicit excluded data categories;
  - [x] anonymous install identifier;
  - [x] opt-out mechanism;
  - [x] PostHog as relevant service/provider;
  - [x] feedback form data including optional email.

Implementation and local verification: see `test-plan.md` and
`manual-tests/CLI_M10B_MANUAL_TESTS.md`. Development capture and both dashboards'
configuration and real Production capture/counts are verified as recorded above.
Operator confirmed the availability-card update and public Privacy Policy
spot-check, including minimal CLI telemetry and both opt-out methods, on
2026-10-22. Backend/web deployment preceded CLI publication. Milestone 10B and
Release Band 7 acceptance are complete on the recorded operator evidence.

## Release Band 7: CLI Telemetry and Privacy Contract
Covered milestones: 10B.
This gives us minimal, anonymous, default-on-with-disclosure CLI telemetry, a strict event/property allowlist, working opt-out mechanisms, the matching Privacy Policy update, and CLI adoption / Harness activity charts in the existing Early Product Funnel dashboard. It is its own band for two reasons: it ships inside the Rust binary rather than on a web deploy, and it has a legal gate. Collection must not go out ahead of disclosure, and a partially implemented allowlist or opt-out is a privacy defect rather than an incomplete feature, so this band is strictly all-or-nothing.

- [x] Prepare CLI **0.1.36** (`v0.1.36`) and matching website status-page date **2026-10-22**, keeping SDK versions unchanged. Crate/lockfile agree; binary reports `agentpm 0.1.36`. Verification is recorded in `test-plan.md`.
- [x] Deploy the Registry/API, docs and Privacy Policy; confirm the disclosure is public before publishing the telemetry-enabled CLI. Operator confirmed backend/web deployment before CLI publication and completed the public disclosure spot-check on 2026-10-22.
- [x] Publish CLI **0.1.36** and deploy the website with the matching status marker. Operator confirmed backend/web deployment before publishing/installing the CLI on **2026-10-22**; installed version and live Production first-use payload both report **0.1.36**.
- [x] Verify real Production CLI/Harness capture and chart counts, then update the availability card and close the remaining M10B dashboard acceptance. Operator confirmed all remaining checks on 2026-10-22.

## Milestone 11A: Python Dependency Declaration and Resolution
> Scope note: establish what a new-format Python Tool may declare and how AgentPM resolves it to exact portable state. Owns the canonical AgentPM target identifier and its single source of truth, the `agent.json` dependency surface and its lint rules, the fixed `PythonResolution` v1 contract including distribution digests, and the full AgentPM-managed `uv` story — acquisition, pinning, verification, caching, override, and the checked-in pin-table generator. Produces resolution state but does not yet persist it in `agent.lock` (11B) or install it into a runtime environment (11C).
>
> This milestone **owns resolver acquisition for the whole stage**. Milestone 11C and Milestone 15 both provision Python environments through the same managed binary; neither re-implements acquisition, discovery, or verification. Build the acquisition surface here as a reusable component so the install path calls it rather than duplicating it.
>
> This milestone **owns the canonical AgentPM target identifier and current-target detection**, because Milestone 11C keys the managed Python environment by consumer target. Milestone 12 consumes the same vocabulary for payload classification; Milestones 13, 15, and 16 consume it for artifact storage, selection, and the CI matrix.
>
> Implementation notes:
> - `runtime.version` is currently enforced as a minimum, not an exact interpreter pin; retain that contract.
> - The new portable resolution must not capture only the publisher's selected wheel/architecture. Exact versions may need environment markers/conditional branches.
> - Target IDs should follow the Rust target-triple vocabulary already used by `agentpm/.github/workflows/release.yml`, and must distinguish relevant ABI/libc rather than only OS + CPU. Do not reuse `client.os`/`client.arch` from the publish descriptor — those describe the publishing machine and are telemetry only.
> - **The `uv` acquisition strategy is decided, not open.** See spec §19 "AgentPM-managed `uv` acquisition". Pinned standalone binary, AgentPM-controlled immutable storage, compiled-in SHA-256, `AGENTPM_UV_PATH` override at exact pinned version, no PATH `uv`, no bundling. Do not implement a second architecture or a PATH-discovery fallback.
> - **`PythonResolution` v1 records distribution digests.** This is fixed in v1 because the contract is frozen here and Stage 2's APDS v1.0.0 freeze is gated on it; digests cannot be added additively later.

- [x] Define canonical AgentPM target identifier representation, including:
  - [x] `any`;
  - [x] `aarch64-apple-darwin`;
  - [x] `x86_64-apple-darwin`;
  - [x] `x86_64-unknown-linux-gnu`;
  - [x] `x86_64-pc-windows-msvc`;
  - [x] room for future supported targets/ABI variants.
- [x] Give the supported-target list **one checked-in machine-readable source of truth**. Today it exists only as the four-entry YAML matrix in `agentpm/.github/workflows/release.yml`, which is not readable by the CLI, by tests, or by the resolver pin-table generator:
  - [x] define the canonical list in one checked-in location;
  - [x] have the canonical target enum, the resolver pin table, and `release.yml`'s matrix derive from it or validate against it;
  - [x] add a check that **fails** when the release matrix and the resolver pin table disagree, so adding a CLI target cannot silently ship a platform whose Python provisioning is impossible.
- [x] Add deterministic current-target detection/mapping rather than treating raw `std::env::consts::{OS,ARCH}` as the compatibility ID.
- [x] Extend Tool runtime schema with optional Python `dependencies` when `runtime.type == "python"`. **Cross-stage note:** `$defs/runtime` is `additionalProperties: false, required: [type, version]`, so this is a conformance-affecting change and Stage 2's APDS v1.0.0 freeze (Stage 2 M1B) is gated on it. Flag the merge to the Stage 2 track.
- [x] Extend all relevant typed runtime representations, including runner-side `RuntimeDecl`, so the new field is understood consistently.
- [x] Reject `dependencies` for Node Tools.
- [x] Add Python requirement syntax validation.
- [x] Add deterministic duplicate/conflict validation.
- [x] Implement the fixed `PythonResolution` v1 contract:
  - [x] `type` exactly `agentpm.python-resolution.v1`;
  - [x] `python.requires` as the normalized runtime Python constraint;
  - [x] `requirements: string[]` containing normalized authored root requirements;
  - [x] `packages[]` containing normalized distribution `name`, exact `version`, required non-empty `hashes[]`, and optional normalized PEP 508 `marker`;
  - [x] allow repeated normalized package names only when distinguished by mutually exclusive marker conditions needed for portable resolution;
  - [x] normalize distribution names deterministically (PEP 503-style lowercase/hyphen normalization preferred);
  - [x] sort `requirements` deterministically;
  - [x] sort `packages` by normalized `(name, marker-or-empty, version)`;
  - [x] sort `hashes` lexicographically within each package entry;
  - [x] do not include wheel filenames/tags, publisher target/architecture, interpreter path, local paths, or environment IDs;
  - [x] do not require a full parent/child dependency graph in v1;
  - [x] digest canonical `PythonResolution` bytes with RFC 8785/JCS when producing `pythonResolutionDigest`.
- [x] Record distribution digests for every locked version:
  - [x] each digest is `sha256:` plus 64 lowercase hex characters, never truncated;
  - [x] `hashes[]` covers **every published distribution of that exact version any supported AgentPM target could install** — applicable wheels plus the sdist where one exists — not the publisher's selected wheel;
  - [x] do **not** key digests by target, filename, or wheel tag; any one listed digest matching is acceptance, which is what keeps the object target-independent;
  - [x] enumerate distributions across the full supported-target set, not only the publishing machine's — under-enumeration produces a lock that installs locally and fails on other platforms;
  - [x] **fail resolution** if any resolved version has no retrievable digest rather than emitting an empty/omitted `hashes` array;
  - [x] add a lint/validation rule rejecting a `PythonResolution` with any package entry lacking digests, so an unverifiable resolution cannot reach `agent.lock` or a release.
- [x] Define the object once as the single shared type, so every later consumer reuses it rather than restating its shape. **Propagating it into `LockedPackage`, resolve/install DTOs, the publish descriptor, artifact metadata, and the release-manifest digest input moved to Milestone 11B**, matching this milestone's scope note that 11A produces resolution state without persisting it. Band 8 acceptance for those surfaces is judged in 11B, not here.
- [x] Do not lock publisher-selected wheel filenames or publisher architecture in `PythonResolution`.
- [x] Integrate AgentPM-managed `uv` as the pinned internal Python dependency engine:
  - [x] pin one exact `uv` version as a compiled-in constant;
  - [x] version-check the binary before every execution, not only at download;
  - [x] never discover or fall back to a PATH-installed `uv`;
  - [x] acquire lazily — nothing downloads until the first operation that actually needs the dependency engine.
- [x] Implement acquisition in this exact resolution order:
  - [x] `AGENTPM_UV_PATH` set → validate the binary reports the pinned version, then use it;
  - [x] managed binary present in cache → validate digest/version, then use it;
  - [x] target not in the pin table → fail with the unsupported-target error **before any network access**;
  - [x] otherwise download from AgentPM storage → verify SHA-256 against the compiled-in pin → cache → use.
- [x] Cache the binary per user, per `uv` version, per target, outside any project directory (for example `~/.cache/agentpm/runtimes/uv/<version>/<target>/uv`) so one download serves every project and a version bump cannot overwrite a binary an older CLI still pins.
- [x] Serve mirrored binaries from AgentPM's **existing** registry object storage / presigned-URL distribution path:
  - [x] Reserve the `runtimes` namespace/storage-key segment server-side before namespace creation; reject case/whitespace and path-style bypasses without changing existing `agentpm` rejection behavior.
  - [x] do not teach the CLI to parse GitHub Releases;
  - [x] do not stand up a new CDN hostname or separate download service for this;
  - [x] honor standard proxy environment variables on this download. **Note the deliberate contrast with telemetry**, which uses a no-proxy client: telemetry must not depend on corporate network config, whereas resolver acquisition must work *through* it;
  - [x] set explicit connect/idle timeouts rather than inheriting `reqwest` defaults.
  - [x] Gate CLI release on the deployed Registry's version/target download routes and SHA-256 verification of every mirrored binary, rather than relying on a manual cross-repo inventory comparison.
- [x] Record the **append-only retention rule** for mirrored resolver artifacts where storage lifecycle policy actually lives, not only in the spec. Published CLI versions pin older `uv` versions permanently, so deleting a mirrored binary retroactively breaks every CLI release that pinned it. Operator confirmed the scoped delete/conditional-write policies in both real buckets; lifecycle review and deployment mapping are recorded beside the Registry S3 integration.
- [x] Add a checked-in pin-table generator (script or `xtask`) — the digest table must not be hand-maintained:
  - [x] accept the desired `uv` version as input;
  - [x] download the upstream artifacts for every supported AgentPM target;
  - [x] verify the upstream GitHub Artifact Attestation against repository `astral-sh/uv`;
  - [x] Gate adoption on real native execution for every supported target, checking the production lock/export commands and portable pylock output before any upload or pin-table write; keep ordinary tests offline and reject missing/stale cross-platform evidence.
  - [x] Execute the hosted native workflow and validate all four downloaded reports against the current pin and smoke contract (run `40043503095`, 2026-10-23).
  - [x] compute SHA-256 independently rather than trusting an upstream checksum file;
  - [x] upload the verified immutable bytes to AgentPM storage (operator completed dev and production uv 0.13.5 ingestion);
  - [x] write the generated, source-controlled pin table (version + per-target `sha256`);
  - [x] **fail** if any supported AgentPM target is missing, so a bump cannot ship a partial table;
  - [x] mark the output as generated so it is never edited by hand — its integrity rests on code review and git history.
- [x] Implement `AGENTPM_UV_PATH` in this milestone, not as a later advanced option — without it an airgapped install has no path forward:
  - [x] enforce **exact** equality with the pinned `uv` version;
  - [x] on mismatch, error (never warn) and name the exact required version so an operator can provision it;
  - [x] document it as serving airgapped environments, enterprise images, and preprovisioned CI images **that provision AgentPM's pinned version** — explicitly not as a way to adopt whatever `uv` an image already contains.
- [x] Make resolver failures distinguishable in error text — these are five different operator problems with five different fixes:
  - [x] unsupported target (names supported targets and the override, no network attempted);
  - [x] network/proxy failure reaching AgentPM storage;
  - [x] downloaded-bytes digest mismatch;
  - [x] `AGENTPM_UV_PATH` version mismatch;
  - [x] resolver executed but failed.
- [x] Do not make `requirements.txt`, `pyproject.toml`, Poetry, or uv project lockfiles authoritative in Stage 1.
- [x] Resolve declared dependencies to exact portable state before publishing, and block publication that cannot yet retain it. **Writing that state into `agent.lock` and gating publish on lock freshness moved to Milestone 11B**, which owns lock persistence; 11A resolves, records the author-side input fingerprint, and refuses to publish a dependency-bearing Tool at all until 11B can persist the result.
- [x] Implement re-resolution/staleness as a recorded input fingerprint rather than a fuzzy comparison:
  - [x] fingerprint covers normalized authored root requirements, normalized `python.requires`, the supported target set, and **the `uv` version that produced the resolution**;
  - [x] a valid matching resolution is reused — `agentpm publish` alone does not trigger re-resolution;
  - [x] a missing/stale fingerprint, or an explicit refresh, triggers re-resolution;
  - [x] a resolver pin bump therefore invalidates stored resolutions. Omitting the `uv` version from the fingerprint lets a lock mix entries produced by different resolver versions with no way to tell, which defeats the only guarantee the pin provides.

M11A implementation status (2026-10-22): declaration/lint, the portable shared
Rust type, target vocabulary, resolver acquisition, structured universal
resolution, canonical digesting and input-fingerprint reuse/refresh are
implemented. `uv` is pinned to **0.13.5**, generated from upstream artifacts whose
GitHub attestations were verified for all four supported targets. Release CI and
the target enum derive from `crates/agentpm-sdk/platforms/targets.json`; pin-table
coverage is checked in normal CI and before release. See `test-plan.md` for
verification evidence.

Review hardening: resolver adoption now requires native execution evidence for
every supported binary, bound to its exact SHA-256/version/target and the current
smoke contract. `Resolver Contract` runs on relevant pin/contract PR changes or
manual dispatch; it does not upload to S3 or update either repo. The production
CLI and real smoke use the same checked-in command arguments, while offline
fakes independently enforce those arguments. The macOS arm64 real ingestion
smoke passed locally. Subsequently, the user reported successful CI and Resolver
Contract runs on the PR; all four downloaded native reports from run
`40043503095` were locally validated against uv 0.13.5, every executable pin,
the current smoke contract and portable output checks on 2026-10-23. The hosted
native-verification gate is satisfied; the later storage progress is recorded below.
See `test-plan.md` for the run link and evidence.

Follow-up review hardening: the CLI release workflow now checks the deployed
Registry inventory and actual mirrored bytes before builds/assets, without
private backend-repo access. Missing/inaccessible mirrors (HTTP 403/404), other
HTTP service failures and transport/proxy failures have distinct diagnostics.
Plain dependency-bearing publication fails before resolver acquisition, even on
a cold cache or with `--refresh-python`. The graph-free v1 contract's coverage
check is explicitly root-only; parser tests preserve conditional transitives and
the native adoption fixture rejects a missing Windows-only transitive, without
claiming that general transitive closure can be proved without dependency edges.

Scope boundary: the unchecked lock/DTO/build/inspection/release-manifest wiring
above belongs to 11B and later owning milestones, per this milestone's explicit
scope note. No v4 lock writes or managed Python environment installation were
added. `publish --dry-run` prepares/reuses a typed author-side resolution under
`target/agentpm/python-resolutions/`; `--refresh-python` refreshes it. Live
dependency-bearing publication is deliberately blocked before upload until
portable lock/release integration exists, rather than publishing declarations
whose exact resolution would be discarded. Legacy publication remains unchanged.

Operator progress (2026-10-23): uv 0.13.5 ingestion completed in both
`agentpm-dev-tools` and `agentpm-tools` under `tools/runtimes/uv/`. The operator
saved scoped delete protection and create-only write requirements in both bucket
policies, and supplied matching lifecycle rules with no current-object expiration
or archival transitions under `tools/`. The local Registry mirror check verified
all four binaries. An initial production probe returned 404 before deployment;
the operator subsequently shared a successful probe against
`https://api.agentpackagemanager.com`, verifying uv 0.13.5 downloads and pinned
digests for all four targets. Production mirror delivery is accepted. This is
not a CLI release or an independent deployment/build audit. Namespace auditing
was confirmed by the user.
The deployment/retention mapping is recorded in
`agentpm-api/app/tools/domain/tools/RESOLVER_STORAGE.md`.

The operator confirmed the individual-command manual walkthrough in
`manual-tests/CLI_M11A_MANUAL_TESTS.md` passed on 2026-10-23, including the local
reserved-namespace check. Its completion record is checked on that confirmation.
Pasted CLI output additionally verifies portable resolution, changed-input cache
invalidation, deliberate engine failure without overwriting the valid record,
and successful recovery. With production mirror delivery also verified, M11A
acceptance is complete; lock persistence/publication and managed environments
remain in their later owning milestones.

Cross-stage handoff: optional Python `runtime.dependencies` changes the closed
manifest schema. Stage 2 M1B now has a working-tree handoff note; its requirement
to inspect the **merged** schema/type/tests remains unchecked. No APDS freeze,
Tool scaffold change, Node/Python SDK change or version bump is claimed.

## Milestone 11B: Lockfile Evolution and Forward Compatibility
> Scope note: persist Milestone 11A resolution state in `agent.lock` and make the lockfile safe to evolve. Owns the `LockedPackage` shape, content-derived version selection, the forward-version guard, resolve/install DTO plumbing so regeneration cannot erase state, the duplicated `PackageKind` reconciliation, **propagation of the Milestone 11A `PythonResolution` type into every remaining contract surface**, and **lifting the M11A block on publishing dependency-bearing Tools**. Milestone 11A defined and validated the type and deliberately refused to publish declarations it could not retain; this milestone is where the resolution reaches the lock, the DTOs, the publish descriptor, artifact metadata, and the release-manifest digest — and therefore where dependency-bearing publication becomes legal.
>
> Split from 11A because the forward-version guard is a standalone correctness fix with value independent of Python work: today `lockfile_version` is not consulted before untagged deserialization, unknown fields are silently ignored, and an older CLI can read a future lock and rewrite it lossily. That is worth shipping on its own merit.
>
> Implementation notes:
> - Fix the forward-compatibility hazard **before** writing any v4 data.
> - Plain `agentpm install` fully regenerates `agent.lock` from the server resolve plan. **Consumer-side** Python resolution state must therefore flow through the resolve DTO/plan or it will be erased.
> - Keep one logical locked package entry per `kind:name@version`; target selection is local machine state, not source-controlled lock state.
> - **`agent.lock` takes on a second role in this milestone, and the container is decided — do not re-open it during implementation.** Until now the lock has been an install output. Writing the author's own pre-publish resolution into it makes it also a record of what this project *is*, which is a different role from what it *depends on*. The two must not share a container:
>   - **consumer role** — "this project installed Tool X@1.2.3, whose resolution is R" → `LockedPackage.python_resolution`, projected from the resolve plan, regenerated on every install;
>   - **author role** — "this project *is* Tool X, and its own dependencies resolve to R" → the **local-root lane**, alongside the Milestone 11A input fingerprint.
> - The author resolution cannot go in `packages`, for two independent reasons. `lock_from_plan` (`crates/agentpm-cli/src/semver/types.rs:379`) builds `packages` into a fresh `BTreeMap` from `plan.items` alone, and an unpublished local release can never come back from the server, so the entry would be erased by the next `agentpm install`. And `LockedPackage.integrity` is the logical release digest, which does not exist before the release does — a self-describing entry would need a placeholder for a required field. Do not create a synthetic `LockedPackage` for the project itself.
> - **There is no local Tool root today, and the current fallback mislabels one.** `LockRoot` has exactly four variants — `LocalAgent`, `RegistryAgent`, `LocalSkill`, `RegistrySkill` — and `build_lock_roots` (`crates/agentpm-cli/src/commands/install.rs:762-776`) branches only on `kind == "skill"`, sending everything else to `LockRoot::LocalAgent` under key `local:agent` with name defaulting to `"local-agent"`. A `kind: tool` project is therefore already recorded as a local *agent*. This milestone needs an explicit local-Tool root representation; adding one is also a small correctness fix, not only new structure.
> - **Preservation across regeneration must be deliberate.** Full install reconstructs both `packages` and `roots`, so merely adding fields to the root type still loses them — nothing in `agent.json` can regenerate a resolution. Install must carry the matching local project's resolution and fingerprint forward. The seam already exists and is already load-bearing: `build_updated_lock` (`install.rs:450`) receives `existing_lock` and already merges from it on the targeted-install path, and `lock_from_packages_and_roots` is already imported for that purpose. Only the `spec.is_none()` full-regeneration branch discards it. This does not weaken the "lock mirrors the resolve plan" invariant, which only ever applied to `packages`.
> - **Matrix builds share one prepared resolution; they do not each resolve.** Milestone 16's fan-in rejects a release whose matrix jobs disagree on the Python resolution digest, and independent resolution against a live index cannot guarantee agreement — one upstream release of an unpinned transitive between the first and last job is enough to break the build. A committed lock is the chosen mechanism because it also keeps a dependency change visible as a reviewable diff and keeps a local publish and a CI publish of the same commit producing the same resolution. A prep job distributing one resolution to every matrix job would also work mechanically; it is rejected because it makes the resolution invisible to review and reproducible only inside CI.
> - The M11A publication block exists in **two** places — the CLI `bail!` in `publish.rs` and the registry's `publish:init` rejection of non-empty `runtime.dependencies`. Both must lift in this milestone, and the registry side must not lift before the lock path actually persists resolution state, or a custom client can publish a dependency-bearing Tool consumers cannot install.
> - Reuse the Milestone 11A `PythonResolution` type verbatim on every surface. Do not restate its field shape, re-derive its ordering, or add a parallel per-surface struct — the whole point of fixing the contract in 11A is that these surfaces share one validated type whose digest is stable across all of them.
> - **Resolutions are larger than the 11A test fixtures suggest.** Those fixtures carry 1–3 digests; real ones carry every distribution digest for all four supported targets across every Python version in range. A single `pydantic>=2.8,<3` dependency measures 5 packages / 56 digests / ~5.7 KB of canonical JSON, so a Tool with a handful of dependencies lands in the tens of KB and a multi-Tool workspace lock can reach low hundreds. This is the intended cost of the digest decision, not a defect — but size it against real dependencies before settling serialization.

- [x] Extend `LockedPackage` with optional Python resolution state. Settle the **final** field layout here so Milestone 14B extends it rather than reshaping it:
  - [x] keep the four existing fields unchanged — `kind`, `name`, `version`, `integrity`;
  - [x] add one optional typed field for Python resolution (for example `python_resolution: Option<PythonResolution>`), `#[serde(default, skip_serializing_if = "Option::is_none")]` so v2/v3 locks round-trip byte-identically;
  - [x] do **not** add any target, selected-artifact, interpreter, or environment-fingerprint field — those are machine-local `.agentpm/` state, not portable lock state;
  - [x] note explicitly in code comments that `integrity` is the single logical package digest, and that Milestone 14B redefines what it *contains* for new-format releases (the 64-char lowercase release digest) **without changing its type or adding sibling digest fields**.
- [x] Confirm the `LockedPackage` layout chosen here satisfies Milestone 14B's integrity requirements before implementing, so the struct is touched once across the two milestones.
- [x] Propagate that same `PythonResolution` type into the remaining contract surfaces (moved here from Milestone 11A, which defined and validated the type but persisted it nowhere):
  - [x] resolve/install DTOs and `ResolvePlan`;
  - [x] publish/build descriptor;
  - [x] artifact inspection metadata;
  - [x] release-manifest digest input;
  - [x] reuse the 11A type itself on each surface — no parallel per-surface struct, no re-derived ordering;
  - [x] add a test asserting the same resolution digests identically through `agent.lock`, the resolve DTO, and the release-manifest digest input, so a divergent re-serialization on any one surface fails rather than silently producing a second digest for the same resolution.
- [x] Add an explicit **local Tool root** representation, and stop routing `kind: tool` into `LockRoot::LocalAgent`:
  - [x] add the variant and its `local:tool`-style key rather than reusing the `_ =>` fallback in `build_lock_roots`;
  - [x] carry the project's own `PythonResolution` and the 11A input fingerprint on that root;
  - [x] do not add a synthetic `LockedPackage` for the project itself, and do not place a placeholder in `integrity`;
  - [x] confirm an existing lock that recorded a Tool project as `local:agent` still reads, and decide explicitly whether it is rewritten to `local:tool` or left alone — either is acceptable, silently producing both keys for one project is not.
- [x] Preserve the local project's resolution across full lock regeneration:
  - [x] carry the matching local root's resolution and fingerprint forward in the `spec.is_none()` branch of `build_updated_lock`, which currently discards them;
  - [x] keep `packages` a pure projection of the resolve plan — preservation applies to the local-root lane only;
  - [x] ensure carried-forward state **cannot** satisfy the publish freshness check when it is stale; preservation must not become a way for an out-of-date resolution to pass.
- [x] Write resolved Python state into `agent.lock` at publish time and gate publication on its freshness (moved here from Milestone 11A, which resolves and fingerprints but persists nothing):
  - [x] update/write the local-root resolution from the prepared resolution;
  - [x] fail publish if manifest declarations and resolved state disagree or the resolution is stale, reusing the 11A input fingerprint rather than inventing a second staleness rule;
  - [x] treat the committed lock as the authoritative shared input for multi-target builds, so every matrix job emits the same `pythonResolutionDigest` rather than resolving independently;
  - [x] **lift the M11A publication block in both places at once** — the CLI `bail!` in `publish.rs` and the registry's `publish:init` rejection of non-empty `runtime.dependencies`. Leaving either in place makes dependency-bearing publish fail for a now-wrong reason; removing only the server one lets a custom client publish a Tool whose resolution never reached the lock;
  - [x] confirm the client-side gate still fails **before** resolver acquisition, so a publish that cannot succeed never pays for a resolver download.
- [x] Relocate the Milestone 11A author-side resolution cache now that the committed lock is authoritative:
  - [x] the committed local-root resolution is the source of truth; any remaining cache is disposable acceleration only, and publish must behave identically with it absent;
  - [x] move it off `<project>/target/agentpm/python-resolutions/`. `target/agentpm/` is the pre-existing publish-output directory (`commands/publish.rs:501`, excluded from the tarball at `:562`), so it predates this cache — but it is build output, which is the wrong home for dependency preparation state;
  - [x] prefer the per-user cache directory already used for the managed resolver binary, so one cache serves every project and nothing unexpected appears beside an author's source;
  - [x] confirm a cold cache plus a committed lock publishes without re-resolving, which is the property CI depends on.
- [x] Size the lock against real dependency sets before finalizing serialization:
  - [x] measure `agent.lock` with at least one realistic multi-dependency Python Tool (something like `openai` plus `pydantic`) rather than the 11A unit fixtures, and record the measured size as evidence;
  - [x] decide deliberately whether `agent.lock` stays pretty-printed at that scale, and write the decision down — it changes diff readability and review burden for every consumer of the lock;
  - [x] confirm the chosen serialization still round-trips v2/v3 locks byte-identically, so a formatting change made for v4 cannot rewrite existing locks;
  - [x] confirm a multi-Tool workspace lock stays within the size assumptions the resolve/install DTO path and registry request limits already impose, and state which limit was checked.
- [x] Evolve lockfile-version selection:
  - [x] replace `requires_v3_lock(...)` with a `minimum_lock_version(...)`-style helper;
  - [x] return minimum version 4 when Python resolution/new release fields are present;
  - [x] retain versions 2/3 for locks that do not need v4 capabilities unless a broader migration is intentionally chosen.
- [x] Add a **forward-version guard before normal lock deserialization/mutation**:
  - [x] reject `lockfile_version` newer than the CLI supports;
  - [x] prevent older CLIs from silently dropping future fields on rewrite;
  - [x] audit `Lock::empty_v2()` and Harness `>=2` checks for correct max-version behavior.
- [x] Preserve current V1/V2-shape and on-disk v2/v3 reads.
- [x] Keep package keys target-independent (`kind:name@version`).
- [x] Update registry resolve/install DTOs and `ResolvePlan` so Python resolution state reaches `lock_from_plan` and survives a plain full-regeneration install.
- [x] Reconcile the duplicated `PackageKind` enumerations while both sides are being modified:
  - [x] the CLI lockfile/semver types define seven variants; the SDK install DTOs define eight, and only the SDK has `Template`;
  - [x] either share one definition across the crates, or keep two with an explicit total-coverage conversion;
  - [x] add a test that fails when one side gains a variant the other lacks, so the drift cannot be reintroduced silently.
- [x] Ensure published Tool release metadata/artifact contains the Tool-specific Python resolution independently of the publisher's broader workspace lock.

M11B implementation/verification (2026-10-23, America/Denver): complete within
this milestone's scope. Author state is stored on `local:tool`; installed Tool
state comes from the Registry plan on `LockedPackage`. Live publication requires
a matching committed lock; dry-run preparation/refresh writes it. Disposable
preparation cache moved to the OS per-user cache. Tool archives carry only their
own typed `agentpm-python.json` metadata, bound to the publish reservation.

Lock version selection is content-based with a monotonic existing-version floor:
no Python state means no forced v4 upgrade, and regeneration never downgrades an
existing lock. Unchanged logical state preserves existing bytes and `generated`.
Removing declarations drops local Python state on normal install. Genuine v2/v3
fixtures produced by the installed pre-11B CLI verify byte compatibility; frozen
and carry-forward regression checks were also verified with deliberate mutations.

Registry persistence uses additive migration `V1.97__Persist_Python_Resolution.sql`
and pinned symbolic marker validation (`dep-logic==0.7.2`). The realistic committed
fixture covers 14 distributions / 113 hashes: 16,213-byte author lock, 15,104-byte
Tool metadata, 322,233-byte serialized 20-Tool lock and 259,001-byte resolve DTO.
Pretty printing is retained deliberately. See `test-plan.md` for tests, limits,
known baseline tooling failures and operator/deployment boundaries. No version
bump, CLI release, production deployment or 11C environment provisioning is claimed.

M11B operator acceptance (2026-10-24, America/Denver): the operator confirmed
hosted CLI CI is green and all local/dev manual tests pass. The runbook completion
record and `test-plan.md` contain the acceptance evidence and the duplicate
file-list fixture note. Milestone 11B is complete; no 11C environment or execution
acceptance is implied.

## Milestone 11C: AgentPM-Managed Python Runtime Environment
> Scope note: give new-format Python Tools somewhere for their dependencies to live and execute. Owns the AgentPM-managed per-Tool environment under `.agentpm/`, eager provisioning during install, execution of dependency-bearing Tools through the managed interpreter, and runtime fingerprint validation. Preserves legacy vendored Tool behavior unchanged.
>
> This milestone **owns the implementation of Python environment provisioning** for the single artifact a consumer installs today. Milestone 15 does not reimplement provisioning; it generalizes this same code path to the target-selected artifact and adds the no-compatible-artifact failure modes. Build the provisioning surface here so Milestone 15 extends it rather than duplicating it.
>
> Implementation notes:
> - Current AgentPM has no venv, no pip/uv integration, and no Python dependency location. `agentpm run` resolves `python`/`python3` from PATH (or `AGENTPM_PYTHON`) and invokes it directly.
> - Existing `_vendor` support is entirely author code (`sys.path.insert`) and must keep working for legacy Tools.
> - **Why `uv` is on the consumer side too.** Replaying exact pins needs no resolver, so this is not about dependency solving — install must never re-resolve. `uv` is here as the *environment/provisioning engine*: it removes the user's `pip`/`ensurepip`/`setuptools` from the install path entirely. Concretely, on Debian and Ubuntu `python3 -m venv` fails unless the separately packaged `python3-venv` is installed, because `ensurepip` is split out of base Python. A pinned static `uv` creates and populates the environment regardless of what venv/pip behavior shipped with the user's interpreter. Do not substitute `python -m venv` + `pip install --no-deps`; it walks straight into that failure.
> - Acquisition comes from Milestone 11A. Call that component; do not re-implement discovery, download, or verification here.
> - The closing test block covers Milestones 11A–11C together; treat it as the integration gate for the trio rather than 11C-only coverage.

- [x] Add an AgentPM-managed per-Tool Python environment under `.agentpm/`, keyed by:
  - [x] Tool identity/version;
  - [x] consumer target;
  - [x] resolved interpreter major/minor or equivalent ABI identity.
- [x] During install, resolve the interpreter using current `AGENTPM_PYTHON`/PATH family and minimum-version rules.
- [x] Create/reuse a managed venv-style environment with that interpreter through AgentPM-managed `uv`, not through the interpreter's own `venv`/`ensurepip`.
- [x] Provision by **replaying** the stored `PythonResolution`:
  - [x] converge the environment to the complete recorded set (a `sync`-style operation), so an interrupted provisioning run self-heals instead of accumulating partial state;
  - [x] do not perform a new dependency resolution at install time;
  - [x] ensure the install path **cannot** add a package absent from the stored resolution.
- [x] Verify every installed distribution against the recorded digests:
  - [x] run hash-verified installation;
  - [x] treat a digest mismatch as a fatal install error — never a warning, and never a silent retry from an alternate source;
  - [x] fail the install session rather than leaving a partially provisioned environment that looks complete.
- [x] Keep install **wheel-only — never build from source**:
  - [x] if no wheel compatible with the consumer target/interpreter exists for a locked version, fail with a distinct no-compatible-wheel error naming package, version, and target;
  - [x] the recorded sdist digest exists because it is part of the published distribution set, not as license to build it.
- [x] Distinguish these three install failures in error text — they look alike to users and have entirely different causes:
  - [x] distribution **gone** from the index;
  - [x] distribution present but **digest mismatch** (a supply-chain signal);
  - [x] distribution present and intact but **no wheel for this target**.
- [x] Update `agentpm run` so new-format dependency-bearing Python Tools execute with the managed environment's Python interpreter.
- [x] Validate the local runtime-environment fingerprint at run time; if interpreter/target changed, deterministically rebuild or fail with an actionable refresh/reinstall instruction.
- [x] Preserve legacy Tool execution through current PATH/`AGENTPM_PYTHON` semantics.
- [x] Preserve existing author-written `_vendor` imports alongside managed environments.
- [x] Warn when a new-format Tool both declares dependencies and vendors likely dependency trees.
- [x] Add tests covering:
  - [x] pure-Python dependency;
  - [x] dependency with native wheel;
  - [x] conditional/marker dependency;
  - [x] interpreter minimum/version changes;
  - [x] future lockfile rejection;
  - [x] plain install lock regeneration retaining Python resolution;
  - [x] digest mismatch on a fetched distribution fails the install;
  - [x] locked version with no target-compatible wheel fails with the no-compatible-wheel error rather than building;
  - [x] install does not install any package absent from the stored resolution, even if a dependency's own metadata requires one;
  - [x] a cached managed `uv` is reused with no network access on second install;
  - [x] `AGENTPM_UV_PATH` pointing at a wrong-version binary is rejected with the required version named;
  - [x] a PATH-installed `uv` of a different version is never selected.

M11C implementation/verification (2026-10-24, America/Denver): managed consumer
environments, exact hash-verified wheel-only replay, run/Harness environment
validation and the dependency-using `validate-order` example are implemented.
Local tests cover real pinned uv, Pydantic/native wheels, interrupted creation
and sync, failed-refresh invalidation, changed interpreters, no ensurepip,
normal/frozen installs and install-finalization failure boundaries. Portable lock
and resolution formats are unchanged. See `test-plan.md` for verification and
limits. The operator has confirmed all four updated Resolver Contract jobs pass,
closing the hosted native verification gate. Review is complete, and the operator
confirmed all cases in `manual-tests/CLI_M11C_MANUAL_TESTS.md` passed on
2026-10-24. **M11C acceptance is complete.** Production rollout, CLI release and
the `summarize-text`/`devwork-copilot` rollout remain Release Band 8 work.

## Milestone 12: Python Tool Artifact Compatibility
> Scope note: establish a conservative compatibility model for the Tool payload itself, distinct from Python dependency portability, so AgentPM can distinguish portable `any` artifacts from target-specific native payloads. This milestone defines package-time classification metadata on top of the canonical target vocabulary. It does not yet implement multi-artifact registry storage, target-aware install selection, CI fan-in publishing, or retroactively classify legacy releases.
> Implementation notes:
> - The canonical target identifier and current-target detection are **defined in Milestone 11A**, because the managed Python environment is keyed by consumer target. Consume that representation here; do not introduce a second target vocabulary.
> - `client.os` / `client.arch` in today's publish descriptor describe the publishing machine only; they are not compatibility metadata and must not be reinterpreted as such.
> - `any` is reserved for payloads AgentPM can safely treat as portable; when uncertain, choose the current concrete target.
> - Declared Python dependencies are installed separately for the consumer and do not make the Tool payload target-specific.
> - `.whl` is currently blocked as an embedded archive. Keep it blocked for normal Tool payloads; target-side dependency installation removes the need to ship wheels inside the Tool tarball.

- [x] Consume the canonical target identifier and current-target detection defined in Milestone 11A; do not redefine either here.
- [x] Define portable `any` **payload** semantics explicitly — what makes a payload safely target-independent, as distinct from `any` merely being a valid target value. The reviewed expanded §22 policy has shared-inventory, local, hosted and operator manual evidence; the later Node correction's fresh CI gate is also confirmed complete below.
- [x] Implement conservative payload classification after final file collection:
  - [x] `.so`;
  - [x] `.dylib`;
  - [x] `.pyd`;
  - [x] `.dll`;
  - [x] `.exe`;
  - [x] executable/native binary signatures where practical;
  - [x] known platform-specific unpacked vendored binary content.
- [x] Treat unknown executable/binary payload conservatively as current-target specific.
- [x] Record the accepted M12 publisher-platform permission limitation in §22:
  Unix preserves executable bits while Windows derives archive modes from
  extensions, so recognized text/data can narrow only on Unix. Keep the existing
  modes and `.py` exception; do not remove permissions or invent a Windows
  narrowing diagnostic. Defer mixed-claim fan-in enforcement to M16 below.
- [x] Ensure declared Python dependencies do not force Tool payload to target-specific.
- [x] Keep `.whl` blocked as an embedded Tool payload unless a non-dependency use case is separately justified.
- [x] Add target + artifact-format metadata to the package-build descriptor.
- [x] Preserve legacy Tool behavior with compatibility unknown/legacy rather than inventing `any`.
- [x] Ensure local publish of a pure source Tool can produce `any`.
- [x] Ensure local publish of a native payload claims only the detected current target.
- [x] Add tests for pure Python/data payload versus native payload classification on supported host families. These must actually execute on macOS and Windows, not only Linux — see the AgentPM CI matrix task in Milestone 16, and schedule it before relying on this coverage. The operator confirmed all four updated Resolver Contract jobs passed after the Windows fixture correction.

M12 implementation/local verification (2026-10-24, America/Denver): completed
Python Tool archives are classified without changing their bytes, manifests or
locks. A typed Rust build descriptor carries canonical `target` and `format`;
publish sends it as `artifact` metadata and dry runs display it. The existing
Registry accepts that additive request field but does not persist/enforce an
artifact inventory. No new release/storage/install model or standalone build
command is introduced. Local Apple Silicon tests pass, including both prepared
dependency-bearing examples. This is initial-classifier evidence, not acceptance
of the revised §22 policies. Format and collection implementations have separate
fresh local evidence below. Independent review, normal CI and all four updated
native Resolver Contract jobs are now operator-confirmed complete, including
Windows after its fixture correction. The operator has now confirmed all manual
tests pass, including the rebuilt Node step, and that final CI is all green.
**M12 acceptance is complete.** Release Band 8 production rollout and its
remaining verification checks are tracked separately below.
See the M12 evidence in `test-plan.md`.

### M12 format-policy follow-up — implementation and verification

The reviewed decisions in `spec.md` §22, "M12 portable format policy", were
authorized for implementation. The items below record the expanded classifier;
initial-classifier evidence above is historical. Directory collection policy
was deferred during this phase and is now implemented below. Updated hosted
verification and manual acceptance are operator-confirmed complete, including
the later Node correction's final CI gate below.

- [x] Before classifier changes, introduce the authoritative portable-format
  inventory at `crates/agentpm-sdk/contracts/tool-payload-formats-v1.json`, as
  specified in §22: grouped text/source suffixes, exact basenames, basename
  prefixes, named text exceptions and binary aliases/signature/header
  definitions. Validate its
  type/version, shape, normalization, uniqueness, category conflicts and bounded
  signature checks. Embed/consume it in Rust; enumerate it in tests instead of
  copying exhaustive inventories into match arms or parameter arrays. Do not
  add runtime policy configuration or a generic rules engine.
- [x] Add a checked-in generation/verification command with a read-only check
  mode for the spec's candidate tables and special-name inventories; run it in
  normal CI. Exhaustive product-doc inventories must use the same source.
  Deliberate data/document mismatches must fail verification. Keep independent
  representative fixtures to verify actual matching and precedence, not only
  consistency between consumers of the same data.
- [x] Include native filename suffixes and literal versioned-name fragments in
  that same inventory and generated/checkable spec section. Remove the Rust
  native-name list; reject malformed/duplicate/conflicting registrations.
  Enumerate native cases from the data and assert `native_content`, not just
  concrete-target fallback. Mutation-test native suffix/fragment drift in both
  the document and inventory while preserving the existing matching behavior.
- [x] Expand portable text/source candidates to the exact suffix and basename
  inventories seeded from §22 into the shared data file, retaining full-file
  text/native-signature checks and case-insensitive comparisons. Include `.pyx`,
  `.pxd` and `.ipynb` as source/data without extension-only warnings. Include
  `.in` text inputs such as `MANIFEST.in` and `requirements.in`, without treating
  them as executable build steps or additional dependency sources. Keep JS/TS
  and `.rtf` outside this initial expanded list with an honest unknown-format
  diagnostic.
- [x] Treat `.env` and `.env.*` as named text-configuration candidates, including
  `.env.example`: case-normalized basename equality for `.env`, basename-prefix
  matching for `.env.`, with both modes declared in the shared policy data.
  Do not reuse extension extraction, match parent directories/substrings,
  override native/binary/executable/environment rules or add a secrets-denial
  policy. Cover positive and negative dotenv name matches independently of the
  data-derived format enumeration.
- [x] Remove the blanket `.egg-info` narrowing rule: inspect a single-file
  `*.egg-info`, its conventional `PKG-INFO`, and other directory members under
  the text/normal per-member rules. Preserve shipped metadata; do not exempt
  bundled native/unknown files. Keep `.dist-info` and known installed-environment
  state conservative. Do not add a blanket `.pth` prohibition.
- [x] Add dedicated format recognition for `.png`, `.jpg`/`.jpeg`, `.webp`,
  `.pdf` and `.woff2` using the exact bounded signature/header checks in §22;
  do not route them through `portable_text` or rely on suffix alone.
- [x] Fail packaging on a recognized binary suffix/signature mismatch, naming
  the file and expected format, and directing the author to rename it to its
  actual format or replace it with valid expected-format data; explain updating
  file references after a rename. Check all relevant members even after another
  member narrows the archive. Fail before publish-init/upload and preserve the
  prior valid archive on failure using the existing atomic packaging pattern.
- [x] Document and test the publish-time compatibility change: previously
  accepted mismatched assets must be fixed before a subsequent publication
  through the updated packager, including a new version of an existing Tool.
  Preserve legacy reads/installs and immutable-version rules. Correctly renaming
  to an unrecognized actual format removes the mismatch error but still narrows
  the payload; do not imply renaming always produces `any`.
- [x] Report concrete-target narrowing with relative path, accurate reason and
  canonical build target in dry-run/live packaging; order/deduplicate diagnostics
  deterministically. Preserve existing executable-bit, native-signature,
  versioned `.so.N`, symlink and embedded-wheel/archive protections.
- [x] Implement narrowing as a warning and binary mismatch as an error under
  §22's output contract: stderr diagnostics survive `--quiet`/non-TTY use;
  narrowing alone does not fail packaging or change manifest-lint `--strict`
  semantics. Keep stable typed code/severity/path/message/help and applicable
  target/expected-format data, render from that data, and carry successful
  `diagnostics` in build descriptors/publish `artifact` metadata. Failed builds
  retain structured errors without returning a successful artifact descriptor.
  Preserve this data for M16's machine-readable build/fan-in surfaces; do not add
  a publish `--machine` flag or implement fan-in in M12.
- [x] Update product docs to explain the expanded format list, configuration/
  metadata exceptions, signature recognition limits, and the boundary between
  payload classification and author responsibility for code behavior.
- [x] Implement and run the follow-up cases in `test-plan.md`, including the
  four native hosted targets; retain initial tests as regression coverage and
  record fresh evidence rather than reusing the initial classifier's results.

The format-policy phase changed no directory traversal/collection exclusions.
The separately reviewed collection policy below is now implemented. See the
separate fresh local verification records in `test-plan.md`; the
combined platform-test item is now checked following the operator's confirmation
of all four updated hosted jobs. Format-policy evidence does not substitute for
the separate collection evidence.

### M12 collection-policy follow-up — implementation and verification

The reviewed "M12 collection policy" in `spec.md` §22 was authorized for
implementation. This is Python Tool collection only, not a general ignore system
or a change to other kinds. Keep the previously implemented format policy intact.
Fresh local evidence is separate from operator-confirmed hosted success and
completed operator manual acceptance.

- [x] Add the reviewed minimal collection rules and shared installed-state
  vocabulary to the checked-in policy contract; derive classifier/collector
  environment names and markers from that source. Keep `.dist-info`
  classification-only, and preserve `.egg-info` text-metadata handling. Extend
  strict data validation and generated inventory/normal CI checks so these
  inventories cannot silently diverge. Do not maintain a second hardcoded list.
- [x] Prune incidental `.git` (including worktree pointer files), named state/
  environment directories, custom-named environment roots with a direct
  `pyvenv.cfg` marker, OS-cruft files and dotenv files under the exact §22
  matching/type rules. Probe markers case-insensitively without executing
  Python, reading marker contents, following links or recursively scanning an
  excluded tree. Fail directory inspection errors with the affected path;
  do not use silent scan caps or let skipped descendants consume entry limits.
- [x] Preserve only exact `.env.example` as the incidental dotenv exception,
  case-insensitively; explicitly record why `.env.sample`/`.env.template`/
  `.env.dist` remain omitted by default. No content-based secrets scanning or
  blanket prohibition on explicitly listed dotenv files.
- [x] Implement explicit-root/subtree overrides and separately requested
  descendants independent of manifest entry order, with member deduplication.
  A normal parent-directory request still filters excluded descendants; an
  explicitly requested excluded directory preserves its subtree. Preserve
  required manifest/entrypoint/generated inputs without accidentally including
  an entire excluded parent. Missing explicit paths stay fatal; overrides do
  not bypass existing packaging/integrity restrictions.
- [x] Report omitted traversal paths and explicit overrides as stable typed
  `info` diagnostics, with matching rules/reasons and help where appropriate.
  Print on stderr in dry/live/quiet/non-TTY runs; preserve them in descriptor
  metadata for later machine/CI consumers. Count only observed omission units,
  not unseen files inside pruned directories. Keep `any` possible with `info`
  diagnostics and preserve warning/error and `--strict` behavior.
- [x] Do not honor `.gitignore` or ecosystem ignore semantics. Do not expand
  automatic omissions to generic build/cache/configuration names, or discard
  Makefile/Dockerfile/CODEOWNERS and other plausible runtime fixtures. Keep
  unrecognized surviving files on the visible narrowing path. Retain existing
  reserved/generated-output exclusions and the immutable legacy read/install
  path, without adding release fields or cross-target byte-equality rules.
- [x] Update product docs for incidental versus explicit inclusion, exact
  exclusions, marker detection, dotenv/case choices and omission reporting.
  State that policy/tooling changes can change subsequent archive bytes;
  checked-in data alone does not promise cross-version byte reproducibility.
- [x] Implement the collection-specific cases in `test-plan.md`, run relevant
  local checks and all four hosted native jobs, and record fresh evidence plus
  operator manual acceptance. Initial/format-policy results do not check off
  any collection implementation or acceptance requirement.

Collection implementation, local tests, updated hosted runs and operator manual
acceptance are complete. The separately authorized Node correction's fresh
hosted CI gate is now confirmed complete as well. Release Band 8 retains its
production rollout checklist.

### Authorized Node packaging correction from manual testing

The operator explicitly authorized this pre-existing collector fix before
resuming the Node runbook step. It does not extend Python collection policy.

- [x] Normalize non-Python Tool member names before deduplication and generated-
  output exclusions, including dotted explicit requests/entrypoint aliases and
  empty relative manifest parents. Exclude old and in-progress build outputs.
- [x] Add CLI archive regressions for default/dotted/absolute/nested-relative
  manifest paths, overlapping requests and repeated packaging. Keep Node's
  dotenv/Git/environment inclusion and ordinary blocked-archive/atomic-failure
  protections. Update docs and the resumable Node runbook assertions.
- [x] Confirm operator acceptance of the rebuilt Node step and completed M12
  manual walkthrough. The operator verified six unique Node members with no
  generated output and subsequently confirmed all manual tests pass.
- [x] Confirm fresh normal CI/native Resolver Contract runs for this correction.
  The operator confirmed final CI is all green after the Node correction and
  CLI 0.1.37 preparation. This closes the last M12 verification gate; production
  release/deployment remains operator-owned Release Band 8 work.

## Release Band 8: Python Tool Dependency Contract

Covered milestones: 11A-12.
This gives us the first-party Python dependency story working end to end for a single published artifact: declaration in `agent.json` with real lint validation, a pinned AgentPM-managed `uv` acquired from AgentPM-controlled storage and resolving to exact portable state, the fixed `PythonResolution` v1 contract with distribution digests, lockfile persistence with a forward-version guard, an AgentPM-managed per-Tool environment under `.agentpm/`, digest-verified wheel-only provisioning, dependency-bearing execution through `agentpm run`, and honest payload-compatibility classification. These four milestones are one band because any smaller cut ships a Tool that can declare dependencies it cannot install, or install dependencies it cannot verify are compatible with the consumer's target. Legacy vendored Tools keep working unchanged throughout. If schedule pressure demands an earlier partial release, the Milestone 11B lockfile forward-version guard is the only piece with standalone value — it is a correctness fix for a hazard that exists today.

- [x] Prepare CLI **0.1.37** (`v0.1.37`) and the matching website status-page
      release date **2026-10-24**. Keep SDK versions and the published **0.1.36**
      APDS tolerance floor unchanged; verify crate/lockfile/binary/status marker.
- [x] Verify the post-merge resolver `ETXTBSY` correction in hosted CI. The
      operator separately confirmed the correction's hosted CI verification is
      complete. This supersedes the earlier pre-fix green rerun; it does not alone
      establish which published CLI version includes the correction. See the
      post-merge evidence in `test-plan.md`.
- [x] After the final required CI/native checks pass, operator publishes CLI
      **0.1.37**, then deploys **Registry**, then **Web**, in that order. CLI tag
      publication remains operator-owned. The CLI release workflow first verifies
      the already-deployed uv inventory/mirrored bytes; do not bypass that gate.
      Operator confirmed production deployments of CLI, Registry and Web complete.
      Installed-version/contract smoke checks and the separate retry-fix CI evidence
      item remain independently tracked rather than inferred from deployment alone.
- [x] Verify installed CLI reports **0.1.37**, the deployed Registry serves the
      Python resolution/install metadata contract, and the deployed status page
      advertises **0.1.37** with **2026-10-24**. Publish/install dependency-bearing
      production Tool versions only after the Registry deployment is verified.
      Installed CLI and real production publish/install metadata were verified;
      the live status page reports **v0.1.37** with a relative release age consistent
      with its source date **2026-10-24**. See the production smoke evidence in
      `test-plan.md`.
- [x] Prepare the examples locally: retain reviewed `summarize-text` **0.1.9**
      source/dependency state; prepare `devwork-copilot` **0.1.7** with exact
      `@zack/summarize-text@0.1.9` and explicit inspection/drafting Tool bindings.
      Update requirements/publish/verification instructions and verify tests, strict
      lint and isolated dry runs. No publication or consumer-lock fabrication.
- [x] After the CLI/Registry deployment, publish the prepared `agentpm-examples/tools-python/summarize-text` **0.1.9** to the production Registry as `@zack/summarize-text`, using its reviewed `agent.lock` and declared Python dependencies rather than a vendored native tree. Operator confirmed successful publication; retry-fix hosted CI verification is separately confirmed complete above.
- [x] After the Tool publication, publish prepared `@zack/devwork-copilot@0.1.7`
      and install it into a consumer workspace, updating the real consumer lock and
      managed environment. Operator installed it in `agent-app-devwork-python`;
      local inspection confirmed Agent/Tool versions, the reviewed resolution and
      the exact platform-applicable installed dependency inventory. Keep old
      published Agent versions unchanged.
- [x] Verify actual production summarization through direct
      `agentpm run @zack/summarize-text@0.1.9` with `OPENAI_API_KEY`. Operator
      supplied successful `ok: true` output and a real summary.
- [x] Verify an actual `summarize-text@0.1.9` Tool call through Harness using
      the published Agent **0.1.7**, rather than only a model-written summary.
      Operator-provided run `run-18e1aa0d9a8fe390-5` contains two summarizer
      invocation/completion pairs, zero reported errors and terminal status
      `handed_off`. The consumer lock selects those Agent/Tool versions; see
      the trace evidence and report-identity limitation in `test-plan.md`.
- [x] Confirm an existing vendored production Tool version still works for legacy consumers. Existing installed `@zack/sentiment-analysis@0.1.5` passed a real CLI machine invocation on Apple Silicon, returning `status: success`, `ok: true` and positive sentiment. Its VADER import was confirmed under `_vendor`, with no declared Python dependencies or managed-resolution metadata. Keep `validate-order` dev-only; no new publication is required.

**Band 8 acceptance is complete.** The original `summarize-text@0.1.8` smoke
attempt remains an Intel-only artifact limitation on the available ARM host;
its successful runtime execution is not claimed. The separately verified
vendored sentiment Tool satisfies the legacy-production execution check.
No immutable old package was modified to obtain a passing result.

The Python SDK demo app is pinned to Agent **0.1.7**, but its runtime check is
blocked by SDK support for lockfile v4 and CLI-managed Python Tool environments.
The operator explicitly deferred SDK changes to Milestone 17 below.
This is not evidence of a successful SDK-app run and does not reopen M12's
completed CLI/Registry acceptance. The independent Harness invocation is now
verified above.

## Milestone 13A: Release and Artifact Data Model

> Scope note: redefine what a published Tool release **is** — one immutable release composed of one or more target artifacts — and where its bytes live. Owns the canonical release-manifest contract, the three new publish/version artifact tables, the `PackageVersion` release-format discriminator, the legacy-column migration, the new immutable S3 layout, per-artifact self-description, and cross-artifact consistency validation. Does not change how uploads are performed (13B).
>
> Split from 13B because this half is additive and independently deployable: new tables, nullable legacy columns, and a release-format discriminator that defaults to legacy. Shipping it alone changes no publish behavior and de-risks 13B's transactional rework.
>
> Implementation notes:
>
> - `PackageVersion` remains the logical immutable version. Add child artifact rows rather than N package versions.
> - Existing `PackageVersion.sha_256/size_bytes/s3_key` are legacy single-artifact columns. Add an explicit release-format discriminator rather than inferring format from whether those columns are null.
> - Current S3 final key uses only a 12-hex digest prefix (`{prefix}{handle}/{name}/{version}/{sha256[:12]}.tar.gz`). New immutable artifact object identity should use full digest plus target.
> - Preserve the legacy read path exactly; do not reinterpret existing rows as degenerate new-format releases.

- [x] Implement the fixed canonical release-manifest contract:
  - [x] type string exactly `agentpm.package.release.v1`;
  - [x] fields: `type`, `kind`, `name`, `version`, `manifestDigest`, optional `pythonResolutionDigest`, `artifacts`;
  - [x] each artifact contains `target`, `digest`, `size`, `contentType`, and `format`;
  - [x] digest fields use `sha256:<64-lowercase-hex>`;
  - [x] `pythonResolutionDigest` is omitted when no Python resolution exists;
  - [x] `artifacts` is complete, target-unique, and sorted lexicographically by canonical target before JCS canonicalization;
  - [x] release manifest contains no upload IDs, temporary keys, mutable scan state, local paths, or publisher-machine telemetry;
  - [x] add typed Rust/Python/client-server representations and schema/validation fixtures.
- [x] Add SQLAlchemy model `PackagePublishRelease` backed by table **`package_publish_releases`**, containing:
  - [x] package/version identity;
  - [x] manifest + manifest digest;
  - [x] Python resolution + resolution digest where present;
  - [x] publisher;
  - [x] pending/finalized/expired status;
  - [x] expiration/resume metadata.
- [x] Add SQLAlchemy model `PackagePublishArtifact` backed by table **`package_publish_artifacts`**, containing:
  - [x] release ID;
  - [x] target;
  - [x] expected SHA-256;
  - [x] expected size;
  - [x] tmp/final S3 keys;
  - [x] upload status.
- [x] Add SQLAlchemy model `PackageVersionArtifact` backed by table **`package_version_artifacts`** under logical `PackageVersion`:
  - [x] target;
  - [x] SHA-256;
  - [x] size;
  - [x] S3 key;
  - [x] content type/artifact format;
  - [x] immutable association to the release.
- [x] Add explicit new-format fields on `PackageVersion`:
  - [x] release-format discriminator; new rows use `agentpm.package.release.v1`, legacy rows remain legacy/null;
  - [x] nullable release-level SHA-256 field (for example `release_sha_256`).
- [x] Migrate legacy scalar artifact columns `sha_256`, `size_bytes`, and `s3_key` to nullable:
  - [x] existing legacy rows keep them populated and authoritative for the legacy read path;
  - [x] new `agentpm.package.release.v1` rows store these legacy scalar columns as `NULL`;
  - [x] new-format target bytes/size/object identity come only from `package_version_artifacts`;
  - [x] new-format logical integrity comes from the release-level digest field;
  - [x] branch read logic on explicit release format, never heuristically on scalar nullability.
- [x] Preserve legacy scalar version columns/read path for existing one-artifact releases; do not reinterpret old rows.
- [x] Ensure unsupported old clients receive a clear "requires newer AgentPM" response for new-format releases rather than a release-manifest URL masquerading as a tarball.
- [x] Define new-format S3 layout using separate immutable objects, conceptually:
  - [x] `<S3_PREFIX>/<ns>/<name>/<version>/release.json`;
  - [x] `<S3_PREFIX>/<ns>/<name>/<version>/artifacts/<target>/<full-sha256>.tar.gz`;
  - [x] staging `uploads/<release-id>/<artifact-id>.tar.gz`.
- [x] Keep legacy S3 key layout readable.
- [x] Persist the canonical release metadata both in normalized DB state and immutable `release.json`.
- [x] Make each artifact independently self-describing:
  - [x] include logical `agent.json`;
  - [x] include Tool-specific Python resolution metadata/descriptor;
  - [x] include target/build descriptor as needed.
- [x] Validate all artifacts in one release share:
  - [x] kind/name/version;
  - [x] manifest digest;
  - [x] Python resolution digest.
- [x] Reject duplicate/conflicting targets.

M13A implementation/local verification completed 2026-10-25. The Registry now
has the additive release/artifact tables and explicit version storage shape;
existing publish routes remain legacy. Typed Rust/Python release models, shared
schema/canonical vectors, immutable document persistence and new-format artifact
preparation/validation primitives are implemented. Install/security compatibility
paths fail explicitly for unsupported release formats; no multi-artifact upload,
finalize, scanning, signing or target-selection workflow is enabled.

The local backend applied V1.98 during development. Its original bytes were
recovered and frozen at Pyway checksum **EAF4B273**; subsequent function hardening
is in V1.99. The operator confirmed successful restart after that recovery.
V1.100 removes reservation-wide final-key uniqueness so an expired reservation
does not block a later retry of the same immutable artifact. Tests apply the
complete sequence; keep these migrations frozen and use forward migrations for
any subsequent correction. A thin entrypoint retains Pyway's CLI/checksum/SQL
behavior while sorting versions numerically so a fresh deployment applies
V1.98 → V1.99 → V1.100 correctly; actual subprocess upgrade and password-log
redaction regressions cover this startup boundary.

Review follow-up: V1.101 extends the deferred inventory backstop to artifact
INSERT/UPDATE/DELETE, including committed pending versions and both sides of
reparenting. Earlier applied migrations stay unchanged. Real PostgreSQL tests
verify invalid mutations fail at commit and a temporarily inconsistent update
can commit after restoring the declared inventory; see `test-plan.md`.

Local verification and limits are recorded in `test-plan.md`. Acceptance gates:

- [x] Complete independent review; operator confirmed review is finished.
- [x] Confirm normal CI passes; operator confirmed success.
- [x] Run the updated native Resolver Contract jobs covering release types and
      artifact preparation; operator confirmed all four target runs passed.
- [x] Verify schema/vector parity and Registry contract tests locally across both
      working checkouts; evidence is recorded in `test-plan.md`. Hosted parity
      against CLI `main` is deferred to Release Band 9 below, after M13B and the
      CLI merge; it does not block completing M13A or starting M13B.
- [x] Complete the operator's M13A manual acceptance. Operator confirmed all
      tests in `manual-tests/RELEASE_M13A_MANUAL_TESTS.md` passed on 2026-10-25.
      M13A is complete; Release Band 9 deployment/storage and hosted parity gates
      remain open below.

## Milestone 13B: Multi-Artifact Upload, Finalize, and Scanning
> Scope note: make the publish transaction work for N artifacts — per-artifact reservations and presigned PUTs under one logical release, streaming uploads, reconciled size limits, reservation expiry/cleanup, rate-limit and timeout fitness for CI, one atomic finalize, post-finalize immutability, and per-artifact malware scanning. Consumes the data model from 13A.
> Implementation notes:
> - The existing `Upload` row has one `tmp_key`, one `final_key`, one digest and one pending row per `(package_id, version)`. A second concurrent artifact for the same version collides today; this is the central constraint to break.
> - Keep the existing `/v1/tools/publish/init|finalize` legacy behavior readable. New-format requests may reuse/version these endpoints but must not silently change legacy semantics.
> - Current publish buffers an entire artifact in memory (`tokio::fs::read`). New package/release upload should stream.
> - Current client and server upload caps disagree (3 GB client vs ~1 GB server). Treat server policy as authoritative.
> - Current malware scan model is asynchronous after publish. Preserve that timing, but create a scan per concrete target artifact and aggregate release-level status from the per-artifact results.
> - **Only malware scanning exists.** GuardDuty object tags drive `malware_status` for every kind. The SBOM/Grype/pip-audit/OSV columns are unwritten schema, so "adapt scanning" means adapting the malware path and nothing else. Three concrete call sites need changing — the version-scoped `s3_key` passed to `_enqueue_scan`, its one-scan-per-version early return, and the single-row read behind `scan_malware_status`.
> - M13A's deferred inventory trigger checks manifest/artifact-row consistency only. No FK/trigger links upload reservations to recorded versions or proves upload completeness; M13B must enforce that prerequisite in finalize.

- [x] Implement release init/resume that can return multiple per-artifact presigned PUT instructions without one-pending-upload collisions.
- [x] Implement per-artifact retries/resume while keeping one logical release reservation.
- [x] Stream artifact PUT bodies instead of `tokio::fs::read` buffering the entire file.
- [x] Reconcile size limits:
  - [x] server returns authoritative per-artifact max;
  - [x] client enforces the server policy;
  - [x] remove contradictory independent 3 GB vs 1 GB expectations.
- [x] Implement server-side cleanup/expiration for orphaned staged release artifacts.
- [x] Implement release-path expiry enforcement and define expired reservation/history retention explicitly; M13A's expiry fields/index do not supply a cleanup job, and V1.100 permits history to linger without defining its retention. Cleanup must be retry-safe and preserve finalized objects.
- [x] Define recovery when `release.json` was created but the DB transaction rolled back: identical bytes may retry, different bytes must not overwrite the immutable key. Object presence alone must not mark a release published; verify publication against committed database state.
- [x] Define and implement the release expiry/retry policy:
  - [x] choose a release-reservation lifetime deliberately, independent of and longer than any single presigned PUT window (today's presign is 900s, sized for one immediate upload);
  - [x] allow re-issuing a presigned PUT for a still-pending child artifact without invalidating the release or already-uploaded siblings;
  - [x] return expiry information the client can act on, so a CI job reports "release reservation expired, re-run the workflow" rather than an opaque S3 or `409` error;
  - [x] keep resume keyed at release + artifact level for the whole window;
  - [x] record the chosen lifetimes in the spec rather than leaving them implicit.
- [x] Audit publish rate limits against multi-artifact CI usage:
  - [x] count worst-case registry calls for a supported release, including presign re-issues and per-job retries;
  - [x] confirm a legitimate matrix publish cannot exhaust the current `10/minute; 100/hour` at `cost=3` budget;
  - [x] set distinct limits/costs for release-scoped endpoints if the audit shows the current budget is insufficient.
- [x] Separate registry-API timeouts from object-transfer timeouts in the publish client:
  - [x] today one 600s total timeout covers the S3 PUT because the same `reqwest` client is reused;
  - [x] prefer per-transfer connect/idle/progress timeouts over a single total-duration cap for artifact bytes.
- [x] Fix two pre-existing publish-path defects while this code is open, and carry the corrected behavior into the new release flow:
  - [x] `app/tools/application/cli.py:2099` resolves `final_exists_and_matches` as `False if code in ("404", "NotFound", "NoSuchKey") else False` — both arms are `False`, so `AccessDenied` or any other S3 error on the final HEAD is treated as "object absent" and triggers a copy. Distinguish genuine not-found from an error that should fail finalize.
  - [x] `publish_init` returns `"resumed": true` on the resume path, but the Rust `InitPublish` struct has no such field, so the client silently discards it and cannot tell a fresh reservation from a resumed one. Either add the field to the client type and surface it, or drop it from the response — do not leave a documented-but-ignored wire field, and make the equivalent release-level resume state explicitly visible to the client.
- [x] Implement one atomic release finalize:
  - [x] all intended artifacts present;
  - [x] verify stored bytes and upload state for every intended reservation child before persisting release rows; reject missing, pending or failed children even when the proposed manifest/artifact inventory is internally consistent, with failure-path tests proving no published version or finalized artifact rows survive;
  - [x] all metadata consistent;
  - [x] target set unique;
  - [x] release manifest constructed/persisted;
  - [x] DB release/artifact rows committed together;
  - [x] release becomes installable only after finalize succeeds.
- [x] Enforce immutable artifact inventory after finalize.
- [x] Enforce new semantic version requirement for adding targets.
- [x] Adapt malware scanning:
  - [x] enqueue one scan per `PackageVersionArtifact`;
  - [x] associate scan row/result with concrete artifact (artifact FK preferred);
  - [x] replace V1.55's unconditional one-scan-per-version index in forward migration V1.104 with null-artifact-only version uniqueness; retain per-artifact uniqueness and exercise actual historical scan migrations in PostgreSQL fixtures;
  - [x] keep that artifact association **optional** on the scan row rather than required, so a later resolution-scoped scan record is additive instead of a migration undoing this one. Stage 1 adds no such row;
  - [x] enforce the artifact/version parent pairing with a composite FK in forward migration V1.103; reject mismatched inserts/updates and validate existing rows without reassignment, while preserving null associations and legacy scans;
  - [x] replace the version-scoped `s3_key` input with the per-artifact key. `_enqueue_scan` is handed `tool_versions.s3_key`, which V1.98's CHECK forces NULL for new-format releases, while `package_version_scans.s3_key` is `NOT NULL`;
  - [x] replace the one-scan-per-version guard. `_enqueue_scan` returns early when any scan row exists for the version, so without this change only the first target is ever enqueued;
  - [x] aggregate for the detail surface. `ToolsService` reads a single scan row into `scan_malware_status`; with N rows it needs a rollup. Keep the DTO field shape if possible so the website needs no change, and treat a shape change as a user-facing contract change;
  - [x] preserve queued/running/unknown semantics;
  - [x] if any artifact is infected, yank/disable the logical version;
  - [x] never display the version as wholly clean unless all required artifact scan states justify it.
- [x] Do not wire existing unused SBOM/Grype columns as a side effect unless deliberately scoped. They are unimplemented schema: nothing writes `sbom_cyclonedx`, `grype_summary`, `grype_findings`, `pip_audit_findings` or `osv_findings` today, so there is no existing vulnerability scanning to adapt or regress.
- [x] Do not add dependency vulnerability analysis here. Declared Python dependencies are not in the archive — wheels are fetched at install time — so archive scanning cannot cover them, and the exposure is described by the stored `PythonResolution`. That is a new capability for Stage 2's Package Health evidence model, not Stage 1 hardening. See spec §25.

### M13B storage-prefix and scan-rollup follow-up — specified after the initial implementation

These were settled during review while 13B was in progress. The checked scanning items
above predate them, so verify each against what actually landed rather than assuming
coverage. Details in spec §22 storage layout and §25.

- [x] Move new-format final objects under the configured `S3_PREFIX`, reusing the existing namespace/package/version structure instead of a bucket-root `packages/` directory:
  - [x] `<S3_PREFIX>/<namespace>/<name>/<version>/release.json` and `.../artifacts/<target>/<full-sha256>.tar.gz`;
  - [x] **this is required because GuardDuty Malware Protection is prefix-scoped to `tools/`.** A bucket-root layout puts every new-format artifact outside the protected scope, so nothing scans or tags it;
  - [x] add prefix-aware tests asserting artifact and release keys fall inside the configured `S3_PREFIX`, so a future layout change cannot silently leave the protected scope;
  - [x] resolve the `S3_PREFIX` divergence while here: `cli.py` defaults it to `tools`, `resolvers.py` to empty, `config.py` to empty, and the M13A release key builders ignored it;
  - [x] verify legacy and new-format objects coexist at the same version path — legacy `<sha12>.tar.gz` versus `release.json` plus `artifacts/` — rather than assuming the `ck_tool_versions_release_storage` XOR makes it safe;
  - [x] note that final objects are created by conditional PUT rather than a staging copy; GuardDuty covers both PUT and Copy creation events, so the protected prefix remains the thing that matters.
- [x] Make a missing-tag/out-of-scope condition visible instead of merely plausible:
  - [x] log the poller's timeout branch explicitly. It currently sets `failed`/`unknown` after `SCAN_MAX_WAIT_MIN` (default 30) and emits no log, so the operator sees only `[guardduty-worker] updated N rows` — identical to a healthy poll, and indistinguishable from a real scanner failure;
  - [x] test the missing-tag path: objects that are never tagged must produce the timeout outcome and that log, not a silent degrade.
- [x] Verify the rollup against spec §25 rather than assuming the implemented aggregate matches:
  - [x] `clean` is **count-based** against `package_version_artifacts`, not status-based — a status-only rollup reports clean when enqueue failed and an artifact has no scan row at all;
  - [x] the rollup is computed server-side once, so the read DTO and any install-side consumer share one safety rule;
  - [x] name and document full row coverage separately from scan completion; keep result/progress aggregation independent and retain the defensive unrelated-artifact filter;
  - [x] `status` never rolls up to null: the Security view renders a missing status as "Scan: not run", meaning published before scanning existed, so a partially enqueued release must read `queued`/`unknown`;
  - [x] `notes` are not concatenated across rows; the poller appends free text that is rendered directly;
  - [x] the three flat DTO fields are unchanged. `scanTone`/`scanMessage` are duplicated across all eight kind pages and `PackageSecurity.test.tsx` covers the malware render path for each; the per-artifact array belongs to M14C, which already owns the public Security/read DTO evolution and the target-artifact inventory;
  - [x] a legacy version with one scan row and no artifact rows rolls up to exactly today's values.
- [x] Record the operator gates: confirm real scanning in dev and production before rollout, and size scanning cost across artifact objects, their bytes, and the `release.json` metadata object that the protected prefix also covers. Prefix alignment reduces configuration drift; it does not prove AWS coverage, and asynchronous scanning still never blocks publication.

### M13B review follow-up: bounded finalization without transfer-time DB locks

- [x] Enforce `MAX_RELEASE_UPLOAD_BYTES` (default 1,000,000,000 aggregate bytes) before reservation/storage and recheck both size budgets before transfer and commit; expose `max_total_bytes` in init and the Rust response type.
- [x] Document the equal 1 GB defaults as deliberate: artifact-first validation provides a precise single-file error, while independent limits permit later tuning; aggregate increases require transfer/request/concurrency sizing, not longer transfer-time DB locks or sum-of-artifacts snapshot disk.
- [x] End the initial read transaction before storage I/O, retain one temporary snapshot at a time, and limit the package/reservation locks to revalidation and DB-only persistence.
- [x] Reauthorize and recheck expiry, signing policy, full reservation intent/inventory, current budgets and version conflicts after storage; preserve idempotent concurrent completion.
- [x] Require explicit S3 SHA-256 request/response confirmation for fresh conditional PUTs instead of downloading them again; independently verify existing final objects and conditional-write race winners. Never use ETag as SHA-256 proof.
- [x] Exercise real PostgreSQL lock availability during every storage call, one-snapshot disk usage, concurrent mutations/completion, checksum failures, race winners and caller-owned pending changes; document remaining per-request/concurrency capacity limits.

M13B implementation and local verification completed 2026-10-25. The separate
release endpoint family is default-off (`RELEASE_PUBLISH_ENABLED=false`) and
limited to Python Tools. Legacy publication retains its format; its shared
upload transport now streams all package kinds. The implementation adds
release/artifact init/resume, presign renewal, bounded verified finalize,
24-hour expiry, five-minute staging cleanup, 30-day history retention,
conditional immutable storage retries, and artifact-specific malware scans.
The existing Security/install new-format gates remain intact. No new release
signatures, target-aware install, build/CI publish command, dependency
vulnerability analysis or public Python/Node SDK changes are introduced.

Forward migration V1.102 adds the reservation's intended release document and
an optional scan-to-artifact association. Earlier applied migrations remain
unchanged. The actual migration and Pyway numeric ordering are exercised on a
disposable PostgreSQL instance; see `test-plan.md` for evidence and limitations.

Acceptance status:

- [x] Complete independent review of the M13B implementation; operator confirmed review is finished, including the follow-up fixes.
- [x] Run updated normal CI and all four Resolver Contract jobs, including the
      streaming HTTP fixtures, and the Registry PostgreSQL release-upload job;
      operator confirmed all pass. Release Contract Parity failed as expected
      against pre-merge CLI `main`; its Release Band 9 gate remains open.
- [x] Complete operator manual acceptance of the gated release flow, interrupted
      uploads, finalization, cleanup and malware rollup; operator confirmed all
      runbook steps pass after the V1.104 fix, including real development recovery.
      Runbook and checked-in archive/request helper are in
      [`manual-tests/RELEASE_M13B_MANUAL_TESTS.md`](manual-tests/RELEASE_M13B_MANUAL_TESTS.md).
- [x] Confirm the V1.104 scan-index recovery fix in independent review; operator
      confirmed Claude reviewed and signed off on the fix.
- [x] Confirm the Registry PostgreSQL CI job passes with V1.104; operator confirmed
      the updated job passed. The Rust/native checks are unaffected by this fix.

M13B acceptance is complete: implementation, independent review including the
V1.104 fix, hosted checks and operator manual testing are all confirmed. The
separate Release Band 9 gates below remain open.

## Release Band 9: Release Data Model and Multi-Artifact Publishing
Covered milestones: 13A-13B.
This gives us the server-side capability for one immutable release composed of multiple target artifacts: the new release/artifact tables, the release-format discriminator, nullable legacy columns with the legacy read path preserved, the new immutable S3 layout, per-artifact reservations and presigned PUTs under one logical release, streaming uploads, reconciled size limits, reservation expiry and cleanup, CI-fit rate limits and timeouts, and one atomic finalize with per-artifact scanning. **The new-format publish path must remain gated and unadvertised at the end of this band**, because consumers cannot yet select or verify a target artifact — that arrives in Band 11. Shipping it here is still worthwhile: the migrations and the transactional upload rework are the highest-risk mechanical changes in the stage, and they get to deploy and bake with legacy publish/install behavior fully intact.

- [x] Prepare CLI **0.1.38** (`v0.1.38`) and matching website status-page date
      **2026-10-26**; verify crate, Cargo lockfile, CLI binary and status marker.
- [x] After required checks pass, operator publishes CLI **0.1.38**, then deploys
      **Registry**, then **Web**, in that order. Push/tag/publication and deployment
      remain operator-owned; complete CLI-main parity below before Registry
      deployment. Keep `RELEASE_PUBLISH_ENABLED=false` in Render throughout this
      band; deployment does not activate the new-format publish protocol.
      Operator confirmed CLI, Registry and Web are deployed to production.
- [x] Verify the installed CLI reports **0.1.38**; operator supplied the installed
      binary's version and successful probe-manifest lint output.
- [x] Confirm `RELEASE_PUBLISH_ENABLED=false` in production; operator verified
      the setting. The new-format publish protocol remains gated.
- [x] Confirm deployed Registry startup/migrations through V1.104 and legacy
      publish/install. The production health endpoint returns HTTP 200; the
      operator confirmed the migration and legacy-flow checks, and the disabled
      release flag is separately confirmed above.
- [x] Verify the deployed status page advertises **0.1.38**; a direct production
      HTTP 200 read shows `Latest: v0.1.38` and `Release: 1d ago`, consistent with
      the checked-in release date **2026-10-26**.
- [x] After M13B and the Release Band 9 CLI changes are merged to public CLI
      `main`, run the Registry's `Release Contract Parity` CI job against that
      ref for the Registry revision to be shipped. It compares schema/vector
      snapshots and executes Python contract tests; record both checked commit
      IDs. Complete this gate before Registry deployment. Manual pre-merge
      dispatch may select a proposed CLI ref, but does not replace this main-ref
      release gate. This is a band-level gate, not an M13A completion prerequisite.
      Confirmed successful [run 40258294740](https://github.com/agentpm-dev/registry/actions/runs/40258294740):
      CLI `main` revision `1b321f09863fad7ed3f38872fb4952be6efcd806` against Registry
      PR merge revision `9e0f5324576a8f64520d0dd646d75f8cde0e4f6b`
      (source head `d5991535e895f8ff2c31b09cbfc615e8f7051625`). Both jobs pass;
      operator subsequently confirmed all three production deployments.
- [x] Verify storage permissions/retention for the configured
      `S3_PREFIX` final-object layout and staging `uploads/`, including real
      GuardDuty scanning/tagging in dev and production before enabling
      the later new-format release flow. This is a Release Band 9 readiness gate,
      not an M13A completion prerequisite. No live S3 write/policy change or
      production deployment is claimed by the M13A implementation; keep the
      new-format publish path gated as required above.
      Pre-deployment configuration inspection and credential setup are documented
      in [`manual-tests/RELEASE_BAND9_READINESS.md`](manual-tests/RELEASE_BAND9_READINESS.md);
      an inspection report alone does not close live production readiness.
      Console policy/lifecycle/encryption/GuardDuty evidence and the completed
      production probe now verify conditional writes, byte/checksum readback,
      clean tags for archive/document, staging deletion and final preservation.
      Registry startup/flag/legacy-flow confirmation remains separately tracked
      above; this probe does not publish a new-format Registry version.
- [x] Review and accept GuardDuty scan volume/cost before rollout: each release's artifact
      objects and total bytes plus the small `release.json` object are scanned
      under the protected prefix. Prefix alignment is not proof of AWS coverage;
      scanning remains asynchronous and does not block publication.
      Operator confirmed cost is acceptable; no additional quantitative cost
      model is required for this rollout, and no pricing estimate is claimed.

Release Band 9 is complete: production deployments, CLI/status smoke, Registry
migrations and legacy publish/install, CLI-main parity, real dev/production
storage/GuardDuty verification and operator cost acceptance are confirmed.
`RELEASE_PUBLISH_ENABLED=false` remains required. Hosted finalize capacity is
not claimed verified; its before-enabling requirement is preserved as an
unchecked Release Band 11 activation gate below.

## Milestone 14A: Canonical Signing Foundation

> Scope note: build the cryptographic groundwork that every later integrity statement depends on — a typed Rust representation of the existing `agentpm.package.signature.v1` statement, RFC 8785/JCS canonicalization in both Rust and Python with shared cross-language test vectors, the canonical release-manifest content binding, and the release-level digest definition. Introduces no new statement types and changes no verification behavior.
>
> Split from 14B/14C because this is pure, provable foundation: a typed v1 that round-trips byte-identically with today's inline `json!` output, plus a canonicalizer with passing vectors, is shippable on its own and makes the rest of Milestone 14 far less risky. Getting canonicalization wrong after v2 statements exist in production is unrecoverable.
>
> Implementation notes:
>
> - There is no Rust signature statement type today; v1 is an inline `json!` object. Introduce a typed v1 representation first and prove byte compatibility with existing signatures before defining anything new.
> - Legacy author signatures are `agentpm.package.signature.v1`; legacy registry attestations are already `agentpm.registry.attestation.v2`. Historical serialization/verification logic for both must be preserved verbatim — old releases must stay verifiable forever.
> - Do not define a separate AgentPM-specific canonical JSON dialect. RFC 8785/JCS is the contract, and both implementations must agree on the published vectors before anything signs with it.

- [x] Introduce typed Rust representation for legacy `agentpm.package.signature.v1`:
  - [x] preserve camelCase wire fields exactly;
  - [x] replace inline `json!` construction;
  - [x] add byte-for-byte fixture proving serialized bytes match current behavior.
- [x] Introduce corresponding typed/server validation helpers rather than open-coded dict-key checks where practical.
- [x] Define canonical release manifest content binding:
  - [x] kind;
  - [x] namespace/name;
  - [x] version;
  - [x] release format version;
  - [x] manifest digest;
  - [x] Python resolution digest when present;
  - [x] sorted complete artifact inventory;
  - [x] target/digest/size/content type per artifact.
- [x] Define release-level SHA-256 as hash of canonical release-manifest bytes.
- [x] Keep `LockedPackage.integrity` as a bare 64-char lowercase hex value for the release digest.
- [x] **Do not** add selected artifact target/integrity to portable `agent.lock`.
- [x] Implement **RFC 8785/JCS** as the canonical JSON contract for all new release-level cryptographic objects; do not define a separate AgentPM canonical-JSON profile:
  - [x] use RFC 8785 semantics for UTF-8/Unicode, object keys, whitespace, and number serialization;
  - [x] normalize set-like arrays before JCS because JCS does not reorder arrays;
  - [x] sort release artifacts lexicographically by canonical target;
  - [x] sort `PythonResolution.requirements` and `.packages` by the v1 contract;
  - [x] normalize new statement timestamps to UTC RFC3339 `Z` form before canonicalization;
  - [x] avoid floating-point values in release/signature/key-set contracts;
  - [x] use shared Rust/Python RFC 8785 vectors rather than relying on serializer coincidence.
- [x] Implement canonicalizer in Rust.
- [x] Implement canonicalizer in Python/server.
- [x] Add shared cross-language test vectors including:
  - [x] ASCII;
  - [x] non-ASCII Unicode;
  - [x] key-order differences;
  - [x] artifact-order normalization.
- [x] Preserve historical serialization/verification logic for legacy author v1 and registry attestation v2.

Implementation evidence: typed legacy author v1 now replaces the inline publish
statement; its bytes and deterministic Ed25519 signature match the historical
fixture. Shared RFC/Unicode vectors exposed incorrect escaped-key/UTF-16
ordering in `serde_jcs` 0.1.0, corrected by updating to 0.2.0. The existing
release schema and sorted inventory remain authoritative. Shared timestamp and
Python-resolution normalization, release digest mutation tests, portable lock
hex round-trip and required snapshot-parity checks pass locally. No new statement
versions, trust material, verification policy, public workflow, schema/migration
or feature-flag change is included. Local verification is recorded in the test
plan. The operator confirmed independent review is complete with no findings.
The operator also confirmed normal CI, the Registry PostgreSQL job and all
Resolver Contract target jobs passed. The operator confirmed all M14A manual
tests passed, including accepted legacy author signature verification and the
consumer install/run. M14A is complete;
Registry contract parity against CLI `main` remains the Band 10 gate below.

Manual walkthrough: [`manual-tests/SIGNING_M14A_MANUAL_TESTS.md`](manual-tests/SIGNING_M14A_MANUAL_TESTS.md).
It includes shared contract inspection and a checked-in no-dependency Tool for
one signed legacy dev publish/security inspection/install/run. Local runbook
sections and fixture packaging were rehearsed, and the operator subsequently
completed the real signed dev smoke and confirmed full manual acceptance.

## Release Band 10: Canonical Signing Foundation

Covered milestones: 14A.
This gives us a typed Rust representation of the existing `agentpm.package.signature.v1` statement proven byte-identical to today's inline output, RFC 8785/JCS canonicalization in both Rust and Python, shared cross-language test vectors, and the canonical release-manifest content binding. It changes no verification behavior and introduces no new statement types, which is precisely why it is its own band: canonicalization disagreements between the two implementations are unrecoverable once v2 statements exist in production, so this ships and bakes before anything signs with it.

- [ ] After the authoritative CLI contract changes are merged to public CLI
      `main`, run the Registry's `Release Contract Parity` job for the Registry
      revision to be shipped. Require a passing comparison of release/signing
      snapshots and Python contract tests before Registry deployment; record both
      checked commit IDs. This is a Band 10 release gate, not a prerequisite for
      14A manual testing. The Registry PostgreSQL job can run earlier. A manual
      dispatch against a proposed CLI ref does not replace the `main`-ref gate.

## Milestone 14B: Release-Level Signatures and Stored-Byte Integrity
> Scope note: introduce the new release-level statements built on the 14A foundation — `agentpm.package.signature.v2` and `agentpm.registry.attestation.v3` bound to the release digest — plus trustworthy stored-byte verification before finalize, fail-closed attestation, cross-language key-ID normalization, and explicit historical-signer semantics. Server-side only; client verification material is Milestone 14C.
> Implementation notes:
> - New-format signatures attach naturally to `PackageVersion` (the logical release), not to each target artifact.
> - Current server finalize compares publisher-declared SHA against publisher-set S3 metadata, which is publisher-controlled. New release finalize needs a trustworthy stored-byte source.
> - Rust and Python display the same author key with different truncated key IDs (16 vs 12 chars). Match cryptographically by full public key and normalize display separately.
> - A normal signer revocation after publication should not retroactively make a cryptographically valid historical release unsigned. Distinguish revocation from compromise.

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

## Milestone 14C: Registry Trust Anchor and Client Verification
> Scope note: make releases independently verifiable by a consumer. Owns the persistent `registry_signing_keys` model, the root-signed registry key set (`agentpm.registry.keyset.v1` served from `GET /v1/registry/signing-keys`), the attestation-key backfill, the root-key operational prerequisite, the public Security/read DTO evolution, the install verification payload, and client-side verification of release digest, author signature, and registry attestation — including the stricter `--require-signature` / `--require-attestation` semantics and tamper tests.
>
> The server-side trust anchor and the client-side verification are one deliverable, not two: the key set exists only so clients can verify, and client verification cannot ship without it. Shipping either half alone would mean either an unused endpoint or a verifier with no trust root.
>
> Implementation notes:
> - Current install receives only counts/booleans. New verification requires the actual statement/signature/public-key material.
> - Registry attestations cannot be verified today because the registry public key/history is not exposed. The explicit trust/key-history contract is the core of this milestone.
> - Backfill and root-key establishment are sequenced **before** enforcement. Enforcing verification against an unpopulated key set would break every already-published version.

- [ ] Before changing any existing table, enumerate its current constraints, indexes and triggers — not just the ones being added. M13B shipped a correct per-artifact unique index while `V1.55`'s unconditional unique index still enforced the superseded version-scoped rule, and the second artifact's scan insert failed at finalize. `tool_versions` already carries the `ck_tool_versions_release_storage` CHECK and three triggers from 13A, so this milestone is exposed to the same pattern. See the **Schema changes** section of `review-checklist.md`.
- [ ] Add persistent `registry_signing_keys` model/table containing at least:
  - [ ] registry key ID;
  - [ ] algorithm;
  - [ ] full public key;
  - [ ] activation/creation metadata;
  - [ ] retirement metadata/status;
  - [ ] historical keys retained for old attestations.
- [ ] Implement the fixed official-registry trust-anchor design: **root-signed registry key set**.
  - [ ] expose `GET /v1/registry/signing-keys`;
  - [ ] return a versioned key-set payload with type `agentpm.registry.keyset.v1`;
  - [ ] include active and retired/historical attestation signing keys required for old release verification;
  - [ ] canonicalize the key-set payload with RFC 8785/JCS;
  - [ ] sign the key set with a separate registry root Ed25519 key;
  - [ ] ship/pin the official registry root public key (or small versioned root key ring) in the CLI as the bootstrap trust anchor;
  - [ ] verify the root signature before accepting any key from the endpoint;
  - [ ] allow local caching only after successful root verification;
  - [ ] select attestation keys by `registryKeyId` from the verified key set;
  - [ ] reject unknown key IDs and invalid/untrusted key-set signatures;
  - [ ] keep retired signing keys available for historical verification;
  - [ ] treat root-key rotation as a separate rare bootstrap/CLI-update concern; dynamic root rotation is out of Stage 1.
- [ ] **Backfill the existing attestation signing key before enforcing verification.** Every already-published version has a `RegistryAttestation` recording the current `REGISTRY_KEY_ID` (default `apm-prod-1`); if that key ID is absent from the signed key set, `--require-attestation` regresses from passing to failing for the entire back catalogue:
  - [ ] seed the public half of the currently configured attestation key into `registry_signing_keys` as an active key;
  - [ ] include it in the first published key set and every later one;
  - [ ] confirm an **existing** `agentpm.registry.attestation.v2` row verifies end-to-end through the new client verification path, not merely that legacy server-side behavior is unchanged;
  - [ ] state in docs/spec that `--require-attestation` covers legacy single-artifact releases as well as new-format releases.
- [ ] **Establish the registry root key as an operational prerequisite, sequenced before enforcement:**
  - [ ] generate the root Ed25519 keypair as a deliberate documented step before enabling the key-set endpoint;
  - [ ] define and document root private-key custody and how key-set signing is performed; do not reuse the current pattern of a plain env-var read behind a log-and-continue failure path;
  - [ ] respect the bootstrap order — root keypair → seed existing attestation keys → root-signed key set → CLI shipped with pinned root public key → only then enforce `--require-attestation`;
  - [ ] make `GET /v1/registry/signing-keys` **fail closed** when a valid root signature cannot be produced, rather than serving an unsigned or partial key set;
  - [ ] keep the pinned root public key auditable in the repository rather than injected at build time from an opaque source, and treat changing it as requiring a CLI release.
- [ ] Evolve the public Security/read DTO for new-format versions so the web UI can inspect release-level integrity/provenance and target artifacts without using truncated digests as verification inputs; keep legacy responses compatible.
- [ ] Update and verify the existing Tool Security/detail surface for new-format versions using the release/artifact data from Milestones 13A–13B and signing foundation/statements from Milestones 14A–14B, without Package Health scoring:
  - [ ] expose the release-level digest;
  - [ ] expose release-level author-signature/registry-attestation state;
  - [ ] expose the available target-artifact inventory and per-artifact digest/scan state where useful;
  - [ ] verify legacy versions continue rendering their existing single-artifact Security data on the same surface.
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
> **Milestone 11C implements Python environment provisioning; this milestone does not reimplement it.** Here the same provisioning code path is generalized to the target-selected artifact and gains the compatibility failure modes — no compatible artifact, changed target, stale environment. If provisioning needs reworking rather than extending, that is a signal Milestone 11's surface was built too narrowly and should be fixed there.
>
> Implementation notes:
> - The canonical target identifier and current-target detection come from Milestone 11A.
> - Current install `PackageArtifact` contains one URL/digest and no target. Extend the contract for new release metadata without requiring clients to download/presign every target.
> - Detect consumer target before install init and send it to the server; server may return the selected artifact URL plus the full signed release inventory for client verification.
> - Keep `agent.lock` platform-neutral. Selected target/artifact and runtime-environment fingerprint belong in `.agentpm/` local state.
> - Current cache filename omits kind, digest, and target; fix it before multiple target payloads exist.
> - Current extraction silently skips symlink/hardlink entries and has no decompressed-size cap. Harden this while changing the install path.
> - Dependency provisioning adds a new install failure stage; do not finalize the install session as successful until the local runtime environment is ready.

- [ ] Detect canonical current target using the Milestone 11A target identifier/detection mapping.
- [ ] Resolve the Tool's Python interpreter using existing family/minimum-version/`AGENTPM_PYTHON` rules before provisioning dependencies.
- [ ] Extend install-init request with target/runtime context needed for selection.
- [ ] Read/verify new release metadata:
  - [ ] logical release digest;
  - [ ] canonical artifact inventory;
  - [ ] Python resolution;
  - [ ] provenance material from Milestone 14C.
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
- [ ] Define install-side expiry and timeout behavior:
  - [ ] keep the install session valid through dependency provisioning, not only through artifact download (today the session TTL and presigned GET are both 10 minutes, sized for one small tarball);
  - [ ] make a mid-install presigned-GET expiry produce an actionable error, and re-obtainable without restarting the whole install where practical;
  - [ ] set deliberate connect and idle/progress timeouts on the download client, which currently uses `reqwest` defaults with no explicit timeout;
  - [ ] record the chosen session and URL lifetimes in the spec rather than inheriting 10 minutes.
- [ ] Keep extracted Tool directory version-terminal if no correctness issue requires a target segment; only one target payload is active per local install.
- [ ] Store selected target/artifact digest/runtime-environment fingerprint in machine-local `.agentpm/` state if persistence is needed.
- [ ] Harden archive extraction:
  - [ ] reject unsupported symlink/hardlink entries explicitly instead of silently skipping them;
  - [ ] retain traversal/absolute-path protection;
  - [ ] enforce decompressed total-byte cap;
  - [ ] enforce entry-count cap on extraction;
  - [ ] ensure failed extraction cannot leave a seemingly complete install directory.
- [ ] Provision dependencies after payload extraction:
  - [ ] create/reuse Milestone-11C managed Python environment;
  - [ ] replay the stored `PythonResolution` through AgentPM-managed `uv`, hash-verified against the recorded digests, without re-resolving;
  - [ ] keep the wheel-only rule — no source builds — and surface the no-compatible-wheel error for the selected target;
  - [ ] call the Milestone-11A resolver acquisition component; do not re-implement acquisition, PATH discovery, or verification here;
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

## Release Band 11: Release Integrity and Verifiable Target-Aware Installs

Covered milestones: 14B–14C, 15.
This is the band where the new release format becomes real for users. It gives us release-level `agentpm.package.signature.v2` and `agentpm.registry.attestation.v3` statements, trustworthy stored-byte verification before finalize, fail-closed attestation, the root-signed registry key set with its backfill and root-key prerequisite, client-side verification of release digest, author signature, and registry attestation, stricter `--require-signature`/`--require-attestation` semantics, and target-aware installation with runtime provisioning for the selected artifact. The gate from Band 9 lifts here and not before: publishers publish multi-artifact signed releases, and consumers select their target, verify the release cryptographically, and provision the right environment. This is the largest band in the stage and the one that cannot be subdivided — a cut that shipped target selection without verification would mean knowingly installing unverified releases, and a cut that shipped verification without target selection would mean a verifier with nothing installable to verify.

- [ ] Before enabling `RELEASE_PUBLISH_ENABLED` in production, confirm hosted
      finalize disk/request/concurrency capacity for the configured size limits.
      This requirement is carried forward from the gated Band 9 rollout, not
      marked satisfied by its tiny storage probe. Record resource/concurrency
      assumptions and the relevant capacity evidence before activation.

## Milestone 16: Headless Signing and GitHub Actions Publishing
> Scope note: make multi-target releases practical for maintainers by adding secure noninteractive signing plus a first official GitHub Actions workflow that packages target artifacts in parallel and performs one final atomic AgentPM publish. Follow existing AgentPM workflow conventions, but keep the package-build/publish protocol CI-provider-neutral. Stage 1 uses PAT auth for AgentPM registry CI; OIDC trusted publishing remains future work.
> Implementation notes:
> - Current `--sign` always prompts for a passphrase and therefore cannot run headless.
> - Preserve `StoredKeyV1` encrypted key material; do not make raw Ed25519 private-key secrets the preferred CI interface.
> - Existing AgentPM workflows establish tag/version guards, pinned actions/toolchains, `uv`, target matrices, and artifact upload. They do **not** establish atomic fan-in publishing; this milestone must add that.
> - `publish --dry-run` is not a sufficient matrix build contract because it produces human output and skips signing/network behavior. Extract or add a provider-neutral package-build CLI surface.
> - Initial registry authentication can use the existing `AGENTPM_TOKEN`/PAT mechanism.
> - Matrix target vocabulary should match the canonical AgentPM targets from Milestone 11A.

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
  - [ ] machine-readable descriptor for fan-in validation, preserving M12's typed
    payload diagnostics; expose structured build errors on failure and keep
    human progress out of machine stdout.
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
  - [ ] reject mixed `any` and concrete-target claims across matrix outputs
    before automatic publication, including permission-derived differences with
    identical member contents and manifest/resolution digests. Identify every
    affected build/claimed target and contributing typed diagnostic paths/reasons
    where present; `any` can have none. Require author correction/reconciliation
    and rebuild; do not silently drop, select or relabel artifacts. Distinct
    concrete targets remain valid, and local single-artifact publish is unchanged.
  - [ ] Test the permission-derived mixed-claim gate with identical recognized
    text bytes under `0644`/`0755` archive modes, plus actual Unix/Windows
    packaging coverage. Assert no publication on mixed claims, actionable
    diagnostics, and acceptance of otherwise valid distinct concrete targets.
  - [ ] when target validation fails, report affected build targets and their
    M12 diagnostic paths/reasons where present; do not scrape human stderr.
  - [ ] authenticate with `AGENTPM_TOKEN`;
  - [ ] load encrypted signing key/passphrase secrets if signing;
  - [ ] perform one atomic release publish/finalize.
- [ ] Ensure a failed/missing target build prevents final publish rather than creating a partial release.
- [ ] Keep workflow/provider concepts out of core release schema; GitLab/other CI should be able to call the same CLI commands.
- [ ] Verify local single-target publish remains supported.
- [ ] Add end-to-end CI example package/run if feasible.
- [ ] Extend **AgentPM's own** CI matrix beyond Linux, separately from the Tool-publishing workflow:
  - [ ] `agentpm/.github/workflows/ci.yml` currently runs `ubuntu-latest` only while `release.yml` ships macOS and Windows, so platform breakage is first seen at tag time;
  - [ ] add macOS and Windows jobs covering at least target detection, payload classification, archive extraction, and local runtime-environment provisioning;
  - [ ] do not let `#[cfg(unix)]`-gated tests be the only coverage for behavior that also has a Windows path;
  - [ ] if a full matrix per pull request is too slow, run it on merge to main and on release tags rather than omitting it.
- [ ] Explicitly document AgentPM OIDC trusted publishing as future work, not Stage 1.

## Release Band 12: Headless Signing and CI Publishing
Covered milestones: 16.
This gives us headless signing with encrypted AgentPM key material plus a noninteractive secret/passphrase source and reusable GitHub Actions patterns for building and publishing multi-target releases, including AgentPM's own cross-platform CI coverage. It follows Band 11 because there is no point automating the publication of signed multi-artifact releases until that format is consumer-installable and verifiable.

## Milestone 17: SDK Lockfile and Managed Python Tool Compatibility

> Scope note: bring the Python and Node SDKs into alignment with the installed-package and runtime contracts delivered by Milestones 11B–16. Both SDKs currently reject lockfile v4 and launch Python Tools directly, bypassing the CLI-managed dependency environment. Keep this deferred until this milestone; it is not a remaining M12 or Release Band 8 implementation item. Preserve existing public loading/callable APIs and legacy Tool behavior. Do not introduce another resolver, environment manager, lock format or registry protocol.
> Dependencies: consume the finalized lock, local install, provenance and runtime behavior from Milestones 11B/11C and 13–16; implementation must use the exact sub-milestone contracts. Documentation and migration hardening follows as Milestone 18.
> Implementation notes:
>
> - Cover both `agentpm-sdk-python` and `agentpm-sdk-node`: a Node consumer can invoke a Python Tool too.
> - Read supported lockfile v2/v3/v4 shapes without rewriting them. Support v4 package and local-root lanes without treating a local author root as an installed release. Reject unsupported future versions with actionable errors.
> - **Execution decision:** both SDKs will invoke all Tools through `agentpm run --machine` rather than constructing and launching Tool commands themselves: app → SDK → CLI → Tool. A compatible installed CLI becomes a prerequisite for SDK Tool calls, including legacy Python and Node Tools. Keep SDK loading/callable APIs and framework adapters; centralize runtime execution in the CLI.
> - Reuse the CLI's authoritative execution path, including machine-result handling. Do not duplicate Python resolution, environment fingerprinting, target/artifact selection or provenance verification in either SDK.
> - This decision supersedes the earlier standalone/no-CLI legacy-execution requirement in the companion planning files; align those requirements before M17 implementation.
> - Do not fix compatibility by lowering a lockfile version, copying dependencies into the app/global Python, setting an ambient interpreter override or silently falling back to the legacy runner.

- [ ] Audit Python and Node SDK installed-package loaders, Agent/Skill composition, exported types, optional adapters and invocation paths against the finalized CLI contracts; record the supported SDK/CLI version matrix and any narrow CLI machine-output adjustment needed for delegation before implementation.
- [ ] Add read-only lockfile v4 support in both SDKs, retaining existing v2/v3 behavior:
  - [ ] validate the lock version before interpreting the payload; reject malformed and unsupported future versions;
  - [ ] resolve exact installed Agent/Skill/Tool references from the lock rather than selecting another installed version;
  - [ ] consume the portable Python resolution and relevant additive installed-package metadata without changing the fixed contract or moving local environment state into the lock;
  - [ ] tolerate supported additive fields without rewriting/dropping them; do not accept arbitrary future lock versions;
  - [ ] handle local author roots and all supported composition kinds without inventing dependency entries or requiring an app-level `agent.json` for an installed Agent.
- [ ] Replace both SDKs' direct Tool execution logic with delegation to a compatible installed AgentPM CLI via `agentpm run --machine` for Python and Node Tools:
  - [ ] preserve project/lock discovery, exact version, selected installed payload, input JSON and existing output/callable shapes;
  - [ ] use the existing CLI machine interface and unwrap its result deliberately; preserve Tool-returned `ok`/`error` data separately from runner failures;
  - [ ] use the verified managed environment and finalized target/integrity checks; never re-resolve, auto-install, rewrite the lock or rebuild a missing environment during an SDK call;
  - [ ] report missing/old CLI, missing/stale environment, tampered metadata and incompatible target/interpreter with actionable errors; never fall back to an ambient Python;
  - [ ] preserve documented caller/entrypoint environment layering, required variables, timeout/cancellation, stderr logging and secret redaction while retaining managed-runtime `PYTHONPATH`/`PYTHONHOME` isolation;
  - [ ] verify no-dependency, legacy vendored and Node Tools through the same CLI path; document the new CLI prerequisite and actionable missing/outdated-CLI errors rather than retaining a second direct-execution fallback.
- [ ] Preserve Python typing/`py.typed`, Node ESM/CJS/declarations and optional framework adapters; do not introduce a breaking import or result-shape change to obtain runtime compatibility.
- [ ] Add shared representative fixtures for lock v2/v3/v4, future-version rejection and Python Tool resolution; consume the same fixtures from both SDK suites rather than independent divergent literals.
- [ ] Implement and run the SDK-specific cases in `test-plan.md`, including real CLI/managed-native-dependency execution from both Python and Node consumers, hosted Linux/macOS/Windows coverage, legacy regressions and no-model-cost fixtures. Run each repo's native tests, formatter, lint/typecheck/build checks.
- [ ] Update the Python devwork app and a representative Node SDK consumer to exercise published Agent/Tool composition with a real consumer lock; verify actual invocation through the SDK wrapper, not only `agentpm run` separately. Preserve reviewed author state and existing immutable releases.
- [ ] Update SDK READMEs and `agentpm-api/docs` with supported lock/CLI versions, managed-runtime prerequisites, installation/repair steps and verified examples. Record that old SDKs are not compatible merely because the CLI install succeeded.
- [ ] Prepare tested SDK package versions, example dependency floors and matching website SDK status markers. Keep publication/deployment operator-owned; record release/install verification separately from local package/build checks.
- [ ] After required checks pass, operator publishes the updated SDK packages and deploys matching documentation/status markers; verify clean consumers install the published SDK versions and invoke a dependency-bearing Python Tool. Record actual distribution versions and the Python/Node smoke results before closing the release band.

## Release Band 13: SDK Lockfile and Managed Python Tool Compatibility

Covered milestones: 17.
Python and Node consumers can load current installed Agent compositions and call dependency-bearing Python Tools through the same managed runtime used by the CLI. This closes the SDK app gap recorded during Band 8 without delaying the already verified CLI/Registry rollout. Acceptance requires both SDKs, native dependency execution, legacy compatibility, documented version floors and separately confirmed SDK distribution smoke checks.

## Milestone 18: Documentation and Migration Hardening
> Scope note: finish Stage 1 by documenting the hardened discovery/CLI/analytics surfaces and the new Python Tool dependency, runtime-environment, portability, release, integrity, and CI models so authors can use the system without founder guidance. This milestone updates Stage 1-facing docs/examples only; it does not perform the Stage 2 category-language/IA rewrite or expand the feature set beyond earlier milestones.
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
  - [ ] what the stored resolution guarantees: **reproducible, digest-verified dependency resolution across supported targets** — not byte-for-byte identical environments on every platform, since the consumer still selects a target-appropriate distribution from the verified set. Do not describe the `uv` pin as the source of reproducibility;
  - [ ] install is wheel-only and never compiles from source, and what the no-compatible-wheel error means;
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
- [ ] Document the managed resolver for operators and CI, not just Tool authors:
  - [ ] AgentPM acquires its own pinned `uv` on first use and does not use a PATH-installed one;
  - [ ] the cache location, and that it is shared across projects;
  - [ ] the first-use network requirement, and that the download honors standard proxy environment variables;
  - [ ] `AGENTPM_UV_PATH` for airgapped/preprovisioned environments, including that it must report AgentPM's exact pinned version;
  - [ ] which hosts need allowlisting for a Python-dependency install to work;
  - [ ] the supported resolver targets, and that ARM Linux/musl are unsupported because AgentPM's CLI does not ship them.
- [ ] Correct current docs/comments that imply Tool `files` entries are globs if the implementation remains literal file/directory paths.
- [ ] Document missing declared `files` path as a publish error.
- [ ] Document lockfile evolution:
  - [ ] existing v2/v3 locks remain supported;
  - [ ] v4 capability state;
  - [ ] future unsupported lock versions fail rather than being rewritten lossily;
  - [ ] no target-qualified package keys;
  - [ ] local selected artifact is not stored in the portable lock.
- [ ] Document the canonical `PythonResolution` v1 shape and explain that it locks exact distributions/markers but not publisher-selected wheels or local target state.
- [ ] Document release/artifact model:
  - [ ] canonical release type `agentpm.package.release.v1`;
  - [ ] logical Tool version/release;
  - [ ] target artifact inventory;
  - [ ] canonical target identifiers + `any`;
  - [ ] immutable release inventory;
  - [ ] legacy scalar artifact fields remain legacy-only while new-format versions use per-target artifact records.
- [ ] Document S3/release concepts only to the degree useful for advanced publishers; avoid forcing internal storage details into basic author docs.
- [ ] Document install behavior:
  - [ ] exact target then `any`;
  - [ ] managed dependency environment creation;
  - [ ] local environment refresh behavior;
  - [ ] no-compatible-target error;
  - [ ] no silent locked-version substitution.
- [ ] Document integrity/provenance:
  - [ ] RFC 8785/JCS as the new canonicalization contract;
  - [ ] root-signed official-registry key-set trust model and historical signing-key rotation;
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
- [ ] Carry forward Milestone 17's verified Python/Node SDK compatibility matrix and managed Tool examples; do not claim older SDKs support v4 or managed execution.
- [ ] Ensure Stage 2 terminology work remains separate.

## Release Band 14: Documentation and Migration Hardening
Covered milestones: 18.
This gives us Stage 1-facing documentation for the new dependency, release, signing, telemetry, and install surfaces, the migration guidance for authors and consumers moving from single-artifact to multi-artifact releases, updated CLI help text, and refreshed registry/docs examples. It is last because documentation written against surfaces that are still moving has to be rewritten; Stage 2 terminology work stays out of scope.
