# CLI Messaging and Exit Codes Manual Verification

Use this runbook for Milestone 9. Run each CLI command yourself and inspect its
output before continuing. Setup creates **empty directories**, not manifests,
builds, generated projects, or passing results. There is no `run section` helper
and no automatic pass/fail summary.

Your existing local registry/backend is used in Section 6. Leave it running;
connection-failure tests use a different, deliberately closed address. Nothing
is published. The optional mock in Section 7 is only for controlled HTTP errors;
it is not a replacement for the real revoked-PAT check.

## Setup and Restart

Start from the **`agentpm` repository root**, with Python 3.9+ and bash or zsh:

```bash
cargo build -p agentpm-cli
python3 -B scripts/cli_m9_manual.py setup
source cli-m9-test/env.sh
"$M9_APM" --version
cat "$M9_ROOT/fixture.json"
```

`M9_APM` is the just-built `target/debug/agentpm`, not the installed release on
your PATH. The fixture record includes its SHA-256 because the version string
alone may not distinguish this build from an older one.

The directories `knowledge/`, `memory/`, `template/`, `tool/`, and `registry/`
are empty starting points. `private-valid/`, `private-revoked/`, and
`private-anonymous/` keep the three private-registry checks separate;
`public-anonymous/` holds the public-install check. `home/`
isolates the few commands that must not read your cached credentials or keys.
`runs/` holds the output files you explicitly save. Everything is gitignored
under `cli-m9-test/`; Milestone 8 fixtures are untouched.

If your terminal restarts, return to the repo root and source the environment
again. Do **not** reset unless you want to discard your work:

```bash
source cli-m9-test/env.sh
```

To start over, stop the optional mock first, then run:

```bash
python3 -B scripts/cli_m9_manual.py setup --reset
source cli-m9-test/env.sh
```

Reset removes only this helper's owned fixture directory. Rebuilding the CLI
also calls for reset so the recorded digest describes the tested binary.

### Reading Exit Codes

Successful commands below should exit `0`. Negative tests deliberately exit `1`
(runtime errors) or `2` (usage errors). Their `if` blocks let you read the actual
exit code without enabling/disabling shell options or closing a terminal that
already has `errexit` enabled. Do not pipe these commands through `tee` and then
interpret the pipe's exit code as the CLI's.

Normal errors should be readable on stderr. Progress lines on an interactive
terminal are fine. A friendly error is **not** a pass unless its exit code also
matches. `M9_EXIT` is used instead of zsh's read-only `status` variable.

## 1. Knowledge Inspect: Fresh, JSON, and Stale

Create the package now; `agent.json` should not exist before `init`:

```bash
cd "$M9_ROOT/knowledge"
"$M9_APM" init --kind knowledge --name m9-knowledge
"$M9_APM" lint --strict
"$M9_APM" knowledge build
"$M9_APM" knowledge inspect .
```

Each command should succeed. Inspect must start with these four lines, followed
by Knowledge-specific details:

```text
Knowledge: m9-knowledge@0.1.0
Target: .
Status: fresh
Manifest: agent.json
```

No redundant `/./` or long working-directory prefix should appear in the human
header. Machine output should still contain its full manifest path:

```bash
"$M9_APM" knowledge inspect . --json > "$M9_OUT/knowledge-inspect.json"
"$M9_PYTHON" -m json.tool "$M9_OUT/knowledge-inspect.json"
"$M9_PYTHON" -c 'import json, os, pathlib; p = json.loads(pathlib.Path(os.environ["M9_OUT"], "knowledge-inspect.json").read_text()); assert pathlib.Path(p["manifest_path"]).is_absolute(); print("Full JSON manifest path retained")'
```

Change a source document without rebuilding, then inspect again:

```bash
printf '\nM9 manual source change.\n' >> knowledge/docs/context.md
"$M9_APM" knowledge inspect .
```

Expect exit `0` and overall `Status: stale`; details should explain the stale
manifest/source state rather than contradict the header with a fresh index.
Restore freshness with an explicit build:

```bash
"$M9_APM" knowledge build
"$M9_APM" knowledge inspect .
```

## 2. Memory Inspect and Missing Sidecar

```bash
cd "$M9_ROOT/memory"
"$M9_APM" init --kind memory --name m9-memory
"$M9_APM" lint --strict
"$M9_APM" memory build
"$M9_APM" memory inspect .
```

Expect exit `0` throughout and the same header structure:

```text
Memory: m9-memory@0.1.0
Target: .
Status: fresh
Manifest: agent.json
```

Memory-specific spaces, records and build details must remain present. Check
the existing JSON layout separately:

```bash
"$M9_APM" memory inspect . --json > "$M9_OUT/memory-inspect.json"
"$M9_PYTHON" -m json.tool "$M9_OUT/memory-inspect.json"
"$M9_PYTHON" -c 'import json, os, pathlib; p = json.loads(pathlib.Path(os.environ["M9_OUT"], "memory-inspect.json").read_text()); assert pathlib.Path(p["target"]["manifest_path"]).is_absolute(); print("Full JSON manifest path retained")'
```

Move the referenced schema aside, preserving it for restoration:

```bash
mv schemas/user-preference.schema.json schemas/user-preference.schema.json.saved
if "$M9_APM" memory inspect .; then M9_EXIT=0; else M9_EXIT=$?; fi
printf 'CLI exit: %s (expected 1)\n' "$M9_EXIT"
```

The error should name `user_preference`, say it references
`schemas/user-preference.schema.json`, explain that the file could not be read,
and suggest restoring the file or correcting the schema path in `agent.json`.
It should not fall back to `is invalid: reading <absolute path>`.

```bash
mv schemas/user-preference.schema.json.saved schemas/user-preference.schema.json
"$M9_APM" memory inspect .
```

Restored inspect should succeed again.

## 3. New: Actual Creation and Useful Success Output

Author the local Template with the real scaffold command, then lint it. This is
a dependency-free test fixture, not a template to publish:

```bash
cd "$M9_ROOT/template"
"$M9_APM" init --kind template --name m9-starter
"$M9_APM" lint --strict
cat agent.json
cat template/README.md
"$M9_APM" new . ./generated --var project_name=m9-generated --token apm_m9.fixture
```

Expect exit `0`, `Created project: generated`, a `Next:` step pointing to
`generated/README.md`, and the authored README-review entrypoint. The target
should not contain `/./` or an unnecessary absolute prefix. Open the actual
output:

```bash
cat generated/README.md
```

Now explicitly test quiet/noninteractive stdout using a **different target**:

```bash
"$M9_APM" new . ./generated-quiet --var project_name=m9-generated-quiet --quiet --token apm_m9.fixture > "$M9_OUT/new-quiet.stdout.txt" 2> "$M9_OUT/new-quiet.stderr.txt"
cat "$M9_OUT/new-quiet.stdout.txt"
cat "$M9_OUT/new-quiet.stderr.txt"
cat generated-quiet/README.md
```

Exit must be `0`; useful final path, README guidance and entrypoint must still
be in stdout, not disappear with progress suppression. Repeating this section
requires fresh target names or a reset; `new` should not overwrite your existing
generated project.

Check the missing-README fallback with another new target. Move the source
README outside `template/` temporarily, then restore it after generation:

```bash
mv template/README.md README.saved
"$M9_APM" new . ./generated-no-readme --var project_name=m9-no-readme --quiet --token apm_m9.fixture
mv README.saved template/README.md
```

Expect `Created project: generated-no-readme`, followed by
`Next: review the generated project before running its entrypoints.`
The authored `Review generated scaffold: cat README.md` line still prints
unchanged: the CLI does not rewrite template authors' commands. No README is
invented in the generated project. If generation fails unexpectedly, restore
`README.saved` before continuing.

## 4. Usage Errors and Unknown Local Key

Bare `serve` should reject missing usage **before starting a server**:

```bash
if "$M9_APM" serve; then M9_EXIT=0; else M9_EXIT=$?; fi
printf 'CLI exit: %s (expected 2)\n' "$M9_EXIT"
"$M9_APM" serve --help
```

Expect clap's usage/help guidance naming required `--mcp`, not the old runtime
`currently requires --mcp` message. `serve --help` should succeed.

For keys, isolate the app-data locations **for this command only**; do not
export a replacement `HOME` into your normal shell:

```bash
if env HOME="$M9_HOME" USERPROFILE="$M9_HOME" XDG_DATA_HOME="$M9_HOME/data" XDG_CONFIG_HOME="$M9_HOME/config" APPDATA="$M9_HOME/data" "$M9_APM" keys export --key-id m9-key-does-not-exist; then M9_EXIT=0; else M9_EXIT=$?; fi
printf 'CLI exit: %s (expected 1)\n' "$M9_EXIT"
```

Expect `No local key with id ... exists` and `agentpm keys list` guidance.
No duplicate `Caused by`/filesystem-exception lines or real home-directory path
should appear. No real key is created, read, or removed.

## 5. Connection Failures Across Command Routes

Leave the real backend running. `$M9_OFFLINE` is `http://127.0.0.1:1`, not your
registry. Port 1 must have no listener. If it does, choose another closed
loopback port before continuing; do not point these tests at production.
Every command here uses an explicitly fake token, overriding your cached PAT.

```bash
cd "$M9_ROOT/registry"
if "$M9_APM" --base-url "$M9_OFFLINE" install @example/m9-tool --token apm_m9.fixture; then M9_EXIT=0; else M9_EXIT=$?; fi
printf 'CLI exit: %s (expected 1)\n' "$M9_EXIT"
```

```bash
if "$M9_APM" --base-url "$M9_OFFLINE" export --skill @example/m9-tool --token apm_m9.fixture; then M9_EXIT=0; else M9_EXIT=$?; fi
printf 'CLI exit: %s (expected 1)\n' "$M9_EXIT"
```

```bash
if "$M9_APM" --base-url "$M9_OFFLINE" new @example/m9-template ./uncreated --token apm_m9.fixture; then M9_EXIT=0; else M9_EXIT=$?; fi
printf 'CLI exit: %s (expected 1)\n' "$M9_EXIT"
```

```bash
if "$M9_APM" --base-url "$M9_OFFLINE" whoami --token apm_m9.fixture; then M9_EXIT=0; else M9_EXIT=$?; fi
printf 'CLI exit: %s (expected 1)\n' "$M9_EXIT"
```

Each must say `Cannot connect`, name the loopback origin and the **user's
operation**, suggest checking network/address/server availability and retrying,
and exit `1`. In particular, export should describe exporting the requested
Skill, not lead with an internal `/v1/...` request or raw reqwest error.
Failed `new` must not announce a created project.

Check the other affected routes individually:

```bash
if "$M9_APM" --base-url "$M9_OFFLINE" login apm_m9.fixture --no-write; then M9_EXIT=0; else M9_EXIT=$?; fi
printf 'CLI exit: %s (expected 1)\n' "$M9_EXIT"
```

```bash
if "$M9_APM" --base-url "$M9_OFFLINE" namespace revoke-signer --namespace m9-fixture --signer-id m9-fixture --token apm_m9.fixture; then M9_EXIT=0; else M9_EXIT=$?; fi
printf 'CLI exit: %s (expected 1)\n' "$M9_EXIT"
```

```bash
cd "$M9_ROOT/tool"
"$M9_APM" init --kind tool --name m9-tool
"$M9_APM" lint --strict
if "$M9_APM" --base-url "$M9_OFFLINE" publish --quiet --token apm_m9.fixture; then M9_EXIT=0; else M9_EXIT=$?; fi
printf 'CLI exit: %s (expected 1)\n' "$M9_EXIT"
```

This last command may create a local archive, but cannot publish to the closed
address. All three failures must retain the same friendly error style and
exit `1`. `login --no-write` must not replace your cached token.

Compare debug output explicitly:

```bash
if env RUST_LOG=agentpm=debug "$M9_APM" --base-url "$M9_OFFLINE" whoami --token apm_m9.fixture; then M9_EXIT=0; else M9_EXIT=$?; fi
printf 'CLI exit: %s (expected 1)\n' "$M9_EXIT"
```

Expect `Registry transport failure` and underlying cause detail **in debug
output**, while the final human error stays friendly. Debug logs may be on
stdout under the existing logging setup. The normal command above should not
dump the transport chain.

## 6. Real Local Backend: Valid, Revoked, and Anonymous

This section tests the real server/client contract. The backend must include
M9's strict supplied-token handling on install resolve/init. An older backend
can turn a rejected PAT into an anonymous 404; that is not a passing result.

Confirm the address (change it if your backend uses another port):

```bash
export M9_REGISTRY=http://127.0.0.1:5000
curl --fail --show-error "$M9_REGISTRY/health"
```

In your **local website**, create a temporary PAT with `packages:read` for an
account that can access an existing private Tool package. Do not revoke your
usual token. Copy the package's real `@namespace/name@version` identifier into
`M9_PRIVATE_PACKAGE`; do not use the mock identifiers from Section 5.

Read the temporary PAT without putting its value in shell history (input is
hidden; press Enter after pasting):

```bash
printf 'Paste the temporary local-registry PAT, then press Enter: '
read -r -s M9_LOCAL_PAT
printf '\n'
export M9_PRIVATE_PACKAGE='@zack-internal/private-docs'
```

The PAT variable is used only for individual child processes, never written to
the fixture files. Do not save shell tracing or include the PAT in screenshots.
Validate it without saving credentials:

```bash
"$M9_APM" --base-url "$M9_REGISTRY" whoami --token "$M9_LOCAL_PAT"
"$M9_APM" --base-url "$M9_REGISTRY" login "$M9_LOCAL_PAT" --no-write
```

Both should exit `0` and identify the expected account. Confirm the private
package really is accessible **before** revocation:

```bash
cd "$M9_ROOT/private-valid"
"$M9_APM" --base-url "$M9_REGISTRY" install "$M9_PRIVATE_PACKAGE" --token "$M9_LOCAL_PAT"
```

Expect exit `0` and installed files. This depends on the backend's normal local
artifact storage being available. An artifact download/configuration failure is
not evidence of a successful install; resolve it or record this case as blocked.

Now revoke **only that temporary PAT** in the local website. Use a different,
still-empty install directory so a previous install cannot hide the request:

```bash
cd "$M9_ROOT/private-revoked"
if "$M9_APM" --base-url "$M9_REGISTRY" install "$M9_PRIVATE_PACKAGE" --token "$M9_LOCAL_PAT"; then M9_EXIT=0; else M9_EXIT=$?; fi
printf 'CLI exit: %s (expected 1)\n' "$M9_EXIT"
```

Expect `Credentials rejected`, guidance about valid/unrevoked PATs and
`AGENTPM_TOKEN`/`--token`/`agentpm login --paste`, and exit `1`. It must **not**
claim the known private package is missing. Backend logs should show install
resolve returning **401**, not an anonymous **404**.

Then try the **same private package anonymously**. Both environment credentials
and cached credentials must be excluded:

```bash
cd "$M9_ROOT/private-anonymous"
if env -u AGENTPM_TOKEN HOME="$M9_HOME" USERPROFILE="$M9_HOME" XDG_DATA_HOME="$M9_HOME/data" XDG_CONFIG_HOME="$M9_HOME/config" APPDATA="$M9_HOME/data" "$M9_APM" --base-url "$M9_REGISTRY" install "$M9_PRIVATE_PACKAGE"; then M9_EXIT=0; else M9_EXIT=$?; fi
printf 'CLI exit: %s (expected 1)\n' "$M9_EXIT"
```

Expect exit `1` and **not found**, not `Credentials rejected`. The server should
return **404**, preserving private-package existence hiding for anonymous users.

Finally, select an existing **public** Tool from your local registry and install
it without any token. This proves the stricter rejection rule did not make all
installs require authentication:

```bash
export M9_PUBLIC_PACKAGE='@zack/bound-agent'
cd "$M9_ROOT/public-anonymous"
env -u AGENTPM_TOKEN HOME="$M9_HOME" USERPROFILE="$M9_HOME" XDG_DATA_HOME="$M9_HOME/data" XDG_CONFIG_HOME="$M9_HOME/config" APPDATA="$M9_HOME/data" "$M9_APM" --base-url "$M9_REGISTRY" install "$M9_PUBLIC_PACKAGE"
```

Expect exit `0` and installed files. Leave the package IDs as shell variables,
not hardcoded fixture packages that may not exist in your registry.

The no-token `whoami` informational path should still exit `0`, even when the
configured server is unreachable:

```bash
env -u AGENTPM_TOKEN HOME="$M9_HOME" USERPROFILE="$M9_HOME" XDG_DATA_HOME="$M9_HOME/data" XDG_CONFIG_HOME="$M9_HOME/config" APPDATA="$M9_HOME/data" "$M9_APM" --base-url "$M9_OFFLINE" whoami
unset M9_LOCAL_PAT
```

Expect `Not authenticated` and no network request. No `login` command in this
runbook writes your credentials. If you cannot create/revoke a local PAT or do
not have a private package, leave this real-backend check pending; the optional
mock cannot satisfy it.

## 7. Optional Controlled HTTP Errors

Use this only for deterministic errors that should not be induced in your real
backend. It listens on a separate available loopback port. It serves no real
packages, validates no real tokens, and changes no database records.

In a **second terminal**, from the repo root:

```bash
source cli-m9-test/env.sh
"$M9_PYTHON" -B "$M9_HELPER" serve
```

Leave that running. In the original terminal:

```bash
source "$M9_ROOT/mock-env.sh"
cd "$M9_ROOT/registry"
"$M9_APM" --base-url "$M9_MOCK" whoami --token apm_m9_ok.fixture
```

Expect exit `0` and `m9@example.test`. Now run each failure independently:

```bash
if "$M9_APM" --base-url "$M9_MOCK" whoami --token apm_m9_decode.fixture; then M9_EXIT=0; else M9_EXIT=$?; fi
printf 'CLI exit: %s (expected 1)\n' "$M9_EXIT"
```

```bash
if "$M9_APM" --base-url "$M9_MOCK" install @private/m9-tool --token apm_m9_rejected.fixture; then M9_EXIT=0; else M9_EXIT=$?; fi
printf 'CLI exit: %s (expected 1)\n' "$M9_EXIT"
```

```bash
if "$M9_APM" --base-url "$M9_MOCK" install @private/m9-tool --token apm_m9_missing.fixture; then M9_EXIT=0; else M9_EXIT=$?; fi
printf 'CLI exit: %s (expected 1)\n' "$M9_EXIT"
```

```bash
if "$M9_APM" --base-url "$M9_MOCK" install @private/m9-tool --token apm_m9_forbidden.fixture; then M9_EXIT=0; else M9_EXIT=$?; fi
printf 'CLI exit: %s (expected 1)\n' "$M9_EXIT"
```

```bash
if "$M9_APM" --base-url "$M9_MOCK" install @private/m9-tool --token apm_m9_server.fixture; then M9_EXIT=0; else M9_EXIT=$?; fi
printf 'CLI exit: %s (expected 1)\n' "$M9_EXIT"
```

| Case | Expected result, always exit `1` |
| --- | --- |
| Malformed successful whoami response | `Invalid registry response` with retry/version guidance |
| HTTP 401 | `Credentials rejected`, not missing package |
| HTTP 404 | Not found, not rejected credentials |
| HTTP 403 | Retains 403, not classified as connection failure or rejected credentials |
| HTTP 500 | Retains 500, not classified as connection failure or rejected credentials |

Stop **only the mock** with Ctrl-C in its terminal. Your existing backend
should remain running. Do not reset fixtures while the mock is active.

## Completion Record

These are operator checks, not results prefilled by setup. Record observations
here or in your review notes, including any failed/blocked case:

- [ ] Knowledge fresh/stale headers and full-path JSON verified.
- [ ] Memory header/JSON and missing-sidecar recovery verified.
- [ ] Local `new` creation and quiet stdout verified.
- [ ] Bare `serve` exits `2`; missing local key exits `1` with useful guidance.
- [ ] All seven connection-failure routes exit `1`; debug retains detail.
- [ ] Real local PAT succeeds, then revocation gives 401/credentials rejected;
      anonymous access to the same private package gives 404/not found, and
      anonymous installation of an existing public package succeeds.
- [ ] No-token `whoami` still succeeds informationally without a network call.
- [ ] Optional mock matrix verified, or explicitly recorded as not run.

Timeout classification is covered by the existing automated timeout test, not a
manual ten-minute wait for the SDK's production request timeout. This runbook
does not retest Harness surfaces or the full Milestone 8 scaffolding matrix.
