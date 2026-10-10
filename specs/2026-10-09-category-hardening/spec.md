# Feature

**Stage 2: Category-Enabling Hardening — Make the Agent Package Real**  
**Status:** Planning handoff / proposed implementation specification; Codex implements, Claude Code
independently reviews.
**Date:** 2026-10-09  
**Companions:** [`tasks.md`](tasks.md), [`test-plan.md`](test-plan.md), [`review-checklist.md`](review-checklist.md).

> **North star:** Someone who encounters AgentPM without its creator present understands what an **Agent
  Package** is, why it exists, what AgentPM manages, how the portable contract works, and how to try or
  create one.

> **Positioning hypothesis, not proven doctrine:** **AgentPM makes agents reusable in the way software
  packages are reusable.**

## Problem / Goal

AgentPM's minimum marketable product includes an operational CLI, hosted Registry, eight artifact kinds,
Node/Python SDKs, and a built-in Harness. These were added incrementally. The underlying capability model
is richer than the current user experience explains: the home page leads with Templates and individual
kinds, the Registry flattens all kinds as peers, the default Agent detail Overview leads with raw
`agent.json`, CLI help never explicitly calls AgentPM an **Agent Package Manager**, and the documentation
is still largely Tool-first. Validity, runtime compatibility, provenance, and the definition standard are
also not yet expressed as one consistent, inspectable contract.

Stage 2 establishes the *Agent Package* as an independently understandable, portable, reusable **top-level
composed Agent system** and AgentPM as an **Agent Package Manager**. It formalizes **AgentPM APDS v1.0.0
(Agent Package Definition Standard)**, makes authoring/publishing/installing/running honor it, introduces
objective Package Health, and reorganizes user-facing surfaces around the actual Agent Package lifecycle.
This is a product and implementation stage, **not just a copy/design stage**.

### Success questions

An experienced developer seeing the product for the first time should be able to answer, from real UI/docs
and a working example:

1. What constitutes an Agent Package, and how is it different from a framework project, repository, npm/PyPI
   package, host configuration, or standalone Tool/Skill?
2. Which reusable Components make up the complete Agent Package, and which relationships are direct or inherited?
3. What do AgentPM CLI, Registry, APDS, and Harness do, and where are other runtimes/frameworks allowed?
4. Where is the authored `agent.json`, and where is the immutable standard that defines its meaning?
5. Is this specific release APDS-conformant, compatible with my Runner/platform, secure/provenanced as far
   as evidence permits, and locally ready to run? These are **different claims**.
6. How do I find, install, configure, and execute a simple real Agent Package; then create, lint, publish,
   and share one of my own?

### Why this category, for whom, and how to judge it

**First intended audience:** experienced Agent builders and lead engineers who have already shipped
prototypes or working agent systems and encounter friction reusing them across repositories, teams,
frameworks, or hosting environments. They understand composition and versioning and need a practical
lifecycle. A secondary entry point is the capable developer building a first simple Agent who wants an
approachable declarative start without committing to a full framework.

**Actual alternatives they compare with:** implementing the Agent directly inside LangChain or custom
application code; developing within Claude/Codex/other hosts and accepting proprietary conventions;
distributing Git repositories as ad hoc agent packages; publishing Tools/Skills separately through npm,
PyPI, MCP or host-specific formats and manually wiring everything; maintaining internal package/runner
infrastructure; or doing nothing until their Agent count grows. Stage 2 must explain why a **complete
composed Agent Package** addresses problems those alternatives leave to each developer, without claiming
those alternatives are bad or must be replaced.

**Three intended differentiated outcomes:**

1. **Authoring speed:** reuse declarative Components instead of recoding the same composition in every app
   or framework.
2. **Distribution and reuse:** publish/install/version/lock complete systems and Components across
   teams/projects with clear contracts.
3. **Runtime clarity and control:** inspect authored composition and, when compatible, run it through an
   observable Harness with approvals/hooks/traces while leaving room for other Runners.

**Category-design boundary:** The phrase *Agent Package Management* is a deliberate, provisional way to
name the missing lifecycle. The website should proudly identify **AgentPM** as an Agent Package Manager,
but should not imply an external ecosystem has already ratified APDS or that every third-party framework
can run an AgentPM Agent Package today. “npm for agents” can be a conversational doorway but is not a
sufficient primary description; APDS manages a complete agent-system definition and dependencies, not
merely a Python/JavaScript code library.

**Project principle:** this is *category-enabling hardening*, not premature category-market proof. Stage 1
makes the product dependable; Stage 2 makes the complete artifact/lifecycle comprehensible and technically
real; Stage 5 user learning tests whether the story resonates with actual developers. Use future developer
interviews to refine positioning rather than hardcode a theory as permanent doctrine.

### Scope and intended output

- **Normative technical foundation:** A versioned APDS contract derived from the **existing**
  `schemas/agentpm.manifest.schema.json` (not a greenfield schema), normative semantics, stable rule IDs,
  shared conformance fixtures, CLI/Registry validators, and publication migration rules.
- **Lifecycle:** `init`, `lint`, publish/finalize, resolve/install/`agent.lock`, Harness
  compatibility/preflight, machine/SDK capability discoverability, and correctness fixes discovered in
  those paths.
- **Inspection and trust:** Universal + kind-specific objective Package Health, standard/source links,
  version-scoped evidence, and useful Agent Package composition/execution inspection.
- **Category-facing UI:** Homepage, Agent Package and Component detail pages, Explore/default discovery,
  namespaces, a versioned standard reference, category explainer, pricing copy, SEO semantics, cohesive
  design primitives and category-aware onboarding.
- **Adoption:** Agent Package-first CLI help and initialization; three first-use journeys; a simple
  low-cost runnable starter Agent Package; data-driven featured content; Package Card/reusable previews and
  an explicit README-embed investigation; a separate AgentPM Developer-stage handoff.
- **Final polish:** Comprehensive documentation and AgentPM-controlled README rewrite **as the final Stage
  2 milestone, after the entirety of Stage 1 has finished**.

### Working definitions and taxonomy (settled)

| Concept | User-facing meaning | Important boundary |
|---|---|---|
| **Agent Package** | Complete *authored* top-level Agent system: `kind: "agent"`, portable according to APDS | Complete does **not** mean self-contained, nonempty, Harness-runnable, or universally executable |
| **Agent Package Component** | Independently published reusable Tool, Skill, Knowledge, Memory Blueprint, Instruction Profile, or Loop | Internally may still be stored and versioned as a package kind |
| **Template** | Project/workspace scaffold with variables, files, entrypoints, and dependencies | Not interchangeable with the Agent Package primitive |
| **Agent** | Acting entity during execution | When referring to the published versioned artifact, say **Agent Package** |
| **Agent Package Management** | Definition, composition, validation, publishing, resolution, versioning, discovery, installing and managing the agent-system artifact | Category hypothesis to validate, not a claim that all ecosystems have adopted it |
| **Agent Package Manager** | Software implementing that lifecycle | **AgentPM CLI** explicitly identifies itself as an Agent Package Manager |
| **Agent Package Registry** | Discovery/distribution/publisher/version management | Does not define conformance solely by accepting the artifact |
| **Agent Package Runner** | Compatible runtime that can interpret/run supported Agent Packages | **AgentPM Harness** is built-in/reference Runner, not a universal mandate |
| **APDS** | Agent Package Definition Standard: versioned structure **and** semantics | Not a model-provider spec, Runner protocol, social rating, or safety certification |

**Product principle:** *Agent Package Management should be a layer, not a cage.* Integrate with frameworks,
host runtimes, MCP, open-source tools and existing development practices rather than requiring vertical
ownership. An artifact can conform to APDS without using the AgentPM Registry or Harness.

### Planning authority and implementation discretion

This document distinguishes three levels:

- **REQUIRED / AGREED:** Product contracts, externally visible behavior, correctness,
  backward-compatibility and acceptance requirements. Codex **must** implement these, or surface a conflict
  to Zack before deviating. Claude must reject silent reinterpretations.
- **PREFERRED / RECOMMENDATION:** Detailed engineering/design approach reflecting this planning
  conversation, especially APDS. Codex may select a more maintainable approach after examining current code
  and document its reasoning; Claude evaluates the tradeoff, not mere stylistic conformity.
- **INVESTIGATE / EXPERIMENT:** A research task with a decision record and acceptance criteria. This is
  **not** permission to silently leave a user-facing requirement unfinished. Where the outcome is genuinely
  contingent (e.g., description prompt A/B, embeddable Card v1), the deliverable can be a documented
  decision/prototype rather than full feature delivery.

**Decision log requirement:** For *every* open choice in §“Open questions,” Codex records options,
preferred approach, selected approach, rationale, constraints, and evidence (including tradeoffs and any
Stage 1 interaction) in the relevant milestone handoff/PR. Claude independently reviews these records. Do
not ask Zack to resolve normal engineering choices unless the agreed product semantics or scope would
change.

---

## Non-goals

- General-purpose, multi-vendor APDS standard federation or arbitrary external schema/semantics plugin
  loaders; no live untrusted GitHub-schema fetch at validation time.
- Replacing current AgentPM manifest field shapes or rebuilding the eight-kind artifact model from scratch;
  no mandatory separate standards repository/organization.
- Demanding a Loop, model, Tools, Profiles, Registry publication, or Harness capability to call an Agent
  Package structurally APDS-conformant.
- Designing a new Harness orchestration engine, abstract Runner framework, provider layer, runtime control
  protocol, SDK rewrite, or general prompt composition standard.
- Advanced semantic/AI search, LLM reranking, AI recommendations, reviews/ratings/comments/follows,
  universal package quality scores, or general eval platform.
- Reverse-dependency discovery (“Used by these Agent Packages”) in Stage 2; retain as future Registry
  enhancement only.
- Reworking subscriptions, checkout/billing plans, or private namespace permissions. Current Free/Pro/Team
  model remains.
- New standalone publication-success dashboard. The CLI already returns the Registry URL on successful
  publish; preserve it.
- Rewriting user-authored README content or displacing kind-specific inspection with generic marketing pages.
- Implementing an end-to-end AgentPM Developer wizard or its flagship Agent Package inside Stage 2. The
  guided “bring your idea → create → optionally publish → share” journey is an explicit **future AgentPM
  Developer-stage** handoff.
- Building an editorial CMS or requiring fixed curated example IDs/content before shipping the discovery
  infrastructure.
- Full GitHub README-embeddable Agent Package Card unless the Stage 2 investigation justifies the effort
  and it is explicitly accepted for this stage. **Investigating/prototyping it is in scope**.
- Treating generated visual mockups as authoritative colors/copy, fixed package metadata, provenance
  evidence, or working external-Runner integrations.

---

## Constraints / Invariants

### 1. Stage ownership, timing, and coordinated shipping

Stage 1 “Immediate Hardening” owns functional search pagination and state correctness, faceted filters,
relevance, stars/trending, namespace pins/search, shared detail shell, technical SEO plumbing, CLI
error/Tool scaffolding, telemetry, Python Tool portability, lockfile v4, multi-artifact release integrity,
signing and client verification, and CI publishing. Source:
`agentpm-dev/cli/specs/2026-10-06-immediate-hardening/{spec.md,tasks.md}`; its exact milestone IDs include
M1–M17 with submilestones 10A/B, 11A/B/C, 13A/B, 14A/B/C.

**Milestone ID convention:** the two stages share an `M<n><A/B/C>` namespace — `M11A`, `M13A`, `M14A/B`,
`M15A/B`, `M16A/B` and `M17` exist in both. Throughout this spec and its companions a bare `M…` reference
means **Stage 2**, and every Stage 1 reference carries an explicit `Stage 1` prefix. Cite split Stage 1
milestones by exact sub-milestone (`Stage 1 M14A`, not `Stage 1 M14`), since the sub-milestones own
materially different contracts.

Stage 2 **consumes those foundations**, does not duplicate them. Integration gates belong in `tasks.md`.
Particular handoffs: Stage 1 M2–M7 (discovery, detail shell, SEO), M8–M9 (CLI), M11B (lock v4), M12–M16
(portability, provenance, artifacts), M10A/B (analytics). Stage 1 M17 focuses its own
migration/documentation corrections; Stage 2 comprehensive category docs/AgentPM-owned READMEs happen
**only after all Stage 1 work is complete**, as Stage 2's **last** milestone. Earlier APDS normative files
and technical UI copy **are allowed**; they are required for functional development.

Deploy in coherent bands. Schema-aware publishing enforcement cannot become a surprise that breaks all
older clients before a compatible CLI and Registry migration path exist. Prefer add/read → validate/observe
→ enforce cutover, test rollback and cutover boundaries. Stage 2 bands can progress while unrelated Stage 1
bands finish. Gate only actual dependencies; do not invent a global Stage 1 blocker except for final
documentation and the APDS freeze gate below.

**REQUIRED — APDS freeze gate on Stage 1 manifest-schema additions.** Stage 1 adds fields to the *same*
`schemas/agentpm.manifest.schema.json` that Stage 2 freezes as immutable v1.0.0, and the affected
definitions are **closed** (`additionalProperties: false`), so each addition changes what is conformant:

| Stage 1 work | Schema location | Current state | Effect on a prematurely frozen v1.0.0 |
|---|---|---|---|
| Stage 1 M2 — `agentpm-harness` Template execution surface | `$defs/templateMetadata.properties.execution_surfaces` | closed `enum` of five values | A Template declaring `agentpm-harness` is **nonconformant** |
| Stage 1 M11A — optional Python `dependencies` on Tool runtime | `$defs/runtime` | `additionalProperties: false`, `required: [type, version]` | A Tool declaring `runtime.dependencies` is **nonconformant** |

Stage 1 M12's payload classification metadata is **not** in this table: it lives in the package-build
descriptor and release manifest, and all target artifacts of one release share a single logical `agent.json`
and manifest digest, so it does not affect APDS conformance. Verify that still holds when M12 lands.

57 of 78 current `$defs` are closed, so effectively any Stage 1 manifest addition is conformance-breaking
for an already-frozen version. Because v1.0.0's normative meaning and fixture outcomes are immutable once
published, **the structural freeze (Milestone 1B) and the normative-semantics freeze (Milestone 1C) MUST NOT
begin until Stage 1's manifest-schema-affecting work has landed** — concretely Stage 1 M2 (Release
Band 2) and Stage 1 M11A (Release Band 8). Those additions are part of v1.0.0's structural contract, not a
later version.

Everything upstream of the freeze proceeds in parallel: Milestone 1A inventory, the Band 4–5 website and
CLI-copy work, and all Stage 2 analysis are unblocked. Only 1B/1C and their downstream conformance and
enforcement milestones (2A–4B) carry this gate. Do **not** resolve the collision by publishing v1.0.0 early
and following it with v1.1.0 inside Stage 2: that doubles the fixture corpus and forces multi-version Runner
advertisement for no product benefit.

### 2. APDS v1.0.0 — authoritative contract bundle (strong preferred design)

**REQUIRED:** Publish a discoverable, versioned immutable **AgentPM APDS v1.0.0** specification containing
the same JSON structural contract as the current schema **plus** normative semantic rules and executable
cross-implementation conformance expectations.

**PREFERRED repository organization:**

```text
cli/
  schemas/
    agentpm.manifest.schema.json          # compatibility/editor entrypoint, not second authority
  standards/
    agentpm/
      1.0.0/
        README.md                         # definition, version, applicability, version policy
        manifest.schema.json              # immutable/versioned agentpm.manifest.schema.json
        semantics.md                      # immutable normative MUST/SHOULD/MAY rules
        conformance/
          README.md                       # how to interpret fixtures and validation levels
          fixtures/
            valid/
            invalid/
            resolved/                    # multi-manifest graphs and expected outcomes
          expected-results.json           # rule IDs/severity/status, paths where useful
```

**REQUIRED — close the existing runtime schema-source override.** Two authoritative schemas and a silent
validation bypass already exist in shipped code, and the "exactly one authoritative contract" requirement is
not met until they are fixed:

- `crates/agentpm-cli/src/manifest.rs` `resolve_schema_source()` prefers a **working-directory-relative**
  `schemas/agentpm.manifest.schema.json` over the compiled-in copy whenever that path exists. Dropping a
  permissive copy into a workspace makes `agentpm lint` pass manifests the bundled contract rejects
  (verified against CLI 0.1.33).
- The `--schema` override is honored by `lint` (`commands/lint.rs`) **and `publish`**
  (`commands/publish.rs`), so client-side publish preflight can be pointed at an arbitrary schema, including
  an `http(s)` URL that `load_schema_value()` will fetch at validation time.

For APDS validation the bundled pinned contract MUST win. Options Codex may choose between: remove the
CWD-relative lookup entirely; keep it only for explicitly-opted-in local schema development behind a
distinct flag; or keep the flag but make it refuse to satisfy APDS conformance and clearly mark results as
unverified-against-standard. In all cases an overridden schema MUST NOT be able to produce a `conformant`
APDS result, MUST be reported in output as a non-authoritative validation, and MUST NOT be reachable by
`publish` preflight in a way that implies conformance. This is a client-side integrity fix; it does not
replace §5's requirement that the Registry independently validate every new release.

This is a **suggested organization**, not a request to create an independent new schema or separate repo.
The uploaded `agentpm.manifest.schema(2).json` and repo `schemas/agentpm.manifest.schema.json` define the
existing eight-kind format; evolve and **version that same contract**. Codex decides how to keep the old
path valid (compatibility alias, generated copy, import/reference, build validation), but **two
independently drifting authoritative schemas are prohibited**. Existing `$id` presently references GitHub
`main`; the normative release requires an immutable/version-stable `$id`, and an editor `$schema` reference
must not double as the semantic standard selector.

**REQUIRED — scaffolds must emit a pinned `$schema`.** `agentpm init` currently writes `$schema` pointing at
the moving `main` branch URL, and `agentpm lint` actively warns when `$schema` is absent, so the field is
both emitted and encouraged. Once `$id` is version-stable, scaffolded manifests MUST emit the **pinned
versioned** URL rather than `main`; otherwise every newly authored package is edited against whatever is at
HEAD while being validated against the frozen contract, which is exactly the split-brain the version-stable
`$id` exists to prevent. `$schema` remains an optional editor/tooling affordance and still MUST NOT be
treated as the standard selector — `standard` is the only declaration. Existing manifests pointing at `main`
keep working; this is a change to what new scaffolds write. Bundle/cache supported contracts in CLI and Registry; no
arbitrary network fetch for routine lint/publish/run.

**Version dimensions MUST stay distinct:** artifact `version`; `standard.id` + `standard.version`;
`agent.lock` format version (Stage 1 prefers v4); CLI version; Registry protocol/version; Harness machine
protocol; Runner supported APDS version.

All eight newly authored kinds continue to use **`agent.json`** as manifest filename, but only `kind:
"agent"` is an **Agent Package**. Kinds: `agent`, `tool`, `skill`, `knowledge`, `memory`, `profile`,
`loop`, `template`. Templates remain scaffolds; the six Component kinds remain independently
publishable/installable.

#### Old-client forward compatibility — tolerance release before any `standard` emission

**REQUIRED, and sequenced before M3A.** The root schema is `additionalProperties: false`, so every CLI
already in users' hands hard-rejects a manifest carrying `standard`: verified on 0.1.33, `agentpm lint`
returns `[ERROR] Additional properties are not allowed ('standard' was unexpected)` and exits 1. The
affected commands are the schema-validating ones — `lint`, `publish` preflight, `new`, `export`,
`knowledge build`, `memory build`. `install`, `harness`, `run` and `serve` perform no schema validation and
no manifest struct uses `deny_unknown_fields`, so installing and running new packages on an old CLI is
unaffected.

`agentpm new` is the sharpest edge in **both** directions, because it validates generated manifests through
a blocking check (`commands/new.rs::validate_generated_manifests_blocking`). An old CLI scaffolding a
Template whose files carry `standard` fails *during `new`*, not at a later lint; and once lint is strict, a
Template whose files lack `standard` fails the same blocking check. See §15.1.

**Required sequence — this is §1's add/read → validate → enforce applied in the client direction:**

1. **A pre-APDS tolerance release.** One narrow change: add `standard` to the root `properties` with a
   permissive shape so the CLI accepts and round-trips it without validating it. No emission, no
   enforcement, no other behavior change. This does **not** depend on APDS being designed and SHOULD ship
   during Stage 1, well before the freeze gate lifts.
2. **Only then** M3A emits `standard` and M4A/M4B enforce it.

Relax the root by that **one known field** — do not open it to `additionalProperties: true`. An open root
permanently stops catching misspelled top-level keys in the version about to be frozen as immutable, which
is what §2's "do not silently loosen current `additionalProperties` contracts" is protecting. Later
extensions use the extension-point policy in §3 ch.2, not a second relaxation.

**Round-trip preservation must be confirmed, not assumed.** `write_manifest_pretty_atomic` takes a
`serde_json::Value`, so preservation depends on each caller. Current behavior, to be re-verified against the
branch at implementation time:

| Path | Behavior | Preserves unknown fields |
|---|---|---|
| `agentpm install <pkg>` (`commands/install.rs`) | writes back the loaded `manifest_value` after mutating dependency entries | **Yes** |
| `knowledge build --write` (`commands/knowledge.rs`) | loads the Value, mutates only the `knowledge` object, writes the whole Value | **Yes** |
| `memory build` (`commands/memory.rs`) | reads the manifest; writes only `.agentpm/memory/build.json` | n/a — never rewrites `agent.json` |
| `agentpm export` (`commands/export.rs`) | builds a **fresh** `json!` Skill scaffold | n/a — but **must be updated to emit `standard`**, since it generates an `agent.json` and the init-focused scaffold tasks do not name it |
| `workspace.rs` metadata writers | typed structs → `to_value` | **No** — unknown fields dropped. These are workspace/template metadata files, not manifests; keep `standard` out of them |

`serde_json` is pinned **without** `preserve_order` deliberately (`crates/agentpm-cli/Cargo.toml`: publish
author signatures depend on it), so a rewrite alphabetizes keys while losing nothing. The tolerance release
MUST NOT change that, and it is consistent with the RFC 8785/JCS canonicalization in §3, which also sorts
keys.

**Minimum CLI version.** Once the tolerance release is out, the Registry's outdated-client rejection and the
package detail page MUST name a concrete minimum version. An "upgrade your CLI" error without a version
floor is not actionable.

**New authored manifest selector:**

```json
{
  "kind": "agent",
  "name": "my-agent",
  "version": "1.0.0",
  "description": "Review technical documents and suggest improvements.",
  "standard": { "id": "agentpm", "version": "1.0.0" }
}
```

This illustrates **common fields**, not a complete publishable package including any other required
packaging metadata. `name` is **local/unscoped** in the current manifest schema; namespace-qualified
`@namespace/name` is a **dependency reference and Registry identity**, not a drop-in replacement for that
local field. Conformance fixtures must use real existing shapes and naming constraints. Do not invent
`"apds": "agentpm/1.0.0"` or rename `kind` to `type` merely because a mockup depicts it.

**Required schema changes:** `standard` with strict supported structure (and future extensibility handled
by selector), nonblank/trimmed `description`, and removal of `tools` from the Agent kind's unconditional
`required` list.

**REQUIRED — `description` becomes a hard failure, owned by one layer.** The rule already exists, but in the
wrong place and at the wrong severity: the schema types `description` as a bare `{"type": "string"}`, while
the Rust CLI emits a separate semantic `[WARN] description should not be empty` (verified on CLI 0.1.33 — a
whitespace-only description lints with a warning, not an error). In v1.0.0 a blank or whitespace-only
`description` MUST be a **conformance error**, not a warning. The **schema is the owning layer**: express it
structurally (for example `minLength` plus a non-whitespace `pattern`) so the Rust CLI and the Python
Registry both enforce it from the pinned contract instead of from separately maintained code. The existing
CLI-side warning MUST then be removed or replaced by the schema-backed error — the same defect reported
twice at two severities is itself a defect. Trimming semantics MUST be identical in both implementations and
covered by a shared fixture. Do not require optional Loop or Profiles; leave the existing valid kind-specific
requirements intact. Support explicit selector in init/lint/publish for **new authored releases**. Avoid
unplanned breaking changes to field names, dependency refs, Loop outcome forms, Memory contracts, Profile
structure, Template semantics, or Tool runtime metadata. Validate namespace-qualified dependencies using
the actual existing `$defs/packageRef` rules.

### 3. Detailed recommended `semantics.md` design — APDS's most important open design

**REQUIRED:** APDS v1.0.0 defines both the shape and **meaning** of authored data without mandating AgentPM
Harness internals. **PREFERRED:** One normative, human-readable `semantics.md` with a glossary, conformance
levels, MUST/SHOULD/MAY, stable rule identifiers, examples and fixture pointers. Do **not** introduce a
bespoke executable semantics language or rewrite the entire Harness implementation. If the document becomes
difficult to maintain, Codex may split chapters into included normative files, with one canonical index and
fixed-version references. No ambiguous duplicated sources of truth.

**Recommended chapter structure:**

1. **Scope, roles, and terminology:** Agent Package vs Component vs Template; manifest identity; dependency
   vs binding; author/validator/Registry/Runner responsibilities; version/immutability semantics; `MUST`,
   `SHOULD`, `MAY`; rule ID naming/version stability; errors vs advisories.
2. **Conformance levels and evidence:** standalone manifest/structural conformance, resolved-composition
   conformance, runtime interpretation/compatibility. A validator with missing dependencies reports **not
   fully evaluated / unresolved**, *not* fabricated success or an automatic intrinsic failure. Distinguish
   unsupported standard, nonconformance, unavailable resolution context, and Runner/environment readiness.
3. **Identity, kinds, and references:** local `name` versus namespace-qualified registry identity; package
   versions, constraints, normalized refs; kind correctness; exact-version resolution and identity;
   optionality of dependencies, Loop, Profile; Template is not Agent Package. Cross-reference existing
   formal JSON definitions.
4. **Dependency graph and composition:** kinds allowed by `tools`, `skills`, `knowledge`, `memory`, `loop`,
   `profile(s)`, and Template dependencies; direct/transitive edges; cycles and duplicate references where
   existing system disallows; how known versions and lock pins are interpreted; unresolved vs invalid
   references; no requirement to rewrite a dependency in the Agent to reuse it.
5. **Agent bindings:** authored global and per-Loop-phase availability; additive composition; binding
   identities are currently versionless names mapped to resolved exact dependency versions; resolve kind
   correctly; binding cannot create a capability not present in the composed graph; phase names checked
   against the resolved Loop; `consumer_context` remains consumer context, not a privilege grant; MCP
   bindings preserve their declared meaning.
6. **Skill dependency inheritance:** a bound Skill's declared Tool dependencies inherit the Skill binding's
   **global or phase** availability; no redundant direct Tool binding required. Effective Tool exposure
   still subject to Loop access and Runner control/approvals. Clarify duplicate directly+indirectly bound
   Tools so deduplication does not hide ambiguous version resolution.
7. **Loop semantics:** `entry_phase` must resolve to a phase; unique phase IDs; `objective`/`outcomes`;
   implicit completion when outcomes omitted; valid transition `from/on/to` and terminal targets `$end`,
   `$abort`, `$handoff`; reachability/well-formedness when resolvable; phase access `tools`, `knowledge`,
   `memory.read/write`; limits, checkpoints and error-policy declarations without overstandardizing
   scheduler implementation. Binding availability **never overrides** phase Loop prohibitions. Invalid
   outcome handling is Runner recovery, not a change to authored contract.
8. **Instruction Profile semantics:** `identity`/`objectives`/`communication` and optional
   boundaries/constraints/capability hints; global-to-phase composition and de-duplication; metadata shapes
   remain per schema; advisory hints do **not grant** Tool/Knowledge/Memory access; incompatible hints may
   be diagnosed without redefining allowed bindings or Loop controls.
9. **Knowledge semantics:** context vs vector representation, build and retrieval contract
   metadata/provenance, declared interface and compatibility requirements; payload
   implementation/index/runtime provider are not mandated by APDS. Cross-check any normative language
   against current Knowledge build/query behavior.
10. **Memory Blueprint semantics:** declarative durable memory schemas/spaces (`document`, `collection`,
    `sequence`), record types, retrieval, governance, capacity/retention, operations (`consolidate`,
    `transform`, `delete`) and supported triggers (`external`, `record_count`, `capacity`, `interval` as
    actually in current schema). **Bound spaces** describe directly accessible memory surfaces in scope.
    **Operations** are governed by Blueprint triggers: globally bound operations participate throughout a run
    when their trigger becomes eligible; phase-bound operations participate only in that phase; operations
    may read/write/target spaces not directly bound in the current phase; external triggers need an external
    runtime invocation. Do **not** pretend declaring the Blueprint persists data or autonomously schedules
    operations.
11. **Template semantics:** `kind: template` defines files root, variables, dependencies, entrypoints,
    execution surfaces; `agentpm new` scaffolds and may install referenced Components/Agents but the Template
    is not itself a runnable Agent Package. Include Stage 1's planned `agentpm-harness` execution surface
    when finalized. Variables never imply runtime secrets.
12. **Conformance and interoperability:** validator obligations, canonical diagnostics and rule/path
    representation, fixtures, explicit-vs-inferred legacy interpretation, unsupported versions, extension
    policy; independent validators and compatible Runners may conform without hosted Registry. Clarify
    whether unknown non-standard extensions are allowed only in specifically defined extension points; do not
    silently loosen current `additionalProperties` contracts.
13. **Runner boundary:** capability discovery/preflight, model/provider choice, approvals, hooks, tool
    runtimes, Knowledge/Memory backends, prompting, scheduling, traces, SDK host protocol and actual
    execution are Runner concerns. APDS states constraints and meaning; it does not dictate prompt assembly
    or imply universally runnable results.

**Sample rules Codex should reconcile against implementation and rewrite precisely:**

- `APDS-ID-001`: Newly authored conformant manifests MUST declare `standard.id` and `standard.version`, and
  validators MUST NOT silently fall back to another standard.
- `APDS-AGENT-001`: `kind: "agent"` MAY omit all dependency collections and Loop/Profile references and
  remain intrinsically valid if common schema requirements hold.
- `APDS-BIND-001`: Effective phase bindings MUST combine applicable global + phase bindings **additively**,
  then honor Loop access restrictions; phase bindings MUST NOT erase global bindings.
- `APDS-BIND-002`: A phase binding MUST name an actual phase in the resolved Agent's Loop when a Loop is
  present; unresolved dependency context MUST be reported distinctly from a conclusive violation.
- `APDS-SKILL-001`: Tools declared by a bound Skill MUST inherit that Skill's binding scope and MUST NOT
  require redundant direct Tool bindings.
- `APDS-LOOP-001`: A Runner MUST NOT expose Tools, Knowledge, or Memory reads/writes in contradiction of
  the relevant phase's Loop access policy solely because a binding exists.
- `APDS-LOOP-002`: A Loop phase without explicit outcomes has the existing implicit `complete` outcome;
  graph transitions/terminal targets MUST resolve according to the existing Loop contract.
- `APDS-PROFILE-001`: Profile boundaries/capability hints MUST NOT be interpreted as granting capabilities
  prohibited by bindings or Loop policy.
- `APDS-MEM-001`: Binding a Memory operation does not require all its source/target spaces to be directly
  bound; Blueprint trigger and scope semantics still govern its eligibility.
- `APDS-MEM-002`: `external`-triggered operations require runtime invocation; the Blueprint alone does not
  autonomously execute work.
- `APDS-TEMPLATE-001`: A Template's execution surfaces describe scaffolded usage; a Template is not
  inherently an Agent Package or Runner-executable artifact.

**These IDs/text are RECOMMENDED examples, not preapproved normative definitions.** Codex verifies every
rule against the current schema, Phase 6/7 implementation, and real conformance examples. If a material
mismatch exists, pause/review rather than codifying accidental implementation behavior as an APDS promise.
Rules need applicability, rationale, validation level, severity, path(s), and fixture mappings.
Human-friendly CLI/Registry messages may show rule IDs unobtrusively; conformance engines must preserve
them programmatically.

**Diagnostic substrate — owned by Stage 1 M8, consumed here.** Rule IDs are only useful next to a real
manifest path, and today kind selection is an eight-branch top-level `oneOf`: any kind-specific violation
reports as the entire manifest being `not valid under any of the schemas listed in the 'oneOf' keyword` at
path `/oneOf`, naming neither the branch nor the field (verified on CLI 0.1.33). **Stage 1 M8 owns
restructuring root kind dispatch** — `if`/`then` per kind, or mapping the `oneOf` failure to the branch
matching the declared `kind` — and owns making the diagnostic shape able to carry an external rule
identifier alongside the path. Stage 2 attaches APDS rule IDs to that substrate and MUST NOT re-cut the
renderer. If Stage 1 M8 has not landed when Stage 2 Band 1 begins, treat it as a prerequisite and
coordinate, rather than forking a parallel diagnostic path.

**Conformance model (recommended):**

- `conformant`: checks applicable at the requested validation level passed.
- `nonconformant`: a definite violation was found (with stable rule ID, severity, JSON path, linked
  standard version).
- `incomplete` / `not_evaluated`: context for required resolved-composition checks is missing; state which
  pieces couldn't be checked, never claim full verification.
- `unsupported`: the declared standard ID/version is not supported; do not reinterpret as a different version.

The *exact status serialization* may be determined by Codex after examining API/CLI patterns, but these
distinctions are invariant. Registry security/ACL/signatures/malware outcomes and Runner readiness are
**not** APDS conformance states.

**Required fixture corpus:** minimal dependency-free valid Agent; optional Loop/Profile; valid 8-kind
examples; Skill-inherited Tools in global and phase scopes; additive global+phase and Loop access
restriction; Memory bound operation targeting not-directly-bound spaces, interval/count/external triggers;
optional/implicit Loop outcomes; invalid/unknown phase, transition, reference and unsupported standard;
empty/whitespace description; malformed schema, wrong-kind dependencies;
unresolved-but-otherwise-structural case; explicit and legacy-inferred differentiation.
`conformance/expected-results.json` or equivalent must carry expected validation level and rule IDs. **Rust
CLI and Python Registry must pass the same fixture corpus independently**, using pinned contract copies.
Avoid only duplicating a single validator or trusting server-supplied validity.

**Contract maintenance policy:** Once published, v1.0.0's normative meaning and fixture outcomes are
immutable. Typos/clarifications not changing conformance can be handled with an explicit controlled policy,
but any changed rule outcome requires a deliberate standard version. Bundles/checksums, schema `$id`,
fixtures and Registry standard pages must point to the same version. Do not bind stable conformance to a
moving `main` branch URL.

#### Expanded APDS implementation reference

Preferred design and concrete cases, for implementers without this chat. This refines the settled requirements above.

#### APDS design rationale and physical distribution (preferred, not compulsory)

The standard must be independently intelligible to an engineer who has not read AgentPM's source code or
this planning conversation. The existing schema is the starting point, **not** a temporary compatibility
artifact to discard. On the current schema baseline:

- the source file is `cli/schemas/agentpm.manifest.schema.json`;
- the manifest uses JSON Schema Draft 2020-12, a top-level `kind` with eight values and a common
  `agent.json` filename;
- top-level `name` is a **local package name** matching the existing name grammar; `@namespace/name` is
  typically the **registry/dependency reference**, not the authored local `name` field;
- top-level `description` already exists but needs a meaningful non-whitespace rule;
- the current `$id` points to a moving `main` branch and should stop being the normative identity of a
  frozen version;
- the current Agent rule incorrectly requires a `tools` array even though an Agent with zero Tools is a
  valid authored system;
- bindings reference versionless `@namespace/name` identities to things declared in dependency lists;
  resolving them to a locked **exact** version is a composition step.

**Recommended repository organization:**

```text
cli/
  schemas/
    agentpm.manifest.schema.json     # compatibility/editor path; derived or redirected
  standards/
    agentpm/
      1.0.0/
        README.md                     # scope, terminology, how to conform
        manifest.schema.json         # canonical evolved existing JSON Schema
        semantics.md                 # normative, versioned interpretation rules
        conformance/
          README.md                   # fixture runner contract + expected outcomes
          expected-results.json      # cases, levels, statuses, rule IDs
          fixtures/
            valid/                   # intrinsic manifestations across eight kinds
            invalid/                 # definite single- or multi-rule violations
            resolved/                # cross-artifact graph inputs and outcomes
            incomplete/              # deliberately unresolved contexts (optional)
```

The `incomplete/` directory and exact filenames are examples, not a mandate. Do **not** force
implementation to keep two separately edited authoritative JSON Schemas. Prefer a build-time copy/generated
compatibility artifact or stable resolver from the old path to the versioned one. The existing path may
remain in developer tooling, but CI should detect divergence. CLI should bundle the frozen schema in its
release; Registry should deploy its own pinned, reviewed copy. Verify hashes/versions against a common
fixture/index, not runtime network retrieval. The website may render the frozen documentation, but must not
silently replace it with whichever commit is at GitHub HEAD.

**The standard is not an implementation service.** A standard implementer can read the schema, semantics
and conformance fixtures; author `agent.json`; validate it; and interpret bindings without owning an
AgentPM account. Registry publication is an optional distribution choice. Harness execution is an optional
Runner choice.

#### The conceptual schema hierarchy and examples

The recommended v1.0.0 schema is a common envelope plus conditional kind-specific rules, not eight
unrelated formats. Existing accepted fields are preserved unless they conflict with a settled semantic
requirement. Illustrative common fields below deliberately use local `name` and distinguish *artifact*
version from *standard* version:

```json
{
  "kind": "agent",
  "name": "sample-research-agent",
  "version": "0.1.0",
  "description": "Researches an engineering question and prepares a concise brief.",
  "standard": { "id": "agentpm", "version": "1.0.0" }
}
```

This **intrinsically conformant minimal example** is expected to lint even though it has no Loop or
dependencies. It is **not** an example of a runnable Agent under the current Harness requirements. If the
current parser assumes a `tools: []` field is present, fix or reconcile that parser; do not turn the parser
limitation into an APDS requirement.

An example package reference is `@publisher/research-skill@0.1.0` or the existing object form `{ "name":
"@publisher/research-skill", "version": "0.1.0" }`, depending on the field and existing schema. An Agent
binding refers to `@publisher/research-skill` without an embedded version; the resolver/lock maps it to the
exact selected dependency. **Do not replace the local manifest name with the fully scoped Registry
identity** as a side effect of APDS standardization.

The only new normative top-level field we have settled is `standard`; the task is *not* to add a new `apds`
string or rework every existing dependency field. `$schema` remains an optional editor/tooling pointer and
cannot substitute for `standard`. A JSON Schema can validate structure and many local constraints, but not
that a phase name matches a dependency Loop or that a transitive Tool is legally exposed during execution.

#### Suggested APDS `README.md` sections

1. Standard title, identifier (`agentpm`), version (`1.0.0`) and status.
2. Normative/informative documents and precedence: schema for shape, semantics for interpretation, fixtures
   for examples; how conflicts are handled before release.
3. Agent Package, Component, Template, Manager, Registry, Runner definitions.
4. Supported artifact kinds and the top-level manifest's local-name rules.
5. How a producer declares APDS and why `$schema` is not a declaration.
6. Validation levels: intrinsic, resolved, and Runner/environment checks (the last is **not** APDS conformance).
7. Role of the standard independent of a specific Registry or Runner.
8. What v1.0.0 does not standardize and compatible implementation expectations.
9. Version immutability policy and links to canonical schema, semantics and fixtures.
10. How to report a semantic ambiguity without silently changing frozen fixtures.

#### Normative semantics chapter blueprint — more detailed than a table of contents

Codex should use the following as an **implementation guide for the actual content of `semantics.md`**, not
replace each bullet with a single sentence. Each rule should have a stable ID, normative statement,
applicability, validation level, failure mode, rationale, illustrative example and a conformance fixture
reference. Where the existing source does not support a proposed interpretation, **surface a reconciliation
issue** before freezing v1.0.0.

**Chapter 1 — Status, scope, and normative language**

- Define `MUST`, `MUST NOT`, `SHOULD`, `SHOULD NOT`, `MAY` and conformance subjects: artifact author,
  validator, registry, and compatible Runner.
- Specify difference between a standalone manifest being structurally conformant and a complete resolved
  graph being semantically conformant.
- Explain that the standard may prescribe meaning or restrictions without mandating how runtime services
  are implemented.
- Specify diagnostics with rule ID, manifest path, declared standard and validation level; avoid conflating
  invalid with missing dependency context.
- Distinguish the immutable normative document from explanatory website/blog/docs copy that can evolve.

**Chapter 2 — Identity, version, namespace, and references**

- Describe common manifest `kind`, local `name`, artifact `version`, purpose `description`, declared
  `standard`, and optional editor `$schema` precisely.
- Distinguish package identity (`@namespace/name`), dependency version selectors (range/exact where
  supported), locked versions, and APDS standard version.
- Artifacts of different kinds can have similar names; identity must include kind where necessary to avoid
  ambiguous resolution.
- The Registry namespace and permissions are distribution semantics; APDS must not require one hosted Registry.
- Describe unknown-property behavior and how a future extension may be introduced without permissive v1.0.0
  rewriting.
- A legacy missing declaration is **not** an alternative normative v1.0.0 authoring format; legacy
  inference is a compatibility adapter outside the new-release contract.

**Chapter 3 — Dependencies and composition**

- Define direct references in `tools`, `skills`, `knowledge`, `memory`, `profile`/`profiles`, `loop` and
  other actual fields according to existing schemas.
- Distinguish a declared direct dependency from dependencies of that dependency; transitive paths remain
  traceable for inspection.
- State that resolving a versionless binding does **not** authorize taking any installed version matching
  the name; it must resolve through the Agent's declared dependency graph/lock.
- Invalid cross-kind references should fail when enough context exists, e.g. declaring a Skill identity in
  a Tool-only binding.
- No implicit requirement that every Agent have a Tool, Profile, Loop, Memory or Knowledge package; missing
  categories are legitimate.
- Reproducibility is expressed through exact resolved versions/digests, but exact `agent.lock`
  serialization is an AgentPM Manager concern and belongs outside the normative manifest schema.

**Chapter 4 — Agent bindings and effective capabilities**

- Document global bindings, phase bindings, the six Component families, Memory spaces/operations, MCP
  bindings, and consumer context only to the extent supported by the current format.
- Global and phase bindings are **additive**. A phase's effective candidate capabilities include globally
  bound capabilities and bindings specifically declared for that phase.
- Phase-specific bindings only apply in their named phase. A declared phase key must identify an actual
  phase in the Agent's resolved Loop to pass resolved-composition validation.
- Direct Tool bindings and Tool capabilities inherited through bound Skills are distinct paths; inheritance
  is not a reason to duplicate a direct binding.
- **Loop phase access rules constrain** effective Tool/Knowledge/Memory exposure even if a capability is
  bound globally or inherited by a Skill.
- Differentiate authored availability, runtime service availability, and actual tool invocation: a binding
  does not authorize bypassing approvals or grant an unsupported Runner capability.
- MCP bound surfaces may add capabilities only as permitted by the authored binding and Loop policy; avoid
  inventing precedence rules not yet specified in source.

**Chapter 5 — Skills and inherited Tools**

- A Skill may list declared Tool dependencies in its own manifest; these are reusable Component
  relationships, not an Agent code import.
- Globally bound Skill → its Tool dependencies are available as global candidates (subject to Loop restrictions).
- Phase-bound Skill → its Tool dependencies are candidates **only** for that phase (subject to Loop restrictions).
- If a Skill and Tool are both bound directly, duplicate availability must not arbitrarily double-expose
  the same exact capability; preserve traceability and de-dup semantics where supported.
- Skill references and scripts describe packaged content; scripts are not automatically executed by merely
  binding the Skill.
- A shell-executor Tool *may* run a Skill script through normal Runner execution/approval, but no automatic
  script privilege is granted by APDS.

**Chapter 6 — Loops and authored orchestration**

- Distinguish a Loop package's graph from the acting runtime Agent. State required authored `entry_phase`,
  phases, transitions, optional outcomes, `limits.max_steps`, optional checkpoints/error policies.
- When a phase omits outcomes, its default outcome is the existing implicit `complete` behavior; document
  precisely how legal transitions/termination are validated against current code.
- Check phase ID uniqueness, entry validity, transition sources, outcome names and target phase/terminal
  marker validity at appropriate validation levels.
- Terminal targets include `$end`, `$abort`, `$handoff` where currently supported; preserve authored
  meaning, avoid adding new terminals.
- Graph may cycle or branch; a UI may show linear cards only as a derived simplification, never
  misrepresent the normative graph.
- A compatible Runner honors declared limits, transitions, access restrictions, checkpoints and error
  policy, but APDS does not mandate a particular event bus/TUI or prompt algorithm.
- A structurally conformant Agent without a Loop remains conformant; the current Harness may legitimately
  mark it incompatible/not runnable.

**Chapter 7 — Instruction Profiles**

- Define Profile identity, objectives, audience, communication, vocabulary, boundaries, constraints and
  compatibility/hints in terms of the current manifest.
- Explain composed Profiles global → phase, preserving additive/de-dup behavior and ordering as implemented.
- Profiles influence prompting/behavior but **do not grant** tool access or waive Loop policy;
  `capability_hints` are advisory.
- Compatibility constraints and hints may inform Runner warnings, not prove universal runtime support.
- Distinguish top-level Agent `description` (purpose) from Profile identity and Loop phase `objective`; no
  APDS requirement that `description` be injected into any particular model prompt.

**Chapter 8 — Knowledge**

- Define `context` versus `vector` mode, authored document/corpus metadata, chunks/sources/embedding/index
  artifacts and provenance where available.
- State what a Knowledge package declares and how a Runner may retrieve/use it; never require a particular
  embedding provider, vector DB or retrieval algorithm.
- Validate referenced artifact paths, index/embedding IDs and intrinsic metadata relationships in a
  deterministic manner where the current schema supports them.
- Treat vector dimension/index compatibility as evidence-based capability information, not universal
  guaranteed executability.
- Knowledge availability may be phase-gated by Loop access and binding scope independently of whether the
  package itself is valid.

**Chapter 9 — Memory Blueprints and memory operations**

- Memory is declarative durable structure/behavior: scopes, record types, spaces (`document`, `collection`,
  `sequence`), retrieval modes, retention/capacity/governance metadata, and operations.
- A bound Memory *space* makes a directly readable/writable surface available in its binding scope (subject
  to Loop access and backend support).
- A bound Memory *operation* participates when eligible according to its Blueprint trigger; globally bound
  operations participate throughout the run, phase-bound operations only in the selected phase.
- The operation MAY read/write/target spaces that are **not directly bound** in that phase; the operation's
  declared inputs/targets and policy govern it. Reject an implementation that demands direct surface
  binding of every operation target.
- Triggers currently include `external`, `interval`, `record_count`, and `capacity` in the inspected
  schema; any implementation must verify exact fields and intended runtime behavior. `external` requires
  explicit runtime invocation, not automatic background execution by the manifest.
- Blueprint validation can establish declared trigger/type/reference consistency, not prove the existence
  of a durable store. Backends, scheduling, cleanup, model content generation and actual persistence are
  Runner responsibilities.

**Chapter 10 — Templates**

- Templates are separately installable/publishable scaffold artifacts, not synonymous with Agent Packages.
- Explain `display_name`, `use_case`, `execution_surfaces`, `files_root`, `dependencies`, `entrypoints`,
  `variables`, and template file paths using existing shape.
- `agentpm new` produces a workspace/project; it is different from installing a complete Agent Package and
  attempting to execute it.
- Stage 1 introduces a `agentpm-harness` Template execution surface; Stage 2 should document what it
  promises and when it is valid.
- Template variables are generation-time configuration; don't encourage putting secrets or credentials into
  authored published templates.

**Chapter 11 — Conformance, interpretation, and backwards compatibility**

- Validators distinguish intrinsic structural validation from resolved graph validation; not every APDS
  rule can be evaluated with only one manifest.
- A result can be `conformant` for the executed validation level while graph-level checks remain `not
  evaluated`; UI must not label such result a full-graph conformance verification.
- `nonconformant` means definite violations, `incomplete` means external information needed, and
  `unsupported` means that the declared standard is not implemented by this validator/Runner. Don't
  translate unsupported into an implicit latest version.
- Legacy inference is an AgentPM compatibility mechanism and must not forge declaration, recompute
  published checksums, or assert retrospective validation that never happened.
- APDS conformance, Registry trust/permissions/scanning, Runner standard support, and local readiness are
  distinct axes with distinct diagnostics.

#### Concrete binding/composition cases that must become normative fixtures

These are behavioral **expected outcomes**, not a request to copy this pseudo-manifest as if it were
schema-valid JSON. Codex must create schema-valid actual manifests using existing field forms and real
references for conformance fixtures.

| Case | Global binding | Phase-specific binding | Loop access | Required interpretation |
|---|---|---|---|---|
| A: additive Tool | Skill S (declares Tool T) | Phase `draft`: Tool U | Tools allowed | In `draft`, T and U are candidates; other phases see T only |
| B: inherited Tool blocked | Skill S (declares Tool T) | None | `review` Tools denied | T is not executable in `review` despite global Skill binding |
| C: phase inheritance | None | Phase `research`: Skill S → Tool T | `research` Tools allowed | T is available only in `research`, not `publish` |
| D: invalid phase key | None | Phase `nonexistent`: Skill S | Loop has `research` and `publish` | Resolved validation fails with unknown phase, not a missing runtime-model error |
| E: profile hint | Profile P recommends tool use | None | All Tools denied | Hint does not grant any Tool; Profile remains a valid authored package |
| F: Memory space vs operation | Memory operation O bound in `review` | Space X *not* directly bound in `review` | Memory operation allowed by policy | O may target X as declared in Blueprint; direct space availability remains restricted |
| G: Memory external trigger | Operation O trigger `external` | Bound globally | Runner never invokes O | Merely binding O must not schedule/execute it |
| H: valid but nonrunnable | No Loop or Tools | None | N/A | APDS intrinsic validation passes; current Harness reports not runnable/incompatible |
| I: versioned workspace | Skill S and Tool T exist at two versions | Binding references versionless identity | Phase allows Tools | Effective dependency picks the exact locked instance in the selected Agent graph, never first name match |

Explicitly test both **global and phase** paths for Skill Tool inheritance, and both **direct** and
**operation-mediated** Memory access. These were carefully established in Phase 7A and must not be
rewritten as stricter direct-binding rules.

#### Validator result shape — proposed contract, not fixed API

Use a small versioned internal result model that multiple surfaces can map to without pretending a public
API field shape is already settled:

```json
{
  "standard": { "id": "agentpm", "version": "1.0.0" },
  "declaration_origin": "explicit",
  "validation_level": "resolved",
  "status": "nonconformant",
  "violations": [
    {
      "rule_id": "APDS-BIND-002",
      "path": "/bindings/phases/nonexistent",
      "severity": "error",
      "message": "No phase with this identifier exists in the resolved Loop"
    }
  ],
  "not_evaluated": []
}
```

The status, field names and diagnostic mapping are recommendations. The invariant is **truthful,
explainable results** with source, level, standard version and applicable rule IDs. A missing network
connection or unresolved package may produce `incomplete` rather than `nonconformant`; an intrinsically
malformed declaration produces `nonconformant`; unsupported version produces `unsupported`. Publish-time
enforcement applies to the server's required levels of validation, not blindly accepting `incomplete` as
verified.

#### Legacy/new manifest behavior matrix (settled outcomes, interface flexible)

| Artifact scenario | Standard declaration | Authoring lint | New Registry publish | Install existing release | Health/standard UI |
|---|---|---|---|---|---|
| New APDS v1.0.0 | Present, supported | Validate | Accept only if validated | Supported | Declared + verified at observed level |
| New manifest with missing `standard` | Missing | Fail normally | Reject | N/A unless historical | Do not claim conformance |
| New manifest with unknown version | Unsupported | Explicit fail | Reject | Explicit unsupported handling | Unsupported, not assumed latest |
| Previously published immutable legacy | Missing historically | Migration mode may inspect | N/A, no republish | Continue to install | Inferred interpretation; not verified solely by inference |
| New version of historically legacy identity | Required on new version | Validate | Require explicit APDS | Works once valid | Independent version-level status |
| Staged session straddling cutover | May be old client | Depends on policy | Deterministic server-side cutoff and explainable error/compat path | No old published bytes altered | No ambiguous hidden acceptance |
| Dependency graph unavailable | Supported declaration | Intrinsic may pass, graph incomplete | Apply server gate at required levels | Install can fail with clear resolution issue | Incomplete, not fully verified |

The server may reject a new release when it cannot satisfy required conformance checks; it must not mark it
verified without evidence. A migration tool is optional, but new authoring and new finalized releases are
strict. Do not silently permit a bypass using an old publish-client endpoint.

#### APDS conformance fixture and CI execution design

Use a **single fixture corpus** that neither Rust nor Python is allowed to customize to hide differences. A
fixture should declare input file(s), validation level, expected status, expected violated rule IDs and
paths where meaningful. Good fixtures include **paired positives and negatives** (one rule changed at a
time) so a validator does not pass by rejecting everything.

Recommended representation:

```json
{
  "case_id": "phase-binding-unknown",
  "root": "fixtures/resolved/invalid-phase/agent.json",
  "dependencies": ["fixtures/resolved/invalid-phase/loop/agent.json"],
  "level": "resolved",
  "expected_status": "nonconformant",
  "expected_rule_ids": ["APDS-BIND-002"]
}
```

The fixture index format is flexible; the need for independent, deterministic conformance is not. Each kind
requires positive cases; each major semantic family needs a positive/negative case; resolved cases need
actual graph context; tests must include schema/tooling parity **and** semantics. Both validators should
run in CI without GitHub network access. Hash or otherwise pin the canonical schema/semantics/fixture
bundle for the released version.

#### APDS preservation and semantic collision checklist

Before freezing v1.0.0, Codex and Claude must explicitly resolve these potential mismatches:

- `name` in the authored manifest is not the same as Registry `@namespace/name`; avoid examples that teach
  the wrong shape.
- The schema has `kind`, not `type`; avoid mockup-generated `type` snippets.
- The new declaration is `standard: {id, version}`, not the mockup-generated `apds: "agentpm/1.0.0"`.
- Optional Agent Loop and Tools must coexist with existing Harness's runnable-with-Loop requirement.
- Exactly which Profile field(s) are accepted by the schema (`profile`, `profiles`, phase
  `bindings.profiles`) must be validated, not guessed.
- Memory triggers include `capacity` in the current source, in addition to `external`, `record_count` and `interval`.
- Templates are a separate kind and must not be counted as Agent Packages.
- Binding identity lookup must preserve versioned graph context even if authored binding names are versionless.
- The standard's normative schema `$id` must be immutable, not a moving `main` URI.
- Any future standard-v1.0.1 change needs an intentional compatibility policy; do not retroactively rewrite
  v1.0.0 fixtures.


### 4. CLI init/lint semantics and migration

- `agentpm init` **defaults to `kind: "agent"`**; default artifact name/description reflect an Agent
  Package (not `my-tool`). Keep `--kind agent` and all seven other explicit kinds working, including Stage
  1's Tool-scaffold repairs. Generate `standard` on all new kinds. JSON serialization MUST safely encode
  quotes, backslashes, newlines and Unicode in user-provided names/descriptions (even where name validation
  rejects invalid names, diagnostics must be deliberate).
- `agentpm lint` and applicable package build/inspect/publish preflight use an explicit supported standard
  selector and shared structural+semantic validation. Unknown/missing selector on *new authored* manifests
  is a clear failure. Preserve honest output states and actionable remediation.
- Do not mutate or republish existing immutable legacy Registry artifacts solely to add `standard`. New
  version of an old package is a **new release** and must comply. Consider optional explicit legacy
  lint/migration mode; do not make silent permissive fallback the default.
- The `description` communicates baseline Agent/Component purpose; optional Profile specifies detailed
  identity/behavior; phase `objective` specifies phase intent. No APDS guarantee that the Runner
  automatically injects top-level description into prompts.

### 5. Registry independent publishing enforcement

- Python Registry validation is independent of CLI results; shared contract **fixtures**, not a shared
  trust in client claims.
- Validate staged bytes of embedded `agent.json` and request/init metadata; enforce kind, name, version,
  standard and relevant artifact association consistency before finalizing a release. Compare embedded
  version to intended release identity, preserve Stage 1's multi-artifact release model. Validate before
  making bytes discoverable, and preserve atomic finalize/rollback behavior.
- Effective server enforcement applies to **every new finalized version**, including newer versions of
  existing packages. Preserve historic published versions' bytes and installability.
- Add cross-release migration rules for in-flight init/finalize sessions crossing rollout, explicit errors
  for incompatible clients/unsupported standards, and observability of rejections. No client-controlled
  cutover timestamp.
- Independent code paths must report APDS failures separately from authorization, signing, attestation,
  malware scanning, namespace policy and artifact integrity.
- Consider feature flag/progressive rollout only if the release path genuinely requires it; do not leave an
  indefinite bypass route in production.

### 6. Resolve, install, lockfile and offline behavior

- Propagate effective APDS `{id,version}` and its **provenance** (explicit declaration vs legacy inferred)
  across resolve/installed metadata/lock records. Do not assert legacy verified conformance. Actual
  installed embedded manifest is authoritative; reconcile API and `agent.lock` claims with pinned package
  identity/version/digest.
- **PREFERRED:** retain **`agent.lock` v4**, which Stage 1 introduces. Integrate additional data without
  silently deleting unknown newer-version fields. If necessary, document a justified compatibility
  migration; do not bump to v5 merely because APDS exists.
- Honor exact locked versions, semver/range satisfaction, all direct+transitive dependencies, integrity
  checks and replay in **`--frozen`** workflows, including directly installed Agent/Skill kinds. Repeated
  clean installs reproduce closure; mismatches fail before corrupting state.
- A valid Agent Package with **zero dependencies** must install successfully without a failing empty
  resolve request. Preserve no-op/frozen lock correctness.
- When the package's APDS standard is unsupported, report that explicitly; do not silently reinterpret
  using current default or ignore version mismatch.
- Reuse Stage 1's target-aware Tool artifact selection, lock v4 and signature/attestation verification; do
  not implement a competing package identity/integrity mechanism.

### 7. Harness / SDK Runner compatibility

- Harness supports/advertises concrete APDS standard IDs/versions and validates the selected resolved Agent
  Package graph against supported semantics before executing. Expose capability info through current CLI,
  TUI, headless/machine preflight/initialization and Node/Python SDK interfaces **without changing
  Harness's core execution model**.
- Keep **APDS conformant**, **Harness compatible**, **locally ready to run** as separate facts. APDS-valid
  Agent missing a Loop is still valid but current Harness should give a precise `not runnable here`
  explanation.
- Fix/check name-only resolution of Component bindings in multi-Agent/multi-version workspaces: all runtime
  capabilities should bind to the **locked exact version** rather than an arbitrary package with the same
  name.
- Headless noninteractive readiness must truthfully reject missing provider/model configuration when
  required. Bootstrap prompting must not be assumed available without a TTY.
- Preserve Runner policies for invalid outcomes, optional warnings, approvals and recovery; no refactoring
  of Engine inner loop, phase outputs, handoff JSON contract, or provider interfaces here.
- **EXPERIMENT:** compare Harness phase behavior with and without optional top-level `description` prompt
  context; evaluate prompt/trace outcome quality, avoid inflating prompts or duplicating
  Profiles/objectives, record decision. Do not impose this on APDS or external Runners.

#### Expanded lifecycle implementation reference

Concrete pass/fail expectations for authoring, publication, install and Runner compatibility.

#### CLI authoring experience: exact user journeys and failure semantics

The intended first-time authoring flow is:

```bash
agentpm init                      # defaults to a minimal Agent Package
agentpm lint                      # strict APDS-aware validation
# edit the Agent's dependencies/bindings and optionally choose a Loop
agentpm install                   # resolve/update the lock and local installation
agentpm harness ...               # only when a compatible Loop/runtime is configured
agentpm publish                   # verified new Registry release
```

This is a conceptual flow, **not** a promise that every command accepts the exact form above in every
environment. Codex must use current CLI argument contracts in executable guides. Default init should
explain how to make a structurally valid Agent **runnable** without requiring a Loop just to pass lint. An
author creating an individual Tool must still have a direct explicit workflow (`agentpm init --kind tool`).

Newly authored `agent.json` should be generated using a real JSON serializer rather than interpolating
arbitrary user input inside string literals. A malicious or accidental quote/newline in a description may
be rejected by purpose/name policy but must never generate accidentally malformed JSON or an injected
field. The `standard` field and any edited default examples should survive publish/manifest generation and
packing intact.

CLI lint should report, for example, that a manifest has an unsupported APDS version **as a version support
error**, not as an unrelated missing Tool; and a missing Loop with an otherwise valid Agent should not be a
lint error. A known Loop with an unknown phase binding should fail resolved validation, with contextual
path/phase/Loop information. Avoid hiding conformance failures in an undifferentiated string-only warning.

#### Publishing: threat model and validation boundaries

The current custom-client threat model matters: the CLI may lint correctly, but a client can call Registry
publication endpoints independently. Registry validation therefore needs to be **authoritative** for every
newly finalized release. Where the Registry accepts separate init metadata and staged tar files, validation
must use both and compare them.

Expected publish pipeline, adjusted for actual existing endpoint design:

1. Authenticate and authorize the requesting publisher/namespace.
2. Create a staged publish session with provisional identity/version/kind/standard metadata.
3. Accept/upload intended artifact byte(s) through existing Stage 1-compatible release path.
4. Before finalization, read the **embedded manifest** from the staged release payload and validate its
   structure, APDS declaration and applicable semantic constraints independently of the client.
5. Compare staged payload manifest to requested publication identity (local name + namespace mapping), kind,
   version and standard metadata; reject mismatch even if both individually appear valid.
6. Verify integrity and signing/attestation policy under their **separate** contracts, plus malware scanning
   and permitted artifact targets where applicable.
7. Atomically finalize the release and make it discoverable only after all mandatory gates pass.
8. Return existing Registry URL and accurate release identity on success; preserve current CLI success
   output behavior.

No privilege boundary should be weakened by APDS changes. A conformant manifest does not override private
namespace ACLs; a signed payload isn't automatically conformant; a clean scan is not a conformance success.
Server-side validation should handle malformed/corrupt/tar-traversal payloads according to existing secure
tar handling, not attempt to parse arbitrary paths as the manifest.

**Concrete negative publication matrix** (new release unless explicitly labeled legacy):

| Attempt | Required server behavior |
|---|---|
| Missing `standard` | Reject at server; actionable response; no published version |
| Unknown `standard.id` or version | Reject unsupported standard without fetching caller-supplied schema |
| Valid metadata but tar's `agent.json` declares another version | Reject mismatch at finalization |
| Valid metadata but tar's `agent.json` is another kind | Reject mismatch |
| Payload manifest has invalid schema or semantics | Reject even if CLI claims it passed lint |
| Duplicate finalize or failed scan | No partial discoverable record; preserve idempotence/atomicity as applicable |
| Current released old version lacks `standard` | Preserve immutable bytes, download/install and historical references |
| Old publisher sends new version after enforcement | New release must conform; identity age is not an exemption |
| Pending upload exists at deployment cutoff | Deterministic documented policy; never trust client clock or silent bypass |
| Multi-artifact Tool release | Every required artifact/embedded manifest association checked; preserve Stage 1 integrity model |

**Migration/open implementation choice:** Whether a pending pre-cutover session gets a bounded grace
completion or a forced restart is for Codex/Claude to recommend after examining transactional behavior.
Requirement: server-controlled, observable, consistent, security-preserving; no silent endless bypass or
mutation of previously finalized versions. Roll out CLI/Registry in compatibility-safe order.

#### Install and lockfile v4: concrete reproducibility contract

The lockfile is the Manager's reproducibility record, not an APDS manifest. Stage 1 introduces v4 for
Python resolution and new release integrity, and fixes forward-version rejection. Stage 2 should **prefer
extending v4** with an effective standard identity plus origin rather than introducing a new lockfile major
version as a reflex.

Recommended conceptual locked entry (field names/placement flexible, not a guaranteed current serializer):

```json
{
  "kind": "agent",
  "name": "@publisher/sample-agent",
  "version": "0.1.0",
  "integrity": "<verified logical package digest>",
  "standard": { "id": "agentpm", "version": "1.0.0" },
  "standard_origin": "explicit"
}
```

If `integrity` currently holds a concrete SHA-256 release digest, do not change that meaning or duplicate
it with conflicting siblings; follow Stage 1's final definition. The illustrative snippet is **not**
permission to write target-specific installed artifact state into source-controlled `agent.lock`.

**Clean workspace/frozen behavior matrix:**

| Situation | Correct result |
|---|---|
| Direct Agent Package with no dependencies | Install the Agent itself; no attempted empty resolve crash |
| Direct Skill with transitive Tool | Install Skill and Tool closure with correct kinds and exact locked versions |
| Existing complete lock, `--frozen` and clean `.agentpm/` | Reproduce entire pinned graph, not only direct root |
| Locked dependency version does not satisfy authored selector | Fail, don't silently choose a new version |
| Lock says one SHA, Registry/download returns another | Fail integrity before updating local state |
| Two Agents reference same Component name at different versions | Preserve both relevant locked identities/graph edges and select the right one at run time |
| Unsupported APDS version in selected artifact | Explicit unsupported handling, not default-to-v1 |
| Older CLI encounters newer unsupported lock version | Fail safely, never deserialize and rewrite away unknown fields |
| Legacy release has no standard in its stored manifest | Install as legacy, annotate inferred interpretation without falsifying verified declaration |
| Regular `install` regenerates lock | Preserve standard+origin through resolve DTO, not dropped as unrecognized metadata |

`--frozen` must not use an assumption that every direct root is a Tool or that all selected versions are
simply their latest. Distinguish the installed artifact contents, lock graph, Registry metadata and local
runtime environment. Exact selected package version and package kind drive identity; target-specific Python
runtime selection remains local machine state under Stage 1's model.

#### Runner support, compatibility and readiness — decision table

| Package context | APDS result | Harness compatibility | Local readiness | Intended UX |
|---|---|---|---|---|
| Minimal Agent with no Loop | Conformant (intrinsic) | Not runnable by current Harness | Not evaluated | Explain that a Loop is needed **for Harness**, not APDS |
| Valid Agent with supported Loop but no model selected | Conformant | Supported | Not ready | Noninteractive preflight fails and points to provider/model configuration |
| Valid Agent with supported Loop/model configured | Conformant | Supported | Ready if remaining dependencies/services satisfied | Allow run; trace truthful capabilities |
| Valid Agent declared with unsupported APDS version | Unsupported by Runner | Unsupported | Not evaluated | Clear standard-version diagnostic and support information |
| Legacy Agent interpreted through supported adapter | Inferred legacy interpretation | Potentially supported | Evaluate normally | Explicit inference; don't display APDS verified |
| Valid Agent uses unsupported backend requirement | Potentially conformant | Runner feature-specific mismatch | Not ready/unsupported depending on feature | Report capability gap; don't claim APDS invalid |
| Valid Agent with binding to wrong resolved version | May be structurally fine | Contract must use pinned version | Not safe to run until corrected | Fail or repair explicit resolution, never arbitrary name match |

For Runner capability advertising, **reuse** existing Harness machine `initialize`/preflight result and SDK
bridge. Avoid creating a second protocol or an entirely new Runtime interface. The supported standard list
should be machine-readable and stable enough for callers to make a go/no-go decision; CLI/TUI may summarize
it. Check that headless preflight does not assume interactive bootstrap can ask for a missing
provider/model.

**Prompt description experiment:** Choose a small deterministic fixture/run set representing at least a
minimal agent and a more complicated multi-phase agent. Compare prompt shape and behavioral usefulness with
and without top-level Agent `description`; inspect whether it conflicts with authored Profile/phase
objective. Record samples, outcome assessments, token/cost impact, and final decision. If no convincing
benefit, keep current prompt behavior. No change to APDS normative requirements.

#### Stage 1 contracts to respect exactly

- Stage 1 M8 Tool scaffold/lint validity and M9 CLI output quality are preexisting work; avoid overwriting
  their changes when modifying init and help.
- Stage 1 M11B's lock v4 and future-lock guard are authoritative, including that source-controlled lock
  entries hold logical identities, not host-target selections.
- Stage 1 M13A/B multi-artifact release storage/upload/finalization and M14A–C
  integrity/signature/attestation define the release evidence surface used by Stage 2. Note the split:
  **M14A** defines the release-level digest and canonicalization (what install/lock integrity depends on),
  **M14B** the release-level signature statements and stored-byte verification, and **M14C** the root-signed
  registry key set plus client-side verification. A *verified* registry-attestation Health signal is only
  possible once M14C is deployed; before that the signal reads unknown/not evaluated.
- Stage 1 M15 target-aware installer/runtime provisioning must be reused, not reimplemented in an APDS installer.
- If Stage 1 is in flight, add a compatibility plan and stage branch/PR coordination before touching shared
  DTOs, migrations or CLI snapshots.


### 8. Package Health and evidence model

**REQUIRED:** An objective evidence model, **not** a weighted quality score, with universal and optional
kind-specific signals. Distinguish **package identity** facts (name, publisher, total installs/stars) from
**specific published version** facts (digest, signing, scan, APDS, dependencies, compatibility). Show
source/evidence or explain absence; never fabricate verification.

**Universal candidate signals:** declared APDS ID/version; verified/legacy-inferred/unresolved conformance;
artifact/release digest/integrity; author signature state; Registry attestation/verification; malware scan
result; release recency; deprecation where genuinely supported. These are present only where the backend
can authoritatively support them.

**Kind-specific candidate signals:** Tool OS/architecture/runtime/target portability; Agent Package
dependency graph and potential Runner compatibility (not local readiness); Knowledge
context/vector/embedding data compatibility; Memory Blueprint record/space/operation contract requirements;
Skill declared Tool/compatibility metadata; Profile constraints/capability hints as advisory; Loop
structural graph semantics; Template supported scaffolding/execution surfaces. The shape must support *not
applicable* without making all kinds pretend to support the same checks.

**Recommended presentation:** concise version-level summary near detail-page identity, deeper objective
evidence/definitions in Health/Security; share relevant primitives across kinds. Decide whether it is a
dedicated Health tab or an expanded Security tab after inspecting Stage 1 shared shell. Signatures do not
prove safety; malware-scanned does not prove safe; conformance does not prove compatibility; compatibility
metadata is not enforced unless the relevant code actually enforces it. Status vocabulary distinguishes
verified/passing, failed, unavailable/unknown, not evaluated/incomplete, not applicable, advisory. Avoid
universal score, ungrounded badges or broad quality claims.

### 9. Design system and mockup interpretation

**Current production website is the default visual-design reference** for existing colors, typography,
brand marks, elevation and established patterns, and the mockups do not override it by merely depicting
something. Refine for coherence, and adopt a mockup styling treatment only where the D21 adoption review
deliberately chooses it — never wholesale, and never by replacing the site with a flat, edge-to-edge,
blue-only dashboard simply because AI mockups did so. Preserve floating/layered
surfaces, generous whitespace, subtle gradient/depth, rounded cards, kind icons and compact
developer-centric code areas where useful.

**Provided visual concepts (bundled relative to this spec):**

- [`assets/landing-structure.png`](assets/landing-structure.png): homepage hierarchy, section order and
  card density, **not authoritative style/copy/metrics**.
- [`assets/detail-structure.png`](assets/detail-structure.png): composition and execution-architecture
  organization, **not authoritative data**.
The three `-layered` images below are **one site-wide redesign sharing a global shell** — read them
together, not as three page comps — and they are a **guide weighted toward layout**, not a design to
reproduce 1:1. Some of their styling is worth taking; **D21** decides which, element by element, before the
shell is built (§9.1). Copy and every displayed value are excluded from adoption in all cases.

- [`assets/landing-layered.png`](assets/landing-layered.png): homepage structure — hero with a featured
  Agent Package card and tabbed command block, the category explainer row, the Components row, and the
  three starter paths. Also carries a stronger layered/floating treatment whose adoption is a D21 call.
- [`assets/detail-layered.png`](assets/detail-layered.png): the detail route — identity card with a trust
  badge strip, a three-action get-started panel (Install / Run with Harness / Load via SDK), the tab bar,
  and the Composition + Execution architecture two-column body. Depth and whitespace treatment is a D21
  call; the data shown is not contract-accurate.
- [`assets/explore-layered.png`](assets/explore-layered.png): Explore — persistent faceted rail carrying
  the Agent Package / Components / Templates / Namespaces hierarchy, a curated Featured row explicitly
  labeled separate from Trending, and per-kind result cards.

**Do not copy mockup inventions** (fictional stats, dates, usage counts, version values, model
integrations, false APDS verification, `"apds"` field instead of `"standard"`, fictional backend
categories, false “production ready” claims). Implement with real manifests/Registry data;
conditional/empty/legacy states. Responsive and keyboard/mobile behavior matter at least as much as the
desktop snapshot.

### 9.1 Global layout system — decide once, apply progressively

**REQUIRED.** The three `-layered` mockups are a **site-wide redesign**, not three independent page
comps. They share a global shell, and several of those elements appear on every route. Once the first page
adopts them the rest are effectively committed, so the shell must be **designed and built once, before**
any page-level milestone consumes it — not re-derived per page and reconciled later.

**The mockups are a guide, weighted heavily toward layout — not a design to reproduce 1:1.** Nothing in
them is adopted automatically, and that cuts both ways: their strongest contribution is **structure,
layout, hierarchy, component inventory and interaction model**, but they also contain **styling ideas worth
taking selectively**. Selective styling adoption is explicitly allowed; what is not allowed is adopting
anything *by default* because it appeared in an image.

Every element is therefore a deliberate call — **adopt / adapt / reject** — recorded with a one-line reason
before the shell is built (see **D21** and M8B.1). Three things are never read off the images regardless:
**copy**, which §12–§15 and M14A/M14B settle, and **every number, name, date, count and badge**, which must
come from real data. The production site remains the default for brand identity, and a styling element is
adopted only where the review deliberately chooses it over the current treatment.

**Global elements shared across all three mockups** — these are the locked-in surface:

| Element | What the mockups establish | Appears on |
|---|---|---|
| **App shell / header** | Floating rounded header inset from the viewport edge, not full-bleed. Logo + primary nav (Explore, Docs, Pricing, Blog) + global search with ⌘K affordance. Anonymous variant ends in Sign in / Sign up; authenticated variant ends in notifications + avatar menu | Every route |
| **Page canvas** | Soft tinted/gradient background with white rounded card surfaces floating on top; centered max-width column with generous outer margin | Every route |
| **Section card** | White rounded card with an icon chip top-left, title, one-line subtitle, optional right-aligned action link. The single most repeated primitive in the set | Every route |
| **Kicker + headline** | Small uppercase letter-spaced kicker above an outsized headline with a supporting paragraph | Landing, Explore |
| **Kind token system** | One icon + tint + label per kind, used identically in nav filters, cards, composition nodes and capability chips | Every route |
| **Package card variants** | Hero/featured, 3-up featured, 2-up search result, and composition node — one data model, several densities | Every route |
| **Command block** | Monospace block with a copy affordance; tabbed on the landing hero, stacked rows on detail | Landing, detail |
| **Trust/evidence row** | Badge strip on the identity card plus a labeled status list with per-row evidence links | Detail |
| **Footer** | Logo + tagline + link row + social icons, with a compact variant | Every route |

Two layout decisions worth naming because they are structural, not cosmetic: **global search moves into the
header** on every route, and the detail page uses an **asymmetric two-column grid** (identity left /
get-started panel right, then Composition left / Execution architecture right). Explore uses a persistent
left facet rail. These grids should come from one shared layout system.

**Already settled — not open to D21.** Two visual systems are shipped, deliberate, and **preserved as-is**;
the mockups depict different ones, and those depictions are rejected by default rather than being an open
question:

- **Per-kind color.** `agentpm-web/src/components/ui/Badge.tsx` defines the tone system, and the shipped
  kind assignment is **Agent → amber, Tool → indigo, Skill → rose, Knowledge → fuchsia, Memory → teal,
  Profile → cyan, Loop → lime, Template → emerald** (with `sky` reserved for the cross-cutting `Signed`
  badge). The mockups assign different hues to the same kinds; adopting them would break recognition for
  existing users for no gain. Keep the shipped assignment. The tone styling is already dark-native
  (`bg-{c}-500/15 text-{c}-300 border-{c}-700/50`), so it carries into a dark theme without rework.
- **Per-package generated identity.** `components/ui/ToolBox.tsx` derives a deterministic two-stop gradient
  from a hash of the package identity (FNV-1a seed → mulberry32 PRNG → two HSL stops), giving every package
  a unique, stable avatar with no authored artwork. It is used in 17 places. The mockups replace it with
  generic per-kind glyph chips and a stock avatar, which is a **downgrade** — it discards free per-package
  recognition. Keep the generated system.

**The two systems are complementary, not alternatives — which is what the mockups get wrong.** The
generated gradient answers *which package is this*; the kind tone and label answer *what kind is it*. The
mockups collapse both into a single per-kind glyph used as the avatar, which conveys the kind twice and the
identity not at all. In any identity context, kind is carried by the tone and label **alongside** the
generated avatar, never by replacing it.

That gives a simple test for which mark to use:

> **Does this stand for a specific package, or for a category of packages?**
> A specific package → **generated identity gradient**. A kind as a category, filter or legend → a
> **per-kind glyph** is reasonable, and is often the better choice at small sizes.

Indicative, not yet locked: identity contexts include the Agent Package Card, Explore result cards, the
detail-page identity block, and the nodes of the detail-page composition chart — all of which name a
specific package and keep the generated avatar. Category contexts include the Explore facet rail, kind
filters and legends, where no specific package is being named and a compact glyph reads better. Settle the
edge cases during M8B.1 by applying the test above rather than by listing surfaces exhaustively here.

If a per-kind glyph set is introduced for those category contexts, it is a **small, deliberate addition**
scoped to them — not a replacement for the generated system and not a license to restyle identity surfaces.
The mockups' glyphs may inform it, subject to D21.

The remaining work here is **consolidation, not invention**: the kind→tone mapping is currently hardcoded in
eight separate card components. M8B.1 centralizes the existing values into one token source; it does not
choose new ones.

**Design adoption review (D21), before the shell is built.** M8B.1 opens with an explicit pass over the
three `-layered` mockups, element by element — the global ones in the table above plus the notable styling
treatments that are genuinely open (surface elevation and shadow depth, corner radii, the background
gradient/tint, card border and divider weight, spacing rhythm and density, badge and pill *shape*, the
monospace command-block look, type scale and weight contrast). Per-kind color and the generated
per-package identity are **excluded from the review** — they are settled above. Each gets **adopt**, **adapt** (take the idea, express
it in current tokens) or **reject** (keep today's treatment), each with a one-line reason. Zack reviews and
signs off on that list before implementation starts. The signed list, not the images, is what the build
follows and what review checks against.

**Sequencing requirement.** Milestone 8B.1 owns the shell and the shared layout system. Every page-level
milestone — M8A, M9A, M9B, M10A, M10B, M12A, M12B, M13A, M13B, M14A, M14B — **consumes** it and MUST NOT
fork its own header, footer, page canvas, section-card or kind-token implementation. Adopting the layout
progressively, route by route, is expected and fine; shipping two competing shells is not. If a route must
ship before 8B.1, say so explicitly in its milestone evidence and schedule its migration.

**One observation, not an instruction:** `detail-layered.png` merges Health and Security into a single
`Health & Security` tab. That is useful evidence for **D12**, which asks whether Health is a dedicated tab
or an expanded Security tab — but the mockup is not the decision. Decide it against the Stage 1 shared
shell and record it.

### 10. Agent Package and Component detail pages

- Agent Package Overview must **lead with human-readable identity, composition and execution
  architecture**, not a raw manifest dump. Show direct and transitive/Skill-inherited Component
  relationships, **exact resolved versions**, identities and links; preserve detailed Bindings, Examples,
  author README and Security inspection. If no Loop or few/no Components, show an honest useful
  empty/limited state rather than implying error or inventing phases.
- Where a Loop exists, render meaningful phases, ordered/graph transitions, scoped capabilities and access
  restrictions using the **actual resolved graph**. Avoid falsely equating a simple three-column
  visualization with an always-linear Loop; support branches, cycles, terminal outcomes or offer readable
  textual fallback.
- Clearly separate selected release version from identity-level popularity/stars. Present explicit standard
  id/version prominently; provide **two distinct affordances**: view actual immutable `agent.json` from
  this version, and view **the corresponding APDS version specification**. Keep raw source accessible and
  inspectable, but secondary by default.
- Conditional actions: **Install**, **Run with Harness** only where applicable / explanatory when
  unavailable, **Load via SDK** where supported. Preserve portability by not implying Harness is mandatory.
- All specialized kinds retain their meaningful existing data: Knowledge modes/corpus/index and query
  usage; Memory spaces/record/lifecycle/governance; Profile identity/audience/communication/boundaries;
  Loop graph/phase/error policy; Skill entrypoint/refs/scripts; Tool runtime/IO; Template
  variables/dependencies/entrypoints/use case/execution surfaces. Share shell/Card/Health primitives but do
  **not** flatten specialized views.
- Publisher-controlled README prose is **not** a product taxonomy authority; don't rewrite it or
  auto-assume descriptions/compatibility claims are verified.

### 11. Registry hosted APDS reference

- Provide a stable human-readable route for a supported standard, **prefer** `/standards/agentpm/1.0.0`
  (exact IA may be refined). It must resolve the **pinned immutable** v1.0.0 schema, semantics and fixture
  references, including authoritative source links, and never fetch arbitrary package-supplied URLs at
  runtime.
- The standard identifier and *verification result* appear as distinct signals on detail pages. For legacy
  release with only inferred interpretation, indicate inference; do not paint it “APDS verified”.
- A separate broader category explainer **prefer** `/agent-package-management` and links to the normative
  spec. Homepage should sell AgentPM the product rather than act like an independent standards-body
  homepage.

### 12. Homepage and category story

- Start with developer reuse/friction problem, AgentPM solution and complete Agent Package near hero; avoid
  introducing six Component kinds as isolated primary products first.
- Early meaningful demo: **one real Agent Package** card showing identity, composition, selected version
  and install/run path, preferably curated from Registry (not hardcoded package IDs). Defer choice of
  example to data/curation; treat initially missing featured content gracefully.
- Explain **Agent Package → reusable Components → Agent Package Management → Agent Package Manager
  (AgentPM) → compatible Runner (Harness)**. This is a mental model, not an arbitrary required literal
  section order.
- Reduce long repeated trending-by-kind feature sections; move broader catalog/discovery content into
  Explore. Preserve small Component/Template entry paths.
- Include three clear starter paths and accurate CLI examples; do not claim universal runtime compatibility
  without evidence. Avoid a hero dominated by zero/weak traffic counts; retain useful analytics elsewhere
  if Stage 1 supplies them.
- Keep AgentPM as star, category education supporting it. No forced brand refresh. Use semantic HTML,
  responsive design, accessibility and graceful loading/error/empty states.

### 13. Explore, namespace, search cards and curation

- Stage 1 owns search/filter/relevance/pagination/ranking/stars/Trending/namespace pins. Stage 2
  **reorganizes labels, counts and result-card meaning** without breaking URLs, cursor behavior, visibility
  or deterministic search order.
- Faceted Kind hierarchy: **Agent Packages** → **Components** (Tools, Skills, Knowledge, Memory Blueprints,
  Instruction Profiles, Loops) → **Templates** → **Namespaces**. Top-level grouping need not imply new
  backend kinds. Rename product-controlled “Agents” to “Agent Packages” only for artifacts; retain “Agent”
  for runtime references.
- More informative result cards for all kinds: Agent composition summary; Tool runtime; Skill capabilities
  and Tool relationships; Knowledge mode/corpus; Memory retrieval/spaces; Profile role; Loop phases;
  Template stack/use case. Do not invent metadata; use available fields or omit gracefully.
- `/explore` with no search becomes useful **data-driven** discovery (featured Agent Packages/curated
  sections, trending, Components, Templates, etc.). Curated **Featured** is not measured **Trending** and
  never inherits dependency metrics. Relevant text queries continue to rank by Stage 1 rules, not boosted
  solely because `kind: agent`.
- **Data-driven featured content:** Allow editorial selection/order/placements without rewriting React
  presentation components. **PREFERRED** lightweight configuration or Registry-backed curation based on
  Stage 1 primitives, not full CMS. Eligibility/visibility checks prevent private, missing, unpublished,
  deleted or otherwise unavailable entries leaking; no user-controlled arbitrary references that bypass
  authorization. Seed actual examples later; fixtures/mock data for UI/tests now.
- Namespace pages group Agent Packages/Components/Templates, preserve featured **any-kind** publisher pins,
  search/navigation and Recent Activity, improve signing/namespace accuracy and breadcrumb/footer language.
- Conditional no-result/empty states may suggest copyable AgentPM Developer prompts **when that Agent
  exists**; until then, don't wire dead CTAs or pretend it is already available.

### 14. Agent Package Card and distribution experiment

- Establish a reusable **Agent Package Card** visual language (purpose, publisher, identity, version,
  composition, authored standard identifier, appropriate trust metadata and install/run or detail action);
  smaller UI variants may share data/presentation primitives across homepage/Explore/detail/OG.
- **The homepage hero card is this Card, shown at full fidelity** — not a decorative hero graphic. Treat the
  landing placement as the showcase for the shareable artifact: the same component and data model that
  Explore, detail, OG previews and the D16 README-embed investigation reuse. That is why it carries more
  content than a typical hero element — a shareable card has to stand alone when it appears somewhere with
  no surrounding page. The density tradeoff is therefore a deliberate Card-design question to settle in
  M8B/M11B, not a hero-layout accident: decide what the Card must say unaccompanied, then let the homepage
  show that. Distribution is the point — this is a primary channel for explaining Agent Package Management
  to people who have never visited the site.
- Cards must not claim current Runner readiness without local preflight. Distinguish identity-level
  engagement from version metadata; show only verified evidence.
- **INVESTIGATE and document** embeddable GitHub README image/badge/card: Markdown copy snippet, generated
  image versus static variant, pinned versus latest, caching/invalidation, relative availability, image
  alt/accessibility, links to canonical Registry detail, auth/private packages, integrity/presentation of
  Health/APDS, technical SEO/social OG reuse. Prototype if cheap; decide full implementation/defer with
  Claude review.
- The name **Agent Package Card** is human-facing Registry/share UI, not the unrelated machine-readable A2A
  “Agent Card” protocol.

#### Expanded Package Health and website reference

Concrete presentation guidance and evidence rules, not permission to invent unsupported metadata.

#### Package Health evidence contract — authoritative, advisory, not applicable

Package Health is **evidence presentation**, not reputation. It must not compute a universal score,
automatically certify safety, or encourage comparison between inherently incomparable artifact kinds. Use a
small cross-kind evidence model with kind-specific extension data. A versioned Tool's target compatibility
cannot be replaced with an Agent's current local Harness-readiness flag; these are different kinds of
claims.

**Proposed evidence record attributes** (a design recommendation; map to established DTO patterns):

- `signal_key`: stable identifier such as `apds_conformance`, `author_signature`, `malware_scan`, or
  `tool_target_support`.
- `scope`: artifact identity, selected published release, selected release artifact, or observed local
  environment. Only the appropriate scope may be shown on a public release page.
- `status`: verified/passed, failed, unknown, unavailable/not evaluated, not applicable, advisory/declared,
  or an appropriately precise existing vocabulary.
- `value`: human-readable summary derived from real data, not copy-generated truth.
- `evidence`: source (Registry verification, signature verification, scanner run, actual compatibility
  metadata, pinned standard validation), timestamp/checksum/report link if available and safe.
- `applicability`: kind/version and any target/runtime requirements controlling whether this check is meaningful.
- `explanation`: what this check **does** and **does not** establish.

**Signal ownership and boundaries:**

| Signal | Best source | Scope | What it cannot prove |
|---|---|---|---|
| APDS declared standard | Actual selected version manifest | Release | It was validated or is runnable |
| APDS conformance | Registry's independent validator/result + validation level | Release/graph context | Safety or quality |
| Legacy inferred APDS meaning | Explicit Registry/Manager compatibility adapter | Historical release | That original author declared a standard |
| Digest/integrity | Stage 1 release/artifact digest verifier | Release or specific target artifact | Absence of malicious behavior |
| Author signature | Stage 1 trusted-key verification | Release | Package behavior is safe |
| Registry attestation | Stage 1 **M14C** attestation verification against the root-signed registry key set | Release | End-user approval of content |
| Malware status | Scanner result, version/time/method where available | Release artifact | Comprehensive malware-free guarantee |
| Tool platform target | Stage 1 target classifier and actual available artifact | Release artifact/target | Other machines are runnable |
| Dependency state | Resolved graph with pinned selected versions | Contextual composition | Runtime credentials/backend readiness |
| Recency | Release timestamps | Release or identity with clear labeling | Quality, support, or stability |
| Stars and installs | Stage 1 identity-level aggregates | Identity | Trust or Health |

**Kind-specific evidence design:**

| Kind | Relevant details, if actually supported | Cautions |
|---|---|---|
| Agent Package | Declared dependency closure, applicable Loop, compatible APDS/Runner family | Never show local readiness based on anonymous server metadata |
| Tool | Node/Python runtime, target OS/arch, interpreter/dependency constraints, packaged artifact | Stage 1 determines compatible artifacts; no false universal platform badge |
| Skill | Entrypoint/references/scripts present, declared Tool deps, compatibility hints | Do not imply scripts auto-run or are scanned/evaluated beyond actual gates |
| Knowledge | `context`/`vector` mode, corpus/vector/index metadata and hashes if verified | Embedding provider availability and quality remain external |
| Memory Blueprint | Declared spaces/record types/operations/triggers, schema integrity | No implied actual persistent backend/storage execution |
| Instruction Profile | Declared identity/objective/communication and advisory compatibility | Boundaries are guidance, not enforcement/safety certification |
| Loop | Graph/intrinsic rule validation, phase/transition coverage | Does not alone make a full Agent Package runnable |
| Template | Declared stack/dependencies/variables/execution surfaces | Scaffold compatibility is not Agent execution |

**UI state examples:**

- `Signed` with verified signature can say exactly which key/author verification applied; `Signature
  supplied, not verified` is materially different.
- `Scanned` should show scan status and if possible when/version; `No result` is not the same as `Clean`.
- A historical legacy release can show `APDS interpretation: agentpm / 1.0.0 (inferred)` while `APDS
  conformance: not verified`; never display a verified badge on inheritance alone.
- An Agent Package with no Loop can show `APDS conformant` and `Harness: not runnable` concurrently; this
  is expected, not contradictory.
- A Profile's OS/architecture Health indicator should normally be **not applicable**, not an empty failed
  hardware compatibility check.
- A Tool that supports macOS arm64 but not Linux x86_64 needs target-specific evidence, not an unqualified
  `Compatible` badge.

**Health UI preference:** retain a compact evidence summary on the common detail shell
(selected-version-scoped), and use a Health/Security deep inspection area for provenance, compatibility,
conformance/legacy nuance and definitions. Reuse Stage 1's Security primitives; don't replace a good
detailed Security tab with a generic green/check display.

#### Agent Package detail page: developer's reading order

The first screen should answer: *What is this? What does it do? What does it contain? How do I try it? Can
I inspect and trust this version?*

1. **Identity area:** category `Agent Package`, name with namespace, purpose, owner, selected version,
   license, identity-level stars/downloads. Version timestamp, digest and release Health are visually
   separate.
2. **Primary actions:** copyable `agentpm install @namespace/name@version`; conditional Harness run; `Load
   via SDK` with actual SDK syntax. Do not surface a dead Harness CTA on nonrunnable packages.
3. **Overview, first substantial block:** a composition depiction of exact direct dependencies and
   nested/Skill-inherited dependencies. Show type icons, names, exact resolved versions and links. If
   installed graph/lock is missing from Registry, qualify the view rather than fabricating resolution.
4. **Execution architecture:** if an actual Loop exists, show its phases and scoped capabilities,
   transitions, access restrictions and meaningful edge labels. For branches/cycles offer graph/layout or
   readable transitions table; never force a false three-step linear sequence.
5. **Standard card:** declared APDS ID/version and status separately, `View agent.json`, `View APDS
   specification` resolved to that exact version. Legacy is labeled inferred.
6. **Health summary:** objective evidence plus drill-down, not generic quality score; relevant kinds and
   unknown states.
7. **Examples:** author-supplied prompts with copyable text, optional configure/run directions. Examples are
   not evals or certified outcomes.
8. **Raw manifest:** accessible full immutable source (with version and provenance), visually secondary to
   human-readable Overview. Preserve author README and existing specialized technical tabs.

**Direct vs transitive visualization**: direct packages are those declared by the Agent;
inherited/transitive packages come from their dependencies, e.g. Tool T declared by Skill S. Do not draw an
unlabeled arrow suggesting T is a direct Agent dependency. If a Skill declares multiple Tools or versions
overlap, display complete paths or an expandable nested group. Large graphs require scalable collapsed
view, not a huge unbounded SVG.

**No Loop / empty state**: a zero-dependency Agent Package is still an inspectable artifact. Show purpose
and `No dependencies declared` or `No Loop bound`, with a clear statement that Harness requires a
compatible Loop; don't render broken cards or mark APDS invalid. If unknown graph/legacy content, expose
missing evidence honestly.

**Specific page/regression map**: current Agent pages have Overview/Bindings/Examples/Readme/Security;
maintain route behavior and deep links. Existing specialized Component pages include Knowledge
retrieval/corpus metadata, Memory lifecycle and record contracts, Profile identity/communication, Loop
phase graph, Skill references/scripts, Tool execution/IO, Template entrypoints/variables/surfaces. An
architectural redesign that removes these panels is a regression even if the shared shell looks cleaner.

#### Homepage narrative — example story, not locked copy

The homepage currently starts with package/component sections because the product grew kind by kind. This
is the wrong conceptual order for Agent Package Management. It should introduce AgentPM the product while
teaching the new category through one concrete example.

**Recommended narrative skeleton:**

1. **Hero: problem plus the Manager.** Explain that complete agent systems are difficult to package, version
   and reuse across applications, frameworks and hosts; AgentPM supplies Agent Package Management. The
   heading explicitly identifies AgentPM as an Agent Package Manager, not merely a Tool installer.
2. **Near-hero real package:** a visually strong Agent Package Card illustrating `@namespace/agent`,
   version, purpose, direct Components, and install/Harness path **only if runnable**. Use a data-driven
   curated package; don't bake fictitious examples into JSX.
3. **System explanation:** compact visual demonstrating `Agent Package`, `Reusable Components`, `APDS
   definition`, `AgentPM Manager/Registry`, `Harness or other compatible Runner`. This should be
   explanatory, not a promise of universal interoperability with named hosts.
4. **Why this is different:** compared to copying Git repos, framework-specific projects, host
   configurations, ad hoc tool packages and internal infrastructure, a reusable composed Agent system has an
   inspectable dependency/version contract.
5. **Explore/Featured:** a small tasteful selection of curated Agent Packages, not six repetitive per-kind
   Trending tables. The default Explore page handles larger discovery.
6. **Components and Templates:** clearly subordinate paths; six Component kinds and separate Templates with
   one-sentence purposes and discoverable links.
7. **Three onboarding journeys:** Try an Agent Package, Build your own, Start from a Template. Each journey
   has a working destination and actionable next step; no wizard required.
8. **Trust/technical links:** where appropriate link APDS spec, Package Health and docs; don't let the
   landing page become a standards body homepage.

The full page may need to be reordered to fit existing hero codebox/design primitives. Preserve effective
current styling: actual brand colors, typography, soft gradients, cards floating with elevation, rounded
corners, generous whitespace and distinctive kind icons. Both mockup sets are **layout references only**,
not authorities for design tokens or claims. Aim for a deliberate systems/product feel rather than generic
startup AI imagery.

#### Explore and curated discovery mechanics

The no-query Explore experience is **not** simply a search results list with an empty query. It can
deliberately contain featured Agent Packages, measured Trending, selected Components and Template pathways.
Search-with-query remains a relevance-driven results view using Stage 1's ranking and cursor behaviors.
This distinction should be explicit in UX and tests.

**Two pipelines:**

| Pipeline | Source | Semantics | Expected content |
|---|---|---|---|
| Featured/curated | Authorized editorial config or Registry-backed curation | Human selection, explicit order/placement | Agent Package showcase and pathways |
| Trending/measured | Stage 1 deterministic ranking on eligible results | Popularity signal, not quality; private-safe | Discover current popular content |

Curated content must resolve through public/authorized visibility and selected-version availability. A
configured private/unpublished/yanked/missing item should not leak metadata, break rendering or produce a
client-side inaccessible link. Preserve placement order after filtering invalid items; provide a sensible
alternate UI when nothing is eligible. Distinguish **publisher namespace pins**, which Stage 1 owns, from
global featured items, which Stage 2 may manage via a separate lightweight source.

**Default Explore hierarchy:** Agent Packages as primary kind, six Component kinds nested/grouped,
Templates as project scaffolds, Namespaces as publishers. This hierarchy is a label/presentation grouping,
not new persisted `kind` enum members. Filtering must retain URL shareability and correct result counts.
Search results should not be artificially pinned to Agent-first order when a Tool is a stronger text match.

**Card content samples by kind** (examples of useful facts, not mandatory all fields):

| Kind | Suggested preview facts | Callout to avoid |
|---|---|---|
| Agent Package | Purpose, Component category counts, Loop present, version, one useful use case | `runnable` without actually checking Runner contract |
| Tool | Node/Python and supported target where declared, one-line capability | `works everywhere` absent evidence |
| Skill | Human-readable skill purpose, Tool dependency or entrypoint info | Skill script autoexecution implication |
| Knowledge | `context`/`vector` and corpus/source count where available | Imagined index/accuracy score |
| Memory | Space/operation categories, retrieval modes | Memory is already persisted |
| Profile | Role/objective/tone, model hint if authored | Enforced safety boundary |
| Loop | Phase count, archetype, flow summary | Always-linear three-step graph |
| Template | Use case, stack, execution surface, scaffold action | Same thing as an Agent Package |
| Namespace | Publisher identity, curated pins/public catalog | Newly invented reputation score |

The same shared card data/view primitives should be considered for result lists, publisher catalogs,
homepage features, social previews and future embeddable cards, but one giant over-generalized component
isn't a requirement.

#### Namespace, pricing and SEO specifics

**Namespace publisher pages:** emphasize `Agent Packages`, `Components`, `Templates` as sections/facets;
retain Stage 1 owner/admin pins for **any** kind and namespace-scoped filtering/pagination. Better category
headings/counts should not cause duplicate search implementations. Recent activity rows should link to
useful publisher release info; maintain signing-policy distinctions and identity versus version facts.
Refine footer/breadcrumb labels instead of mechanically replacing every occurrence of `package`.

**Pricing:** maintain Free ($0), Pro ($7) and Team ($19) plan structure and real Lemon Squeezy billing.
Explain public Agent Packages and Components as free publishing/installing and private namespaces as paid
capability. Team copy should describe shared internal distribution/team governance without claiming a
dedicated private Registry instance. Anonymous users should have a clear route to explore public packages.

**SEO:** Stage 1 owns SSR metadata, canonical URLs, sitemap/robots, OG social previews and query URL crawl
controls. Stage 2 owns user-facing meaning and exact words—titles, H1/H2, category anchors, descriptions,
internal links—based on this taxonomy. Category claims remain hypotheses; avoid `industry standard` or
`works with every Runner` assertions. Structured-data/schema.org expansion remains optional after semantics
stabilize. A card preview rendered from the exact selected public release should not display private or
fictional metadata.

**Versioned APDS page:** if `/standards/agentpm/1.0.0` is adopted, serve human-readable overview plus
canonical immutable schema, semantics and fixtures links; no runtime arbitrary external URI fetch. This is
different from a broader `/agent-package-management` explanatory page, which teaches motivation and market
alternatives while focusing on AgentPM as the product.


### 15. CLI help and first-use journeys

- Top-level `agentpm --help` **explicitly says “Agent Package Manager”** and introduces **“Agent Package
  Management”**. Example short positioning: “AgentPM — Agent Package Manager” with lifecycle and first-use
  examples. Avoid overstuffing help or changing command interfaces merely for marketing.
- Clarify `init` default Agent Package, `new` project Template, `run` individual Tool execution, `harness`
  Agent Package execution/reference Runner, `install` resolved/locked dependencies, `lint` APDS, `publish`
  Registry lifecycle, and when SDK loading applies. Respect Stage 1 error/output hardening and existing
  machine-readable output contracts.
- Three explicit, navigable journeys across homepage/Registry/docs/CLI:
  1. **Try:** discover → install → configure → run with Harness.
  2. **Build:** initialize → compose → validate/test → publish.
  3. **Template:** choose → bootstrap → customize → run.
- Contextual next steps on package pages, empty results, success states; lightweight Get Started landing
  route allowed if useful, but no large wizard required. Preserve `agentpm publish`'s existing Registry URL
  output; link it to sharing/inspection.
- Select or create **one genuine, simple, deterministic-enough, inexpensive, low-credential Agent Package**
  that can be installed and run through Harness with clearly documented provider/model configuration. Not
  the future flagship AgentPM Developer Agent; realistic examples and expected results, minimize API calls.
  If all candidate examples require credentials/network, explain requirements honestly.
- For onboarding, an APDS-valid but nonrunnable blank Agent from `init` needs accurate next steps to add a
  Loop and execution setup rather than falsely promising immediate Harness compatibility.

### 15.1 Example and Template corpus republication

**REQUIRED:** `agentpm-examples` holds roughly 150 published manifests across all eight kinds (59 tool, 23
agent, 16 loop, 13 skill, 11 template, 10 memory, 10 knowledge, 7 profile), none of which declare
`standard`. Templates are the blocking case: a Template ships scaffolded manifests **inside** its
`files_root`, and `agentpm new` validates generated manifests through a blocking check
(`commands/new.rs::validate_generated_manifests_blocking`). So once strict lint lands, `agentpm new
<template>` **fails during scaffolding itself** — not at a later `agentpm lint` — and Journey C breaks end to
end. Published versions are immutable, so the remedy is **new
Template versions**, never edits in place.

Scope the republication deliberately rather than sweeping the corpus:

| Set | Action | Why |
|---|---|---|
| Templates whose `files_root` ships manifests | **Required** new version with `standard` in scaffolded files | `agentpm new` → `lint` must pass; otherwise Journey C and S2-AC-19 fail |
| The M15B starter Agent Package | **Required** new version | The Try journey should show a declared, verified standard, not legacy inference |
| One package per kind | **Recommended** new version | Health/standard/detail surfaces need non-legacy states to render, or all eight kinds display "inferred" |
| Everything else | **Leave published as-is** | Still installable; the most realistic corpus for legacy-inference fixtures and the M1A inventory |

Prior published versions MUST remain byte-identical and installable, and republication MUST NOT mark legacy
versions as APDS-verified. Template `dependencies` pin exact versions at publish time, so a republished
Template must also reference any republished Component versions it should scaffold against, and the
generated workspace must resolve and install. An older CLI must still be able to use the older Template
version; see §4's old-client compatibility requirement.

### 16. AgentPM Developer future-stage handoff

- Stage 2 **prepares** copyable prompts/CTAs only where valid and an actual AgentPM Developer feature is
  available; authoring manually or from Template must remain fully supported.
- Write a self-contained handoff brief for the **AgentPM Developer stage**, in planning docs or a
  separately generated handoff during the relevant Stage 2 milestone. Envision: **discover AgentPM → run
  existing Agent Package → bring user's own idea → build with AgentPM Developer → validate → optionally
  publish → receive Registry link/shareable Card**. Focus on meaningful user-authored creations rather than
  tutorial clones, optional public publication, and reuse/success metrics; Developer remains specialist for
  AgentPM artifacts, not general-purpose coding assistant. Do not implement the journey in Stage 2.

### 17. SEO, pricing, copy and evaluation

- Stage 1 owns SSR metadata/canonical/OG/sitemap/robots/crawl constraints. Stage 2 updates **meaning**, not
  rebuilds SEO plumbing: category-aligned titles/descriptions/H1/H2, nav links, snippets, internal links,
  OG content and schema URLs. Preserve canonical routing for existing detail URLs; new explainer/spec
  routes need explicit SEO/indexability decisions, not infinite dynamic query pages.
- Pricing model unchanged: public Agent Packages **and Components** free; private namespace paid options;
  Team means collaboration/shared internal packages rather than a dedicated separate Registry deployment.
  Public Explore should be discoverable without misleading login gating.
- Prepare light comprehension/first-run verification using existing Stage 1 analytics and optional user
  observation; broader customer interviews, positioning/category-market validation and *Deploy Empathy*
  learning belong to the marketing/user-learning stage, not as a Stage 2 release gate.

#### Expanded onboarding, CLI, validation, and future-stage reference

The complete first-use rationale for implementers who did not attend the planning conversation.

#### CLI copy, user-facing term migration and command examples

The current top-level CLI help identifies itself generically as `AgentPM CLI` and lists commands without
explicitly naming the category. This misses an opportunity for the most tangible Manager surface to
reinforce what the product does.

**Illustrative top-level help intent (not literal clap formatting):**

```text
AgentPM — Agent Package Manager

Create, validate, publish, install, and reuse Agent Packages
and reusable Components through Agent Package Management.

Get started:
  agentpm init               Create an Agent Package definition
  agentpm lint               Validate its APDS declaration/semantics
  agentpm install ...        Install a versioned Agent Package and dependencies
  agentpm harness ...        Run a compatible Agent Package with AgentPM Harness
  agentpm new ...            Bootstrap a project from a Template
  agentpm run ...            Execute an individual Tool
```

Command names/arguments above are conceptual. Codex must preserve actual parser and options; the
author-facing help must be accurate, succinct and not promise nonexistent short-forms. The two exact terms
**Agent Package Manager** and **Agent Package Management** should both appear in relevant top-level help.
Subcommands should distinguish Agent Packages, Components and Templates without repeating marketing prose
in every help screen.

Default init is a deliberate Stage 2 change even though it may change user expectations; this is the right
stage before meaningful adoption. Update default artifact name away from `my-tool`. Review shell
completion, snippets and tests that assumed Tool was the implicit kind. Explicit Tool creation remains
functional, including Stage 1's fixed Tool scaffold.

**Don't perform a blind language replacement**: `agent` as manifest kind, command option or acting runtime
entity often remains correct. User-facing package identity is `Agent Package`. Internally a shared Registry
database may still call all eight things packages. The public taxonomy should not quietly rename
established machine routes/API fields without a compatibility plan.

#### First-run journey details and success conditions

**Journey A — Try an Agent Package (default first experience):**

1. Developer lands on homepage or searches Explore and sees a *real* complete Agent Package, with purpose
   and version.
2. They choose a supported version and copy a correct `agentpm install` command.
3. They are told about model/provider setup and any package-specific required credentials **before** trying to run.
4. `agentpm harness` runs it where supported; otherwise the UI clearly explains an alternative SDK/manual path.
5. They inspect outputs/traces and learn how the Component composition made the package reusable.
6. Suggested next step: tweak/build their own package, or explore reusable Components and Templates.

**Journey B — Build an Agent Package:**

1. Understand `agentpm init` now creates `kind: agent` by default; choose explicit kind for individual Components.
2. Edit Agent description and bind Components/Loop as needed for intended execution.
3. Validate with APDS-aware lint and see clear violations rather than generic Tool-first errors.
4. Install/lock dependencies and execute when runnable; a minimal definition is valid but not necessarily executable.
5. Publish to an appropriate namespace, receiving the existing successful CLI Registry link.
6. View the published version, its APDS declaration, composition and truthful Health; optional sharing later.

**Journey C — Start from a Template:**

1. Discover a Template showing use case, stack and declared execution surfaces.
2. Run `agentpm new` with current supported syntax and choose variables.
3. Understand that the scaffold creates a workspace/project; it is not itself an Agent Package.
4. Inspect generated dependencies, install them, and run through its advertised execution surface (Harness
   if supported).
5. Optionally customize, publish and share any resulting Agent Package/Components.

**Cross-surface transitions:** Homepage CTA → Explore → exact package detail → install/run code →
successful outcome → build or Template pathway; CLI publish → public/private Registry detail URL →
share/inspect; Explore no-result → clear filters or create matching package; Component page → understand
where it fits in an Agent Package. None of these should end with a dead or fictitious AgentPM Developer
button.

#### Starter Agent Package selection — objective constraints

A starter must be small enough to test reliably and inexpensive enough that a new developer is willing to
run it. **It must be a genuine Agent Package**, not a single Tool mislabeled as an Agent. It can use one
simple Loop and one or two meaningful Components, and should produce legible output from a short prompt.
Prefer a useful task with no mandatory external SaaS API keys beyond a chosen model, or clearly disclose
requirements if unavoidable. It must run on the supported Harness path after minimal configuration.

Candidate selection **belongs during implementation**, not here. Check actual existing `agentpm-examples`
content; choose an existing sample if robust or author/publish one if necessary. Do not force the complex
flagship AgentPM Developer into the first-run example. The initial curated site slots should not depend on
a specific hardcoded example ID; choose data later. Preserve expected outputs/test instructions and package
version pin for screenshots and docs.

**Success proof:** Fresh-user smoke run with clean workspace; exact install + Harness command;
elapsed/cost/token observations where feasible; missing-credentials behavior; explanation of output;
beginner comprehension of what was installed. A basic example requiring hidden environment variables, repo
permissions or a complex multi-step approval flow is **not** a successful onboarding asset.

#### Future AgentPM Developer-stage handoff (document only, not Stage 2 implementation)

The later AgentPM Developer stage can explore a meaningful creator adoption loop:

`Discover AgentPM → Run a real existing Agent Package → Bring a personal workflow idea → Use AgentPM
Developer to assemble APDS artifacts → Validate/test/execute → Optionally publish → Share a Registry link
and eventually a README Agent Package Card`.

Guardrails:

- The user's own idea is central; avoid generating many near-identical tutorial clones simply to increase counts.
- A local-only Agent Package is a successful outcome; publishing is voluntary and gated by normal Registry
  permissions/Health checks.
- AgentPM Developer is an AgentPM-artifact builder, not a general-purpose coding assistant or mandatory runtime.
- Assisted authoring is an optional path alongside manual composition and Templates.
- Meaningful reuse, runs, upgrades and follow-on users matter more than raw new-package count.
- Shareable cards should celebrate a real working composition with a Registry link; do not imply
  certifications, endorsements or non-existent trust signals.

**Stage 2 deliverable:** a self-contained Markdown handoff brief with the journey, rationale (Registry
supply/cold start), inputs the future Agent will need (standard, lint, Registry, templates,
discoverability, card), potential acceptance criteria and deferred open questions. This should be portable
to a separate planning chat without this conversation.

#### Positioning validation without requiring market proof to ship

Stage 2 is about making the experience **comprehensible and runnable**. Category-market fit is a separate,
later user-learning question. We have the beginning of a Deploy Empathy-inspired developer interview agenda
but have not selected participants or scripted customer conversations; do not invent outcomes.

Recommended lightweight comprehension and usability review (no elaborate research platform):

- Before reading docs, can an agent-experienced developer describe what the complete Agent Package contains?
- Can they identify APDS as an authored structural/semantic contract, not another name for Harness?
- Can they explain how an Agent Package differs from manually copying a repo, framework app or
  host-specific configuration?
- Can they find Tool/Skill/etc. as reusable Components without mistaking a Template for a package?
- Can they find the raw manifest and exact versioned APDS semantics from a detail page?
- Can they tell the difference between conformant, runnable, local-ready, signed and scanned?
- Can they complete a first Harness run from the Registry with prerequisites clearly disclosed?
- Can they discover how to build and optionally publish their own Agent Package?

Use Stage 1-approved analytics only if available, observe privacy boundaries, and record actual
misinterpretations. Fix material first-use confusion during Stage 2; send broader comparisons, customer
objections and willingness-to-adopt evidence to Stage 5 marketing/user learning. Do not claim user
validation happened unless it did.

#### Comprehensive documentation after Stage 1: what “LAST” specifically means

The comprehensive rewrite spans homepage-linked public docs, Introduction/Quickstart, `README.md` files
controlled by the AgentPM team in CLI/Registry/website/SDK/examples repositories, navigation, examples,
command guides and reference links. **It is the last Stage 2 implementation milestone**, after Stage 1's
final documentation/migration milestone is finished, because the CLI/Registry/lock/release behavior must
stabilize first.

Normative APDS files are built earlier. Early feature-local guidance and new reference page descriptions
are allowed when those features ship. What is deferred is the across-the-ecosystem, polished **coherent
rewrite**; do not try to do it twice while Stage 1's functionality is changing.

Documentation IA should lead with Agent Package understanding and a real try flow; then
build/compose/publish, six Components, Templates, APDS semantics, Registry/install/versioning, Harness
Runner, SDKs, compatibility/Health and migration/legacy. Command snippets must match executable CLI version
and selected package manifests, not fictional mockups. The documentation must also teach that Tool
execution uses `agentpm run`, while Harness runs a compatible Agent Package. Preserve useful technical
material rather than replacing docs with marketing copy.


### 18. Comprehensive docs and READMEs — LAST, hard gate

After **every Stage 1 milestone is complete**, Stage 2's **final milestone** aligns `docs`
Introduction/Quickstart/IA, CLI lifecycle reference, Registry onboarding, APDS reference, Agent Package vs
Component vs Template model, Harness Runner boundaries, SDK and publish/install flows, realistic examples,
category explainer cross-links, and AgentPM-controlled repository READMEs. Ensure product UI/API/CLI
behavior has stabilized before rewriting. The **normative** APDS `README.md`/`semantics.md` needed earlier
are exempt from this gate; Stage 1 may make its own migration/functional docs changes. Never mass-edit
author-controlled published README pages.

---

## Acceptance criteria

IDs are stable requirement references for `tasks.md`, `test-plan.md`, and reviewer sign-off.

### APDS and contract

- **S2-AC-01** — A deterministic, versioned APDS v1.0.0 contract exists with immutable authoritative schema
  derived from `agentpm.manifest.schema.json`, documented normative semantics, stable rule IDs, linked
  fixtures, version policy and canonical docs/source paths. Existing generic schema path cannot drift from
  it, and no runtime schema-source override can silently replace it. The freeze occurred **after** Stage 1's
  manifest-schema additions (M2, M11A) merged, and those fields are conformant under v1.0.0.
- **S2-AC-02** — All eight new kind scaffolds emit `standard: {id:"agentpm",version:"1.0.0"}`; plain
  `agentpm init` defaults to an APDS-valid minimal Agent Package, and `--kind tool` plus every explicit
  other kind remains valid. Names/descriptions serialize safely. `agentpm export`'s generated Skill
  scaffold also emits `standard`. A **tolerance release** that accepts and round-trips `standard` shipped
  **before** any command emits it, manifest-rewriting paths preserve the field, and the outdated-client
  rejection names a concrete minimum CLI version.
- **S2-AC-03** — Agent no longer requires nonempty/declared Tools, Loop or Profiles for APDS validity;
  common required fields including nonblank description and supported standard are validated. Semantics
  preserve Phase 6/7 binding, Loop, Profile, Skill, Memory and Template contracts.
- **S2-AC-04** — CLI Rust and Registry Python each independently evaluate the pinned same fixture corpus at
  standalone and resolved-composition levels; clear deterministic
  conformant/nonconformant/incomplete/unsupported distinctions, rule-level failures and no live arbitrary
  schema fetch.
- **S2-AC-05** — Registry publication independently inspects staged archive bytes and finalizes only
  APDS-valid **new releases**, reconciles metadata/manifest identities, and preserves existing
  ACL/scanning/signing/integrity controls. Historic released versions remain byte-identical/installable and
  legacy inference is not misrepresented as verified conformance.

### Installation and Runner

- **S2-AC-06** — Effective APDS standard and declaration provenance propagate consistently through Registry
  resolve, CLI installed metadata and lock state; v4 preference and forward-guard semantics preserved;
  unsupported standard explicit.
- **S2-AC-07** — Frozen install correctly replays complete exact dependency closure for direct Agent/Skill
  and transitive Components, enforces constraints and checksum verification; dependency-free Agent
  installation succeeds, including no-op resolution.
- **S2-AC-08** — Harness advertises/checks supported APDS versions across interactive/headless/machine/SDK
  surfaces, preserves exact locked Component versions, separates conformant/compatible/ready, and gives
  truthful missing Loop/provider/model preflight failures without changing orchestration engine
  fundamentals.
- **S2-AC-09** — Optional top-level Agent description prompt-inclusion comparison is conducted with
  reproducible examples; an explicit decision is recorded; no APDS requirement is added without approval.

### Health and detail views

- **S2-AC-10** — Objective version-scoped universal + kind-specific Package Health evidence is exposed and
  rendered appropriately across artifact kinds, with accurate
  verified/failed/unavailable/incomplete/not-applicable/advisory handling and no quality score or invented
  safety claims.
- **S2-AC-11** — Agent Package Overview prioritizes human-readable direct/transitive composition, versioned
  Component links, and accurate Loop/phases/bindings; no-Loop/no-dependency and non-linear Loop cases
  remain understandable. Raw manifest is available but secondary; existing tabs and kind-specific
  inspection remain intact.
- **S2-AC-12** — Declared standard and conformance evidence are distinguishable; version-specific
  `agent.json` and immutable human-readable versioned APDS specification have separate functional links;
  legacy inferred states remain labeled.

### Website, discovery, and sharing

- **S2-AC-13** — Homepage teaches developer problem/complete Agent Package first, AgentPM as
  Manager/Registry plus Harness reference Runner, with one real data-driven package visual and clear
  try/build/template paths. Existing visual identity is preserved/evolved rather than replaced wholesale by
  mockup styling, with any adoption traceable to the signed D21 list,
  and the page renders on the shared global shell rather than a bespoke layout.
- **S2-AC-13a** — A single global layout system exists — header (anonymous and authenticated), page canvas,
  section-card primitive, kind tokens, grid primitives and footer — built once and adopted by every route.
  A signed **D21** adoption review records adopt/adapt/reject for every mockup element before build, and
  what shipped matches it; styling adopted from the mockups is deliberate and listed rather than wholesale;
  Stage 1 search, SSR metadata and canonical behavior are intact; shell-level responsive and accessibility
  behavior is verified.
- **S2-AC-14** — Explore and namespace surfaces visually group Agent Packages, six Component kinds,
  Templates, Namespaces; cards for all kinds show helpful actual metadata; no-query Explore offers useful
  data-driven discovery; Stage 1 search, filters, relevance, visibility, trending, stars, pins and
  pagination are preserved.
- **S2-AC-15** — Featured catalog selection/order/placement is data-driven/configurable without
  presentation code changes and visibility-safe; featured and Trending remain distinct.
- **S2-AC-16** — Reusable Agent Package Card primitives appear across appropriate UI/OG surfaces;
  investigation into embeddable README card produces a documented feasibility/decision/prototype and an
  explicitly assigned implement-vs-defer conclusion.
- **S2-AC-17** — Versioned standard route and category explainer are accurate and linked;
  pricing/SEO/breadcrumb/footer/product-controlled text use the agreed taxonomy, with no plan, SEO
  canonical or public-access regression.

### CLI, adoption, documentation

- **S2-AC-18** — `agentpm --help` explicitly says Agent Package Manager and Agent Package Management;
  appropriate subcommands distinguish init/new/run/harness/install/publish/lint; Stage 1 CLI machine
  behavior preserved.
- **S2-AC-19** — Three end-to-end first-use journeys have contextual entry points and accurate commands; a
  real simple low-cost Agent Package is installable and runnable through Harness with stated
  provider/config requirements; `publish` continues to link to the Registry; no dead AgentPM Developer CTA.
  Templates whose `files_root` ships manifests are republished so `agentpm new` output passes strict lint,
  the starter declares a standard, and every prior published version remains byte-identical and installable.
- **S2-AC-20** — Future AgentPM Developer-stage guided creation/publishing/optional sharing brief exists,
  including user-owned ideas, reuse-driven cold-start loop and Card handoff, without implementing the
  wizard.
- **S2-AC-21** — Comprehension and first-run acceptance checks demonstrate that an unfamiliar technically
  sophisticated developer can follow the category model and complete a simple flow; record usability issues
  and fixes, not ungrounded proof of market validation.
- **S2-AC-22** — **Final** Stage 2 milestone (and no earlier broad docs rewrite) comprehensively aligns
  public docs and AgentPM-owned READMEs with implemented Stage 2 contracts, only after all Stage 1
  milestones are complete.

---

## Risks / edge cases

1. **Dual-schema drift:** old moving `main` schema vs immutable v1; editing one but not the other creates
   split-brain authoring/Registry validation. Require canonical source/CI consistency.
2. **New-release hard gate breaks existing clients:** clients omitting `standard`, queued upload sessions
   crossing deployment, already published versions, local legacy artifacts; deploy sequencing and explicit
   error/upgrade guidance are mandatory.
3. **Validator disagreement:** schema draft/regex/semantic differences between Rust and Python; shared
   fixtures must expose parity failures but implementations remain independent.
4. **Validation certainty:** standalone lint without complete graph must not claim resolved verification;
   Registry finalize must fetch/reuse correct versions or explicitly scope what it can determine; if
   resolved conformance is mandatory at publish, define how dependency access and cycles behave.
5. **Incorrect semantics formalization:** Phase 6/7 rules may differ subtly from naive diagrams; coordinate
   exact Scope, Memory operation participation and Loop capability precedence before freeze.
6. **Standard version conflation:** artifact version, APDS version, lock version, Runner version and machine
   protocol drift in serialization/error copy.
7. **Legacy trust inflation:** inferring a legacy interpretation cannot retroactively guarantee compliance
   or forge provenance on existing immutable bytes.
8. **Multi-artifact release races:** staged embedded manifest vs finalized release identity, stale S3 bytes,
   hashing and scanning across targets; preserve atomicity and rollback.
9. **Frozen install incorrect graph:** direct Agent/Skill assumes Tool, version-range mismatch, ignored
   transitive dependencies, clean-install replay, digest mismatch and empty dependency lists.
10. **Name-only runtime resolution:** wrong Component version in workspace, phase restrictions silently
    bypassed, capability hints mistaken for grants, approvals omitted in UI description.
11. **False Runner readiness:** missing provider/model, unsupported backend, no Loop, non-TTY; clearly report
    distinction and safe failure.
12. **Overstated Health:** signatures or scans presented as safety, advisory as enforced, unknown as pass,
    identity-level stars as version-level quality, not-applicable as failure.
13. **Nonlinear Loop flattening:** phase visualization cannot pretend all graphs are three-step straight
    lines; show branches/terminal edges or readable fallback.
14. **Featured visibility and SEO:** stale/private/deleted featured item leak, invalid canonical/robots,
    no-query Explore becoming an arbitrary unrelated sort, possible Stage 1 paging regression.
15. **Mockup fictional facts:** example names/metrics/claims may not match actual Registry/manifest; never
    render invented verification, versions or runtime integrations.
16. **Visual drift:** replacing current floating/layered design with flat design because generative mockup
    suggests blue/white bands; existing site is color/elevation reference.
17. **Onboarding failure despite good copy:** no truly runnable inexpensive Agent, model credential surprise,
    `init` produces valid but nonrunnable Agent without explicit next step, false “one click run” claim.
18. **Documentation race:** publishing new comprehensive category docs while Stage 1 modifies
    lock/publish/search contracts would immediately stale them; final docs require Stage 1 completion.
19. **Overbuilt standards/platform scope:** attempt to solve universal Runners/APDS governance/semantic
    DSL/evals instead of formalizing today's AgentPM contracts.

---

## Open questions

**None of the following are unanswered product decisions Zack must make before implementation.** They are
deliberate implementation or experiment choices: Codex documents its **preferred option** (below),
evaluates alternatives against code reality, makes a justified choice, and Claude reviews. Escalate only if
altering an AGREED requirement. The complete list is retained so none are silently dropped.

| ID | Design decision | Preferred direction (opinion) | Allowable Codex/Claude discretion and required decision evidence |
|---|---|---|---|
| **D01** | APDS rule organization | **One `semantics.md`** organized by semantic domain with normative rule IDs, exceptions, examples and fixture links; no bespoke DSL. | Split if maintainability clearly benefits; document canonical normative index, non-duplication and fixture consistency. |
| **D02** | APDS contract directory and schema migration | **`cli/standards/agentpm/1.0.0/`** authoritative, versioned copy of existing schema, old schema path compatibility alias/generated output. | Choose repo/build mechanism after examining CLI bundling and Registry vendoring; prove no drift or live-fetch dependency. |
| **D03** | Conformance fixtures and outputs | **Valid/invalid/resolved fixture directories** + expected rule/status/JSON-path results; standalone/resolved distinction. | JSONL/index alternative permitted if ergonomic; verify Rust/Python independent parity and CI integration. |
| **D04** | Legacy authored manifests / lint | Strict by default for newly authored; optional explicit upgrade/legacy compatibility mode if useful. | Decide UX and error messages without opening a default bypass; document old local authoring handling. |
| **D05** | Registry staged validation and release gate | Validate embedded manifest at finalize, compare metadata, atomic rejection; phased migration of legacy clients. | Adjust init-time validation as useful, define queued-session cutoff, prove cannot bypass via custom client. |
| **D06** | Lockfile format | **Retain Stage 1 `agent.lock` v4**, carry standard+explicit/inferred provenance with forward-version safeguards. | Only justify v5 if v4 cannot safely express facts; document compatibility and migration evidence. |
| **D07** | Versioned standard web route | **`/standards/agentpm/1.0.0`**, pinned catalog and immutable content. | Build-generated page vs server-side trusted bundle, route nuance; no untrusted runtime fetch. |
| **D08** | Homepage design | Problem/value → real Agent Package composition → how management works → components/templates → pathways, layered existing brand. | Layout/copy/animation details; mockups are conceptual and existing design tokens authoritative. |
| **D09** | Agent Package Card visual architecture | Shared data model and responsive card variants for hero/Explore/detail/OG, real metadata only. | One component vs composable primitives, based on existing Stage 1 shell/OG architecture. |
| **D10** | Featured package selection | Lightweight **data-driven** curated configuration or Registry-backed curation; no fixed IDs in JSX, fallback on unavailability. | Choose source/edit mechanism; compare reuse of Stage 1 namespace pins vs simple config; verify permissions and ordering. |
| **D11** | Explore no-query state | Curated/featured + measured Trending sections, clear Component/Template paths, retain query results relevance unchanged. | Section/layout implementation, no new algorithm or invisible ranking changes. |
| **D12** | Package Health representation | Universal evidence record + kind-specific extensions; concise release summary and deeper detail, reuse Security. | Data type, API envelope, tab/section placement; demonstrate support and correct statuses for all eight kinds. |
| **D13** | CLI description prompt context | Small reproducible A/B, keep only if meaningful outcomes improve and avoid duplication. | Experimental prompts/metrics/sample selection; report evidence even if decision is no change. |
| **D14** | First-run Agent Package | Simple useful example with low credentials/cost and reliable Harness loop; not AgentPM Developer. | Existing package vs create new example, exact use case and publishing time; deliver at least one working route. |
| **D15** | Contextual onboarding vs Get Started page | Contextual entry points + perhaps concise Get Started hub; no wizard. | Choose navigation/IA based on proven first-run flow; record how all three paths stay discoverable. |
| **D16** | README-embeddable Agent Package Card | Reuse OG/static image infrastructure; evaluate pinned/latest semantics, security, caching, access. | Investigation deliverable mandatory; decide implementation or defer with estimated cost, prototype, Claude sign-off. |
| **D17** | Component/Template Health UX | Consistent shared evidence semantics with kind-specific content, preserve specialized detail views. | Shared API/component extension model vs per-kind adapters; no uniform meaningless checklist. |
| **D18** | Category explainer and SEO copy | Product-centered homepage + deeper `/agent-package-management` page; no inflated category claims. | Final page structure/copy/metadata; verify current SEO contracts and user comprehension. |
| **D19** | APDS declaration/status schema in Registry/lock/UI | Distinct `declared` vs `legacy_inferred`, conformance level/status, source evidence; avoid conflation. | Field names and serializer shape coordinated across API, CLI, Harness, Health and UI; backward-compatible rollout. |
| **D21** | Mockup adoption scope — what to take from the `-layered` images and what to leave | An explicit **adopt / adapt / reject** pass over every global element and notable styling treatment, decided and signed off **before** the shell is built. Layout and structure are the mockups' strongest contribution; styling is adopted selectively, never by default. Copy, all displayed values, the shipped per-kind color assignment and the generated per-package identity gradient are **excluded from the review** and preserved as-is. | Codex proposes the classification with a one-line reason per element and flags anything where the mockup clearly beats the current treatment; **Zack signs off before implementation**. Claude checks that what shipped matches the signed list, not the images. |
| **D20** | Runner APDS capability advertisement | Reuse existing Harness machine initialization/preflight and SDK wrappers; don't invent a separate protocol. | Exact surface/serialization; parity tests across TUI/headless/machine SDK and standard-version errors. |

### Decisions already settled: do not reopen by default

- `agentpm init` defaults to Agent Package; explicit Tool init preserved.
- Agent Package may omit Loop, Tools and Profiles; complete authored artifact ≠ runnable.
- One canonical versioned APDS schema evolved from existing `agentpm.manifest.schema.json`, plus normative
  semantic rules and shared fixtures.
- The standard is not identical to Registry publication, Runner compatibility, Health or security verification.
- Registry enforces new-release conformance independently and preserves old published versions without
  falsely verifying them.
- Stage 1 owns existing discovery/SEO/lock/provenance functional hardening.
- Package Health objective, without quality score; universal+kind-specific evidence.
- Data-driven featured/catalog selection, with specific examples selected later.
- Existing website colors, typography and floating/layered feel authoritative; mockups illustrate hierarchy.
- Reverse-dependency discovery deferred; existing publish URL sufficient.
- AgentPM Developer guided bring-your-own-idea publication journey deferred, documented for its own stage.
- Comprehensive docs/READMEs are **the final Stage 2 milestone after all Stage 1 work finishes**.

---

## Related Specs

- **Implementation and review companions:** [`tasks.md`](tasks.md), [`test-plan.md`](test-plan.md),
  [`review-checklist.md`](review-checklist.md).
- **Stage 1 authoritative plan:**
  [`agentpm-dev/cli/specs/2026-10-06-immediate-hardening/spec.md`](https://github.com/agentpm-dev/cli/blob/main/specs/2026-10-06-immediate-hardening/spec.md)
  and
  [`tasks.md`](https://github.com/agentpm-dev/cli/blob/main/specs/2026-10-06-immediate-hardening/tasks.md).
  Inspect current branch before implementation; the Stage 1 plan may evolve.
- **Stage 2 handoff:** `AgentPM_Stage_2_Handoff_Findings(1).md` (context provided during planning; if not
  in repo, use this spec as the self-contained contract).
- **Current structural baseline:** `agentpm-dev/cli/schemas/agentpm.manifest.schema.json` (provided
  reference copy `agentpm.manifest.schema(2).json`); `agentpm.harness.schema.json` for Runner config
  distinctions; current CLI `agentpm --help` reference (`agentpm-help.md`, v0.1.33 at 2026-10-09).
- **Current app surfaces:** `agentpackagemanager.com`, `/explore`, Agent Package detail
  `/agents/<id>/v<version>/overview`, namespace detail, `/pricing`,
  `/docs/latest/getting-started/introduction`, `/docs/latest/getting-started/quickstart`. Keep existing
  public route stability; the route names need not be globally renamed.
- **Strategic inputs:** AgentPM Category Design Document, Category Blueprint, Product Taxonomy, Customer
  Use Cases, Category Ecosystem, Lightning Strike Strategy, Positioning — Working Hypothesis; these are
  *working hypotheses*, not independent licenses to expand this scope.
- **Visual references:** [`assets/landing-structure.png`](assets/landing-structure.png),
  [`assets/detail-structure.png`](assets/detail-structure.png),
  [`assets/landing-layered.png`](assets/landing-layered.png),
  [`assets/detail-layered.png`](assets/detail-layered.png),
  [`assets/explore-layered.png`](assets/explore-layered.png). The three `-layered` images are the
  **authoritative target for structure and layout** across the whole site (see §9.1); the existing live
  website wins on brand styling, design tokens and real facts.

