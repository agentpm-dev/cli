use anyhow::{Context, Result, anyhow, bail};
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
    Skill,
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
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub skills: Vec<String>,
    #[serde(default)]
    pub reserved: ReservedReferences,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct ReservedReferences {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
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
        key: String,
        name: String,
        version: String,
        tools: Vec<String>,
        skills: Vec<String>,
        reserved: ReservedReferences,
    },
    RegistryAgent {
        package_key: String,
        tools: Vec<String>,
        skills: Vec<String>,
        reserved: ReservedReferences,
    },
    LocalSkill {
        key: String,
        name: String,
        version: String,
        tools: Vec<String>,
    },
    RegistrySkill {
        package_key: String,
        tools: Vec<String>,
    },
}

impl DesiredSet {
    /// Build from CLI spec or from top-level package dependencies in a local manifest.
    pub fn from_cli_or_agent_json(
        meta: &serde_json::Value,
        spec: Option<&str>,
        _update_range: bool,
    ) -> Result<Self> {
        if let Some(spec) = spec {
            let req = parse_package_spec(spec)?;
            Ok(DesiredSet { items: vec![req] })
        } else {
            let kind = meta
                .get("kind")
                .and_then(Value::as_str)
                .ok_or_else(|| anyhow!("agent.json is missing kind"))?;

            match kind {
                "agent" => {
                    let mut items = parse_manifest_requirements(meta, "tools", PackageKind::Tool)?;
                    items.extend(parse_manifest_requirements(
                        meta,
                        "skills",
                        PackageKind::Skill,
                    )?);
                    Ok(DesiredSet { items })
                }
                "skill" => Ok(DesiredSet {
                    items: parse_manifest_requirements(meta, "tools", PackageKind::Tool)?,
                }),
                other => Err(anyhow!(
                    "agentpm install currently supports local kind=\"agent\" or kind=\"skill\" manifests, found kind=\"{}\"",
                    other
                )),
            }
        }
    }
}

fn parse_manifest_requirements(
    meta: &serde_json::Value,
    field: &str,
    kind: PackageKind,
) -> Result<Vec<PackageRequirement>> {
    let items = meta
        .get(field)
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    let mut out = Vec::new();
    for item in items {
        let (pkg, rng) = match item {
            Value::String(s) => parse_tool_str(&s)?,
            Value::Object(map) => {
                let name = map
                    .get("name")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow!("{field} entry missing name"))?
                    .to_string();
                let rng = map
                    .get("version")
                    .and_then(|v| v.as_str())
                    .unwrap_or("*")
                    .to_string();
                (name, rng)
            }
            _ => return Err(anyhow!("{field} entry must be a string or an object")),
        };

        out.push(PackageRequirement::new(kind, pkg, rng));
    }
    Ok(out)
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
                key,
                name,
                version,
                tools,
                skills,
                reserved,
            } => {
                root_map.insert(
                    key.clone(),
                    LockedRoot {
                        name: Some(name.clone()),
                        version: Some(version.clone()),
                        tools: tools.clone(),
                        skills: skills.clone(),
                        reserved: reserved.clone(),
                    },
                );
            }
            LockRoot::RegistryAgent {
                package_key,
                tools,
                skills,
                reserved,
            } => {
                root_map.insert(
                    package_key.clone(),
                    LockedRoot {
                        name: None,
                        version: None,
                        tools: tools.clone(),
                        skills: skills.clone(),
                        reserved: reserved.clone(),
                    },
                );
            }
            LockRoot::LocalSkill {
                key,
                name,
                version,
                tools,
            } => {
                root_map.insert(
                    key.clone(),
                    LockedRoot {
                        name: Some(name.clone()),
                        version: Some(version.clone()),
                        tools: tools.clone(),
                        skills: Vec::new(),
                        reserved: ReservedReferences::default(),
                    },
                );
            }
            LockRoot::RegistrySkill { package_key, tools } => {
                root_map.insert(
                    package_key.clone(),
                    LockedRoot {
                        name: None,
                        version: None,
                        tools: tools.clone(),
                        skills: Vec::new(),
                        reserved: ReservedReferences::default(),
                    },
                );
            }
        }
    }

    lock_from_packages_and_roots(packages, root_map)
}

pub fn lock_from_packages_and_roots(
    packages: BTreeMap<String, LockedPackage>,
    mut roots: BTreeMap<String, LockedRoot>,
) -> Lock {
    migrate_reserved_skills(&packages, &mut roots);
    let lockfile_version = if requires_v3_lock(&packages, &roots) {
        3
    } else {
        2
    };
    Lock::V2(LockV2 {
        lockfile_version,
        generated: Utc::now(),
        packages,
        roots,
    })
}

fn requires_v3_lock(
    packages: &BTreeMap<String, LockedPackage>,
    roots: &BTreeMap<String, LockedRoot>,
) -> bool {
    packages.values().any(|pkg| pkg.kind == PackageKind::Skill)
        || roots.iter().any(|(key, root)| {
            key == "local:skill"
                || key.starts_with("local:skill:")
                || key.starts_with("skill:")
                || !root.skills.is_empty()
        })
}

fn migrate_reserved_skills(
    packages: &BTreeMap<String, LockedPackage>,
    roots: &mut BTreeMap<String, LockedRoot>,
) {
    for root in roots.values_mut() {
        if root.reserved.skills.is_empty() {
            continue;
        }

        let mut unresolved = Vec::new();
        for raw in root.reserved.skills.drain(..) {
            match parse_locked_reserved_skill_ref(&raw).and_then(|(name, range)| {
                resolve_declared_package_from_packages(packages, &name, &range, PackageKind::Skill)
            }) {
                Ok(Some(pkg)) => {
                    let key = package_key(pkg.kind, &pkg.name, &pkg.version);
                    if !root.skills.contains(&key) {
                        root.skills.push(key);
                    }
                }
                Ok(None) | Err(_) => unresolved.push(raw),
            }
        }

        root.reserved.skills = unresolved;
    }
}

fn parse_locked_reserved_skill_ref(raw: &Value) -> Result<(String, String)> {
    match raw {
        Value::String(s) => parse_tool_str(s),
        Value::Object(map) => {
            let name = map
                .get("name")
                .and_then(Value::as_str)
                .ok_or_else(|| anyhow!("reserved skill entry missing name"))?
                .to_string();
            let range = map
                .get("version")
                .and_then(Value::as_str)
                .unwrap_or("*")
                .to_string();
            Ok((name, range))
        }
        _ => Err(anyhow!(
            "reserved skill entry must be a string or an object"
        )),
    }
}

pub fn resolve_declared_package_from_packages(
    packages: &BTreeMap<String, LockedPackage>,
    name: &str,
    range: &str,
    kind: PackageKind,
) -> Result<Option<LockedPackage>> {
    let exact = if range == "*" {
        None
    } else {
        semver::Version::parse(range).ok()
    };
    let req =
        if exact.is_none() && range != "*" {
            Some(semver::VersionReq::parse(range).with_context(|| {
                format!("declared package range for {} is not valid semver", name)
            })?)
        } else {
            None
        };

    let mut matches: Vec<(semver::Version, LockedPackage)> = Vec::new();
    for pkg in packages
        .values()
        .filter(|pkg| pkg.kind == kind && pkg.name == name)
    {
        let version = match semver::Version::parse(&pkg.version) {
            Ok(version) => version,
            Err(_) => continue,
        };
        let is_match = if let Some(exact) = &exact {
            &version == exact
        } else if let Some(req) = &req {
            req.matches(&version)
        } else {
            true
        };
        if is_match {
            matches.push((version, pkg.clone()));
        }
    }

    matches.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(matches.pop().map(|(_, pkg)| pkg))
}

pub fn package_key(kind: PackageKind, name: &str, version: &str) -> String {
    format!("{}:{}@{}", kind.as_str(), name, version)
}

impl PackageKind {
    pub fn as_str(self) -> &'static str {
        match self {
            PackageKind::Tool => "tool",
            PackageKind::Agent => "agent",
            PackageKind::Skill => "skill",
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

    pub fn lockfile_version(&self) -> u32 {
        match self {
            Lock::V1(lock) => lock.lockfile_version,
            Lock::V2(lock) => lock.lockfile_version,
        }
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
    fn desired_set_from_skill_manifest_collects_top_level_tools() {
        let desired = DesiredSet::from_cli_or_agent_json(
            &json!({
                "kind": "skill",
                "name": "incident-commander",
                "version": "0.1.0",
                "tools": [{"name":"@zack/slack-post-message","version":"^0.1"}]
            }),
            None,
            false,
        )
        .unwrap();

        assert_eq!(desired.items.len(), 1);
        assert_eq!(desired.items[0].kind, PackageKind::Tool);
        assert_eq!(desired.items[0].name, "@zack/slack-post-message");
    }

    #[test]
    fn desired_set_from_agent_manifest_collects_tools_and_skills() {
        let desired = DesiredSet::from_cli_or_agent_json(
            &json!({
                "kind": "agent",
                "name": "support-agent",
                "version": "0.1.0",
                "tools": ["@zack/echo@0.1.0"],
                "skills": ["@zack/triage-skill@0.2.0"]
            }),
            None,
            false,
        )
        .unwrap();

        assert_eq!(desired.items.len(), 2);
        assert_eq!(desired.items[0].kind, PackageKind::Tool);
        assert_eq!(desired.items[1].kind, PackageKind::Skill);
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
                "kind": "agent",
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
                items: vec![
                    ResolvedPackage {
                        kind: PackageKind::Tool,
                        name: "@zack/slack-post-message".to_string(),
                        version: "0.1.0".to_string(),
                        integrity: "sha256-slack".to_string(),
                    },
                    ResolvedPackage {
                        kind: PackageKind::Skill,
                        name: "@zack/triage-skill".to_string(),
                        version: "0.1.0".to_string(),
                        integrity: "sha256-skill".to_string(),
                    },
                ],
            },
            &[LockRoot::LocalAgent {
                key: "local:agent".to_string(),
                name: "support-agent".to_string(),
                version: "0.1.0".to_string(),
                tools: vec!["tool:@zack/slack-post-message@0.1.0".to_string()],
                skills: Vec::new(),
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
        assert_eq!(lock.lockfile_version, 3);
        let root = lock.roots.get("local:agent").unwrap();
        assert_eq!(root.name.as_deref(), Some("support-agent"));
        assert_eq!(
            root.tools,
            vec!["tool:@zack/slack-post-message@0.1.0".to_string()]
        );
        assert_eq!(
            root.skills,
            vec!["skill:@zack/triage-skill@0.1.0".to_string()]
        );
        assert!(root.reserved.skills.is_empty());
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
                skills: Vec::new(),
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
    fn lock_from_packages_and_roots_migrates_reserved_skills_to_first_class_root_field() {
        let lock = lock_from_packages_and_roots(
            BTreeMap::from([(
                "skill:@zack/triage-skill@0.1.0".to_string(),
                LockedPackage {
                    kind: PackageKind::Skill,
                    name: "@zack/triage-skill".to_string(),
                    version: "0.1.0".to_string(),
                    integrity: "sha256-skill".to_string(),
                },
            )]),
            BTreeMap::from([(
                "local:agent".to_string(),
                LockedRoot {
                    name: Some("support-agent".to_string()),
                    version: Some("0.1.0".to_string()),
                    tools: Vec::new(),
                    skills: Vec::new(),
                    reserved: ReservedReferences {
                        skills: vec![json!("@zack/triage-skill@0.1.0")],
                        knowledge: Vec::new(),
                        memory: Vec::new(),
                        profiles: Vec::new(),
                    },
                },
            )]),
        );

        let Lock::V2(lock) = lock else {
            panic!("expected modern lockfile");
        };
        assert_eq!(lock.lockfile_version, 3);
        let root = lock.roots.get("local:agent").unwrap();
        assert_eq!(
            root.skills,
            vec!["skill:@zack/triage-skill@0.1.0".to_string()]
        );
        assert!(root.reserved.skills.is_empty());
    }

    #[test]
    fn lock_from_packages_and_roots_preserves_unresolvable_reserved_skills() {
        let lock = lock_from_packages_and_roots(
            BTreeMap::new(),
            BTreeMap::from([(
                "local:agent".to_string(),
                LockedRoot {
                    name: Some("support-agent".to_string()),
                    version: Some("0.1.0".to_string()),
                    tools: Vec::new(),
                    skills: Vec::new(),
                    reserved: ReservedReferences {
                        skills: vec![json!("@zack/missing-skill@0.1.0")],
                        knowledge: Vec::new(),
                        memory: Vec::new(),
                        profiles: Vec::new(),
                    },
                },
            )]),
        );

        let Lock::V2(lock) = lock else {
            panic!("expected modern lockfile");
        };
        let root = lock.roots.get("local:agent").unwrap();
        assert!(root.skills.is_empty());
        assert_eq!(
            root.reserved.skills,
            vec![json!("@zack/missing-skill@0.1.0")]
        );
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
                    skills: Vec::new(),
                    reserved: ReservedReferences::default(),
                },
                LockRoot::RegistryAgent {
                    package_key: "agent:@zack/escalation-agent@0.1.0".to_string(),
                    tools: vec!["tool:@zack/slack-post-message@0.2.0".to_string()],
                    skills: Vec::new(),
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
