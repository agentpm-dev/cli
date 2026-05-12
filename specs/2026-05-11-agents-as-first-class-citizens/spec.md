# Feature
Agent Publishing as a First-Class Citizen

## Problem / Goal
AgentPM currently treats tools as the primary publishable and installable artifact. Phase 3 expands AgentPM so agents become first-class versioned package artifacts alongside tools.

The goal is to let users publish, discover, install, lock, and load full agent definitions as portable composition artifacts. An agent artifact describes what an agent is made of and what it depends on. It is not a full runnable application bundle.

An agent package should primarily contain:

- `agent.json` with `kind: "agent"`
- tool dependencies, which are the only resolvable dependency type in this phase
- reserved future references for skills, knowledge, memory, and profiles
- configuration/default metadata needed by future SDKs/adapters
- inline example prompts
- README and license metadata/content where provided

Installing an agent should resolve the agent package, install the agent artifact, resolve and install its tool dependencies, and write a deterministic lockfile that captures package identity, package kind, versions, integrity, and dependency relationships.

## Non-goals
- Do not build a hosted or local agent runtime.
- Do not implement `agentpm run` for agents.
- Do not add model-provider integration.
- Do not add orchestration, planning, memory execution, or prompt assembly behavior.
- Do not implement first-class skills, knowledge artifacts, memory blueprints, or instruction/persona profiles yet.
- Do not add `instructions` to the agent manifest in this phase.
- Do not add `compatibility` to the agent manifest in this phase.
- Do not introduce starter app or template artifacts in this phase.
- Do not physically vendor tool dependencies inside each installed agent directory.
- Do not allow a package name to be reused by different kinds within the same namespace.

## Constraints / Invariants
- Agent artifacts are composition artifacts, not runnable app bundles.
- Tools remain executable artifacts and continue to use `entrypoint`, `runtime`, `inputs`, `outputs`, and `files`.
- Agents must not require `entrypoint`, `runtime`, `inputs`, `outputs`, or `files`.
- Tools are the only dependency type that Phase 3 resolves and installs.
- Reserved agent reference fields are allowed, validated, and preserved, but not resolved or installed.
- Reserved fields are:
  - `skills`
  - `knowledge`
  - `memory`
  - `profiles`
- Reserved reference fields should support the same reference shape as `tools`: string references and object references.
- Agent examples are inline prompt examples in `agent.json`, not separate files.
- Registry/package naming must be package-oriented. The implementation should migrate from tool-specific naming to package naming in small, reviewable steps.
- Package names are unique within a namespace regardless of kind. `@namespace/foo` cannot be both a tool and an agent.
- The publish scope should move to `packages:publish`.
- Existing tool publish/install flows should continue to work.
- Existing tool examples should be verified and updated where necessary.
- The installed package layout should be normalized by kind:

```text
.agentpm/
  agents/
    <namespace>/
      <name>/
        <version>/
          agent.json
          README.md

  tools/
    <namespace>/
      <name>/
        <version>/
          agent.json
          ...
```

- Tool packages must not be duplicated under each installed agent. Instead, tools should live under `.agentpm/tools/...`, agents should live under `.agentpm/agents/...`, and dependency relationships should be represented by the lockfile and SDK metadata.
- Multiple versions of the same tool must be installable at the same time when required by different agents.
- The lockfile should be upgraded to v2 and support package kind, package identity, multiple versions, and agent-to-tool relationships.
- The CLI should read v1 lockfiles for existing tool-only installs where practical, but normal installs should write v2.
- `--frozen` with a v1 lockfile should continue to work for tool-only installs where practical. If a v1 lockfile cannot represent the requested agent dependency graph, fail with a clear message telling the user to run `agentpm install` without `--frozen` to regenerate v2.
- Registry search and UI should support agents alongside tools and namespaces.
- Agents should be visually differentiated from tools in the registry.

## Acceptance criteria
- `agentpm init --kind agent` creates a valid agent manifest using the Phase 3 schema.
- `agentpm lint` validates both tool and agent manifests correctly.
- Agent manifests allow `tools`, `skills`, `knowledge`, `memory`, `profiles`, `examples`, `readme`, `license`, `environment`, `name`, `version`, and `description` where appropriate.
- Agent manifests reject tool-only fields such as `entrypoint`, `runtime`, `inputs`, `outputs`, and `files` unless the schema intentionally allows a shared field.
- Tool manifests continue to validate with no unintended contract breakage.
- Publish accepts both `kind: "tool"` and `kind: "agent"` manifests.
- Agent publish packages only the files relevant to an agent artifact: `agent.json`, README content, license content, and metadata. Inline examples remain in the manifest.
- Agent publish does not require executable entrypoint files or declared package files.
- Publish authorization supports `packages:publish`.
- Backend package identity includes a package kind and is exposed through API responses.
- The backend uses package-oriented naming for new or migrated concepts where practical.
- Package names are unique within a namespace regardless of kind.
- Direct install of an agent spec resolves the agent package and its tool dependencies.
- Installing an agent lays the agent artifact under `.agentpm/agents/...` and its tools under `.agentpm/tools/...`.
- Installing two agents that require different versions of the same tool installs both tool versions without overwriting either version.
- Lockfile v2 records package kind, package name, package version, integrity, and dependency relationships.
- Lockfile v2 can represent multiple versions of the same tool package.
- Existing v1 tool-only lockfiles are handled gracefully according to the constraints above.
- Node SDK exposes minimal installed-agent loading support.
- Python SDK exposes minimal installed-agent loading support.
- SDKs expose agent manifest data, installed path, resolved tool references, and reserved future references as metadata only.
- SDKs do not execute or orchestrate agents.
- Registry search supports `agents` as a first-class type.
- Registry `all` search includes agents, tools, and namespaces.
- Registry search results include package kind.
- Agent detail pages show description, README, examples, tool dependencies, and reserved references when present.
- Tool detail pages continue to work after the package migration.
- Existing tool publish/install/run examples continue to work or are updated for the new package/lockfile behavior.

## Risks / edge cases
- The current backend model is tool-specific, so migration from tools to packages can create large diffs if not split carefully.
- Database table renames can break ORM models, migrations, search indexes, triggers, route code, and UI assumptions.
- Existing package URLs and API fields may still use `tool_id` or `/tools/...`; these need deliberate compatibility decisions.
- Search currently distinguishes tools and namespaces only; adding agents affects SQL, pagination cursors, serializers, UI filters, and totals.
- Lockfile v2 can break existing examples or CI flows if v1 compatibility is not handled carefully.
- A flat lockfile keyed only by package name cannot represent multiple versions of the same tool. The v2 shape must key packages by kind, name, and version or use an equivalent structure.
- Agent install resolution can accidentally download tool dependencies into the wrong directory if artifact kind is not carried through resolve/init/finalize responses.
- Agent publish can accidentally reuse tool packaging logic and require `entrypoint`, `runtime`, or `files`.
- Signature and attestation statements may remain tool-specific unless explicitly generalized.
- Existing PATs with `tools:publish` may fail if the scope migration is abrupt.
- Package name uniqueness across kind needs a database constraint or equivalent service-level enforcement.
- Reserved future reference fields may imply functionality that does not exist yet; docs and warnings should be clear.

## Open questions
- Should `tools:publish` remain temporarily valid for publishing tool packages, or should the migration require new PATs with `packages:publish` immediately?
- Should existing public URLs under `/tools/...` remain as redirects after package-oriented routes are added? - No. tools and agents should still resolve to different paths.
- Should the database migration physically rename tables in one sequence of migrations, or introduce compatibility views/aliases during the transition?
- Should namespace counters become one `num_packages` counter, separate `num_tools` and `num_agents` counters, or both? - For stats we want to know the number of packages. However we want to know how many there are of each and also have separate trending tools vs agents.
- Should agent packages go through the same scan pipeline as tools, or only a lightweight artifact validation plus malware scan? - same pipeline to where it makes sense

## References
- Existing AgentPM manifest schema / `agent.json` contract.
- Existing install and lockfile behavior.
- Existing publish/install provenance and integrity verification.
