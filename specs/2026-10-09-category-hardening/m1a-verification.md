# M1A verification record

Executed locally on **2026-11-13**, against the revisions in the [inventory](m1a-contract-inventory.md#baseline-and-classification). This audit changes internal Markdown only. No new validator, normative fixture set or public contract is introduced.

## Commands and results

Commands below run from the named repository root. All completed commands in this table exited **0**.

| Repository     | Command actually run                                                                                                                                                                                                       | Result                                                                                                                                            |
| -------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------- |
| CLI            | `cargo test -p agentpm-cli --bin agentpm manifest::`                                                                                                                                                                       | 151 passed; eight-kind schema/typed parsing, Memory/Loop/Profile/binding semantic checks, schema embedding and legacy authoring behavior          |
| CLI            | `cargo test -p agentpm-cli --test authoring --quiet`                                                                                                                                                                       | 6 passed; all eight init/lint flows, arbitrary `standard` tolerance, typo rejection, Knowledge JSON preservation, packaging/preflight diagnostics |
| CLI            | `cargo test -p agentpm-cli --bin agentpm harness_plan:: --quiet`                                                                                                                                                           | 24 passed; Agent selection, missing Loop, phase lookup, inherited Tool deduplication, runtime scope, Memory selectors, Profile/MCP readiness      |
| CLI            | `cargo test -p agentpm-cli --bin agentpm harness_engine:: --quiet`                                                                                                                                                         | 159 passed; effective phase/action restrictions, lifecycle triggers, external operations, hooks, limits and runtime behavior with test providers  |
| CLI            | `cargo test -p agentpm-cli --bin agentpm harness_config:: --quiet`                                                                                                                                                         | 11 passed; separate config schema/defaults/semantic checks, invalid implementations and paths                                                     |
| CLI            | `cargo test -p agentpm-cli --bin agentpm semver::types:: --quiet`                                                                                                                                                          | 38 passed; references, package kinds/keys, lock relationships, desired/resolved composition                                                       |
| Registry       | `REGISTRY_PRIVATE_KEY_B64=AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA= .venv/bin/python -m pytest tests/test_publish_helpers.py tests/test_install_helpers.py tests/test_semver_resolver.py -q`                            | 152 passed, 49 subtests passed; dependency extraction/closure, kind, access and publish/install helper contracts                                  |
| Registry       | `REGISTRY_PRIVATE_KEY_B64=AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA= .venv/bin/python -m pytest tests/test_tar_validation.py tests/test_namespace_helpers.py -q`                                                         | 61 passed, 42 subtests passed; bounded archive metadata/Template/Python-resolution validation and namespace helper behavior                       |
| Node SDK       | `pnpm exec vitest run test/load-agent.spec.ts test/load-skill.spec.ts test/load-knowledge.spec.ts test/load-memory.spec.ts test/load-profile.spec.ts test/load-loop.spec.ts test/harness.spec.ts`                          | 68 passed, 1 skipped; installed composition metadata and machine initialization/registration/transport                                            |
| Python SDK     | `.venv/bin/python -m pytest tests/test_load_agent.py tests/test_load_skill.py tests/test_load_knowledge.py tests/test_load_memory.py tests/test_load_profile.py tests/test_load_loop.py tests/test_harness.py -q --no-cov` | 69 passed, 1 skipped (70 collected); equivalent loader/machine boundaries. Existing semver deprecation warnings remain                            |
| CLI + Examples | `../agentpm-api/.venv/bin/python /tmp/agentpm-m1a-probe.py` using the exact probe below                                                                                                                                    | 21 schema/real-CLI edge probes and 17 authored-source checks passed; source bytes unchanged                                                       |

The all-zero Registry key is a public, test-only seed supplied to the existing tests; it is not a live signing key. No production credentials are used.

One initial command, `cargo test -p agentpm-cli --lib manifest:: -- --nocapture`, exited **101** because this package has a binary target, not a library target. The corrected `--bin agentpm` command above passed. The SDK optional real-Harness-workspace tests were skipped because `AGENTPM_HARNESS_CLI`/`AGENTPM_HARNESS_WORKSPACE` were not supplied. Rust's real engine/plan tests ran; this audit does not claim those optional SDK tests, hosted CI, production publishing or cross-platform acceptance passed. Runtime product code is unchanged, so no new full release-band verification is required by this inventory.

## Targeted negative and boundary evidence

### Review addition: Tool environment precedence

Source inspection at `runner.rs:761–778` confirms ambient environment → defaults for absent keys → entrypoint overlay → caller overlay. This is recorded as **P** in the [environment resolution inventory](m1a-contract-inventory.md#tool-environment-resolution), with post-merge managed-Python isolation recorded separately. The required-variable check uses key presence after those steps; an empty existing value does not activate a default.

Additional commands run from the CLI repository root, both exit **0**:

- `cargo test -p agentpm-cli --bin agentpm runner::tests:: --quiet` — **24 passed**, including required-variable failure, defaults, interpreter overrides and legacy `PYTHONPATH` behavior.
- `cargo test -p agentpm-cli --test run_machine sdk_request --quiet` — **1 passed**, proving a caller override wins over `entrypoint.env` through the real SDK-request CLI path.

These existing tests and source inspection substantiate the inventory addition; a dedicated portable four-layer conformance vector remains M1C/M2A work. No execution-policy or schema edit is included.

### Review addition: Tool process invocation

The [Tool invocation inventory](m1a-contract-inventory.md#tool-invocation-contract) now records JSON input on stdin through EOF, JSON output on stdout, separate stderr diagnostics, zero exit as a necessary success condition, input/output schema checks, and transport failure versus schema-valid domain-result data. It traces `ERR_TOOL_OUTPUT_NOT_JSON` and records the SDK historical stdout adapter separately from portable semantics and from the outer SDK-request/machine-envelope transport.

`cargo test -p agentpm-cli --test run_machine --quiet` — **8 passed**, exit **0**. This exercises real subprocess stdin, stdout envelopes/stderr diagnostics, schema rejection, output limits and the SDK compatibility path. The one SDK-request test run for the preceding addition is included in these eight, not counted twice. The previously executed 24 `runner::tests::` tests additionally cover malformed child stdout, unsuccessful exit, output-schema validation, strict versus SDK stdout parsing and domain-failure-as-data behavior.

The local CLI total across the audit and both review additions is **421 unique tests passed**. No runtime code or schema changes were made. Portable invocation rules and dedicated conformance vectors remain M1C/M2A work.

### Authored-shape probes

The executable probe independently applies Python Draft 2020-12 and the existing Rust CLI lint to the same inputs. It intentionally tests **current** behavior; it is not a proposed APDS conformance suite.

| Observation                                                          | Schema      | Normal CLI lint                                       |
| -------------------------------------------------------------------- | ----------- | ----------------------------------------------------- |
| Agent with `tools: []`                                               | Valid       | Pass                                                  |
| Agent without `tools`                                                | Invalid     | Fail                                                  |
| Scoped or uppercase local `name`                                     | Invalid     | Fail                                                  |
| Pinned string/object selector, omitted version, major.minor selector | Valid       | Pass                                                  |
| Authored caret selector                                              | Invalid     | Fail                                                  |
| Binding to declared Tool                                             | Valid       | Pass                                                  |
| Binding to undeclared Tool                                           | Valid shape | Semantic failure                                      |
| Versioned binding identity                                           | Invalid     | Fail                                                  |
| Phase bindings without Loop dependency                               | Valid shape | Semantic failure                                      |
| Unknown phase in unresolved Loop                                     | Valid shape | Pass; resolved phase lookup belongs later             |
| Operation-only Memory binding                                        | Valid       | Pass without resolving Blueprint                      |
| Blank root description                                               | Valid       | Warning; separate `--strict` probe fails              |
| Arbitrary JSON `standard`                                            | Valid       | Pass; not validated as a declaration                  |
| Misspelled `standrad`                                                | Invalid     | Fail                                                  |
| `environment` on an Agent                                            | Valid       | Pass; no root kind restriction                        |
| Traversing `readme` string                                           | Valid shape | Pass; this does not assert packaging accepts the path |

The 17 real-source checks cover all eight kinds and local consumer manifests. Sixteen pass both schema and CLI lint, including their local Memory source-schema checks. `skill-workflow-slack-post-message/agent.json` is deliberately retained as a negative legacy/application example: it lacks the full required publish identity, so both reject it. No source is repaired or rewritten to make this audit pass.

## Reproduce the focused probe

From the **CLI repository root**, with the sibling Examples and Registry repositories and their existing dependencies available, build the CLI if needed (`cargo build -p agentpm-cli`). Save the following Python block as `/tmp/agentpm-m1a-probe.py`, then run the command recorded above. Temporary manifests/config directories are removed automatically; the original examples are linted without `--fix`, and their bytes are checked unchanged.

```python
import copy
import json
import os
from pathlib import Path
import subprocess
import tempfile

from jsonschema import Draft202012Validator

cli = Path.cwd().resolve()
schema = cli / "schemas/agentpm.manifest.schema.json"
validator = Draft202012Validator(json.loads(schema.read_text()))
agent = {
    "kind": "agent",
    "name": "inventory-agent",
    "version": "1.0.0",
    "description": "Existing contract audit",
    "tools": [],
}
cases = []


def case(label, changes=None, remove=(), shape=True, lint=True, strict=False):
    value = copy.deepcopy(agent)
    value.update(changes or {})
    for key in remove:
        value.pop(key)
    cases.append((label, value, shape, lint, strict))


case("empty-tools-agent")
case("missing-tools-agent", remove=["tools"], shape=False, lint=False)
case("scoped-local-name", {"name": "@zack/inventory-agent"}, shape=False, lint=False)
case("uppercase-local-name", {"name": "Inventory"}, shape=False, lint=False)
case("pinned-string-selector", {"tools": ["@zack/tool@1.2.3"]})
case("object-selector", {"tools": [{"name": "@zack/tool", "version": "1.2.3"}]})
case("omitted-selector", {"tools": ["@zack/tool"]})
case("major-minor-selector", {"tools": ["@zack/tool@1.2"]})
case("range-selector", {"tools": ["@zack/tool@^1.2.3"]}, shape=False, lint=False)
case(
    "binding-declared",
    {"tools": ["@zack/tool@1.2.3"], "bindings": {"global": {"tools": ["@zack/tool"]}}},
)
case(
    "binding-undeclared",
    {"bindings": {"global": {"tools": ["@zack/tool"]}}},
    lint=False,
)
case(
    "binding-versioned",
    {
        "tools": ["@zack/tool@1.2.3"],
        "bindings": {"global": {"tools": ["@zack/tool@1.2.3"]}},
    },
    shape=False,
    lint=False,
)
case(
    "phase-requires-loop",
    {
        "bindings": {"phases": {"inspect": {"tools": ["@zack/tool"]}}},
        "tools": ["@zack/tool"],
    },
    lint=False,
)
case(
    "unknown-phase-unresolved-at-lint",
    {
        "loop": "@zack/loop@1.0.0",
        "tools": ["@zack/tool"],
        "bindings": {"phases": {"missing-phase": {"tools": ["@zack/tool"]}}},
    },
)
case(
    "operation-only-memory-binding",
    {
        "memory": ["@zack/state@1.0.0"],
        "bindings": {
            "global": {
                "memory": [{"package": "@zack/state", "operations": ["compact"]}]
            }
        },
    },
)
case("blank-description-warning", {"description": "  "})
case("blank-description-strict", {"description": "  "}, lint=False, strict=True)
case("arbitrary-standard-tolerance", {"standard": ["not", "a", "declaration"]})
case("misspelled-standard", {"standrad": {}}, shape=False, lint=False)
case(
    "environment-on-agent",
    {"environment": {"vars": {"TOKEN": {"required": True, "description": "Token"}}}},
)
case("readme-traversal-shape-only", {"readme": "../README.md"})

with tempfile.TemporaryDirectory(prefix="agentpm-m1a-audit-") as temporary:
    root = Path(temporary)
    environment = dict(os.environ, AGENTPM_TELEMETRY="0", NO_COLOR="1")
    for key in ["XDG_CONFIG_HOME", "XDG_CACHE_HOME", "APPDATA", "LOCALAPPDATA"]:
        folder = root / key
        folder.mkdir()
        environment[key] = str(folder)
    for label, value, shape, lint, strict in cases:
        assert validator.is_valid(value) == shape, (label, "schema expectation")
        (root / "agent.json").write_text(json.dumps(value), encoding="utf-8")
        command = [
            str(cli / "target/debug/agentpm"),
            "lint",
            "--schema",
            str(schema),
            "--format",
            "json",
        ]
        if strict:
            command.append("--strict")
        result = subprocess.run(
            command,
            cwd=root,
            env=environment,
            capture_output=True,
            text=True,
            timeout=30,
        )
        assert (result.returncode == 0) == lint, (label, result.stdout, result.stderr)
        print(f"PASS {label}: schema={shape} lint={lint}")

examples = cli.parent / "agentpm-examples"
selected = [
    "agent-packages/devwork-python",
    "tools-python/sentiment-analysis",
    "tools-python/summarize-text",
    "tools-node/slack-post-message",
    "skill-packages/issue-triage-playbook",
    "skill-packages/incident-handoff-checklist",
    "knowledge-packages/devwork-maintainer-guide",
    "knowledge-packages/agentpm-docs",
    "memory-packages/devwork-maintainer-state",
    "memory-packages/conversation-continuity",
    "profile-packages/devwork-maintainer-style",
    "loop-packages/devwork-triage-loop",
    "loop-packages/react-loop",
    "template-packages/harness-sdk-host-python",
    "template-packages/harness-sdk-host-node",
    "agent-app-python",
    "skill-workflow-slack-post-message",
]
for relative in selected:
    path = examples / relative / "agent.json"
    before = path.read_bytes()
    value = json.loads(before)
    expected = relative != "skill-workflow-slack-post-message"
    assert validator.is_valid(value) == expected, (
        relative,
        list(validator.iter_errors(value)),
    )
    result = subprocess.run(
        [str(cli / "target/debug/agentpm"), "lint", "--schema", str(schema), str(path)],
        cwd=cli,
        env=dict(os.environ, AGENTPM_TELEMETRY="0", NO_COLOR="1"),
        capture_output=True,
        text=True,
        timeout=30,
    )
    assert (result.returncode == 0) == expected, (
        relative,
        result.stdout,
        result.stderr,
    )
    assert path.read_bytes() == before, relative
    print(f"PASS source {relative}: schema/lint={expected}; unchanged")
print(
    f"PASS {len(cases)} edge probes; {len(selected)} source manifests. No Registry calls or source rewrites."
)
```

## Source-index and handoff checks

The manifest index covers all **78** `$defs`; the Harness config index covers all **36**. Every definition link resolves to its own source header, rather than a similarly named root property. The root `oneOf`, kind-specific applicability and required metadata are included. Both schema byte digests are recorded in the index.

The CLI repo has no configured Markdown formatter. The new audit documents use the workspace's existing Node SDK Prettier executable; the embedded reproduction block uses the Python SDK's Black executable. Tracking-file edits remain narrow rather than reformatting the full stage plan. Final source-link/schema-index and formatter results are recorded below.

- Source-link audit: **258 local links** resolve, all **114 definition links** point to their exact schema headers, and both recorded schema digests match unchanged source bytes. Exit **0**.
- Reproduction audit: the Python block above matches the formatted script actually executed, byte for byte. Exit **0**.
- `../agentpm-sdk-node/node_modules/.bin/prettier --check --prose-wrap preserve --embedded-language-formatting off specs/2026-10-09-category-hardening/m1a-contract-inventory.md specs/2026-10-09-category-hardening/m1a-schema-index.md specs/2026-10-09-category-hardening/m1a-verification.md specs/2026-10-09-category-hardening/blog-brief.md` — passed, exit **0**.
- `../agentpm-sdk-python/.venv/bin/black --check /tmp/agentpm-m1a-probe.py` — passed, exit **0**.
- `git diff --check` — passed, exit **0**. All other inspected repositories remain clean.

## Acceptance and operational implications

- **S2-AC-01:** M1A baseline and source map supplied. Full immutable APDS/schema/semantics/fixtures/readability acceptance belongs to later milestones and is **not met by this audit**.
- **S2-AC-03:** Existing eight-kind and Phase 6/7 interpretation documented and tested. Removing the declared-Tools requirement and making root descriptions strictly nonblank are later changes, not hidden M1A edits.
- **D01/D02/D19:** bounded inventory decisions and remaining owning milestones are recorded in the [decision table](m1a-contract-inventory.md#decisions-and-acceptance-boundary).
- **Migrations/private/legacy:** no migration, flag, access-policy or stored-release mutation. Historical release references use previously recorded operator evidence; no new production archive download is claimed. Missing-standard and local consumer examples remain distinct from future authored conformance.
- **Stage 1 shared contracts:** inspected but unchanged: Python resolution, payload classification, release schema, canonicalization, trust roots/signatures, target selection, locks v2/v3/v4 and SDK CLI delegation.
- **M1A review gate complete:** The user confirmed Claude's independent review passed on **2026-11-13**, with no remaining findings after the environment-precedence and Tool-invocation additions. M1A is complete; later APDS acceptance work is unchanged.
