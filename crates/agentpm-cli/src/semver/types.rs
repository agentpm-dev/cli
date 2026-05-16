use anyhow::{Result, anyhow, bail};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum PackageKind {
    #[default]
    Tool,
    Agent,
}

#[derive(Serialize, Deserialize)]
pub struct PackageRequirement {
    pub kind: PackageKind,
    pub name: String,  // "@owner/name"
    pub range: String, // "^1.3", "~2.0.0", "1.3.4"
}

impl PackageRequirement {
    pub fn new(kind: PackageKind, name: impl Into<String>, range: impl Into<String>) -> Self {
        Self {
            kind,
            name: name.into(),
            range: range.into(),
        }
    }

    pub fn tool(name: impl Into<String>, range: impl Into<String>) -> Self {
        Self::new(PackageKind::Tool, name, range)
    }
}

#[derive(Serialize, Deserialize)]
pub struct DesiredSet {
    pub items: Vec<PackageRequirement>,
}

#[derive(Serialize, Deserialize)]
pub struct ResolvePlan {
    pub items: Vec<ResolvedPackage>,
}

#[derive(Serialize, Deserialize)]
pub struct ResolvedPackage {
    pub kind: PackageKind,
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
            let req = parse_package_spec(spec)?;
            Ok(DesiredSet { items: vec![req] })
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

                items.push(PackageRequirement::tool(pkg, rng));
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

            // Lockfile v1 only stores name/version/integrity, so frozen resolution
            // currently treats every locked package as kind=tool. That is
            // intentional for the current tool-only lockfile, but lockfile v2
            // must carry package kind so frozen installs can round-trip agent
            // packages correctly.
            resolved.push(ResolvedPackage {
                kind: PackageKind::Tool,
                name: d.name.clone(),
                version: dep.version.clone(),
                integrity: dep.integrity.clone(),
            });
        }

        Ok(ResolvePlan { items: resolved })
    }
}

pub fn parse_package_spec(spec: &str) -> Result<PackageRequirement> {
    let s = spec.trim();

    // Direct CLI specs still default to kind=tool in this milestone.
    //
    // Today, `agentpm install @namespace/name` remains a tool-oriented surface,
    // and the backend explicitly rejects non-tool resolution/init requests.
    // When agent packages become directly installable, this path should stop
    // asserting `tool` up front and allow the registry/backend to resolve the
    // package kind authoritatively.

    // Find the last '@'
    if let Some(i) = s.rfind('@') {
        if i == 0 {
            // Only the scoped '@' at the start → no version suffix
            if !s.contains('/') {
                return Err(anyhow!("package must be '@owner/name'"));
            }
            return Ok(PackageRequirement::tool(s.to_string(), "*".to_string()));
        } else {
            // Split into package and range at the last '@'
            let package = &s[..i];
            let range = &s[i + 1..];
            if !package.starts_with('@') || !package.contains('/') {
                return Err(anyhow!("package must be '@owner/name'"));
            }
            if range.is_empty() {
                return Ok(PackageRequirement::tool(
                    package.to_string(),
                    "*".to_string(),
                ));
            }
            return Ok(PackageRequirement::tool(
                package.to_string(),
                range.to_string(),
            ));
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parse_package_spec_defaults_to_tool_kind_without_version() {
        let parsed = parse_package_spec("@zack/summarize").unwrap();

        assert_eq!(parsed.kind, PackageKind::Tool);
        assert_eq!(parsed.name, "@zack/summarize");
        assert_eq!(parsed.range, "*");
    }

    #[test]
    fn parse_package_spec_accepts_exact_version() {
        let parsed = parse_package_spec("@zack/summarize@1.2.3").unwrap();

        assert_eq!(parsed.kind, PackageKind::Tool);
        assert_eq!(parsed.name, "@zack/summarize");
        assert_eq!(parsed.range, "1.2.3");
    }

    #[test]
    fn parse_package_spec_accepts_semver_range() {
        let parsed = parse_package_spec("@zack/summarize@^1.2").unwrap();

        assert_eq!(parsed.kind, PackageKind::Tool);
        assert_eq!(parsed.name, "@zack/summarize");
        assert_eq!(parsed.range, "^1.2");
    }

    #[test]
    fn parse_package_spec_treats_empty_version_suffix_as_wildcard() {
        let parsed = parse_package_spec("@zack/summarize@").unwrap();

        assert_eq!(parsed.kind, PackageKind::Tool);
        assert_eq!(parsed.name, "@zack/summarize");
        assert_eq!(parsed.range, "*");
    }

    #[test]
    fn package_requirement_supports_agent_kind() {
        let parsed = PackageRequirement::new(PackageKind::Agent, "@zack/support-agent", "^0.1");

        assert_eq!(parsed.kind, PackageKind::Agent);
        assert_eq!(parsed.name, "@zack/support-agent");
        assert_eq!(parsed.range, "^0.1");
    }

    #[test]
    fn desired_set_from_agent_json_tools_marks_tool_kind() {
        let desired = DesiredSet::from_cli_or_agent_json(
            &json!({
                "tools": [
                    "@zack/summarize@1.2.3",
                    { "name": "@zack/translate", "version": "^2.0" }
                ]
            }),
            None,
            false,
        )
        .unwrap();

        assert_eq!(desired.items.len(), 2);
        assert!(
            desired
                .items
                .iter()
                .all(|item| item.kind == PackageKind::Tool)
        );
        assert_eq!(desired.items[0].name, "@zack/summarize");
        assert_eq!(desired.items[1].range, "^2.0");
    }

    #[test]
    fn desired_set_from_cli_spec_uses_direct_package_parse_path() {
        let desired =
            DesiredSet::from_cli_or_agent_json(&Value::Null, Some("@zack/tool@1.2.3"), false)
                .unwrap();

        assert_eq!(desired.items.len(), 1);
        assert_eq!(desired.items[0].kind, PackageKind::Tool);
        assert_eq!(desired.items[0].name, "@zack/tool");
        assert_eq!(desired.items[0].range, "1.2.3");
    }
}
