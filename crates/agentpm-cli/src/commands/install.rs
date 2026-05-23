use crate::manifest::{
    load_manifest_value, read_lock_or_default, write_lock, write_manifest_pretty_atomic,
};
use crate::prelude::*;
use crate::semver::adapt::{plan_to_sdk_resolve, to_sdk_request};
use crate::semver::types::{
    DesiredSet, Lock, LockRoot, PackageKind, ReservedReferences, ResolvePlan, lock_from_plan,
    package_key,
};
use crate::semver::update::maybe_update_agent_json;
use crate::{io::download::download_and_extract_all, io::fs, ui::Step};
use anyhow::anyhow;
use serde_json::Value;
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
        let mut s = Step::new("Downloading tools", quiet);
        let dl_dir = PathBuf::from(".agentpm/cache");
        let tools_dir = PathBuf::from(".agentpm/tools");
        fs::ensure_dirs(&[&dl_dir, &tools_dir])?;
        download_and_extract_all(&init, &dl_dir, &tools_dir, self.refresh, quiet).await?;
        s.ok("");

        // 6) Finalize (report success for metrics / server-side bookkeeping)
        let mut s = Step::new("Finalizing install", quiet);
        client.install_finalize(&init.session_id).await?;
        s.ok("");

        // 7) Update lockfile (unless --frozen)
        if !self.frozen {
            let roots = build_lock_roots(manifest_value.as_ref(), self.spec.as_deref(), &plan);
            write_lock(".", &lock_from_plan(&plan, &roots))?;
        }

        // 8) If single-spec and not present in agent.json, append or update range if allowed.
        //
        // TODO(milestone-9/10): keep this behavior only for direct tool specs in a
        // local `kind: "agent"` project. Direct agent package installs should not
        // mutate the local manifest; they should be represented as registry roots in
        // lockfile v2 / installed package layout instead.
        if let Some(spec) = &self.spec
            && let Some(manifest_value) = manifest_value.as_mut()
            && manifest_value.get("kind").and_then(Value::as_str) == Some("agent")
            && maybe_update_agent_json(manifest_value, spec, self.update_range)?
        {
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

fn build_lock_roots(
    manifest_value: Option<&Value>,
    spec: Option<&str>,
    plan: &ResolvePlan,
) -> Vec<LockRoot> {
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

        return vec![LockRoot::LocalAgent {
            name,
            version,
            tools,
            reserved: reserved_refs_from_manifest(manifest_value),
        }];
    }

    Vec::new()
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
        build_lock_roots, should_load_manifest_for_install, validate_frozen_lock_compatibility,
    };
    use crate::semver::types::{
        DesiredSet, Lock, LockV1, LockedDependency, PackageKind, ResolvePlan, ResolvedPackage,
    };
    use chrono::Utc;
    use serde_json::{Value, json};
    use std::collections::BTreeMap;
    use std::path::Path;

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

        let err = validate_frozen_lock_compatibility(&lock, Some(&manifest), &desired)
            .unwrap_err();

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

        let roots = build_lock_roots(Some(&manifest), None, &plan);

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
}
