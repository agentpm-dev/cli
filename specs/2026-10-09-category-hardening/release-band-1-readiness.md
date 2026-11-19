# Release Band 1: release preparation

Prepared **2026-11-18**. This completes the builder's release-preparation step;
the new changes still need to be pushed and checked in hosted CI. Publication,
production inspection/backfill, capacity approval and activation remain the
ordered operator gates in [tasks.md](tasks.md#ordered-production-rollout).

## Prepared versions and files

| Surface                             | Prepared state                                                      | Files / deployment condition                                                                                                                                                                                  |
| ----------------------------------- | ------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| CLI                                 | **0.1.45**, previously 0.1.44                                       | `crates/agentpm-cli/Cargo.toml`, regenerated `Cargo.lock`; new public `CHANGELOG.md` with migration notes                                                                                                     |
| Web CLI status                      | **v0.1.45**, planned date **2026-11-18**                            | `agentpm-web/src/app/status/page.tsx`; deploy after the CLI is published/installable, preferably alongside the matching Registry docs. Correct the date if the release occurs on another local calendar date. |
| Registry docs                       | Explain 0.1.45 authoring checks versus 0.1.36 declaration tolerance | `agentpm-api/docs/v0.1/cli/publish.mdx`; deploy with Registry enforcement off initially                                                                                                                       |
| Node/Python SDKs and Rust SDK crate | **0.1.17**, unchanged                                               | No SDK release or new runtime minimum is introduced here                                                                                                                                                      |
| APDS                                | **agentpm / 1.0.0**, unchanged frozen bytes                         | Publish the separate immutable APDS tag and verify the raw schema before publishing CLI 0.1.45                                                                                                                |

The CLI has no existing root changelog; the new entry documents this release
without inventing historical entries. It is marked **Unreleased**. Mark the
entry released with the actual date when publishing the CLI. Its migration notes
cover the explicit selector, nonblank descriptions, rebuilding authored content,
read-only legacy lint and preservation of historical installed releases. Public
copy contains no internal milestone/stage language.

The lockfile diff changes only the CLI package's own version; no dependency
version was upgraded. The website change updates only its existing CLI status
marker. APDS package-page presentation remains later work.

## Version/tag availability and frozen source

A read-only GitHub API check on 2026-11-18 confirmed:

- [Latest published CLI](https://github.com/agentpm-dev/cli/releases/tag/v0.1.44): `v0.1.44`, published `2026-11-11T02:24:25Z`.
- `refs/tags/v0.1.45` did not exist (HTTP 404); 0.1.45 is available for this release.
- `refs/tags/apds-v1.0.0` did not exist (HTTP 404); publication remains an operator step, not a completed prerequisite.

The reviewed frozen bundle source is
`e308bfefb880f2a3a96cc08f4f87d61d3c55a76a`. The current canonical directory is
byte-identical to that commit, and the Registry snapshot still pins it with
inventory SHA-256
`b8580681aec3b113cf8a65061cee9907b07f07aa71c9361822aa8c874310fed6`.
That reviewed source is the candidate target for the APDS tag; a later tag target
must contain exactly the same reviewed bundle. No tag was created in preparation.

Before CLI publication, verify this exact editor URL resolves to the frozen
schema after the operator publishes the APDS tag:

```text
https://raw.githubusercontent.com/agentpm-dev/cli/refs/tags/apds-v1.0.0/standards/agentpm/1.0.0/manifest.schema.json
```

## Reconciled band prerequisites

| Prerequisite                                                        | Evidence                                                                                                                                                                                                                                                                                                               |
| ------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Tolerance shipped before declaration emission                       | [M1A.1 record](m1a1-verification.md): CLI 0.1.36 publication/installation operator-confirmed 2026-10-22; closed-root/schema and preservation coverage reviewed. Tolerance commit `e0143afc0dd92dbe1c05d3fe27bb19c8325ed02f` remains an ancestor of the prepared source.                                                |
| Required earlier schema additions precede freeze                    | [M1B prerequisite record](m1b-verification.md#freeze-prerequisite-evidence): commits `8c68bef66bf0f3bfd6dfceb5f6f5d11c6c80543a` and `2f6908ea676fcbe6ffd92c5d4a3f5790b11189af` remain ancestors; their fields are already in the frozen bundle.                                                                        |
| Canonical definition, resolved discrepancies and independent parity | [M2C frozen-candidate/parity record](m2c-verification.md) and [durable report](m2c-parity-report.json): no open tested mismatch. Fresh 0.1.45 Rust and independent Python executions agree on all 129 corpus cases; schema/bundle checksums and Registry release/signing snapshots agree.                              |
| Newcomer/readability and contract review                            | [M1B review](m1b-verification.md) and [M1C review](m1c-verification.md) record completed independent README/semantics review, with later corpus reconciliation frozen in M2C. No normative bytes changed afterward.                                                                                                    |
| New authored kinds                                                  | Fresh 0.1.45 default init produces an Agent Package; each of the eight scaffold kinds declares the supported standard and passes ordinary lint. Saved builder results: ignored `target/band1/scaffold-results.json`.                                                                                                   |
| Publication bypass/migration and rollout design                     | [M4A real-local evidence](m4a-verification.md) plus [M4B tests/review](m4b-verification.md): custom-client negatives, in-flight/completed sessions, immutable historical release/successor, private graph/signing/integrity gates and backfill mutation regressions. Production smoke evidence remains a release gate. |

## Local verification for this preparation

All commands below passed, exit 0, from their corresponding repository roots:

- CLI: `cargo check --offline -p agentpm-cli` regenerated only its package version in `Cargo.lock`.
- CLI: `cargo fmt --all -- --check`; `cargo clippy --offline --locked --workspace --all-targets -- -D warnings`.
- CLI: `cargo build --offline --locked -p agentpm-cli`; `AGENTPM_TELEMETRY=0 target/debug/agentpm --version` prints **agentpm 0.1.45**.
- CLI: `AGENTPM_APDS_RESULTS=target/band1/apds-rust-results.json cargo test --offline --locked -p agentpm-cli --test apds_validation shared_corpus_matches_actual_rust_validation -- --nocapture` — **129 actual Rust cases**, one integration test passed.
- Registry: `.venv/bin/python -B scripts/check_apds.py --out ../agentpm/target/band1/apds-python-results.json --rust-results ../agentpm/target/band1/apds-rust-results.json` — **129 Python cases, actual Rust/Python results agree case by case**. Synthetic test Registry key and disabled AWS metadata lookup were supplied as in M4B.
- CLI: `../agentpm-api/.venv/bin/python -B scripts/check_apds_schema.py`; `../agentpm-api/.venv/bin/python -B scripts/check_release_contract.py --registry-root ../agentpm-api`; `python3 scripts/check_registry_bootstrap.py --release` — frozen schema/bundle, cross-repo release/signing snapshots and official-root release policy all pass.
- CLI: read-only ancestry checks for the tolerance, required earlier schema and frozen-source commits; `git diff --exit-code e308bfefb880f2a3a96cc08f4f87d61d3c55a76a HEAD -- standards/agentpm/1.0.0` — no bundle drift.
- CLI: eight real `init` → ordinary `lint --format json` probes with the rebuilt binary, including bare/default Agent init — all conformant, no Runner readiness claim.
- Registry: `.venv/bin/python -B -m pytest -q tests/test_docs_content.py` — **5 passed, 166 subtests passed**; updated publish MDX also compiles through the existing web MDX compiler.
- Web: `pnpm typecheck` and `pnpm lint` — success; lint retains six existing unused-variable warnings outside the changed status page, with zero errors.
- Formatting: existing Prettier configuration for the status page, MDX and new Markdown; Markdown/Python-hook whitespace/EOF/conflict equivalents where applicable; `git diff --check` on each changed repository.

Logs are in `/tmp/agentpm-band1-{cargo-check,clippy,build,rust-apds,python-apds,parity,docs,web-typecheck,web-lint}.log`.
Fresh corpus arrays and scaffold results are in ignored `agentpm/target/band1/`.
No new tests were added for the version/status literals. Full runtime regression
suites were not repeated solely for these metadata changes; their prior reviewed
results remain in the milestone records, with fresh hosted checks required after
this prepared patch is pushed.

## Handoff

1. Push the CLI version/lock/changelog and preparation records, Registry MDX, and
   Web status changes to their PRs. Wait for fresh hosted checks for these changes;
   the earlier M4B PR runs covered the pre-bump source.
2. Continue the [ordered rollout](tasks.md#ordered-production-rollout): CLI merge
   and push-only CI, immutable APDS tag/URL verification, then CLI publication.
   Keep the Web status change queued until 0.1.45 is actually available.
3. Deploy the Registry with `APDS_PUBLISH_ENFORCED=false`, then execute the
   remaining production inventory/backfill, isolated capacity, approval and
   coordinated activation/smoke gates. Neither release capacity nor production
   activation is certified by these preparation checks.
