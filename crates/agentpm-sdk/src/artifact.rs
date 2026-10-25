//! Package-time payload metadata. Absence on a legacy artifact means unknown
//! compatibility; callers must not default a missing descriptor to `any`.

use crate::target::Target;
use serde::{Deserialize, Serialize};

pub mod policy;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ArtifactFormat {
    #[serde(rename = "tar.gz")]
    TarGz,
}

impl ArtifactFormat {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::TarGz => "tar.gz",
        }
    }
}

/// Compatibility of the shipped files, independent of declared dependencies
/// and of the publisher's `client.os` / `client.arch` telemetry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolPayloadDescriptor {
    pub target: Target,
    pub format: ArtifactFormat,
    #[serde(default)]
    pub diagnostics: Vec<PayloadDiagnostic>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PayloadSeverity {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PayloadDiagnosticCode {
    NativeContent,
    InstalledState,
    UnknownFormat,
    ExecutableMember,
    UnsupportedMemberType,
    FormatMismatch,
    CollectionOmitted,
    CollectionExplicitOverride,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PayloadDiagnostic {
    pub code: PayloadDiagnosticCode,
    pub severity: PayloadSeverity,
    pub path: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub help: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<Target>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected_format: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rule: Option<String>,
}

impl std::fmt::Display for PayloadDiagnostic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.path, self.message)?;
        if let Some(target) = self.target {
            write!(f, " Tool payload classified as {target}.")?;
        }
        if let Some(help) = &self.help {
            write!(f, "\n{help}")?;
        }
        Ok(())
    }
}

/// A failed build has diagnostics, not a successful artifact descriptor. Keep
/// this typed error available through anyhow context/downcasting for future CI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PayloadBuildFailure {
    pub diagnostics: Vec<PayloadDiagnostic>,
}

impl std::fmt::Display for PayloadBuildFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for (index, diagnostic) in self.diagnostics.iter().enumerate() {
            if index > 0 {
                writeln!(f)?;
            }
            let level = match diagnostic.severity {
                PayloadSeverity::Info => "Info",
                PayloadSeverity::Warning => "Warning",
                PayloadSeverity::Error => "Error",
            };
            write!(f, "{level}: {diagnostic}")?;
        }
        Ok(())
    }
}

impl std::error::Error for PayloadBuildFailure {}

pub fn sort_diagnostics(diagnostics: &mut Vec<PayloadDiagnostic>) {
    diagnostics.sort_by(|a, b| {
        a.path
            .cmp(&b.path)
            .then_with(|| a.code.cmp(&b.code))
            .then_with(|| a.rule.cmp(&b.rule))
    });
    diagnostics.dedup_by(|a, b| a.path == b.path && a.code == b.code && a.rule == b.rule);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn descriptor_uses_canonical_targets_and_explicit_format() {
        for target in Target::SUPPORTED.iter().chain([Target::Any].iter()) {
            let descriptor = ToolPayloadDescriptor {
                target: *target,
                format: ArtifactFormat::TarGz,
                diagnostics: Vec::new(),
            };
            let value = serde_json::to_value(&descriptor).unwrap();
            assert_eq!(value["target"], target.as_str());
            assert_eq!(value["format"], "tar.gz");
            assert_eq!(
                serde_json::from_value::<ToolPayloadDescriptor>(value).unwrap(),
                descriptor
            );
        }
        for value in [
            serde_json::json!({"format":"tar.gz"}),
            serde_json::json!({"target":"any"}),
            serde_json::json!({"target":"unknown","format":"tar.gz"}),
            serde_json::json!({"target":"any","format":"zip"}),
        ] {
            assert!(serde_json::from_value::<ToolPayloadDescriptor>(value).is_err());
        }
    }
}
