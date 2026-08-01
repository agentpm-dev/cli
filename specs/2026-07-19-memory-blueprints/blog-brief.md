# Blog Brief

## Spec

- `agentpm/specs/2026-07-19-memory-blueprints/spec.md`

## Is this blog-worthy?

Yes.

This phase is blog-worthy because it extends AgentPM beyond actions, procedure, composition, starters, and retrieval context into another core agent-system building block: Memory Blueprints.

That is a meaningful product step:

- not just package what an agent can do
- not just package how an agent should operate
- not just package the retrieval context it can carry
- package the structure of durable memory it expects around it

It is also blog-worthy because this was not left as a schema-only idea. Memory shipped as a cross-stack artifact story with authoring, validation, build output, inspection, publish/install support, backend persistence, SDK loading, UI presentation, and seeded public examples.

## What shipped

This phase added Memory Blueprints as a first-class AgentPM artifact across the stack:

- CLI and manifest contract
  - `kind: "memory"` manifests are valid
  - templates and agents can declare Memory dependencies
  - semantic validation covers scopes, record types, spaces, retrieval modes, retention, constraints, lifecycle operations, triggers, and governance annotations

- Build and artifact model
  - `agentpm memory build` generates resolved record contracts
  - generated contracts combine the standard AgentPM memory-record envelope with authored content schemas
  - `memory/contracts/index.json` records the generated contract inventory
  - `memory/build.json` records authored-input and generated-output hashes plus freshness metadata

- Inspection and publish readiness
  - `agentpm memory inspect` works for local packages and installed packages
  - inspect reports normalized states such as `not_built`, `fresh`, `stale`, `invalid`, and `unsupported`
  - `agentpm publish` for Memory packages requires current build artifacts and rejects stale or malformed output

- Init and install flows
  - `agentpm init --kind memory`
  - direct Memory install support
  - agent and template Memory dependency resolution
  - lockfile support for first-class Memory package keys and relationships

- Registry/API
  - Memory packages can be published, searched, viewed, downloaded, and installed as a first-class kind
  - backend publish finalization validates packaged `memory/build.json`, `memory/contracts/index.json`, source schemas, and resolved contracts
  - extracted Memory metadata and contract data are persisted for registry presentation and API use

- Web
  - landing-page support for Memory as its own package kind
  - Memory detail pages for scopes, record types, spaces, lifecycle operations, governance, and record contracts
  - agent and template detail pages now surface Memory dependencies as first-class relationships

- SDKs
  - Node SDK support for installed Memory packages, record-contract loading, and agent `resolvedMemory`
  - Python SDK support for installed Memory packages, record-contract loading, and agent `resolvedMemory`

- Examples and seeded content
  - `@zack/support-customer-state`
  - `@zack/conversation-continuity`
  - `@zack/devwork-maintainer-state`
  - example agents, templates, and apps updated to depend on and display Memory packages

## Why this matters

The broader AgentPM point this work proves is:

- agent portability is not only about tools and retrieval
- serious agent systems also need a portable contract for durable state

Without a memory artifact, teams fall back to app-local conventions:

- ad hoc JSON shapes
- store-specific record formats
- undocumented lifecycle assumptions
- hand-wired persistence that does not move cleanly across runtimes

Memory Blueprints change that by making memory structure itself packageable:

- versioned
- publishable
- installable
- inspectable
- loadable in SDKs
- understandable in the registry UI

That gives memory a portability layer before AgentPM tries to own a runtime memory store.

## Strongest concepts and angles

### Angle 1: Memory should be packaged as a contract before it is hosted as infrastructure

Core idea:

- the first useful step is not a managed memory backend
- the first useful step is a portable contract for what memory records look like

Why it is interesting:

- it is disciplined product scope
- it makes memory interoperable without pretending the storage/runtime problem is already solved

Possible framing:

- “Before memory can be portable, its shape has to be portable.”

### Angle 2: Agent systems need more than tools, skills, and retrieval

Core idea:

- the previous phases expanded AgentPM from tools to skills and knowledge
- this phase adds durable state structure to that artifact model

Why it is interesting:

- it makes the AgentPM packaging story much more complete
- agents now have a package-level way to express not only actions and context, but also expected memory structure

Possible framing:

- “Portable agents need portable memory contracts, not just portable tools.”

### Angle 3: The hard part was contract discipline, not just schema generation

Core idea:

- the important work was not merely generating JSON Schemas
- it was defining a safe, inspectable, reproducible artifact lifecycle

Why it is interesting:

- readers can see that the product problem was:
  - manifest vocabulary
  - schema safety
  - deterministic build output
  - freshness checks
  - publish validation
  - install layout
  - SDK loading

Possible framing:

- “Memory became real when build, publish, inspect, install, and load all agreed on the same contract.”

### Angle 4: Examples made the feature credible

Core idea:

- this phase did not stop at abstract support
- it seeded three visibly different public Memory packages and wired them into real example flows

Why it is interesting:

- users can see simple, rich, and workflow-oriented Memory Blueprints rather than one toy example repeated three times

Possible framing:

- “A package kind starts to matter when real agents and templates begin depending on it.”

## What was learned

- Memory needed to be treated as a contract artifact, not a hidden implementation detail.
  - The strongest outcome was a stable authored blueprint plus generated resolved contracts, not a runtime store.

- Reproducibility mattered immediately.
  - `memory/build.json`, contract hashes, and freshness checks turned out to be essential because publish and inspect both depend on build output being trustworthy and explainable.

- Safe schema handling was a real part of the product, not a cleanup item.
  - Package-relative schema paths, `$ref` behavior, symlink containment, and self-contained generated contracts all mattered because installed packages have to remain valid outside the author’s original directory.

- Memory behaves like a leaf package, but still changes the rest of the stack.
  - Even though Memory packages do not expand their own dependency graphs, they still touched install flows, lockfiles, backend relationship handling, SDK loading, and UI dependency presentation.

- Examples quickly clarified the right scope boundary.
  - The best examples were the ones that made memory visible and believable without pretending current apps were already persisting or consolidating records at runtime.

## Tie-back to AgentPM

This phase reinforces the central AgentPM direction:

- define artifacts once
- version them cleanly
- install them predictably
- move them across runtimes and host languages

Before this phase, AgentPM had artifact stories for:

- tools
- skills
- knowledge
- agents
- templates

After this phase, it also has a real artifact story for:

- memory structure
- resolved record contracts
- declarative lifecycle intent

That strengthens the overall product claim that AgentPM is becoming the packaging and interoperability layer for the major building blocks of agent systems.

## Suggested inputs for ChatGPT Projects

- Possible title ideas
  - `Memory Should Be Portable Before It Is Hosted`
  - `AgentPM Now Packages Memory Blueprints`
  - `Portable Agents Need Portable Memory Contracts`
  - `How AgentPM Turned Memory Structure Into a Package Artifact`

- Possible hook
  - Most teams treat memory as an app-local implementation detail. This phase argues that before memory can be reusable across runtimes, its structure has to become a portable artifact in its own right.

- Supporting examples or screenshots worth using
  - `agentpm memory inspect` output for `support-customer-state`
  - `agentpm memory inspect` output for `conversation-continuity`
  - Memory detail page showing spaces, lifecycle operations, and contract navigation
  - an example app banner showing installed Knowledge plus installed Memory side by side
  - one generated resolved record contract excerpt showing the standard envelope plus authored content schema
