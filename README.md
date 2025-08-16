# AgentPM CLI

Command-line tool for building AgentPM tools and agents.

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

```bash
agentpm --help
agentpm init --kind tool --name demo --description "My first tool"
```

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
