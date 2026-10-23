//! Portable Python dependency state. Disk/resolve DTO integration belongs to M11B.
use anyhow::{Context, Result, ensure};
use pep440_rs::{Version, VersionSpecifiers};
use pep508_rs::{MarkerTree, PackageName, Requirement, VersionOrUrl};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::str::FromStr;

pub const RESOLUTION_TYPE: &str = "agentpm.python-resolution.v1";

pub fn normalize_requirements(values: &[String]) -> Result<Vec<String>> {
    let mut parsed: Vec<Requirement> = Vec::new();
    for (index, value) in values.iter().enumerate() {
        let mut requirement = Requirement::from_str(value)
            .with_context(|| format!("invalid Python requirement at dependencies/{index}"))?;
        ensure!(
            !matches!(requirement.version_or_url, Some(VersionOrUrl::Url(_))),
            "Python dependencies must use index distribution names, not URLs or local paths (dependencies/{index})"
        );
        if let Some(VersionOrUrl::VersionSpecifier(specifiers)) = &requirement.version_or_url {
            let ranges: version_ranges::Ranges<Version> = specifiers.clone().into();
            ensure!(
                !ranges.is_empty(),
                "conflicting Python version constraints at dependencies/{index}"
            );
        }
        for previous in &parsed {
            ensure!(
                previous.name != requirement.name
                    || previous.marker.is_disjoint(&requirement.marker),
                "duplicate or conflicting Python dependency '{}' at dependencies/{index}; combine overlapping requirements into one entry",
                requirement.name
            );
        }
        requirement.extras.sort();
        requirement.extras.dedup();
        parsed.push(requirement);
    }
    let mut normalized: Vec<_> = parsed.iter().map(ToString::to_string).collect();
    normalized.sort();
    Ok(normalized)
}

pub fn minimum_python(version: &str) -> Result<String> {
    let parts: Vec<_> = version.split('.').collect();
    ensure!(
        !parts.is_empty()
            && parts.len() <= 3
            && parts
                .iter()
                .all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit())),
        "runtime.version must be a numeric Python minimum version"
    );
    let version: Version = version.parse()?;
    Ok(format!(">={version}"))
}

pub fn normalize_marker(value: Option<&str>) -> Result<Option<String>> {
    let Some(value) = value else { return Ok(None) };
    ensure!(!value.trim().is_empty(), "empty Python environment marker");
    let marker: MarkerTree = value.parse().context("invalid Python environment marker")?;
    ensure!(
        !marker.is_false(),
        "unsatisfiable Python environment marker"
    );
    Ok(marker.contents().map(|m| m.to_string()))
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PythonConstraint {
    pub requires: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PythonPackage {
    pub name: String,
    pub version: String,
    pub hashes: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub marker: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct ResolutionWire {
    #[serde(rename = "type")]
    resolution_type: String,
    python: PythonConstraint,
    requirements: Vec<String>,
    packages: Vec<PythonPackage>,
}

/// Validates/normalizes entries and authored root coverage, not transitive completeness.
/// V1 deliberately has no dependency edges: the complete transitive set comes from the
/// pinned resolver. A missing non-root package cannot be inferred from this object alone.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(try_from = "ResolutionWire", into = "ResolutionWire")]
pub struct PythonResolution(ResolutionWire);

impl From<PythonResolution> for ResolutionWire {
    fn from(value: PythonResolution) -> Self {
        value.0
    }
}

impl TryFrom<ResolutionWire> for PythonResolution {
    type Error = anyhow::Error;

    fn try_from(mut value: ResolutionWire) -> Result<Self> {
        ensure!(
            value.resolution_type == RESOLUTION_TYPE,
            "unsupported PythonResolution type"
        );
        let requires: VersionSpecifiers = value.python.requires.parse()?;
        let ranges: version_ranges::Ranges<Version> = requires.clone().into();
        ensure!(!ranges.is_empty(), "unsatisfiable Python constraint");
        value.python.requires = requires.to_string();
        value.requirements = normalize_requirements(&value.requirements)?;
        for package in &mut value.packages {
            package.name = package.name.parse::<PackageName>()?.to_string();
            package.version = package.version.parse::<Version>()?.to_string();
            ensure!(
                !package.hashes.is_empty(),
                "Python distribution {}=={} has no retrievable SHA-256 digests",
                package.name,
                package.version
            );
            for hash in &package.hashes {
                ensure!(
                    is_sha256(hash),
                    "invalid SHA-256 distribution digest for {}",
                    package.name
                );
            }
            package.hashes.sort();
            package.hashes.dedup();
            package.marker = normalize_marker(package.marker.as_deref())?;
        }
        value.packages.sort_by(|a, b| {
            (&a.name, &a.marker, &a.version).cmp(&(&b.name, &b.marker, &b.version))
        });
        for (i, package) in value.packages.iter().enumerate() {
            let marker: MarkerTree = package
                .marker
                .as_deref()
                .unwrap_or("python_version >= '0'")
                .parse()?;
            for previous in &value.packages[..i] {
                if previous.name == package.name {
                    let other: MarkerTree = previous
                        .marker
                        .as_deref()
                        .unwrap_or("python_version >= '0'")
                        .parse()?;
                    ensure!(
                        marker.is_disjoint(&other),
                        "overlapping Python resolution branches for {}",
                        package.name
                    );
                }
            }
        }
        let minimum = value
            .python
            .requires
            .strip_prefix(">=")
            .context("PythonResolution requires a minimum Python constraint")?;
        ensure!(
            minimum_python(minimum)? == value.python.requires,
            "PythonResolution requires a normalized minimum Python constraint"
        );
        let mut supported = MarkerTree::FALSE;
        for target in crate::target::Target::SUPPORTED {
            supported.or(target.marker().parse()?);
        }
        supported.and(format!("python_full_version >= '{minimum}'").parse()?);
        // Root coverage only; without dependency edges this cannot prove the transitive closure.
        for root in &value.requirements {
            let root: Requirement = root.parse()?;
            let mut needed = root.marker;
            needed.and(supported.clone());
            let mut covered = MarkerTree::FALSE;
            for package in &value.packages {
                if root.name.as_ref() != package.name {
                    continue;
                }
                if let Some(VersionOrUrl::VersionSpecifier(spec)) = &root.version_or_url
                    && !spec.contains(&package.version.parse()?)
                {
                    continue;
                }
                covered.or(package
                    .marker
                    .as_deref()
                    .map(str::parse)
                    .transpose()?
                    .unwrap_or(MarkerTree::TRUE));
            }
            ensure!(
                needed.is_disjoint(&covered.negate()),
                "Python resolution omitted applicable root dependency {}",
                root.name
            );
        }
        Ok(Self(value))
    }
}

impl PythonResolution {
    pub fn new(
        requires: String,
        requirements: Vec<String>,
        packages: Vec<PythonPackage>,
    ) -> Result<Self> {
        ResolutionWire {
            resolution_type: RESOLUTION_TYPE.into(),
            python: PythonConstraint { requires },
            requirements,
            packages,
        }
        .try_into()
    }
    pub fn packages(&self) -> &[PythonPackage] {
        &self.0.packages
    }
    pub fn requirements(&self) -> &[String] {
        &self.0.requirements
    }
    pub fn requires(&self) -> &str {
        &self.0.python.requires
    }
    pub fn canonical_bytes(&self) -> Result<Vec<u8>> {
        Ok(serde_jcs::to_vec(self)?)
    }
    pub fn digest(&self) -> Result<String> {
        Ok(format!("{:x}", Sha256::digest(self.canonical_bytes()?)))
    }
}

pub fn is_sha256(value: &str) -> bool {
    value.strip_prefix("sha256:").is_some_and(|h| {
        h.len() == 64
            && h.bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    })
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ResolutionInputs {
    pub requirements: Vec<String>,
    pub python_requires: String,
    pub targets: Vec<String>,
    pub uv_version: String,
}

impl ResolutionInputs {
    pub fn new(requirements: &[String], runtime_version: &str, uv_version: &str) -> Result<Self> {
        let mut targets: Vec<_> = crate::target::Target::SUPPORTED
            .iter()
            .map(ToString::to_string)
            .collect();
        targets.sort();
        Ok(Self {
            requirements: normalize_requirements(requirements)?,
            python_requires: minimum_python(runtime_version)?,
            targets,
            uv_version: uv_version.into(),
        })
    }
    pub fn fingerprint(&self) -> Result<String> {
        Ok(format!("{:x}", Sha256::digest(serde_jcs::to_vec(self)?)))
    }
    pub fn matches(&self, resolution: &PythonResolution) -> bool {
        self.requirements == resolution.requirements()
            && self.python_requires == resolution.requires()
    }
}

#[cfg(test)]
mod tests;
