# CLI development checks

Run these commands from the CLI repository root. The implementation checks here live outside the versioned APDS bundle. The shared inputs, expected results and portable comparator live in `standards/agentpm/1.0.0/conformance/`.

## Check generated package scaffolds

```sh
cargo test -p agentpm-cli --test authoring
```

This invokes the real CLI in disposable directories. It compares the default Agent,
all eight explicit kinds and both Knowledge modes with checked-in scaffold snapshots,
then checks ordinary strict lint and standalone APDS validation. It also covers
description encoding, invalid names, existing-file protection, Tool execution/builds,
Template generation and exported Skill manifests. Results appear in the terminal
and the normal CLI test job; a failure identifies the command or snapshot mismatch.
The Harness check supplies local lock context and verifies that a valid Agent without
a Loop is reported as not runnable. No production publication or model call occurs.

## Check actual CLI acceptance of APDS examples

Build the current local CLI and run the probe:

```sh
cargo build -p agentpm-cli
python3 scripts/check_apds_cli_fixtures.py --cli target/debug/agentpm
```

On Windows, use `python` and `target/debug/agentpm.exe`.

The script runs each positive manifest through `agentpm lint --apds standalone --format json`. It requires an actual conformant JSON report for every file; an empty discovery result is a failure. It also checks that the Knowledge reader accepts the fresh vector example and rejects the truncated binary.

Results appear in the terminal, or in GitHub Actions under **AgentPM positive APDS fixture acceptance**. Success exits 0 and reports the accepted manifest count and Knowledge pair; failure exits 1 with the affected fixture and command output. This probe does not establish complete semantic-corpus coverage.

## Run the complete Rust APDS validation corpus

Requirements: the repository's Rust toolchain, Python 3.11 or newer, and the Python dependency listed in the bundle's `conformance/requirements.txt`. For example:

```sh
python3 -m venv .venv-apds
.venv-apds/bin/python -m pip install -r standards/agentpm/1.0.0/conformance/requirements.txt
AGENTPM_APDS_RESULTS=target/apds-rust-results.json cargo test -p agentpm-cli --test apds_validation -- --nocapture
.venv-apds/bin/python standards/agentpm/1.0.0/conformance/run.py --results target/apds-rust-results.json
```

In Windows PowerShell, set `$env:AGENTPM_APDS_RESULTS = "target/apds-rust-results.json"` before the `cargo test` command, omit the leading environment assignment, and use `.venv-apds/Scripts/python.exe` for Python commands.

The Rust integration test invokes the actual CLI for every indexed validation case, supplying the requested scope and real graph/package inputs. It asserts status, exit behavior, unevaluated checks and every expected rule/document/path/severity finding. It exports a focused projection of actual native diagnostics for the portable comparator; additional native findings remain visible in the actual CLI reports. No expected answer is supplied to the production validator.

The compared fields are `case_id`, `level`, `status`, targeted `findings` and `unevaluated`. Native metadata such as `evaluated_rules`, notes, human messages and schema paths is implementation-specific and deliberately excluded. `evaluated_rules` records checks completed in the requested scope, including checks that pass because an optional declaration is absent; it is not branch coverage or proof that every clause of a rule was exercised. Different stopping points can produce different lists without changing the conformance result.

The JSON results are written to `target/apds-rust-results.json` only after the corpus test passes. The comparator reports whether those actual results match the shared expectations. In CI this runs under **Rust APDS results against shared expectations**; full test output remains in Actions logs. Runner observation scenarios require separate runtime tests.

## Validate a package with supplied context

```sh
# Manifest shape and intrinsic semantic relationships
./target/debug/agentpm lint --apds standalone path/to/agent.json

# Complete package files, with an explicit content boundary
./target/debug/agentpm lint --apds packaged --package-root path/to/package path/to/package/agent.json

# Exact selected dependency manifests and relationships
./target/debug/agentpm lint --apds resolved --graph selected-graph.json path/to/agent.json
```

`--graph` accepts the selected graph format described in the shared fixture index: `complete`, `artifacts` and `edges`. Artifact `manifest` paths resolve relative to the graph file, or may be absolute local paths. Each artifact has a unique `id`, namespaced `identity`, `kind`, exact `version` and manifest path. Edges identify the owning artifact (`root` for the evaluated manifest), the dependency field as a JSON Pointer and the selected target ID. The graph contains inputs, not validation verdicts or expected results. It is an AgentPM input format, not a required APDS lockfile or Registry protocol. No network resolution occurs.

Without the requested graph or packaged-file context, dependent checks remain incomplete. A supplied complete package missing required content is nonconformant. A conformant standalone result does not establish resolved-composition or packaged-content conformance. These explicit modes preserve ordinary lint's current authoring/compatibility behavior; they do not change publish/install enforcement.

## Validator capability boundaries

| Evaluation            | Checks                                                                                                                                                                                                                                              | Requires                                                        |
| --------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------- |
| Standalone            | Pinned selector/schema, dependency declaration identities, Agent binding membership, Loop graph/checkpoint/error-policy relationships, Profile constraint identity, Memory declarations/references/policies/durations, Knowledge index associations | Manifest only                                                   |
| Resolved              | Standalone checks plus selected kinds/namespaces/versions/edges, selected dependency manifests, Agent phase and Memory selectors within each owning composition                                                                                     | Local selected dependency graph; incomplete input is reported   |
| Packaged              | Standalone checks plus applicable Knowledge/Skill/Template/Tool content paths, Knowledge counts/bytes and supported vector/index representation, Draft 2020-12 Memory schemas/local references/governance                                           | Explicit complete content root; unavailable context is reported |
| Runner interpretation | Not evaluated by this validator: capability exposure, model execution, environment layering, Memory persistence/trigger execution, approval behavior or generation-time substitution                                                                | Implementation-specific runtime harness                         |

Unknown vector/index representations and composed/ref-mediated non-persistability compatibility that this engine cannot establish return `unsupported` with unevaluated rules; APDS does not mandate AgentPM's float32 representation. The engine does not execute Tools, lifecycle operations, Template entrypoints or provider calls. The manifest schema remains the versioned source of structural requirements.
