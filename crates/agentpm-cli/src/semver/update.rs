// cli/src/semver/update.rs
use anyhow::{Result, anyhow};
use serde_json::{Map, Value};

/// Update agent.json's "tools" array to include/update the CLI spec.
/// - `spec`: "@owner/name@<range>" or "@owner/name" (range defaults to "*")
/// - `update_range`: if false and an existing entry has a different range, returns Err.
///   Returns true if agent.json was modified.
pub fn maybe_update_agent_json(meta: &mut Value, spec: &str, update_range: bool) -> Result<bool> {
    maybe_update_manifest_dependency(meta, "tools", "Tool", spec, update_range)
}

pub fn maybe_update_manifest_dependency(
    meta: &mut Value,
    field: &str,
    label: &str,
    spec: &str,
    update_range: bool,
) -> Result<bool> {
    let desired = desired_dependency_slot(spec)?;

    if !meta.get(field).map(|v| v.is_array()).unwrap_or(false) {
        meta.as_object_mut()
            .ok_or_else(|| anyhow!("agent.json root must be a JSON object"))?
            .insert(field.to_string(), Value::Array(vec![]));
    }

    let dependencies = meta
        .get_mut(field)
        .and_then(Value::as_array_mut)
        .ok_or_else(|| anyhow!("agent.json {} must be an array", field))?;

    // Look for existing entry matching the same package name (exact match on string)
    let mut found_idx: Option<usize> = None;
    let mut existing_range: Option<String> = None;

    for (i, item) in dependencies.iter().enumerate() {
        match parse_dependency_item(item) {
            Some(existing) if existing.package == desired.package => {
                found_idx = Some(i);
                existing_range = existing.range;
                break;
            }
            _ => {}
        }
    }

    match (found_idx, existing_range) {
        // Not found → append a new entry
        (None, _) => {
            dependencies.push(build_dependency_item(&desired));
            Ok(true)
        }

        // Found, same effective range → nothing to do
        (Some(_idx), ref rng) if normalize_range_opt(rng.as_deref()) == desired.range => Ok(false),

        // Found, different range → require --update-range
        (Some(idx), _) => {
            if !update_range {
                return Err(dependency_range_change_error(label, &desired.package));
            }
            if let Some(slot) = dependencies.get_mut(idx) {
                *slot = build_dependency_item(&desired);
            }
            Ok(true)
        }
    }
}

pub fn maybe_update_manifest_singular_dependency(
    meta: &mut Value,
    field: &str,
    label: &str,
    spec: &str,
    update_range: bool,
) -> Result<bool> {
    let desired = desired_dependency_slot(spec)?;

    let root = meta
        .as_object_mut()
        .ok_or_else(|| anyhow!("agent.json root must be a JSON object"))?;

    let existing = root.get(field).cloned().unwrap_or(Value::Null);
    let Some(existing_slot) = parse_dependency_item(&existing) else {
        root.insert(field.to_string(), build_dependency_item(&desired));
        return Ok(true);
    };

    if existing_slot.package != desired.package {
        root.insert(field.to_string(), build_dependency_item(&desired));
        return Ok(true);
    }

    if normalize_range_opt(existing_slot.range.as_deref()) == desired.range {
        return Ok(false);
    }

    if !update_range {
        return Err(dependency_range_change_error(label, &desired.package));
    }

    root.insert(field.to_string(), build_dependency_item(&desired));
    Ok(true)
}

struct DependencySlot {
    package: String,
    range: Option<String>,
}

fn desired_dependency_slot(spec: &str) -> Result<DependencySlot> {
    let (package, range) = parse_cli_spec(spec)?;
    Ok(DependencySlot {
        package,
        range: normalize_range(&range),
    })
}

fn dependency_range_change_error(label: &str, package: &str) -> anyhow::Error {
    anyhow!(
        "{} {} already exists in agent.json with a different version range. Pass --update-range to update it.",
        label,
        package
    )
}

/// Build a canonical dependency item (object form).
/// If `range` is None or "*", omit the "version" field.
fn build_dependency_item(slot: &DependencySlot) -> Value {
    let mut m = Map::new();
    m.insert("name".to_string(), Value::String(slot.package.clone()));
    if let Some(r) = slot.range.as_deref()
        && r != "*"
    {
        m.insert("version".to_string(), Value::String(r.to_string()));
    }
    Value::Object(m)
}

/// Parse a dependency item from agent.json into (name, range?)
/// Supports:
/// - "summarize"
/// - "@zack/summarize"
/// - "@zack/summarize@^1.2"  (string shorthand if you allow it)
/// - { "name": "@zack/summarize", "version": "^1.2" }
fn parse_dependency_item(v: &Value) -> Option<DependencySlot> {
    match v {
        Value::String(s) => {
            let (package, range) = parse_string_name_and_range(s)?;
            Some(DependencySlot { package, range })
        }
        Value::Object(m) => {
            let package = m.get("name")?.as_str()?.to_string();
            let range = m
                .get("version")
                .and_then(|x| x.as_str())
                .map(|s| s.to_string());
            Some(DependencySlot { package, range })
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

#[cfg(test)]
mod tests {
    use super::{
        maybe_update_agent_json, maybe_update_manifest_dependency,
        maybe_update_manifest_singular_dependency,
    };
    use serde_json::json;

    #[test]
    fn maybe_update_agent_json_appends_tool_dependency() {
        let mut manifest = json!({
            "kind": "agent",
            "name": "support-agent",
            "version": "0.1.0",
            "tools": []
        });

        let changed =
            maybe_update_agent_json(&mut manifest, "@zack/summarize@^0.1", false).unwrap();

        assert!(changed);
        assert_eq!(
            manifest["tools"],
            json!([{"name":"@zack/summarize","version":"^0.1"}])
        );
    }

    #[test]
    fn maybe_update_manifest_dependency_appends_skill_dependency() {
        let mut manifest = json!({
            "kind": "agent",
            "name": "support-agent",
            "version": "0.1.0",
            "skills": []
        });

        let changed = maybe_update_manifest_dependency(
            &mut manifest,
            "skills",
            "Skill",
            "@zack/triage-skill@^0.2",
            false,
        )
        .unwrap();

        assert!(changed);
        assert_eq!(
            manifest["skills"],
            json!([{"name":"@zack/triage-skill","version":"^0.2"}])
        );
    }

    #[test]
    fn maybe_update_agent_json_is_noop_for_same_range() {
        let mut manifest = json!({
            "kind": "agent",
            "name": "support-agent",
            "version": "0.1.0",
            "tools": [{"name":"@zack/summarize","version":"^0.1"}]
        });

        let changed =
            maybe_update_agent_json(&mut manifest, "@zack/summarize@^0.1", false).unwrap();

        assert!(!changed);
        assert_eq!(
            manifest["tools"],
            json!([{"name":"@zack/summarize","version":"^0.1"}])
        );
    }

    #[test]
    fn maybe_update_manifest_dependency_rejects_range_change_without_update_range() {
        let mut manifest = json!({
            "kind": "agent",
            "name": "support-agent",
            "version": "0.1.0",
            "skills": [{"name":"@zack/triage-skill","version":"^0.1"}]
        });

        let err = maybe_update_manifest_dependency(
            &mut manifest,
            "skills",
            "Skill",
            "@zack/triage-skill@^0.2",
            false,
        )
        .unwrap_err();

        assert!(
            err.to_string()
                .contains("Skill @zack/triage-skill already exists"),
            "{err:#}"
        );
    }

    #[test]
    fn maybe_update_manifest_dependency_updates_range_in_place() {
        let mut manifest = json!({
            "kind": "agent",
            "name": "support-agent",
            "version": "0.1.0",
            "skills": [{"name":"@zack/triage-skill","version":"^0.1"}]
        });

        let changed = maybe_update_manifest_dependency(
            &mut manifest,
            "skills",
            "Skill",
            "@zack/triage-skill@^0.2",
            true,
        )
        .unwrap();

        assert!(changed);
        assert_eq!(
            manifest["skills"],
            json!([{"name":"@zack/triage-skill","version":"^0.2"}])
        );
    }

    #[test]
    fn maybe_update_manifest_singular_dependency_appends_loop_dependency() {
        let mut manifest = json!({
            "kind": "agent",
            "name": "support-agent",
            "version": "0.1.0"
        });

        let changed = maybe_update_manifest_singular_dependency(
            &mut manifest,
            "loop",
            "Loop",
            "@zack/incident-response-loop@^0.2",
            false,
        )
        .unwrap();

        assert!(changed);
        assert_eq!(
            manifest["loop"],
            json!({"name":"@zack/incident-response-loop","version":"^0.2"})
        );
    }

    #[test]
    fn maybe_update_manifest_singular_dependency_replaces_different_package() {
        let mut manifest = json!({
            "kind": "agent",
            "name": "support-agent",
            "version": "0.1.0",
            "loop": {"name":"@zack/old-loop","version":"^0.1"}
        });

        let changed = maybe_update_manifest_singular_dependency(
            &mut manifest,
            "loop",
            "Loop",
            "@zack/new-loop@^0.2",
            false,
        )
        .unwrap();

        assert!(changed);
        assert_eq!(
            manifest["loop"],
            json!({"name":"@zack/new-loop","version":"^0.2"})
        );
    }
}
