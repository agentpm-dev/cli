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

### Agent manifest shape

A minimal Phase 3 agent manifest should look like:

```json
{
  "kind": "agent",
  "name": "support-agent",
  "version": "0.1.0",
  "description": "Triage support requests using installed tools.",
  "tools": [
    "@zack/slack-post-message@0.1.0"
  ],
  "skills": [],
  "knowledge": [],
  "memory": [],
  "profiles": [],
  "examples": [
    {
      "title": "Triage an incident",
      "prompt": "Summarize this incident and draft a follow-up issue."
    }
  ],
  "readme": "README.md"
}
```

Installing an agent should resolve the agent package, install the agent artifact, resolve and install its tool dependencies, and write a deterministic lockfile that captures package identity, package kind, versions, integrity, and dependency relationships.
Direct package install should become kind-aware: direct tool specs continue to install a tool, while direct agent specs install the agent plus its resolved tool dependencies.

Phase 3 must preserve the existing manifest-driven install workflow while adding direct agent package installation. Users should be able to install dependencies from a local `kind: "agent"` manifest by running `agentpm install`, and they should also be able to install a published agent package by running `agentpm install @namespace/agent-name@version`.

## Key decisions
- Agents are first-class package artifacts with `kind: "agent"`.
- Agents are composition artifacts, not runnable app bundles.
- Tools are the only dependency type resolved in Phase 3.
- Reserved fields for skills, knowledge, memory, and profiles are validated and preserved but not resolved.
- Direct install supports both tools and agents.
- Manifest-driven install from a local `kind: "agent"` manifest remains supported.
- Local agent manifests are not copied into `.agentpm/agents`.
- Registry-installed agent packages are installed into `.agentpm/agents`.
- Tool dependencies are installed into the shared `.agentpm/tools` layout.
- Lockfile v2 records package kind, identity, integrity, multiple versions, and dependency relationships.
- Backend/domain naming should migrate from tools to packages in small steps.

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
- Registry/package naming must migrate from tool-specific naming to package-oriented naming in small, reviewable steps. Prefer package-oriented database tables, ORM models, services, API DTOs, and search indexes where practical, while keeping compatibility shims only where needed to reduce migration risk.
- Package names are unique within a namespace regardless of kind. `@namespace/foo` cannot be both a tool and an agent.
- Tool and agent registry pages should resolve to different public paths. Do not rely on `/tools/...` as the canonical path for agents.
- Namespace/package stats should support both aggregate package counts and separate tool vs agent counts where practical.
- Agent packages should go through the same scan pipeline as tools where the scan behavior applies. Tool-specific scan outputs may remain empty/null for agent packages.
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
- Public registry routing should remain kind-specific where appropriate: tools should continue to have tool pages, and agents should have agent pages. Agents should not be presented under tool routes.
- AgentPM must support two install workflows:
  - Manifest-driven install: `agentpm install` reads the local `agent.json` when it is `kind: "agent"` and installs the tools listed in that manifest.
  - Direct package install: `agentpm install <spec>` resolves the requested package. If the package is a tool, it installs that tool. If the package is an agent, it installs the agent artifact and then resolves/installs that agent's tool dependencies.
- A local `kind: "agent"` manifest is the source of truth for manifest-driven installs and should not be copied into `.agentpm/agents`.
- Only registry-installed agent packages should be written under `.agentpm/agents`.
- Tool dependencies from both local agents and registry-installed agents should install into the normalized shared tools layout under `.agentpm/tools`.
- Agent installs must continue to support multiple versions of the same tool when different agents resolve different tool versions.


### Install workflows

AgentPM supports two agent install workflows.

#### Manifest-driven install

When a project has a local `agent.json` with `kind: "agent"`, running:

```bash
agentpm install 
```
reads the local manifest, resolves the `tools` entries, installs those tools into `.agentpm/tools`, preserves reserved future references in the lockfile, and writes `agentpm.lock`.

The local project manifest is not copied into `.agentpm/agents`; it remains the source of truth.

```text
project/
  agent.json
  agentpm.lock

  .agentpm/
    tools/
      zack/
        slack-post-message/
          0.1.0/
      zack/
        github-issues/
          0.2.3/
```

#### Direct agent package install

When a user runs:

```bash
agentpm install @zack/support-agent@0.1.0
```

AgentPM resolves the package. If the package is `kind`: "agent", AgentPM downloads the agent artifact into .agentpm/agents, reads the installed agent manifest, resolves the agent's tools, installs those tools into .agentpm/tools, and writes agentpm.lock.

```text
.agentpm/
  agents/
    zack/
      support-agent/
        0.1.0/
          agent.json
          README.md

  tools/
    zack/
      slack-post-message/
        0.1.0/
      github-issues/
        0.2.3/
```

### Lockfile v2

Phase 3 should introduce `agentpm.lock` version 2.

Lockfile v2 must represent packages by full package identity, including kind, name, and version. This is required so multiple versions of the same package can coexist when different agents resolve different dependency versions.

Package keys should include kind, name, and version, for example:

```json
{
  "lockfile_version": 2,
  "packages": {
    "agent:@zack/support-agent@0.1.0": {
      "kind": "agent",
      "name": "@zack/support-agent",
      "version": "0.1.0",
      "integrity": "..."
    },
    "tool:@zack/slack-post-message@0.1.0": {
      "kind": "tool",
      "name": "@zack/slack-post-message",
      "version": "0.1.0",
      "integrity": "..."
    },
    "tool:@zack/slack-post-message@0.2.0": {
      "kind": "tool",
      "name": "@zack/slack-post-message",
      "version": "0.2.0",
      "integrity": "..."
    }
  }
}
```

Lockfile v2 must also preserve dependency relationships.

For a manifest-driven install, the lockfile should use a local root relationship rather than treating the local project manifest as an installed registry package

```json
{
  "lockfile_version": 2,
  "packages": {
    "tool:@zack/slack-post-message@0.1.0": {
      "kind": "tool",
      "name": "@zack/slack-post-message",
      "version": "0.1.0",
      "integrity": "..."
    }
  },
  "roots": {
    "local:agent": {
      "name": "my-local-agent",
      "version": "0.1.0",
      "tools": [
        "tool:@zack/slack-post-message@0.1.0"
      ],
      "reserved": {
        "skills": [],
        "knowledge": [],
        "memory": [],
        "profiles": []
      }
    }
  }
}
```

For a direct registry-installed agent, the relationship should use the installed agent package identity:

```json
{
  "lockfile_version": 2,
  "packages": {
    "agent:@zack/support-agent@0.1.0": {
      "kind": "agent",
      "name": "@zack/support-agent",
      "version": "0.1.0",
      "integrity": "..."
    },
    "tool:@zack/slack-post-message@0.1.0": {
      "kind": "tool",
      "name": "@zack/slack-post-message",
      "version": "0.1.0",
      "integrity": "..."
    }
  },
  "roots": {
    "agent:@zack/support-agent@0.1.0": {
      "tools": [
        "tool:@zack/slack-post-message@0.1.0"
      ],
      "reserved": {
        "skills": [],
        "knowledge": [],
        "memory": [],
        "profiles": []
      }
    }
  }
}
```

AgentPM should read v1 lockfiles for existing tool-only workflows where practical, but normal non-frozen installs should write lockfile v2. If --frozen encounters a v1 lockfile that cannot represent the requested agent dependency graph, it should fail with a clear message telling the user to run agentpm install without --frozen to regenerate the lockfile.

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
- Running `agentpm install` with a local `kind: "agent"` manifest resolves and installs the manifest's `tools` entries without installing the local manifest itself into `.agentpm/agents`.
- Running `agentpm install <tool-spec>` continues to install a single tool package directly.
- Running `agentpm install <agent-spec>` installs the agent artifact into `.agentpm/agents`, reads that agent's manifest, resolves its tool dependencies, and installs those tools into `.agentpm/tools`.
- Lockfile v2 can represent both:
  - dependencies resolved from a local manifest-driven install
  - dependencies resolved from a registry-installed agent package
- Lockfile v2 clearly distinguishes a local agent root from registry-installed agent packages.
- Lint/publish/install should emit a warning when reserved future reference fields are present, making clear that these references are preserved but not resolved in Phase 3.

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
- Direct agent package installs and local manifest-driven installs have different roots. The implementation must avoid incorrectly copying local project manifests into `.agentpm/agents`.
- Lockfile v2 must represent local agent relationships without pretending the local project manifest is a registry package.
- `agentpm install <spec>` must detect whether the resolved package is a tool or an agent and branch appropriately.
- Agent install resolution may require a two-phase flow: resolve/download the agent package first, then read its manifest and resolve/download its tool dependencies.

## Open questions
- Should `tools:publish` remain temporarily valid for publishing tool packages, or should the migration require new PATs with `packages:publish` immediately?
- Should the database migration physically rename tables in one sequence of migrations, or introduce compatibility views/aliases during the transition?

## References
- Existing AgentPM manifest schema / `agent.json` contract.
- Existing install and lockfile behavior.
- Existing publish/install provenance and integrity verification.
