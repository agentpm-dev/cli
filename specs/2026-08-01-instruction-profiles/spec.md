# Feature
Phase 6D: Instruction Profiles

## Problem / Goal
Agent identity, role, priorities, communication style, authority boundaries, and non-negotiable behavioral constraints are commonly embedded as ad hoc system-prompt strings inside applications. That makes the behavioral layer difficult to version, review, reuse, discover, govern, and keep consistent across CLI, IDE, chat, workflow, and framework integrations.

Phase 6D introduces Instruction Profiles as a first-class AgentPM package kind.

The technical package kind and manifest vocabulary are `profile` and `profiles`. User-facing product copy should use **Instruction Profile** and **Instruction Profiles**.

An Instruction Profile is a versioned package of structured behavioral metadata that tells a consuming runtime or agent system how an agent should present itself and behave. It does not apply, merge, bind, execute, or enforce those instructions itself.

The phase must:

- Add `profile` as a supported package kind throughout the CLI, resolver, registry API, database, web application, and SDKs.
- Define a strict, structured `profile` manifest contract.
- Support direct Profile initialization, linting, publishing, installation, discovery, and loading.
- Allow Agent packages to depend on multiple Profiles through top-level `profiles`.
- Allow Template packages to depend on multiple Profiles through `template.dependencies.profiles`.
- Resolve and install Profile dependencies for normal Agent installs and `agentpm new` workspaces.
- Expose typed Profile metadata through the Node and Python SDKs.
- Preserve the distinction between Profiles and Skills: Profiles describe stable behavior; Skills package reusable task know-how, procedures, references, scripts, and optional Tool relationships.

### Core definition

> An Instruction Profile is a versioned package of structured behavioral metadata defining an agent's role, objectives, principles, communication guidance, audience posture, authority boundaries, and declared constraints. It is consumed by runtimes and agent systems but does not select, apply, combine, execute, or enforce those instructions itself.

## Manifest contract

A Profile package uses `kind: "profile"` and a required top-level `profile` object.

Profiles use the common package fields already supported by other package kinds:

- `kind`
- `name`
- `version`
- `description`
- optional `readme`
- optional `license`

Profiles do not add `display_name`.

The common `license` field keeps its existing object shape, for example:

```json
{
  "license": {
    "spdx": "MIT",
    "file": "LICENSE"
  }
}
```

The README is package documentation. It is not part of the behavioral contract and SDK/runtime consumers must not implicitly treat README content as Profile instructions.

### Example Profile manifest

```json
{
  "$schema": "https://raw.githubusercontent.com/agentpm-dev/cli/refs/heads/main/schemas/agentpm.manifest.schema.json",
  "kind": "profile",
  "name": "customer-success-advocate",
  "version": "1.0.0",
  "description": "A warm, professional behavior profile for customer-facing SaaS support agents.",
  "readme": "README.md",
  "license": {
    "spdx": "MIT",
    "file": "LICENSE"
  },
  "profile": {
    "identity": {
      "role": "Senior Customer Success Advocate",
      "description": "Represents the company while helping customers resolve product, account, and subscription issues.",
      "expertise": [
        "Customer communication",
        "Software configuration",
        "Account access",
        "Subscription billing"
      ]
    },
    "objectives": [
      "Help customers reach a clear resolution or next step.",
      "Reduce frustration through direct and respectful communication.",
      "Protect customer trust and sensitive information."
    ],
    "principles": [
      "Assume the customer is capable but may be frustrated.",
      "Focus on improving the current situation rather than assigning blame.",
      "Acknowledge uncertainty instead of presenting guesses as facts."
    ],
    "audience": {
      "description": "Customers ranging from non-technical business administrators to experienced developer operations teams.",
      "assumed_knowledge": "Basic familiarity with common software interfaces, accounts, and operating systems.",
      "adaptation": [
        "Match the technical depth demonstrated by the customer.",
        "Explain unfamiliar terminology without becoming patronizing."
      ]
    },
    "communication": {
      "tone": [
        "warm",
        "professional",
        "validating",
        "solution-oriented"
      ],
      "verbosity": "concise",
      "guidelines": [
        "Acknowledge the customer's concern before giving guidance.",
        "State the most useful next action early.",
        "Separate confirmed information from assumptions."
      ],
      "formatting": [
        "Use numbered lists when presenting multiple actions.",
        "Keep paragraphs brief."
      ],
      "vocabulary": {
        "prefer": [
          "resolve",
          "understand",
          "next step"
        ],
        "avoid": [
          "obviously",
          "fault",
          "as an AI"
        ]
      }
    },
    "boundaries": [
      "Do not claim access to systems that are not actually available.",
      "Do not claim authority outside the assigned role.",
      "Do not imply an external action occurred unless the consuming system confirms it."
    ],
    "constraints": [
      {
        "id": "protect-authentication-data",
        "strength": "required",
        "instruction": "Never request a raw password, complete authentication secret, or full payment-card number."
      },
      {
        "id": "avoid-generic-ai-disclaimers",
        "strength": "preferred",
        "instruction": "Do not introduce responses with generic statements about being an AI or language model."
      }
    ],
    "compatibility": {
      "minimum_context_tokens": 8000,
      "requires": {
        "tool_use": false,
        "structured_output": false,
        "multimodal_input": false
      },
      "recommends": {
        "tool_use": true,
        "structured_output": true,
        "multimodal_input": false
      }
    }
  }
}
```

### Profile fields

#### `identity`

Required object describing the role the agent should adopt.

- `role`: required non-empty string.
- `description`: optional non-empty string describing the role in more detail.
- `expertise`: optional non-empty array of unique non-empty strings describing expected areas of familiarity or perspective.

The package-level `name` identifies the artifact. `identity` does not add a separate character or persona name.

#### `objectives`

Required non-empty array of unique non-empty strings describing durable outcomes the role should prioritize across tasks.

Objectives must remain role-level rather than becoming a task procedure or workflow completion checklist.

#### `principles`

Optional non-empty array of unique non-empty strings describing general decision-making posture that should apply across tasks.

Examples include treatment of uncertainty, preference for reversible actions, or separation of facts from inferences.

#### `audience`

Optional object describing the audience the Profile is intended to interact with.

Supported fields:

- `description`: optional non-empty string.
- `assumed_knowledge`: optional non-empty string.
- `adaptation`: optional non-empty array of unique non-empty strings describing how communication should adapt to the audience.

If present, `audience` must contain at least one supported field.

#### `communication`

Required object describing how the agent should communicate.

- `tone`: required non-empty array of unique non-empty open-ended strings. Tone is intentionally not an enum.
- `verbosity`: required enum: `concise`, `balanced`, or `detailed`.
- `guidelines`: optional non-empty array of unique non-empty strings.
- `formatting`: optional non-empty array of unique non-empty strings.
- `vocabulary`: optional object containing:
  - `prefer`: optional non-empty array of unique non-empty strings.
  - `avoid`: optional non-empty array of unique non-empty strings.

If present, `vocabulary` must contain at least one supported field. Exact terms must not appear in both `prefer` and `avoid`; compare after trimming and case-folding.

#### `boundaries`

Optional non-empty array of unique non-empty strings describing the role's authority, access, representation, or capability boundaries.

Boundaries describe what the role should not claim, authorize, or represent. They do not grant or revoke actual runtime permissions.

#### `constraints`

Optional non-empty array of constraint objects.

Each constraint requires:

- `id`: stable kebab-case identifier matching `^[a-z][a-z0-9-]{0,63}$`.
- `strength`: `required` or `preferred`.
- `instruction`: non-empty string containing the behavioral instruction.

Constraint IDs must be unique within the Profile.

`required` and `preferred` express author intent only. AgentPM does not guarantee model compliance and must not describe either value as runtime enforcement.

#### `compatibility`

Optional advisory metadata for consumers. Compatibility metadata must remain vendor-neutral and must not imply reproducible behavior across models.

Supported fields:

- `minimum_context_tokens`: optional positive integer.
- `requires`: optional capability object.
- `recommends`: optional capability object.

The initial capability object supports optional boolean fields:

- `tool_use`
- `structured_output`
- `multimodal_input`

If present, `compatibility`, `requires`, and `recommends` must each contain at least one supported property. Consumers may use these fields to warn or inform; AgentPM does not reject installation or loading based on compatibility hints.

### Required and optional structure

The required Profile core is:

- `profile.identity`
- `profile.identity.role`
- `profile.objectives` with at least one item
- `profile.communication`
- `profile.communication.tone` with at least one item
- `profile.communication.verbosity`

The following are optional:

- `identity.description`
- `identity.expertise`
- `principles`
- `audience`
- `communication.guidelines`
- `communication.formatting`
- `communication.vocabulary`
- `boundaries`
- `constraints`
- `compatibility`

All Profile-specific objects use `additionalProperties: false`.

## Dependency model

### Agents

Agent manifests may declare multiple Profiles through top-level `profiles`:

```json
{
  "kind": "agent",
  "name": "research-assistant",
  "version": "0.1.0",
  "description": "Research assistant.",
  "tools": [],
  "profiles": [
    "@zack/research-planner@1.0.0",
    {
      "name": "@zack/research-analyst",
      "version": "1.0.0"
    }
  ]
}
```

An Agent dependency list means the Profiles are required members of the package graph. It does not mean they are active simultaneously or merged.

### Templates

Template manifests may declare multiple Profiles through `template.dependencies.profiles`:

```json
{
  "template": {
    "dependencies": {
      "tools": [],
      "agents": [],
      "profiles": [
        {
          "name": "@zack/research-analyst",
          "version": "1.0.0"
        }
      ]
    }
  }
}
```

`agentpm new` resolves and installs these packages and writes the resolved Profile references into the generated root Agent manifest's top-level `profiles` array.

### Profiles

Profile packages cannot declare package dependencies.

A `kind: "profile"` manifest must reject non-empty dependency declarations, including top-level `tools`, `agents`, `skills`, `knowledge`, `memory`, or `profiles`, and must not use Template dependency objects to bypass that rule.

## CLI behavior

### `agentpm init --kind profile`

Create:

- `agent.json`
- `README.md`

The generated manifest must:

- use `kind: "profile"`
- use the requested package name and description
- set `readme: "README.md"`
- include a small valid structured Profile starter
- not create instruction Markdown files, scripts, references, generated outputs, or Profile-specific directories
- not create a license file unless initialization behavior is intentionally standardized for all package kinds in a separate change

`--mode` remains Knowledge-only and must be rejected for Profiles when a non-default mode is supplied.

### `agentpm lint`

Schema validation handles the majority of Profile validation. Rust semantic validation must additionally reject:

- duplicate constraint IDs
- a normalized vocabulary term present in both `prefer` and `avoid`

Do not add subjective warnings about the quality, completeness, tone, number of constraints, or enterprise suitability of authored content.

### `agentpm publish`

Profiles use the normal immutable package publishing pipeline.

Publishing must:

- accept `kind: "profile"`
- validate the Profile manifest
- package `agent.json`
- package the referenced README and license file according to existing common behavior
- reject Profile package dependencies
- persist the manifest and common metadata
- create no Profile-specific build metadata or generated output columns
- require no build freshness check

### `agentpm install`

Support direct Profile installation and Profile dependency installation.

Installed Profiles live under:

```text
.agentpm/profiles/<namespace>/<name>/<version>
```

When a direct Profile spec is installed in a local Agent project, update the Agent manifest's top-level `profiles` field using the same range/update behavior used for Skills, Knowledge, and Memory.

Do not make installation interactive. Profiles have no install-time variables or parameter resolution in this phase.

### `agentpm new`

When a Template declares Profile dependencies:

- include them in the resolver request
- install them under `.agentpm/profiles`
- include resolved Profile references in the synthesized root Agent manifest
- include them in the generated lockfile graph
- do not prompt for Profile-specific values
- do not treat Template variables as Profile parameters

## Lockfile behavior

Profiles become a first-class Agent root relationship in lockfile v3.

Add `profiles: Vec<String>` to the relevant root models and Agent root variants. Package keys use:

```text
profile:@namespace/name@version
```

Existing lockfiles may contain unresolved Profile references under `reserved.profiles`. Preserve backward compatibility by:

- continuing to deserialize `reserved.profiles`
- migrating resolvable reserved Profile references into the first-class root `profiles` list when regenerating or normalizing a lockfile
- retaining unresolvable entries in `reserved.profiles`
- avoiding a lockfile version bump unless implementation reveals an incompatible serialization requirement; the intended implementation keeps lockfile version 3

Frozen install behavior must require lockfile v3 for Profile dependency graphs and include Profiles in the corresponding error messaging.

Reachability, pruning, deduplication, root replacement, and transitive Agent dependency traversal must include Profile relationships.

## Registry API and database behavior

Add `profile` to every API, DTO, serializer, route, validation, authorization, resolver, install, search, statistics, and package-kind allowlist that currently supports the other package kinds.

The backend remains authoritative for the stored package kind when resolving direct package specs.

Agent install graph expansion must include top-level Profile dependencies. Skill expansion remains Tool-only. Profile packages are leaves and do not expand dependencies.

Template dependency extraction and `agentpm new` resolution must include `template.dependencies.profiles`.

Profile detail URLs use:

```text
/profiles/<package-id>/v<version>/overview
```

### Database migration

Create a migration parallel to the recent Memory package-kind migrations.

The migration must:

- drop `tool_search_index` before `trending_tools`
- update `tools_kind_check` to allow `profile`
- update `on_install_completed()` so Profile installs contribute to aggregate and per-package download statistics
- recreate `trending_tools` with `profile` included in its package-kind allowlist
- recreate the trending indexes
- recreate `tool_search_index`
- recreate all search indexes
- recreate the install completion trigger according to existing migration patterns

The effective allowlist becomes:

```sql
('tool', 'agent', 'template', 'skill', 'knowledge', 'memory', 'profile')
```

The search document should continue to index package name, namespace handle, and package description. Do not add nested Profile content to full-text search in this phase.

Profiles should receive their own trending partition because ranking is partitioned by `kind`.

## Registry web behavior

Use `profile` in technical route, query, type, and API values. Use **Instruction Profile** in visible labels and explanatory copy.

Add Profile support to:

- search result and trending type unions
- Explore filters and cards
- global search
- route selection and links
- public/private package detail fetches
- package badges and package-kind labels
- landing/discovery presentation where other first-class package kinds are shown
- Agent dependency presentation
- Template dependency presentation

Add Profile-specific types and manifest parsing helpers matching the actual schema.

Provide Profile detail pages using the established package layout patterns. At minimum:

- Overview
- README
- Security

The Overview should present structured metadata without pretending to enforce it. Useful sections include:

- Identity and role
- Expertise
- Objectives
- Principles
- Audience
- Communication style
- Boundaries
- Constraints, including required/preferred labels with clear non-enforcement wording
- Compatibility hints

Do not add a manual/instructions tab that treats README as executable behavior. Do not add build, lifecycle, contracts, or query tabs.

## SDK behavior

### Node SDK

Add public Profile interfaces and `loadProfile`.

`loadProfile(spec, options)` should mirror `loadKnowledge` package-resolution behavior while remaining simpler because Profiles have no generated or referenced behavioral files.

Return:

- `kind: "profile"`
- package name
- version
- description
- package root
- manifest path
- typed manifest
- typed `profile` metadata

Support a `profileDirOverride` option following existing loader test conventions.

The generic Tool `load()` function must reject installed or likely Profile specs with guidance to use `loadProfile`.

`loadAgent` must expose first-class Profile dependency relationships from lockfile roots. Follow the existing resolved Skill, Knowledge, and Memory metadata pattern, including useful package identity and nullable disk paths when a locked package is missing locally.

### Python SDK

Add equivalent TypedDict or other existing-style public Profile types and `load_profile`.

Export `load_profile` from `agentpm.__init__` and `__all__`.

The generic Tool `load()` function must guide Profile callers to `load_profile`.

`load_agent` must expose resolved Profile relationships consistently with Node and with existing Python SDK dependency metadata.

### SDK non-goals

SDK loaders must not:

- compile a system prompt
- flatten structured fields into prose
- load README content as behavioral instructions
- merge Profiles
- select an active Profile
- resolve global or phase bindings
- interpolate variables
- enforce constraints
- reject Profiles based on compatibility hints

## Documentation and examples

Update manifest reference documentation, package-kind lists, CLI help, install/publish/new docs, registry copy, and SDK documentation.

Document the core distinction:

- Profile: stable identity, priorities, communication posture, authority boundaries, and behavioral constraints.
- Skill: reusable know-how for performing a recognizable task or capability, including procedures, references, scripts, and optional Tool relationships.

Document that a Profile's `required` constraints are declarations of intent, not guarantees.

Document that Profile packages are immutable and are not parameterized or rewritten during installation.

Provide at least two examples showing materially different uses of the same schema, such as:

- customer-facing support behavior
- production incident command behavior

## Non-goals

Phase 6D does not:

- Add a Profile build command or generated outputs.
- Add a Profile-specific inspect, query, run, or execution command.
- Compile Profile metadata into a system prompt.
- Select which Profile is active.
- Define Agent phase bindings.
- Define global versus phase Profile semantics.
- Merge, layer, order, or resolve conflicts between Profiles.
- Enforce constraints, permissions, safety rules, authority boundaries, or compatibility requirements.
- Guarantee consistent behavior across models or runtimes.
- Add freeform Profile instruction Markdown files, progressive disclosure, or conditional loading.
- Add arbitrary `instructions`, `prompt`, or `system_prompt` fields.
- Add Profile-owned Tools, Skills, Knowledge, Memory, Agents, Templates, or other dependencies.
- Add Profile parameters, variables, interpolation, install-time prompting, or package mutation.
- Add task procedures, workflow steps, output event protocols, routing actions, or exact machine control tags to the Profile contract.
- Add provider-specific model identifiers or prompt formats.
- Add nested Profile fields to search indexing.
- Redesign the future full Agent composition or binding schema.

## Constraints / Invariants

- The technical package kind is exactly `profile`.
- Technical dependency collection names are exactly `profiles`.
- User-facing naming is Instruction Profile / Instruction Profiles.
- The Profile contract is structured in `agent.json`; README content is documentation only.
- `display_name` is not added to Profile packages.
- Profile packages have no package dependencies.
- Agents and Templates may declare multiple Profile dependencies.
- Dependency declaration does not imply activation, merging, or simultaneous application.
- Profiles remain immutable after publishing and installation.
- Direct and transitive installs remain non-interactive and CI-safe.
- Tone values are open-ended strings.
- Verbosity is the closed enum `concise | balanced | detailed`.
- Constraint strength is the closed enum `required | preferred`.
- `required` never means AgentPM runtime enforcement.
- Compatibility metadata is advisory and vendor-neutral.
- Unknown Profile object fields fail validation because Profile objects use `additionalProperties: false`.
- Existing Tool, Agent, Template, Skill, Knowledge, and Memory behavior must remain compatible.
- Existing v3 lockfiles with `reserved.profiles` must remain readable.
- Private namespace authorization must apply to Profiles exactly as it does to other package kinds.
- Registry names remain unique across package kinds within a namespace according to the existing package model.

## Acceptance criteria

- A valid `kind: "profile"` manifest using the defined contract passes schema and CLI lint validation.
- Invalid Profile manifests fail with actionable paths for missing required fields, invalid enums, extra properties, duplicate constraint IDs, and conflicting vocabulary terms.
- `agentpm init --kind profile --name <name> --description <description>` creates a valid `agent.json` and README without generated outputs or instruction files.
- `agentpm publish --dry-run` succeeds for a valid Profile package without requiring a build.
- Publishing succeeds through the registry API and persists the Profile kind and manifest.
- Publishing rejects Profile manifests that declare package dependencies.
- `agentpm install @namespace/profile@version` installs the artifact under `.agentpm/profiles/...`.
- Installing a Profile directly in a local Agent project updates top-level `profiles` using existing range behavior.
- Agent manifests resolve multiple Profile dependencies, install them, and record first-class `profile:` package and root relationship entries in `agent.lock`.
- Resolvable `reserved.profiles` entries migrate to first-class root `profiles`; unresolved entries remain reserved.
- Frozen installs reject incompatible old lockfiles for Profile dependency graphs and succeed with valid lockfile v3 data.
- Template manifests accept `template.dependencies.profiles`.
- `agentpm new` resolves, installs, and writes resolved Template Profile dependencies into the generated Agent manifest and lockfile.
- The backend resolve graph expands Agent Profile dependencies and treats Profiles as dependency leaves.
- Database constraints, search views, trending views, install statistics, API allowlists, and route helpers all support `profile`.
- Public and private Profiles can be searched, installed, viewed, and authorized according to existing namespace rules.
- Registry UI uses Instruction Profile labels and provides Profile cards, filters, links, and detail pages.
- Node SDK exports typed Profile models and `loadProfile` and resolves Agent Profile relationships.
- Python SDK exports typed Profile models and `load_profile` and resolves Agent Profile relationships.
- SDK loaders return structured manifest data only and do not compile or enforce behavior.
- Existing package kinds continue to initialize, lint, publish, install, search, display, and load without regression.
- No Profile build command or runtime application behavior is introduced.

## Risks / edge cases

- **Profile/Skill overlap:** Authors can still place procedural content in string fields. The schema should make misuse awkward, documentation should state the boundary, but AgentPM must not attempt subjective content classification.
- **False enforcement expectations:** UI and docs may accidentally present required constraints as guarantees. All surfaces must use declaration-oriented language.
- **Reserved lockfile transition:** Existing `reserved.profiles` data can be lost if migration logic drains entries without preserving unresolved references.
- **Incomplete kind audits:** Hardcoded package-kind lists exist across Rust, Python, TypeScript, SQL, tests, route helpers, and error messages. Missing one can produce late failures despite successful linting.
- **Materialized-view migration ordering:** `tool_search_index` depends on `trending_tools`; views and indexes must be dropped and recreated in a safe order.
- **Install reachability:** Profile packages may be pruned from lockfiles if root traversal and installed Agent manifest traversal are not updated together.
- **Direct install mutation:** Direct Profile installs should update local Agent manifests but must not mutate Profile package content.
- **Template synthesis:** It is easy to resolve Template Profiles but accidentally continue writing an empty `profiles` array into the generated root Agent manifest.
- **Wrong package root:** Missing `.agentpm/profiles` routing can place Profile tarballs under Tools or cause SDK lookup failures.
- **Backend/CLI validation drift:** The CLI schema is authoritative for authoring, but direct API publishing must still reject structurally invalid or dependency-bearing Profile artifacts.
- **Compatibility interpretation:** Consumers may treat hints as hard requirements. SDK and UI text must remain advisory.
- **README confusion:** Detail pages or SDKs may accidentally treat README content as part of the behavioral contract.
- **License shape regression:** Examples must use the existing license object rather than a string.
- **Search discoverability:** Nested Profile fields are intentionally not indexed in this phase; package name and description must carry useful discovery text.
- **Private packages:** Profile-specific routes and search cards must not bypass existing namespace visibility checks.

## Open questions

No blocking product questions remain for Phase 6D.

Implementation should preserve existing repository patterns where exact helper names or test commands differ from this spec. Any discovered requirement to change the Profile contract, lockfile version, dependency semantics, or runtime scope must be raised before implementation rather than silently expanded.

## Related Specs

- Phase 2: Agent packages and Agent dependency installation
- Phase 4: Workflow Templates and `agentpm new`
- Phase 6A: Skills as first-class artifacts
- Phase 6B: Knowledge artifacts
- Phase 6C: Memory Blueprints
- Phase 7: Loop and Harness, which may later consume and bind Profiles but is not part of this phase
