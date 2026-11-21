# M5B: Frozen Graph Replay in Visible Local Projects

Claude review passed with no findings. The operator confirmed all hosted CI
passed on **2026-11-20**. The operator subsequently supplied a passing
**51-result** manual audit, completing M5B. These instructions remain available
for repeat verification; Release Band 2 rollout remains separate.

Run one block at a time from the stated directory. Every CLI input, archive,
lock, intentional defect and captured result lives under the gitignored
**`agentpm/cli-m5b-test/<timestamp>-<pid>/`** directory. No `/tmp` navigation is
needed. Open that directory in your editor and keep Terminal A open.

There are two distinct surfaces:

- **Real local API:** your IntelliJ-launched API, local Docker application DB and
  development S3. We reuse the four already-published M5A Tool/Skill/legacy
  releases. Six new declared fixtures are published: Knowledge, Memory, Profile,
  Loop, a composed Agent and an empty Agent. New publications and install
  bookkeeping write to your local database through HTTP. The helper's direct
  database connection is read-only. No production calls or maintained-example
  changes occur.
- **Controlled faults:** a separate loopback server provides the same signed
  fixture archives with deliberately inconsistent responses, or a controlled
  unavailable response. It never calls your real API, database or S3. This lets
  us test states the enforcing Registry normally refuses to publish.

Use Node **20+**, Python **3.11+**, your existing API virtualenv, the reviewed CLI,
and the retained M5A run. All artifacts here use the **single-archive protocol**;
`case.json` calls that protocol `legacy`, but all six new manifests explicitly
have `standard`. The one missing-declaration Tool is reused from M5A unchanged.
Native Python, multi-artifact/target selection and provenance contract coverage
remain the recorded automated/native CI evidence. No model or paid-service call
is needed. Keep local enforcement **enabled throughout this run**.

| Section | Input and intended result                                                                  |
| ------- | ------------------------------------------------------------------------------------------ |
| 1–2     | Prepare visible sources; publish six valid local fixtures                                  |
| 3       | Direct Agent: eight exact packages return after two clean frozen installs                  |
| 4       | Direct/local Skill, local Agent and multiple workspace roots replay correctly              |
| 5       | Empty local/workspace/direct Agents succeed without an empty resolve crash                 |
| 6       | Two installed versions of the same Tool produce distinct version outputs                   |
| 7       | Start the separate fault server; establish a verified baseline                             |
| 8       | Changed ranges, removed dependencies and local identity drift fail before HTTP             |
| 9       | Wrong kind, hidden transitive dependency, stale edge, digest and origin faults fail safely |
| 10      | Cache reuse, clean replay, cache repair and refresh repair retain exact bytes              |
| 11      | Metadata-free v2/v3 locks remain readable and byte-identical                               |
| 12      | Audit saved results, installed bytes and real local publication records                    |

A negative test passes only when its **actual error matches the defect**. An
unrelated 401/403/429, missing executable or unexpected 500 is a stop condition.
`m5b_expect` prints actual/expected exits and saves both output streams;
`check-result` also requires the stated machine error. Do not continue after a
mismatch or rebuild the CLI partway through this run.

## 1. Prepare once, in Terminal A

Start at the **CLI repository root**. The retained M5A path below is the run you
completed earlier; it is read/copied, never changed. If you moved that run, change
`M5B_REUSE` to its actual directory containing `session.json` and `cases/`.

```bash
cd /Users/zackhine/projects/agentpm-project/agentpm
cargo build --locked -p agentpm-cli
umask 077
export M5B_REPO="$PWD"
export M5B_API="$M5B_REPO/../agentpm-api"
export M5B_PYTHON="$M5B_API/.venv/bin/python"
export M5B_APM="$M5B_REPO/target/debug/agentpm"
export M5B_FIXTURE="$M5B_REPO/specs/2026-10-09-category-hardening/manual-tests/fixtures/m5b/fixture.py"
export M5B_ROOT="$M5B_REPO/cli-m5b-test/$(date -u +%Y%m%dT%H%M%SZ)-$$"
export M5B_REUSE="$M5B_REPO/cli-m5a-test/20261120T170444Z-21931"
export M5B_BASE='http://127.0.0.1:5000'
export M5B_NAMESPACE='zack'
export M5B_DATABASE_URL='postgresql://postgres:postgres@127.0.0.1:5433/postgres'

"$M5B_PYTHON" --version
node --version
ls "$M5B_REUSE/session.json"
"$M5B_PYTHON" -B "$M5B_FIXTURE" prepare "$M5B_ROOT" \
  --namespace "$M5B_NAMESPACE" --cli "$M5B_APM" --reuse "$M5B_REUSE"
source "$M5B_ROOT/paths.sh"
cd "$M5B_ROOT"
pwd
ls cases reused projects controlled evidence
git -C "$M5B_REPO" check-ignore "$M5B_ROOT/author.seed"
m5b_expect version 0 env AGENTPM_TELEMETRY=0 "$M5B_APM" --version
cat session.json
```

Expected: CLI **0.1.47**, six new `m5b-…` names and four reused `m5a-…` refs.
`check-ignore` prints the private seed path. Knowledge/Memory were built by the
CLI; new sources and editable consumers were linted. Preparation created archives
and author signatures **only**. No API publication, DB write or install has run.
Builder-test directories may appear briefly while tests run; use your exported
`M5B_ROOT`, not a different timestamp or a `builder-*` directory.

Open these files to see the actual inputs:

```bash
cat cases/agent/source/agent.json
cat reused/skill/source/agent.json
cat cases/knowledge/source/agent.json
cat cases/memory/source/memory/build.json
cat cases/empty-agent/source/agent.json
```

The Agent directly selects Echo v2 and a Skill; that Skill selects Echo v1.
Knowledge, Memory, Profile and Loop supply the remaining kinds. `catalog.json`
records each archive digest and its declared/inferred origin. `reused/` holds
copies of the immutable M5A source/archive/signature evidence, not installed
files. `projects/` contains the consumers you will edit or install into.
`author.seed` is a disposable private signing seed. Keep this ignored directory
private. The generated `paths.sh` contains paths/configuration, **not your PAT**,
and sources the visible `functions.sh` helpers.

## 2. Use your real IntelliJ API and publish six declared fixtures

Run/restart your normal local API from IntelliJ with the reviewed code, its
existing local AWS/signing settings, Docker DB and **`agentpm-dev-tools`** bucket.
Keep `APDS_PUBLISH_ENFORCED=true`; do not toggle it for M5B. If your DB differs
from `localhost:5433/postgres`, correct `M5B_DATABASE_URL` in Terminal A and keep
that export when resuming. No API server wrapper is required.

In Terminal A, still at **`$M5B_ROOT`**:

```bash
read -s 'M5B_TOKEN?Your real LOCAL development PAT: '
printf '\n'
export M5B_TOKEN
m5b_expect whoami 0 m5b whoami
m5b_fixture preflight
m5b namespace add-signer --namespace "$M5B_NAMESPACE" \
  --label 'M5B disposable manual signer' --pubkey "$M5B_ROOT/author-public-key.txt"
```

Expected: your public development namespace exists, APDS columns are present,
and all four reused identities resolve with their **original digests and
origins**. If you reset your local DB or disabled M5A's signer, stop and restore
that local test prerequisite. Do not republish or edit the old versions.
The newly registered signer is for the six new local publications only.

Publish one at a time, dependencies before the Agent:

```bash
m5b_fixture describe knowledge
m5b_expect publish-knowledge 0 m5b_fixture publish knowledge
m5b_fixture inspect-db knowledge

m5b_fixture describe memory
m5b_expect publish-memory 0 m5b_fixture publish memory
m5b_fixture inspect-db memory

m5b_fixture describe profile
m5b_expect publish-profile 0 m5b_fixture publish profile
m5b_fixture inspect-db profile
```

Then:

```bash
m5b_fixture describe loop
m5b_expect publish-loop 0 m5b_fixture publish loop
m5b_fixture inspect-db loop

m5b_fixture describe agent
m5b_expect publish-agent 0 m5b_fixture publish agent
m5b_fixture inspect-db agent

m5b_fixture describe empty-agent
m5b_expect publish-empty-agent 0 m5b_fixture publish empty-agent
m5b_fixture inspect-db empty-agent
```

Expected: six successful reserve → dev-S3 upload → finalize flows, trusted author
signatures, stored `declared` origin and actual Registry conformance evidence.
The archive and source `agent.json` agree. No missing-selector seed is created.
A 429 means wait for `Retry-After`/reset and rerun that same publication step;
never count it as a successful test or repack an existing reservation. Completed
receipts are retained on rerun. Do not use `--require-attestation` for these
installs: the bundled trust root pins production, not your local Registry.

## 3. Direct Agent: restore the whole graph twice

**Input:** a published Agent with eight exact package entries: Agent, Skill,
Knowledge, Memory, Profile, Loop and Echo v1/v2. The direct consumer directory
starts without an `agent.json`; the requested Registry artifact is the root.
This differs from the editable local Agent in section 4.

```bash
cd "$M5B_ROOT/projects/direct-agent"
pwd
ls -a
m5b_expect agent-install 0 m5b install "$(m5b_ref agent)" --require-signature --machine
cat agent.lock
m5b_fixture check-lock direct-agent

m5b_fixture snapshot direct-agent agent-replay
m5b_fixture clean direct-agent
ls -a
m5b_expect agent-replay 0 m5b install "$(m5b_ref agent)" --frozen --require-signature --machine
m5b_fixture unchanged direct-agent agent-replay
m5b_fixture check-lock direct-agent

m5b_fixture snapshot direct-agent agent-replay-again
m5b_fixture clean direct-agent
m5b_expect agent-replay-again 0 m5b install "$(m5b_ref agent)" --frozen --require-signature --machine
m5b_fixture unchanged direct-agent agent-replay-again
```

Expected: every install exits **0** with machine `status: "success"`. `clean`
removes only this owned project's `.agentpm/`, including its cache; it keeps the
lock and authored files. Both replays recreate every original payload byte and
leave `agent.lock` byte-identical. The checker verifies the complete expected
set, each kind/version/digest and actual installed files. Echo v1 cannot be lost
just because it is inherited through a Skill.

## 4. Direct/local Skill, editable Agent and multiple workspace roots

**Direct Skill input:** the Skill itself and its inherited Echo v1 Tool.

```bash
cd "$M5B_ROOT/projects/direct-skill"
m5b_expect skill-install 0 m5b install "$(m5b_ref skill)" --require-signature --machine
m5b_fixture check-lock direct-skill
m5b_fixture snapshot direct-skill skill-replay
m5b_fixture clean direct-skill
m5b_expect skill-replay 0 m5b install "$(m5b_ref skill)" --frozen --require-signature --machine
m5b_fixture unchanged direct-skill skill-replay
m5b_fixture check-lock direct-skill
```

Expected: two packages, with the root still **Skill**, never relabeled Tool.

**Editable local Agent input:** the same component graph plus the unchanged
missing-declaration M5A Tool. There is no published Agent artifact in this
consumer's dependency set: its source `agent.json` is local.

```bash
cd "$M5B_ROOT/projects/local-agent"
cat agent.json
m5b_expect local-install 0 m5b install --require-signature --machine
m5b_fixture check-lock local-agent
m5b_fixture snapshot local-agent local-replay
m5b_fixture clean local-agent
m5b_expect local-replay 0 m5b install --frozen --require-signature --machine
m5b_fixture unchanged local-agent local-replay

m5b_fixture snapshot local-agent local-regenerate
m5b_expect local-regenerate 0 m5b install --require-signature --machine
m5b_fixture unchanged local-agent local-regenerate
m5b_fixture check-lock local-agent
```

Expected: all eight Components return. Exactly one package—the reused legacy
Tool—has `standard_origin: "legacy_inferred"`; it is not promoted to declared or
certified. Other recorded origins remain declared, including `local:agent`.
Both frozen replay and ordinary regeneration preserve the source and lock bytes.

**Local Skill input:** an authored Skill selecting Echo v1, with no published
Skill artifact to download.

```bash
cd "$M5B_ROOT/projects/local-skill"
cat agent.json
m5b_expect local-skill-install 0 m5b install --require-signature --machine
m5b_fixture snapshot local-skill local-skill-replay
m5b_fixture clean local-skill
m5b_expect local-skill-replay 0 m5b install --frozen --require-signature --machine
m5b_fixture unchanged local-skill local-skill-replay
m5b_fixture check-lock local-skill
```

Expected: one installed Tool and an accurate declared `local:skill` root.

**Workspace input:** two local Agent manifests select different Tool versions;
the workspace also names the composed published Agent as a package root.

```bash
cd "$M5B_ROOT/projects/workspace"
cat agent.json agents/second.agent.json agentpm.workspace.json
m5b_expect workspace-install 0 m5b install --require-signature --machine
cat agent.lock
m5b_fixture check-lock workspace
m5b_fixture snapshot workspace workspace-replay
m5b_fixture clean workspace
m5b_expect workspace-replay 0 m5b install --frozen --require-signature --machine
m5b_fixture unchanged workspace workspace-replay
m5b_fixture check-lock workspace
```

Expected: both local roots and the published Agent root remain represented.
Their Tool edges retain v1 versus v2; the eight package entries are deduplicated
by exact identity. Source manifests, workspace metadata and lock remain unchanged.

## 5. Empty dependencies are valid, including multi-root workspaces

**Local Agent input:** no Tools, Skills or other dependencies.

```bash
cd "$M5B_ROOT/projects/empty-local"
cat agent.json
m5b_expect empty-local-install 0 m5b install --machine
m5b_fixture snapshot empty-local empty-local-again
m5b_expect empty-local-again 0 m5b install --machine
m5b_fixture unchanged empty-local empty-local-again
m5b_fixture snapshot empty-local empty-local-frozen
m5b_expect empty-local-frozen 0 m5b install --frozen --machine
m5b_fixture unchanged empty-local empty-local-frozen
m5b_fixture check-lock empty-local
```

Expected: stable success, empty `packages`, and an accurate local root. No
empty dependency-resolution request should reach the API. It is fine if no
`.agentpm/` directory is needed.

**Empty workspace input:** two dependency-free local Agents.

```bash
cd "$M5B_ROOT/projects/empty-workspace"
cat agentpm.workspace.json
m5b_expect empty-workspace-install 0 m5b install --machine
m5b_fixture snapshot empty-workspace empty-workspace-frozen
m5b_expect empty-workspace-frozen 0 m5b install --frozen --machine
m5b_fixture unchanged empty-workspace empty-workspace-frozen
m5b_fixture check-lock empty-workspace
```

Expected: zero packages and **both** local roots, not just the default root.

**Published empty Agent input:** the Agent itself still needs installing.

```bash
cd "$M5B_ROOT/projects/empty-direct"
m5b_expect empty-direct-install 0 m5b install "$(m5b_ref empty-agent)" --require-signature --machine
m5b_fixture snapshot empty-direct empty-direct-again
m5b_expect empty-direct-again 0 m5b install "$(m5b_ref empty-agent)" --require-signature --machine
m5b_fixture unchanged empty-direct empty-direct-again
m5b_fixture snapshot empty-direct empty-direct-frozen
m5b_fixture clean empty-direct
m5b_expect empty-direct-frozen 0 m5b install "$(m5b_ref empty-agent)" --frozen --require-signature --machine
m5b_fixture unchanged empty-direct empty-direct-frozen
m5b_fixture check-lock empty-direct
```

Expected: one Agent artifact, zero dependency entries, stable replay. None of
these successes claims Harness readiness; an Agent without a Loop is not made
runnable by a successful install.

## 6. Verify both exact Tool versions are usable

```bash
cd "$M5B_ROOT/projects/direct-agent"
m5b_expect run-v1 0 m5b run "$(m5b_ref echo-v1)" --input '{"message":"manual"}' --machine
m5b_expect run-v2 0 m5b run "$(m5b_ref echo-v2)" --input '{"message":"manual"}' --machine
```

Expected: both machine envelopes succeed and echo your input. Their
`output.version` values are **`1.0.0`** and **`2.0.0`** respectively. These are
Node fixture Tools, not model/provider calls or a Harness test.

## 7. Start the controlled fault server in Terminal B

Keep the real API running. Open **Terminal B**. In Terminal A, print the exact
source command for this run:

```bash
printf 'source %q\n' "$M5B_ROOT/paths.sh"
```

Copy the printed **actual absolute path** into Terminal B and run it. You do not
need to guess a placeholder or `cd` to the API repo. Then, in Terminal B:

```bash
pwd
printf 'Fixture workspace: %s\n' "$M5B_ROOT"
m5b_fixture serve
```

Expected: a separate `127.0.0.1:<port>` server, never port 5000. It stays in the
foreground until Ctrl-C. This is not another copy of your API. It only reads the
prepared fixture files and writes request evidence under this test directory.
The dummy credential is separate from your actual local PAT.

Back in **Terminal A**, establish an honest controlled baseline:

```bash
cd "$M5B_ROOT/projects/faults"
m5b_expect control-install 0 m5b_fault good install "$(m5b_ref agent)" --require-signature --machine
m5b_fixture check-lock faults
cat agent.lock
```

Expected: the same eight immutable package identities and payloads. The fixture
serves genuine signatures, but this controlled server is not evidence of real
namespace authorization or local Registry attestation trust.

## 8. Local authored changes must fail frozen checks before HTTP

First create a local consumer baseline against the **controlled** server:

```bash
cd "$M5B_ROOT/projects/guards"
cat agent.json
m5b_expect guards-install 0 m5b_fault good install --machine
m5b_fixture check-lock guards
```

Each case below changes only this owned consumer source:

| Case                 | Deliberate issue                                                | Required error                   |
| -------------------- | --------------------------------------------------------------- | -------------------------------- |
| `changed-range`      | Echo v2's selector becomes `^3`, incompatible with its pin      | `No locked tool`                 |
| `removed-dependency` | Remove the directly declared Tool, leaving its old root edge    | `Locked dependency edges differ` |
| `local-identity`     | Change the local package name/version without updating the lock | `Local root identity differs`    |

Run the three visibly labeled cases:

```bash
for m5b_case in changed-range removed-dependency local-identity; do
  m5b_fixture mutate guards "$m5b_case"
  cat agent.json
  m5b_fixture snapshot guards "$m5b_case"
  m5b_expect "$m5b_case" 1 m5b_fault good install --frozen --machine || break
  m5b_fixture check-result "$m5b_case" || break
  m5b_fixture unchanged guards "$m5b_case" || break
  m5b_fixture traffic "$m5b_case" --local || break
  m5b_fixture mutate guards restore
done
m5b_fixture check-lock guards
```

Expected: exit **1**, the specific machine message, no controlled HTTP request,
and unchanged source/lock/installed bytes after each rejection. The snapshot is
taken **after** your intentional edit, so it proves the CLI did not silently fix
or rewrite the input. `restore` copies back only the original fixture source and
lock. If you break out on a mismatch, stop and inspect; do not continue testing.

A fresh workspace must also refuse frozen installation without a lock:

```bash
cd "$M5B_ROOT/projects/fresh"
ls -a
m5b_fixture snapshot fresh missing-lock
m5b_expect missing-lock 1 m5b_fault good install --frozen --machine
m5b_fixture check-result missing-lock
m5b_fixture unchanged fresh missing-lock
m5b_fixture traffic missing-lock --local
```

Expected: `--frozen requires agent.lock`, with no lock created or HTTP request.

## 9. Locked graph, response and archive defects

Return to the controlled direct-Agent consumer. These are **intentional fixture
lock edits**, not recommended ways to update a real project.

| Case                 | What is wrong                                                                   | Where it should fail                           |
| -------------------- | ------------------------------------------------------------------------------- | ---------------------------------------------- |
| `wrong-kind`         | A Tool entry claims Skill while its map key remains Tool                        | Before any HTTP                                |
| `missing-transitive` | Remove Echo v1; only the downloaded Skill declares that hidden edge             | Verified manifest inspection; `No locked tool` |
| `stale-edge`         | Agent's stored Tool edge points to v1, while its immutable manifest requires v2 | Verified manifest inspection; `No locked tool` |
| `pin-digest`         | Lock records a zero digest, Registry init returns the genuine digest            | Init comparison; no archive GET                |
| `root-origin`        | Registry Agent root claims inferred origin despite its declared manifest        | Verified manifest inspection; origin conflict  |

```bash
cd "$M5B_ROOT/projects/faults"
for m5b_case in wrong-kind missing-transitive stale-edge pin-digest root-origin; do
  m5b_fixture mutate faults "$m5b_case"
  cat agent.lock
  m5b_fixture snapshot faults "$m5b_case"
  m5b_expect "$m5b_case" 1 m5b_fault good install "$(m5b_ref agent)" --frozen --machine || break
  m5b_fixture check-result "$m5b_case" || break
  m5b_fixture unchanged faults "$m5b_case" || break
  if [[ "$m5b_case" == wrong-kind ]]; then
    m5b_fixture traffic "$m5b_case" --local || break
  elif [[ "$m5b_case" == pin-digest ]]; then
    m5b_fixture traffic "$m5b_case" --failed --gets 0 || break
  else
    m5b_fixture traffic "$m5b_case" --failed || break
  fi
  m5b_fixture mutate faults restore
done
m5b_fixture check-lock faults
```

Expected: each exact rejection exits 1, does not finalize, and preserves the
existing installation and the deliberately edited lock. The original lock is
restored between cases. Some checks need archive contents even if those bytes
come from a verified cache; that differs from a pre-download rejection.

The next three cases keep the local lock/source intact and change only the
controlled server's behavior. Frozen refresh ensures the corrupt-download case
actually reads payload bytes instead of silently using cache.

| Case               | Response defect                                                     | Required message                     |
| ------------------ | ------------------------------------------------------------------- | ------------------------------------ |
| `init-digest`      | Init returns a different logical digest                             | `differs from the lock/resolve plan` |
| `corrupt-download` | Flip an archive byte without changing its length or advertised hash | `sha256 mismatch`                    |
| `registry-offline` | Return a deliberate HTTP 503 from Registry init                     | `Controlled Registry unavailable`    |

```bash
for m5b_case in init-digest corrupt-download registry-offline; do
  m5b_fixture describe "$m5b_case"
  m5b_fixture snapshot faults "$m5b_case"
  m5b_expect "$m5b_case" 1 m5b_fault "$m5b_case" install "$(m5b_ref agent)" --frozen --refresh --machine || break
  m5b_fixture check-result "$m5b_case" || break
  m5b_fixture unchanged faults "$m5b_case" || break
  if [[ "$m5b_case" == corrupt-download ]]; then
    m5b_fixture traffic "$m5b_case" --failed --gets 8 || break
  else
    m5b_fixture traffic "$m5b_case" --failed --gets 0 || break
  fi
done
```

Expected: no Registry resolve request, no successful finalize and no replaced
installation. The 503 case shows **frozen is not offline**: cached payloads do
not remove the need for a Registry session. It is a controlled availability
simulation; you do not need to stop the real API. The bad archives are never
published to the enforcing Registry or S3.

## 10. Cache reuse, clean replay and repair

**Cached input:** the honest baseline is still installed. No archive GET should
be needed, although init/finalize still occur.

```bash
cd "$M5B_ROOT/projects/faults"
m5b_fixture snapshot faults control-frozen
m5b_expect control-frozen 0 m5b_fault good install "$(m5b_ref agent)" --frozen --machine
m5b_fixture unchanged faults control-frozen
m5b_fixture traffic control-frozen --gets 0

m5b_fixture snapshot faults control-clean
m5b_fixture clean faults
m5b_expect control-clean 0 m5b_fault good install "$(m5b_ref agent)" --frozen --machine
m5b_fixture unchanged faults control-clean
m5b_fixture traffic control-clean --gets 8
```

Expected: no resolve in either replay; cache hit has **0** archive GETs, clean
replay has **8**, one per exact package. Both finalize successfully.

**Bad cache input:** corrupt only Echo v1's cached archive; installed files stay
intact. The CLI should fetch that archive again rather than trust the bad hash.

```bash
m5b_fixture snapshot faults cache-repair
m5b_fixture mutate faults cache
m5b_expect cache-repair 0 m5b_fault good install "$(m5b_ref agent)" --frozen --machine
m5b_fixture unchanged faults cache-repair
m5b_fixture traffic cache-repair --gets 1
```

**Modified installed input:** corrupt Echo v1's installed `main.js`. Frozen
refresh must restore the real payload and keep the existing pins/lock bytes.

```bash
m5b_fixture snapshot faults refresh-repair
m5b_fixture mutate faults payload
m5b_expect refresh-repair 0 m5b_fault good install "$(m5b_ref agent)" --frozen --refresh --machine
m5b_fixture unchanged faults refresh-repair
m5b_fixture traffic refresh-repair --gets 8
m5b_fixture check-lock faults
```

Expected: **8**, not 16, archive GETs. The graph preflight and actual installation
reuse the same verified cache. Refresh must still re-extract and repair the
modified file. Hash comparison covers every installed payload file, not just
`agent.json`.

## 11. Historical lock shapes do not get rewritten

These are local compatibility copies of known-good locks. Removing metadata
here intentionally simulates older locks; do not remove provenance to work
around a real CLI upgrade requirement.

**Tool-only v2 lock:** use the separate single-Tool consumer, still on the
controlled server.

```bash
cd "$M5B_ROOT/projects/old-tool"
cat agent.json
m5b_expect old-tool-install 0 m5b_fault tool install --machine
m5b_fixture mutate old-tool old-v2
cat agent.lock
m5b_fixture snapshot old-tool old-v2-replay
m5b_expect old-v2-replay 0 m5b_fault tool install --frozen --machine
m5b_fixture unchanged old-tool old-v2-replay
m5b_fixture traffic old-v2-replay --gets 0
m5b_fixture mutate old-tool restore
m5b_fixture check-lock old-tool
```

**Complete multi-kind v3 lock:** its package/root identity graph is intact, but
it has none of the later APDS metadata fields.

```bash
cd "$M5B_ROOT/projects/faults"
m5b_fixture mutate faults old-v3
cat agent.lock
m5b_fixture snapshot faults old-v3-replay
m5b_expect old-v3-replay 0 m5b_fault good install "$(m5b_ref agent)" --frozen --machine
m5b_fixture unchanged faults old-v3-replay
m5b_fixture traffic old-v3-replay --gets 0
m5b_fixture mutate faults restore
m5b_fixture check-lock faults
```

Expected: successful, byte-identical historical locks. Frozen mode does not
silently add metadata or upgrade formats. Restore returns the original v4
fixture lock for final inspection. Native Python dependency locks have separate
v4 requirements; these Node fixtures do not bypass them. Legacy v1 Tool pin
coverage remains in the automated integration suite.

## 12. Audit and finish

In Terminal A:

```bash
cd "$M5B_ROOT"
m5b_fixture audit
cat evidence/summary.json
printf 'Retain this run and evidence: %s\n' "$M5B_ROOT"
```

Expected: `status: "pass"`, **51** documented command results present, exact
negative messages, retained byte-comparison and request proofs, correct current
locks/installed payloads, unchanged reviewed CLI binary and real local DB
records for all six new publications. No missing or unrelated failure can be
counted as a pass. Keep the run directory if you want the sources and evidence
available for review.

The summary deliberately keeps `operator_signoff: false` and
`hosted_ci_complete: false`: the fixture cannot grant human approval or claim CI
execution. Hosted CI is separately operator-confirmed passed. Tell Codex when
you accept this manual run so the milestone record can be closed.

In Terminal B, **Ctrl-C** stops only the controlled fixture server. Your normal
IntelliJ API and Docker DB can stay running. These tests do not delete published
local fixture versions, namespace signers or database rows, and do not change
production configuration.
