use crate::prelude::*;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::{fs, io::Write};

pub fn write_atomic(path: &Path, contents: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).ok();
    }
    let tmp = path.with_extension("tmp");
    {
        let mut f = fs::File::create(&tmp).with_context(|| format!("create {}", tmp.display()))?;
        f.write_all(contents.as_bytes())
            .with_context(|| format!("writing {}", tmp.display()))?;
        let _ = f.sync_all();
    }
    fs::rename(&tmp, path)
        .with_context(|| format!("renaming {} -> {}", tmp.display(), path.display()))?;
    Ok(())
}

/// Ensure each directory exists (creates parents as needed).
/// - Accepts relative or absolute paths
/// - Dedupes paths to avoid redundant work
pub fn ensure_dirs<P: AsRef<Path>>(paths: &[P]) -> Result<()> {
    let mut seen: HashSet<PathBuf> = HashSet::new();

    for p in paths {
        let p = p.as_ref();
        if p.as_os_str().is_empty() {
            continue;
        }

        // Normalize relative paths against CWD without touching the filesystem
        let abs = if p.is_absolute() {
            p.to_path_buf()
        } else {
            std::env::current_dir()
                .context("reading current dir")?
                .join(p)
        };

        if seen.insert(abs.clone()) {
            std::fs::create_dir_all(&abs)
                .with_context(|| format!("creating directory {}", abs.display()))?;
        }
    }

    Ok(())
}
