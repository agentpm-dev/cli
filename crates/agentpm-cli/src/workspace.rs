use crate::manifest::{load_manifest_value, write_manifest_pretty_atomic};
use crate::prelude::*;
use crate::semver::types::{
    DesiredSet, Lock, LockRoot, PackageKind, PackageRequirement, ReservedReferences, ResolvePlan,
    lock_from_plan, package_key, resolve_declared_package_from_packages,
};
use anyhow::anyhow;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct WorkspacePackageRoot {
    pub name: String,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct WorkspacePackageRoots {
    #[serde(default)]
    pub tools: Vec<WorkspacePackageRoot>,
    #[serde(default)]
    pub agents: Vec<WorkspacePackageRoot>,
    #[serde(default)]
    pub skills: Vec<WorkspacePackageRoot>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorkspaceMetadata {
    pub schema_version: u32,
    #[serde(default)]
    pub manifests: Vec<String>,
    #[serde(default)]
    pub package_roots: WorkspacePackageRoots,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TemplateOriginMetadata {
    pub schema_version: u32,
    pub source: String,
    pub kind: String,
    pub name: String,
    pub version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub integrity: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    pub generated_at: String,
    #[serde(default)]
    pub variables: BTreeMap<String, String>,
}

#[derive(Debug, Clone)]
pub struct LocalManifestRoot {
    pub rel_path: String,
    pub manifest_value: Value,
}

pub fn workspace_metadata_path(root: &Path) -> PathBuf {
    root.join("agentpm.workspace.json")
}

pub fn template_metadata_path(root: &Path) -> PathBuf {
    root.join(".agentpm").join("template.json")
}

pub fn local_agent_root_key(rel_path: &str) -> String {
    if rel_path == "agent.json" {
        "local:agent".to_string()
    } else {
        format!("local:agent:{rel_path}")
    }
}

pub fn read_workspace_metadata(root: &Path) -> Result<Option<WorkspaceMetadata>> {
    let path = workspace_metadata_path(root);
    if !path.exists() {
        return Ok(None);
    }
    let (value, _) = load_manifest_value(&path)?;
    let metadata: WorkspaceMetadata =
        serde_json::from_value(value).with_context(|| format!("parsing {}", path.display()))?;
    validate_workspace_metadata(root, &metadata)?;
    Ok(Some(metadata))
}

pub fn write_workspace_metadata(root: &Path, metadata: &WorkspaceMetadata) -> Result<()> {
    validate_workspace_metadata(root, metadata)?;
    write_manifest_pretty_atomic(
        &workspace_metadata_path(root),
        &serde_json::to_value(metadata)?,
    )
}

pub fn write_template_metadata(root: &Path, metadata: &TemplateOriginMetadata) -> Result<()> {
    write_manifest_pretty_atomic(
        &template_metadata_path(root),
        &serde_json::to_value(metadata)?,
    )
}

pub fn load_workspace_local_manifests(
    root: &Path,
    metadata: &WorkspaceMetadata,
) -> Result<Vec<LocalManifestRoot>> {
    let mut manifests = Vec::new();
    for rel_path in &metadata.manifests {
        let safe_rel = validate_manifest_rel_path(rel_path)?;
        let abs_path = root.join(&safe_rel);
        let (manifest_value, _) = load_manifest_value(&abs_path)
            .with_context(|| format!("loading workspace manifest {}", abs_path.display()))?;
        if manifest_value.get("kind").and_then(Value::as_str) != Some("agent") {
            return Err(anyhow!(
                "workspace manifest {} must be kind=\"agent\"",
                safe_rel.display()
            ));
        }
        manifests.push(LocalManifestRoot {
            rel_path: path_to_slash_string(&safe_rel),
            manifest_value,
        });
    }
    Ok(manifests)
}

pub fn desired_from_workspace(
    manifests: &[LocalManifestRoot],
    package_roots: &WorkspacePackageRoots,
) -> Result<DesiredSet> {
    let mut items = Vec::new();

    for manifest in manifests {
        items.extend(
            DesiredSet::from_cli_or_agent_json(&manifest.manifest_value, None, false)?.items,
        );
    }

    for tool in &package_roots.tools {
        items.push(PackageRequirement::tool(
            tool.name.clone(),
            tool.version.clone(),
        ));
    }

    for agent in &package_roots.agents {
        items.push(PackageRequirement::new(
            PackageKind::Agent,
            agent.name.clone(),
            agent.version.clone(),
        ));
    }

    for skill in &package_roots.skills {
        items.push(PackageRequirement::new(
            PackageKind::Skill,
            skill.name.clone(),
            skill.version.clone(),
        ));
    }

    Ok(DesiredSet { items })
}

pub fn build_workspace_lock(
    manifests: &[LocalManifestRoot],
    package_roots: &WorkspacePackageRoots,
    plan: &ResolvePlan,
    install_root: &Path,
) -> Result<Lock> {
    let packages = plan_to_locked_packages(plan);
    let mut roots = Vec::new();

    for manifest in manifests {
        roots.push(LockRoot::LocalAgent {
            key: local_agent_root_key(&manifest.rel_path),
            name: manifest
                .manifest_value
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or("local-agent")
                .to_string(),
            version: manifest
                .manifest_value
                .get("version")
                .and_then(Value::as_str)
                .unwrap_or("0.0.0")
                .to_string(),
            tools: resolve_tools_for_manifest(
                &manifest.manifest_value,
                &format!("workspace manifest {}", manifest.rel_path),
                &packages,
            )?,
            skills: resolve_packages_for_manifest(
                &manifest.manifest_value,
                PackageKind::Skill,
                &format!("workspace manifest {}", manifest.rel_path),
                &packages,
            )?,
            reserved: reserved_refs_from_manifest(&manifest.manifest_value),
        });
    }

    for agent_root in &package_roots.agents {
        roots.push(build_registry_agent_root(
            &agent_root.name,
            &agent_root.version,
            &packages,
            install_root,
        )?);
    }

    for skill_root in &package_roots.skills {
        roots.push(build_registry_skill_root(
            &skill_root.name,
            &skill_root.version,
            &packages,
            install_root,
        )?);
    }

    Ok(lock_from_plan(plan, &roots))
}

pub fn validate_workspace_metadata(root: &Path, metadata: &WorkspaceMetadata) -> Result<()> {
    if metadata.schema_version != 1 {
        return Err(anyhow!(
            "{} has unsupported schema_version {}; expected 1",
            workspace_metadata_path(root).display(),
            metadata.schema_version
        ));
    }

    let mut seen_manifests = BTreeSet::new();
    for rel_path in &metadata.manifests {
        let safe_rel = validate_manifest_rel_path(rel_path)?;
        let normalized = path_to_slash_string(&safe_rel);
        if !seen_manifests.insert(normalized.clone()) {
            return Err(anyhow!(
                "workspace manifest path {} is duplicated",
                normalized
            ));
        }
    }

    validate_package_roots("tool", &metadata.package_roots.tools)?;
    validate_package_roots("agent", &metadata.package_roots.agents)?;
    validate_package_roots("skill", &metadata.package_roots.skills)?;

    Ok(())
}

fn validate_package_roots(kind: &str, roots: &[WorkspacePackageRoot]) -> Result<()> {
    let mut seen = BTreeSet::new();
    for root in roots {
        if !root.name.starts_with('@') || !root.name.contains('/') {
            return Err(anyhow!(
                "workspace {} package root {} must be of form @owner/name",
                kind,
                root.name
            ));
        }
        if root.version.trim().is_empty() {
            return Err(anyhow!(
                "workspace {} package root {} must include an exact version",
                kind,
                root.name
            ));
        }
        let key = format!("{}@{}", root.name, root.version);
        if !seen.insert(key.clone()) {
            return Err(anyhow!(
                "workspace {} package root {} is duplicated",
                kind,
                key
            ));
        }
    }
    Ok(())
}

fn validate_manifest_rel_path(rel_path: &str) -> Result<PathBuf> {
    if rel_path.trim().is_empty() {
        return Err(anyhow!("workspace manifest paths must not be empty"));
    }
    let path = Path::new(rel_path);
    if path.is_absolute() {
        return Err(anyhow!(
            "workspace manifest path {} must be relative",
            rel_path
        ));
    }

    let mut clean = PathBuf::new();
    for comp in path.components() {
        match comp {
            Component::Normal(segment) => clean.push(segment),
            Component::CurDir => {}
            Component::ParentDir => {
                return Err(anyhow!(
                    "workspace manifest path {} must not contain ..",
                    rel_path
                ));
            }
            Component::RootDir | Component::Prefix(_) => {
                return Err(anyhow!(
                    "workspace manifest path {} must be relative",
                    rel_path
                ));
            }
        }
    }

    Ok(clean)
}

fn path_to_slash_string(path: &Path) -> String {
    path.components()
        .map(|component| component.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("/")
}

fn plan_to_locked_packages(
    plan: &ResolvePlan,
) -> BTreeMap<String, crate::semver::types::LockedPackage> {
    let mut packages = BTreeMap::new();
    for item in &plan.items {
        packages.insert(
            package_key(item.kind, &item.name, &item.version),
            crate::semver::types::LockedPackage {
                kind: item.kind,
                name: item.name.clone(),
                version: item.version.clone(),
                integrity: item.integrity.clone(),
            },
        );
    }
    packages
}

fn build_registry_agent_root(
    package: &str,
    version: &str,
    packages: &BTreeMap<String, crate::semver::types::LockedPackage>,
    install_root: &Path,
) -> Result<LockRoot> {
    let (owner, name) = split_package_ref(package)?;
    let manifest_path = install_root
        .join(".agentpm")
        .join("agents")
        .join(owner)
        .join(name)
        .join(version)
        .join("agent.json");

    let (manifest_value, _) = load_manifest_value(&manifest_path).with_context(|| {
        format!(
            "loading installed agent manifest {}",
            manifest_path.display()
        )
    })?;

    Ok(LockRoot::RegistryAgent {
        package_key: package_key(PackageKind::Agent, package, version),
        tools: resolve_tools_for_manifest(
            &manifest_value,
            &format!("registry agent {}@{}", package, version),
            packages,
        )?,
        skills: resolve_packages_for_manifest(
            &manifest_value,
            PackageKind::Skill,
            &format!("registry agent {}@{}", package, version),
            packages,
        )?,
        reserved: reserved_refs_from_manifest(&manifest_value),
    })
}

fn build_registry_skill_root(
    package: &str,
    version: &str,
    packages: &BTreeMap<String, crate::semver::types::LockedPackage>,
    install_root: &Path,
) -> Result<LockRoot> {
    let (owner, name) = split_package_ref(package)?;
    let manifest_path = install_root
        .join(".agentpm")
        .join("skills")
        .join(owner)
        .join(name)
        .join(version)
        .join("agent.json");

    let (manifest_value, _) = load_manifest_value(&manifest_path).with_context(|| {
        format!(
            "loading installed skill manifest {}",
            manifest_path.display()
        )
    })?;

    Ok(LockRoot::RegistrySkill {
        package_key: package_key(PackageKind::Skill, package, version),
        tools: resolve_tools_for_manifest(
            &manifest_value,
            &format!("registry skill {}@{}", package, version),
            packages,
        )?,
    })
}

fn resolve_tools_for_manifest(
    manifest_value: &Value,
    manifest_label: &str,
    packages: &BTreeMap<String, crate::semver::types::LockedPackage>,
) -> Result<Vec<String>> {
    resolve_packages_for_manifest(manifest_value, PackageKind::Tool, manifest_label, packages)
}

fn resolve_packages_for_manifest(
    manifest_value: &Value,
    kind: PackageKind,
    manifest_label: &str,
    packages: &BTreeMap<String, crate::semver::types::LockedPackage>,
) -> Result<Vec<String>> {
    let desired = DesiredSet::from_cli_or_agent_json(manifest_value, None, false)
        .with_context(|| format!("reading kind for {}", manifest_label))?;
    let mut resolved = Vec::new();

    for item in desired.items {
        if item.kind != kind {
            continue;
        }
        let pkg = resolve_declared_package_from_packages(packages, &item.name, &item.range, kind)?
            .ok_or_else(|| {
                let dependency_label = kind.as_str();
                anyhow!(
                    "declared {} dependency {}@{} from {} is missing from the resolved package set",
                    dependency_label,
                    item.name,
                    item.range,
                    manifest_label
                )
            })?;
        resolved.push(package_key(pkg.kind, &pkg.name, &pkg.version));
    }

    Ok(resolved)
}

fn split_package_ref(package: &str) -> Result<(String, String)> {
    if !package.starts_with('@') {
        return Err(anyhow!("package must be of form @owner/name"));
    }
    let mut parts = package[1..].splitn(2, '/');
    let owner = parts.next().ok_or_else(|| anyhow!("invalid package"))?;
    let name = parts.next().ok_or_else(|| anyhow!("invalid package"))?;
    Ok((owner.to_string(), name.to_string()))
}

fn reserved_refs_from_manifest(manifest_value: &Value) -> ReservedReferences {
    ReservedReferences {
        skills: Vec::new(),
        knowledge: manifest_array_or_empty(manifest_value, "knowledge"),
        memory: manifest_array_or_empty(manifest_value, "memory"),
        profiles: manifest_array_or_empty(manifest_value, "profiles"),
    }
}

fn manifest_array_or_empty(manifest_value: &Value, field: &str) -> Vec<Value> {
    manifest_value
        .get(field)
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_root(label: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("agentpm-workspace-{label}-{nanos}"));
        fs::create_dir_all(&root).unwrap();
        root
    }

    #[test]
    fn rejects_invalid_workspace_paths() {
        let root = temp_root("invalid-path");
        let err = validate_workspace_metadata(
            &root,
            &WorkspaceMetadata {
                schema_version: 1,
                manifests: vec!["../agent.json".to_string()],
                package_roots: WorkspacePackageRoots::default(),
            },
        )
        .unwrap_err();

        assert!(
            format!("{err:#}").contains("must not contain .."),
            "{err:#}"
        );
    }

    #[test]
    fn rejects_duplicate_workspace_entries() {
        let root = temp_root("dupe");
        let err = validate_workspace_metadata(
            &root,
            &WorkspaceMetadata {
                schema_version: 1,
                manifests: vec!["agent.json".to_string(), "agent.json".to_string()],
                package_roots: WorkspacePackageRoots::default(),
            },
        )
        .unwrap_err();

        assert!(format!("{err:#}").contains("duplicated"), "{err:#}");
    }

    #[test]
    fn build_workspace_lock_records_multiple_local_manifests() {
        let root = temp_root("lock");
        let manifests = vec![
            LocalManifestRoot {
                rel_path: "agent.json".to_string(),
                manifest_value: json!({
                    "kind": "agent",
                    "name": "primary",
                    "version": "0.1.0",
                    "tools": [{"name":"@zack/echo","version":"0.1.0"}]
                }),
            },
            LocalManifestRoot {
                rel_path: "agents/triage.agent.json".to_string(),
                manifest_value: json!({
                    "kind": "agent",
                    "name": "triage",
                    "version": "0.1.0",
                    "tools": [{"name":"@zack/summarize","version":"0.2.0"}]
                }),
            },
        ];

        let lock = build_workspace_lock(
            &manifests,
            &WorkspacePackageRoots::default(),
            &ResolvePlan {
                items: vec![
                    crate::semver::types::ResolvedPackage {
                        kind: PackageKind::Tool,
                        name: "@zack/echo".to_string(),
                        version: "0.1.0".to_string(),
                        integrity: "sha256-echo".to_string(),
                    },
                    crate::semver::types::ResolvedPackage {
                        kind: PackageKind::Tool,
                        name: "@zack/summarize".to_string(),
                        version: "0.2.0".to_string(),
                        integrity: "sha256-summarize".to_string(),
                    },
                ],
            },
            &root,
        )
        .unwrap();

        let Lock::V2(lock) = lock else {
            panic!("expected v2 lock");
        };
        assert!(lock.roots.contains_key("local:agent"));
        assert!(
            lock.roots
                .contains_key("local:agent:agents/triage.agent.json")
        );
    }

    #[test]
    fn build_workspace_lock_rejects_missing_local_manifest_tool_from_plan() {
        let root = temp_root("missing-tool");
        let manifests = vec![LocalManifestRoot {
            rel_path: "agents/reviewer.agent.json".to_string(),
            manifest_value: json!({
                "kind": "agent",
                "name": "reviewer",
                "version": "0.1.0",
                "tools": [{"name":"@zack/summarize","version":"0.2.0"}]
            }),
        }];

        let err = build_workspace_lock(
            &manifests,
            &WorkspacePackageRoots::default(),
            &ResolvePlan {
                items: vec![crate::semver::types::ResolvedPackage {
                    kind: PackageKind::Tool,
                    name: "@zack/echo".to_string(),
                    version: "0.1.0".to_string(),
                    integrity: "sha256-echo".to_string(),
                }],
            },
            &root,
        )
        .unwrap_err();

        assert!(
            format!("{err:#}").contains("declared tool dependency @zack/summarize@0.2.0"),
            "{err:#}"
        );
    }

    #[test]
    fn desired_from_workspace_collects_local_and_registry_roots() {
        let desired = desired_from_workspace(
            &[LocalManifestRoot {
                rel_path: "agent.json".to_string(),
                manifest_value: json!({
                    "kind":"agent",
                    "name":"primary",
                    "version":"0.1.0",
                    "tools":[{"name":"@zack/echo","version":"0.1.0"}]
                }),
            }],
            &WorkspacePackageRoots {
                tools: vec![WorkspacePackageRoot {
                    name: "@zack/translate".to_string(),
                    version: "0.3.0".to_string(),
                }],
                agents: vec![WorkspacePackageRoot {
                    name: "@zack/support-agent".to_string(),
                    version: "0.1.0".to_string(),
                }],
                skills: vec![WorkspacePackageRoot {
                    name: "@zack/triage-skill".to_string(),
                    version: "0.2.0".to_string(),
                }],
            },
        )
        .unwrap();

        assert_eq!(desired.items.len(), 4);
        assert_eq!(desired.items[2].kind, PackageKind::Agent);
        assert_eq!(desired.items[3].kind, PackageKind::Skill);
    }

    #[test]
    fn build_workspace_lock_records_registry_skill_package_roots() {
        let root = temp_root("workspace-skill-root");
        let manifest_path = root
            .join(".agentpm")
            .join("skills")
            .join("zack")
            .join("triage-skill")
            .join("0.2.0")
            .join("agent.json");
        fs::create_dir_all(manifest_path.parent().unwrap()).unwrap();
        fs::write(
            &manifest_path,
            serde_json::to_string_pretty(&json!({
                "kind": "skill",
                "name": "triage-skill",
                "version": "0.2.0",
                "tools": [{"name":"@zack/echo","version":"0.1.0"}]
            }))
            .unwrap(),
        )
        .unwrap();

        let lock = build_workspace_lock(
            &[LocalManifestRoot {
                rel_path: "agent.json".to_string(),
                manifest_value: json!({
                    "kind":"agent",
                    "name":"primary",
                    "version":"0.1.0",
                    "tools":[{"name":"@zack/echo","version":"0.1.0"}],
                    "skills":[]
                }),
            }],
            &WorkspacePackageRoots {
                tools: Vec::new(),
                agents: Vec::new(),
                skills: vec![WorkspacePackageRoot {
                    name: "@zack/triage-skill".to_string(),
                    version: "0.2.0".to_string(),
                }],
            },
            &ResolvePlan {
                items: vec![
                    crate::semver::types::ResolvedPackage {
                        kind: PackageKind::Tool,
                        name: "@zack/echo".to_string(),
                        version: "0.1.0".to_string(),
                        integrity: "sha256-echo".to_string(),
                    },
                    crate::semver::types::ResolvedPackage {
                        kind: PackageKind::Skill,
                        name: "@zack/triage-skill".to_string(),
                        version: "0.2.0".to_string(),
                        integrity: "sha256-skill".to_string(),
                    },
                ],
            },
            &root,
        )
        .unwrap();

        let Lock::V2(lock) = lock else {
            panic!("expected modern lock");
        };
        assert_eq!(lock.lockfile_version, 3);
        assert!(lock.roots.contains_key("skill:@zack/triage-skill@0.2.0"));
    }
}
