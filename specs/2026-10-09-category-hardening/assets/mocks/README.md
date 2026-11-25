# Layout mocks (dark shell)

Working mocks for the Stage 2 web redesign, built as **self-contained HTML** rather than
images so they can be pasted into another model, edited, and diffed. They use the real
Tailwind class names from `agentpm-web`, so changes made here translate almost directly
into implementation.

| File | Covers | Status |
| ---- | ------ | ------ |
| `landing-page.html` | Landing / home | Direction settled |
| `detail-page.html` | Package detail, kind `Agent` | Direction settled |
| `detail-page-tool.html` | Package detail, kind `Tool` | Direction settled |
| `explore-rows.html` | **Explore / search results — the chosen treatment** | Direction settled |
| `explore-page.html` | Explore with 2-up result cards | **Rejected alternative, kept for comparison** |
| `composition-density.html` | Composition panel at three dependency densities | Resolved, folded into `detail-page.html` |
| `health-security.html` | Health & Security tab vs. the Overview Evidence section | Resolved, folded into `detail-page.html` |

The two Explore files are the same page differing only in the result list, so they can be
compared directly. **Rows won** (operator-confirmed 2026-11-23) and `spec.md` §9.1 and §13
were amended to match — the old "2-up search result" card variant no longer applies. The
card version is kept so the decision is not re-litigated, not because it is an option.

The `-layered` PNGs are superseded for **direction** everywhere in this set.

Open in any browser. Tailwind loads from CDN; everything else is inline.

---

## These are references, not the implementation

**The existing `agentpm-web` code is the baseline. This is a change to a live site, not a
rewrite.** Start from what is already there and change it, rather than building to these
files and back-filling. Styling ideas from the mocks are fair to reference wherever the
D21 adoption review chose them — the point is not that the mocks are off-limits, it is
that the existing code must not be forgotten on the way.

Preserved unless D21 deliberately decides otherwise, with the existing value winning any
unexplained difference:

- **Shared components** — `Panel`, `Badge.tsx`, the `ToolBox.tsx` mark components, the
  MDX/prose renderers, existing layout wrappers.
- **The spacing and gap scale**, the **breakpoint set** and where each layout switches,
  **container max-widths**, and the established **Tailwind utility conventions**.

`agentpm-web/CLAUDE.md` already treats changes that bypass the current
utility/component approach as a review finding. The failure to watch for is silent
divergence: a gap, radius or breakpoint from one of these files reaching the codebase
because it happened to be in the file, with nobody deciding to change the existing one.
Review the build as a **diff against today's site**, not as a match to these mocks.

### Mock plumbing that must not ship

Each file is standalone so it can be pasted into another model. That convenience carries
things which are scaffolding, not design intent:

- CDN `<script src="cdn.tailwindcss.com">`.
- Per-file inline `<style>` blocks redefining `.surface`, `.surface-sunken`, `.hero-band`,
  `.edge`.
- A hand-ported `hashString` → `mulberry32` → `gradientFromSeed` and vanilla
  re-implementations of the identity-mark shapes. Seven are ports **of** `ToolBox.tsx`,
  with gradient stops verified byte-identical across 12 seeds — **import the real
  components**. The eighth, Template, has no component to import yet.
- A hardcoded `© 2026` in the footer; compute the year.
- `href="#"` stubs throughout.

The design intent lives in the markup structure, not in the inline CSS or the scripts.

## Rules that are easy to break

Anything below is a decision already made against the specs or the shipped code. These are
the things most likely to get undone by a well-meaning edit.

### The site is dark

Production is `bg-slate-950 text-slate-100` with translucent `slate-900/60` surfaces. The
`-layered` PNGs are a **light-theme redesign of an already-dark site**; only their
*structure* was adopted. Do not reintroduce white card surfaces.

### Header keeps the shipped treatment

`sticky top-0 z-40 backdrop-blur bg-slate-950/70 border-b border-slate-800`, full-bleed —
not the PNG's floating rounded bar. On a dark ground the blur *is* the elevation cue; an
inset bar loses the blur edge that makes it read.

Layout is a three-zone grid: wordmark left, search taking all remaining middle space,
links and avatar right. Search collapses to its own row below `md`. There is no Explore
link in the nav.

### Kind tokens are fixed

Agent amber, Tool indigo, Skill rose, Knowledge fuchsia, Memory teal, Profile cyan, Loop
lime, Template emerald. Pattern is `bg-{c}-500/15 text-{c}-300 border-{c}-700/50`.
`sky` is reserved for the cross-cutting `Signed` badge and is not a kind colour.

### Identity marks keep their shape, not just their gradient

Seven of the eight marks are ported from `agentpm-web/src/components/ui/ToolBox.tsx` with
their real geometry. The eighth, Template, is **a proposal** — see the note under the
table:

| Kind | Shape |
| ---- | ----- |
| Agent | 2×2 quadrant grid, seeded by the Agent's **own Components** |
| Tool | plain gradient rounded square |
| Skill | two fanned cards with a drop shadow |
| Knowledge | cylinder with capped ellipses |
| Memory | rotated knot path |
| Profile | bust |
| Loop | thick ring |
| Template | folded corner — **proposed, not a port** |

`ToolBox.tsx` exports no `TemplateBox`. Without one, Templates fall back to the plain Tool
square and the two kinds become indistinguishable as soon as `TemplateCard`'s randomised
tint is dropped — and Templates are 11 of the 52 results in the live sample, so that
collision is not an edge case. The folded corner reads as "a starter you fill in", stays
legible at 36px, and collides with none of the seven real shapes. It is the one mark in
this set that is a design proposal rather than verified against shipped code, and is a
D21 decision.

`hashString` → `mulberry32` → `gradientFromSeed` is copied constant-for-constant and was
diffed against the shipped component across 12 seeds: **byte-identical stops**. Using the
wrong shape for a kind is a real error, not a cosmetic one — a rounded square on a
Knowledge package is showing the Tool mark.

### Trust claims are constrained by M7A / M6A

This is where the PNGs are furthest out of date, and the part worth checking hardest.

| `-layered` PNG | Here | Why |
| -------------- | ---- | --- |
| "Conformance verified" badge in identity row | removed | Conformance is a scoped result, not an identity attribute |
| "Package Health — all checks are passing" | removed | M7A forbids an aggregate `safe` / `healthy` / passing verdict |
| "APDS conformance — Verified" | "Passed", with levels and scope | Non-cryptographic checks are *passed*, not *verified* |
| "Registry attestation — Verified" | "Verified by Registry", key named | M7A: observer attribution is mandatory; a bare "Verified" badge is prohibited |
| One merged health card | three separate cards | M6A: definition conformance, Runner support and local readiness are different questions |

Two states the PNGs had no vocabulary for are shown deliberately: a resolved graph that is
**incomplete** because a dependency is `legacy_inferred`, and local readiness as **not
evaluated**, since model and provider configuration are properties of the viewer's machine
and cannot come from the Registry. Every integrity row names its observer.

Specifically: **do not render Agent Runner support as "Supported".** M7B reports
`runner_support: not_evaluated`. The landing hero says "Loop declared · readiness not
evaluated" for this reason.

### Terminology

An **Agent Package** is the packaged unit. A bare "agent" is the kind or the general
concept. "Install a whole agent" is wrong; "install a whole Agent Package" is right.
`agent.json`, `agent.lock` and `"kind": "agent"` are literals and stay lowercase.

---

## Landing page

**Hook:** "Build AI agents the way software is supposed to be built." The gradient on
*AI agents* is the shipped `components/landing/Hero.tsx` treatment. The frame carries all
three value themes (reuse, framework independence, composition) without picking one.

**Category is named explicitly** — "AgentPM is the package manager for AI agents" in the
hero, and `AGENT PACKAGE MANAGEMENT` as the eyebrow on the thesis section. Earlier drafts
failed review for not stating the category at all.

**This is a free dev tool, not a SaaS.** The primary CTA is getting the tool
(`brew tap agentpm-dev/tap` / `brew install agentpm`, with a Windows toggle), not a
signup. Register stays developer-direct. No blue marketing CTA button.

**Section weight.** Below the hero there is exactly **one** card: the thesis section
("Agents are software. Treat them like it."). Everything else is a plain section divided
by a top hairline. This is deliberate — when every block was a card, the card stopped
meaning anything, and nesting a code box inside a card inside a section read as packaging
rather than content. A card now signals "this is a package" (the Trending grid) or "this
is the thesis". Please do not re-card the other sections.

**Show, don't describe.** The thesis section is a real `agent.json` snippet with
kind-coloured dependency keys beside the lifecycle (`publish` / `install` / `agent.lock` /
`run` / `harness`). Two earlier prose-heavy versions were rejected for being text-driven.
Section prose is ~35 words.

**"How the pieces fit together"** splits Manager / Harness (on your machine) from
Registry / APDS (shared by everyone), each anchored to a concrete artifact. The grouping
carries the lesson; prose is necessary here because these are genuinely new concepts, but
it is ~15 words per entry. The Harness caveat — *"One way to run an Agent Package, and
other Runners may support the standard independently"* — must survive any rewrite.

**"Start from where you are"** is card-free on purpose. The command is the brightest thing
in each block (`slate-100` mono, sky `$` prompt) because it is what the reader came for;
label and description are the caption above it.

**Trending** shows nine packages, 3×3 on desktop. It carries **no per-card metric and no
stated window**, deliberately: the score is composite (installs, stars, and more), so
naming one input would misdescribe the algorithm and showing one number would present one
input as the rank. Both were tried and removed. Open items are recorded in `tasks.md`
M12B, including the requirement for a floor below which the surface falls back to recency.

---

## Explore

**Rows, not a 2-up card grid.** The reasons are structural and should survive restyling:
names form a single scan column rather than alternating between two; installs and dates sit
in a true right-aligned grid track so they compare without the eye zigzagging, and without
the `flex`/`mt-auto` workaround a 2-up grid needs to align them at all; one hairline-divided
container replaces twenty bordered boxes; and the full width absorbs the per-kind metadata
on the title line instead of forcing a fifth stacked row.

**Cards are still right for the no-query state.** That is browsing, not scanning — and it is
where Featured Agent Packages belongs (tasks M13A), not above live results where it ignores
the query and duplicates the list.

**Kind travels on a colour rail** at the row's leading edge, in the existing kind token.
It is the only kind signal that works peripherally: scrolling a mixed result set shows its
kind distribution without anything being read. Do not tint whole rows.

**Descriptions clamp to two lines from `sm:` up, three below.** Measured at real widths on
the real result set: two lines show 20/20 in full on desktop with only 5 rows needing the
second; one line shows 2/20 on mobile. Single-line `truncate` is the wrong default.

**Per-kind metadata is already in the search payload** — no API work needed. One grammar,
a label plus chips: Agent `Composed of`, Tool `Exposes`, Skill `Uses`, Loop `Phases` as an
ordered flow, Profile `Role`, Memory `Retrieval`, Template `Stack`, Knowledge `Mode`. Two
Tools in the live sample have neither actions nor env vars and correctly render no line at
all — that graceful omission is the behaviour to copy.

**The page header exists because Explore is where cold traffic lands.** The shipped route
has only `<h1 class="sr-only">`, which assumes arrival via the landing page.

## Detail pages

Three weight tiers rather than one uniform section card:

- **Primary** — Composition and Execution architecture. The thesis row.
- **Secondary** — Evidence. Lighter ground (`bg-slate-900/30`), smaller internal cards.
- **Plain** — Source. Just a rule and a heading.

The PNGs apply the same icon-chip + title + subtitle treatment to every section, so by the
third one nothing leads. Execution architecture is flattened from three nested levels to
two, because phase-card-inside-section-card-inside-column collapses badly on narrow
screens.

**Composition panel.** One node per *kind*, not per package, so the graph's shape is
constant whether a package has one Tool or thirty. A `+N` badge says there is more inside;
expanding reveals it without changing the shape. `composition-density.html` shows this at
three densities. The Loop is a distinct full-width binding rather than a sixth chip,
because `loop` is a single binding in the manifest, not one of the component arrays.

**Kind differences.** Tool detail drops the **Examples** and **Bindings** tabs — Tools have
neither. It uses the existing environment-variable support rather than a bespoke
"needs a key" section.

**Tabs.** Agent: Overview / Examples / Readme / Health & Security / Bindings.
Tool: Overview / Readme / Health & Security.

---

## Data

The two Explore mocks are built from a **live API response** — `GET /search?q=zack&type=all`,
52 results, captured 2026-11-23. Every name, version, install count, star, licence, date and
manifest fact in them is real, including the facet counts (`totals_by_type`: Tools 20,
Templates 11, Loops 4, Skills 4, Agents 3, Knowledge 3, Memory 3, Profiles 3, Namespaces 1).
Page 1 happens to contain no Knowledge result, and that is left as-is rather than tidied.

Everything else is real too, from `agentpm-examples`. The Agent detail page matches published
`@zack/devwork-copilot 0.1.10`: five direct Components plus the Loop binding, with
`@zack/github-issues@0.1.2` inherited through `issue-triage-playbook` as the transitive
edge. The Execution panel is generated from `devwork-triage-loop@0.1.0`'s actual phases —
note phase 3, `resolve-or-handoff`, has `access.tools: false`, so the Loop withholds tool
access at the decision point. The PNG shows a Tool chip there, contradicting the real
package; here the denial is rendered as its own state, because "cannot act here" is the
most interesting thing the panel says.

**Known placeholders**, so they are not mistaken for real data:

- Landing hero and section copy is placeholder pending **M14A / M14B**.
- Six of the nine Trending descriptions are written, not real. Only `devwork-copilot`,
  `ops-console` and `summarize-text` carry real descriptions. All nine versions are real.
- `@zack/research-assistant`'s quadrant seeds use only two known Components
  (`wikipedia-scrape`, `summarize-text`) cycled, so its mark will not match production.
  The other two Agents use their full real four.

---

## Mobile

Mocks are desktop-first but the breakpoints are real, so narrow widths are worth checking:

- Header search collapses to its own row below `md`; nav links collapse below `lg`.
- Hero stacks below `lg`; the detail thesis row stacks below `xl`.
- Identity mark drops to 56px and the package name scales `text-xl` → `text-3xl`.
- Evidence label/value pairs stack vertically below `sm` instead of fighting for one line.
- Trending grid steps 1 → 2 → 3 columns; onboarding columns stack with hairline dividers
  switching from vertical to horizontal.
- Dependency grid steps 2 → 3 → 5 columns; command blocks scroll inside themselves; tabs
  scroll horizontally. The page body never scrolls sideways.

Every file starts with `<meta charset="utf-8">`. Without it, browsers opening these over
`file://` fall back to windows-1252 and the `·` separators render as mojibake.

---

## Open questions

1. **Evidence placement.** It sits below the detail thesis row. Arguably it belongs in the
   right rail under Get started, where it is visible without scrolling.
2. **Phase outcome lists** are plain text. They are the edges of a state machine, so a
   small diagram may serve better once Explore's visual language is settled.
3. **No publish-side CTA** exists anywhere on the landing page. A visitor who wants to
   contribute has nowhere to go. May be deliberate while publishing is not open.
4. **Hero box density.** The hero stacks three bordered blocks (CLI install, package
   install, Agent Card). It is the densest box cluster left after de-carding; merging the
   two install blocks behind a tab row is the available next move if it reads heavy.
5. **`Signed` on every row.** All 52 results in the live sample are signed, so the badge
   differentiates nothing today and costs a chip slot on every row. Keep as a deliberate
   always-on trust marker, or show only on absence?
6. **"Used by" on Component rows.** Page 1 holds `@zack/devwork-copilot` and five of its six
   Components as unrelated peers — the biggest missed explanation on the route. Needs a
   reverse-dependency field the search payload does not have, so it is backend work, and it
   must read as supplementary context: Components are independently installable, and framing
   one as "part of an Agent" contradicts the positioning the page header states.
7. **No `TemplateBox`.** `ToolBox.tsx` exports no Template shape, so Templates fell back to
   the plain Tool square and the two kinds became indistinguishable once `TemplateCard`'s
   randomised tint was dropped. The folded-corner mark in the Explore mocks is **the one mark
   that is a proposal rather than a port** — every other shape is copied from `ToolBox.tsx`.
