//! Frozen replay uses pinned packages and verified manifests, never Registry resolution.
use super::*;
use crate::io::download::download_and_extract_with_client;
use crate::semver::types::{
    LockedPackage, PackageRequirement, ResolvedPackage, locked_version_matches,
};
use agentpm_sdk::models::install::InstallInitResponse;

fn dependencies(root: &LockedRoot) -> impl Iterator<Item = (PackageKind, &String)> {
    [
        (PackageKind::Tool, &root.tools),
        (PackageKind::Skill, &root.skills),
        (PackageKind::Knowledge, &root.knowledge),
        (PackageKind::Memory, &root.memory),
        (PackageKind::Profile, &root.profiles),
    ]
    .into_iter()
    .flat_map(|(kind, keys)| keys.iter().map(move |key| (kind, key)))
    .chain(root.r#loop.iter().map(|key| (PackageKind::Loop, key)))
}

fn select<'a>(
    requirement: &PackageRequirement,
    packages: &'a BTreeMap<String, LockedPackage>,
    root: Option<&LockedRoot>,
) -> Result<&'a LockedPackage> {
    let candidates = if let Some(root) = root {
        dependencies(root)
            .filter(|(kind, _)| *kind == requirement.kind)
            .filter_map(|(_, key)| packages.get(key))
            .collect::<Vec<_>>()
    } else {
        packages.values().collect()
    };
    let mut matches = Vec::new();
    for pkg in candidates {
        if pkg.name == requirement.name
            && pkg.kind == requirement.kind
            && locked_version_matches(&pkg.version, &requirement.range)?
        {
            matches.push(pkg);
        }
    }
    match matches.as_slice() {
        [pkg] => Ok(*pkg),
        [] => anyhow::bail!(
            "No locked {} {} satisfies {}; run agentpm install without --frozen to update the lock",
            requirement.kind.as_str(),
            requirement.name,
            requirement.range
        ),
        _ => anyhow::bail!(
            "Lock does not uniquely pin {} {}@{}; use an exact authored selector and regenerate agent.lock without --frozen",
            requirement.kind.as_str(),
            requirement.name,
            requirement.range
        ),
    }
}

fn validate_manifest(
    manifest: &Value,
    root: Option<&LockedRoot>,
    packages: &BTreeMap<String, LockedPackage>,
) -> Result<()> {
    if !matches!(manifest["kind"].as_str(), Some("agent" | "skill")) {
        return Ok(());
    }
    let desired = DesiredSet::from_cli_or_agent_json(manifest, None, false)?;
    let mut selected = BTreeSet::new();
    for requirement in &desired.items {
        if root.is_none() {
            // Older locks need not record a root for a transitive Skill. Check
            // its declarations against the pinned set without selecting a new
            // version: the entire exact set is being restored, including both
            // versions when different Agents need the same Component name.
            let mut satisfied = false;
            for pkg in packages
                .values()
                .filter(|p| p.kind == requirement.kind && p.name == requirement.name)
            {
                satisfied |= locked_version_matches(&pkg.version, &requirement.range)?;
            }
            anyhow::ensure!(
                satisfied,
                "No locked {} {} satisfies {}; run agentpm install without --frozen to update the lock",
                requirement.kind.as_str(),
                requirement.name,
                requirement.range
            );
            continue;
        }
        let pkg = select(requirement, packages, root)?;
        selected.insert(package_key(pkg.kind, &pkg.name, &pkg.version));
    }
    if let Some(root) = root {
        let pinned: BTreeSet<_> = dependencies(root).map(|(_, key)| key.clone()).collect();
        anyhow::ensure!(
            selected == pinned,
            "Locked dependency edges differ from the authored manifest; run agentpm install without --frozen to update the lock"
        );
    }
    Ok(())
}

pub(super) fn plan(
    desired: &DesiredSet,
    lock: &Lock,
    direct: bool,
    local: &[(&str, &Value)],
) -> Result<ResolvePlan> {
    let Lock::V2(value) = lock else {
        return ResolvePlan::from_lock(desired, lock);
    };
    // Validate key/identity and every stored edge before requesting URLs.
    for (key, pkg) in &value.packages {
        anyhow::ensure!(
            *key == package_key(pkg.kind, &pkg.name, &pkg.version),
            "Locked package key differs from its kind/name/version: {key}"
        );
        anyhow::ensure!(
            pkg.kind != PackageKind::Template,
            "`agentpm install` does not support template packages; use `agentpm new`"
        );
    }
    for (key, root) in &value.roots {
        if !key.starts_with("local:") {
            anyhow::ensure!(
                value.packages.contains_key(key),
                "Locked root {key} has no package entry"
            );
        }
        for (kind, child) in dependencies(root) {
            let pkg = value.packages.get(child)
                .ok_or_else(|| anyhow!("Missing locked dependency {child} from {key}; run agentpm install without --frozen"))?;
            anyhow::ensure!(
                pkg.kind == kind,
                "Wrong kind for locked dependency {child} in {key}"
            );
        }
    }
    for (key, manifest) in local {
        if let Some(root) = value.roots.get(*key) {
            anyhow::ensure!(
                root.name
                    .as_deref()
                    .is_none_or(|name| manifest["name"].as_str() == Some(name))
                    && root
                        .version
                        .as_deref()
                        .is_none_or(|version| manifest["version"].as_str() == Some(version)),
                "Local root identity differs from agent.lock at {key}; run agentpm install without --frozen to update the lock"
            );
        }
        validate_manifest(manifest, value.roots.get(*key), &value.packages)
            .with_context(|| format!("checking frozen dependencies for {key}"))?;
    }
    if direct {
        for requirement in &desired.items {
            let mut matches = Vec::new();
            for pkg in value
                .packages
                .values()
                .filter(|p| p.name == requirement.name)
            {
                if locked_version_matches(&pkg.version, &requirement.range)? {
                    matches.push(pkg);
                }
            }
            anyhow::ensure!(
                matches.len() == 1,
                "Direct package {}@{} must identify exactly one locked kind/version; run agentpm install without --frozen or use an exact selector",
                requirement.name,
                requirement.range
            );
        }
    } else {
        // Workspace package roots do not necessarily have a local manifest edge.
        for requirement in &desired.items {
            let covered = local.iter().any(|(key, _)| {
                value.roots.get(*key).is_some_and(|r| {
                    dependencies(r).any(|(_, k)| {
                        value.packages.get(k).is_some_and(|p| {
                            p.kind == requirement.kind
                                && p.name == requirement.name
                                && locked_version_matches(&p.version, &requirement.range)
                                    .unwrap_or(false)
                        })
                    })
                })
            });
            if !covered {
                select(requirement, &value.packages, None)?;
            }
        }
    }
    // Replay the whole portable locked set, including transitive Components and
    // other recorded roots. Root edges, where present, disambiguate versions.
    Ok(ResolvePlan {
        items: value
            .packages
            .values()
            .map(|pkg| ResolvedPackage {
                standard: pkg.standard.clone(),
                standard_origin: pkg.standard_origin,
                kind: pkg.kind,
                name: pkg.name.clone(),
                version: pkg.version.clone(),
                integrity: pkg.integrity.clone(),
                python_resolution: pkg.python_resolution.clone(),
            })
            .collect(),
    })
}

struct Inspection(PathBuf);
impl Drop for Inspection {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Existing digest/provenance/target checks run in an isolated extraction tree.
/// Graph defects must not replace any previously installed package.
pub(super) async fn verify_graph(
    init: &InstallInitResponse,
    plan: &ResolvePlan,
    lock: Option<&Lock>,
    cache: &Path,
    refresh: bool,
    client: &AgentPmClient,
) -> Result<()> {
    let root =
        std::env::temp_dir().join(format!("agentpm-install-{:032x}", rand::random::<u128>()));
    let builder = std::fs::DirBuilder::new();
    #[cfg(unix)]
    let builder = {
        use std::os::unix::fs::DirBuilderExt;
        let mut builder = builder;
        builder.mode(0o700);
        builder
    };
    builder.create(&root)?;
    let stage = Inspection(root);
    let tools = stage.0.join("tools");
    let agents = stage.0.join("agents");
    let skills = stage.0.join("skills");
    let knowledge = stage.0.join("knowledge");
    let memory = stage.0.join("memory");
    let profiles = stage.0.join("profiles");
    let loops = stage.0.join("loops");
    download_and_extract_with_client(
        init,
        cache,
        InstallRoots {
            tools_dir: &tools,
            agents_dir: &agents,
            skills_dir: &skills,
            knowledge_dir: &knowledge,
            memory_dir: &memory,
            profiles_dir: &profiles,
            loops_dir: &loops,
        },
        refresh,
        true,
        Some(client),
    )
    .await?;
    let packages = super::plan_to_packages(plan);
    for item in &plan.items {
        if !matches!(item.kind, PackageKind::Agent | PackageKind::Skill) {
            continue;
        }
        let directory = crate::io::download::resolved_package_dir(
            item.kind,
            &tools,
            &agents,
            &skills,
            &knowledge,
            &memory,
            &profiles,
            &loops,
            &item.name,
            &item.version,
        )?;
        let manifest: Value =
            serde_json::from_slice(&std::fs::read(directory.join("agent.json"))?)?;
        let key = package_key(item.kind, &item.name, &item.version);
        let pinned = match lock {
            Some(Lock::V2(value)) => value.roots.get(&key),
            _ => None,
        };
        if lock.is_some() {
            if let Some(root) = pinned {
                agentpm_sdk::apds::reconcile(
                    &manifest,
                    root.standard.as_ref(),
                    root.standard_origin,
                )?;
            }
            validate_manifest(&manifest, pinned, &packages)
                .with_context(|| format!("checking frozen dependencies for {key}"))?;
        } else {
            // Ordinary resolution can select a highest satisfying version;
            // frozen replay restores only the already locked package set.
            super::build_local_lock_root(&manifest, plan)
                .with_context(|| format!("checking resolved dependencies for {key}"))?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn selectors_enforce_exact_identity_ranges_and_prerelease_bounds() {
        for (version, selector, expected) in [
            ("1.2.3", "1.2.3", true),
            ("1.2.4", "1.2.3", false),
            ("1.2.3", "^1.2", true),
            ("2.0.0", "^1.2", false),
            ("1.2.9", "~1.2", true),
            ("1.3.0", "~1.2", false),
            ("1.2.3", ">=1.0.0, <2.0.0", true),
            ("1.2.3", ">=1.0.0 <2.0.0", true),
            ("1.2.3", "^1.2.0+build.1", true),
            ("1.2.0", "1.2", true),
            ("1.2.3", "1.2", false),
            ("1.2.3-alpha.1", "*", false),
            ("1.2.3-alpha.1", "1.2.3-alpha.1", true),
            ("1.2.3+one", "1.2.3+two", false),
            ("1.2.3", "latest", true),
        ] {
            assert_eq!(
                locked_version_matches(version, selector).unwrap(),
                expected,
                "{version} against {selector}"
            );
        }
        assert!(locked_version_matches("1.2.3", "not-semver").is_err());
        assert!(locked_version_matches("bad-version", "*").is_err());
    }

    #[test]
    fn transitive_ranges_check_locked_set_without_inventing_new_pins() {
        let packages = BTreeMap::from_iter(["1.0.0", "1.5.0"].map(|version| {
            let pkg = LockedPackage {
                standard: None,
                standard_origin: None,
                kind: PackageKind::Tool,
                name: "@fixture/echo".into(),
                version: version.into(),
                integrity: "a".repeat(64),
                python_resolution: None,
            };
            (package_key(pkg.kind, &pkg.name, &pkg.version), pkg)
        }));
        let manifest = json!({"kind":"skill","tools":["@fixture/echo@^1"]});
        validate_manifest(&manifest, None, &packages).unwrap();
        let root = LockedRoot {
            tools: vec!["tool:@fixture/echo@1.0.0".into()],
            ..Default::default()
        };
        validate_manifest(&manifest, Some(&root), &packages).unwrap();
        let outside = json!({"kind":"skill","tools":["@fixture/echo@^2"]});
        assert!(validate_manifest(&outside, None, &packages).is_err());
        // A recorded exact edge must not be replaced by the other satisfying pin.
        let mut changed = manifest.clone();
        changed["tools"] = json!(["@fixture/echo@1.5.0"]);
        assert!(validate_manifest(&changed, Some(&root), &packages).is_err());
    }
}
