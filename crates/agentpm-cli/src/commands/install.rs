use crate::manifest::{
    load_manifest_value, read_lock_or_default, write_lock, write_manifest_pretty_atomic,
};
use crate::prelude::*;
use crate::semver::adapt::{plan_to_sdk_resolve, to_sdk_request};
use crate::semver::types::{
    DesiredSet, Lock, LockRoot, LockedRoot, PackageKind, ReservedReferences, ResolvePlan,
    lock_from_plan, package_key, parse_package_spec,
};
use crate::semver::update::maybe_update_agent_json;
use crate::workspace::{
    build_workspace_lock, desired_from_workspace, load_workspace_local_manifests,
    read_workspace_metadata,
};
use crate::{io::download::download_and_extract_all, io::fs, ui::Step};
use anyhow::anyhow;
use chrono::Utc;
use semver::{Version, VersionReq};
use serde_json::Value;
use std::collections::BTreeMap;
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

        // 1) Load local manifest when present. Only manifest-driven installs require kind=agent.
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

            if manifest_value.get("kind").and_then(Value::as_str) != Some("agent") {
                return Err(anyhow!(
                    "`agentpm install` currently supports kind=agent only."
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
        fs::ensure_dirs(&[&dl_dir, &tools_dir, &agents_dir])?;
        download_and_extract_all(&init, &dl_dir, &tools_dir, &agents_dir, self.refresh, quiet)
            .await?;
        s.ok("");

        // 6) Finalize (report success for metrics / server-side bookkeeping)
        let mut s = Step::new("Finalizing install", quiet);
        client.install_finalize(&init.session_id).await?;
        s.ok("");

        // 7) If a direct tool spec was installed inside a local agent project and
        // it is not present in agent.json yet, append or update the declared range.
        // Direct agent package installs intentionally do not mutate the local
        // manifest; they are represented as registry roots in lockfile v2 instead.
        let mut should_write_manifest = false;
        if let Some(spec) = &self.spec
            && let Some(manifest_value) = manifest_value.as_mut()
            && manifest_value.get("kind").and_then(Value::as_str) == Some("agent")
            && !plan
                .items
                .iter()
                .any(|item| item.kind == PackageKind::Agent)
            && maybe_update_agent_json(manifest_value, spec, self.update_range)?
        {
            should_write_manifest = true;
        }

        // 8) Update lockfile (unless --frozen)
        if !self.frozen {
            let next_lock = if let Some(metadata) = workspace_metadata.as_ref() {
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
    if !lock.is_v1() {
        return Ok(());
    }

    let manifest_is_agent = manifest_value
        .and_then(|value| value.get("kind"))
        .and_then(Value::as_str)
        == Some("agent");

    let desired_has_agent = desired
        .items
        .iter()
        .any(|item| item.kind == PackageKind::Agent);

    if manifest_is_agent || desired_has_agent {
        return Err(anyhow!(
            "--frozen cannot use lockfile v1 for agent dependency graphs; run `agentpm install` without --frozen to regenerate agent.lock v2"
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

    let mut next = match existing_lock {
        Lock::V2(lock) => lock.clone(),
        Lock::V1(_) => match Lock::empty_v2() {
            // Direct installs now accumulate roots in v2 lockfiles. When starting
            // from a legacy v1 lock, we upgrade in place to an empty v2 structure
            // and then add the current request/package state.
            Lock::V2(lock) => lock,
            Lock::V1(_) => unreachable!(),
        },
    };
    next.generated = Utc::now();

    for item in &plan.items {
        // Current scope: direct installs may leave unreachable package entries in
        // the v2 lockfile after roots change. Root reachability is authoritative
        // for intent; lockfile package compaction is deferred to a later cleanup
        // decision instead of being coupled to Milestone 10a root semantics.
        // TODO: add lockfile/package compaction and on-disk prune behavior for
        // packages that are no longer reachable from any root.
        next.packages.insert(
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
        && manifest_value.get("kind").and_then(Value::as_str) == Some("agent")
        && !plan
            .items
            .iter()
            .any(|item| item.kind == PackageKind::Agent)
    {
        let local_root = build_local_root_from_manifest(manifest_value, &next.packages)?;
        next.roots.insert("local:agent".to_string(), local_root);
    }

    for root in build_registry_agent_roots(spec, plan, install_root)? {
        let LockRoot::RegistryAgent {
            package_key,
            tools,
            reserved,
        } = root
        else {
            unreachable!("registry root builder only yields registry agent roots");
        };
        next.roots.insert(
            package_key,
            LockedRoot {
                name: None,
                version: None,
                tools,
                reserved,
            },
        );
    }

    Ok(Lock::V2(next))
}

fn build_lock_roots(
    manifest_value: Option<&Value>,
    spec: Option<&str>,
    plan: &ResolvePlan,
    install_root: &std::path::Path,
) -> Result<Vec<LockRoot>> {
    if let Some(manifest_value) = manifest_value
        && manifest_value.get("kind").and_then(Value::as_str) == Some("agent")
        && spec.is_none()
    {
        let name = manifest_value
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("local-agent")
            .to_string();
        let version = manifest_value
            .get("version")
            .and_then(Value::as_str)
            .unwrap_or("0.0.0")
            .to_string();

        let tools = plan
            .items
            .iter()
            .filter(|item| item.kind == PackageKind::Tool)
            .map(|item| package_key(item.kind, &item.name, &item.version))
            .collect();

        return Ok(vec![LockRoot::LocalAgent {
            key: "local:agent".to_string(),
            name,
            version,
            tools,
            reserved: reserved_refs_from_manifest(manifest_value),
        }]);
    }

    build_registry_agent_roots(spec, plan, install_root)
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

fn build_registry_agent_roots(
    spec: Option<&str>,
    plan: &ResolvePlan,
    install_root: &std::path::Path,
) -> Result<Vec<LockRoot>> {
    let Some(spec) = spec else {
        return Ok(Vec::new());
    };
    let requested = parse_package_spec(spec)?;

    let tools: Vec<String> = plan
        .items
        .iter()
        .filter(|item| item.kind == PackageKind::Tool)
        .map(|item| package_key(item.kind, &item.name, &item.version))
        .collect();

    let mut roots = Vec::new();
    for agent_item in plan
        .items
        .iter()
        .filter(|item| item.kind == PackageKind::Agent && item.name == requested.name)
    {
        let agent_manifest_path =
            installed_agent_manifest_path(install_root, &agent_item.name, &agent_item.version)?;
        let reserved = if agent_manifest_path.exists() {
            let (manifest_value, _) = load_manifest_value(&agent_manifest_path)?;
            reserved_refs_from_manifest(&manifest_value)
        } else {
            crate::semver::types::ReservedReferences::default()
        };

        roots.push(LockRoot::RegistryAgent {
            package_key: package_key(agent_item.kind, &agent_item.name, &agent_item.version),
            tools: tools.clone(),
            reserved,
        });
    }

    Ok(roots)
}

fn build_local_root_from_manifest(
    manifest_value: &Value,
    packages: &BTreeMap<String, crate::semver::types::LockedPackage>,
) -> Result<LockedRoot> {
    let desired = DesiredSet::from_cli_or_agent_json(manifest_value, None, false)?;
    let mut tools = Vec::new();

    for item in desired.items {
        if item.kind != PackageKind::Tool {
            continue;
        }
        if let Some(pkg) = resolve_declared_tool_from_packages(packages, &item.name, &item.range)? {
            tools.push(package_key(pkg.kind, &pkg.name, &pkg.version));
        }
    }

    Ok(LockedRoot {
        name: manifest_value
            .get("name")
            .and_then(Value::as_str)
            .map(str::to_string),
        version: manifest_value
            .get("version")
            .and_then(Value::as_str)
            .map(str::to_string),
        tools,
        reserved: reserved_refs_from_manifest(manifest_value),
    })
}

fn resolve_declared_tool_from_packages(
    packages: &BTreeMap<String, crate::semver::types::LockedPackage>,
    name: &str,
    range: &str,
) -> Result<Option<crate::semver::types::LockedPackage>> {
    let exact = if range == "*" {
        None
    } else {
        Version::parse(range).ok()
    };
    let req = if exact.is_none() && range != "*" {
        Some(
            VersionReq::parse(range)
                .with_context(|| format!("declared tool range for {} is not valid semver", name))?,
        )
    } else {
        None
    };

    let mut matches: Vec<(Version, crate::semver::types::LockedPackage)> = Vec::new();
    for pkg in packages
        .values()
        .filter(|pkg| pkg.kind == PackageKind::Tool && pkg.name == name)
    {
        let version = match Version::parse(&pkg.version) {
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
        skills: manifest_array_or_empty(manifest_value, "skills"),
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
    use super::{
        InstallArgs, build_lock_roots, build_updated_lock, ensure_supported_install_kinds,
        installed_agent_manifest_path, should_load_manifest_for_install,
        validate_frozen_lock_compatibility,
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
    fn manifest_driven_agent_install_builds_local_root_metadata() {
        let manifest = json!({
            "kind": "agent",
            "name": "support-agent",
            "version": "0.1.0",
            "skills": ["@zack/triage-skill@0.1.0"],
            "knowledge": [],
            "memory": [],
            "profiles": [],
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

        let root = temp_root("local-root");
        let roots = build_lock_roots(Some(&manifest), None, &plan, &root).unwrap();

        assert_eq!(roots.len(), 1);
        let crate::semver::types::LockRoot::LocalAgent {
            key,
            name,
            version,
            tools,
            reserved,
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
        assert_eq!(
            reserved.skills,
            vec![Value::String("@zack/triage-skill@0.1.0".to_string())]
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
                "skills": ["@zack/triage-skill@0.1.0"],
                "knowledge": [],
                "memory": [],
                "profiles": []
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
            ],
        };

        let roots =
            build_lock_roots(None, Some("@zack/support-agent@0.1.0"), &plan, &root).unwrap();

        assert_eq!(roots.len(), 1);
        let LockRoot::RegistryAgent {
            package_key,
            tools,
            reserved,
        } = &roots[0]
        else {
            panic!("expected registry agent root");
        };
        assert_eq!(package_key, "agent:@zack/support-agent@0.1.0");
        assert_eq!(
            tools,
            &vec!["tool:@zack/slack-post-message@0.1.0".to_string()]
        );
        assert_eq!(
            reserved.skills,
            vec![Value::String("@zack/triage-skill@0.1.0".to_string())]
        );
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
    fn direct_agent_install_merges_existing_registry_roots() {
        let root = temp_root("merge-registry-roots");
        let existing = Lock::V2(LockV2 {
            lockfile_version: 2,
            generated: Utc::now(),
            packages: BTreeMap::from([(
                "agent:@zack/support-agent@0.1.0".to_string(),
                LockedPackage {
                    kind: PackageKind::Agent,
                    name: "@zack/support-agent".to_string(),
                    version: "0.1.0".to_string(),
                    integrity: "sha256-old-agent".to_string(),
                },
            )]),
            roots: BTreeMap::from([(
                "agent:@zack/support-agent@0.1.0".to_string(),
                LockedRoot {
                    name: None,
                    version: None,
                    tools: vec!["tool:@zack/capitalize@0.1.0".to_string()],
                    reserved: Default::default(),
                },
            )]),
        });

        let manifest_path =
            installed_agent_manifest_path(&root, "@zack/escalation-agent", "0.1.0").unwrap();
        fs::create_dir_all(manifest_path.parent().unwrap()).unwrap();
        fs::write(
            &manifest_path,
            serde_json::to_string_pretty(&json!({
                "kind": "agent",
                "name": "escalation-agent",
                "version": "0.1.0"
            }))
            .unwrap(),
        )
        .unwrap();

        let plan = ResolvePlan {
            items: vec![
                ResolvedPackage {
                    kind: PackageKind::Agent,
                    name: "@zack/escalation-agent".to_string(),
                    version: "0.1.0".to_string(),
                    integrity: "sha256-new-agent".to_string(),
                },
                ResolvedPackage {
                    kind: PackageKind::Tool,
                    name: "@zack/capitalize".to_string(),
                    version: "0.2.0".to_string(),
                    integrity: "sha256-tool".to_string(),
                },
            ],
        };

        let next = build_updated_lock(
            &existing,
            None,
            Some("@zack/escalation-agent@0.1.0"),
            &plan,
            &root,
        )
        .unwrap();

        let Lock::V2(lock) = next else {
            panic!("expected v2 lock");
        };
        assert!(lock.roots.contains_key("agent:@zack/support-agent@0.1.0"));
        assert!(
            lock.roots
                .contains_key("agent:@zack/escalation-agent@0.1.0")
        );
        assert!(
            lock.packages
                .contains_key("agent:@zack/support-agent@0.1.0")
        );
        assert!(
            lock.packages
                .contains_key("agent:@zack/escalation-agent@0.1.0")
        );
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
            items: vec![ResolvedPackage {
                kind: PackageKind::Tool,
                name: "@zack/capitalize".to_string(),
                version: "0.2.0".to_string(),
                integrity: "sha256-tool".to_string(),
            }],
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
                        reserved: Default::default(),
                    },
                ),
                (
                    "agent:@zack/support-agent@0.1.0".to_string(),
                    LockedRoot {
                        name: None,
                        version: None,
                        tools: vec!["tool:@zack/capitalize@0.1.0".to_string()],
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
