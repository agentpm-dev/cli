# Layout mocks (dark shell)

Working mocks for the Stage 2 web redesign, built as **self-contained HTML** rather than
images so they can be pasted into another model, edited, and diffed. They use the real
Tailwind class names from `agentpm-web`, so changes made here translate almost directly
into implementation.

| File | Route | Status |
| ---- | ----- | ------ |
| `detail-page.html` | Package detail | For direction review |

Open in any browser. Tailwind loads from CDN; everything else is inline.

These supersede the `-layered` PNGs for **direction**, not for scope — the PNGs still hold
the Explore and landing structure we have not mocked yet.

## What these change from `detail-layered.png`

The PNG's information order is kept almost entirely. What changed is everything that
depended on it being a light-theme comp, plus four content claims the specs had already
ruled out since those images were made.

**Shell**

1. **Dark, not light.** Production is `bg-slate-950 text-slate-100` with translucent
   `slate-900/60` surfaces. The PNGs are a light redesign of an already-dark site; only
   their *structure* is adopted.
2. **Header keeps the shipped treatment** — `sticky top-0 z-40 backdrop-blur
   bg-slate-950/70 border-b border-slate-800`, full-bleed. Not the PNG's floating rounded
   bar. On a dark ground the blur *is* the elevation cue; an inset bar loses the blur edge
   that makes it read.
3. **Hero-band texture**, scoped to the hero only — two faint radial washes plus a masked
   grid. This is how layering registers without the drop shadows a light comp relies on.
   Running it full-page would compete with the card surfaces.
4. **Kind tokens are the shipped ones**, not the PNG's hues: Agent amber, Tool indigo,
   Skill rose, Knowledge fuchsia, Memory teal, Profile cyan, Loop lime, Template emerald,
   with `sky` reserved for the cross-cutting `Signed` badge.
5. **Generated identity gradient preserved.** `ToolBox.tsx`'s FNV-1a → mulberry32 → two
   HSL stops is ported verbatim into the page script, so the avatar is the real mark. The
   PNG replaces it with a stock robot glyph, which conveys kind twice and loses identity.

**Density** (the M8B.1 decision, shown rather than described)

Three weight tiers instead of one uniform section card:

- **Primary** — Composition and Execution architecture. The thesis row.
- **Secondary** — Evidence. Lighter ground (`bg-slate-900/30`), smaller internal cards.
- **Plain** — Source. Just a rule and a heading; deliberately not a card.

The PNG applies the same icon-chip + title + subtitle treatment to every section, so by
the third one nothing leads. Execution architecture is also flattened from three nested
levels to two, because phase-card-inside-section-card-inside-column collapses badly on
narrow screens.

**Trust claims** (M7A, M6A)

This is where the PNG is furthest out of date, and it is the part worth looking at
hardest:

| `detail-layered.png` | Here | Why |
| -------------------- | ---- | --- |
| "Conformance verified" badge | removed from the identity row | Conformance is a scoped result, not an identity attribute |
| "Package Health — all checks are passing" | removed | M7A forbids an aggregate `safe` / `healthy` / passing verdict |
| "APDS conformance — Verified" | "Passed", with levels and scope | Non-cryptographic checks are *passed*, not *verified* |
| "Registry attestation — Verified" | "Verified by Registry", key named | M7A: observer attribution is mandatory; a bare "Verified" badge is prohibited |
| One merged health card | three separate cards | M6A: definition conformance, Runner support and local readiness are different questions |

The Evidence row also shows the two states the PNG had no vocabulary for: a resolved graph
that is **incomplete** because a dependency is `legacy_inferred`, and local readiness as
**not evaluated**, since model and provider configuration are properties of the viewer's
machine and cannot come from the Registry. Each integrity row names its observer, and the
footnote points at the local alternative
(`agentpm install --require-signature --require-attestation`) — described as a check you
can run, not one already performed.

## Data

Everything is real, from `agentpm-examples`, matching the published `@zack/devwork-copilot
0.1.10`: five direct Components plus the Loop binding, and `@zack/github-issues@0.1.2`
inherited through `issue-triage-playbook` as the transitive edge.

The Execution panel is generated from `devwork-triage-loop@0.1.0`'s actual phases. Note
phase 3, `resolve-or-handoff`, has `access.tools: false` — the Loop withholds tool access
at the decision point. The PNG shows a Tool chip there, which contradicts the real package.
Here the denial is rendered as its own state, because "cannot act here" is the most
interesting thing the panel says.

## Mobile

Mocks are desktop-first but the breakpoints are real, so narrow widths are worth checking:

- Header search collapses to an icon below `md`; nav collapses below `lg`.
- Hero stacks below `lg`; the thesis row stacks below `xl`.
- Identity mark drops to 56px and the package name scales `text-xl` → `text-3xl`.
- Evidence label/value pairs stack vertically below `sm` instead of fighting for one line.
- Dependency grid steps 2 → 3 → 5 columns; command blocks scroll inside themselves;
  tabs scroll horizontally. The page body never scrolls sideways.

## Open questions

1. **Tabs vs. the Overview page.** Composition, Execution and Evidence currently all sit
   under Overview while Health &amp; Security remains a tab. Evidence may belong only in
   that tab, with a one-line summary on Overview.
2. **Evidence placement.** It sits below the thesis row. Arguably it belongs in the right
   rail under Get started, where it is visible without scrolling.
3. **Loop in Composition.** Shown as a distinct full-width binding rather than a sixth
   chip, because `loop` is a single binding in the manifest, not one of the component
   arrays. Worth confirming that reads correctly.
4. **Phase outcome lists** are plain text. They are the edges of a state machine, so a
   small diagram may serve better once Explore's visual language is settled.
