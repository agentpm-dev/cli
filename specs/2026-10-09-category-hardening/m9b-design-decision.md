# M9B — D09 action and Examples completion

Builder decision, 2026-11-27. User confirmed independent Claude review passed with no findings and owner visual acceptance on 2026-11-27. User confirmed all hosted CI passed on 2026-11-27. M9B is complete.

M9A already delivered composition, nonlinear Loop architecture, scoped bindings,
Install/Runner commands, Evidence and Source. M9B verifies those through the full
web suite and completes the task-oriented actions and dedicated Examples tab.
The approved `chatpgt-updates/03-agent-package-detail-approved.html` and its
handoff remain the reference. This consumes D21; no new shell or design system
is introduced. Examples stay off Overview, per the owner’s M9A correction.

| Decision          | Selection and reason                                                                                                                                                                                                                                                                                                                                                    |
| ----------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Get Started       | Retain the approved unboxed three-column Install → Run / alternative SDK layout. Extract the existing section so it can reuse one selected-version instruction model. Put detailed setup behind native disclosures.                                                                                                                                                     |
| Runner gates      | Offer the local Runner command only for an inspectable Loop under a supported Agent interpretation. Otherwise keep the qualified `--json` preflight and explanation. Loop inspection never establishes local readiness; even the available case requires consumer setup.                                                                                                |
| Setup             | Same installed workspace/lock, model/provider and credentials, required backends/runtime services/scopes/approvals and consumer context. No secrets are displayed or copied. Host providers require machine mode; a plain command cannot supply a host callback.                                                                                                        |
| CLI floor         | A reusable `PackageCliGuidance` component carries the documented 0.1.36 declaration-tolerance floor. It explicitly separates publication requirements from installation support and Runner readiness. Latest authoring checks and updates link to existing feature docs. M10A adopts this presentation for other kinds.                                                 |
| SDKs              | Use the existing exact-version `loadAgent` / `load_agent` APIs after installation in the same workspace. Separate SDK package-install controls and links. These are definition/lock reads, not Runs. Unsupported Agent standards do not get a speculative current-standard snippet.                                                                                     |
| Examples          | Preserve authored text and titles, add a copy control per prompt and a native disclosure per usable Run. Instructions use `--headless --input-file prompt-N.txt`; the operator saves the copied prompt. This avoids placing arbitrary author text in shell syntax and works without shell-specific quoting. No browser execution or new input editor/download workflow. |
| Multiple examples | Each card has its own prompt and matching input filename. Runner setup/preflight and consumer-context note appear once after the entire Examples list, not once per prompt. The existing Get Started setup remains in the shared page header.                                                                                                                           |
| Missing context   | No-Loop, unsupported or unavailable workflows keep authored prompt copying, explain limits and omit runnable example instructions. Empty Examples does not invent prompts. Legacy interpretation remains qualified and does not become conformance.                                                                                                                     |

Stage 1 CLI, SDK, API, manifest/lock schemas, public protocols and auth contracts
are consumed unchanged. The shared action model is presentation only. Native
copy/keyboard controls, selected-release shell/SEO and all-kind behavior are
covered by the existing and new tests. Full Health & Security remains M10B.
