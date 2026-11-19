# APDS 1.0.0 conformance tests

Use this corpus to check an APDS implementation against shared inputs and expected results. The fixtures are also small reference examples for authors. Start with the [schema](../manifest.schema.json), [semantic rules](../semantics.md) and [rule index](rule-index.md).

The corpus is independent of AgentPM's CLI, Registry and Runner. It requires no account, credentials, published packages or model calls. Fixture expectations must agree with the schema and semantics; they cannot override either document.

## Run the corpus checks

Requirements: Python 3.11 or newer and the dependency in [requirements.txt](requirements.txt). From the **CLI repository root**, create a virtual environment and run:

```sh
python3 -m venv .venv-apds
.venv-apds/bin/python -m pip install -r standards/agentpm/1.0.0/conformance/requirements.txt
.venv-apds/bin/python standards/agentpm/1.0.0/conformance/run.py
```

On Windows, create the environment with `py -3 -m venv .venv-apds` and replace `.venv-apds/bin/python` with `.venv-apds/Scripts/python.exe` in the remaining commands. Dependency installation needs access to your Python package source; the checks themselves run offline.

For a downloaded APDS bundle, create the environment **outside the bundle**, install its `conformance/requirements.txt`, then run that environment's Python with `/path/to/bundle/conformance/run.py`. The script finds its data relative to itself and works from any directory. Keep generated environments, result files and bytecode outside the bundle: every supplied bundle file participates in its checksum inventory.

Run one case or list the available validation cases:

```sh
.venv-apds/bin/python standards/agentpm/1.0.0/conformance/run.py --case description-whitespace
.venv-apds/bin/python standards/agentpm/1.0.0/conformance/run.py --list
```

A successful default run prints `PASS corpus/schema: <case_id>`, then a summary and `Semantic outcomes were not evaluated`. Exit status is **0** for a matching corpus and **1** for a checksum, input, schema-expectation or comparison failure. A deliberately schema-invalid fixture passes its corpus test when the pinned schema rejects it as expected.

These checks validate the index, fixture schema expectations, graph-file references, rule/case mappings and bundle checksums. They **do not implement semantic validation**, verify packaged-content semantics or execute Runner behavior. AgentPM-specific acceptance tests and validator implementations live in their implementation repositories, outside this bundle.

## Test your validator

Read [fixture-index.json](fixture-index.json). For each case, supply its root manifest and the optional graph or package context to your implementation at the stated evaluation level. Compare what your implementation actually establishes with `expected`; do not use expected results as the implementation's answers.

The [index schema](fixture-index.schema.json) documents the corpus format:

| Field                     | Meaning                                                                              |
| ------------------------- | ------------------------------------------------------------------------------------ |
| `case_id`, `description`  | Stable case identity and purpose                                                     |
| `root`                    | Manifest path relative to this conformance directory                                 |
| `subject`, `level`        | Validator responsibility; `standalone`, `resolved` or `packaged` evaluation          |
| `schema_valid`            | Expected result of the pinned manifest JSON Schema alone, not full semantic validity |
| `rules`, `semantic_cases` | Requirements exercised and links to [interpretation cases A–T](semantic-cases.md)    |
| `graph`                   | Supplied artifact identities/manifests and exact dependency-selection edges          |
| `package`                 | Complete content root, or an explicit statement that content is unavailable          |
| `expected`                | Logical status, targeted findings and checks left unevaluated                        |
| `counterpart`             | Related positive fixture for a negative or alternative case                          |

Graph artifact IDs are local test handles. `identity`, `kind`, `version` and `manifest` describe the artifact; each edge selects a target from a declaring artifact and a JSON Pointer `field`. `from: "root"` means the case's root manifest. Other artifacts may be present without being selected from the root. Follow edges within their owning context; do not select an arbitrary same-named artifact. A wrong-kind/version edge can be intentional negative test data. This representation is not a required resolver, installation layout or lockfile.

Paths in the index are relative to this directory. Paths inside a manifest remain relative to that artifact's content root; a complete `package.root` supplies that boundary. Consumer-context paths belong to the consuming project, not a dependency. A supplied complete package can deliberately omit a required file: that is a definite violation. `availability: "unavailable"` means no content inspection context, so dependent checks remain unevaluated.

The vector Knowledge example uses one source, one chunk, two little-endian float32 values and an actual `agentpm-local` index. Its metadata was derived from those supplied bytes; optional build timestamps are omitted. This is one concrete representation for testing, not a universal APDS vector encoding or retrieval-provider requirement.

To use the comparator, adapt your implementation's results into a JSON array with `case_id`, `level`, `status`, `findings` and `unevaluated`. The index schema's `$defs.result` describes this **test-only projection**, not a universal APDS diagnostic API. Project the rules and findings targeted by each case; implementations may report additional independent diagnostics through their own interfaces. Human-readable messages do not participate in the comparison. For example, the expected projection for `description-whitespace` is:

```json
[
  {
    "case_id": "description-whitespace",
    "level": "standalone",
    "status": "nonconformant",
    "findings": [
      {
        "rule_id": "APDS-ID-002",
        "document": "fixtures/invalid/description-whitespace.json",
        "path": "/description",
        "severity": "error"
      }
    ],
    "unevaluated": []
  }
]
```

After your validator produces `results.json` outside the bundle:

```sh
.venv-apds/bin/python standards/agentpm/1.0.0/conformance/run.py --case description-whitespace --results results.json
```

For the full validation suite, omit `--case` and supply results for every indexed validation case. Unknown, missing and duplicate result IDs fail. A match prints `PASS: <count> supplied implementation result(s) match.` and exits 0. A mismatch exits 1 and identifies the case. An intentionally invalid manifest **passes its test when your implementation detects the expected violation**; accepting it is a test failure. A successful comparator run establishes agreement for the supplied results, not independent proof of how they were produced.

Compare the requested evaluation scope honestly. Standalone success does not establish graph or content validity. `incomplete` means needed inspection context is absent; `unsupported` means the declared standard/version or evaluation capability is unsupported. Neither is an authored violation by itself. Known violations remain violations even if other evidence is unavailable.

### Example integration: AgentPM's Rust CLI

From an AgentPM CLI source checkout, its implementation-specific tests can produce results for this comparator:

```sh
AGENTPM_APDS_RESULTS=target/apds-rust-results.json cargo test -p agentpm-cli --test apds_validation -- --nocapture
.venv-apds/bin/python standards/agentpm/1.0.0/conformance/run.py --results target/apds-rust-results.json
```

Use the Python environment prepared above. In Windows PowerShell, set `$env:AGENTPM_APDS_RESULTS = "target/apds-rust-results.json"` before running Cargo, omit the leading environment assignment, and use `.venv-apds/Scripts/python.exe`. The Rust tests invoke the actual CLI and write the result file only after assertions pass. Implementation code and tests remain outside this bundle. This optional integration requires the CLI repository's Rust toolchain; an independent implementation can produce the same test-only result projection through its own tooling.

## Test a Manager or Runner

[runner-observations.json](runner-observations.json) supplies separate scenarios with fixture references, initial conditions, actions and expected observations. Use your own runtime test harness to exercise the roles and constructs you support. The corpus checker checks these descriptions and their references; it does not run them or count them as validator successes.

The observations cover additive scopes, inherited Tools, exact selected versions, Loop restrictions, Profile applicability, direct Memory access versus lifecycle operations, Tool process/environment behavior, and Template generation. Stateful cases Q–T cover initial armed triggers, unchanged-count no-refire, rearming after a drop and interval baselines. They do not prescribe a scheduler, provider, prompt order or host wire protocol. Managers can use the selected-version graph cases for dependency identity; Runners can additionally test observable capability and execution behavior.

## Coverage and version integrity

The nine original [valid manifests](fixtures/valid/) cover all eight kinds and a dependency-free Agent. Additional positive, negative, resolved and packaged-content cases are indexed explicitly. The `rule_coverage` table in the index maps every semantic rule to validation cases, Runner observations or an explained document/reporting responsibility. It does not manufacture invalid manifests for execution-only rules.

[structural-cases.json](structural-cases.json) retains the shared Unicode description boundaries. Whitespace-only descriptions are schema errors. U+200B and U+001C are outside Unicode `White_Space`; implementations must not silently substitute their language's trimming behavior.

AgentPM's historical missing-standard behavior has a separate [compatibility index](compatibility/index.json). `legacy_inferred` is compatibility interpretation, not explicit APDS authoring conformance or fabricated historical verification. It is excluded from the normative validation cases.

[checksums.json](../checksums.json) pins the supplied schema, semantics, corpus and supporting files. The checker rejects changed or extra files. Once this version is published, changes affecting normative outcomes require a new standard version. For an unpublished candidate only, maintainers may review a controlled change and regenerate derived checksums using `python3 scripts/check_apds_schema.py --write` from the CLI repository root. Regeneration does not choose semantic expectations or authorize rewriting a published contract.
