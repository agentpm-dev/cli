#![allow(dead_code)]

use crate::manifest::LoopCheckpoint;
use crate::{
    harness_config::{HarnessApprovalController, HarnessImplementation},
    harness_runtime::service::{
        HostServiceInvoker, ProcessServiceClient, ProcessServiceConfig, ServiceLifecycleEmitter,
    },
};
use anyhow::Result;
use serde::Deserialize;
use std::collections::{BTreeMap, VecDeque};
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApprovalDecision {
    Approve,
    Deny,
    Pending,
    Failure(String),
}

pub trait ApprovalController {
    fn request_approval(&mut self, checkpoint: &LoopCheckpoint) -> ApprovalDecision;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApprovalCapabilityAdvertisement {
    pub request_approval: bool,
    pub cancellation: bool,
}

impl Default for ApprovalCapabilityAdvertisement {
    fn default() -> Self {
        Self {
            request_approval: true,
            cancellation: false,
        }
    }
}

pub struct ConfiguredApprovalController {
    runtime: ApprovalRuntime,
    capabilities: ApprovalCapabilityAdvertisement,
}

enum ApprovalRuntime {
    Process(Box<ProcessServiceClient>),
    Host {
        invoker: Box<dyn HostServiceInvoker>,
        request_timeout_ms: u64,
    },
}

impl ConfiguredApprovalController {
    pub fn process(
        workspace_root: &Path,
        controller: &HarnessApprovalController,
        timeout_ms: Option<u64>,
        lifecycle_events: Option<ServiceLifecycleEmitter>,
    ) -> Result<Option<Self>> {
        let HarnessImplementation::Process { .. } = &controller.implementation else {
            return Ok(None);
        };
        let mut implementation = controller.implementation.clone();
        if let (
            Some(timeout_ms),
            HarnessImplementation::Process {
                request_timeout_ms, ..
            },
        ) = (timeout_ms, &mut implementation)
        {
            *request_timeout_ms = timeout_ms;
        }
        let client = ProcessServiceClient::start(ProcessServiceConfig {
            service: "approval".into(),
            registry_id: "controller".into(),
            initialize_payload: serde_json::Map::new(),
            implementation,
            workspace_root: workspace_root.to_path_buf(),
            lifecycle_events,
        })?;
        let capabilities = approval_capabilities_from_initialization(
            client.initialization_result(),
            "controller",
        )?;
        Ok(Some(Self {
            runtime: ApprovalRuntime::Process(Box::new(client)),
            capabilities,
        }))
    }

    pub fn host(
        controller: &HarnessApprovalController,
        timeout_ms: Option<u64>,
        invoker: Box<dyn HostServiceInvoker>,
    ) -> Result<Option<Self>> {
        let HarnessImplementation::Host { request_timeout_ms } = &controller.implementation else {
            return Ok(None);
        };
        let capabilities = approval_capabilities_from_initialization(
            &invoker
                .host_service_capabilities("approval", "controller")
                .unwrap_or_else(|| serde_json::json!({})),
            "controller",
        )?;
        Ok(Some(Self {
            runtime: ApprovalRuntime::Host {
                invoker,
                request_timeout_ms: timeout_ms.unwrap_or(*request_timeout_ms),
            },
            capabilities,
        }))
    }

    pub fn capabilities(&self) -> ApprovalCapabilityAdvertisement {
        self.capabilities.clone()
    }
}

impl ApprovalController for ConfiguredApprovalController {
    fn request_approval(&mut self, checkpoint: &LoopCheckpoint) -> ApprovalDecision {
        let result = match &mut self.runtime {
            ApprovalRuntime::Process(client) => client.request(
                "request_approval",
                serde_json::json!({ "checkpoint": checkpoint }),
            ),
            ApprovalRuntime::Host {
                invoker,
                request_timeout_ms,
            } => invoker.invoke_host_service(
                "approval",
                "controller",
                "request_approval",
                serde_json::json!({ "checkpoint": checkpoint }),
                *request_timeout_ms,
            ),
        };
        match result {
            Ok(value) => decode_approval_decision(&value)
                .unwrap_or_else(|err| ApprovalDecision::Failure(err.to_string())),
            Err(err) => ApprovalDecision::Failure(err.to_string()),
        }
    }
}

fn decode_approval_decision(value: &serde_json::Value) -> Result<ApprovalDecision> {
    match value
        .get("decision")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("pending")
    {
        "approve" | "approved" => Ok(ApprovalDecision::Approve),
        "deny" | "denied" => Ok(ApprovalDecision::Deny),
        "pending" => Ok(ApprovalDecision::Pending),
        other => anyhow::bail!("unsupported approval decision `{other}`"),
    }
}

#[derive(Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct PartialApprovalCapabilityAdvertisement {
    request_approval: Option<bool>,
    approval: Option<bool>,
    cancellation: Option<bool>,
}

pub(crate) fn approval_capabilities_from_initialization(
    initialization_result: &serde_json::Value,
    expected_registry_id: &str,
) -> Result<ApprovalCapabilityAdvertisement> {
    if initialization_result
        .get("ready")
        .and_then(serde_json::Value::as_bool)
        .is_some_and(|ready| !ready)
    {
        anyhow::bail!(
            "approval service `{expected_registry_id}` initialized but reported not ready"
        );
    }
    if let Some(registry_id) = initialization_result
        .get("registry_id")
        .and_then(serde_json::Value::as_str)
        && registry_id != expected_registry_id
    {
        anyhow::bail!(
            "approval service initialized as `{registry_id}`, expected `{expected_registry_id}`"
        );
    }

    let capabilities_value = match initialization_result.get("capabilities") {
        Some(capabilities) => capabilities.clone(),
        None => {
            let mut capabilities = serde_json::Map::new();
            for key in ["request_approval", "approval", "cancellation"] {
                if let Some(value) = initialization_result.get(key) {
                    capabilities.insert(key.into(), value.clone());
                }
            }
            if capabilities.is_empty() {
                return Ok(ApprovalCapabilityAdvertisement::default());
            }
            serde_json::Value::Object(capabilities)
        }
    };
    if capabilities_value.is_null() {
        return Ok(ApprovalCapabilityAdvertisement::default());
    }
    let partial: PartialApprovalCapabilityAdvertisement =
        serde_json::from_value(capabilities_value)?;
    let request_approval = partial
        .request_approval
        .or(partial.approval)
        .unwrap_or(true);
    if !request_approval {
        anyhow::bail!(
            "approval service `{expected_registry_id}` does not advertise request_approval support"
        );
    }

    Ok(ApprovalCapabilityAdvertisement {
        request_approval,
        cancellation: partial.cancellation.unwrap_or(false),
    })
}

#[derive(Default)]
pub struct ScriptedApprovalController {
    decisions: BTreeMap<String, VecDeque<ApprovalDecision>>,
}

impl ScriptedApprovalController {
    pub fn push(&mut self, checkpoint_id: impl Into<String>, decision: ApprovalDecision) {
        self.decisions
            .entry(checkpoint_id.into())
            .or_default()
            .push_back(decision);
    }
}

impl ApprovalController for ScriptedApprovalController {
    fn request_approval(&mut self, checkpoint: &LoopCheckpoint) -> ApprovalDecision {
        self.decisions
            .get_mut(&checkpoint.id)
            .and_then(VecDeque::pop_front)
            .unwrap_or(ApprovalDecision::Approve)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn approval_initialization_parses_readiness_and_capabilities() {
        let capabilities = approval_capabilities_from_initialization(
            &json!({
                "registry_id": "controller",
                "ready": true,
                "capabilities": {
                    "request_approval": true,
                    "cancellation": true
                }
            }),
            "controller",
        )
        .unwrap();

        assert!(capabilities.request_approval);
        assert!(capabilities.cancellation);
    }

    #[test]
    fn approval_initialization_accepts_approval_alias() {
        let capabilities = approval_capabilities_from_initialization(
            &json!({
                "registry_id": "controller",
                "ready": true,
                "capabilities": {
                    "approval": true
                }
            }),
            "controller",
        )
        .unwrap();

        assert!(capabilities.request_approval);
        assert!(!capabilities.cancellation);
    }

    #[test]
    fn approval_initialization_rejects_not_ready() {
        let err = approval_capabilities_from_initialization(
            &json!({
                "registry_id": "controller",
                "ready": false,
                "capabilities": {
                    "request_approval": true
                }
            }),
            "controller",
        )
        .unwrap_err();

        assert!(err.to_string().contains("reported not ready"));
    }

    #[test]
    fn approval_initialization_rejects_identity_mismatch() {
        let err = approval_capabilities_from_initialization(
            &json!({
                "registry_id": "other-controller",
                "ready": true,
                "capabilities": {
                    "request_approval": true
                }
            }),
            "controller",
        )
        .unwrap_err();

        assert!(err.to_string().contains("expected `controller`"));
    }

    #[test]
    fn approval_initialization_rejects_missing_request_approval_capability() {
        let err = approval_capabilities_from_initialization(
            &json!({
                "registry_id": "controller",
                "ready": true,
                "capabilities": {
                    "request_approval": false
                }
            }),
            "controller",
        )
        .unwrap_err();

        assert!(err.to_string().contains("request_approval support"));
    }

    #[test]
    fn approval_initialization_defaults_when_capabilities_are_omitted() {
        let capabilities = approval_capabilities_from_initialization(
            &json!({"registry_id": "controller"}),
            "controller",
        )
        .unwrap();

        assert!(capabilities.request_approval);
        assert!(!capabilities.cancellation);
    }
}
