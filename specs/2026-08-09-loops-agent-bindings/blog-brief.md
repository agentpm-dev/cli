# Blog Brief

## Spec

- `agentpm/specs/2026-08-09-loops-agent-bindings/spec.md`

## Is this blog-worthy?

Yes.

This phase is blog-worthy because it adds another core agent-system building block to AgentPM: portable control flow.

Before this work, AgentPM could package:

- tools
- skills
- knowledge
- memory
- profiles
- agents
- templates

This phase adds a first-class package story for iterative orchestration itself:

- phases
- transitions
- checkpoints
- limits
- access intent
- authored agent bindings that describe how an agent expects those surfaces to be scoped

It is also blog-worthy because Loops did not ship as a schema-only idea. They shipped across the stack: manifest support, semantic linting, init flows, install/publish flows, backend persistence, SDK loading, web detail pages, and public seeded examples.

## What shipped

This phase added Loops and authored Agent bindings as first-class AgentPM artifacts across the stack:

- CLI and manifest contract
  - `kind: "loop"` manifests are valid
  - agents can declare a singular top-level `loop`
  - templates can declare singular `template.dependencies.loop`
  - agents can declare structured `bindings`
  - semantic validation covers:
    - entry phase
    - declared phases and outcomes
    - transitions and terminal targets
    - checkpoints
    - limits
    - access intent
    - error policy
    - agent binding shape and loop/binding integrity

- Init, lint, publish, and install
  - `agentpm init --kind loop`
  - lint support for Loop manifests and binding-aware Agent manifests
  - publish support for Loop packages
  - direct Loop install support
  - agent and template dependency resolution for Loop packages

- Lockfile and dependency model
  - lockfile support for first-class `loop:@namespace/name@version` package keys
  - agent roots can carry singular `loop` relationships
  - template-generated local agent roots can synthesize an installed Loop dependency into generated `agent.json`

- Registry/API
  - Loop packages can be published, searched, viewed, downloaded, and installed as a first-class kind
  - backend persistence and relationship handling now treat Loops as first-class package metadata
  - agent generic manifest/detail responses preserve authored top-level `loop` and `bindings`

- Web
  - landing-page support for Loops as their own package kind
  - Loop detail pages for graph overview, phases, transitions, checkpoints, limits, access, and error policy
  - lightweight static loop-graph rendering in the overview tab
  - agent detail pages surface authored bindings in their own tab
  - template and agent detail pages treat Loop dependencies as first-class relationships

- SDKs
  - Node SDK support for installed Loop packages and agent `resolvedLoop`
  - Python SDK support for installed Loop packages and agent `resolvedLoop`
  - `loadLoop()` / `load_loop()` expose structured loop metadata rather than execution behavior
  - agent loaders expose authored `bindings` metadata alongside the resolved Loop relationship

- Examples and seeded content
  - `@zack/support-escalation-loop`
  - `@zack/incident-response-loop`
  - `@zack/devwork-triage-loop`
  - `@zack/react-loop`
  - example templates, agents, and apps updated to depend on and surface Loop packages and bindings

## Why this matters

The broader AgentPM point this phase proves is:

- portable agents do not only need portable tools, context, memory, and behavior
- they also need a portable way to package iterative control flow

Without a Loop artifact, orchestration usually stays trapped in app code:

- one-off graph structures
- runtime-specific node wiring
- hidden approval semantics
- undocumented handoff conditions
- no clean package identity for the workflow itself

Loops change that by turning authored orchestration shape into a package:

- versioned
- publishable
- installable
- inspectable
- searchable
- loadable in SDKs
- visible in the registry UI

That gives control flow its own portability layer before AgentPM tries to own a default execution harness for it.

## Strongest concepts and angles

### Angle 1: Control flow should be portable, not trapped inside app code

Core idea:

- many teams have agent loops today
- very few have a way to package those loops as reusable artifacts

Why it is interesting:

- it separates orchestration shape from runtime implementation
- the same loop can be inspected, versioned, and reused across different systems

Possible framing:

- `Portable agents need portable control flow, not just portable tools.`

### Angle 2: Loops are metadata-first, not a hidden runtime

Core idea:

- AgentPM did not ship a universal Loop executor in this phase
- it shipped a portable authored contract for control flow plus binding metadata

Why it is interesting:

- it keeps product scope disciplined
- it makes the artifact useful before AgentPM owns a default harness

Possible framing:

- `The first job was not to run every loop. It was to make loops portable.`

### Angle 3: Bindings make the orchestration contract more realistic

Core idea:

- a loop alone says how work moves
- bindings say what context surfaces an agent expects around those phases

Why it is interesting:

- it turns an abstract phase graph into something closer to a portable execution contract
- it creates a clean bridge between packageable orchestration and future runtime behavior

Possible framing:

- `A loop needs more than arrows. It needs a declared contract for what each phase can see and use.`

### Angle 4: Examples made the feature credible

Core idea:

- this phase did not stop at one abstract graph
- it seeded support, ops, devwork, and standalone loop patterns

Why it is interesting:

- users can see both domain-specific loops and a more general reusable pattern
- the examples show loops working alongside the other package kinds rather than in isolation

Possible framing:

- `A package kind becomes real when published agents, templates, and apps start depending on it.`

## What was learned

- Loops needed to stay declarative in this phase.
  - The strongest decision was to package control-flow intent without pretending AgentPM already owns the runtime that executes every phase transition.

- A singular Loop dependency model was the right fit.
  - Agents and templates carry one top-level Loop relationship rather than another array-shaped dependency bucket, which made the authored contract easier to read and the generated root-agent behavior easier to explain.

- Bindings clarified the next runtime boundary quickly.
  - As soon as examples began carrying global and phase bindings for tools, knowledge, memory, profiles, and MCP surfaces, it became clear that the next phase opportunity is not “invent more schema,” but “make a default harness apply those contracts in a meaningful way.”

- Loop graphs needed thoughtful presentation in the UI.
  - Rendering phases and transitions as a static graph turned out to be important because text-only transition lists do not communicate iterative control flow nearly as well.

- Examples quickly differentiated domain loops from reusable patterns.
  - Support, incident, and devwork loops proved that Loops fit naturally into the broader AgentPM package story.
  - The standalone `react-loop` example proved that AgentPM can also seed widely recognizable orchestration patterns that are not tied to one application domain.

## Tie-back to AgentPM

This phase reinforces the central AgentPM direction:

- define artifacts once
- version them cleanly
- install them predictably
- move them across runtimes and host languages

Before this phase, AgentPM could package:

- actions
- procedural know-how
- retrieval context
- memory structure
- authored behavior
- agents
- templates

After this phase, it can also package:

- iterative control flow
- approval and handoff structure
- phase-level access intent
- authored execution-surface bindings

That strengthens the overall claim that AgentPM is becoming the packaging and interoperability layer for the major building blocks of agent systems, not just their executable pieces.

## Suggested inputs for ChatGPT Projects

- Possible title ideas
  - `Portable Agents Need Portable Control Flow`
  - `AgentPM Now Packages Loops`
  - `Why Agent Loops Should Be Package Artifacts`
  - `From Packaged Tools to Packaged Orchestration`

- Possible hook
  - Most agent teams can point to a loop in their codebase, but almost none can package that control flow as a reusable artifact. This phase turns iterative orchestration into something publishable, inspectable, and installable.

- Supporting examples or screenshots worth using
  - Loop detail page with the static graph view
  - `agentpm init --kind loop` starter output
  - published `support-escalation-loop`, `incident-response-loop`, or `react-loop` detail pages
  - agent detail page showing the Bindings tab
  - example app startup banners showing loaded Loop packages and authored binding summaries
