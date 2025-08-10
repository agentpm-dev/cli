use crate::assets::{AGENT_JSON_TPL, TOOL_AGENT_JSON_TPL};
use crate::io::fs::write_atomic;
use crate::prelude::*;
use std::path::PathBuf;

#[derive(clap::ValueEnum, Clone, Debug)]
pub enum InitKind {
    Tool,
    Agent,
}

#[derive(Args, Debug)]
pub struct InitArgs {
    /// What to scaffold: a single-tool package or a composed agent
    #[arg(long, value_enum, default_value = "tool")]
    kind: InitKind,

    /// Name for the tool or agent (depending on --kind)
    #[arg(long, default_value = "my-agent")]
    name: String,

    /// Description for the tool or agent
    #[arg(long, default_value = "Starter AgentPM project")]
    description: String,

    /// Output directory (defaults to current dir)
    #[arg(long)]
    out_dir: Option<PathBuf>,
}

impl InitArgs {
    pub async fn run(self, _base_url: String) -> Result<()> {
        let out = self.out_dir.unwrap_or(std::env::current_dir()?);
        match self.kind {
            InitKind::Tool => {
                let rendered = render(
                    TOOL_AGENT_JSON_TPL,
                    &[
                        ("TOOL_NAME", &self.name),
                        ("TOOL_DESCRIPTION", &self.description),
                    ],
                );
                let path = out.join("agent.json");
                write_atomic(&path, &rendered)?;
                println!("Created {}", path.display());
            }
            InitKind::Agent => {
                let rendered = render(
                    AGENT_JSON_TPL,
                    &[
                        ("AGENT_NAME", &self.name),
                        ("AGENT_DESCRIPTION", &self.description),
                    ],
                );
                let path = out.join("agent.json");
                write_atomic(&path, &rendered)?;
                println!("Created {}", path.display());
            }
        }
        Ok(())
    }
}

/// Naive renderer: replaces {{KEY}} with value.
fn render(tpl: &str, vars: &[(&str, &str)]) -> String {
    let mut out = tpl.to_string();
    for (k, v) in vars {
        out = out.replace(&format!("{{{{{}}}}}", k), v);
    }
    out
}
