# Review Checklist

**Stage 2: Category-Enabling Hardening — Make the Agent Package Real**  
**Audience:** Claude Code as independent implementation reviewer, with Codex as implementer.  
**Primary contract:** [`spec.md`](spec.md); **implementation order:** [`tasks.md`](tasks.md); **required tests/evidence:** [`test-plan.md`](test-plan.md).

> **Reviewer charter:** Do not just ask whether the feature works on a demo package. Challenge whether the work **faithfully preserves the agreed category/portable APDS contracts**, integrates with Stage 1, is genuinely reproducible and verifiable, and does not silently broaden the project. The spec's REQUIRED constraints are binding. Its PREFERRED implementations are **strong opinions, not architectural orders**; assess Codex's alternatives on evidence. Never accept fabricated mockup metadata as production fact.

## Review protocol and output

For each milestone/release band, Claude should return:

1. **Verdict:** approve / approve with nonblocking follow-ups / changes required / blocked by prerequisite.
2. **Blocking findings first**, each with severity, impacted `S2-AC-xx`, milestone, reproducible file/line/path and failing test or manual reproduction. Explicitly separate spec violation from optional recommendation.
3. **Implementation discretion review:** which D01–D20 decision records were due, whether preferences were considered, and why the chosen alternatives are better or worse.
4. **Regression/Stage 1 impacts:** all affected interfaces, migrations, feature-flag/cutover and rollout safety.
5. **Evidence:** executed tests/CI status; fixtures/sample payloads; screenshots/URLs; provenance claims and unavailable checks.
6. **Final status:** accepted contracts, residual risk, follow-up tasks and whether any issue needs Zack's decision.

Do not mark a checklist item passed solely because Codex asserted it. Cite code/test evidence or document *not verified*. If the current repo differs from the spec's historical baseline, inspect the diff and adjust recommendations without weakening AGREED outcomes.

---

### Reviewer navigation: six numbered release bands, 37 scoped milestones

The implementation is split into **M1A–M17C** (37 independently reviewable units), grouped into release bands 1–6. Review the **scope note**, original required edge cases, verification evidence and dependencies of each milestone rather than treating the old broad M1–M17 labels as single PRs.

| Band | Milestone focus | Claude must challenge |
|---|---|---|
| **1** | M1A contract inventory; M1B canonical schema; M1C semantics; M2A fixtures; M2B Rust; M2C Python; M3A init; M3B lint; M4A publish gate; M4B legacy rollout | Is APDS faithful to actual schema/Phases 6–7? Can an independent custom Registry client bypass the gate? Does immutable legacy data survive? |
| **2** | M5A lock metadata; M5B frozen closure; M6A support/readiness; M6B exact-version SDK/Runner | Does a clean replay preserve exact kind/version/digest? Is a valid no-Loop Agent still conformant? Do both SDKs and headless see truthful readiness? |
| **3** | M7A universal evidence; M7B kind-specific Health; M8A standard reference; M8B cards; M9A composition; M9B phases/actions; M10A specialized detail; M10B Health UI | Does every UI assertion have authoritative evidence? Does Overview show exact direct/transitive graph and keep specialized tabs? |
| **4** | M11A featured data; M11B Card/embed investigation; M12A/B homepage; M13A/B Explore/namespace; M14A/B category/pricing/SEO | Does existing brand survive? Is Featured data-driven/private-safe? Are Stage 1 ranking, pins, search URLs and pricing intact? |
| **5** | M15A category CLI help; M15B starter Agent; M16A journeys; M16B comprehension/Developer handoff | Can a new developer really run a low-cost package? Are all three journeys complete? Is future Developer work documented but not incorrectly shipped? |
| **6** | M17A docs gate; M17B rewrite; M17C final verification | Was Stage 1 entirely finished before docs? Do working examples and all contracts agree across repos? |

## Contract surfaces

### 1. Category/terminology and product meaning

- [ ] **Agent Package** means `kind: "agent"` complete authored top-level system; six other Component kinds and Templates remain distinct in user-facing descriptions, while internal package-kind modeling is intact.
- [ ] **Agent** remains appropriate for acting runtime entity; packaged/versioned/installed artifact uses **Agent Package** where relevant. No blind global renaming in routes, manifest names, SDK APIs or existing serialized kinds.
- [ ] **AgentPM** is explicitly an **Agent Package Manager**; CLI top-level help literally includes **Agent Package Manager** and **Agent Package Management**.
- [ ] APDS is structural/semantic *definition*, not release provenance, security certification, catalog popularity, Harness support, local readiness or an assertion of third-party Runner interoperability.
- [ ] Agent Package Management is presented as a working category/product hypothesis rather than an unsupported claim of industry standardization or universal adoption.
- [ ] Portability/compatibility boundaries honest: AgentPM is a layer, not a mandatory framework/host/model/Runner replacement.

### 2. APDS source of truth and versioning — strongest review priority (M1A–M2C)

- [ ] APDS 1.0.0 is anchored in the **existing `agentpm.manifest.schema.json`**, not a second invented format. Review schema diff field-by-field: eight kinds, `oneOf`, `$defs`, refs, common fields, per-kind required fields.
- [ ] Exactly one authoritative v1.0.0 schema contract exists. Existing path is compatibility/generated/alias as designed; CI prevents drift. Versioned `$id` stable, editor `$schema` distinct from manifest `standard` declaration. No live mutable `main` GitHub schema fetch in normal authoring/publishing.
- [ ] `standard: {"id":"agentpm","version":"1.0.0"}` has correct strict schema and matches server/CLI/Runner interpretation. Artifact `version`, lockfile version, APDS version, Harness protocol/Runner version not conflated.
- [ ] Manifest filename `agent.json` still common to all kinds; only `kind:agent` means Agent Package. Local `name` is unscoped as in the actual schema; namespaced package identities are references/Registry identifiers. Mockup pseudo-JSON (`"apds"` or `"type"`) hasn't leaked into actual code.
- [ ] New Agent with **no Tools**, **no Loop**, **no Profiles** is intrinsically valid, with meaningful non-whitespace description; Tool/Knowledge/Memory/Profile/Loop/Skill/Template shapes remain valid. No unjustified breaking change to established manifests.
- [ ] Normative APDS `README.md` states scope, release immutability, version policy, implementation roles, glossary, Registry independence and bounded portability.
- [ ] `semantics.md` is actually **normative**: consistent MUST/SHOULD/MAY language, stable semantic rule IDs, applicability, examples, rationale, check level, failure or advisory level and fixture mapping. Not a marketing document or code mirror without normative meaning.
- [ ] Semantic chapter/rules cover **all** agreed domains:
  - [ ] identity/reference/version semantics; direct/transitive dependency graph and kind correctness;
  - [ ] additive Agent global + phase bindings, versionless bindings resolved to pinned package versions, phase names checked against resolved Loop;
  - [ ] Skill-declared Tools inherit Skill scope, globally and per-phase, without duplicate Tool bindings;
  - [ ] Loop tool/knowledge/memory read/write policy constrains bindings, plus outcomes, implicit completion, transitions, terminal targets, limits and error-policy declaration;
  - [ ] Profiles identity/behavior composition/hints are advisory; they cannot grant capabilities;
  - [ ] Knowledge context/vector data/retrieval contracts independent of backend;
  - [ ] Memory scopes, record types, document/collection/sequence spaces, operations and interval/record-count/capacity/external triggers; globally bound operations participate run-wide subject to triggers, phase operations only in phase, operations may target not-directly-bound spaces, declarative Blueprint doesn't execute/persist itself;
  - [ ] Templates scaffold via `new` rather than constitute Runner-executable Agent Packages, including actual execution-surface vocabulary;
  - [ ] standalone vs resolved validation vs Runner interpretation and supported/unsupported/incomplete/conformant/nonconformant distinctions.
- [ ] Authoritative semantics and expected conformance fixture outcomes are immutable for the released version; no semantic drift under same version. Version rule IDs are stable and not reused after changing meaning.
- [ ] No accidental normative requirement for specific provider/model/prompt template/Harness TUI/MCP host/approvals UI/private Registry subscription/full orchestration language.
- [ ] Claude explicitly cross-checks APDS rules with existing Phase 6/7 semantics rather than assuming the current implementation or planning prose is automatically accurate. Flag genuine disagreements before v1.0.0 freeze.

### 3. Validators and Registry independent enforcement (M2A–M4B)

- [ ] Rust CLI and Python Registry validate **independently** against same pinned contract and shared fixture suite; one doesn't call or blindly trust the other for conformance. CI parity failures actionable.
- [ ] Fixtures cover all kinds and actual semantic edge cases, both positive and negative, with stable expected rule/status/path outcomes; incomplete unresolved context distinct from false passing and false failures.
- [ ] Strict new authoring missing/unknown standard fails clearly. If legacy authored compatibility mode exists, it is explicit and cannot bypass new publishing enforcement.
- [ ] CLI `init` defaults to Agent Package with truthful default name, generated APDS declaration, safe JSON serialization, valid optional empty dependency structure; all explicit kinds continue to lint, especially Stage 1 Tool scaffold.
- [ ] Registry independently inspects **staged embedded manifest bytes** and matches init/finalize metadata including kind/name/version/standard before making release visible. A raw/custom API client cannot bypass validation.
- [ ] Every new finalized version—even new version of a legacy identity—must be compliant. Already published old versions stay unchanged/installable; no digest/signature rewrite or fake retroactive conformance.
- [ ] Cutover/in-flight upload policy owned by Registry and tested, old client upgrade path readable, rollback/cleanup idempotent, security/ACL/signing/malware/release scanning unaffected. No endless permissive flag left in production.
- [ ] Validation result states aren't collapsed: malformed structure, definitively invalid resolved relationship, not evaluated for missing graph, unsupported version, inferred legacy state and independent security failure are distinguishable.

### 4. Resolve/install/lock/Runner contract (M5A–M6B)

- [ ] Standard declaration/effective interpretation **and provenance** consistent from manifest → Registry → resolved graph → CLI install → lockfile → Harness. Actual version-specific embedded manifest is authoritative; API cannot silently override it.
- [ ] Existing Stage 1 `agent.lock` v4 retained if feasible; forward-unknown-version/field protection honored. A v5 bump needs convincing migration and reviewer evidence, not simply convenience.
- [ ] `--frozen` correctly handles direct **Agent and Skill roots**, complete transitive closure, version/semver satisfaction, dependency kind, selected artifact targets, exact SHA/release identity and clean-workspace reconstruction. No checksum/constraint bypass.
- [ ] Empty dependency Agent install/resolution **succeeds** and doesn't send a failing empty list to Registry. Test normal and frozen.
- [ ] Harness declares actual supported APDS IDs/versions, validates selected graph, and exposes capabilities across CLI, TUI, headless machine/preflight and Node/Python SDK surfaces using existing protocols where possible.
- [ ] APDS-conformant Agent without Loop is recognized as valid **but not Harness-runnable**; unknown standard distinct; missing model/provider in non-TTY is not optimistic Ready.
- [ ] Multiple locked versions of same Component in workspace cannot be selected by ambiguous name-only lookup. Binding to Skill inheritance and Memory operations honors the correct exact Component version and Loop constraints.
- [ ] Harness inner loop, phase results/terminal handoff, approvals, tracing, hooks, MCP and provider services not accidentally redesigned.
- [ ] Top-level Agent description prompt A/B **conducted and documented**, even if no prompt change adopted; this is not an APDS requirement.

### 5. Health contract and release data semantics (M7A–M10B)

- [ ] Health contract clearly separates **identity-level** name/publisher/stars/installs and **version-level** APDS/provenance/integrity/signature/attestation/scan/compatibility/deprecation.
- [ ] Universal + kind-specific evidence model supports all eight kinds, including *not applicable* for irrelevant checks. Tool OS/architecture does not become generic Loop/Profile “health.”
- [ ] Every pass/fail/unknown/advisory state backed by correct authoritative source, provenance, verification or absence; status labels differentiate `verified`, `failed`, `incomplete`, `unknown`, `not applicable` and advisory where relevant.
- [ ] No universal quality score, stars-as-trust, invented suitability/certification, “signed means safe”, “malware scan clean means safe”, “APDS conformant means runnable”, “compatibility declaration means runtime-enforced” claim.
- [ ] Legacy inferred standard never shown as independently verified; signed artifact not shown as cryptographically verified unless actually checked.
- [ ] Existing Security detail evidence retained, now discoverable through appropriately scoped Package Health; no loss of target-specific artifact evidence from Stage 1.

### 6. Public API/routes/UI and compatibility

- [ ] Changes to schemas/manifests/lockfiles/CLI help/machine events/SDK payloads/Registry resolve/publish/Health DTOs are precisely scoped; backward-compatibility, rollout sequence and version handling documented. No public route change solely to use “Agent Package” label.
- [ ] Versioned standard route uses pinned trusted source and correct canonical links; a malicious manifest cannot direct the app to fetch arbitrary schemas or serve untrusted remote content as normative.
- [ ] All UI renders genuine package/Registry data with authorized visibility; no invented versions, timestamps, download/star counts, conformance or third-party Runner compatibility.

---

## Correctness

### 7. Agent Package detail page (M8A–M9B)

- [ ] Default Overview is **human-readable composition and execution structure first**, raw JSON accessible but visually secondary; screenshots and navigation confirm this is a genuine information-hierarchy change.
- [ ] Direct vs transitive/Skill-inherited dependencies are distinguished, exact resolved Component identity/version links valid, graph cycles/duplicates/unknown metadata handled safely.
- [ ] Global/phase bindings and Loop graph access represented accurately. UI does not falsely linearize arbitrary graph loops or claim an authored Memory operation automatically executed.
- [ ] No-Loop/no-dependency APDS-valid Agent gets useful, honest empty state, not a red invalid badge or fictitious Loop.
- [ ] Header and code examples distinguish **Install**, conditional **Run with Harness**, **Load via SDK** with exact current syntax and selected package version, plus prerequisites where necessary.
- [ ] Distinct functional links: **View `agent.json`** for selected release and **View APDS specification** for supported immutable standard; conformance displayed separately from declared version.
- [ ] Existing Bindings, Examples, Readme and Security/Health tabs remain; published README content treated as author-owned.

### 8. Component/Template detail page preservation (M10A–M10B)

- [ ] Tool: execution runtime, I/O, target compatibility, install/run semantics still inspectable.
- [ ] Skill: entrypoint, references, scripts, declared Tools and availability semantics still inspectable.
- [ ] Knowledge: mode, corpus/doc counts, embedding/index/retrieval/provenance, inspect/query behavior preserved.
- [ ] Memory Blueprint: scopes, spaces, records, lifecycle/operations/triggers, governance/capacity/retention preserved.
- [ ] Instruction Profile: identity, objectives, principles, audience, communication, boundaries/constraints, hints preserved.
- [ ] Loop: graph, phases, objectives/access/outcomes/transitions, checkpoints, limits and error policy preserved.
- [ ] Template: scaffold use case, files/variables/stack/dependencies/entrypoints/execution surfaces, bootstrap/next steps preserved; not mislabeled Agent Package.
- [ ] All kind pages can expose APDS and truthful relevant Health; shared shell and kind-specific views coexist rather than flattening into one generic marketing layout.

### 9. Homepage, Explore, namespaces and visual identity (M11A–M14B)

- [ ] Homepage hierarchy addresses developer problem → complete Agent Package → AgentPM Manager/Registry/Harness role → Components/Templates → get started, with product as hero rather than standards body. Restructures repetitive Feature Inventory content.
- [ ] One **real, curated Agent Package** near hero explains purpose, composition, version, install/Harness where supported; selection is **data-driven**, not a package ID written into JSX. Missing/invalid featured content has healthy fallback.
- [ ] Featured != Trending. Namespace pins can feature any artifact kind; none leaks private/unpublished content. Stage 1 relevance/search sorting/stars/pagination remain deterministic and correct.
- [ ] Explore navigation/facets/cards visibly group **Agent Packages, Components, Templates, Namespaces** without changing backend kinds; meaningful kind-specific card facts and no-query Explore experience work. Text queries never arbitrarily pin Agents above relevant Components.
- [ ] Namespace page balances curated publisher content with search/filter/sort and category hierarchy; recent activity, signing text, filters and member/settings behavior remain valid.
- [ ] Reusable Agent Package Card supports purpose/version/composition/standard and evidence appropriately; cards not confused with A2A Agent Card. README embed feasibility investigation D16 includes pinned/latest/auth/caching/OG and explicit implement/defer decision.
- [ ] Current site palette, typography, elevation, floating cards, icon language and overall style are the visual source of truth. Mockup hierarchy useful, but **do not copy old-flat blue/white design, invented numbers or inaccurate JSON/other-runner badges**. Review desktop and responsive/mobile screenshots against actual production site.
- [ ] Category explainer supports AgentPM, APDS versioned technical route distinct; H1/SEO/canonical/OG/robots/sitemap driven by Stage 1 infrastructure. Pricing messaging accurate without billing changes.

### 10. CLI and developer adoption (M15A–M16B)

- [ ] CLI top-level help explicitly identifies AgentPM as **Agent Package Manager**, names **Agent Package Management** and shows working first-use commands.
- [ ] `agentpm init` default Agent Package and explicit Tool/other kinds; `agentpm new` Template scaffolding; `agentpm run` Tool; `agentpm harness` built-in Agent Package Runner; `install`, `lint`, `publish` aligned without renaming public flags or destabilizing script consumers.
- [ ] **Three usable journeys:** Try/install/configure/Harness run; Build/init/compose/lint/test/publish; Template/new/customize/run. Each has a real start and reasonable next action, not merely decorative flow badges.
- [ ] Simple starter Agent Package selected/built/pinned/tested, with low credential/cost burden, real provider/model setup, expected result and no hidden dependency; not flagship AgentPM Developer.
- [ ] `publish` still supplies Registry detail URL on success; no unnecessary success dashboard scope expansion.
- [ ] AgentPM Developer prompt/CTA conditional on real published/accessible functionality; nonmandatory alternative always available, scoped to AgentPM artifact building rather than general coding.
- [ ] Self-contained **future AgentPM Developer stage** handoff explicitly documents user-owned idea → create/test → optional publish → share, realistic adoption/reuse loop and Card idea. No wizard implemented during Stage 2.
- [ ] Lightweight developer comprehension check genuinely probes category and first run; no false assumption that this proves market-category fit.

---

## Regressions

### 11. Stage 1 boundaries — mandatory at every release band

- [ ] Verified current Stage 1 **actual merged/active code**, not merely planned specs, before editing shared surface.
- [ ] No duplicate rewrite of Stage 1 Explore correctness, search facets/relevance, stars/trending, namespace pins, shared detail shell, technical SEO, CLI error styles, telemetry, Python portability/locking, multi-artifact release/signatures/scan, or billing instrumentation.
- [ ] Stage 1 lockfile v4/forward guards, multi-artifact release integrity/provenance and their migration paths still pass after APDS additions.
- [ ] Stage 1 discovery pagination/URL state and private-result visibility remain correct after category hierarchy/featured changes.
- [ ] Stage 1 author signatures/Registry attestations/malware scanning not mislabeled or weakened by generic APDS Health.
- [ ] Stage 1 Tool scaffolding and template Harness execution surface not broken by changing init default Agent.
- [ ] Stage 1 CLI error/success/machine format remains functional; help changes don't inadvertently alter flags/subcommands.
- [ ] Stage 1 technical SEO/canonical/indexing works after new category/reference routes; no accidental crawl of search queries.
- [ ] **Final-doc gate:** Comprehensive documentation and AgentPM-owned README overhaul was **the last Stage 2 milestone** and started **after Stage 1 was entirely complete**, including Stage 1 M17. Normative APDS technical docs were allowed earlier; do not confuse them with final docs.

### 12. Legacy, privacy and adverse states

- [ ] Historic published versions keep exact immutable bytes/digests/signatures and remain installable; legacy inferred APDS labeled; no old package forced into a silently reissued new release.
- [ ] Unsupported standard, invalid schema, incomplete dependency graph, missing Runner/Loop, no model config, mismatched lock/semver/digest and empty dependency graph all produce correct differing behavior.
- [ ] New-release server gate cannot be bypassed via raw HTTP/custom CLI; staging mutation happens before final publish and failure cleans up atomically.
- [ ] Private/deleted/unpublished curated entries, dependency metadata, Health evidence, namespace pins and OG images cannot be leaked via anonymous pages or APIs.
- [ ] No blanket claims of Runner portability, malware-free safety, runtime compatibility, or verified authors without actual supporting proof.
- [ ] Live user-provided data is not inserted into client HTML without proper escaping; authored README and other untrusted content still rendered safely.

---

## Tests and verification

### 13. Evidence is sufficient, not just present

- [ ] `test-plan.md` required automated and manual checks were mapped to actual test names and commands; any deviation explained. No “verified” checkbox only based on mockup render or code review.
- [ ] Rust CLI and Python Registry run full same APDS conformance fixture corpus, both expected pass/fail/status/rule ID outcomes, and suite is CI-enforced.
- [ ] Registry release security and migration tests include malicious/custom client, mismatched staged manifest, queued session cutover, new version of legacy package, multi-target Tool and immutable historic artifacts.
- [ ] Frozen lock tests replay direct Agent/Skill roots, full transitive graph/semver/integrity and zero-dependency Agents from **clean** workspaces.
- [ ] Harness tests check no-Loop, unsupported APDS, missing provider in noninteractive headless, multiple locked versions, restricted phase and Machine/SDK parity.
- [ ] Package Health tests cover **eight kinds**, release switch and all negative/unknown/inferred/advisory evidence states; no global quality score.
- [ ] Web tests use Stage 1 feature/filter/search implementations, private visibility, data-driven curation, no-query Explore, responsive/mobiles, broken/missing featured entities, versioned standard/raw manifest links, category SEO.
- [ ] Full Try/Build/Template onboarding flows demonstrated against actual test/staging packages, with reasonable cost/prerequisites; docs examples smoke-tested **after** Stage 1 finished.
- [ ] Tests actually executed and passing in relevant CI or reproduced locally; evidence shows commands/status, environment, tests not run and blockers. New docs mention actual commands, not invented ones.

### 14. Final acceptance inventory — every spec item must be accounted for

| Requirement | Expected reviewer evidence |
|---|---|
| **S2-AC-01–04** | APDS bundle/version/schema diff/semantics, shared fixture matrix, Rust+Python conformance results, all-kind init/lint |
| **S2-AC-05** | Registry independent staged-bytes verification, rollout/backward-compat, negative raw-client test |
| **S2-AC-06–07** | APDS lock v4 and provenance, frozen transitive replay, empty deps, digest/semver guard |
| **S2-AC-08–09** | Harness capability/readiness/SDK parity, no Loop+non-TTY model negative test, description experiment report |
| **S2-AC-10–12** | Eight-kind Health applicability/source matrix, Agent composition, standard vs manifest link, legacy-status screenshot |
| **S2-AC-13–17** | Real-data homepage, detail/card UI, Explore/namespace regression/curation change, category reference, SEO/pricing consistency |
| **S2-AC-18–21** | CLI help literals + correct commands, starter try/build/template flows, Developer future-stage brief, lightweight comprehension outcomes |
| **S2-AC-22** | Proof all Stage 1 milestones complete before final docs pass; links/snippets and README/Introduction/Quickstart alignment |

---

## Pattern adherence

### 15. Architecture and scope quality

- [ ] Reused existing repo schemas/CI/validators/DTOs/cards/icons/Stage 1 APIs where possible. New standardized layer is small, explicit and worth its operational burden.
- [ ] No bespoke semantics DSL, arbitrary schema federation, framework-independent execution engine rebuild, general AI search, evaluation/quality scoring, social marketplace, editorial CMS or unrelated feature creep.
- [ ] Existing schema source remains single authority; standard is portable without Registry; Runner can declare support without APDS prescribing implementation.
- [ ] Registry Python validator is independent at evaluation time yet uses same pinned artifacts and fixtures. No “frontend-only” or “client-only” security/validity check.
- [ ] Data-driven featured catalog selection minimizes server and authoring overhead; editor can reconfigure without React code change; privileges enforce visibility.
- [ ] Health architecture avoids duplicating Stage 1 release integrity computation and avoids requiring Tool metadata on unrelated kinds.
- [ ] Site design extends established visual system, not a detached rebrand; mobile diagrams and source availability make sense.
- [ ] No claim “one command and runnable” for valid no-Loop Agent or a starter without configured provider. Documentation teaches distinctions plainly.
- [ ] Every **D01–D20** preferred approach considered; if diverged, Codex's rationale and tradeoffs are specific and reviewer-approved. Preferences do not become unreviewed product-scope changes.

---

## Notes for reviewer

### 16. D01–D20 design-decision review register

Claude should explicitly mark each decision as *documented / evidence adequate / needs changes*. These are design outcomes, not questions to send back to Zack reflexively.

| ID | Preferred starting point | Claude should specifically challenge |
|---|---|---|
| **D01** | One normative `semantics.md` with stable rule IDs and examples | Is the meaning testable, not duplicative, and accurate for Skill/Memory/Loop semantics? |
| **D02** | `cli/standards/agentpm/1.0.0` authoritative existing-schema version; old schema path compatibility | Can old and new diverge; moving schema ID; packaging/no-network path? |
| **D03** | Fixture dirs valid/invalid/resolved + machine expected outcomes | Do Rust/Python validate independently and cover incomplete/unsupported? |
| **D04** | Strict new-authoring standard selection, optional explicit legacy migration | Is there a hidden permissive lint/publish bypass or hostile old-local workflow? |
| **D05** | Server staged-archive validation at finalize, carefully staged enforcement | Can custom client publish invalid bytes; queued sessions/rollback safe? |
| **D06** | Retain Stage 1 lock v4 | Forward guards, compatible serializer and release provenance correctness? |
| **D07** | `/standards/agentpm/1.0.0` pinned catalog | Any arbitrary author URL, moving normative content or bad canonical? |
| **D08** | Problem-first, real Agent Package hero, existing layered visual design | Is the main product actually explained and existing brand preserved? |
| **D09** | Shared Card identity/composition primitives and variants | Are components reusable without hiding domain differences or fictional metadata? |
| **D10** | Config or Registry-backed featured selection | Does selection/order change without JSX changes; privacy/trending separation? |
| **D11** | Curated no-query Explore, normal query relevance unchanged | Are relevance/pagination/private visibility all protected? |
| **D12** | Universal per-version Health evidence + kind-specific extensions | Can each kind avoid meaningless checks, and all claims trace to authority? |
| **D13** | Run description-prompt experiment; no standard mandate | Are examples/cost/output comparisons real enough to justify keep/defer? |
| **D14** | Small low-cost runnable starter, not AgentPM Developer | Is it genuine, published/available, cred/setup clear and reproducible? |
| **D15** | Contextual pathways, possible lightweight Get Started hub | Are all three journeys actually navigable without decorative wizard? |
| **D16** | Investigate README Card: OG reuse, pinned/latest/auth/cache | Is the implement/defer decision and follow-up explicit, with failure modes? |
| **D17** | Health component architecture adapts by kind | Does sharing flatten Profile/Memory/Knowledge/Loop specialty? |
| **D18** | Product-centered homepage, deeper category explainer | Accurate category and SEO claims, no rebranding as standards body? |
| **D19** | Explicit/inferred provenance and certainty distinct across contracts | API/lock/UI/Runner statuses coherent and backward-compatible? |
| **D20** | Existing Harness capability/preflight/machine surfaces | No new unnecessary protocol and SDK/TUI/headless parity proven? |

### 17. Additional reviewer questions

1. **APDS as a standard:** Can a third party inspect authored manifests, validate them against the documented contract and know precisely which semantics they must honor *without reading AgentPM source or using its hosted Registry*? Conversely, can a third-party Runner decline support without making a valid package “invalid”?
2. **Correctness in absence of context:** If only `agent.json` is available, can the evaluator honestly say what it verified and what remains unresolved? Does the Registry overclaim graph conformance based on incomplete dependency access?
3. **Source of truth:** If schema, semantics, registry representation and Harness behavior disagree, is there a clear version-pinned specification and defect process rather than silent app-specific reinterpretation?
4. **Release validity:** Can a hand-crafted tar/finalize request publish a malformed APDS artifact? Can a race, second artifact target or old pending session bypass the standard gate?
5. **Portability:** Is the Agent Package independent of Harness but still sufficiently specified? Are memory backend, model, hooks, approvals and prompt composition correctly left as runtime responsibilities?
6. **Trust:** Does a green badge mean something specific and verifiable for the *selected version*, or does it accidentally synthesize “safe/production-ready” from unrelated inputs?
7. **Discovery:** Can a first-time visitor differentiate Agent Packages and Components in seconds, find a real package, run it with honestly stated prerequisites, and understand a Template is not an Agent Package?
8. **Design fidelity:** Have changes strengthened the existing site's floating/layered aesthetic and kind iconography, rather than imitating the AI mockups' alternate blue palette?
9. **Example data:** Can every featured package/metric/Loop phase/conformance status be traced to actual package data, and can editorial selection change safely without editing UI source?
10. **Real developer value:** Does the experience distinguish AgentPM from sharing a GitHub repo or installing individually packaged npm/PyPI Tools, without claiming a guaranteed future market or ecosystem dominance?

### 18. What must be escalated to Zack

Escalate an explicit product decision if implementation would: change the definition of Agent Package/Component/Template; make Loop/Tools/Profile mandatory for APDS validity; change Skill inheritance/Memory operation participation/Loop access priority; replace existing schema with incompatible artifact format; drop or rewrite historical versions; enforce Registry/Harness use for conformance; add universal quality score; materially alter pricing; require a new v5 lock despite compatibility alternatives; change current brand identity; broaden to advanced semantic search/eval/general-purpose Developer; or move comprehensive docs ahead of Stage 1 completion.

Do **not** escalate ordinary choices like exact standard directory, internal data DTOs, component arrangement, tab organization, selected low-cost starter task, lightweight featured-config storage or the exact wording of a legitimate CLI help example if it satisfies required behavior. Those are explicitly delegated to Codex with Claude review.

### 19. APDS semantic review questions Claude should answer in writing

- [ ] Does the v1.0.0 schema **evolve** the existing `cli/schemas/agentpm.manifest.schema.json` instead of inventing a second independently maintained definition?
- [ ] Did Codex preserve local authored `name` versus `@namespace/name` references and machine `kind` instead of copying mistaken mockup `type`/`apds` fields?
- [ ] Is the versioned `$id` stable and unrelated to the arbitrary user-supplied `$schema` editor value?
- [ ] Does `semantics.md` include **actual normative rules** for every supported kind, with IDs, level, source path and expected fixtures—not generic marketing explanations?
- [ ] Are the important binding semantics demonstrated with valid **positive and negative** examples, including Skill-inherited Tools and Loop-denied phases?
- [ ] Are Memory operation targets allowed beyond directly bound spaces, with global/phase participation and `external`, `interval`, `record_count`, `capacity` triggers interpreted correctly?
- [ ] Are Loop terminal targets, implicit completion, branches, cycles, error policies and phase limits described as actual current contracts?
- [ ] Does Profile composition/hints remain advisory, not a capability/safety enforcement grant?
- [ ] Is a dependency-free, no-Loop, no-Profile Agent valid yet acknowledged as potentially Harness-incompatible?
- [ ] Does a standalone manifest validation result avoid claiming full graph conformance when relevant dependency context is absent?
- [ ] Can Rust CLI and Python Registry test the same pinned fixture corpus **offline** without one shelling to/trusting the other?
- [ ] Is normative v1.0.0 immutable once released, with schema and semantics and website reference pointing to identical versioned source?
- [ ] Does an old missing-standard release retain its bytes and installation ability without APDS verification being invented?

### 20. Detailed cross-surface review questions

- [ ] In publish tests, can a client change submitted kind/version/standard after a valid init and sneak through with a mismatched embedded tar?
- [ ] Is server-side enforcement active for new versions of old identities and secure through cutoff/in-flight sessions?
- [ ] In frozen installs, are direct Agent/Skill, full transitive closure, semver satisfaction and artifact digest all checked before mutating state?
- [ ] Does Runner bind Components via the correct locked version rather than a name-only lookup in a multi-Agent workspace?
- [ ] Can a consumer inspect a public selected release's raw `agent.json` and the exact immutable APDS reference using two distinct links?
- [ ] Is a Health result marked verified solely when its actual source/verification supports that claim, and `not applicable` for irrelevant kind checks?
- [ ] Is `Run with Harness` unavailable/explanatory for an Agent Package that is APDS-valid but lacks compatible Loop or requirements?
- [ ] Can featured item order/placement be changed through configuration/data, and does an inaccessible/private item disappear from all previews?
- [ ] Does no-query Explore become intentional discovery while query results retain Stage 1 relevance/pagination/visibility semantics?
- [ ] Are Test/README/Card mockup images prevented from injecting false star/download numbers or fake runtime compatibility into product pages?
- [ ] Do CLI help and all first-use journeys say **Agent Package Manager** and **Agent Package Management** accurately without renaming `agentpm run` into an Agent Runner?
- [ ] Is the future AgentPM Developer creator journey a self-contained handoff, with user-owned idea and optional publish, **not** a prematurely shipped wizard?

### 21. Final Claude review response format

```markdown
## Verdict
Approve | Approve with follow-ups | Changes required | Blocked

## Contract compliance
- APDS source/semantics/fixtures:
- Lifecycle (author, publish, install, Runner):
- Package Health and UI:
- Discovery/onboarding/docs:

## Blocking findings (ordered by severity)
1. [severity] [AC ID / M ID] path:line — evidence, expected behavior, reproduction, required fix

## D01–D20 decision records
- Complete/accepted:
- Incomplete or unjustified:

## Stage 1 integration and rollout risks
- Dependencies verified:
- Compatibility/migration:
- Docs final-stage gate:

## Verification actually performed
- Tests/commands and results:
- Manual flows/screenshots:
- Not run / cannot verify:

## Follow-ups / deferred work
- README embed disposition:
- AgentPM Developer future-stage handoff:
- Remaining risks / owner:
```

**Final stage sign-off:** Claim “Stage 2 done” only if `S2-AC-01` through `S2-AC-22` are evidenced, `D01`–`D20` have reviewed decisions, every release band's critical path is verified, and the final documentation milestone occurred after Stage 1 completed. Passing unit tests but shipping misleading category/Health claims is **not** successful Stage 2 completion.

