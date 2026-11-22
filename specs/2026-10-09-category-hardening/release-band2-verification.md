# Release Band 2 verification

**Complete — 2026-11-21, America/Denver** (verification timestamps are UTC
2026-11-22). Covers M5A–M6B, S2-AC-06–09 and D06/D13/D20.

The operator confirmed all hosted CI passed, both SDK `AGENTPM_CLI_REF`
overrides were removed, CLI 0.1.47 was released/verified, and the Registry,
SDKs and website were released. The checks below independently verify the public
release artifacts and deployed behavior. Private repository-variable settings,
Render deployment IDs/configuration and the hosted CI matrix remain
operator-reported evidence.

Raw evidence is retained locally under ignored
`release-band2-evidence/20261122T020408Z-a5f10895/`; `summary.json` reports pass.
No paid model requests or package publications occurred during verification.
Production calls were public health/status/resolve reads and ordinary install
sessions for already-published packages. Existing operator test runs and example
consumer apps were untouched.

## Publication and distribution

| Surface                                                                | Verified result                                                                                                                                |
| ---------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------- |
| [CLI release](https://github.com/agentpm-dev/cli/releases/tag/v0.1.47) | Published, non-draft, non-prerelease; all four target archives and checksum files uploaded.                                                    |
| Homebrew                                                               | Installed executable reports `agentpm 0.1.47`; current tap formula selects 0.1.47 and its three payload hashes match GitHub release digests.   |
| Scoop                                                                  | Current bucket selects 0.1.47; Windows archive hash matches the release digest. Windows execution was not repeated locally.                    |
| [npm registry](https://registry.npmjs.org/@agentpm%2fsdk/latest)       | `latest` is 0.1.18. Downloaded tarball matches published SHA-512 integrity. Both ESM and CJS exports expose `supportedStandards`.              |
| [PyPI](https://pypi.org/project/agentpm/0.1.18/)                       | 0.1.18 wheel is available and matches published SHA-256. Fresh isolated installation imports version 0.1.18 and exposes `supported_standards`. |
| [Public API health](https://api.agentpackagemanager.com/health)        | Responds successfully with `Healthy`.                                                                                                          |
| [Deployed status page](https://www.agentpackagemanager.com/status)     | Shows CLI 0.1.47 and both SDKs 0.1.18.                                                                                                         |

CLI publication was `2026-11-22T01:43:26Z`. PyPI wheel upload was
`01:59:42.393238Z`; npm 0.1.18 publication was `02:00:34.250Z`. This verifies
CLI-before-SDK ordering. npm's website displaying an older version does not
contradict the registry's actual `latest` metadata or downloadable artifact.

The installed Homebrew executable is
`/opt/homebrew/Cellar/agentpm/0.1.47/bin/agentpm`, SHA-256
`bddfed7c87e26da67cc0d24f9ec39a43f6106023cd93147b430f503bb8fdd469`.

## Registry and frozen installation

Three public resolve requests verify:

- `@zack/summarize-text@0.1.10`: `agentpm / 1.0.0`, `legacy_inferred`.
- `@zack/summarize-text@0.1.11`: `agentpm / 1.0.0`, `declared`.
- `@zack/devwork-copilot@0.1.10`: the expected eight-package graph, preserving
  declared versus historical origin separately on each exact version.

The released CLI installed the Agent into a fresh consumer with
`--require-signature --require-attestation --machine`. Its v4 lock matches the
resolve response's exact kinds, names, versions, integrities and APDS metadata.
The graph includes the Agent and all six Component kinds, including the
Skill-inherited Tool.

`install @zack/devwork-copilot@0.1.10 --frozen` succeeded in the existing consumer
and in a second consumer starting with only a copy of that lock. Lock bytes were
unchanged; all **40 portable package files** matched. Native Python dependencies
were provisioned separately for each consumer; that is machine-local runtime
state, outside portable payload identity.

Intentional faults in test-owned local files were rejected:

| Fault                               | Actual rejection                                                                                       | Preservation                                                                                                                                          |
| ----------------------------------- | ------------------------------------------------------------------------------------------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------- |
| Package key/`kind` disagreement     | CLI: `Locked package key differs from its kind/name/version`                                           | Faulted lock and valid installed payloads unchanged; original lock restored afterward.                                                                |
| Wrong pinned integrity              | Registry: HTTP 409 `install_plan_changed`, `Package integrity differs from the requested install plan` | Faulted lock and valid installed payloads unchanged; original lock restored afterward.                                                                |
| Authored `^99` versus locked 0.1.10 | CLI: no locked Tool satisfies `^99`                                                                    | Authored manifest and stale local lock unchanged. Uses a deliberately stale synthetic local-root fixture and non-listening loopback Registry address. |

Verifier corrections are retained rather than hidden: `install-verification.log`
initially compared entire venvs across different paths. Its three install
commands succeeded, but that comparison failed on 15 path-dependent activation,
entrypoint and RECORD files. Portable payload files all matched. A continuation
initially expected the CLI inventory diagnostic for wrong integrity; production
correctly rejected it earlier with the specific Registry 409 above. The final
`install-completion-final.log` and `production-install-summary.json` pass with
the correct scope and exact rejection. No product fix was required.

## Published SDK and Harness behavior

The npm tarball was extracted into an isolated directory. The PyPI wheel and its
hash-verified `semver` dependency were installed without an index into a new
venv. Python import location was checked to exclude the source checkout.
Both SDKs communicate with the released Homebrew CLI, not the debug build.

The reviewed shared SDK fixture factory generated synthetic installed state;
actual CLI scaffolding, Knowledge/Memory builds and lint were used. This part
proves runtime selection and interfaces, not publication or signatures for those
synthetic packages. The separate real Registry installation above supplies
release/integrity evidence.

- **Four SDK-hosted Runs**: each published SDK runs both Agents. Model requests
  expose the expected exact Tool, Skill, Knowledge, Memory operation, Profile and
  Loop versions. Actual Tool markers are 1.0.0, 2.0.0, 1.0.0, 2.0.0. Agent 2
  inherits its Tool solely through its Skill.
- **Two released headless Runs**, plus per-Agent JSON preflight, select the same
  graph. Each Run ends successfully with implicit `complete` transitions. Review
  prohibits Tools; no Skill script or external Memory operation runs automatically.
- **Five CLI/SDK report comparisons**: ready, no Loop, missing model, legacy and
  unsupported standard. Both SDKs preserve initialization/preflight envelopes,
  protocol version 1 and the supported-standard list. Blocked states reject Run
  start with `preflight_not_ready`; no-Loop/model failures do not imply APDS
  invalidity, and legacy inference does not become certified conformance.
- **TUI and additional scope/ambiguity/Memory regressions**: retained from M6B's
  accepted 30-result manual audit, both actual TUI Runs and the operator-confirmed
  release-boundary hosted regression passes. These were not repeated as live TUI
  observations during this artifact verification. D13 retains current prompts.

## Retained evidence and completion

Key files: `github-releases.json`, `distribution-summary.json`,
`artifact-summary.json`, `resolve-{historical,declared,agent}.json`,
`published-sdk-summary.json`, `production-install-summary.json`, `summary.json`,
raw SDK/CLI streams, model requests, ended Run reports and consumer locks.
The ignored scripts preserve the exact probes used. The full development unit
suites were not rerun in this turn; the operator confirmed the complete hosted
CI boundary, and the additional probes above exercise actual released artifacts.

All six Release Band 2 checklist items are satisfied by these checks plus the
operator's CI/ref-cleanup/deployment confirmation and accepted M5A–M6B evidence.
**Release Band 2 is complete.** Further migrations must upgrade all participating
CLI installations before distributing extended locks. No new APDS version or
full execution-certification claim is made.
