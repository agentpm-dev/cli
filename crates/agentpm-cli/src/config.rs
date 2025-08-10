use anyhow::{Context, Result};
use directories::ProjectDirs;
use serde::Deserialize;
use std::fs;
use std::path::PathBuf;

/// What we use throughout the CLI after merging file/env/flags.
#[derive(Debug, Clone)]
pub struct Config {
    pub base_url: String,
    #[allow(dead_code)] // TODO: will be used when we add config file writes
    pub config_dir: PathBuf,
    pub token_file: PathBuf,
}

/// What can come from config.toml (all optional).
#[derive(Debug, Deserialize)]
struct FileConfig {
    base_url: Option<String>,
    // expand here later (e.g., profiles, org, etc.)
}

impl Config {
    /// Load config from disk (if present) and merge with CLI flag `base_url`.
    /// Precedence: CLI flag > file > default.
    pub fn load(cli_base_url: String) -> Result<Self> {
        let dirs = project_dirs().context("could not determine config directories")?;
        let config_dir = dirs.config_dir().to_path_buf();
        let cfg_path = config_dir.join("config.toml");
        let token_file = config_dir.join("token.json");

        // Read the file if it exists
        let file_cfg: Option<FileConfig> = if cfg_path.exists() {
            let text = fs::read_to_string(&cfg_path)
                .with_context(|| format!("reading {}", cfg_path.display()))?;
            let parsed: FileConfig =
                toml::from_str(&text).with_context(|| format!("parsing {}", cfg_path.display()))?;
            Some(parsed)
        } else {
            None
        };

        // Merge
        let base_url = if !cli_base_url.is_empty() {
            cli_base_url
        } else if let Some(fc) = &file_cfg {
            fc.base_url.clone().unwrap_or_else(default_base_url)
        } else {
            default_base_url()
        };

        // Ensure config dir exists (don’t error if we can’t; create lazily on writes)
        let _ = fs::create_dir_all(&config_dir);

        Ok(Self {
            base_url,
            config_dir,
            token_file,
        })
    }
}

fn default_base_url() -> String {
    "https://api.agentpackagemanager.local".to_string()
}

fn project_dirs() -> Option<ProjectDirs> {
    // domain, organization, application
    // These values define OS-specific dirs like:
    // macOS: ~/Library/Application Support/com.agentpm/AgentPM
    // Linux: ~/.config/agentpm/AgentPM
    // Windows: C:\Users\<user>\AppData\Roaming\agentpm\AgentPM
    ProjectDirs::from("com", "agentpm", "AgentPM")
}
