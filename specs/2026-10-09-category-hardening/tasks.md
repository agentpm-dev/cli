# Tasks

**Stage 2: Category-Enabling Hardening — Make the Agent Package Real**  
**Implementer:** Codex. **Independent reviewer:** Claude Code.  
**Authoritative requirements:** [`spec.md`](spec.md). **Verification:** [`test-plan.md`](test-plan.md). **Review:** [`review-checklist.md`](review-checklist.md).

> This document follows the **Stage 1 Immediate Hardening** task conventions: numbered **Release Bands**, numbered **Milestones** with A/B/C subtasks when a domain needs smaller slices (plus `.1` decimal inserts where one landed between existing slices), explicit **Scope notes**, implementation notes, checkboxes, and a band integration gate. Each milestone tells an implementer what *changes*, what is *not included*, its dependencies, the tests/evidence, and the acceptance conditions.
>
> **Each milestone has exactly one task list.** Its **Implementation checklist** is the work. Its **Binding requirements** are the authoritative constraints from `spec.md` that the work must satisfy — reference material, deliberately not checkboxes, so nothing is tracked twice. The four evidence requirements that apply to every milestone are stated once under [Per-milestone evidence requirements](#per-milestone-evidence-requirements) rather than repeated 39 times; each milestone's own **Decisions/Acceptance** line carries the IDs it must map.

## How to use this implementation plan

- **Read `spec.md` first.** It is fully self-contained; do not assume access to its planning chat. Especially read the detailed APDS chapter blueprint and cases A–P before changing schema or validation. The blueprint guides the *authoring* of `semantics.md`; AgentPM implementation notes in that blueprint must not be copied into the normative standard.
- **APDS is human-first.** The versioned `README.md` must be a short, navigable introduction and `semantics.md` must be precise without becoming dense or repetitive. Short rules, examples when useful, links to schema/fixtures, and a dedicated newcomer editorial review are **required** alongside technical conformance. Do not sacrifice accuracy for brevity or reproduce the entire Stage 2 implementation spec in the published standard.
- `REQUIRED / AGREED` is not optional. `PREFERRED` is our strong suggested approach but Codex may improve it with evidence. `INVESTIGATE / EXPERIMENT` requires a documented outcome even when the feature itself is deferred. Claude reviews all normal D01–D21 decisions; escalate only product-semantic or high-impact compatibility conflicts.
- Each `Milestone X[A/B/C]` should normally fit a focused PR or review unit. Split again if necessary rather than merging unrelated backend/UI migrations into a giant change. A **Release Band** is a coherent integration/deployment boundary; parallel development within a band is fine with explicit dependencies.
- **Stage 1 owns** search correctness/facets/ranking/stars/trending, namespace pins/scoped search, shared detail shell, technical SEO, CLI scaffold/lint/output hardening, analytics, Python portability, lockfile v4, multi-artifact publishing, digest/signature/attestation and CI. Review actual landed/in-flight Stage 1 work before any overlapping change; do not implement a competing version.
- **Ship M1A.1 first, and early.** The old-client tolerance release has no dependencies and no gate: it is one permissive schema field so every already-released CLI can accept and round-trip `standard` before anything emits it. Without it, publishing a package with `standard` makes `agentpm lint` hard-fail (exit 1) on every CLI in the wild, and `agentpm new` fail during scaffolding. Ship it during Stage 1, in parallel.
- **APDS freeze gate:** M1B/M1C (and therefore M2A–M4B) cannot start until Stage 1 **M2** and **M11A** manifest-schema additions are merged. Those add fields to closed (`additionalProperties: false`) definitions in the very schema v1.0.0 freezes, so freezing first would make legitimate Stage 1 manifests nonconformant. M1A and all Band 4/5 work are unaffected. Details in `spec.md` §1.
- **APDS rollout safety:** new-release Registry enforcement and strict CLI validation must ship in a coordinated order. Historical released bytes remain immutable; cutover handling must be decided and verified. A fixture corpus or standard with unresolved semantic contradictions is not ready to freeze.
- **Documentation hard gate:** comprehensive docs and AgentPM-owned README cleanup begins **only in M17A**, after all Stage 1 milestones—including Stage 1 documentation/migration—are done. Normative APDS specification files and small feature-local help may be authored earlier.
- **Visual references:** `assets/landing-structure.png`, `detail-structure.png`, `landing-layered.png`, `detail-layered.png`, `explore-layered.png`. The three `-layered` images are **one site-wide redesign sharing a global shell**, and they are a **guide weighted toward layout** — strongest on structure, hierarchy and component inventory, with some styling worth taking selectively. Nothing is adopted 1:1 by default; **D21** decides adopt/adapt/reject per element with Zack's sign-off before the shell is built. The **existing website** is the default for colors, typography, design tokens, kind icons and the layered/floating aesthetic; the mockups contain fictional data, illustrative copy and invalid illustrative JSON. Because several elements are global, **M8B.1 builds the shell once and every page milestone adopts it** — see `spec.md` §9.1.
- **Milestone evidence:** Record exact changed repo paths/DTOs, tests actually run, CLI screenshots/terminal output, browser screenshots, backward-compatibility/migration notes, AC references, open D choices with option/rationale, Claude findings, and the merge/deployment order. Never check off tests not performed.
- **Milestone ID convention:** Stage 1 and Stage 2 share an `M<n><A/B/C>` namespace — `M11A`, `M13A`, `M14A/B`, `M15A/B`, `M16A/B` and `M17` all exist in **both** stages. In this document and its companions, a bare `M…` reference **always means this stage (Stage 2)**, and any Stage 1 milestone is **always** written with an explicit `Stage 1` prefix. Never read a bare ID as Stage 1, and never write a Stage 1 reference without the prefix. When citing a Stage 1 milestone that has been split, name the exact sub-milestone (`Stage 1 M10A`, not `Stage 1 M10`) — the sub-milestones own materially different work.
- **Naming reminder:** Agent Package means `kind: agent` complete authored system (valid even empty/no Loop). Six reusable Components are Tool, Skill, Knowledge, Memory Blueprint, Instruction Profile and Loop. Template is separate. AgentPM CLI = Agent Package Manager. Harness = built-in Agent Package Runner, not the definition standard.

## Per-milestone evidence requirements

These apply to **every** milestone and are not repeated per milestone. A milestone is not complete until all
four hold. The AC and decision IDs each milestone must map are on its own `**Decisions:** / **Acceptance:**`
line.

- [ ] Record automated tests and the exact commands actually run, with exit status; attach targeted negative/edge cases, not just a happy-path screenshot.
- [ ] Record migration, legacy/private behavior and any shared Stage 1 contract touched (or state explicitly that none was).
- [ ] Update the evidence mapping for the milestone's listed `S2-AC-xx` IDs and the decision record for its listed `Dxx` IDs.
- [ ] Have Claude Code review requirements, tests, regressions and implementation choices; address blockers before considering the milestone complete.

Each milestone then closes with its own **Exit gate**, which is the review boundary for that slice.

## Milestone and release map

| Release band | What users/implementers get | Milestones |
|---|---|---|
| **1 — APDS definition and enforcement** | Old-client tolerance release; versioned contract; Rust/Python conformance; Agent-first authoring; Registry new-release gate | M1A–M4B |
| **2 — Reproducible install and Runner support** | APDS-aware lock/install/frozen behavior; honest Harness + SDK preflight | M5A–M6B |
| **3 — Inspectable packages and objective Health** | Health evidence; immutable spec links; **global layout/shell system**; visual composition/Loop pages; specialized detail pages | M7A–M10B |
| **4 — Product/category website and discovery** | Data-driven featured content/cards; homepage; Explore; namespace; category/pricing/SEO copy | M11A–M14B |
| **5 — CLI/first-use adoption** | Category-first help; republished example/Template versions; beginner example; try/build/template journeys; future Developer handoff | M15A–M16B |
| **6 — Final docs and overall review** | Full docs/READMEs, command verification and independent approval **after Stage 1** | M17A–M17C |

---

## Milestone 1A: Inventory the Existing AgentPM Manifest and Composition Contracts
> **Scope note:** Create a traceable inventory of what AgentPM already declares and interprets across the eight kinds before changing the standard. Outside this milestone: normative schema edits, new validation engine, Runner redesign.
> **Implementation notes:**
> - A normatively frozen version can only represent behavior we have inspected. Include the manifest source, Rust CLI, Python Registry, SDKs, published examples, and Harness interpretations. Treat implementation discrepancies as review findings, not automatic APDS requirements.
> - **Dependencies:** None; consult latest Stage 1 branch and Phase 6/7 source.
> - **Decisions:** D01, D02, D19. **Acceptance:** S2-AC-01, S2-AC-03.

**Implementation checklist**

- [x] Inventory top-level schema: `$schema`, `$id`, `kind`, local `name`, version, description, and conditional kind requirements.
- [x] Record the eight kinds and separate Agent Package (`agent`) from six Components and `template`.
- [x] Map each kind to its actual manifest metadata fields and related CLI commands.
- [x] Record how dependency declarations are represented (strings versus object selectors) and how Registry `@namespace/name` differs from local manifest `name`.
- [x] Trace global and phase Agent bindings through actual phase lookup and effective capabilities.
- [x] Trace Skill Tool inheritance through declared dependencies, bindings and Runner exposure.
- [x] Trace Memory spaces and operation triggers including `capacity`, `record_count`, `interval`, and `external`.
- [x] Trace Loop terminal outcomes, default omitted outcomes, and phase restrictions.
- [x] Classify observed behavior as structural contract, candidate portable semantics, or AgentPM-specific implementation policy; record the evidence and unresolved interpretation instead of treating current code as automatically normative.
- [x] List published legacy manifests and known common authored examples that will be used for regression.
- [x] Make an explicit cross-repo contract table and attach file paths/line references to the PR.
- [x] Have Claude independently review whether the inventory omits a meaningful authoring or execution behavior. User confirmed Claude review passed on **2026-11-13**, after the environment-precedence and Tool-invocation additions.

**Builder evidence — 2026-11-13:** [Contract inventory](m1a-contract-inventory.md) includes the cross-repo table/source line references for the review handoff, baseline revisions, regression catalog, S/P/I classification, reconciliation register and D01/D02/D19 decisions. The [schema index](m1a-schema-index.md) covers all 78 manifest and 36 Harness configuration definitions. [Verification](m1a-verification.md) records 421 unique CLI tests (including the review additions for Tool environment precedence and process invocation), 213 Registry tests plus 91 subtests, 137 SDK tests passed with two optional SDK tests skipped, and 38 focused schema/CLI/source-manifest probes. No product/schema/migration/shared Stage 1 contract changed. S2-AC-01/S2-AC-03 contributions and later unmet portions are explicit; Claude review passed, as confirmed by the user on **2026-11-13**, with no remaining findings. **M1A is complete.** No other milestone's checklist is completed by this audit.

**Binding requirements** — authoritative constraints from `spec.md` for this milestone. These are requirements to satisfy and verify, not a second task list; the work itself is in the implementation checklist above.

- Inventory current artifacts and cross-repo contracts before editing: Rust CLI schema/lint/init/install/harness paths, Registry schema/publish/finalize/resolve, Python/Node SDK loads and Runner machine initialization, current `agentpm.manifest.schema.json` and Harness configuration schema. Capture exact schema `$id`, `$defs`, eight kinds, `oneOf` requirements, dependency forms, Loop, bindings, Memory operations, Template execution surfaces. Note fields on which current clients rely.

**Verification:** the standard four evidence requirements apply — see [Per-milestone evidence requirements](#per-milestone-evidence-requirements). Map the AC and decision IDs listed in this milestone's header.

**Exit gate:** The scoped deliverable works against real source contracts; the binding requirements are satisfied, the standard evidence requirements are met, and Claude review has passed.

## Milestone 1A.1: Ship the Old-Client Tolerance Release
> **Scope note:** Make every already-released CLI able to accept and round-trip a `standard` field **before** anything emits it. One narrow schema change plus round-trip verification. Outside this milestone: defining APDS, emitting `standard`, validating it, enforcing it, or any other schema loosening.
> **Implementation notes:**
> - **Why this must come first.** The root schema is `additionalProperties: false`, so pre-tolerance clients hard-reject a manifest carrying `standard`. Verified on 0.1.33: `agentpm lint` returns `[ERROR] Additional properties are not allowed ('standard' was unexpected)` and exits 1. Affected validation/generation surfaces include `lint`, `publish` preflight, `new`, `export`, `knowledge build`, `memory build`. Export reads the Tool and generates a fresh Skill rather than independently linting the source. `install`, `harness`, `run` and `serve` do no general manifest-schema validation, so accepting the extra root field does not require a new validator on those paths.
> - `agentpm new` is the sharpest edge because it validates generated manifests through a blocking check (`commands/new.rs::validate_generated_manifests_blocking`): an old CLI scaffolding a Template whose files carry `standard` fails **during `new`**, not at a later lint.
> - This is `spec.md` §1's add/read → validate → enforce sequencing applied in the client direction, which the rest of the plan applies only to the server.
> - **Relax the root by exactly one known field.** Do not move to `additionalProperties: true`: an open root permanently stops catching misspelled top-level keys in the version about to be frozen immutable. Later extensions use the §3 ch.2 extension policy.
> - **This milestone does not depend on APDS being designed and should ship during Stage 1**, in parallel, well before the M1B freeze gate lifts. It is the one piece of Band 1 with no gate on it.
> - **Dependencies:** None. Deliberately ahead of M1B's hard gate.
> - **Decisions:** D02, D04, D19. **Acceptance:** S2-AC-02.

**Implementation checklist**

- [x] Add `standard` to the existing schema's root `properties` with a permissive shape so it is accepted and ignored. Do not add validation, requirements, or emission. Already shipped by Stage 1 M8 (`e0143af`); verified without changing the schema in this milestone.
- [x] Confirm nothing else loosens: root stays `additionalProperties: false` for all other keys, and no `$defs` object is opened. The original tolerance commit changes only that one property; new all-kind negative cases retain required fields and typo rejection.
- [x] Verify each manifest-rewriting path preserves the field. `agentpm install <pkg>` writes back the loaded `manifest_value` (**preserves**); `knowledge build` writes by default, mutating only the `knowledge` object (**preserves**); `memory build` writes `memory/contracts/` and `memory/build.json` (**never rewrites `agent.json`**); `agentpm export` builds a **fresh** Skill scaffold (no declaration copied or emitted); typed `workspace.rs` writers operate on workspace/template metadata, not manifests.
- [x] Add a regression test: a manifest containing `standard` plus one additional unknown top-level key survives `agentpm install <pkg>` write-back and `knowledge build` with no field lost. Both repeated operations are byte-stable; the Knowledge extra-key probe uses a local schema override while the bundled root continues to reject that key.
- [x] Confirm `serde_json` stays pinned **without** `preserve_order` (`crates/agentpm-cli/Cargo.toml` notes publish author signatures depend on it). Rewrites alphabetize keys while losing nothing; Cargo's actual feature graph also confirms no `preserve_order` activation.
- [x] Verify the tolerance implementation on every named validation/generation surface: `lint`, `publish` preflight (dry run), `new`, `export`, `knowledge build`, `memory build` all accept a manifest carrying `standard`. Current build verification retains the already-shipped tolerance behavior; it is not a claim that a historical 0.1.36 binary was re-executed.
- [x] Verify `agentpm new` from a Template whose scaffolded files carry `standard` completes, including `validate_generated_manifests_blocking`. The negative counterpart rejects `standrad` and cleans the failed output.
- [x] Confirm publication of **CLI 0.1.36** and record it as the minimum CLI version named by M4B's outdated-client error and by the detail page's install guidance. Operator confirmed publication/installation on **2026-10-22**, and a live Production first-use event reports **0.1.36**. The schema change landed in Stage 1 M8; the Band 5 handoff / Band 7 publication record supersedes the original 0.1.35 target. This publication record satisfies M3A's publication prerequisite; its other dependencies and acceptance requirements remain unchanged.
- [x] Do **not** emit `standard` from any command in this milestone; emission is M3A. Init and fresh export/generated-root manifests remain without the field; copied authored Template Agents retain it.

**Builder evidence — 2026-11-13:** [M1A.1 verification](m1a1-verification.md) maps T-APDS-20, D02/D04/D19 and the tolerance portion of S2-AC-02. Changes are regression tests and internal evidence only; no new schema, runtime behavior, migration, version bump or release is needed. The 0.1.36 publication prerequisite remains satisfied by the existing operator record. The user confirmed independent Claude review passed with **no findings on 2026-11-13**. **M1A.1 is complete.**

**Binding requirements** — authoritative constraints from `spec.md` for this milestone. These are requirements to satisfy and verify, not a second task list; the work itself is in the implementation checklist above.

- A manifest with `standard` must lint clean on the tolerance build for **all eight kinds**, and must still fail for genuinely invalid content — the relaxation must not mask unrelated defects.
- A manifest with a **misspelled** top-level key (for example `standrad` or `tool`) must still be rejected, proving the root was not opened.
- Round-trip a manifest through `install` twice and confirm idempotence: no field loss, no repeated reordering churn beyond the first alphabetization.
- Confirm an old pre-tolerance CLI's failure mode is understood and documented, since those installs persist in the wild: it is a hard lint error, not a warning, and M4B's upgrade guidance must account for it.

**Verification:** the standard four evidence requirements apply — see [Per-milestone evidence requirements](#per-milestone-evidence-requirements). Map the AC and decision IDs listed in this milestone's header.

**Exit gate:** The tolerance release is published, its version is recorded as the minimum floor, and round-trip preservation is proven by test — not inspection.

## Milestone 1B: Create the Versioned APDS Schema and Standard README
> **Scope note:** Turn the existing `agentpm.manifest.schema.json` into the canonical immutable v1.0.0 structural contract, with a clear definition README and backward-compatible legacy schema path. Outside this milestone: new alternate manifest format, federation or moving `main` URI authority.
> **Implementation notes:**
> - The existing schema is authoritative starting material. The evolved versioned schema becomes normative; the old path is a derived/aliased compatibility entrypoint. `standard` is a structured object, not a single `apds` string.
> - **🚦 HARD GATE — do not start this milestone until Stage 1's manifest-schema additions have landed.** Stage 1 adds fields to the same `schemas/agentpm.manifest.schema.json` this milestone freezes, and the target definitions are closed (`additionalProperties: false`), so each addition changes what is conformant. Required to be merged first: **Stage 1 M2** (`agentpm-harness` added to the closed `$defs/templateMetadata.execution_surfaces` enum; Stage 1 Release Band 2) and **Stage 1 M11A** (optional Python `dependencies` on `$defs/runtime`, which is `additionalProperties: false, required: [type, version]`; Stage 1 Release Band 8). Stage 1 M12's payload classification metadata lives in the build descriptor/release manifest rather than `agent.json`, so it does not gate the freeze — confirm that when M12 lands. These fields are part of v1.0.0's structural contract, not a later standard version. Verify against merged Stage 1 code, not the Stage 1 plan. See `spec.md` §1 "APDS freeze gate" for the collision table.
> - **Dependencies:** M1A; **Stage 1 M2 and M11A merged** (see hard gate above).
> - **Decisions:** D02, D19. **Acceptance:** S2-AC-01, S2-AC-03.

**Implementation checklist**

- [x] **Gate check before any other task here:** confirm Stage 1 M2 and M11A manifest-schema additions are merged, and record the exact commits/schema diff inspected. Also confirm Stage 1 M12 kept payload classification out of `agent.json`; if it did not, this gate extends to M12. If any is outstanding, stop — this milestone is blocked, not startable with a planned-field placeholder.

Historical Stage 1 M11A handoff (2026-10-22, working-tree implementation, not merge evidence; superseded by the 2026-11-14 merged-code verification in [M1B evidence](m1b-verification.md)):
`$defs/runtime` now accepts optional Python `dependencies` and rejects the field
for Node; requirement semantics are validated by the CLI. The portable
`agentpm.python-resolution.v1` type includes non-empty SHA-256 distribution
digest sets and conditional markers. Inspect the merged schema/type/tests before
lifting this gate; no APDS freeze or Stage 2 implementation is claimed here.

- [x] Include Stage 1's landed schema additions in the frozen v1.0.0 structural contract: `agentpm-harness` in `$defs/templateMetadata.execution_surfaces` and optional Python `dependencies` on `$defs/runtime`. Add positive fixtures for each in M2A.
- [x] Create `standards/agentpm/1.0.0/` (or justified equivalent) and make the intended files independently readable.
- [x] Add `standard.id` and `standard.version` to the existing common schema with a fixed supported-v1 expectation.
- [x] Make nonblank `description` a **hard conformance error**, owned by the schema. Today the schema types it as bare `{"type": "string"}` and the Rust CLI emits a separate semantic `[WARN] description should not be empty` (verified on 0.1.33: whitespace-only lints with a warning, not an error). Express it structurally in the pinned schema (`minLength` + non-whitespace `pattern` or equivalent) so Rust and Python both enforce it from the contract.
- [x] Remove or replace the existing CLI-side description warning so the same defect is not reported twice at two severities; confirm trimming semantics are identical in Rust and Python and covered by a shared fixture.
- [x] Remove the incorrect unconditional Agent `tools` requirement while preserving kind-specific Tool requirements.
- [x] Verify optional `loop`, `profile`/`profiles`, bindings, empty deps and omitted deps on minimal Agent manifests.
- [x] Preserve the local `name` shape; do not rewrite it as `@namespace/name`.
- [x] Preserve exact existing eight-kind vocabulary and avoid adding `AgentPackage` as a new machine kind.
- [x] Give normative schema a version-stable `$id`, distinct from optional editor `$schema`.
- [x] Decide and record the pinned `$schema` URL that scaffolds should emit, so new manifests are edited against the same frozen contract they are validated against.
- [x] Select a reproducible build/vendoring mechanism so no two schemas are independently edited.
- [x] Add a CI or build assertion detecting old-path versus canonical-schema drift.
- [x] **Close the runtime schema-source override.** `manifest.rs::resolve_schema_source()` currently prefers a working-directory-relative `schemas/agentpm.manifest.schema.json` over the compiled-in schema, and `--schema` is honored by both `lint` and `publish` (which can point at an `http(s)` URL that `load_schema_value()` fetches). Verified on CLI 0.1.33: a permissive local copy makes lint pass manifests the bundled contract rejects. Make the pinned bundled contract authoritative for APDS validation; an overridden schema must never yield a `conformant` APDS result, must be labeled non-authoritative in output, and must not reach `publish` preflight as if it proved conformance.
- [x] Write the normative `README.md` covering every element the binding requirement below enumerates, including the version/immutability/extension policy and what changes force a new standard version.
- [x] Give the README a human-first editorial pass: plain-language definition, scope, conformance roles, key guarantees and a quick map to schema/semantics/fixtures; remove duplicated field-by-field schema explanations and unnecessary formality.
- [x] Ask Claude to review the README as a technically capable newcomer without access to AgentPM's code or this planning history; address unclear terms and navigation before the standard is frozen. User confirmed full review complete with no remaining findings on **2026-11-14**.
- [x] Demonstrate that offline validation doesn't fetch a mutable repository URL.
- [x] Record selected path/build relationship and a structural before/after diff.

**Completion evidence — 2026-11-14:** [M1B verification](m1b-verification.md) records exact prerequisite commits, D02/D19, test commands, compatibility decisions and S2-AC-01/03 contributions. [Structural before/after diff](m1b-structural-diff.json) limits changes to five paths and confirms all 78 definitions are unchanged. User confirmed Claude's full review, including the [canonical README](../../standards/agentpm/1.0.0/README.md), passed with no remaining findings. **M1B is complete.** The bundle remains unpublished; M1C/M2A own completion of semantics/corpus and the release gate still requires publishing/verifying the immutable tag. No conformance DTO, declaration emission or Registry enforcement is claimed here.

**Binding requirements** — authoritative constraints from `spec.md` for this milestone. These are requirements to satisfy and verify, not a second task list; the work itself is in the implementation checklist above.

- Produce an explicit before/after structural diff of the **existing** `schemas/agentpm.manifest.schema.json`: add `standard:{id,version}`, enforce nonempty trimmed description, remove `tools` as unconditional Agent requirement, retain actual name vs `@namespace/name` reference semantics, keep Loop/Profile/Tools optional for Agent; preserve other kind-specific constraints unless required to correct a conformance defect. **Do not create a greenfield schema.**
- Establish immutable versioned contract location, **prefer** `cli/standards/agentpm/1.0.0/`, with normative `README.md`, versioned `manifest.schema.json`, `semantics.md`, conformance README/fixtures and a stable semantic contract identifier. Document where the canonical source lives and how the existing schema path remains compatible without independently drifting.
- Fix normative JSON Schema `$id` to a version-stable URI and document `$schema` editor affordance separate from manifest `standard` identity. Ensure bundled offline validation never requires fetching GitHub's mutable `main` URL.
- Write `README.md`: what APDS is, scope/roles/portability, exact version/immutability/extension policy, authored package versus runtime, registry independence, eight kinds, terminology, links to schema/semantics/fixtures, how implementers declare support. It must be a concise, developer-friendly **entry point**, not a restatement of the schema or normative semantics.
- Verify minimal `kind:agent` no-Tools/no-Loop/no-Profile common-fields manifest is structurally valid; **not** falsely Harness-runnable. Preserve all other kinds' valid field contracts. Decide if unknown properties are rejected according to existing schema, and write explicit extension policy.

**Verification:** the standard four evidence requirements apply — see [Per-milestone evidence requirements](#per-milestone-evidence-requirements). Map the AC and decision IDs listed in this milestone's header.

**Exit gate:** The scoped deliverable works against real source contracts; the binding requirements are satisfied, the standard evidence requirements are met, and Claude review has passed.

## Milestone 1C: Write and Review Normative APDS Semantics
> **Scope note:** Specify the meaning of the already implemented artifact/composition system in `semantics.md`, with stable normative rules and examples. Outside this milestone: Do not create an executable semantics DSL or silently standardize a runtime defect.
> **Implementation notes:**
> - This is the most important APDS editorial milestone. Follow the detailed chapter blueprint and behavioral cases in `spec.md`. Every normative assertion must have a portable justification and traceable evidence; map testable claims to fixtures without manufacturing invalid cases for purely explanatory guidance. **Write for human readers first**: the chapter blueprint lists guarantees to cover, not a paragraph quota or mandatory verbose rule template.
> - **Normative boundary:** APDS defines artifact validity and authored portable meaning, not incidental AgentPM CLI/Registry/SDK/Harness implementation choices. Apply the three-part inclusion test (validity, meaning, intended interoperability) to each proposed rule. Put AgentPM-specific instructions in the Stage 2 spec or implementation documentation, not the published `semantics.md`.
> - **Dependencies:** M1A/M1B; freeze with initial conformance fixture review from M2A. **Inherits M1B's hard gate:** normative semantics cannot be frozen before Stage 1 M2/M11A schema additions are merged, because their fields carry semantics this document must state.
> - **Decisions:** D01, D19. **Acceptance:** S2-AC-01, S2-AC-03.

**Implementation checklist**

- [x] Define normative terminology and role-specific conformance subjects (author, validator, Registry, Manager, Runner); a role is not obliged to implement unrelated roles.
- [x] Distinguish standalone, resolved-composition and packaged-content validation, unevaluated requirements and unsupported standards; distinguish artifact conformance from the extent of verification.
- [x] Apply the normative inclusion test to each candidate rule: necessary for artifact validity, authored meaning or an intended interoperability guarantee. Label implementation-specific explanations as non-normative; do not codify current Harness behavior by default.
- [x] Write artifact identity, standard and artifact versions, namespaces, reference kinds, direct/transitive composition and resolved-version meaning without specifying a particular Registry, resolver or lockfile format.
- [x] Distinguish dependency declarations from Agent bindings; write global/phase additive binding semantics, Loop restrictions and capability eligibility separately from actual runtime availability/invocation.
- [x] Write Skill Tool inheritance for global and phase bindings, logical identity across multiple direct/inherited paths and the absence of automatic Skill-script execution; do not prescribe de-duplication algorithms or a shell-executor Tool.
- [x] Write Loop entry/phase/outcome/transition/terminal/limit/access/checkpoint/error-policy semantics; make omitted-outcome `complete` a portable identifier, not a prompt-generation mechanism.
- [x] Reconcile transition determinism/completeness, step counting, explicit approval/checkpoint behavior and explicit failure policy without elevating omitted-policy Harness defaults into normative rules.
- [x] Write additive global/phase Instruction Profile applicability, authored constraints and advisory compatibility (`compatibility.requires`/`recommends`); do not impose Profile prompt order, de-duplication or universal natural-language conflict precedence.
- [x] Write Knowledge `context`/`vector`, paths, corpus/embedding/index identities, retrieval defaults, provenance and compatibility; distinguish manifest and packaged-content checks from Runtime retrieval implementation.
- [x] Write Memory scopes, record type schemas/versions, the three space models, retrieval modes, capacity/retention/constraints and governance annotations without specifying a storage backend.
- [x] Write Memory direct-space versus lifecycle-operation access, global/phase participation, triggers (`external`, `interval`, `record_count`, `capacity`), trigger eligibility versus invocation, and operation inputs/targets/source-handling/provenance/cascade behavior.
- [x] Preserve operation access to valid declared input/output/target spaces not directly bound in the current phase; reconcile applicable Loop policy without granting operations a blanket access bypass.
- [x] Write Template files, variable interpolation, dependency and entrypoint declarations, and supported execution-surface meaning; do not make `agentpm new`, Harness installation or automatic command execution normative.
- [x] Define accurate conformance claims and portable logical outcomes without requiring every independent implementation to use AgentPM diagnostic formatting, API enums or user interfaces.
- [x] Keep AgentPM legacy-inferred handling outside normative APDS; the standard requires explicit identification for v1.0.0 authoring but does not mandate retroactive migration of historical releases.
- [x] Explicitly exclude Runner model selection, prompt ordering, approvals UI, hook architecture, storage/retrieval algorithms, scheduling, retry engines and traces unless a specific authored behavior requires an observable guarantee.
- [x] Give each normative rule a stable ID, clear requirement, conformance subject and applicability; keep validation levels, diagnostic expectations, rationale when needed and fixture links traceable without forcing a long repeated prose template. Keep AgentPM diagnostic presentation separate from APDS semantics.
- [x] Create a concise rule index so future validation errors and Registry reference pages can deep link correctly.
- [x] Perform a separate human-readability edit of all eleven chapters: use the reviewed APDS README as the writing benchmark; introduce concepts in plain language, shorten abstract sentences, remove redundant rules and implementation-boundary disclaimers, avoid repeating schema definitions, and use compact examples or tables where they explain difficult distinctions. Preserve every actual semantic guarantee.
- [x] Have Claude read `semantics.md` as a technically capable developer new to APDS: can they find the rules relevant to their role, identify guarantees and implementation freedoms, and understand a representative fixture without the source code or an AI explanation? Resolve unclear or needlessly dense passages.
- [x] Verify every semantic example uses schema-accurate fields; do not copy mockup-generated `type` or `apds` fields or describe internal product behavior as universal.
- [x] Produce a semantic reconciliation log identifying ambiguous intended behavior, implementation-only defaults and any schema/runtime contradictions; escalate actual product-semantic decisions before freezing.
- [x] Require focused Claude rule-by-rule signoff for all eleven chapters, including an explicit audit for AgentPM implementation details mistakenly made normative.

**Completion evidence — 2026-11-14:** [M1C verification](m1c-verification.md) maps all eleven chapters, tests and acceptance boundaries; [inclusion/reconciliation log](m1c-reconciliation.md) records portable versus implementation/compatibility decisions. The [semantic rules](../../standards/agentpm/1.0.0/semantics.md), [rule index](../../standards/agentpm/1.0.0/conformance/rule-index.md) and [A–T case map](../../standards/agentpm/1.0.0/conformance/semantic-cases.md) passed independent review; user confirmed no remaining findings on **2026-11-14**. **M1C is complete**. Review corrected the Memory authority boundary to preserve Phase 7B: Loop Memory flags constrain direct Agent access, while separately bound operations retain declared lifecycle authority. User explicitly reversed the earlier proposed tightening; M6B retains regression verification, not a new prohibition-enforcement fix. A fixture-only ignore exception plus a Git-visible-bundle regression prevents the Agent fixture being omitted. Review includes the revised Memory authority boundary, trigger cases Q–T and their regression evidence. No schema/runtime behavior changes, full semantic engine, complete executable corpus or publication is claimed; initial M2A fixture review and the release publication gate remain.

**Binding requirements** — authoritative constraints from `spec.md` for this milestone. These are requirements to satisfy and verify, not a second task list; the work itself is in the implementation checklist above.

- Write *detailed normative* `semantics.md` aligned to the revised eleven-chapter blueprint and cases A–P in `spec.md`. At minimum cover:
  - role-specific conformance subjects; standalone, resolved-composition and packaged-content evaluation; partial/unsupported verification states and the distinction between conformance and verification;
  - portable artifact identity, declared standard, namespaced references, artifact versions, kind constraints, direct/transitive graph composition and exact selected-version meaning **without requiring `agent.lock`**;
  - **global + phase bindings additive**, scope validity, authored eligibility versus Runner support and Loop access restrictions; MCP bindings and consumer context without prescribing protocol internals or prompt injection;
  - Skill-inherited Tool capability scope and identity for **global and phase** bindings, duplicate paths without extra permissions, no required internal de-duplication algorithm, and no Skill script auto-execution;
  - Loop graph/phase/entry/outcome/transition/terminal/limit/access/checkpoint and explicit error-policy semantics, including the fixed omitted-outcome identifier `complete`; keep omitted-policy recovery/prompt strategies implementation-defined;
  - Profiles' additive global/phase applicability, behavioral `required` versus `preferred` constraints and actual `compatibility.requires`/`recommends` hints; **no normative prompt concatenation order, guidance de-duplication algorithm or automatic conflict precedence**;
  - Knowledge `context`/`vector` structure, packaged content paths, embedding/index identity and compatibility, retrieval defaults and provenance versus verified trust, without mandating a provider or retrieval engine;
  - Memory scopes, record contracts and schema versions, `document`/`collection`/`sequence` models, retrieval modes, capacity/retention/append-only, operation kinds and source-handling/provenance/cascade/governance declarations;
  - Memory direct-space binding versus separately bound operation participation; global/phase scopes, eligibility for `external`/`interval`/`record_count`/`capacity`, and valid operation targets **without direct space bindings**, subject to applicable policy, with no prescribed scheduler/backend;
  - Template file/variable substitution/dependency/entrypoint/declared execution surface semantics, including the `agentpm-harness` identifier, **without requiring `agentpm new`, automatic entrypoint execution or any particular Runner**;
  - conformance distinctions from trust/scanning, Runner compatibility and local readiness; legacy missing-standard inference is **AgentPM compatibility policy, not a normative alternate APDS authoring format**;
  - implementation boundaries: provider, prompts, Profile ordering/de-duplication, approvals UI, hooks, runtime scheduling, storage, retry, I/O and tracing mechanisms must not become normative absent a deliberate observable contract.
- For every candidate rule, first apply the **normative inclusion test**: is it necessary for artifact validity, portable authored meaning, or intended interoperability? Distinguish normatively required behavior from explanatory examples and AgentPM-specific implementation guidance; put the latter **outside** the published normative `semantics.md`.
- Give each **normative** rule a stable ID, concise MUST/SHOULD/MAY statement, conformance subject and applicability. Keep validation level, rationale where useful, expected findings and fixture mapping accessible via short annotations or a rule/fixture index instead of repeating long templates. AgentPM validators should attach useful rule IDs/paths/levels, but **independent APDS implementations need not emit AgentPM's exact diagnostic schema, enum or UI**. The publication gate includes editorial clarity as well as technical completeness. Treat sample IDs `APDS-ID-001`, `APDS-AGENT-001`, `APDS-BIND-001/002`, `APDS-SKILL-001`, `APDS-LOOP-001/002`, `APDS-PROFILE-001`, `APDS-MEM-001/002`, `APDS-TEMPLATE-001` in spec as suggested, not finished rules.
- Audit Phase 6/7 authored semantics against current manifests and Harness as **evidence**, not automatic authority. Record contradictions, undocumented defaults and ambiguous policy interactions; request product decisions before publishing normative v1.0.0, rather than standardizing accidental implementation behavior.
- Record **D01**, **D02**, **D19** decisions and evidence; include canonical schema synchronization check strategy and a candidate-rule classification table (normative / non-normative implementation guidance / AgentPM compatibility). Require focused Claude APDS contract review before freezing the version.

**Verification:** the standard four evidence requirements apply — see [Per-milestone evidence requirements](#per-milestone-evidence-requirements). Map the AC and decision IDs listed in this milestone's header.

**Exit gate:** The scoped deliverable works against real source contracts; the binding requirements are satisfied, the standard evidence requirements are met, and Claude review has passed.

## Milestone 2A: Create a Shared, Executable APDS Conformance Fixture Corpus
> **Scope note:** Convert APDS structure and semantics into deterministic positive, negative, and unresolved-graph test cases. Outside this milestone: assumption that a standalone manifest can validate its entire graph; no fake retrospective verification.
> **Implementation notes:**
> - Both language implementations will read the same normative validation fixtures and expected outcomes/rule IDs. Supply positive/negative counterparts where a rule is validator-testable; **do not** invent an invalid manifest solely to test advisory guidance or require a validator to execute a Runner-only behavior.
> - Cases A–P in `spec.md` cover both authored conformance and Runner interpretation. Tag each case by evaluation role/level; the Rust/Python parity suite covers normative validation results, while execution/capability behavior tests belong to M6A/M6B.
> - **Dependencies:** M1B/M1C normative draft.
> - **Decisions:** D03, D19. **Acceptance:** S2-AC-01, S2-AC-03, S2-AC-04.

**Implementation checklist**

- [x] Define fixture index format with case ID, root manifest, optional resolved graph/packaged files, conformance subject, validation level and expected results; distinguish validator outcomes from Runner-observation expectations.
- [x] Add a short implementer quickstart to `conformance/README.md`: prerequisites, working directory, copyable commands for the available corpus checks, expected output/exit status, and how to run one case or the full available suite. Explain that an intentionally invalid fixture passes its test when the expected violation is detected, and show how another implementation can consume the index. Require no hosted Registry or credentials. State each command's coverage accurately; M2B/M2C must extend the instructions with their validator-suite commands when implemented, keeping Runner observations separate.
- [x] Add one valid authored manifest for each of the eight kinds.
- [x] Add the dependency-free valid Agent case with no `tools`, `loop` or Profiles.
- [x] Add a multi-component valid Agent case with real schema-shaped references.
- [x] Add paired global Skill inherited Tool and phase Skill inherited Tool cases, plus repeated direct/inherited paths to the same resolved Tool without additional permissions (cases A–C and J).
- [x] Add additive global+phase bindings with Loop allowing and denying tools in different phases.
- [x] Add wrong phase and wrong-kind reference failures with stable rule IDs.
- [x] Add Loop missing entry, unknown transition/outcome, invalid target and omitted-outcome `complete` valid cases; include defined checkpoint/limit/error-policy semantics without assuming Harness-only prompt or recovery defaults (case O).
- [x] Add Profile `compatibility.requires.tool_use` versus Loop prohibition plus additive global/phase Profile applicability; do not assert prompt ordering or text de-duplication (cases E and K).
- [x] Add Memory direct space versus operation-mediated access, phase-scoped operation participation versus trigger eligibility, unbound-but-declared operation targets, external trigger without invocation, and valid/invalid operation references (cases F, G, L and M).
- [x] Cover all three Memory space models; record-type/schema references; retrieval modes; `external`, `interval`, `record_count`, `capacity` triggers; retention/source-handling/output/provenance/cascade and supported governance annotation shapes where applicable.
- [x] Add Knowledge `context`/`vector` and Template scaffold positives/negatives, including packaged-file/embedding/index relationships, non-secret variables, declared execution surfaces and entrypoints that are not executed by declaration.
- [x] Add selector missing, unsupported, malformed and whitespace description failures. The whitespace-description case must assert **error** severity, not a warning, and must come from the pinned schema rather than CLI-only logic.
- [x] Add incomplete resolved-graph and unavailable packaged-content cases; distinguish standalone conformance from unverified graph/content and from unsupported Runtime capabilities (cases H, N and P).
- [x] Add separate AgentPM legacy-inference compatibility fixtures without claiming historical undeclared artifacts are explicitly APDS-conformant; keep these outside the normative APDS authoring-validity corpus.
- [x] Map each of the sixteen semantic cases A–P to standalone/resolved/packaged-content validator fixtures, Runner interpretation tests, or both; include supplemental Q–T stateful trigger observations (initial armed count, no refire, rearm, interval baseline/reset) on the Runner side. Cover multi-version graph selection and exact capability identity (case I) without mandating an AgentPM lockfile.
- [x] Pin normative fixture bytes/version for v1.0.0 and document controlled fixture updates.
- [x] Keep detailed fixture expectations, paths and result metadata in the fixture index rather than duplicating them as verbose text in `semantics.md`; link from concise normative rules.
- [x] Review examples against actual parser before declaring case validity.

**Completion evidence — 2026-11-14:** [Verification and decision record](m2a-verification.md) maps 129 shared validation cases, 28 separate Runner observations and all 56 semantic rules to the required coverage. The portable quickstart/comparator, 19 Python checks, five Rust integration tests, 41 actual CLI-positive probes and actual vector-reader positive/truncation checks pass locally. The schema and production behavior are unchanged. **User confirmed Claude review complete with no remaining findings on 2026-11-14; M2A is complete.** See the subsequent [M2B probe/input corrections](m2b-verification.md#reviewed-corpus-and-probe-corrections). Hosted CI remains unverified. Semantic engines/parity remain M2B/M2C; Runner execution remains M6A/M6B. No standard publication or deployment is claimed.

**Binding requirements** — authoritative constraints from `spec.md` for this milestone. These are requirements to satisfy and verify, not a second task list; the work itself is in the implementation checklist above.

- Design conformance harness/fixture manifest, **prefer** `conformance/fixtures/{valid,invalid,resolved}` plus expected-results index and optional packaged-content cases. Label subject/level, expected logical status and stable rule IDs (plus JSON paths/severity where meaningful). No universal APDS diagnostic serialization is required. Retain exact bytes/checksums once APDS v1.0.0 freezes.
- Map **all spec cases A–P** to the right verification surface: standalone and resolved-composition validator assertions, packaged-content validation, and/or compatible Runner capability/behavior tests. The Rust/Python parity gate tests only what independent validators can evaluate; it must not require a Registry to execute Loops, Skills or Memory operations.
- Build positive fixtures for every kind, minimal Agent, global/phase and inherited Tool bindings (including duplicate-path logical identity), additive Profiles, Loop `complete`, Knowledge modes and payload references, Memory direct and operation-mediated access/triggers/policies, Template instantiation declarations, and a resolved graph with multiple exact dependency versions.
- Build negative fixtures for missing/unsupported standard, whitespace description, malformed identity/reference, wrong-kind dependency, unknown bound Loop phase, invalid transitions/outcomes, invalid Memory space/operation/schema reference, invalid kind-specific structures and artifact-content mismatches where observable at the declared validation level.
- Include explicit **incomplete/not fully evaluated** fixtures when a valid standalone manifest lacks external graph or packaged content; separate unsupported-standard/Runner capability from authored invalidity. Test AgentPM legacy inference as a compatibility adapter without pretending historical undeclared artifacts are explicitly APDS-conformant.

**Verification:** the standard four evidence requirements apply — see [Per-milestone evidence requirements](#per-milestone-evidence-requirements). Map the AC and decision IDs listed in this milestone's header.

**Exit gate:** The scoped deliverable works against real source contracts; the binding requirements are satisfied, the standard evidence requirements are met, and Claude review has passed.

## Milestone 2B: Make Rust CLI Validate APDS v1.0.0
> **Scope note:** Implement independent Rust APDS schema/semantics validation with stable diagnostics and offline standard selection. Outside this milestone: Registry dependency and automatic fallback to whatever version is newest.
> **Implementation notes:**
> - Start from existing CLI lint infrastructure and reuse current validation shape. The new contract adds a version selector and graph-aware semantic checks; it does not require rewriting the entire CLI.
> - **Dependencies:** M1B/M1C, M2A fixture corpus; **Stage 1 M8** root kind-union diagnostic restructure (Stage 1 owns it — see `spec.md` §3 "Diagnostic substrate"). Without it, rule IDs have no usable manifest path to attach to; coordinate rather than forking a second renderer.
> - **Decisions:** D03, D19. **Acceptance:** S2-AC-01, S2-AC-04.

**Implementation checklist**

- [x] Map manifest `standard` into explicit supported-contract selection.
- [x] Bundle authoritative v1.0.0 schema; validate offline with no GitHub fetch.
- [x] Verify no schema-source override path (CWD-relative file or `--schema`, including an `http(s)` value) can produce a `conformant` APDS verdict or silently substitute the pinned contract; add a regression test that a permissive local `schemas/agentpm.manifest.schema.json` does not make an invalid manifest pass APDS checks.
- [x] Implement intrinsic checks and rule-ID diagnostics with affected JSON paths. Attach rule IDs to the diagnostic shape Stage 1 M8 produced; do **not** re-cut the lint renderer or re-solve root kind dispatch here.
- [x] Verify a kind-specific APDS violation reports the offending field/path plus rule ID rather than a bare `/oneOf` verdict. If Stage 1 M8 has not landed, record this as a blocked prerequisite instead of working around it.
- [x] Implement resolved-graph checks when dependency definitions are available.
- [x] Implement applicable packaged-content checks when referenced artifact files are available; missing files/context must be reported according to whether the validator was actually supplied a complete package.
- [x] Return incomplete/not evaluated rather than false failure when graph or packaged-content context is absent.
- [x] Return unsupported when declaration is unknown rather than silently using v1.
- [x] Preserve human-readable CLI lint formatting and compatible structured output behavior.
- [x] Run entire fixture corpus against Rust expected results, not selective handpicked cases.
- [x] Test invocation in a disconnected local environment.
- [x] Test deterministic diagnostic ordering and duplicate-rule behavior.
- [x] Record supported conformance role/level, evaluated rules and unevaluated checks in AgentPM CLI output used by downstream publish/inspect code; do not claim full-graph verification from standalone success.
- [x] Document any parser behavior that must be fixed to preserve APDS optional dependencies.

**Completion evidence — 2026-11-14:** [Verification, corrections and capability decisions](m2b-verification.md) records the independent Rust engine, explicit `lint --apds standalone|resolved|packaged` integration, all 129 actual CLI corpus results accepted by the portable comparator, 14 targeted integration tests, 20 Python checks, and full workspace regression. The implementation-specific engine/probes remain outside the standard bundle. The historical positive-probe discovery gap and one contradictory inherited-only Skill input were corrected and documented; schema/rule meanings and expected outcomes are unchanged. **User confirmed Claude review passed with no findings and all CLI hosted CI jobs passed on 2026-11-14; M2B is complete.** M3B retains ordinary authoring/preflight rollout; no later milestone is completed by this work.

**Binding requirements** — authoritative constraints from `spec.md` for this milestone. These are requirements to satisfy and verify, not a second task list; the work itself is in the implementation checklist above.

- Update Rust CLI validation engine to select bundled supported version, run schema and semantic checks with stable diagnostics, distinguish standalone and resolved validation, reject unknown selector rather than silently falling back.
- Create a simple contract capability matrix: which checks can run intrinsically, which require the resolved graph or packaged files, and which are **Runner interpretation/compatibility checks outside validator conformance**; report certainty and unresolved checks without false success. Use AgentPM-specific actionable diagnostics without claiming their exact shape is normative APDS.

**Verification:** the standard four evidence requirements apply — see [Per-milestone evidence requirements](#per-milestone-evidence-requirements). Map the AC and decision IDs listed in this milestone's header.

**Exit gate:** The scoped deliverable works against real source contracts; the binding requirements are satisfied, the standard evidence requirements are met, and Claude review has passed.

## Milestone 2C: Make Python Registry Independently Validate the Same APDS

> **Scope note:** Add a Registry-side APDS implementation that independently evaluates the same pinned fixture corpus and agrees with Rust. Outside this milestone: Do not shell out to Rust CLI or accept a client-authored `valid=true` assertion as verification.
> **Implementation notes:**
>
> - Stage 2 needs two implementations with one normative contract. Python JSON Schema library behavior/regex matching may diverge from Rust; explicitly test and reconcile instead of masking differences.
> - **Dependencies:** M2A and M2B expected outcome model.
> - **Decisions:** D03, D19. **Acceptance:** S2-AC-01, S2-AC-04.

**Implementation checklist**

- [x] Choose existing Registry Python test/runtime conventions and pin the v1.0.0 schema bundle.
- [x] Implement standard ID/version lookup with explicit unsupported result.
- [x] Implement schema checks using compatible JSON Schema draft semantics.
- [x] Implement semantic rules independently in Python, respecting standalone/resolved-composition/packaged-content levels and excluding Runner-only execution behavior from Registry validation.
- [x] Produce stable rule IDs, paths and validation-level/result states.
- [x] Run the same **validator-applicable** fixture corpus against Python and compare case-by-case to Rust outcomes; keep Runner interpretation cases in Harness tests, not as Registry execution requirements.
- [x] Test regex, optional arrays and malformed/unknown property handling for parity.
- [x] Add CI parity/checksum job or equivalent cross-repo fixture synchronization gate.
- [x] Test scanner/signing/ACL failures remain separate from APDS results.
- [x] Document how canonical fixtures reach both repos without relying on mutable production network.
- [x] Have Claude exercise representative same-manifest Rust/Python results independently. **Sign-off:** user supplied Claude’s nine-case direct comparison on 2026-11-15; all contract-visible results agree. The nonblocking `evaluated_rules` difference is documented as implementation-specific completed-check reporting.
- [x] Freeze v1.0.0 contract only after discrepancies and semantic mismatch log are resolved. **Reviewed candidate baseline approved 2026-11-15:** parity/review gates passed, the diagnostic-metadata difference is resolved, and [D06 compatibility policy](d06-lockfile-compatibility.md) is settled. Public immutable tag publication remains Release Band 1.

The freeze records approval of the reviewed candidate. Before publication, deliberate revisions can retain version 1.0.0 after synchronization, testing and renewed review. Publishing the immutable `apds-v1.0.0` tag is the separate Release Band 1 commitment.

**Completion evidence — 2026-11-15:** [M2C verification and discrepancy record](m2c-verification.md) maps D03/D19 and S2-AC-01/04 to the independent Python engine, immutable canonical snapshot, full 129-case actual Rust/Python agreement, 39 additional native parity cases and Registry regression/static checks. The developer guide and parity workflow provide offline commands, checksum synchronization and actual results; implementation/test adapters remain outside the versioned standard. User confirmed Claude code review and Python CI passed and supplied Claude’s completed nine-case independent comparison. Conformance fields agree; the nonblocking diagnostic-metadata difference is documented. The [D06 compatibility decision](d06-lockfile-compatibility.md) resolves the final pre-freeze policy requirement. **The reviewed v1.0.0 candidate is frozen and M2C is complete as of 2026-11-15.** No canonical schema/rule/fixture change, public tag, deployment, publication enforcement or M5A implementation is claimed.

**Binding requirements** — authoritative constraints from `spec.md` for this milestone. These are requirements to satisfy and verify, not a second task list; the work itself is in the implementation checklist above.

- Implement independent Python Registry validation engine that uses the **same pinned standard files/fixtures** but independently evaluates schema/semantic rules. Confirm appropriate JSON Schema draft support and regex/semantic equivalence; avoid simply shelling out to the CLI or accepting client assertion.
- Integrate conformance fixture suite in both CI pipelines, with a single documented fixture update process; fail any change that causes divergence or a schema path/version mismatch. Include a hash/check that packaged canonical files match expected versioned bundle.
- Record **D03** and final **D19** shape, and have Claude independently exercise representative fixtures against both validators and the written semantics.

**Verification:** the standard four evidence requirements apply — see [Per-milestone evidence requirements](#per-milestone-evidence-requirements). Map the AC and decision IDs listed in this milestone's header.

**Exit gate:** The scoped deliverable works against real source contracts; the binding requirements are satisfied, the standard evidence requirements are met, and Claude review has passed.

## Milestone 3A: Default CLI `init` to an APDS Agent Package

> **Scope note:** Change new authoring to start with a valid Agent Package and keep explicit scaffolds for seven other kinds. Outside this milestone: mandatory Loop/Tools/Profile; preserve Stage 1 Tool scaffold repair.
> **Implementation notes:**
>
> - This is an intentional early adopter compatibility change. Existing scripts that relied on `init` producing a Tool should select `--kind tool` explicitly; explain the migration in README/init docs. Per user direction, `--kind` help describes the default Agent Package and lists values without Tool migration copy.
> - **Dependencies:** M1B/M2B; **M1A.1 tolerance release 0.1.36 publication prerequisite satisfied by operator confirmation on 2026-10-22**, recorded in Stage 1's Band 5 handoff / Band 7 publication record; Stage 1 M8 CLI authoring improvements. This publication confirmation does not waive M1B/M2B or other M3A acceptance requirements.
> - **Decisions:** D04. **Acceptance:** S2-AC-02, S2-AC-03.

**Implementation checklist**

- [x] Change clap default kind from Tool to Agent.
- [x] Set default project/artifact name appropriate for an Agent Package (not `my-tool`).
- [x] Write a useful nonblank description and APDS `standard` in default scaffold.
- [x] Emit the **pinned versioned** `$schema` URL in all scaffolds instead of the current moving `main` branch URL (`commands/lint.rs` shows the `main` URL in use today, and lint warns when `$schema` is missing, so the field is both emitted and encouraged). New manifests must be edited against the same frozen contract they are validated against. `$schema` stays an editor affordance and must not be treated as the standard selector.
- [x] Keep optional component and Loop fields absent or empty without violating APDS.
- [x] Generate `standard` for Tool, Skill, Knowledge, Memory, Profile, Loop and Template explicit scaffolds.
- [x] Also emit `standard` from `agentpm export`'s generated Skill scaffold (`commands/export.rs` builds a fresh `json!` manifest, so it is a scaffold generator the init-focused tasks above do not cover).
- [x] Preserve Stage 1's runnable Python Tool scaffold files/defaults and Python/Node Tool authoring/packaging compatibility. `init --kind tool` generates Python only; there is no existing Node scaffold to preserve or new Node template delivered here.
- [x] Use proper JSON encoding for quotes, slashes, Unicode and newlines in generated descriptions.
- [x] Validate invalid local names before writing files; prevent partial/broken files on error.
- [x] Ensure `init --kind agent` equals default behavior.
- [x] Snapshot no-flags and all eight explicit kind scaffolds and lint them with v1.0.0.
- [x] Check templates, docs snippets and help that previously assumed default Tool.
- [x] Explain valid-without-Loop but not-yet-Harness-runnable next steps in immediate CLI output.

**Builder evidence (2026-11-15):** [Implementation, migration decision and verification record](m3a-verification.md) maps S2-AC-02/03 and D04 to real CLI tests, nine scaffold snapshots, negative input/collision checks, full workspace regression and feature-local docs. Implementation and local checks pass. User confirmed independent Claude review passed with no remaining findings on **2026-11-15**. User confirmed all hosted CI passed on **2026-11-15**. The [manual runbook](manual-tests/CLI_M3A_MANUAL_TESTS.md) is prepared and locally replayed. User confirmed all manual tests passed on **2026-11-15**; **M3A is complete**, including the requested operator acceptance. Ordinary authoring enforcement remains M3B.

**Existing-capability clarification:** The plan's "Python/Node Tool scaffolding" wording refers to preserving existing runtime compatibility. The shipped init command has only a Python Tool template; Node regression evidence comes from manually authored Tool manifests and payload packaging. A Node init template would be additional functionality, not an M3A preservation fix.

**Binding requirements** — authoritative constraints from `spec.md` for this milestone. These are requirements to satisfy and verify, not a second task list; the work itself is in the implementation checklist above.

- Change `agentpm init` default `--kind` to `agent` and default name to an Agent Package-appropriate value (not `my-tool`); generate a valid package with `kind`, name, version, meaningful description, supported `standard`; no forced Tools/Loop/Profile.
- Update all seven **explicit** non-Agent kinds to emit standard declaration and valid existing kind-specific scaffold, including Stage 1's repaired Python/Node Tool scaffolding. Verify each with lint/build where relevant.
- Serialize user-provided name/description/inputs using a safe JSON writer, never string interpolation. Validate malformed inputs and duplicates without emitting broken JSON or mutating preexisting files unexpectedly.

**Verification:** the standard four evidence requirements apply — see [Per-milestone evidence requirements](#per-milestone-evidence-requirements). Map the AC and decision IDs listed in this milestone's header.

**Exit gate:** The scoped deliverable works against real source contracts; the binding requirements are satisfied, the standard evidence requirements are met, and Claude review has passed.

## Milestone 3B: Make CLI Lint and Authoring Preflight APDS-aware
> **Scope note:** Use the standard selector and diagnostic model in normal authoring commands with clear strict defaults. Outside this milestone: broad CLI messaging rewrite (Stage 1 M9) and premature full docs rewrite (Stage 2 final milestone).
> **Implementation notes:**
> - Local historical definitions may benefit from explicit migration mode, but strict new authored lint must never silently infer APDS and call it verified.
> - **Dependencies:** M2B, M3A, Stage 1 M8/M9 coordination.
> - **Decisions:** D04. **Acceptance:** S2-AC-02, S2-AC-03, S2-AC-04.

**Implementation checklist**

- [x] Make normal `lint` validate explicitly declared supported standard.
- [x] Distinguish missing standard, unknown version, schema defect and resolved-graph issue in output.
- [x] Provide targeted remediation including version selector and source path.
- [x] Preserve exit statuses, stderr/stdout/machine output guarantees from Stage 1.
- [x] Evaluate need for optional opt-in legacy migration/lint mode and record D04 outcome.
- [x] Prevent permissive migration mode from becoming default publish authorization.
- [x] Ensure build/inspect/publish preflight reuse same canonical validation outcomes.
- [x] Regression-check `agentpm new`, `agentpm run`, Tool exports and the existing publish link.
- [x] Snapshot interactive and noninteractive diagnostics.
- [x] Test whitespace descriptions, malformed names, unknown kinds and incorrect paths.
- [x] Have Claude verify users don't confuse APDS validation with Runner readiness.

**Local implementation evidence (2026-11-16):** [Verification and D04 decision](m3b-verification.md) cover strict ordinary lint, the user-selected read-only `--legacy` adapter over the same APDS engine, shared local build/inspect/publish gates and saved-build rejection. Workspace tests pass **1,287 tests (6 ignored)**; the expanded authoring integration suite passes **9 tests**, Clippy/formatting and frozen-schema checks pass, the actual-CLI fixture probe passes 41 positive manifests plus the Knowledge pair, and public docs tests pass **5 tests plus 166 subtests**. Scope/legacy/migration notes and human/machine diagnostic snapshots are recorded. The [Template-generation review correction](m3b-verification.md#review-correction-template-generation-and-the-next-lint-command) adds the supported declaration to the CLI-generated root and generation-time migration guidance for undeclared nested Agents; 16 authoring tests and 30 Template-generation tests pass after that correction. The follow-up Stage 1 messaging correction deliberately updates both goldens, retains one next-step line and groups nested migration notices; the final `cargo test --workspace --no-fail-fast` passes 1,287 tests with 0 failures and 6 ignored. **User confirmed full Claude review passed on 2026-11-16, including the resolved Template-generation and messaging findings. User confirmed all hosted CI passed for the reviewed implementation on 2026-11-16. User confirmed all [local-project manual tests](manual-tests/CLI_M3B_MANUAL_TESTS.md) passed on 2026-11-17. **M3B is complete**, including independent review, hosted CI and requested operator acceptance.**

**Binding requirements** — authoritative constraints from `spec.md` for this milestone. These are requirements to satisfy and verify, not a second task list; the work itself is in the implementation checklist above.

- Integrate APDS selector/rule diagnostics into `agentpm lint` with actionable CLI messages, correct exit codes, and structured/machine output compatibility where present. Add a direct path/command for validating published/installed artifacts if existing inspect flow permits without inventing an unrelated command family.
- Handle unsupported/missing standard in newly authored files strictly. Investigate explicit opt-in local migration/legacy lint helper; **D04** decision must explain migration UX and why it does not make silent bypass the default.
- Preserve Tool execution `agentpm run`, Template `agentpm new` and `agentpm publish` existing behavior at this milestone; top-level category help/copy polishing is later M15.
- Tests: no-flags init, all eight explicit kinds, dangerous escape characters/Unicode, no-dependency Agent lint, old manifests, unknown standard, direct semantic rule failures; use Stage 1 CLI golden-output conventions.
- Document any authored artifact migration notes needed **now** to avoid developer dead ends; defer broad docs rewrite to M17A–M17C.

**Verification:** the standard four evidence requirements apply — see [Per-milestone evidence requirements](#per-milestone-evidence-requirements). Map the AC and decision IDs listed in this milestone's header.

**Exit gate:** The scoped deliverable works against real source contracts; the binding requirements are satisfied, the standard evidence requirements are met, and Claude review has passed.

## Milestone 4A: Enforce APDS in Registry Publish Finalization
> **Scope note:** Make the Python Registry reject invalid newly uploaded releases by inspecting the staged artifact, independent of CLI validation. Outside this milestone: rewriting existing artifacts; no replacing Stage 1 release integrity or malware/ACL gates.
> **Implementation notes:**
> - Treat the Registry as the authoritative new-release gate even for custom HTTP clients. Preserve atomic finalize and accurate errors.
> - **Dependencies:** M2C; Stage 1 M13A/B release/finalize changes if deployed.
> - **Decisions:** D05, D19. **Acceptance:** S2-AC-05.

**Implementation checklist**

- [x] Trace init/finalize/upload/scan flow and identify the last safe precommit enforcement point.
- [x] Add version-scoped standard ID/origin metadata with backward-compatible migrations/serializers.
- [x] Load actual embedded `agent.json` from staged artifact using existing secure tar validation.
- [x] Run Python APDS structural checks and all resolvable semantic checks.
- [x] Compare artifact local identity, Registry namespace mapping, kind and version against init metadata.
- [x] Compare embedded declared standard against request metadata; reject mismatches.
- [x] Ensure multi-artifact Tool publish applies correct release/artifact association checks.
- [x] Keep malware scan, signer verification, Registry attestation, ACL and APDS statuses separate.
- [x] Reject before discoverability and preserve atomic rollback/idempotent retry behavior.
- [x] Add forged request tests that bypass CLI lint and still fail server.
- [x] Add malformed/missing/unsupported schema and mismatched tar negative tests.
- [x] Expose precise diagnostics and retain existing successful Registry package link.
- [x] Prove unsigned/invalid signature cannot be bypassed merely by APDS conformance.

**Builder verification:** [M4A evidence and decisions](m4a-verification.md) records local archive, PostgreSQL/Pyway, HTTP, docs and static checks. User confirmed full independent Claude review passed with no remaining findings on 2026-11-17. User confirmed all hosted CI passed on 2026-11-17. User confirmed all [real-local publication manual tests](manual-tests/REGISTRY_M4A_MANUAL_TESTS.md) passed on 2026-11-18; the saved acceptance summary records 36 passing cases. Enforcement defaults off while declaration-only recording remains active. M4B owns production history/session inventory and activation. **M4A is complete.**

**Binding requirements** — authoritative constraints from `spec.md` for this milestone. These are requirements to satisfy and verify, not a second task list; the work itself is in the implementation checklist above.

- Inventory published version rows, version identity, init/finalize endpoints, staged tar/manifest bytes, scan/attestation and publish metadata. Record snapshot of legacy cases and queued/pending publish sessions.
- Add `standard` ID/version and explicit/inferred provenance to Registry schema/response DTOs where needed; version rollouts/backfills distinguish inferred from declared and never rewrite immutable tar, digest, signature, release version or malware evidence.
- Validate staged `agent.json` actual bytes, not merely init request payload; validate requested kind/name/version/standard match embedded manifest and resolved package/release metadata, including Stage 1 multi-artifact target release paths. Enforce **every new version**, including a release under legacy package identity.
- Apply independent Python APDS validator; reject structural/semantic violations prior to final commit/discoverability and without bypass via custom client or old CLI. Properly separate APDS errors from ACL/namespace, scan, integrity and signing failures.
- Decide reliable resolution context needed for composition validation: prove what server can validate at publish, report incomplete checks rather than claiming full conformance, and decide hard publishing requirements without blocking legitimate packages that reference unavailable/private graph context unintentionally. Preserve normal dependency-visibility/authorization behavior.

**Verification:** the standard four evidence requirements apply — see [Per-milestone evidence requirements](#per-milestone-evidence-requirements). Map the AC and decision IDs listed in this milestone's header.

**Exit gate:** The scoped deliverable works against real source contracts; the binding requirements are satisfied, the standard evidence requirements are met, and Claude review has passed.

## Milestone 4B: Roll Out New-Publish Enforcement Without Breaking Legacy Releases
> **Scope note:** Define safe transition policy for legacy published bytes, new versions of old identities, and sessions in flight at cutover. Outside this milestone: grandfathering of new versions and false retroactive declaration.
> **Implementation notes:**
> - This milestone implements and verifies the compatibility switch: old immutable releases stay installable, but all new finalized versions need declared v1.0.0 or other explicitly supported APDS when enforcement is active. Per the agreed sequencing, production inspection/backfill, capacity measurements and activation execute as Release Band 1 operator gates; preparing and locally verifying them belongs here.
> - **Dependencies:** M4A, Stage 1 multi-artifact rollout status.
> - **Decisions:** D05, D19. **Acceptance:** S2-AC-05.

**Implementation checklist**

- [x] Inventory available real pre-APDS versions and immutable digest/signature references, and prepare the production inventory/archive-inspection command and procedure. Actual production execution is required by Release Band 1 steps 4–5. **Local database inventory complete (2026-11-18):** 113 versions, 16 pending session rows, captured immutable/signature references in ignored `target/m4b/history-local.json`. Production inventory/archive inspection runs during [Release Band 1's ordered production rollout](#ordered-production-rollout), after deploying the audit command with enforcement off and before activation; null rows are not automatically classified as legacy.
- [x] Define server-controlled enforcement boundary for each new finalized release.
- [x] Choose and record in-flight session restart/grace policy; do not trust client timestamp.
- [x] Make outdated CLI error actionable by naming the **concrete minimum CLI version, 0.1.36** — the M1A.1 tolerance release whose publication was operator-confirmed on 2026-10-22 in Stage 1 Release Band 5 / Release Band 7 — not a generic "upgrade your CLI". Document the same floor and concrete upgrade steps in feature-local `agentpm-api/docs` MDX. Package-page guidance is owned by M9B/M10A alongside their APDS/detail work; M4B adds no APDS-facing web UI. An upgrade error without a version floor is not actionable.
- [x] Preserve already published manifest bytes and stored integrity values exactly.
- [x] Represent effective legacy `agentpm/1.0.0` interpretation as **inferred**, not verified.
- [x] Require explicit standard for new versions of historically legacy package identities.
- [x] Test sessions created before deployment and finalized afterward.
- [x] Test old released version install/lookup and its new APDS-compliant successor side by side.
- [x] Exercise rollback on rejected finalize and ensure no phantom discoverable package exists.
- [x] Add migration/verification script if backend metadata needs backfill; never re-sign old tar.
- [x] Document deployment order of CLI, Registry and public UI to avoid temporary false success.
- [x] Prepare the T-PUB-10 capacity procedure and activation gate covering gate-off/gate-on publication of dependency-heavy Agent Packages/Templates, representative large artifacts, repeat versions and planned concurrency; require GET/byte/extraction/latency/RSS/temp-disk/cleanup measurements against actual resource and timeout limits. **Preparation is complete; measurement has not run.** Execution and reviewed operating-envelope evidence remain mandatory at [Release Band 1 step 6](#ordered-production-rollout), before approval or activation. Count/metadata and per-artifact bounds are not an aggregate artifact-byte budget; see [M4A capacity findings](m4a-verification.md#capacity-evidence-required-before-m4b-activation).
- [x] Have Claude approve migration/security matrix before server enforcement is enabled. **Design/local-evidence approval granted**, as supplied by the user on 2026-11-18. This approval is separate from [Release Band 1 step 7](#ordered-production-rollout), which remains pending production history findings and the measured capacity envelope. The stored-versus-artifact guard finding has been addressed by six passing regression cases, each also failing when the guard is disabled. The user subsequently confirmed full Claude review complete with no new findings on 2026-11-18. Production activation approval is still separate and pending.

**Builder handoff (2026-11-18):** [M4B implementation, compatibility matrix and verification](m4b-verification.md) records the no-grace finalize-time policy, concrete 0.1.36 publisher guidance, explicit artifact-derived history audit/backfill command and local regression evidence. Full Registry suite: **1,071 passed, 7 unrelated opt-in skips, 347 subtests passed**. No production changes or activation occurred. Claude's migration/security design approval is granted, and the user confirmed full Claude review complete with no new findings on 2026-11-18. The user confirmed all PR CI passed on 2026-11-18; actual CLI build and all four Resolver Contract native targets were independently verified in GitHub. Production history inspection/backfill and immutable-reference verification were subsequently operator-confirmed on 2026-11-19 (138 inferred versions, zero remaining backfills or reference changes). The measured capacity envelope and later activation approval remain outstanding. The user subsequently confirmed release-preparation PR CI in all three repos and CLI main/post-merge CI passed on 2026-11-18. **M4B is complete** as the implementation/review/PR-CI milestone. Production execution is tracked by Release Band 1; its remaining capacity/activation/smoke gates stay unchecked there; this completion does not certify production capacity or enable enforcement.

**Binding requirements** — authoritative constraints from `spec.md` for this milestone. These are requirements to satisfy and verify, not a second task list; the work itself is in the implementation checklist above.

- Define and test **rollout cutoff and in-flight session policy** on the server. Prepare compatible CLI release + Registry accepted DTOs before the strict gate activates; explicit upgrades/errors for older publishing clients, failed finalize cleanup and safe rollback.
- Preserve old published versions as installable and their original bytes/signatures; report an effective legacy interpretation only with **legacy-inferred** status, never `verified` unless separately supported by evidence.
- Integration/security tests: direct forged init, request/manifest mismatch, altered staged tar, invalid selector/rule, mixed release artifact targets, private deps, pending session boundary, legacy old version, new version of legacy package, stable server error classification and atomic finalization.
- Produce an end-to-end publication compatibility/migration brief and D05/D19 decision record; require Claude review before enabling strict enforcement in production.

**Verification:** the standard four evidence requirements apply — see [Per-milestone evidence requirements](#per-milestone-evidence-requirements). Map the AC and decision IDs listed in this milestone's header.

**Exit gate:** The scoped deliverable works against real source contracts; the binding requirements are satisfied, the standard evidence requirements are met, and Claude review has passed.

## Release Band 1: APDS Definition, Authoring and New-release Enforcement
Covered milestones: 1A–4B.

**Status: complete.** Independent review, hosted CI, releases, activation, approved capacity evidence and the operator-accepted production smoke scope are complete. The final quick API results are recorded below; larger retry/release-negative/cutover cases retain reviewed local/CI evidence rather than being claimed as additional production runs.
**Entry gate:** Stage 1 M2 and M11A manifest-schema additions must be merged before M1B begins. **M1A and M1A.1 are ungated and should start immediately** — M1A.1 in particular should ship during Stage 1, since nothing may emit `standard` until it is released.

This gives AgentPM one pinned APDS v1.0.0 definition derived from its current schema, well-specified Phase 6/7 semantics, independent Rust/Python conformance, Agent Package-first new authoring, and a server-side new-release gate. Shipping the strict gate must be coordinated with the CLI and legacy session policy. An existing immutable release remains installable, but a new version cannot bypass APDS by using an older client.

- [x] Confirm the M1A.1 tolerance release shipped **before** any command emits `standard`, that its version is recorded as the minimum CLI floor, and that round-trip preservation is proven by test.
- [x] Confirm Stage 1 M2/M11A schema additions were merged **before** the v1.0.0 freeze and are represented in the frozen contract plus fixtures.
- [x] Confirm one canonical immutable schema, normative rules and shared fixture corpus, with offline parity across Rust and Python.
- [x] After complete schema/semantics/corpus and independent contract/readability review, publish the immutable CLI-repo `apds-v1.0.0` tag containing `standards/agentpm/1.0.0/`. Verify the pinned raw schema URL is available **before releasing the CLI that emits those editor links**. M1B creates the candidate and chooses the URI; it does not publish this tag or ship a complete standard. **Publication confirmed 2026-11-18:** the user fetched the public tagged schema and obtained SHA-256 `f123150ff42cea4b29edb5076d7d70697269d20ad85118b4a07fe5af5cf2ab04`, exactly matching the reviewed bundle. APDS 1.0.0 is published; its normative files/tag must not be changed in place.
- [x] Confirm the APDS README and `semantics.md` pass independent newcomer readability review: concise rules, skimmable navigation, clear guarantees, and no unnecessary schema duplication or implementation-specific prose.
- [x] Confirm all eight newly authored kinds lint, default init is Agent, explicit Tool scaffold remains valid.
- [x] Run custom-client server bypass attempts and in-flight/legacy/new-version migration matrix.
- [x] Obtain Claude review of APDS rule coverage, schema/version immutability, security and rollout order.

**Release preparation — 2026-11-18:** [Readiness/version/evidence record](release-band-1-readiness.md) reconciles these prerequisites with the completed milestone records and fresh local checks. CLI **0.1.45**, a new public changelog, matching Registry migration-version guidance and Web CLI status marker (planned 2026-11-18) are prepared. SDK versions remain 0.1.17. Fresh Rust/Python corpus executions agree on all 129 cases; the rebuilt CLI and eight scaffold kinds validate. The checklist above records local/review prerequisites, not production execution. The user subsequently confirmed APDS tag/public-schema publication on 2026-11-18 with the expected SHA-256; CLI release/install confirmation and production tests/activation remain below.

### Ordered production rollout

These are **Release Band 1 operator gates**. Complete code review and hosted CI
before deploying. Production history inspection requires the new Registry audit
command to be deployed in Render; it is required **before enforcement activation**,
not before that initial deployment. Keep enforcement off through inspection and
capacity verification. The band remains open until the rollout and smoke checks
below are evidenced. The M4B checklist references these same operational gates;
local test results do not satisfy them.

Follow this order; the isolated capacity work may run in parallel after the
reviewed build and intended production configuration are available:

1. [x] Confirm independent review and all required pre-merge hosted CLI/Registry/Web CI pass for the exact release revisions. The user confirmed all three release-preparation PRs passed CI on 2026-11-18, and the CLI was merged to main with all push-triggered jobs successful. M4B independent review was already complete. Public main is `914cb00f0e1b0a1c3a31997cb66ee192fbc91e68`; its full APDS directory was independently verified identical to the reviewed frozen source. Actual CI completion is user-confirmed; tag/publication steps remain below.
2. [x] Complete the immutable APDS tag/raw-schema verification above. **CLI main merge and all post-merge jobs are complete**, as confirmed by the user on 2026-11-18; **APDS tag/public-schema publication also passed on 2026-11-18** with the expected schema SHA-256. **CLI 0.1.45 release and installation confirmation also passed**, as confirmed by the user on 2026-11-18. After merging the CLI to `main`, confirm the push-triggered `CI / platform-targets` and Native runtime matrix pass before tagging the CLI release. Then release the matching authoring CLI and confirm it is installable through the supported distribution channels. Keep **0.1.36** documented as the already-shipped declaration-tolerance floor; recommend the current release for authoring checks.
3. [x] Deploy the reviewed Registry code and matching feature-local MDX with **`APDS_PUBLISH_ENFORCED=false`**. Verify the deployed revision and setting, normal historical lookup/install behavior, and availability of `registry audit-apds-history` in the production Render shell from the directory containing `run.py`. This deploy makes inspection possible; it does not activate enforcement. Deploy the prepared Web CLI status marker only after CLI 0.1.45 is published/installable, using its actual release date. APDS-facing package-page presentation remains with M9B/M10A. **Operator confirmation 2026-11-18:** CLI release/installation and Web/Registry deployments completed with additional CI passing; Registry enforcement remains off. Production audit/inspection is the next active step. Historical install and publication smoke evidence remains required in step 9.
4. [x] From that production shell, run `python -m flask --app run registry audit-apds-history --out history-before.json`, then `python -m flask --app run registry audit-apds-history --inspect-artifacts --out history-inspected.json` (or reviewed `--version-id` batches). Retain the production version/digest/signature references and pending-session inventory. Inspect actual archives before deciding declared versus inferred provenance; do not treat null metadata as proof of legacy. Keep the private reports out of Git. **User-confirmed production inspection — 2026-11-19:** enforcement false, 138 versions inspected, zero pending sessions, 138 metadata backfills proposed (all `legacy_inferred`), zero actually backfilled. Private reports are in `/tmp/agentpm-apds-audit.NugfKM` on the service; the summary was supplied by the operator. The user subsequently confirmed step 5 applied 138 metadata backfills; post-apply inspection/fingerprint verification also passed, as supplied by the user on 2026-11-19.
5. [x] Review production discrepancies and proposed metadata changes. If backfill is needed, deliberately run the reviewed `--inspect-artifacts --apply` command with a fresh report filename, then inspect again to confirm no missing backfill remains in the selected batch. Compare immutable version/artifact/signature/attestation fingerprints before/after; no archives or signatures may be rewritten. Resolve blockers before activation; a no-change result is valid when no backfill is required. **User-confirmed apply — 2026-11-19:** `APDS history: 138 versions; 138 backfilled`, saved as `/tmp/agentpm-apds-audit.NugfKM/history-applied.json`. **Post-verification confirmed by the user:** enforcement false, zero remaining backfills, 138 stored `legacy_inferred` origins, zero missing original versions and zero immutable-reference changes, with the verification PASS line. This completes step 5; it does not certify capacity or authorize activation. **Full report retention/verification — 2026-11-19:** the operator's checksum-framed copy/paste archive was verified and unpacked locally into ignored `release-band1-evidence/production-audit/`; the builder independently rechecked all four 138-version reports and confirmed the same results. The private checksum inventory and verification summary are retained with the reports.
6. [x] Satisfy **T-PUB-10** using the operator-approved local Docker approach (2026-11-19), without an additional Render environment: constrain the actual Linux Registry to 0.5 CPU / 512 MB RAM, use the normal Gunicorn entrypoint/defaults and disposable PostgreSQL/Redis/S3 emulator, and explicitly review differences from production architecture, CPU performance, disk, S3 latency and proxy timeouts. Cover both protocols, gate off/on, the realistic Agent Package/Template graphs, representative large artifacts, repeated publications and planned concurrency. Record GET counts/bytes, cumulative extraction work, latency, peak worker RSS/temp-disk use and cleanup; review an acceptable operating envelope or verify mitigation. Use the [single-command capacity runbook](manual-tests/REGISTRY_CAPACITY.md). A successful script is measurement evidence, not automatic activation approval; the quick profile only verifies tooling. Confirm actual production worker/timeout configuration and review the local-to-production uncertainty before step 7. Maximum-load experiments are not required on the live Registry. **Production baseline reconfirmed by the user 2026-11-19:** 0.5 CPU / 512 MB RAM, start command `gunicorn run:app`, with no production configuration changes. Capture effective worker/timeout settings and disk headroom during measurement. **Builder measurement 2026-11-19:** the single-command representative run passed 100 finalize attempts (98 publications, one APDS rejection, one controlled timeout), with 306.04 MiB peak cgroup memory, no OOM/unplanned restart and no sequential temp-file residue. See [measurement evidence and limits](release-band-1-readiness.md#local-capacity-measurement-completed--2026-11-19). **Claude independently approved the measured envelope on 2026-11-19**, within the tested protocols/settings, approximately 26 nodes, 64 MiB compressed/128 MiB expanded and at most two concurrent publishers. Production activation was explicitly not approved. Mandatory orphan reclamation plus production configuration confirmation remain prerequisites under step 7; no maximum-size or sustained-load certification is implied.
7. [x] Obtain Claude's activation approval against the production history findings, migration/security matrix and measured capacity envelope. Review/test the required orphan cleanup (including SIGKILL during extraction and named snapshots), pass follow-up Registry CI, and deploy it with enforcement still off. Retain `python scripts/inspect_publication_capacity.py` output for actual workers/timeouts, temp mount type/disk headroom and live Redis-backed rate limiting. The operator confirmed HTTP `/health` and no additional proxy/CDN; review Render's documented response/health thresholds and the remaining single-sync-worker queue starvation risk. **Follow-up review 2026-11-19:** Claude accepted the cleanup mechanism and real SIGKILL/snapshot proof and re-approved the revised 333.90 MiB measured envelope. The two nonblocking guard-coverage comments were closed with isolated tests and in-memory mutation checks. Runtime code is unchanged by that coverage follow-up. **Operator confirmation 2026-11-19:** CI passed and the Registry cleanup deploy completed with enforcement still off. **Production probe received 2026-11-19:** one sync worker/thread, 30-second timeouts, ext4 `/tmp`, 48.69 GiB free, 512 MiB memory, live Redis counters and enforcement false all confirmed. A remaining mismatch is production `preload_app=true` versus the measured false setting, explained by Render's automatic native-Python `GUNICORN_CMD_ARGS` default. **Aligned follow-up probe received:** `preload_app=false`, one sync worker/thread, 30-second timeouts, ext4 `/tmp` with 52.73 GiB free, 512 MiB memory, zero inspection names, live Redis counters and enforcement false. Both operator probe outputs are retained privately. The configuration mismatch is resolved. Claude subsequently approved activation conditional only on green hosted Registry CI for the exact deployed revision, with no further review required. The builder verified successful hosted runs on `72bb09016632a80a3ca621eda9125bbfa0597796`, then found that the release job's explicit pytest list omitted the cleanup/probe test files. Both are now included; the exact updated job selection passes locally (449 tests and 24 subtests). The operator subsequently confirmed the corrected hosted CI passed and the Registry was redeployed with enforcement enabled. Claude's final CI condition is satisfied; application code is unchanged by the workflow correction. See the [CI correction and cutover record](release-band-1-readiness.md#final-ci-coverage-correction-and-operator-cutover--2026-11-19). Closing the Render shell does not require replaying the already-applied history backfill; the four reports are retained locally. Confirm publisher upgrade guidance and the no-grace pending-session policy are ready. Record the release revisions and retain the evidence before changing the flag.
8. [x] **Operator-confirmed cutover simplification:** there are no current users or active publishes, so no separate publishing-pause mechanism is needed. After step 7's CI condition is met, set **`APDS_PUBLISH_ENFORCED=true`** in Render and redeploy **all** Registry workers. Confirm the new deployment is serving and old workers are gone; record server UTC activation time, deployment ID, commit and configuration evidence. If publication traffic starts before the cutover, pause it and drain running finalizes first. Pending sessions receive finalize-time validation regardless of age; compliant unexpired sessions can finish, while invalid sessions need correction, rebuild and a fresh reservation. **Operator-confirmed activation:** corrected hosted CI passed and the Registry redeployment completed with `APDS_PUBLISH_ENFORCED=true`. This is operator-reported deployment evidence; no builder production connection or mutation occurred. Retain the precise deployment ID, commit and UTC activation time with step 9 evidence; the current deployed commit was subsequently supplied as `80f860a0465dae15eae4e8e57d56033b87b499c7`; the original activation UTC and deployment ID have not been supplied.
9. [x] Finish the **operator-requested quick production API check**: resolve the published GitHub Tool, upload/finalize a tiny Agent archive missing `standard`, confirm HTTP 422 with `APDS-ID-001 /standard` and CLI upgrade guidance, then confirm that rejected version cannot resolve. See the quick script below. **PASS confirmed:** the operator supplied `quick-api-results.json` for `0.0.0-smoke-676fe4773c39`; the builder verified the locally saved report: published Tool resolves, finalize HTTP 422 with `apds_nonconformant` / `APDS-ID-001 /standard` and CLI >= 0.1.36 guidance, and rejected version resolve returns `no_satisfying_version` without that version. This closes step 9 under the agreed scope. The operator requested a quick finish instead of the larger remaining production matrix: completed-finalize retries, release-protocol negative cases and cutoff/staged-session policy retain their reviewed local/PostgreSQL/hosted-CI evidence and are **not claimed as newly run production tests**. Keep enforcement enabled; ordinary rollback retains it.

**Current production evidence:**

- Immutable APDS publication, CLI **0.1.45 → 0.1.46** release/install confirmation, independent review and hosted CI are operator-confirmed complete. Registry enforcement is enabled; matching Registry docs and Web updates are deployed.
- Production history inspection/backfill preserved all 138 historical versions and immutable references. Reports are retained privately; fresh historical install archives/locks match the audit.
- Both publication protocols succeeded: summarize-text **0.1.11 / ID 139**; github-issues **0.1.2 / ID 140**; issue-triage-playbook **0.1.1 / ID 141**; devwork-copilot **0.1.10 / ID 142**. Do not repeat successful publications. The failed Agent 0.1.9 reservation expires normally after one day; 0.1.10 used a fresh reservation.
- Smoke runbook steps **2–5 passed**. Both new clean installs reported machine success with required signature/attestation flags in the runbook. The builder checked all nine lock entries, canonical Python release integrity, cached archive provenance, installed manifests/READMEs and managed native imports. Consumer apps and installed locks were not manually edited.
- The post-activation **Render operational probe passed** on deployed commit **80f860a0465dae15eae4e8e57d56033b87b499c7**: enforcement true, one sync worker/thread, preload false, 30-second timeouts, ext4 `/tmp`, **59.93 GiB free**, 512 MiB memory limit, zero managed/unrecognized inspection names, health HTTP 200 and two fresh Redis counters. The probe's `activation_approved: false` is its constant non-approval marker, not a failed runtime check. Its scope remains shell/config interpretation plus live loopback health/Redis, not process argv or another capacity run. Deployment ID/original UTC activation time and a separate publication-window log review have not been supplied; no such evidence is invented.
- The temporary-directory setgid failure and README/archive omission were corrected, reviewed, CI-tested and released. Final CLI verification: **1,297 passed, 0 failed, 6 ignored**, clippy/format checks passing; Registry/real-Linux cleanup evidence remains in the [readiness record](release-band-1-readiness.md). Python declarations now fail locally for missing/unsafe/invalid-text/oversized files while undeclared discovery stays optional.

**Final quick check:** from any local directory, run `python3 /tmp/agentpm-quick-api-smoke.py` (also retained as `release-band1-evidence/smoke/20261120T034804Z/quick-api-smoke.py`). It prompts for the existing PAT if `AGENTPM_TOKEN` is unset, performs the small raw-API negative test, and saves `quick-api-results.json` under `BAND1_SMOKE` or a private temporary directory. No new version is expected to publish; a tiny pending upload remains under normal expiry. The builder syntax-checked the script only and has not executed it against production. **Final result received and verified: step 9 is complete.** The private saved report matches the operator-pasted result; no new version was published by the negative test.

**Release Band 1 is closed.** No further smoke command is required under the operator-accepted scope.

The [smoke runbook](manual-tests/RELEASE_BAND1_SMOKE.md) and [readiness/incident record](release-band-1-readiness.md) retain detailed commands, version history, measured capacity and scope limits. Private evidence lives under ignored `release-band1-evidence/`. The original larger matrix is not silently presented as completed: the operator explicitly requested quick API checks to finish, with the other behavior supported by reviewed local/CI tests. Remaining live signature/attestation row-fingerprint comparison was not rerun after activation; old payload integrity and required verification were exercised by the production installs.

Detailed operator commands, inspection semantics and measurement requirements are
in `agentpm-api/app/apds/ROLLOUT.md`; implementation/test evidence is in
[M4B verification](m4b-verification.md). Do not close this band solely because the
code is merged, deployed with enforcement off, or locally verified.

---

## Milestone 5A: Carry APDS Identity Through Resolve, Install and agent.lock
> **Scope note:** Make resolved and installed packages retain their effective APDS standard ID/version and the distinction between authored declaration and historical inference. Outside this milestone: Do not introduce host-target metadata into the portable lock or duplicate Stage 1 lock v4.
> **Implementation notes:**
> - The authoritative bytes are the installed artifact manifest; versioned Registry records, resolve DTOs and lock metadata are cached/derived views that must be reconciled rather than blindly trusted.
> - **Dependencies:** M4B; Stage 1 M11B lock v4, **Stage 1 M14A** release-digest contract (M14A defines release-level SHA-256 over canonical release-manifest bytes and keeps `LockedPackage.integrity` a bare 64-char lowercase hex; M14B covers release-level signature statements and stored-byte integrity, which this milestone does not depend on).
> - **Pre-freeze decision settled 2026-11-15:** [D06 record](d06-lockfile-compatibility.md) selects extended v4 and a minimum-CLI rollout. Implement known metadata for installed packages and represented local roots, including justified `legacy_inferred`; prove regeneration and SDK 0.1.17 compatibility. Implemented as optional `standard` plus `standard_origin` (`declared` / `legacy_inferred`), requiring v4 and CLI 0.1.47+. See [verification](m5a-verification.md); publication and rollout verification remain pending.
> - **Decisions:** D06, D19. **Acceptance:** S2-AC-06.

**Implementation checklist**

- [x] Read Stage 1's final `LockedPackage`, resolve plan and forward-version checks as implemented.
- [x] Choose standard/origin lock shape while preferring lockfile version 4.
- [x] Add explicit versus legacy inferred origin to Registry resolve responses.
- [x] Map APDS metadata through CLI resolve DTO, in-memory plan and lock serialization.
- [x] Retain new metadata through ordinary install lock regeneration.
- [x] Reconcile the locked version/kind/name with the actual downloaded embedded manifest.
- [x] Verify integrity using Stage 1's logical release digest rules, not new ad hoc checksum logic.
- [x] Do not write chosen platform target or local Python interpreter into portable lock entries.
- [x] Test v2/v3 reads, new v4 writes, future lock version rejection and unknown-field safety.
- [x] Test multiple root kinds and exact version identity collisions.
- [x] Document any justified v5 proposal before coding it and require Claude compatibility review. **Not applicable:** D06 retains v4; no v5 proposal or implementation. User confirmed Claude review of the selected extension passed with no remaining findings on 2026-11-20.
- [x] Check legacy inferred status is not promoted to declared/verified by serialization.
- [x] Prepare and builder-replay the [M5A manual runbook](manual-tests/CLI_M5A_MANUAL_TESTS.md), with visible sources, exact commands, expectations and captured results under gitignored `cli-m5a-test/`. Real local API/DB/dev-S3 publications and installs are distinct from controlled loopback download faults. Four isolated builder checks pass; those checks are separate from operator acceptance below.
- [x] Complete operator manual testing after Claude review and **before hosted CI**, as the user explicitly requested. User supplied the passing real-local audit on 2026-11-20: **38 command results**, all expected/actual exits matching, in `cli-m5a-test/20261120T170444Z-21931/evidence/summary.json`. The helper's fixed `operator_signoff: false` and `hosted_ci_complete: false` fields do not self-grant approval. Declared/legacy installs, v4 metadata, regeneration, supported frozen replay and exact rejection/unchanged-state checks passed the operator-run audit.
- [x] After manual acceptance, push the reviewed changes together and confirm hosted Registry/CLI/SDK CI, including native target jobs, before marking M5A complete. **Operator confirmed all hosted CI passed on 2026-11-20.**

**Binding requirements** — authoritative constraints from `spec.md` for this milestone. These are requirements to satisfy and verify, not a second task list; the work itself is in the implementation checklist above.

- Read Stage 1's finalized `agent.lock` v4 contract and implement APDS standard+declared/inferred metadata without introducing an extra file format when possible. Verify no existing v4 reader silently drops it or rewrites future unknown fields.
- Carry effective standard and provenance through Registry resolve DTO, CLI resolved package plan, installed state and lock records; compare against actual embedded manifest + pinned release kind/name/version/digest.
- Validate unsupported standard and malformed/mismatched identity in install path with clear remediation; preserve verified release integrity and target selection from Stage 1.

**Verification:** the standard four evidence requirements apply — see [Per-milestone evidence requirements](#per-milestone-evidence-requirements). Map the AC and decision IDs listed in this milestone's header.

**Exit gate:** The scoped deliverable works against real source contracts; the binding requirements are satisfied, the standard evidence requirements are met, and Claude review has passed.

**Implementation evidence — 2026-11-20:** [M5A verification](m5a-verification.md) records the Registry/CLI/lock propagation, embedded-manifest reconciliation, ordinary regeneration and frozen metadata regressions, Stage 1 integrity/target safeguards, actual published SDK 0.1.17 probes and unchanged-lock rejection by released CLI 0.1.46. CLI 0.1.47 is prepared but unreleased. User confirmed Claude review passed with no remaining findings on 2026-11-20. User supplied a passing 38-result operator-run manual audit on 2026-11-20. **User confirmed all hosted CI passed on 2026-11-20; M5A is complete.** Reader-before-shared-emission release verification and restoring SDK CI selection to CLI `main` remain Release Band 2 gates. M5B graph/semver/empty-Agent fixes are not claimed.

## Milestone 5B: Fix Frozen Installs, Dependency Closure and Empty-Dependency Agents
> **Scope note:** Make a clean workspace reproducibly install a locked Agent/Skill/Component graph and gracefully handle empty dependency sets. Outside this milestone: broad resolver rearchitecture or redo of Stage 1 Python target-aware install.
> **Implementation notes:**
> - `--frozen` is meaningful only when it reproduces exact root and transitive closure from the lock. Current Tool assumptions in direct kinds and checksum/version satisfaction must be audited explicitly.
> - **Dependencies:** M5A; Stage 1 M15 target-aware installer.
> - **Decisions:** D06. **Acceptance:** S2-AC-07.

**Implementation checklist**

- [x] Reproduce frozen direct Agent root, not just frozen Tool root.
- [x] Reproduce frozen direct Skill root and its Tool dependencies.
- [x] Reconstruct full transitive closure in an initially empty `.agentpm` workspace.
- [x] Validate every locked dependency satisfies authored version range.
- [x] Validate exact kind identity and selected version for every edge.
- [x] Fail on missing lock entry rather than filling it from latest Registry version.
- [x] Fail on Registry returned digest mismatch before modifying installed state.
- [x] Install valid Agent with no deps without sending erroneous empty resolve request.
- [x] Test normal install lock regeneration preserves every APDS origin/metadata field.
- [x] Test duplicate same-named package references and multi-Agent/multi-version coexistence.
- [x] Test legacy artifact install/read still succeeds with accurate inferred metadata.
- [x] Test offline/frozen behavior separately from normal online resolve.
- [x] Add integration tests for repeated clean installs producing same pinned closure.
- [x] Document failures with actionable command/error output (not fabricated package version).
- [x] Address the user-reported Node invocation smoke flake with an explicit test-only startup timeout; full-suite and 24 concurrent repeated runs pass. Production timeout defaults remain unchanged.
- [x] Complete independent Claude review. User confirmed review passed with no findings on 2026-11-20.
- [x] Confirm hosted CLI/native-target CI. User confirmed all hosted CI passed on 2026-11-20.
- [x] Prepare and builder-replay the [M5B manual runbook](manual-tests/CLI_M5B_MANUAL_TESTS.md): visible sources and 51 command expectations under ignored `cli-m5b-test/`, six new declared local publications reusing four immutable M5A fixtures, complete graph replay and isolated faults. Five builder tests pass; all 28 shell blocks parse. Builder verification makes no real API/DB/S3 writes.
- [x] Complete operator manual acceptance using the M5B runbook. User supplied the passing **51-result** audit from `cli-m5b-test/20261121T014721Z-49256/evidence/summary.json`; all 51 expected/actual exits, 12 intended negative messages, 30 byte-comparison proofs, 18 request proofs and 11 current project locks/payloads were independently checked from saved files. No new API/DB calls were made for that inspection. The helper approval flags remain false by design.

**Binding requirements** — authoritative constraints from `spec.md` for this milestone. These are requirements to satisfy and verify, not a second task list; the work itself is in the implementation checklist above.

- Fix `install --frozen` direct Agent and Skill cases that currently assume a Tool; verify exact kind and selected version for roots and dependencies and reconstruct **entire transitive closure** on a fresh workspace.
- Check authored semver ranges vs frozen locked versions and artifact integrity across Registry responses, downloaded tar/sidecars, lock and local installation; mismatch must fail without corrupting working state.
- Treat an Agent Package with no dependencies as a successful no-op dependency resolution/installation, including frozen and nonfrozen modes; do not submit an empty resolver list that fails.
- Regression test normal/frozen reinstall, clean workspace recreation, duplicated/transitive refs, kind-specific roots, legacy packages, mismatched digests, unsupported standard, v4 reader/writer roundtrip and older locks allowed by migration policy.
- Record **D06** decision if any format bump is even considered; must show why v4 cannot safely work and how all consumers migrate.

**Verification:** the standard four evidence requirements apply — see [Per-milestone evidence requirements](#per-milestone-evidence-requirements). Map the AC and decision IDs listed in this milestone's header.

**Exit gate:** The scoped deliverable works against real source contracts; the binding requirements are satisfied, the standard evidence requirements are met, and Claude review has passed.

**Implementation evidence — 2026-11-20:** [M5B verification](m5b-verification.md) maps S2-AC-07 / T-LOCK-02–06 and D06 to complete frozen replay, authored constraint and integrity failures, multi-Agent/version coexistence, older-lock compatibility, empty Agents/workspaces, and unchanged-state checks. The additional user-reported Node invocation flake is addressed with a test-only startup allowance and concurrent repeated runs. CLI 0.1.47 remains prepared and unreleased; there is no new lock format, APDS change, SDK runtime change or production action. Local automated and focused CLI smoke evidence is recorded; User confirmed Claude review passed with no findings on 2026-11-20. User confirmed all hosted CI passed on 2026-11-20. **The operator supplied the passing 51-result manual audit; M5B is complete.** Reader-first publication and SDK CI variable cleanup remain Release Band 2.

## Milestone 6A: Validate Harness Standard Support and Real Preflight Readiness
> **Scope note:** Teach Harness which APDS versions it understands and distinguish conformant definition, compatible Runner, and configured runtime. Outside this milestone: new Runner implementation or change to phase/approval engine design.
> **Implementation notes:**
> - An Agent Package with no Loop is valid APDS but the current Harness requires a Loop to run. Model/provider readiness must not be assumed when no terminal is available.
> - **Dependencies:** M5B and M2B APDS; CLI/Runner machine protocol current source.
> - **Decisions:** D20. **Acceptance:** S2-AC-08.

**Implementation checklist**

- [x] Define supported standards capability list with exact version IDs.
- [x] Check selected Agent and relevant resolved Component standards before run.
- [x] Preserve legacy interpretation as qualified status.
- [x] Return unsupported-standard result for unknown version, without fallback.
- [x] Return valid-but-not-runnable reason for absent Loop.
- [x] Keep readiness errors separate from conformance violations.
- [x] Test missing model/provider in headless preflight returns not-ready.
- [x] Retain interactive bootstrap prompt where a TTY is actually available.
- [x] Expose same supported standards through existing machine initialization/preflight.
- [x] Add clear CLI/TUI status text without claiming one Runner is mandatory for APDS.
- [x] Test unsupported Memory/Knowledge backend as Runner limitation, not APDS invalidity.
- [x] Prove no accidental change in loop stop/transition/approval behavior.

**Binding requirements** — authoritative constraints from `spec.md` for this milestone. These are requirements to satisfy and verify, not a second task list; the work itself is in the implementation checklist above.

- Define Harness supported-standard capabilities (`agentpm` v1.0.0) and integrate into current preflight/machine initialization/CLI/TUI/headless/SDK report surfaces, keeping Harness protocol versions distinct. Preserve existing APIs when possible.
- Use pinned Agent graph and standard provenance to validate selected Agent + relevant Components before execution; error explicitly on unsupported declared standards, structural conformance failures or unresolved composition.
- Display distinct readiness outcome: **APDS conformance** vs **Harness support** vs **local runtime ready**. APDS-valid without Loop is a **not runnable** explanation, not invalid APDS. Don't block external Runner use.
- Ensure headless/noninteractive preflight fails truthfully when required model/provider data is missing; prevent TUI bootstrap assumptions from leaking into machine mode.

**Verification:** the standard four evidence requirements apply — see [Per-milestone evidence requirements](#per-milestone-evidence-requirements). Map the AC and decision IDs listed in this milestone's header.

**Exit gate:** The scoped deliverable works against real source contracts; the binding requirements are satisfied, the standard evidence requirements are met, and Claude review has passed.

**Builder evidence — 2026-11-21:** [M6A verification and D20 interface choice](m6a-verification.md) maps S2-AC-08 / T-RUN-01–04,07 to selected-graph definition/provenance checks, exact supported-standard reporting, qualified legacy interpretation, no-Loop compatibility, noninteractive model/provider failures, optional backend limitations and unchanged execution regressions. Final workspace: **1,353 passed, 0 failed, 6 opt-in ignores**; all **10 native environment tests** also pass, as do Clippy, formatting, docs and APDS/SDK drift guards. The actual init/lint/empty-install/preflight smoke passes with unchanged source/lock bytes. Existing protocol version 1 is retained; new fields are additive. **User confirmed independent Claude review passed with no remaining findings on 2026-11-21, including the scoped D20 interface choice. User confirmed all hosted CI passed on 2026-11-21. The [manual runbook](manual-tests/CLI_M6A_MANUAL_TESTS.md) is prepared with 21 visible local cases, 26 command expectations and four TUI checks under ignored `cli-m6a-test/`. Four builder checks pass, including actual shell-block and PTY replay; all 23 runbook shell blocks parse. User supplied the passing 26-result audit with all four TUI observations on 2026-11-21. Saved diagnostics, reports, model traffic and source hashes were independently rechecked; the disclosed 13-result builder recovery remains recorded in verification. M6A is complete.** M6B exact runtime binding fixes, SDK wrapper parity and D13 remain untouched.

**Review correction:** legacy APDS diagnostics now exempt only the missing declaration; other structural/resolved findings are visible warnings with rule IDs and source paths. Legacy status and source bytes stay unchanged. Root/Component description and resolved-phase regression cases pass; the full workspace and checks above were rerun. Selector diagnostic messages also share the same relative source path as their structured field, with passing root/Component assertions. User confirmed independent re-review passed with no further findings on 2026-11-21.

## Milestone 6B: Fix Exact-version Harness Bindings and SDK Visibility
> **Scope note:** Resolve actual pinned Components in Harness and make standard support visible consistently to headless, TUI and SDK clients. Outside this milestone: rewrite of Node/Python SDK package models or new machine protocol.
> **Implementation notes:**
> - Versionless authored binding IDs must resolve **via the selected Agent graph**. Name-only workspace search can select a different compatible-looking version and violates reproducibility.
> - **Dependencies:** M6A; M5A/B locked identity; Stage 1 runtime changes.
> - **Decisions:** D13, D20. **Acceptance:** S2-AC-08, S2-AC-09.

**Implementation checklist**

- [x] Trace current Harness Component lookup that only uses a name.
- [x] Use selected Agent's resolved graph/lock to determine exact Component version and kind.
- [x] Test workspace with two Agents binding the same named Tool at different versions.
- [x] Verify Skill-inherited Tools are exposed only in valid global/phase scope and Loop policy.
- [x] Verify Memory operation target semantics remain as in Phase 7A, including operation access to declared spaces not directly phase-bound and scope/trigger eligibility remaining distinct from actual invocation.
- [x] Verify the reviewed M1C/Phase 7B Memory authority boundary: Loop read/write flags restrict direct Agent access, not separately bound lifecycle operations. Check that eligible authorized operations can use declared unbound targets while those spaces remain unavailable for direct Agent access; unbound/out-of-phase/ineligible operations remain restricted. Preserve edge-trigger/rearm and interval baseline/reset meanings without prescribing a scheduler. See [M1C-R06](m1c-reconciliation.md#reconciliation-outcomes).
- [x] Test supported Runner interpretation separately from schema validation: global/phase additive Profiles without prescribed prompt ordering; no automatic Skill script execution; Loop implicit `complete` transition and authored access/checkpoint/error-policy meaning, within features Harness actually supports.
- [x] Confirm Knowledge backend compatibility and local readiness failures are not reported as APDS invalidity.
- [x] Expose standard support list through Node SDK bridge/host APIs.
- [x] Expose equivalent support list through Python SDK bridge/host APIs.
- [x] Confirm TUI, headless and machine preflight outputs agree semantically.
- [x] Set up A/B test fixture for optional Agent description in prompt context.
- [x] Compare outputs, duplication, token footprint and traceability; record D13 decision.
- [x] Keep existing prompt behavior if experiment doesn't show material benefit.
- [x] Record D20 interface choice and Claude signoff on exact-version resolution. User confirmed full Claude review passed with no remaining findings on 2026-11-21, after the candidate-suppression assertion correction.

**Binding requirements** — authoritative constraints from `spec.md` for this milestone. These are requirements to satisfy and verify, not a second task list; the work itself is in the implementation checklist above.

- Fix binding-to-installed-Component resolution where name-only lookup could select the wrong version in multi-Agent/multi-version workspaces. Use exact lock identity/version and test phase/global plus Skill-inherited Tool scopes and Loop access restrictions.
- Validate TUI, headless, machine initialization/controls, and Node/Python SDK parity for compatibility/status output, including back-compat for consumers of existing payload fields.
- **D13 experiment:** select a small repeatable loop-based Agent; compare prompts/outputs with and without injecting top-level Agent `description`; measure fidelity, duplication, token/cost effects, and phase output quality; record keep/defer/reject and tests for chosen behavior. Do **not** amend APDS semantics based solely on preference.
- **D20 decision:** describe why the selected capabilities advertisement belongs in existing Harness interfaces rather than a new standard protocol. Require Claude to review no Loop/provider model errors and version correctness.

**Verification:** the standard four evidence requirements apply — see [Per-milestone evidence requirements](#per-milestone-evidence-requirements). Map the AC and decision IDs listed in this milestone's header.

**Exit gate:** The scoped deliverable works against real source contracts; the binding requirements are satisfied, the standard evidence requirements are met, and Claude review has passed.

**Builder evidence — 2026-11-21:** [M6B verification](m6b-verification.md) records exact selected-graph runtime construction, all-six-kind multi-version snapshot tests, actual Node/Python-hosted Tool execution, global/phase/Skill/Memory/Profile/Loop interpretation, and real CLI/SDK report parity. Existing protocol version 1 and response envelopes are preserved. [D13 comparison](m6b-description-decision.md) selects **defer injection and retain current prompts**: reproducible one-/three-phase runs expose added prompt duplication without claiming a real-model quality benefit. Final local Rust workspace: **1,359 passed, 0 failed, 6 opt-in ignores**; native environment regressions: **10 passed**. Node: **119 passed, 2 optional skips**; Python: **120 passed, 2 optional skips**, with all twelve new real-CLI/transport cases passing in each SDK. Formatting, Clippy, SDK type/lint/build checks, feature-local docs and APDS/shared-fixture guards pass. CLI 0.1.47 remains prepared/unreleased; SDK APIs are recorded as Unreleased, without publication. **User confirmed independent Claude review passed with no remaining findings on 2026-11-21, including D13/D20 and the guard assertions. User confirmed all hosted CI passed on 2026-11-21 after the Windows corrections and macOS dependency-download rerun. User supplied the passing 30-result audit with both TUI observations on 2026-11-21; saved evidence and pinned hashes were independently rechecked. M6B is complete.** The selected graph's simultaneous active-name multi-version limitation is explicit and does not label the artifact APDS-invalid.

**Review correction — 2026-11-21:** directly assert that ambiguous Tool and Memory candidates remain suppressed, independently of the fatal ambiguity diagnostic. Reverting lookup to arbitrary first-match fails the new assertion; restoring the uniqueness check passes both focused exact-version tests. Production behavior is unchanged. See the [verification record](m6b-verification.md); user subsequently confirmed independent review passed with no remaining findings on 2026-11-21. The user reports a future ambiguity solution is being planned in Stage 3; M6B retains the documented limitation and explicit safeguard.

**SDK CI triage — 2026-11-21:** both failed SDK jobs use old CLI pin `25bd743a86099cee318d4e7d39490c9ea66ac23b`; M6B adds the shared fixture at reviewed CLI commit `1134b3c0aa0de5c978356ff6b271d9aad257d4fe`, and both committed SDK snapshots match it. Update both SDK repositories’ temporary `AGENTPM_CLI_REF` variables (or explicit dispatch `cli_ref`) to that reviewed commit, rerun and verify the checkout before recording CI success. CLI `main` does not yet contain the fixture. [Evidence](m6b-verification.md#remaining-gates-and-release-order). User subsequently requested resuming manual runbook preparation; no CI pass is claimed.

**Manual runbook prepared — 2026-11-21:** [M6B operator guide](manual-tests/CLI_M6B_MANUAL_TESTS.md) provides ten visible local cases, 30 captured expectations and two actual TUI Run observations under ignored `cli-m6b-test/`. Four builder checks pass, including actual guide shell replay, both real PTY Runs and the complete audit; all 13 shell blocks parse. Covers exact six-kind versions through headless/SDK execution, phase/Skill scope, qualified ambiguity and the Skill-only workaround, five CLI/SDK report comparisons and the reproducible D13 experiment. No Registry or paid service is used. Existing operator runs are untouched; builder cleanup is limited to its own UUID directory. [Verification](m6b-verification.md#manual-runbook-preparation--2026-11-21) records evidence and limits. Hosted CI and operator manual acceptance are now confirmed passed; M6B is complete.

**Windows SDK CI correction — 2026-11-21:** shared-fixture drift is resolved, but subsequent Windows jobs exposed literal `node` lookup versus `node.exe` in CLI preflight and Node test cleanup racing subprocess working-directory release. CLI readiness now recognizes extensionless Windows executable paths, with a platform-aware PATH/absolute-path regression; Node cleanup awaits bounded asynchronous native retries. Both SDK execution tests expose preflight diagnostics before running. Latest local validation: 27 Harness-plan tests, two exact-version tests, Node 119 passed / two optional skips, Python 120 passed / two optional skips; formatting, Clippy, SDK checks and fixture guards pass. [Verification](m6b-verification.md#windows-sdk-ci-correction--2026-11-21). Push the CLI correction and update both temporary SDK CLI refs to that new commit before rerunning; the earlier `1134b3c` pin lacks this fix. Windows hosted CI and operator acceptance remain pending; no release is claimed.

**Hosted CI confirmation — 2026-11-21:** user confirmed all CI passed after the Windows corrections in CLI `b7a762be2f6129a9b98ef8fc3751577a1a1659cb`. The macOS Python 3.11 native-test failure was a PyPI connection timeout; its rerun passed without another code change. This closes the hosted CI gate. User subsequently supplied the passing [30-result/two-TUI manual audit](manual-tests/CLI_M6B_MANUAL_TESTS.md); all saved checks, both fresh ended TUI Runs and pinned hashes were independently rechecked without changing operator files. **M6B is complete.** Release Band 2 rollout remains separate.

**Manual acceptance — 2026-11-21:** operator run `cli-m6b-test/20261121T204948Z-11496` passes all 30 command checks and both TUI observations, with CLI SHA-256 `5db89f2870604489de3122050d83c748eb3dcf8b9d8560d708f83dd2dd978f72`. Rechecked expected diagnostics/statuses, six-kind runtime versions, Tool markers, SDK envelopes, description experiment, ended Run reports and source/CLI/SDK hashes from saved evidence. No new Runs, Registry requests or evidence edits were made. Summary approval fields intentionally remain false; operator/CI acceptance is recorded here. Independent review, hosted CI and manual acceptance are passed; **M6B is complete**. Release Band 2 remains the next separate rollout gate.

## Release Band 2: Pinned Install and Harness Runner Compatibility
Covered milestones: 5A–6B.

**D06 ordering:** publish CLI 0.1.47 (or a later release containing its APDS-aware v4 reader/writer), confirm installability through supported distribution channels, and upgrade every developer/CI/container CLI before examples or automation distribute extended locks. SDK 0.1.17 already tolerates these additive fields; no SDK release is required solely for lock reading. Registry response additions can deploy before the CLI: older DTO readers ignore them, while the new CLI also reconciles older responses against artifact bytes.

This gives users exact APDS metadata through resolve/install/lock v4, reproducible direct/transitive frozen installs, no-dependency Agent install behavior, and Harness capability/preflight reporting that differentiates a conformant Agent Package from a supported Runner and a ready local environment. It retains the existing Harness engine rather than introducing a new framework.

**Version preparation — 2026-11-21 (before publication; completed below):** CLI Cargo metadata and lock already agree on **0.1.47**. Both public SDKs are now prepared as **0.1.18**, with matching versioned Unreleased changelog sections; Python `uv.lock` carries the same project version without changing the dependency snapshot. Version consistency, Cargo locked metadata, Python locked-state check, shared fixture guards, Node build/package-content checks and both SDK test suites pass locally (Node 119 / Python 120, each with two existing optional skips). No publication, tag, merge, deployment or CI-ref change was made. User will push these changes and rerun hosted CI, then merge CLI to `main` before restoring both SDK workflows to CLI `main`. This preparation does not close the publication or release-band gates below.

**Web version preparation — 2026-11-21 (before deployment; completed below):** `agentpm-web/src/app/status/page.tsx` now carries CLI **0.1.47** and both SDKs **0.1.18**, dated **2026-11-21**, ready for deployment once those releases are available. TypeScript, targeted ESLint, formatting and diff checks pass. The web change is prepared locally; publication and deployment remain pending.

- [x] After the matching CLI changes merge into `main`, remove the temporary `AGENTPM_CLI_REF` repository variable in both `agentpm-dev/sdk-node` and `agentpm-dev/sdk-python`, or reset it to `main`. Run both SDK CI workflows against CLI `main` and confirm they pass; for manual dispatch, use `cli_ref=main` rather than the temporary commit pin.
- [x] Prepare and publish new Node/Python SDK versions for the added support-list API/types after the compatible CLI is released. Preserve existing raw payload access; SDK 0.1.17 remains compatible for extended-v4 lock reading. Run SDK CI against the matching CLI-owned fixture revision before publication.
- [x] Reproduce frozen multi-kind dependency closure from clean workspace; test wrong digest/range/kind and legacy versions.
- [x] Verify minimal Agent is conformant but not Harness runnable without Loop.
- [x] Verify supported versions through TUI/headless/machine/SDK and missing-model headless failure.
- [x] Verify multi-version workspaces use exactly locked Component identity and Skill/Memory/Loop semantics.

**Completion — 2026-11-21:** user confirmed all hosted CI passed, both SDK CLI-ref overrides were removed, CLI 0.1.47 was released and verified, and Registry/SDK/web deployments completed. [Independent release verification](release-band2-verification.md) confirms Homebrew/Scoop manifests and release hashes, npm `latest` / PyPI 0.1.18 artifacts and their hashes, CLI-before-SDK publication ordering, live web/health and declared/legacy Registry metadata. The released CLI passes a signed/attested eight-package install, frozen replay and clean reconstruction with identical lock bytes and 40 portable payload files; wrong kind/digest/range faults are rejected without replacing valid state. Fresh published SDKs pass four exact-version Runs, five report-parity cases and two additional released headless Runs. M6B's accepted TUI/scope/Memory evidence and user-confirmed hosted regressions complete the cross-surface coverage. Raw evidence: ignored `release-band2-evidence/20261122T020408Z-a5f10895/summary.json`. Private CI-ref settings and Render configuration were not independently read. No package versions or paid model calls were created during verification. Changelogs now record the release date. **Release Band 2 is complete.**

---

## Milestone 7A: Inventory Universal Package Health Evidence
> **Scope note:** Define which release facts are actually verifiable and what each can legitimately claim. Outside this milestone: quality score, reviews, ranking changes or retrospective fabricated proof.
> **Implementation notes:**
> - Evidence can refer to package identity, selected version, release artifact, or local Runner state. The Health API/public page must never confuse those scopes.
> - **Dependencies:** APDS status from M2B/M2C and M4A/M4B (this stage); Stage 1 M12–M16 provenance and portability evidence. **Specifically Stage 1 M14C** (root-signed registry key set, attestation-key backfill, and client-side verification of release digest / author signature / registry attestation, Stage 1 Release Band 11) is what makes a *verified* attestation signal possible at all. Until M14C is deployed, the `registry_attestation` signal must read `unknown`/`not evaluated` rather than `verified` — Stage 2 Band 3 may legitimately ship ahead of Stage 1 Band 11 with that honest state, but must not imply verification it cannot perform.
> - **Decisions:** D12, D19. **Acceptance:** S2-AC-10.

**Implementation checklist**

- [x] List all proposed universal signals with exact originating DB fields/services.
- [x] Classify each as identity, release, artifact or local-environment scoped.
- [x] Define declared standard separate from independently verified APDS conformance result.
- [x] Define explicit APDS versus legacy inferred origin in Health terms.
- [x] Define verified signature versus merely present author signature.
- [x] Define Registry attestation versus unsupported/unverified attestation.
- [x] Define malware scan status and unavailable/unknown scan distinction.
- [x] Define logical release/artifact digest and integrity evidence from Stage 1.
- [x] Define maintenance recency and deprecation only when actual data exists.
- [x] Separate star/installs as identity-level popularity, not Health.
- [x] Specify status vocabulary and evidence freshness/report link semantics.
- [x] Write a signal-to-source matrix as a review artifact before building UI.
- [x] Have Claude challenge every trust-related label for overstated claims.

**Binding requirements** — authoritative constraints from `spec.md` for this milestone. These are requirements to satisfy and verify, not a second task list; the work itself is in the implementation checklist above.

- Inventory authoritative source for every proposed Health signal: APDS declaration/conformance level, release/artifact digests, author signer verification, Registry attestation, scan, license, release date, dependency integrity/resolution, deprecation, Tool targets/platform/runtime/compatibility. Mark unavailable data plainly.
- Define evidence origin and freshness/verification semantics for each field; signatures can exist without independent verification; clean malware scan ≠ safe; declared platform compatibility ≠ local enforcement; APDS inference ≠ verified conformance.
- Separate identity-level stars/installs/publisher from selected-version integrity/conformance/release metadata; preserve Stage 1 metrics calculations rather than duplicating scores.

**Builder evidence — 2026-11-22:** [Source inventory and D12/D19 decisions](m7a-health-evidence-inventory.md), [verification and remaining gates](m7a-verification.md). Registry checks: 169 passed plus 11 subtests, including 22 new source probes; Rust provenance: 6 passed, 1 opt-in proof test ignored; existing eight-kind Security UI: 18 passed. APDS bundle parity and touched-file format/static checks pass. No runtime/API/UI/schema/migration changes or production requests. The audit distinguishes stored proof presence from independent verification and records missing freshness/deprecation/report data for M7B. S2-AC-10 is covered only for this evidence-design slice; public aggregation/rendering remains later work. User confirmed independent Claude review passed with no open findings on 2026-11-22, completing the trust-label review item. User confirmed hosted CI passed on 2026-11-22. **M7A is complete.** M7B and its dated operator renewal task remain pending.

**Verification:** the standard four evidence requirements apply — see [Per-milestone evidence requirements](#per-milestone-evidence-requirements). Map the AC and decision IDs listed in this milestone's header.

**Exit gate:** The scoped deliverable works against real source contracts; the binding requirements are satisfied, the standard evidence requirements are met, and Claude review has passed.

## Milestone 7B: Implement Kind-specific Health Extensions and Registry Responses
> **Scope note:** Expose a coherent Health evidence envelope with optional kind-specific facts, only where data is supported. Outside this milestone: OS compatibility badge on Profile/Loop/Template unless meaningful and sourced.
> **Implementation notes:**
> - Prefer one shared evidence representation with per-kind providers/adapters, not eight diverging APIs or eight copies of security math.
> - Use the [M7A source inventory](m7a-health-evidence-inventory.md) for authoritative fields, allowed claims and missing-data boundaries. Expose existing evidence and record actual future checks; do not add new vulnerability scanners, formal deprecation infrastructure or retrospective certification to fill absent signals.
> - **Dated operator dependency:** the retained production key-set evidence expires `2026-12-07T22:31:48Z`. Reconfirm the served set and renew it before expiry even if M7B is not ready. This is signed key-set renewal under the existing Stage 1 custody procedure; it does not require rotating the root or attestation signing keys.
> - **Dependencies:** M7A; Stage 1 final release artifact model.
> - **Decisions:** D12, D17, D19. **Acceptance:** S2-AC-10.

**Implementation checklist**

- [x] Implement universal signal aggregation in Registry with version scope.
- [x] Operator (Zack): renew and deploy the production root-signed key set before the recorded `2026-12-07T22:31:48Z` expiry, following Stage 1 M14C's existing custody/renewal procedure. Retain historical attestation verification keys; record the served replacement's validity period, root/origin verification and a strict attestation-install smoke check. Do not wait for M7B completion to meet this deadline.
- [x] Reconcile APDS identity/origin with the selected manifest; keep declaration, inferred compatibility and actual validation levels/results distinct, including incomplete or unsupported evidence.
- [x] Separate signature/attestation presence from verification results; reuse Stage 1 verifiers for any verified claim and include its observer, named check and subject/trust context, otherwise report not evaluated/unknown. Distinguish Registry-reported checks from actual local verification; local verification instructions are not completed results.
- [x] Use full authoritative legacy artifact digests and the selected release's publication date; never substitute display hashes or the latest release's date.
- [x] Expose existing per-artifact scan start/finish timestamps and retain progress, result and target coverage separately; a completed clean release summary requires every expected artifact to be covered and complete.
- [x] Record actual evaluation timestamps for future Registry APDS validations; keep older untimed results unknown, retain existing scan dates where available, and distinguish evaluation/verification, retrieval and publication times. Do not fabricate timestamp backfills or rewrite historical artifacts/reports.
- [x] Preserve unknown/unavailable for missing deprecation, vulnerability/SBOM, scanner-version or report-link evidence; never infer a positive result from absent data, optional columns or an old publication date.
- [x] Make cache behavior preserve concrete selected-version identity and source/check timestamps; a fresh HTTP retrieval must not imply fresh validation, scan or cryptographic verification.
- [x] Add Tool target, runtime, dependency portability evidence only from real Stage 1 outputs.
- [x] Add Agent graph/Runner support evidence without pretending server knows local readiness.
- [x] Add Skill declared Tool/reference/script facts with no auto-execute claim.
- [x] Add Knowledge mode/corpus/index evidence where metadata/hash is trustworthy.
- [x] Add Memory spaces/record type/operation/trigger facts without backend-execution claim.
- [x] Add Profile guidance/hints and Loop graph validity as appropriate advisory facts.
- [x] Add Template scaffold stack/execution surface compatibility only when declared.
- [x] Support `not applicable` and absent evidence explicitly, not green success defaults.
- [x] Prevent leakage of private dependency metadata through public Health aggregation.
- [x] Test one recent and one legacy release for every kind.
- [x] Test forged report/status fields cannot override authoritative source.
- [x] Test version switching changes release Health but not identity-level star state.
- [x] Test existing scan dates, older APDS reports without dates, newly timed validations, partial/running scan coverage and cached evidence; verify honest unknown states and unchanged historical evidence/artifact bytes.
- [x] Test that missing observer/trust context cannot yield verified status and that Registry-reported verification cannot be presented as a completed local check.
- [x] Add a clock-controlled expired-key-set Health fixture, including the expiry boundary and a previously cached verified result: current attestation verification degrades to unknown/not evaluated with an expired-trust reason, never fresh verified or an invalid-package verdict. Confirm renewed trusted material permits a new attributed verification result, unrelated evidence is unchanged, and existing strict CLI verification remains fail-closed.
- [x] Record D12/D17 representation decision and review evidence language.

**Binding requirements** — authoritative constraints from `spec.md` for this milestone. These are requirements to satisfy and verify, not a second task list; the work itself is in the implementation checklist above.

- Propose **universal per-version evidence envelope** plus **kind-specific extensions/adapters**; define status vocabulary verified/passing, failed, unavailable/unknown, incomplete, advisory, not-applicable. Avoid implying all kinds have Tool-style OS support.
- Build or extend Registry DTO(s)/server aggregation using existing services. Avoid inventing per-kind database tables or duplicating Stage 1 integrity computation without justification. Handle old releases, incomplete evidence and offline/failed retrieval safely.
- Test representative release of **each of eight kinds** (including Template): universal signal applicability, kind extensions, nonexistent data, ambiguous verification, adversarial forged signals, version switch, public/private visibility, no false safety claims.
- Record D12/D17/D19 selected shape and Claude review of evidence/wording before UI badges ship.

**Builder evidence — 2026-11-22:** [M7B implementation, D12/D17/D19 decisions and verification](m7b-verification.md). Added the shared selected-version Health endpoint, attributed Registry checks, per-kind facts, safe dependency projections and future APDS evaluation timestamps; existing Security transport and immutable artifacts are preserved. Final local Registry suite: 1,200 passed plus 350 subtests, 11 documented environment-dependent skips; Rust provenance: 6 passed, 1 existing opt-in proof test ignored. Python format/static hooks, docs checks and APDS bundle parity pass. User confirmed independent Claude review passed with no findings on 2026-11-22. User confirmed hosted CI passed on 2026-11-22. The operator signed a renewed public key set expiring `2027-02-20T19:41:17Z`; its signature/inventory were verified locally against the pinned production root. User confirmed configuration-only deployment with no new code pushed. A read-only public GET independently verified the exact served renewal against the pinned root, with [retained evidence](m7b-keyset-renewal.json); the user subsequently supplied a successful strict signature/attestation install result, completing the renewal task. **M7B is complete.** Implementation tests used isolated fixtures; no UI work or key rotation occurred. M7B code deployment remains a release-band action.

**Verification:** the standard four evidence requirements apply — see [Per-milestone evidence requirements](#per-milestone-evidence-requirements). Map the AC and decision IDs listed in this milestone's header.

**Exit gate:** The scoped deliverable works against real source contracts; the binding requirements are satisfied, the standard evidence requirements are met, and Claude review has passed.

## Milestone 8A: Publish the Versioned APDS Website Reference
> **Scope note:** Give developers a stable explanation and exact authoritative source for APDS 1.0.0, linked from package pages. Outside this milestone: user-supplied runtime schema fetching and confusing it with a general category landing page.
> **Implementation notes:**
> - Preferred route `/standards/agentpm/1.0.0`; immutable schema and semantics are bundled/build-pinned. Deep links must lead to the exact version.
> - **Dependencies:** M1B/C frozen contract; Stage 1 M7 technical SEO foundation when available. **Global shell:** this route is built before M8B.1 establishes the site-wide shell — either sequence it after M8B.1, or record explicitly that it ships on the current shell and schedule its migration in M8B.1's adoption order. Do not hand-roll a one-off layout for it.
> - **Decisions:** D07. **Acceptance:** S2-AC-12, S2-AC-17.

**Implementation checklist**

- [x] Choose trusted version catalog representation and deployment integration.
- [x] Publish human-readable standard overview for `agentpm / 1.0.0`.
- [x] Present the short README entry point and readable section/rule navigation before deep technical references; render the frozen normative text faithfully without creating a conflicting or more verbose second authority.
- [x] Link to exact JSON Schema (canonical `$id`) and normative `semantics.md`.
- [x] Link to fixture/conformance instructions and authoritative GitHub source.
- [x] Make raw manifest `View agent.json` links separately point to selected artifact version.
- [x] Make APDS declaration distinct from verification status in UI label.
- [x] Use inferred legacy label where appropriate, not verified conformance.
- [x] Test unknown standard version route yields honest not-supported response.
- [x] Ensure no untrusted package URL is fetched for standards content.
- [x] Coordinate canonical/robots/sitemap decisions with Stage 1 SEO.
- [x] Test stable links, keyboard navigation, accessible code/link typography.
- [x] Record D07 trusted-source/build-versus-runtime choice.

**Builder evidence — 2026-11-25:** [M8A implementation and D07 verification](m8a-verification.md). The web deployment bundles the published README, semantics and schema, checks their frozen hashes before every build, renders the reference at `/standards/agentpm/1.0.0`, and adds separate selected-version manifest/APDS links across all eight kinds. Security and Health share one APDS identity/report validator and safe projection; the web displays only the new additive `apds_evidence` field, never raw JSONB, separately from declarations and legacy inference. Review regressions cover contradictory reports, older/cached API responses without the projection, and sparse/extended projected level maps without a fixed web-side level set. The route deliberately uses the current global shell and existing Panel/prose primitives; its adoption into the new shell is assigned to M8B.1 below. Feature-local MDX explains the links and evidence boundaries. User confirmed independent Claude review is complete with no new findings on **2026-11-25**. Hosted CI, manual browser review and production deployment remain pending; **M8A is not yet marked complete**. Broader S2-AC-17 category/pricing/layout work remains with its assigned later milestones.

**Binding requirements** — authoritative constraints from `spec.md` for this milestone. These are requirements to satisfy and verify, not a second task list; the work itself is in the implementation checklist above.

- Add versioned standard reference route, prefer `/standards/agentpm/1.0.0`: human-readable intro, normative `semantics.md` and schema links, stable canonical source and version, supporting fixture docs. Use **trusted pinned** content; no arbitrary standard URL fetch from authors or moving `main` for immutable normative bytes.
- Expose separate links/components for declared standard vs conformance evidence, actual version-specific raw `agent.json`, and verified/legacy inference states. Ensure links work with real current detail routes and new/old releases.

**Verification:** the standard four evidence requirements apply — see [Per-milestone evidence requirements](#per-milestone-evidence-requirements). Map the AC and decision IDs listed in this milestone's header.

**Exit gate:** The scoped deliverable works against real source contracts; the binding requirements are satisfied, the standard evidence requirements are met, and Claude review has passed.

## Milestone 8B: Create Reusable Component, Composition and Package Card UI Primitives
> **Scope note:** Build shared website building blocks for real Agent Package composition, kind labels, standard status and Package Health without rebranding the site. Outside this milestone: Do not copy mockup metrics, invented `apds` manifest fields or fake integrations.
> **Implementation notes:**
> - Reuse current AgentPM theme, gradients, elevation, rounded floating surfaces and kind icons. The first mockups teach hierarchy; second mockups demonstrate depth; production design tokens remain authoritative.
> - **Dependencies:** M7B Health, Stage 1 M6 shell, M8A standard references.
> - **Decisions:** D09. **Acceptance:** S2-AC-11, S2-AC-12, S2-AC-16.

**Implementation checklist**

- [ ] Inventory existing design tokens, reusable cards and Stage 1 shared detail shell.
- [ ] Document colors/elevation/typography to preserve in before/after reference captures.
- [ ] Decide what the Agent Package Card must convey when it appears **unaccompanied** off-site — in a README, a social preview, a pasted link. That standalone case sets the content floor for the full-fidelity variant, including the homepage hero placement; it is not a hero-balance question. Record the decision, since M11B's embed investigation and M12A's hero both depend on it.
- [ ] Build a compact Agent Package identity/summary card from real DTOs.
- [ ] Build reusable kind icon and category label primitives for six Components/Template.
- [ ] Build exact-version link and direct/transitive dependency chip primitives.
- [ ] Build truth-state badges that distinguish declaration, verification, inference and unknown.
- [ ] Build a graph-to-UI mapping for resolved Agent composition without relying on raw JSON on screen.
- [ ] Build Loop phase/transition/capability model that handles branches and cycles.
- [ ] Provide keyboard/screen-reader text fallback for large or interactive graphs.
- [ ] Support empty deps, missing resolved context, private dependencies and unsupported Loop.
- [ ] Test cards with long names, narrow viewport, theme support if existing site has it.
- [ ] Keep presentation architecture flexible across detail, featured, Explore and OG usage.
- [ ] Record D09 variant approach and why it fits existing React/Next patterns.

**Binding requirements** — authoritative constraints from `spec.md` for this milestone. These are requirements to satisfy and verify, not a second task list; the work itself is in the implementation checklist above.

- Audit current website tokens/layout components and Stage 1 detail shell/OG implementation; document which existing cards, icons, layered shadows, typography and gradients must be preserved. Shared primitives should make pages clearer, not flatten current look.
- Build reusable kind icon/label, version/tag/source, truth-state, concise composition-chip and responsive Agent Package identity/card primitives. Avoid coding mockup placeholder values into production.
- Implement a reusable graph-to-view model for direct/transitive resolved Components and Loop phase/scoped binding summaries, with honest missing/invalid/nonlinear cases and accessible text fallback. Prove readability in light/dark/mobile if current site supports both.
- Include links to standard/raw manifest, Health, and kind-specific detail surfaces in component design examples. Record D07/D09 implementation decisions and screenshot deviations from mockup with rationale.

**Verification:** the standard four evidence requirements apply — see [Per-milestone evidence requirements](#per-milestone-evidence-requirements). Map the AC and decision IDs listed in this milestone's header.

**Exit gate:** The scoped deliverable works against real source contracts; the binding requirements are satisfied, the standard evidence requirements are met, and Claude review has passed.

## Milestone 8B.1: Establish the Global Layout and Shell System
> **Scope note:** Build the site-wide shell and shared layout system the `-layered` mockups establish — header, page canvas, section-card primitive, kind tokens, grids, footer — **once**, so every page-level milestone adopts it instead of re-deriving it. Outside this milestone: redesigning any individual page's content, changing brand tokens/palette, writing final category copy, or adopting mockup data.
> **Implementation notes:**
> - **Why this is its own milestone.** The three `-layered` images are one site-wide redesign sharing a global shell, not three page comps. Several elements (header, search placement, page canvas, section card, kind tokens, footer) appear on every route, so the first page to adopt them commits the rest. Building the shell once here avoids two competing shells and a reconciliation pass later. See `spec.md` §9.1 for the element inventory.
> - **Already settled, excluded from D21:** per-kind color (the shipped `Badge.tsx` tone assignment) and the generated per-package identity gradient (`ToolBox.tsx`) are preserved as-is. The mockups depict different kind colors and replace the generated avatar with stock glyphs; both depictions are rejected by default rather than reviewed. A per-kind glyph set **may** still be introduced for genuine category contexts such as the Explore facet rail — see the identity-vs-category test in `spec.md` §9.1 — but never as a replacement for identity marks. The work here is centralizing what already exists, not choosing new values. See `spec.md` §9.1.
> - **The mockups are a guide, weighted toward layout — not a design to reproduce 1:1.** Their strongest contribution is structure, layout, hierarchy, component inventory and interaction model, but they also contain styling ideas worth taking selectively. Nothing is adopted by default simply because it appears in an image, and nothing is rejected by default either. **D21** resolves this per element before the build begins.
> - **The mockups are a light-theme redesign of an already-dark site; the theme is not up for adoption.** Production is dark today — `src/app/layout.tsx` renders `min-h-screen bg-slate-950 text-slate-100`, the header is `sticky top-0 z-40 backdrop-blur bg-slate-950/70 border-b border-slate-800`, and surfaces are translucent darks (`bg-slate-950/60`, `bg-slate-900/60`, `bg-slate-800/50`). The mockups depict floating **white** cards on a tinted light canvas. Adopt their *structure* — tinted canvas, elevated surfaces floating above it — expressed in the existing dark palette; never their light values. Operator-confirmed (2026-11-22): the current color scheme is kept. This also means the mockups' soft grey drop shadows are not the elevation mechanism here: on `slate-950` they barely read, and `backdrop-blur` plus border contrast is the dark-native equivalent. Treat any mockup treatment that depends on a light ground as `reject` without spending review time on it.
> - Always excluded regardless of the review: **every number, name, date, count and badge** (must come from real data). The production site is the default for brand identity; a mockup styling treatment is adopted only where the review deliberately chooses it.
> - **Copy is no longer uniformly excluded (amended 2026-11-25).** The bounded list of wording approved on the 2026-11-25 design references — landing hook, thesis heading, core-four Standard/Registry/Manager/Runner vocabulary, the three onboarding path labels and commands, Explore contextual filter copy and group-shortcut labels, and the Agent Package / Component / Template terminology — is an agreed positioning direction that M14A/M14B should not redo. Everything outside that list stays open to M14 as before. Approval covers wording only: factual, legal or behavioral inaccuracies are still corrected, and it does **not** extend to type size (see the prose floor).
> - **This is a change to an existing site, not a rewrite. The existing code is the baseline; the mocks are the reference.** A second, newer set of references now exists alongside the `-layered` PNGs: the self-contained HTML mocks in `assets/mocks/` (`landing-page.html`, `detail-page.html`, `detail-page-tool.html`, `composition-density.html`, `health-security.html`, described in that directory's `README.md`). They are closer to the intended result than the PNGs — already dark, already on production tokens, already carrying the M7A/M6A trust-claim corrections — and the same D21 rules apply to them: reference weighted toward structure, layout, hierarchy and interaction model, with styling adopted selectively rather than wholesale. What is **new** here is the direction of default: start from what `agentpm-web` already has and change it, rather than building to the mock and back-filling. Concretely, these are preserved unless D21 deliberately decides otherwise, with the existing value winning any unexplained difference: **shared components** (`Panel`, `Badge.tsx`, the `ToolBox.tsx` mark components, the MDX/prose renderers, existing layout wrappers), the **spacing and gap scale**, the **breakpoint set** and where each layout actually switches, **container max-widths**, and the established **Tailwind utility conventions**. `agentpm-web/CLAUDE.md` already treats changes that bypass the current utility/component approach as a review finding; this note is that rule applied to mock-derived work. Equally, a mock value is not forbidden simply for being new — reference it freely where D21 chose it, and say so.
> - **Type scale is the named exception to "existing wins": the direction is to enlarge, and shrinking back needs the argument.** Operator-confirmed (2026-11-22): the production site's text is harder to read than the mocks', and the mocks are comfortable throughout. The measured difference is **not** overall scale — the mocks actually use *more* small type than production (184 `text-[11px]` and 32 `text-[10px]` uses across the five files, where production floors at 12px). The difference is **what each size is allowed to carry**. In `agentpm-web/src`, `<p>` elements break down as **42 at `text-xs` (12px), 37 at `text-sm` (14px), and only 4 at `text-base` or larger** — so more production paragraphs are set at 12px than at 14px, and 12px is doing real prose work (helper text, release descriptions, explanations under fields). The mocks never set a paragraph below 13px, sit mostly at 14–15px, and reserve ≤11px strictly for micro-labels that are scanned rather than read (uppercase eyebrows, kind chips, version strings, counts). Adopt that discipline as the rule: **nothing that carries a sentence goes below 13px; 10–11px is for labels only; monospace/code blocks are exempt.** The test is mechanical rather than a matter of taste: an element with **five or more words and terminal punctuation** is prose and takes the floor; a shorter fragment without terminal punctuation is a label and may go smaller; a `<pre>`/`<code>` block is exempt at any size, because code is scanned by shape, not read as prose. Applied to the 2026-11-25 design references this classifies 20 small-type elements as 15 prose (raise), 3 labels (leave) and 1 code block (exempt) — the code exemption was the clause missing from the original rule, and its absence is what made that audit look worse than it was. This is a readability floor, not a styling preference, so it is the default rather than something to be argued for — the preceding note's "existing value wins" does not apply to type size. Weight and leading still go through D21 normally.
> - **The HTML mocks' scaffolding is not an implementation target.** Each file is standalone so it can be pasted into another model, which means it carries things that must **not** reach the codebase: a CDN `<script src="cdn.tailwindcss.com">`, per-file inline `<style>` blocks redefining `.surface` / `.surface-sunken` / `.hero-band` / `.edge`, a hand-ported `hashString` → `mulberry32` → `gradientFromSeed` plus vanilla re-implementations of all seven identity-mark shapes, a hardcoded `© 2026`, and `href="#"` stubs throughout. The mark shapes in particular are ports **of** `ToolBox.tsx` and were verified byte-identical to it — import the real components rather than reimplementing them, and compute the footer year. Treat these as mock plumbing; the design intent sits in the markup structure, not in the inline CSS or scripts.
> - Two structural (not cosmetic) decisions to make deliberately here: global search moves **into the header** on every route, and Explore uses a persistent left facet rail. Both should come from one shared layout system.
> - **Detail-route geometry is kind-dependent, not a fixed asymmetric two-column grid** (superseded 2026-11-25). The earlier rule — identity left / get-started panel right, then Composition left / Execution right — is replaced on both halves. Header: a wide **unboxed** package header, identity and metadata left, a quiet **At a Glance** right, **full-width Get Started below**; the two tall side-by-side cards were rejected as uneven and box-heavy. At a Glance carries only facts real for the selected version — **never force exactly three**, invent zeros, or render blank placeholders; one or two meaningful facts is a valid result. Body: geometry follows content density per kind — Agent Composition stacks **above** Execution Architecture on desktop as well as mobile, while a Tool's Interface and Runtime sit side by side and deeply nested JSON Schemas may need full width. The shared primitives must permit both.
> - Progressive adoption is expected — routes can migrate one at a time. What is not acceptable is a second header/footer/canvas implementation living alongside the first.
> - **Dependencies:** M8B card primitives; Stage 1 M6 shared detail shell and Stage 1 M7 SEO/SSR structure (the shell must not break server-rendered metadata or canonical behavior).
> - **Consumed by:** M8A, M9A, M9B, M10A, M10B, M12A, M12B, M13A, M13B, M14A, M14B. Those milestones adopt this shell; they do not fork it.
> - **Decisions:** **D21** (primary — the adoption review gates this milestone), D08, D09, D12, D17. **Acceptance:** S2-AC-11, S2-AC-13, S2-AC-13a, S2-AC-14, S2-AC-17.

**Implementation checklist**

- [ ] **Run the D21 design adoption review first, before building anything.** Walk the **four approved 2026-11-25 design references** (`assets/mocks/chatpgt-updates/01-landing-approved.html`, `02-explore-approved.html`, `03-agent-package-detail-approved.html`, `04-tool-detail-approved.html`, with decisions in `AgentPM_Stage2_Web_Design_Decisions_Handoff.md`) element by element, treating the three `-layered` PNGs as **historical only** — they are a light-theme redesign of an already-dark site and several of their elements are explicitly rejected above. Walk the mockups element by element and classify each as **adopt** (take as shown), **adapt** (take the idea, express it in current tokens) or **reject** (keep today's treatment), with a one-line reason each. Cover the global layout elements **and** the genuinely open styling treatments: surface elevation/shadow depth, corner radii, background gradient/tint, card border and divider weight, spacing rhythm and density, badge/pill *shape*, the monospace command-block look, and type scale/weight contrast. **Excluded from the review** (already settled, preserved as-is): per-kind color, the generated per-package identity gradient, copy, and every displayed value. The mockups are a guide weighted toward layout — selective styling adoption is expected, wholesale adoption is not.
- [ ] Put the classification in front of Zack and **get sign-off before implementation starts**. The signed list — not the images — is what the build follows and what review checks against. Record it where the milestone evidence lives.
- [ ] Flag anything where the mockup treatment is clearly better than the current site so it is a conscious choice rather than an omission; equally, flag anything in the mockups that would fight the existing brand.
- [ ] Capture before/after reference screenshots of the current production site at desktop, tablet and narrow widths, so layout change is reviewable against reality rather than against the mockups.
- [ ] Inventory the current global shell: header, nav, search placement, footer, page container widths, background treatment, and every route that renders them.
- [ ] **Include the existing primitives in that inventory, and make the D21 sheet say which ones each element reuses.** Before classifying anything, list what is already available to reuse — shared components, the spacing/gap scale, the breakpoint set, container widths, the Tailwind utility conventions — so D21 is a decision about *changing* known values rather than a fresh style selection. Then, for every element classified `adopt` or `adapt`, name the existing component or token it builds on, or state plainly that none fits and why a new one is justified. The cheap failure here is silent divergence: a mock's gap, radius or breakpoint landing in the code simply because it was in the file, with nobody having decided to change the existing one. Reviewing the build as a **diff against today's site** rather than as a match to the mocks is what catches it.
- [ ] **Apply the prose type floor and prove it by audit, not by eye.** Raise every element that carries a sentence to at least 13px, keeping 10–11px for scan-only micro-labels (uppercase eyebrows, kind chips, version strings, counts). The concrete starting list is the **42 `<p className="...text-xs...">` occurrences in `agentpm-web/src`** — each is either prose that must move up, or a label that should not have been a paragraph. Both outcomes are fine; what is not fine is leaving it at 12px by default. Audit with a grep for `text-xs` and `text-[1[012]px]` on paragraph, `<dd>`, list-item and description elements, and record the count before and after so the change is reviewable as a number rather than a screenshot impression. Watch for the inverse error too: do not globally bump `text-xs`, because the shipped chips, `<dt>` labels and mono hashes legitimately use it and enlarging those flattens the hierarchy that makes the mocks readable.
- [ ] Build the global header as one component with **anonymous** (Sign in / Sign up) and **authenticated** (notifications, avatar menu) variants. **Keep the shipped full-bleed sticky treatment** (`sticky top-0 z-40 backdrop-blur bg-slate-950/70 border-b border-slate-800`); the mockups' floating rounded bar inset from the viewport edge is **rejected**. Operator-confirmed (2026-11-22). The blur is functional, not decorative: content passing under a translucent blurred bar is how elevation reads on a dark ground, and an inset floating bar gives up the full-bleed blur edge that makes the effect legible. Consolidating the two variants into one component is the work here; the treatment does not change.
- [ ] Move global search into the header across all routes, including the ⌘K affordance, and confirm it does not regress Stage 1's search behavior, query URLs or cursor state.
- [ ] Build the page canvas: tinted/gradient background with elevated card surfaces floating above it, centered max-width column, consistent outer margin. **Use the shipped dark palette** — `bg-slate-950` canvas with translucent `slate-900/60`-class surfaces — not the mockups' white-on-light values. The adopted idea is the canvas/surface separation, not its lightness.
- [ ] Adopt a **subtle hero-band background texture/gradient** on the landing page (the mockups' treatment is the starting point, not the required artwork). Operator-confirmed (2026-11-22) as worth keeping: on a dark ground this is how layering and depth register without drop shadows. **Scope it to the hero band only** — run full-page, it competes with the card surfaces for the same depth signal and flattens the hierarchy it is meant to create. Respect `prefers-reduced-motion` if any movement is involved, and confirm it does not reduce text contrast below the shell's accessibility baseline.
- [ ] Build the **section card** primitive — icon chip, title, one-line subtitle, optional right-aligned action link — as **one** available container, **not** the default for every major section. Superseded 2026-11-25: the approved landing page is mostly open sections with restrained dividers and a single thesis card, and detail pages use panels only where they hold genuinely structured technical content. Do not nest a card inside a card for visual consistency alone. Where a section does not need a container, it does not get one.
- [ ] **Decide section-card density and weight tiers as part of D21, before the primitive ships.** The landing mockup stacks four full-width section cards with identical icon-chip + title + subtitle treatment; by the third the pattern reads as a template rather than a hierarchy, so nothing leads. Define at least one lighter-weight variant (or an explicit rule for when a section is *not* a card) so one section can carry the page's thesis while the rest support it. The primitive is good and over-applied in the mockups; because every page milestone consumes it, uniform application becomes site-wide once built. Related narrow-width risk to resolve here: the detail mockup nests cards three deep in Execution architecture (section card → phase card → capability chips).
- [ ] Centralize the **existing** kind token system into one source of truth consumed by nav filters, facet lists, cards, composition nodes and capability chips. This is consolidation, not invention: the shipped assignment is **Agent → amber, Tool → indigo, Skill → rose, Knowledge → fuchsia, Memory → teal, Profile → cyan, Loop → lime, Template → emerald** (`sky` is reserved for the cross-cutting `Signed` badge), currently hardcoded across eight card components. Preserve those values exactly — the mockups assign different hues to the same kinds and adopting them would break recognition for existing users.
- [ ] Preserve the **generated per-package identity gradient** (`components/ui/ToolBox.tsx`: deterministic FNV-1a → mulberry32 → two HSL stops, seeded by package identity, used in 17 places). The mockups replace it with generic per-kind glyph chips and a stock avatar; that is a downgrade and is rejected. Every package keeps a unique, stable avatar with no authored artwork.
- [ ] Apply the identity-vs-category test when choosing which mark a surface uses: **does this stand for a specific package, or for a category of packages?** Specific package → generated identity gradient. Kind as a category/filter/legend → a per-kind glyph is reasonable and often better at small sizes. Indicative (not locked): Agent Package Card, Explore result cards, detail identity block and composition-chart nodes are identity contexts; the Explore facet rail and kind filters/legends are category contexts. Resolve edge cases with the test rather than an exhaustive surface list.
- [ ] Keep the two systems complementary in identity contexts: the generated avatar carries *which package*, while the kind tone and label carry *what kind*. Never convey kind by replacing the avatar — that is the specific mistake the mockups make, and it conveys kind twice while losing identity entirely.
- [ ] If a per-kind glyph set is needed for category contexts, scope it as a **small deliberate addition** for those surfaces only. It must not leak into identity surfaces or become a second avatar system. The mockups' glyphs may inform it, subject to D21.
- [ ] Confirm the tone styling carries into the planned dark theme without rework — the shipped pattern (`bg-{c}-500/15 text-{c}-300 border-{c}-700/50`) is already dark-native.
- [ ] Build the shared grid primitives: Explore's persistent left facet rail, and the detail route's **wide unboxed header** (identity left, quiet At a Glance right, full-width Get Started below). The previously specified fixed asymmetric two-column detail grid is **superseded 2026-11-25** — the two tall side-by-side cards were rejected as uneven and box-heavy, and body geometry now follows content density per kind: Agent Composition stacks **above** Execution Architecture on desktop as well as mobile, while a Tool's Interface and Runtime sit side by side. Build primitives that permit both; do not hard-code one geometry for all kinds.
- [ ] Build the global footer with its full and compact variants.
- [ ] Build the kicker + headline + supporting-paragraph pattern used by landing and Explore.
- [ ] Confirm every primitive matches the **signed D21 list**: production tokens everywhere except the styling treatments deliberately marked adopt/adapt, and nothing adopted that the list does not name. Produce a token-by-token diff annotated against that list.
- [ ] Verify responsive behavior for every primitive at narrow widths, including header nav collapse, facet rail behavior and card reflow.
- [ ] Verify keyboard navigation, focus order, visible focus states, ARIA labelling on icon-only controls, and reduced-motion handling in the shell.
- [ ] Confirm SSR/metadata, canonical links and crawlability from Stage 1 M7 still work under the new shell on every migrated route.
- [ ] Record the adoption order for the remaining routes and which milestone migrates each, so partial rollout is deliberate and tracked. **M8A handoff (2026-11-25):** `/standards/agentpm/1.0.0` is delivered on the current shared shell; migrate it in M8B.1 as the first content-route adoption after the shared shell/Panel primitives are ready, before the M9/M10 package-page migrations. Preserve the immutable rendered text, rule anchors, canonical URL and source-link targets.

**Binding requirements** — authoritative constraints from `spec.md` §9 and §9.1 for this milestone. These are requirements to satisfy and verify, not a second task list; the work itself is in the implementation checklist above.

- Per-kind color and the generated per-package identity gradient are **settled and preserved**, not D21 inputs. The shipped assignment and the `ToolBox` generator survive the redesign unchanged; the mockups' conflicting kind hues and stock avatars are rejected.
- The `-layered` mockups are a **guide weighted toward layout**, not a design to reproduce 1:1. Their structure, layout, hierarchy, component inventory and interaction model are the strongest signal; styling may be adopted **selectively** where the signed D21 review chooses it, and never by default. Copy and every displayed number, name, date, count and badge are always excluded — §12–§15 and M14A/M14B settle copy, and values come from real data. The production site is the default for brand identity.
- Exactly one implementation of each global element may exist: header, page canvas, section card, kind tokens, grid primitives, footer. A second competing shell is a regression even if each looks correct in isolation.
- Progressive route-by-route adoption is acceptable and expected. Any route shipping before its migration must be named explicitly in milestone evidence with its migration scheduled, not left undeclared.
- The shell must not regress Stage 1's work: search behavior and query URL/cursor state, SSR metadata, canonical/robots handling, private-result visibility, or the shared detail shell.
- Accessibility and responsive behavior are part of this milestone, not a later polish pass — the shell is what every route inherits.

**Verification:** the standard four evidence requirements apply — see [Per-milestone evidence requirements](#per-milestone-evidence-requirements). Map the AC and decision IDs listed in this milestone's header.

**Exit gate:** The D21 adoption review is signed off, the global shell and layout primitives exist as single implementations matching that signed list, responsive/accessibility behavior is verified, Stage 1 search/SEO behavior is intact, and the route adoption order is recorded.

## Milestone 9A: Replace Agent Overview JSON-first Layout with Composition
> **Scope note:** Make the selected Agent Package version understandable as a whole system before visitors need to inspect raw manifest JSON. Outside this milestone: loss of Bindings, Examples, Readme, Security or actual manifest link.
> **Implementation notes:**
> - The diagram is a representation of real authored/resolved relationships. Direct dependencies are not equivalent to Skill-inherited Tools; each link must resolve to an exact version where known.
> - **Dependencies:** M5B resolved graph, M8B shared components, Stage 1 M6 shared shell. **M8B.1 global shell and layout system — adopt it, do not fork it.** This milestone consumes the shared header, page canvas, section card, kind tokens, grid primitives and footer; it must not create a second implementation of any of them. The `-layered` mockups govern structure/layout only (see `spec.md` §9.1); production tokens and §12–§15 copy still win.
> - **Decisions:** D09. **Acceptance:** S2-AC-11.

**Implementation checklist**

- [ ] **Build the two independently staged sticky surfaces.** A compact package identity strip (mark, breadcrumbs, name, version selector) docks under the global nav as soon as the full name/version row scrolls out of view; the tab bar docks **later and separately**, only once it reaches the bottom of that strip. Each reverses independently on scroll back. **There must be no scroll position where the package name and version are unavailable** — that gap is the defect this staging exists to prevent. Clicking the compact identity returns to the full header. Account for the taller mobile global header, keep the strip short on mobile, let tabs scroll horizontally, and do not build a second global header. The approved mocks demonstrate this with vanilla scroll listeners; **rebuild with React state and the router** — the behavior is required, the mock's implementation is not.
- [ ] **Make every in-page anchor clear both sticky surfaces.** Phase links, Readme headings, Health and Bindings jumps must land below the docked identity strip *and* the docked tab bar, which needs real `scroll-margin-top` accounting for both plus correct keyboard focus placement, not only visual scroll position.
- [ ] **Preserve the cross-links between related regions — they are part of the explanation, not decoration.** The approved Agent page carries 14. Specifically: Execution Architecture outcome destinations link to their phase cards so the transition graph is navigable; each Bindings phase row offers **View execution phase →** into the matching Overview phase and **View binding JSON ↓** into the selected version's source; the Overview Evidence summary links out to **Health & Security →**. Keep `→` for forward transitions and `↶` for backtracks, and **remove the `↗`**, which reads as an external link. Checked 2026-11-25: the glyph appears **6 times, all in the Agent mock's Composition panel** on Component package names (`issue-triage-playbook ↗`, `summarize-text ↗`, `devwork-maintainer-guide ↗`, `devwork-maintainer-state ↗`, `devwork-maintainer-style ↗`, `devwork-triage-loop ↗`) — **not** on Execution transitions, where the handoff's §6 wording places it. Those links go to each Component's own detail page, so they are internal navigation and the outward arrow actively misleads. Removal is safe: the glyph appears **zero times in shipped `agentpm-web`**, which marks genuine external links with `target="_blank"` and no arrow convention, so nothing existing is broken and no competing convention has to be preserved.
- [ ] **Stack Composition above Execution Architecture on desktop as well as mobile.** The earlier two-column pairing left dead space under Composition for longer Loops, and the stack teaches the right order: what this Agent is made of, then how that system can operate. Composition gets full width; Execution's phase cards keep a comfortable reading measure rather than stretching across the page.
- [ ] **Render Execution Architecture as a transition graph, not a numbered sequence.** The real `devwork-triage-loop` has 3 phases and **7 authored outcome transitions**, including backward transitions and `$end`/`$handoff` terminals — all verified against the live manifest. Preserve authored phase objectives, outcome IDs and destination links; `access.tools: false` on `resolve-or-handoff` must be visible rather than showing Tools as usable. Never imply declaration order is execution order. Handle one-phase, branched, cyclic, Loop-free and long-Loop states.
- [ ] Define new Overview reading order: identity/purpose/actions, composition, execution, standard/Health, examples, raw source.
- [ ] Show exact kind/name/version for each direct declared Component.
- [ ] Show transitive Skill-declared Tools with inherited relationship and source Skill.
- [ ] Link to real Component detail routes for selected versions.
- [ ] Distinguish authored direct reference from resolution result when unavailable.
- [ ] Provide no-dependencies state that says valid Agent Package, not invalid/broken.
- [ ] Support graph fan-out, multiple Skills and repeated dependencies without double-counting.
- [ ] Handle packages with private dependencies using appropriate authorization/limited state.
- [ ] Keep actual `agent.json` accessible as a full source view.
- [ ] Maintain all existing tab links and author-owned README rendering.
- [ ] **Fix the live phase-ordering bug on the Bindings tab.** `/agents/<id>/<version>/bindings` currently lists phase bindings **alphabetically** — observed 2026-11-22 on `@zack/devwork-copilot@0.1.10` as `draft-comment`, `inspect-issue`, `resolve-or-handoff`. That is not execution order, and `inspect-issue` is the Loop's `entry_phase`, so a reader scanning the page infers the wrong sequence. Order phases by the resolved Loop's actual traversal from `entry_phase`, and fall back to authored `loop.phases` array order when a Loop is absent or unresolved. Never sort phase identifiers as strings. This is a correctness fix to existing output, independent of any layout change.
- [ ] **Make Overview and the Bindings tab a summary/reference pair rather than duplicates.** Overview's execution panel shows bindings *joined to* phase order and the Loop's `access` flags, progressively disclosed — it answers "what can this reach, and when". The Bindings tab remains the complete flat reference: every binding in one scannable, deep-linkable view, including anything Overview does not surface. Retaining the tab is required (`spec.md` §"Specific page/regression map": removing these panels "is a regression even if the shared shell looks cleaner"), so the two surfaces must be deliberately differentiated instead of one quietly duplicating the other. Same summary-vs-detail split as Health summary on Overview versus the Health & Security tab.
- [ ] **Place `consumer_context` and `bindings.mcp` by what they mean, not by where they sit in the manifest.** `consumer_context` is a file the consuming project supplies and is explicitly "not a privilege grant" (`spec.md`); surface it with the run/install actions as something the visitor must provide, never among bound Components. `bindings.mcp` describes Tools the Agent exposes outward, so it belongs with execution/integration facts. Both still appear in full on the Bindings tab. State the empty case explicitly — "no MCP surfaces declared" — rather than hiding the row, consistent with how absent evidence is handled elsewhere.
- [ ] Build accessible text/table fallback for composition graph.
- [ ] Test long names, mobile overflow, unknown component, legacy version, and selection changes.
- [ ] Capture screenshots and ask Claude to inspect accuracy versus manifest/lock.

**Binding requirements** — authoritative constraints from `spec.md` for this milestone. These are requirements to satisfy and verify, not a second task list; the work itself is in the implementation checklist above.

- Restructure Agent Overview information hierarchy: purpose/publisher/version, direct/transitive Component graph, phase/Loop architecture, APDS standard/source, Health summary, Examples, secondary raw `agent.json`; retain existing Bindings/Examples/Readme/Security tabs.
- Render exact versioned links for direct Tools/Skills/Knowledge/Memory/Profiles/Loop and transitive Skill-inherited Tools; distinguish authored declarations vs resolved references where needed. Never rely only on name in multi-version graph.
- Test minimal no-Tools/no-Loop Agent; complex multi-Component Agent; missing/unresolved dependency; legacy package; private Component access; nonlinear Loop; out-of-date/unsupported standard; long names/mobile/keyboard.

**Verification:** the standard four evidence requirements apply — see [Per-milestone evidence requirements](#per-milestone-evidence-requirements). Map the AC and decision IDs listed in this milestone's header.

**Exit gate:** The scoped deliverable works against real source contracts; the binding requirements are satisfied, the standard evidence requirements are met, and Claude review has passed.

## Milestone 9B: Show Actual Loop Architecture, Standard, Health and Run Actions
> **Scope note:** Expose how a compatible Agent Package is structured for execution and how to use it, while separating APDS conformance from Harness availability. Outside this milestone: fake linear phase flow and hidden model/credential prerequisites.
> **Implementation notes:**
> - The primary detail page should teach installed/runnable distinctions. Exact command syntax must be checked in the current CLI and SDK.
> - **Dependencies:** M6A/B Harness compatibility, M7B Health, M8A standard reference, M9A composition. **M8B.1 global shell and layout system — adopt it, do not fork it.** This milestone consumes the shared header, page canvas, section card, kind tokens, grid primitives and footer; it must not create a second implementation of any of them. The `-layered` mockups govern structure/layout only (see `spec.md` §9.1); production tokens and §12–§15 copy still win.
> - **Decisions:** D09. **Acceptance:** S2-AC-11, S2-AC-12.

**Implementation checklist**

- [ ] Show Loop presence and actual phase IDs/objectives/access policies when authored.
- [ ] Represent transitions and outcomes including branch/cycle/terminal targets correctly.
- [ ] Show global and phase bindings and relevant inherited Tools, without inventing permissions.
- [ ] Make Install primary available action with selected version.
- [ ] Add minimum CLI version/update guidance from M4B's documented compatibility floor (0.1.36 tolerance floor) beside the existing package actions, using a shared presentation also consumed by M10A. Distinguish new-publication requirements from selected-release installation/Runner requirements; preserve honest historical-version compatibility. M4B owns the MDX/error wording; this milestone owns the Agent detail-page presentation.
- [ ] Show Run with Harness only when supported, or visible explanation when unavailable.
- [ ] Show Load via Node/Python SDK with actual current supported package/version syntax.
- [ ] Link to raw selected-version `agent.json` and exact versioned APDS specification.
- [ ] Place standard identity and conformance evidence as separate labels.
- [ ] Include selected-version Health summary with link to deeper evidence.
- [ ] Render Examples with actual authored prompts or honest empty state.
- [ ] **Make example prompts actionable, on the Examples tab as well as the Overview section.** The live tab (`/agents/<id>/<version>/examples`, observed 2026-11-22) renders six authored examples as title + description prose only — no code block, no copy control, no route into running them. These are literally text a visitor would paste, so present each prompt in a selectable monospace block with a copy control and a run affordance consistent with the Run-with-Harness action above, falling back to the honest no-Loop explanation when the Agent cannot run. Keep the tab and the Overview section rendering the same authored data the same way; they may differ in how many are shown, not in what an example *is*.
- [ ] Provide no-Loop Agent explanatory section, not a broken Run button.
- [ ] Test unknown standard, legacy inferred, backend unavailable and no-model-ready examples.
- [ ] Verify mobile/code copy/keyboard behavior and screen reader reading order.
- [ ] Claude reviews content truthfulness and compatibility messaging.

**Binding requirements** — authoritative constraints from `spec.md` for this milestone. These are requirements to satisfy and verify, not a second task list; the work itself is in the implementation checklist above.

- Depict effective global and per-phase bindings without inventing permissions, Memory operation execution, approvals or arbitrary linear transition order. Show nonlinear graph branches/terminal outcomes correctly or provide accessible fallback.
- Header actions: Install, conditional **Run with Harness** (available only when meaningful, otherwise explanatory) and Load via Node/Python SDK. Copy code matches actual CLI/SDK syntax, current version and package kind; avoid promising one-click execution without provider setup.
- Show standard id/version as fact, conformance as separately evidenced status; link **View agent.json** and **View APDS specification** to correct version; preserve security/provenance inspection and selected-version identity distinction.
- Preserve author-managed README and existing Examples detail; optional inline copyable starter prompt section uses authored examples only and correctly handles none.
- Capture before/after screenshots and Claude design/content review against current site and both mockup families (hierarchy yes, fictional facts and palette no).

**Verification:** the standard four evidence requirements apply — see [Per-milestone evidence requirements](#per-milestone-evidence-requirements). Map the AC and decision IDs listed in this milestone's header.

**Exit gate:** The scoped deliverable works against real source contracts; the binding requirements are satisfied, the standard evidence requirements are met, and Claude review has passed.

## Milestone 10A: Preserve All Specialized Detail Pages Under the New Hierarchy
> **Scope note:** Retain each Component/Template kind’s technical inspection features while consistently exposing standard, identity and version. Outside this milestone: Do not turn all eight kinds into generic Agent Package marketing views.
> **Implementation notes:**
> - Reuse common shell and source/standard/Health primitives but keep kind-specific data and actions. README prose belongs to authors.
> - **Dependencies:** M8B primitives, Stage 1 M6 detail shell. **M8B.1 global shell and layout system — adopt it, do not fork it.** This milestone consumes the shared header, page canvas, section card, kind tokens, grid primitives and footer; it must not create a second implementation of any of them. The `-layered` mockups govern structure/layout only (see `spec.md` §9.1); production tokens and §12–§15 copy still win.
> - **Decisions:** D17. **Acceptance:** S2-AC-11, S2-AC-12.

**Implementation checklist**

- [ ] Audit existing Tool runtime/input/output/execution detail against proposed shell.
- [ ] Preserve Skill entrypoint, scripts, references, compatible environments and Tool dependencies.
- [ ] Preserve Knowledge mode, corpus/docs, embedding/index/provenance, query inspection.
- [ ] Preserve Memory scope, record contracts, spaces, lifecycle and governance tabs.
- [ ] Preserve Instruction Profile identity/objectives/audience/communication/boundaries.
- [ ] Preserve Loop phase graph, outcomes/transitions/limits/error policy.
- [ ] Preserve Template use case, variables, stack, dependencies, entrypoints, bootstrap.
- [ ] Add category-correct breadcrumbs/labels without changing stable route IDs.
- [ ] Add standard manifest/spec links to every applicable kind.
- [ ] Apply the shared minimum CLI version/update guidance from M9B to Component and Template detail actions, consistent with M4B's Registry errors and MDX migration guidance. Verify all eight kinds and legacy/new selected versions without implying older releases became uninstallable or that the tolerance floor establishes Runner readiness.
- [ ] Keep author READMEs verbatim and technically relevant tab names accessible.
- [ ] Verify each kind on legacy, new and empty-content examples.
- [ ] Check shared shell behavior on responsive layouts and version switching.
- [ ] Claude flags any missing specialized inspection as a regression.

**Binding requirements** — authoritative constraints from `spec.md` for this milestone. These are requirements to satisfy and verify, not a second task list; the work itself is in the implementation checklist above.

- Preserve/augment Tool runtime and I/O, Skill entrypoints/reference/scripts, Knowledge mode/corpus/embedding and query flows, Memory Blueprint spaces/record/lifecycle/governance, Profile identity/communication/constraints, Loop graph/phases/outcomes/limits, Template variables/stack/dependencies/entrypoints/surfaces.
- Add declared APDS and exact `agent.json`/spec links on all kinds without calling every kind an Agent Package; align breadcrumbs, shell, identity/version facts and signing-policy wording.

**Verification:** the standard four evidence requirements apply — see [Per-milestone evidence requirements](#per-milestone-evidence-requirements). Map the AC and decision IDs listed in this milestone's header.

**Exit gate:** The scoped deliverable works against real source contracts; the binding requirements are satisfied, the standard evidence requirements are met, and Claude review has passed.

## Milestone 10B: Render Honest Package Health and Security Across All Eight Kinds
> **Scope note:** Present universal and kind-specific objective evidence with correct applicability, source and version scope. Outside this milestone: universal quality score or all-green default on missing data.
> **Implementation notes:**
> - Preferred short version summary near identity plus detailed Health/Security view. Codex may choose tab structure after reviewing existing Security; never remove SHA/signature/scan drill-down.
> - **Dependencies:** M7A/B Health data, M10A preserved specialized shell, Stage 1 M6 security panels. **M8B.1 global shell and layout system — adopt it, do not fork it.** This milestone consumes the shared header, page canvas, section card, kind tokens, grid primitives and footer; it must not create a second implementation of any of them. The `-layered` mockups govern structure/layout only (see `spec.md` §9.1); production tokens and §12–§15 copy still win.
> - **Decisions:** D12, D17. **Acceptance:** S2-AC-10, S2-AC-12.

**Implementation checklist**

- [ ] **Use one shared set of plain-English APDS scope captions across every kind.** The approved Agent and Tool mocks worded the same three scopes differently, which contradicts the requirement that Tool Health reuse the *same* disclosure language. Adopt the Agent mock's sentences verbatim: Standalone — "Manifest structure and rules that can be checked without dependencies."; Packaged content — "Required files inside the published artifact and their declared relationships."; Resolved composition — "Selected dependency versions and the relationships between them."
- [ ] **Plumb the existing per-scope conformance data through to the web — this is not a new evidence source.** `GET /tools/{id}/{version}/security` already returns `apds_validation.levels` keyed `standalone` / `packaged` / `resolved`, each with `status`, `evaluated_rules`, `issues`, `unevaluated` and `runner_evaluated` (verified live 2026-11-25). `get-tool.ts` does not parse the field today, so nothing renders it. Scope the work as serializer-to-render plumbing, not as building conformance evidence. Findings come from that real data; an incomplete check must never conceal a known failure, and passed scopes must not produce empty accordions.
- [ ] **Serialize `python_resolution` so the resolved-distributions panel can be built from real data.** The Registry already stores the full resolution — a JSONB column on the version entity under the frozen `agentpm.python-resolution.v1` contract, with canonical normalization, digest and a 2 MB cap — and `health_composition.py` already reads it to emit `python_resolution_status` (`not_evaluated` / `unknown` / `passed`). It is simply **never serialized into any API response**: `GET /tools/{id}/{version}` returns the seven `resolved_*_dependencies` but not this. Required: an API serializer field, a matching web schema field, and the render. No new capture, storage or CLI change. Three constraints — the count is a resolution **set spanning platforms** (all 17 entries in the verified `@zack/summarize-text` example carry platform markers), so do not present it as "N wheels that install everywhere"; the column is nullable, so Node Tools and older versions need a real absent state, not an empty panel; and reuse the existing `python_resolution_status` vocabulary rather than inventing new words.
- [ ] Implement concise release-scoped Health summary with clear status labels.
- [ ] Implement detailed evidence inspection and source/freshness explanation.
- [ ] Distinguish signed, verified signature, Registry attestation and scanned facts.
- [ ] Keep the observer and named check visible in every verified label and accessible name, including compact summaries/badges; prohibit bare "Verified" and attribution available only in tooltips/drill-down. Distinguish "Signature verified by Registry" from instructions to verify locally, and show a completed local check only when actually observed.
- [ ] Distinguish declared versus inferred APDS, and validation level completeness.
- [ ] Render not-applicable state for irrelevant kind-specific signals.
- [ ] Render unknown/unavailable/not evaluated without green checks.
- [ ] Display Tool target/platform only from available release artifact evidence.
- [ ] Display Agent Runner compatibility without claiming visitor local readiness.
- [ ] **Do not render Agent Runner support as supported.** M7B's per-kind extension reports `runner_support: "not_evaluated"` *and* `local_readiness: "not_evaluated"` for Agents, because the Registry does not run the Harness — so neither a positive nor a negative support verdict is available from the server. The strongest honest server-side statement is that a Loop is declared (with its phase count); anything beyond that belongs to `agentpm harness` preflight output. Both the Health detail view and any Overview summary must respect this. Caught while mocking: an earlier Overview draft showed "Runner support — Supported" with a success badge, which the data never supported.
- [ ] **Render per-artifact evidence as one row per target, never averaged or merged.** `PackageHealth.artifacts[]` carries target, digest, size, format and that artifact's own scan state. A multi-target Tool must show each target separately with its own digest and scan result; a release-level "clean" summary is only permitted when every expected artifact is covered and complete, per M7B's scan-coverage rule. A single-archive release renders one row, so the same component covers both cases without a special path.
- [ ] Preserve existing Security SHA-256/license/signature/attestation/scan data.
- [ ] Verify identity-level stars/install counts do not change with version selector.
- [ ] Test private/missing/failing evidence, old version and each of eight kinds.
- [ ] Review warnings and UI copy for certification/safety overclaims.
- [ ] Test Registry-reported versus completed local verification, missing observer/context and instruction-only states across Health summaries/details; assert no unqualified verified badge or accessible name.
- [ ] Capture screenshots and Claude audit signal-by-signal against backend evidence.

**Binding requirements** — authoritative constraints from `spec.md` for this milestone. These are requirements to satisfy and verify, not a second task list; the work itself is in the implementation checklist above.

- Implement concise per-version Health summaries and deeper inspection in dedicated Health tab or reworked Security section; choose tab structure via D12, preserving existing Security evidence (signatures, attestations, scans, SHA/license).
- Render semantic empty/unknown/not-applicable/advisory states appropriately and accessible, never universal green success check by default; provide source/freshness and version switching.
- Test all eight kinds, each with health evidence present and absent, legacy inferred version, withheld private evidence, accurate compatibility/advisory copy, regression of specialized tabs/actions, responsive behavior.
- Record D12/D17 tradeoffs and Claude review for consistent architecture and non-misleading health.

**Verification:** the standard four evidence requirements apply — see [Per-milestone evidence requirements](#per-milestone-evidence-requirements). Map the AC and decision IDs listed in this milestone's header.

**Exit gate:** The scoped deliverable works against real source contracts; the binding requirements are satisfied, the standard evidence requirements are met, and Claude review has passed.

## Release Band 3: Inspectability, Versioned Standard Reference and Package Health
Covered milestones: 7A–10B (including 8B.1).
**Layout note:** M8B.1 establishes the site-wide shell the `-layered` mockups define. It lands in this band because the detail-page work (M9A–M10B) is the first consumer; Band 4's homepage, Explore and namespace milestones consume the same shell. Build it once here.

This gives each published Agent Package an intelligible composition and execution Overview, deep access to its immutable manifest and corresponding versioned APDS standard, version-scoped objective Package Health, and preserved specialized detail views for all eight kinds. A Health display is evidence-backed rather than a quality score or blanket safety claim.

- [ ] Verify full standard/raw manifest links and direct/transitive Component links to exact versions.
- [ ] Verify Loop phase scopes/branches/cycles are rendered without fabricated execution behavior.
- [ ] Audit universal and kind-specific Health signals for every kind, status, source, visibility and version scope.
- [ ] Have Claude inspect all eight kind pages and current-design-token fidelity versus mockups.
- [ ] Confirm one global shell exists — header, page canvas, section card, kind tokens, grids, footer — with no competing second implementation, and that the route adoption order for Band 4 is recorded.
- [ ] Confirm the shell preserves Stage 1 search behavior, query URL/cursor state, SSR metadata and canonical handling, and meets responsive/keyboard/ARIA expectations.

---

## Milestone 11A: Implement Data-driven Featured Package Selection
> **Scope note:** Provide one configurable source for choosing and ordering real featured artifacts on homepage/Explore without changing presentation JSX. Outside this milestone: CMS, AI recommendation engine, invented metrics, or final featured example selection.
> **Implementation notes:**
> - Prefer lightweight config or Registry-backed editorial records. Namespace pins already exist under Stage 1, but they are publisher-local and may not be the right contract for global placement.
> - **Dependencies:** Stage 1 M4/M5 popularity/pins; new shared cards M8B.
> - **Decisions:** D10. **Acceptance:** S2-AC-15.

**Implementation checklist**

- [ ] Inspect Stage 1 namespace pins and existing featured/trending data sources.
- [ ] Compare static checked-in config and small Registry-backed curation; record D10.
- [ ] Define placements (hero, Explore featured, other optional areas) and explicit ordering.
- [ ] Represent package kind/identity and version policy without hardcoding IDs in components.
- [ ] Add server authorization/visibility filtering for private/unpublished/yanked/deleted items.
- [ ] Avoid leaking even the title of an inaccessible curated artifact to anonymous users.
- [ ] Handle missing selections, no eligible items and partial lists gracefully.
- [ ] Keep Featured visually/semantically separate from Stage 1 measured Trending.
- [ ] Resolve correct version and real metrics for selected public releases.
- [ ] Seed data-driven test fixtures without deciding permanent highlighted packages.
- [ ] Test stable ordering after suppressed/ineligible items.
- [ ] Test internal publisher edit workflow or config procedure without frontend changes.
- [ ] Document D10 selected source and how curator updates it safely.

**Binding requirements** — authoritative constraints from `spec.md` for this milestone. These are requirements to satisfy and verify, not a second task list; the work itself is in the implementation checklist above.

- Select lightweight feature/curation source (**prefer** data configuration or Registry-backed curation reusing Stage 1), define fields for ordered placement(s), kind, artifact identity, selected version/latest behavior and optional editorial label. Do not embed fixed IDs inside components or invent a CMS.
- Implement server-validated eligibility and visibility/authorization; gracefully skip unavailable, private, unpublished, deleted, unindexed or malformed entries. A bad featured entry must not crash homepage/Explore or leak private metadata.
- Support changing featured package selection and order **without editing presentation JSX**; test empty, partial, full and stale lists. Explicitly distinguish **Featured (curated)** from **Trending (Stage 1 measured)** and from namespace pins.
- Produce data fixture(s) for later golden starter examples, but **do not require choosing actual featured packages now**.

**Verification:** the standard four evidence requirements apply — see [Per-milestone evidence requirements](#per-milestone-evidence-requirements). Map the AC and decision IDs listed in this milestone's header.

**Exit gate:** The scoped deliverable works against real source contracts; the binding requirements are satisfied, the standard evidence requirements are met, and Claude review has passed.

## Milestone 11B: Build Reusable Agent Package Cards and Investigate README Embedding
> **Scope note:** Create grounded card variants and produce an explicit technical decision/prototype for Markdown-embeddable GitHub README Agent Package Cards. Outside this milestone: Full dynamic public image service is not automatically authorized by the investigation.
> **Implementation notes:**
> - Keep homepage, Explore, social preview and detail visual identity consistent but allow different information density. A card is not an A2A Agent Card protocol document.
> - **Dependencies:** M8B shared primitives; M11A featured data; Stage 1 M7 OG infrastructure.
> - **Decisions:** D09, D16. **Acceptance:** S2-AC-16.

**Implementation checklist**

- [ ] Define compact listing versus featured/hero Agent Package card variant requirements.
- [ ] Show real purpose, selected version, six Component categories where present and install action.
- [ ] Avoid implying everything is universally runnable or explicitly APDS verified.
- [ ] Reuse Stage 1 social/OG preview generation where practical.
- [ ] Prototype Markdown image/link embedding with correct Registry destination.
- [ ] Compare static snapshot vs dynamically rendered card, cache/invalidations and abuse limits.
- [ ] Compare pinned package version versus latest-version image behavior.
- [ ] Decide which trust/Health signals can safely appear on public embeddable images.
- [ ] Check private namespace access and accidental exposure through image caching or card URLs.
- [ ] Check GitHub README image rendering and reasonable image dimensions.
- [ ] Record D16 analysis, prototype screenshot, decision to implement or defer, and rough scope.
- [ ] Keep any deferred actual embedding implementation tracked as later work.
- [ ] Coordinate output with future AgentPM Developer sharing handoff.

**Binding requirements** — authoritative constraints from `spec.md` for this milestone. These are requirements to satisfy and verify, not a second task list; the work itself is in the implementation checklist above.

- Build Agent Package Card variants as appropriate for homepage hero, featured listing, Explore card and OG/share preview; show actual identity/purpose/composition/release standard, with controlled truncation/responsiveness and absent metadata fallbacks. Preserve visual consistency with current site rather than following synthetic mockup colors.
- **Mandatory D16 investigation/prototype:** GitHub README Markdown embed: image URL/static vs dynamic card, pinned/latest semantics, change invalidation, cache, auth/private redaction, proper alt/accessibility, source provenance, share link, costs/abuse, OG reuse. Write decision **implement v1 now** or **defer with a concrete later task/brief**, and ask Claude to review tradeoff. Do not silently label full embedding “done” based on OG alone.

**Verification:** the standard four evidence requirements apply — see [Per-milestone evidence requirements](#per-milestone-evidence-requirements). Map the AC and decision IDs listed in this milestone's header.

**Exit gate:** The scoped deliverable works against real source contracts; the binding requirements are satisfied, the standard evidence requirements are met, and Claude review has passed.

## Milestone 12A: Redesign Homepage Hero Around the Complete Agent Package
> **Scope note:** Replace Tool/Template-first hero hierarchy with developer problem → AgentPM Manager → real composed Agent Package. Outside this milestone: Not a total color/brand redesign; not a standards-body homepage.
> **Implementation notes:**
> - Preserve current layered/floating visual language and colors. Generated mockups illustrate structure but may include fictional names, counts, false verification and other inaccurate strings.
> - **Dependencies:** M11A/B curation/card, M6 supported run path, Stage 1 M7 SEO primitives. **M8B.1 global shell and layout system — adopt it, do not fork it.** This milestone consumes the shared header, page canvas, section card, kind tokens, grid primitives and footer; it must not create a second implementation of any of them. The `-layered` mockups govern structure/layout only (see `spec.md` §9.1); production tokens and §12–§15 copy still win.
> - **Decisions:** D08. **Acceptance:** S2-AC-13.

**Implementation checklist**

- [ ] **Build the thesis composition animation as an approved design element.** The featured Agent's Component families converge toward its Agent Package identity: staggered node reveal, joins tracing inward, Agent identity resolving last. Three properties make it acceptable and must survive implementation — it fires **once** on entering the viewport and is static thereafter (no perpetual motion); it is gated behind `prefers-reduced-motion: no-preference`, with reduced-motion visitors receiving the same illustration **fully composed** rather than degraded or empty; and it reinforces **composition**, not a runtime dependency graph. Hovers stay restrained. This was explicitly preferred over an animation-heavy marketing style — do not add a second lengthy animated section.
- [ ] **Make home Trending a genuine cross-kind ranking over a comparable score.** No curated "balanced" mix of kinds, no fabricated ecosystem scale, no single-input metric presented as composite, no false popularity window. It is an acceptable outcome that all nine results are Tools or that a kind is absent; use the planned low-activity fallback when the ranking is not yet meaningful. **Verify before describing the score as composite:** `trending_score` currently equals `installs_last_7d` exactly for every sampled package, including the two with stars, so it is not composite today.
- [ ] Inventory current `website/src/app/page.tsx`, Hero, Trending, Templates and codebox sections.
- [ ] Identify which existing sections should move to Explore or be compacted.
- [ ] Write hero copy centered on reusable complete Agent systems and AgentPM as Manager.
- [ ] Explicitly distinguish Agent Package, Components, APDS and built-in Harness Runner.
- [ ] Render a real curated Agent Package hero card with exact version and composition. This is the **shareable Agent Package Card at full fidelity** (M8B/M11B primitive, same data model as Explore/detail/OG and the D16 README-embed investigation) — not a bespoke hero graphic. Its content density follows from what the Card must convey when it appears **unaccompanied** somewhere off-site; settle that in the Card design rather than trimming it here for hero balance.
- [ ] Link through to current detail route and use real install/Harness command where applicable.
- [ ] Show sensible fallback if no eligible featured Agent Package exists.
- [ ] Preserve install-CLI affordance and honest SDK/framework portability statement.
- [ ] Present Templates as a separate scaffold route rather than the category primitive.
- [ ] Ensure main CTA leads to populated Agent Package discovery and next actions.
- [ ] Avoid hero stat panels emphasizing zero recent installs or fabricated metrics.
- [ ] Capture before/after desktop views and category comprehension check.

**Binding requirements** — authoritative constraints from `spec.md` for this milestone. These are requirements to satisfy and verify, not a second task list; the work itself is in the implementation checklist above.

- Audit existing `website/src/app/page.tsx`, Hero, Trending, Templates and kind-specific showcase components and current design tokens. Draft content hierarchy and before/after section map; retire rather than replicate redundant stacked component-specific trending blocks.
- Rewrite hero around developer problem and missing reusable artifact, with explicit **AgentPM — Agent Package Manager**, meaningful primary CTA, simple Agent Package example and correct Install/Run command hierarchy; copy avoids overselling ecosystem adoption.
- Present a real, data-driven featured Agent Package composition card with versioned link, component summary, standard, installation/Harness snippet when applicable and graceful no-featured fallback; never display fictitious numbers or incorrect package facts.
- Include concise product/standard/Registry/Harness reference Runner model, reusable Component/Template pathways secondary to Agent Package, reduced low-volume metric emphasis and three onboarding paths.

**Verification:** the standard four evidence requirements apply — see [Per-milestone evidence requirements](#per-milestone-evidence-requirements). Map the AC and decision IDs listed in this milestone's header.

**Exit gate:** The scoped deliverable works against real source contracts; the binding requirements are satisfied, the standard evidence requirements are met, and Claude review has passed.

## Milestone 12B: Complete Responsive Homepage Story, Component Paths and Starter Paths
> **Scope note:** Make the full homepage a coherent category-first product journey that works on desktop/mobile and connects to real destinations. Outside this milestone: repetitive six-kind feature inventory and fictional ready-to-run promise.
> **Implementation notes:**
> - Broader discovery belongs to Explore. Homepage should be deliberate and comparatively concise, with Components as secondary reusable parts and Templates clearly distinct.
> - **The detail page, not the homepage, is the strongest positioning surface — size this milestone accordingly.** The marketing doc's §5 messaging requirement is to "quickly clarify that the managed artifact is the complete agent system — not merely its code," contrasting "npm packages libraries" with "AgentPM packages the agent itself: its capabilities, behavior, dependencies, knowledge, memory, execution structure." The package detail page's side-by-side Composition and Execution architecture — including the transitive dependency hanging off a Skill — *is* that clarification rendered as structure instead of prose, which no competing surface shows. So the homepage's job is narrower than the mockup assumes: establish the category with its familiar package-manager anchor, make the "not just code" correction immediately, and route to a real detail page quickly. Carrying the whole argument across four equal-weight homepage sections is what produces the density problem M8B.1's D21 density decision addresses.
> - **Dependencies:** M12A hero; M11A featured; M15B starter later—use safe interim fallback. **M8B.1 global shell and layout system — adopt it, do not fork it.** This milestone consumes the shared header, page canvas, section card, kind tokens, grid primitives and footer; it must not create a second implementation of any of them. The `-layered` mockups govern structure/layout only (see `spec.md` §9.1); production tokens and §12–§15 copy still win.
> - **Decisions:** D08. **Acceptance:** S2-AC-13, S2-AC-19.

**Implementation checklist**

- [ ] Build compact visual explanation of Agent Package and how Components compose into it.
- [ ] Explain Manager/Registry/APDS/Harness roles without implying all Runners support APDS now.
- [ ] **Do not reproduce the mockup's "Harness / Compatible Runners" third-party framework logo row** (LangChain, CrewAI, "and more…"). It fails on two counts, not one. First, it asserts interop that does not exist — the overclaim the Runner-support item above already forbids. Second, and more importantly, it **inverts the positioning**: per `assets/AgentPM_Marketing - Google Docs.pdf` §1 the alternatives AgentPM competes against *are* framework-centric and host-centric development, and §3's differentiated value is framework-independence ("Keep the Agent definition separate from any single framework, host, model provider, or runtime"). A framework logo wall reads as "accessory to the framework you already chose," which is the wrong comparison set — §5 explicitly rejects `agent framework` and `MCP platform` as categories for that reason. If a compatibility story is needed here, state the portability property in AgentPM's own terms and name only integrations that genuinely exist today.
- [ ] Add small featured Agent Package row with accurate data-driven cards.
- [ ] **This row ships as "Trending", scored from a composite signal, and must not be labelled with any single metric or window.** Decision: `Trending` over `Recently published`, with no per-card figure and no stated method line. Rationale and consequences for implementation. (a) Trending is **composite** — installs plus stars plus whatever else the score admits — so naming one input ("most installed in the last 7 days") would misdescribe the algorithm, and rendering one number per card would present one input as if it were the rank. Both were tried in the mock and removed. Omitting the basis is therefore correct here, not a disclosure gap. (b) Confirm the Registry actually computes a composite ranking before using the label. If the underlying query is really `ORDER BY published_at DESC`, the honest label is recency no matter what the heading says — the same class of defect as a conformance label asserting evidence that was never collected. (c) Document the score's inputs in docs, not on the landing page, so the ranking stays auditable without the marketing surface committing to a formula that will change. (d) The thin-data case is **not** covered by the existing absent-featured-data test on the item below: with real volume currently in the low tens, a `Trending` heading over a short list advertises sparse adoption rather than concealing it. Define a floor (minimum distinct installers, or minimum qualifying packages) under which the surface falls back to `Recently published` or is suppressed, and test that fallback with present-but-thin data, not just with empty data.
- [ ] Add compact navigation for all six Component kinds with one-line meaning.
- [ ] Add distinct Template pathway with real browsing link.
- [ ] Add three onboarding paths (Try, Build, Template) and current real commands/routes.
- [ ] Reuse kind icons and current brand palette/elevation tokens.
- [ ] Check meaningful responsive layout and code-box overflow on small screens.
- [ ] Test keyboard focus, readable diagram alt/text fallback and contrast.
- [ ] Test anonymous, authenticated and absent-featured-data rendering.
- [ ] Test link destinations and query state without introducing dead routes.
- [ ] Verify homepage copy against actual standard version and available Runner support.
- [ ] Capture desktop/tablet/mobile screenshots; Claude review layout and category meaning.

**Binding requirements** — authoritative constraints from `spec.md` for this milestone. These are requirements to satisfy and verify, not a second task list; the work itself is in the implementation checklist above.

- Preserve existing AgentPM brand palette, typography, floating panels/gradients and responsive UX; borrow diagrams/information density from `assets/*-structure.png` and layered feel from `assets/*-layered.png` without copying their styling or invented integrations.
- Wire real Explore/Agent/package-doc destinations; confirm no disabled signup/navigation regression or reliance on a currently nonexistent AgentPM Developer Agent.
- Verify responsive/mobile, keyboard, accessibility, signed-in/out, no featured data and realistic long package descriptions; create side-by-side screenshots and initial developer comprehension walkthrough.
- Record D08 final design decisions; Claude reviews category clarity, copy accuracy, mockup consistency and real-data behavior.

**Verification:** the standard four evidence requirements apply — see [Per-milestone evidence requirements](#per-milestone-evidence-requirements). Map the AC and decision IDs listed in this milestone's header.

**Exit gate:** The scoped deliverable works against real source contracts; the binding requirements are satisfied, the standard evidence requirements are met, and Claude review has passed.

## Milestone 13A: Reorganize Explore and Enrich Every Kind of Search Card
> **Scope note:** Make Explore a category-aware discovery experience while leaving Stage 1 search correctness and deterministic ranking intact. Outside this milestone: new relevance algorithm, recursive popularity scoring or AI search.
> **Implementation notes:**
> - No-query `/explore` is curated discovery. A text query remains Stage 1 result ranking. Grouping kinds in UI must not alter persisted machine kind or break URLs/cursors.
> - **Before enabling no-query filtered browse:** Stage 1 M2's faceted filters run JSONB predicates (`jsonb_each`, `jsonb_array_elements_text`, `jsonb_array_length`) inside the `base` CTE with no expression indexes, and today they rely on the required text query narrowing the candidate set first. Removing that requirement makes them scan the full search index. Add expression indexes or denormalized filter columns before turning on no-query browse for these facets — see `TODO(M13A)` in `agentpm-api/app/tools/application/search.py` (`_PACKAGE_FILTER_SQL`).
> - **Dependencies:** Stage 1 M1–M4 discovery; M11A featured; M8B cards. **M8B.1 global shell and layout system — adopt it, do not fork it.** This milestone consumes the shared header, page canvas, section card, kind tokens, grid primitives and footer; it must not create a second implementation of any of them. The `-layered` mockups govern structure/layout only (see `spec.md` §9.1); production tokens and §12–§15 copy still win.
> - **Decisions:** D11. **Acceptance:** S2-AC-14, S2-AC-15.

**Implementation checklist**

- [ ] Group Kind facet into Agent Packages, six Components, Templates and Namespaces.
- [ ] **Add three artifact-group shortcuts of equal visual weight** — Agent Packages, Components, Templates — that invoke the **same real filter state** rather than a second parallel filtering system. Namespaces stay separately searchable and are **not** a fourth group, nor covered by any claim that results are installable.
- [ ] **Use neutral monochrome category marks for the group shortcuts**, never generated per-package identity gradients, which belong to named packages only. A combined six-in-one Components glyph was **rejected as illegible at small size**; the approved mark is a three-shape collection (square + ring + overlapping tiles) for independent building blocks. It must not be the Tool icon alone, which would misrepresent every Component as a Tool. Templates use the folded-corner concept, which still requires an actual component to be created (see M8B).
- [ ] **Derive the heading and description from the active filters, not from which shortcut was clicked.** The default "Agent Packages, Components, and Templates in one list" message must change when any group or individual kind is selected, and keep changing as the selection changes. Counts are truthful and scoped to current filters.
- [ ] Preserve each underlying kind/filter value and URL query parameter interpretation.
- [ ] Show accurate grouped result counts and labels for selected filters.
- [ ] **Build the no-query `/explore` as a populated, browsable cross-kind Trending list in the same row language as search** — not a featured-Agent-first card gallery (superseded 2026-11-25). Use genuine live Registry ranking with a truthful low-activity fallback. The approved mock previews a subset of real captured `q=zack` items sorted for demonstration; **that order is not a verified registry-wide ranking and must not ship as one**. Curated Featured, if it appears on other routes, stays visibly distinct from measured Trending and never inherits dependency metrics.
- [ ] Keep curated Featured separate from measured Trending and publisher pins.
- [ ] **Render search results as full-width rows, not a 2-up card grid.** Operator-confirmed 2026-11-23 against the real 52-result response for `q=zack`; `assets/mocks/explore-rows.html` is the chosen treatment and `assets/mocks/explore-page.html` is retained as the rejected alternative so the comparison is not re-litigated. `spec.md` §9.1 and §13 are amended to match — the old "2-up search result" card variant no longer applies. Structural reasons, in priority order: names form a **single scan column**; installs and dates occupy a **true right-aligned grid track** and so compare without the eye zigzagging, and without the `flex`/`mt-auto` workaround a 2-up grid needs to align them at all; one hairline-divided container replaces twenty bordered boxes; the full width absorbs the per-kind metadata on the title line instead of forcing a fifth stacked row. **The no-query state uses the same rows** (operator-confirmed 2026-11-25, reversing my earlier recommendation to switch to a card gallery there). The reason is positioning, not layout: an Agent Package is the category anchor on *Landing*, not a privileged ranking on Explore, and someone who uses AgentPM only for Tools must get an equally good discovery path. One row language across both states.
- [ ] **Carry kind on a colour rail at the row's leading edge**, in the existing kind token, alongside the kind chip and the identity mark. The rail is the only kind signal that works peripherally: scrolling a mixed result set shows its kind distribution without anything being read or counted. Do **not** tint the whole row — and replace `TemplateCard`'s current hash-selected tint (one of cyan/emerald/amber/rose/indigo/sky) with the single emerald Template token, since four of those six collide with other kinds' tokens and one is the reserved `Signed` sky.
- [ ] **Clamp result descriptions to two lines from `sm:` up and three below it.** Measured on the real result set at the real widths: two lines show 20/20 descriptions in full on desktop with only 5 rows needing the second line, while one line shows 2/20 on mobile. Single-line `truncate` is the wrong default and was the only genuine objection to rows before it was measured.
- [ ] Enrich the Agent Package result row with real composition/Loop summary.
- [ ] **Enrich Tool/Skill/Knowledge/Memory/Profile/Loop/Template rows with kind-specific metadata, all of which is already in the search payload** — nothing here needs new API work, and `assets/mocks/explore-rows.html` renders each from live data. One parallel label-plus-chips grammar: Agent `Composed of` from `manifest.tools/skills/knowledge/memory/profiles/loop`; Tool `Exposes` from `inputs.properties.action.enum` and `environment.vars`; Skill `Uses` from `manifest.tools` plus `skill.references`; Loop `Phases` from `loop.phases` rendered as an ordered flow (`inspect-issue → draft-comment → resolve-or-handoff`), which teaches what a Loop is better than any prose on the page; Profile `Role` from `profile.identity.role`; Memory `Retrieval` from `memory.spaces` and `memory.operations`; Template `Stack` from the literal `template.stack` and `template.use_case` fields; Knowledge `Mode` from `knowledge.mode`. Before this, six of eight kinds were informationally identical — name, version, description, kind chip, Signed, license, installs, date — with only the chip colour differing.
- [ ] **Give Explore a visible page header that names the category.** The route currently renders only `<h1 class="sr-only">`, which assumes arrival via the landing page; someone who uses global search lands here cold and meets nine unexplained nouns with no anchor, and the facet rail teaches the Agent Packages / Components split only structurally, through indentation, in the place people go to filter rather than to learn. State the two-tier model in words and offer it as the first filter: eyebrow naming Agent Package Management, a visible `<h1>`, and a lede saying Agent Packages are complete agent systems while Components are the reusable parts they are built from **and are installable on their own** — that last clause matters, or the copy demotes Components to mere parts and contradicts their standalone value.
- [ ] **Put the facet counts on the facets.** `totals_by_type` is already in the search response and is currently rendered only as a flat text run above the results (`Tools: 20 Agents: 3 Skills: 4 …`). On the rail checkboxes those same numbers turn the filter list from a guess into a map; remove the duplicated text row.
- [ ] **Collapse the kind-specific filter groups by default.** `kindInPlay()` returns true whenever "All Artifacts" is selected, so the default rail renders every conditional group at once — roughly 45 controls before the user has narrowed anything. Agent-includes, Tool runtime, Skill compatibility, Knowledge mode, Memory retrieval and Template are only meaningful after a kind is chosen: render them as labelled, counted disclosures, collapsed by default and auto-expanded when their kind is the only one selected. "Artifact type" and "More filters" stay open, being cross-kind.
- [ ] **Build the active-filter chip row** that `app/explore/layout.tsx` already carries a commented-out TODO for (`<FilterChip label="lang: python" />`). Without it the only record of what is narrowing the result set is the rail, which is off-screen on mobile and scrolled out of view on desktop.
- [ ] Use absent/unknown field fallbacks instead of invented attributes. Verify with the two real Tools that have neither actions nor environment variables (`@zack/native-release-check`, `@zack/release-check-5f7d58034c75`): their rows must omit the metadata line entirely rather than render an empty label.
- [ ] **Decided 2026-11-25: no `Signed` badge on result rows.** All 52 results in the live sample are signed, so the badge currently differentiates nothing and costs a chip position on every row. Retain the optional Signed filter under More filters, and retain signing evidence on package detail. An absent badge is **not** absent evidence and must not read as unknown. Revisit once third-party publishers make the signal discriminating.
- [ ] **Consider a "used by" relationship on Component rows, and treat it as backend work.** Page 1 of `q=zack` contains `@zack/devwork-copilot` together with five of its six Components, listed as unrelated peers — the page is showing an Agent Package and most of its parts while teaching nothing about the relationship, which is the single largest missed explanation on the route. The search payload has no reverse-dependency field (an Agent's manifest lists its Components; a Component does not know its dependents), so this needs an API addition and is **not** free card design. When it ships it must read as supplementary context, never as the Component's identity: Components are independently installable artifacts, and framing one as "part of an Agent" contradicts the reusable-building-block positioning the page header states.
- [ ] Preserve relevance sorting for text searches; do not force Agent-first ranking.
- [ ] Preserve Stage 1 pagination/cursor/history/filter URL state.
- [ ] Improve context-specific no-results CTA without hiding clear filters.
- [ ] Test public/private visibility, no-query, query, kinds, empty/error/loading and long names.
- [ ] Test tiny catalogs and a mix of exact and fuzzy matches.
- [ ] Claude independently compares results/order with Stage 1 known-good behavior.

**Binding requirements** — authoritative constraints from `spec.md` for this milestone. These are requirements to satisfy and verify, not a second task list; the work itself is in the implementation checklist above.

- Group kind filters/facets/UI counts as Agent Packages / Components (six kinds) / Templates / Namespaces while preserving actual backend kind constants, existing deep link query params, URL history and search semantics.
- Make no-query `/explore` a useful data-driven discovery surface with Featured (curated) and Trending (measured) sections, Component and Template pathways; handle no curated data with safe fallback. Query-bearing Explore must keep Stage 1's deterministic relevance, pagination, facet combination and visibility.
- Expand kind-specific result summaries using existing trustworthy metadata: Agent composition; Tool runtime/target; Skill capabilities/dependencies; Knowledge mode/corpus; Memory spaces/retrieval; Profile identity; Loop phase/archetype; Template stack/use case/surfaces; anonymous users should see only public data.
- Align package-detail breadcrumbs, search labels/status, catalog taxonomy, loading/no-result/error states, filters and footer copy with new category. Stage 2 may add copyable “build something like this” idea prompts, but only active AgentPM Developer destinations when Agent is actually available.
- Regression suite for no-query Explore; text relevancy and stable sort with/without filters; public/private mixed; missing curated items; namespace-specific card/pin result order; back/forward and cursor navigation; long labels and responsive layout.

**Verification:** the standard four evidence requirements apply — see [Per-milestone evidence requirements](#per-milestone-evidence-requirements). Map the AC and decision IDs listed in this milestone's header.

**Exit gate:** The scoped deliverable works against real source contracts; the binding requirements are satisfied, the standard evidence requirements are met, and Claude review has passed.

## Milestone 13B: Update Namespace Publisher Catalogs to Match Category Hierarchy
> **Scope note:** Organize each publisher page as a clear catalog of Agent Packages, Components and Templates while preserving Stage 1 namespace features. Outside this milestone: new namespace search implementation and Agent-only pinned policy.
> **Implementation notes:**
> - Namespace Owner/Admin pins can highlight any supported artifact kind. Namespace-scoped searches share the same service as Explore, with hard server constraint.
> - **Dependencies:** Stage 1 M5 scoped discovery/pins; M13A category cards. **M8B.1 global shell and layout system — adopt it, do not fork it.** This milestone consumes the shared header, page canvas, section card, kind tokens, grid primitives and footer; it must not create a second implementation of any of them. The `-layered` mockups govern structure/layout only (see `spec.md` §9.1); production tokens and §12–§15 copy still win.
> - **Decisions:** D11. **Acceptance:** S2-AC-14.

**Implementation checklist**

- [ ] Show Agent Packages and Components/Templates as understandable catalog groups.
- [ ] Preserve Stage 1 scoped search, facets, sort, URL state and pagination.
- [ ] Preserve Owner/Admin pin management for any eligible kind.
- [ ] Render pins with shared enriched card primitives where practical.
- [ ] Render accurate type summary/counts rather than generated peer-kind word salad.
- [ ] Ensure recent-activity entries lead to useful artifact/version destinations.
- [ ] Use correct signing/attestation wording for namespace security policy.
- [ ] Keep publisher identity/permission controls unchanged.
- [ ] Test no pins, some pins, invalid/yanked pins and mixed visibility.
- [ ] Test anonymous pages versus Owner/Admin management states.
- [ ] Compare equivalent namespace query results to filtered Explore for semantic consistency.
- [ ] Claude checks there is no private data leakage or type-priority distortion.

**Binding requirements** — authoritative constraints from `spec.md` for this milestone. These are requirements to satisfy and verify, not a second task list; the work itself is in the implementation checklist above.

- Adjust namespace pages to showcase Agent Packages, Components and Templates while respecting owner/admin pinned **any-kind** selections and Stage 1 scoped search, sorting, pagination and namespace-signing semantics. Improve Recent Activity click-through only if trivial and correctly grounded.
- Record D11 and any DTO change against Stage 1 shared result API; Claude checks no arbitrary Agent sort boost and no private data leakage.

**Verification:** the standard four evidence requirements apply — see [Per-milestone evidence requirements](#per-milestone-evidence-requirements). Map the AC and decision IDs listed in this milestone's header.

**Exit gate:** The scoped deliverable works against real source contracts; the binding requirements are satisfied, the standard evidence requirements are met, and Claude review has passed.

## Milestone 14A: Publish a Deeper Agent Package Management Explainer and Shared Terminology
> **Scope note:** Give the category its own focused explanation while making all shared website chrome identify AgentPM and artifact kinds correctly. Outside this milestone: misleading industry-standard claim and generic global rename of machine fields.
> **Implementation notes:**
> - Prefer `/agent-package-management` for why/the mental model and `/standards/agentpm/1.0.0` for normative technical rules. The homepage remains AgentPM-first.
> - **Dependencies:** M12 homepage, M8A normative APDS reference; Stage 1 M7 SEO. **M8B.1 global shell and layout system — adopt it, do not fork it.** This milestone consumes the shared header, page canvas, section card, kind tokens, grid primitives and footer; it must not create a second implementation of any of them. The `-layered` mockups govern structure/layout only (see `spec.md` §9.1); production tokens and §12–§15 copy still win.
> - **Decisions:** D18. **Acceptance:** S2-AC-17.

**Implementation checklist**

- [ ] Write product-focused explanation of developer reuse problem and common alternatives.
- [ ] Define Agent Package versus six Component kinds versus Template.
- [ ] Explain APDS semantics, Manager, Registry and compatible Runner without overpromising support.
- [ ] Explain why this differs from copying Git repos or packaging individual Tools.
- [ ] Link to real install/run/build flows and versioned APDS technical page.
- [ ] Audit top nav, footer, breadcrumbs, category naming and search prompt language.
- [ ] Replace product-controlled Agents labels with Agent Packages when referring to artifacts.
- [ ] Retain Agent where referring to acting runtime behavior or machine `kind: agent`.
- [ ] Avoid inflating unverified claims such as `works in every framework`.
- [ ] Check meta title, H1/H2 and internal-link labels for category terms.
- [ ] Capture anonymous reader comprehension feedback and necessary copy changes.
- [ ] Record D18 route/design/claim rationale and Claude editorial review.

**Binding requirements** — authoritative constraints from `spec.md` for this milestone. These are requirements to satisfy and verify, not a second task list; the work itself is in the implementation checklist above.

- Add a concise deeper category explainer, **prefer** `/agent-package-management`, linking to AgentPM Manager/Registry/Harness product actions, APDS normative route, portable Agent composition and the main competitor/status-quo problem; AgentPM remains hero product, not generic standards site.
- Audit shared navigation, mobile chrome, breadcrumbs, footer, pricing, metadata titles/descriptions, H1/H2, internal links, OG previews, and social snippets for Agent Package vs Component vs Agent acting-entity usage. Avoid blind global renames and route breaks.
- Review security/legal honesty of “standard”, “open”, “framework-independent”, “works everywhere”, “verified”, runtime provider and category adoption claims; no unproven universal compatibility or fake third-party endorsement.

**Verification:** the standard four evidence requirements apply — see [Per-milestone evidence requirements](#per-milestone-evidence-requirements). Map the AC and decision IDs listed in this milestone's header.

**Exit gate:** The scoped deliverable works against real source contracts; the binding requirements are satisfied, the standard evidence requirements are met, and Claude review has passed.

## Milestone 14B: Align Pricing Copy, SEO Semantics and Shared Navigation
> **Scope note:** Carry consistent product/category terminology into pricing, social/search metadata and reusable site navigation without changing the billing model. Outside this milestone: checkout or plan pricing changes; no new SEO engine.
> **Implementation notes:**
> - Keep existing Free/Pro/Team behavior. Fix only copy and public access explanation; preserve Stage 1 canonical/sitemap/robots behavior.
> - **Dependencies:** M14A category story; Stage 1 M7 SEO and **Stage 1 M10A** analytics (Stage 1 M10B is CLI telemetry and is not a dependency here). **M8B.1 global shell and layout system — adopt it, do not fork it.** This milestone consumes the shared header, page canvas, section card, kind tokens, grid primitives and footer; it must not create a second implementation of any of them. The `-layered` mockups govern structure/layout only (see `spec.md` §9.1); production tokens and §12–§15 copy still win.
> - **Decisions:** D18. **Acceptance:** S2-AC-17.

**Implementation checklist**

- [ ] State public Agent Package and Component publishing/installation is free according to existing plan rules.
- [ ] Describe paid private namespaces and shared internal Team work accurately.
- [ ] Do not claim dedicated Registry hosting is included in Team if not true.
- [ ] Add clear anonymous Explore entry where appropriate.
- [ ] Audit homepage, explore, category page, namespace, package details and pricing metadata.
- [ ] Use unique descriptive titles/H1/H2 and category-related anchor text.
- [ ] Check canonical behavior for new routes and parameterized Explore URLs.
- [ ] Validate OG/social images have truthful public selected-version data.
- [ ] Keep new route indexing intentional; no crawl explosion from filters.
- [ ] Review claim language for signing, scans, conformity and Runner support.
- [ ] Test pricing navigation and existing checkout/signup regressions.
- [ ] Capture SEO/meta screenshots/test results and Claude copy review.

**Binding requirements** — authoritative constraints from `spec.md` for this milestone. These are requirements to satisfy and verify, not a second task list; the work itself is in the implementation checklist above.

- Pricing pass: Free/public packages and Components, private namespaces on paid plans, Team as shared internal publisher workspace, accurate access for public Explore. **No plan/checkout changes.**
- Reuse Stage 1 SSR SEO, canonical, sitemap and robots implementation; ensure new standard/category routes are intentionally crawlable and arbitrary Explore query/filter states aren't accidentally indexed; preserve old package detail canonical/OG paths.
- Test links/canonicals/share previews, anonymous browsing, SEO snapshots, pricing consistency, page navigation and comprehension; record D18 copy/IA rationale.

**Verification:** the standard four evidence requirements apply — see [Per-milestone evidence requirements](#per-milestone-evidence-requirements). Map the AC and decision IDs listed in this milestone's header.

**Exit gate:** The scoped deliverable works against real source contracts; the binding requirements are satisfied, the standard evidence requirements are met, and Claude review has passed.

## Release Band 4: Category-first Website, Curated Discovery and Sharing Foundation
Covered milestones: 11A–14B.

This gives visitors a product-first homepage centered on complete Agent Packages, an intentional no-query Explore, richer cards and namespace catalog hierarchy, a deeper category page, accurate pricing/SEO meaning, and lightweight data-driven featured package selection. It also includes an explicit investigation into GitHub README-embeddable cards without requiring a CMS or a new recommendation engine.

- [ ] Change featured package choices/order using data/config without editing React presentation code.
- [ ] Verify Featured and Trending are separately labeled and private/ineligible curated items never leak.
- [ ] Verify query relevance, filters/cursors and namespace pins still behave exactly as Stage 1 promises.
- [ ] Verify website preserves existing palette/elevation while adopting composition-centric hierarchy; review SEO and pricing truthfulness.
- [ ] Verify homepage, Explore and namespace all render on the **M8B.1 shell** rather than forked layouts, and that any route still on the old shell is explicitly named with a migration owner.

---

## Milestone 15A: Make CLI Help Identify AgentPM as an Agent Package Manager
> **Scope note:** Turn CLI discovery into an accurate guide to Agent Package Management and the Tool-vs-Harness distinction. Outside this milestone: breaking command renames, argument redesign or Stage 1 error/TTY rewrite.
> **Implementation notes:**
> - The literal terms Agent Package Manager and Agent Package Management must appear in top-level help. Keep help compact, current and accurate.
> - **Dependencies:** M3A init, M5 install, M6 Harness; Stage 1 M8/M9 changes.
> - **Decisions:** D14. **Acceptance:** S2-AC-18.

**Implementation checklist**

- [ ] Set top-level clap help heading to explicitly describe AgentPM as an Agent Package Manager.
- [ ] Explain Agent Package Management in one concise lifecycle sentence.
- [ ] Show working Get Started command examples, tested against current parser.
- [ ] Explain default `agentpm init` now generates an Agent Package.
- [ ] Explain `--kind tool`/other Components for explicit initialization.
- [ ] Explain `agentpm lint` as APDS-aware definition validation.
- [ ] Explain `agentpm install` as dependency/lock resolution and local install.
- [ ] Explain `agentpm new` as Template workspace scaffolding.
- [ ] Explain `agentpm run` as Tool execution, not full Agent Package run.
- [ ] Explain `agentpm harness` as reference Runner for compatible Agent Packages.
- [ ] Preserve publish Registry link copy and SDK loading explanation.
- [ ] Snapshot `--help` top-level and relevant subcommands under noninteractive output.
- [ ] Test code examples rather than copying mockup-generated pseudo-CLI syntax.

**Binding requirements** — authoritative constraints from `spec.md` for this milestone. These are requirements to satisfy and verify, not a second task list; the work itself is in the implementation checklist above.

- Rewrite top-level `agentpm --help`: explicitly **“AgentPM — Agent Package Manager”** and **“Agent Package Management”**, summarize package lifecycle, show short working first-use commands. Keep commands/options backward-compatible; no verbose marketing essay in clap help.
- Update relevant help for `init` (new Agent default, Component kinds), `lint` (APDS), `publish` (Registry and link), `install` (dependencies/lock), `new` (Template scaffold), `run` (individual Tool), `harness` (Run an Agent Package with built-in reference Runner), and SDK-facing context. Verify CLI output snapshots/TTY/non-TTY and machine-readable contracts.
- Provide honest next steps for a default `agentpm init` scaffold that is **valid but not necessarily runnable** until a compatible Loop/model setup is present. Preserve existing CLI success publish Registry link; no duplicate dashboard requirement.

**Verification:** the standard four evidence requirements apply — see [Per-milestone evidence requirements](#per-milestone-evidence-requirements). Map the AC and decision IDs listed in this milestone's header.

**Exit gate:** The scoped deliverable works against real source contracts; the binding requirements are satisfied, the standard evidence requirements are met, and Claude review has passed.

## Milestone 15A.1: Republish Example Packages and Templates Under APDS
> **Scope note:** Bring the `agentpm-examples` corpus into the new contract where it actually matters, so scaffolding and first-run demos produce conformant workspaces. Owns new published versions of affected Templates and of the packages used in first-run/featured surfaces. Outside this milestone: mass-republishing the whole corpus, mutating existing published versions, or authoring new example functionality.
> **Implementation notes:**
> - **Why this exists:** `agentpm-examples` currently holds ~150 published manifests (59 tool, 23 agent, 16 loop, 13 skill, 11 template, 10 memory, 10 knowledge, 7 profile), none declaring `standard`. Templates are the blocking case: a Template ships scaffolded manifests *inside* `files_root` (for example `template-packages/support-assistant-workspace/template/agents/answer-drafter.agent.json`), and those files have no `standard`. Once M3B lint is strict, `agentpm new <template>` produces a workspace that immediately fails `agentpm lint` — breaking Journey C end to end. Published versions are immutable, so the fix is **new Template versions**, not edits in place.
> - Scope the republish deliberately rather than sweeping the corpus:
>   - **Required:** every Template whose `files_root` contains manifests — scaffolded files must declare `standard` and the generated workspace must lint clean.
>   - **Required:** the Agent Package selected as the M15B starter, so the Try journey demonstrates a declared-and-verified standard rather than legacy inference.
>   - **Recommended:** at least one package per kind, so Package Health, the standard card and the detail/Explore surfaces have non-legacy examples to render instead of eight "inferred" states.
>   - **Leave alone:** the rest. They stay installable as legacy and are the most realistic corpus for M2A's legacy-inference fixtures and M1A's inventory. Do not republish for tidiness.
> - Template `dependencies` pin exact versions at publish time, so a republished Template must also point at any republished Component versions it should scaffold against; verify the generated workspace resolves and installs.
> - Keep old Template versions published and installable. A consumer on an older CLI must still be able to use the older Template version (see the old-client compatibility requirement in `spec.md` §4).
> - **Dependencies:** M3A/M3B (scaffold + strict lint), M4A/M4B (publish enforcement live, so republished versions are validated server-side). Must complete **before** M16A demonstrates Journey C and before M15B's starter is pinned for docs/screenshots.
> - **Decisions:** D04, D14. **Acceptance:** S2-AC-02, S2-AC-19.

**Implementation checklist**

- [ ] Inventory `agentpm-examples`: every manifest by kind, published version, and whether it is reachable through a Template `files_root`, the first-run journey, or a featured/detail surface.
- [ ] Identify every Template whose `files_root` ships `agent.json`/`*.agent.json` files and list the scaffolded manifests needing `standard`.
- [ ] Add `standard` to those scaffolded template files and bump the Template package version.
- [ ] Verify `agentpm new <template>` output passes strict `agentpm lint` for every republished Template, from a clean directory.
- [ ] Verify the scaffolded workspace still installs and resolves, including exact pinned Component versions.
- [ ] Republish the M15B starter Agent Package with a declared standard and pin the exact version used by docs, screenshots and the homepage card.
- [ ] Republish at least one package per kind so non-legacy Health/standard states are renderable; record which identity/version was chosen per kind.
- [ ] Confirm prior published versions of every touched package remain byte-identical and installable, with digests recorded before and after.
- [ ] Confirm republished versions pass independent Registry validation (M4A) rather than only local lint.
- [ ] Record explicitly which examples were deliberately **not** republished and why, so the remaining legacy corpus is an intentional fixture set rather than an oversight.
- [ ] Update any example README/usage text that shows a manifest without `standard`, limited to AgentPM-owned example content.

**Binding requirements** — authoritative constraints from `spec.md` for this milestone. These are requirements to satisfy and verify, not a second task list; the work itself is in the implementation checklist above.

- A consumer running an **older CLI** against an older Template version must still scaffold and lint successfully; a consumer on the new CLI must get the republished Template. Verify both, and verify the old client's behavior against a *republished* Template matches whatever §4's old-client policy decided.
- A Template that scaffolds an Agent with no Loop must still produce a lint-clean workspace and an honest "not runnable by Harness" explanation, not a lint failure.
- Republishing must not alter any existing version's bytes, digest, signature or attestation, and must not retroactively mark legacy versions as APDS-verified.

**Verification:** the standard four evidence requirements apply — see [Per-milestone evidence requirements](#per-milestone-evidence-requirements). Map the AC and decision IDs listed in this milestone's header.

**Exit gate:** The scoped deliverable works against real source contracts; the binding requirements are satisfied, the standard evidence requirements are met, and Claude review has passed.

## Milestone 15B: Select or Publish a Reliable Low-cost First-run Agent Package
> **Scope note:** Provide one real, small, useful, tested Agent Package for the first-run try path. Outside this milestone: Not AgentPM Developer, not a generic misleading Tool-only demo and not a hardcoded featured registry ID.
> **Implementation notes:**
> - Candidate selection can occur during implementation. Prefer existing genuine examples when reliable, otherwise author one with small composition and low external prerequisites.
> - **Dependencies:** M6 Harness valid path; M11A featured selection; M15A.1 republished examples (the chosen starter must declare a standard); Stage 1 compatibility as deployed.
> - **Decisions:** D14. **Acceptance:** S2-AC-19.

**Implementation checklist**

- [ ] Inventory example repo and Registry candidates against first-run requirements.
- [ ] Choose or create a useful Agent Package with small understandable composition.
- [ ] Ensure Loop is compatible with currently shipped Harness if demonstrating run.
- [ ] Avoid mandatory external SaaS credentials besides clearly explained model provider where feasible.
- [ ] Document required model selection and estimated short-run cost/latency.
- [ ] Write a short starting prompt and representative expected output shape.
- [ ] Install fresh workspace from the public/authorized Registry and verify exact closure.
- [ ] Run through supported Harness TUI or headless path and capture evidence.
- [ ] Test missing provider/credential case and actionable remediation.
- [ ] Pin working package version in tutorial/demo codeboxes.
- [ ] Keep featured selection data-driven and swap example via curation, not JSX rewrite.
- [ ] Record why the example is useful and what it demonstrates about Components.
- [ ] Claude independently follows try flow without unstated setup assumptions.

**Binding requirements** — authoritative constraints from `spec.md` for this milestone. These are requirements to satisfy and verify, not a second task list; the work itself is in the implementation checklist above.

- Select **or create** real beginner-friendly published Agent Package, not the complex flagship AgentPM Developer. Candidate must require few external credentials, have predictable behavior, small genuine composition, short run, low model cost, readable example prompts/outcomes and clear provider/model setup.
- Verify exact dependency closure and Harness command on clean compatible systems (or capture documented environment constraints); keep starter version pinned in tutorial/example instructions to avoid drift; avoid a hidden paid service dependency.
- Test complete beginner try flow end-to-end, fallback for unavailable credentials or provider, CLI init/lint/publish help and all kind-specific init paths.
- Document D14 starter choice/cost/limitations; Claude reviews that a fresh developer can actually follow examples without unlisted prerequisites.

**Verification:** the standard four evidence requirements apply — see [Per-milestone evidence requirements](#per-milestone-evidence-requirements). Map the AC and decision IDs listed in this milestone's header.

**Exit gate:** The scoped deliverable works against real source contracts; the binding requirements are satisfied, the standard evidence requirements are met, and Claude review has passed.

## Milestone 16A: Connect Try, Build and Template Journeys Across the Product
> **Scope note:** Turn homepage, Explore, package/Template details and CLI into three explicit paths with meaningful completion. Outside this milestone: forced account wizard, mandatory Developer assistant and dead destination.
> **Implementation notes:**
> - Prefer contextual onboarding plus perhaps a concise Get Started hub if useful. User chooses path; default experience encourages actually running an Agent Package.
> - **Dependencies:** M12–M15; Stage 1 analytics if deployed.
> - **Decisions:** D15. **Acceptance:** S2-AC-19.

**Implementation checklist**

- [ ] Map Try journey: discover → install → provider/configure → Harness run.
- [ ] Map Build journey: default init → compose → lint/install → test → publish.
- [ ] Map Template journey: choose → `agentpm new` → customize → run via supported surface.
- [ ] Identify exact current routes/commands and first meaningful success for each path.
- [ ] Link homepage CTAs, Explore, package detail and Template detail contextually.
- [ ] Make valid-but-not-runnable default init's next step explicit.
- [ ] Ensure successful CLI publish still provides Registry URL and page loads.
- [ ] Give creators a next step to author/share after successfully running existing package.
- [ ] Use copyable AgentPM Developer prompts only where tool is actually available.
- [ ] Provide manual/Template alternative where Developer isn't ready.
- [ ] Improve no-result and empty states without hiding clear/search controls.
- [ ] Decide and document whether small Get Started hub is worth introducing (D15).
- [ ] Test mobile/accessibility, signed-out and no-permission states and all CTAs.
- [ ] Use Stage 1 metrics only within privacy-approved instrumentation scope.

**Binding requirements** — authoritative constraints from `spec.md` for this milestone. These are requirements to satisfy and verify, not a second task list; the work itself is in the implementation checklist above.

- Map three complete journeys with actual routes and commands: **Try** discover/install/configure/Harness; **Build** init/compose/lint/test/publish; **Template** choose/new/customize/run. Capture first successful outcome and correct next action for each. No forced signup wizard.
- Add contextual entry points/CTAs and small helpful empty states across homepage, Explore, Agent detail, Template detail and relevant completed paths. Choose whether a concise `Get Started` hub adds value (**D15**) while avoiding duplicated docs or long funnel.
- Ensure installing/running naturally leads to how to author or publish one's own package; publishing returns the existing Registry link and, where Card foundation is available, offers appropriate sharing affordance. Never require Developer for authoring.
- AgentPM Developer integration **only when present**: copyable scoped prompts on package pages or no-result searches; hide/replace with direct manual/Template guidance if absent. No imaginary launch CTA.
- Conduct small technical-developer comprehension walk-through: category definition, Agent vs Agent Package, Components, APDS vs Runner readiness, first successful run, build/publish entry. Use Stage 1 analytics if available, with appropriate privacy and no new covert data collection. Record misunderstanding and fix high-severity copy blockers.

**Verification:** the standard four evidence requirements apply — see [Per-milestone evidence requirements](#per-milestone-evidence-requirements). Map the AC and decision IDs listed in this milestone's header.

**Exit gate:** The scoped deliverable works against real source contracts; the binding requirements are satisfied, the standard evidence requirements are met, and Claude review has passed.

## Milestone 16B: Validate Comprehension and Write the Future AgentPM Developer Handoff
> **Scope note:** Check basic category comprehension and preserve the future user-owned creation/publishing adoption loop in a standalone brief. Outside this milestone: Do not implement the full AgentPM Developer wizard or assert category-market fit was proven.
> **Implementation notes:**
> - The user is learning Deploy Empathy and intends to discuss customer conversations separately. This milestone asks for lightweight comprehension evidence, not a full research program.
> - **Dependencies:** M16A journeys; M11B card investigation.
> - **Decisions:** D15, D16. **Acceptance:** S2-AC-20, S2-AC-21.

**Implementation checklist**

- [ ] Prepare a short newcomer comprehension script for Agent Package versus Component/Template.
- [ ] Include APDS versus Harness compatibility versus local runtime readiness questions.
- [ ] Observe ability to find/inspect the full `agent.json` and immutable standard.
- [ ] Observe ability to install/run first package with disclosed model requirements.
- [ ] Observe ability to find Build and Publish paths without founder guidance.
- [ ] Record concrete misunderstandings and fix material UX/copy blockers.
- [ ] Document participation and evidence honestly; if no user sessions occur, mark unverified.
- [ ] Write self-contained AgentPM Developer stage brief with cold-start supply rationale.
- [ ] Describe bring-your-own-idea assisted authoring and optional publication.
- [ ] Describe user-owned ideas rather than tutorial clone generation.
- [ ] Include Registry link + future README Agent Package Card as optional sharing outcome.
- [ ] State AgentPM Developer scope: help build AgentPM artifacts, not general coding.
- [ ] Define meaningful reuse/runs as success signals, not raw publication count.
- [ ] Link D16 README card investigation and outstanding future decisions.
- [ ] Claude reviews handoff for portability to a new chat without this conversation.

**Binding requirements** — authoritative constraints from `spec.md` for this milestone. These are requirements to satisfy and verify, not a second task list; the work itself is in the implementation checklist above.

- Produce a **self-contained future AgentPM Developer-stage handoff brief**: developer brings personal idea, Developer helps assemble APDS artifacts, lint/test/run, optional publish, Registry link and eventual README Card; user-owned ideas, no repetitive tutorial clones, no mandatory publish, meaningful reusability and outcome tracking, Developer restricted to AgentPM-building domain. Explicitly mark as **deferred implementation**.
- Cross-check README Card D16 outcome and document follow-on implementation location (later Stage 2 if approved or subsequent stage) in Developer handoff.
- Claude reviews three journeys, accessibility, meaningful completion, no false promises/dead CTAs, and the preserved future-stage brief.

**Verification:** the standard four evidence requirements apply — see [Per-milestone evidence requirements](#per-milestone-evidence-requirements). Map the AC and decision IDs listed in this milestone's header.

**Exit gate:** The scoped deliverable works against real source contracts; the binding requirements are satisfied, the standard evidence requirements are met, and Claude review has passed.

## Release Band 5: First-run Agent Package Adoption and Developer-stage Handoff
Covered milestones: 15A–16B (including 15A.1).

This gives the CLI explicit Agent Package Manager/Management identity, one real economical starter Agent Package, and three actionable Try/Build/Template journeys across web and CLI. The guided user-owned idea→AgentPM Developer→optional publish→share journey is handed to its future stage, not implemented prematurely.

- [ ] Follow Try from anonymous Explore/detail to real install, configuration and Harness run.
- [ ] Follow Build from default Agent init to APDS lint and publish, preserving existing Registry link.
- [ ] Follow Template from scaffold to advertised supported execution surface, and confirm the scaffolded workspace passes strict `agentpm lint` from a republished Template version.
- [ ] Confirm republished example/Template versions are conformant while their prior published versions remain byte-identical and installable.
- [ ] Review beginner comprehension and deliver a self-contained future AgentPM Developer handoff including README card follow-up.

---

## Milestone 17A: Inventory and Reorganize Docs After Stage 1 Is Fully Complete
> **Scope note:** After the hard Stage 1 completion gate, define a single category-consistent documentation architecture across AgentPM-owned repos. Outside this milestone: Do not begin while any Stage 1 milestone remains incomplete; no author README rewriting.
> **Implementation notes:**
> - This is the final Stage 2 milestone family. Earlier normative APDS files and feature-local notes are allowed, but broad docs/README cleanup waits for the final Stage 1 contract.
> - **Dependencies:** ALL Stage 1 work complete; all prior Stage 2 feature contracts stabilized.
> - **Decisions:** D18. **Acceptance:** S2-AC-22.

**Implementation checklist**

- [ ] Verify Stage 1 complete including M17 SDK compatibility and M18 documentation and migration.
- [ ] Record exact released CLI, lockfile, publish, Python portability, signing and search contracts.
- [ ] Inventory CLI, website/Registry, Node/Python SDK, examples and owned READMEs.
- [ ] Map docs Introduction/Quickstart/navigation and current Tool-first dead ends.
- [ ] Identify user-authored Registry READMEs and explicitly exclude modification.
- [ ] Propose category-first docs IA with Try, Build, Templates, Components and reference.
- [ ] Ensure APDS immutable spec remains canonical rather than paraphrased alternate source.
- [ ] Plan route changes/redirects and protect existing docs deep links.
- [ ] List all command snippets needing executable proof and package version pinning.
- [ ] Review draft IA with Claude before cross-repo editing begins.

**Binding requirements** — authoritative constraints from `spec.md` for this milestone. These are requirements to satisfy and verify, not a second task list; the work itself is in the implementation checklist above.

- Verify Stage 1 fully complete and record which final CLI/Registry/lock/provenance/search/SEO contracts actually shipped. If not fully done, **do not start** comprehensive rewrite; keep normative APDS docs and functionality notes already written.
- Inventory all AgentPM-controlled user-facing docs and README sources, including CLI repository, Registry/website, SDKs, examples, homepage onboarding, docs Introduction/Quickstart/IA and in-repo READMEs. Do not rewrite package-author READMEs in the hosted Registry.
- Restructure docs navigation around: what is an Agent Package and why it matters; install/run first; build/publish; Components (six kinds); Templates; APDS schema/semantics and conformance; Registry/versions; Harness Runner and model prerequisites; Node/Python SDK integration; compatibility/Health/security and truthful limitation statements.

**Verification:** the standard four evidence requirements apply — see [Per-milestone evidence requirements](#per-milestone-evidence-requirements). Map the AC and decision IDs listed in this milestone's header.

**Exit gate:** The scoped deliverable works against real source contracts; the binding requirements are satisfied, the standard evidence requirements are met, and Claude review has passed.

## Milestone 17B: Rewrite Introduction, Quickstart, Guides and AgentPM-owned READMEs

> **Scope note:** Publish one consistent Agent Package-first explanation across docs, help, website and repository-owned READMEs. Outside this milestone: Do not replace technical depth with marketing blurbs or claim universal Runner support.
> **Implementation notes:**
>
> - Quickstart should show a real first Agent Package installation and Harness run before making users learn all six Components. Preserve alternate authoring/Template paths.
> - **Dependencies:** M17A approved IA and completed Stage 1 prerequisite.
> - **Decisions:** D18. **Acceptance:** S2-AC-22.

**Implementation checklist**

- [ ] Rewrite Introduction around problem, Agent Package definition, Manager/Registry/APDS/Runner roles.
- [ ] Rewrite Quickstart with working starter install/configure/Harness commands.
- [ ] Provide manual Build path with `agentpm init` new Agent default and optional Loop.
- [ ] Provide Template starter with `agentpm new` semantics, not Agent install conflation.
- [ ] Update Component guides for Tool, Skill, Knowledge, Memory, Profile, Loop roles.
- [ ] Update APDS authoring/schema/semantics links and version support info; distinguish published normative semantics from AgentPM-specific CLI/Registry/Harness implementation guidance.
- [ ] Update publish, install/lock, legacy migration, frozen and release provenance guidance.
- [ ] Update Harness Runner and Node/Python SDK integration help with realistic prerequisites.
- [ ] Update CLI, Registry, SDK, examples and website owned README positioning.
- [ ] After the final docs rewrite, regenerate `agentpm-examples/knowledge-packages/agentpm-docs` from `agentpm-api/docs/v0.1` using its existing corpus/embedding pipeline; refresh copied sources, chunks, provenance, vectors and CLI-generated Knowledge metadata together. Bump the package version consistently, including the prep script's version constant, validate with `agentpm lint`, `agentpm knowledge build`/`inspect` and a publish dry-run, and arrange a new published version. Never hand-edit the vendored pages or replace an existing published version.
- [ ] Use Agent Package for artifact, Agent for runtime entity; no blind global replace.
- [ ] Preserve existing technical references and backwards-compatible docs redirects.
- [ ] Cross-check every reference link and current CLI syntax with actual shipped code.
- [ ] Separate objective Package Health facts from trust/quality/compatibility promises.

**Binding requirements** — authoritative constraints from `spec.md` for this milestone. These are requirements to satisfy and verify, not a second task list; the work itself is in the implementation checklist above.

- Rewrite Introduction and Quickstart to show a real Agent Package-first path instead of leading with Tool creation/use. Provide alternate manual build and Template routes, clear `init` default Agent Package, package-vs-component taxonomy and exact CLI commands.
- Unify all AgentPM-controlled README positioning with current website and CLI help: Agent Package Manager; Agent Package Management; Agent Packages; Components; Templates; APDS; Registry; Harness reference Runner; portability without universal interoperability claim.
- Cross-link immutable versioned APDS schema+semantics/fixtures and Registry `/standards/...` page. Do not make README/docs another moving authority for normative semantics; normative contract remains version-pinned.
- Update guides for publication enforcement, explicit/legacy standards, lockfile v4 + frozen behavior, Python target compatibility, provenance verification, Package Health signals, and common errors based on actual Stage 1+2 shipped behavior.

**Verification:** the standard four evidence requirements apply — see [Per-milestone evidence requirements](#per-milestone-evidence-requirements). Map the AC and decision IDs listed in this milestone's header.

**Exit gate:** The scoped deliverable works against real source contracts; the binding requirements are satisfied, the standard evidence requirements are met, and Claude review has passed.

## Milestone 17C: Run Final End-to-end Documentation and Cross-product Verification

> **Scope note:** Prove every public first-use example actually works and all Stage 2 contracts agree after the complete docs rewrite. Outside this milestone: unexecuted tests claimed as passed; no changes to settled product semantics just for wording.
> **Implementation notes:**
>
> - The release gate spans Rust CLI, Python Registry, Node/Python SDKs, Next website and real starter. Claude reviews before marking Stage 2 complete.
> - **Dependencies:** M17A/B; entire Stage 1 and Stage 2 completion.
> - **Decisions:** D01–D21 closure. **Acceptance:** S2-AC-01, S2-AC-02, S2-AC-03, S2-AC-04, S2-AC-05, S2-AC-06, S2-AC-07, S2-AC-08, S2-AC-09, S2-AC-10, S2-AC-11, S2-AC-12, S2-AC-13, S2-AC-14, S2-AC-15, S2-AC-16, S2-AC-17, S2-AC-18, S2-AC-19, S2-AC-20, S2-AC-21, S2-AC-22.

**Implementation checklist**

- [ ] Run actual docs commands for install/try, build/lint/publish and Template flows.
- [ ] Check missing credential and no-Loop valid-but-not-runnable documentation.
- [ ] Cross-verify docs and homepage code boxes reference the same selected version.
- [ ] Verify the newly published `agentpm-docs` Knowledge package in a clean consumer: copied sources and retrieved chunks reflect the final docs (including Tool scaffolding and standard selection), vectors/counts/provenance/build hashes agree, and the prior published version remains byte-identical and installable. Record the new version, source revision and query evidence.
- [ ] Verify all APDS standard/manifest/fixture URLs refer to immutable v1.0.0.
- [ ] Recheck old/new legacy publication and install behavior after all migrations.
- [ ] Verify eight kind detail views, Health states and anonymous/private visibility.
- [ ] Verify Stage 1 search/SEO/lock/provenance regressions remain green.
- [ ] Capture final screenshots and ensure current brand palette/elevation preserved.
- [ ] Map all S2-AC criteria to tests/screenshots/report links.
- [ ] Close all D01–D21 with written reviewed decisions, not ambiguous unresolved choices.
- [ ] Provide Claude independent review package and resolve blocking findings.
- [ ] Record legitimate scope deferrals, future Developer handoff and open category-learning questions.
- [ ] Do not declare Stage 2 complete if Stage 1 hard gate or integration tests are outstanding.

**Binding requirements** — authoritative constraints from `spec.md` for this milestone. These are requirements to satisfy and verify, not a second task list; the work itself is in the implementation checklist above.

- Ensure agentpm-examples golden starter, README snippets, homepage codeboxes, docs and CLI help all pass exact-command smoke tests (with appropriate sandbox/test credentials); distinguish `agentpm run` Tool from Harness Agent.
- Verify all public links/canonicals, README badges/Card status, docs version consistency, searchability, accessibility and no regressions from final naming pass. No blind filewide Agent→Agent Package replacements.
- Have Claude independently review docs from a developer's perspective, verify tests/evidence and record unresolved follow-up topics instead of implying they're shipped.

**Verification:** the standard four evidence requirements apply — see [Per-milestone evidence requirements](#per-milestone-evidence-requirements). Map the AC and decision IDs listed in this milestone's header.

**Exit gate:** The scoped deliverable works against real source contracts; the binding requirements are satisfied, the standard evidence requirements are met, and Claude review has passed.

## Release Band 6: Final Documentation, Full Integration and Independent Approval

Covered milestones: 17A–17C.

This gives the finished AgentPM ecosystem one accurate Agent Package-first documentation story across docs, CLI, Registry, SDKs and owned READMEs, based on stabilized Stage 1 and Stage 2 contracts. The whole stage is only complete after the high-risk lifecycle and UX matrices pass and Claude has reviewed the evidence.

- [ ] Document that Stage 1, including its M18 docs work, was fully complete before M17A began.
- [ ] Execute actual install/run/build/Template documentation commands, not just link checks.
- [ ] Map S2-AC-01–22 and D01–D21 to concrete evidence and reviewed decisions.
- [ ] Resolve blocking Claude findings and record deferred category-market questions and future-stage work.

---

## End-of-stage signoff and follow-on coordination

- [ ] Ensure all six numbered release bands have complete evidence and no unresolved blocking security/reproducibility failure.
- [ ] Validate the immutable APDS source files, schema compatibility path, Rust/Python fixture parity, Registry enforcement and accurate legacy status.
- [ ] Validate the full user mental model: Agent Package, Component, Template, APDS, Manager, Registry, Runner.
- [ ] Hand off future **reverse dependency discovery** as a deferred Registry idea, not an incomplete Stage 2 task.
- [ ] Hand off the **AgentPM Developer guided creator journey** separately, with optional publish/share and README card investigation notes.
- [ ] Retain first-run and customer-comprehension questions for Stage 5 user learning; no claim that category-market fit is established.
- [ ] Deliver Codex implementation records and Claude independent review findings with evidence links and unresolved issues clearly marked.
