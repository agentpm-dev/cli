# `agentpm` Repo Review Guide

This repo is the Rust CLI and schema layer.

## Review Focus
- CLI correctness
- install/publish flow regressions
- schema/lockfile compatibility
- release/distribution risk

## Blocking Issues
- breaking manifest compatibility
- changing CLI behavior unexpectedly
- resolution/install determinism regressions
- failing Rust verification

## Preferred Emphasis
- correctness first
- compatibility second
- release consequences third

## Low-Value Review Noise
- avoid style-only feedback unless it violates existing Rust conventions or toolchain expectations
