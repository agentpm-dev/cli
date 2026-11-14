# Feature

**Stage 2: Category-Enabling Hardening — Make the Agent Package Real**  
**Status:** Planning handoff / proposed implementation specification; Codex implements, Claude Code
independently reviews.
**Date:** 2026-10-09  
**Companions:** [`tasks.md`](tasks.md), [`test-plan.md`](test-plan.md), [`review-checklist.md`](review-checklist.md).

**M1A implementation evidence (2026-11-13):** [Existing contract inventory](m1a-contract-inventory.md), [schema source index](m1a-schema-index.md), and [local verification](m1a-verification.md). This baseline records current behavior and reconciliation items; it does not freeze APDS or supersede the requirements below.

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
M1–M18 with submilestones 10A/B, 11A/B/C, 13A/B, 14A/B/C.

**Milestone ID convention:** the two stages share an `M<n><A/B/C>` namespace — `M11A`, `M13A`, `M14A/B`,
`M15A/B`, `M16A/B` and `M17` exist in both. Throughout this spec and its companions a bare `M…` reference
means **Stage 2**, and every Stage 1 reference carries an explicit `Stage 1` prefix. Cite split Stage 1
milestones by exact sub-milestone (`Stage 1 M14A`, not `Stage 1 M14`), since the sub-milestones own
materially different contracts.

Stage 2 **consumes those foundations**, does not duplicate them. Integration gates belong in `tasks.md`.
Particular handoffs: Stage 1 M2–M7 (discovery, detail shell, SEO), M8–M9 (CLI), M11B (lock v4), M12–M16
(portability, provenance, artifacts), M10A/B (analytics), Stage 1 M17 (SDK lock/runtime
compatibility). Stage 1 M18 focuses its own
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

**Stage 1 release handoff (updated 2026-10-22):** Release Band 5 records CLI **0.1.36**
as the tolerance-release minimum for M4B's outdated-client error and package-detail
upgrade guidance, superseding the original 0.1.35 preparation target. Operator
confirmed **v0.1.36 publication/installation on 2026-10-22**, and live Production
first-use capture reports that CLI version. **M3A's publication prerequisite is
now satisfied; its M1B/M2B dependencies and other acceptance requirements are
not waived.** The publication record is in
[`Stage 1 Release Band 5`](../2026-10-06-immediate-hardening/tasks.md#release-band-5-cli-authoring-and-messaging-quality).

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

**REQUIRED — human-first APDS documentation.** `README.md` and `semantics.md` are written for developers
who need to understand what to implement and which guarantees they can rely on, including readers who
have never used AgentPM. **Clarity and concision are acceptance requirements, not optional polish.** A
technically correct standard that developers cannot reasonably read or navigate is not finished.

- **`README.md` is the entry point:** Explain APDS, the artifact kinds, conformance roles, core guarantees,
  scope/limitations, and where to find the normative rules. Use a short, navigable overview; link to detail
  rather than reproducing the entire specification.
- **`semantics.md` is the precise, readable contract:** Introduce concepts in plain language, then state
  short, unambiguous rules. Prefer active voice, familiar terms, compact examples and skimmable headings.
  For example, prefer "Global bindings apply in every phase. Phase bindings add capabilities to that phase;
  they do not replace global bindings. Loop restrictions still apply" over an abstract paragraph that
  communicates the same guarantee.
- **One source of each fact:** Do not restate schema field definitions, repeat a rule across several
  chapters, or narrate every imaginable implementation strategy. Refer to the versioned schema, a rule ID,
  or the conformance fixture instead. Keep AgentPM implementation history outside normative APDS content.
- **Rigorous without boilerplate:** Stable rule IDs, conformance subjects/levels, expected results and
  fixture mappings remain required, but they need not be expanded into a separate long template for every
  rule. Store detailed validation metadata in a concise rule index/fixture record where it is more useful;
  give rationale or examples in the prose when they clarify a non-obvious point.
- **Explain freedoms clearly:** A reader should know both what the contract guarantees and what a
  compatible implementation may do differently, without repeated lists of forbidden internal algorithms.
  Tables, diagrams or short examples are welcome when easier to understand than dense paragraphs.
- **Review for humans:** Codex must do a dedicated editorial pass and Claude must independently read both
  files as a technically capable newcomer. Remove needless jargon, duplicated qualifications, long
  sentences and mechanically formal prose without deleting genuine interoperability guarantees.

There is **no arbitrary page or word limit**: Memory and Loop semantics may need more detail than simple
identity rules. The standard should nonetheless be the **shortest clear document that fully specifies APDS
v1.0.0**. These guidelines are instructions for authoring the standard, not additional normative rules
that implementations must satisfy.

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
Rule applicability, validation level, expected failures/paths, rationale where useful, and fixture
mappings must remain traceable; they need not appear as a lengthy repeated template in the prose.
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

**Writing guidance:** Keep this README a short, navigable introduction, not a second copy of
`semantics.md` or the JSON Schema. A reader should quickly understand what APDS promises, which role they
are implementing and where to find the precise requirements.

#### Normative semantics chapter blueprint — more detailed than a table of contents

Codex should use the following as an **implementation guide for the actual content of `semantics.md`**,
not as a requirement to turn every bullet into a long paragraph. Give each normative rule a stable ID,
clear requirement and scope; keep validation levels, failure expectations and fixture references traceable
through concise annotations or the rule/fixture index. Add rationale and examples when they genuinely
clarify meaning, not as repetitive boilerplate. Prioritize the human-first documentation requirements
above. Where existing source does not support a proposed interpretation, **surface a reconciliation
issue** before freezing v1.0.0.

**Chapter 1 — Status, scope, conformance, and normative language**

- Define `MUST`, `MUST NOT`, `SHOULD`, `SHOULD NOT`, and `MAY` as normative requirement terms. Define conformance subjects such as artifact authors, validators, registries, Managers, and compatible Runners. Requirements apply only to the conformance subjects and capabilities explicitly identified by each rule; implementations are not required to fulfill unrelated roles.
- Establish the scope of APDS as a standard for the **structure and portable semantics of Agent Packages and their Components**. Structural requirements define valid artifact representations. Semantic requirements define the meaning of authored declarations, their relationships, and the guarantees necessary for compatible interpretation. APDS does not prescribe internal implementation architecture unless specific observable behavior is necessary to preserve those semantics.
- Distinguish **standalone artifact conformance** from **resolved-composition conformance**. Standalone validation evaluates requirements that can be determined from an artifact independently. Resolved-composition validation evaluates relationships requiring referenced artifacts. Passing standalone validation does not establish full graph conformance, and unavailable dependency context does not itself establish nonconformance.
- Define how implementations declare and report their supported conformance roles and validation levels. Validators must not claim verification of requirements they have not evaluated. Runners must preserve the applicable semantics of the constructs they claim to support, without being required to support every optional capability or use a particular execution architecture.
- Distinguish APDS conformance from distribution trust, Runner compatibility, and environment-specific execution readiness. These are separate properties; none should be inferred solely from another.
- Establish stable identifiers for normative rules and define how those rules relate to conformance tests. Require that validation outcomes distinguish established violations from requirements that could not be evaluated. Leave implementation-specific diagnostic presentation and serialization unspecified unless explicitly standardized.
- Distinguish normative requirements from non-normative explanations, examples, and implementation guidance. Explanatory material may illustrate a requirement but must not introduce additional requirements implicitly.
- Establish version immutability: a published APDS version defines a stable structural and semantic contract. Changes affecting normative requirements or conformance outcomes require appropriate standard versioning. Non-normative documentation may evolve independently without redefining the published contract.

**Implementation guidance for Codex and Claude Code — not normative APDS content**

When authoring this chapter and the remainder of `semantics.md`, use the following inclusion test:

1. Is the proposed rule necessary to determine whether an artifact is valid?
2. Is it necessary to preserve the intended meaning of an authored declaration?
3. Is it necessary to guarantee an explicitly intended aspect of interoperability?

If none applies, the behavior should generally remain implementation-defined rather than becoming normative.

Existing AgentPM CLI, Registry, SDK, and Harness behavior should inform the identification of intended semantics, but implementation choices must not automatically become standard requirements. In particular, internal algorithms, prompt construction, de-duplication strategies, caching, scheduling, recovery mechanisms, and presentation details should remain outside the normative contract unless their observable behavior is deliberately standardized.

Keep this guidance in the Stage 2 implementation specification. Do not reproduce AgentPM-specific development instructions or historical implementation explanations in the published normative `semantics.md`. Where the existing implementation and the proposed portable semantics disagree, document the discrepancy for reconciliation before finalizing APDS v1.0.0.

**Chapter 2 — Artifact identity, versions, namespaces, and references**

- Define the common manifest fields `kind`, local `name`, artifact `version`, purpose `description`, declared `standard`, and optional editor `$schema` according to the versioned schema. Clarify that `standard` identifies the normative definition contract, while `$schema` is an optional tooling reference and does not override the declared standard.
- Distinguish artifact identity, namespaced references (`@namespace/name`), artifact versions, dependency version selectors (ranges or exact versions where supported), and APDS standard versions. Define their respective meanings without conflating an artifact's version with the version of the standard used to define it.
- Define how artifact kinds participate in identity and reference interpretation. Artifacts of different kinds may have similar names; references must resolve unambiguously to the intended artifact kind and identity. Do not assume that a matching name alone establishes the correct dependency.
- Define the portable meaning and structure of namespaced references without requiring a particular hosted Registry. Namespace ownership, authentication, authorization, publication permissions, and Registry-specific identity management are distribution concerns rather than universal APDS requirements.
- Define the semantics of version references and resolved dependency identity. A version selector expresses an authored dependency constraint; a resolved dependency identifies a specific artifact version. APDS may define the requirements necessary for unambiguous composition, but does not mandate a particular dependency-resolution algorithm, lockfile format, local installation layout, or package-management implementation.
- Define unknown-property behavior according to the declared versioned schema. Unsupported properties must not be silently reinterpreted as valid APDS v1.0.0 extensions. Future extensions or changes to the manifest contract must follow the standard's versioning rules rather than implicitly expanding the meaning of an immutable standard version.
- Require explicit supported standard identification for artifacts authored under APDS v1.0.0. Artifacts lacking a standard declaration are not an alternative conformant v1.0.0 authoring format. Implementations may provide backward-compatibility mechanisms for historical artifacts, but such mechanisms must not be represented as normative APDS declarations or proof of conformance.

**Chapter 3 — Dependencies and composition**

- Define how artifacts declare direct Component dependencies using the existing manifest fields, including `tools`, `skills`, `knowledge`, `memory`, `profile`/`profiles`, `loop`, and other applicable fields according to each artifact kind's schema. Preserve the supported dependency relationships and kind restrictions of APDS v1.0.0 rather than introducing new composition capabilities.
- Distinguish **dependency declarations**, **dependency references**, and **bindings**:
    - A dependency declaration identifies a Component required by an artifact's authored composition.
    - A dependency reference identifies an artifact using the supported identity and version-constraint forms.
    - An Agent binding identifies how an already declared Component or capability participates in the Agent's execution context. A binding does not implicitly create an undeclared dependency.
    - The validity and meaning of binding scopes and effective capabilities are defined in subsequent chapters.
- Define **direct and transitive dependencies**. Direct dependencies are declared by an artifact; transitive dependencies are reached through the dependency declarations of referenced artifacts. A resolved composition must preserve the identity, kind, and dependency relationships needed to interpret that composition unambiguously. APDS does not prescribe how a Manager stores, visualizes, or presents the resulting dependency graph.
- Define the interpretation of version-constrained references within a resolved composition. When a binding or another reference identifies a dependency without repeating its version, the reference must resolve to the appropriate artifact in the containing composition's resolved dependency context. It must not arbitrarily select an unrelated installed artifact merely because its name matches. The standard should establish unambiguous reference semantics without requiring a particular dependency-resolution algorithm or lockfile implementation.
- Define cross-kind dependency and reference validity according to the supported APDS artifact relationships. A reference requiring a Tool must not resolve to a Skill or another incompatible kind. Violations requiring referenced artifact information are established during resolved-composition validation; unavailable dependency context must not automatically be treated as a confirmed violation.
- Preserve the optional nature of Agent Package composition. An Agent Package may validly omit Tools, Skills, Knowledge, Memory Blueprints, Instruction Profiles, or a Loop. APDS conformance does not require an Agent Package to contain every Component family, or to be immediately executable by a particular Runner.
- Define the distinction between an authored dependency constraint and an exact resolved dependency. Reproducible composition requires unambiguous identification of the selected artifact versions, while integrity verification may rely on additional artifact metadata supplied by a distribution or package-management system. APDS does not mandate the `agent.lock` format, a particular digest algorithm, dependency storage layout, or installation mechanism.

**Chapter 4 — Agent bindings and effective capabilities**

- Define the meaning of Agent bindings according to the existing manifest schema, including global bindings, phase-specific bindings, direct Component bindings, Memory spaces and operations, MCP imports, and consumer context where supported. Distinguish the declaration of a dependency from a binding that makes the dependency or its capabilities available within a particular execution scope. Do not introduce new binding types or capabilities beyond those defined by APDS v1.0.0.
- Define **global and phase-specific binding scopes**:
    - Globally bound Components and capabilities are applicable throughout an Agent run, subject to applicable access restrictions.
    - Phase-bound Components and capabilities are applicable within their declared phase.
    - Global and phase bindings are **additive**. The effective candidate bindings in a phase include the applicable global bindings and the bindings declared specifically for that phase.
    - A phase binding does not implicitly override, replace, or remove a global binding.
    - Binding scope determines where a declaration applies; it does not, by itself, guarantee that the capability can execute.
- Define **phase reference validity**. Phase-specific bindings must identify phases declared by the Agent's resolved Loop. References to nonexistent phases are invalid when the required composition context is available. An Agent without a Loop may still declare valid global bindings, but phase-specific bindings cannot reference nonexistent execution phases. These relationships belong to resolved-composition validation rather than requiring a particular Runner to detect them during execution.
- Define **direct and inherited capability availability**. An Agent may bind a Tool directly or gain access to Tool capabilities through a bound Skill's declared Tool dependencies. These are distinct composition relationships with potentially different binding scopes. A Tool inherited through a Skill does not require a duplicate direct Tool binding. The detailed inheritance semantics are specified in Chapter 5.
- Define the relationship between **bindings and Loop access policy**:
    - Bindings establish which Components and capabilities are applicable within an Agent's global or phase-specific scope.
    - Loop access declarations constrain which applicable capabilities may be exposed or exercised within a phase.
    - A globally bound or inherited capability does not bypass a restriction declared by the active Loop phase.
    - Restrictions on Tools, Knowledge, and Memory must be interpreted according to their respective access-policy definitions in the current APDS contract.
    - Memory's distinction between direct space access and Blueprint-governed operations must be preserved. An operation is not automatically prohibited from accessing a declared target merely because that target space is not directly bound in the current phase; Chapter 9 defines the applicable operation semantics.
    - The effective capability set is determined by applicable bindings, inherited capabilities, and authored access restrictions—not by any one of those declarations in isolation.
- Distinguish **authored capability eligibility**, **Runner support**, **runtime availability**, and **actual invocation**:
    - A binding expresses an authored relationship and its applicable scope.
    - A compatible Runner must preserve the applicable binding and access-policy semantics for constructs it claims to support.
    - A binding does not guarantee that the required runtime service, credentials, backend, or execution environment is available.
    - A binding does not independently grant permission to bypass applicable access restrictions or execution authorization requirements.
    - APDS does not mandate a particular capability-discovery mechanism, Tool invocation architecture, approval interface, or runtime provisioning strategy.
- Define **MCP binding semantics** to the extent represented in the existing Agent manifest. MCP imports identify external capabilities available within their declared binding scope. Imported capabilities remain subject to applicable authored access restrictions, including Loop Tool-access policy where relevant. APDS must not imply that an MCP import bypasses restrictions applicable to equivalent directly bound capabilities. MCP connection management, transport, authentication, discovery, and invocation remain implementation-defined unless explicitly required by the authored contract. Do not invent precedence or conflict-resolution rules absent from the existing APDS semantics.
- Define the meaning of **consumer context** as authored contextual input associated with Agent composition, according to the existing manifest fields. Distinguish contextual information from capability grants, bindings, and execution permissions. APDS may establish its applicability and reference semantics but must not require a particular model prompt structure, context-injection technique, or ordering strategy.
- Distinguish **semantic requirements from implementation strategies**. APDS defines the scope and effective meaning of bindings and access restrictions, but does not mandate how a Runner constructs internal capability tables, caches resolved bindings, assembles model context, presents available Tools, or dispatches execution. Implementations may use different mechanisms provided they preserve the applicable authored semantics.

**Chapter 5 — Skills and inherited Tools**

- Define the relationship between Skills and their declared Tool dependencies according to the existing manifest schema. A Skill may declare Tools as reusable Component dependencies. These declarations establish dependency and capability relationships, not an implicit code import, automatic Tool invocation, or additional Agent dependency declaration.
- Define **Tool capability inheritance through globally bound Skills**. When an Agent binds a Skill globally, the Tool dependencies declared by that Skill become applicable Tool capability candidates throughout the Agent run, subject to the active Loop's access restrictions and other applicable authored constraints. An Agent does not need to declare duplicate direct Tool bindings solely to make these inherited capabilities available.
- Define **Tool capability inheritance through phase-bound Skills**. When an Agent binds a Skill to a particular phase, its declared Tool dependencies become applicable capability candidates within that phase only, unless those Tools are independently available through another applicable binding. A phase-bound Skill does not implicitly make its Tools available globally or in unrelated phases.
- Define how **direct and inherited Tool bindings interact**:
    - A Tool may be available through a direct Agent binding, one or more bound Skills, or both.
    - Multiple binding paths to the same resolved Tool capability do not create additional execution permissions or change the identity of that capability.
    - Each applicable binding path contributes according to its own scope. A narrower binding does not remove availability established through an independent, broader binding.
    - Capability identity must be determined using the resolved dependency context, not merely a matching Tool name. Distinct versions or distinct resolved capabilities must not be incorrectly treated as identical.
    - APDS defines the resulting capability semantics but does not prescribe a particular internal de-duplication algorithm, capability registry, or presentation strategy.
- Distinguish **Skill content from Tool execution**. A Skill may contain authored instructions, references, and scripts according to its artifact contract. Binding a Skill makes its applicable content and declared capabilities available within the relevant scope; the binding itself does not constitute a command to execute its scripts. The presence of executable content in a Skill does not automatically grant execution privileges or bypass applicable access restrictions.
- Define the boundary between **Skill declarations and Runner execution**. A compatible Runner must preserve the authored dependency, inheritance, binding-scope, and access-restriction semantics for supported Skills. How the Runner presents Skill instructions, loads references, makes scripts accessible, or executes authorized capabilities remains implementation-defined. APDS does not mandate a shell executor, a particular script execution mechanism, or an approval interface.

- **Non-normative implementation example**

A Runner may allow an authorized Tool, such as a shell-execution Tool, to invoke a script packaged with a Skill when permitted by the applicable bindings, Loop access policy, and Runner execution controls. This is one possible implementation strategy, not an APDS requirement or an automatic privilege conferred by binding the Skill.

**Chapter 6 — Loops and authored orchestration**

- Define a Loop as a reusable, declarative orchestration contract that describes the phases, possible outcomes, transitions, and execution constraints of an Agent workflow. Distinguish the authored Loop graph from the runtime Agent executing it. Define the meaning of the existing manifest fields, including required `entry_phase`, `phases`, and `transitions`, and optional phase `outcomes`, `limits`, `checkpoints`, and `error_policy`. Do not introduce additional orchestration constructs beyond those supported by APDS v1.0.0.
- Define **phase and outcome semantics**:
    - Each phase has a stable identifier and an authored `objective` describing its intended purpose.
    - `entry_phase` identifies the phase where execution of the Loop begins.
    - Phases may declare explicit outcomes representing the possible results of completing that phase.
    - When a phase omits outcomes, it has exactly one implicit outcome with the identifier complete. This identifier is part of the APDS control-flow contract and may be referenced by transitions through on: "complete". A compatible Runner may use its own mechanism to determine phase completion, but must preserve the defined outcome identifier and its transition semantics.
    - Outcome identifiers are authored control-flow values, distinct from the specific mechanism a Runner uses to obtain or represent phase results.
    - APDS does not mandate a particular model-prompt format, structured-output mechanism, or phase-result serialization unless separately defined by the standard.
- Define **transition semantics and graph validity**:
    - A transition associates a source phase (`from`) and an outcome (`on`) with a destination (`to`).
    - Transition sources must identify declared phases, and outcome references must correspond to valid outcomes of their source phases, including the implicit `complete` outcome where applicable.
    - Transition destinations must identify another declared phase or a supported terminal target.
    - Define the applicable uniqueness, determinism, and transition-completeness requirements based on the established Loop contract. In particular, determine which combinations of source phase and outcome constitute ambiguous or invalid transitions; do not silently invent fallback transitions.
    - Validate phase identifier uniqueness, entry-phase existence, transition references, outcome identifiers, and checkpoint references at the appropriate validation level.
    - A Loop may contain branches and cycles. Its semantics must not assume a linear sequence or prohibit revisiting phases unless an explicit authored constraint requires it.
- Define the portable meaning of the three supported **terminal targets**:
    - `$end` represents normal termination of the Loop.
    - `$abort` represents termination through an abort path.
    - `$handoff` represents termination that yields control for a handoff rather than continuing the current Loop.
    - A compatible Runner must preserve these distinctions when interpreting authored transitions.
    - APDS does not mandate the particular output format, event representation, external handoff mechanism, or user interface used to realize terminal behavior.
- Define **authored execution limits**:
    - `limits.max_steps`, when declared, constrains the number of permitted Loop phase executions according to the existing contract.
    - A compatible Runner must not silently exceed an applicable authored limit.
    - Define the intended counting and limit-exhaustion semantics sufficiently to preserve the meaning of the declaration across implementations.
    - The mechanism for tracking execution counts, reporting limit exhaustion, and managing runtime resources remains implementation-defined except where necessary to preserve the authored constraint.
- Define **checkpoint semantics** according to the supported approval-checkpoint contract:
    - A checkpoint declares a control-flow condition associated with execution of a specified phase.
    - The existing `approval` checkpoint type represents an approval requirement before entering its declared `before_phase`.
    - `on_reject` identifies the declared transition destination when approval is rejected.
    - A compatible Runner supporting approval checkpoints must preserve their control-flow meaning and must not silently treat an unsatisfied checkpoint as approved.
    - APDS does not mandate a particular approval interface, human-interaction mechanism, callback API, or approval-storage implementation.
    - Clearly distinguish unsupported checkpoint capabilities from valid checkpoints that are supported but awaiting approval.
- Define **error-policy semantics** according to the existing manifest contract:
    - Distinguish Tool failure handling from phase failure handling where separately declared.
    - Define the authored meanings of supported actions such as retry, fail-phase, abort, and handoff, including retry limits and exhaustion behavior where applicable.
    - A compatible Runner supporting the declared error-policy construct must preserve the intended failure-handling behavior rather than silently replacing it with an unrelated policy.
    - APDS does not mandate internal exception types, retry scheduling algorithms, logging infrastructure, or error-presentation mechanisms.
    - Where an error policy is omitted, distinguish requirements explicitly defined by APDS from defaults or recovery decisions left to the Runner. Do not elevate incidental implementation defaults into normative behavior without deliberate justification.
- Define the relationship between **Loop access policy and Agent bindings**:
    - Loop phases may constrain effective Tool, Knowledge, and Memory capabilities according to their declared access policies.
    - Phase access restrictions apply to the capabilities made applicable through global bindings, phase bindings, and inherited dependencies.
    - A globally bound or inherited capability does not override a restriction declared by the active phase.
    - Preserve the distinction between direct Memory space access and Blueprint-governed Memory operations, as defined in the relevant chapters.
    - APDS defines the authored meaning of access restrictions, not the internal capability-filtering or execution-dispatch mechanism used by a Runner.
- Distinguish **Loop conformance from Runner executability**. A Loop may be structurally and semantically conformant independently of whether a particular Runner supports its declared execution features. Likewise, an Agent Package may conform to APDS without declaring a Loop. Runner compatibility and local execution readiness must be evaluated separately from artifact conformance.

**Implementation guidance for Codex and Claude Code — not normative APDS content**

- Reconcile the exact transition-determinism, outcome-completeness, step-counting, and omitted-policy behaviors against the existing Loop schema and established semantics before finalizing normative rules. Do not treat every Harness recovery behavior or default as an interoperability requirement.
- Treat execution mechanisms—including prompt assembly, model-driven outcome selection, retry scheduling, approval UI, internal event buses, tracing, and output rendering—as Runner implementation decisions unless a specific observable guarantee is necessary to preserve the authored Loop contract.
- The AgentPM website may simplify a Loop into a linear visual summary for readability, but must not misrepresent branches, cycles, or terminal transitions. This is an AgentPM UI requirement, not a normative APDS rule.
- Where existing Runner behavior conflicts with the portable interpretation of an authored Loop declaration, document the discrepancy for reconciliation rather than automatically incorporating the Runner's behavior into APDS.

**Chapter 7 — Instruction Profiles**

- Define an Instruction Profile as a reusable declaration of an Agent's intended identity, behavioral guidance, communication style, and constraints. Describe the supported fields according to the existing manifest schema, including:
    - `identity`: role, description, and expertise.
    - `objectives` and `principles`: durable priorities and decision-making guidance.
    - `audience`: intended audience, assumed knowledge, and adaptation guidance.
    - `communication`: tone, verbosity, guidelines, formatting, and vocabulary preferences.
    - `boundaries`: authored limits on the role's intended behavior or authority.
    - `constraints`: behavioral instructions identified by stable IDs and an authored `strength` of `required` or `preferred`.
    - `compatibility`: advisory context requirements and capability expectations expressed through `minimum_context_tokens`, `requires`, and `recommends`.
    - Preserve the meanings and structural restrictions of the existing schema rather than introducing additional Profile capabilities.
- Define **Profile binding and composition semantics**:
    - A globally bound Profile contributes its authored behavioral guidance throughout the Agent run.
    - A phase-bound Profile contributes its guidance within the declared phase.
    - Global and phase-bound Profiles are additive. A phase-bound Profile does not implicitly replace or remove globally applicable Profiles.
    - Multiple applicable Profiles contribute to the Agent's effective behavioral guidance within their respective scopes.
    - Profile references must resolve through the Agent's declared dependency composition, preserving the intended Profile identity and version.
    - APDS defines the applicability of Profile guidance but does not mandate a particular prompt-assembly order, concatenation strategy, de-duplication algorithm, or internal representation of the composed guidance.
- Define the meaning of **behavioral guidance and constraints**:
    - Profile identity, objectives, principles, audience, and communication declarations express the author's intended behavior for an Agent operating with that Profile.
    - Boundaries describe behavior or authority the Profile instructs the Agent to avoid.
    - Constraints with `strength: "required"` express stronger authored behavioral requirements than those marked `preferred`. This distinction must be preserved when interpreting the Profile.
    - These declarations describe intended Agent behavior; they do not independently grant capabilities, authorize actions, override Loop access restrictions, or supersede applicable runtime authorization and safety controls.
    - APDS defines the authored meaning and relative strength of Profile guidance without guaranteeing that a probabilistic model will satisfy every behavioral instruction in every execution.
- Define **compatibility declarations and capability hints**:
    - `compatibility.minimum_context_tokens` expresses an authored context-capacity expectation.
    - `compatibility.requires` identifies capabilities the author considers necessary for the intended use of the Profile.
    - `compatibility.recommends` identifies capabilities considered beneficial but not necessary.
    - Capability hints such as `tool_use`, `structured_output`, and `multimodal_input` describe compatibility expectations; they do not grant access to those capabilities or require a Runner to provide them.
    - The distinction between required and recommended capabilities must be preserved when interpreting compatibility information.
    - Compatibility declarations may inform Runner capability checks, warnings, or compatibility decisions, but do not establish universal executability or independently determine APDS conformance based on the capabilities of a particular runtime.
- Define the relationship between **Profile guidance and other Agent declarations**:
    - The Agent's top-level `description` expresses its general purpose.
    - An Instruction Profile defines reusable role identity, behavioral objectives, principles, communication guidance, and constraints.
    - A Loop phase's `objective` describes the intended purpose of that execution phase.
    - These declarations have distinct authored meanings and may contribute to the Agent's effective execution context without being interchangeable.
    - Profile guidance does not replace the Agent's purpose or the Loop's control-flow and access-policy semantics.
    - APDS does not mandate that these declarations be inserted into a particular model prompt, in a particular order, or using a particular instruction hierarchy.
- Define the boundary between **portable Profile semantics and Runner interpretation**:
    - A compatible Runner claiming support for Instruction Profiles must preserve the applicability, scope, and intended meaning of supported Profile declarations.
    - APDS does not prescribe how a Runner converts Profile guidance into model instructions, manages repeated guidance, resolves competing natural-language instructions, or represents behavioral context internally.
    - Where multiple applicable Profiles contain conflicting guidance, do not invent universal precedence rules based solely on global versus phase scope, declaration order, or implementation-specific prompt construction.
    - Any normative conflict-resolution or precedence behavior must be explicitly defined as part of the portable standard rather than inferred from an existing Runner's implementation.

**Implementation guidance for Codex and Claude Code — not normative APDS content**

- Review existing Profile composition and prompt-building behavior to identify the authored semantics already established by the manifest and Agent binding model. Do not automatically convert current Harness de-duplication, concatenation order, or prompt-section ordering into normative requirements.
- Distinguish de-duplicating repeated references to the same resolved Profile from de-duplicating natural-language instruction content. Neither requires a prescribed implementation algorithm, but an implementation must preserve the intended applicability of the authored guidance.
- Preserve the distinction between author-declared behavioral requirements and runtime-enforced capability restrictions. A Profile constraint marked `required` is not equivalent to a Tool permission or Loop access policy.
- If existing Profile conflicts reveal a need for defined precedence semantics, document the issue for deliberate consideration rather than inventing a rule based on current prompt ordering.
- Keep AgentPM-specific prompt assembly, instruction merging, and runtime compatibility-check strategies in implementation documentation rather than the normative APDS specification.

**Chapter 8 — Knowledge**

- Define a Knowledge package as a reusable artifact containing authored context or prepared retrieval data that may be consumed by an Agent. Describe the supported manifest fields according to the existing schema, including `mode`, `content_type`, `language`, `documents`, `context`, `corpus`, `chunking`, `embedding`, `indexes`, `retrieval`, and `provenance`. Preserve the distinction between authored declarations, build-derived metadata, and packaged data without requiring a particular Knowledge-building tool or workflow.
- Define the portable meaning of the two supported **Knowledge modes**:
    - `context` identifies Knowledge intended for direct contextual use. Its declared documents and associated metadata describe content that a consuming Runner may make available to an Agent.
    - `vector` identifies a prepared retrieval corpus containing chunk/source data and associated embeddings, with optional packaged retrieval indexes.
    - Vector-mode Knowledge requires the corpus and embedding declarations established by the versioned schema, including the required paths and embedding metadata.
    - The modes define the type and intended use of packaged Knowledge, not the internal mechanism by which a Runner loads documents, constructs model context, retrieves information, or presents results.
    - APDS does not require a Runner to inject an entire context-mode corpus into a model prompt or use a particular retrieval technique for vector-mode Knowledge.
- Define **packaged content and metadata relationships**:
    - Document, corpus, embedding, index, and provenance paths identify content within the Knowledge artifact according to the supported relative-path rules.
    - Declared paths and identifiers must retain their intended relationships to the packaged artifacts.
    - Vector corpus metadata describes the prepared chunks and sources; embedding metadata describes the associated vectors, including their identifier, provider/model labels, dimensions, similarity metric, normalization state, and payload location.
    - When retrieval indexes are declared, their references to embedding identifiers must correspond to the intended embedding data in the package.
    - Where the contract defines counts, hashes, dimensions, or other verifiable relationships, implementations performing the applicable validation level should verify their consistency with the available artifact data.
    - Distinguish validation of manifest metadata from validation requiring access to packaged files. Missing artifact content during standalone manifest validation must not automatically be treated as a confirmed semantic violation.
    - Do not introduce additional mandatory file layouts, index formats, embedding algorithms, or metadata relationships beyond those established by APDS v1.0.0.
- Define **embedding and retrieval compatibility semantics**:
    - Embedding declarations describe properties of the prepared vectors and provide information necessary to evaluate retrieval compatibility.
    - Dimensions, metric, normalization state, embedding identity, and declared index relationships must retain their authored meaning.
    - A consuming Runner must not silently reinterpret incompatible embedding or index metadata as though it were compatible.
    - The existence of a valid vector Knowledge package does not guarantee that a particular Runner supports its embedding representation, retrieval index, or required query-time capabilities.
    - APDS does not mandate a particular embedding provider, vector database, similarity-search implementation, query-embedding service, or retrieval algorithm.
- Define **retrieval declarations and defaults**:
    - Retrieval metadata, where declared, expresses authored retrieval preferences or default behavior, including supported fields such as `strategy`, `default_top_k`, `default_score_threshold`, and `return_citations`.
    - Distinguish descriptive strategy labels and advisory defaults from requirements that are explicitly normative under the APDS contract.
    - A compatible Runner may implement retrieval through different mechanisms while preserving the meaning of the supported declarations it interprets.
    - Do not require a particular ranking algorithm, score normalization scheme, citation presentation format, or context-window assembly strategy unless explicitly established by the standard.
- Define **provenance semantics**:
    - Provenance metadata describes the declared origin or preparation of Knowledge, including available source-manifest references, generation timestamps, and builder information.
    - Authored or build-generated provenance metadata must not automatically be interpreted as independently verified provenance.
    - APDS may define the structural and semantic validity of provenance declarations, but authentication, attestations, signatures, and independent verification belong to separate trust and distribution mechanisms unless explicitly incorporated into the definition contract.
    - A valid Knowledge package is not necessarily current, accurate, trustworthy, or suitable for every use case.
- Define **Knowledge binding and availability semantics**:
    - An Agent's Knowledge dependency and binding declarations identify which Knowledge packages are applicable in global or phase-specific scopes.
    - Globally bound Knowledge is applicable throughout the Agent run, while phase-bound Knowledge is applicable only within its declared phase, subject to other independently applicable bindings.
    - Loop phase access restrictions may prohibit Knowledge use even when the package is otherwise bound and available.
    - The validity of a Knowledge artifact is independent of whether a particular Runner supports its mode, can access its content, or permits Knowledge use in the current phase.
    - APDS defines the authored availability and access semantics, not the Runner's storage, loading, caching, retrieval, or model-context construction strategy.

**Implementation guidance for Codex and Claude Code — not normative APDS content**

- Reconcile the exact required and optional fields for `context` and `vector` modes against the existing versioned schema. In particular, preserve the current distinction between required vector corpus/embedding declarations and optional retrieval indexes.
- Separate intrinsic manifest validation, packaged-file consistency validation, and runtime retrieval compatibility. Do not claim that a schema-valid manifest proves every referenced file exists or that its embeddings are usable by every Runner.
- Review the current Knowledge build output and metadata relationships to identify meaningful portable invariants. Do not automatically standardize AgentPM-specific build commands, directory conventions, index-building algorithms, or runtime loading strategies.
- Treat retrieval defaults and open-ended strategy labels according to their actual declared meaning. Avoid converting descriptive metadata or advisory preferences into strict execution requirements without a deliberate interoperability reason.
- Keep AgentPM-specific Knowledge Runtime behavior, provider configuration, indexing implementations, and Package Health presentation outside the normative APDS specification.
- Where the existing implementation relies on a specific Knowledge format or retrieval strategy that is not already part of the portable contract, document it as an implementation capability or compatibility constraint rather than silently making it a universal APDS requirement.

**Chapter 9 — Memory Blueprints, spaces, and lifecycle operations**

- Define a Memory Blueprint as a reusable, declarative contract describing an Agent's intended durable Memory structures, record formats, retrieval capabilities, lifecycle policies, and operations. A Memory Blueprint defines **what durable Memory represents and how it is intended to behave**, not which storage engine or runtime service implements it. Describe the supported manifest fields according to the versioned schema, including `scopes`, `record_types`, `spaces`, and optional `operations`. Do not introduce additional Memory models, operation types, or execution requirements beyond those established by APDS v1.0.0.
- Define **Memory scopes and record contracts**:
    - `scopes` declares reusable logical scope keys used to distinguish or partition Memory records.
    - `record_types` defines named record content contracts, each with its own version, description, and reference to a packaged JSON Schema.
    - Record type versions are independent of the Memory Blueprint's artifact version and the APDS standard version.
    - Record schemas define the expected structure of durable record content. A compatible implementation supporting record validation must preserve those authored content constraints.
    - A Memory space's `scope` declaration identifies the ordered scope keys that participate in its logical record identity. The meaning of the declared scope keys must be preserved without requiring a particular database key format, tenant model, or storage partitioning strategy.
    - A Memory space's `record_types` declaration identifies which declared record types the space permits.
    - References between spaces, scope keys, and record types must resolve to the corresponding declarations within the Blueprint.
- Define the three supported **Memory space models**:
    - `document` represents a logical Memory space organized around document-style durable content.
    - `collection` represents a logical Memory space containing multiple records that can be managed and retrieved according to its declared contract.
    - `sequence` represents a logical Memory space whose records have meaningful ordering.
    - Each model must preserve its distinct authored meaning. A compatible implementation must not silently treat an ordered sequence as an unordered collection, or otherwise discard distinctions necessary to interpret the declared model.
    - Define any applicable cardinality, ordering, identity, and update semantics that are explicitly established by the APDS v1.0.0 contract. Do not invent additional restrictions based solely on a particular storage backend's implementation.
    - APDS does not require a relational database, document store, vector database, filesystem layout, or other particular persistence technology.
- Define **retrieval semantics**:
    - Each Memory space declares supported logical retrieval modes through `retrieval.modes`.
    - The current supported modes are `key`, `filter`, `chronological`, `full_text`, and `semantic`.
    - These modes describe the retrieval capabilities the Memory space is intended to expose, not the internal algorithms used to implement them.
    - `chronological` retrieval must preserve the relevant ordering semantics of the stored records.
    - `semantic` retrieval expresses a requirement for meaning-based retrieval capability without prescribing an embedding provider, vector representation, or search algorithm.
    - A consuming Runner must distinguish supported retrieval modes from modes it cannot realize. An unsupported retrieval capability does not, by itself, make the authored Blueprint nonconformant.
    - Do not prescribe query languages, ranking algorithms, embedding models, or API response formats unless explicitly defined elsewhere in the APDS contract.
- Define **capacity, retention, and space constraints**:
    - `capacity.max_records` expresses an authored maximum record count for a space.
    - `retention.ttl` expresses the intended duration for which applicable records remain active.
    - `retention.on_expire` distinguishes the supported `delete` and `archive` outcomes for expired records.
    - `constraints.append_only`, when declared, expresses the authored restriction on modifying existing records versus appending new records.
    - These declarations have distinct meanings and must not be silently reinterpreted as interchangeable advisory labels.
    - Define the portable meaning of each declaration and the observable requirements for implementations that claim to enforce it.
    - Do not prescribe a particular cleanup scheduler, transaction mechanism, archival storage system, or garbage-collection strategy.
    - Distinguish an APDS-conformant Blueprint from a runtime environment that can actually satisfy its declared capacity, retention, and persistence requirements.
- Define **direct Memory space bindings**:
    - An Agent may bind individual Memory spaces globally or within specific phases.
    - A globally bound Memory space establishes a directly accessible Memory surface throughout the Agent run, subject to applicable Loop access restrictions and runtime support.
    - A phase-bound Memory space establishes a directly accessible surface within that phase only, unless independently bound through another applicable scope.
    - Global and phase bindings are additive; binding a space in a phase does not remove an independently declared global binding.
    - Direct space bindings determine which Memory surfaces are available for direct Agent access. They do not independently define the inputs, outputs, or targets of Blueprint lifecycle operations.
    - Preserve the distinction between direct Memory reads/writes and operation-mediated Memory access. A space not directly bound in a phase is not thereby prohibited from being accessed by an eligible, authorized Blueprint operation.
- Define **Memory lifecycle operation types**:
    - `consolidate` declares an operation that derives destination Memory records from one or more source record contracts.
    - `transform` declares an operation that converts source record content into a destination record contract, subject to its declared output behavior.
    - `delete` declares an operation targeting one or more Memory spaces for deletion according to its authored policy.
    - Each operation has its own declared identity, description, trigger, and type-specific references or policies.
    - Consolidation and transformation operations declare their source inputs and output destinations using space-and-record-type references.
    - Delete operations declare target spaces and the supported derived-record cascading policy.
    - Operation references must identify valid declared spaces and applicable record types. Validation must distinguish invalid references from cases where required artifact content is unavailable.
    - APDS defines the meaning of these operations and their declared relationships, not the algorithm used to summarize, transform, generate, or otherwise process record content.
- Define **Memory operation binding and participation semantics**:
    - A Memory operation is a separately bindable lifecycle capability, distinct from directly exposing a Memory space to the Agent.
    - A globally bound operation participates throughout the Agent run, subject to its Blueprint trigger and applicable execution restrictions.
    - A phase-bound operation participates only within its declared phase, subject to its Blueprint trigger and applicable execution restrictions.
    - Global and phase operation bindings are additive.
    - Binding an operation establishes its participation scope; the operation becomes eligible according to its declared trigger rather than merely because a phase has started or a binding exists.
    - **An operation may read, write, transform, or delete records in spaces that are not directly bound in the current phase**, provided those spaces are valid targets under the operation's declared contract and its execution is otherwise permitted.
    - An implementation must not require every operation input, output, or target space to be independently exposed through a direct Agent space binding.
    - Operation participation and operation eligibility must be distinguished from actual execution. A Runner may implement different scheduling and dispatch mechanisms while preserving the meaning of the authored trigger and operation scope.
- Define the four supported **Memory operation triggers**:
    - `external` declares an operation intended to be invoked through an explicit external or runtime action. Merely declaring or binding an externally triggered operation does not schedule or execute it automatically.
    - `interval` declares recurring eligibility based on the authored elapsed-time interval expressed by `every`.
    - `record_count` declares eligibility when the referenced Memory space reaches the specified record-count `threshold`.
    - `capacity` declares eligibility when the referenced Memory space reaches its declared capacity.
    - Trigger references must identify valid spaces where applicable. A capacity trigger must refer to a space with a meaningful declared capacity.
    - Define trigger eligibility independently from the internal mechanism used to detect the condition or schedule execution.
    - Do not prescribe polling frequencies, timer implementations, background worker architectures, or trigger-dispatch algorithms.
    - Clarify that trigger eligibility does not necessarily imply immediate execution, successful completion, or availability of the required backend.
    - Preserve the distinction between a Blueprint's trigger definition, an Agent's operation binding scope, and a Runner's ability to invoke or schedule the operation.
- Define **operation output and source-handling policies**:
    - `source_handling` expresses the authored disposition of source records after a successful consolidation or transformation.
    - Preserve the distinct meanings of `retain`, `retain_until_expiration`, and `delete_after_success`.
    - A transform's `output_mode`, when declared, distinguishes creation of a new output record from replacement of the input record where supported.
    - `preserve_provenance` expresses whether the operation should retain source-record provenance in derived output.
    - A delete operation's `cascade_derived_records` declaration expresses whether derived records are included in its intended deletion scope.
    - Define the portable meaning of successful completion and the relevant source/output relationships sufficiently to preserve authored lifecycle intent.
    - APDS does not mandate a particular transaction engine, record-processing algorithm, retry strategy, or internal provenance representation unless necessary to preserve an explicitly defined observable guarantee.
- Define **Memory governance and data-handling annotations**:
    - Record content schemas may contain supported governance annotations describing data classification, sensitivity, persistence restrictions, or sharing restrictions.
    - Define the recognized annotation names and values according to the APDS v1.0.0 contract, including currently supported `x-agentpm-*` annotations where applicable.
    - Distinguish descriptive classification metadata from explicit restrictions such as non-persistable or non-shareable content.
    - A compatible implementation claiming support for these restrictions must preserve their intended meaning and must not silently reinterpret restrictive declarations as permission to retain or disclose data.
    - APDS does not establish universal privacy-law compliance, encryption requirements, identity management, organizational policy, or regulatory certification merely because a Blueprint includes governance annotations.
    - Where the existing governance annotations do not define an enforceable portable contract, clarify their status rather than inventing enforcement obligations from their names alone.
- Define the relationship between **Memory and Loop access policy**:
    - Loop phases may constrain Memory read and write access according to their declared policies.
    - Direct Memory space access is governed by the applicable Agent binding scope and Loop access restrictions.
    - Blueprint lifecycle operations are governed by their declared operation contracts, triggers, binding participation scopes, and applicable execution restrictions.
    - A restriction on directly exposed Memory surfaces must not automatically be interpreted as a requirement to directly bind every space targeted by an operation.
    - APDS should define the intended interaction between authored Memory access restrictions and operation eligibility without assuming that all Memory access occurs through direct Agent-facing read/write operations.
    - Do not invent additional authorization paths or imply that lifecycle operations bypass applicable restrictions merely because their targets need not be directly bound.
- Define **Memory conformance and execution boundaries**:
    - Intrinsic Blueprint validation establishes that the declared scopes, record types, spaces, retrieval modes, operations, and triggers satisfy their applicable structural and semantic requirements.
    - Validation involving packaged record schemas must verify applicable schema references and relationships when those artifacts are available.
    - Resolved-composition validation establishes that Agent Memory bindings reference the intended Blueprint, spaces, and operations, and that phase references are valid.
    - APDS conformance does not prove that a durable Memory store exists, that records have actually been persisted, that a Runner supports every retrieval capability, or that retention and lifecycle operations have executed successfully.
    - Compatible Runners must preserve the semantics of the Memory constructs they claim to support, but may use different backends, scheduling systems, storage layouts, content-generation techniques, and execution architectures.
    - Missing backend capabilities or unavailable runtime services must be distinguished from invalid authored Memory declarations.

**Implementation guidance for Codex and Claude Code — not normative APDS content**

- Reconcile the exact Memory fields, optionality, operation variants, record-schema requirements, and cross-reference rules against the existing manifest schema and Phase 6C/7A decisions before finalizing APDS v1.0.0.
- Treat the established distinction between **direct space bindings and operation bindings** as a fixed semantic requirement. This includes the ability of an eligible operation to target valid spaces that are not directly bound in the active phase. Do not inadvertently weaken or reverse this behavior while formalizing the standard.
- Reconcile the exact interaction between Loop Memory read/write policy and Blueprint lifecycle operations. Do not assume that a Loop's direct-space restrictions automatically invalidate an otherwise valid operation target, and do not invent an unrestricted operation execution bypass.
- Distinguish trigger eligibility, operation participation, invocation, and execution. In particular, preserve externally triggered operations as explicitly invoked capabilities rather than treating their declaration as permission for autonomous background execution.
- Reconcile omitted policy values, transform output modes, source handling, provenance, and derived-record deletion behavior against the current contract. Do not automatically elevate Harness-specific scheduling, retry, or cleanup defaults into normative APDS requirements.
- Review the currently supported governance annotations, including their `x-agentpm-*` names, and distinguish which have defined portable meanings from those that are descriptive or dependent on implementation capabilities. Do not claim universal privacy or security guarantees without an explicit contract.
- Create conformance fixtures covering all three space models, supported retrieval modes, valid and invalid cross-references, the four trigger types, global versus phase operation scopes, operation targets that are not directly bound, source-handling policies, and missing runtime capabilities.
- Where a proposed Memory semantic is not clearly established by the current schema or prior design decisions, document it for deliberate reconciliation rather than inferring it from the behavior of one Memory Runtime implementation.

**Chapter 10 — Templates and project scaffolding**

- Define a Template as a reusable artifact describing how to generate a project or workspace from packaged files, declared variables, dependencies, and entrypoints. Templates are a distinct APDS artifact kind, not synonymous with Agent Packages. Instantiating a Template creates a project or workspace that may contain or depend on Agent Packages and Components; the Template itself does not become an Agent Package merely because it generates one. APDS does not require Templates to be published to or retrieved from a particular Registry.
- Define the supported **Template manifest structure** according to the versioned schema:
    - `display_name` and `use_case` describe the Template's intended purpose.
    - `stack` optionally describes the technologies or project environments associated with the Template.
    - `execution_surfaces` declares the execution or integration environments the generated project is intended to support.
    - `files_root` identifies the packaged directory containing the source files used to instantiate the Template.
    - `variables` declares generation-time configuration values.
    - `dependencies` identifies the Agent Packages and Components associated with the generated project.
    - `entrypoints` declares suggested commands for interacting with the generated project.
    - Preserve the existing field shapes, required properties, and supported values rather than introducing a new scaffolding contract.
- Define **Template file and instantiation semantics**:
    - `files_root` identifies a safe, artifact-relative location containing the files from which the generated project is constructed.
    - Template instantiation produces project files derived from the packaged content and applicable variable substitutions.
    - Define the supported `{{ variable_name }}` placeholder syntax and its relationship to declared Template variables, according to the existing contract.
    - File references and instantiation behavior must preserve the intended packaged content and must not permit authored relative paths to escape their permitted boundaries.
    - APDS does not prescribe a particular filesystem API, destination-directory naming convention, interactive setup wizard, or project-generation command.
- Define **Template variable semantics**:
    - Variables have declared names, descriptions, required status, and optional defaults.
    - Required variables express values that must be resolved for the generated project where applicable.
    - Declared defaults provide values when no explicit value is supplied.
    - Variable substitution must preserve the distinction between declared placeholders and unrelated file content.
    - Template variables are intended for non-secret generation-time configuration, not for embedding runtime credentials into published artifacts.
    - APDS defines the meaning of variable declarations and supported substitution syntax without requiring a particular prompting interface, configuration source, or secrets-management implementation.
- Define **Template dependency semantics**:
    - `dependencies` identifies Agent Packages and reusable Components associated with the generated project, using the supported dependency groups and reference formats.
    - These references retain their declared artifact identities, kinds, and version constraints.
    - Template dependencies describe the intended composition or requirements of the generated workspace; they are distinct from the Template artifact's own identity.
    - APDS does not mandate when or how dependencies are installed, which package manager performs resolution, or how the resulting workspace stores its resolved dependency graph.
- Define **execution surface semantics**:
    - `execution_surfaces` declares the intended execution or integration environments for the generated project.
    - Define the meanings of the existing supported values, including `python-sdk`, `node-sdk`, `agentpm-run`, `agentpm-harness`, `agentpm-serve-mcp`, and `multi-agent-workspace`.
    - An execution surface declaration expresses authored compatibility intent. It does not guarantee that the consuming environment has the required dependencies, services, credentials, or runtime capabilities.
    - A compatible implementation must preserve the meaning of execution surface identifiers it interprets, but is not required to provide every declared execution environment.
    - APDS does not mandate a particular Runner, SDK implementation, process-launch mechanism, or execution interface.
- Define **Template entrypoint semantics**:
    - Each entrypoint declares a human-readable `label` and an associated `command` intended for use with the generated project.
    - Entrypoints identify available or recommended ways to interact with the generated workspace.
    - Declaring an entrypoint does not automatically execute its command during Template instantiation.
    - APDS does not mandate a particular command executor, shell, terminal interface, or automated execution policy.
    - The ability to execute an entrypoint depends on the consuming environment and its applicable execution controls.
- Distinguish **Template conformance, successful instantiation, and project executability**:
    - APDS conformance establishes that the Template's authored structure and semantic relationships satisfy the applicable definition contract.
    - Validation involving packaged files or referenced dependencies may require additional artifact context beyond the standalone manifest.
    - Successful Template instantiation establishes that the intended project files were generated, but does not by itself establish that every declared dependency is installed or every entrypoint can execute successfully.
    - A generated project may target a Runner or integration environment that is unavailable to the current consumer without making the Template itself nonconformant.
    - APDS does not require a particular CLI command, Registry workflow, or project bootstrap implementation.

**Implementation guidance for Codex and Claude Code — not normative APDS content**

- Reconcile the exact Template field requirements, supported dependency groups, execution surface identifiers, file path rules, variable placeholders, and entrypoint structures against the existing versioned manifest schema.
- Preserve the distinction between the Template artifact, the generated workspace, and any Agent Packages or Components referenced by that workspace.
- Review `agentpm new` as one implementation of Template instantiation. Its command syntax, prompts, generated workspace configuration, dependency installation behavior, and user experience must not automatically become normative APDS requirements.
- Define the portable meaning of `agentpm-harness` and other existing execution surface identifiers without requiring independent implementations to use AgentPM's CLI or Harness.
- Distinguish Template definition validation from packaged-file verification, successful project generation, dependency installation, and runtime execution.
- Preserve the existing non-secret purpose of Template variables. Do not introduce requirements for a universal credential-management system or imply that manifest validation can detect every sensitive value.
- Where the current Template generator has implementation-specific defaults for file copying, substitution, overwrite handling, or workspace layout, do not automatically elevate those details into normative requirements unless they are necessary for the portable meaning of an authored Template.

**Chapter 11 — Conformance, interpretation, and compatibility**

- Define **APDS conformance as a property of an artifact and its applicable authored relationships**, evaluated against the normative requirements of its declared standard version. Distinguish an artifact's actual conformance from a particular implementation's ability to verify that conformance. A successful validation result establishes only that the requirements within the performed validation scope were satisfied.
- Define **validation levels and their applicability**:
    - **Standalone validation** evaluates structural and intrinsic semantic requirements that can be determined from an artifact without resolving external dependencies.
    - **Resolved-composition validation** evaluates semantic relationships requiring referenced artifacts, such as dependency kinds, binding targets, Loop phase references, and cross-artifact composition constraints.
    - **Packaged-content validation**, where applicable, evaluates requirements involving files or generated artifacts referenced by the manifest, such as Knowledge payload metadata or Memory record schemas.
    - Passing one validation level does not imply that all other applicable levels have been evaluated or satisfied.
    - Requirements that cannot be evaluated because the necessary dependency graph or artifact content is unavailable must not automatically be treated as violations.
- Define the **meaning of conformance outcomes**:
    - **Conformant:** All applicable normative requirements within the completed validation scope have been satisfied.
    - **Nonconformant:** One or more definite violations of applicable normative requirements have been established.
    - **Incomplete / not fully evaluated:** The available information is insufficient to determine whether all applicable requirements are satisfied.
    - **Unsupported:** The implementation does not support the declared standard identity/version, validation level, or relevant interpretation capability.
    - Clearly distinguish an established violation from an inability to evaluate a requirement.
    - These terms describe logical validation outcomes. APDS does not mandate a particular API response schema, result enum, diagnostic format, or user-interface representation unless separately standardized.
- Define **conformance claims and verification boundaries**:
    - A validator must not claim verification of requirements it has not evaluated.
    - A successful standalone validation result must not be presented as proof of full resolved-composition conformance.
    - An implementation may support only particular validation levels or conformance roles, provided its claims accurately describe that support.
    - A validator must not silently substitute another standard version when the declared version is unsupported.
    - A declaration identifying a supported APDS standard does not itself establish that the artifact satisfies that standard.
    - Unsupported validation must not be misrepresented as either successful verification or a confirmed semantic violation.
- Define **standard-version interpretation**:
    - An artifact's declared `standard` identifies the normative structural and semantic contract against which its conformance is evaluated.
    - Compatible implementations must interpret supported declarations according to the identified standard version rather than silently applying unrelated or newer semantics.
    - Changes to a standard's normative requirements must follow its versioning rules.
    - Implementations may support multiple standard versions, but support for one version does not imply support for every other version.
    - Compatibility mechanisms for artifacts created under older or different definition contracts must remain distinguishable from explicit conformance to the declared APDS version.
- Distinguish **artifact conformance from distribution, runtime compatibility, and execution readiness**:
    - **APDS conformance** concerns the validity and portable meaning of the authored artifact and its applicable relationships.
    - **Distribution trust** concerns matters such as artifact provenance, signatures, attestations, permissions, and security scanning.
    - **Runner compatibility** concerns whether a particular Runner supports the declared standard and relevant execution constructs.
    - **Execution readiness** concerns whether the current environment has the required configuration, services, credentials, and resources.
    - These are independent determinations. None establishes the others automatically.
    - A conformant Agent Package may be unsupported by a particular Runner or not ready to execute in a particular environment without becoming nonconformant.
- Define **conformance reporting and diagnostic principles**:
    - Normative requirements should have stable rule identifiers that allow validation findings to be associated with the relevant standard requirements.
    - Validation results should identify the evaluated standard version and scope sufficiently to support accurate interpretation of the outcome.
    - Findings should distinguish confirmed violations, unevaluated requirements, and unsupported capabilities.
    - APDS may provide shared conformance fixtures and expected semantic outcomes to support independent implementations.
    - The standard does not require identical diagnostic wording, error serialization, reporting interfaces, or internal validation architecture.

**Implementation guidance for Codex and Claude Code — not normative APDS content**

- Implement the distinction between standalone, resolved-composition, and packaged-content validation using the actual requirements of APDS v1.0.0. Not every artifact kind necessarily requires every validation level.
- Use shared conformance fixtures to verify that the Rust CLI and Python Registry validators agree on applicable normative outcomes. They need not have identical internal architectures or user-facing diagnostic formats.
- Preserve AgentPM's legacy compatibility behavior for previously published artifacts that lack an explicit `standard` declaration. Legacy inference must not modify immutable published artifacts, recompute historical integrity values, invent manifest declarations, or imply that validation occurred when it did not.
- Keep legacy-inferred standard interpretation distinguishable from explicitly declared and independently verified APDS conformance in Registry metadata, CLI output, lockfile handling, and Package Health.
- Preserve the distinction between APDS validity, Registry publication/security checks, Harness compatibility, and local Harness readiness.
- Do not automatically classify missing dependency context, unavailable runtime services, unsupported Runner capabilities, or unsupported standard versions as APDS semantic violations.
- Codex and Claude should determine the most maintainable internal validation result representation, provided these distinctions remain observable and tests prevent misleading conformance claims.
- Do not introduce a universal validation API, certification process, or third-party conformance service as part of APDS v1.0.0.

#### Concrete binding and composition cases for APDS conformance fixtures

The following cases define expected interpretations of authored APDS relationships and provide a shared behavioral reference for independent validators and compatible Runners.

These are **semantic expectations**, not a requirement to copy the examples directly into manifests. Codex must create schema-valid fixture artifacts using the actual APDS v1.0.0 field structures, supported references, and declared dependency relationships.

Distinguish three forms of verification:

- **Standalone validation:** Confirm intrinsic manifest validity without requiring referenced artifacts.
- **Resolved-composition validation:** Confirm cross-artifact relationships, dependency identities, binding references, phase references, and other semantics that require a resolved graph.
- **Runner interpretation tests:** Confirm that a Runner claiming support for a construct preserves its normative meaning during execution or capability evaluation.

Not every case requires all three forms of verification. Conformance fixtures should establish expected normative outcomes without requiring every APDS implementation to provide a Runner or adopt AgentPM-specific execution mechanisms.

| Case | Authored composition / bindings | Relevant conditions | Required APDS interpretation |
|---|---|---|---|
| **A — Additive Tool bindings** | Skill S declares Tool T and is bound globally. Tool U is bound to phase `draft`. | Tool access is permitted in the applicable phases. | During `draft`, both T and U are applicable Tool candidates. Other phases retain T through the global Skill binding but do not inherit U solely from its `draft` binding. |
| **B — Inherited Tool restricted by Loop** | Skill S declares Tool T and is bound globally. | Phase `review` prohibits Tool use. | T remains part of the authored composition but cannot be exercised in `review`. Skill inheritance does not bypass Loop access restrictions. |
| **C — Phase-scoped Skill inheritance** | Skill S declares Tool T and is bound only to phase `research`. | The Loop also contains phase `publish`. | T is applicable through S in `research`, but not in `publish` unless independently available through another applicable binding. |
| **D — Invalid phase reference** | Skill S is bound to phase `nonexistent`. | The resolved Loop declares only `research` and `publish`. | Resolved-composition validation establishes an invalid phase reference. This is an authored composition violation, not a missing runtime capability. |
| **E — Profile capability hints** | Profile P declares `compatibility.requires.tool_use: true` or an equivalent supported capability expectation. | The active Loop phase prohibits Tool use. | P's compatibility declaration does not grant Tool access or override Loop policy. The Profile can remain APDS-conformant independently of whether the selected Runner satisfies its declared compatibility expectations. |
| **F — Memory operation targeting an unbound space** | Memory operation O is bound in phase `review` and declares Memory space X as an input, output, or target. X is not directly bound in `review`. | O's trigger is eligible and its execution is permitted under applicable policy. | O may access X through its declared operation contract. X does not become a directly accessible Agent Memory surface merely because O references it. Direct space binding is not required solely for operation-mediated access. |
| **G — Externally triggered Memory operation** | Operation O declares trigger `external` and is bound globally. | No explicit invocation of O occurs. | Binding O does not automatically execute or schedule it. Its declared trigger requires explicit invocation through a supported execution mechanism. |
| **H — Conformant Agent without a Loop** | An otherwise valid Agent Package declares neither a Loop nor Tool dependencies. | The manifest satisfies its applicable APDS structural and intrinsic semantic requirements. | The Agent Package is not nonconformant merely because it lacks a Loop or Tools. Whether a particular Runner can execute it is a separate compatibility/readiness determination. |
| **I — Versioned dependency identity** | Skill S and Tool T each exist in multiple installed versions. The Agent's bindings reference their versionless identities. | The Agent has a resolved composition selecting specific versions. | References resolve to the appropriate artifacts within that Agent's resolved dependency context, not arbitrary installed artifacts with matching names. APDS does not mandate a particular lockfile or dependency-resolution implementation. |
| **J — Multiple paths to the same Tool** | Tool T is bound directly and is also inherited through globally bound Skill S. | Both paths resolve to the same Tool artifact/version. | The paths do not create additional execution permissions or alter T's capability identity. APDS does not mandate a specific internal de-duplication algorithm or presentation format. |
| **K — Additive Instruction Profiles** | Profile A is bound globally; Profile B is bound to phase `review`. | Both Profiles are valid and resolved. | Both A and B contribute applicable guidance during `review`. Other phases receive A without automatically receiving B. APDS does not prescribe prompt concatenation order, instruction merging, or natural-language conflict resolution. |
| **L — Direct Memory space binding scope** | Space X is bound globally; space Y is bound only in phase `review`. | Direct Memory access is permitted by the applicable Loop policy. | X remains directly applicable across phases. Y is directly applicable in `review` but not other phases solely because of that binding. Read/write permissions remain subject to the applicable Loop restrictions. |
| **M — Phase-scoped Memory operation** | Operation O is bound only to phase `review`. | O's declared trigger becomes eligible while another phase is active. | O does not gain participation outside `review` solely because its trigger condition is satisfied. Trigger eligibility and binding participation scope are distinct requirements. |
| **N — Incomplete composition context** | An Agent declares references to external Components. | The validator receives the Agent manifest but not the referenced Component definitions. | Standalone validation may succeed, while dependent resolved-composition checks remain unevaluated. Missing context alone must not be reported as confirmed nonconformance. |
| **O — Implicit Loop outcome** | Phase `research` omits explicit `outcomes`. A transition declares `from: "research"`, `on: "complete"`, and a valid destination. | The Loop otherwise satisfies its structural and semantic requirements. | The phase has the APDS-defined implicit outcome `complete`. The transition refers to that outcome validly. A Runner may choose how phase completion is determined but must preserve the declared outcome and transition meaning. |
| **P — Conformance versus runtime support** | An Agent Package has a valid Knowledge, Memory, or other Component dependency. | The selected Runner cannot provide a required runtime capability or backend. | Artifact conformance and Runner compatibility/readiness remain separate determinations. A missing runtime capability does not retroactively invalidate an otherwise conformant authored artifact. |

**Required fixture coverage and interpretation rules**

- Create actual schema-valid manifests for all positive cases, and deliberately invalid manifests or resolved graphs for negative cases.
- Exercise both global and phase-specific Skill-to-Tool inheritance, including cases where Loop Tool access permits and prohibits execution.
- Exercise direct Tool bindings and inherited Tool bindings independently and together, including repeated paths to the same resolved Tool.
- Exercise global and phase-specific Instruction Profile applicability without making prompt order or natural-language conflict resolution a normative requirement.
- Exercise both direct Memory space access and operation-mediated Memory access, including valid operation targets that are not directly bound.
- Exercise Memory operation binding participation separately from trigger eligibility and execution.
- Exercise version-aware dependency references using a resolved graph containing multiple versions, without requiring a particular AgentPM lockfile format.
- Exercise valid standalone manifests whose resolved-composition checks cannot yet be completed, distinguishing incomplete evaluation from confirmed semantic violations.
- Exercise the implicit Loop outcome `complete` and its relationship to authored transitions.
- Exercise the distinction between APDS artifact conformance and unsupported or unavailable Runner capabilities.
- Associate each fixture with the applicable normative rule IDs, required validation level, and expected outcome.
- Keep validation fixtures deterministic. Where a case requires runtime interpretation, specify the observable semantic expectation without prescribing the Runner's internal implementation.
- Do not turn AgentPM-specific CLI commands, prompt construction, storage strategies, scheduling mechanisms, or UI representations into requirements for independent APDS implementations.

These fixtures must preserve the Phase 7A composition decisions, particularly **additive binding scopes**, **Skill Tool inheritance**, and the distinction between **direct Memory space bindings and Blueprint operation bindings**. Formalizing APDS must not weaken those authored semantics or introduce stricter direct-binding requirements that were not part of the original contract.

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

**Stage 1 reader constraint:** M11B's v4 readers reject unknown structural fields,
including optional additions to locked package/root entries. Retaining v4 under
D06 does not by itself make these readers compatible with standard/origin fields.
Before the APDS freeze, M5A must settle safe extension preservation versus an
explicit minimum-CLI rollout, or justify a format bump under D06. See the
[Stage 1 lockfile compatibility note](../2026-10-06-immediate-hardening/spec.md#21-lockfile-evolution).
Do not remove strict field rejection without protecting read-modify-write and
resolve-plan regeneration from losing unknown state or ignoring required semantics.

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
  Its `README.md` and `semantics.md` also pass a dedicated human-readability review: a technically capable
  newcomer can quickly find what APDS defines, the rules applicable to their role, and the guarantees they
  can expect without needing AgentPM source code or an AI-generated explanation.
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
| **D01** | APDS rule organization | **One concise, human-readable `semantics.md`** organized by semantic domain with short normative rules, stable IDs and linked fixtures; no bespoke DSL. `README.md` is the approachable entry point. | Split if maintainability clearly benefits; document canonical normative index, non-duplication and fixture consistency. Preserve complete guarantees without bloated rule templates or duplicated schema prose; require independent editorial review. |
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

