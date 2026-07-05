// cli/src/semver/adapt.rs
use crate::semver::types::{DesiredSet, PackageKind, ResolvePlan, ResolvedPackage};
use agentpm_sdk::models::install as sdkm;

pub fn to_sdk_request(desired: &DesiredSet) -> sdkm::ResolveRequest {
    sdkm::ResolveRequest {
        items: desired
            .items
            .iter()
            .map(|d| sdkm::PackageRequirement {
                kind: to_sdk_kind(d.kind),
                name: d.name.clone(),
                range: d.range.clone(),
            })
            .collect(),
    }
}

pub fn plan_to_sdk_resolve(plan: &ResolvePlan) -> sdkm::ResolveResponse {
    sdkm::ResolveResponse {
        items: plan
            .items
            .iter()
            .map(|it| sdkm::ResolvedPackage {
                kind: to_sdk_kind(it.kind),
                name: it.name.clone(),
                version: it.version.clone(),
                integrity: it.integrity.clone(),
            })
            .collect(),
    }
}

// Implement From<SDK> for CLI (allowed: target type local to this crate)
impl From<sdkm::ResolveResponse> for ResolvePlan {
    fn from(r: sdkm::ResolveResponse) -> Self {
        ResolvePlan {
            items: r
                .items
                .into_iter()
                .map(|it| ResolvedPackage {
                    kind: from_sdk_kind(it.kind),
                    name: it.name,
                    version: it.version,
                    integrity: it.integrity,
                })
                .collect(),
        }
    }
}

fn to_sdk_kind(kind: PackageKind) -> sdkm::PackageKind {
    match kind {
        PackageKind::Tool => sdkm::PackageKind::Tool,
        PackageKind::Agent => sdkm::PackageKind::Agent,
        PackageKind::Skill => sdkm::PackageKind::Skill,
        PackageKind::Knowledge => sdkm::PackageKind::Knowledge,
    }
}

fn from_sdk_kind(kind: sdkm::PackageKind) -> PackageKind {
    match kind {
        sdkm::PackageKind::Tool => PackageKind::Tool,
        sdkm::PackageKind::Agent => PackageKind::Agent,
        sdkm::PackageKind::Skill => PackageKind::Skill,
        sdkm::PackageKind::Knowledge => PackageKind::Knowledge,
        sdkm::PackageKind::Template => {
            panic!("template packages are not supported in install resolution yet")
        }
    }
}
