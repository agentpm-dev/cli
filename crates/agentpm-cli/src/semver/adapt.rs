// cli/src/semver/adapt.rs
use crate::semver::types::{DesiredSet, ResolvePlan, ResolvedItem};
use agentpm_sdk::models::install as sdkm;

pub fn to_sdk_request(desired: &DesiredSet) -> sdkm::ResolveRequest {
    sdkm::ResolveRequest {
        items: desired
            .items
            .iter()
            .map(|d| sdkm::ResolveReqItem {
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
            .map(|it| sdkm::ResolveRespItem {
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
                .map(|it| ResolvedItem {
                    name: it.name,
                    version: it.version,
                    integrity: it.integrity,
                })
                .collect(),
        }
    }
}
