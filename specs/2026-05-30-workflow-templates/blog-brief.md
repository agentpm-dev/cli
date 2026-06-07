# Blog Brief

## Spec

- `agentpm/specs/2026-05-30-workflow-templates/spec.md`

## Is this blog-worthy?

Yes.

This spec is blog-worthy because it marks the point where AgentPM stops being only a package manager for reusable building blocks and becomes a bootstrap layer for complete agent projects.

That is a real product shift:

- not just install a tool
- not just install an agent package
- generate a runnable, editable project with code, docs, lockfiles, and workspace topology already in place

## What shipped

This spec added workflow templates as a first-class part of the AgentPM stack:

- CLI
  - `kind: "template"` manifests are valid
  - `agentpm init --kind template` scaffolds template packages
  - `agentpm new <template-ref> [target-dir]` generates projects from published or local templates
  - required variables can be provided with `--var` or prompted for interactively
  - generated projects get:
    - a synthesized root `agent.json`
    - `agent.lock`
    - `agentpm.workspace.json` where needed
    - `.agentpm/template.json`

- Registry/API
  - template packages can be published and finalized
  - template packages are searchable as a first-class kind
  - template detail pages have their own route and metadata shape
  - template publish receipts and URLs point to template pages instead of tool pages

- Web
  - templates appear in search, namespaces, and landing-page discovery
  - the site now positions templates as the fastest path to a working project
  - template detail pages show:
    - use case
    - execution surfaces
    - stack
    - included package dependencies
    - bootstrap command
    - entrypoints

- Examples
  - official template sources now live under `agentpm-examples/template-packages/*`
  - official generated apps now exist for:
    - Node SDK research assistant
    - Python SDK triage worker
    - CLI automation worker
    - MCP tool server
    - multi-agent support workspace
  - the examples repo now tells both sides of the story:
    - author a template
    - publish it
    - generate a real project from it

## Why this matters

The broader AgentPM point this work proves is:

- users do not just want installable packages
- they also want credible starting points

Without templates, users still face blank-page friction:

- which tools should I install?
- how should the project be structured?
- how do I wire SDK, CLI, or MCP usage?
- what README or env guidance should I start from?

Templates answer that by packaging the starting point itself.

That makes AgentPM useful one layer earlier in the workflow:

- before custom orchestration code exists
- before the project layout exists
- before users know which lower-level packages to assemble

## Strongest concepts and angles

### Angle 1: The fastest path to value is not a package, it is a project starter

Core idea:

- packages are reusable building blocks
- templates are runnable starting points

Why it is interesting:

- it reframes AgentPM from “registry of callable artifacts” to “bootstrap layer for agent systems”
- it acknowledges that most users need help before they are ready to compose packages by hand

Possible framing:

- “The first thing most agent builders need is not another tool. It is a starter project they can actually run.”

### Angle 2: Templates should scaffold code, not execute code

Core idea:

- `agentpm new` copies, renders, installs AgentPM dependencies, and writes lockfiles
- it does not run template hooks, package managers, or user code

Why it is interesting:

- this is the right trust boundary for a package manager
- it preserves auditability and keeps templates understandable

Possible framing:

- “A workflow template should generate your project, not secretly run it.”

### Angle 3: One package manager, multiple execution surfaces

Core idea:

- the official templates now span:
  - Node SDK
  - Python SDK
  - shell/`agentpm run`
  - MCP
  - multi-manifest workspaces

Why it is interesting:

- it proves AgentPM is not tied to one orchestration model
- the same packaging layer can bootstrap several different styles of agent system

Possible framing:

- “A useful agent package manager has to meet people where they work: SDKs, shell scripts, MCP servers, and workspaces.”

### Angle 4: Generated projects can still stay honest about AgentPM’s boundaries

Core idea:

- generated root manifests stay valid normal `kind: "agent"` manifests
- multi-agent templates use workspace metadata instead of inventing recursive `agents[]`
- templates do not pretend AgentPM already has a workflow runtime

Why it is interesting:

- the product story gets stronger without overclaiming
- scaffolding can be ambitious without breaking the manifest model

Possible framing:

- “You can scaffold a multi-agent workspace today without pretending recursive agent orchestration already exists.”

## What was learned

- The distinction between a template package and the generated project has to stay sharp.
  - template authors should maintain a package-level README and a generated-project README separately
  - the root generated `agent.json` should stay synthesized by `agentpm new`

- Local template authoring mattered almost immediately.
  - `agentpm new ./template-dir ...` was necessary for a believable template workflow
  - publish-only authoring would have been too slow and awkward

- Workspace semantics matter more once templates arrive.
  - a generated project may mix:
    - local manifests
    - published agent roots
    - direct tool dependencies
  - `agentpm.workspace.json` became an important contract surface, not just an implementation detail

- Good examples forced the docs and UI to get better.
  - landing-page framing
  - template detail pages
  - namespace support
  - examples docs
  - `agentpm new` docs
  all had to evolve once templates were real

- Safety and permissiveness needed careful balance.
  - preserving unknown placeholders is good for docs/examples inside scaffolds
  - but template authors still need warnings and validation to catch mistakes

## Tie-back to AgentPM

This work reinforces the central AgentPM direction:

- define artifacts once
- version them cleanly
- install them predictably
- expose them across multiple runtime surfaces

Before this spec, AgentPM had a strong story for:

- tools
- agents

After this spec, it also has a strong story for:

- the project starter itself

That makes AgentPM feel less like a narrow package runner and more like the packaging and onboarding layer for agent systems.

## Suggested inputs for ChatGPT Projects

- Possible title ideas
  - `Why AgentPM needed workflow templates`
  - `From packages to project starters`
  - `The fastest path to a working agent project`
  - `Templates are the missing layer in agent packaging`

- Possible hook
  - “Most agent tooling assumes you already know how to wire the project together. Workflow templates are what you add when you realize the blank page is the real bottleneck.”

- Supporting examples or screenshots worth using
  - the registry landing page with the workflow templates section above tools/agents
  - a template detail page showing bootstrap command, use case, stack, and included dependencies
  - `agentpm new @zack/research-assistant-node my-project`
  - a generated project tree with:
    - `agent.json`
    - `agent.lock`
    - `agentpm.workspace.json`
    - `.agentpm/template.json`
  - side-by-side examples repo structure:
    - `template-packages/*`
    - generated app directories
  - the multi-agent workspace example showing:
    - generated local `agents/*.agent.json`
    - published agent roots in `agentpm.workspace.json`
