# Feature
Phase 7A: Loops & Agent Bindings

## Problem / Goal
AgentPM already packages reusable agent capabilities, procedures, knowledge, memory contracts, and behavioral profiles, but the orchestration pattern that determines how those pieces participate in an agent run still commonly lives as framework-specific application code.

That orchestration layer is often the difference between a tool-calling assistant and an agentic system. Common patterns include planning, acting, reviewing, retrying, pausing for approval, handing control back to a host, and terminating when work is complete. When that structure is embedded directly in app code it is difficult to version, inspect, compare, reuse, or adapt across runtimes.

Phase 7A introduces **Loop** as AgentPM's final first-class package kind and adds **Agent bindings** that compose existing Agent dependencies into a Loop.

A Loop is a portable declarative orchestration contract. It describes the phases and control-flow semantics of an agent run without implementing an execution engine.

Agent bindings are a portable composition contract. They describe which concrete Tool, Skill, Knowledge, Memory Blueprint, and Instruction Profile dependencies participate globally, within named Loop phases, or on named MCP surfaces, plus an optional consumer-owned context-file convention.

The phase must:

- Add `loop` as a publishable, installable, discoverable AgentPM package kind.
- Define a strict declarative Loop manifest contract for phases, outcomes, transitions, terminal targets, access intent, limits, approval checkpoints, and error policy.
- Keep Loop execution semantics generic enough that new orchestration patterns do not require new AgentPM archetype enums or CLI releases.
- Allow each Agent to depend on at most one Loop through top-level `loop`.
- Allow Agent manifests to define optional `bindings` for global artifacts, phase-specific artifacts, Memory Blueprint spaces/operations, named MCP Tool surfaces, and consumer-owned context files.
- Keep binding package identities versionless so version selection remains owned by the Agent's top-level dependency declarations and lockfile resolution.
- Add local Agent semantic validation ensuring packages referenced by bindings are declared in the corresponding top-level Agent dependency collection.
- Deliberately avoid inspecting referenced Loop phases, Memory spaces, Memory operations, or cross-artifact runtime-policy conflicts during Agent lint.
- Resolve, install, lock, publish, search, display, and load Loop package metadata using existing first-class package patterns.
- Expose typed Loop metadata and typed Agent binding metadata through Node and Python SDKs.
- Leave runtime interpretation, model/provider selection, prompt assembly, live Memory storage, MCP process management, approval UX, and execution to Phase 7B or external consumers.

### Core definitions

> A Loop is a versioned, portable orchestration contract describing the phases, valid phase outcomes, control-flow transitions, limits, checkpoints, access intent, and failure behavior of an agent run. It is declarative metadata and does not execute the loop itself.

> Agent bindings are versionless references from an Agent's already-declared package dependencies into global, phase, Memory, MCP, and consumer-context composition surfaces. Bindings describe intended composition but do not execute or enforce it.

### Architectural boundary

The intended long-term layering is:

- **Tool** = capability
- **Skill** = reusable task know-how / procedure
- **Knowledge** = prepared retrieval context
- **Memory Blueprint** = durable memory structure, governance, retrieval, and lifecycle contract
- **Instruction Profile** = stable role, behavioral posture, communication guidance, boundaries, and constraints
- **Loop** = orchestration pattern
- **Agent** = portable composition of the above, including bindings
- **Template** = project/application bootstrap and implementation scaffolding
- **Harness/runtime** = execution implementation that interprets the Agent, Loop, bindings, runtime configuration, and live integrations

Phase 7A must define enough portable metadata that a runtime can construct a credible agent execution harness without inventing the intended orchestration or package composition. It must not implement that harness.

## Loop manifest contract

A Loop package uses `kind: "loop"` and a required top-level `loop` object.

Loops use existing common package metadata:

- `kind`
- `name`
- `version`
- `description`
- optional `readme`
- optional `license`

Loops do not add `display_name`.

README content is package documentation only. Runtimes and SDK loaders must not implicitly treat README content as Loop instructions.

### Example Loop manifest

```json
{
  "$schema": "https://raw.githubusercontent.com/agentpm-dev/cli/refs/heads/main/schemas/agentpm.manifest.schema.json",
  "kind": "loop",
  "name": "incident-response-loop",
  "version": "1.0.0",
  "description": "A bounded triage, investigation, review, and response loop with approval and escalation paths.",
  "loop": {
    "archetype": "investigate_review_respond",
    "entry_phase": "triage",
    "limits": {
      "max_steps": 16
    },
    "phases": [
      {
        "id": "triage",
        "objective": "Assess the incident and determine whether investigation should proceed.",
        "access": {
          "tools": false,
          "knowledge": true,
          "memory": {
            "read": true,
            "write": false
          }
        },
        "outcomes": [
          {
            "id": "proceed",
            "description": "The incident has enough information to begin investigation."
          },
          {
            "id": "cannot-proceed",
            "description": "The incident cannot be investigated safely or meaningfully."
          }
        ]
      },
      {
        "id": "investigate",
        "objective": "Gather evidence, test hypotheses, and update the working understanding of the incident.",
        "access": {
          "tools": true,
          "knowledge": true,
          "memory": {
            "read": true,
            "write": true
          }
        }
      },
      {
        "id": "review",
        "objective": "Evaluate the evidence and decide whether more investigation, escalation, or response is appropriate.",
        "access": {
          "tools": false,
          "knowledge": true,
          "memory": {
            "read": true,
            "write": false
          }
        },
        "outcomes": [
          {
            "id": "needs-more-evidence",
            "description": "Important questions remain and another investigation cycle is required."
          },
          {
            "id": "ready",
            "description": "The evidence is sufficient to prepare the incident response."
          },
          {
            "id": "escalate",
            "description": "The incident requires an external actor or system to take over."
          }
        ]
      },
      {
        "id": "respond",
        "objective": "Produce and deliver the final incident response using the reviewed evidence.",
        "access": {
          "tools": true,
          "knowledge": false,
          "memory": {
            "read": true,
            "write": true
          }
        }
      }
    ],
    "transitions": [
      { "from": "triage", "on": "proceed", "to": "investigate" },
      { "from": "triage", "on": "cannot-proceed", "to": "$abort" },
      { "from": "investigate", "on": "complete", "to": "review" },
      { "from": "review", "on": "needs-more-evidence", "to": "investigate" },
      { "from": "review", "on": "ready", "to": "respond" },
      { "from": "review", "on": "escalate", "to": "$handoff" },
      { "from": "respond", "on": "complete", "to": "$end" }
    ],
    "checkpoints": [
      {
        "id": "approve-response",
        "type": "approval",
        "before_phase": "respond",
        "on_reject": "review"
      }
    ],
    "error_policy": {
      "tool_failure": {
        "action": "retry",
        "max_retries": 2,
        "on_exhausted": "fail_phase"
      },
      "phase_failure": {
        "action": "abort"
      }
    }
  },
  "readme": "README.md",
  "license": {
    "spdx": "Apache-2.0"
  }
}
```

### Stable Loop identifiers

Phase IDs, explicit outcome IDs, checkpoint IDs, and MCP binding IDs use the same stable lowercase kebab-case style used by Profile constraint IDs:

```text
^[a-z](?:[a-z0-9]|-(?=[a-z0-9])){0,63}$
```

Examples:

- `plan`
- `source-review`
- `needs-more-evidence`
- `approve-finalization`
- `research-tools`

Memory space and operation identifiers continue to use the Memory Blueprint key contract (`^[a-z][a-z0-9_]*$`) and must not be converted to Loop-style kebab-case.

### `archetype`

`loop.archetype` is optional descriptive metadata.

- It is a non-empty open-ended string.
- It may be used for documentation, discovery, comparison, or registry display.
- It does not control runtime behavior.
- AgentPM must not introduce an execution switch statement over a closed list of archetypes.
- A publisher may use a previously unseen archetype without requiring an AgentPM release as long as the actual Loop structure is valid.

Examples include:

- `single_agent_iterative`
- `planner_executor_review`
- `supervisor_worker`
- `human_in_the_loop`
- `debate_with_judge`

### `entry_phase`

`loop.entry_phase` is required and must reference a declared phase ID.

The phases array is not implicitly ordered for execution. Control flow is determined by `entry_phase`, phase outcomes, and `transitions`.

### Phases

`loop.phases` is required and must contain at least one phase.

Each phase requires:

- `id`: stable unique identifier.
- `objective`: non-empty human-readable description of what the phase is intended to accomplish.

Each phase may additionally define:

- `access`
- `outcomes`

The runtime must not infer phase semantics from the phase ID itself. `objective` carries the portable human/model-facing meaning of the phase.

#### Phase outcomes

`outcomes` is optional.

When omitted, the phase has exactly one implicit valid outcome:

```text
complete
```

When `outcomes` is present:

- it must contain at least one item;
- every item is an object with required `id` and `description`;
- IDs must be unique within the phase;
- descriptions must be non-empty;
- there is no implicit `complete` outcome unless the author explicitly declares an outcome with `id: "complete"`.

Example:

```json
"outcomes": [
  {
    "id": "revise",
    "description": "Additional execution is required."
  },
  {
    "id": "ready",
    "description": "The result is ready for finalization."
  }
]
```

Outcomes are named semantic results, not executable conditions. The Loop must not add expressions such as confidence thresholds, JSONPath, callbacks, scripts, or arbitrary boolean logic to choose an outcome.

A runtime decides how to obtain one of the declared outcomes. For example, a model-based runtime may use structured output, while another runtime may map host-managed logic to the same outcome IDs.

### Phase access intent

A phase may declare `access`:

```json
"access": {
  "tools": false,
  "knowledge": true,
  "memory": {
    "read": true,
    "write": false
  }
}
```

Supported fields:

- `tools`: optional boolean.
- `knowledge`: optional boolean.
- `memory`: optional object with optional `read` and `write` booleans; if present, it must contain at least one of them.

Semantics:

- `true` expresses that the Loop permits the activity in this phase.
- `false` expresses that the Loop author intends the activity to be prohibited in this phase.
- omission means the Loop expresses no opinion about that activity.
- `knowledge: true` means bound Knowledge may be used; it does not instruct the runtime to retrieve at phase start.
- `memory.write: true` means Memory writes may be performed; it does not instruct the runtime to manufacture a write at phase end.
- access metadata does not name concrete packages, spaces, Tools, providers, or storage adapters.
- Skills and Profiles are intentionally not part of `access`; they are behavioral/procedural inputs rather than external action/state surfaces.

Phase 7A validates and exposes this metadata but does not enforce it. AgentPM's future canonical harness is expected to treat explicit Loop access restrictions as constraints over available Agent bindings. External consumers remain free to adapt the portable metadata differently.

Agent lint, install, publish, and SDK loaders must not compare Loop access intent against Agent bindings.

### Transitions

`loop.transitions` is required and must contain at least one transition.

Each transition has exactly:

- `from`: source phase ID.
- `on`: valid outcome ID for the source phase.
- `to`: another phase ID or a standardized terminal target.

Example:

```json
{
  "from": "review",
  "on": "revise",
  "to": "execute"
}
```

Transitions deliberately do not support:

- condition expressions
- callbacks
- scripts
- probabilities
- state transformations
- provider/model instructions
- Tool calls
- embedded code

Semantic validation must ensure:

- every `from` phase exists;
- every non-terminal `to` phase exists;
- `on` matches a valid outcome of the source phase;
- `complete` is valid for a source phase with omitted outcomes;
- each valid phase/outcome pair has exactly one transition;
- duplicate or ambiguous transitions for the same phase/outcome pair fail;
- every phase is reachable from `entry_phase`;
- at least one terminal target is reachable from `entry_phase`.

These rules make the graph deterministic without turning the Loop into a general workflow language.

### Terminal targets

Phase 7A defines exactly three standardized terminal targets:

- `$end`: successful completion of the Loop.
- `$abort`: intentional unsuccessful termination; execution should not continue.
- `$handoff`: yield control to an external actor or system.

Terminal targets are closed AgentPM vocabulary because consumers need portable meanings for whole-run termination.

Custom extensibility belongs in phase outcome IDs, not custom `$...` terminal names.

`$handoff` does not identify or invoke another Agent and must not introduce Agent-to-Agent dependencies.

Do not add `$pause`, `$retry`, `$approval`, `$error`, `$fail`, or arbitrary custom terminal names in Phase 7A. Approval is modeled as a checkpoint, retry is modeled by error policy, and unexpected failures are runtime/error-policy concerns.

### Limits

`loop.limits` is optional.

Initial contract:

```json
"limits": {
  "max_steps": 16
}
```

`max_steps` must be a positive integer.

A **step** means one execution of one Loop phase. Tool calls, model calls, user/assistant message turns, and complete plan/execute/review cycles are not independently counted as Loop steps by this contract.

Phase 7A records the limit but does not count or enforce steps.

Do not use the ambiguous term `max_turns` in the Loop contract.

### Approval checkpoints

`loop.checkpoints` is optional.

Phase 7A supports one checkpoint type:

```text
approval
```

Shape:

```json
{
  "id": "approve-response",
  "type": "approval",
  "before_phase": "respond",
  "on_reject": "review"
}
```

Fields:

- `id`: required unique stable ID.
- `type`: required and currently exactly `approval`.
- `before_phase`: required declared phase ID.
- `on_reject`: required transition target, either a declared phase ID or one of `$end`, `$abort`, `$handoff`.

Semantics:

- The checkpoint indicates that a compatible runtime should suspend before entering `before_phase` and request external approval.
- Approval continues into `before_phase`.
- Rejection transfers control to `on_reject`.
- The Loop does not declare who approves, how approval is surfaced, which UI or transport is used, or how long suspension lasts.
- Phase 7A does not execute, suspend, resume, or request approvals.
- Only one approval checkpoint may target the same `before_phase` in Phase 7A to avoid undefined ordering among multiple approval gates.

### Error policy

`loop.error_policy` is optional.

Initial supported categories are:

- `tool_failure`
- `phase_failure`

#### Tool failure

Tool failure policy supports these actions:

- `retry`
- `fail_phase`
- `abort`
- `handoff`

When `action` is `retry`:

- `max_retries` is required and must be a positive integer.
- `on_exhausted` is required and must be `fail_phase`, `abort`, or `handoff`.

For non-`retry` actions, `max_retries` and `on_exhausted` must be absent.

`fail_phase` delegates to the declared `phase_failure` policy. If a Tool failure can produce `fail_phase`, `phase_failure` must be present.

#### Phase failure

Phase failure supports:

- `abort`
- `handoff`

The Loop does not define provider-specific exception classes, retry backoff, jitter, transport errors, model errors, or arbitrary failure expressions.

Phase 7A records error policy only; it does not execute retries or failures.

## Agent dependency and binding contract

### Agent Loop dependency

An Agent may declare at most one Loop through top-level `loop`:

```json
{
  "kind": "agent",
  "name": "incident-response-agent",
  "version": "1.0.0",
  "description": "Incident response agent.",
  "tools": [],
  "loop": "@acme/incident-response-loop@1.0.0"
}
```

The field uses the existing package-reference shape and owns version/range resolution just like other Agent dependency declarations.

The top-level `loop` property is overloaded by kind:

- `kind: "agent"` uses a Loop package reference.
- `kind: "loop"` uses the structured Loop metadata object.

Other package kinds must reject top-level `loop`.

Loop packages are dependency leaves and cannot declare Tools, Skills, Knowledge, Memory, Profiles, Agents, Templates, or other Loops.

Existing Agents remain valid without a Loop.

The existing Agent invariant requiring top-level `tools` remains unchanged in Phase 7A; tool-less Agents may continue to declare `tools: []`.

### Binding package identities

Top-level Agent dependency declarations continue to use the existing version-capable `packageRef` contract.

Bindings use a new versionless package identity shape:

```text
@namespace/package
```

Binding references must not include versions or ranges.

This prevents drift between:

```json
"tools": ["@acme/search-logs@1.2.0"]
```

and:

```json
"bindings": {
  "phases": {
    "investigate": {
      "tools": ["@acme/search-logs"]
    }
  }
}
```

Version selection belongs to dependency resolution and the lockfile. Bindings identify already-declared dependencies by package identity.

### Agent `bindings`

`bindings` is optional and Agent-only.

If present, it may contain:

- `global`
- `phases`
- `mcp`
- `consumer_context`

No binding surface executes anything in Phase 7A.

### Global bindings

`bindings.global` may contain any of:

- `tools`
- `skills`
- `knowledge`
- `memory`
- `profiles`

Package arrays use versionless package identities.

Example:

```json
"global": {
  "tools": ["@acme/get-incident-context"],
  "skills": ["@acme/incident-investigation"],
  "knowledge": ["@acme/incident-runbooks"],
  "memory": [
    {
      "package": "@acme/incident-memory",
      "spaces": ["incident_state"]
    }
  ],
  "profiles": ["@acme/incident-responder"]
}
```

### Phase bindings

`bindings.phases` is an object keyed by authored phase IDs.

Each phase binding supports the same package kinds as `global`:

- `tools`
- `skills`
- `knowledge`
- `memory`
- `profiles`

Example:

```json
"phases": {
  "review": {
    "skills": ["@acme/incident-investigation"],
    "knowledge": ["@acme/incident-runbooks"],
    "memory": [
      {
        "package": "@acme/incident-memory",
        "spaces": ["incident_state", "evidence", "review_history"],
        "operations": ["compact_evidence"]
      }
    ],
    "profiles": ["@acme/incident-reviewer"]
  }
}
```

Rules:

- `bindings.phases` requires top-level Agent `loop` to be present.
- Phase keys use the stable Loop identifier syntax.
- Phase bindings are not required for every Loop phase.
- An unbound phase has no phase-specific Agent bindings; the runtime may still have global bindings and runtime-specific defaults.
- Agent semantic lint does not resolve the referenced Loop and therefore does not verify that a phase key exists in the Loop.
- Install and publish do not reject unknown phase names merely by inspecting the referenced Loop.
- SDK loaders expose authored binding metadata without resolving effective phase behavior.

### Additive binding semantics

Global and phase bindings are additive by intended contract:

```text
effective authored association for a phase = global bindings ∪ phase bindings
```

The same package identity appearing globally and in a phase does not create a second dependency.

For Memory, global and phase entries for the same package combine by the set union of referenced spaces and operations.

Phase 7A does not add generic `inherit`, `replace`, `exclude`, `override`, or precedence machinery.

This additive rule describes the authored composition. A runtime still decides how to realize Profiles, Skills, Knowledge retrieval, Memory access, and other behavior.

### Loop access versus Agent bindings

Loop access metadata and Agent bindings are independently portable metadata.

Example:

- Loop phase declares `access.tools: false`.
- Agent binds `@acme/search-logs` to that phase.

The Agent remains valid. AgentPM lint, publish, install, resolver, and SDK loaders must not warn, reject, or rewrite the binding because of the Loop access declaration.

Phase 7A does not evaluate runtime policy conflicts.

The future AgentPM harness is expected to treat Loop access restrictions as constraints over Agent bindings and may explain that a bound capability is inactive under the Loop. External consumers may adapt the metadata differently.

Do not add `override_loop_policy` or equivalent escape hatches to the manifest.

### Memory bindings

Memory bindings reference an Agent's already-declared Memory Blueprint package and selected public identifiers from that Blueprint.

Shape:

```json
{
  "package": "@acme/incident-memory",
  "spaces": [
    "incident_state",
    "evidence"
  ],
  "operations": [
    "compact_evidence"
  ]
}
```

Rules:

- `package` is required and uses a versionless package identity.
- At least one of `spaces` or `operations` is required.
- If present, each array must be non-empty and unique.
- `spaces` uses Memory Blueprint key syntax.
- `operations` uses Memory Blueprint key syntax.
- `spaces` identifies Memory spaces associated with the binding scope.
- `operations` identifies Memory lifecycle operations associated with the binding scope.
- The Agent does not redefine an operation's type, trigger, inputs, outputs, source handling, or provenance behavior.
- The Memory Blueprint remains authoritative for operation semantics and triggers, including `external`, `record_count`, `capacity`, and `interval`.
- `operations` may therefore bind operations with any valid Blueprint trigger; exact trigger evaluation is a runtime concern.
- An operation-only Memory binding is valid.
- Binding a Memory operation does not implicitly grant unrestricted access to every space touched by that operation.

Agent semantic lint verifies only that the `package` identity appears in the Agent's top-level `memory` dependency array. It does not resolve the Blueprint to verify that named spaces or operations exist.

Record-type-level narrowing is not part of Phase 7A.

### Named MCP bindings

`bindings.mcp` is an optional array of named MCP Tool surfaces.

Shape:

```json
"mcp": [
  {
    "id": "investigation-tools",
    "tools": [
      "@acme/get-incident-context",
      "@acme/search-logs",
      "@acme/fetch-service-metrics"
    ]
  },
  {
    "id": "response-tools",
    "tools": [
      "@acme/post-incident-update"
    ]
  }
]
```

Each MCP binding requires:

- unique `id` using the stable kebab-case identifier contract;
- non-empty unique `tools` array using versionless package identities.

Semantics:

> An MCP binding declares that the listed Agent Tool dependencies are intended to be exposed together as a named MCP surface.

The Agent does not define:

- host
- port
- transport
- process model
- server lifecycle
- endpoint URL
- authentication
- whether multiple named surfaces share one physical MCP server

AgentPM's Phase 7B harness may use `agentpm serve --mcp` to realize these bindings and report what it is doing transparently. Another runtime may expose them through a different MCP implementation or ignore the hint.

MCP bindings are orthogonal to global/phase Tool bindings:

- top-level `tools` declares package dependencies;
- `bindings.mcp` declares intended MCP exposure;
- global/phase Tool bindings declare orchestration association.

None of these implicitly creates either of the others.

Agent semantic lint verifies that every MCP Tool identity appears in top-level `tools`.

### Consumer context

`bindings.consumer_context` is an optional Agent-global extension point for consumer-owned context.

Shape:

```json
"consumer_context": {
  "file": "INCIDENT_AGENT.md"
}
```

Rules:

- `file` is required when `consumer_context` is present.
- It reuses the existing safe relative-path contract.
- The path is interpreted relative to the runtime/workspace root, not the installed Agent package root.
- The file is not part of the Agent package and must not be added to the published archive merely because the manifest names it.
- The file may be absent; absence does not invalidate the Agent.
- Phase 7A does not read, parse, merge, compile, or enforce the file.
- The Agent author chooses the filename; AgentPM does not standardize `AGENTPM.md` or any other magic filename.
- Consumer context is Agent-global only in Phase 7A. Phase-specific consumer context is deferred.

The object form is intentional so future compatible fields can be added without replacing a bare string contract.

### Agent binding semantic lint

Agent semantic validation in Phase 7A is intentionally local and non-resolving.

For each binding reference:

- bound Tool identities must exist in top-level `tools`;
- bound Skill identities must exist in top-level `skills`;
- bound Knowledge identities must exist in top-level `knowledge`;
- bound Memory package identities must exist in top-level `memory`;
- bound Profile identities must exist in top-level `profiles`;
- MCP Tool identities must exist in top-level `tools`.

The comparison ignores version/range syntax in the dependency declarations and compares canonical package identity.

Semantic lint must also reject:

- duplicate package identities within one binding array after canonical identity normalization;
- duplicate Memory package entries within the same binding scope;
- duplicate MCP IDs;
- invalid/unsafe consumer-context paths if not already rejected by schema;
- `bindings.phases` when top-level `loop` is absent.

Agent semantic lint must not:

- resolve the Loop package;
- validate phase names against the Loop;
- resolve a Memory Blueprint;
- validate bound Memory space names;
- validate bound Memory operation names;
- inspect Tool/Skill/Knowledge/Profile contents;
- compare Loop access declarations with bindings;
- determine whether Profiles conflict;
- determine whether MCP-bound Tools are phase-usable;
- compile prompts or effective runtime capabilities.

## Template dependency model

Template dependency metadata adds optional singular `loop`:

```json
{
  "template": {
    "dependencies": {
      "tools": [],
      "agents": [],
      "skills": [],
      "knowledge": [],
      "memory": [],
      "profiles": [],
      "loop": "@acme/incident-response-loop@1.0.0"
    }
  }
}
```

Rules:

- `template.dependencies.loop` is optional.
- It uses the existing package-reference shape.
- A direct Template Loop is resolved during `agentpm new`, installed, and written as an exact-version reference into the synthesized root Agent's top-level `loop`.
- Generated local Agent manifests may independently declare their own Loop dependencies.
- The direct Template Loop must not be copied into every generated local Agent.
- Template `loop` is dependency metadata only; Templates do not define Agent bindings themselves.

## CLI behavior

### `agentpm init --kind loop`

Create:

- `agent.json`
- `README.md`

The generated manifest must:

- use `kind: "loop"`;
- use the requested package name and description;
- set `readme: "README.md"`;
- include a small valid Loop starter demonstrating an entry phase, an iterative phase, an explicit decision outcome, and terminal transition;
- contain no package dependencies;
- contain no build metadata, generated files, scripts, runtime configuration, provider/model configuration, or Agent bindings.

Do not add `agentpm loop build`, `agentpm loop inspect`, `agentpm loop run`, or equivalent commands in Phase 7A.

`--mode` remains Knowledge-only and must be rejected for Loops when a non-default mode is supplied.

### `agentpm lint`

Loop semantic lint must validate the graph and cross-field rules defined above after schema validation and typed Loop parsing succeed.

At minimum, reject:

- whitespace-only required text;
- duplicate phase IDs;
- duplicate explicit outcome IDs within a phase;
- missing/unknown entry phase;
- unknown transition source or destination phases;
- invalid transition outcome for the source phase;
- missing transition for a valid phase/outcome pair;
- multiple transitions for the same phase/outcome pair;
- unreachable phases;
- graphs with no reachable terminal target;
- duplicate checkpoint IDs;
- unknown checkpoint phase or rejection target;
- multiple approval checkpoints targeting the same phase;
- invalid error-policy cross-field combinations;
- forbidden package dependencies on Loop packages.

Agent semantic lint must implement only the local binding-to-dependency rules described above.

Do not add subjective warnings about whether a Loop is a good orchestration design, has too many/few phases, should use a different archetype, or should allow different capabilities.

### `agentpm publish`

Loops use the normal immutable package publishing pipeline.

Publishing must:

- accept `kind: "loop"`;
- validate the Loop manifest and semantic graph contract;
- package `agent.json` plus referenced common README/license files according to existing behavior;
- reject Loop-owned package dependencies;
- require no build command or freshness metadata;
- create no Loop-specific generated metadata or database columns.

Agent publishing must accept top-level `loop` and `bindings` once they pass local schema/semantic validation. Publish must not resolve Loop phase names, Memory space names, Memory operation names, or policy conflicts solely to validate bindings.

### `agentpm install`

Support direct Loop installation and Agent Loop dependency installation.

Installed Loops live under:

```text
.agentpm/loops/<namespace>/<name>/<version>
```

Package keys use:

```text
loop:@namespace/name@version
```

When a Loop is installed directly in a local Agent project:

- update the Agent's singular top-level `loop` reference using the repository's existing dependency range behavior where applicable;
- installing a different Loop package replaces the prior singular Loop dependency rather than creating a list;
- do not create or modify Agent bindings automatically.

Loop installation is non-interactive and does not execute the Loop.

### `agentpm new`

When a Template declares a direct Loop dependency:

- include it in resolution;
- install it under `.agentpm/loops`;
- write the exact resolved Loop reference to the synthesized root Agent's top-level `loop`;
- preserve Loop dependencies declared by generated local Agent manifests;
- do not add bindings automatically;
- do not prompt for Loop-specific values.

## Lockfile behavior

Loops become a first-class singular Agent relationship in lockfile v3.

Add an optional `loop` package key to relevant Agent root models, for example:

```json
{
  "loop": "loop:@acme/incident-response-loop@1.0.0"
}
```

Rules:

- keep lockfile version 3 unless implementation reveals a genuinely incompatible serialization requirement;
- older v3 locks that omit `loop` remain readable;
- no legacy `reserved.loop` migration is required unless the current codebase already contains such a field;
- local and registry Agent roots may reference zero or one Loop;
- reachability, pruning, replacement, deduplication, refresh, frozen mode, and transitive installed-Agent traversal must include the singular Loop relationship;
- Loop packages are leaves;
- a standalone direct Loop install does not need an Agent-style root solely because its package kind is `loop`;
- Agent `bindings` remain authored manifest metadata and are not duplicated into the lockfile.

Frozen installs must fail clearly if an Agent's required Loop relationship is absent, incompatible, wrong-kind, or cannot be satisfied from the lockfile.

## Registry API and database behavior

Add `loop` to every API, DTO, serializer, resolver, route, validation, authorization, install, search, statistics, and package-kind allowlist that supports first-class package kinds.

Backend behavior:

- persist the complete Loop manifest in existing manifest JSON storage;
- add no Loop-specific generated metadata columns;
- recognize Agent top-level `loop` as a singular package dependency;
- require an Agent Loop dependency to resolve to stored kind `loop`;
- treat Loop packages as leaves;
- recognize Template `dependencies.loop` as an optional direct Loop dependency;
- require Template Loop references to resolve to stored kind `loop`;
- include Agent Loop dependencies in normal Agent install-graph expansion;
- do not expand Template Loop dependencies during generic Template install; they are processed by `agentpm new` according to existing Template semantics;
- do not inspect or execute Agent bindings on the backend;
- do not perform cross-package binding/phase/Memory semantic validation in the registry.

### Database migration

Create a migration parallel to recent Profile/Memory package-kind migrations.

The migration must:

- drop `tool_search_index` before `trending_tools` if required by the current dependency order;
- update `tools_kind_check` to allow `loop`;
- update `on_install_completed()` so Loop installs contribute to aggregate and per-package download statistics;
- recreate `trending_tools` with `loop` included as its own package-kind partition;
- recreate trending/search indexes and `tool_search_index` according to existing migration patterns;
- preserve existing kinds and rows.

The effective package-kind allowlist becomes:

```sql
('tool', 'agent', 'template', 'skill', 'knowledge', 'memory', 'profile', 'loop')
```

Nested Loop fields and Agent bindings are not added to full-text search in Phase 7A. Existing package name, namespace, and description indexing is sufficient.

## Registry web behavior

Use **Loop** as both the technical and user-facing package label.

Add Loop support to:

- Explore filters and cards;
- global search;
- trending;
- namespace listings;
- package-kind badges;
- public/private package detail fetches;
- canonical routes and links;
- Agent dependency presentation;
- Template dependency presentation;
- landing/discovery surfaces where all first-class package kinds are enumerated.

Canonical Loop detail URLs use:

```text
/loops/<package-id>/v<version>/overview
```

Loop detail pages should use established package layout patterns and include at minimum:

- Overview
- README
- Security

Loop Overview should render authored metadata such as:

- archetype when present;
- entry phase;
- phases and objectives;
- explicit/implicit outcomes;
- transitions;
- standardized terminal targets;
- access declarations;
- limits;
- approval checkpoints;
- error policy.

The UI must make clear that the Loop is a declarative orchestration contract and is not being executed by the registry.

Agent detail surfaces should expose orchestration metadata when present:

- resolved Loop dependency;
- global bindings;
- phase bindings grouped by authored phase key;
- Memory spaces/operations named by bindings;
- named MCP surfaces and Tool memberships;
- consumer-context filename.

The UI must not pretend that phase names or Memory identifiers were cross-package validated if the registry did not perform that validation.

Do not add execution controls, run-state simulation, model/provider configuration, approval controls, MCP host/port controls, or prompt previews in Phase 7A.

## SDK behavior

### Node SDK

Add typed Loop models and `loadLoop`.

`loadLoop(spec, options)` should mirror the installed-package resolution behavior used by `loadProfile` / other metadata-only loaders.

Return according to existing SDK conventions:

- `kind: "loop"`;
- package name/version/key/integrity where currently exposed;
- package root;
- manifest path;
- typed manifest;
- typed `loop` metadata.

Support a `loopDirOverride` option following existing loader test conventions.

Update generic Tool `load()` guidance to direct Loop callers to `loadLoop`.

Update `loadAgent` and public Agent manifest types to expose:

- resolved singular Loop relationship from lockfile roots;
- typed top-level Agent `loop` reference;
- typed `bindings` metadata, including global/phase package identities, Memory bindings, MCP bindings, and consumer context.

SDKs must not:

- execute phases or transitions;
- calculate effective bindings;
- validate bound phase names against the Loop;
- validate Memory spaces/operations against a Blueprint;
- enforce Loop access declarations;
- interpret error policy;
- start MCP servers;
- read consumer-context files;
- compile prompts;
- select providers/models;
- invoke a future harness.

### Python SDK

Add equivalent typed models and `load_loop`.

Export `load_loop` through public package exports and update `load_agent` with the same Loop relationship and binding metadata semantics as Node.

Node and Python should agree on field names, nullability, and non-execution boundaries where practical.

## Documentation and examples

Update manifest reference documentation, package-kind lists, CLI help, publish/install/new docs, registry copy, SDK references, and Agent composition documentation.

Documentation must clearly explain:

- Loop versus Agent versus Template responsibilities;
- Loop as orchestration contract rather than runtime code;
- open-ended archetype versus graph-defined execution structure;
- implicit `complete` outcome semantics;
- closed terminal-target semantics;
- phase access omission versus explicit true/false;
- global + phase additive binding intent;
- versionless binding references versus versioned top-level dependencies;
- Memory space/operation binding semantics;
- MCP binding intent without network/process configuration;
- consumer-context file ownership and workspace-relative semantics;
- Agent lint's intentionally local validation boundary;
- Phase 7B as the future AgentPM canonical harness implementation.

Provide multiple realistic Loop examples demonstrating materially different orchestration patterns, including at least:

- iterative planner/executor/reviewer behavior;
- human approval and handoff behavior;
- a different custom archetype that proves new orchestration patterns do not require a new AgentPM enum.

At least one published Agent example should exercise all major binding surfaces.

## Non-goals

Phase 7A does not:

- Implement `agentpm harness` or any Agent execution engine.
- Add Node or Python Loop execution support.
- Execute model calls, Tool calls, Skills, Knowledge retrieval, Memory reads/writes, Memory operations, approvals, retries, transitions, or handoffs.
- Add `agentpm loop build`, `loop inspect`, `loop run`, or generated Loop outputs.
- Add a general workflow/condition/expression language.
- Add arbitrary code, scripts, callbacks, JSONPath, CEL, JavaScript, Python, cron, or boolean expressions to Loop transitions.
- Make `archetype` a closed enum or runtime dispatch key.
- Add provider-specific model identifiers, prompts, token budgets, temperatures, credentials, or inference settings to Loops or Agent bindings.
- Define prompt-concatenation or instruction-precedence rules across Profiles, Skills, Loop objectives, consumer context, or host instructions.
- Define live Memory persistence adapters, scope values, databases, identity sources, or record CRUD APIs.
- Add record-type-level Memory bindings.
- Redefine Memory Blueprint operations or triggers from Agent bindings.
- Validate Agent-bound Memory space/operation names by resolving the Blueprint during lint/publish.
- Validate Agent phase binding keys by resolving the Loop during lint/publish.
- Reject or warn about Loop-access-versus-binding conflicts during lint, publish, install, or SDK loading.
- Add binding override/inheritance/replacement languages.
- Add Agent-to-Agent dependencies or make `$handoff` invoke another Agent.
- Standardize an `AGENTPM.md` consumer-context filename.
- Read or package the consumer-context file.
- Add MCP host, port, transport, authentication, or process configuration to Agent manifests.
- Infer MCP bindings from Tool bindings or vice versa.
- Remove the existing Agent requirement for a top-level `tools` array.
- Add Loop-owned package dependencies.
- Index nested Loop or binding metadata for search.

## Constraints / Invariants

- `loop` is the exact technical package kind.
- A Loop package has a required structured top-level `loop` object.
- An Agent may declare zero or one top-level `loop` package reference.
- Existing Agents without Loops remain valid.
- Loop packages are immutable dependency leaves.
- Templates may declare an optional direct `template.dependencies.loop` entry for the synthesized root Agent.
- The graph, not `archetype`, defines Loop control flow.
- Phase arrays are not implicitly execution-ordered.
- Omitted phase outcomes mean exactly one implicit outcome: `complete`.
- Present `outcomes` remove the implicit outcome unless `complete` is explicitly authored.
- Every valid phase/outcome pair maps to exactly one transition.
- Standard terminal targets are exactly `$end`, `$abort`, and `$handoff` in Phase 7A.
- `$handoff` never creates an Agent dependency.
- Transition objects remain only `from`, `on`, and `to`.
- Loop access omission means no opinion; it is distinct from `false`.
- Phase access is metadata only in 7A and is not compared with Agent bindings during package validation.
- Agent binding references are versionless package identities.
- Top-level Agent dependency declarations remain the source of package version/range resolution.
- Every bound package must be declared in the corresponding top-level Agent dependency collection.
- Agent lint validates dependency membership only; it does not inspect referenced package contents.
- Global and phase bindings are additive and do not add generic override/replace/inheritance rules.
- Memory bindings may name Blueprint spaces and lifecycle operations but may not redefine them.
- MCP bindings name Tool groupings only; network/process details remain runtime concerns.
- Consumer context uses a safe workspace-relative path and is not packaged with the Agent.
- AgentPM does not standardize the consumer-context filename.
- README remains package documentation for Loops and is not runtime Loop guidance.
- Direct and transitive installs remain non-interactive and CI-safe.
- Loop publishing requires no build/freshness step.
- Existing Tool, Agent, Template, Skill, Knowledge, Memory, and Profile behavior remains compatible.
- Private namespace authorization applies to Loops exactly as it does to other package kinds.
- Registry names remain unique across package kinds within a namespace according to the existing package model.

## Acceptance criteria

- A minimal valid `kind: "loop"` manifest passes shared schema validation and CLI lint.
- A full Loop using archetype, limits, phases, access, explicit/implicit outcomes, transitions, all three terminal targets, approval checkpoint, and error policy passes validation.
- Invalid Loop graphs fail with actionable paths/messages for unknown phases, invalid outcomes, missing/ambiguous transitions, unreachable phases, missing reachable terminals, checkpoint errors, and error-policy inconsistencies.
- `agentpm init --kind loop --name <name> --description <description>` creates a valid `agent.json` and README with no generated outputs or dependencies.
- `agentpm publish --dry-run` succeeds for a valid Loop without any build step.
- Publishing rejects dependency-bearing or structurally/semantically invalid Loop packages.
- Direct Loop installation places files under `.agentpm/loops/...` and preserves the package without executing it.
- Direct Loop installation in a local Agent updates singular top-level `loop` without generating bindings.
- Agent manifests accept zero or one Loop dependency and optional `bindings`.
- Binding schema rejects versions/ranges in binding package identities.
- Agent lint rejects a bound Tool/Skill/Knowledge/Memory/Profile package that is not declared in the corresponding top-level Agent dependency list.
- Agent lint rejects MCP Tool references not declared in top-level `tools`.
- Agent lint rejects phase bindings when the Agent has no top-level Loop.
- Agent lint does not reject an authored phase key solely because the referenced Loop was not inspected.
- Agent lint does not reject a Memory space/operation name solely because the referenced Blueprint was not inspected.
- Agent lint does not reject a Tool binding that conflicts with `access.tools: false` in the referenced Loop.
- Memory bindings accept package + spaces, package + operations, or package + both, with at least one selector present.
- Named MCP bindings accept multiple unique surfaces and remain independent from phase/global Tool bindings.
- Consumer context accepts safe relative paths, rejects unsafe paths, is optional at runtime, and is not packaged as an Agent file.
- Agent Loop dependencies resolve and record singular first-class Loop relationships in lockfile v3.
- Loop relationships participate in frozen mode, reachability, pruning, deduplication, replacement, refresh, and installed Agent traversal.
- Template manifests accept an optional singular `template.dependencies.loop` entry.
- `agentpm new` resolves/installs the direct Template Loop and writes its exact reference to the synthesized root Agent while preserving generated local Agent Loop declarations.
- Backend publish/install/resolve/search/trending/statistics/private authorization support `loop`.
- Database constraints and materialized views accept the eighth package kind without regressing existing kinds.
- Registry UI exposes Loop discovery/detail pages and Agent orchestration/binding metadata without implying execution.
- Node SDK exports typed Loop metadata, `loadLoop`, Agent Loop relationships, and Agent binding types.
- Python SDK exports equivalent `load_loop` and Agent relationship/binding metadata.
- SDK loaders expose metadata only and do not execute, validate cross-package binding content, enforce access, start MCP, or read consumer context.
- Existing package kinds continue to initialize, lint, publish, install, resolve, search, display, and load without regression.
- No Harness/runtime implementation is introduced in Phase 7A.

## Risks / edge cases

- **Workflow-language creep:** Loop is the artifact most likely to accumulate expressions, callbacks, and runtime-specific logic. Review must reject additions that turn transitions or checkpoints into a programming language.
- **Archetype overreach:** A runtime switch over archetype names would make third-party orchestration patterns require AgentPM releases and undermine portability.
- **Outcome ambiguity:** If explicit outcomes silently retain `complete`, transitions become surprising. Omission versus explicit outcome semantics must remain exact.
- **Graph incompleteness:** Missing transitions or duplicate transitions force runtimes to invent control flow. Semantic lint must guarantee exactly one target per valid phase/outcome pair.
- **Unreachable graph content:** Authored phases that cannot be reached can mislead consumers and should fail semantic lint.
- **Infinite loops:** A graph may intentionally cycle. `max_steps` is optional, so runtimes must still own their operational safety defaults. Phase 7A should not impose a mandatory limit solely to make execution safe.
- **Checkpoint semantics:** Approval is declarative. UI/API code must not imply the registry can approve, resume, or execute a Loop.
- **Access/enforcement confusion:** Explicit access metadata expresses Loop author intent but is not enforced in 7A. Package validation must not incorrectly treat a conflicting Agent binding as malformed.
- **Binding/version drift:** Reusing normal version-capable package refs inside bindings would create two version sources. Binding schema must enforce versionless identities.
- **Cross-package validation temptation:** Resolving Loops/Memory packages during Agent lint would complicate authoring/publishing and blur the metadata/runtime boundary. Local lint must stay local.
- **Memory operation meaning:** Memory operations are lifecycle contracts such as consolidate/transform/delete, not generic write methods. Agent docs/examples must not redefine them as arbitrary CRUD operations.
- **Memory identifier syntax:** Memory space/operation names use snake_case while Loop/binding IDs use kebab-case. Examples and schema references must preserve the distinction.
- **Global/phase duplication:** Additive semantics must deduplicate by package identity and union Memory selectors without creating duplicate dependencies.
- **Profile composition ambiguity:** Binding multiple Profiles does not define merge precedence or prompt order. SDK/UI must not imply otherwise.
- **MCP coupling:** MCP binding metadata must not accidentally hard-code `agentpm serve` host/port behavior into the portable Agent contract.
- **MCP/tool-policy interaction:** A Tool may be MCP-exposed but unavailable in a Loop phase under a future harness. 7A must preserve both declarations independently.
- **Consumer-context security:** The path is consumer-owned and runtime-relative. Safe-relative-path validation must prevent absolute/traversal paths, while runtimes remain responsible for whether/how they read the file.
- **Template singularity:** Direct Template Loop dependencies must remain at most one because the synthesized root Agent has singular `loop`; generated local Agents may still use different Loops.
- **Lockfile model mismatch:** Existing root relationships are mostly plural vectors. Singular Loop relationships should not be accidentally modeled as multi-Loop Agent semantics merely for implementation convenience.
- **Incomplete kind audit:** `loop` is the eighth first-class package kind and hardcoded kind lists span Rust, Python, TypeScript, SQL, SDKs, docs, tests, route helpers, and UI.
- **Migration ordering:** Search/trending materialized-view dependencies must follow existing safe migration order.
- **README confusion:** Registry/SDKs must not treat Loop README content as executable phase guidance.
- **Harness leakage:** Implementation must not introduce hidden execution logic simply because Phase 7B is expected to consume the metadata next.

## Open questions

No blocking product questions remain for Phase 7A.

Implementation should preserve repository patterns where exact helper names, lockfile models, route helpers, or test commands differ from this spec. If implementation reveals that a contract change is required—especially a lockfile version bump, additional terminal target, new transition field, provider/runtime configuration, cross-package linting, or binding override semantics—it must be raised before implementation rather than silently expanded.

## Related Specs

- Phase 2: Agent packages and Agent dependency installation
- Phase 4: Workflow Templates and `agentpm new`
- Phase 6A: Skills as first-class artifacts
- Phase 6B: Knowledge artifacts
- Phase 6C: Memory Blueprints
- Phase 6D: Instruction Profiles
- Phase 7B: AgentPM Harness command and runtime execution (companion phase; not implemented here)
