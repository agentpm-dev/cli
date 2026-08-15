use crate::manifest::{
    load_manifest_value, read_lock_or_default, write_lock, write_manifest_pretty_atomic,
};
use crate::prelude::*;
use crate::semver::adapt::{plan_to_sdk_resolve, to_sdk_request};
use crate::semver::types::{
    DesiredSet, Lock, LockRoot, LockedRoot, PackageKind, ReservedReferences, ResolvePlan,
    lock_from_packages_and_roots, lock_from_plan, package_key, parse_package_spec,
    resolve_declared_package_from_packages, split_package_ref,
};
use crate::semver::update::{
    maybe_update_agent_json, maybe_update_manifest_dependency,
    maybe_update_manifest_singular_dependency,
};
use crate::workspace::{
    build_workspace_lock, desired_from_workspace, load_workspace_local_manifests,
    read_workspace_metadata,
};
use crate::{
    io::download::{InstallRoots, download_and_extract_all},
    io::fs,
    ui::Step,
};
use anyhow::anyhow;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::io::IsTerminal;
use std::path::{Path, PathBuf};

#[derive(clap::Parser, Debug, Clone)]
pub struct InstallArgs {
    // Path to the manifest (agent.json)
    #[arg(long, default_value = "agent.json")]
    pub manifest: String,

    /// Optional spec like @owner/name@^1.2
    pub spec: Option<String>,

    /// Fail if lockfile would change (no re-resolution)
    #[clap(long)]
    pub frozen: bool,

    /// Re-resolve and re-download even if the lock looks satisfied
    #[clap(long)]
    pub refresh: bool,

    /// Update range in agent.json if spec conflicts
    #[clap(long)]
    pub update_range: bool,

    /// Reduce output
    #[clap(long)]
    pub quiet: bool,

    /// Fail install if the registry attestation is missing for any artifact
    #[clap(long)]
    pub require_attestation: bool,

    /// Personal Access Token for headless auth (overrides env/file)
    #[arg(long, value_name = "PAT", env = "AGENTPM_TOKEN")]
    pub token: Option<String>,
}

impl InstallArgs {
    pub async fn run(&self, base_url: String) -> Result<()> {
        let cfg = Config::load(base_url.clone())?;

        // Quiet if the user asked OR stderr is not a terminal (piped/redirected)
        let auto_quiet = !std::io::stderr().is_terminal();
        let quiet = self.quiet || auto_quiet;

        // 0) Load PAT (login is not required, but send in incase private repo; for MVP we use cached token)
        let mut s = Step::new("Reading credentials", quiet);
        let token = resolve_token(&cfg, self.token.clone())?;
        if let Some(t) = &token {
            s.ok(format!("using {}", mask_token(t)));
        } else {
            s.ok("none (public installs only)");
        }

        let mut client = AgentPmClient::new(base_url.clone())?;
        if let Some(ref tok) = token {
            client = client.with_token(tok.clone());
        }

        // 1) Load local manifest when present. Manifest-driven installs support local agents and skills.
        let manifest_path = PathBuf::from(&self.manifest);
        let project_root = manifest_path
            .parent()
            .filter(|path| !path.as_os_str().is_empty())
            .unwrap_or(Path::new("."))
            .to_path_buf();
        let workspace_metadata = read_workspace_metadata(&project_root)?;
        if workspace_metadata.is_some() && self.spec.is_some() {
            return Err(anyhow!(
                "`agentpm install <spec>` is not supported for workspace projects; update the workspace manifests or agentpm.workspace.json and run `agentpm install`"
            ));
        }

        let mut manifest_value =
            if should_load_manifest_for_install(self.spec.as_deref(), &manifest_path) {
                Some(
                    load_manifest_value(&manifest_path)
                        .with_context(|| format!("loading {}", manifest_path.display()))?
                        .0,
                )
            } else {
                None
            };
        let workspace_manifests = if self.spec.is_none() {
            if let Some(metadata) = workspace_metadata.as_ref() {
                Some(load_workspace_local_manifests(&project_root, metadata)?)
            } else {
                None
            }
        } else {
            None
        };

        // 2) Determine desired set
        let desired = if let Some(spec) = self.spec.as_deref() {
            DesiredSet::from_cli_or_agent_json(&Value::Null, Some(spec), self.update_range)?
        } else if let Some(metadata) = workspace_metadata.as_ref() {
            let manifests = workspace_manifests
                .as_ref()
                .ok_or_else(|| anyhow!("workspace manifests failed to load"))?;
            desired_from_workspace(manifests, &metadata.package_roots)?
        } else {
            let manifest_value = manifest_value
                .as_ref()
                .ok_or_else(|| anyhow!("agent.json is required for manifest-driven install"))?;

            let kind = manifest_value.get("kind").and_then(Value::as_str);
            if kind != Some("agent") && kind != Some("skill") {
                return Err(anyhow!(
                    "`agentpm install` currently supports local kind=\"agent\" and kind=\"skill\" manifests only."
                ));
            }

            DesiredSet::from_cli_or_agent_json(manifest_value, None, self.update_range)?
        };

        // 3) If --frozen and lock exists, use it directly; else resolve
        let lock = read_lock_or_default(&project_root)?;
        let steps_label = if self.frozen {
            "Validating lockfile"
        } else {
            "Resolving versions"
        };
        let mut s = Step::new(steps_label, quiet);

        if self.frozen {
            validate_frozen_lock_compatibility(&lock, manifest_value.as_ref(), &desired)?;
        }

        let plan = if self.frozen {
            ResolvePlan::from_lock(&desired, &lock)?
        } else {
            let sdk_req = to_sdk_request(&desired);
            let sdk_resp = client.resolve_install(&sdk_req).await?; // network call → concrete versions + integrity
            ensure_supported_install_kinds(&sdk_resp)?;
            let plan: ResolvePlan = sdk_resp.into();
            plan
        };
        s.ok("");

        let precomputed_next_lock =
            if !self.frozen && self.spec.is_none() && workspace_metadata.is_none() {
                Some(build_updated_lock(
                    &lock,
                    manifest_value.as_ref(),
                    self.spec.as_deref(),
                    &plan,
                    project_root.join(".agentpm").as_path(),
                )?)
            } else {
                None
            };

        // 4) Init download session → presigned URLs
        let mut s = Step::new("Requesting download URLs", quiet);
        let init = client.install_init(&plan_to_sdk_resolve(&plan)).await?; // includes per-artifact presigned URL + expected hash

        if self.require_attestation {
            let mut missing: Vec<String> = Vec::new();

            for a in &init.artifacts {
                let att_ok = a
                    .signing
                    .as_ref()
                    .map(|s| s.registry_attested)
                    .unwrap_or(false); // treat absence as not attested

                if !att_ok {
                    // name is already "@ns/tool"
                    missing.push(format!("{}@{}", a.name, a.version));
                }
            }

            if !missing.is_empty() {
                s.err("failed");
                let list = missing.join(", ");
                return Err(anyhow::anyhow!(
                    "Registry attestation required but missing for: {list}"
                ));
            }
        }

        s.ok("");

        // 5) Download + verify + extract (parallel)
        let mut s = Step::new("Downloading packages", quiet);
        let dl_dir = project_root.join(".agentpm/cache");
        let tools_dir = project_root.join(".agentpm/tools");
        let agents_dir = project_root.join(".agentpm/agents");
        let skills_dir = project_root.join(".agentpm/skills");
        let knowledge_dir = project_root.join(".agentpm/knowledge");
        let memory_dir = project_root.join(".agentpm/memory");
        let profiles_dir = project_root.join(".agentpm/profiles");
        let loops_dir = project_root.join(".agentpm/loops");
        fs::ensure_dirs(&[
            &dl_dir,
            &tools_dir,
            &agents_dir,
            &skills_dir,
            &knowledge_dir,
            &memory_dir,
            &profiles_dir,
            &loops_dir,
        ])?;
        download_and_extract_all(
            &init,
            &dl_dir,
            InstallRoots {
                tools_dir: &tools_dir,
                agents_dir: &agents_dir,
                skills_dir: &skills_dir,
                knowledge_dir: &knowledge_dir,
                memory_dir: &memory_dir,
                profiles_dir: &profiles_dir,
                loops_dir: &loops_dir,
            },
            self.refresh,
            quiet,
        )
        .await?;
        s.ok("");

        // 6) Finalize (report success for metrics / server-side bookkeeping)
        let mut s = Step::new("Finalizing install", quiet);
        client.install_finalize(&init.session_id).await?;
        s.ok("");

        // 7) If a direct tool spec was installed inside a local agent project and
        // it is not present in agent.json yet, append or update the declared range.
        // Direct skill installs follow the same pattern, but write into the
        // top-level `skills` field instead of `tools`.
        // Direct agent package installs intentionally do not mutate the local
        // manifest; they are represented as registry roots in the lockfile.
        let mut should_write_manifest = false;
        if let Some(spec) = &self.spec
            && let Some(manifest_value) = manifest_value.as_mut()
            && manifest_value.get("kind").and_then(Value::as_str) == Some("agent")
            && let Some(kind) = direct_declared_dependency_kind(spec, &plan)?
            && match kind {
                PackageKind::Tool => {
                    maybe_update_agent_json(manifest_value, spec, self.update_range)?
                }
                PackageKind::Skill => maybe_update_manifest_dependency(
                    manifest_value,
                    "skills",
                    "Skill",
                    spec,
                    self.update_range,
                )?,
                PackageKind::Knowledge => maybe_update_manifest_dependency(
                    manifest_value,
                    "knowledge",
                    "Knowledge",
                    spec,
                    self.update_range,
                )?,
                PackageKind::Memory => maybe_update_manifest_dependency(
                    manifest_value,
                    "memory",
                    "Memory",
                    spec,
                    self.update_range,
                )?,
                PackageKind::Profile => maybe_update_manifest_dependency(
                    manifest_value,
                    "profiles",
                    "Profile",
                    spec,
                    self.update_range,
                )?,
                PackageKind::Loop => maybe_update_manifest_singular_dependency(
                    manifest_value,
                    "loop",
                    "Loop",
                    spec,
                    self.update_range,
                )?,
                PackageKind::Agent => false,
            }
        {
            should_write_manifest = true;
        }

        // 8) Update lockfile (unless --frozen)
        if !self.frozen {
            let next_lock = if let Some(next_lock) = precomputed_next_lock {
                next_lock
            } else if let Some(metadata) = workspace_metadata.as_ref() {
                let manifests = workspace_manifests
                    .as_ref()
                    .ok_or_else(|| anyhow!("workspace manifests failed to load"))?;
                build_workspace_lock(manifests, &metadata.package_roots, &plan, &project_root)?
            } else {
                build_updated_lock(
                    &lock,
                    manifest_value.as_ref(),
                    self.spec.as_deref(),
                    &plan,
                    project_root.join(".agentpm").as_path(),
                )?
            };
            write_lock(&project_root, &next_lock)?;
        }

        if should_write_manifest && let Some(manifest_value) = manifest_value.as_ref() {
            write_manifest_pretty_atomic(&manifest_path, manifest_value)?;
        }

        Step::final_msg("Installed ✓", quiet);

        Ok(())
    }
}

fn should_load_manifest_for_install(spec: Option<&str>, manifest_path: &std::path::Path) -> bool {
    spec.is_none() || manifest_path.exists()
}

fn direct_declared_dependency_kind(spec: &str, plan: &ResolvePlan) -> Result<Option<PackageKind>> {
    let requested = parse_package_spec(spec)?;
    Ok(plan
        .items
        .iter()
        .find(|item| item.name == requested.name)
        .map(|item| item.kind))
}

fn ensure_supported_install_kinds(
    resp: &agentpm_sdk::models::install::ResolveResponse,
) -> Result<()> {
    if resp
        .items
        .iter()
        .any(|item| item.kind == agentpm_sdk::models::install::PackageKind::Template)
    {
        return Err(anyhow!(
            "`agentpm install` does not support template packages; use `agentpm new` to generate a project from a template"
        ));
    }

    Ok(())
}

fn validate_frozen_lock_compatibility(
    lock: &Lock,
    manifest_value: Option<&Value>,
    desired: &DesiredSet,
) -> Result<()> {
    let manifest_is_agent = manifest_value
        .and_then(|value| value.get("kind"))
        .and_then(Value::as_str)
        == Some("agent");
    let manifest_is_skill = manifest_value
        .and_then(|value| value.get("kind"))
        .and_then(Value::as_str)
        == Some("skill");

    let desired_has_agent = desired
        .items
        .iter()
        .any(|item| item.kind == PackageKind::Agent);
    let desired_has_skill = desired
        .items
        .iter()
        .any(|item| item.kind == PackageKind::Skill);
    let desired_has_knowledge = desired
        .items
        .iter()
        .any(|item| item.kind == PackageKind::Knowledge);
    let desired_has_memory = desired
        .items
        .iter()
        .any(|item| item.kind == PackageKind::Memory);
    let desired_has_profile = desired
        .items
        .iter()
        .any(|item| item.kind == PackageKind::Profile);
    let desired_has_loop = desired
        .items
        .iter()
        .any(|item| item.kind == PackageKind::Loop);
    let manifest_has_knowledge = manifest_value
        .and_then(|value| value.get("knowledge"))
        .and_then(Value::as_array)
        .is_some_and(|items| !items.is_empty());
    let manifest_has_memory = manifest_value
        .and_then(|value| value.get("memory"))
        .and_then(Value::as_array)
        .is_some_and(|items| !items.is_empty());
    let manifest_has_profile = manifest_value
        .and_then(|value| value.get("profiles"))
        .and_then(Value::as_array)
        .is_some_and(|items| !items.is_empty());
    let manifest_has_loop = manifest_value
        .and_then(|value| value.get("loop"))
        .is_some_and(|value| !value.is_null());

    let requires_v3 = manifest_is_skill
        || desired_has_skill
        || (manifest_is_agent
            && (manifest_has_knowledge
                || desired_has_knowledge
                || manifest_has_memory
                || desired_has_memory
                || manifest_has_profile
                || desired_has_profile
                || manifest_has_loop
                || desired_has_loop));
    if (manifest_is_agent || desired_has_agent) && lock.is_v1() {
        return Err(anyhow!(
            "--frozen cannot use lockfile v1 for agent dependency graphs; run `agentpm install` without --frozen to regenerate agent.lock v2"
        ));
    }
    if requires_v3 && lock.lockfile_version() < 3 {
        return Err(anyhow!(
            "--frozen cannot use lockfile v{} for Skill, Knowledge, Memory, Profile, or Loop dependency graphs; run `agentpm install` without --frozen to regenerate agent.lock v3",
            lock.lockfile_version()
        ));
    }

    Ok(())
}

fn build_updated_lock(
    existing_lock: &Lock,
    manifest_value: Option<&Value>,
    spec: Option<&str>,
    plan: &ResolvePlan,
    install_root: &std::path::Path,
) -> Result<Lock> {
    if spec.is_none() {
        let roots = build_lock_roots(manifest_value, None, plan, install_root)?;
        return Ok(lock_from_plan(plan, &roots));
    }

    let mut packages = match existing_lock {
        Lock::V2(lock) => lock.packages.clone(),
        Lock::V1(_) => BTreeMap::new(),
    };
    let mut roots = match existing_lock {
        Lock::V2(lock) => lock.roots.clone(),
        Lock::V1(_) => BTreeMap::new(),
    };
    let mut removed_local_dependency_keys = Vec::new();

    let new_registry_roots = build_registry_package_roots(spec, plan, install_root)?;
    let replacing_direct_registry_root = direct_registry_root_identity(&new_registry_roots);
    if let Some((kind, package_name)) = replacing_direct_registry_root.as_ref() {
        remove_superseded_direct_registry_roots(&mut roots, &packages, *kind, package_name);
    }

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

    if let Some(manifest_value) = manifest_value
        && !plan.items.iter().any(|item| {
            manifest_value.get("kind").and_then(Value::as_str) == Some(item.kind.as_str())
        })
    {
        let (root_key, local_root) = build_local_root_from_manifest(manifest_value, &packages)?;
        if let Some(previous_root) = roots.get(&root_key) {
            removed_local_dependency_keys =
                removed_root_dependency_keys(previous_root, &local_root)
                    .into_iter()
                    .collect();
        }
        roots.insert(root_key, local_root);
    }

    for root in new_registry_roots {
        let (root_key, locked_root) = locked_root_from_root(root)?;
        roots.insert(root_key, locked_root);
    }

    if replacing_direct_registry_root.is_some() {
        packages = prune_unreachable_packages(packages, &roots, install_root)?;
    } else if !removed_local_dependency_keys.is_empty() {
        prune_removed_unreachable_packages(
            &mut packages,
            &roots,
            install_root,
            &removed_local_dependency_keys,
        )?;
    }

    Ok(lock_from_packages_and_roots(packages, roots))
}

fn removed_root_dependency_keys(previous: &LockedRoot, next: &LockedRoot) -> BTreeSet<String> {
    root_dependency_keys(previous)
        .difference(&root_dependency_keys(next))
        .cloned()
        .collect()
}

fn root_dependency_keys(root: &LockedRoot) -> BTreeSet<String> {
    let mut keys = BTreeSet::new();
    keys.extend(root.tools.iter().cloned());
    keys.extend(root.skills.iter().cloned());
    keys.extend(root.knowledge.iter().cloned());
    keys.extend(root.memory.iter().cloned());
    keys.extend(root.profiles.iter().cloned());
    if let Some(loop_key) = &root.r#loop {
        keys.insert(loop_key.clone());
    }
    keys
}

fn direct_registry_root_identity(new_registry_roots: &[LockRoot]) -> Option<(PackageKind, String)> {
    new_registry_roots.iter().find_map(|root| match root {
        LockRoot::RegistryAgent { package_key, .. } => {
            parse_package_key_name(package_key, "agent:").map(|name| (PackageKind::Agent, name))
        }
        LockRoot::RegistrySkill { package_key, .. } => {
            parse_package_key_name(package_key, "skill:").map(|name| (PackageKind::Skill, name))
        }
        LockRoot::LocalAgent { .. } | LockRoot::LocalSkill { .. } => None,
    })
}

fn parse_package_key_name(package_key: &str, prefix: &str) -> Option<String> {
    package_key
        .strip_prefix(prefix)
        .and_then(|rest| rest.rsplit_once('@'))
        .map(|(name, _)| name.to_string())
}

fn remove_superseded_direct_registry_roots(
    roots: &mut BTreeMap<String, LockedRoot>,
    packages: &BTreeMap<String, crate::semver::types::LockedPackage>,
    kind: PackageKind,
    package_name: &str,
) {
    roots.retain(|key, _| {
        let expected_prefix = match kind {
            PackageKind::Agent => "agent:",
            PackageKind::Skill => "skill:",
            PackageKind::Knowledge
            | PackageKind::Memory
            | PackageKind::Profile
            | PackageKind::Loop
            | PackageKind::Tool => return true,
        };
        if !key.starts_with(expected_prefix) {
            return true;
        }
        packages
            .get(key)
            .is_some_and(|pkg| pkg.kind != kind || pkg.name != package_name)
    });
}

fn prune_unreachable_packages(
    packages: BTreeMap<String, crate::semver::types::LockedPackage>,
    roots: &BTreeMap<String, LockedRoot>,
    install_root: &Path,
) -> Result<BTreeMap<String, crate::semver::types::LockedPackage>> {
    let reachable = collect_reachable_packages(&packages, roots, install_root)?;
    Ok(packages
        .into_iter()
        .filter(|(key, _)| reachable.contains(key))
        .collect())
}

fn prune_removed_unreachable_packages(
    packages: &mut BTreeMap<String, crate::semver::types::LockedPackage>,
    roots: &BTreeMap<String, LockedRoot>,
    install_root: &Path,
    candidate_keys: &[String],
) -> Result<()> {
    let reachable = collect_reachable_packages(packages, roots, install_root)?;
    for key in candidate_keys {
        if !reachable.contains(key) {
            packages.remove(key);
        }
    }
    Ok(())
}

fn collect_reachable_packages(
    packages: &BTreeMap<String, crate::semver::types::LockedPackage>,
    roots: &BTreeMap<String, LockedRoot>,
    install_root: &Path,
) -> Result<BTreeSet<String>> {
    let mut reachable = BTreeSet::new();
    let mut queue = VecDeque::new();

    for (key, root) in roots {
        if key.starts_with("agent:") || key.starts_with("skill:") {
            queue.push_back(key.clone());
        }
        enqueue_root_dependencies(root, &mut queue);
    }

    while let Some(package_key) = queue.pop_front() {
        if !reachable.insert(package_key.clone()) {
            continue;
        }
        let Some(pkg) = packages.get(&package_key) else {
            continue;
        };
        enqueue_manifest_dependencies(pkg, packages, install_root, &mut queue)?;
    }

    Ok(reachable)
}

fn enqueue_root_dependencies(root: &LockedRoot, queue: &mut VecDeque<String>) {
    queue.extend(root.tools.iter().cloned());
    queue.extend(root.skills.iter().cloned());
    queue.extend(root.knowledge.iter().cloned());
    queue.extend(root.memory.iter().cloned());
    queue.extend(root.profiles.iter().cloned());
    if let Some(loop_key) = &root.r#loop {
        queue.push_back(loop_key.clone());
    }
}

fn enqueue_manifest_dependencies(
    pkg: &crate::semver::types::LockedPackage,
    packages: &BTreeMap<String, crate::semver::types::LockedPackage>,
    install_root: &Path,
    queue: &mut VecDeque<String>,
) -> Result<()> {
    match pkg.kind {
        PackageKind::Agent => {
            if let Some(manifest_value) =
                load_installed_manifest_value(pkg.kind, &pkg.name, &pkg.version, install_root)?
            {
                queue.extend(resolve_declared_packages_from_manifest(
                    &manifest_value,
                    packages,
                    &format!("registry agent {}@{}", pkg.name, pkg.version),
                    PackageKind::Tool,
                )?);
                queue.extend(resolve_declared_packages_from_manifest(
                    &manifest_value,
                    packages,
                    &format!("registry agent {}@{}", pkg.name, pkg.version),
                    PackageKind::Skill,
                )?);
                queue.extend(resolve_declared_packages_from_manifest(
                    &manifest_value,
                    packages,
                    &format!("registry agent {}@{}", pkg.name, pkg.version),
                    PackageKind::Knowledge,
                )?);
                queue.extend(resolve_declared_packages_from_manifest(
                    &manifest_value,
                    packages,
                    &format!("registry agent {}@{}", pkg.name, pkg.version),
                    PackageKind::Memory,
                )?);
                queue.extend(resolve_declared_packages_from_manifest(
                    &manifest_value,
                    packages,
                    &format!("registry agent {}@{}", pkg.name, pkg.version),
                    PackageKind::Profile,
                )?);
                if let Some(loop_key) = resolve_declared_loop_from_manifest(
                    &manifest_value,
                    packages,
                    &format!("registry agent {}@{}", pkg.name, pkg.version),
                )? {
                    queue.push_back(loop_key);
                }
            }
        }
        PackageKind::Skill => {
            if let Some(manifest_value) =
                load_installed_manifest_value(pkg.kind, &pkg.name, &pkg.version, install_root)?
            {
                queue.extend(resolve_declared_packages_from_manifest(
                    &manifest_value,
                    packages,
                    &format!("registry skill {}@{}", pkg.name, pkg.version),
                    PackageKind::Tool,
                )?);
            }
        }
        PackageKind::Knowledge
        | PackageKind::Memory
        | PackageKind::Profile
        | PackageKind::Loop
        | PackageKind::Tool => {}
    }
    Ok(())
}

fn load_installed_manifest_value(
    kind: PackageKind,
    name: &str,
    version: &str,
    install_root: &Path,
) -> Result<Option<Value>> {
    let manifest_path = match kind {
        PackageKind::Agent => installed_agent_manifest_path(install_root, name, version)?,
        PackageKind::Skill => installed_skill_manifest_path(install_root, name, version)?,
        PackageKind::Knowledge
        | PackageKind::Memory
        | PackageKind::Profile
        | PackageKind::Loop
        | PackageKind::Tool => return Ok(None),
    };
    if !manifest_path.exists() {
        return Err(anyhow!(
            "installed {} manifest for {}@{} is missing at {}; cannot safely compact lockfile packages",
            kind.as_str(),
            name,
            version,
            manifest_path.display()
        ));
    }
    let (manifest_value, _) = load_manifest_value(&manifest_path)?;
    Ok(Some(manifest_value))
}

fn build_lock_roots(
    manifest_value: Option<&Value>,
    spec: Option<&str>,
    plan: &ResolvePlan,
    install_root: &std::path::Path,
) -> Result<Vec<LockRoot>> {
    if let Some(manifest_value) = manifest_value
        && spec.is_none()
    {
        let key = if manifest_value.get("kind").and_then(Value::as_str) == Some("skill") {
            "local:skill".to_string()
        } else {
            "local:agent".to_string()
        };
        let root = build_local_lock_root(manifest_value, plan)?;
        return Ok(vec![
            match manifest_value.get("kind").and_then(Value::as_str) {
                Some("skill") => LockRoot::LocalSkill {
                    key,
                    name: root.name.unwrap_or_else(|| "local-skill".to_string()),
                    version: root.version.unwrap_or_else(|| "0.0.0".to_string()),
                    tools: root.tools,
                },
                _ => LockRoot::LocalAgent {
                    key,
                    name: root.name.unwrap_or_else(|| "local-agent".to_string()),
                    version: root.version.unwrap_or_else(|| "0.0.0".to_string()),
                    tools: root.tools,
                    skills: root.skills,
                    knowledge: root.knowledge,
                    memory: root.memory,
                    profiles: root.profiles,
                    r#loop: root.r#loop,
                    reserved: root.reserved,
                },
            },
        ]);
    }

    build_registry_package_roots(spec, plan, install_root)
}

fn installed_agent_manifest_path(
    install_root: &std::path::Path,
    package: &str,
    version: &str,
) -> Result<PathBuf> {
    let (owner, name) = split_package_ref(package)?;
    Ok(install_root
        .join("agents")
        .join(owner)
        .join(name)
        .join(version)
        .join("agent.json"))
}

fn installed_skill_manifest_path(
    install_root: &std::path::Path,
    package: &str,
    version: &str,
) -> Result<PathBuf> {
    let (owner, name) = split_package_ref(package)?;
    Ok(install_root
        .join("skills")
        .join(owner)
        .join(name)
        .join(version)
        .join("agent.json"))
}

#[cfg(test)]
fn installed_knowledge_manifest_path(
    install_root: &std::path::Path,
    package: &str,
    version: &str,
) -> Result<PathBuf> {
    let (owner, name) = split_package_ref(package)?;
    Ok(install_root
        .join("knowledge")
        .join(owner)
        .join(name)
        .join(version)
        .join("agent.json"))
}

#[cfg(test)]
fn installed_memory_manifest_path(
    install_root: &std::path::Path,
    package: &str,
    version: &str,
) -> Result<PathBuf> {
    let (owner, name) = split_package_ref(package)?;
    Ok(install_root
        .join("memory")
        .join(owner)
        .join(name)
        .join(version)
        .join("agent.json"))
}

#[cfg(test)]
fn installed_profile_manifest_path(
    install_root: &std::path::Path,
    package: &str,
    version: &str,
) -> Result<PathBuf> {
    let (owner, name) = split_package_ref(package)?;
    Ok(install_root
        .join("profiles")
        .join(owner)
        .join(name)
        .join(version)
        .join("agent.json"))
}

#[cfg(test)]
fn installed_loop_manifest_path(
    install_root: &std::path::Path,
    package: &str,
    version: &str,
) -> Result<PathBuf> {
    let (owner, name) = split_package_ref(package)?;
    Ok(install_root
        .join("loops")
        .join(owner)
        .join(name)
        .join(version)
        .join("agent.json"))
}

fn build_registry_package_roots(
    spec: Option<&str>,
    plan: &ResolvePlan,
    install_root: &std::path::Path,
) -> Result<Vec<LockRoot>> {
    let Some(spec) = spec else {
        return Ok(Vec::new());
    };
    let requested = parse_package_spec(spec)?;

    let mut roots = Vec::new();
    let packages = plan_to_packages(plan);
    for item in plan.items.iter().filter(|item| {
        (item.kind == PackageKind::Agent || item.kind == PackageKind::Skill)
            && item.name == requested.name
    }) {
        let root = match item.kind {
            PackageKind::Agent => {
                let manifest_path =
                    installed_agent_manifest_path(install_root, &item.name, &item.version)?;
                if manifest_path.exists() {
                    let (manifest_value, _) = load_manifest_value(&manifest_path)?;
                    LockRoot::RegistryAgent {
                        package_key: package_key(item.kind, &item.name, &item.version),
                        tools: resolve_declared_packages_from_manifest(
                            &manifest_value,
                            &packages,
                            &format!("registry agent {}@{}", item.name, item.version),
                            PackageKind::Tool,
                        )?,
                        skills: resolve_declared_packages_from_manifest(
                            &manifest_value,
                            &packages,
                            &format!("registry agent {}@{}", item.name, item.version),
                            PackageKind::Skill,
                        )?,
                        knowledge: resolve_declared_packages_from_manifest(
                            &manifest_value,
                            &packages,
                            &format!("registry agent {}@{}", item.name, item.version),
                            PackageKind::Knowledge,
                        )?,
                        memory: resolve_declared_packages_from_manifest(
                            &manifest_value,
                            &packages,
                            &format!("registry agent {}@{}", item.name, item.version),
                            PackageKind::Memory,
                        )?,
                        profiles: resolve_declared_packages_from_manifest(
                            &manifest_value,
                            &packages,
                            &format!("registry agent {}@{}", item.name, item.version),
                            PackageKind::Profile,
                        )?,
                        r#loop: resolve_declared_loop_from_manifest(
                            &manifest_value,
                            &packages,
                            &format!("registry agent {}@{}", item.name, item.version),
                        )?,
                        reserved: reserved_refs_from_manifest(),
                    }
                } else {
                    LockRoot::RegistryAgent {
                        package_key: package_key(item.kind, &item.name, &item.version),
                        tools: Vec::new(),
                        skills: Vec::new(),
                        knowledge: Vec::new(),
                        memory: Vec::new(),
                        profiles: Vec::new(),
                        r#loop: None,
                        reserved: ReservedReferences::default(),
                    }
                }
            }
            PackageKind::Skill => {
                let manifest_path =
                    installed_skill_manifest_path(install_root, &item.name, &item.version)?;
                if manifest_path.exists() {
                    let (manifest_value, _) = load_manifest_value(&manifest_path)?;
                    LockRoot::RegistrySkill {
                        package_key: package_key(item.kind, &item.name, &item.version),
                        tools: resolve_declared_packages_from_manifest(
                            &manifest_value,
                            &packages,
                            &format!("registry skill {}@{}", item.name, item.version),
                            PackageKind::Tool,
                        )?,
                    }
                } else {
                    LockRoot::RegistrySkill {
                        package_key: package_key(item.kind, &item.name, &item.version),
                        tools: Vec::new(),
                    }
                }
            }
            PackageKind::Knowledge => unreachable!(),
            PackageKind::Memory => unreachable!(),
            PackageKind::Profile => unreachable!(),
            PackageKind::Loop => unreachable!(),
            PackageKind::Tool => unreachable!(),
        };

        roots.push(root);
    }

    Ok(roots)
}

fn build_local_root_from_manifest(
    manifest_value: &Value,
    packages: &BTreeMap<String, crate::semver::types::LockedPackage>,
) -> Result<(String, LockedRoot)> {
    let kind = manifest_value
        .get("kind")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("manifest is missing kind"))?;
    let key = if kind == "skill" {
        "local:skill".to_string()
    } else {
        "local:agent".to_string()
    };
    let mut root = build_local_lock_root(
        manifest_value,
        &ResolvePlan {
            items: packages
                .values()
                .map(|pkg| crate::semver::types::ResolvedPackage {
                    kind: pkg.kind,
                    name: pkg.name.clone(),
                    version: pkg.version.clone(),
                    integrity: pkg.integrity.clone(),
                })
                .collect(),
        },
    )?;
    root.name = manifest_value
        .get("name")
        .and_then(Value::as_str)
        .map(str::to_string);
    root.version = manifest_value
        .get("version")
        .and_then(Value::as_str)
        .map(str::to_string);
    Ok((key, root))
}

fn build_local_lock_root(manifest_value: &Value, plan: &ResolvePlan) -> Result<LockedRoot> {
    let packages = plan_to_packages(plan);
    Ok(LockedRoot {
        name: manifest_value
            .get("name")
            .and_then(Value::as_str)
            .map(str::to_string),
        version: manifest_value
            .get("version")
            .and_then(Value::as_str)
            .map(str::to_string),
        tools: resolve_declared_packages_from_manifest(
            manifest_value,
            &packages,
            "local manifest",
            PackageKind::Tool,
        )?,
        skills: if manifest_value.get("kind").and_then(Value::as_str) == Some("agent") {
            resolve_declared_packages_from_manifest(
                manifest_value,
                &packages,
                "local manifest",
                PackageKind::Skill,
            )?
        } else {
            Vec::new()
        },
        knowledge: if manifest_value.get("kind").and_then(Value::as_str) == Some("agent") {
            resolve_declared_packages_from_manifest(
                manifest_value,
                &packages,
                "local manifest",
                PackageKind::Knowledge,
            )?
        } else {
            Vec::new()
        },
        memory: if manifest_value.get("kind").and_then(Value::as_str) == Some("agent") {
            resolve_declared_packages_from_manifest(
                manifest_value,
                &packages,
                "local manifest",
                PackageKind::Memory,
            )?
        } else {
            Vec::new()
        },
        profiles: if manifest_value.get("kind").and_then(Value::as_str) == Some("agent") {
            resolve_declared_packages_from_manifest(
                manifest_value,
                &packages,
                "local manifest",
                PackageKind::Profile,
            )?
        } else {
            Vec::new()
        },
        r#loop: if manifest_value.get("kind").and_then(Value::as_str) == Some("agent") {
            resolve_declared_loop_from_manifest(manifest_value, &packages, "local manifest")?
        } else {
            None
        },
        reserved: reserved_refs_from_manifest(),
    })
}

fn resolve_declared_loop_from_manifest(
    manifest_value: &Value,
    packages: &BTreeMap<String, crate::semver::types::LockedPackage>,
    manifest_label: &str,
) -> Result<Option<String>> {
    let desired = DesiredSet::from_cli_or_agent_json(manifest_value, None, false)
        .with_context(|| format!("reading kind for {}", manifest_label))?;
    let Some(item) = desired
        .items
        .into_iter()
        .find(|item| item.kind == PackageKind::Loop)
    else {
        return Ok(None);
    };

    let Some(pkg) = resolve_declared_package_from_packages(
        packages,
        &item.name,
        &item.range,
        PackageKind::Loop,
    )?
    else {
        return Err(anyhow!(
            "declared loop dependency {}@{} is missing from the resolved package set",
            item.name,
            item.range
        ));
    };

    Ok(Some(package_key(pkg.kind, &pkg.name, &pkg.version)))
}

fn resolve_declared_packages_from_manifest(
    manifest_value: &Value,
    packages: &BTreeMap<String, crate::semver::types::LockedPackage>,
    manifest_label: &str,
    kind: PackageKind,
) -> Result<Vec<String>> {
    let desired = DesiredSet::from_cli_or_agent_json(manifest_value, None, false)
        .with_context(|| format!("reading kind for {}", manifest_label))?;
    let mut resolved = Vec::new();
    for item in desired.items {
        if item.kind != kind {
            continue;
        }
        if let Some(pkg) =
            resolve_declared_package_from_packages(packages, &item.name, &item.range, kind)?
        {
            resolved.push(package_key(pkg.kind, &pkg.name, &pkg.version));
        } else {
            let dependency_label = kind.as_str();
            return Err(anyhow!(
                "declared {} dependency {}@{} is missing from the resolved package set",
                dependency_label,
                item.name,
                item.range
            ));
        }
    }
    Ok(resolved)
}

fn reserved_refs_from_manifest() -> ReservedReferences {
    ReservedReferences::default()
}

fn plan_to_packages(plan: &ResolvePlan) -> BTreeMap<String, crate::semver::types::LockedPackage> {
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

fn locked_root_from_root(root: LockRoot) -> Result<(String, LockedRoot)> {
    Ok(match root {
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
        } => (
            key,
            LockedRoot {
                name: Some(name),
                version: Some(version),
                tools,
                skills,
                knowledge,
                memory,
                profiles,
                r#loop,
                reserved,
            },
        ),
        LockRoot::RegistryAgent {
            package_key,
            tools,
            skills,
            knowledge,
            memory,
            profiles,
            r#loop,
            reserved,
        } => (
            package_key,
            LockedRoot {
                name: None,
                version: None,
                tools,
                skills,
                knowledge,
                memory,
                profiles,
                r#loop,
                reserved,
            },
        ),
        LockRoot::LocalSkill {
            key,
            name,
            version,
            tools,
        } => (
            key,
            LockedRoot {
                name: Some(name),
                version: Some(version),
                tools,
                skills: Vec::new(),
                knowledge: Vec::new(),
                memory: Vec::new(),
                profiles: Vec::new(),
                r#loop: None,
                reserved: ReservedReferences::default(),
            },
        ),
        LockRoot::RegistrySkill { package_key, tools } => (
            package_key,
            LockedRoot {
                name: None,
                version: None,
                tools,
                skills: Vec::new(),
                knowledge: Vec::new(),
                memory: Vec::new(),
                profiles: Vec::new(),
                r#loop: None,
                reserved: ReservedReferences::default(),
            },
        ),
    })
}

#[cfg(test)]
mod tests {
    use super::{
        InstallArgs, build_lock_roots, build_updated_lock, ensure_supported_install_kinds,
        installed_agent_manifest_path, installed_knowledge_manifest_path,
        installed_loop_manifest_path, installed_memory_manifest_path,
        installed_profile_manifest_path, installed_skill_manifest_path,
        should_load_manifest_for_install, validate_frozen_lock_compatibility,
    };
    use crate::semver::types::{
        DesiredSet, Lock, LockRoot, LockV1, LockV2, LockedDependency, LockedPackage, LockedRoot,
        PackageKind, ResolvePlan, ResolvedPackage,
    };
    use crate::workspace::{WorkspaceMetadata, WorkspacePackageRoot, WorkspacePackageRoots};
    use agentpm_sdk::models::install as sdkm;
    use axum::Router;
    use axum::body::Body;
    use axum::extract::State;
    use axum::http::header::AUTHORIZATION;
    use axum::http::{HeaderMap, Response, StatusCode};
    use axum::routing::{get, post};
    use chrono::Utc;
    use serde_json::{Value, json};
    use std::collections::BTreeMap;
    use std::fs;
    use std::net::SocketAddr;
    use std::path::{Path, PathBuf};
    use std::sync::Arc;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn direct_spec_install_without_local_manifest_does_not_require_manifest() {
        assert!(!should_load_manifest_for_install(
            Some("@zack/some-tool"),
            Path::new("/definitely/missing/agent.json")
        ));
    }

    #[test]
    fn manifest_driven_install_requires_manifest() {
        assert!(should_load_manifest_for_install(
            None,
            Path::new("agent.json")
        ));
    }

    #[test]
    fn direct_install_rejects_template_packages_until_new_exists() {
        let err = ensure_supported_install_kinds(&sdkm::ResolveResponse {
            items: vec![sdkm::ResolvedPackage {
                kind: sdkm::PackageKind::Template,
                name: "@zack/research-template".to_string(),
                version: "0.1.0".to_string(),
                integrity: "sha256-template".to_string(),
            }],
        })
        .unwrap_err();

        assert!(
            format!("{err:#}").contains("does not support template packages"),
            "{err:#}"
        );
    }

    #[test]
    fn direct_install_allows_skill_packages() {
        ensure_supported_install_kinds(&sdkm::ResolveResponse {
            items: vec![sdkm::ResolvedPackage {
                kind: sdkm::PackageKind::Skill,
                name: "@zack/triage-skill".to_string(),
                version: "0.1.0".to_string(),
                integrity: "sha256-skill".to_string(),
            }],
        })
        .unwrap();
    }

    #[test]
    fn direct_install_allows_knowledge_packages() {
        ensure_supported_install_kinds(&sdkm::ResolveResponse {
            items: vec![sdkm::ResolvedPackage {
                kind: sdkm::PackageKind::Knowledge,
                name: "@zack/python-docs".to_string(),
                version: "0.1.0".to_string(),
                integrity: "sha256-knowledge".to_string(),
            }],
        })
        .unwrap();
    }

    #[test]
    fn direct_install_allows_profile_packages() {
        ensure_supported_install_kinds(&sdkm::ResolveResponse {
            items: vec![sdkm::ResolvedPackage {
                kind: sdkm::PackageKind::Profile,
                name: "@zack/support-persona".to_string(),
                version: "0.1.0".to_string(),
                integrity: "sha256-profile".to_string(),
            }],
        })
        .unwrap();
    }

    #[test]
    fn direct_install_allows_loop_packages() {
        ensure_supported_install_kinds(&sdkm::ResolveResponse {
            items: vec![sdkm::ResolvedPackage {
                kind: sdkm::PackageKind::Loop,
                name: "@zack/incident-response-loop".to_string(),
                version: "0.1.0".to_string(),
                integrity: "sha256-loop".to_string(),
            }],
        })
        .unwrap();
    }

    #[test]
    fn frozen_v1_lockfile_rejects_local_agent_graphs() {
        let lock = Lock::V1(LockV1 {
            lockfile_version: 1,
            generated: Utc::now(),
            dependencies: BTreeMap::new(),
        });
        let desired = DesiredSet {
            items: vec![crate::semver::types::PackageRequirement::tool(
                "@zack/slack-post-message",
                "^0.1",
            )],
        };
        let manifest = json!({
            "kind": "agent",
            "name": "support-agent",
            "version": "0.1.0",
            "tools": ["@zack/slack-post-message@^0.1"]
        });

        let err = validate_frozen_lock_compatibility(&lock, Some(&manifest), &desired).unwrap_err();

        assert!(
            format!("{err:#}").contains("run `agentpm install` without --frozen"),
            "{err:#}"
        );
    }

    #[test]
    fn frozen_v1_lockfile_allows_tool_only_graphs() {
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
        let desired = DesiredSet {
            items: vec![crate::semver::types::PackageRequirement::tool(
                "@zack/echo",
                "^0.1",
            )],
        };

        validate_frozen_lock_compatibility(&lock, None, &desired).unwrap();
    }

    #[test]
    fn frozen_v2_lockfile_rejects_skill_graphs_that_require_v3() {
        let lock = Lock::V2(LockV2 {
            lockfile_version: 2,
            generated: Utc::now(),
            packages: BTreeMap::new(),
            roots: BTreeMap::new(),
        });
        let desired = DesiredSet {
            items: vec![crate::semver::types::PackageRequirement::new(
                PackageKind::Skill,
                "@zack/triage-skill",
                "^0.1",
            )],
        };

        let err = validate_frozen_lock_compatibility(&lock, None, &desired).unwrap_err();

        assert!(
            format!("{err:#}").contains("regenerate agent.lock v3"),
            "{err:#}"
        );
    }

    #[test]
    fn frozen_v2_lockfile_rejects_profile_graphs_that_require_v3() {
        let lock = Lock::V2(LockV2 {
            lockfile_version: 2,
            generated: Utc::now(),
            packages: BTreeMap::new(),
            roots: BTreeMap::new(),
        });
        let manifest = json!({
            "kind": "agent",
            "name": "support-agent",
            "version": "0.1.0",
            "profiles": ["@zack/support-persona@^0.1"]
        });
        let desired = DesiredSet {
            items: vec![crate::semver::types::PackageRequirement::new(
                PackageKind::Profile,
                "@zack/support-persona",
                "^0.1",
            )],
        };

        let err = validate_frozen_lock_compatibility(&lock, Some(&manifest), &desired).unwrap_err();

        let err_text = format!("{err:#}");
        assert!(
            err_text.contains("Skill, Knowledge, Memory, Profile, or Loop dependency graphs"),
            "{err_text}"
        );
        assert!(err_text.contains("regenerate agent.lock v3"), "{err_text}");
    }

    #[test]
    fn frozen_v2_lockfile_rejects_loop_graphs_that_require_v3() {
        let lock = Lock::V2(LockV2 {
            lockfile_version: 2,
            generated: Utc::now(),
            packages: BTreeMap::new(),
            roots: BTreeMap::new(),
        });
        let manifest = json!({
            "kind": "agent",
            "name": "support-agent",
            "version": "0.1.0",
            "loop": "@zack/incident-response-loop@^0.1"
        });
        let desired = DesiredSet {
            items: vec![crate::semver::types::PackageRequirement::new(
                PackageKind::Loop,
                "@zack/incident-response-loop",
                "^0.1",
            )],
        };

        let err = validate_frozen_lock_compatibility(&lock, Some(&manifest), &desired).unwrap_err();

        let err_text = format!("{err:#}");
        assert!(err_text.contains("Loop dependency graphs"), "{err_text}");
        assert!(err_text.contains("regenerate agent.lock v3"), "{err_text}");
    }

    #[test]
    fn manifest_driven_agent_install_builds_local_root_metadata() {
        let manifest = json!({
            "kind": "agent",
            "name": "support-agent",
            "version": "0.1.0",
            "skills": ["@zack/triage-skill@0.1.0"],
            "knowledge": ["@zack/python-docs@0.1.0"],
            "memory": ["@zack/session-memory@0.1.0"],
            "loop": "@zack/incident-response-loop@0.1.0",
            "profiles": ["@zack/support-persona@0.1.0"],
            "tools": ["@zack/slack-post-message@0.1.0"]
        });
        let plan = ResolvePlan {
            items: vec![
                ResolvedPackage {
                    kind: PackageKind::Tool,
                    name: "@zack/slack-post-message".to_string(),
                    version: "0.1.0".to_string(),
                    integrity: "sha256-tool".to_string(),
                },
                ResolvedPackage {
                    kind: PackageKind::Skill,
                    name: "@zack/triage-skill".to_string(),
                    version: "0.1.0".to_string(),
                    integrity: "sha256-skill".to_string(),
                },
                ResolvedPackage {
                    kind: PackageKind::Knowledge,
                    name: "@zack/python-docs".to_string(),
                    version: "0.1.0".to_string(),
                    integrity: "sha256-knowledge".to_string(),
                },
                ResolvedPackage {
                    kind: PackageKind::Memory,
                    name: "@zack/session-memory".to_string(),
                    version: "0.1.0".to_string(),
                    integrity: "sha256-memory".to_string(),
                },
                ResolvedPackage {
                    kind: PackageKind::Profile,
                    name: "@zack/support-persona".to_string(),
                    version: "0.1.0".to_string(),
                    integrity: "sha256-profile".to_string(),
                },
                ResolvedPackage {
                    kind: PackageKind::Loop,
                    name: "@zack/incident-response-loop".to_string(),
                    version: "0.1.0".to_string(),
                    integrity: "sha256-loop".to_string(),
                },
            ],
        };

        let root = temp_root("local-root");
        let roots = build_lock_roots(Some(&manifest), None, &plan, &root).unwrap();

        assert_eq!(roots.len(), 1);
        let crate::semver::types::LockRoot::LocalAgent {
            key,
            name,
            version,
            tools,
            skills,
            knowledge,
            memory,
            r#loop,
            profiles,
            reserved,
            ..
        } = &roots[0]
        else {
            panic!("expected local agent root");
        };
        assert_eq!(key, "local:agent");
        assert_eq!(name, "support-agent");
        assert_eq!(version, "0.1.0");
        assert_eq!(
            tools,
            &vec!["tool:@zack/slack-post-message@0.1.0".to_string()]
        );
        assert_eq!(skills, &vec!["skill:@zack/triage-skill@0.1.0".to_string()]);
        assert_eq!(
            knowledge,
            &vec!["knowledge:@zack/python-docs@0.1.0".to_string()]
        );
        assert_eq!(
            memory,
            &vec!["memory:@zack/session-memory@0.1.0".to_string()]
        );
        assert_eq!(
            r#loop.as_deref(),
            Some("loop:@zack/incident-response-loop@0.1.0")
        );
        assert_eq!(
            profiles,
            &vec!["profile:@zack/support-persona@0.1.0".to_string()]
        );
        assert!(reserved.skills.is_empty());
        assert!(reserved.profiles.is_empty());
    }

    #[test]
    fn manifest_driven_skill_install_builds_local_skill_root_metadata() {
        let manifest = json!({
            "kind": "skill",
            "name": "triage-skill",
            "version": "0.1.0",
            "tools": ["@zack/slack-post-message@0.1.0"]
        });
        let plan = ResolvePlan {
            items: vec![ResolvedPackage {
                kind: PackageKind::Tool,
                name: "@zack/slack-post-message".to_string(),
                version: "0.1.0".to_string(),
                integrity: "sha256-tool".to_string(),
            }],
        };

        let root = temp_root("local-skill-root");
        let roots = build_lock_roots(Some(&manifest), None, &plan, &root).unwrap();

        assert_eq!(roots.len(), 1);
        let LockRoot::LocalSkill {
            key,
            name,
            version,
            tools,
        } = &roots[0]
        else {
            panic!("expected local skill root");
        };
        assert_eq!(key, "local:skill");
        assert_eq!(name, "triage-skill");
        assert_eq!(version, "0.1.0");
        assert_eq!(
            tools,
            &vec!["tool:@zack/slack-post-message@0.1.0".to_string()]
        );
    }

    #[test]
    fn manifest_driven_agent_install_resolves_memory_version_ranges_to_pinned_version() {
        let manifest = json!({
            "kind": "agent",
            "name": "support-agent",
            "version": "0.1.0",
            "memory": ["@zack/session-memory@^0.1"]
        });
        let plan = ResolvePlan {
            items: vec![
                ResolvedPackage {
                    kind: PackageKind::Memory,
                    name: "@zack/session-memory".to_string(),
                    version: "0.1.0".to_string(),
                    integrity: "sha256-memory-1".to_string(),
                },
                ResolvedPackage {
                    kind: PackageKind::Memory,
                    name: "@zack/session-memory".to_string(),
                    version: "0.1.7".to_string(),
                    integrity: "sha256-memory-2".to_string(),
                },
            ],
        };

        let root = temp_root("local-memory-range");
        let roots = build_lock_roots(Some(&manifest), None, &plan, &root).unwrap();

        let LockRoot::LocalAgent { memory, .. } = &roots[0] else {
            panic!("expected local agent root");
        };
        assert_eq!(
            memory,
            &vec!["memory:@zack/session-memory@0.1.7".to_string()]
        );
    }

    #[test]
    fn manifest_driven_agent_install_rejects_wrong_kind_for_memory_dependency() {
        let manifest = json!({
            "kind": "agent",
            "name": "support-agent",
            "version": "0.1.0",
            "memory": ["@zack/session-memory@0.1.0"]
        });
        let plan = ResolvePlan {
            items: vec![ResolvedPackage {
                kind: PackageKind::Knowledge,
                name: "@zack/session-memory".to_string(),
                version: "0.1.0".to_string(),
                integrity: "sha256-knowledge".to_string(),
            }],
        };

        let root = temp_root("local-memory-wrong-kind");
        let err = build_lock_roots(Some(&manifest), None, &plan, &root).unwrap_err();

        assert!(
            format!("{err:#}").contains(
                "declared memory dependency @zack/session-memory@0.1.0 is missing from the resolved package set"
            ),
            "{err:#}"
        );
    }

    #[test]
    fn manifest_driven_agent_install_rejects_missing_memory_dependency() {
        let manifest = json!({
            "kind": "agent",
            "name": "support-agent",
            "version": "0.1.0",
            "memory": ["@zack/session-memory@0.1.0"]
        });
        let plan = ResolvePlan { items: Vec::new() };

        let root = temp_root("local-memory-missing");
        let err = build_lock_roots(Some(&manifest), None, &plan, &root).unwrap_err();

        assert!(
            format!("{err:#}").contains(
                "declared memory dependency @zack/session-memory@0.1.0 is missing from the resolved package set"
            ),
            "{err:#}"
        );
    }

    #[test]
    fn manifest_driven_agent_install_resolves_profile_version_ranges_to_pinned_version() {
        let manifest = json!({
            "kind": "agent",
            "name": "support-agent",
            "version": "0.1.0",
            "profiles": ["@zack/support-persona@^0.1"]
        });
        let plan = ResolvePlan {
            items: vec![
                ResolvedPackage {
                    kind: PackageKind::Profile,
                    name: "@zack/support-persona".to_string(),
                    version: "0.1.0".to_string(),
                    integrity: "sha256-profile-1".to_string(),
                },
                ResolvedPackage {
                    kind: PackageKind::Profile,
                    name: "@zack/support-persona".to_string(),
                    version: "0.1.7".to_string(),
                    integrity: "sha256-profile-2".to_string(),
                },
            ],
        };

        let root = temp_root("local-profile-range");
        let roots = build_lock_roots(Some(&manifest), None, &plan, &root).unwrap();

        let LockRoot::LocalAgent { profiles, .. } = &roots[0] else {
            panic!("expected local agent root");
        };
        assert_eq!(
            profiles,
            &vec!["profile:@zack/support-persona@0.1.7".to_string()]
        );
    }

    #[test]
    fn manifest_driven_agent_install_resolves_loop_version_ranges_to_pinned_version() {
        let manifest = json!({
            "kind": "agent",
            "name": "support-agent",
            "version": "0.1.0",
            "loop": "@zack/incident-response-loop@^0.1"
        });
        let plan = ResolvePlan {
            items: vec![
                ResolvedPackage {
                    kind: PackageKind::Loop,
                    name: "@zack/incident-response-loop".to_string(),
                    version: "0.1.0".to_string(),
                    integrity: "sha256-loop-1".to_string(),
                },
                ResolvedPackage {
                    kind: PackageKind::Loop,
                    name: "@zack/incident-response-loop".to_string(),
                    version: "0.1.7".to_string(),
                    integrity: "sha256-loop-2".to_string(),
                },
            ],
        };

        let root = temp_root("local-loop-range");
        let roots = build_lock_roots(Some(&manifest), None, &plan, &root).unwrap();

        let LockRoot::LocalAgent { r#loop, .. } = &roots[0] else {
            panic!("expected local agent root");
        };
        assert_eq!(
            r#loop.as_deref(),
            Some("loop:@zack/incident-response-loop@0.1.7")
        );
    }

    #[test]
    fn manifest_driven_agent_install_rejects_wrong_kind_for_loop_dependency() {
        let manifest = json!({
            "kind": "agent",
            "name": "support-agent",
            "version": "0.1.0",
            "loop": "@zack/incident-response-loop@0.1.0"
        });
        let plan = ResolvePlan {
            items: vec![ResolvedPackage {
                kind: PackageKind::Profile,
                name: "@zack/incident-response-loop".to_string(),
                version: "0.1.0".to_string(),
                integrity: "sha256-profile".to_string(),
            }],
        };

        let root = temp_root("local-loop-wrong-kind");
        let err = build_lock_roots(Some(&manifest), None, &plan, &root).unwrap_err();

        assert!(
            format!("{err:#}").contains(
                "declared loop dependency @zack/incident-response-loop@0.1.0 is missing from the resolved package set"
            ),
            "{err:#}"
        );
    }

    #[test]
    fn manifest_driven_agent_install_rejects_missing_loop_dependency() {
        let manifest = json!({
            "kind": "agent",
            "name": "support-agent",
            "version": "0.1.0",
            "loop": "@zack/incident-response-loop@0.1.0"
        });
        let plan = ResolvePlan { items: Vec::new() };

        let root = temp_root("local-loop-missing");
        let err = build_lock_roots(Some(&manifest), None, &plan, &root).unwrap_err();

        assert!(
            format!("{err:#}").contains(
                "declared loop dependency @zack/incident-response-loop@0.1.0 is missing from the resolved package set"
            ),
            "{err:#}"
        );
    }

    #[test]
    fn manifest_driven_agent_install_rejects_wrong_kind_for_profile_dependency() {
        let manifest = json!({
            "kind": "agent",
            "name": "support-agent",
            "version": "0.1.0",
            "profiles": ["@zack/support-persona@0.1.0"]
        });
        let plan = ResolvePlan {
            items: vec![ResolvedPackage {
                kind: PackageKind::Knowledge,
                name: "@zack/support-persona".to_string(),
                version: "0.1.0".to_string(),
                integrity: "sha256-knowledge".to_string(),
            }],
        };

        let root = temp_root("local-profile-wrong-kind");
        let err = build_lock_roots(Some(&manifest), None, &plan, &root).unwrap_err();

        assert!(
            format!("{err:#}").contains(
                "declared profile dependency @zack/support-persona@0.1.0 is missing from the resolved package set"
            ),
            "{err:#}"
        );
    }

    #[test]
    fn manifest_driven_agent_install_rejects_missing_profile_dependency() {
        let manifest = json!({
            "kind": "agent",
            "name": "support-agent",
            "version": "0.1.0",
            "profiles": ["@zack/support-persona@0.1.0"]
        });
        let plan = ResolvePlan { items: Vec::new() };

        let root = temp_root("local-profile-missing");
        let err = build_lock_roots(Some(&manifest), None, &plan, &root).unwrap_err();

        assert!(
            format!("{err:#}").contains(
                "declared profile dependency @zack/support-persona@0.1.0 is missing from the resolved package set"
            ),
            "{err:#}"
        );
    }

    #[test]
    fn direct_agent_install_builds_registry_root_from_installed_agent_manifest() {
        let root = temp_root("registry-root");
        let manifest_path =
            installed_agent_manifest_path(&root, "@zack/support-agent", "0.1.0").unwrap();
        fs::create_dir_all(manifest_path.parent().unwrap()).unwrap();
        fs::write(
            &manifest_path,
            serde_json::to_string_pretty(&json!({
                "kind": "agent",
                "name": "support-agent",
                "version": "0.1.0",
                "tools": ["@zack/slack-post-message@0.1.0"],
                "skills": ["@zack/triage-skill@0.1.0"],
                "knowledge": ["@zack/python-docs@0.1.0"],
                "memory": ["@zack/session-memory@0.1.0"],
                "loop": "@zack/incident-response-loop@0.1.0",
                "profiles": ["@zack/support-persona@0.1.0"]
            }))
            .unwrap(),
        )
        .unwrap();

        let plan = ResolvePlan {
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
                ResolvedPackage {
                    kind: PackageKind::Skill,
                    name: "@zack/triage-skill".to_string(),
                    version: "0.1.0".to_string(),
                    integrity: "sha256-skill".to_string(),
                },
                ResolvedPackage {
                    kind: PackageKind::Knowledge,
                    name: "@zack/python-docs".to_string(),
                    version: "0.1.0".to_string(),
                    integrity: "sha256-knowledge".to_string(),
                },
                ResolvedPackage {
                    kind: PackageKind::Memory,
                    name: "@zack/session-memory".to_string(),
                    version: "0.1.0".to_string(),
                    integrity: "sha256-memory".to_string(),
                },
                ResolvedPackage {
                    kind: PackageKind::Profile,
                    name: "@zack/support-persona".to_string(),
                    version: "0.1.0".to_string(),
                    integrity: "sha256-profile".to_string(),
                },
                ResolvedPackage {
                    kind: PackageKind::Loop,
                    name: "@zack/incident-response-loop".to_string(),
                    version: "0.1.0".to_string(),
                    integrity: "sha256-loop".to_string(),
                },
            ],
        };

        let roots =
            build_lock_roots(None, Some("@zack/support-agent@0.1.0"), &plan, &root).unwrap();

        assert_eq!(roots.len(), 1);
        let LockRoot::RegistryAgent {
            package_key,
            tools,
            skills,
            knowledge,
            memory,
            r#loop,
            profiles,
            reserved,
            ..
        } = &roots[0]
        else {
            panic!("expected registry agent root");
        };
        assert_eq!(package_key, "agent:@zack/support-agent@0.1.0");
        assert_eq!(
            tools,
            &vec!["tool:@zack/slack-post-message@0.1.0".to_string()]
        );
        assert_eq!(skills, &vec!["skill:@zack/triage-skill@0.1.0".to_string()]);
        assert_eq!(
            knowledge,
            &vec!["knowledge:@zack/python-docs@0.1.0".to_string()]
        );
        assert_eq!(
            memory,
            &vec!["memory:@zack/session-memory@0.1.0".to_string()]
        );
        assert_eq!(
            r#loop.as_deref(),
            Some("loop:@zack/incident-response-loop@0.1.0")
        );
        assert_eq!(
            profiles,
            &vec!["profile:@zack/support-persona@0.1.0".to_string()]
        );
        assert!(reserved.skills.is_empty());
        assert!(reserved.profiles.is_empty());
    }

    #[test]
    fn direct_agent_install_does_not_require_installed_manifest_for_registry_root_shape() {
        let root = temp_root("missing-registry-manifest");
        let plan = ResolvePlan {
            items: vec![ResolvedPackage {
                kind: PackageKind::Agent,
                name: "@zack/support-agent".to_string(),
                version: "0.1.0".to_string(),
                integrity: "sha256-agent".to_string(),
            }],
        };

        let roots =
            build_lock_roots(None, Some("@zack/support-agent@0.1.0"), &plan, &root).unwrap();

        assert_eq!(roots.len(), 1);
        let LockRoot::RegistryAgent { reserved, .. } = &roots[0] else {
            panic!("expected registry agent root");
        };
        assert!(reserved.skills.is_empty());
    }

    #[test]
    fn direct_skill_install_builds_registry_skill_root() {
        let root = temp_root("registry-skill-root");
        let manifest_path =
            installed_skill_manifest_path(&root, "@zack/triage-skill", "0.1.0").unwrap();
        fs::create_dir_all(manifest_path.parent().unwrap()).unwrap();
        fs::write(
            &manifest_path,
            serde_json::to_string_pretty(&json!({
                "kind": "skill",
                "name": "triage-skill",
                "version": "0.1.0",
                "tools": ["@zack/slack-post-message@0.1.0"]
            }))
            .unwrap(),
        )
        .unwrap();

        let plan = ResolvePlan {
            items: vec![
                ResolvedPackage {
                    kind: PackageKind::Skill,
                    name: "@zack/triage-skill".to_string(),
                    version: "0.1.0".to_string(),
                    integrity: "sha256-skill".to_string(),
                },
                ResolvedPackage {
                    kind: PackageKind::Tool,
                    name: "@zack/slack-post-message".to_string(),
                    version: "0.1.0".to_string(),
                    integrity: "sha256-tool".to_string(),
                },
            ],
        };

        let roots = build_lock_roots(None, Some("@zack/triage-skill@0.1.0"), &plan, &root).unwrap();

        assert_eq!(roots.len(), 1);
        let LockRoot::RegistrySkill { package_key, tools } = &roots[0] else {
            panic!("expected registry skill root");
        };
        assert_eq!(package_key, "skill:@zack/triage-skill@0.1.0");
        assert_eq!(
            tools,
            &vec!["tool:@zack/slack-post-message@0.1.0".to_string()]
        );
    }

    #[test]
    fn direct_knowledge_install_does_not_build_registry_root() {
        let root = temp_root("registry-knowledge-root");
        let manifest_path =
            installed_knowledge_manifest_path(&root, "@zack/python-docs", "0.1.0").unwrap();
        fs::create_dir_all(manifest_path.parent().unwrap()).unwrap();
        fs::write(
            &manifest_path,
            serde_json::to_string_pretty(&json!({
                "kind": "knowledge",
                "name": "python-docs",
                "version": "0.1.0",
                "description": "Knowledge",
                "knowledge": {
                    "mode": "context",
                    "documents": [{"path":"knowledge/docs/context.md"}]
                }
            }))
            .unwrap(),
        )
        .unwrap();

        let plan = ResolvePlan {
            items: vec![ResolvedPackage {
                kind: PackageKind::Knowledge,
                name: "@zack/python-docs".to_string(),
                version: "0.1.0".to_string(),
                integrity: "sha256-knowledge".to_string(),
            }],
        };

        let roots = build_lock_roots(None, Some("@zack/python-docs@0.1.0"), &plan, &root).unwrap();

        assert!(roots.is_empty());
    }

    #[test]
    fn direct_memory_install_does_not_build_registry_root() {
        let root = temp_root("registry-memory-root");
        let manifest_path =
            installed_memory_manifest_path(&root, "@zack/session-memory", "0.1.0").unwrap();
        fs::create_dir_all(manifest_path.parent().unwrap()).unwrap();
        fs::write(
            &manifest_path,
            serde_json::to_string_pretty(&json!({
                "kind": "memory",
                "name": "session-memory",
                "version": "0.1.0",
                "description": "Memory",
                "memory": {
                    "scopes": {
                        "user": { "description": "User scope." }
                    },
                    "record_types": {
                        "user_preference": {
                            "description": "Preference record.",
                            "schema": "schemas/user-preference.schema.json",
                            "version": "1.0.0"
                        }
                    },
                    "spaces": {
                        "profile": {
                            "description": "Profile space.",
                            "model": "document",
                            "scope": ["user"],
                            "record_types": ["user_preference"],
                            "retrieval": { "modes": ["key"] }
                        }
                    }
                }
            }))
            .unwrap(),
        )
        .unwrap();

        let plan = ResolvePlan {
            items: vec![ResolvedPackage {
                kind: PackageKind::Memory,
                name: "@zack/session-memory".to_string(),
                version: "0.1.0".to_string(),
                integrity: "sha256-memory".to_string(),
            }],
        };

        let roots =
            build_lock_roots(None, Some("@zack/session-memory@0.1.0"), &plan, &root).unwrap();

        assert!(roots.is_empty());
    }

    #[test]
    fn direct_agent_install_replaces_existing_registry_root_for_same_package() {
        let root = temp_root("replace-registry-root");
        let existing = Lock::V2(LockV2 {
            lockfile_version: 3,
            generated: Utc::now(),
            packages: BTreeMap::from([
                (
                    "agent:@zack/support-agent@0.1.0".to_string(),
                    LockedPackage {
                        kind: PackageKind::Agent,
                        name: "@zack/support-agent".to_string(),
                        version: "0.1.0".to_string(),
                        integrity: "sha256-old-agent".to_string(),
                    },
                ),
                (
                    "tool:@zack/old-tool@0.1.0".to_string(),
                    LockedPackage {
                        kind: PackageKind::Tool,
                        name: "@zack/old-tool".to_string(),
                        version: "0.1.0".to_string(),
                        integrity: "sha256-old-tool".to_string(),
                    },
                ),
                (
                    "agent:@zack/escalation-agent@0.1.0".to_string(),
                    LockedPackage {
                        kind: PackageKind::Agent,
                        name: "@zack/escalation-agent".to_string(),
                        version: "0.1.0".to_string(),
                        integrity: "sha256-escalation-agent".to_string(),
                    },
                ),
                (
                    "tool:@zack/capitalize@0.1.0".to_string(),
                    LockedPackage {
                        kind: PackageKind::Tool,
                        name: "@zack/capitalize".to_string(),
                        version: "0.1.0".to_string(),
                        integrity: "sha256-capitalize-legacy".to_string(),
                    },
                ),
            ]),
            roots: BTreeMap::from([
                (
                    "agent:@zack/support-agent@0.1.0".to_string(),
                    LockedRoot {
                        name: None,
                        version: None,
                        tools: vec!["tool:@zack/old-tool@0.1.0".to_string()],
                        skills: Vec::new(),
                        knowledge: Vec::new(),
                        memory: Vec::new(),
                        r#loop: None,
                        profiles: Vec::new(),
                        reserved: Default::default(),
                    },
                ),
                (
                    "agent:@zack/escalation-agent@0.1.0".to_string(),
                    LockedRoot {
                        name: None,
                        version: None,
                        tools: vec!["tool:@zack/capitalize@0.1.0".to_string()],
                        skills: Vec::new(),
                        knowledge: Vec::new(),
                        memory: Vec::new(),
                        r#loop: None,
                        profiles: Vec::new(),
                        reserved: Default::default(),
                    },
                ),
            ]),
        });

        let manifest_path =
            installed_agent_manifest_path(&root, "@zack/support-agent", "0.2.0").unwrap();
        fs::create_dir_all(manifest_path.parent().unwrap()).unwrap();
        fs::write(
            &manifest_path,
            serde_json::to_string_pretty(&json!({
                "kind": "agent",
                "name": "support-agent",
                "version": "0.2.0",
                "tools": ["@zack/capitalize@0.2.0"],
                "skills": ["@zack/triage-skill@0.2.0"],
                "knowledge": [{"name":"@zack/python-docs","version":"0.2.0"}]
            }))
            .unwrap(),
        )
        .unwrap();

        let other_manifest_path =
            installed_agent_manifest_path(&root, "@zack/escalation-agent", "0.1.0").unwrap();
        fs::create_dir_all(other_manifest_path.parent().unwrap()).unwrap();
        fs::write(
            &other_manifest_path,
            serde_json::to_string_pretty(&json!({
                "kind": "agent",
                "name": "escalation-agent",
                "version": "0.1.0",
                "tools": ["@zack/capitalize@0.1.0"]
            }))
            .unwrap(),
        )
        .unwrap();

        let skill_manifest_path =
            installed_skill_manifest_path(&root, "@zack/triage-skill", "0.2.0").unwrap();
        fs::create_dir_all(skill_manifest_path.parent().unwrap()).unwrap();
        fs::write(
            &skill_manifest_path,
            serde_json::to_string_pretty(&json!({
                "kind": "skill",
                "name": "triage-skill",
                "version": "0.2.0",
                "tools": ["@zack/summarize@0.2.0"]
            }))
            .unwrap(),
        )
        .unwrap();

        let plan = ResolvePlan {
            items: vec![
                ResolvedPackage {
                    kind: PackageKind::Agent,
                    name: "@zack/support-agent".to_string(),
                    version: "0.2.0".to_string(),
                    integrity: "sha256-new-agent".to_string(),
                },
                ResolvedPackage {
                    kind: PackageKind::Tool,
                    name: "@zack/capitalize".to_string(),
                    version: "0.2.0".to_string(),
                    integrity: "sha256-tool".to_string(),
                },
                ResolvedPackage {
                    kind: PackageKind::Skill,
                    name: "@zack/triage-skill".to_string(),
                    version: "0.2.0".to_string(),
                    integrity: "sha256-skill".to_string(),
                },
                ResolvedPackage {
                    kind: PackageKind::Knowledge,
                    name: "@zack/python-docs".to_string(),
                    version: "0.2.0".to_string(),
                    integrity: "sha256-knowledge".to_string(),
                },
                ResolvedPackage {
                    kind: PackageKind::Tool,
                    name: "@zack/summarize".to_string(),
                    version: "0.2.0".to_string(),
                    integrity: "sha256-summarize".to_string(),
                },
            ],
        };

        let next = build_updated_lock(
            &existing,
            None,
            Some("@zack/support-agent@0.2.0"),
            &plan,
            &root,
        )
        .unwrap();

        let Lock::V2(lock) = next else {
            panic!("expected v2 lock");
        };
        assert_eq!(lock.lockfile_version, 3);
        assert!(!lock.roots.contains_key("agent:@zack/support-agent@0.1.0"));
        assert!(lock.roots.contains_key("agent:@zack/support-agent@0.2.0"));
        assert!(
            lock.roots
                .contains_key("agent:@zack/escalation-agent@0.1.0")
        );
        assert!(
            !lock
                .packages
                .contains_key("agent:@zack/support-agent@0.1.0")
        );
        assert!(!lock.packages.contains_key("tool:@zack/old-tool@0.1.0"));
        assert!(
            lock.packages
                .contains_key("agent:@zack/support-agent@0.2.0")
        );
        assert!(
            lock.packages
                .contains_key("agent:@zack/escalation-agent@0.1.0")
        );
        assert!(lock.packages.contains_key("tool:@zack/capitalize@0.2.0"));
        assert!(lock.packages.contains_key("tool:@zack/capitalize@0.1.0"));
        assert!(lock.packages.contains_key("skill:@zack/triage-skill@0.2.0"));
        assert!(
            lock.packages
                .contains_key("knowledge:@zack/python-docs@0.2.0")
        );
        assert!(lock.packages.contains_key("tool:@zack/summarize@0.2.0"));

        let support_root = lock.roots.get("agent:@zack/support-agent@0.2.0").unwrap();
        assert_eq!(
            support_root.tools,
            vec!["tool:@zack/capitalize@0.2.0".to_string()]
        );
        assert_eq!(
            support_root.skills,
            vec!["skill:@zack/triage-skill@0.2.0".to_string()]
        );
        assert_eq!(
            support_root.knowledge,
            vec!["knowledge:@zack/python-docs@0.2.0".to_string()]
        );
    }

    #[test]
    fn direct_agent_upgrade_preserves_older_dependencies_required_by_local_root() {
        let root = temp_root("preserve-local-root-dependency");
        let existing = Lock::V2(LockV2 {
            lockfile_version: 3,
            generated: Utc::now(),
            packages: BTreeMap::from([
                (
                    "tool:@zack/capitalize@0.1.0".to_string(),
                    LockedPackage {
                        kind: PackageKind::Tool,
                        name: "@zack/capitalize".to_string(),
                        version: "0.1.0".to_string(),
                        integrity: "sha256-capitalize-old".to_string(),
                    },
                ),
                (
                    "agent:@zack/support-agent@0.1.0".to_string(),
                    LockedPackage {
                        kind: PackageKind::Agent,
                        name: "@zack/support-agent".to_string(),
                        version: "0.1.0".to_string(),
                        integrity: "sha256-support-old".to_string(),
                    },
                ),
            ]),
            roots: BTreeMap::from([
                (
                    "local:agent".to_string(),
                    LockedRoot {
                        name: Some("local-support-agent".to_string()),
                        version: Some("0.1.0".to_string()),
                        tools: vec!["tool:@zack/capitalize@0.1.0".to_string()],
                        skills: Vec::new(),
                        knowledge: Vec::new(),
                        memory: Vec::new(),
                        r#loop: None,
                        profiles: Vec::new(),
                        reserved: Default::default(),
                    },
                ),
                (
                    "agent:@zack/support-agent@0.1.0".to_string(),
                    LockedRoot {
                        name: None,
                        version: None,
                        tools: vec!["tool:@zack/capitalize@0.1.0".to_string()],
                        skills: Vec::new(),
                        knowledge: Vec::new(),
                        memory: Vec::new(),
                        r#loop: None,
                        profiles: Vec::new(),
                        reserved: Default::default(),
                    },
                ),
            ]),
        });

        let manifest_path =
            installed_agent_manifest_path(&root, "@zack/support-agent", "0.2.0").unwrap();
        fs::create_dir_all(manifest_path.parent().unwrap()).unwrap();
        fs::write(
            &manifest_path,
            serde_json::to_string_pretty(&json!({
                "kind": "agent",
                "name": "support-agent",
                "version": "0.2.0",
                "tools": ["@zack/capitalize@0.2.0"]
            }))
            .unwrap(),
        )
        .unwrap();

        let plan = ResolvePlan {
            items: vec![
                ResolvedPackage {
                    kind: PackageKind::Agent,
                    name: "@zack/support-agent".to_string(),
                    version: "0.2.0".to_string(),
                    integrity: "sha256-support-new".to_string(),
                },
                ResolvedPackage {
                    kind: PackageKind::Tool,
                    name: "@zack/capitalize".to_string(),
                    version: "0.2.0".to_string(),
                    integrity: "sha256-capitalize-new".to_string(),
                },
            ],
        };

        let next = build_updated_lock(
            &existing,
            None,
            Some("@zack/support-agent@0.2.0"),
            &plan,
            &root,
        )
        .unwrap();

        let Lock::V2(lock) = next else {
            panic!("expected v2 lock");
        };
        assert!(lock.roots.contains_key("local:agent"));
        assert!(!lock.roots.contains_key("agent:@zack/support-agent@0.1.0"));
        assert!(lock.roots.contains_key("agent:@zack/support-agent@0.2.0"));
        assert!(lock.packages.contains_key("tool:@zack/capitalize@0.1.0"));
        assert!(lock.packages.contains_key("tool:@zack/capitalize@0.2.0"));

        let local_root = lock.roots.get("local:agent").unwrap();
        assert_eq!(
            local_root.tools,
            vec!["tool:@zack/capitalize@0.1.0".to_string()]
        );
        let registry_root = lock.roots.get("agent:@zack/support-agent@0.2.0").unwrap();
        assert_eq!(
            registry_root.tools,
            vec!["tool:@zack/capitalize@0.2.0".to_string()]
        );
    }

    #[test]
    fn direct_agent_upgrade_preserves_older_profile_required_by_local_root() {
        let root = temp_root("preserve-local-root-profile");
        let existing = Lock::V2(LockV2 {
            lockfile_version: 3,
            generated: Utc::now(),
            packages: BTreeMap::from([
                (
                    "profile:@zack/support-persona@0.1.0".to_string(),
                    LockedPackage {
                        kind: PackageKind::Profile,
                        name: "@zack/support-persona".to_string(),
                        version: "0.1.0".to_string(),
                        integrity: "sha256-profile-old".to_string(),
                    },
                ),
                (
                    "agent:@zack/support-agent@0.1.0".to_string(),
                    LockedPackage {
                        kind: PackageKind::Agent,
                        name: "@zack/support-agent".to_string(),
                        version: "0.1.0".to_string(),
                        integrity: "sha256-support-old".to_string(),
                    },
                ),
            ]),
            roots: BTreeMap::from([
                (
                    "local:agent".to_string(),
                    LockedRoot {
                        name: Some("local-support-agent".to_string()),
                        version: Some("0.1.0".to_string()),
                        tools: Vec::new(),
                        skills: Vec::new(),
                        knowledge: Vec::new(),
                        memory: Vec::new(),
                        r#loop: None,
                        profiles: vec!["profile:@zack/support-persona@0.1.0".to_string()],
                        reserved: Default::default(),
                    },
                ),
                (
                    "agent:@zack/support-agent@0.1.0".to_string(),
                    LockedRoot {
                        name: None,
                        version: None,
                        tools: Vec::new(),
                        skills: Vec::new(),
                        knowledge: Vec::new(),
                        memory: Vec::new(),
                        r#loop: None,
                        profiles: vec!["profile:@zack/support-persona@0.1.0".to_string()],
                        reserved: Default::default(),
                    },
                ),
            ]),
        });

        let manifest_path =
            installed_agent_manifest_path(&root, "@zack/support-agent", "0.2.0").unwrap();
        fs::create_dir_all(manifest_path.parent().unwrap()).unwrap();
        fs::write(
            &manifest_path,
            serde_json::to_string_pretty(&json!({
                "kind": "agent",
                "name": "support-agent",
                "version": "0.2.0",
                "profiles": ["@zack/support-persona@0.2.0"]
            }))
            .unwrap(),
        )
        .unwrap();

        let plan = ResolvePlan {
            items: vec![
                ResolvedPackage {
                    kind: PackageKind::Agent,
                    name: "@zack/support-agent".to_string(),
                    version: "0.2.0".to_string(),
                    integrity: "sha256-support-new".to_string(),
                },
                ResolvedPackage {
                    kind: PackageKind::Profile,
                    name: "@zack/support-persona".to_string(),
                    version: "0.2.0".to_string(),
                    integrity: "sha256-profile-new".to_string(),
                },
            ],
        };

        let next = build_updated_lock(
            &existing,
            None,
            Some("@zack/support-agent@0.2.0"),
            &plan,
            &root,
        )
        .unwrap();

        let Lock::V2(lock) = next else {
            panic!("expected v2 lock");
        };
        assert!(lock.roots.contains_key("local:agent"));
        assert!(!lock.roots.contains_key("agent:@zack/support-agent@0.1.0"));
        assert!(lock.roots.contains_key("agent:@zack/support-agent@0.2.0"));
        assert!(
            lock.packages
                .contains_key("profile:@zack/support-persona@0.1.0")
        );
        assert!(
            lock.packages
                .contains_key("profile:@zack/support-persona@0.2.0")
        );

        let local_root = lock.roots.get("local:agent").unwrap();
        assert_eq!(
            local_root.profiles,
            vec!["profile:@zack/support-persona@0.1.0".to_string()]
        );
        let registry_root = lock.roots.get("agent:@zack/support-agent@0.2.0").unwrap();
        assert_eq!(
            registry_root.profiles,
            vec!["profile:@zack/support-persona@0.2.0".to_string()]
        );
    }

    #[test]
    fn direct_agent_upgrade_prunes_old_profile_when_unreachable() {
        let root = temp_root("prune-unreachable-profile");
        let existing = Lock::V2(LockV2 {
            lockfile_version: 3,
            generated: Utc::now(),
            packages: BTreeMap::from([
                (
                    "profile:@zack/support-persona@0.1.0".to_string(),
                    LockedPackage {
                        kind: PackageKind::Profile,
                        name: "@zack/support-persona".to_string(),
                        version: "0.1.0".to_string(),
                        integrity: "sha256-profile-old".to_string(),
                    },
                ),
                (
                    "agent:@zack/support-agent@0.1.0".to_string(),
                    LockedPackage {
                        kind: PackageKind::Agent,
                        name: "@zack/support-agent".to_string(),
                        version: "0.1.0".to_string(),
                        integrity: "sha256-support-old".to_string(),
                    },
                ),
            ]),
            roots: BTreeMap::from([(
                "agent:@zack/support-agent@0.1.0".to_string(),
                LockedRoot {
                    name: None,
                    version: None,
                    tools: Vec::new(),
                    skills: Vec::new(),
                    knowledge: Vec::new(),
                    memory: Vec::new(),
                    r#loop: None,
                    profiles: vec!["profile:@zack/support-persona@0.1.0".to_string()],
                    reserved: Default::default(),
                },
            )]),
        });

        let manifest_path =
            installed_agent_manifest_path(&root, "@zack/support-agent", "0.2.0").unwrap();
        fs::create_dir_all(manifest_path.parent().unwrap()).unwrap();
        fs::write(
            &manifest_path,
            serde_json::to_string_pretty(&json!({
                "kind": "agent",
                "name": "support-agent",
                "version": "0.2.0",
                "profiles": ["@zack/support-persona@0.2.0"]
            }))
            .unwrap(),
        )
        .unwrap();

        let plan = ResolvePlan {
            items: vec![
                ResolvedPackage {
                    kind: PackageKind::Agent,
                    name: "@zack/support-agent".to_string(),
                    version: "0.2.0".to_string(),
                    integrity: "sha256-support-new".to_string(),
                },
                ResolvedPackage {
                    kind: PackageKind::Profile,
                    name: "@zack/support-persona".to_string(),
                    version: "0.2.0".to_string(),
                    integrity: "sha256-profile-new".to_string(),
                },
            ],
        };

        let next = build_updated_lock(
            &existing,
            None,
            Some("@zack/support-agent@0.2.0"),
            &plan,
            &root,
        )
        .unwrap();

        let Lock::V2(lock) = next else {
            panic!("expected v2 lock");
        };
        assert!(!lock.roots.contains_key("agent:@zack/support-agent@0.1.0"));
        assert!(lock.roots.contains_key("agent:@zack/support-agent@0.2.0"));
        assert!(
            !lock
                .packages
                .contains_key("profile:@zack/support-persona@0.1.0")
        );
        assert!(
            lock.packages
                .contains_key("profile:@zack/support-persona@0.2.0")
        );

        let registry_root = lock.roots.get("agent:@zack/support-agent@0.2.0").unwrap();
        assert_eq!(
            registry_root.profiles,
            vec!["profile:@zack/support-persona@0.2.0".to_string()]
        );
    }

    #[test]
    fn direct_agent_upgrade_preserves_older_loop_required_by_local_root() {
        let root = temp_root("preserve-local-root-loop");
        let existing = Lock::V2(LockV2 {
            lockfile_version: 3,
            generated: Utc::now(),
            packages: BTreeMap::from([
                (
                    "loop:@zack/incident-response-loop@0.1.0".to_string(),
                    LockedPackage {
                        kind: PackageKind::Loop,
                        name: "@zack/incident-response-loop".to_string(),
                        version: "0.1.0".to_string(),
                        integrity: "sha256-loop-old".to_string(),
                    },
                ),
                (
                    "agent:@zack/support-agent@0.1.0".to_string(),
                    LockedPackage {
                        kind: PackageKind::Agent,
                        name: "@zack/support-agent".to_string(),
                        version: "0.1.0".to_string(),
                        integrity: "sha256-support-old".to_string(),
                    },
                ),
            ]),
            roots: BTreeMap::from([
                (
                    "local:agent".to_string(),
                    LockedRoot {
                        name: Some("local-support-agent".to_string()),
                        version: Some("0.1.0".to_string()),
                        tools: Vec::new(),
                        skills: Vec::new(),
                        knowledge: Vec::new(),
                        memory: Vec::new(),
                        r#loop: Some("loop:@zack/incident-response-loop@0.1.0".to_string()),
                        profiles: Vec::new(),
                        reserved: Default::default(),
                    },
                ),
                (
                    "agent:@zack/support-agent@0.1.0".to_string(),
                    LockedRoot {
                        name: None,
                        version: None,
                        tools: Vec::new(),
                        skills: Vec::new(),
                        knowledge: Vec::new(),
                        memory: Vec::new(),
                        r#loop: Some("loop:@zack/incident-response-loop@0.1.0".to_string()),
                        profiles: Vec::new(),
                        reserved: Default::default(),
                    },
                ),
            ]),
        });

        let manifest_path =
            installed_agent_manifest_path(&root, "@zack/support-agent", "0.2.0").unwrap();
        fs::create_dir_all(manifest_path.parent().unwrap()).unwrap();
        fs::write(
            &manifest_path,
            serde_json::to_string_pretty(&json!({
                "kind": "agent",
                "name": "support-agent",
                "version": "0.2.0",
                "loop": "@zack/incident-response-loop@0.2.0"
            }))
            .unwrap(),
        )
        .unwrap();

        let plan = ResolvePlan {
            items: vec![
                ResolvedPackage {
                    kind: PackageKind::Agent,
                    name: "@zack/support-agent".to_string(),
                    version: "0.2.0".to_string(),
                    integrity: "sha256-support-new".to_string(),
                },
                ResolvedPackage {
                    kind: PackageKind::Loop,
                    name: "@zack/incident-response-loop".to_string(),
                    version: "0.2.0".to_string(),
                    integrity: "sha256-loop-new".to_string(),
                },
            ],
        };

        let next = build_updated_lock(
            &existing,
            None,
            Some("@zack/support-agent@0.2.0"),
            &plan,
            &root,
        )
        .unwrap();

        let Lock::V2(lock) = next else {
            panic!("expected v2 lock");
        };
        assert!(lock.roots.contains_key("local:agent"));
        assert!(!lock.roots.contains_key("agent:@zack/support-agent@0.1.0"));
        assert!(lock.roots.contains_key("agent:@zack/support-agent@0.2.0"));
        assert!(
            lock.packages
                .contains_key("loop:@zack/incident-response-loop@0.1.0")
        );
        assert!(
            lock.packages
                .contains_key("loop:@zack/incident-response-loop@0.2.0")
        );

        let local_root = lock.roots.get("local:agent").unwrap();
        assert_eq!(
            local_root.r#loop.as_deref(),
            Some("loop:@zack/incident-response-loop@0.1.0")
        );
        let registry_root = lock.roots.get("agent:@zack/support-agent@0.2.0").unwrap();
        assert_eq!(
            registry_root.r#loop.as_deref(),
            Some("loop:@zack/incident-response-loop@0.2.0")
        );
    }

    #[test]
    fn direct_agent_upgrade_prunes_old_loop_when_unreachable() {
        let root = temp_root("prune-unreachable-loop");
        let existing = Lock::V2(LockV2 {
            lockfile_version: 3,
            generated: Utc::now(),
            packages: BTreeMap::from([
                (
                    "loop:@zack/incident-response-loop@0.1.0".to_string(),
                    LockedPackage {
                        kind: PackageKind::Loop,
                        name: "@zack/incident-response-loop".to_string(),
                        version: "0.1.0".to_string(),
                        integrity: "sha256-loop-old".to_string(),
                    },
                ),
                (
                    "agent:@zack/support-agent@0.1.0".to_string(),
                    LockedPackage {
                        kind: PackageKind::Agent,
                        name: "@zack/support-agent".to_string(),
                        version: "0.1.0".to_string(),
                        integrity: "sha256-support-old".to_string(),
                    },
                ),
            ]),
            roots: BTreeMap::from([(
                "agent:@zack/support-agent@0.1.0".to_string(),
                LockedRoot {
                    name: None,
                    version: None,
                    tools: Vec::new(),
                    skills: Vec::new(),
                    knowledge: Vec::new(),
                    memory: Vec::new(),
                    r#loop: Some("loop:@zack/incident-response-loop@0.1.0".to_string()),
                    profiles: Vec::new(),
                    reserved: Default::default(),
                },
            )]),
        });

        let manifest_path =
            installed_agent_manifest_path(&root, "@zack/support-agent", "0.2.0").unwrap();
        fs::create_dir_all(manifest_path.parent().unwrap()).unwrap();
        fs::write(
            &manifest_path,
            serde_json::to_string_pretty(&json!({
                "kind": "agent",
                "name": "support-agent",
                "version": "0.2.0",
                "loop": "@zack/incident-response-loop@0.2.0"
            }))
            .unwrap(),
        )
        .unwrap();

        let plan = ResolvePlan {
            items: vec![
                ResolvedPackage {
                    kind: PackageKind::Agent,
                    name: "@zack/support-agent".to_string(),
                    version: "0.2.0".to_string(),
                    integrity: "sha256-support-new".to_string(),
                },
                ResolvedPackage {
                    kind: PackageKind::Loop,
                    name: "@zack/incident-response-loop".to_string(),
                    version: "0.2.0".to_string(),
                    integrity: "sha256-loop-new".to_string(),
                },
            ],
        };

        let next = build_updated_lock(
            &existing,
            None,
            Some("@zack/support-agent@0.2.0"),
            &plan,
            &root,
        )
        .unwrap();

        let Lock::V2(lock) = next else {
            panic!("expected v2 lock");
        };
        assert!(!lock.roots.contains_key("agent:@zack/support-agent@0.1.0"));
        assert!(lock.roots.contains_key("agent:@zack/support-agent@0.2.0"));
        assert!(
            !lock
                .packages
                .contains_key("loop:@zack/incident-response-loop@0.1.0")
        );
        assert!(
            lock.packages
                .contains_key("loop:@zack/incident-response-loop@0.2.0")
        );

        let registry_root = lock.roots.get("agent:@zack/support-agent@0.2.0").unwrap();
        assert_eq!(
            registry_root.r#loop.as_deref(),
            Some("loop:@zack/incident-response-loop@0.2.0")
        );
    }

    #[test]
    fn direct_skill_install_replaces_existing_registry_root_for_same_package() {
        let root = temp_root("replace-skill-registry-root");
        let existing = Lock::V2(LockV2 {
            lockfile_version: 3,
            generated: Utc::now(),
            packages: BTreeMap::from([
                (
                    "skill:@zack/triage-skill@0.1.0".to_string(),
                    LockedPackage {
                        kind: PackageKind::Skill,
                        name: "@zack/triage-skill".to_string(),
                        version: "0.1.0".to_string(),
                        integrity: "sha256-old-skill".to_string(),
                    },
                ),
                (
                    "tool:@zack/slack-post-message@0.1.0".to_string(),
                    LockedPackage {
                        kind: PackageKind::Tool,
                        name: "@zack/slack-post-message".to_string(),
                        version: "0.1.0".to_string(),
                        integrity: "sha256-old-tool".to_string(),
                    },
                ),
                (
                    "skill:@zack/escalation-skill@0.1.0".to_string(),
                    LockedPackage {
                        kind: PackageKind::Skill,
                        name: "@zack/escalation-skill".to_string(),
                        version: "0.1.0".to_string(),
                        integrity: "sha256-escalation-skill".to_string(),
                    },
                ),
                (
                    "tool:@zack/capitalize@0.1.0".to_string(),
                    LockedPackage {
                        kind: PackageKind::Tool,
                        name: "@zack/capitalize".to_string(),
                        version: "0.1.0".to_string(),
                        integrity: "sha256-capitalize-legacy".to_string(),
                    },
                ),
            ]),
            roots: BTreeMap::from([
                (
                    "skill:@zack/triage-skill@0.1.0".to_string(),
                    LockedRoot {
                        name: None,
                        version: None,
                        tools: vec!["tool:@zack/slack-post-message@0.1.0".to_string()],
                        skills: Vec::new(),
                        knowledge: Vec::new(),
                        memory: Vec::new(),
                        r#loop: None,
                        profiles: Vec::new(),
                        reserved: Default::default(),
                    },
                ),
                (
                    "skill:@zack/escalation-skill@0.1.0".to_string(),
                    LockedRoot {
                        name: None,
                        version: None,
                        tools: vec!["tool:@zack/capitalize@0.1.0".to_string()],
                        skills: Vec::new(),
                        knowledge: Vec::new(),
                        memory: Vec::new(),
                        r#loop: None,
                        profiles: Vec::new(),
                        reserved: Default::default(),
                    },
                ),
            ]),
        });

        let manifest_path =
            installed_skill_manifest_path(&root, "@zack/triage-skill", "0.2.0").unwrap();
        fs::create_dir_all(manifest_path.parent().unwrap()).unwrap();
        fs::write(
            &manifest_path,
            serde_json::to_string_pretty(&json!({
                "kind": "skill",
                "name": "triage-skill",
                "version": "0.2.0",
                "tools": ["@zack/slack-post-message@0.2.0", "@zack/summarize@0.2.0"]
            }))
            .unwrap(),
        )
        .unwrap();

        let other_manifest_path =
            installed_skill_manifest_path(&root, "@zack/escalation-skill", "0.1.0").unwrap();
        fs::create_dir_all(other_manifest_path.parent().unwrap()).unwrap();
        fs::write(
            &other_manifest_path,
            serde_json::to_string_pretty(&json!({
                "kind": "skill",
                "name": "escalation-skill",
                "version": "0.1.0",
                "tools": ["@zack/capitalize@0.1.0"]
            }))
            .unwrap(),
        )
        .unwrap();

        let plan = ResolvePlan {
            items: vec![
                ResolvedPackage {
                    kind: PackageKind::Skill,
                    name: "@zack/triage-skill".to_string(),
                    version: "0.2.0".to_string(),
                    integrity: "sha256-new-skill".to_string(),
                },
                ResolvedPackage {
                    kind: PackageKind::Tool,
                    name: "@zack/slack-post-message".to_string(),
                    version: "0.2.0".to_string(),
                    integrity: "sha256-tool".to_string(),
                },
                ResolvedPackage {
                    kind: PackageKind::Tool,
                    name: "@zack/summarize".to_string(),
                    version: "0.2.0".to_string(),
                    integrity: "sha256-summarize".to_string(),
                },
            ],
        };

        let next = build_updated_lock(
            &existing,
            None,
            Some("@zack/triage-skill@0.2.0"),
            &plan,
            &root,
        )
        .unwrap();

        let Lock::V2(lock) = next else {
            panic!("expected v2 lock");
        };
        assert_eq!(lock.lockfile_version, 3);
        assert!(!lock.roots.contains_key("skill:@zack/triage-skill@0.1.0"));
        assert!(lock.roots.contains_key("skill:@zack/triage-skill@0.2.0"));
        assert!(
            lock.roots
                .contains_key("skill:@zack/escalation-skill@0.1.0")
        );
        assert!(!lock.packages.contains_key("skill:@zack/triage-skill@0.1.0"));
        assert!(
            !lock
                .packages
                .contains_key("tool:@zack/slack-post-message@0.1.0")
        );
        assert!(lock.packages.contains_key("skill:@zack/triage-skill@0.2.0"));
        assert!(
            lock.packages
                .contains_key("skill:@zack/escalation-skill@0.1.0")
        );
        assert!(
            lock.packages
                .contains_key("tool:@zack/slack-post-message@0.2.0")
        );
        assert!(lock.packages.contains_key("tool:@zack/summarize@0.2.0"));
        assert!(lock.packages.contains_key("tool:@zack/capitalize@0.1.0"));

        let triage_root = lock.roots.get("skill:@zack/triage-skill@0.2.0").unwrap();
        assert_eq!(
            triage_root.tools,
            vec![
                "tool:@zack/slack-post-message@0.2.0".to_string(),
                "tool:@zack/summarize@0.2.0".to_string()
            ]
        );
    }

    #[test]
    fn direct_agent_upgrade_fails_if_surviving_registry_manifest_is_missing() {
        let root = temp_root("missing-surviving-registry-manifest");
        let existing = Lock::V2(LockV2 {
            lockfile_version: 2,
            generated: Utc::now(),
            packages: BTreeMap::from([
                (
                    "agent:@zack/support-agent@0.1.0".to_string(),
                    LockedPackage {
                        kind: PackageKind::Agent,
                        name: "@zack/support-agent".to_string(),
                        version: "0.1.0".to_string(),
                        integrity: "sha256-support-old".to_string(),
                    },
                ),
                (
                    "agent:@zack/escalation-agent@0.1.0".to_string(),
                    LockedPackage {
                        kind: PackageKind::Agent,
                        name: "@zack/escalation-agent".to_string(),
                        version: "0.1.0".to_string(),
                        integrity: "sha256-escalation-old".to_string(),
                    },
                ),
            ]),
            roots: BTreeMap::from([
                (
                    "agent:@zack/support-agent@0.1.0".to_string(),
                    LockedRoot {
                        name: None,
                        version: None,
                        tools: Vec::new(),
                        skills: Vec::new(),
                        knowledge: Vec::new(),
                        memory: Vec::new(),
                        r#loop: None,
                        profiles: Vec::new(),
                        reserved: Default::default(),
                    },
                ),
                (
                    "agent:@zack/escalation-agent@0.1.0".to_string(),
                    LockedRoot {
                        name: None,
                        version: None,
                        tools: Vec::new(),
                        skills: Vec::new(),
                        knowledge: Vec::new(),
                        memory: Vec::new(),
                        r#loop: None,
                        profiles: Vec::new(),
                        reserved: Default::default(),
                    },
                ),
            ]),
        });

        let manifest_path =
            installed_agent_manifest_path(&root, "@zack/support-agent", "0.2.0").unwrap();
        fs::create_dir_all(manifest_path.parent().unwrap()).unwrap();
        fs::write(
            &manifest_path,
            serde_json::to_string_pretty(&json!({
                "kind": "agent",
                "name": "support-agent",
                "version": "0.2.0"
            }))
            .unwrap(),
        )
        .unwrap();

        let plan = ResolvePlan {
            items: vec![ResolvedPackage {
                kind: PackageKind::Agent,
                name: "@zack/support-agent".to_string(),
                version: "0.2.0".to_string(),
                integrity: "sha256-support-new".to_string(),
            }],
        };

        let err = build_updated_lock(
            &existing,
            None,
            Some("@zack/support-agent@0.2.0"),
            &plan,
            &root,
        )
        .unwrap_err();

        assert!(format!("{err:#}").contains("cannot safely compact lockfile packages"));
        assert!(format!("{err:#}").contains("@zack/escalation-agent@0.1.0"));
    }

    #[test]
    fn manifest_driven_install_replaces_registry_roots_with_local_root() {
        let existing = Lock::V2(LockV2 {
            lockfile_version: 2,
            generated: Utc::now(),
            packages: BTreeMap::from([(
                "agent:@zack/support-agent@0.1.0".to_string(),
                LockedPackage {
                    kind: PackageKind::Agent,
                    name: "@zack/support-agent".to_string(),
                    version: "0.1.0".to_string(),
                    integrity: "sha256-agent".to_string(),
                },
            )]),
            roots: BTreeMap::from([(
                "agent:@zack/support-agent@0.1.0".to_string(),
                LockedRoot {
                    name: None,
                    version: None,
                    tools: vec!["tool:@zack/capitalize@0.1.0".to_string()],
                    skills: Vec::new(),
                    knowledge: Vec::new(),
                    memory: Vec::new(),
                    r#loop: None,
                    profiles: Vec::new(),
                    reserved: Default::default(),
                },
            )]),
        });
        let manifest = json!({
            "kind": "agent",
            "name": "local-support-agent",
            "version": "0.1.0",
            "tools": ["@zack/capitalize@0.2.0"],
            "skills": ["@zack/triage-skill@0.1.0"]
        });
        let plan = ResolvePlan {
            items: vec![
                ResolvedPackage {
                    kind: PackageKind::Tool,
                    name: "@zack/capitalize".to_string(),
                    version: "0.2.0".to_string(),
                    integrity: "sha256-tool".to_string(),
                },
                ResolvedPackage {
                    kind: PackageKind::Skill,
                    name: "@zack/triage-skill".to_string(),
                    version: "0.1.0".to_string(),
                    integrity: "sha256-skill".to_string(),
                },
            ],
        };

        let next = build_updated_lock(
            &existing,
            Some(&manifest),
            None,
            &plan,
            Path::new(".agentpm"),
        )
        .unwrap();

        let Lock::V2(lock) = next else {
            panic!("expected v2 lock");
        };
        assert_eq!(lock.roots.len(), 1);
        let local = lock.roots.get("local:agent").expect("local root missing");
        assert_eq!(local.name.as_deref(), Some("local-support-agent"));
        assert_eq!(local.version.as_deref(), Some("0.1.0"));
        assert_eq!(local.tools, vec!["tool:@zack/capitalize@0.2.0".to_string()]);
        assert!(!lock.roots.contains_key("agent:@zack/support-agent@0.1.0"));
        assert!(
            !lock
                .packages
                .contains_key("agent:@zack/support-agent@0.1.0")
        );
    }

    #[test]
    fn direct_agent_install_preserves_existing_local_root() {
        let root = temp_root("preserve-local-root");
        let existing = Lock::V2(LockV2 {
            lockfile_version: 2,
            generated: Utc::now(),
            packages: BTreeMap::from([(
                "tool:@zack/capitalize@0.1.0".to_string(),
                LockedPackage {
                    kind: PackageKind::Tool,
                    name: "@zack/capitalize".to_string(),
                    version: "0.1.0".to_string(),
                    integrity: "sha256-tool".to_string(),
                },
            )]),
            roots: BTreeMap::from([(
                "local:agent".to_string(),
                LockedRoot {
                    name: Some("local-support-agent".to_string()),
                    version: Some("0.1.0".to_string()),
                    tools: vec!["tool:@zack/capitalize@0.1.0".to_string()],
                    skills: Vec::new(),
                    knowledge: Vec::new(),
                    memory: Vec::new(),
                    r#loop: None,
                    profiles: Vec::new(),
                    reserved: Default::default(),
                },
            )]),
        });
        let manifest_path =
            installed_agent_manifest_path(&root, "@zack/support-agent", "0.1.0").unwrap();
        fs::create_dir_all(manifest_path.parent().unwrap()).unwrap();
        fs::write(
            &manifest_path,
            serde_json::to_string_pretty(&json!({
                "kind": "agent",
                "name": "support-agent",
                "version": "0.1.0"
            }))
            .unwrap(),
        )
        .unwrap();
        let manifest = json!({
            "kind": "agent",
            "name": "local-support-agent",
            "version": "0.1.0",
            "tools": ["@zack/capitalize@0.1.0"]
        });
        let plan = ResolvePlan {
            items: vec![ResolvedPackage {
                kind: PackageKind::Agent,
                name: "@zack/support-agent".to_string(),
                version: "0.1.0".to_string(),
                integrity: "sha256-agent".to_string(),
            }],
        };

        let next = build_updated_lock(
            &existing,
            Some(&manifest),
            Some("@zack/support-agent@0.1.0"),
            &plan,
            &root,
        )
        .unwrap();

        let Lock::V2(lock) = next else {
            panic!("expected v2 lock");
        };
        assert!(lock.roots.contains_key("local:agent"));
        assert!(lock.roots.contains_key("agent:@zack/support-agent@0.1.0"));
    }

    #[test]
    fn direct_tool_install_updates_local_root_and_preserves_registry_roots() {
        let existing = Lock::V2(LockV2 {
            lockfile_version: 2,
            generated: Utc::now(),
            packages: BTreeMap::from([
                (
                    "tool:@zack/capitalize@0.1.0".to_string(),
                    LockedPackage {
                        kind: PackageKind::Tool,
                        name: "@zack/capitalize".to_string(),
                        version: "0.1.0".to_string(),
                        integrity: "sha256-cap".to_string(),
                    },
                ),
                (
                    "agent:@zack/support-agent@0.1.0".to_string(),
                    LockedPackage {
                        kind: PackageKind::Agent,
                        name: "@zack/support-agent".to_string(),
                        version: "0.1.0".to_string(),
                        integrity: "sha256-agent".to_string(),
                    },
                ),
            ]),
            roots: BTreeMap::from([
                (
                    "local:agent".to_string(),
                    LockedRoot {
                        name: Some("local-support-agent".to_string()),
                        version: Some("0.1.0".to_string()),
                        tools: vec!["tool:@zack/capitalize@0.1.0".to_string()],
                        skills: Vec::new(),
                        knowledge: Vec::new(),
                        memory: Vec::new(),
                        r#loop: None,
                        profiles: Vec::new(),
                        reserved: Default::default(),
                    },
                ),
                (
                    "agent:@zack/support-agent@0.1.0".to_string(),
                    LockedRoot {
                        name: None,
                        version: None,
                        tools: vec!["tool:@zack/capitalize@0.1.0".to_string()],
                        skills: Vec::new(),
                        knowledge: Vec::new(),
                        memory: Vec::new(),
                        r#loop: None,
                        profiles: Vec::new(),
                        reserved: Default::default(),
                    },
                ),
            ]),
        });
        let manifest = json!({
            "kind": "agent",
            "name": "local-support-agent",
            "version": "0.1.0",
            "tools": ["@zack/capitalize@0.1.0", "@zack/summarize@0.2.0"]
        });
        let plan = ResolvePlan {
            items: vec![ResolvedPackage {
                kind: PackageKind::Tool,
                name: "@zack/summarize".to_string(),
                version: "0.2.0".to_string(),
                integrity: "sha256-summarize".to_string(),
            }],
        };

        let next = build_updated_lock(
            &existing,
            Some(&manifest),
            Some("@zack/summarize@0.2.0"),
            &plan,
            Path::new(".agentpm"),
        )
        .unwrap();

        let Lock::V2(lock) = next else {
            panic!("expected v2 lock");
        };
        let local = lock.roots.get("local:agent").expect("local root missing");
        assert!(
            local
                .tools
                .contains(&"tool:@zack/capitalize@0.1.0".to_string())
        );
        assert!(
            local
                .tools
                .contains(&"tool:@zack/summarize@0.2.0".to_string())
        );
        assert!(lock.roots.contains_key("agent:@zack/support-agent@0.1.0"));
    }

    #[test]
    fn direct_skill_install_with_local_agent_manifest_updates_local_root_skills() {
        let root = temp_root("direct-skill-local-agent");
        let existing = Lock::V2(LockV2 {
            lockfile_version: 2,
            generated: Utc::now(),
            packages: BTreeMap::from([(
                "tool:@zack/capitalize@0.1.0".to_string(),
                LockedPackage {
                    kind: PackageKind::Tool,
                    name: "@zack/capitalize".to_string(),
                    version: "0.1.0".to_string(),
                    integrity: "sha256-cap".to_string(),
                },
            )]),
            roots: BTreeMap::from([(
                "local:agent".to_string(),
                LockedRoot {
                    name: Some("local-support-agent".to_string()),
                    version: Some("0.1.0".to_string()),
                    tools: vec!["tool:@zack/capitalize@0.1.0".to_string()],
                    skills: Vec::new(),
                    knowledge: Vec::new(),
                    memory: Vec::new(),
                    r#loop: None,
                    profiles: Vec::new(),
                    reserved: Default::default(),
                },
            )]),
        });
        let manifest = json!({
            "kind": "agent",
            "name": "local-support-agent",
            "version": "0.1.0",
            "tools": ["@zack/capitalize@0.1.0"],
            "skills": ["@zack/triage-skill@0.1.0"]
        });

        let skill_manifest_path =
            installed_skill_manifest_path(&root, "@zack/triage-skill", "0.1.0").unwrap();
        fs::create_dir_all(skill_manifest_path.parent().unwrap()).unwrap();
        fs::write(
            &skill_manifest_path,
            serde_json::to_string_pretty(&json!({
                "kind": "skill",
                "name": "triage-skill",
                "version": "0.1.0",
                "tools": ["@zack/slack-post-message@0.1.0"]
            }))
            .unwrap(),
        )
        .unwrap();

        let plan = ResolvePlan {
            items: vec![
                ResolvedPackage {
                    kind: PackageKind::Skill,
                    name: "@zack/triage-skill".to_string(),
                    version: "0.1.0".to_string(),
                    integrity: "sha256-skill".to_string(),
                },
                ResolvedPackage {
                    kind: PackageKind::Tool,
                    name: "@zack/slack-post-message".to_string(),
                    version: "0.1.0".to_string(),
                    integrity: "sha256-tool".to_string(),
                },
            ],
        };

        let next = build_updated_lock(
            &existing,
            Some(&manifest),
            Some("@zack/triage-skill@0.1.0"),
            &plan,
            &root,
        )
        .unwrap();

        let Lock::V2(lock) = next else {
            panic!("expected modern lock");
        };
        let local = lock.roots.get("local:agent").expect("local root missing");
        assert_eq!(local.tools, vec!["tool:@zack/capitalize@0.1.0".to_string()]);
        assert_eq!(
            local.skills,
            vec!["skill:@zack/triage-skill@0.1.0".to_string()]
        );
        let skill_root = lock
            .roots
            .get("skill:@zack/triage-skill@0.1.0")
            .expect("skill root missing");
        assert_eq!(
            skill_root.tools,
            vec!["tool:@zack/slack-post-message@0.1.0".to_string()]
        );
    }

    #[test]
    fn direct_knowledge_install_without_local_manifest_keeps_package_only_v2_lock() {
        let existing = Lock::V2(LockV2 {
            lockfile_version: 2,
            generated: Utc::now(),
            packages: BTreeMap::new(),
            roots: BTreeMap::new(),
        });
        let root = temp_root("direct-knowledge-package-only");
        let manifest_path =
            installed_knowledge_manifest_path(&root, "@zack/python-docs", "0.1.0").unwrap();
        fs::create_dir_all(manifest_path.parent().unwrap()).unwrap();
        fs::write(
            &manifest_path,
            serde_json::to_string_pretty(&json!({
                "kind": "knowledge",
                "name": "python-docs",
                "version": "0.1.0",
                "description": "Knowledge",
                "knowledge": {
                    "mode": "context",
                    "documents": [{"path":"knowledge/docs/context.md"}]
                }
            }))
            .unwrap(),
        )
        .unwrap();
        let plan = ResolvePlan {
            items: vec![ResolvedPackage {
                kind: PackageKind::Knowledge,
                name: "@zack/python-docs".to_string(),
                version: "0.1.0".to_string(),
                integrity: "sha256-knowledge".to_string(),
            }],
        };

        let next = build_updated_lock(
            &existing,
            None,
            Some("@zack/python-docs@0.1.0"),
            &plan,
            &root,
        )
        .unwrap();

        let Lock::V2(lock) = next else {
            panic!("expected v2 lock");
        };
        assert_eq!(lock.lockfile_version, 2);
        assert!(lock.roots.is_empty());
        assert!(
            lock.packages
                .contains_key("knowledge:@zack/python-docs@0.1.0")
        );
    }

    #[test]
    fn direct_memory_install_without_local_manifest_keeps_package_only_v2_lock() {
        let existing = Lock::V2(LockV2 {
            lockfile_version: 2,
            generated: Utc::now(),
            packages: BTreeMap::new(),
            roots: BTreeMap::new(),
        });
        let root = temp_root("direct-memory-package-only");
        let manifest_path =
            installed_memory_manifest_path(&root, "@zack/session-memory", "0.1.0").unwrap();
        fs::create_dir_all(manifest_path.parent().unwrap()).unwrap();
        fs::write(
            &manifest_path,
            serde_json::to_string_pretty(&json!({
                "kind": "memory",
                "name": "session-memory",
                "version": "0.1.0",
                "description": "Memory",
                "memory": {
                    "scopes": {
                        "user": { "description": "User scope." }
                    },
                    "record_types": {
                        "user_preference": {
                            "description": "Preference record.",
                            "schema": "schemas/user-preference.schema.json",
                            "version": "1.0.0"
                        }
                    },
                    "spaces": {
                        "profile": {
                            "description": "Profile space.",
                            "model": "document",
                            "scope": ["user"],
                            "record_types": ["user_preference"],
                            "retrieval": { "modes": ["key"] }
                        }
                    }
                }
            }))
            .unwrap(),
        )
        .unwrap();
        let plan = ResolvePlan {
            items: vec![ResolvedPackage {
                kind: PackageKind::Memory,
                name: "@zack/session-memory".to_string(),
                version: "0.1.0".to_string(),
                integrity: "sha256-memory".to_string(),
            }],
        };

        let next = build_updated_lock(
            &existing,
            None,
            Some("@zack/session-memory@0.1.0"),
            &plan,
            &root,
        )
        .unwrap();

        let Lock::V2(lock) = next else {
            panic!("expected v2 lock");
        };
        assert_eq!(lock.lockfile_version, 2);
        assert!(lock.roots.is_empty());
        assert!(
            lock.packages
                .contains_key("memory:@zack/session-memory@0.1.0")
        );
    }

    #[test]
    fn direct_profile_install_without_local_manifest_keeps_package_only_v2_lock() {
        let existing = Lock::V2(LockV2 {
            lockfile_version: 2,
            generated: Utc::now(),
            packages: BTreeMap::new(),
            roots: BTreeMap::new(),
        });
        let root = temp_root("direct-profile-package-only");
        let manifest_path =
            installed_profile_manifest_path(&root, "@zack/support-persona", "0.1.0").unwrap();
        fs::create_dir_all(manifest_path.parent().unwrap()).unwrap();
        fs::write(
            &manifest_path,
            serde_json::to_string_pretty(&json!({
                "kind": "profile",
                "name": "support-persona",
                "version": "0.1.0",
                "description": "Profile",
                "profile": {
                    "identity": {
                        "role": "Support specialist"
                    },
                    "communication": {
                        "tone": ["clear", "calm"]
                    }
                }
            }))
            .unwrap(),
        )
        .unwrap();
        let plan = ResolvePlan {
            items: vec![ResolvedPackage {
                kind: PackageKind::Profile,
                name: "@zack/support-persona".to_string(),
                version: "0.1.0".to_string(),
                integrity: "sha256-profile".to_string(),
            }],
        };

        let next = build_updated_lock(
            &existing,
            None,
            Some("@zack/support-persona@0.1.0"),
            &plan,
            &root,
        )
        .unwrap();

        let Lock::V2(lock) = next else {
            panic!("expected v2 lock");
        };
        assert_eq!(lock.lockfile_version, 2);
        assert!(lock.roots.is_empty());
        assert!(
            lock.packages
                .contains_key("profile:@zack/support-persona@0.1.0")
        );
    }

    #[test]
    fn direct_loop_install_without_local_manifest_keeps_package_only_v2_lock() {
        let existing = Lock::V2(LockV2 {
            lockfile_version: 2,
            generated: Utc::now(),
            packages: BTreeMap::new(),
            roots: BTreeMap::new(),
        });
        let root = temp_root("direct-loop-package-only");
        let manifest_path =
            installed_loop_manifest_path(&root, "@zack/incident-response-loop", "0.1.0").unwrap();
        fs::create_dir_all(manifest_path.parent().unwrap()).unwrap();
        fs::write(
            &manifest_path,
            serde_json::to_string_pretty(&json!({
                "kind": "loop",
                "name": "incident-response-loop",
                "version": "0.1.0",
                "description": "Loop",
                "loop": {
                    "entry_phase": "triage",
                    "phases": [
                        {
                            "id": "triage",
                            "objective": "Review the incoming incident context and choose the next action.",
                            "outcomes": [
                                {
                                    "id": "needs-investigation",
                                    "description": "More evidence is required before the loop can finish."
                                },
                                {
                                    "id": "complete",
                                    "description": "The loop gathered enough context to end successfully."
                                },
                                {
                                    "id": "handoff",
                                    "description": "The loop should transfer control to a human or another system."
                                }
                            ]
                        },
                        {
                            "id": "investigate",
                            "objective": "Gather missing evidence before finalizing the response.",
                            "outcomes": [
                                {
                                    "id": "complete",
                                    "description": "The missing evidence has been collected."
                                },
                                {
                                    "id": "handoff",
                                    "description": "Investigation cannot continue without external intervention."
                                }
                            ]
                        }
                    ],
                    "transitions": [
                        { "from": "triage", "on": "needs-investigation", "to": "investigate" },
                        { "from": "triage", "on": "complete", "to": "$end" },
                        { "from": "triage", "on": "handoff", "to": "$handoff" },
                        { "from": "investigate", "on": "complete", "to": "$end" },
                        { "from": "investigate", "on": "handoff", "to": "$handoff" }
                    ]
                }
            }))
            .unwrap(),
        )
        .unwrap();
        let plan = ResolvePlan {
            items: vec![ResolvedPackage {
                kind: PackageKind::Loop,
                name: "@zack/incident-response-loop".to_string(),
                version: "0.1.0".to_string(),
                integrity: "sha256-loop".to_string(),
            }],
        };

        let next = build_updated_lock(
            &existing,
            None,
            Some("@zack/incident-response-loop@0.1.0"),
            &plan,
            &root,
        )
        .unwrap();

        let Lock::V2(lock) = next else {
            panic!("expected v2 lock");
        };
        assert_eq!(lock.lockfile_version, 2);
        assert!(lock.roots.is_empty());
        assert!(
            lock.packages
                .contains_key("loop:@zack/incident-response-loop@0.1.0")
        );
    }

    #[test]
    fn direct_profile_install_with_local_agent_manifest_updates_local_root_profiles() {
        let root = temp_root("direct-profile-local-agent");
        let existing = Lock::V2(LockV2 {
            lockfile_version: 2,
            generated: Utc::now(),
            packages: BTreeMap::from([(
                "tool:@zack/capitalize@0.1.0".to_string(),
                LockedPackage {
                    kind: PackageKind::Tool,
                    name: "@zack/capitalize".to_string(),
                    version: "0.1.0".to_string(),
                    integrity: "sha256-cap".to_string(),
                },
            )]),
            roots: BTreeMap::from([(
                "local:agent".to_string(),
                LockedRoot {
                    name: Some("local-support-agent".to_string()),
                    version: Some("0.1.0".to_string()),
                    tools: vec!["tool:@zack/capitalize@0.1.0".to_string()],
                    skills: Vec::new(),
                    knowledge: Vec::new(),
                    memory: Vec::new(),
                    r#loop: None,
                    profiles: Vec::new(),
                    reserved: Default::default(),
                },
            )]),
        });
        let manifest = json!({
            "kind": "agent",
            "name": "local-support-agent",
            "version": "0.1.0",
            "tools": ["@zack/capitalize@0.1.0"],
            "profiles": ["@zack/support-persona@0.1.0"]
        });

        let plan = ResolvePlan {
            items: vec![ResolvedPackage {
                kind: PackageKind::Profile,
                name: "@zack/support-persona".to_string(),
                version: "0.1.0".to_string(),
                integrity: "sha256-profile".to_string(),
            }],
        };

        let next = build_updated_lock(
            &existing,
            Some(&manifest),
            Some("@zack/support-persona@0.1.0"),
            &plan,
            &root,
        )
        .unwrap();

        let Lock::V2(lock) = next else {
            panic!("expected modern lock");
        };
        let local = lock.roots.get("local:agent").expect("local root missing");
        assert_eq!(local.tools, vec!["tool:@zack/capitalize@0.1.0".to_string()]);
        assert_eq!(
            local.profiles,
            vec!["profile:@zack/support-persona@0.1.0".to_string()]
        );
    }

    #[test]
    fn direct_knowledge_install_prunes_standalone_knowledge_roots_from_existing_lock() {
        let existing = Lock::V2(LockV2 {
            lockfile_version: 3,
            generated: Utc::now(),
            packages: BTreeMap::from([(
                "knowledge:@zack/python-docs@0.1.0".to_string(),
                LockedPackage {
                    kind: PackageKind::Knowledge,
                    name: "@zack/python-docs".to_string(),
                    version: "0.1.0".to_string(),
                    integrity: "sha256-old-knowledge".to_string(),
                },
            )]),
            roots: BTreeMap::from([(
                "knowledge:@zack/python-docs@0.1.0".to_string(),
                LockedRoot {
                    name: None,
                    version: None,
                    tools: Vec::new(),
                    skills: Vec::new(),
                    knowledge: Vec::new(),
                    memory: Vec::new(),
                    r#loop: None,
                    profiles: Vec::new(),
                    reserved: Default::default(),
                },
            )]),
        });
        let root = temp_root("direct-knowledge-prune-root");
        let manifest_path =
            installed_knowledge_manifest_path(&root, "@zack/python-docs", "0.1.0").unwrap();
        fs::create_dir_all(manifest_path.parent().unwrap()).unwrap();
        fs::write(
            &manifest_path,
            serde_json::to_string_pretty(&json!({
                "kind": "knowledge",
                "name": "python-docs",
                "version": "0.1.0",
                "description": "Knowledge",
                "knowledge": {
                    "mode": "context",
                    "documents": [{"path":"knowledge/docs/context.md"}]
                }
            }))
            .unwrap(),
        )
        .unwrap();
        let plan = ResolvePlan {
            items: vec![ResolvedPackage {
                kind: PackageKind::Knowledge,
                name: "@zack/python-docs".to_string(),
                version: "0.1.0".to_string(),
                integrity: "sha256-knowledge".to_string(),
            }],
        };

        let next = build_updated_lock(
            &existing,
            None,
            Some("@zack/python-docs@0.1.0"),
            &plan,
            &root,
        )
        .unwrap();

        let Lock::V2(lock) = next else {
            panic!("expected v2 lock");
        };
        assert_eq!(lock.lockfile_version, 2);
        assert!(!lock.roots.contains_key("knowledge:@zack/python-docs@0.1.0"));
        assert!(
            lock.packages
                .contains_key("knowledge:@zack/python-docs@0.1.0")
        );
    }

    #[test]
    fn direct_tool_install_rebuilds_local_root_from_declared_manifest_tools() {
        let existing = Lock::V2(LockV2 {
            lockfile_version: 2,
            generated: Utc::now(),
            packages: BTreeMap::from([
                (
                    "tool:@zack/capitalize@0.1.0".to_string(),
                    LockedPackage {
                        kind: PackageKind::Tool,
                        name: "@zack/capitalize".to_string(),
                        version: "0.1.0".to_string(),
                        integrity: "sha256-cap".to_string(),
                    },
                ),
                (
                    "tool:@zack/translate@0.3.0".to_string(),
                    LockedPackage {
                        kind: PackageKind::Tool,
                        name: "@zack/translate".to_string(),
                        version: "0.3.0".to_string(),
                        integrity: "sha256-trans".to_string(),
                    },
                ),
            ]),
            roots: BTreeMap::new(),
        });
        let manifest = json!({
            "kind": "agent",
            "name": "local-support-agent",
            "version": "0.1.0",
            "tools": [
                "@zack/capitalize@0.1.0",
                "@zack/translate@^0.3",
                "@zack/summarize@0.2.0"
            ]
        });
        let plan = ResolvePlan {
            items: vec![ResolvedPackage {
                kind: PackageKind::Tool,
                name: "@zack/summarize".to_string(),
                version: "0.2.0".to_string(),
                integrity: "sha256-summarize".to_string(),
            }],
        };

        let next = build_updated_lock(
            &existing,
            Some(&manifest),
            Some("@zack/summarize@0.2.0"),
            &plan,
            Path::new(".agentpm"),
        )
        .unwrap();

        let Lock::V2(lock) = next else {
            panic!("expected v2 lock");
        };
        let local = lock.roots.get("local:agent").expect("local root missing");
        assert_eq!(
            local.tools,
            vec![
                "tool:@zack/capitalize@0.1.0".to_string(),
                "tool:@zack/translate@0.3.0".to_string(),
                "tool:@zack/summarize@0.2.0".to_string()
            ]
        );
    }

    #[tokio::test]
    async fn workspace_install_rebuilds_lock_without_rewriting_workspace_file() {
        let root = temp_root("workspace-install");
        fs::create_dir_all(root.join("agents")).unwrap();
        crate::manifest::write_manifest_pretty_atomic(
            &root.join("agent.json"),
            &json!({
                "kind": "agent",
                "name": "primary",
                "version": "0.1.0",
                "description": "Primary",
                "tools": [{"name":"@zack/echo","version":"0.1.0"}],
                "skills": [],
                "knowledge": [],
                "memory": [],
                "profiles": []
            }),
        )
        .unwrap();
        crate::manifest::write_manifest_pretty_atomic(
            &root.join("agents/reviewer.agent.json"),
            &json!({
                "kind": "agent",
                "name": "reviewer",
                "version": "0.1.0",
                "description": "Reviewer",
                "tools": [{"name":"@zack/review","version":"0.2.0"}],
                "skills": [],
                "knowledge": [],
                "memory": [],
                "profiles": []
            }),
        )
        .unwrap();

        let workspace = WorkspaceMetadata {
            schema_version: 1,
            manifests: vec![
                "agent.json".to_string(),
                "agents/reviewer.agent.json".to_string(),
            ],
            package_roots: WorkspacePackageRoots {
                tools: Vec::new(),
                agents: vec![WorkspacePackageRoot {
                    name: "@zack/support-agent".to_string(),
                    version: "0.1.0".to_string(),
                }],
                skills: Vec::new(),
                knowledge: Vec::new(),
                memory: Vec::new(),
                profiles: Vec::new(),
            },
        };
        crate::workspace::write_workspace_metadata(&root, &workspace).unwrap();
        let workspace_before = fs::read_to_string(root.join("agentpm.workspace.json")).unwrap();

        crate::manifest::write_lock(
            &root,
            &Lock::V2(LockV2 {
                lockfile_version: 2,
                generated: Utc::now(),
                packages: BTreeMap::new(),
                roots: BTreeMap::new(),
            }),
        )
        .unwrap();

        let tool_echo_tar = build_tarball(&[(
            "agent.json",
            serde_json::to_string_pretty(&json!({
                "kind":"tool",
                "name":"echo",
                "version":"0.1.0",
                "description":"Echo",
                "entrypoint":{"command":"python","args":["echo.py"]},
                "runtime":{"type":"python","version":"3.12"},
                "inputs":{},
                "outputs":{},
                "files":["echo.py"]
            }))
            .unwrap(),
        )]);
        let tool_review_tar = build_tarball(&[(
            "agent.json",
            serde_json::to_string_pretty(&json!({
                "kind":"tool",
                "name":"review",
                "version":"0.2.0",
                "description":"Review",
                "entrypoint":{"command":"python","args":["review.py"]},
                "runtime":{"type":"python","version":"3.12"},
                "inputs":{},
                "outputs":{},
                "files":["review.py"]
            }))
            .unwrap(),
        )]);
        let tool_translate_tar = build_tarball(&[(
            "agent.json",
            serde_json::to_string_pretty(&json!({
                "kind":"tool",
                "name":"translate",
                "version":"0.3.0",
                "description":"Translate",
                "entrypoint":{"command":"python","args":["translate.py"]},
                "runtime":{"type":"python","version":"3.12"},
                "inputs":{},
                "outputs":{},
                "files":["translate.py"]
            }))
            .unwrap(),
        )]);
        let agent_tar = build_tarball(&[(
            "agent.json",
            serde_json::to_string_pretty(&json!({
                "kind":"agent",
                "name":"support-agent",
                "version":"0.1.0",
                "description":"Support Agent",
                "tools":[{"name":"@zack/translate","version":"0.3.0"}],
                "skills":[],
                "knowledge":[],
                "memory":[],
                "profiles":[]
            }))
            .unwrap(),
        )]);

        let initial_state = InstallTestState {
            base_url: String::new(),
            echo_tar: tool_echo_tar.clone(),
            echo_sha: sha_hex(&tool_echo_tar),
            review_tar: tool_review_tar.clone(),
            review_sha: sha_hex(&tool_review_tar),
            translate_tar: tool_translate_tar.clone(),
            translate_sha: sha_hex(&tool_translate_tar),
            agent_tar: agent_tar.clone(),
            agent_sha: sha_hex(&agent_tar),
        };

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr: SocketAddr = listener.local_addr().unwrap();
        let base_url = format!("http://127.0.0.1:{}", addr.port());
        let state = Arc::new(InstallTestState {
            base_url: base_url.clone(),
            ..initial_state
        });
        let app = Router::new()
            .route("/v1/tools/install/resolve", post(test_resolve))
            .route("/v1/tools/install/init", post(test_init))
            .route("/v1/tools/install/finalize", post(test_finalize))
            .route("/artifact/echo", get(get_echo))
            .route("/artifact/review", get(get_review))
            .route("/artifact/translate", get(get_translate))
            .route("/artifact/agent", get(get_agent))
            .with_state(state);
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        let args = InstallArgs {
            manifest: root.join("agent.json").to_string_lossy().into_owned(),
            spec: None,
            frozen: false,
            refresh: false,
            update_range: false,
            quiet: true,
            require_attestation: false,
            token: None,
        };

        args.run(base_url).await.unwrap();

        let workspace_after = fs::read_to_string(root.join("agentpm.workspace.json")).unwrap();
        assert_eq!(workspace_after, workspace_before);

        let lock: Value =
            serde_json::from_str(&fs::read_to_string(root.join("agent.lock")).unwrap()).unwrap();
        assert!(lock["roots"].get("local:agent").is_some());
        assert!(
            lock["roots"]
                .get("local:agent:agents/reviewer.agent.json")
                .is_some()
        );
        assert!(
            lock["roots"]
                .get("agent:@zack/support-agent@0.1.0")
                .is_some()
        );
        assert!(lock["packages"].get("tool:@zack/echo@0.1.0").is_some());
        assert!(lock["packages"].get("tool:@zack/review@0.2.0").is_some());
        assert!(lock["packages"].get("tool:@zack/translate@0.3.0").is_some());
        assert!(
            lock["packages"]
                .get("template:@zack/research-template@0.1.0")
                .is_none()
        );

        server.abort();
        let _ = fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn direct_install_sends_bearer_token_to_resolve_endpoint() {
        let root = temp_root("install-auth-ok");
        fs::create_dir_all(&root).unwrap();

        let tool_tar = build_tarball(&[(
            "agent.json",
            serde_json::to_string_pretty(&json!({
                "kind":"tool",
                "name":"private-tool",
                "version":"0.1.0",
                "description":"Private",
                "entrypoint":{"command":"python","args":["tool.py"]},
                "runtime":{"type":"python","version":"3.12"},
                "inputs":{},
                "outputs":{},
                "files":["tool.py"]
            }))
            .unwrap(),
        )]);

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr: SocketAddr = listener.local_addr().unwrap();
        let base_url = format!("http://127.0.0.1:{}", addr.port());
        let state = Arc::new(AuthInstallTestState {
            base_url: base_url.clone(),
            expected_auth: "Bearer secret-token".to_string(),
            tool_tar: tool_tar.clone(),
            tool_sha: sha_hex(&tool_tar),
        });
        let app = Router::new()
            .route("/v1/tools/install/resolve", post(auth_test_resolve))
            .route("/v1/tools/install/init", post(auth_test_init))
            .route("/v1/tools/install/finalize", post(test_finalize))
            .route("/artifact/private-tool", get(get_auth_tool))
            .with_state(state);
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        let args = InstallArgs {
            manifest: root.join("agent.json").to_string_lossy().into_owned(),
            spec: Some("@zack/private-tool@0.1.0".to_string()),
            frozen: false,
            refresh: false,
            update_range: false,
            quiet: true,
            require_attestation: false,
            token: Some("secret-token".to_string()),
        };

        args.run(base_url).await.unwrap();

        server.abort();
        let _ = fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn direct_install_surfaces_unauthorized_when_private_resolve_rejects() {
        let root = temp_root("install-auth-401");
        fs::create_dir_all(&root).unwrap();

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr: SocketAddr = listener.local_addr().unwrap();
        let base_url = format!("http://127.0.0.1:{}", addr.port());
        let state = Arc::new(AuthInstallTestState {
            base_url: base_url.clone(),
            expected_auth: "Bearer secret-token".to_string(),
            tool_tar: Vec::new(),
            tool_sha: String::new(),
        });
        let app = Router::new()
            .route("/v1/tools/install/resolve", post(auth_test_resolve))
            .with_state(state);
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        let args = InstallArgs {
            manifest: root.join("agent.json").to_string_lossy().into_owned(),
            spec: Some("@zack/private-tool@0.1.0".to_string()),
            frozen: false,
            refresh: false,
            update_range: false,
            quiet: true,
            require_attestation: false,
            token: None,
        };

        let err = args.run(base_url).await.unwrap_err().to_string();
        assert!(err.contains("unauthorized"));

        server.abort();
        let _ = fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn direct_skill_install_updates_local_agent_manifest_skills_field() {
        let root = temp_root("install-local-agent-skill");
        fs::create_dir_all(&root).unwrap();
        crate::manifest::write_manifest_pretty_atomic(
            &root.join("agent.json"),
            &json!({
                "kind": "agent",
                "name": "support-agent",
                "version": "0.1.0",
                "tools": [{"name":"@zack/echo","version":"0.1.0"}],
                "skills": []
            }),
        )
        .unwrap();
        crate::manifest::write_lock(
            &root,
            &Lock::V2(LockV2 {
                lockfile_version: 2,
                generated: Utc::now(),
                packages: BTreeMap::from([(
                    "tool:@zack/echo@0.1.0".to_string(),
                    LockedPackage {
                        kind: PackageKind::Tool,
                        name: "@zack/echo".to_string(),
                        version: "0.1.0".to_string(),
                        integrity: "sha256-echo".to_string(),
                    },
                )]),
                roots: BTreeMap::new(),
            }),
        )
        .unwrap();

        let skill_tar = build_tarball(&[
            (
                "agent.json",
                serde_json::to_string_pretty(&json!({
                    "kind":"skill",
                    "name":"triage-skill",
                    "version":"0.1.0",
                    "description":"Skill",
                    "skill":{"entrypoint":"SKILL.md"},
                    "tools":[]
                }))
                .unwrap(),
            ),
            ("SKILL.md", "# Skill\n".to_string()),
        ]);

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr: SocketAddr = listener.local_addr().unwrap();
        let base_url = format!("http://127.0.0.1:{}", addr.port());
        let state = Arc::new(DirectSkillInstallTestState {
            base_url: base_url.clone(),
            skill_tar: skill_tar.clone(),
            skill_sha: sha_hex(&skill_tar),
        });
        let app = Router::new()
            .route("/v1/tools/install/resolve", post(direct_skill_resolve))
            .route("/v1/tools/install/init", post(direct_skill_init))
            .route("/v1/tools/install/finalize", post(test_finalize))
            .route("/artifact/direct-skill", get(get_direct_skill))
            .with_state(state);
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        let args = InstallArgs {
            manifest: root.join("agent.json").to_string_lossy().into_owned(),
            spec: Some("@zack/triage-skill@0.1.0".to_string()),
            frozen: false,
            refresh: false,
            update_range: false,
            quiet: true,
            require_attestation: false,
            token: None,
        };

        args.run(base_url).await.unwrap();

        let manifest_after: Value =
            serde_json::from_str(&fs::read_to_string(root.join("agent.json")).unwrap()).unwrap();
        assert_eq!(
            manifest_after["tools"],
            json!([{"name":"@zack/echo","version":"0.1.0"}])
        );
        assert_eq!(
            manifest_after["skills"],
            json!([{"name":"@zack/triage-skill","version":"0.1.0"}])
        );
        assert_eq!(
            fs::read_to_string(
                installed_skill_manifest_path(
                    &root.join(".agentpm"),
                    "@zack/triage-skill",
                    "0.1.0",
                )
                .unwrap()
                .parent()
                .unwrap()
                .join("SKILL.md"),
            )
            .unwrap(),
            "# Skill\n"
        );

        server.abort();
        let _ = fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn direct_knowledge_install_updates_local_agent_manifest_without_standalone_root() {
        let root = temp_root("install-local-agent-knowledge");
        fs::create_dir_all(&root).unwrap();
        crate::manifest::write_manifest_pretty_atomic(
            &root.join("agent.json"),
            &json!({
                "kind": "agent",
                "name": "research-agent",
                "version": "0.1.0",
                "tools": [{"name":"@zack/echo","version":"0.1.0"}],
                "knowledge": []
            }),
        )
        .unwrap();
        crate::manifest::write_lock(
            &root,
            &Lock::V2(LockV2 {
                lockfile_version: 2,
                generated: Utc::now(),
                packages: BTreeMap::from([(
                    "tool:@zack/echo@0.1.0".to_string(),
                    LockedPackage {
                        kind: PackageKind::Tool,
                        name: "@zack/echo".to_string(),
                        version: "0.1.0".to_string(),
                        integrity: "sha256-echo".to_string(),
                    },
                )]),
                roots: BTreeMap::new(),
            }),
        )
        .unwrap();

        let knowledge_tar = build_tarball(&[
            (
                "agent.json",
                serde_json::to_string_pretty(&json!({
                    "kind":"knowledge",
                    "name":"sample-docs",
                    "version":"0.1.0",
                    "description":"Knowledge",
                    "knowledge":{
                        "mode":"context",
                        "documents":[{"path":"knowledge/docs/context.md"}]
                    }
                }))
                .unwrap(),
            ),
            ("knowledge/docs/context.md", "# Sample Docs\n".to_string()),
        ]);

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr: SocketAddr = listener.local_addr().unwrap();
        let base_url = format!("http://127.0.0.1:{}", addr.port());
        let state = Arc::new(DirectKnowledgeInstallTestState {
            base_url: base_url.clone(),
            knowledge_tar: knowledge_tar.clone(),
            knowledge_sha: sha_hex(&knowledge_tar),
        });
        let app = Router::new()
            .route("/v1/tools/install/resolve", post(direct_knowledge_resolve))
            .route("/v1/tools/install/init", post(direct_knowledge_init))
            .route("/v1/tools/install/finalize", post(test_finalize))
            .route("/artifact/direct-knowledge", get(get_direct_knowledge))
            .with_state(state);
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        let args = InstallArgs {
            manifest: root.join("agent.json").to_string_lossy().into_owned(),
            spec: Some("@zack/sample-docs@0.1.0".to_string()),
            frozen: false,
            refresh: false,
            update_range: false,
            quiet: true,
            require_attestation: false,
            token: None,
        };

        args.run(base_url).await.unwrap();

        let manifest_after: Value =
            serde_json::from_str(&fs::read_to_string(root.join("agent.json")).unwrap()).unwrap();
        assert_eq!(
            manifest_after["tools"],
            json!([{"name":"@zack/echo","version":"0.1.0"}])
        );
        assert_eq!(
            manifest_after["knowledge"],
            json!([{"name":"@zack/sample-docs","version":"0.1.0"}])
        );

        let lock_after = crate::manifest::read_lock_or_default(&root).unwrap();
        let Lock::V2(lock_after) = lock_after else {
            panic!("expected v2 lock");
        };
        assert_eq!(lock_after.lockfile_version, 3);
        assert!(
            lock_after
                .packages
                .contains_key("knowledge:@zack/sample-docs@0.1.0")
        );
        assert!(
            !lock_after
                .roots
                .contains_key("knowledge:@zack/sample-docs@0.1.0")
        );
        assert_eq!(
            lock_after
                .roots
                .get("local:agent")
                .expect("local agent root missing")
                .knowledge,
            vec!["knowledge:@zack/sample-docs@0.1.0".to_string()]
        );
        assert!(
            installed_knowledge_manifest_path(&root.join(".agentpm"), "@zack/sample-docs", "0.1.0")
                .unwrap()
                .exists()
        );

        server.abort();
        let _ = fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn direct_memory_install_updates_local_agent_manifest_without_standalone_root() {
        let root = temp_root("install-local-agent-memory");
        fs::create_dir_all(&root).unwrap();
        crate::manifest::write_manifest_pretty_atomic(
            &root.join("agent.json"),
            &json!({
                "kind": "agent",
                "name": "support-agent",
                "version": "0.1.0",
                "tools": [{"name":"@zack/echo","version":"0.1.0"}],
                "memory": []
            }),
        )
        .unwrap();
        crate::manifest::write_lock(
            &root,
            &Lock::V2(LockV2 {
                lockfile_version: 2,
                generated: Utc::now(),
                packages: BTreeMap::from([(
                    "tool:@zack/echo@0.1.0".to_string(),
                    LockedPackage {
                        kind: PackageKind::Tool,
                        name: "@zack/echo".to_string(),
                        version: "0.1.0".to_string(),
                        integrity: "sha256-echo".to_string(),
                    },
                )]),
                roots: BTreeMap::new(),
            }),
        )
        .unwrap();

        let memory_tar = build_tarball(&[
            (
                "agent.json",
                serde_json::to_string_pretty(&json!({
                    "kind":"memory",
                    "name":"session-memory",
                    "version":"0.1.0",
                    "description":"Memory",
                    "memory":{
                        "scopes":{"user":{"description":"User scope."}},
                        "record_types":{
                            "user_preference":{
                                "description":"Preference record.",
                                "schema":"schemas/user-preference.schema.json",
                                "version":"1.0.0"
                            }
                        },
                        "spaces":{
                            "profile":{
                                "description":"Profile space.",
                                "model":"document",
                                "scope":["user"],
                                "record_types":["user_preference"],
                                "retrieval":{"modes":["key"]}
                            }
                        }
                    }
                }))
                .unwrap(),
            ),
            (
                "schemas/user-preference.schema.json",
                serde_json::to_string_pretty(&json!({
                    "$schema":"https://json-schema.org/draft/2020-12/schema",
                    "type":"object"
                }))
                .unwrap(),
            ),
            (
                "memory/build.json",
                serde_json::to_string_pretty(&json!({
                    "type":"agentpm-memory-contracts",
                    "format_version":1,
                    "manifest_path":"agent.json",
                    "source_manifest_hash":"sha256:manifest",
                    "source_schemas_hash":"sha256:schemas",
                    "source_contract_inputs_hash":"sha256:inputs",
                    "contracts_index_hash":"sha256:index",
                    "contracts_hash":"sha256:contracts",
                    "contract_count":1,
                    "source_schemas":[{
                        "path":"schemas/user-preference.schema.json",
                        "sha256":"sha256:schema-file"
                    }]
                }))
                .unwrap(),
            ),
            (
                "memory/contracts/index.json",
                serde_json::to_string_pretty(&json!({
                    "type":"agentpm-memory-contract-index",
                    "format_version":1,
                    "contracts":[{
                        "space":"profile",
                        "record_type":"user_preference",
                        "schema_version":"1.0.0",
                        "path":"memory/contracts/profile.user_preference.schema.json",
                        "sha256":"sha256:contract-file"
                    }]
                }))
                .unwrap(),
            ),
            (
                "memory/contracts/profile.user_preference.schema.json",
                serde_json::to_string_pretty(&json!({
                    "$schema":"https://json-schema.org/draft/2020-12/schema",
                    "type":"object",
                    "properties":{"id":{"type":"string"}},
                    "required":["id"]
                }))
                .unwrap(),
            ),
        ]);

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr: SocketAddr = listener.local_addr().unwrap();
        let base_url = format!("http://127.0.0.1:{}", addr.port());
        let state = Arc::new(DirectMemoryInstallTestState {
            base_url: base_url.clone(),
            memory_tar: memory_tar.clone(),
            memory_sha: sha_hex(&memory_tar),
        });
        let app = Router::new()
            .route("/v1/tools/install/resolve", post(direct_memory_resolve))
            .route("/v1/tools/install/init", post(direct_memory_init))
            .route("/v1/tools/install/finalize", post(test_finalize))
            .route("/artifact/direct-memory", get(get_direct_memory))
            .with_state(state);
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        let args = InstallArgs {
            manifest: root.join("agent.json").to_string_lossy().into_owned(),
            spec: Some("@zack/session-memory@0.1.0".to_string()),
            frozen: false,
            refresh: false,
            update_range: false,
            quiet: true,
            require_attestation: false,
            token: None,
        };

        args.run(base_url).await.unwrap();

        let manifest_after: Value =
            serde_json::from_str(&fs::read_to_string(root.join("agent.json")).unwrap()).unwrap();
        assert_eq!(
            manifest_after["memory"],
            json!([{"name":"@zack/session-memory","version":"0.1.0"}])
        );

        let lock_after = crate::manifest::read_lock_or_default(&root).unwrap();
        let Lock::V2(lock_after) = lock_after else {
            panic!("expected v2 lock");
        };
        assert_eq!(lock_after.lockfile_version, 3);
        assert!(
            lock_after
                .packages
                .contains_key("memory:@zack/session-memory@0.1.0")
        );
        assert!(
            !lock_after
                .roots
                .contains_key("memory:@zack/session-memory@0.1.0")
        );
        assert_eq!(
            lock_after
                .roots
                .get("local:agent")
                .expect("local agent root missing")
                .memory,
            vec!["memory:@zack/session-memory@0.1.0".to_string()]
        );
        let installed_manifest =
            installed_memory_manifest_path(&root.join(".agentpm"), "@zack/session-memory", "0.1.0")
                .unwrap();
        let installed_root = installed_manifest.parent().expect("installed memory root");
        assert!(installed_manifest.exists());
        assert!(
            installed_root
                .join("schemas/user-preference.schema.json")
                .exists()
        );
        assert!(installed_root.join("memory/build.json").exists());
        assert!(installed_root.join("memory/contracts/index.json").exists());
        assert!(
            installed_root
                .join("memory/contracts/profile.user_preference.schema.json")
                .exists()
        );

        server.abort();
        let _ = fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn direct_profile_install_updates_local_agent_manifest_without_standalone_root() {
        let root = temp_root("install-local-agent-profile");
        fs::create_dir_all(&root).unwrap();
        crate::manifest::write_manifest_pretty_atomic(
            &root.join("agent.json"),
            &json!({
                "kind": "agent",
                "name": "support-agent",
                "version": "0.1.0",
                "tools": [{"name":"@zack/echo","version":"0.1.0"}],
                "profiles": []
            }),
        )
        .unwrap();
        crate::manifest::write_lock(
            &root,
            &Lock::V2(LockV2 {
                lockfile_version: 2,
                generated: Utc::now(),
                packages: BTreeMap::from([(
                    "tool:@zack/echo@0.1.0".to_string(),
                    LockedPackage {
                        kind: PackageKind::Tool,
                        name: "@zack/echo".to_string(),
                        version: "0.1.0".to_string(),
                        integrity: "sha256-echo".to_string(),
                    },
                )]),
                roots: BTreeMap::new(),
            }),
        )
        .unwrap();

        let profile_tar = build_tarball(&[(
            "agent.json",
            serde_json::to_string_pretty(&json!({
                "kind":"profile",
                "name":"support-persona",
                "version":"0.1.0",
                "description":"Profile",
                "profile":{
                    "identity":{"role":"Support specialist"},
                    "communication":{"tone":["calm","clear"]}
                }
            }))
            .unwrap(),
        )]);

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr: SocketAddr = listener.local_addr().unwrap();
        let base_url = format!("http://127.0.0.1:{}", addr.port());
        let state = Arc::new(DirectProfileInstallTestState {
            base_url: base_url.clone(),
            profile_tar: profile_tar.clone(),
            profile_sha: sha_hex(&profile_tar),
        });
        let app = Router::new()
            .route("/v1/tools/install/resolve", post(direct_profile_resolve))
            .route("/v1/tools/install/init", post(direct_profile_init))
            .route("/v1/tools/install/finalize", post(test_finalize))
            .route("/artifact/direct-profile", get(get_direct_profile))
            .with_state(state);
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        let args = InstallArgs {
            manifest: root.join("agent.json").to_string_lossy().into_owned(),
            spec: Some("@zack/support-persona@0.1.0".to_string()),
            frozen: false,
            refresh: false,
            update_range: false,
            quiet: true,
            require_attestation: false,
            token: None,
        };

        args.run(base_url).await.unwrap();

        let manifest_after: Value =
            serde_json::from_str(&fs::read_to_string(root.join("agent.json")).unwrap()).unwrap();
        assert_eq!(
            manifest_after["profiles"],
            json!([{"name":"@zack/support-persona","version":"0.1.0"}])
        );

        let lock_after = crate::manifest::read_lock_or_default(&root).unwrap();
        let Lock::V2(lock_after) = lock_after else {
            panic!("expected v2 lock");
        };
        assert_eq!(lock_after.lockfile_version, 3);
        assert!(
            lock_after
                .packages
                .contains_key("profile:@zack/support-persona@0.1.0")
        );
        assert!(
            !lock_after
                .roots
                .contains_key("profile:@zack/support-persona@0.1.0")
        );
        assert_eq!(
            lock_after
                .roots
                .get("local:agent")
                .expect("local agent root missing")
                .profiles,
            vec!["profile:@zack/support-persona@0.1.0".to_string()]
        );
        assert!(
            installed_profile_manifest_path(
                &root.join(".agentpm"),
                "@zack/support-persona",
                "0.1.0",
            )
            .unwrap()
            .exists()
        );

        server.abort();
        let _ = fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn direct_loop_install_updates_local_agent_manifest_without_standalone_root() {
        let root = temp_root("install-local-agent-loop");
        fs::create_dir_all(&root).unwrap();
        crate::manifest::write_manifest_pretty_atomic(
            &root.join("agent.json"),
            &json!({
                "kind": "agent",
                "name": "support-agent",
                "version": "0.1.0",
                "tools": [{"name":"@zack/echo","version":"0.1.0"}],
                "loop": null
            }),
        )
        .unwrap();
        crate::manifest::write_lock(
            &root,
            &Lock::V2(LockV2 {
                lockfile_version: 2,
                generated: Utc::now(),
                packages: BTreeMap::from([(
                    "tool:@zack/echo@0.1.0".to_string(),
                    LockedPackage {
                        kind: PackageKind::Tool,
                        name: "@zack/echo".to_string(),
                        version: "0.1.0".to_string(),
                        integrity: "sha256-echo".to_string(),
                    },
                )]),
                roots: BTreeMap::new(),
            }),
        )
        .unwrap();

        let loop_tar = build_tarball(&[(
            "agent.json",
            serde_json::to_string_pretty(&json!({
                "kind":"loop",
                "name":"incident-response-loop",
                "version":"0.1.0",
                "description":"Loop",
                "loop":{
                    "entry_phase":"triage",
                    "phases":[
                        {
                            "id":"triage",
                            "objective":"Review the incoming incident context and choose the next action.",
                            "outcomes":[
                                {
                                    "id":"needs-investigation",
                                    "description":"More evidence is required before the loop can finish."
                                },
                                {
                                    "id":"complete",
                                    "description":"The loop gathered enough context to end successfully."
                                },
                                {
                                    "id":"handoff",
                                    "description":"The loop should transfer control to a human or another system."
                                }
                            ]
                        },
                        {
                            "id":"investigate",
                            "objective":"Gather missing evidence before finalizing the response.",
                            "outcomes":[
                                {
                                    "id":"complete",
                                    "description":"The missing evidence has been collected."
                                },
                                {
                                    "id":"handoff",
                                    "description":"Investigation cannot continue without external intervention."
                                }
                            ]
                        }
                    ],
                    "transitions":[
                        {"from":"triage","on":"needs-investigation","to":"investigate"},
                        {"from":"triage","on":"complete","to":"$end"},
                        {"from":"triage","on":"handoff","to":"$handoff"},
                        {"from":"investigate","on":"complete","to":"$end"},
                        {"from":"investigate","on":"handoff","to":"$handoff"}
                    ]
                }
            }))
            .unwrap(),
        )]);

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr: SocketAddr = listener.local_addr().unwrap();
        let base_url = format!("http://127.0.0.1:{}", addr.port());
        let state = Arc::new(DirectLoopInstallTestState {
            base_url: base_url.clone(),
            loop_tar: loop_tar.clone(),
            loop_sha: sha_hex(&loop_tar),
        });
        let app = Router::new()
            .route("/v1/tools/install/resolve", post(direct_loop_resolve))
            .route("/v1/tools/install/init", post(direct_loop_init))
            .route("/v1/tools/install/finalize", post(test_finalize))
            .route("/artifact/direct-loop", get(get_direct_loop))
            .with_state(state);
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        let args = InstallArgs {
            manifest: root.join("agent.json").to_string_lossy().into_owned(),
            spec: Some("@zack/incident-response-loop@0.1.0".to_string()),
            frozen: false,
            refresh: false,
            update_range: false,
            quiet: true,
            require_attestation: false,
            token: None,
        };

        args.run(base_url).await.unwrap();

        let manifest_after: Value =
            serde_json::from_str(&fs::read_to_string(root.join("agent.json")).unwrap()).unwrap();
        assert_eq!(
            manifest_after["loop"],
            json!({"name":"@zack/incident-response-loop","version":"0.1.0"})
        );

        let lock_after = crate::manifest::read_lock_or_default(&root).unwrap();
        let Lock::V2(lock_after) = lock_after else {
            panic!("expected v2 lock");
        };
        assert_eq!(lock_after.lockfile_version, 3);
        assert!(
            lock_after
                .packages
                .contains_key("loop:@zack/incident-response-loop@0.1.0")
        );
        assert!(
            !lock_after
                .roots
                .contains_key("loop:@zack/incident-response-loop@0.1.0")
        );
        let local_root = lock_after
            .roots
            .get("local:agent")
            .expect("local agent root missing");
        assert_eq!(local_root.tools, vec!["tool:@zack/echo@0.1.0".to_string()]);
        assert!(local_root.skills.is_empty());
        assert!(local_root.knowledge.is_empty());
        assert!(local_root.memory.is_empty());
        assert_eq!(
            local_root.r#loop.as_deref(),
            Some("loop:@zack/incident-response-loop@0.1.0")
        );
        assert!(local_root.profiles.is_empty());
        assert!(
            installed_loop_manifest_path(
                &root.join(".agentpm"),
                "@zack/incident-response-loop",
                "0.1.0",
            )
            .unwrap()
            .exists()
        );

        server.abort();
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn direct_local_agent_loop_replacement_prunes_old_loop_when_unreachable() {
        let root = temp_root("replace-local-agent-loop");
        let existing = Lock::V2(LockV2 {
            lockfile_version: 3,
            generated: Utc::now(),
            packages: BTreeMap::from([
                (
                    "loop:@zack/incident-response-loop@0.1.0".to_string(),
                    LockedPackage {
                        kind: PackageKind::Loop,
                        name: "@zack/incident-response-loop".to_string(),
                        version: "0.1.0".to_string(),
                        integrity: "sha256-loop-old".to_string(),
                    },
                ),
                (
                    "loop:@zack/escalation-loop@0.1.0".to_string(),
                    LockedPackage {
                        kind: PackageKind::Loop,
                        name: "@zack/escalation-loop".to_string(),
                        version: "0.1.0".to_string(),
                        integrity: "sha256-loop-new".to_string(),
                    },
                ),
            ]),
            roots: BTreeMap::from([(
                "local:agent".to_string(),
                LockedRoot {
                    name: Some("support-agent".to_string()),
                    version: Some("0.1.0".to_string()),
                    tools: Vec::new(),
                    skills: Vec::new(),
                    knowledge: Vec::new(),
                    memory: Vec::new(),
                    profiles: Vec::new(),
                    r#loop: Some("loop:@zack/incident-response-loop@0.1.0".to_string()),
                    reserved: Default::default(),
                },
            )]),
        });

        let manifest_value = json!({
            "kind": "agent",
            "name": "support-agent",
            "version": "0.1.0",
            "loop": "@zack/escalation-loop@0.1.0"
        });
        let plan = ResolvePlan {
            items: vec![ResolvedPackage {
                kind: PackageKind::Loop,
                name: "@zack/escalation-loop".to_string(),
                version: "0.1.0".to_string(),
                integrity: "sha256-loop-new".to_string(),
            }],
        };

        let next = build_updated_lock(
            &existing,
            Some(&manifest_value),
            Some("@zack/escalation-loop@0.1.0"),
            &plan,
            &root,
        )
        .unwrap();

        let Lock::V2(lock) = next else {
            panic!("expected v2 lock");
        };
        assert_eq!(
            lock.roots
                .get("local:agent")
                .and_then(|root| root.r#loop.as_deref()),
            Some("loop:@zack/escalation-loop@0.1.0")
        );
        assert!(
            !lock
                .packages
                .contains_key("loop:@zack/incident-response-loop@0.1.0")
        );
        assert!(
            lock.packages
                .contains_key("loop:@zack/escalation-loop@0.1.0")
        );
    }

    #[test]
    fn direct_local_agent_tool_replacement_prunes_old_tool_when_unreachable() {
        let root = temp_root("replace-local-agent-tool");
        let existing = Lock::V2(LockV2 {
            lockfile_version: 3,
            generated: Utc::now(),
            packages: BTreeMap::from([
                (
                    "tool:@zack/echo@0.1.0".to_string(),
                    LockedPackage {
                        kind: PackageKind::Tool,
                        name: "@zack/echo".to_string(),
                        version: "0.1.0".to_string(),
                        integrity: "sha256-tool-old".to_string(),
                    },
                ),
                (
                    "tool:@zack/summarize@0.1.0".to_string(),
                    LockedPackage {
                        kind: PackageKind::Tool,
                        name: "@zack/summarize".to_string(),
                        version: "0.1.0".to_string(),
                        integrity: "sha256-tool-new".to_string(),
                    },
                ),
            ]),
            roots: BTreeMap::from([(
                "local:agent".to_string(),
                LockedRoot {
                    name: Some("support-agent".to_string()),
                    version: Some("0.1.0".to_string()),
                    tools: vec!["tool:@zack/echo@0.1.0".to_string()],
                    skills: Vec::new(),
                    knowledge: Vec::new(),
                    memory: Vec::new(),
                    profiles: Vec::new(),
                    r#loop: None,
                    reserved: Default::default(),
                },
            )]),
        });

        let manifest_value = json!({
            "kind": "agent",
            "name": "support-agent",
            "version": "0.1.0",
            "tools": ["@zack/summarize@0.1.0"]
        });
        let plan = ResolvePlan {
            items: vec![ResolvedPackage {
                kind: PackageKind::Tool,
                name: "@zack/summarize".to_string(),
                version: "0.1.0".to_string(),
                integrity: "sha256-tool-new".to_string(),
            }],
        };

        let next = build_updated_lock(
            &existing,
            Some(&manifest_value),
            Some("@zack/summarize@0.1.0"),
            &plan,
            &root,
        )
        .unwrap();

        let Lock::V2(lock) = next else {
            panic!("expected v2 lock");
        };
        assert_eq!(
            lock.roots.get("local:agent").map(|root| root.tools.clone()),
            Some(vec!["tool:@zack/summarize@0.1.0".to_string()])
        );
        assert!(!lock.packages.contains_key("tool:@zack/echo@0.1.0"));
        assert!(lock.packages.contains_key("tool:@zack/summarize@0.1.0"));
    }

    async fn assert_manifest_dependency_failure_happens_before_init_and_download(
        label: &str,
        manifest: Value,
        resolve_items: Value,
        expected_error: &str,
    ) {
        let root = temp_root(label);
        fs::create_dir_all(&root).unwrap();
        crate::manifest::write_manifest_pretty_atomic(&root.join("agent.json"), &manifest).unwrap();

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr: SocketAddr = listener.local_addr().unwrap();
        let base_url = format!("http://127.0.0.1:{}", addr.port());
        let state = Arc::new(ManifestDependencyFailureState { resolve_items });
        let app = Router::new()
            .route(
                "/v1/tools/install/resolve",
                post(manifest_dependency_failure_resolve),
            )
            .route(
                "/v1/tools/install/init",
                post(manifest_dependency_failure_init_should_not_run),
            )
            .with_state(state);
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        let args = InstallArgs {
            manifest: root.join("agent.json").to_string_lossy().into_owned(),
            spec: None,
            frozen: false,
            refresh: false,
            update_range: false,
            quiet: true,
            require_attestation: false,
            token: None,
        };

        let err = args.run(base_url).await.unwrap_err();
        let err_text = format!("{err:#}");
        assert!(err_text.contains(expected_error), "{err_text}");
        assert!(
            !err_text.contains("install_init_called_unexpectedly"),
            "{err_text}"
        );
        assert!(!root.join(".agentpm").exists());
        assert!(!root.join("agent.lock").exists());

        server.abort();
        let _ = fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn manifest_driven_profile_dependency_failure_happens_before_init_and_download() {
        assert_manifest_dependency_failure_happens_before_init_and_download(
            "install-local-agent-profile-wrong-kind",
            json!({
                "kind": "agent",
                "name": "support-agent",
                "version": "0.1.0",
                "profiles": ["@zack/capitalize-skill@*"]
            }),
            json!([{
                "kind": "skill",
                "name": "@zack/capitalize-skill",
                "version": "0.1.0",
                "integrity": "sha256-skill"
            }]),
            "declared profile dependency @zack/capitalize-skill@* is missing from the resolved package set",
        )
        .await;
    }

    #[tokio::test]
    async fn manifest_driven_tool_dependency_failure_happens_before_init_and_download() {
        assert_manifest_dependency_failure_happens_before_init_and_download(
            "install-local-agent-tool-wrong-kind",
            json!({
                "kind": "agent",
                "name": "support-agent",
                "version": "0.1.0",
                "tools": ["@zack/capitalize@*"]
            }),
            json!([{
                "kind": "skill",
                "name": "@zack/capitalize",
                "version": "0.1.0",
                "integrity": "sha256-skill"
            }]),
            "declared tool dependency @zack/capitalize@* is missing from the resolved package set",
        )
        .await;
    }

    #[tokio::test]
    async fn manifest_driven_memory_dependency_failure_happens_before_init_and_download() {
        assert_manifest_dependency_failure_happens_before_init_and_download(
            "install-local-agent-memory-wrong-kind",
            json!({
                "kind": "agent",
                "name": "support-agent",
                "version": "0.1.0",
                "memory": ["@zack/session-memory@*"]
            }),
            json!([{
                "kind": "knowledge",
                "name": "@zack/session-memory",
                "version": "0.1.0",
                "integrity": "sha256-knowledge"
            }]),
            "declared memory dependency @zack/session-memory@* is missing from the resolved package set",
        )
        .await;
    }

    fn temp_root(label: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("agentpm-install-{label}-{nanos}"))
    }

    #[derive(Clone)]
    struct InstallTestState {
        base_url: String,
        echo_tar: Vec<u8>,
        echo_sha: String,
        review_tar: Vec<u8>,
        review_sha: String,
        translate_tar: Vec<u8>,
        translate_sha: String,
        agent_tar: Vec<u8>,
        agent_sha: String,
    }

    #[derive(Clone)]
    struct AuthInstallTestState {
        base_url: String,
        expected_auth: String,
        tool_tar: Vec<u8>,
        tool_sha: String,
    }

    #[derive(Clone)]
    struct DirectSkillInstallTestState {
        base_url: String,
        skill_tar: Vec<u8>,
        skill_sha: String,
    }

    #[derive(Clone)]
    struct DirectKnowledgeInstallTestState {
        base_url: String,
        knowledge_tar: Vec<u8>,
        knowledge_sha: String,
    }

    #[derive(Clone)]
    struct DirectMemoryInstallTestState {
        base_url: String,
        memory_tar: Vec<u8>,
        memory_sha: String,
    }

    #[derive(Clone)]
    struct DirectProfileInstallTestState {
        base_url: String,
        profile_tar: Vec<u8>,
        profile_sha: String,
    }

    #[derive(Clone)]
    struct DirectLoopInstallTestState {
        base_url: String,
        loop_tar: Vec<u8>,
        loop_sha: String,
    }

    #[derive(Clone)]
    struct ManifestDependencyFailureState {
        resolve_items: Value,
    }

    async fn test_resolve(
        State(state): State<Arc<InstallTestState>>,
        body: String,
    ) -> Result<Response<Body>, StatusCode> {
        let req: Value = serde_json::from_str(&body).map_err(|_| StatusCode::BAD_REQUEST)?;
        let mut items = Vec::new();
        let empty = Vec::new();
        for item in req["items"].as_array().unwrap_or(&empty) {
            let kind = item["kind"].as_str().unwrap();
            let name = item["name"].as_str().unwrap();
            match (kind, name) {
                ("tool", "@zack/echo") => items.push(json!({
                    "kind":"tool","name":name,"version":"0.1.0","integrity":state.echo_sha
                })),
                ("tool", "@zack/review") => items.push(json!({
                    "kind":"tool","name":name,"version":"0.2.0","integrity":state.review_sha
                })),
                ("agent", "@zack/support-agent") => {
                    items.push(json!({
                        "kind":"agent","name":name,"version":"0.1.0","integrity":state.agent_sha
                    }));
                    items.push(json!({
                        "kind":"tool","name":"@zack/translate","version":"0.3.0","integrity":state.translate_sha
                    }));
                }
                _ => return Err(StatusCode::BAD_REQUEST),
            }
        }
        let response = json!({ "items": items });
        Ok(Response::builder()
            .status(StatusCode::OK)
            .header("content-type", "application/json")
            .body(Body::from(response.to_string()))
            .unwrap())
    }

    async fn test_init(
        State(state): State<Arc<InstallTestState>>,
        body: String,
    ) -> Result<Response<Body>, StatusCode> {
        let req: Value = serde_json::from_str(&body).map_err(|_| StatusCode::BAD_REQUEST)?;
        let mut artifacts = Vec::new();
        let empty = Vec::new();
        for item in req["items"].as_array().unwrap_or(&empty) {
            let kind = item["kind"].as_str().unwrap();
            let name = item["name"].as_str().unwrap();
            let (url, integrity) = match (kind, name) {
                ("tool", "@zack/echo") => (
                    format!("{}/artifact/echo", state.base_url),
                    state.echo_sha.as_str(),
                ),
                ("tool", "@zack/review") => (
                    format!("{}/artifact/review", state.base_url),
                    state.review_sha.as_str(),
                ),
                ("tool", "@zack/translate") => (
                    format!("{}/artifact/translate", state.base_url),
                    state.translate_sha.as_str(),
                ),
                ("agent", "@zack/support-agent") => (
                    format!("{}/artifact/agent", state.base_url),
                    state.agent_sha.as_str(),
                ),
                _ => return Err(StatusCode::BAD_REQUEST),
            };
            artifacts.push(json!({
                "kind":kind,
                "name":name,
                "version":item["version"],
                "integrity":integrity,
                "presigned_url":url,
                "size":12,
                "content_type":"application/gzip"
            }));
        }
        let response = json!({
            "session_id":"session-1",
            "expires_at":"2026-06-01T00:00:00Z",
            "artifacts":artifacts
        });
        Ok(Response::builder()
            .status(StatusCode::OK)
            .header("content-type", "application/json")
            .body(Body::from(response.to_string()))
            .unwrap())
    }

    async fn auth_test_resolve(
        State(state): State<Arc<AuthInstallTestState>>,
        headers: HeaderMap,
        body: String,
    ) -> Result<Response<Body>, StatusCode> {
        let auth = headers
            .get(AUTHORIZATION)
            .and_then(|value| value.to_str().ok());
        if auth != Some(state.expected_auth.as_str()) {
            return Ok(Response::builder()
                .status(StatusCode::UNAUTHORIZED)
                .body(Body::empty())
                .unwrap());
        }

        let req: Value = serde_json::from_str(&body).map_err(|_| StatusCode::BAD_REQUEST)?;
        let item = &req["items"][0];
        let response = json!({
            "items": [{
                "kind": item["kind"],
                "name": item["name"],
                "version": "0.1.0",
                "integrity": state.tool_sha,
            }]
        });
        Ok(Response::builder()
            .status(StatusCode::OK)
            .header("content-type", "application/json")
            .body(Body::from(response.to_string()))
            .unwrap())
    }

    async fn auth_test_init(
        State(state): State<Arc<AuthInstallTestState>>,
        body: String,
    ) -> Result<Response<Body>, StatusCode> {
        let req: Value = serde_json::from_str(&body).map_err(|_| StatusCode::BAD_REQUEST)?;
        let item = &req["items"][0];
        let response = json!({
            "session_id":"session-1",
            "expires_at":"2026-06-01T00:00:00Z",
            "artifacts": [{
                "kind": item["kind"],
                "name": item["name"],
                "version": item["version"],
                "integrity": state.tool_sha,
                "presigned_url": format!("{}/artifact/private-tool", state.base_url),
                "size": 12,
                "content_type":"application/gzip"
            }]
        });
        Ok(Response::builder()
            .status(StatusCode::OK)
            .header("content-type", "application/json")
            .body(Body::from(response.to_string()))
            .unwrap())
    }

    async fn test_finalize() -> StatusCode {
        StatusCode::NO_CONTENT
    }

    async fn direct_skill_resolve(
        State(state): State<Arc<DirectSkillInstallTestState>>,
        body: String,
    ) -> Result<Response<Body>, StatusCode> {
        let req: Value = serde_json::from_str(&body).map_err(|_| StatusCode::BAD_REQUEST)?;
        let item = &req["items"][0];
        let response = json!({
            "items": [{
                "kind": "skill",
                "name": item["name"],
                "version": "0.1.0",
                "integrity": state.skill_sha,
            }]
        });
        Ok(Response::builder()
            .status(StatusCode::OK)
            .header("content-type", "application/json")
            .body(Body::from(response.to_string()))
            .unwrap())
    }

    async fn direct_knowledge_resolve(
        State(state): State<Arc<DirectKnowledgeInstallTestState>>,
        body: String,
    ) -> Result<Response<Body>, StatusCode> {
        let req: Value = serde_json::from_str(&body).map_err(|_| StatusCode::BAD_REQUEST)?;
        let item = &req["items"][0];
        let response = json!({
            "items": [{
                "kind": "knowledge",
                "name": item["name"],
                "version": "0.1.0",
                "integrity": state.knowledge_sha,
            }]
        });
        Ok(Response::builder()
            .status(StatusCode::OK)
            .header("content-type", "application/json")
            .body(Body::from(response.to_string()))
            .unwrap())
    }

    async fn direct_skill_init(
        State(state): State<Arc<DirectSkillInstallTestState>>,
        body: String,
    ) -> Result<Response<Body>, StatusCode> {
        let req: Value = serde_json::from_str(&body).map_err(|_| StatusCode::BAD_REQUEST)?;
        let item = &req["items"][0];
        let response = json!({
            "session_id":"session-1",
            "expires_at":"2026-06-01T00:00:00Z",
            "artifacts": [{
                "kind": "skill",
                "name": item["name"],
                "version": item["version"],
                "integrity": state.skill_sha,
                "presigned_url": format!("{}/artifact/direct-skill", state.base_url),
                "size": 12,
                "content_type":"application/gzip"
            }]
        });
        Ok(Response::builder()
            .status(StatusCode::OK)
            .header("content-type", "application/json")
            .body(Body::from(response.to_string()))
            .unwrap())
    }

    async fn direct_knowledge_init(
        State(state): State<Arc<DirectKnowledgeInstallTestState>>,
        body: String,
    ) -> Result<Response<Body>, StatusCode> {
        let req: Value = serde_json::from_str(&body).map_err(|_| StatusCode::BAD_REQUEST)?;
        let item = &req["items"][0];
        let response = json!({
            "session_id":"session-1",
            "expires_at":"2026-06-01T00:00:00Z",
            "artifacts": [{
                "kind": "knowledge",
                "name": item["name"],
                "version": item["version"],
                "integrity": state.knowledge_sha,
                "presigned_url": format!("{}/artifact/direct-knowledge", state.base_url),
                "size": 12,
                "content_type":"application/gzip"
            }]
        });
        Ok(Response::builder()
            .status(StatusCode::OK)
            .header("content-type", "application/json")
            .body(Body::from(response.to_string()))
            .unwrap())
    }

    async fn direct_memory_resolve(
        State(state): State<Arc<DirectMemoryInstallTestState>>,
        body: String,
    ) -> Result<Response<Body>, StatusCode> {
        let req: Value = serde_json::from_str(&body).map_err(|_| StatusCode::BAD_REQUEST)?;
        let item = &req["items"][0];
        let response = json!({
            "items": [{
                "kind": "memory",
                "name": item["name"],
                "version": "0.1.0",
                "integrity": state.memory_sha,
            }]
        });
        Ok(Response::builder()
            .status(StatusCode::OK)
            .header("content-type", "application/json")
            .body(Body::from(response.to_string()))
            .unwrap())
    }

    async fn direct_profile_resolve(
        State(state): State<Arc<DirectProfileInstallTestState>>,
        body: String,
    ) -> Result<Response<Body>, StatusCode> {
        let req: Value = serde_json::from_str(&body).map_err(|_| StatusCode::BAD_REQUEST)?;
        let item = &req["items"][0];
        let response = json!({
            "items": [{
                "kind": "profile",
                "name": item["name"],
                "version": "0.1.0",
                "integrity": state.profile_sha,
            }]
        });
        Ok(Response::builder()
            .status(StatusCode::OK)
            .header("content-type", "application/json")
            .body(Body::from(response.to_string()))
            .unwrap())
    }

    async fn direct_loop_resolve(
        State(state): State<Arc<DirectLoopInstallTestState>>,
        body: String,
    ) -> Result<Response<Body>, StatusCode> {
        let req: Value = serde_json::from_str(&body).map_err(|_| StatusCode::BAD_REQUEST)?;
        let item = &req["items"][0];
        let response = json!({
            "items": [{
                "kind": "loop",
                "name": item["name"],
                "version": "0.1.0",
                "integrity": state.loop_sha,
            }]
        });
        Ok(Response::builder()
            .status(StatusCode::OK)
            .header("content-type", "application/json")
            .body(Body::from(response.to_string()))
            .unwrap())
    }

    async fn manifest_dependency_failure_resolve(
        State(state): State<Arc<ManifestDependencyFailureState>>,
        body: String,
    ) -> Result<Response<Body>, StatusCode> {
        let response = json!({
            "items": state.resolve_items
        });
        let _req: Value = serde_json::from_str(&body).map_err(|_| StatusCode::BAD_REQUEST)?;
        Ok(Response::builder()
            .status(StatusCode::OK)
            .header("content-type", "application/json")
            .body(Body::from(response.to_string()))
            .unwrap())
    }

    async fn manifest_dependency_failure_init_should_not_run() -> Response<Body> {
        Response::builder()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .body(Body::from("install_init_called_unexpectedly"))
            .unwrap()
    }

    async fn direct_memory_init(
        State(state): State<Arc<DirectMemoryInstallTestState>>,
        body: String,
    ) -> Result<Response<Body>, StatusCode> {
        let req: Value = serde_json::from_str(&body).map_err(|_| StatusCode::BAD_REQUEST)?;
        let item = &req["items"][0];
        let response = json!({
            "session_id":"session-1",
            "expires_at":"2026-06-01T00:00:00Z",
            "artifacts": [{
                "kind": "memory",
                "name": item["name"],
                "version": item["version"],
                "integrity": state.memory_sha,
                "presigned_url": format!("{}/artifact/direct-memory", state.base_url),
                "size": 12,
                "content_type":"application/gzip"
            }]
        });
        Ok(Response::builder()
            .status(StatusCode::OK)
            .header("content-type", "application/json")
            .body(Body::from(response.to_string()))
            .unwrap())
    }

    async fn direct_profile_init(
        State(state): State<Arc<DirectProfileInstallTestState>>,
        body: String,
    ) -> Result<Response<Body>, StatusCode> {
        let req: Value = serde_json::from_str(&body).map_err(|_| StatusCode::BAD_REQUEST)?;
        let item = &req["items"][0];
        let response = json!({
            "session_id":"session-1",
            "expires_at":"2026-06-01T00:00:00Z",
            "artifacts": [{
                "kind": "profile",
                "name": item["name"],
                "version": item["version"],
                "integrity": state.profile_sha,
                "presigned_url": format!("{}/artifact/direct-profile", state.base_url),
                "size": 12,
                "content_type":"application/gzip"
            }]
        });
        Ok(Response::builder()
            .status(StatusCode::OK)
            .header("content-type", "application/json")
            .body(Body::from(response.to_string()))
            .unwrap())
    }

    async fn direct_loop_init(
        State(state): State<Arc<DirectLoopInstallTestState>>,
        body: String,
    ) -> Result<Response<Body>, StatusCode> {
        let req: Value = serde_json::from_str(&body).map_err(|_| StatusCode::BAD_REQUEST)?;
        let item = &req["items"][0];
        let response = json!({
            "session_id":"session-1",
            "expires_at":"2026-06-01T00:00:00Z",
            "artifacts": [{
                "kind": "loop",
                "name": item["name"],
                "version": item["version"],
                "integrity": state.loop_sha,
                "presigned_url": format!("{}/artifact/direct-loop", state.base_url),
                "size": 12,
                "content_type":"application/gzip"
            }]
        });
        Ok(Response::builder()
            .status(StatusCode::OK)
            .header("content-type", "application/json")
            .body(Body::from(response.to_string()))
            .unwrap())
    }

    async fn get_echo(State(state): State<Arc<InstallTestState>>) -> Response<Body> {
        Response::builder()
            .status(StatusCode::OK)
            .body(Body::from(state.echo_tar.clone()))
            .unwrap()
    }

    async fn get_review(State(state): State<Arc<InstallTestState>>) -> Response<Body> {
        Response::builder()
            .status(StatusCode::OK)
            .body(Body::from(state.review_tar.clone()))
            .unwrap()
    }

    async fn get_translate(State(state): State<Arc<InstallTestState>>) -> Response<Body> {
        Response::builder()
            .status(StatusCode::OK)
            .body(Body::from(state.translate_tar.clone()))
            .unwrap()
    }

    async fn get_agent(State(state): State<Arc<InstallTestState>>) -> Response<Body> {
        Response::builder()
            .status(StatusCode::OK)
            .body(Body::from(state.agent_tar.clone()))
            .unwrap()
    }

    async fn get_auth_tool(State(state): State<Arc<AuthInstallTestState>>) -> Response<Body> {
        Response::builder()
            .status(StatusCode::OK)
            .body(Body::from(state.tool_tar.clone()))
            .unwrap()
    }

    async fn get_direct_skill(
        State(state): State<Arc<DirectSkillInstallTestState>>,
    ) -> Response<Body> {
        Response::builder()
            .status(StatusCode::OK)
            .body(Body::from(state.skill_tar.clone()))
            .unwrap()
    }

    async fn get_direct_knowledge(
        State(state): State<Arc<DirectKnowledgeInstallTestState>>,
    ) -> Response<Body> {
        Response::builder()
            .status(StatusCode::OK)
            .body(Body::from(state.knowledge_tar.clone()))
            .unwrap()
    }

    async fn get_direct_memory(
        State(state): State<Arc<DirectMemoryInstallTestState>>,
    ) -> Response<Body> {
        Response::builder()
            .status(StatusCode::OK)
            .body(Body::from(state.memory_tar.clone()))
            .unwrap()
    }

    async fn get_direct_profile(
        State(state): State<Arc<DirectProfileInstallTestState>>,
    ) -> Response<Body> {
        Response::builder()
            .status(StatusCode::OK)
            .body(Body::from(state.profile_tar.clone()))
            .unwrap()
    }

    async fn get_direct_loop(
        State(state): State<Arc<DirectLoopInstallTestState>>,
    ) -> Response<Body> {
        Response::builder()
            .status(StatusCode::OK)
            .header("content-type", "application/gzip")
            .body(Body::from(state.loop_tar.clone()))
            .unwrap()
    }

    fn build_tarball(files: &[(&str, String)]) -> Vec<u8> {
        let mut tar_bytes = Vec::new();
        {
            let mut builder = tar::Builder::new(&mut tar_bytes);
            for (path, contents) in files {
                let bytes = contents.as_bytes();
                let mut header = tar::Header::new_gnu();
                header.set_size(bytes.len() as u64);
                header.set_mode(0o644);
                header.set_cksum();
                builder.append_data(&mut header, *path, bytes).unwrap();
            }
            builder.finish().unwrap();
        }

        let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        use std::io::Write;
        encoder.write_all(&tar_bytes).unwrap();
        encoder.finish().unwrap()
    }

    fn sha_hex(bytes: &[u8]) -> String {
        use sha2::{Digest, Sha256};

        let mut hasher = Sha256::new();
        hasher.update(bytes);
        hex::encode(hasher.finalize())
    }
}
