# M8B — D09 Card and composition primitives

Builder selection: 2026-11-25. User confirmed independent Claude review passed with no remaining findings on
2026-11-25. User also confirmed all hosted CI passed; M8B is complete.

## Design authority and inventory

The four numbered HTML files and
[handoff](assets/mocks/chatpgt-updates/AgentPM_Stage2_Web_Design_Decisions_Handoff.md)
in `assets/mocks/chatpgt-updates/` are the current references. The HTML files
directly under `assets/mocks/` are superseded where these references differ.
`assets/mocks/composition-density.html` remains current specifically for §10's
one-node-per-kind and `+N` density behavior, as clarified during implementation.
This selection does not grant or replace the later D21 shell adoption gate.

| Existing source                                                                           | Preserved treatment and M8B use                                                                                                                                                                                            |
| ----------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `agentpm-web/src/app/layout.tsx`, `Header.tsx`, `Footer.tsx`                              | Existing dark slate canvas, Inter font, full-width sticky navigation and auth/search behavior. No shell changes.                                                                                                           |
| `ui/Panel.tsx`                                                                            | Existing rounded-2xl surfaces, slate-900 background, slate-800 border and BASE/RAISED/OVERLAY opacity layering. Full and compact cards reuse RAISED.                                                                       |
| `ui/Badge.tsx`, existing kind cards                                                       | Agent amber; Tool indigo; Skill rose; Knowledge fuchsia; Memory teal; Profile cyan; Loop lime; Template emerald. Sky remains a separate Signed signal.                                                                     |
| `ui/ToolBox.tsx`                                                                          | Existing deterministic gradients, layered Skill shape, Agent mosaic and kind-specific generated marks. No algorithm/geometry changes.                                                                                      |
| `ui/AgentCard.tsx`, `SkillCard.tsx`                                                       | Existing seed interpretation is preserved. Agent seed functions moved without semantic changes into a shared module, with old exports retained. Skill composition marks use the same package + first Tool seed convention. |
| Eight detail layouts, `ToolDetailPanel`, `VersionToggle`, `StarRail`, `PackageDefinition` | Existing Stage 1 layout patterns and M8A definition links remain in place. There is no existing single `PackageDetailShell` component to replace.                                                                          |
| `CodeBox.tsx`, `AppLink`, `lib/seo.ts`                                                    | Existing copy/scroll behavior, route mapping, canonical/SEO conventions reused. Command text is exact-version data; source/security routes are structural helpers.                                                         |
| `public/og.png`, `publicMetadata`                                                         | Current OG is a static 1200×630 image, not an existing package-image rendering service. New serializable card facts reserve that size for later consumers; no OG endpoint is introduced.                                   |

Existing production is dark only. A new light theme would expand scope. New body
copy uses the existing slate palette at 14–16px; small kind pills retain the existing
Badge treatment. M8B.1 still owns site-wide typography/token consolidation.

## D09 selection: shared facts and composable views

Use a serializable selected-release model with small React primitives, rather than
one component that fetches data, owns the page shell and encodes every page layout.
`AgentPackageCard` provides full and compact densities over the same model;
composition, Loop, kind/category, identity, source, evidence and At a Glance views
can be used independently by the later page milestones. Interactivity stays in the
existing copy component, small React expansion state and native inherited-group
disclosures. No diagram library, client-side
API fetch, global state or new dependency was necessary.

The full standalone content floor is **identity, publisher, purpose, selected exact
version, real composition or its unavailable state, qualified declaration, separate
source/evidence links, and a pinned install command**. Installation does not establish
Runner/local readiness. This is the floor for M12A's hero and M11B's embed inquiry;
removing the command merely to balance the hero would violate it. Compact in-site
cards may omit the command while retaining the same underlying facts.

An image/OG consumer must make its own density/accessibility decisions inside the
1200×630 budget. The full HTML card is not asserted to fit unchanged into that
image. M11B must resolve off-site visibility, pinned/latest semantics, caching,
alt text and implement-versus-defer; this milestone does not claim an embed service.
Private and unknown visibility are explicit in the model.

## Composition and execution interpretation

The direct graph has at most six nodes, one representative per present Component
kind. `+N` means the remaining direct packages, not the total. React controls open
exact-version lists **below** the graph, keeping its shape fixed. Absent kinds
vanish; Loop is a single binding and never gets a `+N`. Multiple resolved Loops
produce an unavailable/inconsistent view instead of arbitrary selection.

Inherited/transitive dependencies are grouped separately under each exact parent,
including two versions of one Tool and multiple paths to the same exact artifact.
Representatives currently retain supplied resolution order; M9A still settles
manifest-versus-alphabetic order, large-list caps and deep-link expansion policy.
Current Registry resolution
is labeled as such: it is not a historical lock or downloaded-byte verification.
An unavailable/private/inconsistent inspection supplies no partial graph or zero
counts. Only an available empty graph says no dependencies.

The Health graph supplies name/version/digest but no package UUID. Exact authorized
DTO identities supply links; missing/conflicting IDs remain readable unlinked
identities rather than guessed routes or an arbitrary first match. The existing
Agent DTO plus its exact Skill DTO proves all six families and both Tool versions.

Loop rendering is an outcome adjacency/text view, not a row of phases implying a
linear sequence. Branches, backtracks, all three terminal destinations, approval
rejection destinations, error policies and access declarations remain actual facts.
Scopes combine additively; Memory spaces and operations remain separate. Missing
context differs from no binding. Unsupported standard declarations do not receive
an assumed v1 interpretation. Legacy interpretation is qualified.

This is a projection for inspection, not a replacement APDS validator, local
capability evaluator or execution proof. Effective capability/page integration is
M9; the primitives deliberately label authored scope candidates and restrictions.
The same text lists/disclosures remain available to keyboard and screen-reader
users, so graph density cannot remove the underlying explanation.

## Boundaries and differences from the mocks

- Retain existing generated identities and tones; category marks are neutral.
  Template's proposed folded corner remains for D21, and its existing specialized
  identity is not replaced with a made-up `TemplateBox`.
- Use React `aria-expanded` controls, native inherited-group disclosures and linked
  outcome lists rather than mock listeners or copied static graph coordinates.
  Expanded package lists are outside the graph, preventing both graph growth and
  the narrow-width overflow caught during implementation.
- Composition stacks above execution in the examples, matching the approved Agent
  reference. No universal side-by-side Agent layout is introduced.
- Existing dark panels are used for structured cards/technical groups. No new global
  section-card system or gratuitous page-wide container hierarchy is introduced.
- Card/Loop/source examples use captured local real contracts, with synthetic cases
  clearly labeled. No mock package, score, date, readiness or installation is a
  production default. The captures use the system sans fallback; production's
  existing Next/Inter setup is untouched.
- Sticky identity/tab staging and global header offsets require the actual shell.
  M8B provides focusable outcome destinations and an anchor-offset hook; M8B.1/M9
  still own the staged sticky surfaces, sticky-safe placement and route adoption.

D07 remains the M8A pinned `/standards/agentpm/1.0.0` catalog and separately versioned
raw manifest route. Unsupported declarations never fetch an author-supplied URL.
Conformance is not inferred from declaration, and a verified evidence primitive
keeps its named check, observer and trust context visible.
