# Blog Brief

## Spec

- `agentpm/specs/2026-07-02-knowledge-base-vector-context-artifacts/spec.md`

## Is this blog-worthy?

Yes.

This phase is blog-worthy because it marks the point where AgentPM stops being only a packaging layer for actions, procedures, compositions, and starters, and becomes a packaging layer for prepared retrieval context too.

This is a meaningful product expansion:

- not just package what an agent can do
- not just package how an agent should operate
- package the context corpus the agent should carry with it

It also shipped as a complete cross-stack capability rather than a partial CLI experiment.

## What shipped

This phase added Knowledge as a first-class AgentPM artifact across the stack:

- CLI
  - `kind: "knowledge"` manifests are valid
  - `agentpm init --kind knowledge --mode context`
  - `agentpm init --kind knowledge --mode vector`
  - `agentpm knowledge build` for context and vector packages
  - `agentpm knowledge inspect`
  - `agentpm knowledge query`
  - `agentpm publish` validation and packaging support for Knowledge
  - `agentpm install` support for Knowledge in agents, templates, and direct installs
  - lockfile support for Knowledge dependencies and migration from older reserved forms

- Knowledge package model
  - context-mode Knowledge packages can carry direct documents as prepared context
  - vector-mode Knowledge packages can carry:
    - source documents
    - `chunks.jsonl`
    - `sources.jsonl`
    - embedding payloads
    - built local exact-search metadata
  - vector metadata and index outputs are validated and kept consistent with manifest-declared retrieval settings

- Local retrieval workflow
  - exact local vector query path shipped for the MVP
  - query supports direct vector JSON input
  - query also supports adapter/embedding-command input for converting raw text into vectors outside AgentPM
  - inspect and query work against both local package directories and installed packages

- Registry/API
  - Knowledge packages can be published and installed as a first-class kind
  - search and detail payloads include Knowledge package data
  - private/public install resolution and publish validation support Knowledge
  - trending and landing-page discovery were updated so Knowledge appears alongside the other package kinds

- Web
  - the landing page now has a dedicated Knowledge section
  - Knowledge has first-class search/discovery support
  - Knowledge detail pages render Knowledge-specific overview data
  - vector and context packages present distinct metadata, retrieval, and corpus information
  - profile billing UI was also corrected to render manual grants honestly during the tail end of the phase

- SDKs
  - Node SDK: `loadKnowledge()` and agent `resolvedKnowledge`
  - Python SDK: `load_knowledge()` and agent `resolved_knowledge`
  - both SDKs resolve Knowledge installed through agents and templates, not only directly-rooted installs

- Examples and seeded content
  - a multi-file context package for support guidance
  - a vector package for devwork maintainer guidance
  - a larger real-content vector package built from AgentPM docs
  - updated templates, agent packages, and example apps to depend on and surface Knowledge

## Why this matters

The broader AgentPM point this work proves is:

- a serious agent packaging layer cannot stop at tools, skills, and agents
- it also needs a portable story for context and retrieval corpora

Without Knowledge packaging, retrieval remains ad hoc:

- every app invents its own chunking format
- every app invents its own metadata contract
- every app invents its own install and portability story
- prepared corpora do not move cleanly across runtimes or host languages

Knowledge changes that by giving prepared context a package form:

- versioned
- publishable
- installable
- inspectable
- queryable
- loadable in SDKs

That makes retrieval context part of the same artifact system as the rest of the agent stack.

## Strongest concepts and angles

### Angle 1: Retrieval context should be portable, not re-built from scratch in every app

Core idea:

- most teams already know how to build a vector corpus
- very few teams have a clean way to package and move that corpus between systems

Why it is interesting:

- it reframes vector search from an app-local implementation detail into a reusable artifact
- it makes retrieval part of deployment and distribution, not just preprocessing

Possible framing:

- “If tools and skills are portable, your retrieval corpus should be portable too.”

### Angle 2: Knowledge is not a vector database product, it is a packaging contract

Core idea:

- AgentPM did not try to become a hosted retrieval system in this phase
- it shipped a local exact-search MVP with explicit artifact contracts

Why it is interesting:

- the product stays disciplined
- the useful thing here is the package format and interoperability surface, not a grand unified search backend

Possible framing:

- “The first job was not to build a vector platform. It was to make retrieval corpora portable.”

### Angle 3: One package manager now covers both procedure and context

Core idea:

- the previous phase made Skills first-class
- this phase makes Knowledge first-class

Why it is interesting:

- together they form a much more credible artifact model for agent systems
- AgentPM can now package:
  - actions
  - procedure
  - context
  - composition
  - starters

Possible framing:

- “Agent systems need more than tools. They need procedure and context with the same portability story.”

### Angle 4: Real examples matter more than abstract schemas

Core idea:

- this phase did not stop at CLI and schema support
- it seeded public Knowledge packages and real example app usage

Why it is interesting:

- users can see the full story:
  - author
  - build
  - inspect
  - query
  - publish
  - install
  - load in an app

Possible framing:

- “A packaging feature becomes real when the examples repo starts depending on it.”

## What was learned

- The package contract mattered as much as the retrieval math.
  - The important work was not just cosine similarity and file parsing.
  - It was defining stable manifest fields, build metadata, packaging rules, install layout, and SDK load behavior.

- Leaf-package behavior needed its own lock/install policy.
  - Knowledge behaves more like a leaf dependency than an orchestration root.
  - The install and lockfile story had to be made explicit so direct installs, agents, templates, CLI flows, and SDK loading all agreed.

- The SDK loading contract was subtler than it first looked.
  - Knowledge installed through an agent or template still needs to be loadable even when it is not a dedicated root.
  - The final SDK behavior had to follow installed artifact layout plus lock metadata, rather than assuming a dedicated Knowledge root always exists.

- Examples exposed real workflow expectations quickly.
  - Users want to see real chunking and embedding workflows, not only handcrafted JSONL fixtures.
  - The docs-backed vector example turned out to be important because it showed an end-to-end path with real content and recognizable tooling.

- Manual grants surfaced a separate product/UI truth problem.
  - Access control correctly honored manual entitlements before the profile billing UI did.
  - Fixing that made the product more honest: a manual grant now reads as a manual grant, not as a paid plan.

## Tie-back to AgentPM

This phase reinforces the central AgentPM direction:

- define artifacts once
- version them cleanly
- install them predictably
- move them across runtimes

Before this phase, AgentPM had a stronger story for:

- tool artifacts
- skill artifacts
- agent artifacts
- template artifacts

After this phase, it also has a real story for:

- prepared context bundles
- portable retrieval corpora

That makes the broader artifact model more credible:

- Tool = executable capability
- Skill = procedural know-how
- Knowledge = prepared retrieval context
- Agent = orchestration/composition
- Template = starter system

## Suggested inputs for ChatGPT Projects

- Possible title ideas
  - `Knowledge is now a first-class package in AgentPM`
  - `Shipping portable retrieval context in AgentPM`
  - `Why AgentPM now packages context, not just tools`
  - `From tools to retrieval corpora: making Knowledge portable`

- Possible hook
  - “Most teams know how to build a vector corpus. Very few have a clean way to version it, publish it, install it, and move it between apps. This phase turns that corpus into a package.”

- Supporting examples or screenshots worth using
  - a Knowledge detail page for a context package
  - a Knowledge detail page for a vector package with corpus and retrieval metadata
  - `agentpm init --kind knowledge --mode vector`
  - `agentpm knowledge build`
  - `agentpm knowledge inspect`
  - `agentpm knowledge query`
  - the docs-backed `agentpm-docs` package
  - example app output showing loaded Knowledge alongside Skills and tools

- Suggested proof points to mention
  - Knowledge is now a first-class package kind
  - both context and vector modes shipped
  - local exact-search query shipped for the MVP
  - publish/install flows support Knowledge end-to-end
  - Node and Python SDKs can load Knowledge
  - real public Knowledge packages were seeded into the registry
