use super::*;

pub(super) struct CustomMemoryRuntimeActivation {
    pub(super) runtime: Option<CustomMemoryRuntime>,
    pub(super) unavailable_packages: BTreeMap<String, String>,
    pub(super) unavailable_spaces: BTreeMap<(String, String), String>,
    pub(super) capabilities:
        BTreeMap<String, crate::harness_runtime::memory::MemoryRuntimeCapabilityDescriptor>,
}

pub(super) fn activate_custom_memory_runtime_for_plan(
    plan: &ResolvedHarnessPlan,
    runtime: &RuntimeSnapshot,
    host_bridge: Option<MachineHostBridgeHandle>,
    service_events: Option<&ServiceLifecycleEvents>,
) -> CustomMemoryRuntimeActivation {
    let routes = custom_memory_routes(plan);
    if routes.is_empty() {
        return CustomMemoryRuntimeActivation {
            runtime: None,
            unavailable_packages: BTreeMap::new(),
            unavailable_spaces: BTreeMap::new(),
            capabilities: BTreeMap::new(),
        };
    }
    let mapped_available_spaces = runtime
        .memory
        .iter()
        .filter(|space| routes.contains_key(&space.package) && space.state == "available")
        .cloned()
        .collect::<Vec<_>>();
    if mapped_available_spaces.is_empty() {
        return CustomMemoryRuntimeActivation {
            runtime: None,
            unavailable_packages: BTreeMap::new(),
            unavailable_spaces: BTreeMap::new(),
            capabilities: BTreeMap::new(),
        };
    }
    let mut active_spaces = Vec::new();
    let mut unavailable_packages = BTreeMap::new();
    let mut unavailable_spaces = BTreeMap::new();
    let mut capabilities_by_runtime = BTreeMap::new();
    let mut runtimes = HashMap::new();
    for runtime_id in routes.values().cloned().collect::<BTreeSet<_>>() {
        let mapped_spaces = mapped_available_spaces
            .iter()
            .filter(|space| routes.get(&space.package) == Some(&runtime_id))
            .cloned()
            .collect::<Vec<_>>();
        if mapped_spaces.is_empty() {
            continue;
        }
        let Some(entry) = plan.config.config.memory.runtimes.get(&runtime_id) else {
            mark_custom_memory_runtime_unavailable(
                &mut unavailable_packages,
                &mapped_spaces,
                format!("memory.packages references undefined MemoryRuntime `{runtime_id}`"),
            );
            continue;
        };
        let activation = match &entry.implementation {
            crate::harness_config::HarnessImplementation::Process { .. } => {
                crate::harness_runtime::memory::process_memory_runtime_service(
                    &plan.workspace_root,
                    &runtime_id,
                    entry,
                    &mapped_spaces,
                    service_events.map(ServiceLifecycleEvents::emitter),
                )
                .map_err(|err| {
                    anyhow!("configured MemoryRuntime `{runtime_id}` could not start: {err}")
                })
            }
            crate::harness_config::HarnessImplementation::Host { request_timeout_ms } => {
                let bridge = host_bridge.clone().ok_or_else(|| {
                    let message = format!(
                        "configured MemoryRuntime `{runtime_id}` requires a machine host service"
                    );
                    crate::harness_runtime::memory::emit_memory_host_service_failure(
                        service_events.map(ServiceLifecycleEvents::emitter).as_ref(),
                        &runtime_id,
                        message.clone(),
                    );
                    anyhow!(message)
                });
                bridge.and_then(|bridge| {
                    let capabilities = bridge
                        .host_service_capabilities("memory", &runtime_id)
                        .ok_or_else(|| {
                            let message = format!(
                                "configured MemoryRuntime `{runtime_id}` host service is not registered"
                            );
                            crate::harness_runtime::memory::emit_memory_host_service_failure(
                                service_events.map(ServiceLifecycleEvents::emitter).as_ref(),
                                &runtime_id,
                                message.clone(),
                            );
                            anyhow!(message)
                        })?;
                    Ok((
                        crate::harness_runtime::knowledge::ServiceRuntime::host(
                            Box::new(bridge),
                            *request_timeout_ms,
                        ),
                        capabilities,
                    ))
                })
            }
        };
        let (service_runtime, capabilities) = match activation {
            Ok(activation) => activation,
            Err(err) => {
                mark_custom_memory_runtime_unavailable(
                    &mut unavailable_packages,
                    &mapped_spaces,
                    err.to_string(),
                );
                continue;
            }
        };
        let descriptor = match crate::harness_runtime::memory::validate_memory_runtime_capabilities(
            &capabilities,
            &runtime_id,
            &mapped_spaces,
        ) {
            Ok(descriptor) => descriptor,
            Err(err) => {
                if matches!(
                    entry.implementation,
                    crate::harness_config::HarnessImplementation::Host { .. }
                ) {
                    crate::harness_runtime::memory::emit_memory_host_service_failure(
                        service_events.map(ServiceLifecycleEvents::emitter).as_ref(),
                        &runtime_id,
                        err.to_string(),
                    );
                }
                mark_custom_memory_runtime_unavailable(
                    &mut unavailable_packages,
                    &mapped_spaces,
                    format!("configured MemoryRuntime `{runtime_id}` is not ready: {err}"),
                );
                continue;
            }
        };
        for space in mapped_spaces {
            if let Some(reason) = custom_memory_space_unrealizable_reason(&space, &descriptor) {
                unavailable_spaces.insert(
                    (space.package.clone(), space.space.clone()),
                    format!("configured MemoryRuntime `{runtime_id}` cannot realize: {reason}"),
                );
            } else {
                active_spaces.push(space);
            }
        }
        capabilities_by_runtime.insert(runtime_id.clone(), descriptor);
        runtimes.insert(runtime_id, service_runtime);
    }
    let runtime = (!active_spaces.is_empty()).then(|| {
        CustomMemoryRuntime::with_lifecycle(
            active_spaces,
            runtimes,
            service_events.map(ServiceLifecycleEvents::emitter),
        )
    });
    CustomMemoryRuntimeActivation {
        runtime,
        unavailable_packages,
        unavailable_spaces,
        capabilities: capabilities_by_runtime,
    }
}

fn mark_custom_memory_runtime_unavailable(
    unavailable_packages: &mut BTreeMap<String, String>,
    spaces: &[MemorySpaceRuntimeSnapshot],
    reason: String,
) {
    for space in spaces {
        unavailable_packages.insert(space.package.clone(), reason.clone());
    }
}

fn custom_memory_space_unrealizable_reason(
    space: &MemorySpaceRuntimeSnapshot,
    capabilities: &crate::harness_runtime::memory::MemoryRuntimeCapabilityDescriptor,
) -> Option<String> {
    let root = space.root.as_ref()?;
    let manifest_path = root.join("agent.json");
    let manifest = load_manifest_value(&manifest_path)
        .and_then(|(value, _)| parse_memory_manifest(&value))
        .ok()?;
    crate::harness_runtime::memory::unrealizable_memory_spaces(&manifest, capabilities)
        .into_iter()
        .find(|diagnostic| diagnostic.space == space.space)
        .map(|diagnostic| diagnostic.reason)
}

pub(super) fn apply_custom_memory_activation_to_runtime(
    runtime: &mut RuntimeSnapshot,
    activation: &CustomMemoryRuntimeActivation,
) {
    for space in &mut runtime.memory {
        if let Some(reason) = activation.unavailable_packages.get(&space.package) {
            space.state = "unavailable".into();
            space.readiness_reason = Some(reason.clone());
            space.record_types.clear();
            continue;
        }
        if let Some(reason) = activation
            .unavailable_spaces
            .get(&(space.package.clone(), space.space.clone()))
        {
            space.state = "unavailable".into();
            space.readiness_reason = Some(reason.clone());
            space.record_types.clear();
            continue;
        }
        let Some(runtime_capabilities) = activation.capabilities.get(&space.runtime) else {
            continue;
        };
        space.retrieval_modes = space
            .retrieval_modes
            .iter()
            .filter(|mode| runtime_capabilities.retrieval_modes.contains(mode))
            .cloned()
            .collect();
        if space.retrieval_modes.is_empty() {
            space.state = "unavailable".into();
            space.readiness_reason = Some("Memory space has no supported retrieval modes".into());
            space.record_types.clear();
        }
    }
}

pub(super) fn custom_memory_routes(plan: &ResolvedHarnessPlan) -> BTreeMap<String, String> {
    plan.config
        .config
        .memory
        .packages
        .iter()
        .map(|(package, mapping)| (package.clone(), mapping.runtime.clone()))
        .collect()
}
