use anyhow::{Result, anyhow, bail};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

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
                let (pkg, rng) = match tool {
                    Value::String(s) => {
                        let (name, ver) = parse_tool_str(s)?;
                        (name, ver)
                    }
                    Value::Object(map) => {
                        let name = map
                            .get("name")
                            .and_then(|v| v.as_str())
                            .ok_or_else(|| anyhow!("tool entry missing name"))?
                            .to_string();
                        let rng = map
                            .get("version")
                            .and_then(|v| v.as_str())
                            .unwrap_or("*")
                            .to_string();
                        (name, rng)
                    }
                    _ => return Err(anyhow!("tool must be a string or an object")),
                };

                items.push(Desired {
                    name: pkg.to_string(),
                    range: rng.to_string(),
                });
            }

            Ok(DesiredSet { items })
        }
    }
}

fn parse_tool_str(s: &str) -> Result<(String, String)> {
    // Split on the LAST '@' so we don't confuse the leading scope '@'
    if let Some(idx) = s.rfind('@')
        && idx > 0
    {
        let name = &s[..idx];
        let ver = &s[idx + 1..];
        if name.contains('/') {
            return Ok((name.to_string(), ver.to_string()));
        }
    }
    // No version suffix → default "*"
    if s.contains('/') {
        Ok((s.to_string(), "*".to_string()))
    } else {
        bail!("invalid tool string: expected @scope/name or @scope/name@version");
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
    let s = spec.trim();

    // Find the last '@'
    if let Some(i) = s.rfind('@') {
        if i == 0 {
            // Only the scoped '@' at the start → no version suffix
            if !s.contains('/') {
                return Err(anyhow!("package must be '@owner/name'"));
            }
            return Ok((s.to_string(), "*".to_string()));
        } else {
            // Split into package and range at the last '@'
            let package = &s[..i];
            let range = &s[i + 1..];
            if !package.starts_with('@') || !package.contains('/') {
                return Err(anyhow!("package must be '@owner/name'"));
            }
            if range.is_empty() {
                return Ok((package.to_string(), "*".to_string()));
            }
            return Ok((package.to_string(), range.to_string()));
        }
    }

    // No '@' at all → invalid (missing scope)
    Err(anyhow!("package must start with '@owner/'"))
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
