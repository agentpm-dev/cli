# AgentPM APDS 1.0.0 semantics

**Identifier:** `agentpm` · **Version:** `1.0.0` · **Status:** normative specification

This document defines what APDS 1.0.0 declarations mean. Start with the [README](README.md) for the artifact kinds and roles; use the [schema](manifest.schema.json) for exact field shapes. The [rule index](conformance/rule-index.md) lists each rule's subjects, evaluation scopes and related [semantic cases](conformance/semantic-cases.md).

## 1. Scope, roles and normative language

An author defines an artifact. A validator checks supplied declarations, dependencies or files. A Manager selects and manages artifacts. A Registry distributes them. A Runner interprets supported declarations during execution. One implementation can perform several roles.

**MUST** and **MUST NOT** express requirements. **SHOULD** and **SHOULD NOT** express recommendations whose departure needs a considered reason. **MAY** expresses a permitted choice. Lowercase uses have their ordinary meaning. Examples and explanations clarify rules; they add no requirements.

### APDS-SCOPE-001

**Role-specific support.** An implementation MUST preserve the rules applicable to the roles and constructs it claims to support. A Registry acting as a validator has validator responsibilities; it need not execute a Runner. Supporting one optional capability does not require implementing unrelated capabilities.

### APDS-SCOPE-002

**One versioned contract.** Authors and validators MUST apply the schema and these semantic rules together for APDS 1.0.0. Normative fixture expectations MUST agree with both. A fixture cannot override a rule. The published normative files and rule meanings MUST remain immutable under this version; changes follow the [versioning policy](README.md#version-100-immutability-and-extensions).

Model/provider selection, prompt assembly, approval interfaces, hooks, storage engines, retrieval algorithms, schedulers, internal retries and trace formats remain implementation choices except where a rule below defines an observable authored guarantee. Unsupported execution constructs affect Runner compatibility, not the validity of an otherwise conformant artifact.

## 2. Artifact identity, versions, namespaces and references

An artifact has its own kind, local name and version. A dependency reference identifies another artifact through a namespace and, optionally, a version selector. These are different identities used for different purposes.

### APDS-ID-001

**Explicit standard.** An author claiming APDS 1.0.0 MUST declare `standard: { "id": "agentpm", "version": "1.0.0" }`. The optional editor `$schema` neither substitutes for that declaration nor selects different semantics. A validator MUST NOT silently substitute a supported version for an unsupported declaration. Historical compatibility handling is outside this normative authoring format.

### APDS-ID-002

**Artifact identity and purpose.** Authors MUST preserve the schema's distinction between local `name`, artifact `version` and declared standard version. A distribution namespace qualifies a reference, not the local manifest name. The root `description` MUST contain non-whitespace text as defined by the schema's explicit Unicode `White_Space` class. `kind: "agent"` identifies an Agent Package; `template` identifies a scaffold. The six other kinds identify Components.

### APDS-ID-003

**Reference forms.** Authors MUST use the schema's package-reference forms: a namespaced string with an optional numeric version selector, or an object containing `name` and optional `version`. A two-component selector such as `1.2` selects that major/minor line; a complete version selects that version. Omission leaves the version unconstrained. Literal `latest`, `*`, caret, tilde and comparison-range syntax are not additional APDS 1.0.0 manifest forms. The field containing a reference determines its required artifact kind.

### APDS-ID-004

**Content paths.** A path declared as artifact-relative MUST identify content inside its containing artifact using the schema's relative-path syntax. Implementations reading supplied content MUST reject an escape from that boundary, including an escape through a resolved link. Consumer-owned context paths are relative to the consuming project instead; they are not paths into a dependency package. Missing inspection context is handled by [APDS-CONF-002](#apds-conf-002).

## 3. Dependencies and composition

A dependency states what the artifact relies on. A binding states where a declared Component participates. Installing or resolving a dependency does not itself bind it or invoke it.

### APDS-GRAPH-001

**Allowed relationships.** Dependencies MUST follow their kind-specific schema. An Agent may declare any of the six Component families, including a single Loop. A Skill may declare Tools. A Template's dependency groups may declare Agents and Components. Knowledge, Memory, Profile and Loop artifacts have no package dependency declarations. The top-level `profile` object is Profile metadata; Agent Profile dependencies use plural `profiles`.

An Agent Package MAY omit all dependencies, bindings and its Loop. A validator MUST NOT require them merely because a particular Runner needs them to execute an Agent.

### APDS-GRAPH-002

**Selected versions.** A Manager supplying a resolved composition MUST identify the selected artifact versions and preserve each declaring artifact's dependency edges and constraints. References MUST resolve to the intended kind, namespace, name and selected version in their owning composition, not an arbitrary installed artifact with the same name. An unavailable graph leaves dependent checks unevaluated; a supplied graph selecting an incompatible kind or version establishes a violation.

No particular resolver, lockfile, integrity algorithm or installation layout is required. Different versions may occur in different dependency contexts; they are not interchangeable capabilities.

### APDS-GRAPH-003

**Unambiguous references.** An author MUST NOT declare duplicate identities within one dependency collection, including equivalent string/object references. Versionless bindings MUST resolve unambiguously through the containing Agent's selected composition. A Manager or Runner MUST NOT conceal a version ambiguity by merging distinct selected artifacts. Direct and transitive paths remain distinguishable even when they reach the same artifact.

## 4. Agent bindings and effective capabilities

Global bindings apply throughout a run. Phase bindings add declarations for their named phase. The active Loop can restrict which resulting capabilities may be used.

### APDS-BIND-001

**Declared membership.** Each explicit Tool, Skill, Knowledge, Memory or Profile binding MUST name an Agent dependency from the matching top-level collection. Bindings use versionless package identities; they do not introduce dependencies. Memory selectors MUST identify spaces or operations in the selected Blueprint when that Blueprint is available for inspection. Inherited Skill Tools follow [APDS-SKILL-002](#apds-skill-002), without requiring duplicate Agent Tool declarations.

### APDS-BIND-002

**Phase names.** An Agent with phase bindings MUST declare a Loop dependency. Every binding phase MUST exist in that resolved Loop. Absence of the dependency declaration is a standalone defect; an unknown phase in a supplied Loop is a resolved-composition defect. A validator without the Loop cannot claim to have checked its phase names.

### APDS-BIND-003

**Additive scopes.** A Runner MUST combine applicable global and active-phase bindings. A phase binding MUST NOT replace or remove a global binding. For the same Memory Blueprint, applicable space and operation selectors combine by set union. Other phases receive only their own phase bindings plus global bindings and independently applicable inheritance.

### APDS-BIND-004

**Eligibility is not invocation.** Applicable bindings and inherited capabilities are candidates subject to Loop access rules, execution authorization and runtime support. A Runner MUST NOT treat a binding as automatic invocation or permission to bypass those restrictions. It MUST distinguish an unavailable implementation or environment from an invalid declaration.

### APDS-BIND-005

**MCP and consumer context.** Each `bindings.mcp` entry MUST have a unique ID and select Tool identities declared by the Agent. It identifies a named Model Context Protocol (MCP) Tool surface; it does not authorize unrelated capabilities or bypass Loop Tool restrictions. A Runner interpreting imported or exposed Tools MUST preserve those restrictions. Connection, discovery, transport and authentication mechanisms remain implementation-defined.

`bindings.consumer_context.file` identifies consumer-owned contextual input. A Runner MUST NOT interpret its contents as an additional capability grant. How it supplies that context to a model remains implementation-defined.

**Binding example** — the Agent declares `@example/review-skill`, `@example/publish`, and a Loop containing `draft`. This is the value of its `bindings` object:

```json
{
  "global": { "skills": ["@example/review-skill"] },
  "phases": { "draft": { "tools": ["@example/publish"] } }
}
```

If the Skill declares a Tool, that Tool is applicable globally. The direct publish Tool is additionally applicable in `draft`. [Cases A–D](conformance/semantic-cases.md) distinguish applicability, restrictions and invalid phase references.

## 5. Skills and inherited Tools

A Skill packages procedural instructions and supporting resources. Tools supply executable capabilities. Their relationship is a dependency and scope relationship, not an implicit script invocation.

### APDS-SKILL-001

**Skill resources.** An author MUST identify the main instructions through `skill.entrypoint`; supplied complete content MUST contain the declared entrypoint, references and scripts at their safe relative paths. `skill.compatibility` labels are descriptive compatibility hints. A Runner MUST NOT treat binding a Skill, or finding a script in it, as a command or execution privilege.

### APDS-SKILL-002

**Tool inheritance.** A Runner supporting a bound Skill MUST make that Skill's declared Tool dependencies capability candidates in the Skill's binding scope. A global Skill contributes them globally; a phase Skill contributes them only in that phase unless another applicable path contributes them elsewhere. No duplicate direct Tool dependency or binding is required solely for inheritance. Loop restrictions still constrain inherited Tools.

### APDS-SKILL-003

**Repeated paths.** Multiple applicable paths to the same resolved Tool MUST preserve its identity and MUST NOT create extra permissions. Each path contributes only within its own scope. A narrower path does not remove a broader independent path. Distinct selected versions MUST NOT be collapsed merely because names match. Internal de-duplication and presentation are implementation choices.

### Tool execution contracts

#### APDS-TOOL-001

**Process invocation.** For a Tool process, a compatible Runner MUST supply one input JSON value on stdin that satisfies `inputs`. A successful invocation requires successful process termination, one JSON value on stdout, and satisfaction of `outputs`. Non-JSON or log-prefixed stdout and unsuccessful process termination are invocation failures, even if some output looks valid. Diagnostics belong on stderr; diagnostic text alone does not make an otherwise successful invocation fail.

A schema-valid application result such as `{ "ok": false }` remains result data if the process succeeds. Host request/result envelopes are not part of the Tool's JSON payload. Runtime services may realize invocation differently while preserving these declared input/output and success distinctions; APDS does not prescribe a host wire protocol or diagnostic codes.

The JSON-only output rule describes APDS-conforming Tool execution. An implementation MAY retain an explicitly identified legacy compatibility adapter that accepts historical output, but MUST NOT report that relaxed acceptance as compliance with this strict Tool-output contract.

#### APDS-TOOL-002

**Entrypoint and runtime.** A Runner interpreting a Tool MUST preserve its declared command, argument order, working-directory meaning and configured timeout. A relative working directory is relative to the Tool's content root. A declared `runtime` identifies the interpreter family and minimum version; it MUST agree with the declared interpreter. Python `runtime.dependencies` are distribution requirements, not Component references, and MUST NOT be interpreted as package bindings or Node dependencies. A Runner unable to realize a declared entrypoint setting MUST report that execution limitation rather than silently reinterpret the value. An unavailable interpreter or required runtime service is a readiness failure, not an intrinsic artifact defect.

#### APDS-TOOL-003

**Environment precedence.** For ordinary Tool variables, a Runner MUST start with ambient environment values, fill absent keys from `environment.vars` defaults, overlay `entrypoint.env`, then overlay caller overrides. An existing empty value is present; defaults do not replace it. Required declared variables MUST be present in the resulting invocation environment. Runtime-managed interpreter variables may be reserved by the execution environment; this ordering does not grant permission to override runtime isolation. On non-Tool kinds the root `environment` field alone introduces no process invocation behavior.

## 6. Loops and authored orchestration

A Loop describes control flow: where execution starts, which phase outcomes exist and where each outcome leads. Array position does not imply execution order. `archetype` is a descriptive label, not an alternate control-flow algorithm.

### APDS-LOOP-001

**Phases and outcomes.** Phase IDs MUST be unique, `entry_phase` MUST identify a declared phase, and phase objectives MUST contain non-whitespace text. Explicit outcomes MUST have unique IDs within their phase and nonblank descriptions. Omitted outcomes mean exactly one implicit outcome, `complete`. An explicit outcome list replaces that implicit set; `complete` is available only if explicitly listed. Runners MUST preserve the outcome identifiers, while choosing their own completion mechanism.

### APDS-LOOP-002

**Deterministic graph.** Each transition MUST name a declared source phase, a valid outcome for that source, and a declared destination phase or one of `$end`, `$abort`, `$handoff`. Every valid phase/outcome pair MUST have exactly one transition. All declared phases MUST be reachable from `entry_phase` through outcome transitions, and at least one terminal MUST be reachable. Branches and cycles MAY occur; this reachability requirement does not assert that every execution terminates. A Runner MUST NOT invent fallback transitions for malformed control flow.

### APDS-LOOP-003

**Terminals.** A Runner MUST preserve `$end` as normal termination, `$abort` as abort termination and `$handoff` as yielding control rather than continuing this Loop. It may choose its own result representation and handoff mechanism.

### APDS-LOOP-004

**Step limits.** One step is one execution of one Loop phase, including a revisit through a cycle. Tool calls, model turns and Tool retries within that execution are not additional Loop steps. With `limits.max_steps` present, a Runner MUST NOT begin another phase execution once that many steps have executed. A checkpoint wait or rejected entry does not execute the guarded phase. A Runner may impose stricter operational limits; exhaustion does not authorize exceeding the authored bound or fabricate a successful phase outcome.

### APDS-LOOP-005

**Approval checkpoints.** Checkpoint IDs MUST be unique. Each `before_phase` and `on_reject` MUST identify a declared phase or, for rejection, a supported terminal. Before entering a guarded phase, a Runner supporting approval checkpoints MUST obtain approval. Multiple checkpoints for that phase apply in authored array order. Approval continues toward phase entry; rejection transfers to `on_reject`. A pending or unsupported approval MUST NOT be treated as approval. The approval interface and waiting mechanism remain implementation choices.

### APDS-LOOP-006

**Explicit error policy.** A Runner supporting the declared policy MUST interpret Tool failure actions as follows: `retry` permits at most `max_retries` additional attempts, then applies `on_exhausted`; `fail_phase` delegates to the declared phase-failure policy; `abort` and `handoff` preserve the corresponding terminal distinction. Retry requires a positive `max_retries` and `on_exhausted`; other actions MUST omit both. A policy capable of `fail_phase` MUST declare `phase_failure`, whose actions are `abort` or `handoff`. Omitted policies leave recovery to the Runner; they do not add an implicit authored retry or transition. Exception classification and backoff remain implementation choices.

### APDS-LOOP-007

**Access policy.** A Runner MUST interpret explicit `false` in phase `access.tools` or `access.knowledge` as a prohibition of Tool or Knowledge use, and in `access.memory.read`/`write` as a prohibition of direct Agent Memory reads/writes. `true` permits the corresponding activity under the Loop policy but grants no unbound capability; omission expresses no Loop opinion. Tool restrictions apply to direct, inherited and applicable MCP Tool capabilities. Knowledge permission does not request retrieval, and Memory write permission does not request a write. Separately bound lifecycle operations have the authority described in [APDS-MEM-011](#apds-mem-011).

**Loop example** — this complete `loop` value uses an implicit outcome and an explicit terminal transition:

```json
{
  "entry_phase": "review",
  "phases": [
    {
      "id": "review",
      "objective": "Review the supplied evidence.",
      "access": { "tools": false }
    }
  ],
  "transitions": [{ "from": "review", "on": "complete", "to": "$end" }]
}
```

The transition is valid because omitting `outcomes` supplies `complete`. An inherited Tool still cannot be used in this phase. See [cases B and O](conformance/semantic-cases.md).

## 7. Instruction Profiles

A Profile describes intended behavior. It can guide an Agent's identity, priorities and communication without granting capabilities or controlling a Loop's transitions.

### APDS-PROFILE-001

**Authored guidance.** A Runner supporting Profiles MUST preserve the distinct meanings of `identity` (role, description, expertise), `objectives` and `principles` (durable priorities), `audience` (intended readers and adaptation), `communication` (tone, verbosity, guidelines, formatting and vocabulary), and `boundaries` (limits on intended behavior or authority). These declarations do not replace the Agent's purpose or a phase's objective.

Constraint IDs MUST be unique within a Profile. A Runner MUST preserve the stronger authored obligation of `strength: "required"` relative to `preferred`. These are behavioral instructions, not permissions or a guarantee that a model always obeys them.

### APDS-PROFILE-002

**Additive guidance.** Global and phase Profiles MUST contribute within their applicable binding scopes under [APDS-BIND-003](#apds-bind-003). Repeated references to the same selected Profile MUST preserve its identity and applicability. No precedence follows solely from phase scope, declaration order or prompt placement. Combining Profiles does not define universal natural-language conflict resolution, prompt concatenation order or a de-duplication algorithm.

### APDS-PROFILE-003

**Compatibility expectations.** A consumer interpreting `compatibility` MUST distinguish the author's necessary capability hints in `requires` from beneficial hints in `recommends`, and preserve `minimum_context_tokens` as a context-capacity expectation. A `true` capability hint asserts that expectation; `false` or omission does not assert it and is not a prohibition. Hints such as `tool_use`, `structured_output` and `multimodal_input` MUST NOT grant capabilities, override Loop prohibitions or make a conformant Profile invalid merely because the chosen Runner lacks them. Consumers may use them for compatibility decisions or warnings.

## 8. Knowledge

Knowledge supplies authored context or prepared retrieval data. Its metadata describes that content and how it is intended to be used; a declaration alone does not verify its accuracy or trustworthiness.

### APDS-KNOW-001

**Modes and content.** Authors and consumers MUST preserve `context` as direct contextual material and `vector` as a prepared retrieval corpus with associated embeddings. Context-mode `documents` identify supplied material; `content_type` and `language` describe it, and `context` holds build-derived aggregate metadata. Vector mode MUST include the schema-required corpus paths and embedding declaration; retrieval indexes remain optional. Chunking metadata describes preparation, not a required query algorithm. A Runner need not inject an entire context corpus into a prompt.

### APDS-KNOW-002

**Packaged relationships.** In supplied complete content, declared documents, corpus, vector, index and provenance paths MUST resolve within the artifact. Declared counts and verifiable content relationships MUST agree with their payloads. A validator MUST distinguish checking metadata shape from inspecting actual files. A hash can only be verified when its represented bytes and hash convention are known; APDS does not invent an unspecified aggregate-hash algorithm or file layout from a string label.

### APDS-KNOW-003

**Embeddings and indexes.** Consumers MUST preserve embedding identity, provider/model labels, dimensions, metric, normalization state and vector payload association. Declared indexes MUST reference the intended embedding ID; repeated index IDs MUST NOT create an ambiguous association. `agentpm-local` identifies the schema's supported packaged index type, not a requirement that every Runner implement it. A Runner MUST NOT silently use incompatible embedding or index data as if compatible. Unsupported representation or query-time capabilities are compatibility limits, not proof of artifact nonconformance.

### APDS-KNOW-004

**Retrieval preferences.** Consumers interpreting retrieval metadata MUST preserve `default_top_k` as the default result count, `default_score_threshold` as a filtering hint and `return_citations` as the default citation preference. `strategy` is an open-ended descriptive label, not an implicit algorithm specification. Explicit consumer choices may override defaults. These declarations do not fix ranking, score normalization, citation rendering or an embedding provider.

### APDS-KNOW-005

**Provenance and scope.** A consumer MUST distinguish declared `sources_manifest_path`, `generated_at` and builder metadata from independently verified provenance. Binding scope and Loop Knowledge restrictions govern availability under [APDS-BIND-003](#apds-bind-003) and [APDS-LOOP-007](#apds-loop-007). Neither validity nor a binding proves content accuracy, retrieval support or trust.

## 9. Memory Blueprints, spaces and lifecycle operations

A Blueprint describes durable memory, not a live store. Spaces expose logical records; separately bound operations perform lifecycle work under declared inputs, outputs and policies. Record data belongs to runtime state, not the authored Blueprint.

### APDS-MEM-001

**Scopes and record contracts.** Space `scope` keys and `record_types` MUST refer to declarations in the Blueprint. Their ordered scope tuple identifies the intended logical partition; values come from the consumer. A record type's version identifies its content contract independently of the Blueprint and APDS versions. An operation's space/record-type pair MUST be a declared, permitted pairing, and trigger/delete references MUST identify declared spaces.

### APDS-MEM-002

**Space models.** Consumers MUST preserve these logical distinctions: `document` has one current record for each complete space/scope tuple; `collection` permits multiple unordered records distinguished by ID; `sequence` permits multiple ordered records distinguished by ID and ordered by ordinal. A sequence MUST retain monotonically increasing ordinals within that space/scope identity. Physical keys, concurrency and ordinal assignment mechanisms are not fixed.

### APDS-MEM-003

**Retrieval.** A document space MUST include `key` retrieval; a sequence MUST include `chronological`. A collection needs at least one declared mode. Implementations claiming support MUST preserve `key` lookup, `filter` selection, `chronological` ordering, `full_text` text retrieval and `semantic` meaning-based retrieval as distinct capabilities. Unsupported modes MUST be reported as capability limits rather than invalidating the authored Blueprint. No query language, embedding model, ranking algorithm or response API is required.

### APDS-MEM-004

**Capacity, retention and durations.** `capacity.max_records` declares the maximum active-record count within a complete space/scope tuple. A consumer claiming to enforce it MUST preserve that bound rather than silently treating it as a preference. `retention.ttl` declares active lifetime; after expiration `delete` removes active availability, while `archive` removes active retrieval eligibility and permits implementation-specific archival retention.

TTL and interval declarations MUST use positive elapsed ISO 8601 durations: a positive integer week component, or positive integer day/hour/minute/second components, with at least one component. Calendar years/months and fractional or zero components are not supported by this contract. Examples are `P1W`, `P7D` and `PT30M`. Cleanup timing, clock storage and archival technology are implementation choices, not new fields or promises of immediate physical erasure.

### APDS-MEM-005

**Append-only.** `constraints.append_only: true` MUST occur only on a collection or sequence. A consumer enforcing it MUST reject ordinary direct update/delete rather than modifying existing records through that surface. Explicit retention and authored lifecycle operations retain their separate mutation authority under [APDS-MEM-010](#apds-mem-010); append-only does not prohibit those declared operations. A `document` space is incompatible with append-only because it defines a current mutable logical document.

### APDS-MEM-006

**Packaged record schemas.** A complete Blueprint MUST supply valid Draft 2020-12 JSON Schemas at its declared record-schema paths. Source `$ref` values MUST remain in-document `#...` references; they do not authorize arbitrary remote fetching. Applicable record content MUST satisfy its selected record-type schema. A validator inspecting only the manifest MUST leave source-file checks unevaluated. A source schema MUST NOT directly require a property that it marks non-persistable. More complex schema combinations still require evaluation against the actual constraints, not a claim based on a shallow scan.

### APDS-MEM-007

**Operation kinds.** A `consolidate` operation derives output from one or more source pairings; `transform` converts exactly one source pairing into its declared output; `delete` targets its declared spaces. Authors MUST preserve their type-specific fields and valid references. A Runner MUST NOT reinterpret these declarations as arbitrary code, Tool bindings or unrestricted CRUD privileges. Record processing techniques remain implementation choices.

### APDS-MEM-008

**Triggers.** A Runner interpreting an operation MUST preserve the distinction between its trigger condition and execution:

| Trigger        | Eligibility meaning                                                                      |
| -------------- | ---------------------------------------------------------------------------------------- |
| `external`     | Requires an explicit invocation; declaring or binding it does not schedule or execute it |
| `interval`     | Recurring eligibility based on the baseline and successful completion described below    |
| `record_count` | An edge from below the positive `threshold` to that active scoped count or higher        |
| `capacity`     | An edge reaching the named space's declared positive `capacity.max_records`              |

A capacity trigger MUST identify a space with capacity metadata. For each selected Blueprint operation and resolved scope, count/capacity triggers start armed; an initial observation at or above the threshold satisfies the count condition. Successful execution consumes that edge. A Runner MUST NOT repeatedly fire successful operations merely because an unchanged count remains at or above the threshold; it MUST rearm after observing a count below it. Failure retry policy is separate from rearming.

An interval trigger MUST remain dormant until relevant scoped input/target state exists. Its initial baseline starts with that state; first eligibility is baseline plus `every`. After successful execution, next eligibility is completion time plus `every`. Re-evaluating a trigger or re-entering a phase MUST NOT itself rearm a consumed edge or reset an established interval baseline. `every` is a minimum elapsed interval, not a promised execution cadence.

Eligibility still requires operation participation in the applicable binding scope and does not guarantee immediate execution, success or backend availability. Polling, yield points, state storage and failure cooldowns remain Runner choices; they must preserve these eligibility guarantees.

### APDS-MEM-009

**Space access and operation participation.** A Runner MUST distinguish direct space bindings from operation bindings. Global and phase bindings combine additively for each. A phase-only operation does not participate elsewhere merely because its trigger is eligible. An eligible and authorized operation MAY use its declared inputs, output or target spaces without directly binding those spaces to the Agent. Those operation targets MUST NOT thereby become directly exposed Agent Memory surfaces. An operation-only binding is valid.

### APDS-MEM-010

**Successful output and source handling.** A Runner supporting lifecycle operations MUST preserve their declared policies. Successful consolidate/transform completion establishes a valid output under its destination record contract before applying success-dependent source handling. `retain` keeps sources, `retain_until_expiration` keeps them under their declared retention, and `delete_after_success` deletes them only after successful output. Inputs using `retain_until_expiration` MUST have retention metadata.

Transform `output_mode` defaults to `create`, producing a new record. `replace_input` MUST target the same space and record type as its one input and use `source_handling: "retain"`; it updates that logical record rather than creating another. This is explicit lifecycle authority even on a space that is append-only for direct access, subject to [APDS-MEM-011](#apds-mem-011). `preserve_provenance: true` requires retention of the source relationship in derived output; `false` does not require that relationship. Delete `cascade_derived_records` determines whether its intended deletion scope includes derived records. This does not fix transaction engines, retry scheduling or provenance storage.

### APDS-MEM-011

**Direct access and lifecycle operations.** Loop `access.memory.read` and `access.memory.write` restrictions govern direct Agent access to Memory spaces. A Runner MUST NOT treat those direct-access restrictions as automatically prohibiting separately bound lifecycle operations from accessing their declared inputs, outputs or targets. Operations remain subject to their binding scope, triggers, Blueprint policies, runtime authorization and backend support. An operation MUST NOT make its target spaces directly accessible to the Agent, and no direct binding of every target space is required.

### APDS-MEM-012

**Governance annotations.** Record-schema authors MUST use the supported `x-agentpm-*` names and values; unknown names in that vocabulary are invalid. `x-agentpm-data-class` categorizes data as `public`, `internal`, `personal`, `authentication`, `financial`, `health`, `legal`, `operational` or `other`. `x-agentpm-sensitivity` labels severity as `low`, `moderate`, `high` or `critical`; these classifications alone do not imply access grants or a particular encryption policy.

`x-agentpm-persist` and `x-agentpm-shareable` are booleans. A consumer claiming support MUST NOT durably persist a value marked `x-agentpm-persist: false`, or disclose a value marked `x-agentpm-shareable: false` across Agent, user, tenant, organization, export or synchronization boundaries. A `true` value does not override other restrictions. These annotations do not certify privacy, legal compliance or security; a consumer unable to honor a restriction must identify that capability limit rather than treating it as permission.

### APDS-MEM-013

**Logical record mapping.** A Memory implementation claiming record-contract compatibility MUST be able to map records to the logical envelope below. Physical storage need not use this representation.

| Field                                    | Logical meaning                                                                |
| ---------------------------------------- | ------------------------------------------------------------------------------ |
| `id`                                     | Nonempty record identifier                                                     |
| `record_type`, `space`, `schema_version` | The declared pairing and record-type version                                   |
| `scope`                                  | Exactly the space's declared keys, with nonempty string values                 |
| `created_at`                             | RFC 3339 creation timestamp                                                    |
| `content`                                | Content satisfying the selected record schema                                  |
| `ordinal`                                | Nonnegative sequence position; required for sequences, absent for other models |
| `updated_at`, `expires_at`               | Optional RFC 3339 update and expiration timestamps                             |
| `provenance`                             | Optional source-record IDs and declared operation identity                     |

Source-record IDs, when supplied, MUST be unique nonempty strings; an operation identity MUST refer to a declared Blueprint operation. Source IDs may survive deletion of the referenced records. Blueprint package/version identity comes from the containing composition or store binding rather than a mandatory repeated envelope field.

**Operation example** — this is a complete transform declaration. Assume declared space `notes` permits record type `note`, and `summary` permits `summary`:

```json
{
  "type": "transform",
  "description": "Prepare a summary from a note.",
  "trigger": { "type": "external" },
  "inputs": [{ "space": "notes", "record_type": "note" }],
  "output": { "space": "summary", "record_type": "summary" },
  "source_handling": "retain",
  "preserve_provenance": true
}
```

A phase can bind only this operation, with no direct spaces. Explicit invocation and operation authorization are still required. A prohibition on direct Agent Memory writes does not itself prohibit the operation's declared output write. See [cases F, G, L and M](conformance/semantic-cases.md).

## 10. Templates and project scaffolding

A Template describes a generated project. The Template, that project and its referenced Agents or Components keep separate identities.

### APDS-TEMPLATE-001

**Files and variables.** `files_root` MUST identify the safe artifact-relative directory supplying scaffold content. A compatible instantiator MUST preserve the intended files and declared `{{ variable_name }}` substitutions without allowing paths to escape their permitted roots. Variable names MUST be unique. Explicit generation values override defaults; unresolved required variables or declared placeholders needed by the generated content MUST NOT be silently treated as successfully resolved. Unrelated content is not a substitution instruction. Variables are non-secret configuration, not fields for publishing runtime credentials. Prompting, destination naming, copying and overwrite interfaces are implementation choices.

### APDS-TEMPLATE-002

**Dependencies and entrypoints.** A consumer MUST preserve the identities, kinds and version constraints in Template dependency groups. They describe the generated workspace's intended composition. `entrypoints` label suggested commands for that project; declaring or instantiating a Template MUST NOT itself be interpreted as authorization to execute them. Successful generation does not establish successful installation or command execution.

### APDS-TEMPLATE-003

**Execution surfaces.** A consumer interpreting an execution-surface identifier MUST preserve its authored target:

| Identifier              | Intended integration                               |
| ----------------------- | -------------------------------------------------- |
| `python-sdk`            | A Python SDK consumer application                  |
| `node-sdk`              | A Node SDK consumer application                    |
| `agentpm-run`           | Direct Tool invocation through AgentPM             |
| `agentpm-harness`       | Agent execution through AgentPM's built-in Runner  |
| `agentpm-serve-mcp`     | Serving Tool capabilities over MCP through AgentPM |
| `multi-agent-workspace` | A workspace composing multiple Agents              |

`display_name` and `use_case` describe purpose; `stack` labels project technologies. These are compatibility intentions, not evidence that dependencies, services, credentials or runtime support exist. Supporting APDS does not oblige an implementation to provide every listed integration or a particular generation command.

## 11. Conformance, interpretation and compatibility

Conformance concerns an artifact and its applicable authored relationships. A validation result records what one evaluation established from the supplied context. These are related but different claims.

### APDS-CONF-001

**Evaluation scopes.** Validators MUST identify which applicable requirements they evaluated:

| Scope                  | Available evidence                                                   |
| ---------------------- | -------------------------------------------------------------------- |
| Standalone (intrinsic) | The manifest's shape and local semantic relationships                |
| Resolved-composition   | Selected dependency manifests and their cross-artifact relationships |
| Packaged-content       | Referenced files or generated artifacts supplied for inspection      |

Runner interpretation checks establish observable behavior for supported constructs; they are not a requirement that a manifest validator execute Tools, lifecycle operations, model calls or a Runner. Passing one scope MUST NOT be presented as passing another unevaluated scope.

### APDS-CONF-002

**Absent context versus absent content.** A validator MUST distinguish unavailable dependencies/files from definite authored defects. Requirements needing unavailable inspection context remain unevaluated. When an otherwise complete supplied artifact lacks required content, or an available graph establishes an invalid reference, the corresponding requirement is violated. A definite violation remains a violation even if other checks are unevaluated.

### APDS-CONF-003

**Logical outcomes.** An implementation MUST preserve these distinctions in its claims:

| Outcome                          | Meaning                                                                                       |
| -------------------------------- | --------------------------------------------------------------------------------------------- |
| Conformant within a stated scope | All applicable requirements in that completed scope were satisfied                            |
| Nonconformant                    | At least one definite applicable violation was established                                    |
| Incomplete / not fully evaluated | Needed evidence is unavailable, so remaining requirements cannot be decided                   |
| Unsupported                      | The declared standard/version, evaluation scope or interpretation capability is not supported |

Unsupported or incomplete evaluation MUST NOT masquerade as successful verification or, by itself, as a definite semantic violation. A supported declaration alone establishes no conformance. Implementations may support several versions, but MUST interpret each using its declared contract, not substitute another version.

### APDS-CONF-004

**Independent evidence.** A consumer MUST distinguish artifact conformance, distribution trust, Runner compatibility and local execution readiness. Signatures, attestations and scanning provide their own evidence; publication or installation alone does not prove APDS conformance. Missing credentials, services or a supported backend do not retroactively invalidate an otherwise conformant artifact.

### APDS-CONF-005

**Truthful reporting.** Implementations MUST limit support and verification claims to their actual roles, capabilities and completed checks. Findings SHOULD identify the standard version, evaluated scope, affected declaration and stable rule ID so a developer can locate the requirement. APDS does not require a particular enum, API result shape, error wording, UI, certification service or internal validator architecture. Missing-standard compatibility interpretations MUST remain distinguishable from explicit APDS 1.0.0 conformance and MUST NOT fabricate historical validation evidence.
