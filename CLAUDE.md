# `agentpm` Repo Review Guide

This repo is the Rust CLI and schema layer.

## Review Focus
- CLI correctness
- install/publish flow regressions
- schema/lockfile compatibility
- release/distribution risk
- missing docs updates when user-facing behavior changed
- unnecessary dependency additions
- unintended command output or error wording drift

## Blocking Issues
- breaking manifest compatibility
- changing CLI behavior unexpectedly
- resolution/install determinism regressions
- failing Rust verification
- user-facing behavior changes with no documentation update when docs should change

## Specific Review Checks
- If manifest behavior changed, compare the implementation against both `schemas/agentpm.manifest.schema.json` and `crates/agentpm-cli/src/manifest.rs`.
- If lockfile behavior changed, treat it as contract-sensitive and inspect the install/resolution path plus any `agent.lock` read/write behavior.
- If publish or install behavior changed, review both the CLI command and the shared Rust SDK client path.
- If command output or error wording changed, verify the change is intentional and not incidental drift.
- If a dependency was added, check whether the gain justifies the added maintenance/release surface.

## Preferred Emphasis
- correctness first
- compatibility second
- release consequences third

## Low-Value Review Noise
- avoid style-only feedback unless it violates existing Rust conventions or toolchain expectations
