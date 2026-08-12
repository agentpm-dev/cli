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
    Knowledge,
    Memory,
    Profile,
    Loop,
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
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub knowledge: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub memory: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub profiles: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub r#loop: Option<String>,
    #[serde(default, skip_serializing_if = "ReservedReferences::is_empty")]
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

impl ReservedReferences {
    pub fn is_empty(&self) -> bool {
        self.skills.is_empty()
            && self.knowledge.is_empty()
            && self.memory.is_empty()
            && self.profiles.is_empty()
    }
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
        knowledge: Vec<String>,
        memory: Vec<String>,
        profiles: Vec<String>,
        r#loop: Option<String>,
        reserved: ReservedReferences,
    },
    RegistryAgent {
        package_key: String,
        tools: Vec<String>,
        skills: Vec<String>,
        knowledge: Vec<String>,
        memory: Vec<String>,
        profiles: Vec<String>,
        r#loop: Option<String>,
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
                    items.extend(parse_manifest_requirements(
                        meta,
                        "knowledge",
                        PackageKind::Knowledge,
                    )?);
                    items.extend(parse_manifest_requirements(
                        meta,
                        "memory",
                        PackageKind::Memory,
                    )?);
                    items.extend(parse_manifest_requirements(
                        meta,
                        "profiles",
                        PackageKind::Profile,
                    )?);
                    if let Some(loop_requirement) =
                        parse_manifest_requirement(meta, "loop", PackageKind::Loop)?
                    {
                        items.push(loop_requirement);
                    }
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
        out.push(parse_manifest_dependency_value(&item, field, kind)?);
    }
    Ok(out)
}

fn parse_manifest_requirement(
    meta: &serde_json::Value,
    field: &str,
    kind: PackageKind,
) -> Result<Option<PackageRequirement>> {
    let Some(item) = meta.get(field) else {
        return Ok(None);
    };

    if item.is_null() {
        return Ok(None);
    }

    Ok(Some(parse_manifest_dependency_value(item, field, kind)?))
}

fn parse_manifest_dependency_value(
    item: &Value,
    field: &str,
    kind: PackageKind,
) -> Result<PackageRequirement> {
    let (pkg, rng) = match item {
        Value::String(s) => parse_tool_str(s)?,
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

    Ok(PackageRequirement::new(kind, pkg, rng))
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

    // Direct CLI specs still default to kind=tool in the outbound request.
    //
    // The backend resolves by package name and returns the authoritative kind
    // in the response plan, which is how direct Skill installs work today even
    // though the initial CLI request is tool-shaped. If direct specs later
    // become explicitly kind-aware on the CLI side, this parser is the place
    // to stop defaulting to `tool`.

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
                knowledge,
                memory,
                profiles,
                r#loop,
                reserved,
            } => {
                root_map.insert(
                    key.clone(),
                    LockedRoot {
                        name: Some(name.clone()),
                        version: Some(version.clone()),
                        tools: tools.clone(),
                        skills: skills.clone(),
                        knowledge: knowledge.clone(),
                        memory: memory.clone(),
                        profiles: profiles.clone(),
                        r#loop: r#loop.clone(),
                        reserved: reserved.clone(),
                    },
                );
            }
            LockRoot::RegistryAgent {
                package_key,
                tools,
                skills,
                knowledge,
                memory,
                profiles,
                r#loop,
                reserved,
            } => {
                root_map.insert(
                    package_key.clone(),
                    LockedRoot {
                        name: None,
                        version: None,
                        tools: tools.clone(),
                        skills: skills.clone(),
                        knowledge: knowledge.clone(),
                        memory: memory.clone(),
                        profiles: profiles.clone(),
                        r#loop: r#loop.clone(),
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
                        knowledge: Vec::new(),
                        memory: Vec::new(),
                        profiles: Vec::new(),
                        r#loop: None,
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
                        knowledge: Vec::new(),
                        memory: Vec::new(),
                        profiles: Vec::new(),
                        r#loop: None,
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
    prune_standalone_leaf_roots(&mut roots);
    migrate_reserved_skills(&packages, &mut roots);
    migrate_reserved_knowledge(&packages, &mut roots);
    migrate_reserved_memory(&packages, &mut roots);
    migrate_reserved_profiles(&packages, &mut roots);
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

fn prune_standalone_leaf_roots(roots: &mut BTreeMap<String, LockedRoot>) {
    roots.retain(|key, _| {
        !key.starts_with("knowledge:") && !key.starts_with("memory:") && !key.starts_with("loop:")
    });
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
                || key.starts_with("knowledge:")
                || !root.knowledge.is_empty()
                || key.starts_with("memory:")
                || !root.memory.is_empty()
                || key.starts_with("profile:")
                || !root.profiles.is_empty()
                || key.starts_with("loop:")
                || root.r#loop.is_some()
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
            match parse_locked_reserved_package_ref(&raw).and_then(|(name, range)| {
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

fn migrate_reserved_knowledge(
    packages: &BTreeMap<String, LockedPackage>,
    roots: &mut BTreeMap<String, LockedRoot>,
) {
    for root in roots.values_mut() {
        if root.reserved.knowledge.is_empty() {
            continue;
        }

        let mut unresolved = Vec::new();
        for raw in root.reserved.knowledge.drain(..) {
            match parse_locked_reserved_package_ref(&raw).and_then(|(name, range)| {
                resolve_declared_package_from_packages(
                    packages,
                    &name,
                    &range,
                    PackageKind::Knowledge,
                )
            }) {
                Ok(Some(pkg)) => {
                    let key = package_key(pkg.kind, &pkg.name, &pkg.version);
                    if !root.knowledge.contains(&key) {
                        root.knowledge.push(key);
                    }
                }
                Ok(None) | Err(_) => unresolved.push(raw),
            }
        }

        root.reserved.knowledge = unresolved;
    }
}

fn migrate_reserved_memory(
    packages: &BTreeMap<String, LockedPackage>,
    roots: &mut BTreeMap<String, LockedRoot>,
) {
    for root in roots.values_mut() {
        if root.reserved.memory.is_empty() {
            continue;
        }

        let mut unresolved = Vec::new();
        for raw in root.reserved.memory.drain(..) {
            match parse_locked_reserved_package_ref(&raw).and_then(|(name, range)| {
                resolve_declared_package_from_packages(packages, &name, &range, PackageKind::Memory)
            }) {
                Ok(Some(pkg)) => {
                    let key = package_key(pkg.kind, &pkg.name, &pkg.version);
                    if !root.memory.contains(&key) {
                        root.memory.push(key);
                    }
                }
                Ok(None) | Err(_) => unresolved.push(raw),
            }
        }

        root.reserved.memory = unresolved;
    }
}

fn migrate_reserved_profiles(
    packages: &BTreeMap<String, LockedPackage>,
    roots: &mut BTreeMap<String, LockedRoot>,
) {
    for root in roots.values_mut() {
        if root.reserved.profiles.is_empty() {
            continue;
        }

        let mut unresolved = Vec::new();
        for raw in root.reserved.profiles.drain(..) {
            match parse_locked_reserved_package_ref(&raw).and_then(|(name, range)| {
                resolve_declared_package_from_packages(
                    packages,
                    &name,
                    &range,
                    PackageKind::Profile,
                )
            }) {
                Ok(Some(pkg)) => {
                    let key = package_key(pkg.kind, &pkg.name, &pkg.version);
                    if !root.profiles.contains(&key) {
                        root.profiles.push(key);
                    }
                }
                Ok(None) | Err(_) => unresolved.push(raw),
            }
        }

        root.reserved.profiles = unresolved;
    }
}

fn parse_locked_reserved_package_ref(raw: &Value) -> Result<(String, String)> {
    match raw {
        Value::String(s) => parse_tool_str(s),
        Value::Object(map) => {
            let name = map
                .get("name")
                .and_then(Value::as_str)
                .ok_or_else(|| anyhow!("reserved package entry missing name"))?
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

pub(crate) fn split_package_ref(package: &str) -> Result<(String, String)> {
    if !package.starts_with('@') {
        return Err(anyhow!("package must be of form @owner/name"));
    }
    let mut parts = package[1..].splitn(2, '/');
    let owner = parts.next().ok_or_else(|| anyhow!("invalid package"))?;
    let name = parts.next().ok_or_else(|| anyhow!("invalid package"))?;
    Ok((owner.to_string(), name.to_string()))
}

impl PackageKind {
    pub fn as_str(self) -> &'static str {
        match self {
            PackageKind::Tool => "tool",
            PackageKind::Agent => "agent",
            PackageKind::Skill => "skill",
            PackageKind::Knowledge => "knowledge",
            PackageKind::Memory => "memory",
            PackageKind::Profile => "profile",
            PackageKind::Loop => "loop",
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
    fn desired_set_from_agent_manifest_collects_tools_skills_and_knowledge() {
        let desired = DesiredSet::from_cli_or_agent_json(
            &json!({
                "kind": "agent",
                "name": "support-agent",
                "version": "0.1.0",
                "tools": ["@zack/echo@0.1.0"],
                "skills": ["@zack/triage-skill@0.2.0"],
                "knowledge": ["@zack/python-docs@0.1.0"]
            }),
            None,
            false,
        )
        .unwrap();

        assert_eq!(desired.items.len(), 3);
        assert_eq!(desired.items[0].kind, PackageKind::Tool);
        assert_eq!(desired.items[1].kind, PackageKind::Skill);
        assert_eq!(desired.items[2].kind, PackageKind::Knowledge);
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
    fn package_requirement_supports_profile_kind() {
        let parsed = PackageRequirement::new(PackageKind::Profile, "@zack/support-persona", "^0.1");

        assert_eq!(parsed.kind, PackageKind::Profile);
        assert_eq!(parsed.name, "@zack/support-persona");
        assert_eq!(parsed.range, "^0.1");
        assert_eq!(
            package_key(parsed.kind, &parsed.name, "0.1.0"),
            "profile:@zack/support-persona@0.1.0"
        );
    }

    #[test]
    fn package_requirement_supports_loop_kind() {
        let parsed =
            PackageRequirement::new(PackageKind::Loop, "@zack/incident-response-loop", "^0.1");

        assert_eq!(parsed.kind, PackageKind::Loop);
        assert_eq!(parsed.name, "@zack/incident-response-loop");
        assert_eq!(parsed.range, "^0.1");
        assert_eq!(
            package_key(parsed.kind, &parsed.name, "0.1.0"),
            "loop:@zack/incident-response-loop@0.1.0"
        );
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
    fn lock_from_plan_records_local_agent_root_and_profiles() {
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
                    ResolvedPackage {
                        kind: PackageKind::Profile,
                        name: "@zack/support-persona".to_string(),
                        version: "0.1.0".to_string(),
                        integrity: "sha256-profile".to_string(),
                    },
                ],
            },
            &[LockRoot::LocalAgent {
                key: "local:agent".to_string(),
                name: "support-agent".to_string(),
                version: "0.1.0".to_string(),
                tools: vec!["tool:@zack/slack-post-message@0.1.0".to_string()],
                skills: Vec::new(),
                knowledge: Vec::new(),
                memory: Vec::new(),
                r#loop: None,
                profiles: vec!["profile:@zack/support-persona@0.1.0".to_string()],
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
        assert_eq!(
            root.profiles,
            vec!["profile:@zack/support-persona@0.1.0".to_string()]
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
                knowledge: Vec::new(),
                memory: Vec::new(),
                r#loop: None,
                profiles: vec!["profile:@zack/support-persona@0.1.0".to_string()],
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
        assert_eq!(
            lock.roots["agent:@zack/support-agent@0.1.0"].profiles,
            vec!["profile:@zack/support-persona@0.1.0".to_string()]
        );
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
                    knowledge: Vec::new(),
                    memory: Vec::new(),
                    r#loop: None,
                    profiles: Vec::new(),
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
                    knowledge: Vec::new(),
                    memory: Vec::new(),
                    r#loop: None,
                    profiles: Vec::new(),
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
    fn lock_from_packages_and_roots_migrates_reserved_knowledge_to_first_class_root_field() {
        let lock = lock_from_packages_and_roots(
            BTreeMap::from([(
                "knowledge:@zack/python-docs@0.1.0".to_string(),
                LockedPackage {
                    kind: PackageKind::Knowledge,
                    name: "@zack/python-docs".to_string(),
                    version: "0.1.0".to_string(),
                    integrity: "sha256-knowledge".to_string(),
                },
            )]),
            BTreeMap::from([(
                "local:agent".to_string(),
                LockedRoot {
                    name: Some("support-agent".to_string()),
                    version: Some("0.1.0".to_string()),
                    tools: Vec::new(),
                    skills: Vec::new(),
                    knowledge: Vec::new(),
                    memory: Vec::new(),
                    r#loop: None,
                    profiles: Vec::new(),
                    reserved: ReservedReferences {
                        skills: Vec::new(),
                        knowledge: vec![json!("@zack/python-docs@0.1.0")],
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
            root.knowledge,
            vec!["knowledge:@zack/python-docs@0.1.0".to_string()]
        );
        assert!(root.reserved.knowledge.is_empty());
    }

    #[test]
    fn lock_from_packages_and_roots_preserves_unresolvable_reserved_knowledge() {
        let lock = lock_from_packages_and_roots(
            BTreeMap::new(),
            BTreeMap::from([(
                "local:agent".to_string(),
                LockedRoot {
                    name: Some("support-agent".to_string()),
                    version: Some("0.1.0".to_string()),
                    tools: Vec::new(),
                    skills: Vec::new(),
                    knowledge: Vec::new(),
                    memory: Vec::new(),
                    r#loop: None,
                    profiles: Vec::new(),
                    reserved: ReservedReferences {
                        skills: Vec::new(),
                        knowledge: vec![json!("@zack/missing-knowledge@0.1.0")],
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
        assert!(root.knowledge.is_empty());
        assert_eq!(
            root.reserved.knowledge,
            vec![json!("@zack/missing-knowledge@0.1.0")]
        );
    }

    #[test]
    fn lock_from_packages_and_roots_migrates_reserved_memory_to_first_class_root_field() {
        let lock = lock_from_packages_and_roots(
            BTreeMap::from([(
                "memory:@zack/session-memory@0.1.0".to_string(),
                LockedPackage {
                    kind: PackageKind::Memory,
                    name: "@zack/session-memory".to_string(),
                    version: "0.1.0".to_string(),
                    integrity: "sha256-memory".to_string(),
                },
            )]),
            BTreeMap::from([(
                "local:agent".to_string(),
                LockedRoot {
                    name: Some("support-agent".to_string()),
                    version: Some("0.1.0".to_string()),
                    tools: Vec::new(),
                    skills: Vec::new(),
                    knowledge: Vec::new(),
                    memory: Vec::new(),
                    r#loop: None,
                    profiles: Vec::new(),
                    reserved: ReservedReferences {
                        skills: Vec::new(),
                        knowledge: Vec::new(),
                        memory: vec![json!("@zack/session-memory@0.1.0")],
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
            root.memory,
            vec!["memory:@zack/session-memory@0.1.0".to_string()]
        );
        assert!(root.reserved.memory.is_empty());
    }

    #[test]
    fn lock_from_packages_and_roots_preserves_unresolvable_reserved_memory() {
        let lock = lock_from_packages_and_roots(
            BTreeMap::new(),
            BTreeMap::from([(
                "local:agent".to_string(),
                LockedRoot {
                    name: Some("support-agent".to_string()),
                    version: Some("0.1.0".to_string()),
                    tools: Vec::new(),
                    skills: Vec::new(),
                    knowledge: Vec::new(),
                    memory: Vec::new(),
                    r#loop: None,
                    profiles: Vec::new(),
                    reserved: ReservedReferences {
                        skills: Vec::new(),
                        knowledge: Vec::new(),
                        memory: vec![json!("@zack/missing-memory@0.1.0")],
                        profiles: Vec::new(),
                    },
                },
            )]),
        );

        let Lock::V2(lock) = lock else {
            panic!("expected modern lockfile");
        };
        let root = lock.roots.get("local:agent").unwrap();
        assert!(root.memory.is_empty());
        assert_eq!(
            root.reserved.memory,
            vec![json!("@zack/missing-memory@0.1.0")]
        );
    }

    #[test]
    fn lock_from_plan_deduplicates_shared_memory_package_across_multiple_agent_roots() {
        let lock = lock_from_plan(
            &ResolvePlan {
                items: vec![ResolvedPackage {
                    kind: PackageKind::Memory,
                    name: "@zack/session-memory".to_string(),
                    version: "0.1.0".to_string(),
                    integrity: "sha256-memory".to_string(),
                }],
            },
            &[
                LockRoot::LocalAgent {
                    key: "local:agent".to_string(),
                    name: "support-agent".to_string(),
                    version: "0.1.0".to_string(),
                    tools: Vec::new(),
                    skills: Vec::new(),
                    knowledge: Vec::new(),
                    memory: vec!["memory:@zack/session-memory@0.1.0".to_string()],
                    r#loop: None,
                    profiles: Vec::new(),
                    reserved: ReservedReferences::default(),
                },
                LockRoot::LocalAgent {
                    key: "local:agent:agents/reviewer.agent.json".to_string(),
                    name: "reviewer".to_string(),
                    version: "0.1.0".to_string(),
                    tools: Vec::new(),
                    skills: Vec::new(),
                    knowledge: Vec::new(),
                    memory: vec!["memory:@zack/session-memory@0.1.0".to_string()],
                    r#loop: None,
                    profiles: Vec::new(),
                    reserved: ReservedReferences::default(),
                },
            ],
        );

        let Lock::V2(lock) = lock else {
            panic!("expected v2 lockfile");
        };
        assert_eq!(lock.lockfile_version, 3);
        assert_eq!(
            lock.packages
                .keys()
                .filter(|key| key.as_str() == "memory:@zack/session-memory@0.1.0")
                .count(),
            1
        );
        assert_eq!(
            lock.roots["local:agent"].memory,
            vec!["memory:@zack/session-memory@0.1.0".to_string()]
        );
        assert_eq!(
            lock.roots["local:agent:agents/reviewer.agent.json"].memory,
            vec!["memory:@zack/session-memory@0.1.0".to_string()]
        );
    }

    #[test]
    fn lock_from_plan_deduplicates_shared_profile_package_across_multiple_agent_roots() {
        let lock = lock_from_plan(
            &ResolvePlan {
                items: vec![ResolvedPackage {
                    kind: PackageKind::Profile,
                    name: "@zack/support-persona".to_string(),
                    version: "0.1.0".to_string(),
                    integrity: "sha256-profile".to_string(),
                }],
            },
            &[
                LockRoot::LocalAgent {
                    key: "local:agent".to_string(),
                    name: "support-agent".to_string(),
                    version: "0.1.0".to_string(),
                    tools: Vec::new(),
                    skills: Vec::new(),
                    knowledge: Vec::new(),
                    memory: Vec::new(),
                    r#loop: None,
                    profiles: vec!["profile:@zack/support-persona@0.1.0".to_string()],
                    reserved: ReservedReferences::default(),
                },
                LockRoot::LocalAgent {
                    key: "local:agent:agents/reviewer.agent.json".to_string(),
                    name: "reviewer".to_string(),
                    version: "0.1.0".to_string(),
                    tools: Vec::new(),
                    skills: Vec::new(),
                    knowledge: Vec::new(),
                    memory: Vec::new(),
                    r#loop: None,
                    profiles: vec!["profile:@zack/support-persona@0.1.0".to_string()],
                    reserved: ReservedReferences::default(),
                },
            ],
        );

        let Lock::V2(lock) = lock else {
            panic!("expected v2 lockfile");
        };
        assert_eq!(lock.lockfile_version, 3);
        assert_eq!(
            lock.packages
                .keys()
                .filter(|key| key.as_str() == "profile:@zack/support-persona@0.1.0")
                .count(),
            1
        );
        assert_eq!(
            lock.roots["local:agent"].profiles,
            vec!["profile:@zack/support-persona@0.1.0".to_string()]
        );
        assert_eq!(
            lock.roots["local:agent:agents/reviewer.agent.json"].profiles,
            vec!["profile:@zack/support-persona@0.1.0".to_string()]
        );
    }

    #[test]
    fn lock_from_plan_deduplicates_shared_loop_package_across_multiple_agent_roots() {
        let lock = lock_from_plan(
            &ResolvePlan {
                items: vec![ResolvedPackage {
                    kind: PackageKind::Loop,
                    name: "@zack/incident-response-loop".to_string(),
                    version: "0.1.0".to_string(),
                    integrity: "sha256-loop".to_string(),
                }],
            },
            &[
                LockRoot::LocalAgent {
                    key: "local:agent".to_string(),
                    name: "support-agent".to_string(),
                    version: "0.1.0".to_string(),
                    tools: Vec::new(),
                    skills: Vec::new(),
                    knowledge: Vec::new(),
                    memory: Vec::new(),
                    r#loop: Some("loop:@zack/incident-response-loop@0.1.0".to_string()),
                    profiles: Vec::new(),
                    reserved: ReservedReferences::default(),
                },
                LockRoot::LocalAgent {
                    key: "local:agent:agents/reviewer.agent.json".to_string(),
                    name: "reviewer".to_string(),
                    version: "0.1.0".to_string(),
                    tools: Vec::new(),
                    skills: Vec::new(),
                    knowledge: Vec::new(),
                    memory: Vec::new(),
                    r#loop: Some("loop:@zack/incident-response-loop@0.1.0".to_string()),
                    profiles: Vec::new(),
                    reserved: ReservedReferences::default(),
                },
            ],
        );

        let Lock::V2(lock) = lock else {
            panic!("expected v2 lockfile");
        };
        assert_eq!(lock.lockfile_version, 3);
        assert_eq!(
            lock.packages
                .keys()
                .filter(|key| key.as_str() == "loop:@zack/incident-response-loop@0.1.0")
                .count(),
            1
        );
        assert_eq!(
            lock.roots["local:agent"].r#loop.as_deref(),
            Some("loop:@zack/incident-response-loop@0.1.0")
        );
        assert_eq!(
            lock.roots["local:agent:agents/reviewer.agent.json"]
                .r#loop
                .as_deref(),
            Some("loop:@zack/incident-response-loop@0.1.0")
        );
    }

    #[test]
    fn lock_from_packages_and_roots_migrates_reserved_profiles_to_first_class_root_field() {
        let lock = lock_from_packages_and_roots(
            BTreeMap::from([(
                "profile:@zack/support-persona@0.1.0".to_string(),
                LockedPackage {
                    kind: PackageKind::Profile,
                    name: "@zack/support-persona".to_string(),
                    version: "0.1.0".to_string(),
                    integrity: "sha256-profile".to_string(),
                },
            )]),
            BTreeMap::from([(
                "local:agent".to_string(),
                LockedRoot {
                    name: Some("support-agent".to_string()),
                    version: Some("0.1.0".to_string()),
                    tools: Vec::new(),
                    skills: Vec::new(),
                    knowledge: Vec::new(),
                    memory: Vec::new(),
                    r#loop: None,
                    profiles: Vec::new(),
                    reserved: ReservedReferences {
                        skills: Vec::new(),
                        knowledge: Vec::new(),
                        memory: Vec::new(),
                        profiles: vec![json!("@zack/support-persona@0.1.0")],
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
            root.profiles,
            vec!["profile:@zack/support-persona@0.1.0".to_string()]
        );
        assert!(root.reserved.profiles.is_empty());
    }

    #[test]
    fn lock_from_packages_and_roots_preserves_unresolvable_reserved_profiles() {
        let lock = lock_from_packages_and_roots(
            BTreeMap::new(),
            BTreeMap::from([(
                "local:agent".to_string(),
                LockedRoot {
                    name: Some("support-agent".to_string()),
                    version: Some("0.1.0".to_string()),
                    tools: Vec::new(),
                    skills: Vec::new(),
                    knowledge: Vec::new(),
                    memory: Vec::new(),
                    r#loop: None,
                    profiles: Vec::new(),
                    reserved: ReservedReferences {
                        skills: Vec::new(),
                        knowledge: Vec::new(),
                        memory: Vec::new(),
                        profiles: vec![json!("@zack/missing-profile@0.1.0")],
                    },
                },
            )]),
        );

        let Lock::V2(lock) = lock else {
            panic!("expected modern lockfile");
        };
        let root = lock.roots.get("local:agent").unwrap();
        assert!(root.profiles.is_empty());
        assert_eq!(
            root.reserved.profiles,
            vec![json!("@zack/missing-profile@0.1.0")]
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
                    knowledge: Vec::new(),
                    memory: Vec::new(),
                    r#loop: None,
                    profiles: Vec::new(),
                    reserved: ReservedReferences::default(),
                },
                LockRoot::RegistryAgent {
                    package_key: "agent:@zack/escalation-agent@0.1.0".to_string(),
                    tools: vec!["tool:@zack/slack-post-message@0.2.0".to_string()],
                    skills: Vec::new(),
                    knowledge: Vec::new(),
                    memory: Vec::new(),
                    r#loop: None,
                    profiles: Vec::new(),
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
