use super::runtime_snapshot::{knowledge_snapshots_from_plan, memory_snapshots_from_plan};
use super::*;

pub(super) fn register_host_service(
    plan: &ResolvedHarnessPlan,
    bridge: &MachineHostBridgeHandle,
    payload: &Value,
) -> std::result::Result<HostServiceRegistration, String> {
    let role = payload
        .get("role")
        .and_then(Value::as_str)
        .ok_or_else(|| "host service registration requires payload.role".to_string())?
        .to_string();
    let registry_id = payload
        .get("registry_id")
        .or_else(|| payload.get("id"))
        .and_then(Value::as_str)
        .ok_or_else(|| {
            "host service registration requires payload.registry_id or payload.id".to_string()
        })?
        .to_string();
    let service = HostServiceRegistration { role, registry_id };
    let capabilities = payload.get("capabilities").cloned().unwrap_or_else(|| {
        if service.role == "memory" {
            payload.clone()
        } else {
            json!({})
        }
    });
    validate_host_service_readiness(&service, payload)?;
    if !configured_host_services(plan).contains(&service) {
        if service.role == "hook" {
            let registrations = sdk_host_hook_registrations(&service.registry_id, payload)?;
            if registrations.is_empty() {
                return Err(
                    "SDK hook registration requires payload.hooks with at least one Hook ID".into(),
                );
            }
            bridge.register_host_service(&service, capabilities);
            bridge.register_sdk_host_hooks(registrations);
            return Ok(service);
        }
        if service.role == "approval" && service.registry_id == "controller" {
            crate::harness_runtime::approval::approval_capabilities_from_initialization(
                &capabilities,
                "controller",
            )
            .map_err(|err| err.to_string())?;
            bridge.register_host_service(&service, capabilities);
            bridge.register_sdk_approval_controller();
            return Ok(service);
        }
        return Err(format!(
            "host service `{}` with registry ID `{}` is not configured",
            service.role, service.registry_id
        ));
    }
    validate_configured_host_service_registration(plan, &service, payload, &capabilities)?;
    bridge.register_host_service(&service, capabilities);
    Ok(service)
}

pub(super) fn host_service_registration_response(service: &HostServiceRegistration) -> Value {
    let (active, reason) = host_service_activation_status(service);
    json!({
        "registered": true,
        "service": service,
        "active": active,
        "reason": reason,
    })
}

fn host_service_activation_status(
    service: &HostServiceRegistration,
) -> (bool, Option<&'static str>) {
    match service.role.as_str() {
        "embedding" | "knowledge" | "memory" => (true, None),
        _ => (true, None),
    }
}

fn validate_host_service_readiness(
    service: &HostServiceRegistration,
    payload: &Value,
) -> std::result::Result<(), String> {
    if payload
        .get("ready")
        .and_then(Value::as_bool)
        .is_some_and(|ready| !ready)
    {
        return Err(format!(
            "host service `{}` with registry ID `{}` registered but reported not ready",
            service.role, service.registry_id
        ));
    }
    Ok(())
}

fn validate_configured_host_service_registration(
    plan: &ResolvedHarnessPlan,
    service: &HostServiceRegistration,
    payload: &Value,
    capabilities: &Value,
) -> std::result::Result<(), String> {
    match service.role.as_str() {
        "hook" => validate_configured_host_hook_registration(plan, service, payload, capabilities),
        "embedding" => crate::harness_runtime::knowledge::validate_embedding_provider_capabilities(
            capabilities,
            &service.registry_id,
        )
        .map_err(|err| err.to_string()),
        "knowledge" => {
            let routes = custom_knowledge_routes(plan);
            let mapped_packages = knowledge_snapshots_from_plan(plan)
                .into_iter()
                .filter(|package| routes.get(&package.name) == Some(&service.registry_id))
                .collect::<Vec<_>>();
            crate::harness_runtime::knowledge::validate_knowledge_runtime_capabilities(
                capabilities,
                &service.registry_id,
                &mapped_packages,
            )
            .map_err(|err| err.to_string())
        }
        "memory" => {
            let routes = custom_memory_routes(plan);
            let mapped_spaces = memory_snapshots_from_plan(plan)
                .into_iter()
                .filter(|space| routes.get(&space.package) == Some(&service.registry_id))
                .collect::<Vec<_>>();
            crate::harness_runtime::memory::validate_memory_runtime_capabilities(
                payload,
                &service.registry_id,
                &mapped_spaces,
            )
            .map(|_| ())
            .map_err(|err| err.to_string())
        }
        "approval" if service.registry_id == "controller" => {
            crate::harness_runtime::approval::approval_capabilities_from_initialization(
                capabilities,
                "controller",
            )
            .map_err(|err| err.to_string())?;
            Ok(())
        }
        _ => Ok(()),
    }
}

fn validate_configured_host_hook_registration(
    plan: &ResolvedHarnessPlan,
    service: &HostServiceRegistration,
    payload: &Value,
    capabilities: &Value,
) -> std::result::Result<(), String> {
    let expected_hooks = configured_host_hook_ids(plan, &service.registry_id);
    if expected_hooks.is_empty() {
        return Ok(());
    }
    let Some(advertised_hooks) = payload
        .get("hooks")
        .or_else(|| capabilities.get("hooks"))
        .cloned()
    else {
        return Err(format!(
            "host hook service `{}` must advertise payload.hooks or capabilities.hooks",
            service.registry_id
        ));
    };
    crate::harness_runtime::hook::validate_hook_service_initialization(
        &json!({
            "registry_id": service.registry_id.clone(),
            "hooks": advertised_hooks,
        }),
        &service.registry_id,
        &expected_hooks,
    )
    .map_err(|err| err.to_string())
}

fn configured_host_hook_ids(
    plan: &ResolvedHarnessPlan,
    implementation: &str,
) -> Vec<HarnessHookId> {
    let mut hook_ids = Vec::new();
    for binding in plan
        .config
        .config
        .hooks
        .bindings
        .iter()
        .filter(|binding| binding.implementation == implementation)
    {
        if !hook_ids.contains(&binding.hook) {
            hook_ids.push(binding.hook.clone());
        }
    }
    hook_ids
}

fn sdk_host_hook_registrations(
    registry_id: &str,
    payload: &Value,
) -> std::result::Result<Vec<SdkHostHookRegistration>, String> {
    let hooks = payload
        .get("hooks")
        .and_then(Value::as_array)
        .ok_or_else(|| "SDK hook registration requires payload.hooks".to_string())?;
    let request_timeout_ms = payload
        .get("request_timeout_ms")
        .and_then(Value::as_u64)
        .unwrap_or(SDK_HOST_REQUEST_TIMEOUT_MS);
    let mut registrations = Vec::new();
    for hook in hooks {
        let Some(hook) = hook.as_str() else {
            return Err("SDK hook registration payload.hooks entries must be strings".into());
        };
        let hook: HarnessHookId =
            serde_json::from_value(Value::String(hook.to_string())).map_err(|err| {
                format!("SDK hook registration contains unsupported Hook ID `{hook}`: {err}")
            })?;
        registrations.push(SdkHostHookRegistration {
            registry_id: registry_id.to_string(),
            hook,
            request_timeout_ms,
        });
    }
    Ok(registrations)
}

pub(super) fn missing_required_host_services(
    plan: &ResolvedHarnessPlan,
    bridge: &MachineHostBridgeHandle,
) -> Vec<HostServiceRegistration> {
    required_host_services(plan)
        .into_iter()
        .filter(|service| !bridge.has_host_service(service))
        .collect()
}

pub(super) fn required_host_services(plan: &ResolvedHarnessPlan) -> Vec<HostServiceRegistration> {
    let mut services = Vec::new();
    if let Some(model) = &plan.config.config.model
        && matches!(
            plan.config
                .config
                .providers
                .models
                .get(&model.provider)
                .map(|entry| &entry.implementation),
            Some(crate::harness_config::HarnessImplementation::Host { .. })
        )
    {
        services.push(HostServiceRegistration {
            role: "model".into(),
            registry_id: model.provider.clone(),
        });
    }
    services.extend(host_hook_services(plan));
    if matches!(
        plan.config
            .config
            .approvals
            .controller
            .as_ref()
            .map(|controller| &controller.implementation),
        Some(crate::harness_config::HarnessImplementation::Host { .. })
    ) {
        services.push(HostServiceRegistration {
            role: "approval".into(),
            registry_id: "controller".into(),
        });
    }
    services.extend(required_host_embedding_services(plan));
    services.extend(required_host_knowledge_services(plan));
    services.extend(required_host_memory_services(plan));
    dedupe_host_services(services)
}

fn required_host_embedding_services(plan: &ResolvedHarnessPlan) -> Vec<HostServiceRegistration> {
    plan.config
        .config
        .knowledge
        .embedding_matches
        .iter()
        .filter_map(|item| {
            let entry = plan
                .config
                .config
                .providers
                .embeddings
                .get(&item.embedding_provider)?;
            matches!(
                entry.implementation,
                crate::harness_config::HarnessImplementation::Host { .. }
            )
            .then(|| HostServiceRegistration {
                role: "embedding".into(),
                registry_id: item.embedding_provider.clone(),
            })
        })
        .collect()
}

fn required_host_knowledge_services(plan: &ResolvedHarnessPlan) -> Vec<HostServiceRegistration> {
    plan.config
        .config
        .knowledge
        .packages
        .values()
        .filter_map(|mapping| {
            let entry = plan
                .config
                .config
                .knowledge
                .runtimes
                .get(&mapping.runtime)?;
            matches!(
                entry.implementation,
                crate::harness_config::HarnessImplementation::Host { .. }
            )
            .then(|| HostServiceRegistration {
                role: "knowledge".into(),
                registry_id: mapping.runtime.clone(),
            })
        })
        .collect()
}

fn required_host_memory_services(plan: &ResolvedHarnessPlan) -> Vec<HostServiceRegistration> {
    plan.config
        .config
        .memory
        .packages
        .values()
        .filter_map(|mapping| {
            let entry = plan.config.config.memory.runtimes.get(&mapping.runtime)?;
            matches!(
                entry.implementation,
                crate::harness_config::HarnessImplementation::Host { .. }
            )
            .then(|| HostServiceRegistration {
                role: "memory".into(),
                registry_id: mapping.runtime.clone(),
            })
        })
        .collect()
}

fn configured_host_services(plan: &ResolvedHarnessPlan) -> Vec<HostServiceRegistration> {
    let mut services = Vec::new();
    services.extend(
        plan.config
            .config
            .providers
            .models
            .iter()
            .filter(|(_, entry)| {
                matches!(
                    entry.implementation,
                    crate::harness_config::HarnessImplementation::Host { .. }
                )
            })
            .map(|(id, _)| HostServiceRegistration {
                role: "model".into(),
                registry_id: id.clone(),
            }),
    );
    services.extend(host_hook_services(plan));
    services.extend(
        plan.config
            .config
            .providers
            .embeddings
            .iter()
            .filter(|(_, entry)| {
                matches!(
                    entry.implementation,
                    crate::harness_config::HarnessImplementation::Host { .. }
                )
            })
            .map(|(id, _)| HostServiceRegistration {
                role: "embedding".into(),
                registry_id: id.clone(),
            }),
    );
    services.extend(
        plan.config
            .config
            .knowledge
            .runtimes
            .iter()
            .filter(|(_, entry)| {
                matches!(
                    entry.implementation,
                    crate::harness_config::HarnessImplementation::Host { .. }
                )
            })
            .map(|(id, _)| HostServiceRegistration {
                role: "knowledge".into(),
                registry_id: id.clone(),
            }),
    );
    services.extend(
        plan.config
            .config
            .memory
            .runtimes
            .iter()
            .filter(|(_, entry)| {
                matches!(
                    entry.implementation,
                    crate::harness_config::HarnessImplementation::Host { .. }
                )
            })
            .map(|(id, _)| HostServiceRegistration {
                role: "memory".into(),
                registry_id: id.clone(),
            }),
    );
    if matches!(
        plan.config
            .config
            .approvals
            .controller
            .as_ref()
            .map(|controller| &controller.implementation),
        Some(crate::harness_config::HarnessImplementation::Host { .. })
    ) {
        services.push(HostServiceRegistration {
            role: "approval".into(),
            registry_id: "controller".into(),
        });
    }
    dedupe_host_services(services)
}

fn host_hook_services(plan: &ResolvedHarnessPlan) -> Vec<HostServiceRegistration> {
    plan.config
        .config
        .hooks
        .bindings
        .iter()
        .filter_map(|binding| {
            let entry = plan
                .config
                .config
                .hooks
                .implementations
                .get(&binding.implementation)?;
            matches!(
                entry.implementation,
                crate::harness_config::HarnessImplementation::Host { .. }
            )
            .then(|| HostServiceRegistration {
                role: "hook".into(),
                registry_id: binding.implementation.clone(),
            })
        })
        .collect()
}

fn dedupe_host_services(services: Vec<HostServiceRegistration>) -> Vec<HostServiceRegistration> {
    let mut seen = BTreeSet::new();
    services
        .into_iter()
        .filter(|service| seen.insert((service.role.clone(), service.registry_id.clone())))
        .collect()
}
