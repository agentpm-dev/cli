use anyhow::{Result, anyhow};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct Desired {
    pub name: String,  // "@owner/name"
    pub range: String, // "^1.3", "~2.0.0", "1.3.4"
}

#[derive(Serialize, Deserialize)]
pub struct DesiredSet {
    pub items: Vec<Desired>,
}

#[derive(Serialize, Deserialize)]
pub struct ResolvePlan {
    pub items: Vec<ResolvedItem>,
}

#[derive(Serialize, Deserialize)]
pub struct ResolvedItem {
    pub name: String,      // "@owner/name"
    pub version: String,   // "1.3.4"
    pub integrity: String, // "sha256-..."
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Lock {
    pub lockfile_version: u32,
    pub generated: DateTime<Utc>,
    pub dependencies: std::collections::BTreeMap<String, LockedDependency>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LockedDependency {
    pub version: String,
    pub integrity: String,
}

impl DesiredSet {
    /// Build from CLI spec or from agent.json.tools
    pub fn from_cli_or_agent_json(
        meta: &serde_json::Value,
        spec: Option<&str>,
        _update_range: bool,
    ) -> Result<Self> {
        if let Some(spec) = spec {
            let (pkg, rng) = parse_spec(spec)?;
            Ok(DesiredSet {
                items: vec![Desired {
                    name: pkg,
                    range: rng,
                }],
            })
        } else {
            // Expect `tools` array in agent.json
            let tools = meta
                .get("tools")
                .and_then(|v| v.as_array())
                .ok_or_else(|| anyhow!("agent.json has no tools array"))?;

            let mut items = Vec::new();
            for tool in tools {
                let pkg = tool
                    .get("name")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow!("tool entry missing name"))?;
                let rng = tool.get("version").and_then(|v| v.as_str()).unwrap_or("*"); // allow wildcard default
                items.push(Desired {
                    name: pkg.to_string(),
                    range: rng.to_string(),
                });
            }

            Ok(DesiredSet { items })
        }
    }
}

impl ResolvePlan {
    /// Build a ResolvePlan from the lock file, checking that all desired packages exist.
    pub fn from_lock(desired: &DesiredSet, lock: &Lock) -> Result<ResolvePlan> {
        let mut resolved = Vec::new();

        for d in &desired.items {
            let dep = lock
                .dependencies
                .get(&d.name)
                .ok_or_else(|| anyhow!("name {} not found in lockfile", d.name))?;

            // NOTE: we don't re-check the semver range here — frozen mode assumes lock is truth.
            resolved.push(ResolvedItem {
                name: d.name.clone(),
                version: dep.version.clone(),
                integrity: dep.integrity.clone(),
            });
        }

        Ok(ResolvePlan { items: resolved })
    }
}

fn parse_spec(spec: &str) -> Result<(String, String)> {
    let parts: Vec<&str> = spec.rsplitn(2, '@').collect();
    if parts.len() == 2 {
        let range = parts[0].to_string();
        let package = parts[1].to_string();
        if !package.starts_with('@') {
            return Err(anyhow!("package must start with '@owner/'"));
        }
        Ok((package, range))
    } else {
        // no explicit version range → use "*"
        let package = spec.to_string();
        if !package.starts_with('@') {
            return Err(anyhow!("package must start with '@owner/'"));
        }
        Ok((package, "*".to_string()))
    }
}

pub fn lock_from_plan(plan: &ResolvePlan) -> Lock {
    let mut deps = std::collections::BTreeMap::new();

    for item in &plan.items {
        deps.insert(
            item.name.clone(),
            LockedDependency {
                version: item.version.clone(),
                integrity: item.integrity.clone(),
            },
        );
    }

    Lock {
        lockfile_version: 1,
        generated: Utc::now(),
        dependencies: deps,
    }
}
