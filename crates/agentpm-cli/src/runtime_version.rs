use anyhow::{Context, Result, bail};
use semver::Version;

pub(crate) fn parse_runtime_version(raw: &str) -> Result<Version> {
    let normalized = raw
        .trim()
        .strip_prefix(">=")
        .or_else(|| raw.trim().strip_prefix('='))
        .unwrap_or_else(|| raw.trim())
        .trim();
    let parts = normalized.split('.').collect::<Vec<_>>();
    if parts.is_empty() || parts.len() > 3 {
        bail!("invalid runtime version");
    }
    let major = parts[0].parse::<u64>().context("invalid major version")?;
    let minor = parts
        .get(1)
        .map(|part| part.parse::<u64>())
        .transpose()
        .context("invalid minor version")?
        .unwrap_or(0);
    let patch = parts
        .get(2)
        .map(|part| part.parse::<u64>())
        .transpose()
        .context("invalid patch version")?
        .unwrap_or(0);
    Ok(Version::new(major, minor, patch))
}

pub(crate) fn extract_runtime_version(output: &str) -> Option<Version> {
    let bytes = output.as_bytes();
    for start in 0..bytes.len() {
        if !bytes[start].is_ascii_digit() {
            continue;
        }
        let mut end = start + 1;
        while end < bytes.len() && (bytes[end].is_ascii_digit() || bytes[end] == b'.') {
            end += 1;
        }
        let candidate = &output[start..end].trim_matches('.');
        if let Ok(version) = parse_runtime_version(candidate) {
            return Some(version);
        }
    }
    None
}
