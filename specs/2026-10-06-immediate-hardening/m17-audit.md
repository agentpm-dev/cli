# M17 SDK audit and implementation boundary

Audited 2026-11-09 before implementation. Python and Node SDK 0.1.16 read only
lock v2/v3 and independently select interpreters, spawn Tools, parse their output
and impose process limits. Python also imports Unix-only resource at module load.
Both composition loaders already preserve exact locked dependency references and
support Agent, Skill, Knowledge, Memory, Profile and Loop metadata. Keep these
read-only APIs, optional adapters, Python typing and Node ESM/CJS/declarations.

SDK 0.1.17 will read v2/v3/v4 and delegate **every Tool** to CLI >=0.1.43.
The older SDKs remain incompatible with v4. CLI 0.1.42 lacks a way to execute an
SDK-selected override directory and apply caller env after manifest env. A narrow
`run --sdk-request --machine` bridge reads a versioned stdin object containing
input, environment overrides and the exact installed directory. Secrets are not
put in argv or temporary request files. The exact ref is still an ordinary run
argument; CLI validates directory identity and retains all normal selection,
managed environment, schema, timeout and output checks. No registry protocol,
manifest schema, install flow or environment manager changes are needed.

CLI lookup uses existing SDK Harness convention AGENTPM, otherwise agentpm on
PATH. Each call checks CLI version, sends stdin JSON, and unwraps only a complete
schema-version-1 result after process completion. Tool-returned error data stays
data. CLI errors retain category and repair guidance, with supplied/ambient
credential values redacted. Metadata loading needs no CLI. Required env checks
use ambient, entrypoint and caller env; caller wins, defaults fill missing values.
CLI owns runtime checks, output limits and timeout/process-tree cleanup. SDKs
retain bounded transport and interruption cleanup, not Tool-execution policy.

Additive lock fields are read without mutation. Root/container/package reference
validation rejects malformed and future versions before composition use. Lock-only
projects are discoverable; local:tool author entries cannot become installed Agent
roots. Shared JSON fixtures live under the Rust SDK contracts directory with byte-identical SDK test
snapshots. Local parity and hosted tests consume the same vectors. Hosted matrix
execution and publication of SDK distributions remain operator acceptance gates.

## Supported version matrix

| SDK | CLI | Lock / execution behavior |
| --- | --- | --- |
| Python/Node 0.1.16 | Earlier CLI install flows | v2/v3 metadata only; historical SDK-owned execution; not current v4 compatibility |
| Python/Node 0.1.17 | Missing or below 0.1.43 | v2/v3/v4 metadata loading; Tool invocation fails with CLI install/update guidance |
| Python/Node 0.1.17 | 0.1.43 (machine protocol v1) | v2/v3/v4 metadata; all Tool calls use the CLI-owned runner |
| Python/Node 0.1.17 | Any | Unsupported future lock versions are rejected when reading composition metadata |

Review-visible runtime difference: interpreter/schema/environment failures now
come from CLI invocation, rather than SDK interpreter probing during load. The
SDK no longer sets a second temporary HOME or Node/Python resource flags. Public
loaders/callable/result types remain; the CLI is the execution policy owner.
Legacy JSON-after-log handling and the 10 MB SDK output budget remain explicitly
scoped compatibility behavior inside the CLI bridge.

## Verification and release order

Local macOS arm64 verification is recorded in test-plan.md. The SDK native fixture
uses the CLI manifest linter, a disposable local Registry, real archive digests,
pinned uv and the shared pydantic resolution. Node test-fixture type corrections
allow full source/test typechecking; no exported type was weakened. Both SDKs
ignore .agentpm state per the user's follow-up. SDK package artifacts and example
source overrides are local evidence only.

Publish CLI 0.1.43, confirm Homebrew/Scoop installation or upgrade reports that
version (or a newer compatible release), then publish either SDK 0.1.17. Both
SDK publications remain blocked until those distribution checks pass; a source
merge or GitHub release asset alone is insufficient. This protects every Tool
call, including legacy Python and Node Tools. The SDK CI checks out public
CLI main by default; proposed-ref runs can use cli_ref/AGENTPM_CLI_REF before that
merge. Hosted platform passes, published PyPI/npm clean-consumer checks and
docs/status deployment remain operator gates in Release Band 13. No Registry
schema/protocol, Tool release or production data was changed by M17.
