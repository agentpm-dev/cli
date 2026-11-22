//! Derived standard identity, separate from validation evidence or Runner readiness.
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Standard {
    pub id: String,
    pub version: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StandardOrigin {
    Declared,
    LegacyInferred,
}

pub fn validate(standard: Option<&Standard>, origin: Option<StandardOrigin>) -> Result<()> {
    ensure!(
        standard.is_some() == origin.is_some(),
        "APDS standard and standard_origin must be recorded together; run agentpm install again to regenerate the lock"
    );
    if let Some(standard) = standard {
        ensure!(
            standard.id == "agentpm" && standard.version == "1.0.0",
            "Unsupported APDS standard {}/{}; upgrade AgentPM to a release supporting this standard",
            standard.id,
            standard.version
        );
    }
    Ok(())
}

/// Missing selectors are historical interpretation, never authored declarations.
pub fn from_manifest(manifest: &Value) -> Result<(Standard, StandardOrigin)> {
    ensure!(
        manifest.is_object(),
        "Package manifest must be a JSON object"
    );
    let (standard, origin) = match manifest.get("standard") {
        None => (
            Standard {
                id: "agentpm".into(),
                version: "1.0.0".into(),
            },
            StandardOrigin::LegacyInferred,
        ),
        Some(value) => {
            let standard = serde_json::from_value::<Standard>(value.clone()).map_err(|error| {
                anyhow::anyhow!("Malformed APDS standard declaration: {error}; rebuild the package with a supported standard")
            })?;
            (standard, StandardOrigin::Declared)
        }
    };
    validate(Some(&standard), Some(origin))?;
    Ok((standard, origin))
}

pub fn reconcile(
    manifest: &Value,
    standard: Option<&Standard>,
    origin: Option<StandardOrigin>,
) -> Result<(Standard, StandardOrigin)> {
    validate(standard, origin)?;
    let observed = from_manifest(manifest)?;
    if let Some(expected) = standard {
        ensure!(
            expected == &observed.0 && origin == Some(observed.1),
            "APDS standard/origin differs from the embedded package manifest; retry resolution or contact the publisher. Legacy inference is not a declaration"
        );
    }
    Ok(observed)
}
