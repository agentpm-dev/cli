# Workflow Templates Release Brief

This note captures the release-ready scope for workflow templates before Milestone 8 example-template work begins.

## Scope shipped

Workflow templates are now first-class package kinds across the current AgentPM platform:

- CLI manifest schema and lint support for `kind: "template"`
- `agentpm init --kind template`
- template publishing with `packages:publish`
- template-aware install/download plumbing for `agentpm new`
- `agentpm new` generation from:
  - published template refs
  - local template directories
  - direct local `agent.json` paths
- workspace generation with:
  - generated root `agent.json`
  - optional local `agents/*.agent.json`
  - `agentpm.workspace.json`
  - `.agentpm/template.json`
  - runnable `agent.lock`
- post-generation `agentpm install` support for generated workspace projects
- registry discovery/detail UX for templates
- documentation for template authoring and consumption

## Key product behaviors

- Templates are consumed with `agentpm new`, not `agentpm install`.
- `agentpm new` copies and renders scaffold files but does not execute template-provided code during scaffolding.
- Generated projects may include multiple local manifests and/or registry agent roots without adding recursive `agents[]` support to normal `kind: "agent"` manifests.
- The template artifact itself is not kept as a permanent runtime dependency in `agent.lock`.

## Verification completed

Manual verification was completed during implementation and review for:

- published-template bootstrap flow
- local-template bootstrap flow
- generated workspace layout
- generated `agent.json` and `agent.lock` correctness
- multi-agent workspace behavior
- retry-safe cleanup on failed generation
- non-empty target directory refusal
- no script execution during scaffolding
- template registry search/detail/namespace visibility
- tool/agent regression checks on core package flows

Automated verification was also rerun during Milestone 7 closeout:

- `cargo test -p agentpm-cli`
- `python -m unittest tests/test_search_helpers.py tests/test_install_helpers.py tests/test_publish_helpers.py tests/test_tar_validation.py`
- `pnpm typecheck`

Results:

- CLI tests: `129 passed`
- backend helper tests: `33 passed`
- web typecheck: passed

## Release surfaces

Release-facing surfaces updated before Milestone 8:

- repo README
- CLI help text for `agentpm new`
- CLI docs for `init`, `install`, `new`, and `publish`
- `agent.json` docs for templates
- getting-started intro CLI command list
- registry/web template search/detail UI

## Deferred to Milestone 8

Milestone 8 will cover official template examples and execution-surface-specific validation, including:

- Python SDK workflow template
- Node SDK workflow template
- CLI automation template
- MCP server template
- examples-site follow-up docs adjustments

## Known design boundaries

- `agentpm.workspace.json` is currently written for generated projects in general, even when the project is structurally simple.
- Generated workspace projects support `agentpm install` for lock regeneration, but not `agentpm install <spec>` for topology changes.
- Template variables are generation-time scaffold values only; runtime secrets belong in `.env.example` and runtime environment configuration.
