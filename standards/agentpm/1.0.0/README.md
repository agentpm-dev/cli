# AgentPM APDS 1.0.0

Agent Package Definition Standard

**Identifier:** `agentpm` · **Version:** `1.0.0` · **Status:** normative specification

AgentPM's Agent Package Definition Standard (APDS) **1.0.0** defines how Agent Packages and their reusable Components are described, composed and interpreted.

An Agent Package is a complete, authored Agent system. It can combine Tools, Skills, Knowledge, Memory Blueprints, Instruction Profiles and a Loop in one reusable definition.

For example, a document-review Agent Package might combine a Skill containing review instructions, a Knowledge package with reference material and a Loop defining the review workflow. APDS 1.0.0 defines how those pieces relate across compatible Runners.

APDS 1.0.0 gives developers a common contract for building, validating and running compatible Agent Packages without requiring a particular Registry, framework or Runner.

A structural pass alone does not establish full conformance to APDS 1.0.0. The schema, semantic rules and applicable conformance expectations together define this version.

## Documents and precedence

| Document                                                                     | Role                                                            |
| ---------------------------------------------------------------------------- | --------------------------------------------------------------- |
| [manifest.schema.json](manifest.schema.json)                                 | Defines valid manifest structure                                |
| [semantics.md](semantics.md)                                                 | Defines what declarations mean and how they must be interpreted |
| [conformance/rule-index.md](conformance/rule-index.md)                       | Locates semantic rules by subject and evaluation scope          |
| [conformance/semantic-cases.md](conformance/semantic-cases.md)               | Illustrates composition and Runner interpretation expectations  |
| [conformance/README.md](conformance/README.md)                               | Explains validation and conformance tests                       |
| [conformance/fixture-index.json](conformance/fixture-index.json)             | Supplies validation inputs, context and expected results        |
| [conformance/runner-observations.json](conformance/runner-observations.json) | Supplies separate Manager/Runner behavior scenarios             |
| [conformance/fixtures/valid/](conformance/fixtures/valid/)                   | Expected structurally valid manifests for all eight kinds       |
| [conformance/structural-cases.json](conformance/structural-cases.json)       | Expected acceptance and rejection of description edge cases     |
| [checksums.json](checksums.json)                                             | Identifies the versioned files and their checksums              |

The schema and semantic rules apply together. Fixtures illustrate expected outcomes; they do not override either document. Resolve conflicts explicitly before release. Examples and implementation guidance do not introduce additional normative requirements.

To start implementing structural validation:

1. Obtain the [versioned schema](manifest.schema.json) from this bundle and retain a trusted local copy.
2. Validate manifests with a JSON Schema **Draft 2020-12** validator using that copy.
3. Check that the nine [valid fixtures](conformance/fixtures/valid/) pass, including the minimal Agent with no Components.
4. Apply the description cases in [structural-cases.json](conformance/structural-cases.json) and compare results with each case's expected outcome.

Then follow the [conformance quickstart](conformance/README.md#run-the-corpus-checks) and use [semantics.md](semantics.md) with the shared [fixture index](conformance/fixture-index.json) for checks beyond structure.

## Terminology and roles

| Term                    | Manifest `kind` | Meaning                                                                             |
| ----------------------- | --------------- | ----------------------------------------------------------------------------------- |
| **Agent Package**       | `agent`         | A versioned, authored Agent system                                                  |
| **Component**           | —               | An independently reusable artifact that an Agent Package can compose                |
| **Tool**                | `tool`          | An executable capability with declared inputs and outputs                           |
| **Skill**               | `skill`         | Procedural know-how for a task, with optional Tool dependencies                     |
| **Knowledge**           | `knowledge`     | Authored context or prepared retrieval data for an Agent                            |
| **Memory Blueprint**    | `memory`        | A declaration of durable memory structures, record formats, policies and operations |
| **Instruction Profile** | `profile`       | Reusable identity, behavioral guidance, communication style and constraints         |
| **Loop**                | `loop`          | A declarative workflow of phases, outcomes and transitions                          |
| **Template**            | `template`      | A scaffold for creating a project or workspace                                      |
| **Manager**             | —               | Software that discovers, installs and manages packages and their dependencies       |
| **Registry**            | —               | A service that distributes package versions and associated metadata                 |
| **Runner**              | —               | Software that interprets supported declarations and executes an Agent or capability |

Authors produce declarations. Validators evaluate the supplied manifest, composition or content. Managers, Registries and Runners may perform validation in their own workflows, but their roles and the evidence available to them differ.

## Artifact kinds and names

All eight artifact kinds use `agent.json`. Only `agent` is an Agent Package. The six Component kinds remain independently usable; Templates remain scaffolds.

The top-level `name` is local and unscoped, such as `document-reviewer`. Dependency references use namespaced identities, such as `@example/search`.

An Agent Package may omit all Components. Conformance does not establish whether a particular Runner can execute it.

## Declaring the standard

A producer declares the intended contract through `standard`:

```json
{
  "kind": "agent",
  "name": "document-reviewer",
  "version": "1.0.0",
  "description": "Review documents and explain proposed changes.",
  "standard": { "id": "agentpm", "version": "1.0.0" }
}
```

The top-level `version` identifies the artifact version. `standard.id` and `standard.version` select the APDS 1.0.0 contract.

An optional `$schema` URI helps editors locate the structural schema; it is not a standard declaration and cannot override `standard`. A declaration is also not proof that anyone validated the package. Compatibility handling for older manifests without `standard` belongs to implementations, not to a second APDS authoring format.

## Evaluation levels and evidence

- **Standalone (intrinsic) validation:** check the supplied manifest's structure and local relationships without resolving external dependencies.
- **Resolved-composition validation:** check relationships against the actual selected dependency manifests.
- **Packaged-content validation:** check supplied referenced files and their declared relationships.
- **Runner and environment:** check supported capabilities, runtime availability, credentials and local readiness. These checks are **not conformance to APDS 1.0.0**.

When dependencies or packaged files aren't available for inspection, the corresponding checks remain unevaluated. Missing required content from an otherwise complete artifact may be a conformance violation. Report what was checked and what remains unknown. Signing, scanning, Runner compatibility and readiness require separate evidence.

## Registry and Runner independence

APDS 1.0.0 is independent of a particular Registry, Manager or Runner. Implementers can consume the contract and fixtures without using AgentPM's CLI, service, lockfile or built-in Runner. A Registry can validate a package without executing it; a Runner can interpret a supported package obtained through another distribution mechanism.

## Boundaries and implementation support

APDS 1.0.0 does not choose a model provider, prompt format, storage backend, approval interface, deployment platform or orchestration framework. It does not require a Registry to run Tools, Loops or Memory operations during validation.

An implementation claiming support for APDS 1.0.0 should identify this version, its role, the evaluation levels it provides and any unsupported capabilities. It must not present proprietary checks or a replacement schema as proof of conformance to APDS 1.0.0. The [semantic contract](semantics.md) specifies portable interpretation; implementation strategies remain separate.

## Version 1.0.0 immutability and extensions

Once published, the APDS 1.0.0 schema, semantic rules and normative expected fixture outcomes are immutable. Do not move its release tag or replace its bytes. Any normative-file change requires a new standard version: a major version for incompatible validity or meaning changes, a minor version for compatible additions, and a patch version for corrections preserving validity and meaning. Even editorial corrections receive new published bytes under a new version.

Objects stay closed wherever the schema declares `additionalProperties: false`. Unknown root keys are errors, not implicit extensions. Adding a field to a released contract requires a new standard version and an explicit compatibility decision. Existing open data surfaces, such as Tool input/output JSON Schemas, do not open unrelated manifest objects.

## Canonical source and editor links

The canonical source is this versioned directory in the `agentpm-dev/cli` repository. AgentPM's compatibility/editor path, `schemas/agentpm.manifest.schema.json`, mirrors this schema. That convenience path is not a separate authority or an APDS requirement.

The schema's version-stable `$id`, also suitable for an editor `$schema`, is:

```text
https://raw.githubusercontent.com/agentpm-dev/cli/refs/tags/apds-v1.0.0/standards/agentpm/1.0.0/manifest.schema.json
```

Validators should use a trusted bundled copy of their supported contract, without fetching a mutable repository branch during routine validation. Development checks against other schemas cannot establish conformance to APDS 1.0.0.

The release tag `apds-v1.0.0` identifies this versioned bundle.

## Reporting ambiguity

Report ambiguities through the [CLI repository's issue tracker](https://github.com/agentpm-dev/cli/issues). Identify APDS 1.0.0 and include the relevant schema path or semantic rule, a minimal manifest or composition, and the expected versus observed interpretation. Include the implementation/version when its behavior exposed the ambiguity.

Handle corrections under the [versioning policy](#version-100-immutability-and-extensions); an ambiguity report does not change this version's meaning.
