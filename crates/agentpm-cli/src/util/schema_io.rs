use anyhow::{Context, Result};
use serde_json::Value;
use std::{
    fs,
    path::{Path, PathBuf},
};

pub fn load_schema_value(source: &str) -> Result<Value> {
    if source.starts_with("http://") || source.starts_with("https://") {
        // TODO: Minimal blocking fetch for MVP. You can switch to reqwest async if you prefer.
        let body = ureq::get(source).call()?.into_string()?;
        Ok(serde_json::from_str(&body).context("Invalid schema JSON")?)
    } else {
        let s = fs::read_to_string(source).with_context(|| format!("read {}", source))?;
        Ok(serde_json::from_str(&s).context("Invalid schema JSON")?)
    }
}

/// Returns (value, raw_string) for potential rewriting on --fix
pub fn load_json(path: &Path) -> Result<(serde_json::Value, String)> {
    let raw =
        fs::read_to_string(path).with_context(|| format!("Failed to read {}", path.display()))?;
    let v: Value = serde_json::from_str(&raw)
        .with_context(|| format!("Invalid JSON in {}", path.display()))?;
    Ok((v, raw))
}

/// Resolve which manifest files to lint based on user args.
/// Rules:
/// - No args → ["./agent.json"]
/// - Dir arg → <dir>/agent.json
/// - File arg → that file (must be named agent.json)
/// - Glob arg → all matches
pub fn discover_manifest_files(args: &[String]) -> Result<Vec<PathBuf>> {
    if args.is_empty() {
        let p = PathBuf::from("agent.json");
        return Ok(if p.exists() { vec![p] } else { vec![] });
    }

    let mut out = Vec::new();
    for a in args {
        // crude glob detect
        let looks_like_glob = a.contains('*') || a.contains('?') || a.contains('[');
        if looks_like_glob {
            // optional: only if you kept globwalk, else remove this branch
            let (base, pat) = split_pattern(a);
            let walker = globwalk::GlobWalkerBuilder::from_patterns(base, &[pat])
                .case_insensitive(true)
                .max_depth(20)
                .build()?;
            for e in walker.into_iter().filter_map(Result::ok) {
                out.push(e.path().to_path_buf());
            }
            continue;
        }

        let p = PathBuf::from(a);
        if p.is_dir() {
            let candidate = p.join("agent.json");
            if candidate.exists() {
                out.push(candidate);
            }
        } else if p.is_file() {
            if p.file_name().and_then(|s| s.to_str()) == Some("agent.json") {
                out.push(p);
            }
        } else {
            // treat as file path that doesn't exist yet — do nothing
        }
    }

    out.sort();
    out.dedup();
    Ok(out)
}

fn split_pattern(pat: &str) -> (&str, &str) {
    if let Some(pos) = pat.find('/') {
        (&pat[..pos], &pat[pos + 1..])
    } else {
        (".", pat)
    }
}
