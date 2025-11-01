// cli/src/semver/update.rs
use anyhow::{Result, anyhow};
use serde_json::{Map, Value};

/// Update agent.json's "tools" array to include/update the CLI spec.
/// - `spec`: "@owner/name@<range>" or "@owner/name" (range defaults to "*")
/// - `update_range`: if false and an existing entry has a different range, returns Err.
///   Returns true if agent.json was modified.
pub fn maybe_update_agent_json(meta: &mut Value, spec: &str, update_range: bool) -> Result<bool> {
    let (spec_pkg, spec_range) = parse_cli_spec(spec)?;
    let desired_range = normalize_range(&spec_range); // None means "no version field" (equiv to "*")

    // Ensure "tools" is an array
    if !meta.get("tools").map(|v| v.is_array()).unwrap_or(false) {
        // create tools: []
        meta.as_object_mut()
            .ok_or_else(|| anyhow!("agent.json root must be a JSON object"))?
            .insert("tools".to_string(), Value::Array(vec![]));
    }

    let tools = meta
        .get_mut("tools")
        .and_then(Value::as_array_mut)
        .ok_or_else(|| anyhow!("agent.json tools must be an array"))?;

    // Look for existing entry matching the same package name (exact match on string)
    let mut found_idx: Option<usize> = None;
    let mut existing_range: Option<String> = None;

    for (i, item) in tools.iter().enumerate() {
        match parse_tools_item(item) {
            Some((name, rng)) if name == spec_pkg => {
                found_idx = Some(i);
                existing_range = rng;
                break;
            }
            _ => {}
        }
    }

    match (found_idx, existing_range) {
        // Not found → append a new entry
        (None, _) => {
            let new_item = build_tools_item(&spec_pkg, desired_range.as_deref());
            tools.push(new_item);
            Ok(true)
        }

        // Found, same effective range → nothing to do
        (Some(_idx), ref rng) if normalize_range_opt(rng.as_deref()) == desired_range => Ok(false),

        // Found, different range → require --update-range
        (Some(idx), _) => {
            if !update_range {
                return Err(anyhow!(
                    "Tool {} already exists in agent.json with a different version range. Pass --update-range to update it.",
                    spec_pkg
                ));
            }
            // Replace with object form { name, version? }
            let new_item = build_tools_item(&spec_pkg, desired_range.as_deref());
            if let Some(slot) = tools.get_mut(idx) {
                *slot = new_item;
            }
            Ok(true)
        }
    }
}

/// Build a canonical tools item (object form).
/// If `range` is None or "*", omit the "version" field.
fn build_tools_item(package: &str, range: Option<&str>) -> Value {
    let mut m = Map::new();
    m.insert("name".to_string(), Value::String(package.to_string()));
    if let Some(r) = range
        && r != "*"
    {
        m.insert("version".to_string(), Value::String(r.to_string()));
    }
    Value::Object(m)
}

/// Parse a tools array item from agent.json into (name, range?)
/// Supports:
/// - "summarize"
/// - "@zack/summarize"
/// - "@zack/summarize@^1.2"  (string shorthand if you allow it)
/// - { "name": "@zack/summarize", "version": "^1.2" }
fn parse_tools_item(v: &Value) -> Option<(String, Option<String>)> {
    match v {
        Value::String(s) => parse_string_name_and_range(s),
        Value::Object(m) => {
            let name = m.get("name")?.as_str()?.to_string();
            let range = m
                .get("version")
                .and_then(|x| x.as_str())
                .map(|s| s.to_string());
            Some((name, range))
        }
        _ => None,
    }
}

/// Parse CLI spec "@owner/name@range" or "@owner/name".
fn parse_cli_spec(spec: &str) -> Result<(String, String)> {
    // Split on the last '@' to allow namespaced packages
    let (pkg, rng_opt) = split_name_and_optional_range(spec);
    if !pkg.starts_with('@') || !pkg.contains('/') {
        return Err(anyhow!(
            "Invalid spec '{}'. Expected @owner/name or @owner/name@<range>.",
            spec
        ));
    }
    Ok((pkg, rng_opt.unwrap_or_else(|| "*".to_string())))
}

/// Parse a string that might be:
/// - "summarize"
/// - "@zack/summarize"
/// - "@zack/summarize@^1.2"
fn parse_string_name_and_range(s: &str) -> Option<(String, Option<String>)> {
    let (name, range) = split_name_and_optional_range(s);
    Some((name, range))
}

fn split_name_and_optional_range(s: &str) -> (String, Option<String>) {
    let s = s.trim();

    if let Some(idx) = s.rfind('@') {
        // If the last '@' is the leading scope marker (idx == 0)
        // OR the part before it doesn't contain '/', it's not a version separator.
        if idx == 0 || !s[..idx].contains('/') {
            return (s.to_string(), None);
        }

        let name = &s[..idx];
        let range_part = s[idx + 1..].trim();

        if range_part.is_empty() {
            // Treat trailing '@' as "no range" (caller can default to "*")
            (name.to_string(), None)
        } else {
            (name.to_string(), Some(range_part.to_string()))
        }
    } else {
        (s.to_string(), None)
    }
}

/// Normalize an optional range:
/// - None or "*" → None (meaning omit "version")
/// - Otherwise keep as-is
fn normalize_range(range: &str) -> Option<String> {
    let r = range.trim();
    if r.is_empty() || r == "*" {
        None
    } else {
        Some(r.to_string())
    }
}

/// Same as `normalize_range` but for Option<&str>
fn normalize_range_opt(r: Option<&str>) -> Option<String> {
    match r {
        None => None,
        Some(s) => normalize_range(s),
    }
}
