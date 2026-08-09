# Blog Brief

## Spec

- `agentpm/specs/2026-08-01-instruction-profiles/spec.md`

## Is this blog-worthy?

Yes.

This phase is blog-worthy because it extends AgentPM into another core agent-system building block: authored behavior.

Before this work, AgentPM already had artifact stories for:

- tools
- skills
- knowledge
- memory
- agents
- templates

This phase adds a first-class package story for the behavioral layer that usually gets trapped inside app-local system prompts:

- role
- objectives
- principles
- communication posture
- audience adaptation
- boundaries
- declared constraints

It is also blog-worthy because the feature did not stop at schema support. Instruction Profiles shipped as a real artifact flow across the stack: init, lint, publish, install, search, detail pages, SDK loading, and public seeded examples.

## What shipped

This phase added Instruction Profiles as a first-class AgentPM artifact across the stack:

- CLI and manifest contract
  - `kind: "profile"` manifests are valid
  - agents can declare top-level `profiles`
  - templates can declare `template.dependencies.profiles`
  - semantic validation covers structured authored behavior fields like identity, objectives, communication, boundaries, constraints, and compatibility hints

- Init, lint, publish, and install
  - `agentpm init --kind profile`
  - lint support for Profile manifests and diagnostics
  - publish support for Profile packages
  - direct Profile install support
  - agent and template dependency resolution for installed Profile packages

- Lockfile and dependency model
  - lockfile support for first-class `profile:@namespace/name@version` package keys
  - agent roots can carry first-class `profiles` entries
  - template-generated local agent roots can synthesize installed Profile dependencies into generated `agent.json`

- Registry/API
  - Profile packages can be published, searched, viewed, downloaded, and installed as a first-class kind
  - backend persistence and relationship handling now treat Profiles as first-class package metadata instead of pass-through prompt text

- Web
  - landing-page support for Profiles as their own package kind
  - Profile detail pages for identity, objectives, communication, boundaries, constraints, compatibility, and package metadata
  - agent and template detail pages surface Profile dependencies as first-class relationships

- SDKs
  - Node SDK support for installed Profile packages and agent `resolvedProfiles`
  - Python SDK support for installed Profile packages and agent `resolvedProfiles`
  - `loadProfile()` / `load_profile()` expose structured authored metadata, not raw prompt blobs

- Examples and seeded content
  - `@zack/support-response-style`
  - `@zack/incident-operator-style`
  - `@zack/devwork-maintainer-style`
  - example templates, agents, and apps updated to depend on and surface Profiles

## Why this matters

The broader AgentPM point this phase proves is:

- agent portability is not only about actions, procedure, retrieval, and memory
- serious agent systems also need a portable artifact for authored behavior

Without a Profile artifact, behavior usually lives in app-local prompt strings:

- difficult to version
- difficult to review
- difficult to reuse
- difficult to govern
- difficult to keep consistent across runtimes

Instruction Profiles change that by turning the behavioral layer itself into a package:

- versioned
- publishable
- installable
- inspectable
- searchable
- loadable in SDKs
- visible in the registry UI

That gives authored behavior a portability layer before AgentPM tries to own a default harness or enforcement runtime.

## Strongest concepts and angles

### Angle 1: Behavior should be packaged separately from execution

Core idea:

- tool code, workflow code, and authored behavioral posture are different things
- they should not all be trapped in the same system prompt or app file

Why it is interesting:

- it gives engineers a clean mental model
- behavior becomes a reusable artifact instead of app glue

Possible framing:

- `Portable agents need portable behavior, not just portable tools.`

### Angle 2: Instruction Profiles are the missing artifact between prompts and systems

Core idea:

- many teams already know they need stable agent role and communication guidance
- what they lack is a disciplined artifact model for that layer

Why it is interesting:

- it connects familiar prompt-engineering practice to familiar software-engineering structure
- the question shifts from “what prompt should we paste here?” to “what behavior package should this runtime consume?”

Possible framing:

- `A system prompt is not a packaging format.`

### Angle 3: Structured behavior metadata is better than one opaque prompt blob

Core idea:

- the important move was not merely adding another package kind
- it was choosing structured authored behavior over free-form prompt dumps

Why it is interesting:

- identity, objectives, communication, boundaries, and constraints become inspectable and separately understandable
- SDKs and UIs can present the behavior contract without pretending to execute it

Possible framing:

- `Behavior becomes reusable when you can inspect its shape, not just paste its text.`

### Angle 4: Profiles made the package graph feel more complete

Core idea:

- previous phases covered action, know-how, retrieval context, and durable memory
- this phase adds the durable behavior layer those other artifacts often assume but do not model

Why it is interesting:

- it rounds out the artifact model for agent systems
- one example agent now carries every current major AgentPM package kind in the same stack

Possible framing:

- `Portable agents need a package graph for behavior as much as they need one for tools and memory.`

## What was learned

- Profiles needed to stay metadata-first.
  - The strongest decision in the phase was to package authored behavior without pretending AgentPM already owns how runtimes must merge, apply, or enforce it.

- The distinction between Profiles and Skills matters.
  - Profiles describe durable role-level behavior.
  - Skills package procedural know-how, references, scripts, and optional tool relationships.
  - Keeping that boundary clear made the package model easier to explain.

- Structured fields were worth the strictness.
  - Identity, objectives, communication, boundaries, and constraints turned out to be much more useful when they were separately typed and validated instead of dropped into one free-form prompt field.

- First-class dependency treatment matters even for leaf packages.
  - Profiles do not expand their own dependency graph, but they still needed first-class support in install flows, lockfiles, backend relationships, SDK loaders, and UI dependency sections.

- Examples quickly exposed the boundary between “inspectable” and “operational.”
  - Some example apps only display Profile metadata.
  - Some also pass Profile guidance into their runtime prompt.
  - That clarified the next-phase opportunity: a default AgentPM harness is where Profile application should become more consistent and more impactful.

## Tie-back to AgentPM

This phase reinforces the central AgentPM direction:

- define artifacts once
- version them cleanly
- install them predictably
- move them across runtimes and host languages

Before this phase, AgentPM could package:

- tools
- skills
- knowledge
- memory
- agents
- templates

After this phase, it can also package:

- authored behavior
- role identity
- communication guidance
- boundaries and declared constraints

That strengthens the overall claim that AgentPM is becoming the packaging and interoperability layer for the major building blocks of agent systems, not just their executable parts.

## Suggested inputs for ChatGPT Projects

- Possible title ideas
  - `Behavior Should Be Portable Too`
  - `AgentPM Now Packages Instruction Profiles`
  - `Portable Agents Need Portable Behavior`
  - `A System Prompt Is Not a Packaging Format`

- Possible hook
  - Most teams still keep agent behavior in app-local system prompts. This phase argues that role, objectives, communication style, and boundaries should be versioned, reusable artifacts in their own right.

- Supporting examples or screenshots worth using
  - `agentpm init --kind profile` output and scaffold
  - a Profile detail page showing identity, objectives, communication, and constraints
  - an agent detail page showing Profile dependencies as first-class relationships
  - a template detail page showing installed Profile dependencies
  - a Python or Node SDK example loading an installed Profile package
  - one example app banner showing Knowledge, Memory, and Profile packages loaded side by side
