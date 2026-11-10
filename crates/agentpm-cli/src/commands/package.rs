//! Provider-neutral build bundles. Descriptors travel beside their archive;
//! fan-in validates local bytes and identities before any Registry mutation.
use super::publish::{self, PublishArgs};
use crate::prelude::*;
use agentpm_sdk::{
    artifact::{ArtifactFormat, PayloadBuildFailure, PayloadDiagnostic, ToolPayloadDescriptor},
    models::install::PackageKind,
    python::PythonReleaseMetadata,
    release::{ArtifactMetadata, ReleaseArtifact, ReleaseManifest, manifest_digest},
    signing::{AuthorSignatureV1, AuthorSignatureV2, ReleaseAuthorSignature},
    target::Target,
};
use anyhow::{bail, ensure};
use base64::{Engine, engine::general_purpose::STANDARD as B64};
use ed25519_dalek::Signer;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::HashSet,
    fs,
    io::Read,
    path::{Path, PathBuf},
};

const BUILD_TYPE: &str = "agentpm.package.build.v1";
const MAX_DESCRIPTOR: u64 = 4 * 1024 * 1024;

#[cfg(test)]
mod tests;

#[derive(Args, Debug)]
pub struct PackageArgs {
    /// Manifest to package; Python dependencies require a prepared agent.lock
    #[arg(long, default_value = "agent.json")]
    pub manifest: String,
    /// Override the bundled schema with a URL or file
    #[arg(long)]
    pub schema: Option<String>,
    /// Treat manifest-lint warnings as errors
    #[arg(long)]
    pub strict: bool,
    /// Bundle directory containing package-build.json and its archive
    #[arg(long, default_value = "target/agentpm/build")]
    pub output: PathBuf,
    /// Emit one JSON result on stdout; progress and warnings stay on stderr
    #[arg(long)]
    pub machine: bool,
}

impl PackageArgs {
    pub async fn run(self, base_url: String) -> Result<()> {
        let result = self.build(base_url).await;
        match result {
            Ok((build, path)) => {
                if self.machine {
                    println!(
                        "{}",
                        json!({"schema_version":1,"status":"success","descriptor_path":path,"build":build})
                    );
                } else {
                    println!("Package built: {}", path.display());
                }
                Ok(())
            }
            Err(error) => {
                if self.machine {
                    let diagnostics = error
                        .downcast_ref::<PayloadBuildFailure>()
                        .map(|e| e.diagnostics.clone())
                        .unwrap_or_default();
                    println!(
                        "{}",
                        json!({"schema_version":1,"status":"error","error":{"message":error.to_string(),"diagnostics":diagnostics}})
                    );
                }
                Err(error)
            }
        }
    }

    async fn build(&self, base_url: String) -> Result<(PackageBuild, PathBuf)> {
        let args = PublishArgs {
            manifest: self.manifest.clone(),
            schema: self.schema.clone(),
            strict: self.strict,
            ..Default::default()
        };
        let mut prepared = args.prepare(&Config::load(base_url)?, self.machine).await?;
        let build = PackageBuild::from_prepared(&mut prepared.tar_path, &mut prepared.meta)?;
        fs::create_dir_all(&self.output)?;
        let destination = self.output.join(&build.artifact_path);
        if fs::canonicalize(&prepared.tar_path)?
            != fs::canonicalize(&destination).unwrap_or_else(|_| destination.clone())
        {
            fs::copy(&prepared.tar_path, &destination)?;
        }
        let descriptor = self.output.join("package-build.json");
        let temporary = self.output.join("package-build.json.tmp");
        fs::write(&temporary, serde_json::to_vec_pretty(&build)?)?;
        fs::rename(&temporary, &descriptor)?;
        Ok((build, descriptor))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PackageBuild {
    #[serde(rename = "type")]
    build_type: String,
    artifact_path: String,
    build_target: Target,
    target: Option<Target>,
    format: ArtifactFormat,
    artifact_sha256: String,
    artifact_size: u64,
    manifest_digest: String,
    python_resolution_digest: Option<String>,
    diagnostics: Vec<PayloadDiagnostic>,
    metadata: Value,
}

impl PackageBuild {
    pub(super) fn from_prepared(path: &mut PathBuf, metadata: &mut Value) -> Result<Self> {
        let python = metadata["manifest"]["kind"] == "tool"
            && metadata["manifest"]["runtime"]["type"] == "python";
        let descriptor: Option<ToolPayloadDescriptor> = metadata
            .get("artifact")
            .map(|v| serde_json::from_value(v.clone()))
            .transpose()?;
        if python {
            let filename = path.file_name().unwrap().to_string_lossy();
            let name = filename.strip_suffix(".tar.gz").unwrap_or(&filename);
            let destination = path.with_file_name(format!("{name}.release.tar.gz"));
            publish::release_artifact::write_release_artifact(path, &destination)?;
            *path = destination;
        }
        let (digest, size) = publish::file_digest_and_len(path)?;
        metadata["sha256"] = digest.clone().into();
        metadata["size"] = size.into();
        Ok(Self {
            build_type: BUILD_TYPE.into(),
            artifact_path: path.file_name().unwrap().to_string_lossy().into_owned(),
            build_target: Target::current()?,
            target: descriptor.as_ref().map(|d| d.target),
            format: ArtifactFormat::TarGz,
            artifact_sha256: format!("sha256:{digest}"),
            artifact_size: size,
            manifest_digest: manifest_digest(&metadata["manifest"])?,
            python_resolution_digest: metadata["python_resolution_digest"]
                .as_str()
                .map(|d| format!("sha256:{d}")),
            diagnostics: descriptor.map(|d| d.diagnostics).unwrap_or_default(),
            metadata: metadata.clone(),
        })
    }

    fn read(path: &Path) -> Result<(Self, PathBuf)> {
        let file = fs::File::open(path)?;
        ensure!(
            file.metadata()?.len() <= MAX_DESCRIPTOR,
            "package descriptor exceeds size limit"
        );
        let build: Self = serde_json::from_reader(file)?;
        ensure!(
            build.build_type == BUILD_TYPE,
            "unsupported package-build descriptor"
        );
        ensure!(
            build.build_target != Target::Any,
            "build host must be concrete"
        );
        ensure!(
            Path::new(&build.artifact_path).components().count() == 1
                && !build.artifact_path.contains(['/', '\\'])
                && build.artifact_path != "..",
            "artifact path must name a sibling file"
        );
        let archive = path
            .parent()
            .unwrap_or(Path::new("."))
            .join(&build.artifact_path);
        let (sha, size) = publish::file_digest_and_len(&archive)?;
        ensure!(
            build.artifact_sha256 == format!("sha256:{sha}") && build.artifact_size == size,
            "artifact digest/size differs from package-build descriptor"
        );
        ensure!(
            build.metadata["sha256"] == sha && build.metadata["size"] == size,
            "inconsistent artifact metadata"
        );
        ensure!(
            build.manifest_digest == manifest_digest(&build.metadata["manifest"])?,
            "manifest digest differs from package-build descriptor"
        );
        crate::manifest::parse_publish_manifest(&build.metadata["manifest"])?;
        let python = build.metadata["manifest"]["kind"] == "tool"
            && build.metadata["manifest"]["runtime"]["type"] == "python";
        ensure!(
            python == build.target.is_some(),
            "only Python Tools carry a compatibility target"
        );
        let mut expected_python = None;
        if let Some(value) = build.metadata.get("python_resolution") {
            let metadata = PythonReleaseMetadata {
                python_resolution: serde_json::from_value(value.clone())?,
                python_resolution_digest: build.metadata["python_resolution_digest"]
                    .as_str()
                    .unwrap_or("")
                    .into(),
            };
            expected_python = Some(metadata.release_manifest_digest()?);
        }
        ensure!(
            build.python_resolution_digest == expected_python,
            "Python resolution digest differs from package-build descriptor"
        );
        ensure!(
            !crate::semver::types::declares_python_dependencies(&build.metadata["manifest"])
                || expected_python.is_some(),
            "declared Python dependencies require a committed resolution"
        );
        let mut archive_reader =
            tar::Archive::new(flate2::read::GzDecoder::new(fs::File::open(&archive)?));
        let mut names = HashSet::new();
        let mut found_manifest = false;
        let mut found_artifact = false;
        let mut found_python = false;
        for entry in archive_reader.entries()? {
            let mut entry = entry?;
            let name = entry.path()?.to_string_lossy().into_owned();
            publish::ensure_safe_tar_name(&name)?;
            ensure!(
                names.insert(name.clone()) && names.len() <= publish::MAX_TAR_ENTRIES,
                "duplicate or excessive archive members"
            );
            if [
                "agent.json",
                agentpm_sdk::release::ARTIFACT_METADATA_FILE,
                agentpm_sdk::python::RESOLUTION_METADATA_FILE,
            ]
            .contains(&name.as_str())
            {
                ensure!(
                    entry.header().entry_type().is_file() && entry.size() <= MAX_DESCRIPTOR,
                    "invalid archive metadata member"
                );
                let mut bytes = Vec::new();
                entry
                    .by_ref()
                    .take(MAX_DESCRIPTOR + 1)
                    .read_to_end(&mut bytes)?;
                let value: Value = serde_json::from_slice(&bytes)?;
                match name.as_str() {
                    "agent.json" => {
                        ensure!(
                            manifest_digest(&value)? == build.manifest_digest,
                            "archive manifest differs from descriptor"
                        );
                        found_manifest = true;
                    }
                    agentpm_sdk::release::ARTIFACT_METADATA_FILE => {
                        let metadata: ArtifactMetadata = serde_json::from_value(value)?;
                        ensure!(
                            Some(metadata.target) == build.target
                                && metadata.format == build.format,
                            "embedded artifact target differs from descriptor"
                        );
                        found_artifact = true;
                    }
                    _ => {
                        let metadata: PythonReleaseMetadata = serde_json::from_value(value)?;
                        ensure!(
                            Some(metadata.release_manifest_digest()?) == expected_python,
                            "embedded Python resolution differs from descriptor"
                        );
                        found_python = true;
                    }
                }
            }
        }
        ensure!(
            found_manifest && found_artifact == python && found_python == expected_python.is_some(),
            "missing or unexpected archive metadata"
        );
        // Collection diagnostics survive in the descriptor; content diagnostics
        // are recomputed from the shipped archive rather than trusting JSON.
        if python {
            let actual = publish::payload::describe(&archive)?;
            if actual.target == Target::Any {
                ensure!(
                    build.target == Some(Target::Any),
                    "portable archive was relabeled as a concrete target"
                );
            } else {
                ensure!(
                    build.target == Some(build.build_target),
                    "concrete artifact must match its build host"
                );
            }
            let descriptor: ToolPayloadDescriptor =
                serde_json::from_value(build.metadata["artifact"].clone())?;
            ensure!(
                Some(descriptor.target) == build.target
                    && descriptor.diagnostics == build.diagnostics,
                "inconsistent payload diagnostics/target"
            );
            for mut diagnostic in actual.diagnostics {
                // The archive can have been built on a different host. Its
                // content reason remains the same; normalize only the target
                // field before checking the producer's typed diagnostic.
                if diagnostic.target.is_some() {
                    diagnostic.target = build.target;
                }
                ensure!(
                    build.diagnostics.contains(&diagnostic),
                    "missing or changed payload diagnostic for {}",
                    diagnostic.path
                );
            }
        }
        Ok((build, archive))
    }
}

fn validate_set(builds: &[(PackageBuild, PathBuf)], expected: &[Target]) -> Result<()> {
    ensure!(!builds.is_empty(), "no package builds supplied");
    let first = &builds[0].0;
    let mut targets = HashSet::new();
    let mut hosts = HashSet::new();
    let portable = builds.iter().any(|(b, _)| b.target == Some(Target::Any));
    let concrete = builds
        .iter()
        .any(|(b, _)| b.target.is_some_and(|t| t != Target::Any));
    if portable && concrete {
        let details = builds
            .iter()
            .map(|(b, p)| {
                format!(
                    "{}: build {} claims {}; diagnostics: {}",
                    p.display(),
                    b.build_target,
                    b.target
                        .map(|t| t.to_string())
                        .unwrap_or_else(|| "legacy".into()),
                    b.diagnostics
                        .iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join("; ")
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        bail!(
            "Mixed any/concrete matrix claims; correct permissions or compatibility and rebuild every affected output.\n{details}"
        );
    }
    for (build, _) in builds {
        ensure!(
            build.metadata["manifest"] == first.metadata["manifest"]
                && build.manifest_digest == first.manifest_digest
                && build.python_resolution_digest == first.python_resolution_digest,
            "matrix package identity, manifest or Python resolution mismatch"
        );
        ensure!(
            targets.insert(build.target),
            "duplicate artifact target: {:?}",
            build.target
        );
        ensure!(
            hosts.insert(build.build_target),
            "duplicate matrix build host"
        );
    }
    if !expected.is_empty() {
        let required: HashSet<_> = expected.iter().copied().collect();
        ensure!(
            required.len() == expected.len() && !required.contains(&Target::Any),
            "expected build targets must be unique concrete hosts"
        );
        ensure!(
            hosts == required,
            "missing or unexpected matrix build host; expected {required:?}, got {hosts:?}"
        );
    }
    ensure!(
        first.target.is_some() || builds.len() == 1,
        "legacy packages require one build"
    );
    Ok(())
}

pub(super) async fn publish_builds(args: &PublishArgs, cfg: &Config, token: String) -> Result<()> {
    let builds = args
        .builds
        .iter()
        .map(|p| PackageBuild::read(p))
        .collect::<Result<Vec<_>>>()?;
    validate_set(&builds, &args.expected_build_targets)?;
    let client = AgentPmClient::new(cfg.base_url.clone())?.with_token(token);
    publish_prepared(args, &client, builds).await
}

pub(super) async fn publish_prepared(
    args: &PublishArgs,
    client: &AgentPmClient,
    builds: Vec<(PackageBuild, PathBuf)>,
) -> Result<()> {
    validate_set(&builds, &args.expected_build_targets)?;
    let first = &builds[0].0;
    let key = if args.sign {
        Some(crate::keys::signing::load_publish_key(
            args.key_id.as_deref(),
            args.key_file.as_deref(),
        )?)
    } else {
        None
    };
    let mut metadata = first.metadata.clone();
    if let Some(namespace) = &args.namespace {
        metadata["namespace_handle"] = namespace.clone().into();
    }
    use std::io::IsTerminal;
    let mut step = crate::ui::Step::new(
        "Publishing package",
        args.quiet || !std::io::stderr().is_terminal(),
    );
    let result: Result<_> = async {
        Ok(if first.target.is_some() {
            let namespace = args
                .namespace
                .as_deref()
                .context("Python release publication requires --namespace <handle>")?;
            let release = ReleaseManifest {
                release_type: agentpm_sdk::release::RELEASE_TYPE.into(),
                kind: PackageKind::Tool,
                name: format!(
                    "@{namespace}/{}",
                    metadata["manifest"]["name"]
                        .as_str()
                        .context("missing package name")?
                ),
                version: metadata["manifest"]["version"]
                    .as_str()
                    .context("missing package version")?
                    .into(),
                manifest_digest: first.manifest_digest.clone(),
                python_resolution_digest: first.python_resolution_digest.clone(),
                artifacts: builds
                    .iter()
                    .map(|(b, _)| ReleaseArtifact {
                        target: b.target.unwrap(),
                        digest: b.artifact_sha256.clone(),
                        size: b.artifact_size,
                        content_type: "application/gzip".into(),
                        format: b.format,
                    })
                    .collect(),
            };
            release.validate()?;
            metadata["release"] = serde_json::to_value(&release)?;
            let signatures = key
                .as_ref()
                .map(|(key, public)| -> Result<_> {
                    let statement =
                        AuthorSignatureV2::new(&release, &chrono::Utc::now().to_rfc3339())?;
                    Ok(ReleaseAuthorSignature {
                        algo: "ed25519".into(),
                        public_key_b64: public.clone(),
                        signature_b64: B64
                            .encode(key.sign(&statement.canonical_bytes()?).to_bytes()),
                        statement_json: statement,
                    })
                })
                .transpose()?;
            let reservation = client.init_release(&metadata).await?;
            let returned: HashSet<_> = reservation
                .artifacts
                .iter()
                .map(|a| a.target.as_str())
                .collect();
            let wanted: HashSet<_> = release
                .artifacts
                .iter()
                .map(|a| a.target.as_str())
                .collect();
            ensure!(
                returned == wanted && returned.len() == reservation.artifacts.len(),
                "Registry returned an unexpected upload inventory"
            );
            for (build, path) in &builds {
                let instruction = reservation
                    .artifacts
                    .iter()
                    .find(|a| Some(a.target.as_str()) == build.target.map(|t| t.as_str()))
                    .unwrap();
                client.upload_release_artifact(instruction, path).await?;
            }
            if let Some(signature) = signatures {
                client
                    .finalize_signed_release(&reservation.release_id, &[signature])
                    .await?
            } else {
                client.finalize_release(&reservation.release_id).await?
            }
        } else {
            let extra = key
                .as_ref()
                .map(|(key, public)| -> Result<_> {
                    let manifest = &metadata["manifest"];
                    let statement = AuthorSignatureV1::new(
                        manifest["kind"].as_str().unwrap().into(),
                        manifest["name"].as_str().unwrap().into(),
                        manifest["version"].as_str().unwrap().into(),
                        first.artifact_sha256.clone(),
                        chrono::Utc::now().to_rfc3339(),
                    );
                    publish::signed_finalize_extra(&statement, key, public)
                })
                .transpose()?;
            client
                .publish_package_from_path(&metadata, &builds[0].1, &first.artifact_path, extra)
                .await?
        })
    }
    .await;
    let receipt = match result {
        Ok(receipt) => {
            step.ok("done");
            receipt
        }
        Err(error) => {
            step.err("publication failed");
            return Err(error);
        }
    };
    println!(
        "✓ Published {}@{}\n  id: {}\n  url: {}",
        receipt.name, receipt.version, receipt.id, receipt.url
    );
    Ok(())
}
