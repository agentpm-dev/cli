# Test Plan

**Stage 2: Category-Enabling Hardening — Make the Agent Package Real**  
**Scope:** APDS contract and independent conformance, cross-repository lifecycle, objective Health, category UI, CLI/onboarding, release migrations, documentation.  
**Sources:** [`spec.md`](spec.md) acceptance criteria **S2-AC-01–22**; [`tasks.md`](tasks.md) milestones **M1A–M17C**, including **M1A.1**, **M8B.1** and **M15A.1** (40 reviewable slices) and numbered release bands **1–6**.

> **Verification is not complete by running a happy-path demo or viewing mockups.** Each milestone needs automated checks and focused manual validation. Run commands in the named **repository root** after dependencies are installed. The exact new conformance fixture runner/CLI switches are left to Codex's implementation and **must be recorded here once created**. Do not claim a test ran or passed if only source was inspected. Production mutation and external model calls require explicit staging/test credentials, never production secrets.

## Required verification

### General rules

- Each milestone must cite changed files/PRs, AC and D identifiers, test commands with exit status, screenshots where appropriate, and blocked/skipped verification with a reason and follow-up. Claude should be able to reproduce the evidence.
- **Functional differences matter:** verify schema structure *and* semantics, version provenance, publishing enforcement, exact locked graph, Runner compatibility/readiness, Health evidence truthfulness, and nonquery Registry curation.
- Use hermetic/local unit fixtures for most tests. Use a staging Registry with throwaway namespaces/packages for publish/install and explicit provider config or test model for Harness. Do not mutate production, sign with live private keys unnecessarily, or rely on real accounts in unit tests.
- For schema changes, independently validate both Rust and Python using **identical version-pinned fixtures**. Test against older published versions and old clients; don't only test new scaffolds.
- For UI, compare current live design tokens and behavior, not AI-generated mockup palette/copy/metrics. Use semantic/accessibility checks, responsive screenshots and actual API fixtures.
- Every status/badge must have a test that proves where its data came from (or verifies unknown/legacy fallback). No “all checks passing” or “works with any runner” default assumptions.
- Run a **full regression pass at the release band boundary** and again after M17C. If Stage 1 code changes during the window, re-run affected integration suites.

### Verification crosswalk — numbered release bands and A/B/C milestones

The table below lets Codex and Claude find the relevant **named tests** without guessing which broad area a milestone represents. The test IDs are requirements for coverage, not assertions that corresponding test files already exist. Implementers add exact test paths/commands to the evidence record as the code lands.

| Milestone | Main test evidence required | Critical negative/edge condition |
|---|---|---|
| M1A — contract inventory | Schema/Phase 6–7 cross-repo contract audit | Incorrect assumptions about local `name` versus scoped reference |
| M1A.1 — tolerance release | T-APDS-20 | Old CLI accepts `standard`; misspelled key still rejected; no field loss on rewrite |
| M1B — immutable schema | T-APDS-01–05, 16–18; schema/hash drift checks | Agent w/o Tools valid; mutable `$id` rejected as normative identity; freeze gate observed |
| M1C — semantics | T-APDS-06–14; rule-to-fixture index | Skill inheritance, Loop denial, Memory operation targets |
| M2A — fixtures | T-APDS-03–14 + positive/negative rule coverage | Incomplete graph is not automatically nonconformant |
| M2B — Rust validator | T-APDS-01–16, 18–19 in Rust CI | Unknown standard does not fall back; deterministic paths/IDs; schema-source override cannot forge conformance |
| M2C — Python validator | T-APDS-01–16 in Python CI, parity report | Independent validation, not client trust or CLI shellout |
| M3A — init | T-CLI-01–03,06 | Default Agent but explicit Tool works; valid escaped JSON |
| M3B — lint | T-CLI-04–06 | Missing/unsupported standard fails strict authored lint |
| M4A — publish gate | T-PUB-01–03,06–08 | Raw API forged release cannot publish |
| M4B — rollout | T-PUB-04–05,09 | Old release immutable, old client/inflight behavior |
| M5A — lock/APDS | T-LOCK-01,05,07–08 | Standard origin survives ordinary regeneration; forward guard |
| M5B — frozen/empty | T-LOCK-02–06 | Direct Skill/Agent closure, semver, tamper, empty dependencies |
| M6A — Harness capability | T-RUN-01–04,07 | Valid no-Loop not APDS-invalid; missing model noninteractive |
| M6B — exact versions/SDK | T-RUN-01,05–08 | Multi-Agent same-named different versions; prompt A/B decision |
| M7A — universal Health | T-HEALTH-01,03 | Missing signature/scan does not become verified |
| M7B — kind-specific Health | T-HEALTH-02–05 | N/A per kind; private graph data hidden |
| M8A — APDS web reference | T-DETAIL-04,06 | Source and standard separate; no remote arbitrary fetch |
| M8B — shared visual primitives | T-DETAIL-01–03, T-WEB-07 | Nonlinear Loop and accessible alternate graph |
| M8B.1 — global layout/shell | T-WEB-12 | Signed D21 list predates build; one shell only; Stage 1 search/SEO intact |
| M9A — composition | T-DETAIL-01–02,05 | Direct vs transitive exact version; no-deps empty state |
| M9B — phase/actions | T-DETAIL-02–04 | No unsupported Harness CTA or fabricated linear phase |
| M10A — specialized details | T-DETAIL-05, T-HEALTH-05 | Existing Memory/Knowledge/Loop/Tool/Template tabs preserved |
| M10B — Health UI | T-HEALTH-01–05 | Correct selected-release status, legacy/unknown/failed |
| M11A — data-driven curation | T-WEB-02–03 | Change featured choice/order without JSX; private item leak |
| M11B — Card/embed inquiry | T-WEB-07 plus D16 prototype/evidence | Pinned/latest/caching/privacy not silently skipped |
| M12A — homepage hero | T-WEB-01,02,07 | Real curated package/fallback; current palette preserved |
| M12B — responsive homepage | T-WEB-01,07 + accessibility screenshots | Three real pathways, no empty/dead CTA |
| M13A — Explore | T-WEB-03–05 | No-query versus query semantics; no relevance regression |
| M13B — namespace | T-WEB-06 | Any-kind pins, role states, scoped search unchanged |
| M14A — category explainer | T-WEB-08–09 | No interoperability or “industry standard” overclaims |
| M14B — pricing/SEO | T-WEB-08–10 | No SEO canonical/robots or billing plan regression |
| M15A — CLI help | T-ONB-01 + T-CLI-01–06 | Exact category words; Tool `run` vs Agent Harness |
| M15A.1 — examples republish | T-ONB-09 | `agentpm new` output lints clean; prior versions byte-identical; non-republished set deliberate |
| M15B — starter | T-ONB-02,05 | Real run, low cost and truthful credential requirements |
| M16A — journeys | T-ONB-02–04,06 | Try/Build/Template actionable, no nonexistent Developer link |
| M16B — comprehension/handoff | T-ONB-07–08 | User-owned idea, optional publishing, observed confusion logged |
| M17A — docs gate | T-DOCS-01 | Entire Stage 1 complete before comprehensive rewrite |
| M17B — docs rewrite | T-DOCS-02–03 | Canonical standard linked, no authored README rewrite |
| M17C — final verification | T-DOCS-01–04 + all T suites | All D01–D21 and S2-AC-01–22 evidence reviewed |

### Suggested commands grounded in current repository structure

These are baseline commands, not proof that all projects currently expose the same runner. Codex should reconcile exact CI/build paths and add specific newly authored tests when implementing.

| Repository/root | Existing practical checks | Notes |
|---|---|---|
| `agentpm-dev/cli` root | `cargo fmt --all -- --check` · `cargo test --workspace` · `cargo clippy --workspace --all-targets -- -D warnings` | Workspace in `Cargo.toml` contains `crates/agentpm-cli`, `crates/agentpm-sdk`; if project CI uses feature flags/platform-specific profiles, record them too. |
| `agentpm-dev/registry` root | `python -m pytest tests` **if project test environment includes pytest**; otherwise invoke the runner established by Registry CI and record that command | Current repo contains `tests/test_publish_helpers.py`, `test_install_helpers.py`, `test_tar_validation.py`, `test_search_helpers.py`, etc.; add APDS parity/publish/Health cases accordingly. Confirm migrations via the actual Registry migration runner. |
| `agentpm-dev/website` root | `pnpm typecheck` · `pnpm lint` · `pnpm test` · `pnpm build` | Verified scripts in `website/package.json`; add Vitest/React Testing Library coverage and browser/manual snapshots. |
| Node/Python SDK repositories | Repo-native build/typecheck/test from current CI | Do not invent generic `npm test`/`pytest` when different CI is configured; record exact SDK consumer tests for Harness capability/version changes. |
| Standard contract bundle | A new deterministic fixture verification command chosen in M2 (run against CLI Rust and Registry Python independently) | **Codex must specify exact scripts/targets** in final implementation notes and make them CI-gated, not a prose-only checklist. |

**Baseline CLI manual shell commands** (replace package identities with actual staging, pinned published fixtures; never copy fake IDs from mockups):

```sh
agentpm --help
agentpm init --help
agentpm lint --help
agentpm install --help
agentpm run --help
agentpm harness --help
agentpm new --help

# Isolated empty directory; new default must be kind: agent
mkdir -p /tmp/agentpm-s2-default-init && cd /tmp/agentpm-s2-default-init
agentpm init
agentpm lint

# Repeat in separate clean directories for each explicit --kind value
# --kind agent|tool|skill|knowledge|memory|profile|loop|template
```

The `agentpm init` invocation and `agentpm lint` without flags are based on existing help, but Codex must inspect current behavior and adapt to actual optional prompts/flags. For locked, publish and Harness commands, use exact tested CLI interface from Stage 1+2, not an invented dry-run flag.

---

## Automated checks

### A. APDS contract and schema test matrix (M1A–M2C; S2-AC-01,03,04)

| ID | Test | Expected outcome |
|---|---|---|
| **T-APDS-01** | Validate authoritative bundle contains immutable versioned `README.md`, evolved existing schema, normative semantics, conformance README/index/fixtures | Correct v1.0.0 paths and links; no schema duplication/drift |
| **T-APDS-02** | Inspect schema `$id`, manifest `standard`, editor `$schema`, pinned asset location, source hash; no moving `main` URL as authoritative contract | Immutable version identity; offline validation works; **scaffolds emit the pinned versioned `$schema`**, not the `main` branch URL, while `standard` remains the only selector |
| **T-APDS-03** | Valid minimal Agent manifest with common fields, `kind:agent`, supported `standard`, **no `tools`, `loop`, `profiles`, `bindings`** | Intrinsically APDS-valid; no Harness claim |
| **T-APDS-04** | Valid scaffold/fixtures for all eight kinds using actual current schema field shapes | Pass; local manifest `name` distinct from namespaced dependency reference |
| **T-APDS-19** | Kind-specific APDS violation on each of the eight kinds (missing kind-required field, invalid kind-specific shape) | Diagnostic names the offending field and JSON path plus the stable rule ID; **never** a bare `/oneOf` verdict with the manifest echoed; built on Stage 1 M8's renderer, not a Stage 2 fork |
| **T-APDS-05** | Missing/blank/whitespace-only description, wrong selector id/version, invalid kind, unknown top-level key, malformed refs | Correct stable rule/structural diagnostic and nonzero lint status. Whitespace-only `description` is an **error**, not a warning, is enforced by the pinned schema in both Rust and Python with identical trimming, and is **not** additionally reported by the legacy CLI-side warning |
| **T-APDS-06** | Valid resolved Agent with global + phase Tool/Skill/Profile/Knowledge/Memory bindings | Availability additive, no overwrite of globals |
| **T-APDS-07** | Bound global Skill → declared Tool, bound phase Skill → declared Tool, without direct Tool binding | Tool inherits exact Skill scope, limited by Loop access |
| **T-APDS-08** | Loop `tools:false`/`knowledge:false`/`memory.read:false`/`memory.write:false` despite globally/phase-bound capabilities | Binding does not bypass Loop prohibition |
| **T-APDS-09** | Unknown binding phase, invalid Loop entry/transition/outcome, duplicated phase identifiers, implicit `complete` outcome | Correct success/failure at appropriate validation level |
| **T-APDS-10** | Profile capability hints/boundaries requiring unavailable Tool but Loop forbids it | Hints/boundaries do not grant access; warnings vs errors reflect declared rule |
| **T-APDS-11** | Memory Blueprint operations: global/phase bound; targets space not directly bound; external/interval/count/capacity triggers; governance/retention declarations | Valid authored relationships; trigger/participation semantics preserved; no automatic persistence/execution implied |
| **T-APDS-12** | Knowledge context vs vector, Profile required identity/objectives/communication, Template variables/entrypoints/execution surfaces, Tool runtime shape | Kind-specific schema and semantic rule coverage |
| **T-APDS-13** | Standalone Agent lacks resolved deps; resolved graph provided in separate run | Standalone non-failure but **incomplete** for unresolved checks; resolved run decisive |
| **T-APDS-14** | New manifest unsupported standard; old published manifest with inferred interpretation | `unsupported` vs `legacy_inferred` clearly distinguished; no fake verification |
| **T-APDS-15** | Run entire fixture corpus through Rust validator and Python validator independently | Expected statuses, rule IDs, paths and severities consistent (allow documented equivalent human text) |
| **T-APDS-16** | Mutate original unversioned schema or fixture file after bundling; verify CI synchronization/immutable contract guard | Divergence detected, immutable published contract not silently altered |
| **T-APDS-20** | **Tolerance release (M1A.1):** on the tolerance build, lint a manifest carrying `standard` for all eight kinds; lint a manifest with a misspelled top-level key (`standrad`, `tool`); run `agentpm install <pkg>` and `knowledge build --write` against a manifest holding `standard` plus one extra unknown key; run `agentpm new` from a Template whose scaffolded files carry `standard`; confirm `publish` preflight, `export`, `memory build` accept it | `standard` accepted everywhere and never lost on rewrite (key reordering from the deliberate no-`preserve_order` build is expected); misspelled keys still rejected, proving the root was not opened; `agentpm new` clears `validate_generated_manifests_blocking`; no command emits `standard` yet |
| **T-APDS-18** | Place a permissive `schemas/agentpm.manifest.schema.json` in the working directory, and separately pass `--schema` (file and `http(s)` forms) to `lint` and `publish`; attempt to validate a manifest the pinned contract rejects | Override cannot yield `conformant`; result labeled non-authoritative; `publish` preflight cannot imply conformance from an overridden schema; no network fetch during routine validation |
| **T-APDS-17** | **Freeze-gate evidence:** confirm Stage 1 M2/M11A schema additions were merged before v1.0.0 froze, then validate a Template declaring `agentpm-harness` and a Tool declaring `runtime.dependencies` against the frozen contract | Both are **conformant** under v1.0.0; no need for a v1.0.1/v1.1.0 to accept legitimate Stage 1 manifests |

**Fixture inventory required:** include positive and negative fixtures for each *normative semantic rule*, not just the examples above; map stable rule IDs to fixture cases in the conformance index. Generated/no-dependency Agent, multi-version dependency graph and early MMP legacy artifacts are essential fixtures.

**Design/contract review check:** Claude compares `semantics.md` and fixture outcomes against existing Phase 6/7 behavior: additive scope, Skill inheritance, Memory operation participation vs directly bound spaces, Loop priority, Profiles advisory. A JSON Schema pass alone is insufficient.

### B. CLI authoring/init/lint (M3A–M3B; S2-AC-02,03,18)

- **T-CLI-01:** `agentpm init` default generates `kind:"agent"` with valid meaningful default name/description/version/standard and a **pinned versioned `$schema`** (not the moving `main` URL), with no residual "Missing $schema" warning; CLI scaffolds optional tools/loop/profiles correctly. `agentpm lint` passes. A Harness run with no Loop gives an accurate not-runnable explanation rather than a schema failure.
- **T-CLI-02:** Explicit `--kind` for all eight kinds, plus `agentpm export`'s generated Skill scaffold; each generated `agent.json` declares `standard` and passes its intrinsic lint; Tool kind retains Stage 1 scaffold corrections, correct runtime/entrypoint shape and safe behavior.
- **T-CLI-03:** User description with quotes, Unicode, backslash, escape/newline; JSON writer produces valid JSON or user-friendly validation without accidentally emitting invalid syntax. Invalid local names fail deliberately.
- **T-CLI-04:** Missing/unsupported `standard` in new authored manifest fails; optional legacy migration mode (if selected in D04) is opt-in, clearly marked, does not become default bypass.
- **T-CLI-05:** Lint diagnostics show stable rule identity and useful paths for invalid phase, Skill, Memory, Template or Tool; nonzero exits and machine-mode structures remain compatible.
- **T-CLI-06:** `init` doesn't overwrite existing project unexpectedly; Tool/Template/SDK/new/run/publish unaffected absent explicitly agreed changes.

### C. Registry independent finalize, rollout, and security (M4A–M4B; S2-AC-04,05)

- **T-PUB-01:** Valid APDS 1.0.0 new Agent Package and every other kind can publish via normal CLI; Registry independently reads staged bytes and validates before finalize.
- **T-PUB-02:** Custom/raw client bypasses CLI and sends invalid JSON/unsupported selector/wrong metadata with syntactically valid init request; Registry rejects before release visible.
- **T-PUB-03:** Metadata declares one name/kind/version/standard, embedded tar `agent.json` declares another; Registry rejects with precise classification and rollback/cleanup.
- **T-PUB-04:** Valid new version of old legacy package **must** include supported standard; old historical version remains installable unchanged with same digest and signature.
- **T-PUB-05:** In-flight publish session created before strict gate but finalized after cutoff follows **documented server policy**; cannot change cutoff by client timestamp; no unintended inaccessible artifact.
- **T-PUB-06:** Multi-artifact Tool release from Stage 1: validate authoritative embedded manifest for each relevant target/release record while preserving target metadata, scan, signer and atomic finalize semantics.
- **T-PUB-07:** Authorization/visibility/namespace-signing policy, malware and stored-byte verification continue to protect finalize; APDS conformance does **not** grant publish permissions.
- **T-PUB-08:** Resolver/private/transitive dependency context limited or unavailable yields truthful incomplete or explicit failure according to documented policy; not “fully verified” without graph evaluation.
- **T-PUB-09:** Existing CLI published-url success output preserved; old incompatible CLI receives explicit upgrade guidance naming the **concrete minimum CLI version** (the M1A.1 tolerance release) when the strict gate is active, and the package detail page shows the same floor.

### D. Resolve/install/lock integrity and `--frozen` (M5A–M5B; S2-AC-06,07)

- **T-LOCK-01:** Stage 1 lock v4 retained if feasible; APDS `{id,version}` plus declaration provenance roundtrip without dropping data or older/future unknown-version protections.
- **T-LOCK-02:** Clean install explicit Agent root with transitive Skill→Tool, Knowledge, Memory, Profile, Loop; frozen clean reinstallation reconstructs identical kind/name/version/digest closure.
- **T-LOCK-03:** Direct Skill install and frozen replay preserve **Skill**, not accidentally Tool; direct Tool and Template paths unchanged.
- **T-LOCK-04:** Authored dependency semver/range no longer satisfied by frozen pin → fail before state mutation; hidden transitive closure or missing lock entry → fail explicitly.
- **T-LOCK-05:** Registry artifact checksum mismatch and tampered local cache fail securely; never accidentally accept a different digest as matching.
- **T-LOCK-06:** Agent with **no dependencies** resolves/installs successfully; repeated normal/frozen install is stable/no-op as appropriate.
- **T-LOCK-07:** Unsupported APDS standard, bad release/provenance conflict and legacy inferred selected release produce honest results and preserve old installability.
- **T-LOCK-08:** Stage 1 portable Tool target selection and release/artifact identity remain correct on supported architectures (run target-specific test suites only where runners exist).

### E. Harness and SDK compatibility/preflight (M6A–M6B; S2-AC-08,09)

- **T-RUN-01:** Standard compatibility advertised consistently by CLI, TUI, headless/machine preflight and Node/Python SDK wrappers; Harness protocol and APDS version dimensions separate.
- **T-RUN-02:** Valid minimal Agent lacking Loop reported **APDS valid but not Harness-runnable**; not falsely classified as APDS-invalid.
- **T-RUN-03:** Unsupported APDS declared version fails supported-Runner preflight with precise diagnostic, not a lower-level unknown shape error.
- **T-RUN-04:** Noninteractive/headless missing provider/model returns not ready with an actionable message; no TTY prompt assumed. Interactive bootstrap retains normal prompt behavior.
- **T-RUN-05:** Multi-Agent workspace with the same Component identity at multiple locked versions uses the **intended exact dependency version** for each Agent/phase; direct/Skill-inherited Tools and Memory operations checked.
- **T-RUN-06:** Phase access rejects bound Tool/Knowledge/Memory where Loop forbids it; approval gating remains enforced; Profile hints never create capabilities.
- **T-RUN-07:** Historical valid Runner outcomes, `$end`/`$abort`/`$handoff`, implicit completion, checkpoint, retry, memory operations, TUI/headless trace/events remain intact.
- **T-RUN-08:** D13 top-level Agent description prompt A/B: capture deterministic/runnable comparison cases with/without description, model/token/cost/quality notes, selected recommendation and regression for retained behavior. **Evidence and decision**, not necessarily code change.

### F. Health model and detail pages (M7A–M10B; S2-AC-10,11,12)

- **T-HEALTH-01:** Universal evidence derives from actual release APDS/signature/attestation/scan/digest/license/date data; missing/legacy/incomplete/unsupported/unknown statuses reflect authority. Stars/downloads remain **identity-level**.
- **T-HEALTH-02:** Tool architecture/OS/runtime signal and each of seven other kinds' relevant compatibility/contract signals render or correctly `not applicable`; no universal Tool-platform warning for Profiles/Loops/Templates.
- **T-HEALTH-03:** Signature present but unverified, scan unknown, attestation absent, advisory compatibility, outdated release, inferred APDS → never a green “safe / fully verified” or universal score.
- **T-HEALTH-04:** Switch versions on same package; signals and `agent.json`/standard links change with selected version, while stars/publisher identity remain identity-level.
- **T-HEALTH-05:** Public vs private evidence: anonymous view cannot leak private Component/dependency/maintainer data through Health, composition graph or featured cards.
- **T-DETAIL-01:** Valid Agent with multi-kind direct and Skill-inherited/transitive deps shows correct exact names/versions/link targets and clear relationships in Overview.
- **T-DETAIL-02:** Agent without Loop/deps displays a truthful useful Overview and disables/explains Harness action; no empty graph crash or fictitious phase.
- **T-DETAIL-03:** Nonlinear Loop (branch, cycle, `$handoff`/`$abort`) renders accurately or meaningful fallback; display of Loop tools and approvals reflects actual bindings/access.
- **T-DETAIL-04:** Distinct “View agent.json” opens selected version's raw manifest and “View APDS specification” opens immutable supported version route; unavailable/legacy link behavior honest.
- **T-DETAIL-05:** All kinds preserve their dedicated previous inspection fields/tabs, plus appropriate Health/APDS details; no flattened generic card regression.
- **T-DETAIL-06:** Versioned APDS route links pinned schema/semantics/fixtures and cannot fetch attacker-controlled arbitrary URI from package manifest.

### G. Website IA, featured, search and content accuracy (M8A–M14B; S2-AC-13–17)

- **T-WEB-01:** Homepage primary heading/value line, hero example and top CTAs communicate Agent Package/AgentPM and lead to real links; Component and Template paths present but secondary.
- **T-WEB-02:** Home/Explore/namespace featured selection **changes without editing React presentation code**; curated placement order deterministic, no hard-coded production IDs in JSX. Missing/removed/private entries skipped safely.
- **T-WEB-03:** Featured and Trending distinct sources; text-query search relevance unchanged, no arbitrary Agent Package rank boost, no dependency inherited installs/stars in Trending.
- **T-WEB-04:** No-query `/explore` shows useful curated + measured discovery; filtered query preserves Stage 1 query URL, facets, sort, result counts, cursor back/forward and private/public authorization.
- **T-WEB-05:** Kind facets and cards convey Agent Packages vs six Components vs Templates/Namespaces; show real per-kind metadata with sane unavailable fallbacks.
- **T-WEB-06:** Namespace public/private, publisher pins **of any kind**, scoped search/filter/sort/cursor, recent-activity links, signing copy and grouped counts remain accurate.
- **T-WEB-07:** Agent Package Card variants share actual identity/description/version/composition/provenance data; mobile truncation/accessibility/OG dimensions acceptable, no false “production ready”.
- **T-WEB-08:** APDS reference, category explainer, pricing and existing detail routes: canonical/index/robots/sitemap/OG/social previews and noindex Explore-query boundaries match Stage 1 behavior; no duplicate indexable parameter URLs.
- **T-WEB-09:** Pricing correctly reflects public packages/Components free, private namespace plans, Team sharing. Checkout/login unchanged and public Explore accessible.
- **T-WEB-10:** Visual regression reference from actual current site: brand tokens/elevation/floating cards/icons preserved except where the signed D21 list deliberately adopts or adapts a mockup treatment; mockups never the source for copy or any displayed value; color contrast, keyboard, narrow viewport and text overflow checked.
- **T-WEB-12:** Global layout and shell (M8B.1). Assert exactly **one** implementation of each global element — header (anonymous and authenticated variants), page canvas, section card, kind tokens, grid primitives, footer — with no competing second implementation anywhere in the codebase. Verify global search in the header does not regress Stage 1 search behavior, query URLs, facets or cursor state. Verify SSR metadata, canonical links and robots behavior from Stage 1 M7 still hold on every migrated route. Confirm the shipped per-kind tone assignment and the generated per-package identity gradient are preserved unchanged (and centralized rather than re-chosen); confirm identity surfaces (Card, Explore results, detail identity, composition nodes) render generated avatars with kind carried by tone/label alongside, while category surfaces (facet rail, kind filters) may use compact glyphs and no glyph stands in for a specific package's identity; confirm the signed **D21** adopt/adapt/reject list exists and predates implementation, and produce a **token-by-token diff annotated against that list** — every adopted treatment named on it, nothing adopted that is not — alongside before/after screenshots of the live production site at desktop, tablet and narrow widths. Check header nav collapse, facet rail behavior, card reflow, keyboard focus order, visible focus states, ARIA labels on icon-only controls, and reduced-motion handling. Confirm the recorded route adoption order matches what actually shipped, and that any route still on the old shell is explicitly named with a migration owner.
- **T-WEB-11:** README embedded Card investigation D16 yields realistic snippet/prototype/decision, tests pinned-vs-latest/private/auth/cache/OG if implemented; if deferred, verify explicit future task and no broken embed UI shown now.

### H. CLI help, onboarding, starter package, learning and docs (M15A–M17C; S2-AC-18–22)

- **T-ONB-01:** `agentpm --help` literally contains **Agent Package Manager** and **Agent Package Management**; command help accurately distinguishes `init`, `new`, `lint`, `install`, `publish`, `run` Tool and `harness` Agent Package; sample commands work.
- **T-ONB-02:** Complete Try journey anonymous browse → selected public Agent → install → configure real provider/model → Harness run with expected outcome; document time/cost/credentials and compare on supported clean environment.
- **T-ONB-03:** Build journey init Agent → compose minimal execution-ready dependencies/Loop → lint → publish (staging) → published Registry URL → inspect; don't promise default init is immediately runnable.
- **T-ONB-04:** Template journey browse → `agentpm new` → customize → supported execution surface → first successful run; Template is never mislabeled Agent Package.
- **T-ONB-05:** Starter Agent Package isn't flagship AgentPM Developer, needs limited credentials/calls/cost, use real published version or staged equivalent with representative fixtures; verify deterministic-enough expected behavior.
- **T-ONB-06:** AgentPM Developer CTAs visible only when real destination exists, otherwise safe manual/Template guidance; no nonexistent link/dead button.
- **T-ONB-07:** Future AgentPM Developer-stage handoff includes own idea → assisted build → lint/test/run → optional publish → share with Card, along with nonmandatory Developer and meaningful reuse goals; implementation explicitly deferred.
- **T-ONB-08:** Conduct lightweight unfamiliar-developer task walk-through: can explain artifact vs Component, APDS vs Harness vs Registry, identify readiness, install/run or create/share; record observations, fixes and remaining positioning questions. This tests comprehension, **not market demand**.
- **T-ONB-09:** Example/Template republication: for every Template whose `files_root` ships manifests, `agentpm new` from a clean directory produces a workspace that passes strict `agentpm lint`, resolves and installs with exact pinned Component versions, and gives an honest not-runnable explanation when the scaffolded Agent has no Loop. Every prior published version of every touched package is byte-identical (digest recorded before and after) and still installable, with no retroactive APDS-verified marking. Republished versions pass independent Registry validation, not just local lint. An older CLI still scaffolds/lints an older Template version. The deliberately **not** republished set is recorded with rationale.
- **T-DOCS-01:** **Before M17A:** all Stage 1 milestones confirmed complete; otherwise broad docs/README rewrite is blocked. Stage 1 functional docs and early APDS normative files unaffected.
- **T-DOCS-02:** Introduction/Quickstart and CLI/Registry/SDK/Harness/Component/Template/Health documents agree on terminology and working paths; generic Tool-first Quickstart replaced with genuine Agent Package-first guide.
- **T-DOCS-03:** AgentPM-owned README, docs code fences, homepage code boxes, versioned normative APDS links and starter example match actual commands/versions; no author-controlled published README modifications.
- **T-DOCS-04:** Docs/README links, SEO, navigation, screenshots, generated code examples and SDK snippets pass smoke test; no stale pre-Stage 1 lock/publish/provenance guidance.

---

### I. Reproducible reference fixture scenarios and expected-result matrices

**I1. Authored manifest versus resolved graph.** Construct a minimal `kind: agent` manifest with only common required fields and the declared supported standard. Structural validation passes, resolved graph validation is either unnecessary (no refs) or correctly tagged as not evaluated; Harness run without Loop rejects *Runner compatibility*, not APDS. Add one malformed identical manifest with a blank description and verify both validators reject with matching diagnostic family.

**I2. Versioned Skill inheritance.** Construct a resolved Agent depending on a Skill which depends on Tool T. Bind Skill globally; allow tools in phase P and deny in phase Q. Expect T exposed as a candidate in P but not Q, with no explicit Agent Tool binding. Construct another Agent with Skill only in P; T must not appear in Q. Populate a workspace with T@1 and T@2; require runtime to take pinned exact resolved version.

**I3. Memory operation participation.** Construct a Blueprint with Space X and operation O targeting X, but bind O in phase `review` without direct X binding in that phase. Graph validation must accept valid Blueprint targets, and operation eligibility follows its trigger. For `external` trigger, no uninvoked automatic execution. For `interval`, `record_count`, `capacity`, validate authored trigger grammar and participation scope separately from actual scheduling.

**I4. Loop graph semantics.** Positive: valid branch/cycle with explicit terminal outcomes and correct phase access. Negative: missing entry, duplicated phase, transition with nonexistent outcome/target, bound phase key not in resolved Loop. Do not require UI to show branches as false sequential columns; compare raw graph to derived display/accessibility fallback.

**I5. Publish split trust boundaries.** Use a staged Registry test release with APDS-valid manifest and a separate failing signature, then reverse (valid signature on APDS-invalid manifest). Each must fail its own gate with distinct error. Repeat with a custom client whose init metadata differs from embedded tar name/version/standard. No partial discoverable release in any failure.

**I6. Legacy migration.** Pin a previously published release with no `standard`; record bytes/digest/signature and installed graph before rollout, then compare after. The newer version of the same identity must contain explicit `standard` and pass independent Registry validation. Old release remains installable as inferred, with no `APDS verified` badge merely due to inference.

**I7. Lock and build portability.** Freeze a graph including Agent, Skill→Tool, Knowledge and Memory, then recreate under clean `.agentpm`; verify logical identities/versions/digests match. Retry with corrupted archive, changed requirement/range or missing lock entry and ensure safe fail. If Stage 1 target-specific artifact semantics are present, verify logical `agent.lock` remains independent of local machine target.

**I8. Health and visibility.** Prepare representative selected releases for eight kinds, including version with no scan, version with present-but-unverified signature, legacy APDS, Tool with only one supported architecture, and Profile with no platform relevance. Assert status, applicability, source and no fictional green badge. Include hidden/private dependencies in a public Agent: the anonymous composition/Health/featured surfaces must not leak inaccessible metadata.

**I9. Curation separation.** With one featured item deleted, one private, one public, one unlisted and two real Trending candidates, verify eligible featured cards remain in configured order, Trending ranking follows Stage 1 data, and anonymous pages don't reveal private entry titles or rendered OG image metadata.

**I10. Developer comprehension.** Walk one experienced agent developer through a page without prior explanation. Record whether they distinguish Agent Package from Tool, APDS from Harness, published release from local readiness, and Template from an installed Agent. Record outcome and blockers; don't interpret this single walkthrough as market validation.

## Manual checks

### Release-band walkthroughs

| Band | Manual scenario | Evidence to collect |
|---|---|---|
| **1: APDS + publish** | Install the tolerance release and confirm an old-style workspace plus a `standard`-bearing manifest both work; create one new artifact of each kind; lint; inspect versioned normative schema/rules; publish representative new release; try forged publish; inspect unchanged legacy release | CLI transcripts (redacted), schema fixture result report, staging API status, version diff, rollout/rollback notes |
| **2: install + Harness** | Fresh install existing Agent with Skill-inherited Tool, Memory operations and Loop; inspect lock; frozen reinstall in empty dir; TUI run then headless/SDK preflight; try no Loop and missing model | Lock snapshots with safe metadata, run traces/exit statuses, machine capability payload and result, no secrets |
| **3: Health + detail** | View Agent Overview composition, Loop phases, standard/raw `agent.json`, Health; select old/legacy version; inspect Tool, Knowledge, Memory, Profile, Loop, Skill and Template pages | Desktop/mobile screenshots, API evidence source map, negative-state screenshots |
| **4: category UI** | Walk every route and confirm one consistent shell (header/search/canvas/section cards/kind tokens/footer); anonymous home → featured Agent → Explore search and no-query discovery → namespace pinned items → standard/category/pricing; change featured configuration and repeat | Before/after screenshots, URL/canonical captures, role/visibility matrix, featured config change evidence |
| **5: onboarding** | Republish an affected Template and scaffold from it; follow Try, Build and Template pathways from first visit; install/run simple starter; publish own example to staging; capture final package link; attempt Developer CTA when not available | Step-by-step command/result transcript, 3 user-journey recordings or notes, failed-precondition explanations |
| **6: final docs** | Confirm Stage 1 complete; follow docs from fresh workstation; verify CLI/SDK/examples and normative reference; conduct comprehension/reviewer pass | Gate verification, docs change map, working code snippet check, Claude sign-off and known follow-ups |

### High-risk manual negative cases

1. **Legacy immutability:** Retrieve an old published version and compare bytes/checksum before and after rollout; confirm no retroactive `standard` injection or “verified” badge.
2. **Unsupported standard selector:** New authoring, publishing, installation and Runner preflight should report distinct explicit failures; do not invent a “fallback current version.”
3. **Partial graph:** Author valid standalone Agent with missing/private Component graph; inspect lint, Registry conformance, Health and Runner readiness separately. Decide whether missing context means incomplete or a definite failure according to published rules.
4. **Nonlinear Loop:** Inspect graph and actual allowed phase capabilities; no misleading three-step linear-only diagram and no accidental memory/tool permission bypass.
5. **Stage 1 publishing:** Replay multi-artifact Tool and author signature/attestation release; ensure new standard gate does not break target artifact verification.
6. **Privacy and curation:** Curate a private/deleted package then browse anonymous homepage, Explore, namespace, social image; private metadata cannot leak.
7. **Browser/web accessibility:** Narrow viewport, keyboard focus, long descriptions, reduced-motion where supported, screen reader labels on icons/code copy/diagrams and canonical link behavior.
8. **Environment readiness:** Harness headless with missing model, missing runtime backend or approvals; no optimistic ready state; no hidden external API costs in first-run examples.
9. **D13 prompt A/B:** Repeat selected tasks and archive redacted prompt/trace comparisons; if indeterminate, document no change rather than pretending improved quality.
10. **D16 README embed:** Validate feasibility of static/dynamic URL and pinned/latest/private/caching choices if prototype exists; otherwise archive decision and concrete follow-up scope.

---

## Expected evidence

### Per-milestone evidence template

```text
Milestone:
Changed repositories / PRs:
AC IDs validated:
D IDs resolved (selected approach, alternatives, rationale):
Stage 1 contracts inspected and status:
Commands executed, exit codes, CI links:
New fixture cases / tests (names, expected statuses):
Manual scenarios and observed outcomes:
Screenshots (UI) / console snippets (CLI/API) / traces (Harness):
Schema / API / lock / manifest migration and rollback notes:
Privacy/security/compatibility negative case verified:
Known gaps, skipped checks and owners:
Claude review: approved / changes requested / blocked:
```

### Artifact requirements

- **APDS:** fixed-version contract bundle and checksum; `semantics.md` stable rule index; schema field diff; fixture manifest/result report for **both independent implementations**; statement of structural/resolved/Runner boundaries.
- **Publishing:** old client/new client matrix; malicious/custom client negative tests; server cutoff/pending-session and rollback evidence; legacy immutable release digest sample; registry audit/error classifications.
- **Install/Harness:** lock v4 roundtrip and frozen graph sample; exact version resolution proof; machine capabilities payload; TUI/headless/SDK parity; non-TTY no-model and no-Loop results; description-context experiment.
- **Health:** field-to-source map, per-kind applicability matrix and status screenshots including unknown/legacy/failed and advisory conditions; no universal score.
- **Web:** a single-shell audit (one header/canvas/section-card/kind-token/grid/footer implementation) plus the route adoption order; the signed D21 adopt/adapt/reject list with its sign-off date, and a token-by-token diff annotated against it; annotated screenshots against **existing** current production design and both mockup families; real feature config and fallback; mobile keyboard/ARIA checks; SEO metadata URLs; search regression suite.
- **Onboarding:** one actual runnable low-cost Agent Package with pinned version; successful Try/Build/Template steps; comprehension observations; future AgentPM Developer handoff.
- **Docs:** explicit proof Stage 1 fully complete before rewrite, verified CLI/sdk snippets and final Claude feedback.

### Reviewer sign-off criteria

Claude Code should **request changes** if any of the following holds: APDS semantics contradict the currently implemented contract without product approval; Rust/Python tests only pass by sharing the same unverified client conclusion; Registry accepts an invalid new release via raw API; frozen lock replay/empty deps fails; APDS-valid no-Loop Agent is labeled nonconformant; Health inflates trust; a featured private package leaks; main page changes the brand unilaterally; a milestone lacks decision evidence; final docs began before Stage 1 finished.

---

## Out of scope

- General customer interviews, market/category hypothesis validation, conversion lift experiments or proof of category leadership; capture product comprehension and pass the broader research questions to Stage 5/user-learning work.
- Universal Agent quality scoring, evaluation framework or comparative benchmark across model providers, AI/semantic search, social functionality and reverse-dependency Registry indexing.
- Production load/performance chaos tests beyond changed endpoints and Stage 1-established acceptance thresholds; adopt existing CI/performance constraints when relevant.
- Proving APDS executes on arbitrary external frameworks/Runners or compatibility with non-AgentPM standards beyond explicitly supported APDS versions. Do not advertise unsupported third-party runtime integrations.
- Executing real customer data, arbitrary malicious packages or private Registry releases without authorization. Use fixtures/sandbox packages and approved staging environments.
- Implementing full AgentPM Developer guided creation/publishing wizard in Stage 2; only future-stage handoff and valid integration hooks.
- Full embeddable README Card implementation if D16 investigation justifiably defers it; the **investigation and decision** remain required.
- Mass-rewriting author-controlled READMEs, changing pricing/billing behavior, or broad API/CLI breaking changes absent explicit product approval.

