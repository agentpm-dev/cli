# M8B builder verification — 2026-11-25

Builder implementation and local verification are complete. User confirmed
independent Claude review passed with no remaining findings on 2026-11-25.
User confirmed all hosted CI passed. **M8B is complete.** No production
deployment is claimed.

## Delivered scope and source contracts

[D09 decision/inventory](m8b-design-decision.md) records the preserved tokens,
standalone card content floor, approved reference mapping and layout choices.
The latest numbered `chatpgt-updates` HTML files and handoff guide the work;
`composition-density.html` remains current specifically for §10's density rules.

The web adds shared selected-release card models and full/compact views; kind/category,
generated identity, exact-version dependency, source, evidence and variable-count
At a Glance primitives; a bounded composition projection/view; and nonlinear Loop
and authored scope views. The only existing code refactors extract the unchanged
Agent seed helpers and Loop response schema into reusable modules, preserving their
old exports. There are no schema, CLI, SDK or API runtime changes.

The existing server read helper obtains version-bound Health with `cache: 'no-store'`.
It rejects subject mismatch and hides 401/403/404. Invalid/unavailable/private graphs
provide no partial identity inventory. Exact UUID links require matching authorized
kind/name/version DTOs; an absent or conflicting match stays explicitly unlinked.
The card uses the selected version, not the package's latest release.

Composition has at most six **direct-kind nodes**, each showing one concrete package.
`+N` reveals remaining direct packages below the graph, preserving its dimensions.
Absent kinds have no node or zero placeholder. Loop never has `+N`; contradictory
multiple Loops do not select an arbitrary representative. Inherited/transitive
dependencies group separately under the exact parent, preserving multiple paths.

Loop views preserve actual outcomes, backtracks, terminal destinations, approvals,
access declarations and additive scopes. Missing resolved context differs from an
explicitly absent binding. Legacy and unsupported standards remain qualified.
Memory spaces and separately bound operations are not merged into one authority.
The view does not compute local readiness or claim execution/definition verification.

Evidence primitives require a named check, Registry observer, trust context,
consistent label, a valid verification time and unexpired trust before rendering
verification. The observer/check remain visible. No bare Verified, quality score,
production-ready or safety claim is introduced. M8A's authoritative APDS projection
and raw-manifest/specification separation remain intact.

The captured fixture at `agentpm-web/src/test-fixtures/m8b/local-release.json` is
from four anonymous local GETs: the existing M5B Agent and its Health response,
plus its exact Skill and Loop DTOs. It includes all six Component families, direct
Tool v2 and Skill-inherited Tool v1. No production requests, new publications,
private-namespace reads or artifact downloads were needed. Synthetic nonlinear,
30-Tool and 64-Tool cases are test-only and labeled as such.

## Tests actually run

**Review clarification — Loop expansion exclusions:** comments immediately beside
each downstream exclusion point to the enforced singleton early return above them.
They document the no-`+N`/no-expansion policy rather than providing independent
validation. This annotation changes no emitted JavaScript; the source manifest
records the comment-only difference from the validated build input. No additional
behavioral tests were needed.

**Review correction — composition convergence:** the dependency fixed-point walk
stops after a full pass adds no reachable nodes. A deterministic work-budget test
uses the schema maxima (256 nodes, 4096 edges: 128 Skills with shared Tool
dependencies). It failed before the fix with 1,052,928 reachability lookups and
passes after the early exit. Reversed edges and disconnected cycles retain the
same reachability results; no fixed two-pass limit is imposed. This changes no
workflow or display data. All six SSR examples and their CSS were regenerated
and remain byte-identical to the retained browser capture inputs; see
[render equivalence](assets/m8b/convergence-render-equivalence.json).

**Review correction — APDS-LOOP-002 reachability:** `loopPresentation` now walks
outcome transitions from `entry_phase`, rejects unreachable declared phases and
requires at least one reachable terminal. Checkpoint rejection and error-policy
exits do not count toward either requirement. The view supplies an explanatory
unavailable state rather than rendering the invalid workflow. Declared and legacy
cases are covered. Cycles remain allowed, including a reachable closed cycle when
another outcome path reaches a terminal; this does not promise termination.

The new negative model/view cases failed before the fix (6 failures). After the
fix, isolated mutations disabling either new guard each produce 2 failed and
2 passed reachability cases. The checkout itself was not mutated. The full suite,
format/type/lint/build checks, API docs and component captures were refreshed.
Retained mutation output: [phase guard](assets/m8b/reachability-mutation-phase.txt)
and [terminal guard](assets/m8b/reachability-mutation-terminal.txt).

All commands below exited 0 after the recorded corrections.

| Command / environment                                                                                                                                                                  | Result                                                                                                                                                                                                                                                  |
| -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Web `pnpm test`                                                                                                                                                                        | **421 tests passed across 67 files**. Includes the existing search, shared detail, all-kind, APDS, privacy and SEO regression tests.                                                                                                                    |
| `pnpm exec vitest run src/components/packages/PackagePrimitives.test.tsx src/components/packages/PackageExamples.test.tsx src/lib/package-presentation.test.ts`                        | Targeted card/graph/Loop/DTO tests passed. The final full suite includes the additional multiple-Loop model regression.                                                                                                                                 |
| `pnpm typecheck`                                                                                                                                                                       | Passed.                                                                                                                                                                                                                                                 |
| `pnpm lint`                                                                                                                                                                            | Passed; six existing unused-variable warnings in blog, Hero, SkillCard, Stars and key utilities remain.                                                                                                                                                 |
| `pnpm build` with CI placeholder environment in an isolated source copy                                                                                                                | Passed on final source; frozen APDS checksum check, compile, lint, type validation and page generation completed.                                                                                                                                       |
| `M8B_CAPTURE_DIR=/private/tmp/agentpm-m8b-captures pnpm exec vitest run src/components/packages/PackageExamples.test.tsx`                                                              | Actual React SSR examples generated from the captured DTOs; no mock app/CDN or production route added.                                                                                                                                                  |
| `pnpm exec tailwindcss -i src/styles/globals.css -o /private/tmp/agentpm-m8b-captures/styles.css --minify`                                                                             | Existing Tailwind tokens compiled for reference captures.                                                                                                                                                                                               |
| `node scripts/check-package-primitives-browser.mjs /private/tmp/agentpm-m8b-captures`                                                                                                  | **18 captures/checks passed** at 390, 768 and 1440 CSS pixels with reduced motion. No body overflow; one versus 64 Tools and expanded 64-Tool graph dimensions match. Native inherited disclosure keyboard and actual phase-anchor focus checks passed. |
| API `PYTHONPATH=.:tests REGISTRY_PRIVATE_KEY_B64=AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA= AWS_EC2_METADATA_DISABLED=true .venv/bin/python -m pytest -q tests/test_docs_content.py` | **5 tests plus 173 subtests passed**. The key is an all-zero test fixture.                                                                                                                                                                              |
| Repo-native Prettier on changed/new TS/TSX/JS/JSON/Markdown/MDX                                                                                                                        | Passed; no Rust/Python implementation changed. Existing planning files receive narrow edits, preserving concurrent reference cleanup.                                                                                                                   |

The final build copy is
`/var/folders/rk/2xd4x19d0z59ly99hyx9s5lw0000gn/T/agentpm-m8b-build-qqbwg4r_`.
It excludes `.env*`, `.git` and shared `.next`, and links the installed dependencies.
The actual build environment was:

```sh
env API_BASE_URL=https://ci.invalid INTERNAL_KEY=ci-placeholder-not-a-real-key \
  CLERK_SECRET_KEY=sk_test_ci_placeholder_not_a_real_key \
  CLERK_ISSUER=https://ci.invalid CLERK_JWT_TEMPLATE=agentpm API_AUDIENCE=agentpm-api \
  NEXT_PUBLIC_CLERK_PUBLISHABLE_KEY=pk_test_Y2kuaW52YWxpZCQ \
  NEXT_TELEMETRY_DISABLED=1 pnpm build
```

The first sandboxed build could not resolve Google Fonts; the same isolated build
passed with font network access. Existing Browserslist/Next ESLint configuration
notices remain. The running user-owned dev server and its `.next` were not changed.

## Regression coverage and captures

- Exact selected version versus latest; all six kinds; distinct direct/inherited
  Tool versions; same artifact reached by both paths; missing/conflicting link IDs;
  malformed, duplicate, dangling and unreachable graph records; populated failed/
  incomplete signals; duplicate resolution signals; bound Health identity checks.
- Thirty direct Tools become one Tool node with `+29`, not thirty nodes. Keyboard
  expansion exposes the exact extra links outside the six-node graph. Singleton
  Loop never has a count button. Inherited-only Tools stay under their parent Skill;
  absent Tool/Knowledge/Loop nodes disappear. Multiple Loop guards are tested both
  in the model and independently in the view.
- Branch/cycle/terminal/approval behavior; missing outcomes and targets; additive
  Memory space/operation scopes; actual Loop access restrictions; APDS-LOOP-002 phase/terminal reachability; absent versus
  unavailable Loop; unsupported versus legacy standard interpretation.
- Each truth state, expired/future verification, malformed/misattributed evidence
  and an unavailable clock. Missing check/observer tests retain a matching label
  so another guard cannot accidentally mask the one under test.
- Exact pinned install command copies correctly; long identity text stays complete;
  variable At a Glance omits unknowns, preserves meaningful zero and supports 4+
  facts. Existing generated identities/tones remain in use.

[Browser results](assets/m8b/browser-results.json) and
[capture checksums](assets/m8b/capture-checksums.json) retain every referenced image.
[Source checksums](assets/m8b/source-checksums.json) pin the web sources and local
fixture; the final isolated build's TS/TSX files were compared byte-for-byte.
Representative comparisons:

- [Before mobile](assets/m8b/before-390.png), [after mobile](assets/m8b/after-390.png).
- [Before desktop](assets/m8b/before-1440.png), [after desktop](assets/m8b/after-1440.png).
- [Expanded text view](assets/m8b/expanded-390.png).
- [64 Tools collapsed](assets/m8b/stress-closed-768.png),
  [expanded](assets/m8b/stress-expanded-768.png),
  [one-Tool control](assets/m8b/single-768.png).

These are isolated component reference captures with existing compiled Tailwind,
not screenshots of a migrated production page. They use a system sans fallback;
production Inter remains unchanged. Static captures do not hydrate React controls;
React interaction tests separately cover `+N` keyboard state and clipboard behavior.
The browser checks use real viewport emulation, native inherited disclosures and
phase focus. Live page, screen-reader and interaction checks belong to M8B.1/M9/M10
once these primitives are adopted. M8B has no separate live-site manual gate;
reviewing the saved component captures is optional.

## Acceptance mapping and remaining work

| ID             | M8B evidence                                                                                                                                                         | Boundary still owned later                                                                                                              |
| -------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------- |
| S2-AC-11       | Exact graph mapping, fixed kind nodes/`+N`, parent-grouped inheritance, empty/private/missing states, nonlinear Loop and authored scope views                        | M9 adopts the actual Overview and settles representative order, large expansion policy, deep links and effective capability inspection. |
| S2-AC-12 / D07 | Selected manifest and pinned APDS links remain separate; declaration/legacy/unsupported and evidence states remain distinct                                          | M8A route already exists; M9/M10 integrate these primitives into their page changes.                                                    |
| S2-AC-16 / D09 | Shared serializable card facts, full/compact composition/identity/source/evidence primitives; standalone install-command floor; existing 1200×630 OG budget recorded | M11B's OG/embed feasibility, privacy/caching and implement-versus-defer decision remain required. No image service is claimed.          |

Global shell/D21 is M8B.1. Independently staged sticky identity/tabs, actual shell
offset integration and page adoption are M9/M10; this slice supplies focusable
outcome destinations and an anchor-offset hook without building a competing shell.
No page's search, auth, metadata, canonical rules, tabs or README were redesigned.

All M8B implementation, independent review and hosted-CI gates are satisfied.
Full end-to-end route adoption and off-site distribution remain assigned to later
milestones; they must not be inferred from M8B completion.
