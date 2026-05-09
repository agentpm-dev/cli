# Blog Brief

## Spec

- `agentpm/specs/2026-05-03-interop-ecosystem-glue/spec.md`

## Is this blog-worthy?

Yes.

This spec is blog-worthy because it marks a real shift in what AgentPM can do in practice:

- tools are no longer only loadable through AgentPM SDKs
- installed AgentPM tools can now be:
  - run directly from the shell
  - exposed to MCP-compatible clients
  - exported into starter Skill workflows

That makes this a strong candidate for one or more posts about interoperability, packaging boundaries, and how agent tooling should scale beyond one framework or runtime.

## What shipped

This spec added three major interoperability surfaces to the AgentPM CLI:

### 1. `agentpm run`

A universal shell-facing execution surface for installed AgentPM tools.

What it does:

- runs installed tools directly from the CLI
- supports unversioned specs resolved through `agent.lock`
- supports exact versions
- supports `@latest`
- supports semver ranges
- accepts JSON via:
  - stdin
  - `--input`
  - `--input-file`
- routes execution through the same shared runner used by the rest of the system

Important implementation idea:

- AgentPM remains the source of truth for:
  - manifest contract
  - runtime declaration
  - environment defaults
  - subprocess execution behavior

### 2. `agentpm serve --mcp`

A local HTTP MCP server that exposes locked AgentPM tools to MCP-compatible clients.

What it does:

- starts on `127.0.0.1:7331` by default
- exposes all tools pinned in `agent.lock` by default
- supports narrowing exposure with:
  - `--tool`
  - `--tools`
- derives MCP tool metadata from installed `agent.json`
- routes tool calls through the same shared AgentPM runner used by `agentpm run`

Important implementation idea:

- MCP is treated as an adapter over AgentPM’s canonical packaging/execution model
- it does not introduce a second runtime stack
- tool results are returned through MCP without inventing a new AgentPM-specific result envelope

Manual verification was completed with:

- raw HTTP `curl` calls
- Claude Code
- Codex

### 3. `agentpm export --skill`

A starter Skill scaffold generator for installed tools.

What it does:

- resolves an installed tool
- generates:
  - `SKILL.md`
  - `references/tool-contract.md`
  - `references/examples.md`
  - `scripts/run.sh`
- keeps the main skill concise
- pushes deeper contract/runtime/environment details into reference files
- delegates execution back to `agentpm run`

Important implementation idea:

- this is intentionally a scaffold, not a polished final workflow artifact
- Skills are not yet treated as first-class AgentPM artifacts
- the generated skill is meant to be tailored by a human or team after export

## Why this matters

The big strategic point is that AgentPM now has a clearer answer to:

“How does a packaged tool become usable outside of an AgentPM-native SDK integration?”

Before this work, the answer leaned more heavily on:

- direct SDK loading
- app-level integration

After this work, the answer is broader:

- use `agentpm run` for shell and script execution
- use `agentpm serve --mcp` for MCP-compatible clients
- use `agentpm export --skill` when the target ecosystem wants workflow guidance plus a stable execution path

This is exactly the kind of “ecosystem glue” that supports AgentPM’s broader positioning:

- package once
- version cleanly
- install predictably
- expose through ecosystem-native surfaces when needed

## Strongest concepts and angles

These are the strongest reusable ideas from the spec. ChatGPT Projects should feel free to choose one or more rather than forcing them into a single post.

### Angle 1: Why agent systems need a universal execution boundary

Core idea:

- `agentpm run` is not just a convenience command
- it is the execution boundary that lets packaged tools move cleanly into shells, scripts, Skills, and MCP clients

Why it is interesting:

- it connects packaging to actual day-to-day use
- it gives AgentPM a “runtime entry point” analogous to how package managers and CLIs expose installed software in traditional engineering systems

Possible framing:

- “A package manager for agents is not enough if the installed artifact has no universal way to run.”

### Angle 2: MCP should be an adapter, not a second runtime

Core idea:

- MCP integration should sit on top of the same packaging/install/runtime truth
- it should not fork execution semantics from the package manager

Why it is interesting:

- many integrations become fragile when the “adapter path” and the “native path” diverge
- AgentPM’s MCP implementation is notable because it routes through the same runner as `agentpm run`

Possible framing:

- “If an MCP server exposes a tool, it should expose the same packaged artifact you would run locally.”

### Angle 3: Skills as manuals, `agentpm run` as execution

Core idea:

- a Skill should carry workflow guidance
- AgentPM should carry contract and execution

Why it is interesting:

- it addresses context scaling problems in agent systems
- it separates:
  - what the tool is
  - when to use it
  - how it actually executes

This is the strongest connection to the example work added around `slack-post-message`.

Important strategic nuance:

- this is not only a UX preference
- it is a scaling pattern for agent systems

The emerging pattern is:

- give an agent one stable execution surface such as `agentpm run`
- let Skills carry the narrower, tool-specific usage guidance
- avoid forcing the agent to carry dozens of full tool contracts in active context at once

In other words:

- Skills act like manuals
- `agentpm run` acts like the universal hand that executes packaged tools

Why that matters:

- lower context overhead
- less tool overload
- clearer separation between:
  - contract metadata
  - workflow guidance
  - execution behavior

This is especially useful as the number of available tools grows. A system with 5 tools can often get away with directly exposing every contract everywhere. A system with 50 or more tools benefits much more from:

- on-demand guidance through Skills
- one canonical execution boundary
- keeping the full tool contract loaded only when needed

This is a place where AgentPM’s model becomes stronger than a naive “register every tool directly with the agent” pattern.

Possible framing:

- “Skills should explain usage. Package managers should own execution.”

### Angle 4: AgentPM tools as ecosystem-native building blocks

Core idea:

- the same packaged tool can now show up in:
  - shell workflows
  - SDK-based apps
  - MCP tool lists
  - Skill-based workflows

Why it is interesting:

- it demonstrates portability in a way that is more concrete than abstract architecture claims
- it helps show that AgentPM is not only about publishing packages, but about making those packages reusable across contexts

Possible framing:

- “A useful agent tool should not need to be rewritten for every client surface.”

### Angle 5: The missing layer between packaging and orchestration

Core idea:

- frameworks and agent runtimes get a lot of attention
- but a lot of practical pain sits one layer lower:
  - execution
  - version resolution
  - runtime normalization
  - environment handling
  - interoperability surfaces

Why it is interesting:

- it fits AgentPM’s broader editorial lane
- it starts from concrete engineering pain rather than hype

Possible framing:

- “Before orchestration gets interesting, execution has to become boring.”

## Audience

Primary audience:

- technically strong software engineers
- platform-minded AI/agent builders
- people already experimenting with tools, frameworks, MCP, or agent workflows
- engineers who understand:
  - package managers
  - lockfiles
  - SDKs
  - runtime boundaries
  - CLI tooling
  - version resolution

Good reader questions this spec helps answer:

- How should installed agent tools be executed outside of one SDK?
- What is the relationship between packaging and MCP?
- How should Skills relate to packaged tools?
- What does interoperability actually look like for agent tools in practice?

## Specific facts ChatGPT Projects should know

These details are useful if the strategist has no repo access.

- CLI surfaces added:
  - `agentpm run`
  - `agentpm serve --mcp`
  - `agentpm export --skill`

- `agentpm run` supports:
  - unversioned specs via `agent.lock`
  - exact versions
  - `latest`
  - semver ranges
  - JSON via stdin / `--input` / `--input-file`

- `agentpm serve --mcp`:
  - is HTTP-only for now
  - defaults to `127.0.0.1:7331`
  - exposes all locked tools by default
  - supports filtering with `--tool` / `--tools`
  - was manually verified with Claude Code and Codex

- `agentpm export --skill`:
  - generates starter scaffolds only
  - delegates execution to `agentpm run`
  - now produces better starter examples and clearer trigger descriptions than the earliest draft

- A dedicated example was added in `agentpm-examples` around:
  - `@zack/slack-post-message`
  - showing the pattern:
    - install tool
    - export skill
    - keep execution delegated to `agentpm run`

- There is a broader strategic direction behind that example:
  - do not think of Skills as replacing packaged tools
  - think of Skills as the layer that tells an agent when and how to use a packaged tool
  - think of `agentpm run` as the universal execution boundary beneath those Skills
  - this is intended to scale better than exposing a large flat set of tool contracts directly to an agent at all times

## Potential titles

These are starting points, not prescriptions.

- Why Agent Tools Need a Universal Execution Surface
- MCP Should Be an Adapter, Not a Second Runtime
- Skills Are Manuals. Package Managers Own Execution.
- Package Once, Expose Everywhere: A Better Pattern for Agent Tools
- The Missing Layer Between Packaging and Orchestration in Agent Systems
- How to Make Agent Tools Portable Across Shells, MCP, and Skills

## Recommended directions for ChatGPT Projects

I would encourage Projects to consider at least three post directions:

1. A practical interoperability post
- centered on:
  - `run`
  - `serve --mcp`
  - `export --skill`
- most concrete and feature-legible

2. A conceptual architecture post
- centered on:
  - one canonical runner
  - adapters over packaging truth
  - avoiding duplicated runtime stacks

3. A skills-pattern post
- centered on:
  - why Skills should carry workflow guidance
  - why execution should stay delegated to AgentPM
  - why this helps agents scale better than carrying every tool contract directly in active context

## Tie-back to AgentPM

This spec supports AgentPM’s positioning very directly:

- AgentPM is not only about publishing tool artifacts
- it is about making those artifacts:
  - runnable
  - portable
  - versioned
  - reusable
  - adaptable across ecosystems

The strongest line of argument is not:

- “look at these three new CLI commands”

It is:

- “AgentPM now makes packaged agent tools usable across multiple real integration surfaces without changing the underlying contract.”
