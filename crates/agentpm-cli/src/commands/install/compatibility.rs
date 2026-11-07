use super::*;
use agentpm_sdk::{SdkError, models::install::ResolveRequest};

fn replacement(error: &SdkError, desired: &DesiredSet) -> Option<ResolveRequest> {
    let SdkError::Api { body, .. } = error else {
        return None;
    };
    if body.code.as_deref() != Some("no_compatible_artifact") {
        return None;
    }
    let details = body.details.as_ref()?;
    let name = details["name"].as_str()?;
    let item = desired.items.iter().find(|item| item.name == name)?;
    // Registry treats bare versions as exact, whereas Rust VersionReq defaults
    // them to caret semantics. Never turn an exact request into a recovery offer.
    if semver::Version::parse(&item.range).is_ok() {
        return None;
    }
    let range = if item.range == "latest" {
        "*"
    } else {
        &item.range
    };
    let requirement = semver::VersionReq::parse(range).ok()?;
    if requirement
        .comparators
        .iter()
        .any(|c| c.op == semver::Op::Exact)
    {
        return None;
    }
    let current = semver::Version::parse(details["version"].as_str()?).ok()?;
    let version = details["compatible_newer_versions"]
        .as_array()?
        .iter()
        .filter_map(|v| v.as_str().and_then(|s| semver::Version::parse(s).ok()))
        .filter(|v| *v > current && requirement.matches(v))
        .max()?;
    let mut request = to_sdk_request(desired);
    request
        .items
        .iter_mut()
        .find(|item| item.name == name)?
        .range = version.to_string();
    Some(request)
}

pub(super) fn compatibility_recovery(
    error: &SdkError,
    desired: &DesiredSet,
    locked: bool,
    machine: bool,
) -> Result<Option<ResolveRequest>> {
    if locked || machine || !std::io::stdin().is_terminal() || !std::io::stderr().is_terminal() {
        return Ok(None);
    }
    let Some(request) = replacement(error, desired) else {
        return Ok(None);
    };
    let item = request
        .items
        .iter()
        .find(|item| {
            desired
                .items
                .iter()
                .any(|d| d.name == item.name && d.range != item.range)
        })
        .context("missing compatible replacement")?;
    eprint!(
        "{error}\nInstall compatible {}@{} instead? [y/N] ",
        item.name, item.range
    );
    use std::io::Write;
    std::io::stderr().flush()?;
    let mut response = String::new();
    std::io::stdin().read_line(&mut response)?;
    Ok(
        if matches!(response.trim().to_ascii_lowercase().as_str(), "y" | "yes") {
            Some(request)
        } else {
            None
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn recovery_only_offers_newer_versions_inside_a_nonexact_range() {
        let error = SdkError::Api {
            status: 422,
            body: agentpm_sdk::error::ApiErrorBody {
                code: Some("no_compatible_artifact".into()),
                message: None,
                details: Some(
                    serde_json::json!({"name":"@zack/tool","version":"1.0.0","compatible_newer_versions":["2.0.0","1.2.0"]}),
                ),
            },
        };
        for (range, expected) in [
            ("^1.0", Some("1.2.0")),
            ("1.0.0", None),
            ("=1.0.0", None),
            ("~1.0", None),
        ] {
            let desired = DesiredSet {
                items: vec![crate::semver::types::PackageRequirement::tool(
                    "@zack/tool",
                    range,
                )],
            };
            assert_eq!(
                replacement(&error, &desired)
                    .map(|r| r.items[0].range.clone())
                    .as_deref(),
                expected
            );
            assert!(
                compatibility_recovery(&error, &desired, true, false)
                    .unwrap()
                    .is_none()
            );
            assert!(
                compatibility_recovery(&error, &desired, false, true)
                    .unwrap()
                    .is_none()
            );
        }
    }
}
