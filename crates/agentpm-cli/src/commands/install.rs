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
use crate::{io::download::download_and_extract_all, io::fs, ui::Step};
use anyhow::anyhow;
use chrono::Utc;
use semver::{Version, VersionReq};
use serde_json::Value;
use std::collections::BTreeMap;
use std::io::IsTerminal;
use std::path::PathBuf;

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

        // 2) Determine desired set
        let desired = if let Some(spec) = self.spec.as_deref() {
            DesiredSet::from_cli_or_agent_json(&Value::Null, Some(spec), self.update_range)?
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
        let lock = read_lock_or_default(".")?;
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
        let dl_dir = PathBuf::from(".agentpm/cache");
        let tools_dir = PathBuf::from(".agentpm/tools");
        let agents_dir = PathBuf::from(".agentpm/agents");
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
            let next_lock = build_updated_lock(
                &lock,
                manifest_value.as_ref(),
                self.spec.as_deref(),
                &plan,
                PathBuf::from(".agentpm").as_path(),
            )?;
            write_lock(".", &next_lock)?;
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
        build_lock_roots, build_updated_lock, installed_agent_manifest_path,
        should_load_manifest_for_install, validate_frozen_lock_compatibility,
    };
    use crate::semver::types::{
        DesiredSet, Lock, LockRoot, LockV1, LockV2, LockedDependency, LockedPackage, LockedRoot,
        PackageKind, ResolvePlan, ResolvedPackage,
    };
    use chrono::Utc;
    use serde_json::{Value, json};
    use std::collections::BTreeMap;
    use std::fs;
    use std::path::{Path, PathBuf};
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
            name,
            version,
            tools,
            reserved,
        } = &roots[0]
        else {
            panic!("expected local agent root");
        };
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

    fn temp_root(label: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("agentpm-install-{label}-{nanos}"))
    }
}
