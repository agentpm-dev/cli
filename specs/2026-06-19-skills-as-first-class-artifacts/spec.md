# Feature
Phase 6A: Skills as First-Class Artifacts

## Problem / Goal
AgentPM currently supports Skills only as an exported interoperability scaffold generated from an installed tool via `agentpm export --skill`. That export path is useful, but Skills are not yet publishable, versioned, installable, searchable, or usable as dependency graph nodes.

The goal of Phase 6A is to make Skills a first-class AgentPM package kind:

- `kind: "skill"` in `agent.json`
- publishable and versioned through the existing package publish flow
- installable and lockable through the existing package install flow
- discoverable through registry search, trending, and package detail pages
- referenceable by agents and templates
- able to declare AgentPM tool dependencies through the existing top-level `tools` array
- able to package authored skill files such as `SKILL.md`, references, and scripts

Strategically, this expands AgentPM from packaging only what an agent can execute to also packaging how an agent performs work. A Skill is not just tool documentation. A Skill can be a procedural playbook, task-specific reasoning guide, instruction/reference bundle, multi-tool workflow, or a pure process artifact that uses no tools at all.

## Non-goals
- Do not replace or remove `agentpm export --skill`. Export remains a scaffold-generation path, not the canonical Skill lifecycle.
- Do not require every Skill to wrap or execute a tool.
- Do not require every Skill to call `agentpm run`, although AgentPM-authored tool invocation examples should prefer it.
- Do not introduce a separate `skill.json`; `agent.json` remains the canonical AgentPM manifest file for all package kinds.
- Do not introduce recursive Skill dependencies in this phase. Skills may depend on tools, but Skills may not depend on other Skills.
- Do not allow tools to depend on other tools, agents, or skills.
- Do not allow agents to depend on agents.
- Do not create a separate database table for Skills. The existing shared package table currently named `tools` remains the physical table for tools, agents, templates, and skills.
- Do not enforce model/runtime/export compatibility at runtime in this phase. Compatibility metadata is descriptive and discoverable only.
- Do not add an `--install` flag to `agentpm export --skill` in this phase.
- Do not crawl `SKILL.md` links to infer extra package files.
- Do not rename the physical `tools` database table or broadly refactor package terminology beyond what is necessary for Skill support.

## Constraints / Invariants

### Product model
- A Tool is executable capability.
- A Skill is reusable operational know-how.
- A Skill must be useful to a model or agent runtime even when it does not execute code.
- A first-class Skill is an AgentPM package with its own `agent.json`, version, artifact, registry page, install location, and lockfile entries.
- `agentpm export --skill` remains a tool-derived scaffold generator. Exported scaffold files can become a publishable Skill once an `agent.json` is generated or added.

### Manifest and schema
- `agent.json` remains the canonical manifest file for skills.
- Add `skill` to the top-level `kind` enum.
- Add a top-level `skill` metadata block for skill-specific metadata.
- Skill tool dependencies must use the existing top-level `tools` array, matching the existing agent convention.
- Skill `tools` are optional because Skills may be procedural-only.
- Skill `skills` is not allowed in Phase 6A.
- Agents may use top-level `skills` as first-class package references.
- Templates may include skill dependencies in `template.dependencies.skills`.
- Existing future placeholders `knowledge`, `memory`, and `profiles` remain reserved/non-first-class.

Recommended minimum Skill manifest:

```json
{
  "kind": "skill",
  "name": "incident-commander",
  "version": "0.1.0",
  "description": "Incident response coordination playbook.",
  "tools": [],
  "skill": {
    "entrypoint": "SKILL.md"
  }
}
```

Recommended tool-backed Skill manifest:

```json
{
  "kind": "skill",
  "name": "slack-incident-update",
  "version": "0.1.0",
  "description": "A playbook for posting structured incident updates to Slack.",
  "tools": [
    {
      "name": "@zack/slack-post-message",
      "version": "0.1.1"
    }
  ],
  "skill": {
    "entrypoint": "SKILL.md",
    "references": [
      "references/tool-contract.md",
      "references/examples.md"
    ],
    "scripts": [
      "scripts/run.sh"
    ],
    "compatibility": {
      "runtimes": ["agentpm-run", "shell"],
      "export_targets": ["markdown"]
    }
  }
}
```

Suggested `skill` metadata block:

- `entrypoint` — required string path, usually `SKILL.md`
- `references` — optional string array of supporting authored reference files
- `scripts` — optional string array of helper scripts
- `compatibility` — optional object containing metadata-only arrays such as:
  - `model_families`
  - `runtimes`
  - `environments`
  - `export_targets`

All declared file paths must be safe relative paths inside the package directory. They must not be absolute, contain `..`, use Windows drive roots, use UNC paths, or escape the manifest root.

### Packaging
- Skills do not require a `files` array.
- For `kind: "skill"`, `agent.json` is the packaging contract.
- The Skill artifact should include only:
  - `agent.json`
  - the file at `skill.entrypoint`
  - any files listed in `skill.references[]`
  - any files listed in `skill.scripts[]`
  - `readme` file if the existing top-level `readme` field points to a local file and existing publish behavior supports that path
  - `license.file` if present
- Preserve declared relative paths in the tarball so links like `[Tool contract](references/tool-contract.md)` continue to work after install.
- Do not crawl Markdown links or include unlisted files.
- Reuse existing tar safety checks, file count limits, artifact byte limits, blocked embedded archive checks, and deterministic tar metadata patterns.

Current publish code has separate package functions for tools, agents, and templates. It validates the manifest, parses it into `PublishManifest`, then dispatches to `package_tool`, `package_agent`, or `package_template`. Phase 6A should add a Skill branch and a `package_skill` implementation rather than overloading tool packaging.

### Install and dependency graph
- Agents can depend on tools and skills.
- Skills can depend on tools.
- Templates can depend on tools, agents, and skills.
- Skills must not depend on skills in Phase 6A.
- Direct `agentpm install <spec>` should support installing a Skill package.
- Manifest-driven install should continue to support local `kind: "agent"` manifests and should also resolve the agent's declared skills.
- Manifest-driven install must support local `kind: "skill"` manifests in Phase 6A. Running `agentpm install` in a Skill package directory must resolve the Skill's top-level `tools`, download/extract those tools, and write a `local:skill` lock root. This is required so Skill authors can test tool-backed Skills locally before publishing.
- `agentpm install` should continue to reject direct Template package installs with the existing message recommending `agentpm new`.
- Install extraction must create a skills install root, expected shape:

```text
.agentpm/skills/<namespace>/<name>/<version>/
```

Existing install currently ensures `.agentpm/cache`, `.agentpm/tools`, and `.agentpm/agents`; Phase 6A must add `.agentpm/skills` and update download/extract logic to route Skill artifacts there.

### Lockfile
- First-class Skills require `lockfile_version: 3`.
- Phase 6A must continue to read existing v1/v2 lockfiles where supported today, but any lockfile that contains resolved Skill packages or first-class `skills` root relationships must be written as v3.
- Non-frozen installs may upgrade v2 lockfiles to v3.
- `--frozen` with a v1/v2 lockfile must fail if the desired graph includes first-class Skill dependencies that cannot be represented by the existing lockfile, with an actionable message telling the user to run `agentpm install` without `--frozen`.
- Lockfile v3 must support `skill` packages in `packages` keys:

```json
{
  "packages": {
    "skill:@zack/slack-incident-update@0.1.0": {
      "kind": "skill",
      "name": "@zack/slack-incident-update",
      "version": "0.1.0",
      "integrity": "..."
    }
  }
}
```

- Skills must move out of `reserved` now that they are first-class.
- Existing lock roots currently include a `reserved.skills` placeholder. Phase 6A should change root shape so `skills` is a first-class field next to `tools`.
- Keep `knowledge`, `memory`, and `profiles` under `reserved`.
- Desired lock root direction for a local agent:

```json
{
  "roots": {
    "local:agent": {
      "name": "support-agent",
      "version": "0.1.0",
      "tools": [
        "tool:@zack/slack-post-message@0.1.1"
      ],
      "skills": [
        "skill:@zack/slack-incident-update@0.1.0"
      ],
      "reserved": {
        "knowledge": [],
        "memory": [],
        "profiles": []
      }
    }
  }
}
```

- Desired lock root direction for a Skill that depends on tools:

```json
{
  "roots": {
    "skill:@zack/slack-incident-update@0.1.0": {
      "tools": [
        "tool:@zack/slack-post-message@0.1.1"
      ],
      "skills": [],
      "reserved": {
        "knowledge": [],
        "memory": [],
        "profiles": []
      }
    }
  }
}
```

- Do not silently drop old `reserved.skills` entries when reading existing lockfiles. During non-frozen install, migrate `reserved.skills` entries into first-class `skills` relationships when they can be resolved as Skill packages.
- If old `reserved.skills` entries cannot be resolved, preserve them safely or fail with a clear migration error rather than silently removing user intent.
- 
### Export command
Current `agentpm export --skill <PACKAGE_REF>` generates a starter scaffold from an installed tool:

```text
skills/<tool-name>/
  SKILL.md
  references/
    tool-contract.md
    examples.md
  scripts/
    run.sh
```

Keep that behavior, but align it with first-class Skills:

- Add optional manifest generation, recommended flag: `--manifest`.
- `--manifest` should generate a starter `agent.json` with `kind: "skill"`, `tools` containing the source tool pinned to the resolved version, and a `skill` block referencing `SKILL.md`, `references/tool-contract.md`, `references/examples.md`, and `scripts/run.sh`.
- Export should prefer installed/locked package metadata when available.
- If the source tool is not installed, export may resolve the tool from the registry to fetch the metadata needed for scaffold generation.
- Remote export must not install the tool, mutate `agent.lock`, or change workspace manifests.
- Remote export should work for public packages and for private packages when the caller has credentials/access.
- No `--install` flag in this phase.

### Init command
- Add `agentpm init --kind skill`.
- It should generate a minimal authored Skill starter, separate from the tool-derived export path.
- Generated files:

```text
agent.json
SKILL.md
```

- Do not generate `references/` or `scripts/` in the init path unless the user explicitly chooses a fuller scaffold in a future phase.

Generated `agent.json`:

```json
{
  "kind": "skill",
  "name": "incident-commander",
  "version": "0.1.0",
  "description": "Incident response coordination playbook",
  "tools": [],
  "skill": {
    "entrypoint": "SKILL.md"
  }
}
```

Generated `SKILL.md` should be workflow-oriented:

```md
---
name: incident-commander
description: Incident response coordination playbook
---

# Incident Commander

## When to use this skill

TODO: Describe the task, situation, or workflow cues that should trigger this skill.

## Procedure

TODO: Add the step-by-step process the agent should follow.

## Inputs and context needed

TODO: List the information the agent should gather before using this skill.

## Expected output

TODO: Describe the format or outcome this skill should produce.

## Safety and escalation notes

TODO: Add constraints, review requirements, or escalation paths.
```

### Backend and database
- The physical `tools` table is used for all package kinds.
- Current database check constraint allows only `tool`, `agent`, and `template`; add `skill`.
- Current backend publish validation allows only `tool`, `agent`, and `template`; add `skill`.
- Current backend install item normalization allows only `tool`, `agent`, and `template`; add `skill`.
- Current backend detail URL helper routes tools, agents, and templates; add skills.
- Existing S3 layout can remain under the current package prefix for compatibility.
- Malware scanning/registry attestation/signature flow should apply to skills the same way it applies to agents and templates.
- Dependency access validation during publish finalize must include Skill dependencies:
  - for `kind: "agent"`, validate top-level `tools` and `skills`
  - for `kind: "skill"`, validate top-level `tools`
  - for `kind: "template"`, validate `template.dependencies.tools`, `template.dependencies.agents`, and `template.dependencies.skills`

### Search, trending, and registry UX
- Skills must appear in search and trending alongside tools, agents, and templates.
- Add `skills` to supported search types/filter tabs.
- Add Skill counts to `totals_by_type`.
- Search SQL is already mostly package-kind based through `tool_search_index.kind`; include `skill` anywhere the code currently enumerates package kinds.
- `Trending` and `Most downloaded` are package-only sorts; include skills there too.
- Registry detail pages should have a Skill-specific route and presentation, likely `/skills/<package_id>/v<version>/overview`.
- Skill detail page should emphasize:
  - `SKILL.md` / README-style manual content
  - description
  - declared tool dependencies, if any
  - compatibility metadata, if any
  - scripts and references, if declared
  - install command
  - published version, signatures, license, and namespace visibility
- Do not present Skills as executable tools.

### Node and Python SDKs
- Node and Python SDKs must recognize `kind: "skill"` anywhere package kinds are modeled or returned.
- SDK support for Skills is inspect/load only in Phase 6A. Skills are not directly runnable SDK artifacts.
- Keep the existing generic `load(...)` API tool-only. It currently returns a callable tool function, so it must not be overloaded to return non-callable Skill or Agent objects.
- If `load(...)` is called with a Skill package ref, it should fail clearly with guidance to use `load_skill` / `loadSkill`.
- Add `load_skill` in Python and `loadSkill` in Node, modeled after the existing `load_agent` / `loadAgent` behavior.
- Loaded Skills should include:
  - `kind: "skill"`
  - package name/version/description
  - manifest metadata
  - the `skill` metadata block
  - `entrypoint_path`
  - `entrypoint_content`
  - declared `references` and `scripts` paths, if present
  - `resolved_tools`
  - package/install directory metadata if the SDK already exposes that for agents/tools
- Do not add `run_skill`, `skill.run`, or equivalent execution APIs in Phase 6A.
- Loaded Agents should add `resolved_skills` alongside the existing `resolved_tools`.
- `resolved_skills` should contain resolved Skill package refs with concrete names and versions, matching the existing `resolved_tools` pattern.
- A Skill-aware SDK usage pattern should look like:

```python
agent = load_agent("@ns/support-agent@0.1.0")
skill_ref = agent["resolved_skills"][0]

skill = load_skill(f"{skill_ref['name']}@{skill_ref['version']}")
tool_ref = skill["resolved_tools"][0]

tool = load(f"{tool_ref['name']}@{tool_ref['version']}")
out = tool({"text": "hello"})
```

### Documentation
Docs must clearly explain the two authoring paths:

1. Start from scratch:

```bash
agentpm init --kind skill --name incident-commander --description "Incident response coordination playbook"
```

2. Start from a tool contract:

```bash
agentpm export --skill @zack/slack-post-message --manifest
```

Docs must say:

- `agentpm init --kind skill` is for authoring a workflow/playbook directly.
- `agentpm export --skill <tool>` is for generating a starter Skill from an existing tool contract.
- Exported skills are generated starting points.
- First-class skills are published packages with their own `agent.json`, version, dependencies, install behavior, and registry page.
- Remote export can resolve a non-installed tool but does not install it or modify the workspace.

## Acceptance criteria
- `agentpm lint` accepts a valid `kind: "skill"` manifest with `skill.entrypoint` and optional `tools`, `references`, `scripts`, `compatibility`, `readme`, and `license` fields.
- `agentpm lint` rejects a Skill manifest that declares `skills`, has missing `skill.entrypoint`, or uses unsafe paths in `skill.entrypoint`, `skill.references`, or `skill.scripts`.
- `agentpm init --kind skill --name incident-commander --description "Incident response coordination playbook"` creates a valid `agent.json` and `SKILL.md`.
- `agentpm publish --dry-run` succeeds for a valid Skill with only `agent.json` and `SKILL.md`.
- Skill publish tarball includes `agent.json` and all declared Skill files while preserving relative paths.
- Skill publish tarball does not include undeclared extra files.
- Skill publish rejects missing declared files and unsafe declared paths.
- Backend publish accepts `manifest.kind == "skill"` and stores the package with kind `skill`.
- Backend publish rejects kind conflicts when a package name already exists as another kind.
- Backend publish finalize validates access to declared dependencies for Skills and for agents/templates that reference Skills.
- Direct `agentpm install @namespace/skill-name@version` resolves, downloads, verifies, extracts to `.agentpm/skills/...`, and writes a lockfile package entry with `kind: "skill"`.
- `agentpm install` supports manifest-driven installs for local `kind: "skill"` manifests, resolving top-level `tools` and writing a `local:skill` lock root.
- Installing an agent with declared `skills` resolves those Skill packages and their tool dependencies.
- Installing a Skill with declared `tools` resolves those tool packages.
- Lockfile roots represent `skills` as a first-class root field rather than under `reserved.skills`.
- Existing old lockfiles with `reserved.skills` are handled safely during read/update/migration.
- `agentpm export --skill <installed-tool> --manifest` generates the existing scaffold plus a valid `kind: "skill"` `agent.json`.
- `agentpm export --skill <non-installed-public-tool> --manifest` can resolve from the registry and generate the scaffold without installing the tool or modifying the lockfile.
- `agentpm export --skill <tool>` without `--manifest` remains backward compatible with the current scaffold shape.
- Search supports a `skills` filter/type and includes skills in `all` results, trending, newest, relevance, and most-downloaded package streams where appropriate.
- Search totals include `skills`.
- Registry detail pages can render a Skill package and link to the proper `/skills/...` URL.
- Existing tool, agent, and template publish/install/search flows continue to pass.
- Python SDK exposes `load_skill(...)` and returns an inspectable Skill object with entrypoint content and `resolved_tools`.
- Node SDK exposes `loadSkill(...)` and returns an inspectable Skill object with entrypoint content and `resolvedTools` or the SDK's existing naming convention.
- Existing SDK `load(...)` remains tool-only and continues returning a callable tool function.
- SDK `load(...)` does not return Skill objects; attempting to load a Skill through `load(...)` fails with a clear message directing users to `load_skill` / `loadSkill`.
- Loaded agents in both SDKs include `resolved_skills` / `resolvedSkills` in addition to existing resolved tool metadata.
- SDK package-kind models, search models, resolve/install models, and package detail models recognize `skill`.

## Risks / edge cases
- Lockfile shape migration may break existing v2 consumers if not handled carefully.
- The current code uses `tools` naming for shared package concepts. Skill implementation should avoid broad renames that create unnecessary churn.
- Search DTOs and frontend types may currently enumerate only `tool|agent|template`; missing one enum update can cause Skill results to disappear or render as tools.
- Publish file selection for Skills is different from tools and templates; accidental directory crawling could publish extra files, while overly strict selection could omit needed references/scripts.
- Relative paths in `SKILL.md` may break if tar paths are not preserved exactly.
- Remote export requires enough registry metadata to render tool contract/examples. If the current API cannot fetch manifests without installing artifacts, implementation may need a small package metadata/read endpoint or reuse existing install init/download without mutating local state.
- Direct install of a package spec resolves by package name and returns stored DB kind; requested kind is not authoritative. Ensure Skill direct install behaves consistently with this existing backend behavior.
- Agents depending on skills and skills depending on tools introduces a two-level graph expansion. Cycle risk is limited because skill-to-skill and agent-to-agent are disallowed.
- Package detail URLs, receipt URLs, docs links, and frontend route generation may each need independent Skill handling.
- Existing tests that assert `tools|agents|templates` totals or item types may fail until `skills` is added.
- SDK `load(...)` has a callable-tool contract. Accidentally broadening it to return non-callable Skill objects would create confusing runtime behavior and type ambiguity.
- SDK naming conventions differ slightly between Python and Node. Keep behavior aligned while following each SDK's existing naming style, such as `resolved_skills` in Python and `resolvedSkills` if that is the Node convention.
- Loaded Skill entrypoint content requires reading installed artifact files. If SDKs currently only read manifests for agents, Skill loading may need a small shared file-read helper.

## Open questions
- Should `skill.compatibility` values be strict enums in the JSON schema, or string arrays with documented recommended values for forward compatibility? Probably enums right now.
- Should `skill.references` and `skill.scripts` be arrays of strings only, or arrays of objects with labels/descriptions? Recommendation for Phase 6A: strings only.
- Should the generated export manifest use the tool leaf name directly, such as `slack-post-message`, or append `-skill` to avoid package name collision with the tool? Recommendation: use the leaf name by default only if package kind uniqueness allows same name across kinds; otherwise append `-skill`.
- Should Skill detail pages render `SKILL.md` from the installed artifact, from `readme_markdown`, or only from a declared/readme field? Recommendation: Probably want to show both. Normal README tab + probably show the SKILL.md content in the overview if it is the entrypoint.

## Related Specs
- Existing manifest schema: `schemas/agentpm.manifest.schema.json`
- Existing CLI publish implementation
- Existing CLI install/lockfile v2 implementation
- Existing CLI export skill scaffold implementation
- Existing CLI init implementation
- Existing backend publish/install service
- Existing shared package data model using the `tools` and `tool_versions` tables
- Existing registry search service and `tool_search_index` materialized view
- Phase 4 workflow template spec
- Phase 5 private namespace and package access rules
