use crate::auth::read_key_file;
use crate::commands::keys::key_id_from_pub_b64;
use crate::keys::signing::{
    StoredKeyV1, decrypt_private, keystore_dir, prompt_passphrase_with_fallback,
};
use crate::manifest::{
    AgentManifest, Entrypoint, PublishManifest, SkillManifest, TemplateManifest, ToolManifest,
    load_manifest_value, parse_publish_manifest, resolve_schema_source, validate_manifest_value,
};
use crate::prelude::*;
use crate::ui::Step;
use anyhow::{anyhow, bail};
use base64::Engine;
use ed25519_dalek::{Signer, SigningKey};
use flate2::{Compression, write::GzEncoder};
use serde_json::{Map as JsonMap, Value as JsonValue};
use sha2::{Digest, Sha256};
use std::fs::{File, Metadata, read_link, symlink_metadata};
use std::io::IsTerminal;
use std::path::Component;
use std::{
    collections::HashSet,
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
};
use tar::{Builder as TarBuilder, EntryType, Header};
use tracing::{info, warn};
use walkdir::WalkDir;

const MAX_ARTIFACT_BYTES: u64 = 3 * 1024 * 1024 * 1024; // 3 GB
const MAX_TAR_ENTRIES: usize = 15_000;
static BLOCKED_EMBEDDED_EXTS: &[&str] = &[".zip", ".whl", ".7z", ".rar", ".tar", ".tgz", ".tar.gz"];

const MAX_README_BYTES: usize = 512 * 1024; // 512 KB
const MAX_LICENSE_BYTES: usize = 128 * 1024; // 128 KB

#[derive(Args, Debug, Default)]
pub struct PublishArgs {
    /// Path to the manifest (agent.json)
    #[arg(long, default_value = "agent.json")]
    pub manifest: String,

    /// Override schema URL or path (same flag as lint)
    #[arg(long, value_name = "URL|PATH")]
    pub schema: Option<String>,

    /// Treat warnings as errors (same semantics as lint)
    #[arg(long)]
    pub strict: bool,

    /// Dry-run: validate and package but do not upload
    #[arg(long)]
    pub dry_run: bool,

    /// Suppress progress output (also auto-enabled when not a TTY)
    #[arg(long)]
    pub quiet: bool,

    /// Sign this publish using a local key
    #[arg(long)]
    pub sign: bool,

    /// Key id to use when --sign (from `agentpm keys list`)
    #[arg(long, value_name = "KEY_ID")]
    pub key_id: Option<String>,

    /// Personal Access Token for headless auth (overrides env/file)
    #[arg(long, value_name = "PAT", env = "AGENTPM_TOKEN")]
    pub token: Option<String>,

    /// Namespace handle to publish into
    #[arg(long, value_name = "HANDLE")]
    pub namespace: Option<String>,
}

impl PublishArgs {
    pub async fn run(self, base_url: String) -> Result<()> {
        let cfg = Config::load(base_url.clone())?;

        // Quiet if the user asked OR stderr is not a terminal (piped/redirected)
        let auto_quiet = !std::io::stderr().is_terminal();
        let quiet = self.quiet || auto_quiet;

        // 1) Load PAT (login is required; for MVP we use cached token)
        let mut s = Step::new("Reading credentials", quiet);
        let token = resolve_token(&cfg, self.token.clone())?
            .ok_or_else(|| anyhow!(
                "No credentials. Provide a PAT via:\n  • --token <PAT>\n  • AGENTPM_TOKEN env var\nOr run `agentpm login --paste` to save one locally."
            ))?;
        s.ok(format!("using {}", mask_token(&token)));

        // 2) Validate manifest using the same schema as `lint`
        let mut s = Step::new("Validating manifest", quiet);
        let manifest_path = PathBuf::from(&self.manifest);
        let (mut manifest_value, _raw) = load_manifest_value(&manifest_path)
            .with_context(|| format!("loading {}", manifest_path.display()))?;

        let schema_source = resolve_schema_source(self.schema);
        let (schema_ok, issues) = validate_manifest_value(
            &schema_source,
            &manifest_path.to_string_lossy(),
            &mut manifest_value,
            /*fix=*/ false,
        )?;

        // Print issues like lint
        for i in &issues {
            let badge = if i.level == "error" { "ERROR" } else { "WARN " };
            eprintln!("  [{badge}] {}", i.message);
            if !i.instance_path.is_empty() {
                eprintln!("        at instance {}", i.instance_path);
            }
            if !i.schema_path.is_empty() {
                eprintln!("        vs schema  {}", i.schema_path);
            }
        }

        // strict semantics match lint
        let has_error = issues.iter().any(|i| i.level == "error");
        let has_warning = issues.iter().any(|i| i.level == "warning");
        let ok_flag = if self.strict {
            schema_ok && !has_warning
        } else {
            schema_ok && !has_error
        };
        if !ok_flag {
            s.err("failed");
            return Err(anyhow!(
                "Manifest validation failed (strict={})",
                self.strict
            ));
        }
        s.ok("schema + semantics");

        // 3) Strongly typed manifest split
        let publish_manifest = parse_publish_manifest(&manifest_value)?;

        // 4) Package files by artifact kind
        let mut s = Step::new("Packaging files", quiet);
        let tar_path = match &publish_manifest {
            PublishManifest::Tool(mf) => {
                package_tool(mf, &manifest_path).context("packaging tool into tar.gz")?
            }
            PublishManifest::Agent(mf) => {
                package_agent(mf, &manifest_path).context("packaging agent into tar.gz")?
            }
            PublishManifest::Template(mf) => {
                package_template(mf, &manifest_path).context("packaging template into tar.gz")?
            }
            PublishManifest::Skill(mf) => package_skill(mf, &manifest_path, &manifest_value)
                .context("packaging skill into tar.gz")?,
        };
        let (sha256_hex, size_bytes) = file_digest_and_len(&tar_path)?;
        if size_bytes > MAX_ARTIFACT_BYTES {
            bail!(
                "artifact is too large ({} bytes > {} bytes).",
                size_bytes,
                MAX_ARTIFACT_BYTES
            );
        }

        s.ok(format!(
            "{} bytes, sha256: {}",
            size_bytes,
            &sha256_hex[..12]
        ));

        info!(
            "Package ready: {} ({} bytes, sha256:{})",
            tar_path.display(),
            size_bytes,
            sha256_hex
        );

        let manifest_dir = manifest_path.parent().unwrap_or_else(|| Path::new("."));

        let skill_manual_payload = match &publish_manifest {
            PublishManifest::Skill(manifest) => {
                let entrypoint_path = manifest_dir.join(&manifest.skill.entrypoint);
                let manual_text = read_utf8_with_cap(&entrypoint_path, MAX_README_BYTES)
                    .with_context(|| {
                        format!("reading skill entrypoint {}", manifest.skill.entrypoint)
                    })?;
                let sha = hex_sha256(manual_text.as_bytes());
                Some(serde_json::json!({
                    "path": manifest.skill.entrypoint,
                    "sha256": sha,
                    "content": manual_text
                }))
            }
            _ => None,
        };

        // ---- README (optional) ----
        let manifest_readme_path = manifest_value.get("readme").and_then(|v| v.as_str());

        let readme_payload = match discover_readme(manifest_dir, manifest_readme_path) {
            Some((p, rel_path)) => {
                match read_utf8_with_cap(&p, MAX_README_BYTES) {
                    Ok(md_text) => {
                        let sha = hex_sha256(md_text.as_bytes());
                        Some(serde_json::json!({
                            "path": rel_path,
                            "sha256": sha,
                            "content": md_text
                        }))
                    }
                    Err(e) => {
                        // Non-fatal: warn and omit
                        eprintln!("Warning: skipping README ({}).", e);
                        None
                    }
                }
            }
            None => None,
        };

        // ---- LICENSE (optional) ----
        let (license_spdx_opt, license_file_opt) = pick_license_paths(&manifest_value);

        let license_payload = {
            // Prefer file content if provided; spdx is added if present
            let mut file_block: Option<serde_json::Value> = None;
            if let Some(rel) = license_file_opt {
                let p = manifest_dir.join(rel);
                if p.exists() {
                    match read_utf8_with_cap(&p, MAX_LICENSE_BYTES) {
                        Ok(text) => {
                            let sha = hex_sha256(text.as_bytes());
                            file_block = Some(serde_json::json!({
                                "path": rel,
                                "sha256": sha,
                                "content": text
                            }));
                        }
                        Err(e) => {
                            eprintln!("Warning: skipping license.file ({}).", e);
                        }
                    }
                } else {
                    eprintln!("Warning: license.file '{}' not found.", rel);
                }
            }

            if license_spdx_opt.is_none() && file_block.is_none() {
                None
            } else {
                let mut obj = JsonMap::new();
                if let Some(spdx) = license_spdx_opt {
                    obj.insert("spdx".into(), JsonValue::String(spdx.to_string()));
                }
                if let Some(fb) = file_block
                    && let Some(m) = fb.as_object()
                {
                    if let Some(v) = m.get("path") {
                        obj.insert("path".into(), v.clone());
                    }
                    if let Some(v) = m.get("sha256") {
                        obj.insert("sha256".into(), v.clone());
                    }
                    if let Some(v) = m.get("content") {
                        obj.insert("content".into(), v.clone());
                    }
                }
                Some(JsonValue::Object(obj))
            }
        };

        if let Some(r) = &readme_payload
            && let (Some(p), Some(sha)) = (r.get("path"), r.get("sha256"))
        {
            println!("✓ README: {} (sha256 {})", p, sha.as_str().unwrap_or(""));
        }
        if let Some(m) = &skill_manual_payload
            && let (Some(p), Some(sha)) = (m.get("path"), m.get("sha256"))
        {
            println!(
                "✓ Skill manual: {} (sha256 {})",
                p,
                sha.as_str().unwrap_or("")
            );
        }
        if let Some(l) = &license_payload {
            let spdx = l.get("spdx").and_then(|v| v.as_str()).unwrap_or("");
            if let Some(p) = l.get("path") {
                println!(
                    "✓ License: {} (from {}, sha256 {})",
                    spdx,
                    p.as_str().unwrap_or("LICENSE"),
                    l.get("sha256").and_then(|v| v.as_str()).unwrap_or("")
                );
            } else if !spdx.is_empty() {
                println!("✓ License: {}", spdx);
            }
        }

        if self.dry_run {
            println!("Dry-run: artifact created at {}", tar_path.display());
            return Ok(());
        }

        // 5) Upload to registry (MVP: single call, multipart form)
        //    POST {base_url}/v1/tools/publish
        //    Authorization: Bearer <PAT>
        //    multipart fields:
        //      - metadata: JSON (manifest + client info + sha256/size)
        //      - artifact: file (application/gzip)
        let mut s = Step::new("Uploading artifact", quiet);
        let client = AgentPmClient::new(cfg.base_url.clone())?.with_token(token);

        let meta = build_publish_metadata(
            manifest_value,
            &sha256_hex,
            size_bytes,
            readme_payload,
            skill_manual_payload,
            license_payload,
            self.namespace.as_deref(),
        );
        let filename = match &publish_manifest {
            PublishManifest::Tool(mf) => {
                artifact_filename(&mf.name, &mf.version, Some(&mf.runtime))
            }
            PublishManifest::Agent(mf) => artifact_filename(&mf.name, &mf.version, None),
            PublishManifest::Template(mf) => artifact_filename(&mf.name, &mf.version, None),
            PublishManifest::Skill(mf) => artifact_filename(&mf.name, &mf.version, None),
        };

        // Build optional finalize_extra
        let finalize_extra: Option<serde_json::Value> = if self.sign {
            // pick a local key (reuse helper from keys code)
            let (_key_id, stored) = select_local_key(self.key_id.as_deref())?;
            let pass = prompt_passphrase_with_fallback("Key passphrase: ")?;
            let sk_bytes = decrypt_private(&stored, &pass)?;
            let sk_arr: [u8; 32] = sk_bytes
                .as_slice()
                .try_into()
                .map_err(|_| anyhow!("expected 32-byte ed25519 secret key"))?;
            let signing_key = SigningKey::from_bytes(&sk_arr);

            // Minimal statement (server checks name/version/digest)
            let statement = serde_json::json!({
                "type": "agentpm.package.signature.v1",
                "kind": manifest_kind(&publish_manifest),
                "name": manifest_name(&publish_manifest),
                "version": manifest_version(&publish_manifest),
                "artifactDigest": format!("sha256:{sha256_hex}"),
                "createdAt": chrono::Utc::now().to_rfc3339(),
            });

            // The server verifies against sorted-key JSON bytes. This matches
            // serde_json's default map serialization in this crate as long as
            // we do not enable serde_json's preserve_order feature.
            let statement_bytes = serde_json::to_vec(&statement)?;
            let sig = signing_key.sign(&statement_bytes);
            let signature_b64 = base64::engine::general_purpose::STANDARD.encode(sig.to_bytes());

            Some(serde_json::json!({
                "author_signatures": [{
                    "algo": "ed25519",
                    "public_key_b64": stored.public_key_b64,
                    "signature_b64": signature_b64,
                    "statement_json": statement
                }]
            }))
        } else {
            None
        };

        let res = client
            .publish_package_from_path(&meta, &tar_path, &filename, finalize_extra)
            .await;

        let receipt = match res {
            Ok(r) => {
                s.ok("done");
                r
            }
            Err(e) => {
                s.err("upload failed");
                return Err(e.into());
            }
        };

        // Print receipt
        println!("✓ Published {}@{}", receipt.name, receipt.version);
        println!("  id:   {}", receipt.id);
        println!("  url:  {}", receipt.url);
        if !receipt.message.is_empty() {
            println!("  note: {}", receipt.message);
        }

        Ok(())
    }
}

// === Helpers ===

fn build_publish_metadata(
    manifest: JsonValue,
    sha256_hex: &str,
    size_bytes: u64,
    readme_payload: Option<JsonValue>,
    skill_manual_payload: Option<JsonValue>,
    license_payload: Option<JsonValue>,
    namespace_handle: Option<&str>,
) -> JsonValue {
    let mut meta = serde_json::json!({
        "manifest": manifest,
        "sha256": sha256_hex,
        "size": size_bytes,
        "readme": readme_payload,
        "skill_manual": skill_manual_payload,
        "license": license_payload,
        "client": {
            "product": "agentpm-cli",
            "version": env!("CARGO_PKG_VERSION"),
            "os": std::env::consts::OS,
            "arch": std::env::consts::ARCH,
        }
    });
    if let Some(handle) = namespace_handle
        && let Some(obj) = meta.as_object_mut()
    {
        obj.insert(
            "namespace_handle".into(),
            JsonValue::String(handle.to_owned()),
        );
    }
    meta
}

/// Create a .tar.gz containing:
/// - agent.json (as root/agent.json)
/// - entrypoint (preserve the relative path)
/// - files patterns (globs/dirs), preserving relative paths
fn package_tool(manifest: &ToolManifest, manifest_path: &Path) -> Result<PathBuf> {
    // Root directory where manifest lives
    let root = manifest_path.parent().unwrap_or(Path::new("."));
    let out_dir = root.join("target").join("agentpm");
    fs::create_dir_all(&out_dir).ok();

    let out_path = out_dir.join(artifact_filename(
        &manifest.name,
        &manifest.version,
        Some(&manifest.runtime),
    ));
    write_artifact_atomically(&out_path, |tar| {
        let mut member_count: usize = 0;

        {
            let mut header = tar::Header::new_gnu();
            let manifest_bytes = fs::read(manifest_path)?;
            header.set_size(manifest_bytes.len() as u64);
            header.set_mode(0o644);
            header.set_cksum();

            ensure_safe_tar_name("agent.json")?;
            member_count += 1;
            if member_count > MAX_TAR_ENTRIES {
                bail!("too many files in package (> {MAX_TAR_ENTRIES})");
            }

            tar.append_data(&mut header, "agent.json", manifest_bytes.as_slice())?;
        }

        let (ep_abs, ep_tar_name) = validate_and_locate_entrypoint(root, &manifest.entrypoint)?;
        append_checked(tar, &ep_abs, &ep_tar_name, &mut member_count)?;

        let mut seen: HashSet<String> = HashSet::new();
        seen.insert(ep_tar_name.clone());

        for pat in &manifest.files {
            let rel = Path::new(pat);
            let abs = root.join(rel);
            if !abs.exists() {
                warn!("files entry does not exist: {}", abs.display());
                continue;
            }

            if abs.is_file() {
                let tar_name = rel_to_tar_name(rel);
                if seen.insert(tar_name.clone()) {
                    append_checked(tar, &abs, &tar_name, &mut member_count)?;
                }
            } else {
                for entry in WalkDir::new(&abs).into_iter().filter_map(|e| e.ok()) {
                    let path_abs = entry.path();
                    if path_abs.is_dir() {
                        continue;
                    }

                    let rel_path = match path_abs.strip_prefix(root) {
                        Ok(r) => r.to_path_buf(),
                        Err(_) => continue,
                    };
                    let tar_name = rel_to_tar_name(&rel_path);

                    if tar_name.starts_with("target/agentpm/") {
                        continue;
                    }

                    if seen.insert(tar_name.clone()) {
                        append_checked(tar, path_abs, &tar_name, &mut member_count)?;
                    }
                }
            }
        }

        Ok(())
    })?;
    Ok(out_path)
}

fn package_agent(manifest: &AgentManifest, manifest_path: &Path) -> Result<PathBuf> {
    let root = manifest_path.parent().unwrap_or(Path::new("."));
    let out_dir = root.join("target").join("agentpm");
    fs::create_dir_all(&out_dir).ok();

    let out_path = out_dir.join(artifact_filename(&manifest.name, &manifest.version, None));
    write_artifact_atomically(&out_path, |tar| {
        let manifest_bytes = fs::read(manifest_path)?;
        let mut header = tar::Header::new_gnu();
        header.set_size(manifest_bytes.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();

        ensure_safe_tar_name("agent.json")?;
        tar.append_data(&mut header, "agent.json", manifest_bytes.as_slice())?;
        Ok(())
    })?;
    Ok(out_path)
}

fn package_template(manifest: &TemplateManifest, manifest_path: &Path) -> Result<PathBuf> {
    let root = manifest_path.parent().unwrap_or(Path::new("."));
    let out_dir = root.join("target").join("agentpm");
    fs::create_dir_all(&out_dir).ok();

    let out_path = out_dir.join(artifact_filename(&manifest.name, &manifest.version, None));
    write_artifact_atomically(&out_path, |tar| {
        let mut member_count: usize = 0;

        let manifest_bytes = fs::read(manifest_path)?;
        let mut header = tar::Header::new_gnu();
        header.set_size(manifest_bytes.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        ensure_safe_tar_name("agent.json")?;
        member_count += 1;
        tar.append_data(&mut header, "agent.json", manifest_bytes.as_slice())?;

        let files_root = Path::new(&manifest.template.files_root);
        let files_root_abs = root.join(files_root);
        if !files_root_abs.exists() {
            bail!(
                "template.files_root does not exist: {}",
                files_root_abs.display()
            );
        }

        for entry in WalkDir::new(&files_root_abs)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path_abs = entry.path();
            if path_abs.is_dir() {
                continue;
            }

            let rel_path = path_abs.strip_prefix(root).with_context(|| {
                format!(
                    "template file must stay within project root: {}",
                    path_abs.display()
                )
            })?;
            let tar_name = rel_to_tar_name(rel_path);

            if tar_name.starts_with("target/agentpm/") {
                continue;
            }

            append_checked(tar, path_abs, &tar_name, &mut member_count)?;
        }

        Ok(())
    })?;
    Ok(out_path)
}

fn package_skill(
    manifest: &SkillManifest,
    manifest_path: &Path,
    manifest_value: &serde_json::Value,
) -> Result<PathBuf> {
    let root = manifest_path.parent().unwrap_or(Path::new("."));
    let out_dir = root.join("target").join("agentpm");
    fs::create_dir_all(&out_dir).ok();

    let out_path = out_dir.join(artifact_filename(&manifest.name, &manifest.version, None));
    write_artifact_atomically(&out_path, |tar| {
        let mut member_count: usize = 0;

        let manifest_bytes = fs::read(manifest_path)?;
        let mut header = tar::Header::new_gnu();
        header.set_size(manifest_bytes.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        ensure_safe_tar_name("agent.json")?;
        member_count += 1;
        tar.append_data(&mut header, "agent.json", manifest_bytes.as_slice())?;

        let mut seen: HashSet<String> = HashSet::new();

        append_declared_skill_file(
            tar,
            root,
            &manifest.skill.entrypoint,
            &mut member_count,
            &mut seen,
        )?;

        for rel in &manifest.skill.references {
            append_declared_skill_file(tar, root, rel, &mut member_count, &mut seen)?;
        }

        for rel in &manifest.skill.scripts {
            append_declared_skill_file(tar, root, rel, &mut member_count, &mut seen)?;
        }

        if let Some(readme_path) = manifest_value.get("readme").and_then(|v| v.as_str()) {
            append_optional_skill_file(
                tar,
                root,
                "readme",
                readme_path,
                &mut member_count,
                &mut seen,
            )?;
        }

        if let Some(license_file) = pick_license_paths(manifest_value).1 {
            append_optional_skill_file(
                tar,
                root,
                "license.file",
                license_file,
                &mut member_count,
                &mut seen,
            )?;
        }

        Ok(())
    })?;
    Ok(out_path)
}

fn artifact_temp_path(out_path: &Path) -> PathBuf {
    let file_name = out_path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "artifact.tar.gz".to_string());
    out_path.with_file_name(format!("{file_name}.tmp"))
}

fn write_artifact_atomically<F>(out_path: &Path, build: F) -> Result<()>
where
    F: FnOnce(&mut TarBuilder<GzEncoder<File>>) -> Result<()>,
{
    let tmp_path = artifact_temp_path(out_path);
    let result: Result<()> = (|| {
        let f = fs::File::create(&tmp_path)
            .with_context(|| format!("creating {}", tmp_path.display()))?;
        let enc = GzEncoder::new(f, Compression::default());
        let mut tar = TarBuilder::new(enc);
        build(&mut tar)?;
        tar.finish()?;
        Ok(())
    })();

    match result {
        Ok(()) => {
            fs::rename(&tmp_path, out_path).with_context(|| {
                format!("renaming {} -> {}", tmp_path.display(), out_path.display())
            })?;
            Ok(())
        }
        Err(err) => {
            let _ = fs::remove_file(&tmp_path);
            Err(err)
        }
    }
}

fn append_declared_skill_file<W: Write>(
    tar: &mut TarBuilder<W>,
    root: &Path,
    rel: &str,
    member_count: &mut usize,
    seen: &mut HashSet<String>,
) -> Result<()> {
    let (abs, tar_name) = validate_declared_skill_path(root, rel)?;
    if seen.insert(tar_name.clone()) {
        append_checked(tar, &abs, &tar_name, member_count)?;
    }
    Ok(())
}

fn append_optional_skill_file<W: Write>(
    tar: &mut TarBuilder<W>,
    root: &Path,
    field_label: &str,
    rel: &str,
    member_count: &mut usize,
    seen: &mut HashSet<String>,
) -> Result<()> {
    let (abs, tar_name) = match validate_declared_skill_path(root, rel) {
        Ok(ok) => ok,
        Err(err) => {
            eprintln!("Warning: skipping {} '{}' ({}).", field_label, rel, err);
            return Ok(());
        }
    };
    if seen.insert(tar_name.clone()) {
        append_checked(tar, &abs, &tar_name, member_count)?;
    }
    Ok(())
}

fn validate_declared_skill_path(root: &Path, rel: &str) -> Result<(PathBuf, String)> {
    let rel_path = Path::new(rel);
    if rel_path.is_absolute() {
        bail!("skill file path must be relative: {}", rel_path.display());
    }
    for component in rel_path.components() {
        if matches!(
            component,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        ) {
            bail!("skill file path must stay within the package root: {}", rel);
        }
    }

    let abs = root.join(rel_path);
    if !abs.exists() {
        bail!("declared skill file not found: {}", abs.display());
    }
    let md = fs::metadata(&abs).with_context(|| format!("stat {}", abs.display()))?;
    if !md.is_file() {
        bail!("declared skill path is not a file: {}", abs.display());
    }

    let tar_name = rel_to_tar_name(rel_path);
    ensure_safe_tar_name(&tar_name)?;
    Ok((abs, tar_name))
}

fn append_checked<W: std::io::Write>(
    tar: &mut tar::Builder<W>,
    src: &Path,
    tar_name: &str,
    member_count: &mut usize,
) -> Result<()> {
    ensure_safe_tar_name(tar_name)?;
    if is_embedded_archive(tar_name) {
        bail!("embedded archive not allowed in package: {tar_name}");
    }
    *member_count += 1;
    if *member_count > MAX_TAR_ENTRIES {
        bail!("too many files in package (> {MAX_TAR_ENTRIES})");
    }
    append_path_to_tar_named(tar, src, tar_name)
}

fn is_embedded_archive(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    // handle double extensions like .tar.gz
    BLOCKED_EMBEDDED_EXTS.iter().any(|ext| lower.ends_with(ext))
}

fn ensure_safe_tar_name(name: &str) -> Result<()> {
    // forbid absolute paths and parent traversal
    if name.starts_with('/') {
        bail!("unsafe absolute path in archive: {name}");
    }
    if name.split('/').any(|seg| seg == "..") {
        bail!("unsafe parent traversal in archive path: {name}");
    }
    Ok(())
}

// Normalize a relative Path into a forward-slash tar name (portable across OSes)
fn rel_to_tar_name(p: &Path) -> String {
    p.components()
        .map(|c| c.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

fn is_valid_module_name(s: &str) -> bool {
    if s.is_empty() || s.starts_with('.') || s.ends_with('.') || s.contains("..") {
        return false;
    }
    s.chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.')
}

/// Resolve entrypoint.args to a concrete file:
/// - python -m pkg.mod  => pkg/mod/__main__.py OR pkg/mod.py
/// - otherwise          => first non-flag arg is the script path
fn validate_and_locate_entrypoint(
    root: &Path,
    entrypoint: &Entrypoint,
) -> Result<(PathBuf, String)> {
    let args: &[String] = &entrypoint.args;
    if args.is_empty() {
        bail!("`entrypoint.args` must have at least one element");
    }

    // 1) Python module mode: look for "-m" and take the next token as module
    if let Some(mpos) = args.iter().position(|a| a == "-m") {
        let module = args
            .get(mpos + 1)
            .ok_or_else(|| anyhow!("`python -m` requires a module name right after `-m`"))?;
        if !is_valid_module_name(module) {
            bail!("invalid Python module name for `-m`: {module}");
        }
        // map pkg.mod -> pkg/mod
        let mod_path = module.replace('.', "/");
        let candidate_pkg_main = Path::new(&mod_path).join("__main__.py");
        let candidate_module_py = Path::new(&mod_path).with_extension("py");

        let ep_rel = if root.join(&candidate_pkg_main).is_file() {
            candidate_pkg_main
        } else if root.join(&candidate_module_py).is_file() {
            candidate_module_py
        } else {
            bail!(
                "`python -m {module}`: could not find `{}/__main__.py` or `{}.py` under {}. \
                 Make sure your `files` includes the package/module.",
                mod_path,
                mod_path,
                root.display()
            );
        };

        if ep_rel.is_absolute()
            || ep_rel
                .components()
                .any(|c| matches!(c, Component::ParentDir))
        {
            bail!(
                "resolved entrypoint must be a relative path within the project: {}",
                ep_rel.display()
            );
        }

        let ep_abs = root.join(&ep_rel);
        let md = std::fs::metadata(&ep_abs)
            .with_context(|| format!("entrypoint file not found: {}", ep_abs.display()))?;
        if !md.is_file() {
            bail!("`entrypoint` is not a file: {}", ep_abs.display());
        }

        let tar_name = rel_to_tar_name(&ep_rel);
        return Ok((ep_abs, tar_name));
    }

    // 2) Script mode: first non-flag token is the script path
    let (idx, script) = args
        .iter()
        .enumerate()
        .find(|(_, a)| !a.starts_with('-'))
        .ok_or_else(|| {
            anyhow!("`entrypoint.args` must start with the script path; flags go after the script")
        })?;

    let ep_rel = Path::new(script);
    if ep_rel.is_absolute() {
        bail!(
            "`entrypoint.args[{idx}]` must be a relative path (got absolute: {})",
            ep_rel.display()
        );
    }
    if ep_rel
        .components()
        .any(|c| matches!(c, Component::ParentDir))
    {
        bail!(
            "`entrypoint.args[{idx}]` must not contain `..`: {}",
            ep_rel.display()
        );
    }

    let ep_abs = root.join(ep_rel);
    let md = std::fs::metadata(&ep_abs)
        .with_context(|| format!("entrypoint file not found: {}", ep_abs.display()))?;
    if !md.is_file() {
        bail!(
            "`entrypoint.args[{idx}]` is not a file: {}",
            ep_abs.display()
        );
    }

    let tar_name = rel_to_tar_name(ep_rel);
    Ok((ep_abs, tar_name))
}

fn append_path_to_tar_named<W: Write>(
    tar: &mut TarBuilder<W>,
    abs: &Path,
    name_in_tar: &str,
) -> Result<()> {
    let meta = symlink_metadata(abs).with_context(|| format!("stat {}", abs.display()))?;

    // Build a fresh GNU header
    let mut header = Header::new_gnu();

    // Normalize metadata for reproducibility
    header.set_uid(0);
    header.set_gid(0);
    header.set_mtime(0);

    // Mode: preserve exec bit on regular files, otherwise 0644; for symlinks mode is ignored
    // ----- Mode (platform-specific) -----
    #[cfg(unix)]
    {
        header.set_mode(mode_from_meta(&meta));
    }
    #[cfg(windows)]
    {
        header.set_mode(mode_from_meta_for_name(&meta, name_in_tar));
    }

    if meta.file_type().is_symlink() {
        // Tar symlink: size must be 0 and entry type is Symlink
        header.set_entry_type(EntryType::Symlink);
        header.set_size(0);
        // Read the link target (store as linkname inside the tar)
        let target = read_link(abs).with_context(|| format!("readlink {}", abs.display()))?;
        let target_str = target.to_string_lossy();
        header
            .set_link_name(&*target_str)
            .context("setting tar link name")?;
        header.set_cksum();
        // Append header only (no payload for symlink)
        tar.append_data(&mut header, name_in_tar, &mut std::io::empty())?;
        return Ok(());
    }

    if meta.is_file() {
        // Regular file: set size and stream the payload
        header.set_entry_type(EntryType::Regular);
        header.set_size(meta.len());
        header.set_cksum();
        let mut f = File::open(abs).with_context(|| format!("open {}", abs.display()))?;
        tar.append_data(&mut header, name_in_tar, &mut f)?;
        return Ok(());
    }

    if meta.is_dir() {
        // If you want to include explicit directory entries (optional)
        header.set_entry_type(EntryType::Directory);
        header.set_size(0);
        header.set_cksum();
        tar.append_data(
            &mut header,
            name_in_tar.trim_end_matches('/').to_string() + "/",
            &mut std::io::empty(),
        )?;
        return Ok(());
    }

    bail!("unsupported file type for {}", abs.display());
}

#[cfg(unix)]
fn mode_from_meta(meta: &Metadata) -> u32 {
    use std::os::unix::fs::PermissionsExt;
    let m = meta.permissions().mode();
    let exec = (m & 0o111) != 0;
    if exec { 0o755 } else { 0o644 }
}

#[cfg(windows)]
fn mode_from_meta_for_name(_meta: &Metadata, name_in_tar: &str) -> u32 {
    // Heuristic: treat common Windows “executables” as executable in the tar.
    // Everything else gets 0644.
    use std::ffi::OsStr;
    let ext = Path::new(name_in_tar)
        .extension()
        .and_then(OsStr::to_str)
        .map(|s| s.to_ascii_lowercase())
        .unwrap_or_default();
    match ext.as_str() {
        "exe" | "bat" | "cmd" | "ps1" | "com" => 0o755,
        _ => 0o644,
    }
}

fn file_digest_and_len(path: &Path) -> Result<(String, u64)> {
    let mut f = fs::File::open(path)?;
    let mut sha = Sha256::new();
    let mut len = 0_u64;
    let mut buf = [0_u8; 16 * 1024];
    loop {
        let n = f
            .read(&mut buf)
            .map_err(|e| anyhow!("hashing {}: {}", path.display(), e))?;
        if n == 0 {
            break;
        }
        sha.update(&buf[..n]);
        len += n as u64;
    }
    let hex = format!("{:x}", sha.finalize());
    Ok((hex, len))
}

fn runtime_suffix(runtime: &serde_json::Value) -> String {
    let t = runtime.get("type").and_then(|v| v.as_str()).unwrap_or("");
    let v = runtime
        .get("version")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if t.is_empty() {
        return "".into();
    }
    if v.is_empty() {
        format!("-{}", t)
    } else {
        format!("-{}{}", t, v.replace('.', ""))
    }
}

fn artifact_filename(name: &str, version: &str, runtime: Option<&serde_json::Value>) -> String {
    let suffix = runtime.map(runtime_suffix).unwrap_or_default();
    format!("{}-{}{}.tar.gz", name, version, suffix)
}

fn manifest_kind(manifest: &PublishManifest) -> &str {
    match manifest {
        PublishManifest::Tool(mf) => &mf.kind,
        PublishManifest::Agent(mf) => &mf.kind,
        PublishManifest::Template(mf) => &mf.kind,
        PublishManifest::Skill(mf) => &mf.kind,
    }
}

fn manifest_name(manifest: &PublishManifest) -> &str {
    match manifest {
        PublishManifest::Tool(mf) => &mf.name,
        PublishManifest::Agent(mf) => &mf.name,
        PublishManifest::Template(mf) => &mf.name,
        PublishManifest::Skill(mf) => &mf.name,
    }
}

fn manifest_version(manifest: &PublishManifest) -> &str {
    match manifest {
        PublishManifest::Tool(mf) => &mf.version,
        PublishManifest::Agent(mf) => &mf.version,
        PublishManifest::Template(mf) => &mf.version,
        PublishManifest::Skill(mf) => &mf.version,
    }
}

fn select_local_key(key_id_flag: Option<&str>) -> Result<(String, StoredKeyV1)> {
    let dir = keystore_dir()?;
    let mut found: Vec<(String, StoredKeyV1)> = Vec::new();

    if dir.exists() {
        for ent in fs::read_dir(&dir)? {
            let ent = ent?;
            if !ent.file_type()?.is_file() {
                continue;
            }
            if !ent.file_name().to_string_lossy().ends_with(".json") {
                continue;
            }

            let path = ent.path();
            match read_key_file(&path) {
                Ok(k) => {
                    let id = key_id_from_pub_b64(&k.public_key_b64)?;
                    found.push((id, k));
                }
                Err(_) => {
                    // Optionally log: eprintln!("Skipping unreadable key file: {}", path.display());
                    continue;
                }
            }
        }
    }

    if let Some(want) = key_id_flag {
        found
            .into_iter()
            .find(|(id, _)| id == want)
            .ok_or_else(|| anyhow!("No local key with id {}", want))
    } else {
        match found.len() {
            0 => Err(anyhow!("No local keys. Run `agentpm keys generate`.")),
            1 => Ok(found.into_iter().next().unwrap()),
            _ => {
                // Helpful hint if multiple
                let ids = found
                    .iter()
                    .map(|(id, k)| format!("{id}\t{}", k.label))
                    .collect::<Vec<_>>()
                    .join("\n");
                Err(anyhow!(
                    "Multiple keys present; pass --key-id <id>. Available:\n{}",
                    ids
                ))
            }
        }
    }
}

fn hex_sha256(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

fn read_utf8_with_cap(path: &Path, max_bytes: usize) -> Result<String> {
    let md = fs::metadata(path).with_context(|| format!("stat {}", path.display()))?;
    if md.len() as usize > max_bytes {
        bail!(
            "{} is too large ({} bytes > {} bytes)",
            path.display(),
            md.len(),
            max_bytes
        );
    }
    let bytes = fs::read(path).with_context(|| format!("read {}", path.display()))?;
    let s = String::from_utf8(bytes)
        .with_context(|| format!("{} is not valid UTF-8 text", path.display()))?;
    Ok(s)
}

fn discover_readme(base: &Path, from_manifest: Option<&str>) -> Option<(PathBuf, String)> {
    if let Some(p) = from_manifest {
        let abs = base.join(p);
        if abs.exists() {
            return Some((abs, p.to_string()));
        }
        return None;
    }
    // Auto-discover common names
    for cand in ["README.md", "README", "README.txt"] {
        let p = base.join(cand);
        if p.exists() {
            return Some((p, cand.to_string()));
        }
    }
    None
}

// Best-effort fetch of license paths from manifest JSON
fn pick_license_paths(manifest_json: &serde_json::Value) -> (Option<&str>, Option<&str>) {
    // returns (license_spdx, license_file)
    let spdx = manifest_json
        .get("license")
        .and_then(|l| l.get("spdx"))
        .and_then(|v| v.as_str());

    let file = manifest_json
        .get("license")
        .and_then(|l| l.get("file"))
        .and_then(|v| v.as_str());

    (spdx, file)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(prefix: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("agentpm-{prefix}-{unique}"));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn tar_entries(path: &Path) -> Vec<String> {
        let f = File::open(path).unwrap();
        let gz = flate2::read::GzDecoder::new(f);
        let mut ar = tar::Archive::new(gz);
        let mut out = Vec::new();
        for entry in ar.entries().unwrap() {
            let entry = entry.unwrap();
            out.push(entry.path().unwrap().to_string_lossy().into_owned());
        }
        out
    }

    #[test]
    fn validate_declared_skill_path_rejects_parent_dir_component() {
        let dir = temp_dir("skill-path-parent");
        let err = validate_declared_skill_path(&dir, "../secret.md").unwrap_err();
        assert!(format!("{err:#}").contains("skill file path must stay within the package root"));
    }

    #[test]
    fn validate_declared_skill_path_rejects_parent_dir_only_path() {
        let dir = temp_dir("skill-path-dotdot");
        let err = validate_declared_skill_path(&dir, "..").unwrap_err();
        assert!(format!("{err:#}").contains("skill file path must stay within the package root"));
    }

    #[test]
    fn validate_declared_skill_path_rejects_absolute_path() {
        let dir = temp_dir("skill-path-absolute");
        let err = validate_declared_skill_path(&dir, "/etc/passwd").unwrap_err();
        assert!(format!("{err:#}").contains("skill file path must be relative"));
    }

    #[test]
    fn publish_metadata_includes_namespace_handle_when_provided() {
        let meta = build_publish_metadata(
            serde_json::json!({"kind": "tool", "name": "demo", "version": "0.1.0"}),
            "abc123",
            42,
            None,
            None,
            None,
            Some("zack"),
        );

        assert_eq!(
            meta.get("namespace_handle").and_then(|v| v.as_str()),
            Some("zack")
        );
    }

    #[test]
    fn publish_metadata_omits_namespace_handle_when_not_provided() {
        let meta = build_publish_metadata(
            serde_json::json!({"kind": "tool", "name": "demo", "version": "0.1.0"}),
            "abc123",
            42,
            None,
            None,
            None,
            None,
        );

        assert!(meta.get("namespace_handle").is_none());
    }

    #[test]
    fn publish_metadata_includes_skill_manual_when_provided() {
        let meta = build_publish_metadata(
            serde_json::json!({"kind": "skill", "name": "demo-skill", "version": "0.1.0"}),
            "abc123",
            42,
            None,
            Some(serde_json::json!({
                "path": "SKILL.md",
                "sha256": "deadbeef",
                "content": "# Manual"
            })),
            None,
            None,
        );

        assert_eq!(
            meta.get("skill_manual")
                .and_then(|v| v.get("path"))
                .and_then(|v| v.as_str()),
            Some("SKILL.md")
        );
    }

    #[tokio::test]
    async fn publish_dry_run_succeeds_for_tool_manifest() {
        let dir = temp_dir("publish-tool");
        let manifest_path = dir.join("agent.json");
        let script_path = dir.join("main.py");
        let asset_path = dir.join("data.txt");

        fs::write(
            &manifest_path,
            serde_json::to_string_pretty(&serde_json::json!({
                "kind": "tool",
                "name": "tool-pub-test",
                "version": "0.1.0",
                "description": "test tool",
                "runtime": { "type": "python", "version": "3.11" },
                "entrypoint": {
                    "command": "python",
                    "args": ["main.py"]
                },
                "files": ["data.txt"],
                "inputs": { "type": "object" },
                "outputs": { "type": "object" }
            }))
            .unwrap()
                + "\n",
        )
        .unwrap();
        fs::write(&script_path, "print('hi')\n").unwrap();
        fs::write(&asset_path, "hello\n").unwrap();

        let args = PublishArgs {
            manifest: manifest_path.to_string_lossy().into_owned(),
            schema: Some(
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../../schemas/agentpm.manifest.schema.json")
                    .to_string_lossy()
                    .into_owned(),
            ),
            strict: false,
            dry_run: true,
            quiet: true,
            sign: false,
            key_id: None,
            token: Some("dummy-token".into()),
            namespace: None,
        };

        args.run("https://example.com".into()).await.unwrap();

        let tar_path = dir.join("target/agentpm/tool-pub-test-0.1.0-python311.tar.gz");
        assert!(tar_path.exists(), "expected {}", tar_path.display());
        let entries = tar_entries(&tar_path);
        assert!(entries.contains(&"agent.json".to_string()));
        assert!(entries.contains(&"main.py".to_string()));
        assert!(entries.contains(&"data.txt".to_string()));
    }

    #[tokio::test]
    async fn publish_dry_run_succeeds_for_agent_manifest() {
        let dir = temp_dir("publish-agent");
        let manifest_path = dir.join("agent.json");

        fs::write(
            &manifest_path,
            serde_json::to_string_pretty(&serde_json::json!({
                "kind": "agent",
                "name": "agent-pub-test",
                "version": "0.2.0",
                "description": "test agent",
                "tools": ["@zack/slack-post-message@0.1.0"],
                "skills": [],
                "knowledge": [],
                "memory": [],
                "profiles": [],
                "examples": [
                    { "title": "Example", "prompt": "Do the thing." }
                ]
            }))
            .unwrap()
                + "\n",
        )
        .unwrap();

        let args = PublishArgs {
            manifest: manifest_path.to_string_lossy().into_owned(),
            schema: Some(
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../../schemas/agentpm.manifest.schema.json")
                    .to_string_lossy()
                    .into_owned(),
            ),
            strict: false,
            dry_run: true,
            quiet: true,
            sign: false,
            key_id: None,
            token: Some("dummy-token".into()),
            namespace: None,
        };

        args.run("https://example.com".into()).await.unwrap();

        let tar_path = dir.join("target/agentpm/agent-pub-test-0.2.0.tar.gz");
        assert!(tar_path.exists(), "expected {}", tar_path.display());
        let entries = tar_entries(&tar_path);
        assert_eq!(entries, vec!["agent.json".to_string()]);
    }

    #[tokio::test]
    async fn publish_dry_run_succeeds_for_template_manifest() {
        let dir = temp_dir("publish-template");
        let manifest_path = dir.join("agent.json");
        let scaffold_dir = dir.join("template");
        let scaffold_readme = scaffold_dir.join("README.md");
        let scaffold_env = scaffold_dir.join(".env.example");
        fs::create_dir_all(&scaffold_dir).unwrap();

        fs::write(
            &manifest_path,
            serde_json::to_string_pretty(&serde_json::json!({
                "kind": "template",
                "name": "template-pub-test",
                "version": "0.3.0",
                "description": "test template",
                "template": {
                    "display_name": "Template Pub Test",
                    "use_case": "research",
                    "execution_surfaces": ["python-sdk"],
                    "stack": ["python"],
                    "files_root": "template",
                    "variables": [],
                    "dependencies": {
                        "tools": [],
                        "agents": []
                    },
                    "entrypoints": [
                        { "label": "Run", "command": "python main.py" }
                    ]
                }
            }))
            .unwrap()
                + "\n",
        )
        .unwrap();
        fs::write(&scaffold_readme, "# {{ project_name }}\n").unwrap();
        fs::write(&scaffold_env, "OPENAI_API_KEY=\n").unwrap();

        let args = PublishArgs {
            manifest: manifest_path.to_string_lossy().into_owned(),
            schema: Some(
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../../schemas/agentpm.manifest.schema.json")
                    .to_string_lossy()
                    .into_owned(),
            ),
            strict: false,
            dry_run: true,
            quiet: true,
            sign: false,
            key_id: None,
            token: Some("dummy-token".into()),
            namespace: None,
        };

        args.run("https://example.com".into()).await.unwrap();

        let tar_path = dir.join("target/agentpm/template-pub-test-0.3.0.tar.gz");
        assert!(tar_path.exists(), "expected {}", tar_path.display());
        let entries = tar_entries(&tar_path);
        assert!(entries.contains(&"agent.json".to_string()));
        assert!(entries.contains(&"template/README.md".to_string()));
        assert!(entries.contains(&"template/.env.example".to_string()));
    }

    #[tokio::test]
    async fn publish_dry_run_succeeds_for_minimal_skill_manifest() {
        let dir = temp_dir("publish-skill-minimal");
        let manifest_path = dir.join("agent.json");
        let skill_path = dir.join("SKILL.md");

        fs::write(
            &manifest_path,
            serde_json::to_string_pretty(&serde_json::json!({
                "kind": "skill",
                "name": "incident-commander",
                "version": "0.1.0",
                "description": "Incident response coordination playbook.",
                "skill": {
                    "entrypoint": "SKILL.md"
                }
            }))
            .unwrap()
                + "\n",
        )
        .unwrap();
        fs::write(&skill_path, "# Incident Commander\n").unwrap();

        let args = PublishArgs {
            manifest: manifest_path.to_string_lossy().into_owned(),
            schema: Some(
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../../schemas/agentpm.manifest.schema.json")
                    .to_string_lossy()
                    .into_owned(),
            ),
            strict: false,
            dry_run: true,
            quiet: true,
            sign: false,
            key_id: None,
            token: Some("dummy-token".into()),
            namespace: None,
        };

        args.run("https://example.com".into()).await.unwrap();

        let tar_path = dir.join("target/agentpm/incident-commander-0.1.0.tar.gz");
        assert!(tar_path.exists(), "expected {}", tar_path.display());
        let entries = tar_entries(&tar_path);
        assert_eq!(entries.len(), 2);
        assert!(entries.contains(&"agent.json".to_string()));
        assert!(entries.contains(&"SKILL.md".to_string()));
    }

    #[tokio::test]
    async fn publish_dry_run_succeeds_for_skill_with_references_and_scripts() {
        let dir = temp_dir("publish-skill-rich");
        let manifest_path = dir.join("agent.json");
        let skill_path = dir.join("SKILL.md");
        let references_dir = dir.join("references");
        let scripts_dir = dir.join("scripts");
        fs::create_dir_all(&references_dir).unwrap();
        fs::create_dir_all(&scripts_dir).unwrap();

        fs::write(
            &manifest_path,
            serde_json::to_string_pretty(&serde_json::json!({
                "kind": "skill",
                "name": "slack-incident-update",
                "version": "0.1.0",
                "description": "A playbook for posting incident updates to Slack.",
                "tools": [
                    { "name": "@zack/slack-post-message", "version": "0.1.1" }
                ],
                "skill": {
                    "entrypoint": "SKILL.md",
                    "references": [
                        "references/tool-contract.md",
                        "references/examples.md"
                    ],
                    "scripts": ["scripts/run.sh"]
                }
            }))
            .unwrap()
                + "\n",
        )
        .unwrap();
        fs::write(&skill_path, "# Slack Incident Update\n").unwrap();
        fs::write(references_dir.join("tool-contract.md"), "contract\n").unwrap();
        fs::write(references_dir.join("examples.md"), "examples\n").unwrap();
        fs::write(scripts_dir.join("run.sh"), "#!/bin/sh\necho ok\n").unwrap();

        let args = PublishArgs {
            manifest: manifest_path.to_string_lossy().into_owned(),
            schema: Some(
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../../schemas/agentpm.manifest.schema.json")
                    .to_string_lossy()
                    .into_owned(),
            ),
            strict: false,
            dry_run: true,
            quiet: true,
            sign: false,
            key_id: None,
            token: Some("dummy-token".into()),
            namespace: None,
        };

        args.run("https://example.com".into()).await.unwrap();

        let tar_path = dir.join("target/agentpm/slack-incident-update-0.1.0.tar.gz");
        let entries = tar_entries(&tar_path);
        assert_eq!(entries.len(), 5);
        assert!(entries.contains(&"agent.json".to_string()));
        assert!(entries.contains(&"SKILL.md".to_string()));
        assert!(entries.contains(&"references/tool-contract.md".to_string()));
        assert!(entries.contains(&"references/examples.md".to_string()));
        assert!(entries.contains(&"scripts/run.sh".to_string()));
    }

    #[tokio::test]
    async fn publish_dry_run_succeeds_for_skill_with_readme_and_license() {
        let dir = temp_dir("publish-skill-readme-license");
        let manifest_path = dir.join("agent.json");
        let skill_path = dir.join("SKILL.md");
        let readme_path = dir.join("README.md");
        let license_path = dir.join("LICENSE");

        fs::write(
            &manifest_path,
            serde_json::to_string_pretty(&serde_json::json!({
                "kind": "skill",
                "name": "governance-review",
                "version": "0.1.0",
                "description": "Governance review playbook.",
                "readme": "README.md",
                "license": {
                    "spdx": "MIT",
                    "file": "LICENSE"
                },
                "skill": {
                    "entrypoint": "SKILL.md",
                    "references": ["SKILL.md"]
                }
            }))
            .unwrap()
                + "\n",
        )
        .unwrap();
        fs::write(&skill_path, "# Governance Review\n").unwrap();
        fs::write(&readme_path, "# Readme\n").unwrap();
        fs::write(&license_path, "MIT License\n").unwrap();

        let args = PublishArgs {
            manifest: manifest_path.to_string_lossy().into_owned(),
            schema: Some(
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../../schemas/agentpm.manifest.schema.json")
                    .to_string_lossy()
                    .into_owned(),
            ),
            strict: false,
            dry_run: true,
            quiet: true,
            sign: false,
            key_id: None,
            token: Some("dummy-token".into()),
            namespace: None,
        };

        args.run("https://example.com".into()).await.unwrap();

        let tar_path = dir.join("target/agentpm/governance-review-0.1.0.tar.gz");
        let entries = tar_entries(&tar_path);
        assert!(entries.contains(&"agent.json".to_string()));
        assert!(entries.contains(&"SKILL.md".to_string()));
        assert!(entries.contains(&"README.md".to_string()));
        assert!(entries.contains(&"LICENSE".to_string()));
        assert_eq!(
            entries
                .iter()
                .filter(|entry| entry.as_str() == "SKILL.md")
                .count(),
            1
        );
    }

    #[tokio::test]
    async fn publish_dry_run_omits_undeclared_skill_files() {
        let dir = temp_dir("publish-skill-undeclared");
        let manifest_path = dir.join("agent.json");
        let skill_path = dir.join("SKILL.md");
        let ignored_path = dir.join("notes.txt");

        fs::write(
            &manifest_path,
            serde_json::to_string_pretty(&serde_json::json!({
                "kind": "skill",
                "name": "triage-operator",
                "version": "0.1.0",
                "description": "Triage operator playbook.",
                "skill": {
                    "entrypoint": "SKILL.md"
                }
            }))
            .unwrap()
                + "\n",
        )
        .unwrap();
        fs::write(&skill_path, "# Triage Operator\n").unwrap();
        fs::write(&ignored_path, "do not include\n").unwrap();

        let args = PublishArgs {
            manifest: manifest_path.to_string_lossy().into_owned(),
            schema: Some(
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../../schemas/agentpm.manifest.schema.json")
                    .to_string_lossy()
                    .into_owned(),
            ),
            strict: false,
            dry_run: true,
            quiet: true,
            sign: false,
            key_id: None,
            token: Some("dummy-token".into()),
            namespace: None,
        };

        args.run("https://example.com".into()).await.unwrap();

        let tar_path = dir.join("target/agentpm/triage-operator-0.1.0.tar.gz");
        let entries = tar_entries(&tar_path);
        assert!(entries.contains(&"SKILL.md".to_string()));
        assert!(!entries.contains(&"notes.txt".to_string()));
    }

    #[tokio::test]
    async fn publish_dry_run_fails_for_missing_declared_skill_file() {
        let dir = temp_dir("publish-skill-missing");
        let manifest_path = dir.join("agent.json");

        fs::write(
            &manifest_path,
            serde_json::to_string_pretty(&serde_json::json!({
                "kind": "skill",
                "name": "missing-file-skill",
                "version": "0.1.0",
                "description": "Missing file should fail.",
                "skill": {
                    "entrypoint": "SKILL.md"
                }
            }))
            .unwrap()
                + "\n",
        )
        .unwrap();

        let args = PublishArgs {
            manifest: manifest_path.to_string_lossy().into_owned(),
            schema: Some(
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../../schemas/agentpm.manifest.schema.json")
                    .to_string_lossy()
                    .into_owned(),
            ),
            strict: false,
            dry_run: true,
            quiet: true,
            sign: false,
            key_id: None,
            token: Some("dummy-token".into()),
            namespace: None,
        };

        let err = args.run("https://example.com".into()).await.unwrap_err();
        assert!(format!("{err:#}").contains("declared skill file not found"));
        assert!(
            !dir.join("target/agentpm/missing-file-skill-0.1.0.tar.gz")
                .exists(),
            "failed packaging should not leave a final tarball behind"
        );
    }

    #[tokio::test]
    async fn publish_dry_run_fails_for_unsafe_skill_path() {
        let dir = temp_dir("publish-skill-unsafe");
        let manifest_path = dir.join("agent.json");

        fs::write(
            &manifest_path,
            serde_json::to_string_pretty(&serde_json::json!({
                "kind": "skill",
                "name": "unsafe-file-skill",
                "version": "0.1.0",
                "description": "Unsafe file should fail.",
                "skill": {
                    "entrypoint": "../SKILL.md"
                }
            }))
            .unwrap()
                + "\n",
        )
        .unwrap();

        let args = PublishArgs {
            manifest: manifest_path.to_string_lossy().into_owned(),
            schema: Some(
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../../schemas/agentpm.manifest.schema.json")
                    .to_string_lossy()
                    .into_owned(),
            ),
            strict: false,
            dry_run: true,
            quiet: true,
            sign: false,
            key_id: None,
            token: Some("dummy-token".into()),
            namespace: None,
        };

        let err = args.run("https://example.com".into()).await.unwrap_err();
        assert!(format!("{err:#}").contains("Manifest validation failed"));
    }
}
