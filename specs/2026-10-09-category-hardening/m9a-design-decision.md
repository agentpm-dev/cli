# M9A — D09 Agent composition and reading order

Builder decision, 2026-11-26. User confirmed independent Claude review passed with no other findings on 2026-11-26. Hosted CI remains pending.

The design authority is [03-agent-package-detail-approved.html](assets/mocks/chatpgt-updates/03-agent-package-detail-approved.html)
and the [handoff](assets/mocks/chatpgt-updates/AgentPM_Stage2_Web_Design_Decisions_Handoff.md).
[composition-density.html](assets/mocks/composition-density.html) supplements the
single-package-per-kind reference specifically for overflow. The older Agent mock
is not the implementation target. This consumes D21's shell, typography, cards,
canvas, kind tones and generated identities; it introduces no replacement shell.

| Choice                     | Implementation and reason                                                                                                                                                                                                                                                                                                                                                                                                |
| -------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Whole-system reading order | Open identity/At a Glance, full-width Get Started, Composition, Execution Architecture, compact Evidence, visible Source. Examples remain on their own tab per owner correction. Composition and execution remain stacked.                                                                                                                                                                                               |
| Bounded diagram            | One card per present direct kind, at most six; no placeholder cards for absent kinds. Loop is singular. A package-per-node diagram would grow with Tool fan-out.                                                                                                                                                                                                                                                         |
| Primary package            | First distinct direct package in resolver/authored declaration order within each kind. The display does not make a popularity or capability ranking.                                                                                                                                                                                                                                                                     |
| Overflow                   | `+N` opens the additional packages inside the kind card, as clarified by the owner against the density mock. The collapsed kind diagram remains bounded to six nodes; an expanded card may grow vertically, with a bounded scroll pane. First 20 additional packages, then Show all. Every relationship remains in the complete text/table disclosure; a component hash opens its containing group and bypasses the cap. |
| Inheritance                | Exact Tool identities stay grouped under their parent Skill. Shared packages are not duplicated in the node inventory; parent relationships are retained. Direct and inherited are distinct relationships.                                                                                                                                                                                                               |
| Links and source           | UUID plus selected version from authorized DTOs; ambiguous or unavailable identities have no guessed link. Raw manifest remains complete and separate from the supported APDS reference.                                                                                                                                                                                                                                 |
| Two docks                  | React measures the actual header/title/strip/tabs. Identity docks when the full name/version row leaves view; tabs reach their own sticky threshold later. Each reverses independently.                                                                                                                                                                                                                                  |
| Navigation                 | Existing router/version selector and tabs retained. Phase/source/hash destinations receive focus and clear all three surfaces. Agent README headings opt into namespaced, duplicate-safe IDs using the existing slug dependency; HTML/link safety remains unchanged.                                                                                                                                                     |
| Phase display              | Discovery from `entry_phase` over actual outcome edges is a reading order, not an execution sequence. Outcomes, cycles, terminals, approval rejection destinations and access policies remain visible. `→` and `↶` replace any implication of a linear pipeline.                                                                                                                                                         |
| Bindings                   | Overview joins additive authored scopes to phases with kind disclosures. The Bindings tab retains the complete authored reference, traversal order, authored Loop fallback order and unknown keys in source order. Ambiguous versionless bindings remain explicitly unresolved.                                                                                                                                          |
| Context and MCP            | Consumer-owned context accompanies install/Runner requirements, never capability grants. MCP is an outward export fact in execution and the complete Bindings reference. Empty and unavailable cases differ.                                                                                                                                                                                                             |

The data boundary is the existing Agent detail DTO, Health's complete authorized
current-resolution graph, and exact selected Skill/Loop detail DTOs. React cache is
request-scoped. Nested reads have concurrency four, never fetch archives and never
substitute `/latest`. A graph failure exposes authored references and a limited
state, not partial private graph contents or a success-shaped empty graph.

The approved HTML's fictional facts and action syntax are not product contracts.
Install and existing SDK snippets remain pinned. The owner subsequently requested
the verified Harness Run/preflight commands in M9A; these now preserve the selected
version, short configured-Runner copy and the consumer-context note. Minimum CLI
version/update guidance, expanded runtime/SDK onboarding and example actions remain M9B. The page makes no
local readiness, fresh cryptographic verification or historical lock claim.

The owner additionally authorized correcting M8B.1's refresh flash on landing and
detail pages. CSS now supplies full viewport dimensions and gutters before
hydration; client measurements refine scrollbar alignment. Search fallbacks reserve
their real height. The original grid spacing, opacity and washes are retained.
This exception does not reopen other page-specific milestones.

See [verification](m9a-verification.md) for actual contracts, screenshots, executed
checks, test-fixture qualifications and remaining gates.

## Owner visual review corrections (2026-11-26)

The initial interpretation that overflow should open below the diagram was wrong.
The density mock puts it inside each kind card. The owner confirmed this during
review; the current implementation and direct regression assertions follow it.
Inherited Tools still use a separate group per exact Skill, with a rose-colored
Skill identity, component count and independent disclosure. Individual kind cards
no longer carry package identity icons; the Agent root retains its generated mark.

The visible Source footer replaces the separate Package definition panel and
header resource/latest-stable line. Standard badges and Source use the same frozen
reference destination. Overview examples were removed at the owner's explicit
request; M9B will enhance the dedicated Examples tab. Full-width tab dividers,
consistent top spacing, reference section icons, readable header badges and compact
Evidence were corrected in this review. Deferred command and deep Health work is
explicit in M9B/M10B in tasks.md.

The later owner follow-up also requires phase pills to distinguish bound, empty
and explicitly denied channels. Tools and Knowledge are struck through for their
explicit false flags; direct Memory read/write are independently marked. Separate
Memory operations retain their own pill and are not disabled by direct-space
flags. Global plus phase scope and exact bound-Skill inheritance feed the counts;
unresolved inheritance never becomes a false "none bound" assertion. Missing
access declarations express no Loop opinion, not a permission grant.

Global declarations and MCP exports now share a compact block outside the Loop
view. A bad phase binding can disable workflow inspection without hiding a valid
authored global Profile. Mobile docking measures the actual name and picker edges
against both header rows; APDS result scopes stack on narrow screens.
