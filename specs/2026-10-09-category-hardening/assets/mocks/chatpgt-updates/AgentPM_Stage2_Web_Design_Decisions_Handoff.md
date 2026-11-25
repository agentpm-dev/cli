# AgentPM Stage 2 — Website Design Decisions and Implementation Handoff

**Prepared:** November 25, 2026  
**Purpose:** Hand off the four reviewed HTML design references and the decisions behind them to Claude for a focused `spec.md` / `tasks.md` reconciliation, followed by Codex implementation.  
**Stage status:** **Milestone 8A is currently active.** The website implementation milestones beginning at M8B are **upcoming**. Design review/approval of a mock is **not** milestone completion or permission to skip its implementation, verification, or D21 gates.

## 1. Read this first: what is agreed and what this handoff is not

Four surfaces were reviewed iteratively with the project owner and **approved as the Stage 2 design references**:

1. Landing / homepage.
2. Explore / search / zero-query browsing.
3. Agent Package detail, including Overview, Health & Security, and Bindings.
4. Tool detail, demonstrating that the same page architecture works for a reusable Component without forcing Agent-only concepts onto it.

**The HTML is a visual, interaction, and copy reference, not production source.** Implement **inside the existing `agentpm-web` Next.js/React application**. Preserve working components, design tokens, identity generation, search and filters, authentication, package routing, SEO, accessibility, and especially the real site's better responsive behavior. Do not build a parallel app or port the mock's Tailwind CDN, hand-written identity generators, static data, ad hoc state management, or placeholder links.

The original Claude handoff's `README.md` is useful provenance but **not the final design authority wherever a later approved mock or a decision recorded here supersedes it**. The existing Stage 2 `spec.md` and `tasks.md` also predate parts of this review. **Claude's immediate assignment is to reconcile them, not to assume they already say the same thing.** Preserve independent product/semantic/security requirements not affected by the visual decisions.

**Positioning north star:** Agent Package Management is the category; AgentPM is a Manager + Registry + Runner that supports the APDS definition standard. An **Agent Package** is the complete authored top-level Agent system (`kind: "agent"`). **Components** are the six independently reusable kinds: Tool, Skill, Knowledge, Memory Blueprint, Instruction Profile, and Loop. **Templates** are project/workspace scaffolds and are *neither* Agent Packages nor Components. Individual Components are valid products in their own right: a developer may use AgentPM exclusively for Tools, for example. Don't make the registry Agent-only.

**Main problem to communicate:** Complete AI agent systems are difficult to reuse when their Tools, instructions, Knowledge, Memory, and workflows are scattered across application code and host/framework conventions. AgentPM gives the *complete authored system* a portable, versioned, installable definition. Do **not** reduce the positioning to “npm for agents,” an imitation of an existing package manager, or a broad list of packaging commands. Use the familiar package-management lifecycle to support the explanation, not substitute for the developer problem.

## 2. The four files: final source mapping and optional renames

The files below are the **latest approved versions**, not the older alternatives in the original mock ZIP. The filenames on the left exist in the handoff's source context; the right column proposes shorter names for the project directory. **A rename is optional; update references consistently if renamed.**

| Latest file | Suggested project filename | What it represents |
| --- | --- | --- |
| `AgentPM_Landing_Page_Stage2_Composition_Refined_Core_Four.html` | `01-landing-approved.html` | Approved problem-first landing copy, shareable featured package card, composition illustration, light core-four section, onboarding, Trending. |
| `AgentPM_Explore_Stage2_Contextual_Browse_Copy.html` | `02-explore-approved.html` | Approved no-query discovery, three conceptual groups, row results, contextual filter copy, neutral category marks. |
| `AgentPM_Agent_Detail_Stage2_Bindings_Experiment.html` | `03-agent-package-detail-approved.html` | Latest Agent page combining the approved header, stacked composition/execution, independent sticky identity/tabs, Health & Security, and Bindings. |
| `AgentPM_Tool_Detail_Stage2_Schema_Derived_Run_Example.html` | `04-tool-detail-approved.html` | Latest Tool page with the shared shell, Interface/Runtime inspection, schema-derived starter Run input, neutral output variants, environment Copy controls, and Health. |

The term **approved** means *agreed direction/design reference*, not a finished production UI or verified data feed. If there is an unexplained difference between the mock and existing working application behavior, inspect the live implementation before deciding; keep existing behavior unless the review explicitly changed it.

**Do not accidentally use earlier superseded files:** `detail-page.html`, `detail-page-tool.html`, `explore-rows.html`, `explore-page.html`, or prior `*Experiment*` files from intermediate turns. The rejected two-up Explore result cards and original side-by-side Agent detail layout are **not alternatives still under consideration**.

## 3. Global design language and reusable rules

### 3.1 Visual shell and information hierarchy

- **Dark theme is intentional.** Preserve the site's current dark slate-based design, subtle gradients/depth, kind-color tokens, typography, and identity marks. Earlier light-themed, floating-white-card PNGs are superseded as design direction.
- **Keep the existing full-bleed sticky global navigation**, with centered search and its established mobile behavior. Do not introduce a floating inset nav just because an older `-layered` image showed it. The original Claude mock handoff records this decision.
- **Reduce gratuitous boxes.** The agreed landing page is mostly open sections with restrained dividers; its thesis is the one major concept card and package cards represent actual packages. Technical detail and disclosures may use panels, but avoid cards nested inside cards solely for visual consistency.
- **Keep real package identities distinct from kind/category symbols.** Named packages use their generated identity marks (existing `ToolBox.tsx` behavior and real deterministic gradients); category shortcuts use neutral representations, not a fake package avatar. Preserve existing kind-to-color mapping: Agent amber, Tool indigo, Skill rose, Knowledge fuchsia, Memory teal, Profile cyan, Loop lime, Template emerald.
- **Template icon:** the folded-corner mark from the reviewed mock is a **proposed new kind mark**, not a verified existing `TemplateBox` component. Treat it as an explicit addition to consider in D21, not as an existing export. The neutral category/group marks are conceptually separate from per-package identity gradients.
- **Real data only.** No fabricated packages, scores, adoption metrics, security checks, version numbers, readiness states, or installed artifacts. Some HTML uses captured examples and static ranking for visual preview; production must use current version-specific manifest, Registry, validation, and resolution evidence.
- **Respect reduced-motion preferences.** Use accessible keyboard navigation, visible focus, responsive type/layout, horizontal scrolling for long commands or tab labels instead of body overflow, and accessible textual representations for diagrams.
- **Copy decisions are now agreed for these four reviewed surfaces.** The old planning rule that all mock copy is purely placeholder should be updated accordingly. These texts are approved positioning/copy directions, subject only to factual, legal, or implementation corrections—not a fresh copywriting exercise in M14.

### 3.2 Shared Package Detail shell (all eight kinds)

The Agent and Tool experiments established one reusable *page pattern*, not one identical page per kind:

1. Open, unboxed **package identity** area: actual identity mark, namespace/name, kind, selected version/version picker, description, publisher, appropriate release metadata, and applicable trust affordances.
2. Lightweight, **kind-specific At a Glance** on the right on desktop; stack or reflow on small screens. Facts are structural/capability facts from the selected artifact and inspected evidence—not stars/installs repeated from the header. **Never force exactly three** facts, invent zeros, or render blank metric placeholders; 1–2 meaningful facts are fine.
3. Full-width, task-oriented **Get Started** below identity. Install followed by the appropriate next action; alternative SDK/consumer path only when supported. Don't force Tool/Agent commands onto declarative Components or Templates.
4. **Compact package identity becomes sticky independently** as soon as the original name/version row scrolls under global navigation. The tab bar becomes sticky **later**, only when it reaches the bottom of the compact identity strip. There must be no interim region where the package name/version is entirely unavailable. The compact strip includes package mark, breadcrumbs, name, and version selector; clicking identity may return to full header. Account for the taller mobile global header, scroll anchors, sticky offsets, and focus/keyboard behavior.
5. Tabs are **kind-appropriate**. Agent: Overview / Examples / Readme / Health & Security / Bindings. Tool: Overview / Readme / Health & Security. Keep existing real README/MDX rendering and content behavior; do not ship mock placeholder README text or add empty tabs for arbitrary kinds.
6. Primary inspection areas teach what makes that kind distinct; **layout may be stacked or side by side based on actual content density**. Evidence and Source remain accessible but secondary to the primary inspection content.

**Suggested kind-specific inspection hierarchy (guidance, not a mandate for exactly two panels):**

| Kind | Primary inspection focus |
| --- | --- |
| Agent Package | Composition; Execution Architecture (real phases/outcomes/links) |
| Tool | Interface (inputs/outputs); Runtime & Execution |
| Skill | Instructions & Resources; Tool Dependencies |
| Knowledge | Content & Documents; Retrieval Configuration |
| Memory Blueprint | Spaces & Record Contracts; Lifecycle Operations |
| Instruction Profile | Identity & Guidance; Constraints & Compatibility |
| Loop | Phases & Transitions; Access & Execution Policies |
| Template | Project Structure; Variables & Entrypoints |

Don't create seven more HTML mocks merely to implement these. Adapt the existing specialized detail surfaces without dropping important manifest semantics: Skill files/scripts, Knowledge context/vector modes, Memory scopes/governance/triggers/operations, Profile identity/behavioral constraints, Loop graph, and Template scaffolding. Support missing/empty optional declarations and varying content density gracefully.

### 3.3 Trust and verification language is a shared system

- No aggregate “safe,” “healthy,” or unqualified conformance badge. **APDS conformance, integrity/provenance, scanning, artifact compatibility, Runner support, and local readiness are different dimensions**; a Registry cannot establish whether a viewer has local credentials or a working model provider.
- Explicit observer attribution: *Passed* applies to checks; *Verified by Registry* must name who performed verification; *Declared*, *Inferred*, *Incomplete*, *Unsupported*, *Not evaluated* are meaningful distinctions. Don't turn inferred legacy APDS compatibility into explicit verified conformance.
- **APDS scopes should be visible immediately**: Standalone (manifest/local), Packaged content (supplied files), Resolved composition (selected dependencies). Explain each in plain language. Provide lightweight “What do these checks mean?” help independent of actual results.
- **Per-scope finding details are dynamic.** If packaged content *and* resolved composition are incomplete, render independent explanations. For definite failures, show actual violations; for unsupported, explain missing checking capability; for incomplete, explain missing inspection context. An incomplete check must not conceal a known failure. Passed scopes needn't produce meaningless accordions. Never hardcode mock findings.
- Health uses a **lighter page grammar**: open Definition/Metadata areas, thin dividers for lower-priority sections, a light Integrity & Provenance grouping where useful, separate Scanning and Artifacts, independently accessible verification command with Copy, and kind-specific support/readiness evidence. Tool target/provisioning evidence is not Agent Runner support.
- Overview Evidence is an honest, qualified **summary**, not a duplication of full Health content. Keep raw versioned manifest and pinned APDS reference separately accessible.

## 4. Landing — approved decisions and rationale

**Approved header:** **“Build AI agents the way software is supposed to be built.”** This is intentionally aspirational, not an invitation to obsess over npm analogies. The supporting copy now states the actual reuse problem: components scattered through project code; bring them together into versioned Agent Packages. **Do not revert to feature-only or package-manager-comparison copy.**

**Thesis:** **“Agents are software. Treat them like it.”** An Agent Package defines a complete system through Components, bindings, and workflow so it can be versioned, shared, and reused. The displayed `agent.json` is clearly labeled an **excerpt**; it isn't necessarily a standalone copyable full manifest. The companion publish/install/lock/run lifecycle helps a developer connect the new noun to familiar engineering practices.

**Signature visual moment:** The thesis includes a **small, deliberate composition visualization**: the real featured Agent's six Component families/identity marks converge toward an Agent Package identity. It reinforces composition, not a required runtime dependency graph. Short reveal on entering viewport and restrained hovers, not perpetual animation; support reduced motion. This experiment was explicitly preferred over an animation-heavy SaaS style. Avoid inventing a second lengthy visual section.

**Core four:** Section heading **“The four parts of Agent Package Management”**. Category-level terms are **Standard, Registry, Manager, Runner**, with **APDS, AgentPM Registry, AgentPM CLI, AgentPM Harness** as the corresponding implementations. This is about the *category*, not a proprietary four-step SaaS pipeline or “on your machine vs shared by everyone.” The final layout is **plain, airy columns with small icons and subtle vertical accents**, **not four boxed cards or a heavy connected-node diagram**. No mandatory sequence implied: the standard defines the common contract, the others are independent ecosystem roles. “Runner” is the role; Harness is AgentPM's Runner implementation.

**Hero and shareable Agent Package card:** The featured card is **a reusable, self-contained, potentially embeddable package identity** (analogous to a developer's portable gamer card), not a decorative hero graphic. Its exact version, composition and independently meaningful install command belong in the card; **do not trim the card's command solely to remove perceived duplication**. The extraneous unpinned package-install strip on the hero's left was removed, while the actual CLI installation remains the primary first action. Card reuse across detail, preview, sharing/OG, and the M11B/D16 embedding investigation should be considered. Do not claim embedding already exists.

**Start from where you are:** Three paths, with valid syntax and actual semantics:

- **Try one:** install an Agent Package **and** show `agentpm harness` so readers understand installation is not execution. Readiness/credentials may still be required; don't claim every package runs immediately.
- **Build one:** `agentpm init` defaults to an **Agent Package as of Stage 2**. Do **not** add `--kind agent` based on old CLI assumptions.
- **Start from a Template:** `agentpm new <template> .` includes an explicit destination (the approved example ends with `.`). Don't imply a Template runs like a Tool.

**Trending:** Retain the nine-card grid and visual hierarchy, but **merge the existing per-kind Trending results by a genuinely comparable ranking score**, show the actual top results across kinds, and use the previously planned low-activity fallback. **No curated “balanced” mix of kinds, no fabricated ecosystem scale, no misleading single-input metrics or false popularity window.** It's fine if all nine are Tools, or a particular kind is absent. Cards in the static mock are real package examples but do not establish a verified live cross-kind ranking.

**Deferred, not blocking Stage 2:** An authentic **AgentPM Harness TUI screen recording**, ideally following the Try flow with the future **AgentPM Developer Agent**, once Stage 3 produces a suitable demonstration. Do not simulate production activity or delay the current website to obtain it.

## 5. Explore — approved decisions and rationale

**Search and browsing should share a coherent product.** Claude's **row-based** search result treatment is approved; the two-up search-card alternative was rejected because rows support aligned names, dates, installs, descriptions, and kind metadata. Retain kind-colored left rail, generated package mark, clamped descriptions (generally two lines desktop / three on narrow), meaningful per-kind metadata and omission of unavailable fields. Do not tint whole rows or hardcode invented per-kind information.

**Default no-query `/explore` is not empty.** Show a **populated, browsable list** using genuine **cross-kind Trending** from the live Registry, with a truthful fallback when activity is weak. Use **the same row language as search** rather than switching to a separate featured-Agent card gallery. The mock previews a subset of real captured `q=zack` items sorted for demonstration: **that order is not a verified registry-wide Trending ranking** and must not ship as one.

**Three conceptual group shortcuts, with equal visual importance:**

- **Agent Packages:** complete authored systems (`agent`).
- **Components:** independent Tools, Skills, Knowledge, Memory Blueprints, Instruction Profiles, and Loops (six kinds).
- **Templates:** scaffolds to start projects (`template`).

An Agent Package is the category anchor on Landing, **not a privileged search ranking** on Explore. Visitors who use AgentPM only for Tools should have an equally good discovery path. **Namespaces** remain separately searchable/filterable entities, not a fourth artifact kind nor part of the claim that all results are “installable.”

**Group shortcut identity:** Use neutral monochrome versions of existing kinds/category icon language, without generated per-package gradients. The first six-in-one Components icon was **rejected as illegible at small size**. The approved Components mark is a **simpler three-shape collection** (square + ring + overlapping tiles) representing independent building blocks, *not* the Tool icon alone (which would misrepresent all Components as Tools). Templates use the folded-corner concept, subject to explicit component creation as needed.

**Filtering:** Preserve Stage 1's real filters, semantics, URLs, query text, back/forward/cursor, ordering, sort controls, and namespace/public-private correctness. The parent Components filter represents the six kinds; a specific child kind narrows correctly. Advanced kind-dependent groups start collapsed and can open contextually. Category shortcuts should invoke the same real filter state, not create a second filtering system.

**Contextual text:** The “Agent Packages, Components, and Templates in one list...” default message must change when users select Agent Packages, Components, Templates, or individual kinds; derive displayed heading/description from the **actual active filters**, not only from shortcuts. The contextual copy in the approved HTML is the reference. Result counts are truthful and scoped to current filters.

**Signed:** Drop repetitive Signed badges from every result row (all 52 items in the captured sample had them and they added little comparison value). **Retain the existing optional Signed filter under More filters for now**, and retain signing evidence on package detail. Do not confuse absent signature with unknown evidence.

**CTA:** Broad “Can't find what you're looking for?” with actions for `agentpm init` and browsing Templates. Covers someone searching for any kind, not only an Agent. The future AgentPM Developer Agent could later become an optional build path, **not** a live Stage 2 link.

**Important backend gate:** Enabling filtered no-query browse can change the query planner dramatically. Current Stage 2 tasks M13A explicitly flag unindexed JSONB facet predicates that previously relied on a narrowing text query. Preserve that investigation and make necessary index/denormalization/performance corrections **before** enabling all no-query filters against the full search index. Do not consider a static populated mock proof that the live query can do this safely.

## 6. Agent Package Detail — approved decisions and rationale

**Identity and Get Started:** The original two tall side-by-side cards were **rejected as uneven and box-heavy**. The selected treatment is a wide, unboxed package header: identity/name/description/metadata on the left; a quiet **At a Glance** on the right; **full-width horizontal Get Started below**, with Install → Run with Harness and optional SDK consumption. The content remains intact; the change is hierarchy and balance. Version selection is real, not static styling.

The `@zack/devwork-copilot@0.1.10` example has **6 direct Component dependencies (including its Loop), 3 Loop phases, and 7 authored transitions**. Those facts are a fast *preview* of deeper diagrams, not redundant analytics. They must be computed for the selected version, and skipped/replaced when an Agent omits its Loop or another dimension. Never manufacture three figures solely for a balanced grid.

**Overview order:** **Composition stacked above Execution Architecture** on desktop as well as mobile. The previous two-column pairing left dead space under Composition with longer Loops. Stacking also teaches: **what this Agent is made of → how that system can operate**. Composition receives full width; Execution's individual phase cards retain a comfortable reading width instead of stretching to fill the page. Technical panels here are acceptable because they communicate actual structured building blocks.

**Composition is explorable, not decoration.** One visible node/group per Component kind can reveal exact package identities, selected versions, and individual links; direct and Skill-inherited Tools must remain distinguishable. No misleading automatic-availability claim: bindings, inherited candidates, access restrictions and supported runtime are separate concepts. Avoid an unreadable one-node-per-package diagram for Agents with dozens of Tools, but don't hide exact version links behind decorative shapes. Use textual fallback for graph accessibility or missing resolution data.

**Execution Architecture is transition-aware, not a numbered 1→2→3 sequence.** The actual `devwork-triage-loop` has three phases and **seven outcome transitions**, including backward transitions to earlier phases and `$end`/`$handoff` terminals. Preserve authored phase objectives, outcome IDs, destination phase links, restrictions and meaningful capability summaries; `access.tools: false` in `resolve-or-handoff` must be visible rather than showing Tools as usable. Forward transitions can use `→`, backtracks `↶`; clickable destination names navigate within the view. **Remove the `↗` icon** that looked like an external link. No invented transition or false implication that declaration order determines execution order. Handle one-phase, branched, cyclic, Loop-free and long-Loop states.

**Sticky detail navigation (two independent triggers):** When original identity/name/version leaves view, show compact breadcrumb/name/version beneath global nav; *later* dock tabs directly beneath that strip when the tabs reach it. Scroll back reverses each state independently. Anchored phase links, Readme navigation and Health/Bindings jumps must land below both sticky surfaces. Keep the compact identity short on mobile and let tabs scroll horizontally. Do not rebuild a second global header.

**Bindings stays a separate tab, with a narrower job.** Overview communicates the **interpreted/effective system**: composition, inherited candidates, effective per-phase capabilities, Loop access and transitions. **Bindings is a compact inspector of the authored `bindings` object**: consumer context, global bindings and specific phase bindings. Collapsed phase rows show kind indicators only by default, because that answers “what kinds are declared here?” without a wall of nested blocks. Expand to see exact referenced packages, direct Memory spaces, and separately bound Memory lifecycle operations. Keep global-plus-phase additive behavior, preserve versionless binding identity, don't silently display inherited Tools as explicitly declared, and distinguish operation eligibility from automatic execution. Provide **View execution phase** cross-link to the correct Overview phase and **View binding JSON** to the selected version's source. Show named MCP Tool surfaces when declared; omit empty groups rather than rendering empty cards.

**Health & Security** uses the common treatment from §3.3. The initial Claude Health mock was considered too box-heavy. Final design uses lighter open sections/dividers, per-scope APDS status plus plain-English help, finding-specific expanders, independent verification, and observer attribution. **The latest Agent HTML contains Health AND Bindings together**—do not substitute an older Health-only mock.

**Remaining tabs:** Examples and Readme can carry forward their real existing behavior; adapt to the shared shell, not invented prototype content.

## 7. Tool Detail — approved decisions and rationale

**Test of generalization:** The Tool page intentionally looks like the Agent page in *shell and navigation*, but its primary inspection content is **Interface** + **Runtime & Execution**, not Agent composition or Loop orchestration. For this simple Tool, two columns work. **Do not require equal geometry across kinds**; deeply nested JSON Schemas may need full width, while Agent execution workflows are better stacked.

**Tool header:** Same airy identity + At a Glance + full-width Get Started and independently sticky identity/tabs. The `@zack/summarize-text@0.1.11` snapshot illustrates 2 input fields, 1 required environment variable, and **17 pinned Python distributions**. These are version-specific snapshot facts, not promises applicable to all Tools.

**Get Started:** Install → `agentpm run` → optional SDK loading. Do not use Harness for direct Tool invocation. The header shows a **generic environment-requirements notice linking to Environment**, not a huge list of required secrets. If there are no required env vars, omit the notice. Example data must come from the actual selected Tool schema or an author-provided usage example.

**Schema-derived starter command (approved refinement):** Prefer a valid authored example. Otherwise, derive a conservative starter JSON input from the selected version's `inputs` schema: required fields, actual enums/const/default/example values where appropriate, strings may use their authored description as a clearly labeled **placeholder** if it satisfies constraints, numbers/booleans use valid schema-compatible values, nested arrays/objects obey required/min/max constraints. **Validate the assembled input against the schema** before displaying a copyable command. Never invent semantically meaningful domain-specific payloads, credentials, or fake examples. If reliable generation is impossible for complex schemas (`oneOf`, conditional rules, refs, patterns, etc.), fall back to the actual supported `--input-file input.json` command and explain the file must conform to the schema. The mock's string is a starter, **not** a guarantee that the underlying Tool will execute successfully without credentials/runtime.

**Tool Interface outputs:** Render the actual authored JSON Schema (including `oneOf`/`anyOf` variants) with meaningful author-supplied labels if available, otherwise neutral **Variant 1**, **Variant 2**, etc. The Tool's `ok` key is *its own application convention*, **not an APDS-wide success/error field**. Don't label branches green “Success” or red “Error” based solely on key names, nor force every Tool into two output variants. This explanation is **Codex guidance in an HTML comment**, not paragraph copy that must appear to visitors.

**Environment:** Show all declared `environment.vars` entries, whether required or optional, with authored descriptions and safe public defaults where appropriate. Each row has a keyboard-accessible **Copy variable name** button. **Never copy or expose secret values**. Make the section link target reachable under sticky navigation. Runtime section preserves interpreter/runtime family/minimum, command/working directory/timeout, dependency metadata and readiness limitations.

**Resolved Environment:** Keep the expandable list of **17 pinned Python distributions** from this example. It showcases resolved dependency preparation and reproducibility. Don't assert every platform has identical wheels/ABI or that package-level pins prove a particular viewer's environment is ready. Read actual selected lock/resolution evidence, qualify platform-specific targets, and handle absent or unsupported resolution evidence honestly.

**Health & Security:** Reuse the **same** APDS-help disclosure visual language, per-scope finding accordions, evidence vocabulary and quiet metadata presentation as the Agent page, not a visually different implementation for Tools. Show target/platform/provisioning info where applicable, not Agent-only Runner support. The Tool mock didn't have every integrity record; do not invent missing signers/timestamps or attestations.

**Tabs:** Tool: Overview, Readme, Health & Security. No empty Agent-style Bindings/Examples tabs. Usable Tool examples can be offered within Interface/Readme where authored, without inventing a mandatory Examples tab.

## 8. What must *not* be inferred from these mocks

- A static HTML screenshot is not evidence that a component or API already exists in `agentpm-web`; names in comments are **references to inspect**, not guarantees.
- **Comment precedence within Tool mock:** one inherited `AT A GLANCE` comment still describes the Agent's 6 Components / 3 phases / 7 transitions. That is stale template commentary, **not** Tool data or an implementation rule. Follow the later Tool-specific At a Glance comment and the actual Tool snapshot (2 input fields / 1 required env variable / 17 pinned Python distributions); remove/repair the stale copied comment when incorporating the design.
- A mock click/toggle/scroll script is not the required production architecture or a license to bypass React state/router/ARIA patterns.
- A mock static trust badge is not an actual verification result; the package example may be legacy-inferred at one APDS level. Use real scope-specific evidence and dates for the selected release.
- “Signed” is not a universal guarantee. Dropping the repeated badge from search rows is not dropping proof or the advanced search filter.
- A Tool schema with a string `description` does not imply its suggested placeholder is valid for every string schema; constraints and validation govern generation.
- An Agent's Loop, Tools, Profiles, Memory operations, or runtime are **optional in the APDS definition**. A package can conform without being runnable by AgentPM Harness.
- A `kind: "template"` package scaffolds a workspace; it isn't a standalone Agent Package or an additional Component family.
- A declared Memory operation is not necessarily an exposed Memory space; a trigger becoming eligible is not invocation.
- A publisher-authored README is not a source of authoritative security or APDS conformance claims.
- The mocks are not permission to rewrite existing working pages, search sorting, filtering semantics, mobile layouts or signed JSON protocols.

## 9. Stage 2 planning-document reconciliation for Claude

**Stage status:** The project is **on M8A**. The following are proposed updates to **later** milestone text and D21 decisions; they are not claims that website implementation has begun or completed. Some original spec/tasks sections contain **explicit contradictions** to the reviewed HTML because they were written for the earlier floating/light `-layered` concepts. Reconcile surgically rather than discarding unrelated Stage 1 and APDS work.

| Planning area | Reconciliation to make | Guardrail |
| --- | --- | --- |
| **D21 / `spec.md` §§9–9.1 / M8B.1** | Replace old floating rounded global nav, white-card canvas, universal section-card rule and mandatory asymmetric detail layout with approved dark full-bleed header, selective panel weights, airy identity + full-width Get Started, and kind-dependent panel geometry. Record adopt/adapt/reject decisions against *latest four* HTML references while preserving shipped tokens and marks. | D21 remains a **review/sign-off gate**; don't silently declare it completed from these conversations. Single shared implementation; don't fork shells. |
| **M8B** | Reusable identity marks, category glyphs including proposed Template and simplified Components group mark, package-card densities, exact-version component links, accessible composition/outcome graph, responsive sticky-detail primitives, At a Glance with variable count. | Real DTOs/selected version. Preserve original package identity. Shareable Agent Package Card must work unaccompanied. |
| **M9A / M9B** | Agent: stacked, explorable Composition and nonlinear phase/outcome-driven Execution Architecture; improved unboxed header/Get Started; independent sticky compact identity and tabs; exact APDS/source links; clear effective vs authored semantics. | Keep Loop-free/no-deps cases valid, proper access restrictions and transitive inheritance, accurate phase graph. |
| **M10A** | Adopt shared detail shell **without** forcing Agent panels/tabs onto all kinds; use the cross-kind inspection matrix in §3.2 and Tool reference as a demonstrated pattern. | Preserve existing specialized kinds and read-only/unsupported states. |
| **M10B / Health requirements** | Carry over lighter evidence page, three plain-English APDS scopes, independent help, dynamic per-scope finding disclosure, attribution, verified/declared/inferred distinction, copyable independent verification, kind-specific compatibility evidence. | No fabricated status, signature, scan, platform support or local readiness. No aggregate health score. |
| **M11A / M11B** | Full-fidelity featured Agent Package card remains self-contained and shareable. Home Trending must be genuine **cross-kind** ranking without hand-curated kind balance. | Distinguish featured/curated from Trending measured if both appear on *other* routes. No fabricated rank. Keep off-site embed as investigation, not completed feature. |
| **M12A / M12B** | Lock reviewed problem-first hero/thesis copy, thesis composition animation, plain core-four roles, CLI/Harness and `agentpm init` default-Agent path, correct Template destination, light section rhythm, deferred Stage 3 TUI recording. | Don't reopen approved copy during generic copywriting. Check current CLI syntax/readiness. |
| **M13A** | Adopt approved three equal artifact-group shortcuts; row-based results; **populated zero-query cross-kind Trending list**, not a featured-Agent-first gallery; contextual filter-aware copy; neutral group marks; Signed badge absent from rows but advanced filter retained. | Preserve Stage 1 faceting/URL/sort behavior and **fix no-query JSONB facet performance issue** before unrestricted browse. Namespace is separate. |
| **M13B** | Use the same Agent Packages / Components / Templates taxonomy and row/card kind identity rules for publisher catalogs, without erasing namespace-level discovery or pins. | Namespace is not a ninth publishable artifact kind. |
| **M14A / M14B** | Treat reviewed landing/Explore/package copy and “Standard/Registry/Manager/Runner” distinctions as intentional approved wording; align remaining site/docs/pricing/SEO around it. | Factual verification still wins; approval isn't a license for inaccurate claims. |
| **M16A / M16B** | Implement real Try/Build/Template journeys and capture newcomer comprehension checks. Do not show an AgentPM Developer Agent CTA before its availability. | Current work is M8A; defer TUI Developer Agent recording to Stage 3. |

**Specific old-text contradictions to search for:** `floating rounded header`, `white rounded card`, `two-column body`, `asymmetric two-column`, `curated Featured row` as the default Explore experience, `2-up search result`, copying generic stock avatars, and the assumption that mock copy is always excluded. Some of these appear as **required** implementation checklist items in M8B.1 and need explicit replacement, not merely an added appendix that conflicts with them.

**D21 nuance:** Keep the formal adopt/adapt/reject audit and existing site baseline. Record that the owner has approved the four HTML **design references** while the formal D21 traceability, component audit, required evidence, and sign-off are still implementation work. Do not use these approvals to bypass acceptance criteria or other planned milestones.

## 10. Verification criteria to carry into implementation

These are handoff acceptance themes, not a replacement for the full Stage 2 test plan:

- **Landing:** Problem intelligible without reading documentation; distinct Agent Package/Component/Template language; shareable card works independently; true cross-kind Trending and fallback; animation respects reduced motion; valid Try/Build/Template commands; no mandatory Harness claim.
- **Explore:** No-query browse contains real live results; filter-only browse works and performs acceptably; Agent Packages, Components and Templates have equal entry points; child kind filters work; sorting/ranking and URL/cursor state stay correct; text search preserves Stage 1 ranking; missing metadata omitted; signed filter works despite absent row badge; query/no-results/error/loading/permission states.
- **Agent Detail:** Exact-version identity, direct/inherited links, no-Loop states, real nonlinear transitions and terminal labels, phase access restrictions, readable text alternatives, independently staged sticky bars with no name/version gap, sticky-safe anchor navigation, precise scope evidence, authored Bindings separate from effective capabilities, per-scope disclosure states.
- **Tool Detail:** Simple and complex schemas (required/enum/numeric bounds/oneOf/nested); valid schema-derived example or honest input-file fallback; no inferred success/error convention; required-env notice scales; every environment row has safe Copy name action; resolved packages/platform/ABI reality shown accurately; shared Health help and statuses; same sticky UX.
- **Cross-kind:** Meaningful 0–3+ At a Glance facts (only when real), kind-specific actions/tabs, no invented metrics or empty panels, real versioned README/manifest rendering, auth/private/public/legacy/unsupported states, keyboard/focus, mobile layout, reduced motion and SSR/SEO continuity.

## 11. Deferred or deliberately open (do not slip into Stage 2 scope)

- **Stage 3:** Real AgentPM Harness TUI screen recording featuring the eventual AgentPM Developer Agent and showing a truthful Try/run workflow. This is a future enhancement to the approved Landing, not a current dependency.
- **Later possibility:** AgentPM Developer Agent as a build CTA on Explore or getting-started pathways **when available**, never a dead button now.
- **Potential future enhancement:** Tool schema-driven input **editor/playground**. Stage 2 only needs honest starter-command generation and the current page treatment.
- **M11B/D16 investigation:** README/social/embeddable Agent Package Card capability. Preserve its full-fidelity independent content floor now; don't claim an embed service already exists.
- **Product implementation decisions still to check:** The exact source-backed data/DTOs, real renderer/component availability, cross-kind Trending score comparability, fallback policy, actual conformance evidence surface, Template identity component, tool schema complexity handling, and no-query facet query-plan performance. These are verification tasks, not reasons to reopen agreed visual choices wholesale.

## 12. Recommended review action for Claude

1. Read this handoff and **inspect all four latest HTML files** (including HTML comments), then read current `spec.md` and `tasks.md` at the **M8A-active** baseline. The handoff summarizes decisions that a screenshot alone cannot convey.
2. Identify any real semantic, factual, accessibility, Stage 1 regression, or cross-page consistency issue. **Do not reopen approved stylistic choices merely because the older plan differs.** If a decision genuinely conflicts with product requirements, flag it explicitly and explain why rather than changing it silently.
3. Make **surgical** updates to the affected spec/tasks milestones and D21 decision/evidence references. Preserve milestones, sequencing, existing cross-product requirements and test gates; don't turn visual approval into an implementation-complete checkbox.
4. Ensure the four filename references match the user's eventual renamed files; label superseded mocks as historical. Keep provenance/captured-data limitations and current API/CLI realities visible.
5. Return a concise change summary and any remaining genuine blockers for owner approval before Codex implementation.

**Design review outcome:** Landing, Explore, Agent Detail (including Health & Security and Bindings), and Tool Detail are **approved design references**. The substantive remaining work is implementing them faithfully and verifiably in the existing website, with cross-kind adaptation, real data, accessibility, and current Stage 2 planning reconciled first.
