use crate::auth::read_key_file;
use crate::commands::keys::key_id_from_pub_b64;
use crate::keys::signing::{
    StoredKeyV1, decrypt_private, keystore_dir, prompt_passphrase_with_fallback,
};
use crate::manifest::{
    Entrypoint, ToolManifest, load_manifest_value, parse_tool_manifest, resolve_schema_source,
    validate_manifest_value,
};
use crate::prelude::*;
use crate::ui::Step;
use anyhow::{anyhow, bail};
use base64::Engine;
use ed25519_dalek::{Signer, SigningKey};
use flate2::{Compression, write::GzEncoder};
use sha2::{Digest, Sha256};
use std::fs::{File, symlink_metadata, read_link};
use std::io::IsTerminal;
use std::os::unix::fs::MetadataExt;
use std::path::Component;
use std::{
    collections::HashSet,
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
};
use tar::{Builder as TarBuilder, Header, EntryType};
use tracing::{info, warn};
use walkdir::WalkDir;

const MAX_ARTIFACT_BYTES: u64 = 3 * 1024 * 1024 * 1024; // 3 GB
const MAX_TAR_ENTRIES: usize = 15_000;
static BLOCKED_EMBEDDED_EXTS: &[&str] = &[
    ".zip", ".whl", ".7z", ".rar", ".tar", ".tgz", ".tar.gz",
];

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

        // 3) Strongly typed and kind enforcement (shared)
        let mf = parse_tool_manifest(&manifest_value)?;

        // 4) Package files: agent.json + entrypoint + declared files
        let mut s = Step::new("Packaging files", quiet);
        let tar_path = package_tool(&mf, &manifest_path).context("packaging files into tar.gz")?;
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
            // TODO: namespace_handle eventually when a user can have more than one (orgs + user)
            "client": {
                "product": "agentpm-cli",
                "version": env!("CARGO_PKG_VERSION"),
                "os": std::env::consts::OS,
                "arch": std::env::consts::ARCH,
            }
        });
        let filename = artifact_filename(&mf.name, &mf.version, &mf.runtime);

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
                "type": "agentpm.tool.signature.v1",
                "name": mf.name,
                "version": mf.version,
                "artifactDigest": format!("sha256:{sha256_hex}"),
                "createdAt": chrono::Utc::now().to_rfc3339(),
            });

            // Canonical-ish JSON (stable enough for our purpose)
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
            .publish_tool_from_path(&meta, &tar_path, &filename, finalize_extra)
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

    let mut member_count: usize = 0;

    // 1) agent.json
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

    // 2) entrypoint
    let (ep_abs, ep_tar_name) = validate_and_locate_entrypoint(root, &manifest.entrypoint)?;
    append_checked(&mut tar, &ep_abs, &ep_tar_name, &mut member_count)?;

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
                append_checked(&mut tar, &abs, &tar_name, &mut member_count)?;
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
                    append_checked(&mut tar, path_abs, &tar_name, &mut member_count)?;
                }
            }
        }
    }

    tar.finish()?;
    Ok(out_path)
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
    if name.starts_with('/') { bail!("unsafe absolute path in archive: {name}"); }
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
    #[cfg(unix)]
    let mode = {
        let m = meta.mode();
        // Keep rw bits, but preserve execute bits if present; default to 0644
        let exec = m & 0o111 != 0;
        if exec { 0o755 } else { 0o644 }
    };
    #[cfg(not(unix))]
    let mode = 0o644;
    header.set_mode(mode);

    if meta.file_type().is_symlink() {
        // Tar symlink: size must be 0 and entry type is Symlink
        header.set_entry_type(EntryType::Symlink);
        header.set_size(0);
        // Read the link target (store as linkname inside the tar)
        let target = read_link(abs)
            .with_context(|| format!("readlink {}", abs.display()))?;
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
        let mut f = File::open(abs)
            .with_context(|| format!("open {}", abs.display()))?;
        tar.append_data(&mut header, name_in_tar, &mut f)?;
        return Ok(());
    }

    if meta.is_dir() {
        // If you want to include explicit directory entries (optional)
        header.set_entry_type(EntryType::Directory);
        header.set_size(0);
        header.set_cksum();
        tar.append_data(&mut header, name_in_tar.trim_end_matches('/').to_string() + "/", &mut std::io::empty())?;
        return Ok(());
    }

    bail!("unsupported file type for {}", abs.display());
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
