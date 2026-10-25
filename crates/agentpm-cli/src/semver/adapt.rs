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
                python_resolution: it.python_resolution.clone(),
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
                    python_resolution: it.python_resolution,
                })
                .collect(),
        }
    }
}

fn to_sdk_kind(kind: PackageKind) -> sdkm::PackageKind {
    kind
}

fn from_sdk_kind(kind: sdkm::PackageKind) -> PackageKind {
    kind
}
