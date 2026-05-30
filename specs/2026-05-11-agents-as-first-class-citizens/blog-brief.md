# Blog Brief

## Spec

- `agentpm/specs/2026-05-11-agents-as-first-class-citizens/spec.md`

## Is this blog-worthy?

Yes.

This spec is blog-worthy because it marks the point where AgentPM moves from "package and run tools" to "treat agents themselves as installable, publishable, searchable packages."

That is a real product shift, not just an internal refactor.

## What shipped

This spec made agents first-class across the AgentPM stack:

- CLI
  - agent manifests are supported as a real package kind
  - lockfile v2 can represent agent roots, local agent roots, and multiple versions of the same tool
  - direct package install can install published agents into `.agentpm/agents/...`
  - manifest-driven installs and direct installs now coexist with clearer root semantics

- Registry/API
  - publish flows accept agent packages
  - install resolution expands an agent package into its tool dependency graph
  - search supports agent package results alongside tools and namespaces

- SDKs
  - Node: `loadAgent(...)`
  - Python: `load_agent(...)`
  - both load installed agent packages, read lockfile v2 roots, and expose exact resolved tool refs

- Website
  - landing page now reflects tools and agents as distinct package surfaces
  - search/explore supports agents
  - agent detail pages exist with overview, readme, security, and examples
  - namespace pages now display mixed tool + agent package results

- Examples
  - three current recommended apps now consume published agent packages instead of only local manifests
  - agent package sources are split into `agent-packages/*`
  - the examples now tell a clearer “publish an agent, install it, load it, use its tools” story

## Why this matters

The broader AgentPM point this work proves is:

- tools are not the only unit that should be packageable
- agent definitions themselves need:
  - identity
  - versioning
  - install semantics
  - dependency graphs
  - SDK loading surfaces

Without that, every app has to rebuild its own agent composition layer from scratch.

With this work, AgentPM can now say:

- package tools
- package agents that depend on tools
- install both
- load both
- search both

That is a much stronger ecosystem story than “AgentPM is just a tool runner.”

## Strongest concepts and angles

### Angle 1: Agents should be packages too

Core idea:

- if tools can be packaged, versioned, and installed, the agent definitions that compose them should be packageable too

Why it is interesting:

- most agent systems still treat the “agent” as app-local glue
- this work turns the agent definition into something portable and installable

Possible framing:

- “The missing layer in agent tooling is not just packaging tools. It is packaging the compositions built from them.”

### Angle 2: Lockfiles are what make agent composition real

Core idea:

- agent manifests express intent
- `agent.lock` v2 records the exact realized tool graph

Why it is interesting:

- this is what allows:
  - exact resolved tool versions
  - multiple versions of the same tool to coexist
  - `loadAgent()` / `load_agent()` to surface exact resolved tool refs instead of guessing

Possible framing:

- “Agent packages become real only when their dependency graph is pinned, inspectable, and loadable.”

### Angle 3: The SDK pattern is now clearer: load the agent, then load its tools

Core idea:

- `loadAgent(...)` / `load_agent(...)` do not execute an agent
- they expose the installed manifest plus exact resolved tool refs
- the app then decides which tools to load and invoke

Why it is interesting:

- it keeps the SDK surface explicit and composable
- it avoids pretending the SDK is an all-in-one agent runtime

Possible framing:

- “An agent package should tell your app exactly which tools it resolved to, without hiding the execution model.”

### Angle 4: Example apps now tell the right product story

Core idea:

- the examples repo no longer stops at local-manifest demos
- the recommended apps now install published agent packages and do real work with them

Why it is interesting:

- it is a much stronger proof point than a synthetic toy `loadAgent()` snippet
- it shows:
  - publish
  - install
  - SDK load
  - real app orchestration

Possible framing:

- “The best example of an agent package is not a contrived demo. It is a real app that installs one and does useful work.”

## What was learned

- The lockfile/root story matters more than it first appears.
  - repeated direct installs, manifest-driven installs, and mixed local vs registry roots needed explicit semantics
  - otherwise you get ghost packages on disk or roots that do not reflect user intent

- Public contract naming debt shows up fast.
  - internal tables still use tool-oriented names in places
  - the web/API layer had to be deliberate about not leaking tool-only concepts into agent surfaces

- SDKs needed exact lock-root lookup behavior, not just directory scanning.
  - installed layout alone is not enough to recover the exact resolved dependency graph for an agent

- Python and Node app roots behave differently in practice.
  - the Python SDK needed `pyproject.toml` root detection for installed-agent apps
  - that kind of cross-language packaging detail is easy to miss until real examples use the flow end to end

## Tie-back to AgentPM

This work reinforces the central AgentPM idea:

- package once
- version cleanly
- install predictably
- expose the same packaged artifact across different orchestration styles and runtimes

Before this spec, that story was strong for tools.

After this spec, it is much stronger for the larger unit most people actually want to move around:

- the agent definition itself

That makes AgentPM look less like a narrow tool runtime and more like a packaging layer for agent systems.

## Suggested inputs for ChatGPT Projects

- Possible title ideas
  - `Why agents need package managers too`
  - `Agents should be installable packages`
  - `From packaged tools to packaged agents`
  - `What it takes to make agent definitions portable`

- Possible hook
  - “Most teams package tools, but keep the agent definitions that compose them trapped inside app code. That is the wrong boundary.”

- Supporting examples or screenshots worth using
  - the landing page showing separate Tools and Agents sections
  - the agent detail page with dependencies, examples, and security tab
  - an `agent.lock` v2 excerpt showing an agent root plus resolved tool package keys
  - a Node or Python snippet that does:
    - `loadAgent(...)`
    - read `resolvedTools`
    - `load(...)` a resolved tool
  - side-by-side examples app structure:
    - `agent-packages/*`
    - `agent-app-*`
