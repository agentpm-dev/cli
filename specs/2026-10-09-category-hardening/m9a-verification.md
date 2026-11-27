# M9A — Agent Overview verification

Builder evidence for **S2-AC-11**, **D09**, supporting **D21**. User confirmed independent Claude
review passed with no other findings on 2026-11-26; hosted CI remains pending; owner visual review is complete as confirmed on 2026-11-26; this record does not mark
the milestone complete or authorize a deployment.

## Latest review correction — authoritative current results

Claude found that the final focused selection omitted the existing M8B.1 prose-floor
guard. Six explanatory paragraphs still used `text-xs`; those now use `text-sm`
(14px). An explanatory MCP fallback that inherited 12px metadata styling also has
an explicit prose-floor size. Code, compact kind pills and scan labels retain
their existing sizes. The guard itself was not changed or weakened.

A fresh run against the corrected tree passed:

- `pnpm exec vitest run src/components/ShellReadability.test.tsx`: **2 passed**.
- `pnpm test`: **485 passed, 76 files**, exit 0.
- `pnpm typecheck`: exit 0.
- `pnpm lint`: exit 0, six existing warnings, no errors.
- Prettier on the touched production files: clean.

[summary.json](assets/m9a/summary.json) and
[source-checksums.json](assets/m9a/source-checksums.json) are now the authoritative
handoff record. The original 478-test snapshot is retained separately as
[initial-full-pass-summary.json](assets/m9a/initial-full-pass-summary.json).
The focused polish/states records are historical; they are not substitutes for
this fresh full-suite result. Browser captures and the production build were not
replayed for this typography-only correction and remain explicitly historical.
Claude re-review passed, as confirmed by the user on 2026-11-26; hosted CI remains outstanding.

## Delivered scope

- `agentpm-web/src/lib/get-agent-inspection.ts` joins existing selected-release
  Agent/Health/Skill/Loop DTOs, with request-scoped caching and four concurrent
  metadata reads at most. `agent-inspection.ts` supplies authored selectors,
  meaningful facts and phase reading order. It does not install or execute content.
- Agent `layout.tsx`, Overview, Bindings, tabs and README adopt the shared shell.
  `PackageDetailFrame.tsx` implements separate identity/tab docking and focused
  hash navigation. `PhaseBindings.tsx` provides progressive scope inspection.
  Shared Composition/Loop primitives preserve exact relationships and real edges.
- The complete selected `agent.json`, author README, Examples, Bindings and
  Security routes remain. Existing SDK snippets remain. The owner later pulled the basic verified Run/preflight commands into M9A; expanded onboarding and compatibility guidance remain M9B.
- Feature-local documentation is in
  `agentpm-api/docs/v0.1/agent.json/agents.mdx`. No API behavior, schema, migration,
  CLI, SDK contract, identity algorithm or dependency version changed.
- The owner-reported refresh flash was corrected in the shared HeroBand/CSS and
  search loading placeholders. This is the explicitly authorized M8B.1 carryover.

The [D09 record](m9a-design-decision.md) explains overflow, primary selection,
fallbacks and intentional differences from the approved HTML. Source/input pins
and executed evidence are under [assets/m9a](assets/m9a/summary.json).

## Contract and test map

| Requirement                                                     | Evidence                                                                                                                                                                                                                                                                                                                                                                                               |
| --------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| T-DETAIL-01 / AC-11: exact six-kind composition and inheritance | `get-agent-inspection.test.ts`, `agent-inspection.test.ts`, Overview tests, shared primitive tests and captured local M5B contracts. Direct Tool v2 and Skill-inherited Tool v1 remain distinct.                                                                                                                                                                                                       |
| Bounded diagram, large groups and shared identities             | Primitive/density tests, `CompositionNavigation.test.tsx` (60th-node hash), 256-node loader test (peak four reads), controlled browser 40-Tool version. The original pass checked detached expansion. The owner subsequently corrected this: `+N`/Show all now expand inside the kind card, retaining six kind nodes and bounded internal scrolling; focused regression tests cover the new placement. |
| T-DETAIL-02: no dependencies/Loop                               | Minimal selected release with zero meaningful dependency count, no invented phase graph or runnable claim; loader and page tests, real minimal before/after captures, controlled empty release.                                                                                                                                                                                                        |
| T-DETAIL-03: real Loop semantics                                | Existing nonlinear Loop validator tests plus LoopView/phase-order tests: outcomes, branches, cycles, terminals, approval rejection, Tool/Knowledge/Memory denial and bound Skill inheritance. No model or Tool is executed.                                                                                                                                                                            |
| Private/incomplete/unsupported                                  | Loader/page fixtures reject partial graph nodes, do not read nested metadata on unavailable graphs, distinguish authored references, do not substitute current semantics for unsupported declarations, and suppress arbitrary same-name version links. No live private signed-in session is claimed.                                                                                                   |
| Source and regression preservation                              | Overview full-source tests; all-eight-kind `PackageDetailShell.test.tsx`; existing SEO, Security, raw manifest, examples, binding and README tests.                                                                                                                                                                                                                                                    |
| Sticky, responsive and keyboard                                 | Isolated Chrome at 1440/768/390/320: identity-only and combined stages, reverse scroll, native phase/source jumps and focus below combined offsets. Controlled Bindings-to-Overview jumps and mobile README hash.                                                                                                                                                                                      |
| Selected release, legacy and unknown                            | Browser version selector, legacy/empty synthetic versions; layout tests for selected publication timestamp versus latest stable and unknown; no Health/signature proof invented.                                                                                                                                                                                                                       |
| Owner refresh report                                            | Landing, Agent and Tool at 1440/768/390 with external hydration bundles blocked: grid already spans the viewport. Reload/hydration preserves full width without body overflow.                                                                                                                                                                                                                         |

## Executed checks

Run web commands from `agentpm-web`:

```sh
pnpm test
pnpm typecheck
pnpm lint
pnpm build
```

Final exits, counts, logs and source SHA-256 values are in
[summary.json](assets/m9a/summary.json) and
[source-checksums.json](assets/m9a/source-checksums.json).
Lint has six existing warnings; no new errors. The production build runs from a
temporary source copy with placeholder private configuration, the read-only local
fixture API and a separate `.next`; it does not disrupt the operator's dev app.
The original production source is built before the separate auth-isolated fixture
copy described below. Tests and typecheck run against the workspace source.

Documentation check from `agentpm-api`:

```sh
REGISTRY_PRIVATE_KEY_B64=AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA= \
REGISTRY_KEY_ID=docs-test-only \
  .venv/bin/python -m pytest -q tests/test_docs_content.py
```

Touched web/MDX/evidence source files are formatted with the installed web Prettier.
Frozen APDS and generated identity source remain unchanged.
The documentation import requires a test-only key environment variable; the bare
pytest attempt stopped at collection before this fixture configuration was supplied.

## Browser evidence and reproduction

The actual local app and Registry remain on ports 3000/5000. The subject is the
existing local M5B multi-kind Agent, with no new publication:

```sh
node scripts/check-agent-detail-browser.mjs \
  ../agentpm/specs/2026-10-09-category-hardening/assets/m9a/browser \
  http://localhost:3000
```

[Actual-app report](assets/m9a/browser/report.json) and captures include desktop,
tablet, mobile, phase jumps, and pre-hydration landing/Agent/Tool backgrounds. The
captured local M5B release has no authored phase bindings; that limitation is
explicitly recorded rather than counted as a Bindings success.

The [before captures](assets/m9a/before/report.json) use an existing minimal local
M4A Agent on the original source; [minimal after](assets/m9a/minimal-after/report.json)
uses the same UUID/version after the change. The approved static mock has its own
[reference capture](assets/m9a/reference/report.json); its values are illustrative.

`scripts/m9a-registry-fixture.mjs` serves retained M8B local DTOs with explicitly
synthetic bindings and four release variants. It is GET-only, never proxies the
real Registry, and rejects mutation. Run it with an output file for its allocated
port, then configure an isolated web source copy's `API_BASE_URL` accordingly.

For the controlled build only, Clerk middleware/provider, analytics identity and
the unused StarRail sign-out callback are isolated using the exact retained
[transform](assets/m9a/controlled-auth-isolation.txt). All package presentation,
fetch/projection, router and sticky code remains the workspace source. Initial
attempts with placeholder credentials could not hydrate Clerk and were not counted
as passing evidence. **This fixture build is not authentication verification.**
The normal real-app browser pass and existing auth/transport tests are separate.
No real Clerk secret, user session or `.env` file was copied.

```sh
node scripts/check-agent-detail-browser.mjs \
  ../agentpm/specs/2026-10-09-category-hardening/assets/m9a/controlled-browser \
  http://localhost:3019 \
  /agents/e5ac9c75-3ba3-42fa-b283-03f40136f1ab/v1.0.0/overview \
  --require-bindings --skip-first-paint --fixture-matrix
```

The [controlled report](assets/m9a/controlled-browser/report.json) must pass all
four widths, source disclosure/focus and cross-tab phase jumps; the additional
version matrix tests 40 Tools, expansion, legacy, empty release and README focus.
Fresh isolated Chrome profiles block every non-GET/HEAD request, including
analytics. No production Registry request, publication, install, mutation or model
call is performed by these checks.

## Risks and remaining acceptance

- Resolution is current and authorization-scoped, not the historical publisher's
  lock. Missing nested metadata or an ambiguous versionless binding remains
  qualified, with complete authored source available.
- Metadata fan-out is bounded to four reads at a time, but a maximum-size graph
  still needs multiple round trips. No new API aggregation endpoint is introduced.
- Desktop/mobile Chromium and semantic tests passed; Safari/Firefox, a live
  authenticated private browser session and hosted CI remain unverified here.
- Claude must inspect the real contracts, implementation, screenshots and D09
  decisions. Operator visual acceptance and hosted CI still precede completion.
  No M9B or other page-specific milestone is marked complete by this work.

## Owner polish follow-up (2026-11-26)

This follow-up supersedes the initial visual treatment and its detached overflow
interpretation. Initial full-suite/build/browser evidence above remains historical;
those broad checks were not all repeated for cosmetic changes, as the owner
requested. Current focused check results and source pins are in
[polish-summary.json](assets/m9a/polish-summary.json); the
[desktop/mobile captures](assets/m9a/polish-browser/report.json) cover the updated
header, Composition, Execution, compact Evidence and visible Source.

| Owner points | Result / boundary                                                                                                                                                                                                                                                                                                                                                                                                           |
| ------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 1, 12, 13    | Removed latest-stable/resource header line and standalone Package definition panel. Added visible Source heading, version-pinned raw manifest/APDS reference buttons, with complete JSON still inspectable.                                                                                                                                                                                                                 |
| 2            | Readable 12px header pills, kind dot, separately styled declaration marker; the standard badge uses the same supported reference as Source. Legacy/unsupported remain qualified.                                                                                                                                                                                                                                            |
| 3            | At a Glance uses the approved composition/workflow description and a tighter title-to-description gap. Values remain actual selected-release facts.                                                                                                                                                                                                                                                                         |
| 4, 6         | Approved package/layers/lightning section glyphs and brighter Get Started helper text.                                                                                                                                                                                                                                                                                                                                      |
| 5            | Numbered 02 Runner column and honest Loop/model requirements. The owner later requested this command in M9A: current CLI help/source confirms the positional exact-version selector. The page now has a copyable Run command for an inspectable Loop, a labeled `--json` preflight otherwise, and the consumer-context note. No Run or paid model request was made; richer onboarding/minimum-version guidance remains M9B. |
| 7            | Browse components opens this Agent's complete selected relationship reference locally, with focus; it no longer sends the reader to general Explore.                                                                                                                                                                                                                                                                        |
| 8, 9         | No identity icons on kind cards; kind-tinted overflow with chevron expands inside the card. Inherited Tools use dashed pills under separate exact Skill groups; rose/red Skill name/version and component count remain.                                                                                                                                                                                                     |
| 10           | Loop heading/facts and compact actual phase/outcome cards follow the reference. Only the entry phase uses the larger entry arrow; other phases use a clear filled-dot marker. Phase-specific chips are separated from a global binding area rather than duplicating global scope in every phase. Denials, additive semantics, backtracks, terminals and raw policies remain available.                                      |
| 11           | Three bounded Evidence rows: APDS conformance, integrity/provenance and Runner support. Real evaluated scopes and unknown states remain; scoped verification detail is disclosed, not replaced by an invented aggregate Verified badge. The same APDS projection/status guard now feeds Security and the summary. Full evidence presentation is explicitly M10B.                                                            |
| 14, 15       | Install icon added. License still requires actual license data. Existing singular/plural star behavior remains, tested.                                                                                                                                                                                                                                                                                                     |
| 16, 17       | Full-width top/bottom tab dividers and top content spacing consistent with other detail pages; sticky navigation remains one existing frame.                                                                                                                                                                                                                                                                                |
| 18           | Overview examples removed by explicit owner instruction. Dedicated Examples route remains; M9B's action/copy work is now explicitly tab-only.                                                                                                                                                                                                                                                                               |

The local screenshot subject has one package per kind and no phase bindings; no
fictional chips were added to resemble the richer mock. Inline large-group
placement/keyboard/deep-link behavior is asserted in focused tests. Current
recording does not claim a new full production build or hosted CI result.

### Binding-state and mobile follow-up

The local `@zack/bound-agent@0.1.0` binds a Tool to `triage`, but its selected
`@zack/incident-response-loop@0.1.0` declares `assess`, `execute`, `review`. This
source mismatch warrants a qualified workflow fallback. The warning now names the
missing phase, and valid authored global Profile/MCP declarations remain visible
in the compact combined block. No published manifest was edited. See
[local case evidence](assets/m9a/global-bindings-followup.json).

Phase pills now cover bound, none-bound and explicitly denied states. Missing
access is no Loop opinion; separately bound Memory operations do not inherit
direct read/write prohibitions. Counts include applicable global/phase scope and
Tools inherited from exact, unambiguously bound Skills. Targeted tests cover these
semantic distinctions and the consumer-context/command rendering. Backtrack
arrows use the same lime color as their destination links.

The [mobile check](assets/m9a/mobile-followup/report.json) captures the real
wrapped-name/picker case: the global header is 116px high; the compact identity
starts at its lower edge when the first real identifier clears it, before the
wrapped row as a whole disappears. APDS scopes now form aligned vertical rows.
Only this focused mobile check and the affected tests/static checks were replayed;
there is no new full matrix, production build, hosted CI or independent review
claim. The [latest follow-up summary](assets/m9a/states-summary.json) records commands
and source pins. Existing browser evidence remains scoped to its earlier captures.

### Explicit phase states and usable Runner commands

The owner requested these follow-ups during visual review, including pulling the
basic command controls into M9A. Current CLI source and `harness --help` confirm
`agentpm harness [OPTIONS] [AGENT]`, with exact `@owner/name@version` selection.
Available workflow inspection shows the copyable Run command; an unavailable/no-Loop
case shows a clearly labeled `--json` preflight instead. Both retain declared
consumer context below the command; valid-case copy says configured **Runner**.
No command was executed beyond help; there were no model calls or publications.

Focused tests distinguish explicit Tool/Knowledge denial, empty scope and missing
access policy; Memory read/write denial versus separately bound operations; partial
Memory restrictions; and exact inherited Tools from a bound Skill. The denied
state is accessible text as well as a strike-through. Global scope contributes to
phase counts, while global-only Profiles remain in the separate reference block.

The latest [states summary](assets/m9a/states-summary.json) includes 105 passing
tests in five affected files, typecheck/lint and the single-width mobile identity/
Evidence check. Older broader results are retained historically rather than
counted as fresh checks on each styling edit. Independent review subsequently passed,
as confirmed by the user on 2026-11-26; hosted CI remains pending.

### Earlier independent review handoff (historical)

The owner confirmed the visual review is complete on 2026-11-26 after the final
license check. The current focused checks pass, source pins match, and no builder
blocker remains before Claude review. Hosted CI and Claude acceptance are still
pending. This earlier handoff used `states-summary.json`; the authoritative current full-suite record is now `summary.json`/source pins. Review those alongside the
approved Agent and density mocks; older build and matrix results are explicitly
retained as historical evidence rather than fresh final-revision results.

### Namespace attribution follow-up

At the owner’s request, the metadata row now identifies the owning namespace by
its exact `@handle`, using the same namespace destination and an initial derived
from that handle. It no longer presents the publishing user’s display name as
the namespace. Header tests cover both a different publisher display name and
an absent publisher display name. Current full-suite results/source pins are in
the main summary; Claude re-review passed, as confirmed by the user on 2026-11-26; hosted CI remains outstanding.

### Independent review accepted

The user confirmed on 2026-11-26 that Claude review passed with no other findings,
after the prose-floor correction and namespace attribution update. This is
operator-reported independent acceptance, not builder self-certification. All
implementation/review checklist work is complete; hosted CI is the remaining
milestone gate. There is no deployment claim.
