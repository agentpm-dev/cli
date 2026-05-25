# Test Plan

## Required verification
- Verify tool manifests still initialize, lint, publish, install, load, and run successfully.
- Verify agent manifests initialize, lint, publish, install, and load successfully.
- Verify agents are package artifacts, not runnable app bundles.
- Verify agent publish does not require tool-only fields.
- Verify agent install resolves and installs tool dependencies.
- Verify normalized install layout separates `.agentpm/agents` and `.agentpm/tools`.
- Verify lockfile v2 supports multiple versions of the same tool.
- Verify v1 lockfile compatibility behavior for existing tool-only installs.
- Verify backend package migration preserves existing tool data.
- Verify registry search and UI distinguish tools from agents.
- Verify SDKs can load installed agents as metadata and do not execute agents.
- Verify manifest-driven install still works:
  - create a local `agent.json` with `kind: "agent"` and `tools`
  - run `agentpm install`
  - confirm tools install under `.agentpm/tools`
  - confirm the local manifest is not copied under `.agentpm/agents`
- Verify direct tool install still works:
  - run `agentpm install @namespace/tool-name@version`
  - confirm the tool installs under `.agentpm/tools`
- Verify direct agent install works:
  - run `agentpm install @namespace/agent-name@version`
  - confirm the agent installs under `.agentpm/agents`
  - confirm the agent's tools install under `.agentpm/tools`
- Verify lockfile v2 represents local manifest roots and registry-installed agent roots differently.

## Automated checks
Run the relevant commands for each repo after implementation. Adjust command names to match each repo’s actual scripts.

### CLI / Rust
- `cargo fmt --check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test --all`
- `agentpm init --kind tool --name my-tool --description "Test tool" --out-dir /tmp/agentpm-tool-init`
- `agentpm init --kind agent --name my-agent --description "Test agent" --out-dir /tmp/agentpm-agent-init`
- `agentpm lint /tmp/agentpm-tool-init/agent.json`
- `agentpm lint /tmp/agentpm-agent-init/agent.json`
- `agentpm publish --manifest /tmp/agentpm-agent-init/agent.json --dry-run`

### Backend / Python API
- `python -m pytest`
- Run migration upgrade command used by the repo.
- Run migration downgrade command if the repo supports downgrade verification.
- Run tests for package publish init/finalize.
- Run tests for package install resolve/init/finalize.
- Run tests for package search across tools, agents, namespaces, and all.

### Node SDK
- `npm test` or the repo’s equivalent package test command.
- Add and run a test that loads an installed agent and exposes resolved tool refs.

### Python SDK
- `python -m pytest` or the repo’s equivalent package test command.
- Add and run a test that loads an installed agent and exposes resolved tool refs.

### Registry UI
- `npm test` or the repo’s equivalent test command.
- `npm run lint` or the repo’s equivalent lint command.
- `npm run build` or the repo’s equivalent production build command.

## Manual checks
- Create a new tool package with `agentpm init --kind tool` and verify the generated manifest matches tool expectations.
- Create a new agent package with `agentpm init --kind agent` and verify the generated manifest matches agent expectations.
- Lint a valid tool manifest and a valid agent manifest.
- Lint an invalid agent manifest that includes `entrypoint`, `runtime`, `inputs`, `outputs`, or `files` if those fields are intended to be rejected.
- Publish a test tool package to a local/staging registry.
- Publish a test agent package to a local/staging registry.
- Confirm publish requires or accepts `packages:publish` according to the final compatibility decision.
- Install a test tool package directly.
- Install a test agent package directly.
- Confirm the installed agent appears under `.agentpm/agents/<namespace>/<name>/<version>/`.
- Confirm the installed tool dependencies appear under `.agentpm/tools/<namespace>/<name>/<version>/`.
- Confirm no tool packages are physically duplicated under the installed agent directory.
- Install two agents that depend on different versions of the same tool.
- Confirm both tool versions exist under `.agentpm/tools/...`.
- Confirm lockfile v2 records both tool versions and the correct agent-to-tool relationships.
- Confirm `--frozen` succeeds for a valid v2 agent lockfile.
- Confirm `--frozen` with an unsupported v1 agent graph fails with a clear migration message.
- Confirm existing tool-only v1 lockfiles are handled gracefully where practical.
- Load an installed agent from the Node SDK and verify manifest, path, resolved tools, and reserved references.
- Load an installed agent from the Python SDK and verify manifest, path, resolved tools, and reserved references.
- Search the registry for tools only.
- Search the registry for agents only.
- Search the registry with `all` and verify tools, agents, and namespaces can appear.
- Open an agent detail page and verify README, examples, dependencies, and reserved references display correctly.
- Open an existing tool detail page and verify no obvious regression.
- In a clean temp project, create a local `kind: "agent"` manifest with two tools and run `agentpm install`.
- Confirm `.agentpm/tools` contains the resolved tool versions.
- Confirm `.agentpm/agents` is absent or does not contain the local agent.
- Install a published agent package with `agentpm install @namespace/agent-name@version`.
- Confirm `.agentpm/agents` contains the installed agent package.
- Confirm `.agentpm/tools` contains the agent's resolved tool dependencies.
- Install two agents that depend on different versions of the same tool and confirm both tool versions are present.

## Expected evidence
Report back with:

- Passing CLI/Rust command output.
- Passing backend test output.
- Passing SDK test output.
- Passing registry UI lint/build/test output.
- Migration command output or logs.
- Example generated agent manifest.
- Example lockfile v2 showing:
  - an agent package
  - at least one tool package
  - an agent-to-tool relationship
  - two versions of the same tool where applicable
- Directory tree showing normalized `.agentpm/agents` and `.agentpm/tools` layout.
- Screenshots or output snippets for registry search and agent detail page.
- Notes for anything that could not be verified.
- Output from `agentpm install` for both manifest-driven and direct agent install workflows.
- Directory tree snippets showing `.agentpm/agents` and `.agentpm/tools`.
- `agentpm.lock` snippets showing:
  - `local:agent` relationship for local manifest install
  - `agent:@namespace/name@version` relationship for registry-installed agent install
  - multiple versions of the same tool when applicable

## Out of scope
- Running or invoking agents.
- Testing model-provider integrations.
- Testing orchestration behavior.
- Testing first-class skills, knowledge artifacts, memory blueprints, or instruction/persona profiles.
- Testing starter app/template generation.
- Testing private namespace billing or paid-plan behavior unless already covered by existing package publish/install tests.
