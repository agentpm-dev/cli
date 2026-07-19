# Feature
Phase 6C: Memory Blueprints

## Problem / Goal
AgentPM needs a first-class `memory` artifact kind that packages the declarative structure, governance, lifecycle expectations, and interoperability contracts for durable agent memory.

Today, memory implementations are typically application-specific. Teams use inconsistent record shapes, names, retention behavior, summarization rules, scope models, and privacy conventions. That makes memory difficult to reuse across agents, difficult to audit, and difficult to move between runtimes or storage backends.

A Memory Blueprint must define what memory records look like and how a compatible runtime is expected to treat them, without packaging live memory data or implementing a storage engine. The artifact is a contract and policy package, not a concrete memory store.

The Phase 6C implementation must:

- Add `kind: "memory"` as a publishable, installable AgentPM artifact kind.
- Let blueprint authors define reusable scope keys, record types, logical memory spaces, retrieval requirements, retention rules, governance annotations, and declarative lifecycle operations.
- Generate resolved record contracts that combine the standard AgentPM memory-record envelope with each valid space-and-record-type pairing.
- Require `agentpm memory build` before publish.
- Make `agentpm publish` verify that generated contracts are present and current without rebuilding them.
- Allow agents and templates to depend on Memory Blueprint packages.
- Add SDK loaders that expose installed Memory Blueprint metadata and generated contracts, without adding live-memory CRUD APIs.
- Present Memory Blueprint metadata and resolved contracts in the registry UI using existing package-detail patterns.

The long-term architecture is:

- Memory Blueprint: declares structure, governance, lifecycle policy, and operation contracts.
- Agent artifact: later owns bindings between loops, memory operations, skills/tools, and storage adapters.
- Harness/runtime: later evaluates triggers, enforces policies, invokes bound implementations, and moves live records.
- Memory store: persists and retrieves records using any compatible backend.

Phase 6C prepares the contract surfaces needed by that future runtime behavior but does not implement the runtime behavior itself.

## Non-goals
- Do not implement live memory persistence, mutation, deletion, or querying.
- Do not add a hosted AgentPM memory service.
- Do not add memory-store adapters for Postgres, Redis, vector databases, graph databases, files, or SaaS providers.
- Do not provision infrastructure from a blueprint.
- Do not execute retention, summarization, consolidation, redaction, deletion, or migration logic.
- Do not bind Memory Blueprint operations to AgentPM tools or skills in this phase.
- Do not add agent loop or harness bindings; those belong to Phase 7.
- Do not add embedding provider, model, dimension, metric, or vector payload requirements to Memory Blueprints.
- Do not package live or seed memory records.
- Do not standardize cross-store synchronization or import/export execution.
- Do not implement automatic schema migrations for stored memory records.
- Do not add arbitrary condition-expression evaluation for lifecycle triggers.
- Do not add graph memory semantics in the MVP.
- Do not add SDK methods for live record CRUD such as `get`, `put`, `search`, or `delete`.

## Constraints / Invariants

### Artifact identity and dependencies
- The manifest kind is `memory`.
- “Memory Blueprint” is the product-facing term used in docs and UI.
- A Memory Blueprint has no package dependencies.
- Agents may reference Memory Blueprints through the existing top-level `memory` dependency array.
- Templates may reference Memory Blueprints through `template.dependencies.memory`.
- Memory packages must remain independently reusable and must not reference tools, skills, knowledge, agents, templates, loops, profiles, or other memory packages.

### Declarative-only boundary
- The blueprint describes intended behavior but does not implement it.
- Operations declare what inputs, outputs, triggers, and source-handling behavior a compatible runtime must support.
- Operations must not reference tools, skills, prompts, models, providers, commands, scripts, runtime functions, or storage implementations.
- Retrieval declarations describe required logical behavior, not physical backend technology.
- Scope values come from the consuming agent/runtime. The blueprint only defines scope keys and which spaces require them.

### Top-level memory shape
A `kind: "memory"` manifest must have a top-level `memory` object with:

- `scopes`
- `record_types`
- `spaces`
- optional `operations`

Recommended high-level shape:

```json
{
  "kind": "memory",
  "name": "conversation-continuity",
  "version": "0.1.0",
  "description": "Portable memory structure for conversational continuity.",
  "memory": {
    "scopes": {},
    "record_types": {},
    "spaces": {},
    "operations": {}
  }
}
```

All map keys under `scopes`, `record_types`, `spaces`, and `operations` must match:

```text
^[a-z][a-z0-9_]*$
```

All maps must reject duplicate keys through normal JSON parsing semantics and must use `additionalProperties: false` within each defined object shape.

### Scopes
Scopes are reusable logical partition keys declared once and referenced by spaces.

```json
"scopes": {
  "user": {
    "description": "The user whose memory is being retained."
  },
  "conversation": {
    "description": "A single conversation or thread."
  }
}
```

Scope requirements:

- `description` is required and must be non-empty.
- Scope keys are author-defined and domain-specific.
- A space references scopes using an ordered `scope` array.
- Every referenced scope must exist in the top-level `scopes` map.
- At runtime, every record in the space must carry exactly the declared scope keys.
- Scope values are non-empty strings in the canonical logical record contract.
- Scope order is significant for canonical identity and generated contract determinism.

Example:

```json
"scope": ["organization", "customer", "case"]
```

### Record types
Record types define the semantic meaning and content schema of memory records.

```json
"record_types": {
  "interaction": {
    "version": "1.0.0",
    "description": "One user, assistant, or tool interaction.",
    "schema": "schemas/interaction.schema.json"
  }
}
```

Record-type requirements:

- `version` is required and must be full semantic version syntax.
- `description` is required and must be non-empty.
- `schema` is required and must be a safe package-relative path.
- Referenced schema files must exist and remain inside the package root.
- Referenced schemas must be valid JSON and valid JSON Schema Draft 2020-12.
- The referenced schema validates only the record `content` payload, not the AgentPM envelope.
- The active schema should normally describe an object, but valid Draft 2020-12 schemas may use composition keywords when needed.
- Memory build must reject schemas that redefine the AgentPM envelope because only the schema under `content` is embedded into the resolved contract.
- Phase 6C records one active schema version per record type. Historical schema registration and migration execution are out of scope.

### Field-level governance annotations
Memory content schemas may use AgentPM-owned JSON Schema extension keywords.

Initial vocabulary:

```json
{
  "x-agentpm-data-class": "personal",
  "x-agentpm-sensitivity": "high",
  "x-agentpm-persist": true,
  "x-agentpm-shareable": false
}
```

Allowed values:

- `x-agentpm-data-class`: `public`, `internal`, `personal`, `authentication`, `financial`, `health`, `legal`, `operational`, `other`
- `x-agentpm-sensitivity`: `low`, `moderate`, `high`, `critical`
- `x-agentpm-persist`: boolean
- `x-agentpm-shareable`: boolean

Semantics:

- `x-agentpm-data-class` categorizes the type of data.
- `x-agentpm-sensitivity` communicates handling severity independently from data class.
- `x-agentpm-persist: false` means a compatible runtime must not persist the annotated value as durable memory.
- `x-agentpm-shareable: false` means a compatible runtime must not expose the annotated value across agent, user, tenant, organization, export, or synchronization boundaries unless a future binding or policy explicitly defines an equally strict approved boundary.
- These annotations are contract metadata in Phase 6C. AgentPM validates their shape and allowed values but does not enforce them against live records yet.
- Unknown `x-agentpm-*` keywords must fail validation to prevent misspelled or unsupported governance contracts.
- Non-AgentPM custom JSON Schema keywords may remain allowed according to the JSON Schema implementation, but they are not interpreted by AgentPM.

### Spaces
Spaces define logical memory organization and behavior independently from physical storage.

```json
"spaces": {
  "recent_interactions": {
    "description": "Short-term ordered interaction history.",
    "model": "sequence",
    "record_types": ["interaction"],
    "scope": ["user", "conversation"],
    "retrieval": {
      "modes": ["chronological"]
    },
    "capacity": {
      "max_records": 20
    },
    "retention": {
      "ttl": "P7D",
      "on_expire": "delete"
    }
  }
}
```

Required fields:

- `description`
- `model`
- `record_types`
- `scope`
- `retrieval`

Allowed `model` values:

- `document`
- `collection`
- `sequence`

Model semantics:

- `document`: exactly one current logical record exists for each complete space-and-scope tuple. Updating the logical document replaces or mutates the current record according to the runtime implementation.
- `collection`: multiple unordered records may exist for the same complete space-and-scope tuple.
- `sequence`: multiple ordered records may exist for the same complete space-and-scope tuple.

`graph` is intentionally deferred.

Space requirements:

- `record_types` must contain at least one unique record-type key.
- Every record type must exist in the top-level `record_types` map.
- `scope` must contain at least one unique top-level scope key.
- Every scope key must exist in the top-level `scopes` map.
- The space-and-record-type pairings explicitly declared by `record_types` are the only pairings for which resolved record contracts are generated.
- A space may optionally define `capacity`, `retention`, and `constraints`.

### Retrieval
Retrieval describes logical capabilities a compatible consumer must support.

```json
"retrieval": {
  "modes": ["key", "filter", "chronological", "full_text", "semantic"]
}
```

Allowed retrieval modes:

- `key`
- `filter`
- `chronological`
- `full_text`
- `semantic`

Rules:

- `modes` is required, must contain at least one value, and must be unique.
- `document` spaces must include `key`.
- `sequence` spaces must include `chronological`.
- `collection` spaces have no required mode beyond at least one declared mode.
- `semantic` means semantic retrieval capability is required but does not declare or lock an embedding provider, model, dimensions, metric, index, or database.
- `full_text` and `semantic` may coexist.
- Retrieval defaults such as top-k, score thresholds, query prompts, or context-injection behavior are out of scope.

### Capacity
Optional capacity declaration:

```json
"capacity": {
  "max_records": 20
}
```

Rules:

- `max_records` must be an integer greater than zero.
- Capacity is declarative and not enforced by the CLI in Phase 6C.
- Capacity may be used by a `capacity` trigger.
- Byte-size and token-size capacity are deferred because their runtime semantics are not stable enough for MVP.

### Retention
Optional retention declaration:

```json
"retention": {
  "ttl": "P30D",
  "on_expire": "delete"
}
```

Fields:

- `ttl`: required when `retention` is present; ISO 8601 duration string.
- `on_expire`: required when `retention` is present.

Allowed `on_expire` values:

- `delete`
- `archive`

Rules:

- `delete` means the record should no longer remain available as active memory after expiration.
- `archive` means the runtime may move the record out of active retrieval while retaining it according to implementation-specific archival behavior.
- `refresh_on_access`, decay scoring, importance scoring, and indefinite-retention flags are deferred.
- No runtime enforcement occurs in Phase 6C.

### Constraints
Optional space constraints:

```json
"constraints": {
  "append_only": true
}
```

Initial constraint:

- `append_only`: boolean

Rules:

- `append_only: true` is only valid for `sequence` or `collection` spaces.
- `document` spaces cannot be append-only because their defining semantic is one mutable current logical document per complete scope tuple.
- Additional constraints are deferred.

### Lifecycle operations
Operations are optional declarative integration contracts.

Initial operation types:

- `consolidate`
- `transform`
- `delete`

Operations are intentionally narrow in MVP.

#### Consolidate
A consolidate operation converts one or more source records into a destination record.

```json
"consolidate_recent_interactions": {
  "type": "consolidate",
  "description": "Convert recent interactions into a durable summary.",
  "trigger": {
    "type": "record_count",
    "space": "recent_interactions",
    "threshold": 20
  },
  "inputs": [
    {
      "space": "recent_interactions",
      "record_type": "interaction"
    }
  ],
  "output": {
    "space": "conversation_history",
    "record_type": "conversation_summary"
  },
  "source_handling": "delete_after_success",
  "preserve_provenance": true
}
```

Rules:

- `description`, `trigger`, `inputs`, `output`, `source_handling`, and `preserve_provenance` are required.
- Every source and output space/record type must exist.
- Each referenced record type must be permitted by its referenced space.
- `inputs` must contain at least one unique space-and-record-type pair.
- Allowed `source_handling` values:
  - `retain`
  - `retain_until_expiration`
  - `delete_after_success`
- `preserve_provenance` is boolean.

#### Transform
A transform operation converts one source record contract into another record contract.

Rules:

- Same source/output cross-reference rules as consolidate.
- Exactly one input pairing is required.
- `source_handling` and `preserve_provenance` use the same values and semantics as consolidate.

#### Delete
A delete operation declares a set of spaces that may be deleted as one lifecycle action.

```json
"delete_user_memory": {
  "type": "delete",
  "description": "Delete user-scoped durable memory.",
  "trigger": {
    "type": "external"
  },
  "targets": [
    { "space": "profile" },
    { "space": "conversation_history" }
  ],
  "cascade_derived_records": true
}
```

Rules:

- `description`, `trigger`, `targets`, and `cascade_derived_records` are required.
- `targets` must contain at least one unique space reference.
- Every target space must exist.
- Audit-output contracts are deferred from MVP to avoid implying live enforcement infrastructure.

### Triggers
Allowed trigger types:

- `external`
- `record_count`
- `capacity`
- `interval`

Semantics:

- `external`: the consuming application, agent binding, or harness explicitly invokes the operation.
- `record_count`: operation becomes eligible when a declared space reaches or exceeds a record threshold.
- `capacity`: operation becomes eligible when the declared space reaches its configured capacity.
- `interval`: operation becomes eligible on a recurring elapsed-time interval.

Shapes:

```json
{ "type": "external" }
```

```json
{
  "type": "record_count",
  "space": "recent_interactions",
  "threshold": 20
}
```

```json
{
  "type": "capacity",
  "space": "recent_interactions"
}
```

```json
{
  "type": "interval",
  "every": "P1D"
}
```

Rules:

- `record_count.threshold` must be greater than zero.
- `record_count.space` and `capacity.space` must exist.
- A `capacity` trigger requires the target space to declare `capacity.max_records`.
- `interval.every` must be an ISO 8601 duration.
- Arbitrary field matching, JSONPath, boolean expression languages, cron syntax, business events such as `case_resolved`, and model/token thresholds are deferred.
- Domain-specific business events should use `external` in Phase 6C and be mapped by a future agent binding or consumer runtime.

### Canonical logical memory-record envelope
AgentPM owns a standard logical record envelope. It is not a required physical database layout. A compatible runtime must be able to map stored records to and from the resolved contract shape.

Minimal envelope fields:

```json
{
  "id": "mem_123",
  "record_type": "interaction",
  "space": "recent_interactions",
  "scope": {
    "user": "user_123",
    "conversation": "conversation_456"
  },
  "schema_version": "1.0.0",
  "created_at": "2026-07-18T18:00:00Z",
  "ordinal": 42,
  "content": {}
}
```

Common properties:

- `id`: required non-empty string.
- `record_type`: required constant in each resolved contract.
- `space`: required constant in each resolved contract.
- `scope`: required object with exactly the space’s declared scope keys and non-empty string values.
- `schema_version`: required constant matching the record type version.
- `created_at`: required RFC 3339 date-time.
- `updated_at`: optional RFC 3339 date-time.
- `expires_at`: optional RFC 3339 date-time.
- `ordinal`: required non-negative integer for `sequence` spaces; omitted from generated contracts for other models.
- `provenance`: optional object.
- `content`: required and validated by the record-type JSON Schema.

Provenance shape:

```json
{
  "source_record_ids": ["mem_1", "mem_2"],
  "operation": "consolidate_recent_interactions"
}
```

Rules:

- `source_record_ids` is optional, unique, and contains non-empty strings.
- `operation` is optional and, when present, must reference a declared blueprint operation.
- A derived record may retain source IDs even after source records are no longer resolvable.
- Blueprint package identity is not repeated on every record; package/version identity belongs to the containing installed package, export bundle, or future store binding.

### Logical identity
- For `document`, the unique logical identity is blueprint package + space + complete ordered scope tuple.
- For `collection`, multiple records may exist under the same complete scope tuple and are distinguished by `id`.
- For `sequence`, multiple records may exist under the same complete scope tuple and are ordered by required `ordinal`.
- `ordinal` must be monotonically increasing within a complete space-and-scope identity. Assignment and concurrency behavior are runtime responsibilities.
- Physical database keys and naming conventions are not defined by the blueprint.

### Memory build
Add:

```bash
agentpm memory build
```

Suggested arguments:

```bash
agentpm memory build --manifest agent.json
```

The command must:

1. Resolve and validate the manifest using the existing embedded manifest schema flow.
2. Require `kind: "memory"`.
3. Perform semantic validation across scopes, record types, spaces, retrieval rules, retention, constraints, operations, and triggers.
4. Resolve each record-type schema path safely within the package root.
5. Parse and compile each content schema as Draft 2020-12.
6. Validate supported AgentPM governance keywords recursively.
7. Generate one resolved record contract for every explicitly allowed space-and-record-type pairing.
8. Generate a consumer-facing contract index.
9. Generate build metadata with source and output hashes.
10. Replace generated output atomically.
11. Never modify `agent.json` or author-owned schema files.

Generated paths:

```text
memory/
  contracts/
    index.json
    <space>.<record_type>.schema.json
  build.json
```

The command owns all files under `memory/contracts/` and `memory/build.json`.

A successful write build must fully replace `memory/contracts/` so removed pairings do not leave stale files.

### Generated resolved record contracts
Each generated schema must:

- Use JSON Schema Draft 2020-12.
- Set `additionalProperties: false` for the envelope.
- Use `const` for `space`, `record_type`, and `schema_version`.
- Generate an exact `scope` object with required keys and `additionalProperties: false`.
- Require `ordinal` only for sequence spaces.
- Include common optional timestamps and provenance.
- Embed or safely reference the author content schema in a way that remains valid after package installation.
- Preserve all supported field-level governance annotations in the resolved `content` contract.
- Have deterministic property ordering and pretty JSON serialization.
- Use a stable generated `$id` format that does not depend on local absolute paths. Recommended format:

```text
agentpm://memory/<package-name>/<package-version>/<space>/<record-type>/<schema-version>
```

The exact encoding must be documented and deterministic.

Recommended filename:

```text
<space>.<record_type>.schema.json
```

Because map keys are constrained, this is collision-safe within a package.

### Contract index
`memory/contracts/index.json` is consumer-facing and must contain:

```json
{
  "format_version": 1,
  "contracts": [
    {
      "space": "recent_interactions",
      "record_type": "interaction",
      "schema_version": "1.0.0",
      "model": "sequence",
      "path": "memory/contracts/recent_interactions.interaction.schema.json"
    }
  ]
}
```

Rules:

- Contract entries are sorted by space, then record type.
- `path` is package-relative and safe.
- `contract_count` does not need to be duplicated here; consumers can use array length.
- The index is authoritative for enumerating generated resolved contracts.

### Build metadata
`memory/build.json` is for freshness and integrity verification.

Required shape:

```json
{
  "type": "agentpm-memory-contracts",
  "format_version": 1,
  "agentpm_version": "0.1.x",
  "built_at": "2026-07-18T18:00:00Z",
  "manifest_path": "agent.json",
  "source_manifest_hash": "sha256:...",
  "source_schemas_hash": "sha256:...",
  "source_contract_inputs_hash": "sha256:...",
  "contracts_index_hash": "sha256:...",
  "contracts_hash": "sha256:...",
  "contract_count": 3
}
```

Hash semantics:

- `source_manifest_hash`: hash of the raw current `agent.json` bytes. This intentionally detects any manifest edit, including non-memory metadata changes, and keeps publish behavior simple and conservative.
- `source_schemas_hash`: aggregate hash of referenced source schema paths and bytes, sorted by path.
- `source_contract_inputs_hash`: aggregate hash of the canonical serialized `memory` object plus referenced schema path/byte entries. This is the semantic build-input hash.
- `contracts_index_hash`: hash of generated `memory/contracts/index.json` bytes.
- `contracts_hash`: aggregate hash of generated contract schema paths and bytes, sorted by path.
- `built_at` and `agentpm_version` must not contribute to output hashes.
- Hash strings use the existing `sha256:<hex>` convention.

### Check mode and freshness verification
Implement an internal Memory build mode parallel to Knowledge:

- `Write`: validates and regenerates outputs.
- `Check`: validates sources and computes expected hashes/contracts without writing.

Publish verification must:

- Require `memory/build.json`.
- Require `memory/contracts/index.json`.
- Require all indexed contract files.
- Reject unsupported `type` or `format_version`.
- Recompute current source hashes in check mode.
- Recompute expected resolved contracts deterministically.
- Compare current source hashes, index hash, contract aggregate hash, and count with build metadata.
- Reject manually modified, missing, extra indexed, stale, or unsupported generated output.
- Return an actionable error ending with `Run agentpm memory build to refresh it.`
- Never modify package files during publish.

Extra unindexed files under `memory/contracts/` should fail publish so the published contract set exactly matches the generated build.

### Inspect command
Add:

```bash
agentpm memory inspect <PATH_OR_PACKAGE>
agentpm memory inspect <PATH_OR_PACKAGE> --json
```

It should mirror Knowledge target resolution:

- local directory
- local `agent.json`
- installed package reference
- optional `memory:` prefix

Inspect must show:

- package name/version
- manifest path/package root
- scope count and scope names
- record-type count and versions
- spaces and models
- allowed record types per space
- retrieval, capacity, retention, and constraints
- operations and trigger types
- generated contract count
- build freshness
- specific mismatches when stale

Inspect should report stale build state rather than failing solely because generated outputs are stale. Invalid source declarations should still fail.

No `agentpm memory query` command is added.

### Init scaffolding
Add:

```bash
agentpm init --kind memory --name <name>
```

The scaffold should create a valid, useful starter blueprint, for example:

```text
agent.json
README.md
schemas/user-preference.schema.json
```

Starter manifest:

```json
{
  "$schema": "...",
  "kind": "memory",
  "name": "<name>",
  "version": "0.1.0",
  "description": "Describe the durable memory contract this blueprint provides.",
  "memory": {
    "scopes": {
      "user": {
        "description": "The user whose memory is being retained."
      }
    },
    "record_types": {
      "user_preference": {
        "version": "1.0.0",
        "description": "Durable structured preferences for one user.",
        "schema": "schemas/user-preference.schema.json"
      }
    },
    "spaces": {
      "profile": {
        "description": "The current durable profile for one user.",
        "model": "document",
        "record_types": ["user_preference"],
        "scope": ["user"],
        "retrieval": {
          "modes": ["key"]
        }
      }
    }
  },
  "readme": "README.md"
}
```

Init does not automatically run `memory build`.

### Lint behavior
`agentpm lint` must validate authored Memory Blueprint source declarations without requiring generated output.

Lint must include:

- top-level manifest schema validation
- referenced schema path safety and existence
- valid JSON and Draft 2020-12 schema compilation
- supported AgentPM governance keyword validation
- top-level map-key patterns
- scope/record/space/operation cross-references
- model-specific retrieval and constraint rules
- retention and trigger duration validation
- operation source/output compatibility
- duplicate array-entry checks

Lint must not require `memory/build.json` or `memory/contracts/`.

Memory semantic issues should use existing `LintIssue` formatting with useful `instance_path` values rooted under `/memory` and file labels for source schema errors where possible.

### Publish and archive behavior
- Extend `PublishManifest` with `Memory`.
- Add strongly typed Rust manifest models for all Memory Blueprint structures.
- `agentpm publish` must support `kind: "memory"`.
- Publish must run the normal manifest validation and Memory check-mode readiness validation.
- Publish must include source schemas, generated contracts, build metadata, README, license, and other ordinary package files according to existing archive rules.
- Memory packages have no dependency manifest entries.
- Existing archive path safety, signing, integrity, size, and symlink rules remain unchanged.

### Installation and lockfiles
- Add `PackageKind::Memory` anywhere package kinds are modeled.
- Install Memory packages under:

```text
.agentpm/memory/<owner>/<name>/<version>/
```

- Package keys use:

```text
memory:@owner/name@version
```

- Agents resolve top-level `memory` dependencies.
- Templates resolve `template.dependencies.memory`.
- Memory dependencies participate in lockfile roots and package entries using the same conventions as Knowledge.
- Existing reserved memory relationship/root handling must be converted to first-class resolution rather than duplicated.
- Memory packages themselves have no dependency relationship entries beyond an empty/default relationship shape if the lockfile format requires one.
- Existing agents with empty `memory` arrays remain valid.
- Existing lockfiles remain readable according to current compatibility rules.

### SDK behavior
Node SDK:

```ts
loadMemory(...)
```

Python SDK:

```python
load_memory(...)
```

These methods should mirror `loadKnowledge` / `load_knowledge` semantics:

- Resolve an installed Memory Blueprint package.
- Confirm package kind is `memory`.
- Return manifest metadata, package root, build metadata, contract index, and resolved contract file locations or parsed schemas according to existing SDK style.
- Do not load live memory records.
- Do not add store adapters or CRUD methods.

Public SDK types should include Memory Blueprint manifest metadata and generated contract/index/build metadata.

The SDK may expose the canonical logical `MemoryRecord` type for future compatibility, but a runtime record validator is optional and should only be included if it fits existing SDK patterns without introducing substantial new JSON Schema infrastructure.

### Backend and registry
- Add memory to backend package-kind validation, models, serialization, publish routes, install/download flows, private namespace checks, search, and detail APIs.
- Memory packages follow the same public/private namespace access rules as other package kinds.
- During publish finalization, perform server-side structural defense-in-depth validation of the uploaded Memory package after generic artifact validation.
- Backend validation must require and safely parse memory/build.json and memory/contracts/index.json, verify supported metadata formats, require every indexed contract and declared source schema to exist in the archive, reject unsafe or duplicate paths, and enforce bounded file-count and file-size limits.
- Backend validation is not the authoritative freshness check. It must not regenerate resolved contracts, recompute the full Memory build, or duplicate the CLI’s MemoryBuildMode::Check logic.
- Publish/finalize must preserve the uploaded built package unchanged.
- Extract the Memory build metadata, contract index, and indexed resolved contract schemas during publish finalization and persist a bounded registry-facing representation on the package version.
- The uploaded archive remains the canonical package artifact. Persisted Memory metadata and contract schemas are derived presentation/API data used so detail requests do not need to reopen and scan the package archive from object storage.
- Prefer a compact Memory-specific JSONB representation on the package version unless the implementation deliberately introduces a reusable package-file persistence model. Do not add one database column per contract.
- Registry detail responses must expose Memory Blueprint manifest metadata and the generated contract index needed for the frontend Memory Model view.
- Provide an authorized API path for retrieving individual resolved contracts without requiring every contract schema to be included in the base package-detail response.
- Apply normal package visibility and private-namespace authorization to all Memory metadata and contract responses.
- Do not add a live memory API, persistence store, record CRUD operations, trigger execution, retention enforcement, or lifecycle-operation execution.

### Registry UI
Add a Memory Blueprint-specific package detail presentation that reuses existing package-detail layout, typography, cards, badges, tabs, code viewers, and responsive behavior.

Display:

- Memory Blueprint badge/label
- description and README
- scopes with descriptions
- record types with version and source schema path
- spaces with model, scope, accepted record types, retrieval modes, capacity, retention, and constraints
- lifecycle operations and trigger types
- field-level governance annotations discovered in source/resolved schemas
- generated resolved record contracts
- build metadata/freshness where appropriate

Recommended organization:

- Overview
- Memory Model
- Record Contracts
- README

The exact tab structure should follow the existing UI conventions rather than introducing a new design system.

UI rules:

- Clearly distinguish author-defined content fields from AgentPM envelope fields.
- Show governance annotations in readable labels.
- Do not imply that AgentPM currently stores or enforces live memory.
- Do not imply that semantic retrieval selects a specific vector provider.
- Preserve private-package access restrictions.

### Documentation
Add or update:

- Manifest reference for `kind: "memory"`
- Memory Blueprint authoring guide
- `agentpm memory build` reference
- `agentpm memory inspect` reference
- publishing guide explaining required build freshness
- SDK references for `loadMemory` / `load_memory`
- agents and templates dependency documentation
- registry/package-kind documentation
- example Memory Blueprint package
- common memory-record envelope and resolved contract documentation
- governance annotation semantics
- lifecycle operation and trigger semantics
- explicit Phase 6C non-goals and Phase 7 handoff

## Acceptance criteria
- `kind: "memory"` validates in the shared manifest schema and rejects fields/dependencies not allowed for Memory Blueprints.
- A valid Memory Blueprint can declare reusable scopes, versioned record types, three supported space models, retrieval modes, retention, capacity, constraints, governance annotations, and optional lifecycle operations.
- Invalid cross-references fail lint/build with actionable paths and messages.
- Invalid or unsupported `x-agentpm-*` governance annotations fail lint/build.
- `agentpm init --kind memory` creates a valid starter package.
- `agentpm memory build` generates deterministic resolved contracts, index, and build metadata without modifying `agent.json` or source schemas.
- Every explicitly valid space-and-record-type pairing produces exactly one resolved contract.
- Sequence contracts require `ordinal`; document and collection contracts do not include it.
- Generated scope contracts require exactly the scopes declared by the space.
- Generated content contracts preserve source JSON Schema constraints and governance annotations.
- Re-running `agentpm memory build` with unchanged inputs produces byte-identical contract schemas and index; only informational build metadata such as `built_at` may change.
- Removed or renamed pairings do not leave stale generated contract files after build.
- `agentpm publish` fails when a Memory Blueprint has never been built.
- `agentpm publish` fails when the manifest, source schemas, contract index, contract files, or build metadata are stale, missing, unsupported, or manually modified.
- `agentpm publish` never runs Memory build or changes local files.
- `agentpm memory inspect` works for local and installed Memory packages and reports fresh/stale build state in text and JSON modes.
- Agents can publish, install, and lock Memory Blueprint dependencies.
- Templates can publish, generate, install, and lock Memory Blueprint dependencies.
- Memory packages install under `.agentpm/memory/...` and use `memory:` lockfile package keys.
- Existing tool, agent, template, skill, and knowledge workflows remain functional.
- Node and Python SDKs can load installed Memory Blueprint metadata and generated contracts.
- Backend routes and registry pages support public and private Memory packages.
- The registry Memory Model view exposes scopes, record types, spaces, operations, governance, and resolved contracts without implying live runtime enforcement.
- Documentation includes a complete reference blueprint and explains the blueprint/store/harness separation.
- All required automated and manual verification in `test-plan.md` passes.

## Risks / edge cases
- The shared top-level `memory` property is overloaded between agent dependency arrays and Memory Blueprint metadata, so dependent-schema logic must prevent ambiguous or invalid shapes.
- The current agent manifest requires `tools`; Phase 6C should not accidentally change that invariant unless another spec explicitly does so.
- JSON Schema libraries may treat unknown extension keywords permissively, so AgentPM must recursively validate supported `x-agentpm-*` names and values itself.
- Referenced content schemas may contain local `$ref` values. Build must either preserve a valid installed-package-relative reference graph or resolve/bundle schemas safely. The implementation must not produce contracts whose references only work from the author’s original directory.
- Canonical JSON serialization and ordering must be stable across platforms to avoid false stale-build failures.
- Raw `agent.json` hashing detects harmless metadata edits, but this conservative behavior is acceptable and must be documented.
- ISO 8601 duration validation may require a dedicated parser or strict format validation; do not silently accept arbitrary strings.
- Sequence `ordinal` assignment is not implemented in this phase, but the generated contract makes it required. Docs must explain that the runtime owns assignment.
- `archive` retention semantics are intentionally implementation-neutral and may vary across runtimes; docs must avoid promising stronger behavior.
- `x-agentpm-shareable` cannot provide complete access control by itself. It is field-level contract metadata, not a substitute for future space/store/agent authorization bindings.
- A record type allowed in multiple spaces produces multiple contracts. UI and SDK APIs must key by both space and record type.
- Large or complex source schemas may make generated contracts large. Reuse references where safe rather than deeply duplicating every schema if duplication creates unreasonable package growth.
- Circular or remote JSON Schema references may complicate offline build and registry rendering. Remote fetches should not be required for publish; unresolved remote dependencies should fail clearly unless already supported by current schema tooling.
- Private packages must not leak manifest or contract schema details through search, detail, or file endpoints.
- Existing hardcoded package-kind lists are spread across CLI, backend, SDKs, docs, tests, and frontend. Missing one may create partial support.
- Existing reserved `memory` lockfile/root shapes may conflict with first-class dependency resolution if cleanup is incomplete.

## Open questions
- Should resolved contracts embed the content schema directly or use package-relative `$ref` values after validating the full reference graph? Prefer the approach that keeps contracts portable after installation and displayable in the registry.
- Should the SDK return parsed contract schemas, paths to contract schemas, or both? Follow the closest existing `loadKnowledge` style while making contract enumeration straightforward.
- Should the registry API return parsed contract contents directly or expose a package-file endpoint used by the UI? Reuse existing archive/file patterns where possible.
- Should source schemas be required to have their own `$id`? This is not required unless needed to make local `$ref` resolution deterministic.
- Should `readme` be required for Memory Blueprints or remain optional under the global manifest rules? Prefer consistency with other non-template package kinds.
- Should a pure live-record validation helper be included in the SDKs now? It is optional and should be omitted if it would substantially expand scope.

## Related Specs
- Phase 6A: Skills as First-Class Artifacts
- Phase 6B: Knowledge Base & Vector Context Artifacts
- Phase 7: Loop and Harness
- Existing private namespaces and package visibility spec
- Existing workflow templates spec
