# Feature

Stage 1: Immediate Hardening

## Problem / Goal

AgentPM has completed its MMP and now needs to be hardened before meaningful early traffic, direct outreach, and category marketing increase user exposure.

The purpose of Stage 1 is not category creation. It is to make the existing product feel intentional, dependable, and measurable so early users do not hit avoidable friction, confusing failures, unfinished UI, portability surprises, or analytics blind spots.

The working north star for this stage is:

> **Make the existing product feel intentional, dependable, and measurable.**

Stage 1 is a parallel post-MMP stage. It may run alongside Stage 2: Category-Enabling Hardening and Stage 5: Category Marketing & User Learning. When work overlaps, Stage 1 owns functional hardening and correctness; Stage 2 owns category-facing terminology, information architecture, onboarding narrative, and product education.

The positioning/category hypothesis remains a planning constraint:

> **AgentPM makes agents reusable in the way software packages are reusable.**

Stage 1 should not try to prove that hypothesis through messaging changes, but it must not introduce product behavior or architecture that undermines portability, inspectability, reproducibility, or the distinction between complete Agent Packages and their reusable components.

This stage covers five major workstreams:

1. Registry and discovery hardening.
2. Stars, trending, and namespace curation.
3. CLI consistency and lint-error quality.
4. Analytics, billing-funnel measurement, privacy, and feedback.
5. Cross-platform Python Tool packaging, multi-artifact releases, integrity, provenance, and CI publishing.

---

## Non-goals

Stage 1 does **not** attempt to:

- Redesign AgentPM’s category-facing information architecture.
- Rewrite the landing page, docs, registry terminology, READMEs, or onboarding around the final Agent Package category language.
- Perform a blind global rename from `Agent` to `Agent Package`.
- Introduce semantic/embedding/AI-powered search, LLM reranking, AI recommendations, or any other search feature that creates an AI inference cost.
- Build a recommendation engine.
- Build social features beyond GitHub-style stars:
  - no reviews;
  - no ratings;
  - no comments;
  - no follows;
  - no social feed.
- Build a universal package quality score.
- Build a broad evaluation framework.
- Build advanced Package Health UX; Stage 2 owns Package Health presentation.
- Redesign every package-detail page or remove useful kind-specific inspection surfaces.
- Replace authored README content or treat README wording as product-controlled taxonomy.
- Build a generalized CI platform.
- Require GitHub Actions for normal local publishing.
- Require users to adopt `uv` as their project package manager.
- Invalidate or republish legacy Python Tools.
- Retrofitting compatibility metadata onto legacy Python Tool versions.
- Allow adding architectures to an already finalized Tool version.
- Add Rosetta/emulation support as part of this stage.
- Build sophisticated revenue analytics inside AgentPM; Lemon Squeezy remains billing source of truth.
- Enable PostHog session replay, experimentation, NPS, or broad behavioral profiling in Stage 1.
- Build broad structured-data/SEO schema.org work before Stage 2 settles category semantics.

---

## Constraints / Invariants

### 1. Stage boundary

- Stage 1 owns correctness, discoverability foundations, UX consistency, measurement, and portability hardening.
- Stage 2 owns the deliberate category-facing information architecture and terminology migration.
- Stage 1 may identify Stage 2 findings but should not quietly absorb Stage 2 redesign scope.
- Stage 1 must not introduce user-facing terminology or structures that make Stage 2 harder to implement.

### 2. Registry search and discovery

- Search remains deterministic and inexpensive.
- Use PostgreSQL full-text search/trigram/structured filtering. No AI search.
- Text search and filtering are part of one backend query:
  - text defines match/relevance;
  - filters constrain eligibility;
  - filters are not applied client-side after fetching an arbitrary top-N result list.
- Filters must compose with:
  - query;
  - kind;
  - sort;
  - visibility;
  - pagination.
- Active filter/search state must be URL-addressable.
- Query, kind, sort, or filter changes reset pagination to page 1.
- Browser Back/Forward must restore Explore state.
- Within a multi-select filter group, values normally use OR semantics.
- Across filter groups, groups use AND semantics.
- Agent Package `Includes` filters use AND semantics across selected component types.
- Global Explore and namespace-scoped discovery must reuse the same search/filter/sort/pagination/result-card foundation.
- Namespace-scoped discovery adds a hard namespace constraint rather than a parallel search implementation.

### 3. Search relevance

Existing high-level relevance signals remain strongest:

- package/component name;
- namespace;
- top-level description.

Expand search into selected user-meaningful structured semantic metadata at lower weights, including:

- Agent Packages:
  - examples/prompts where useful;
  - dependency/package identities at very low weight;
  - component/binding identities at very low weight.
- Templates:
  - display name;
  - use case;
  - stack;
  - execution surfaces.
- Skills:
  - descriptive compatibility/runtime labels;
  - meaningful authored metadata.
- Knowledge:
  - content type;
  - language;
  - mode;
  - document descriptions.
- Memory:
  - space descriptions;
  - record descriptions;
  - operation descriptions;
  - retrieval modes.
- Instruction Profiles:
  - role;
  - expertise;
  - objectives;
  - audience description.
- Loops:
  - archetype;
  - phase objectives;
  - outcome descriptions.

Dependency/package identity matches must participate in search at very low weight so dependent Agent Packages can be discovered without outranking the directly matching dependency.

Do not index arbitrary full manifests, schema blobs, hashes, paths, generated build metadata, or full README bodies in Stage 1.

Trigram typo/fuzzy behavior should remain focused on names and namespaces rather than broad free-form fields.

### 4. Filters v1

The filter sidebar should become a real faceted-search surface.

Universal filters:

- Kind.
- Namespace via typeahead/search, not an enumerated list.
- License.
- Signed.
- Updated/published recency.

Kind-specific filters:

- Tool:
  - Runtime: Python / Node.
- Agent Package:
  - Includes:
    - Skills;
    - Knowledge;
    - Memory;
    - Profiles;
    - Loop;
    - MCP.
- Template:
  - Stack;
  - Execution Surface.
- Skill:
  - Runtime Compatibility.
- Knowledge:
  - Mode: context / vector.
- Memory:
  - Supports Retrieval:
    - key;
    - filter;
    - chronological;
    - full text;
    - semantic.
- Instruction Profile:
  - no kind-specific v1 filter.
- Loop:
  - no kind-specific v1 filter.

Kind filtering should move into the Filters area so search and filtering feel like one coherent control surface.

Do not expose a filter merely because a field exists in the schema.

### 5. Template execution surface

Add `agentpm-harness` as a first-class Template execution-surface enum.

Relevant Templates should be updated to use it where appropriate.

Harness should not be represented indirectly through `agentpm-run` when Harness is the intended execution surface.

### 6. Pagination

Pagination must use one authoritative page-size contract across frontend and backend.

Known current defects:

- frontend displays `limit=20` but does not send `limit=20`; backend defaults to 30;
- page counts are therefore wrong;
- the initial no-seek state is not recorded in cursor history;
- Page 2 cannot construct a backend Previous cursor to Page 1;
- cursor history is capped at 10, which conflicts with conventional complete Previous/Next navigation;
- private-package behavior differs across sorts;
- strict/relaxed relevance behavior may change totals mid-pagination and must be tested/hardened.

Acceptance invariant:

> Sequential Next/Previous navigation must work across the complete result set in both directions without losing query, kind, sort, filter, visibility, or cursor state.

### 7. Trending

Trending signal computation must cover the full eligible result set.

The current `trending_tools` top-12-per-kind truncation may remain useful for presentation, but it must not constrain Explore sorting or pagination.

Trending v1 should remain understandable and deterministic.

Recommended ranking priority:

1. installs in the last 7 days;
2. total stars;
3. all-time installs;
4. latest publish/update timestamp;
5. stable ID/slug tie-breaker.

Do not use alphabetical order as the meaningful fallback when all recent signals are zero.

Do not collapse unlike time horizons into a misleading opaque score unless a simple, documented formula is justified.

### 8. Stars

Stars are identity-level, not version-level.

Star contract:

- authenticated users only;
- one star per user per package/component identity;
- clicking again removes the star;
- public star count;
- star relation stores `created_ts`;
- users may star their own artifacts;
- private artifacts can be starred only by authorized users;
- private star activity/counts must not leak private package existence into public discovery.

UI:

- show passive star count on Explore and namespace result cards;
- package/component detail pages should expose starring as a prominent identity-level action;
- preferred detail treatment is an integrated footer rail at the bottom of the main identity card;
- starred state should change star fill and subtly change the rail state;
- avoid gamification such as confetti;
- remove the Tool-only `Score & rating / Coming Soon` placeholder;
- ratings are not part of Stage 1.

### 9. Package popularity signals

The current repeated `0/week` and `▼ -100%` treatments are poor signals for an early ecosystem.

Replace weak rolling-percentage treatment with more durable signals such as:

- stars;
- total installs.

Recent activity can continue to power Trending without being the dominant visible card/detail metric.

Do not manufacture activity or hide real data to imply traction.

### 10. Namespace pinned packages

Pinned items are namespace-owner/admin curation.

Rules:

- pins attach to identity, not version;
- only namespace Owner/Admin can manage pins;
- only artifacts owned by that namespace can be pinned;
- ordering is explicit;
- keep the maximum modest; default implementation target: 6 pins;
- normal visitors see no empty pinned section when there are no pins;
- Owners/Admins see an empty-management state when there are no pins;
- once pins exist, everyone with visibility to the namespace can see them.

### 11. Namespace discovery

Namespace package lists must gain:

- search within namespace;
- kind filtering;
- applicable universal filters;
- applicable kind-specific filters;
- sorts;
- corrected pagination.

Reuse the Explore foundation rather than creating separate search logic.

### 12. Explore states

Loading text:

- all/mixed search: `Searching AgentPM…`
- kind-specific: `Searching Tools…`, `Searching Skills…`, `Searching Agent Packages…`, etc.

No-results state should:

- clearly say no results were found;
- preserve editable query text;
- offer `Clear filters` when filters are active;
- offer `Search all kinds` when a kind is restricting the result;
- avoid Stage 1 AI-generation prompts.

Errors must render a visible recoverable error state, not silently appear as zero results.

### 13. Package detail shell

Shared Stage 1 changes should be implemented in the common detail shell where possible:

- star rail;
- improved popularity signals;
- remove Tool-only Score & Rating placeholder;
- hide/remove Tool Evaluations tab while it only says `Coming Soon`.

Preserve useful kind-specific surfaces:

- Agent Package:
  - dependency/composition view;
  - Bindings;
  - Examples;
  - README;
  - Security.
- Template:
  - Bootstrap;
  - Next Steps;
  - use case;
  - stack;
  - execution surfaces;
  - entrypoints.
- Knowledge:
  - Install;
  - Inspect/Query;
  - corpus/retrieval/provenance metadata.
- Memory:
  - Lifecycle;
  - Record Contracts;
  - retrieval/governance metadata.
- Profile:
  - identity/objectives/audience/communication/boundaries/compatibility.
- Loop:
  - graph summary;
  - Phases.
- Skill:
  - entrypoint;
  - references;
  - scripts;
  - compatibility.

README content is author-owned and should not be treated as AgentPM-controlled category copy.

### 14. Technical SEO

Stage 1 owns technical SEO/indexing correctness, not category-facing copy strategy.

Required technical SEO primitives:

- unique page titles;
- unique meta descriptions;
- canonical URLs;
- OpenGraph/Twitter metadata;
- sensible share previews;
- sitemap coverage;
- robots/indexing policy;
- correct indexing of public pages;
- private/authenticated surfaces not indexed;
- 404/error pages not indexed;
- search/filter result URLs such as arbitrary `/explore?q=...` combinations should default to `noindex,follow`;
- canonicalize Explore to `/explore` unless a deliberate indexable facet is introduced later;
- docs-version canonical behavior must be intentional;
- package-version canonical behavior must be intentional;
- basic heading hierarchy and alt/accessibility metadata should be checked.

Immutable package-version pages should normally canonicalize to themselves.

Stage 2 owns the category-facing wording of titles/descriptions/headings.

### 15. CLI output contract

Human CLI output should answer, in order:

1. what happened;
2. what object/path was affected;
3. why it failed when applicable;
4. what the user can do next.

Machine-readable output formats should remain stable and should not be forced into human presentation conventions.

Known priorities:

- every `agentpm init --kind ...` scaffold should lint successfully immediately;
- Tool scaffold must stop generating invalid `files`, `entrypoint.command`, and `entrypoint.args`;
- semantic/domain lint messages should appear before generic schema failures;
- suppress parent `oneOf`/`anyOf` noise when a more specific semantic error already covers the same subtree;
- deduplicate equivalent `/properties/<kind>` and `/dependentSchemas/<kind>` failures;
- never inline huge containing objects in human lint output;
- for closed unions, prefer actionable expected-values messaging;
- centralize friendly transport errors across `install`, `export`, `new`, `whoami`, and similar commands;
- `whoami` must exit non-zero on connection failure;
- normalize inspect output shape across Knowledge and Memory;
- normalize displayed paths and remove `/./`;
- reuse one lint renderer across `lint` and publish validation;
- `new` should report created target/path clearly;
- `serve --mcp` requirement should be clap-level argument validation rather than runtime messaging;
- `keys export` should report `no local key with id ...` rather than duplicated filesystem errors.

Headless key/signing support is required by the cross-platform CI work even though it originated as a CLI usability gap.

### 16. Analytics

Use PostHog as the initial analytics layer.

The purpose is learning, not comprehensive growth instrumentation.

Prefer server-side events whenever AgentPM already knows the authoritative outcome.

Initial event vocabulary should include, at minimum:

Web/registry:
- `landing_viewed`
- `docs_viewed`
- `explore_searched`
- `package_viewed`
- `namespace_viewed`
- `signup_started`
- `account_created`
- `pricing_viewed`
- `checkout_started`
- `package_starred`
- `feedback_submitted`

Authoritative server/backend events:
- `package_install_completed`
- `publish_completed`
- `subscription_started`
- `subscription_updated`
- `subscription_cancelled`
- `subscription_payment_failed` if supported cleanly by existing Lemon Squeezy webhook handling.

Local-only CLI/Harness milestones:
- `cli_first_used`
- `harness_started`
- `harness_completed`

Lemon Squeezy remains billing source of truth.

Clicking checkout is intent; webhook events are authoritative billing truth.

### 17. CLI telemetry privacy contract

Minimal anonymous CLI telemetry is enabled by default with clear disclosure and easy opt-out.

Provide:

- environment opt-out such as `AGENTPM_TELEMETRY=0`;
- preferably a persistent config setting as well.

Use a strict allowlist.

Allowed examples:

- event name;
- CLI version;
- OS family;
- CPU architecture;
- anonymous install ID;
- limited runtime mode metadata.

Never collect:

- prompts;
- Tool inputs/outputs;
- Harness conversations;
- Tool arguments/results;
- file contents;
- filesystem paths;
- environment variables;
- secrets;
- package contents;
- private package identities.

AgentPM should enforce this structurally rather than relying only on policy text.

Update the Privacy Policy as part of the implementation.

Do not enable PostHog session replay in Stage 1.

### 18. Feedback

Use PostHog survey capabilities as the backend/reporting layer while keeping AgentPM-controlled UI and placement.

Initial feedback experience should be context-first, not NPS-first.

Recommended prompts:

- What were you trying to do?
- What got in the way or surprised you?
- Email (optional).
- Checkbox: user is open to a follow-up conversation.

Provide a persistent Feedback entry from site/footer/docs/registry chrome.

Reproducible bugs may branch to GitHub Issues.

Do not require GitHub Issues as the only feedback path.

### 19. Python Tool dependency model

This affects Python Tools only.

Legacy Python Tools remain valid and continue using legacy install semantics.

For new-format Python Tools:

- authors declare dependency intent in `agent.json`;
- AgentPM resolves exact dependency versions;
- AgentPM records resolved dependency state in `agent.lock`;
- dependency artifacts are resolved for the consumer target at install time;
- do not vendor the publisher's local resolved environment into the Tool package by default.

Add an optional Python dependency array only when `runtime.type == "python"`.

Recommended shape:

```json
"runtime": {
  "type": "python",
  "version": "3.11",
  "dependencies": [
    "openai>=1.51.0",
    "pydantic>=2.8,<3"
  ]
}
```

Lint must enforce:

- dependencies only allowed for Python runtime;
- requirement syntax is valid;
- duplicate/conflicting declarations are rejected or normalized deterministically.

AgentPM owns dependency resolution.

`uv` may be the implementation mechanism but should not be a requirement on the user's project tooling.

Preferred UX:

- AgentPM manages/bundles/acquires a known compatible `uv`;
- users do not need to maintain a `uv` project or `uv.lock`.

Do not make `requirements.txt`, `pyproject.toml`, Poetry lock, uv lock, etc. alternate authoritative dependency sources in Stage 1.

`agent.json` is author intent.

`agent.lock` is AgentPM-resolved state.

### 20. Lockfile evolution

Reuse `agent.lock`; do not create a user-managed `agent.python.lock`.

Introduce an additive lockfile evolution, likely `lockfile_version: 4`.

Requirements:

- v3 remains readable with legacy semantics;
- v4 supports optional Python dependency resolution metadata;
- new CLI supports both;
- older CLI encountering an unsupported newer lockfile must fail clearly rather than silently misread it.

The workspace lock may contain Tool-specific resolved Python dependency state.

Published Tool releases must carry the relevant Tool-specific resolved dependency snapshot independently of the publisher's broader workspace.

### 21. Python payload portability

Dependency portability and Tool payload portability are distinct.

Declared Python dependencies are installed for the target and therefore do not make the Tool payload architecture-specific.

Tool payload itself may be:

- portable `any`;
- platform/architecture-specific.

Local publish should classify conservatively.

Examples:

- pure `.py`, JSON, text, etc. may be classified `any`;
- `.so`, `.dylib`, `.pyd`, native executables, or platform-specific vendored binary contents imply target-specific payload;
- unknown binary payload should default to conservative target-specific classification.

Do not forbid `_vendor`, but new-format Python Tools that both declare dependencies and vendor dependency trees should at least warn because they may reintroduce architecture coupling.

### 22. Published version / artifact model

One logical Python Tool version may contain multiple immutable runtime artifacts.

Example:

```text
@foo/tool@1.2.0
  ├── macos-arm64
  ├── macos-x86_64
  └── linux-x86_64
```

All artifacts in one version must share:

- Tool identity;
- Tool version;
- manifest contract;
- resolved Python dependency state.

Only target-specific payload/build metadata may differ.

Adding support for another target requires a new semantic version.

Published version artifact/compatibility set is immutable after finalize.

### 23. S3 / release layout

A multi-target Tool version is one logical release containing multiple S3 objects, not one giant tarball.

Conceptual storage:

```text
tools/<tool-id>/<version>/
  release.json
  artifacts/
    any/tool.tar.gz
    macos-arm64/tool.tar.gz
    macos-x86_64/tool.tar.gz
    linux-x86_64/tool.tar.gz
```

Exact key names may follow current storage conventions, but the data model must preserve:

- release-level metadata;
- per-artifact object identity;
- per-artifact target;
- per-artifact SHA-256;
- per-artifact size;
- immutable artifact inventory.

Artifacts should remain independently inspectable/self-describing.

Each artifact should contain the logical manifest and resolved Python dependency metadata required to install/inspect that artifact.

### 24. Atomic publishing

Multi-target publish is atomic at the release level.

Recommended flow:

1. each build runner produces a target artifact;
2. final CI/local publish job collects all target artifacts;
3. AgentPM uploads all artifacts into staging/unpublished state;
4. validate manifest/dependency-state equivalence;
5. verify target uniqueness and hashes;
6. complete signing/scanning/attestation requirements;
7. finalize once;
8. only then does the Tool version become visible/installable.

If any artifact upload or validation fails, the version must not become partially published.

### 25. GitHub Actions v1

The first official CI integration only needs to solve:

> Build and publish a Python Tool across selected target runners.

The underlying CLI/protocol must remain CI-provider-neutral.

Typical initial target matrix:

- macOS arm64;
- macOS x86_64;
- Linux x86_64.

Linux arm64 may be added if practical, but it is not required to prove the architecture.

Matrix jobs build package artifacts.

A final job downloads the artifacts and performs one atomic publish.

GitHub Actions must be a thin composition of normal AgentPM CLI capabilities, not a separate proprietary publishing path.

Local publish remains supported.

### 26. Install-time artifact resolution

For new-format Tool versions:

1. determine consumer OS;
2. determine CPU architecture;
3. determine Python/runtime compatibility;
4. load published artifact inventory;
5. select exact compatible target artifact where available;
6. otherwise use `any` if compatible;
7. install locked Python dependency versions using target-compatible distributions;
8. verify selected artifact integrity before use.

If no compatible artifact exists:

- fail before extraction/runtime execution;
- report current target;
- report available targets.

Interactive installs may offer **supported** recovery paths such as:

- build/install from a supported source/portable fallback;
- choose a compatible newer version when the user is not pinned;
- explain how to upgrade when the exact locked version is incompatible.

Never silently substitute:

- an incompatible artifact;
- a different locked version.

Headless/non-interactive installs fail deterministically.

Rosetta/emulation fallback is out of scope.

### 27. Legacy Python Tool compatibility

Legacy Python Tool versions:

- remain valid;
- may contain vendored dependencies;
- retain old artifact/install semantics;
- do not gain false portability claims;
- may display compatibility as unknown/unspecified later;
- do not require republishing.

Do not mutate old release metadata to pretend legacy compatibility is known.

### 28. Release integrity model

Current single-artifact integrity and signing remain valid for legacy packages.

New-format multi-artifact Tool releases introduce a canonical release manifest that binds:

- identity;
- version;
- manifest digest;
- resolved Python dependency-lock digest;
- complete artifact inventory;
- every artifact target;
- every artifact SHA-256.

Compute a release digest over canonical serialization of this release manifest.

For new releases:

- `agent.lock` top-level package `integrity` becomes release-level integrity;
- lock state may additionally record the selected target artifact and artifact-level integrity.

Example:

```json
"tool:@foo/native-tool@1.2.0": {
  "kind": "tool",
  "name": "@foo/native-tool",
  "version": "1.2.0",
  "integrity": "sha256:<release-digest>",
  "artifact": {
    "target": "macos-arm64",
    "integrity": "sha256:<artifact-digest>"
  }
}
```

### 29. Canonical signing format

Do not carry forward today's implicit Rust/Python JSON serialization agreement.

Define a formal canonical serialization contract for new release-level signed statements.

At minimum:

- UTF-8;
- deterministic object-key ordering;
- no insignificant whitespace;
- defined Unicode behavior;
- deterministic artifact ordering by target tuple;
- normalized timestamp representation if timestamps participate;
- no floating-point ambiguity.

Create cross-language Rust/Python test vectors.

### 30. Author signatures and registry attestations

Legacy statement versions remain valid.

New multi-artifact releases should use new release-level author-signature and registry-attestation statement versions.

The author signature should cryptographically bind the complete release manifest/release digest rather than one target tarball.

The registry attestation should attest to the complete immutable release.

Per-artifact SHA-256 values remain separately verified.

Avoid requiring one independent author signature per target unless there is a strong technical need.

### 31. Client-side provenance verification

Install must continue cryptographically verifying downloaded artifact digests.

Stage 1 should also add actual client-side verification for release provenance rather than trusting only registry booleans.

Requirements:

- verify release-level author signature when present;
- verify registry attestation cryptographically when required/requested;
- add a `--require-signature`-style enforcement path alongside attestation requirements;
- preserve namespace signing policy semantics;
- do not claim signature verification merely because the registry reported a boolean.

### 32. Server-side finalize integrity

Preferably strengthen finalize so the registry independently verifies uploaded artifact bytes against declared artifact digests before finalizing the release.

Do not rely only on client-declared expected digest plus client-supplied S3 metadata.

Finalized release should bind:

- declared artifact digest;
- actual stored-byte digest;
- release-manifest artifact digest reference.

### 33. Headless signing

Headless signing is required for CI publishing.

Provide at least one supported secure noninteractive path.

The exact mechanism may be:

- encrypted local key plus passphrase supplied via secret/env/file;
- CI-specific key material;
- another documented noninteractive credential path.

Do not make interactive passphrase entry mandatory in GitHub Actions.

OIDC/keyless signing may be explored later but is not required for Stage 1.

---

## Acceptance criteria

### Registry / Explore

- Search pagination correctly handles 0, 1, 2, 3+, and >10 pages.
- Page count matches actual backend page size.
- Next/Previous works:
  - 1 → 2 → 3;
  - 3 → 2 → 1.
- Browser Back/Forward restores full Explore state.
- Changing query/kind/sort/filter resets to page 1.
- Private accessible results behave consistently across supported sorts.
- Mixed namespace/package result pagination remains stable.
- Strict/relaxed relevance does not produce misleading page counts or invalid continuation behavior.
- Filters sidebar replaces `Coming soon`.
- Filter state is URL-addressable.
- Namespace filter uses searchable typeahead.
- Kind-specific filters appear only when relevant.
- Search expansion returns lower-weight semantic/dependency matches without outranking direct identity matches.
- Fixed relevance fixture set passes.
- All/mixed loading state says `Searching AgentPM…`.
- Kind-specific loading text is correct.
- No-result state provides clear recovery actions.
- Search/filter errors produce visible recoverable UI.
- Trending can rank the complete result set and no longer depends on top-12 materialized-view truncation.
- Trending does not degrade to alphabetical order as its meaningful fallback.
- Explore cards show star count and improved popularity signals.
- Tool Evaluations tab and Score & Rating placeholder no longer expose empty unfinished UI.

### Stars

- Authenticated user can star/unstar a public artifact.
- Star count updates correctly.
- One star per user per identity.
- Star is version-independent.
- `created_ts` is stored.
- Public star count appears on result cards and detail pages.
- Private star activity does not leak private artifact existence.
- Detail-page star rail has distinct starred/unstarred persistent states.

### Namespaces

- Owner/Admin can pin eligible namespace-owned artifacts.
- Pin ordering is explicit.
- Maximum pin count is enforced.
- Visitors see no empty pin section.
- Owner/Admin sees empty-state management prompt when no pins exist.
- Namespace package list supports search/filter/sort/pagination using shared discovery infrastructure.

### Package detail / Template metadata

- Shared weekly-change signal is removed/replaced.
- `agentpm-harness` is a valid Template execution surface.
- Relevant Templates can publish with that surface.
- Existing specialized detail tabs remain functional.

### Technical SEO

- Major public routes have deliberate unique metadata.
- Package-version canonical behavior is explicit and tested.
- Docs-version canonical behavior is explicit and tested.
- Public package/namespace pages are indexable.
- Private/auth pages and arbitrary Explore query/filter pages are not indexable.
- Sitemap and robots behavior match intended boundaries.
- OG/Twitter metadata renders sensible previews.

### CLI

- `agentpm init` for all eight kinds produces lint-valid manifests.
- Tool init no longer fails its immediate lint.
- Lint suppresses redundant giant `oneOf`/`anyOf` noise when covered by better semantic errors.
- Duplicate schema paths are deduplicated.
- Human lint output never dumps huge containing objects.
- Closed union failures identify expected alternatives where practical.
- Most actionable lint errors appear before generic schema fallback.
- `publish --dry-run` uses consistent lint rendering.
- `install`, `export`, `new`, `whoami` share friendly transport-failure formatting.
- `whoami` exits non-zero on connection failure.
- Knowledge/Memory inspect use a consistent structural output pattern.
- Displayed paths are normalized.
- `new` success clearly reports created target/path.
- `serve` missing `--mcp` is handled by clap/argument validation.
- `keys export` invalid-id error is concise and actionable.

### Analytics / feedback

- PostHog integrated.
- Initial event taxonomy implemented.
- Authoritative server-side events used where available.
- Lemon Squeezy webhook events populate billing conversion events.
- Early Product Funnel dashboard exists.
- CLI telemetry follows allowlist policy.
- CLI telemetry can be disabled.
- Privacy Policy updated.
- Feedback UI is AgentPM-branded and backed by PostHog survey capability.
- Feedback can capture:
  - goal;
  - obstacle/surprise;
  - optional email;
  - follow-up consent.
- GitHub Issues remains available for reproducible bug reports.
- No session replay required.

### Python dependency portability

- New Python Tool manifests may declare dependencies.
- Non-Python Tools cannot use Python dependency declarations.
- `agent.lock` v4 can store resolved Python dependency state.
- Legacy v3 lockfiles continue to work.
- Legacy Python Tools continue to install with legacy semantics.
- New Python Tool publish resolves exact dependency versions reproducibly.
- New Python Tool install resolves target-compatible Python distributions rather than consuming publisher-vendored dependencies.
- AgentPM controls the dependency resolver implementation and does not require the user's project to use `uv`.

### Multi-artifact release

- One Tool version can contain multiple target artifacts.
- Local publish can still publish one compatible artifact.
- Multi-target CI can publish one atomic release.
- All target artifacts must agree on identity, version, manifest, and dependency lock.
- Release does not become visible until all required artifacts validate and finalize.
- S3 stores per-target artifact objects plus release metadata.
- Adding a target requires a new Tool version.
- Install deterministically selects exact target or `any`.
- No compatible artifact produces actionable error before runtime.
- Interactive recovery never silently changes a locked version or selects an incompatible artifact.

### Integrity / signing

- New release manifest canonically binds all target artifact hashes.
- Release-level integrity is computed deterministically.
- Lockfile can record release integrity plus selected artifact integrity.
- New author-signature statement binds the release.
- New registry attestation binds the release.
- Legacy signature/attestation formats remain supported.
- Rust and Python canonicalization pass shared test vectors.
- Installer verifies selected artifact digest.
- Installer can cryptographically verify release-level author signature.
- Installer can cryptographically verify registry attestation when requested/required.
- Headless CI signing path exists.
- Preferably server finalization independently verifies uploaded artifact bytes before release finalization.

---

## Risks / edge cases

### Search / discovery

- Expanding FTS fields can make broad searches noisy.
- Dependency matches can overwhelm direct matches if weighted too strongly.
- Strict/relaxed result-set switching can make total counts unstable across pagination.
- Mixed namespace/package cursor pagination is complex; preserve architecture unless a rewrite is justified.
- Filter combinatorics can lead to hard-to-maintain ad hoc query branching if not implemented as a coherent faceted-query layer.
- Moving Kind into Filters must not reduce discoverability or break deep links.
- Namespace typeahead must avoid expensive unbounded lookups.

### Stars / trending

- New ecosystem activity is sparse, so ranking signals may tie frequently.
- Star counts are popularity signals, not quality.
- Do not let stars become a package-health proxy.
- Trending must remain deterministic under low activity.
- Private stars/counts require careful visibility enforcement.

### CLI

- Over-suppressing schema errors could hide useful structural diagnostics.
- Semantic-error precedence should suppress only redundant parent noise.
- Machine-readable lint formats must not regress.
- Friendly error wrappers should preserve enough low-level detail for debugging, potentially under verbose/debug mode.
- Fixing Tool scaffold must preserve intended minimalism without accidentally creating a large framework template.

### Analytics/privacy

- Anonymous IDs can become privacy-sensitive if combined too aggressively.
- Avoid accidental telemetry of package names/private identities from generic event payloads.
- Billing events must not duplicate or contradict Lemon Squeezy source of truth.
- Feedback forms may collect personal information; privacy policy/storage handling must cover it.
- PostHog defaults/autocapture must not unintentionally collect more than the Stage 1 contract allows.

### Python packaging

- Python requirement resolution must remain reproducible while selecting platform-compatible artifacts.
- Exact version locking can still fail later if upstream distributions disappear; define/install-error behavior clearly.
- Pure-Python detection is not perfect; conservative classification is safer than false portability claims.
- Vendored binary payloads can be difficult to classify reliably.
- Tool code may shell out to native system dependencies that are not visible from Python dependency metadata.
- `any` classification must not be used when runtime behavior is actually OS-specific.
- User-authored native files need target-specific builds even if Python dependencies are portable.
- Lockfile v4 migration must not break older workspaces.

### Multi-artifact release

- Partial uploads must never become visible as finalized releases.
- Final publish job must detect matrix jobs that built from different commits/manifests/locks.
- Duplicate target artifacts must fail clearly.
- One target may produce an artifact while another fails; release stays unpublished.
- S3 orphan/staging cleanup is needed for failed publishes.
- Artifact inventory and DB/S3 records must not diverge.
- Release manifest must have deterministic artifact ordering.

### Integrity/signing

- New canonical serialization is security-sensitive.
- Cross-language canonicalization drift would invalidate signatures.
- Release-level signing must bind all artifact hashes and relevant metadata.
- Registry-attestation migration must not confuse legacy and new statement versions.
- Client-side signature verification needs a trustworthy way to obtain signer/registry public keys and active-policy context.
- Signer revocation semantics for historical releases must be defined:
  - whether a signature remains historically valid but signer is now revoked;
  - whether current-policy checks differ from cryptographic validity.
- Registry independent hashing may add finalize latency/cost.
- CI secret handling for signing keys must be documented carefully.

---

## Open questions

These questions should be resolved during implementation planning if not already answerable from existing repo patterns:

1. What exact internal representation should the faceted filter query use so new Stage 2 filters can be added without branching explosion?
2. Should the namespace pin maximum be exactly 6 or configurable later?
3. What exact updated-recency filter buckets should v1 expose?
4. Should star counts appear beside total installs everywhere or vary by surface?
5. What exact PostHog SDK/server integration pattern best matches the existing frontend/backend architecture?
6. What persistent CLI config mechanism already exists, if any, for telemetry opt-out?
7. Should `cli_first_used` be emitted only once per anonymous install ID or once per CLI version?
8. Which Lemon Squeezy webhook event names currently map cleanly to subscription started/updated/cancelled/payment failed?
9. Does the CLI already bundle/download external binaries in a pattern that can be reused for AgentPM-managed `uv`?
10. What exact Python requirements parser/resolver API should AgentPM use?
11. Should resolved Python dependency metadata be stored directly in `agent.lock` entries or in a referenced lock subsection keyed by Tool identity?
12. What exact release-manifest JSON schema/version name should be used?
13. What exact S3 key convention should be adopted while preserving existing legacy object layout?
14. How should staging/orphaned multi-artifact uploads be garbage-collected?
15. Should release metadata be stored as a physical `release.json` S3 object, normalized DB rows, or both? Current recommendation: both logical registry state and self-describing release metadata.
16. What exact canonical JSON format/spec should be adopted for signed release statements?
17. What exact new statement version names should replace/augment `agentpm.package.signature.v1` and `agentpm.registry.attestation.v2`?
18. What exact historical signer-revocation semantics should install-time verification use?
19. What secure headless signing input should be supported first:
    - env passphrase for encrypted local key;
    - key file + secret;
    - direct private key secret;
    - another existing repo-compatible mechanism?
20. Can server-side hashing be implemented efficiently at finalize without downloading large objects through the web process?
21. Which target set should the initial official GitHub Actions example guarantee:
    - macOS arm64;
    - macOS x86_64;
    - Linux x86_64;
    - optionally Linux arm64?
22. Should `agentpm package` become a new explicit CLI command, or is there an existing build/publish artifact-creation path that should be made reusable by CI?
23. How should interactive install offer a compatible newer version when the requested spec is unversioned versus explicitly versioned versus lockfile-pinned?
24. What exact compatibility metadata should Stage 2 Package Health read from the new release model?

---

## Related Specs

- Stage 2: Category-Enabling Hardening — separate handoff/spec.
- Future Package Health spec.
- Future AgentPM Developer / category onboarding spec.
- Future advanced eval/quality framework spec.
- Future import/migration spec.
- Future additional CI providers / release automation spec.
