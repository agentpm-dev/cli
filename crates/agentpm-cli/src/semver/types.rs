use anyhow::{Result, anyhow, bail};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum PackageKind {
    #[default]
    Tool,
    Agent,
}

#[derive(Debug, Serialize, Deserialize)]
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

#[derive(Debug, Serialize, Deserialize)]
pub struct DesiredSet {
    pub items: Vec<PackageRequirement>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ResolvePlan {
    pub items: Vec<ResolvedPackage>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ResolvedPackage {
    pub kind: PackageKind,
    pub name: String,      // "@owner/name"
    pub version: String,   // "1.3.4"
    pub integrity: String, // "sha256-..."
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Lock {
    V1(LockV1),
    V2(LockV2),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LockV1 {
    pub lockfile_version: u32,
    pub generated: DateTime<Utc>,
    pub dependencies: BTreeMap<String, LockedDependency>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LockedDependency {
    pub version: String,
    pub integrity: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LockV2 {
    pub lockfile_version: u32,
    pub generated: DateTime<Utc>,
    pub packages: BTreeMap<String, LockedPackage>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub roots: BTreeMap<String, LockedRoot>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LockedPackage {
    pub kind: PackageKind,
    pub name: String,
    pub version: String,
    pub integrity: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct LockedRoot {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(default)]
    pub tools: Vec<String>,
    #[serde(default)]
    pub reserved: ReservedReferences,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct ReservedReferences {
    #[serde(default)]
    pub skills: Vec<Value>,
    #[serde(default)]
    pub knowledge: Vec<Value>,
    #[serde(default)]
    pub memory: Vec<Value>,
    #[serde(default)]
    pub profiles: Vec<Value>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)]
pub enum LockRoot {
    LocalAgent {
        name: String,
        version: String,
        tools: Vec<String>,
        reserved: ReservedReferences,
    },
    RegistryAgent {
        package_key: String,
        tools: Vec<String>,
        reserved: ReservedReferences,
    },
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
            let pkg = lock
                .find_unique_locked_package(&d.name, d.kind)?
                .ok_or_else(|| anyhow!("name {} not found in lockfile", d.name))?;

            // NOTE: we don't re-check the semver range here — frozen mode assumes lock is truth.
            resolved.push(ResolvedPackage {
                kind: pkg.kind,
                name: pkg.name.clone(),
                version: pkg.version.clone(),
                integrity: pkg.integrity.clone(),
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

pub fn lock_from_plan(plan: &ResolvePlan, roots: &[LockRoot]) -> Lock {
    let mut packages = BTreeMap::new();

    for item in &plan.items {
        packages.insert(
            package_key(item.kind, &item.name, &item.version),
            LockedPackage {
                kind: item.kind,
                name: item.name.clone(),
                version: item.version.clone(),
                integrity: item.integrity.clone(),
            },
        );
    }

    let mut root_map = BTreeMap::new();
    for root in roots {
        match root {
            LockRoot::LocalAgent {
                name,
                version,
                tools,
                reserved,
            } => {
                root_map.insert(
                    "local:agent".to_string(),
                    LockedRoot {
                        name: Some(name.clone()),
                        version: Some(version.clone()),
                        tools: tools.clone(),
                        reserved: reserved.clone(),
                    },
                );
            }
            LockRoot::RegistryAgent {
                package_key,
                tools,
                reserved,
            } => {
                root_map.insert(
                    package_key.clone(),
                    LockedRoot {
                        name: None,
                        version: None,
                        tools: tools.clone(),
                        reserved: reserved.clone(),
                    },
                );
            }
        }
    }

    Lock::V2(LockV2 {
        lockfile_version: 2,
        generated: Utc::now(),
        packages,
        roots: root_map,
    })
}

pub fn package_key(kind: PackageKind, name: &str, version: &str) -> String {
    format!("{}:{}@{}", kind.as_str(), name, version)
}

impl PackageKind {
    pub fn as_str(self) -> &'static str {
        match self {
            PackageKind::Tool => "tool",
            PackageKind::Agent => "agent",
        }
    }
}

impl Lock {
    pub fn empty_v2() -> Self {
        Self::V2(LockV2 {
            lockfile_version: 2,
            generated: Utc::now(),
            packages: BTreeMap::new(),
            roots: BTreeMap::new(),
        })
    }

    pub fn is_v1(&self) -> bool {
        matches!(self, Lock::V1(_))
    }

    pub fn find_unique_locked_package(
        &self,
        name: &str,
        kind: PackageKind,
    ) -> Result<Option<LockedPackage>> {
        match self {
            Lock::V1(lock) => Ok(lock.dependencies.get(name).map(|dep| LockedPackage {
                kind: PackageKind::Tool,
                name: name.to_string(),
                version: dep.version.clone(),
                integrity: dep.integrity.clone(),
            })),
            Lock::V2(lock) => {
                let matches: Vec<&LockedPackage> = lock
                    .packages
                    .values()
                    .filter(|pkg| pkg.name == name && pkg.kind == kind)
                    .collect();

                match matches.len() {
                    0 => Ok(None),
                    1 => Ok(Some(matches[0].clone())),
                    _ => bail!(
                        "multiple locked versions found for {}; use an explicit version selector",
                        name
                    ),
                }
            }
        }
    }

    pub fn locked_tool_packages(&self) -> Vec<LockedPackage> {
        match self {
            Lock::V1(lock) => lock
                .dependencies
                .iter()
                .map(|(name, dep)| LockedPackage {
                    kind: PackageKind::Tool,
                    name: name.clone(),
                    version: dep.version.clone(),
                    integrity: dep.integrity.clone(),
                })
                .collect(),
            Lock::V2(lock) => lock
                .packages
                .values()
                .filter(|pkg| pkg.kind == PackageKind::Tool)
                .cloned()
                .collect(),
        }
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

    #[test]
    fn lock_from_plan_writes_v2_for_tool_only_plan() {
        let lock = lock_from_plan(
            &ResolvePlan {
                items: vec![ResolvedPackage {
                    kind: PackageKind::Tool,
                    name: "@zack/echo".to_string(),
                    version: "0.1.0".to_string(),
                    integrity: "sha256-echo".to_string(),
                }],
            },
            &[],
        );

        let Lock::V2(lock) = lock else {
            panic!("expected v2 lockfile");
        };
        assert_eq!(lock.lockfile_version, 2);
        assert!(lock.roots.is_empty());
        assert!(lock.packages.contains_key("tool:@zack/echo@0.1.0"));
    }

    #[test]
    fn lock_from_plan_records_local_agent_root_and_reserved_refs() {
        let lock = lock_from_plan(
            &ResolvePlan {
                items: vec![ResolvedPackage {
                    kind: PackageKind::Tool,
                    name: "@zack/slack-post-message".to_string(),
                    version: "0.1.0".to_string(),
                    integrity: "sha256-slack".to_string(),
                }],
            },
            &[LockRoot::LocalAgent {
                name: "support-agent".to_string(),
                version: "0.1.0".to_string(),
                tools: vec!["tool:@zack/slack-post-message@0.1.0".to_string()],
                reserved: ReservedReferences {
                    skills: vec![json!("@zack/triage-skill@0.1.0")],
                    knowledge: vec![],
                    memory: vec![],
                    profiles: vec![],
                },
            }],
        );

        let Lock::V2(lock) = lock else {
            panic!("expected v2 lockfile");
        };
        let root = lock.roots.get("local:agent").unwrap();
        assert_eq!(root.name.as_deref(), Some("support-agent"));
        assert_eq!(
            root.tools,
            vec!["tool:@zack/slack-post-message@0.1.0".to_string()]
        );
        assert_eq!(
            root.reserved.skills,
            vec![json!("@zack/triage-skill@0.1.0")]
        );
    }

    #[test]
    fn lock_from_plan_records_registry_agent_root() {
        let lock = lock_from_plan(
            &ResolvePlan {
                items: vec![
                    ResolvedPackage {
                        kind: PackageKind::Agent,
                        name: "@zack/support-agent".to_string(),
                        version: "0.1.0".to_string(),
                        integrity: "sha256-agent".to_string(),
                    },
                    ResolvedPackage {
                        kind: PackageKind::Tool,
                        name: "@zack/slack-post-message".to_string(),
                        version: "0.1.0".to_string(),
                        integrity: "sha256-tool".to_string(),
                    },
                ],
            },
            &[LockRoot::RegistryAgent {
                package_key: "agent:@zack/support-agent@0.1.0".to_string(),
                tools: vec!["tool:@zack/slack-post-message@0.1.0".to_string()],
                reserved: ReservedReferences::default(),
            }],
        );

        let Lock::V2(lock) = lock else {
            panic!("expected v2 lockfile");
        };
        assert!(
            lock.packages
                .contains_key("agent:@zack/support-agent@0.1.0")
        );
        assert!(lock.roots.contains_key("agent:@zack/support-agent@0.1.0"));
    }

    #[test]
    fn lock_v2_can_represent_two_versions_of_same_tool() {
        let lock = lock_from_plan(
            &ResolvePlan {
                items: vec![
                    ResolvedPackage {
                        kind: PackageKind::Tool,
                        name: "@zack/slack-post-message".to_string(),
                        version: "0.1.0".to_string(),
                        integrity: "sha256-tool-1".to_string(),
                    },
                    ResolvedPackage {
                        kind: PackageKind::Tool,
                        name: "@zack/slack-post-message".to_string(),
                        version: "0.2.0".to_string(),
                        integrity: "sha256-tool-2".to_string(),
                    },
                ],
            },
            &[],
        );

        let Lock::V2(lock) = lock else {
            panic!("expected v2 lockfile");
        };
        assert!(
            lock.packages
                .contains_key("tool:@zack/slack-post-message@0.1.0")
        );
        assert!(
            lock.packages
                .contains_key("tool:@zack/slack-post-message@0.2.0")
        );
    }

    #[test]
    fn lock_v2_can_attribute_different_tool_versions_to_different_agent_roots() {
        let lock = lock_from_plan(
            &ResolvePlan {
                items: vec![
                    ResolvedPackage {
                        kind: PackageKind::Agent,
                        name: "@zack/support-agent".to_string(),
                        version: "0.1.0".to_string(),
                        integrity: "sha256-agent-1".to_string(),
                    },
                    ResolvedPackage {
                        kind: PackageKind::Agent,
                        name: "@zack/escalation-agent".to_string(),
                        version: "0.1.0".to_string(),
                        integrity: "sha256-agent-2".to_string(),
                    },
                    ResolvedPackage {
                        kind: PackageKind::Tool,
                        name: "@zack/slack-post-message".to_string(),
                        version: "0.1.0".to_string(),
                        integrity: "sha256-tool-1".to_string(),
                    },
                    ResolvedPackage {
                        kind: PackageKind::Tool,
                        name: "@zack/slack-post-message".to_string(),
                        version: "0.2.0".to_string(),
                        integrity: "sha256-tool-2".to_string(),
                    },
                ],
            },
            &[
                LockRoot::RegistryAgent {
                    package_key: "agent:@zack/support-agent@0.1.0".to_string(),
                    tools: vec!["tool:@zack/slack-post-message@0.1.0".to_string()],
                    reserved: ReservedReferences::default(),
                },
                LockRoot::RegistryAgent {
                    package_key: "agent:@zack/escalation-agent@0.1.0".to_string(),
                    tools: vec!["tool:@zack/slack-post-message@0.2.0".to_string()],
                    reserved: ReservedReferences::default(),
                },
            ],
        );

        let Lock::V2(lock) = lock else {
            panic!("expected v2 lockfile");
        };

        let support_root = lock.roots.get("agent:@zack/support-agent@0.1.0").unwrap();
        let escalation_root = lock
            .roots
            .get("agent:@zack/escalation-agent@0.1.0")
            .unwrap();

        assert_eq!(
            support_root.tools,
            vec!["tool:@zack/slack-post-message@0.1.0".to_string()]
        );
        assert_eq!(
            escalation_root.tools,
            vec!["tool:@zack/slack-post-message@0.2.0".to_string()]
        );
    }

    #[test]
    fn frozen_v1_lockfile_still_resolves_tool_only_plan() {
        let lock = Lock::V1(LockV1 {
            lockfile_version: 1,
            generated: Utc::now(),
            dependencies: BTreeMap::from([(
                "@zack/echo".to_string(),
                LockedDependency {
                    version: "0.1.0".to_string(),
                    integrity: "sha256-echo".to_string(),
                },
            )]),
        });

        let desired =
            DesiredSet::from_cli_or_agent_json(&Value::Null, Some("@zack/echo@^0.1"), false)
                .unwrap();
        let plan = ResolvePlan::from_lock(&desired, &lock).unwrap();

        assert_eq!(plan.items.len(), 1);
        assert_eq!(plan.items[0].kind, PackageKind::Tool);
        assert_eq!(plan.items[0].version, "0.1.0");
    }

    #[test]
    fn frozen_v2_rejects_ambiguous_unversioned_tool_lookup() {
        let lock = lock_from_plan(
            &ResolvePlan {
                items: vec![
                    ResolvedPackage {
                        kind: PackageKind::Tool,
                        name: "@zack/echo".to_string(),
                        version: "0.1.0".to_string(),
                        integrity: "sha256-echo-1".to_string(),
                    },
                    ResolvedPackage {
                        kind: PackageKind::Tool,
                        name: "@zack/echo".to_string(),
                        version: "0.2.0".to_string(),
                        integrity: "sha256-echo-2".to_string(),
                    },
                ],
            },
            &[],
        );

        let desired =
            DesiredSet::from_cli_or_agent_json(&Value::Null, Some("@zack/echo"), false).unwrap();
        let err = ResolvePlan::from_lock(&desired, &lock).unwrap_err();

        assert!(
            format!("{err:#}").contains("multiple locked versions found"),
            "{err:#}"
        );
    }
}
