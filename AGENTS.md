# `agentpm` Repo Guide

This repo contains the Rust CLI, schema, and packaging/install/publish flows.

## Purpose
- manifest handling
- install flows
- publish flows
- CLI UX
- schema and lockfile behavior

## Local Rules
- Formatting/linting checks run on commit via pre-commit.
- This repo is open source.
- Release/distribution affects Homebrew workflows.
- Be careful with CLI and manifest contract stability.
- Consult `.pre-commit-config.yaml` for the repo’s expected local verification tools and formatting/lint expectations.

## Builder Guidance
- Do not change the `agent.json` schema unless explicitly asked.
- Be conservative with install/publish behavior.
- Prefer clear CLI behavior over internal cleverness.
- Keep compatibility and determinism in mind for locks, installs, and resolution.
- Prefer narrow, localized diffs.
- Avoid broad refactors unless the task clearly requires them or the user asked for them.
- Preserve existing error wording unless the task is intentionally changing UX.
- Do not add new dependencies casually.
- Update tests when behavior changes.
- Update docs when command behavior or user workflow changes.

## Common Patterns In This Repo
- `crates/agentpm-cli` owns the CLI entrypoint and subcommands.
- `crates/agentpm-sdk` is the shared Rust HTTP client and model layer used by the CLI.
- Shared concerns such as auth, manifests, config, and terminal UI live in reusable modules under `agentpm-cli/src`.
- Publish and install flows are contract-sensitive and should be treated as product behavior, not just implementation details.
- The manifest schema source of truth is `schemas/agentpm.manifest.schema.json`.
- Manifest loading and validation behavior is centered in `crates/agentpm-cli/src/manifest.rs`.
- `agent.lock` behavior is a contract surface and should be treated as compatibility-sensitive.

## Common Workflows

### When changing CLI behavior
1. update the relevant command module
2. inspect shared auth, manifest, config, or UI helpers if the flow crosses module boundaries
3. update `agentpm-api/docs` if the user workflow changed
4. run the relevant Rust verification commands

### When changing manifest behavior
1. inspect `schemas/agentpm.manifest.schema.json`
2. inspect `crates/agentpm-cli/src/manifest.rs`
3. inspect the commands that rely on that behavior, especially `lint`, `init`, `publish`, and `install`
4. do not change schema or manifest semantics casually

### When changing publish or install behavior
1. inspect both the CLI command and the shared Rust SDK client
2. preserve manifest, lockfile, and API compatibility unless the spec explicitly changes them
3. verify docs if the user experience changed
4. update tests when behavior changes
5. update `agentpm-api/docs` if the user workflow changed
6. run verification

## Decision Guide
- If the change is command flow or UX, start in `crates/agentpm-cli`.
- If the change is primarily API transport or response modeling, inspect `crates/agentpm-sdk` as well.
- If the change touches manifests, treat compatibility as sensitive by default.

| If the change is about... | Start here | Also inspect |
|---|---|---|
| command flow or UX | `crates/agentpm-cli/src/commands/*` | `ui.rs`, `auth.rs`, `config.rs` |
| manifest validation or semantics | `crates/agentpm-cli/src/manifest.rs` | `schemas/agentpm.manifest.schema.json` |
| lockfile behavior | install/resolution flow | manifest handling, `agent.lock` read/write logic, and related CLI UX |
| API request/response handling | `crates/agentpm-sdk/src/client.rs` | related CLI command |
| publish or install behavior | command module | Rust SDK client + manifest handling |

## Do / Don’t
- Don’t change schema behavior casually.
  Do trace schema-related behavior through `schemas/agentpm.manifest.schema.json` and `crates/agentpm-cli/src/manifest.rs`.
- Don’t broad-refactor command flows by default.
  Do keep diffs localized to the command and its immediate helpers unless the task clearly requires more.
- Don’t change error wording casually.
  Do preserve existing wording unless the spec or UX explicitly calls for a change.
- Don’t change lockfile shape or resolution behavior casually.
  Do treat lockfile changes as compatibility-sensitive product behavior.

## Verification
- Consult `.pre-commit-config.yaml` for the expected formatting/lint checks.
- Run the narrowest relevant test first, then broader checks when practical.
- `cargo fmt --all` is a required final step for Rust changes before handoff.
- If Rust files were edited, do not report completion until formatting has been run.
- Favor repo-native commands such as:
  - `cargo fmt --all --check`
  - `cargo clippy --workspace --all-targets -- -D warnings`
  - `cargo test --workspace`
- If you cannot run a check, mention that explicitly in the final response.

## Never Do This
- Don’t casually change CLI flags or output contracts.
- Don’t break schema expectations.
- Don’t introduce release/distribution surprises without explicit reason.
