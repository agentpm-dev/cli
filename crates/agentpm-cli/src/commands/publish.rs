use crate::auth;
use crate::manifest::{
    ToolManifest, load_manifest_value, parse_tool_manifest, resolve_schema_source,
    validate_manifest_value,
};
use crate::prelude::*;
use crate::ui::Step;
use anyhow::{anyhow, bail};
use flate2::{Compression, write::GzEncoder};
use sha2::{Digest, Sha256};
use std::io::IsTerminal;
use std::path::Component;
use std::{
    collections::HashSet,
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
};
use tar::Builder as TarBuilder;
use tracing::{info, warn};
use walkdir::WalkDir;

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
}

impl PublishArgs {
    pub async fn run(self, base_url: String) -> Result<()> {
        let cfg = Config::load(base_url.clone())?;

        // Quiet if user asked OR stderr is not a terminal (piped/redirected)
        let auto_quiet = !std::io::stderr().is_terminal();
        let quiet = self.quiet || auto_quiet;

        // 1) Load PAT (login is required; for MVP we use cached token)
        let mut s = Step::new("Reading credentials", quiet);
        let token = match auth::read_token(&cfg)? {
            Some(t) => {
                s.ok("");
                t.access_token
            }
            None => {
                s.err("not logged in");
                return Err(anyhow!(
                    "Not logged in. Run `agentpm login` to store a Personal Access Token."
                ));
            }
        };

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

        // 3) Strongly typed and kind enforcement (shared)
        let mf = parse_tool_manifest(&manifest_value)?;

        // 4) Package files: agent.json + entrypoint + declared files
        let mut s = Step::new("Packaging files", quiet);
        let tar_path = package_tool(&mf, &manifest_path).context("packaging files into tar.gz")?;
        let (sha256_hex, size_bytes) = file_digest_and_len(&tar_path)?;
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

        if self.dry_run {
            println!("Dry-run: package created at {}", tar_path.display());
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

        let meta = serde_json::json!({
            "manifest": manifest_value,
            "sha256": sha256_hex,
            "size": size_bytes,
            "client": {
                "product": "agentpm-cli",
                "version": env!("CARGO_PKG_VERSION"),
                "os": std::env::consts::OS,
                "arch": std::env::consts::ARCH,
            }
        });
        let filename = artifact_filename(&mf.name, &mf.version, &mf.runtime);

        let res = client
            .publish_tool_from_path(&meta, &tar_path, &filename)
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

/// Create a .tar.gz containing:
/// - agent.json (as root/agent.json)
/// - entrypoint (preserve the relative path)
/// - files patterns (globs/dirs), preserving relative paths
fn package_tool(manifest: &ToolManifest, manifest_path: &Path) -> Result<PathBuf> {
    // Root directory where manifest lives
    let root = manifest_path.parent().unwrap_or(Path::new("."));
    let out_dir = root.join("target").join("agentpm");
    fs::create_dir_all(&out_dir).ok();

    let out_path = out_dir.join(format!("{}-{}.tar.gz", manifest.name, manifest.version));
    let f =
        fs::File::create(&out_path).with_context(|| format!("creating {}", out_path.display()))?;
    let enc = GzEncoder::new(f, Compression::default());
    let mut tar = TarBuilder::new(enc);

    // 1) agent.json
    {
        let mut header = tar::Header::new_gnu();
        let manifest_bytes = fs::read(manifest_path)?;
        header.set_size(manifest_bytes.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        tar.append_data(&mut header, "agent.json", manifest_bytes.as_slice())?;
    }

    // 2) entrypoint
    let (ep_abs, ep_tar_name) = validate_and_locate_entrypoint(root, &manifest.entrypoint)?;
    append_path_to_tar_named(&mut tar, &ep_abs, &ep_tar_name)?;

    // Start dedup set using normalized tar names
    let mut seen: HashSet<String> = HashSet::new();
    seen.insert(ep_tar_name.clone());

    // 3) files array (dirs/globs)
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
                append_path_to_tar_named(&mut tar, &abs, &tar_name)?;
            }
        } else {
            for entry in WalkDir::new(&abs).into_iter().filter_map(|e| e.ok()) {
                let path_abs = entry.path();
                if path_abs.is_dir() {
                    continue;
                }

                // Compute a project-relative path then normalize to a tar name
                let rel_path = match path_abs.strip_prefix(root) {
                    Ok(r) => r.to_path_buf(),
                    Err(_) => {
                        // Outside the project root, skip (extra safety)
                        continue;
                    }
                };
                let tar_name = rel_to_tar_name(&rel_path);

                // Exclude our packaging output
                if tar_name.starts_with("target/agentpm/") {
                    continue;
                }

                if seen.insert(tar_name.clone()) {
                    append_path_to_tar_named(&mut tar, path_abs, &tar_name)?;
                }
            }
        }
    }

    tar.finish()?;
    Ok(out_path)
}

// Normalize a relative Path into a forward-slash tar name (portable across OSes)
fn rel_to_tar_name(p: &Path) -> String {
    p.components()
        .map(|c| c.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

fn validate_and_locate_entrypoint(root: &Path, entrypoint: &str) -> Result<(PathBuf, String)> {
    let ep_rel = Path::new(entrypoint);

    // Must be a relative path with no parent traversal
    if ep_rel.is_absolute() {
        bail!(
            "`entrypoint` must be a relative path (got absolute: {})",
            ep_rel.display()
        );
    }
    if ep_rel
        .components()
        .any(|c| matches!(c, Component::ParentDir))
    {
        bail!("`entrypoint` must not contain `..`: {}", ep_rel.display());
    }

    let ep_abs = root.join(ep_rel);
    let md = std::fs::metadata(&ep_abs)
        .with_context(|| format!("entrypoint file not found: {}", ep_abs.display()))?;
    if !md.is_file() {
        bail!("`entrypoint` is not a file: {}", ep_abs.display());
    }

    let tar_name = rel_to_tar_name(ep_rel); // exactly matches manifest string, normalized
    Ok((ep_abs, tar_name))
}

fn append_path_to_tar_named<W: Write>(
    tar: &mut TarBuilder<W>,
    abs: &Path,
    name_in_tar: &str,
) -> Result<()> {
    let mut header = tar::Header::new_gnu();
    let md = std::fs::metadata(abs)?;
    header.set_size(md.len());
    header.set_mode(0o644);
    header.set_cksum();
    let mut f = std::fs::File::open(abs)?;
    tar.append_data(&mut header, name_in_tar, &mut f)?;
    Ok(())
}

fn file_digest_and_len(path: &Path) -> Result<(String, u64)> {
    let mut f = fs::File::open(path)?;
    let mut sha = Sha256::new();
    let len = io::copy(&mut f, &mut sha::Writer(&mut sha))
        .map_err(|e| anyhow!("hashing {}: {}", path.display(), e))?;
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

fn artifact_filename(name: &str, version: &str, runtime: &serde_json::Value) -> String {
    format!("{}-{}{}.tar.gz", name, version, runtime_suffix(runtime))
}

// helper to use io::copy into a hasher
mod sha {
    use sha2::Digest;
    pub struct Writer<'a, D: Digest>(pub &'a mut D);
    impl<'a, D: Digest> std::io::Write for Writer<'a, D> {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.0.update(buf);
            Ok(buf.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
}
