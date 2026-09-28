# Blog Brief

## Phase

- Phase 7B: Built-In Harness Runtime
- `agentpm/specs/2026-08-20-harness/spec.md`

## Is this blog-worthy?

Yes.

This phase is blog-worthy because it turns AgentPM from a packaging system for agent artifacts into a packaging system with a canonical reference runtime.

Before this phase, AgentPM could define and distribute the building blocks of an agent system:

- Tools
- Skills
- Knowledge
- Memory Blueprints
- Instruction Profiles
- Loops
- Agents
- Templates

Phase 7B proves those artifacts can be interpreted together by one transparent runtime:

- from the CLI
- from an interactive terminal UI
- from one-shot headless scripts
- from Node and Python SDK host applications
- with the same Rust HarnessEngine underneath

The important story is not "AgentPM added another runner." The important story is that portable agent artifacts need a runtime boundary that is inspectable, verifiable, and shared across surfaces.

## What shipped

Phase 7B added the AgentPM Harness: the canonical AgentPM reference executor for installed Agents.

Major shipped areas:

- `agentpm harness`
  - default interactive Ratatui TUI
  - one-shot `--headless`
  - `--machine` protocol for SDK-hosted apps
  - JSON readiness/preflight reporting
  - JSON reports and JSONL traces for Runs

- Canonical Rust HarnessEngine
  - Loop traversal
  - phase execution
  - outcomes and terminal targets
  - max-step and runtime limits
  - checkpoints and approvals
  - cancellation
  - phase-local transcripts
  - cross-phase output and action-ledger state

- Runtime configuration
  - `agentpm.harness.json`
  - model/provider selection
  - workspace scopes
  - runtime state directory
  - Hook bindings
  - approvals
  - Knowledge and Memory runtime mappings
  - MCP import/export configuration
  - trace and TUI branding options

- EffectivePhase computation
  - authored Agent bindings
  - Skill-inherited capabilities
  - Loop access restrictions
  - runtime augmentation
  - provider readiness
  - explicit suppression reasons

- Model support
  - OpenAI
  - Anthropic
  - Ollama
  - custom process and SDK-hosted model runtimes
  - provider-native action/tool serialization where available

- Tool and Skill execution
  - Harness ToolRuntime delegates through public `agentpm run --machine`
  - Tool argument validation, runtime enforcement, retries, and cancellation
  - Skill resources remain distinct from executable Tool authority

- Hooks, approvals, and controls
  - typed HookRuntime points
  - process and SDK-hosted Hooks
  - approval controllers
  - TUI approval controls
  - external Memory operation controls
  - cancellation through Harness control paths

- Knowledge runtime
  - local retrieval
  - embedding-provider execution
  - custom KnowledgeRuntime
  - Pinecone and pgvector reference provider paths and conformance

- Memory runtime
  - local SQLite MemoryRuntime
  - direct Memory read/write actions
  - semantic Memory retrieval
  - lifecycle operations
  - durable trigger state
  - persistence review
  - custom MemoryRuntime
  - PostgreSQL/pgvector and Redis reference provider paths and conformance

- MCP support
  - Agent-authored outward MCP exports through Harness-managed surfaces
  - scoped inward MCP imports as runtime Tool augmentation
  - machine lifecycle support for exported surfaces
  - TUI, headless, and machine export hosting behavior

- SDK-hosted Harness
  - Node SDK machine client
  - Python SDK machine client
  - typed callbacks for Hooks, providers, approvals, events, cancellation, and reports
  - parity verification across headless, Node SDK, Python SDK, and TUI surfaces

- TUI
  - responsive terminal shell
  - workspace readiness rail
  - active vs terminal Run states
  - composer behavior
  - approvals/cancellation controls
  - Trace, Memory, Reports, and detail views
  - no render-loop blocking for runtime service activation

- Phase output and cross-phase state
  - reliable phase completion output
  - bounded cross-phase prompt section
  - compact prior-phase action ledger
  - measurement of output preservation and repeated-action behavior

- Documentation and examples
  - Harness CLI/config/protocol/provider/SDK docs
  - manual release verification runbooks
  - focused Harness template family:
    - `harness-minimal-agent`
    - `harness-sdk-host-node`
    - `harness-sdk-host-python`
    - `harness-embedding-provider`
    - `harness-provider-lab`
    - `harness-mcp-bridge`

## Why this matters

The broader AgentPM point this phase proves is:

- a portable Agent needs more than installable parts
- it needs a clear runtime contract for how those parts are assembled, governed, executed, observed, and extended

Without a built-in Harness, every application author has to recreate the same hard boundary:

- resolve the Agent and lockfile
- decide which capabilities are available in each phase
- assemble prompts
- call a model provider
- execute Tools
- retrieve Knowledge
- persist and retrieve Memory
- handle approvals
- enforce Loop limits
- expose or consume MCP
- trace what happened
- write a report
- make cancellation and errors deterministic

That is too much behavior to leave implicit in every host app.

Harness makes the runtime boundary explicit. It gives AgentPM a reference implementation that third-party runtimes can compare against, SDK-hosted apps can drive, and users can inspect when something behaves unexpectedly.

## Strongest concepts and angles

### Angle 1: Portable agents need a reference runtime

Core idea:

- Packaging is not enough once artifacts start depending on one another.
- A reusable Agent needs a known interpretation of Loop phases, bindings, Tools, Knowledge, Memory, approvals, and runtime services.

Why it is interesting:

- This maps directly to familiar package-manager instincts.
- A lockfile is only useful if the installed graph has a predictable execution boundary.

Possible framing:

- `Portable agents need more than portable packages. They need a portable runtime contract.`

### Angle 2: The Harness is a reference executor, not a framework takeover

Core idea:

- Harness does not replace host apps, frameworks, or SDKs.
- It gives them a common engine they can launch, observe, and extend.

Why it is interesting:

- Node and Python hosts can own providers, Hooks, approvals, and UI/API boundaries without reimplementing Loop traversal.
- This keeps AgentPM's role clear: package and interpret portable contracts, not force every app into one framework.

Possible framing:

- `The runtime should be shared. The application boundary should stay yours.`

### Angle 3: Capability availability is part of agent behavior

Core idea:

- What a model can see and call changes what it does.
- Harness computes EffectivePhase from authored bindings, Loop access, runtime readiness, and live integrations so capability availability is explicit instead of accidental.

Why it is interesting:

- The Milestone 20A.1 measurements showed a practical version of this: smaller/faster models re-ran retrieval when the Tool remained available, and stopped when the response phase removed that capability.
- This turns "why did the agent do that?" into something inspectable.

Possible framing:

- `A phase is not just a prompt. It is a capability boundary.`

### Angle 4: Runtime state should be observable, not hidden in app code

Core idea:

- Harness emits structured events, trace files, reports, TUI state, and machine events from the same engine.
- The goal is not only execution, but explanation.

Why it is interesting:

- Reports and traces make failed, cancelled, approval-required, and limit-reached Runs diagnosable.
- The TUI makes readiness, active Runs, approvals, output, Memory, and traces visible without requiring a custom app.

Possible framing:

- `A useful agent runtime should leave a receipt.`

### Angle 5: Examples complete the runtime story

Core idea:

- A runtime becomes credible when users can start from focused templates rather than an abstract reference matrix.
- Phase 7B ends with runnable templates for the major entry points.

Why it is interesting:

- The template family keeps the on-ramp digestible:
  - minimal TUI/headless
  - SDK-hosted Node
  - SDK-hosted Python
  - embedding provider
  - provider lab
  - MCP bridge
- The team deliberately declined a single "full reference" mega-template because it would be more confusing than useful.

Possible framing:

- `A runtime is only real when the first project is easy to generate.`

## What was learned

- A single engine mattered more than multiple surface implementations.
  - The most important architectural choice was keeping Loop traversal and runtime semantics in the Rust HarnessEngine while TUI, headless, and SDK surfaces drove the same engine.

- The TUI exposed runtime bugs that headless tests did not.
  - Long-lived sessions made stale state, repeated Runs, cancellation, trace isolation, and MCP export hosting issues visible. Those are exactly the bugs a one-shot runner hides.

- Required tool choice solved more output loss than fallback logic did.
  - The initial hypothesis emphasized fallback from phase-local transcript content. Live measurement showed the shipped outcome came mostly from requiring a completion action and making `phase_completion.output` explicit.

- The cross-phase action ledger was implemented correctly and still did not govern retrieval behavior.
  - Measurement showed the ledger rendered into prompts and was legible, but smaller/faster models still repeated successful search actions when the Tool was available. That pushed the guidance toward phase capability scoping rather than more prompt prose.

- Runtime redaction has to cover artifacts and provider-bound prompts.
  - Secret redaction could not stop at reports/traces. Structured values rendered into prompts and native provider turns also needed the same treatment.

- "Available" has to mean live, not declared.
  - MCP exports in particular reinforced this: a configured surface is not ready until Harness actually starts and snapshots it.

- Manual verification needed its own tooling.
  - Cross-surface equivalence was too important to leave as ad hoc inspection. The release verification script and manual runbooks became part of the product-quality surface.

- Focused templates are better than a full-reference template.
  - The Harness feature set is broad enough that a single template would overload the user. Focused starters teach entry points better and keep setup believable.

## Tie-back to AgentPM

Phase 7B reinforces the central AgentPM direction:

- define agent-system artifacts once
- version them cleanly
- install them predictably
- execute them through explicit runtime contracts
- make them observable across hosts and languages

Earlier phases made the pieces portable. Harness makes their composition executable.

That moves AgentPM closer to being the packaging and interoperability layer for agent systems:

- artifacts remain immutable and portable
- workspace runtime config realizes them locally
- SDKs host integrations without owning orchestration
- reports and traces make execution inspectable
- templates make the path copyable

The phase is a concrete answer to a practical question:

> If an Agent package is portable, what actually runs it?

For AgentPM, the answer is now: the Harness.

## Suggested inputs for ChatGPT Projects

- Possible title ideas
  - `Portable Agents Need a Reference Runtime`
  - `AgentPM Harness: A Runtime Boundary for Packaged Agents`
  - `What Runs a Portable Agent?`
  - `From Agent Packages to Agent Execution`
  - `Why Agent Runtimes Need Receipts`

- Possible hook
  - Installing an agent package is only half the problem. The harder question is what runtime interprets its Loop, bindings, Tools, Knowledge, Memory, approvals, and external services the same way every time.

- Supporting examples or screenshots worth using
  - TUI screenshot showing active Run vs terminal Run states
  - TUI preflight/readiness panel
  - Trace panel with a selected event detail
  - a `report.json` excerpt showing terminal status, phases, actions, and trace path
  - `agentpm harness --headless --report reports/run.json --input "..."`
  - `agentpm new @zack/harness-minimal-agent my-harness-agent`
  - SDK host snippets from the Node and Python Harness templates
  - a small `agentpm.harness.json` excerpt showing model/provider/runtime config
  - a focused template list showing why there is no single full-reference template

- Suggested caution for the eventual post
  - Do not frame Harness as "AgentPM's agent framework." The stronger and more accurate framing is "AgentPM's reference runtime for portable AgentPM artifacts."
  - Do not oversell the action ledger as a behavioral fix for repeated retrieval. The measured result is more nuanced and more useful: capability scoping changed behavior where prompt-visible history did not.
  - Keep the post grounded in engineering pain: reproducibility, observability, runtime boundaries, and cross-language host integration.
