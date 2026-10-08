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
- Build AgentPM OIDC/trusted-publisher authentication in Stage 1; CI v1 may use `AGENTPM_TOKEN`, while OIDC remains a future hardening path.
- Implement dynamic N-of-M namespace signature thresholds; Stage 1 preserves the current effective policy that `required` means at least one valid trusted author signature.
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

New-format multi-artifact Tool versions must remain inspectable in the existing detail/Security experience. Stage 1 should expose release-level integrity/provenance and the artifact target inventory without turning this into the broader Stage 2 Package Health redesign.

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

Legacy Python Tools remain valid and continue using legacy install/runtime semantics.

For new-format Python Tools:

- authors declare dependency intent in `agent.json`;
- AgentPM resolves and records exact Python distribution versions in a portable resolution model;
- AgentPM records that resolution in `agent.lock`;
- the published Tool release carries the Tool-specific resolution independently of the publisher's broader workspace;
- the consumer installs the locked dependency set for its own target/interpreter;
- the published Tool payload does not contain the publisher's local Python environment.

Add an optional dependency array only when `runtime.type == "python"`:

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

- `dependencies` is allowed only for Python runtime;
- each entry is valid Python requirement syntax;
- duplicates/conflicts are rejected or normalized deterministically;
- Node Tools cannot declare Python dependencies.

`runtime.version` remains a minimum interpreter version, consistent with current runner behavior.

AgentPM owns Python dependency resolution.

`uv` is the preferred implementation mechanism, but is an AgentPM implementation dependency rather than a user-project requirement. Users do not need to use `uv`, maintain a `uv.lock`, or convert their repository into a uv project.

Do not make `requirements.txt`, `pyproject.toml`, Poetry lockfiles, uv lockfiles, or similar project files alternate authoritative dependency sources in Stage 1. A future import command may populate the AgentPM manifest from those sources.

The resolved state must be portable across supported platforms. Do not lock the publisher's selected wheel filename or publisher architecture. The resolution may contain environment markers/conditional dependency branches where a transitive dependency differs by platform or Python version, but every selected package version must be exact.

#### Canonical `PythonResolution` v1 contract

Stage 1 uses one explicit portable resolution object everywhere the resolved Python dependency state crosses a contract boundary: `agent.lock`, resolve/install DTOs, publish descriptors, artifact metadata, and release-manifest digesting.

```json
{
  "type": "agentpm.python-resolution.v1",
  "python": {
    "requires": ">=3.11"
  },
  "requirements": [
    "openai>=1.51.0",
    "pydantic>=2.8,<3"
  ],
  "packages": [
    {
      "name": "annotated-types",
      "version": "0.7.0"
    },
    {
      "name": "colorama",
      "version": "0.4.6",
      "marker": "sys_platform == 'win32'"
    },
    {
      "name": "openai",
      "version": "1.51.2"
    },
    {
      "name": "pydantic",
      "version": "2.9.2"
    }
  ]
}
```

Contract rules:

- `type` is exactly `agentpm.python-resolution.v1`.
- `python.requires` records the normalized minimum/constraint implied by the Tool runtime contract; it is not a publisher interpreter path or machine fingerprint.
- `requirements` records the normalized authored root requirement strings that were resolved.
- `packages` is the complete exact resolved distribution set needed by those roots.
- Each package entry contains normalized distribution `name`, exact `version`, and optional normalized PEP 508 `marker`.
- The same normalized distribution name may appear more than once only when entries are distinguished by mutually exclusive marker conditions needed for portable resolution.
- Distribution names use one deterministic normalization (PEP 503-style lowercase/hyphen normalization is preferred).
- `requirements` and `packages` must be emitted in deterministic order; package ordering is by normalized `(name, marker-or-empty, version)`.
- Do not record wheel filenames, wheel tags, local paths, publisher architecture, selected target, interpreter path, or a local environment identifier.
- Stage 1 does not require preserving a full parent/child dependency graph in this object; the exact resolved set plus markers is the portable lock contract.
- The serialized/digested form is deterministic. When `PythonResolution` participates in the release manifest, its digest is SHA-256 over its RFC 8785/JCS canonical JSON bytes.

`agent.json` is author intent.

`agent.lock` is AgentPM-resolved workspace state.

### 20. AgentPM-managed Python runtime environment

Current AgentPM execution has no venv, no dependency installation, and no `PYTHONPATH` integration. New-format Python Tools therefore require an AgentPM-owned runtime environment.

Use an AgentPM-managed per-Tool environment under `.agentpm/`, separate from the immutable extracted Tool payload. The exact directory naming may follow existing path helpers, but the environment identity must include:

- Tool identity;
- Tool version;
- consumer target;
- resolved Python interpreter major/minor or equivalent ABI-relevant interpreter identity.

A conceptual shape is:

```text
.agentpm/
  tools/<namespace>/<name>/<version>/...
  runtimes/python/<namespace>/<name>/<version>/<target>/py3.11/...
```

For new-format Python Tools with declared dependencies:

1. resolve the Python interpreter using the same family/version rules as `agentpm run`, honoring `AGENTPM_PYTHON`;
2. create/reuse an AgentPM-managed Python environment with that interpreter;
3. use AgentPM-managed `uv` to install the exact applicable locked dependency versions into that environment;
4. execute the Tool with the managed environment's Python interpreter.

A venv-style managed environment is preferred over modifying the Tool payload or installing into the user's global interpreter.

Legacy Tools continue using today's PATH/`AGENTPM_PYTHON` execution behavior and author-managed `_vendor` code.

Existing `_vendor` imports must continue to work even when a new-format Tool also has an AgentPM-managed environment. A new-format Tool that both declares dependencies and vendors dependency content should receive a portability warning.

Dependency provisioning should happen during `agentpm install` so a completed install is runnable. `agentpm run` must validate that the managed environment still matches the selected interpreter/target; if the operator changes `AGENTPM_PYTHON` or the environment becomes stale, AgentPM must either rebuild deterministically or fail with an actionable reinstall/refresh instruction rather than running against an incompatible environment.

### 21. Lockfile evolution

Reuse `agent.lock`; do not create a user-managed `agent.python.lock`.

The current lock implementation has an important forward-compatibility hazard: on-disk `lockfile_version` is not used to dispatch deserialization, unknown fields are silently ignored, and an older CLI can therefore read a future lock and later rewrite it without newer fields.

Stage 1 must add an explicit lockfile compatibility guard before normal deserialization/mutation.

Requirements:

- existing V1/V2-shape locks and current on-disk versions 2/3 remain readable;
- new Python resolution/release-integrity state requires a minimum lockfile version of 4;
- the existing V2-shaped Rust struct may remain the structural representation if the change is additive; a new enum arm is not required solely because the number becomes 4;
- replace the current `requires_v3_lock` concept with a `minimum_lock_version(...)`-style calculation capable of returning 2, 3, or 4 from content;
- a CLI must reject a lock whose `lockfile_version` is newer than the maximum version it understands before it can rewrite the file;
- unknown future fields must never be silently discarded through a read-modify-write cycle;
- `Lock::empty_v2()` and Harness/preflight version checks must be audited so they do not accidentally accept unsupported future locks merely because the version is `>= 2`.

Keep one logical lock entry per AgentPM package key:

```text
tool:@namespace/name@1.2.3
```

Do not add target to the package key and do not create one locked package entry per architecture.

For new-format releases, `LockedPackage.integrity` represents the logical release integrity. The selected local target artifact is machine-local state and must **not** be written into the portable/source-controlled `agent.lock`.

Add optional typed Python resolution state to the locked Tool package entry (or a referenced typed subsection keyed by the same logical Tool identity).

Plain `agentpm install` currently regenerates the lock from the resolve plan. Therefore the registry resolve/install contract must carry the new Python resolution state so regeneration cannot silently erase it.

Published Tool releases must carry the relevant Tool-specific resolution independently of the publisher workspace lock.

Reconcile the duplicated package-kind enumerations while this work touches both. `PackageKind` exists twice today — seven variants in the CLI's lockfile/semver types and eight variants in the SDK install DTOs, where only the latter has `Template`. They are kept in agreement by hand, and this stage modifies both the lock entry types and the resolve/install DTOs, which is exactly when a silent divergence becomes expensive.

Either share one definition across both crates, or keep two definitions with an explicit total-coverage conversion and a test that fails when one side gains a variant the other lacks. Do not leave the drift to manual review.

### 22. Python payload portability and target identity

Dependency portability and Tool payload portability are distinct.

Declared Python dependencies are installed for the consumer target and therefore do not make the published Tool payload architecture-specific.

The Tool payload itself may be:

- portable `any`;
- target-specific.

Use a canonical AgentPM target identifier that includes OS, CPU architecture, and relevant ABI/libc distinction. Prefer Rust-compatible target-triple vocabulary where it matches AgentPM's existing release conventions, for example:

- `aarch64-apple-darwin`;
- `x86_64-apple-darwin`;
- `x86_64-unknown-linux-gnu`;
- `x86_64-pc-windows-msvc`;
- special value `any`.

The initial official CI matrix does not need to cover every supported target, but the release schema must not hardcode only the initial three.

Local packaging must classify conservatively.

Examples:

- pure `.py`, JSON, text, and similar source/data payloads may be `any`;
- `.so`, `.dylib`, `.pyd`, `.dll`, native executables, native libraries, or known platform-specific vendored binary contents imply target-specific payload;
- unknown executable/binary payload defaults to the current detected target rather than `any`.

Declared dependencies do not affect payload classification.

Do not unblock `.whl` as a normal embedded Tool payload merely to support dependency resolution. Wheels should be acquired by AgentPM/uv at consumer install time rather than bundled into the release payload.

Current Tool packaging treats `manifest.files` entries as literal paths and only warns if a declared path does not exist. Stage 1 should harden this: a declared `files` path that is missing at package/publish time is a fatal packaging error rather than a silently omitted artifact input.

### 23. Published version / artifact model

One logical new-format Python Tool version may contain one or more immutable runtime artifacts.

Example:

```text
@foo/tool@1.2.0
  ├── any
  ├── aarch64-apple-darwin
  ├── x86_64-apple-darwin
  └── x86_64-unknown-linux-gnu
```

All artifacts in one version must share:

- Tool identity;
- Tool version;
- logical manifest digest;
- resolved Python dependency-resolution digest.

Only target-specific payload/build metadata may differ.

Adding support for another target requires a new semantic version.

Published version artifact/compatibility inventory is immutable after finalize.

The registry data model should make the release/artifact relationship explicit:

- `PackageVersion` remains the logical immutable release/version;
- new child `PackageVersionArtifact` records own target-specific object metadata;
- legacy scalar `sha_256`, `size_bytes`, and `s3_key` semantics remain supported for legacy single-artifact rows;
- new release-format rows are distinguished explicitly rather than inferred from nullable data.

The Stage 1 persistence decision is:

- add an explicit release-format field on `PackageVersion`; legacy rows remain legacy/null and new rows use `agentpm.package.release.v1`;
- add a nullable release-level SHA-256 field (for example `release_sha_256`) on `PackageVersion`;
- migrate legacy scalar artifact columns `sha_256`, `size_bytes`, and `s3_key` to nullable at the database/schema level;
- existing legacy rows keep those three scalar values populated and they remain authoritative for the legacy single-artifact read path;
- new `agentpm.package.release.v1` rows set the legacy scalar artifact columns to `NULL`;
- new-format artifact bytes/size/object identity are authoritative only through `PackageVersionArtifact`;
- new-format logical release integrity is authoritative through the release-level digest field;
- no read path may choose legacy-vs-new behavior merely because a scalar happens to be null; branch on the explicit release format.

Old CLI versions must not be allowed to misinterpret a new release manifest as a legacy tarball. When a client cannot consume the release format, fail clearly with a "requires newer AgentPM" style error.

### 24. Release sessions, S3 layout, and atomic publishing

The existing `Upload` row is a one-object reservation and cannot represent concurrent target artifacts for the same package version. Preserve it for legacy publishing.

Introduce a release-level publish reservation with child artifact uploads for the new format.

The Stage 1 model/table names are fixed as:

- SQLAlchemy model `PackagePublishRelease` → table `package_publish_releases`;
- SQLAlchemy model `PackagePublishArtifact` → table `package_publish_artifacts`;
- SQLAlchemy model `PackageVersionArtifact` → table `package_version_artifacts`.

Do not reuse the legacy `uploads` table as the primary multi-artifact abstraction. It remains the compatibility path for legacy single-artifact publishing.

Conceptually:

```text
PackagePublishRelease
  release_id
  package_id
  version
  manifest
  python_resolution
  status
  publisher
  expires_at

PackagePublishArtifact
  artifact_id
  release_id
  target
  expected_sha256
  expected_size
  tmp_key
  final_key
  status
```

The canonical immutable release document has type **`agentpm.package.release.v1`** and this logical shape:

```json
{
  "type": "agentpm.package.release.v1",
  "kind": "tool",
  "name": "@namespace/tool",
  "version": "1.2.0",
  "manifestDigest": "sha256:<64-lowercase-hex>",
  "pythonResolutionDigest": "sha256:<64-lowercase-hex>",
  "artifacts": [
    {
      "target": "aarch64-apple-darwin",
      "digest": "sha256:<64-lowercase-hex>",
      "size": 12345,
      "contentType": "application/gzip",
      "format": "tar.gz"
    }
  ]
}
```

Release-manifest contract rules:

- `pythonResolutionDigest` is omitted when the release has no Python resolution.
- `artifacts` is the complete immutable installable artifact inventory and is sorted lexicographically by canonical `target` before canonicalization/digesting.
- Targets are unique within one release.
- `digest` values bind concrete target bytes; `manifestDigest` binds the logical `agent.json`; `pythonResolutionDigest` binds the canonical `PythonResolution` object.
- The release manifest contains no upload-session IDs, S3 temporary keys, local file paths, publisher machine identity, or mutable scan state.
- The release manifest is canonicalized with RFC 8785/JCS and the release integrity is SHA-256 of those canonical bytes.

A new-format publish should use one release init/finalize transaction:

1. create or resume the release reservation;
2. register the complete intended target inventory;
3. obtain one presigned PUT per artifact;
4. upload each artifact independently;
5. validate all child uploads;
6. verify manifest/resolution equivalence and target uniqueness;
7. construct/persist the canonical release manifest;
8. complete author-signature and registry-attestation requirements;
9. atomically finalize the logical `PackageVersion` and all `PackageVersionArtifact` rows;
10. only then expose the version to install resolution.

A partial artifact upload must never create a partially installable release.

Retries/resume should operate at the release + artifact level, not collide because one `(package_id, version)` pending `Upload` already exists.

Failed/expired release sessions need server-side cleanup of staged S3 objects.

Use separate immutable S3 objects per target, not one giant archive.

Conceptual new-format storage:

```text
packages/<namespace>/<name>/<version>/
  release.json
  artifacts/
    any/<full-sha256>.tar.gz
    aarch64-apple-darwin/<full-sha256>.tar.gz
    x86_64-apple-darwin/<full-sha256>.tar.gz
    x86_64-unknown-linux-gnu/<full-sha256>.tar.gz
```

Staging may use:

```text
uploads/<release-id>/<artifact-id>.tar.gz
```

Use full digests in immutable keys when the digest is part of object identity; do not make the current 12-character prefix the load-bearing multi-artifact identifier.

Keep legacy S3 keys readable.

Store release metadata both as normalized registry state and as an immutable, self-describing `release.json` object.

Each target artifact remains independently inspectable and contains at least the logical `agent.json` plus the Tool-specific dependency-resolution metadata required to inspect/reconstruct its contract.

New publish uploads should be streamed rather than reading the complete artifact into RAM. Reconcile the current client 3 GB cap and server 1 GB default into one server-authoritative effective upload policy.

Define an explicit expiry and retry policy for release reservations. Current single-artifact publishing presigns a PUT for 900 seconds and assumes one client uploads one object immediately. A multi-artifact release inverts both assumptions: artifacts may be produced by separate CI matrix jobs that finish minutes apart, and a slow or retried job must not invalidate the release.

Requirements:

- the release reservation lifetime must be independent of, and longer than, any individual artifact's presigned PUT window;
- a client must be able to request a fresh presigned PUT for a still-pending child artifact without invalidating the release or sibling artifacts already uploaded;
- expiry must be reported to the client in a form it can act on, so a CI job can fail with "release reservation expired, re-run the workflow" rather than an opaque S3 or 409 error;
- resuming must remain possible for the whole window, keyed at release + artifact level rather than colliding on one pending `(package_id, version)` row;
- choose concrete values deliberately rather than inheriting 900 seconds, and state them in the spec once chosen.

Reconcile publish rate limits with CI usage. `publish_init` and `publish_finalize` currently carry `10/minute; 100/hour` at `cost=3` per call. A multi-artifact release performs more registry calls than a single-artifact publish, and a matrix workflow may retry individual jobs. Audit the effective call count for a worst-case supported release, including presign re-issues and retries, and ensure a legitimate CI publish cannot exhaust the limit. If the new flow needs different limits or a distinct cost for release-scoped calls, set them here rather than discovering the ceiling in CI.

Audit client HTTP timeouts against streaming uploads. The publish client currently applies a single 600-second total timeout that also covers the S3 PUT, because the same `reqwest` client is reused. Streaming large artifacts under one release makes that ceiling load-bearing. Separate registry-API timeouts from object-transfer timeouts, and prefer per-transfer progress/idle timeouts over one total-duration cap for artifact bytes.

Two pre-existing defects in the current publish path should be corrected while this code is being restructured, and must not be carried into the new release flow:

- finalize currently resolves whether the destination object already exists with a conditional whose branches are both `False`, so any S3 error on that HEAD — including `AccessDenied` — is treated as "object absent" and triggers a copy. Genuine not-found must be distinguished from an error that should fail finalize.
- `publish_init` returns a `resumed` flag that the client's typed response has no field for, so resume state is silently discarded and a caller cannot distinguish a fresh reservation from a resumed one. Either surface it or remove it; the release-level flow must make resume state explicitly visible to the client rather than leaving an ignored wire field.

### 25. Malware scanning for multi-artifact releases

Preserve the existing asynchronous GuardDuty malware-scan model rather than blocking release finalization until scanning completes.

For new-format releases:

- enqueue one scan per concrete target artifact;
- associate scan results with the target artifact (direct artifact FK preferred over relying only on raw S3 key);
- the version may retain queued/running scan state after publish, consistent with current behavior;
- if any artifact is found infected, the logical Tool version must be yanked/disabled consistently, because one version represents one immutable artifact set;
- unscanned/failed/unknown artifacts must never be displayed as clean.

Do not treat SBOM/Grype columns that happen to exist in the current schema as implemented Stage 1 vulnerability scanning unless this work explicitly wires them into the flow.

### 26. GitHub Actions v1 and package-build CLI

The first official CI integration only needs to solve:

> Build and publish a Python Tool across selected target runners.

The underlying CLI/protocol must remain CI-provider-neutral.

Introduce or extract a reusable package-build CLI surface suitable for CI. An explicit `agentpm package` command is preferred if no existing command can provide a stable machine-readable artifact + descriptor output. `publish --dry-run` is not sufficient by itself because it skips signing/network stages and currently only prints a human artifact path.

The build command should produce:

- target artifact;
- canonical target identifier;
- artifact SHA-256 and size;
- manifest digest;
- Python resolution digest;
- machine-readable descriptor used by the final publish job.

The initial official GitHub Actions workflow should follow existing AgentPM repo conventions:

- tag-triggered `v*`;
- fail if Git tag does not match `agent.json.version`;
- pinned toolchain/setup actions;
- `astral-sh/setup-uv@v6` for Python work where appropriate;
- minimal job permissions;
- matrix jobs build/package only;
- `actions/upload-artifact` transports build outputs;
- one final fan-in job uses `actions/download-artifact`, validates all descriptors, and performs one atomic AgentPM publish.

Initial required matrix:

- `aarch64-apple-darwin`;
- `x86_64-apple-darwin`;
- `x86_64-unknown-linux-gnu`.

Linux arm64 and Windows may be added later or opportunistically, but must not block proving the model.

CI v1 may authenticate to AgentPM with `AGENTPM_TOKEN`.

OIDC/trusted publishing for the AgentPM registry is explicitly deferred, although existing SDK release workflows establish it as a desirable future direction.

Local `agentpm publish` remains supported and may internally package the current target or `any` as appropriate.

Extend AgentPM's own CI to cover the platforms this stage reasons about. `agentpm/.github/workflows/ci.yml` currently runs on `ubuntu-latest` only, while `release.yml` builds and ships macOS and Windows binaries — so platform-specific breakage is first observed at tag time. This stage introduces current-target detection, payload classification by native-binary signature, and per-target artifact handling: code whose whole purpose is behaving differently per platform, and which Linux-only CI cannot meaningfully exercise.

Requirements:

- add macOS and Windows jobs to the AgentPM CLI CI matrix, at minimum running the test suites that cover target detection, payload classification, archive extraction, and local runtime-environment provisioning;
- the existing `#[cfg(unix)]`-gated tests must not be the only coverage for behavior that also has a Windows path;
- if full cross-platform CI is too slow for every pull request, run the platform matrix on merge to main and on release tags rather than omitting it.

### 27. Install-time artifact resolution and local state

For new-format Tool versions:

1. resolve the logical AgentPM package version and release integrity;
2. detect the consumer target;
3. resolve the applicable Python interpreter for Python Tools;
4. send target/runtime context to install initialization;
5. obtain the canonical release manifest plus only the selected artifact's presigned URL (and available-target metadata for diagnostics);
6. select/validate exact target first, otherwise compatible `any`;
7. verify the release manifest/release digest;
8. verify the selected artifact belongs to that signed release;
9. stream-download and recompute the selected artifact SHA-256;
10. extract safely;
11. create/reuse the AgentPM-managed Python environment and install the applicable locked dependency versions;
12. finalize the install session.

Do not presign/download every target artifact for a consumer that needs only one.

The portable `agent.lock` records the logical release integrity and Python resolution, not the machine-selected artifact.

If AgentPM persists the selected artifact target/digest or runtime environment fingerprint, store it in machine-local `.agentpm/` state.

Cache identity must include enough information to prevent collisions across kind, package, version, target, and artifact digest. The current `<owner>-<name>-<version>.tgz` naming is insufficient.

The extracted Tool path may remain version-terminal because one consumer installs one selected payload for that version; target/interpreter-specific dependency environments belong in separate machine-local runtime state.

If no compatible artifact exists:

- fail before runtime execution;
- report current target;
- report available targets;
- never silently install an incompatible artifact.

Interactive installs may offer a compatible newer version only when the request is not exact/lockfile-pinned and the user explicitly accepts it.

A locked exact version is never silently changed.

Headless/non-interactive install fails deterministically.

Rosetta/emulation fallback is out of scope.

While modifying extraction, harden archive behavior:

- symlinks/hardlinks that cannot be safely supported should fail explicitly rather than being silently skipped;
- enforce decompressed-size and entry-count limits;
- validate downloaded byte count against expected size where practical;
- keep atomic `.part` download behavior and cache digest re-verification.

Audit install-side expiry and timeouts alongside the download changes. The presigned GET is currently 10 minutes and the `InstallSession` TTL matches it, which was sized for one modest tarball. Dependency provisioning now extends the install beyond download, and selected artifacts may be larger.

Requirements:

- the install session must remain valid through dependency provisioning, not only through artifact download;
- a presigned GET that expires mid-install must produce an actionable error, and ideally be re-obtainable without restarting the whole install;
- the download client currently uses `reqwest` defaults with no explicit timeout; set deliberate connect and idle/progress timeouts rather than leaving transfers unbounded;
- state the chosen session and URL lifetimes explicitly rather than inheriting 10 minutes.

### 28. Legacy Python Tool compatibility

Legacy Python Tool versions:

- remain valid;
- may contain unpacked vendored dependencies;
- retain existing single-artifact install semantics;
- retain current PATH/`AGENTPM_PYTHON` runtime behavior;
- do not receive invented portability claims;
- may display compatibility as unknown/legacy later;
- do not require republishing.

Do not mutate historical release metadata to pretend legacy compatibility is known.

### 29. Release integrity model

Current single-artifact integrity/signing remains valid for legacy releases.

New-format releases introduce an immutable canonical release manifest binding:

- kind;
- namespace/name;
- version;
- manifest digest;
- Python resolution digest when present;
- release format version;
- complete artifact inventory;
- every artifact target;
- every artifact SHA-256;
- every artifact size/content type needed by the install contract.

Compute the release digest as SHA-256 over the canonical release-manifest bytes.

For new releases, `LockedPackage.integrity` stores the 64-character lowercase release SHA-256. Preserve the current lockfile hex convention rather than mixing `sha256:<hex>` into the existing field.

Do **not** record the locally selected target/artifact digest in `agent.lock`.

Artifact integrity is read from the verified release manifest and may be stored in local AgentPM install/cache state.

### 30. Canonical signing format

Do not carry forward today's implicit agreement between Rust `serde_json::to_vec(Value)` and Python `json.dumps(sort_keys=True)` as the new signing contract.

For all new release-level cryptographic objects, Stage 1 adopts **RFC 8785 JSON Canonicalization Scheme (JCS)** directly. Do not define a second AgentPM-specific canonical JSON profile.

Contract rules:

- canonicalize JSON with RFC 8785/JCS in both Rust and Python;
- UTF-8/Unicode, object-key ordering, whitespace elimination, and number serialization follow RFC 8785;
- JCS does not reorder arrays, so the schema must normalize set-like arrays before canonicalization:
  - release `artifacts` sorted lexicographically by canonical target;
  - `PythonResolution.requirements` and `.packages` sorted by their contract rules;
- new signature/attestation timestamps, when present, are normalized to UTC RFC3339 with `Z` before canonicalization;
- avoid floating-point values in signed/release contracts;
- canonicalization libraries must be covered by shared cross-language vectors rather than assumed equivalent.

Create shared Rust/Python RFC 8785 vectors including non-ASCII values and at least one published AgentPM release/signature fixture.

Legacy v1 author-signature and v2 registry-attestation verification must retain their historical serialization behavior exactly.

Before adding new statement versions, introduce typed Rust representations for the existing v1 author statement and assert byte-for-byte compatibility with today's JSON output. New statements should be typed/versioned rather than inline `json!` dictionaries.

### 31. Author signatures and registry attestations

Statement version numbers are independent by type.

Preserve:

- `agentpm.package.signature.v1` for legacy single-artifact author signatures;
- `agentpm.registry.attestation.v2` for legacy registry attestations.

Introduce distinct new release-level versions, expected to be:

- `agentpm.package.signature.v2`;
- `agentpm.registry.attestation.v3`.

The new author statement binds the logical release digest, not one target artifact.

The new registry attestation binds the same immutable release identity/digest plus registry provenance facts.

Per-artifact integrity remains bound indirectly through the release manifest.

Signatures/attestations should continue attaching to `PackageVersion`, which naturally represents the logical release; do not create one signature per target artifact.

If the user explicitly requested `--sign` and the submitted signature is rejected, publishing must fail with an actionable signature-validation error rather than silently succeeding unsigned merely because namespace mode is optional.

For new-format releases, registry-attestation creation is part of successful finalize. A registry signing/configuration failure should leave the release unpublished rather than silently producing an unattested new-format release.

Stage 1 preserves current effective namespace threshold semantics:

- `off`;
- `optional`;
- `required` = at least one valid trusted author signature.

Dynamic N-of-M thresholds are deferred.

Normalize the Rust/Python display key-ID inconsistency, but do not use truncated key IDs as cryptographic identity. Verification matches full `public_key_b64`.

### 32. Client-side provenance verification and trust material

Install must continue recomputing downloaded artifact digests.

For new-format releases, install must also cryptographically verify provenance rather than trusting counts/booleans from the registry.

Extend the install/init contract so the client receives, in one coherent verification payload:

- canonical release manifest or equivalent bytes/data;
- expected release digest;
- author signature material:
  - signature;
  - statement;
  - full public key;
  - signer metadata/status;
- namespace signing mode/effective threshold;
- registry attestation statement/signature;
- registry key ID and trustworthy public-key lookup/reference information.

Do not make install depend on the current UI-oriented Security DTO whose displayed SHA-256 is truncated.

Add `--require-signature`:

- if specified, require at least one cryptographically valid trusted author signature even when namespace policy is optional/off;
- when namespace signing mode is `required`, client verification must enforce the effective current threshold (one) as part of verification.

Change `--require-attestation` to mean cryptographically verified registry attestation, not `registry_attested: true`.

Registry attestation verification requires a trust anchor.

For the official AgentPM registry, Stage 1 uses a **root-signed registry key-set**:

- persist attestation signing keys in a `registry_signing_keys` table/model containing at least:
  - `key_id`;
  - `algo`;
  - `public_key_b64`;
  - activation/creation timestamp;
  - retirement timestamp/status;
- expose `GET /v1/registry/signing-keys`;
- the response is a versioned key-set document (type `agentpm.registry.keyset.v1`) containing active and historical attestation-verification keys needed for existing releases;
- canonicalize the key-set payload with RFC 8785/JCS and sign it with a separate AgentPM registry **root** Ed25519 key;
- ship/pin the official registry root public key (or small versioned root key ring) with the CLI as the bootstrap trust anchor;
- the CLI verifies the root signature on the key set before trusting any attestation signing key from it;
- cache a successfully verified key set locally with normal refresh/expiry behavior, but never use an unverified cached or network key set;
- attestation `registryKeyId` selects a key from the verified key set;
- retired signing keys remain in the signed key set so historical attestations remain verifiable;
- an unknown key ID or invalid/untrusted key-set signature fails attestation verification;
- rotation of the routine attestation signing key must not require pinning that signing key directly in the CLI;
- root-key rotation is a separate rare bootstrap event and may require shipping an updated CLI/root-key ring; dynamic root rotation is out of Stage 1.

This avoids trusting a public key merely because it arrived next to the signature while still allowing routine registry signing-key rotation and historical verification.

**Existing attestation keys must be migrated into the key set before verification is enforced.** Every version published to date carries a registry attestation recording the current `REGISTRY_KEY_ID` (default `apm-prod-1`). Because `--require-attestation` now means cryptographically verified, and verification selects a key from the root-verified key set by `registryKeyId`, an attestation whose key ID is absent from that key set becomes unverifiable.

Requirements:

- seed the public half of the currently configured attestation signing key into `registry_signing_keys` as an active key, so historical `agentpm.registry.attestation.v2` rows remain verifiable;
- include it in the signed key set from the first published key set onward;
- the scope of `--require-attestation` must be stated explicitly: it applies to legacy single-artifact releases as well as new-format releases, which is why the backfill is mandatory rather than optional. If a future decision narrows it to new-format releases only, that must be written here rather than inferred;
- treat "an existing published version fails `--require-attestation` after this work lands" as a regression, not expected behavior — the flag passes for those versions today.

**The registry root key is a new operational dependency, not only a code change.** Requirements:

- generate the root Ed25519 keypair as a deliberate, documented act before the key-set endpoint is enabled;
- the root private key must have a stated custody posture — where it lives, who or what can use it, and how key-set signing is performed. It must not simply inherit the current attestation key's pattern of a plain environment variable read behind a failure path that logs and continues;
- bootstrap order is load-bearing and must be respected: generate the root keypair → seed existing attestation keys into `registry_signing_keys` → produce a root-signed key set → ship a CLI with the root public key pinned → only then may `--require-attestation` enforce cryptographic verification;
- the key-set endpoint fails closed. If a valid root signature cannot be produced, the endpoint must return an error rather than an unsigned or partially signed key set. The current attestation path's fail-open behavior must not be carried over to the trust anchor;
- the root public key shipped in the CLI is release-engineering state: changing it requires a CLI release, and the pinned value must be auditable in the repository rather than injected at build time from an opaque source.

Treat truncated registry/author key IDs as display values only.

Historical author signer revocation semantics:

- cryptographic validity of a historical signature is independent from current signer activation state;
- a release signature accepted while the signer was authorized at publish remains historically valid after ordinary signer revocation/rotation;
- the client/UI may additionally report that the signer is now revoked;
- retroactive compromise invalidation policy is out of scope for Stage 1.

### 33. Server-side stored-byte integrity

Strengthen new-format release finalization so the registry obtains a trustworthy checksum of the bytes actually stored in S3 before finalizing the release.

Do not rely only on:

- publisher-declared `expected_sha`;
- publisher-set `x-amz-meta-sha256`.

Use an S3/storage-layer checksum mechanism where practical or independently stream/hash the staged object server-side/background-side.

For every finalized new artifact:

```text
declared artifact digest
=
trusted stored-byte digest
=
release-manifest artifact digest
```

Size must likewise match the S3 object `ContentLength`/trusted stored size.

A mismatch leaves the release unpublished.

### 34. Headless signing

Headless signing is required for CI publishing.

Preserve the existing encrypted `StoredKeyV1` key-envelope model.

Provide a secure noninteractive path centered on:

- encrypted AgentPM key file/key record;
- explicit key selection (`--key-file` and/or existing `--key-id`);
- passphrase from a secret source such as `AGENTPM_KEY_PASSPHRASE`.

The preferred CI path should not require storing raw Ed25519 private-key bytes as a GitHub secret.

CI documentation may instruct users to store the encrypted key JSON as a secret or secure file, materialize it on the ephemeral runner, and provide its passphrase separately.

Never print passphrases, decrypted private key material, or full secret paths in normal output.

OIDC/keyless AgentPM registry publishing is deferred.

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
- New-format Tool Security/detail data can represent release-level digest/signature/attestation state plus the immutable target-artifact inventory without breaking legacy single-artifact versions.

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
- AgentPM produces portable exact Python resolution state rather than locking publisher-specific wheels.
- New dependency-bearing Tool locks use minimum lockfile version 4.
- Current v2/v3 lockfiles remain readable.
- Unsupported future lockfile versions fail before lossy read/rewrite.
- Plain `agentpm install` regeneration preserves Python resolution state because the resolve contract carries it.
- `agent.lock` remains target-independent:
  - logical release integrity is locked;
  - selected local target/artifact is not written into the portable lock.
- AgentPM creates/reuses an isolated per-Tool Python runtime environment keyed by target and interpreter identity.
- New Python Tool install provisions the exact applicable locked dependency versions into the AgentPM-managed environment.
- `agentpm run` executes new-format dependency-bearing Tools through the managed Python environment.
- Legacy Python Tools continue to install/run with legacy vendored/PATH semantics.
- AgentPM controls the resolver implementation and does not require the user's project to use `uv`.

### Multi-artifact release

- One Tool version can contain multiple immutable target artifacts.
- Canonical target IDs use the AgentPM target model, including `any`.
- Local publish can still publish one `any` or current-target artifact.
- New-format publish uses a release reservation with child artifact uploads rather than one legacy `Upload` row per target.
- Multi-target CI can publish one atomic release.
- All target artifacts agree on identity, version, manifest digest, and Python resolution digest.
- Release does not become visible until all required artifact uploads validate and one release-level finalize succeeds.
- Registry persists one logical `PackageVersion` plus per-target artifact records.
- S3 stores a canonical `release.json` plus separate immutable target objects.
- New-format uploads stream rather than buffering the full artifact in RAM.
- Client/server upload-size policy is consistent and server-authoritative.
- Missing declared Tool `files` paths fail packaging rather than silently omitting content.
- Adding a target requires a new Tool version.
- Legacy single-artifact release/storage behavior remains supported.
- Each new target artifact receives malware scanning; infection of any target yanks/disables the logical version.

### Integrity / signing

- New release manifest canonically binds all target artifact hashes and Python resolution digest.
- Release-level integrity is deterministic.
- New lock entries store release integrity only, not local selected-artifact state.
- Existing author-signature v1 serialization/verification remains byte-compatible.
- New author-signature v2 binds the release.
- Existing registry-attestation v2 remains valid.
- New registry-attestation v3 binds the release.
- Rust and Python canonicalization pass shared vectors including Unicode.
- New-format finalize obtains a trustworthy stored-byte digest before release publication.
- Installer recomputes selected artifact digest and checks it against the verified release manifest.
- Installer cryptographically verifies author signatures when present/required.
- `--require-signature` enforces at least one valid trusted signature.
- `--require-attestation` verifies registry attestation cryptographically rather than trusting a boolean.
- Registry public-key history/trust material supports verification of current and historical attestations.
- Historical signatures remain cryptographically valid after normal signer revocation when they were accepted while authorized; current revoked status may still be surfaced.
- Stage 1 retains the effective one-signature namespace threshold; dynamic N-of-M policy is deferred.
- Explicit `--sign` cannot silently degrade to an unsigned successful publish if signature validation fails.
- New-format publish fails closed if registry attestation cannot be created.
- Headless CI signing works with encrypted AgentPM key material and a noninteractive secret passphrase source.

### Target-aware installation

- Install selects exact target when available, otherwise compatible `any`.
- Only the selected artifact receives a presigned URL/download.
- No compatible artifact produces an actionable error before runtime.
- Selected target/artifact state remains machine-local.
- Cache identity cannot collide across kind/package/version/target/artifact digest.
- Exact lockfile-pinned versions are never silently changed.
- Interactive recovery requires explicit consent.
- Headless install fails deterministically.
- Download size is validated where practical.
- Unsafe symlink/hardlink archive entries do not silently disappear; extraction fails explicitly if unsupported.
- Decompressed-size and entry-count limits protect extraction.
- Legacy install behavior remains intact.

### CI publishing

- Provider-neutral CLI package-build output contains target, artifact digest/size, manifest digest, and Python resolution digest.
- Official GitHub Actions workflow checks tag version against `agent.json.version`.
- Matrix jobs build/package and upload CI artifacts only.
- A final fan-in job downloads every target artifact and performs one atomic publish.
- CI v1 supports required macOS arm64, macOS x86_64, and Linux x86_64 targets.
- CI v1 can authenticate with `AGENTPM_TOKEN`.
- OIDC/trusted AgentPM publishing is not required in Stage 1.

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

- A portable dependency lock must represent conditional/transitive differences across targets without collapsing to the publisher environment.
- Exact version locking can still fail later if upstream distributions disappear; install errors must distinguish unavailable distribution from AgentPM compatibility failure.
- AgentPM-managed Python environments must be keyed strongly enough that Python ABI/interpreter changes cannot reuse incompatible native packages.
- Changing `AGENTPM_PYTHON` after install may invalidate the local managed environment and must trigger deterministic rebuild/error behavior.
- Pure-Python payload detection is not perfect; conservative classification is safer than false portability claims.
- Tool code may shell out to native system dependencies that are not visible from Python dependency metadata.
- `any` must not be used when runtime behavior is genuinely OS/ABI-specific.
- Missing declared files becoming fatal may expose currently tolerated broken manifests and should produce actionable errors.
- AgentPM-managed uv acquisition/versioning must not accidentally make users manage uv themselves.

### Lockfile evolution

- Current lock parsing silently accepts unknown future fields and can rewrite them away; the forward-version guard is security/reproducibility sensitive.
- Content-derived lockfile versions can move backward today. The new `minimum_lock_version` logic must define whether this behavior remains acceptable.
- New Python resolution state must flow through resolve plans or plain install will erase it.
- Keep one logical package entry per Tool version; target-qualified lock keys would break current uniqueness/accessor assumptions.
- Older CLI behavior against new-format registry releases must fail clearly rather than downloading release metadata as if it were a tarball.

### Multi-artifact release

- Partial uploads must never become visible as finalized releases.
- Release/session and artifact-upload state can diverge from S3 unless finalize/cleanup is carefully transactional.
- Final publish must detect matrix jobs built from different commits/manifests/resolution states.
- One target may build/upload while another fails; the logical release remains unpublished.
- Staged object cleanup must tolerate retries and expired release sessions.
- Release manifest ordering must be deterministic.
- New `PackageVersionArtifact`/release fields must coexist with legacy scalar version columns without ambiguous read semantics.
- Per-artifact malware scanning must not accidentally mark the logical release clean when only one target was scanned.
- Streaming uploads need robust retry/error behavior and must preserve required presigned headers/checksums.

### Integrity/signing

- New canonical serialization is security-sensitive.
- Typed v1 migration must remain byte-for-byte compatible or existing signatures will stop verifying.
- Cross-language canonicalization drift would invalidate new signatures.
- Registry-attestation verification is meaningless without a client trust anchor; a key delivered next to its own signature cannot be blindly trusted.
- Registry-key rotation must preserve historical verification.
- Current 12-vs-16-character key-ID mismatch must not become part of cryptographic identity.
- New-format publish failing closed on registry-attestation infrastructure errors may expose operational misconfiguration; that is intentional but requires monitoring.
- Historical signer revocation has separate cryptographic-validity and current-trust-state meanings.
- Server-side stored-byte hashing/checksum verification may add finalize latency/cost.
- CI secret handling for encrypted signing keys/passphrases must be documented carefully.

### Installation/runtime

- Current cache names collide more easily than the logical package model allows; migration must not accidentally reuse stale cache entries.
- Current extraction silently skips symlink/hardlink entries and lacks decompressed-size caps; hardening may cause previously malformed packages to fail, which is preferable to incomplete installs.
- Managed dependency environments consume disk and need safe refresh/reuse semantics.
- Install currently completes after download/extraction; dependency provisioning introduces a new failure point that must not leave a falsely successful session.
- A dependency environment created with one interpreter must not be reused with another incompatible interpreter.

## Implementation choices Codex may resolve within invariants

The following are **not** pre-milestone contract blockers. Codex may choose the implementation that best matches existing repo patterns, provided the surrounding invariant in this spec is preserved. The Stage 1 contract decisions for `PythonResolution`, release-manifest type/schema, release-reservation table names, legacy scalar-column semantics, RFC 8785 canonicalization, and official-registry trust anchors are already fixed above and must not be reopened implicitly during implementation.

1. What exact internal representation should the faceted filter query use so new Stage 2 filters can be added without branching explosion?
2. Should the namespace pin maximum be exactly 6 or remain a small configurable constant?
3. Should updated-recency v1 remain 30/90/365 days after implementation UX review?
4. Should star counts appear beside total installs on every surface or vary slightly by density?
5. What exact PostHog SDK/server integration pattern best matches the existing frontend/backend architecture?
6. What persistent CLI config mechanism should store telemetry opt-out if the current config object does not already provide one?
7. Which Lemon Squeezy webhook event names map cleanly to subscription started/updated/cancelled/payment failed?
8. Should AgentPM bundle `uv`, download/cache a pinned standalone `uv`, or use another managed acquisition strategy? **Invariant:** the user's project must not be required to manage uv and AgentPM must pin/version-check the resolver it uses.
9. What exact on-disk local runtime-state metadata/file layout should record target/interpreter/environment fingerprint and selected artifact digest under `.agentpm/`? **Invariant:** this state is machine-local and never enters portable `agent.lock`.
10. What retention interval/background job should clean expired staged release artifacts? **Invariant:** cleanup must be retry-safe and must never delete finalized release objects.
11. Can S3 checksum headers provide the required trusted stored-byte SHA-256 in the current presigned flow, or should AgentPM independently stream/hash staged objects before finalize? **Invariant:** new release finalization cannot trust only publisher-declared metadata.
12. Should dependency environments be rebuilt automatically by `agentpm run` when the resolved interpreter changes, or should run fail with an explicit `agentpm install --refresh` instruction? **Invariant:** an incompatible existing environment is never silently reused.
13. Should the new reusable packaging surface be named `agentpm package`, or should an existing command be refactored into an equivalent machine-readable build mode? **Invariant:** the CI-facing build contract is provider-neutral and emits the required artifact descriptor.
14. Should Windows be added to the first official Tool publish matrix immediately given the existing CLI Windows release support, or follow after the required macOS/Linux proof? **Invariant:** AgentPM's own CLI CI still gains Windows coverage for target/runtime/extraction behavior in this stage.
15. What exact compatibility/trust fields should Stage 2 Package Health read from the new release/artifact model?

## Related Specs

- Stage 2: Category-Enabling Hardening — separate handoff/spec.
- Future Package Health spec.
- Future AgentPM Developer / category onboarding spec.
- Future advanced eval/quality framework spec.
- Future import/migration spec.
- Future additional CI providers / release automation spec.
