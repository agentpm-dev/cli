# Blog Brief

## Is this blog-worthy?
- Yes

## Why it matters
- This is the first phase where AgentPM stops being only a package manager for executable tools and becomes a packaging layer for procedural know-how.
- It proves that Skills can be authored, published, installed, searched, loaded, and used across the full AgentPM stack instead of existing only as an export scaffold.
- It gives AgentPM a stronger story for how real agent systems are built: tools provide actions, Skills provide procedure, agents compose both.

## Audience
- Agent engineers
- Tool authors
- Builders
- Maintainers
- Broader AI/agent audience

## Strongest angles
- Angle 1
  - Skills are now first-class packages, not just prompt files or internal app docs.
- Angle 2
  - AgentPM can package both what an agent can do and how it should do it.
- Angle 3
  - Real examples now show Skills influencing runtime behavior across Node and Python apps, not just registry metadata.

## What was built
- Added `kind: "skill"` as a first-class AgentPM package kind.
- Added CLI support for initializing, publishing, packaging, installing, extracting, and locking Skills.
- Added backend support for Skill publish/install resolution, registry detail pages, search, trending, and dependency resolution.
- Added frontend registry support for Skill discovery, distinct Skill visuals, Skill detail pages, manual/readme/security views, and Skill-aware search/navigation.
- Added dedicated Skill manual (`SKILL.md`) storage and registry rendering, separate from README handling.
- Added Node and Python SDK support for:
  - `loadSkill()` / `load_skill()`
  - agent `resolvedSkills`
  - loading installed Skills as inspectable artifacts with manual content and resolved tools
- Fixed a real SDK contract issue so Skills installed through agents can still be loaded even when they do not have dedicated lock roots.
- Seeded the public ecosystem with real Skill packages and updated example agents, templates, and apps to use them.

## What was learned
- Surprising implementation detail
  - The lockfile/SDK contract for Skills was more subtle than for tools. Tools never needed dedicated roots to load, but the first Skill loader implementation assumed they did.
- Important tradeoff
  - The cleanest fix was to keep the CLI lockfile shape small and stable, and make the SDKs more flexible by falling back to installed Skill artifacts plus locked package metadata.
- Constraint that shaped the result
  - Skills were deliberately kept non-executable in this phase. The value comes from packaging procedure and tool relationships, not inventing a new Skill runtime.

## Tie-back to AgentPM
- This phase reinforces the core AgentPM direction: package the building blocks of agent systems once, version them cleanly, and make them portable across runtimes.
- It moves AgentPM closer to being the interoperability layer for agent systems rather than only a tool registry.
- It makes the broader artifact model more credible:
  - Tool = executable capability
  - Skill = procedural know-how
  - Agent = orchestration/composition
  - Template = starter system

## Suggested inputs for ChatGPT Projects
- Possible title ideas
  - Skills Are Now First-Class Packages in AgentPM
  - AgentPM Now Packages Procedure, Not Just Tools
  - From Tool Registry to Agent Building Blocks: Shipping Skills in AgentPM
- Possible hook
  - Most agent stacks can package actions, but not procedure. This phase closes that gap by making Skills versioned, installable, searchable artifacts.
- Supporting examples or screenshots worth using
  - Registry Skill detail page showing Manual + dependency sections
  - Search or trending UI with Skills mixed alongside tools and agents
  - `agentpm init --kind skill` or publish flow for a real Skill package
  - Example app banner showing loaded Skills plus loaded tools
  - A public Skill package such as:
    - `incident-handoff-checklist`
    - `issue-triage-playbook`
    - `research-brief-playbook`
- Suggested proof points to mention
  - Skills are published and installed like packages
  - Skills can depend on tools
  - Agents can depend on Skills
  - SDKs can load Skill manuals and resolved tool refs
  - Real example apps now use Skill manuals to shape agent behavior
