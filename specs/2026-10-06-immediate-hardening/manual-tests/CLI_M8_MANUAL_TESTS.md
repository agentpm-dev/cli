# CLI Scaffolding and Lint Manual Verification

Use this runbook for Milestone 8. The helper generates disposable fixtures, runs
the real compiled CLI, saves complete stdout/stderr and exit codes, and checks
the expected result of each section. You run one section at a time and inspect
the output and generated files before continuing.

Nothing is published. No provider keys, registry account, local database, or
second terminal are required. Section 8 starts and stops its own loopback-only
mock registry on an available port; it tests the real install client, not the
production registry. Fixtures are test artifacts, not publishable examples.

## Setup

Run from the **`agentpm` repo root**, using Python 3.9 or newer:
`--reset` replaces any previous `cli-m8-test/` fixtures and results, including
the builder's smoke-test output. Omit it to refuse replacement instead.

```bash
cargo build -p agentpm-cli
python3 -B scripts/cli_m8_manual.py setup --reset
source cli-m8-test/env.sh
```

If `python3` is older than 3.9, substitute a newer installed interpreter for the
setup command. The generated `M8_PYTHON` points to that interpreter.

The helper defaults to `target/debug/agentpm`, **not** the installed `agentpm`
on your PATH. The installed release may have the same version string without
these changes. `fixture.json` records the exact binary path, version and SHA-256;
rebuilding a different binary requires regenerating the fixtures.

Setup creates `cli-m8-test/`, which is gitignored:

| Directory | Purpose |
| --- | --- |
| `baseline/` | Real `init` output for all eight kinds, used to construct cases |
| `scaffolds/` | Separate directories for the fresh-init section |
| `tool/`, `existing-source/` | Runnable source, dry-run archive, source-preservation case |
| `missing-file/`, `missing-directory/`, `missing-glob/` | Packaging failure cases |
| `invalid/` | Invalid manifests for the union-diagnostic cases |
| `standard/` | Reserved metadata and unknown-root-key cases for every kind |
| `knowledge/`, `knowledge-forward/`, `install/` | Manifest write-back cases |
| `runs/` | Per-command output, exit-code metadata, section results and final summary |

To regenerate, explicitly discard **only this helper's** fixtures and results:

```bash
python3 -B scripts/cli_m8_manual.py setup --reset
source cli-m8-test/env.sh
```

After a reset, rerun all eight sections for a complete summary. Without a reset,
sections are independent and can be repeated. Repeating one section replaces
its evidence and invalidates the previous final summary.

### Reading Results

Every command is printed with its directory and arguments. Each section should
finish with `PASS: <section>` and the **helper** should exit `0`.

Negative cases intentionally make the **CLI** exit `1`, so seeing `Error: Lint
failed`, a failed-manifest heading, or a missing-file error is expected there.
The helper checks that the exit was exactly `1` before calling the case passed.
An unexpected result makes the helper exit `1` and leaves a failed section
record. These commands do not enable `set -e`, use zsh's read-only `status`
variable, or pipe away the command's exit status.

Each command produces `<label>.stdout.txt`, `<label>.stderr.txt`, and
`<label>.command.json` under `$M8_OUT`. The command record includes stdin, actual
and expected exit codes, and the tested binary's digest. The large-value machine
case truncates only the helper's terminal preview; its output files are complete.

## 1. Fresh Scaffolds: All Eight Kinds

```bash
"$M8_PYTHON" -B "$M8_HELPER" run scaffolds
```

Runs `init --kind <kind> --name m8-<kind>`, `lint --strict`, and
`lint --strict --format json` in each directory under `$M8_ROOT/scaffolds`.

Check:

- Tool, Agent, Template, Skill, Knowledge, Memory, Profile and Loop all pass.
- Pretty lint identifies `agent.json` as valid without warnings.
- JSON reports contain `"ok": true` and `"issues": []`.
- None of the generated manifests emits `standard`.

For example, inspect `$M8_ROOT/scaffolds/tool/agent.json` and
`$M8_OUT/lint-json-memory.stdout.txt`.

## 2. Tool Source, Runtime, Preservation and Archive

```bash
"$M8_PYTHON" -B "$M8_HELPER" run tool
```

Runs the source with JSON stdin using your setup Python, repeats Tool init over
an existing source file, and runs `publish --dry-run` with an explicitly fake
token. The token is needed only to pass credential selection; dry-run never
contacts the registry.

Check:

- `$M8_ROOT/tool/agent.json` declares `runtime: {"type":"python","version":"3"}`,
  `files: ["main.py"]`, and `entrypoint: {"command":"python3","args":["main.py"]}`.
- The source echoes `{"message":"hello","nested":{"count":2}}` as JSON.
- Init says `Kept existing .../main.py` in the preservation case. That file still
  contains the `M8 existing source: keep me` comment.
- The archive is `$M8_ROOT/tool/target/agentpm/m8-tool-0.1.0-python3.tar.gz`.
  The helper checks that it contains the unchanged manifest and source.

Optional direct archive inspection:

```bash
tar -tzf "$M8_ROOT/tool/target/agentpm/m8-tool-0.1.0-python3.tar.gz"
```

This verifies the runtime declaration, not registry search itself: nothing is
published for a live Runtime facet check.

## 3. Missing Files, Directories and Literal Globs

```bash
"$M8_PYTHON" -B "$M8_HELPER" run packaging
```

Each manifest is structurally valid and passes strict lint. Packaging is the
separate filesystem check: all three `publish --dry-run` commands must exit `1`.

Check that each error names the affected `manifest.files` entry, says it does
not exist, and tells the author to create it or correct the literal path. The
`*.py` case must fail even though `main.py` exists: globs are not expanded.
No archive or temporary artifact should remain in any of these cases' output
directories. See `missing-*-publish.stderr.txt` under `$M8_OUT`.

## 4. Human Diagnostics and Publish Context

```bash
"$M8_PYTHON" -B "$M8_HELPER" run diagnostics
```

All lint commands in this section must exit `1`. Inspect the printed output
against this table; the helper also asserts the pointers and message fragments.

| Case | Expected diagnostic |
| --- | --- |
| `missing-tools` | Missing required `tools` at `/tools`, schema `/oneOf/0/required` |
| `package-ref` | Accepted package-reference string and object forms at `/tools/0` |
| `memory-operation` | `consolidate, transform, delete` at `/memory/operations/refresh/type`, shown once |
| `memory-trigger` | Accepted trigger types including `record_count`, at the trigger's `/type` |
| `loop-transition` | Semantic invalid-transition message first, at `/loop/transitions/0/to`; no dumped Loop object |
| `loop-policy` | Accepted Tool failure actions including `retry` |
| `knowledge-mode` | Accepted modes `context` and `vector` |
| `memory-binding` | Missing selector guidance: `spaces or operations` |
| `entrypoint` | Valid interpreter guidance at `/entrypoint/command`; runtime mismatch can also appear |
| `large-value` | A concise package-reference error; no `M8_LARGE_ECHO_MARKER` or huge object in human output |

Each output must identify `agent.json` and the failing instance path, not start
with an unattributed error. Distinct failed constraints can still be shown;
identical errors should not repeat.

The helper also runs the invalid Loop through `publish --dry-run` and checks
that its stderr includes the exact same contextual lint block. Compare
`diagnostic-loop-transition.stdout.txt` with `publish-context.stderr.txt`.

## 5. JSON and NDJSON Contracts

```bash
"$M8_PYTHON" -B "$M8_HELPER" run machine
```

Runs four invalid manifests through both machine formats. Check:

- CLI exit `1`, but stdout is valid JSON/NDJSON; the final CLI error is on stderr.
- Reports retain `file`, `ok`, and `issues`.
- Normal issues retain `file`, `level`, `message`, `instance_path`, `schema_path`.
- No human-only fields or unpopulated `rule_id` appear.
- The Agent missing `tools` still points to `/tools` and `/oneOf/0/required`.
- The large-value machine files **do** retain the raw marker. Human truncation
  must not silently discard machine detail; large raw messages here are expected.

External rule-ID rendering is covered by the Rust tests, not simulated as a new
CLI feature: no APDS rules or rule-ID authoring option were introduced here.

## 6. Reserved Metadata Without Opening the Root

```bash
"$M8_PYTHON" -B "$M8_HELPER" run standard
```

For every kind, the helper tests `standard` as an object, string, null, boolean,
array and number. All must pass strict lint with no issues or manifest mutation.
Each `standrad` and `tool` unknown-root-key case must instead exit `1` with an
`/additionalProperties` diagnostic. This section is intentionally verbose.

This is acceptance and preservation only, not standard validation or enforcement.
The final manifests in `standard/<kind>/` contain the opaque object example.

## 7. Knowledge Build Write-Back

```bash
"$M8_PYTHON" -B "$M8_HELPER" run knowledge
```

Two context-mode Knowledge packages are built locally with no embedding service.
`knowledge build` already writes by default; there is no `--write` flag.

Check `before.json`, `after.json`, and `agent.json` in both case directories:

- The normal case accepts and preserves `standard` under the bundled schema.
- The forward-field case first fails strict lint when explicitly using the
  repository's unmodified schema: `future_extension` is not an allowed root key.
- Its local `schemas/agentpm.manifest.schema.json` allows only that extra test
  field. With this existing schema override, lint/build succeed and preserve
  both `standard` and `future_extension`.
- Both builds actually update context metadata with `document_count: 1`.

The custom schema is solely a write-back regression fixture. It does not imply
that production manifests accept arbitrary root fields.

## 8. Install Write-Back Through a Local Registry

```bash
"$M8_PYTHON" -B "$M8_HELPER" run install
```

The helper starts a localhost registry on port `0` (an available ephemeral port),
serves a real SHA-256-verified Skill archive, runs the real CLI's
`install @manual/m8-skill@0.1.0 --refresh`, and stops the server even on failure.
`@manual/m8-skill` exists only in this fixture; do not publish it or try to install
it from the real registry. No separate server terminal is needed.

Check:

- CLI exit `0` and `PASS: install`.
- `$M8_ROOT/install/agent.json` now declares that Skill at version `0.1.0`.
- `standard` and `future_extension` match `before.json` exactly in `after.json`.
- The downloaded Skill has `SKILL.md` under `install/.agentpm/skills/`.
- `$M8_OUT/install-requests.json` contains resolve, init, artifact download, and
  finalize requests. This proves the write-back followed a real download flow.

The extra root field deliberately tests lossless write-back. Install does not
make that field schema-valid; do not expect this fixture to pass ordinary lint
afterward. A firewall must allow the CLI to reach this temporary loopback server.

## Finish

After inspecting all eight sections:

```bash
"$M8_PYTHON" -B "$M8_HELPER" summary
```

Expected: helper exit `0`, `"status": "passed"`, all eight sections listed, and
`$M8_OUT/summary.json` written. Missing sections, incomplete command sets, failed
commands, stale binary evidence, or missing output files must fail the summary.

The summary records command/assertion results, not your visual approval. Record
that separately below. Results are disposable; copy the needed evidence elsewhere
before using `setup --reset` if you need to retain it.

### Operator Record

- Tested date:
- CLI path and SHA-256 (`fixture.json`):
- Sections inspected:
- Readability or unexpected-output notes:
- Remaining issues:

Recording the actual **released** CLI version carrying `standard` tolerance is
still a release-time task. A successful local walkthrough does not complete it.

## Optional: Blog Sitemap Follow-Up

This is separate from the eight-section CLI summary. With your API and web app
running normally, open `/sitemap.xml` and follow its `/sitemap/pages.xml` entry.
Confirm that `/blog` and an individual `/blog/<known-published-slug>` are present.
Use an actual post from `/blog`, not a fabricated slug. No account or sign-in is
needed, and private registry pages must remain absent.
