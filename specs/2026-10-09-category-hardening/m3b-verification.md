# M3B authoring validation evidence

Date: 2026-11-16. Implementation and local verification complete; user confirmed full Claude review passed with no remaining findings. Hosted CI and the requested manual testing remain pending. This record covers M3B only; final acceptance remains open.

## Changed contracts

- `crates/agentpm-cli/src/manifest/authoring.rs` adapts the existing pinned Rust APDS evaluator for strict local authoring. Ordinary lint requires an explicit supported selector and evaluates standalone rules. Missing declarations and unsupported IDs/versions have rule IDs, source paths and targeted remediation. Editor-schema warnings remain separate from normative results.
- `commands/lint.rs` preserves `file`, `ok`, `issues`, JSON/NDJSON stdout, nonzero failure and stderr command errors. Ordinary reports now include scoped `apds` metadata. Explicit contextual modes keep their existing engine and context requirements. Explicit filenames, including `*.agent.json`, are actually read; absent inputs cannot produce a successful empty report.
- `commands/publish.rs` and `commands/package.rs` share strict canonical authoring outcomes, including saved-build manifests. A self-consistent descriptor/archive digest cannot bypass declaration or semantic validation. Existing package contents, target fan-in, signing, uploads, receipts and publish link remain covered by regression tests.
- Knowledge/Memory command boundaries require the same standalone validation before local build or authored inspection. Existing content/freshness checks remain separate. Historical installed inspection recognizes references, installed directories, manifest paths and canonicalized symlinks without rewriting those packages or assigning an APDS verdict.
- `commands/new.rs` declares the supported standard on its CLI-synthesized root and prints migration guidance for undeclared Template-provided local Agent manifests. Rendering preserves those files rather than inserting declarations into authored Template content.
- `lint_output.rs` reuses the existing Stage 1 issue renderer and shares the scoped APDS summary between lint and publish. There is no new independent lint engine.
- CLI README, developer script README and seven feature-local Registry docs explain strict authoring, legacy preview, scope and local migration. No public docs route changed.

The schema, immutable standard bundle/checksums, Registry Python implementation/snapshot, lockfile contract, SDK contracts and Runner architecture are unchanged. Existing compatibility readers remain for installed artifacts and Template consumers; they are not ordinary authoring authorization.

## D04: explicit read-only legacy preview

**Decision:** provide `agentpm lint --legacy`. The user explicitly selected the flag after discussing the single-engine alternative.

For an undeclared object, the command inserts `agentpm / 1.0.0` into an in-memory copy and calls the same APDS evaluator. Its report is labeled `compatibility: "legacy-unverified"` and omits the `apds` verdict. A successful preview means those compatibility checks found no errors; it does not certify that a historical release declared APDS. Other defects still fail. Explicit malformed or unsupported declarations are never replaced.

The flag conflicts with `--fix`, `--schema` and `--apds`. It does not write a declaration, rebuild output, change a lock or authorize publication. Package/publish have no legacy bypass. This offers a diagnostic path for editable old sources without silently turning inferred metadata into verified conformance.

`--fix` can add the pinned editor hint only after explicit supported input passes canonical validation. A no-op leaves the original bytes and key order intact; missing, invalid and unsupported declarations are not written. A local schema override is loaded and labeled non-authoritative but cannot replace canonical checks or remove the declaration requirement.

## Acceptance and regression mapping

| Requirement                         | Evidence                                                                                                                                                                                                                                                                                                                                                                                                      |
| ----------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| S2-AC-02, T-CLI-01–03/06 regression | Existing init/export/new/run tests pass, including default empty Agent, every explicit kind, safe Unicode/escape serialization and preservation of existing files. Authoring tests remove `standard` from all eight generated kinds and prove strict lint/publication reject before packaging. The CLI-synthesized Template root now gains the declaration; immutable schema and kind defaults are unchanged. |
| S2-AC-03, T-CLI-04/05               | Ordinary lint uses the pinned standalone engine for Tool, Skill, Loop, Memory, Template, binding and identity failures. Empty Agent remains valid; whitespace descriptions, malformed names, unknown kinds and bad paths fail with applicable rules and paths.                                                                                                                                                |
| S2-AC-04, T-CLI-05                  | All 129 shared cases still match expected Rust outcomes. Explicit resolved/package contexts preserve conformant/nonconformant/incomplete/unsupported distinctions. Ordinary standalone success does not claim files, graph context or Runner behavior were evaluated. Existing independent Python parity evidence remains in M2C; no Python semantic implementation changed here.                             |
| D04, T-CLI-04                       | Explicit legacy preview is labeled/read-only, invalid input still fails, malformed/unsupported selectors never fall back, conflicting write/scope/schema flags are rejected, and strict new publish still rejects the same undeclared source.                                                                                                                                                                 |
| T-CLI-05, Stage 1 output            | Human snapshots and JSON snapshots for missing/unsupported standard; NDJSON equivalence, exact issue/report fields, failure exits, stderr separation and shared lint/publish error rendering. Actual TTY invocation checked separately below.                                                                                                                                                                 |
| T-CLI-06, historical behavior       | Existing `new`, installed Tool execution, exports, telemetry and signing/upload suites pass. Added installed Knowledge/Memory checks through references, absolute directory/manifest paths and Unix symlinks; source bytes remain unchanged. Saved-build negative tests recompute metadata/archive digests before checking semantic rejection.                                                                |

Golden diagnostics live outside the immutable corpus in `crates/agentpm-cli/tests/fixtures/authoring/`, with LF checkout attributes to keep the text comparisons stable on Windows. New coverage is in `tests/authoring_validation.rs`; existing focused integration tests were adapted only where they author new manifests. Historical fixtures remain undeclared where the consumer compatibility boundary is being tested.

## Commands and results

Commands run from the CLI root unless a different root is specified. Success results below are exit 0. HTTP test fixtures use disposable loopback services and synthetic credentials; the full workspace suite needed sandbox approval for local listeners. No production publication occurred.

| Command                                                                                                                                            | Result                                                                                                                                                                                              |
| -------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `cargo check -p agentpm-cli --all-targets`                                                                                                         | Pass.                                                                                                                                                                                               |
| `cargo test --workspace --no-fail-fast`                                                                                                            | **1,287 passed, 6 ignored, 0 failed**, across 18 test targets including doc-tests. CLI unit target: 1,150 passed, 5 ignored.                                                                        |
| `cargo test -p agentpm-cli --test authoring --test authoring_validation --quiet`                                                                   | Original focused run passed 15 existing authoring tests and 6 initial new tests. Expanded `authoring_validation` run passes 9 tests; further negative legacy cases were added within that test set. |
| `cargo test -p agentpm-cli --test ci_publishing --test payload_compatibility --test python_resolution`                                             | 25 passed, including saved-build mutation rejection and publication-link preservation.                                                                                                              |
| `../agentpm-api/.venv/bin/python -B scripts/check_apds_schema.py`                                                                                  | Frozen bundle/schema authority checks pass; no standard files changed.                                                                                                                              |
| `../agentpm-api/.venv/bin/python -B -m unittest discover -s scripts -p 'test_apds_*.py' -v`                                                        | 20 infrastructure tests passed.                                                                                                                                                                     |
| `../agentpm-api/.venv/bin/python -B scripts/check_apds_cli_fixtures.py --cli target/debug/agentpm`                                                 | Actual CLI accepts 41 positive manifests; Knowledge reader accepts fresh vectors and rejects truncation. This probe is additional to the 129-case corpus checks.                                    |
| `cargo fmt --all` and `cargo fmt --all --check`                                                                                                    | Pass.                                                                                                                                                                                               |
| `cargo clippy --workspace --all-targets -- -D warnings`                                                                                            | Pass.                                                                                                                                                                                               |
| Registry root: `REGISTRY_PRIVATE_KEY_B64=AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA= .venv/bin/python -B -m pytest -q tests/test_docs_content.py` | 5 tests and 166 subtests passed.                                                                                                                                                                    |
| Sibling SDK Prettier on touched Markdown/MDX/JSON; both repos `git diff --check`                                                                   | Pass.                                                                                                                                                                                               |

During integration, historical producer fixtures initially failed the newly strict authoring gate. New-authored fixture builders now declare the supported standard. The historical telemetry fixture explicitly uses read-only legacy inspection with source-byte preservation. A permissive development-schema test now asserts that it cannot bypass canonical checks. Production enforcement was not weakened to retain those old expectations.

An actual TTY run of `AGENTPM_TELEMETRY=0 NO_COLOR=1 .../target/debug/agentpm lint` in a disposable generated project with its selector removed exited **1** and rendered:

```text
✗ agent.json
  [ERROR] [APDS-ID-001] Required field `standard` is missing. Review this local manifest, add `"standard":{"id":"agentpm","version":"1.0.0"}`, then run `agentpm lint` again. Existing published artifacts must not be rewritten.
        at instance /standard
  APDS standalone scope: nonconformant; Runner behavior not evaluated.
Error: Lint failed
```

Automated snapshots cover piped human output plus JSON/NDJSON. This TTY check is a builder diagnostic probe, not an operator manual-test completion claim. Local suite logs under `/tmp/agentpm-m3b-*.log` are temporary supporting evidence; checked-in tests and snapshots are the durable evidence.

## Migration, risks and remaining gates

- Old editable sources must be reviewed and given an explicit supported declaration before new lint/build/package/publish. Legacy preview may help identify remaining defects. Rebuild derived Knowledge/Memory content and saved bundles after editing; do not modify immutable installed or historical published bytes.
- Template `new` generation and historical installed execution remain compatible. Its CLI-synthesized root now declares the supported standard. Older Template-provided nested Agent manifests retain their rendered contents; completion output and feature-local docs explain local migration before strict authoring. Workspaces previously generated by an older CLI may also need their local root upgraded.
- A standalone pass is not resolved-composition, complete-package or execution-readiness approval. Contextual commands and current content/freshness/runtime checks still apply. Claude confirmed this specific scope/readiness review item passes; see the review evidence below.
- User confirmed full Claude review passed on 2026-11-16, including the resolved generator and messaging findings. Hosted CI and requested manual testing remain pending. Overall acceptance remains open; no milestone-complete claim is made. Public tag, Registry enforcement and coordinated rollout remain the owning later milestones/release-band work.
- No production writes, push, merge, version bump, release or deployment were performed. The only cross-repo changes are feature-local public documentation in `agentpm-api`.

## Independent review: scope versus Runner readiness

On 2026-11-16, the user relayed Claude’s pass for the explicitly assigned readiness-clarity item. Human APDS success and failure summaries carry the validation scope and state that Runner behavior was not evaluated. JSON exposes `runner_evaluated: false`, `level`, `evaluated_rules` and `unevaluated`. The legacy preview explicitly identifies inferred compatibility, says APDS conformance and Runner behavior were not evaluated, and states that source is unchanged.

That initial signoff closed only the readiness checklist item; the user subsequently confirmed full review passed after the findings below were addressed. Manual testing will be prepared after review, using visible local-project fixtures and clear issue/command/expected-result explanations as requested.

## Review correction: Template generation and the next lint command

The earlier migration rationale incorrectly treated the CLI-synthesized root like Template-authored content. `synthesize_root_manifest` creates fresh local JSON, so it now emits the same supported `standard` as init/export alongside its already-pinned editor schema. No published bytes, installed dependency, lock shape or Template source is rewritten.

Completion output folds root lint into its single next-step line. Undeclared Template-provided local Agents are grouped under one heading, with a shared declaration and an explicit lint command per file. Already-declared nested Agents receive no missing-declaration notice. Public CLI README and `cli/new.mdx` now distinguish current root generation, old generated workspaces and nested authored-file migration.

The updated declared-Template integration checks supported root output while keeping workspace/origin metadata free of an invented standard. A new real-CLI regression creates a Template with two undeclared nested Agents and a render variable, runs `new`, and then runs bare strict lint from the generated project: it passes with `conformant` and `runner_evaluated: false`. Explicit nested lint fails with `APDS-ID-001` at `/standard`; the compatibility preview passes. The Template source is byte-identical, and the generated nested file matches only the expected variable substitution before and after lint.

Validation after the correction: `cargo test -p agentpm-cli --test authoring --quiet` passes **16 tests**; `cargo test -p agentpm-cli --bin agentpm commands::new::tests --quiet` passes **30 tests**, including hosted/local Template fixture flows using local listeners. Workspace Clippy with warnings denied, Rust formatting and Markdown/MDX formatting pass. Registry docs tests pass **5 tests plus 166 subtests**. Both repos pass `git diff --check`. This initial focused rerun omitted the Stage 1 messaging goldens; the subsequent full-workspace verification below supersedes that incomplete regression evidence. The user subsequently confirmed full independent review passed after the follow-up messaging correction below. Manual testing remains pending.

## Review correction: Stage 1 messaging goldens and full regression

Claude identified two failing `tests/messaging.rs` goldens after the first generator correction. The focused authoring and `new` unit runs omitted that integration target. The added output was intentional, but leaving its existing exact-output tests unchanged was a regression-check gap.

The final output retains one `Next:` line, combining root lint with README/project review. Both messaging goldens were deliberately updated; the README case now pins the full stdout rather than only a prefix. Authored entrypoint text, relative path formatting, exit statuses and stdout/stderr routing remain intact. Multiple undeclared nested manifests share one `Local manifests missing standard` heading and one declaration example, followed by each filename and lint command. The real-CLI authoring regression now checks two undeclared manifests, one heading, one `Next:` line and byte preservation. The declared-only case confirms the migration notice is absent.

Final checks on this tree:

- `cargo test -p agentpm-cli --test messaging --test authoring --quiet`: **6 messaging and 16 authoring tests passed**. The first sandbox run could not bind the messaging HTTP fixtures; the permitted rerun passed.
- `cargo test --workspace --no-fail-fast`: **1,287 passed, 0 failed, 6 ignored**, exit 0, across 18 targets including doc-tests. This is the current result, superseding the earlier 1,286-test pre-generator baseline and the reviewer-reported red intermediate tree. The local run log is `/tmp/agentpm-m3b-review-workspace.log`; it is supporting temporary evidence, not a committed artifact.
- Workspace Clippy with warnings denied, `cargo fmt --all` and `cargo fmt --all --check`: passed.
- Registry docs tests: **5 tests plus 166 subtests passed**. Updated public completion-output documentation passes Prettier; both repos pass `git diff --check`.

The user subsequently confirmed full independent review passed. Hosted CI remains pending. No manual runbook or operator-test completion is claimed by these builder regression runs.

## Full independent review signoff

On 2026-11-16, the user confirmed Claude review fully passed, with no remaining findings. This includes D04, readiness clarity, root standard generation, grouped migration guidance and the deliberate Stage 1 messaging-golden updates. Hosted CI and requested manual testing remain unconfirmed. The manual runbook will use visible fixtures in the local project directory and explain each issue, command, expected result and actual-result interpretation, following the user’s requested review/CI/manual sequence.
