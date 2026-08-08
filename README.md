# AgentPM CLI

Command-line tool for building, publishing, installing, and validating AgentPM packages.

AgentPM currently supports seven package kinds:

- **tools**: executable capabilities with entrypoints, runtime requirements, inputs, and outputs
- **skills**: procedural packages that capture playbooks, checklists, and reasoning guides around tools
- **knowledge**: prepared context packages for direct context injection or local vector retrieval
- **memory**: declarative Memory Blueprints that define durable record shapes, retrieval semantics, and lifecycle policy
- **profiles**: authored Instruction Profiles that define portable role, objective, communication, and constraint metadata
- **agents**: composition artifacts that declare tool, skill, knowledge, memory, and profile dependencies plus examples
- **templates**: workflow scaffolds that generate editable AgentPM workspaces

Agent packages are not runnable app bundles. They install as portable composition metadata plus resolved tool, skill, knowledge, memory, and profile dependencies.

[![CI](https://github.com/agentpm-dev/cli/actions/workflows/ci.yml/badge.svg)](https://github.com/agentpm-dev/cli/actions/workflows/ci.yml)
[![Homebrew tap](https://img.shields.io/badge/homebrew-agentpm--dev%2Ftap-blue)](https://github.com/agentpm-dev/homebrew-tap)
[![Scoop bucket](https://img.shields.io/badge/scoop-agentpm--dev%2Fscoop--bucket-blue)](https://github.com/agentpm-dev/scoop-bucket)


## Installation

### Option A — Package Managers

#### Homebrew (macOS & Linux)

Install via our tap:
```bash
brew tap agentpm-dev/tap
brew install agentpm
```

Upgrade:
```bash
brew update && brew upgrade agentpm
```

Uninstall:
```bash
brew uninstall agentpm
```

Verify:
```bash
which agentpm
agentpm --version
brew info agentpm
```

#### Scoop (Windows)

Install via our Scoop bucket:
```powershell
scoop bucket add agentpm https://github.com/agentpm-dev/scoop-bucket
scoop install agentpm
```

Upgrade:
```powershell
scoop update
scoop update agentpm
```

Uninstall:
```bash
scoop uninstall agentpm
```

Verify:
```bash
where agentpm
agentpm --version
scoop info agentpm
```

### Option B — From source (devs & contributors)

Requires the Rust toolchain.

From the repo root:
```bash

cargo install --path crates/agentpm-cli --locked
```
binaries go to: ~/.cargo/bin (ensure it's on your PATH)

Update later:
```bash
cargo install --path crates/agentpm-cli --locked
```

Uninstall:
```bash
cargo uninstall agentpm-cli
```

### Option C — Prebuilt binaries (no Rust toolchain)

#### macOS (Intel/Apple Silicon) and Linux x86_64.

One-liner (installs to ~/.local/bin by default)
```bash
curl -fsSL https://raw.githubusercontent.com/agentpm-dev/cli/main/scripts/install-latest.sh | bash
```

Review first, then run:
```bash
curl -fsSL -o install.sh https://raw.githubusercontent.com/agentpm-dev/cli/main/scripts/install-latest.sh
bash install.sh
```

Custom install location:
```bash
PREFIX=/usr/local sudo bash -c "$(curl -fsSL https://raw.githubusercontent.com/agentpm-dev/cli/main/scripts/install-latest.sh)"
```

The installer downloads the latest GitHub Release asset for your OS/arch and places agentpm on your PATH.

**macOS PATH note**

The installer defaults to `~/.local/bin`, which isn’t on `PATH` by default on macOS.

**Option A — add it to PATH (recommended for user-local installs):**
```bash
# zsh (macOS default): add to both login and interactive shells
echo 'export PATH="$HOME/.local/bin:$PATH"' >> ~/.zprofile
echo 'export PATH="$HOME/.local/bin:$PATH"' >> ~/.zshrc
# reload current shell
source ~/.zprofile 2>/dev/null || true
source ~/.zshrc    2>/dev/null || true
# verify
which agentpm && agentpm --version
```

**Option B — install system-wide (no PATH changes needed on most Macs):**
```bash
PREFIX=/usr/local sudo bash -c "$(curl -fsSL https://raw.githubusercontent.com/agentpm-dev/cli/main/scripts/install-latest.sh)"
```
Tip: different terminals (Terminal.app, iTerm, VS Code) may read different startup files. Adding to both ~/.zprofile and ~/.zshrc covers most setups.

## Quick start

Create a new AgentPM package:

```bash
agentpm --help
agentpm init --kind tool --name demo --description "My first tool"
agentpm init --kind skill --name triage-playbook --description "My first skill"
agentpm init --kind knowledge --name docs-corpus --description "My first knowledge package"
agentpm init --kind memory --name conversation-continuity --description "My first memory blueprint"
agentpm init --kind profile --name support-style --description "My first instruction profile"
agentpm init --kind agent --name support-agent --description "My first agent"
agentpm init --kind template --name research-template --description "My workflow template"
```

### Workflow templates

Templates are publishable scaffold packages that generate editable AgentPM workspaces.

Generate a project from a published template:

```bash
agentpm new @zack/research-template my-project
cd my-project
# run the generated project's documented entrypoint
```

`agentpm new`:

- copies and renders scaffold files from the template
- installs declared tool, skill, agent, knowledge, memory, and profile dependencies into the generated workspace
- writes `agent.json`, `agentpm.workspace.json`, `agent.lock`, and `.agentpm/template.json`
- does not execute template-provided scripts or generated app code during scaffolding

You can also generate from a local template package while authoring:

```bash
agentpm new ./my-template ./my-template-test
agentpm new ./my-template/agent.json ./my-template-test
```

Variable resolution works like this:

- `--var key=value` overrides everything else
- template defaults apply next
- `project_name` is inferred from `target-dir` when possible
- interactive terminals prompt for any remaining required variables
- non-interactive runs fail clearly if required values are still missing

Generated workspace notes:

- `agentpm.workspace.json` is committed workspace topology for multi-root generated projects
- `.agentpm/template.json` records where the scaffold came from
- additional local generated agents belong under `agents/`
- the generated root `agent.json` is synthesized by `agentpm new`, not copied from `template.files_root`

After generation, edit the generated manifests and rerun:

```bash
agentpm install
```

For generated workspace projects, `agentpm install` rebuilds `agent.lock` from the manifests and workspace metadata. `agentpm install <spec>` is not supported inside those workspace projects.

### Manifest-driven local agent install

When a local `agent.json` has `kind: "agent"`, run:

```bash
agentpm install
```

That:

- resolves the tools, skills, knowledge, memory, and profile packages declared in the local manifest
- installs tools under `.agentpm/tools/<namespace>/<name>/<version>/`
- installs skills under `.agentpm/skills/<namespace>/<name>/<version>/`
- installs knowledge under `.agentpm/knowledge/<namespace>/<name>/<version>/`
- installs memory under `.agentpm/memory/<namespace>/<name>/<version>/`
- installs profiles under `.agentpm/profiles/<namespace>/<name>/<version>/`
- writes `agent.lock` (Skill-containing graphs use `lockfile_version: 3`)
- keeps the local `agent.json` as the source of truth

### Direct package install

Install a tool directly:

```bash
agentpm install @zack/capitalize@0.1.0
```

Install an agent package directly:

```bash
agentpm install @zack/support-agent@0.1.0
```

Install a memory package directly:

```bash
agentpm install @zack/profile-memory@0.1.0
```

Install a profile package directly:

```bash
agentpm install @zack/support-style@0.1.0
```

Direct agent install writes:

- the installed agent under `.agentpm/agents/<namespace>/<name>/<version>/`
- the agent’s resolved skills under `.agentpm/skills/<namespace>/<name>/<version>/`
- the agent’s resolved knowledge dependencies under `.agentpm/knowledge/<namespace>/<name>/<version>/`
- the agent’s resolved tools under `.agentpm/tools/<namespace>/<name>/<version>/`
- the agent’s resolved memory dependencies under `.agentpm/memory/<namespace>/<name>/<version>/`
- the agent’s resolved profile dependencies under `.agentpm/profiles/<namespace>/<name>/<version>/`
- an `agent:@namespace/name@version` root in `agent.lock`

Once a tool is installed, AgentPM can expose the same packaged artifact through the shell, MCP, and Skill scaffolds:

```bash
agentpm install @zack/capitalize
agentpm run @zack/capitalize --input '{"text":"hello world"}'
agentpm serve --mcp
agentpm export --skill @zack/capitalize
```

That interoperability flow keeps AgentPM as the source of truth for packaging and execution while making installed tools usable from other ecosystems.

### Publishing

Use:

```bash
agentpm publish
```

The preferred publish scope is:

- `packages:publish`

That scope can publish tools, skills, agents, and templates.

### Manifest schema

See the [schema docs](./schemas/README.md) for `agent.json` format, validation, and examples.

### Local template authoring

For template authoring and local verification:

```bash
agentpm init --kind template --name my-template --description "My workflow template"
cd my-template
# edit root agent.json and files under template/
agentpm new . ../my-template-test
cd ../my-template-test
# run the generated project's documented entrypoint
cd ../my-template
agentpm publish
```

That flow lets template authors verify scaffold rendering and dependency installation locally before publishing.

Keep two READMEs:

- root `README.md` for the template package shown in the registry
- `template/README.md` for the generated project shown to scaffold consumers

## Contributing

- Toolchain pinned via rust-toolchain.toml
- CI checks: cargo fmt, cargo clippy -D warnings, cargo test

### Run locally:

```bash
cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo test --all
```

#### Pre-commit hooks (optional):

```bash
pre-commit install --hook-type pre-commit --hook-type pre-push
pre-commit run --all-files
```

## License

MIT — see [LICENSE](https://raw.githubusercontent.com/agentpm-dev/cli/refs/heads/main/LICENSE)
