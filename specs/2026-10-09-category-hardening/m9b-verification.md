# M9B — verification

User confirmed independent Claude review passed with no findings and owner visual
acceptance on 2026-11-27. **M9B is not yet complete: hosted CI remains pending.**

## Scope and sources

**S2-AC-11**, **S2-AC-12**, **D09**, consuming the approved **D21** shell.
Read the [decision record](m9b-design-decision.md),
[summary](assets/m9b/summary.json) and [baseline revisions](assets/m9b/baseline.json).
Approved reference: `assets/mocks/chatpgt-updates/03-agent-package-detail-approved.html`
and its adjacent design handoff. The pre-change Get Started and Examples content is retained in
`assets/m9b/preview/Before*.tsx`. The isolated Examples baseline uses the original
default Panel markup without its unused Next Link import; it is not a Next route
capture. The after preview renders actual new production components and CSS.

Production changes are confined to the Agent detail layout/Examples route,
selected-version action instructions and reusable action/setup/CLI guidance
components. Feature-local docs are in `agentpm-api/docs/v0.1/agent.json/agents.mdx`.
No package, schema, API, SDK, CLI or lock migration or release is needed here.
There were no production publications or model/Tool Runs.

## Acceptance and tests

| Requirement                                        | Evidence                                                                                                                                                                                                                                                                                                                        |
| -------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| T-DETAIL-02: valid no-Loop/no-dependency states    | Existing minimal Agent loader/Overview tests plus action/Examples tests: Install stays available, no fake Run control; copying authored prompts remains available.                                                                                                                                                              |
| T-DETAIL-03: nonlinear architecture and bindings   | Full regression includes the unchanged Loop/Composition/PhaseBindings/Bindings and M9A graph suites. No phase graph, permission or Memory semantics was rewritten.                                                                                                                                                              |
| T-DETAIL-04, S2-AC-12: source/standard/conformance | Existing selected-version Overview, raw-source/APDS, evidence guards, all-kind shell and SEO tests remain green. Declaration and evidence remain separate.                                                                                                                                                                      |
| T-PUB-09: shared compatibility copy                | New guard asserts 0.1.36, historical install qualification, publication versus Runner distinction and real docs links. Shared component is ready for M10A; M9B does not alter other kind routes.                                                                                                                                |
| Exact commands and SDKs                            | Prerelease/current-selected versus latest tests, copy bytes, actual production `agentActions` export and [13 real CLI/SDK checks](assets/m9b/action-contracts.json). Supported/legacy/no-Loop metadata loads succeed in Node and Python; missing model/credentials fail before execution.                                       |
| Actionable Examples                                | Authored multiline/metacharacter/script text is copied literally, escaped by React and never interpolated into commands. Each Run uses its matching input file. Multi-example assertion verifies exactly one shared setup/context note below the list. Empty/unsupported/unavailable/no-Loop states are tested.                 |
| Privacy and failure states                         | The route uses the existing request-cached authorized inspection, with no new endpoint or authorization path. Missing selected release is not found; unavailable graph preserves copying and suppresses Run. Existing private/partial graph suppression tests stay green. No live signed-in private browser session is claimed. |
| Responsive and accessibility                       | [Browser report](assets/m9b/browser/report.json): 1440/768/390/320, native Enter/Tab/disclosure and exact SDK/example copy, overflow before/after expansion; six qualified negative states at 390. Two authored examples are displayed. Native code scrolling can receive keyboard focus before Copy.                           |

## Executed commands

From `agentpm-web`:

- `pnpm test`: **500 passed, 78 files**, exit 0.
- `pnpm typecheck`: exit 0.
- `pnpm lint`: exit 0; six pre-existing warnings.
- `pnpm build`: exit 0, including frozen-standard reference check and Next production build.
- `pnpm exec prettier --check` on touched web source and feature MDX: exit 0.

From `agentpm-api`, with the same public synthetic zero-key fixture used by CI:

- `REGISTRY_PRIVATE_KEY_B64=AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA= .venv/bin/python -m pytest -q tests/test_docs_content.py`:
  **5 passed, 179 subtests passed**, exit 0. An initial run without this required
  fixture failed during app import; no private key was read or changed.

Logs are retained in [assets/m9b](assets/m9b). The full-suite run initially found
one existing all-kind test's unscoped Python-button selector matching both SDK
loading and SDK installation tabs. It now scopes the selection to the Load format
group; product behavior and coverage are preserved. The M8B.1 prose-floor guard
is unchanged and passes in the full suite.

## Browser and contract reproduction

The local API/web were not running. Instead, the task used an isolated local Vite
preview with the production action/Examples components, CSS and retained API-based
fixture props. This verifies these changed components; it does **not** claim full
Next routing/auth/Registry integration. The full production Next build and route
unit tests are separate evidence. The HTML mocks are layout references, not data.

To repeat the isolated preview, copy `assets/m9b/preview` to `/private/tmp/m9b-ui`,
then from `agentpm-web` run `node /private/tmp/m9b-ui/server.mjs` (port 3029).
That helper exports actual production action strings to `actions.json`. In another
terminal, run `node /private/tmp/m9b-ui/browser.mjs <output-directory> http://127.0.0.1:3029`.
The helper starts an isolated Chrome profile and blocks every request except
GET/HEAD. `--before-only` captures the prior revision separately. No Next or
production authentication source is stubbed or changed.

From `agentpm`, run `python3 specs/2026-10-09-category-hardening/assets/m9b/action-contract-probe.py`
while the exported actions file exists. It uses the existing SDK fixture builder,
local CLI binary, Node built SDK and Python SDK virtualenv. Fixtures/locks are
synthetic, linted, isolated in temporary directories and pinned to the requested
Agent identity. It runs JSON preflight, a headless missing-model rejection and
Node/Python metadata loads. It does not run a model or Tool or contact a Registry.
The additional missing-provider-credential case is separately checked.

## Remaining gates and risks

- Hosted CI remains pending; independent Claude review and owner visual acceptance are user-confirmed passed.
- Browser screenshots cover actual hydrated components with synthetic props;
  live Next/authenticated private journeys and remote model/Tool execution were
  deliberately not attempted. Reuse the release-band journey for full deployment acceptance.
- A public package page cannot inspect the visitor's machine. Run commands remain
  conditional instructions; local preflight/provider setup is required, and the
  page makes no promise that a selected package will execute successfully.
- Other kind pages adopt shared compatibility guidance in M10A. Full Health &
  Security detail stays in M10B; broader onboarding remains with its later milestones.

## Independent review and owner acceptance

On 2026-11-27, the user confirmed Claude review passed with no findings and
separately confirmed visual acceptance. These are operator-reported acceptances,
not builder self-certification. Hosted CI is the only remaining milestone gate.
No new local test run or deployment is claimed by this status update.
