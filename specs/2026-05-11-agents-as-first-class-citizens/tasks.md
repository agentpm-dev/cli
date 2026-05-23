# Tasks

## Milestone 1: Define the agent manifest contract
- [ ] Update the manifest schema so `kind: "agent"` is a first-class artifact kind.
- [ ] Ensure agent manifests require `kind`, `name`, `version`, `description`, and `tools`.
- [ ] Ensure agent manifests do not require tool-only fields: `entrypoint`, `runtime`, `inputs`, `outputs`, or `files`.
- [ ] Add `examples` as inline prompt examples in `agent.json`.
- [ ] Add reserved future reference fields: `skills`, `knowledge`, `memory`, and `profiles`.
- [ ] Make reserved future reference fields support the same string/object dependency-reference shape as `tools`.
- [ ] Keep `compatibility` out of the schema for this phase.
- [ ] Keep `instructions` out of the schema for this phase.
- [ ] Add schema tests for valid tool manifests, valid agent manifests, invalid tool manifests, and invalid agent manifests.
- [ ] Add schema tests proving reserved future fields validate and are preserved but do not imply install behavior.

## Milestone 2: Update init and lint for agent manifests
- [ ] Update the `agentpm init --kind agent` template to match the Phase 3 agent schema.
- [ ] Include example prompt metadata in the generated agent template.
- [ ] Include empty reserved future reference arrays in the generated agent template or document why they are omitted by default.
- [ ] Ensure `agentpm init --kind tool` remains unchanged except for any schema URL/version updates.
- [ ] Update lint output to clearly validate both tool and agent manifests.
- [ ] Add lint warnings for reserved future fields explaining they are validated and preserved but not resolved in Phase 3.
- [ ] Confirm `--strict`, `--format pretty`, `--format json`, `--format ndjson`, and `--fix` still behave correctly.

## Milestone 3: Introduce package terminology in shared domain types
- [ ] Identify CLI, SDK, backend, and UI types that represent generic package identity but are currently named `Tool` or `ToolVersion`.
- [ ] Introduce package-oriented shared types with `kind`, `name`, `version`, and `integrity`.
- [ ] Keep tool-specific types only where behavior is truly executable-tool-specific.
- [ ] Update resolve/install DTOs so artifact kind is carried through every step.
- [ ] Update publish DTOs so artifact kind is carried through every step.
- [ ] Keep existing tool behavior working while new package-oriented types are introduced.
- [ ] Add tests around package identity parsing for `@namespace/name`, ranges, exact versions, tools, and agents.

## Milestone 4: Migrate database identity from tools to packages
- [ ] Create a database migration plan that moves the canonical package identity from `tools` to `packages`.
- [ ] Rename or replace `tools` with `packages` where practical.
- [ ] Add `kind` to package identity with allowed values `tool` and `agent`.
- [ ] Enforce unique package names within a namespace regardless of kind.
- [ ] Rename or replace `tool_versions` with `package_versions` where practical.
- [ ] Preserve version metadata fields needed by both tools and agents: manifest, sha, size, S3 key, description, readme, license, publish/yank metadata.
- [ ] Keep runtime nullable and tool-specific.
- [ ] Update ORM/domain models to use package-oriented naming.
- [ ] Update any triggers that currently reference tool creation or tool version publication.
- [ ] Update namespace counters from tool-specific counters to package-aware counters.
- [ ] Add migration tests or manual verification steps confirming existing tool rows migrate successfully.

## Milestone 5: Migrate uploads, signatures, attestations, scans, and install sessions
- [ ] Rename or adapt upload references from `tool_id` to `package_id`.
- [ ] Update pending upload uniqueness to apply to package/version.
- [ ] Rename or adapt tool signatures to package signatures.
- [ ] Update signature statement type to be package-kind-aware.
- [ ] Rename or adapt registry attestations from tool-version references to package-version references.
- [ ] Update registry attestation statement payloads to include package kind.
- [ ] Rename or adapt scan tables from tool-version scans to package-version scans.
- [ ] Decide whether agent packages use the full scan pipeline or a lightweight subset, then implement the Phase 3 behavior.
- [ ] Update install session plan JSON to store package-oriented items.
- [ ] Confirm existing install metrics still work after package migration.

## Milestone 6: Generalize backend publish flow
- [ ] Update publish auth to require `packages:publish` for package publishing.
- [ ] Decide whether `tools:publish` remains temporarily accepted for tool packages, then implement that compatibility behavior.
- [ ] Update publish init to resolve/create a package record rather than a tool record.
- [ ] Update publish init to persist package kind from the manifest.
- [ ] Update publish init to generate package-oriented S3 keys or intentionally preserve existing S3 paths with documented compatibility behavior.
- [ ] Update publish finalize to create package version records.
- [ ] Update publish finalize receipts to include package kind and package id.
- [ ] Ensure agent publish accepts `kind: "agent"` and does not require runtime, entrypoint, inputs, outputs, or files.
- [ ] Ensure tool publish still validates and packages executable tool artifacts exactly as before.
- [ ] Update URLs in publish receipts so agent receipts do not hardcode tool detail routes.
- [ ] Add backend tests for publishing tools and publishing agents.

## Milestone 7: Split CLI publish packaging by artifact kind
- [ ] Refactor CLI publish parsing so it can parse either a tool manifest or an agent manifest.
- [ ] Keep existing tool packaging behavior for `kind: "tool"`.
- [ ] Add agent packaging behavior for `kind: "agent"`.
- [ ] Agent packages should include root `agent.json`.
- [ ] Agent packages should include README and license payloads using the existing metadata/content flow where possible.
- [ ] Agent packages should not require or package entrypoint files.
- [ ] Agent packages should not require `files` globs.
- [ ] Update artifact filename generation so it works for agents without runtime suffixes.
- [ ] Update signing statement generation so it is package-kind-aware.
- [ ] Update CLI publish output wording from tool-specific language to package-aware language.
- [ ] Add CLI tests for `agentpm publish --dry-run` on a tool and an agent.

## Milestone 8: Implement lockfile v2
- [ ] Design lockfile v2 with package entries keyed by kind, name, and version or an equivalent multi-version-safe identity.
- [ ] Include package kind, package name, package version, and integrity for every package entry.
- [ ] Include relationship data showing which tools were resolved for each installed agent.
- [ ] Include reserved future references from the agent manifest in relationship metadata or another preserved metadata section.
- [ ] Implement v2 lockfile read/write support.
- [ ] Preserve v1 lockfile read support for existing tool-only installs where practical.
- [ ] Ensure normal non-frozen installs write v2.
- [ ] Ensure `--frozen` works with v1 lockfiles for tool-only installs where practical.
- [ ] Ensure `--frozen` fails clearly when a v1 lockfile cannot represent an agent dependency graph.
- [ ] Add tests for two agents depending on different versions of the same tool.
- [ ] Add tests proving two versions of the same tool can both be represented in lockfile v2.
- [ ] Represent local manifest-driven installs with a local root relationship such as `local:agent`.
- [ ] Represent registry-installed agents with package identity keys such as `agent:@namespace/name@version`.
- [ ] Ensure lockfile v2 can distinguish local agent dependency relationships from registry-installed agent dependency relationships.
- [ ] Add tests for lockfile output from `agentpm install` with no spec and from `agentpm install <agent-spec>`.

## Milestone 9: Generalize backend install resolution
- [ ] Update install resolve to accept package items and return package kind.
- [ ] Resolve direct agent package specs as packages with `kind: "agent"`.
- [ ] When resolving an agent package, inspect its manifest and resolve its `tools` dependencies.
- [ ] Keep reserved future references preserved but unresolved.
- [ ] Return a complete resolved package graph to the CLI.
- [ ] Ensure semver range resolution still works for tool package dependencies.
- [ ] Ensure semver range resolution works for agent package specs.
- [ ] Ensure package name conflicts across kind are impossible or fail clearly.
- [ ] Add backend tests for installing a tool, installing an agent, and installing agents with different tool versions.

## Milestone 10: Implement normalized package install layout in the CLI
- [ ] Update CLI install to download tool artifacts into `.agentpm/tools/...`.
- [ ] Update CLI install to download agent artifacts into `.agentpm/agents/...`.
- [ ] Ensure download/extract logic chooses target directory based on package kind from install init responses.
- [ ] Update install progress text from tool-specific language to package-aware language.
- [ ] Ensure installing an agent downloads the agent artifact and all resolved tool dependencies.
- [ ] Ensure installing two agents with different versions of the same tool keeps both tool versions.
- [ ] Ensure install does not duplicate tool artifacts under each installed agent.
- [ ] Ensure `--refresh`, `--frozen`, `--update-range`, `--require_attestation`, `--quiet`, and `--token` still behave correctly.
- [ ] Add CLI integration tests for direct tool install, direct agent install, and manifest-driven agent dependency install.
- [ ] Preserve manifest-driven install behavior for local `kind: "agent"` manifests when running `agentpm install` without a spec.
- [ ] Add direct package install branching for `agentpm install <spec>` so resolved tool packages install directly and resolved agent packages install the agent artifact before resolving its tool dependencies.
- [ ] Ensure local manifest-driven installs do not copy the local `agent.json` into `.agentpm/agents`.
- [ ] Ensure registry-installed agent packages are written under `.agentpm/agents`.
- [ ] Ensure tool dependencies from both install workflows are installed into `.agentpm/tools`.

## Milestone 10a: Define workspace install-set and lockfile root accumulation behavior
- [ ] Merge repeated direct agent installs into the existing registry root set in `agent.lock` instead of replacing prior registry roots with only the latest direct install request.
- [ ] Keep direct tool installs in a local `kind: "agent"` project represented through the `local:agent` root by mutating `agent.json`, rather than adding separate registry roots for those tools.
- [ ] Treat manifest-driven installs with no spec as authoritative for the local project and replace the root set with the current `local:agent` intent.
- [ ] On manifest-driven installs, remove superseded registry roots from `agent.lock` but leave unreferenced installed packages on disk for now.
- [ ] Treat `agent.lock` as the source of truth for intended installs even when unreferenced packages remain on disk under `.agentpm/agents` or `.agentpm/tools`.
- [ ] Update lock root construction to preserve multiple registry agent roots instead of assuming a single agent item in the current install plan.
- [ ] Add a later pruning or cleanup behavior for removing unreferenced installed packages from disk once root accumulation semantics are stable.
- [ ] Add CLI tests for direct install followed by direct install, direct install followed by manifest-driven install, and manifest-driven install followed by direct install.
- [ ] Document the chosen behavior for lockfile root accumulation and workspace install semantics.

## Milestone 11: Add minimal Node SDK agent loading
- [ ] Add a Node SDK method for loading installed agents, such as `loadAgent`.
- [ ] Resolve installed agent paths from `.agentpm/agents/...`.
- [ ] Load and return the agent manifest.
- [ ] Read lockfile v2 and expose resolved tool references for the agent.
- [ ] Expose reserved future references as metadata only.
- [ ] Do not execute or orchestrate agents.
- [ ] Ensure existing Node SDK tool loading continues to work.
- [ ] Add Node SDK tests for loading an installed agent.

## Milestone 12: Add minimal Python SDK agent loading
- [ ] Add a Python SDK method for loading installed agents, such as `load_agent`.
- [ ] Resolve installed agent paths from `.agentpm/agents/...`.
- [ ] Load and return the agent manifest.
- [ ] Read lockfile v2 and expose resolved tool references for the agent.
- [ ] Expose reserved future references as metadata only.
- [ ] Do not execute or orchestrate agents.
- [ ] Ensure existing Python SDK tool loading continues to work.
- [ ] Add Python SDK tests for loading an installed agent.

## Milestone 13: Add package-aware search backend support
- [ ] Replace or adapt `tool_search_index` with a package-aware search index.
- [ ] Include package kind in search index rows.
- [ ] Add `agents` as a valid search type.
- [ ] Keep `tools` as a valid search type.
- [ ] Keep `namespaces` as a valid search type.
- [ ] Update `all` search so it includes tools, agents, and namespaces.
- [ ] Update search serializers to return `itemType: "tool"`, `itemType: "agent"`, or `itemType: "namespace"`.
- [ ] Update `totals_by_type` to include agents.
- [ ] Update cursor logic as needed so mixed package search pagination remains stable.
- [ ] Add backend search tests for tools, agents, namespaces, and all.

## Milestone 14: Add registry UI support for agents
- [ ] Add visual differentiation for tool and agent package cards.
- [ ] Add an agents filter/search tab or equivalent UI control.
- [ ] Add agent detail pages.
- [ ] Show agent description, README, license, version, namespace, and install command.
- [ ] Show inline examples from `agent.json`.
- [ ] Show tool dependencies from the agent manifest or resolved package metadata.
- [ ] Show reserved future references when present, marked as future/reserved.
- [ ] Keep existing tool detail pages working.
- [ ] Optionally add a tool-page “used by agents” count/list if the data is cheap after the package migration.
- [ ] Add UI tests or manual verification steps for tool search, agent search, and agent detail pages.

## Milestone 15: Update docs and examples
- [ ] Document the difference between tool artifacts, agent artifacts, and future templates/starter apps.
- [ ] Document the Phase 3 agent manifest fields.
- [ ] Document that tools are the only resolved dependency type in Phase 3.
- [ ] Document that `skills`, `knowledge`, `memory`, and `profiles` are reserved, validated, and preserved but not resolved yet.
- [ ] Document normalized install layout for `.agentpm/agents` and `.agentpm/tools`.
- [ ] Document lockfile v2 at a high level.
- [ ] Update CLI docs for `agentpm init --kind agent`, `agentpm publish`, and `agentpm install`.
- [ ] Update PAT/auth docs to use `packages:publish`.
- [ ] Add a sample agent package.
- [ ] Update `agentpm-examples` for the latest CLI and lockfile behavior.
- [ ] Verify existing tool examples still work.
- [ ] Add an example showing two agents depending on different versions of the same tool.
- [ ] Document the difference between manifest-driven install and direct package install.
- [ ] Add docs showing `agentpm install` from a local `kind: "agent"` manifest.
- [ ] Add docs showing `agentpm install @namespace/agent-name@version`.
- [ ] Update examples to cover both workflows.

## Milestone 16: Final regression and release readiness
- [ ] Run full CLI test suite.
- [ ] Run backend test suite.
- [ ] Run SDK test suites.
- [ ] Run registry UI checks.
- [ ] Add CLI coverage for `agentpm install` with a local `kind: "agent"` manifest containing tools.
- [ ] Add CLI coverage for `agentpm install <tool-spec>`.
- [ ] Add CLI coverage for `agentpm install <agent-spec>`.
- [ ] Add CLI coverage for an agent package whose tool dependency resolves to a different version than another installed agent's dependency.
- [ ] Publish a test tool package in a non-production environment.
- [ ] Publish a test agent package in a non-production environment.
- [ ] Install the test agent and confirm its tool dependencies are installed.
- [ ] Confirm lockfile v2 is written and contains the expected package graph.
- [ ] Confirm existing tool-only install flow still works.
- [ ] Confirm existing tool run flow still works.
- [ ] Confirm search and detail pages work for both tools and agents.
- [ ] Confirm release notes mention the lockfile change and package terminology migration.
