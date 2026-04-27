# `agentpm` Repo Guide

This repo contains the Rust CLI, schema, and packaging/install/publish flows.

## Purpose
- manifest handling
- install flows
- publish flows
- CLI UX
- schema and lockfile behavior

## Local Rules
- `rustfmt` and `clippy` run on commit.
- This repo is open source.
- Release/distribution affects Homebrew workflows.
- Be careful with CLI and manifest contract stability.

## Builder Guidance
- Do not change the `agent.json` schema unless explicitly asked.
- Be conservative with install/publish behavior.
- Prefer clear CLI behavior over internal cleverness.
- Keep compatibility and determinism in mind for locks, installs, and resolution.

## Verification
- Run Rust-native verification when meaningful.
- Favor repo-native commands such as formatting, clippy, and tests where relevant.

## Never Do This
- Don’t casually change CLI flags or output contracts.
- Don’t break schema expectations.
- Don’t introduce release/distribution surprises without explicit reason.
