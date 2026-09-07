use super::*;

pub(super) fn runtime_snapshot_from_plan(plan: &ResolvedHarnessPlan) -> RuntimeSnapshot {
    RuntimeSnapshot {
        session_id: String::new(),
        workspace_root: plan.workspace_root.clone(),
        state_dir: plan.state_dir.clone(),
        agent: plan.selected_agent.as_ref().map(|agent| PackageSnapshot {
            kind: "agent".into(),
            name: agent.name.clone(),
            version: agent.version.clone(),
            root: agent.manifest_path.parent().map(PathBuf::from),
        }),
        loop_package: plan.loop_package.as_ref().map(package_snapshot),
        package_graph: plan.package_graph.values().map(package_snapshot).collect(),
        runtime_config_sources: BTreeMap::from([(
            "state_dir".into(),
            format!("{:?}", plan.config.state_dir_source.kind),
        )]),
        runtime_scopes: plan.runtime_scopes.clone(),
        consumer_context: Some(ConsumerContextSnapshot {
            state: format!("{:?}", plan.consumer_context.state),
            file: plan.consumer_context.file.clone(),
            path: plan.consumer_context.path.clone(),
            content: None,
            byte_size: plan.consumer_context.byte_size,
            approximate_tokens: plan.consumer_context.approximate_tokens,
            sha256: plan.consumer_context.sha256.clone(),
        }),
        services: plan
            .capabilities
            .iter()
            .filter(|capability| capability.kind == "model_provider")
            .map(|capability| ServiceReadinessSnapshot {
                kind: capability.kind.clone(),
                identity: capability.identity.clone(),
                state: format!("{:?}", capability.state),
            })
            .collect(),
        hook_registrations: plan
            .config
            .config
            .hooks
            .bindings
            .iter()
            .map(|binding| format!("{:?}:{}", binding.hook, binding.implementation))
            .collect(),
        profiles: plan.profiles.values().cloned().collect(),
        profile_bindings: plan.profile_bindings.clone(),
        tools: tool_snapshots_from_plan(plan),
        skills: skill_snapshots_from_plan(plan),
        knowledge: knowledge_snapshots_from_plan(plan),
        memory: memory_snapshots_from_plan(plan),
        capability_candidates: plan
            .capabilities
            .iter()
            .map(|capability| RuntimeCapabilitySnapshot {
                kind: capability.kind.clone(),
                identity: capability.identity.clone(),
                scope: capability.scope.clone(),
                source: capability.source.clone(),
                state: capability_state_label(capability.state).into(),
            })
            .collect(),
        model: plan
            .config
            .config
            .model
            .as_ref()
            .map(|model| ModelProviderSelection {
                provider: model.provider.clone(),
                model: model.model.clone(),
                options: model.options.clone(),
            }),
    }
}

fn tool_snapshots_from_plan(plan: &ResolvedHarnessPlan) -> Vec<ToolRuntimeSnapshot> {
    let bound_tools = plan
        .capabilities
        .iter()
        .filter(|capability| capability.kind == "tool")
        .map(|capability| {
            (
                capability.identity.clone(),
                (
                    capability_state_label(capability.state).to_string(),
                    capability.source.clone(),
                ),
            )
        })
        .collect::<BTreeMap<_, _>>();
    plan.package_graph
        .values()
        .filter(|package| package.kind == PackageKind::Tool)
        .filter_map(|package| {
            let (state, source) = bound_tools.get(&package.name)?;
            let path = package.root.join("agent.json");
            let manifest = load_manifest_value(&path)
                .and_then(|(value, _)| parse_tool_manifest(&value))
                .ok()?;
            Some(ToolRuntimeSnapshot {
                name: package.name.clone(),
                version: package.version.clone(),
                description: manifest
                    .description
                    .unwrap_or_else(|| "AgentPM Tool capability.".into()),
                root: Some(package.root.clone()),
                input_schema: manifest.inputs,
                state: state.clone(),
                source: source.clone(),
            })
        })
        .collect()
}

fn skill_snapshots_from_plan(plan: &ResolvedHarnessPlan) -> Vec<SkillRuntimeSnapshot> {
    let bound_skills = plan
        .capabilities
        .iter()
        .filter(|capability| capability.kind == "skill")
        .map(|capability| {
            (
                capability.identity.clone(),
                (
                    capability_state_label(capability.state).to_string(),
                    capability.source.clone(),
                ),
            )
        })
        .collect::<BTreeMap<_, _>>();
    plan.package_graph
        .values()
        .filter(|package| package.kind == PackageKind::Skill)
        .filter_map(|package| {
            let (state, source) = bound_skills.get(&package.name)?;
            let path = package.root.join("agent.json");
            let manifest = load_manifest_value(&path)
                .and_then(|(value, _)| parse_skill_manifest(&value))
                .ok()?;
            let mut resources = vec![SkillResourceSnapshot {
                id: "entrypoint".into(),
                path: manifest.skill.entrypoint.clone(),
                kind: "entrypoint".into(),
            }];
            resources.extend(manifest.skill.references.iter().map(|reference| {
                SkillResourceSnapshot {
                    id: reference.clone(),
                    path: reference.clone(),
                    kind: "reference".into(),
                }
            }));
            Some(SkillRuntimeSnapshot {
                name: package.name.clone(),
                version: package.version.clone(),
                description: manifest
                    .description
                    .unwrap_or_else(|| "AgentPM Skill resource.".into()),
                root: Some(package.root.clone()),
                resources,
                state: state.clone(),
                source: source.clone(),
            })
        })
        .collect()
}

pub(super) fn knowledge_snapshots_from_plan(
    plan: &ResolvedHarnessPlan,
) -> Vec<KnowledgeRuntimeSnapshot> {
    let bound_knowledge = plan
        .capabilities
        .iter()
        .filter(|capability| capability.kind == "knowledge")
        .map(|capability| {
            (
                capability.identity.clone(),
                (
                    capability_state_label(capability.state).to_string(),
                    capability.source.clone(),
                ),
            )
        })
        .collect::<BTreeMap<_, _>>();
    plan.package_graph
        .values()
        .filter(|package| package.kind == PackageKind::Knowledge)
        .filter_map(|package| {
            let (candidate_state, source) = bound_knowledge.get(&package.name)?;
            let path = package.root.join("agent.json");
            let manifest = load_manifest_value(&path)
                .and_then(|(value, _)| parse_knowledge_manifest(&value))
                .ok()?;
            let (runtime, state, readiness_reason) =
                knowledge_runtime_readiness(plan, package, &manifest, candidate_state);
            let mut snapshot = crate::harness_runtime::knowledge::knowledge_snapshot_from_manifest(
                &package.root,
                &manifest,
                source.clone(),
                runtime,
                state,
                readiness_reason,
            );
            snapshot.name = package.name.clone();
            Some(snapshot)
        })
        .collect()
}

pub(super) fn memory_snapshots_from_plan(
    plan: &ResolvedHarnessPlan,
) -> Vec<MemorySpaceRuntimeSnapshot> {
    let Some(agent) = &plan.selected_agent else {
        return Vec::new();
    };
    let agent_manifest = load_manifest_value(&agent.manifest_path)
        .and_then(|(value, _)| serde_json::from_value::<AgentManifest>(value).map_err(Into::into))
        .ok();
    let Some(agent_manifest) = agent_manifest else {
        return Vec::new();
    };

    let mut snapshots = Vec::new();
    if let Some(bindings) = agent_manifest.bindings.as_ref() {
        if let Some(global) = bindings.global.as_ref() {
            snapshots.extend(memory_binding_snapshots_from_plan(
                plan,
                &global.memory,
                "global",
            ));
        }
        let mut phase_bindings = bindings.phases.iter().collect::<Vec<_>>();
        phase_bindings.sort_by_key(|(phase_id, _)| *phase_id);
        for (phase_id, scope) in phase_bindings {
            snapshots.extend(memory_binding_snapshots_from_plan(
                plan,
                &scope.memory,
                &format!("phase:{phase_id}"),
            ));
        }
    }
    snapshots
}

fn memory_binding_snapshots_from_plan(
    plan: &ResolvedHarnessPlan,
    bindings: &[AgentMemoryBinding],
    binding_scope: &str,
) -> Vec<MemorySpaceRuntimeSnapshot> {
    let mut snapshots = Vec::new();
    for binding in bindings {
        let package_name = package_identity_for_harness_runtime(&binding.package);
        let Some(package) = plan
            .package_graph
            .values()
            .find(|package| package.kind == PackageKind::Memory && package.name == package_name)
        else {
            continue;
        };
        let manifest_path = package.root.join("agent.json");
        let manifest = load_manifest_value(&manifest_path)
            .and_then(|(value, _)| parse_memory_manifest(&value))
            .ok();
        let Some(manifest) = manifest else {
            continue;
        };
        let contract_result =
            crate::harness_runtime::memory::validate_and_load_memory_contracts(&package.root);

        for space_name in &binding.spaces {
            let Some(space) = manifest.memory.spaces.get(space_name) else {
                continue;
            };
            let (runtime, mut state, mut readiness_reason) =
                memory_runtime_readiness(plan, &package_name);
            let (semantic, semantic_unavailable_reason) =
                memory_semantic_snapshot_for_plan(plan, space);
            let capabilities = if runtime == "local" && semantic.is_none() {
                crate::harness_runtime::memory::MemoryRuntimeCapabilityDescriptor::local_sqlite()
            } else {
                crate::harness_runtime::memory::MemoryRuntimeCapabilityDescriptor::local_sqlite_with_semantic()
            };
            let unrealizable = crate::harness_runtime::memory::unrealizable_memory_spaces(
                &manifest,
                &capabilities,
            )
            .into_iter()
            .map(|diagnostic| (diagnostic.space, diagnostic.reason))
            .collect::<BTreeMap<_, _>>();
            if state == "available"
                && let Some(reason) = unrealizable.get(space_name)
            {
                state = "unavailable".into();
                readiness_reason = semantic_unavailable_reason
                    .clone()
                    .or_else(|| Some(reason.clone()));
            }
            let retrieval_modes = memory_retrieval_modes_for_runtime(space, &capabilities);
            if state == "available" && retrieval_modes.is_empty() {
                state = "unavailable".into();
                readiness_reason = semantic_unavailable_reason
                    .clone()
                    .or_else(|| Some("Memory space has no supported retrieval modes".into()));
            }
            let record_types = if state == "available" {
                match &contract_result {
                    Ok(contracts) => memory_record_type_snapshots(&manifest, contracts, space_name),
                    Err(err) => {
                        state = "unavailable".into();
                        readiness_reason = Some(err.to_string());
                        Vec::new()
                    }
                }
            } else {
                Vec::new()
            };
            snapshots.push(MemorySpaceRuntimeSnapshot {
                package: package.name.clone(),
                package_version: package.version.clone(),
                space: space_name.clone(),
                model: space.model.clone(),
                description: space.description.clone(),
                root: Some(package.root.clone()),
                runtime: runtime.clone(),
                source: "agent_binding".into(),
                state,
                readiness_reason,
                binding_scope: binding_scope.to_string(),
                scope_keys: space.scope.clone(),
                retrieval_modes,
                semantic,
                append_only: space
                    .constraints
                    .as_ref()
                    .and_then(|constraints| constraints.append_only)
                    .unwrap_or(false),
                record_types,
            });
        }
    }
    snapshots
}

fn memory_retrieval_modes_for_runtime(
    space: &crate::manifest::MemorySpace,
    capabilities: &crate::harness_runtime::memory::MemoryRuntimeCapabilityDescriptor,
) -> Vec<MemoryRetrievalMode> {
    space
        .retrieval
        .modes
        .iter()
        .filter(|mode| capabilities.retrieval_modes.contains(mode))
        .cloned()
        .collect()
}

fn memory_semantic_snapshot_for_plan(
    plan: &ResolvedHarnessPlan,
    space: &crate::manifest::MemorySpace,
) -> (Option<KnowledgeEmbeddingSnapshot>, Option<String>) {
    if !space
        .retrieval
        .modes
        .contains(&MemoryRetrievalMode::Semantic)
    {
        return (None, None);
    }
    let Some(semantic) = &plan.config.config.memory.local.semantic else {
        return (
            None,
            Some("Memory semantic retrieval requires memory.local.semantic configuration".into()),
        );
    };
    if !plan
        .config
        .config
        .providers
        .embeddings
        .contains_key(&semantic.embedding_provider)
    {
        return (
            None,
            Some(format!(
                "Memory semantic retrieval references undefined EmbeddingProvider `{}`",
                semantic.embedding_provider
            )),
        );
    }
    if !configured_embedding_provider_is_realizable(plan, &semantic.embedding_provider) {
        return (
            None,
            Some(format!(
                "Memory semantic retrieval requires available EmbeddingProvider `{}`",
                semantic.embedding_provider
            )),
        );
    }
    (
        Some(KnowledgeEmbeddingSnapshot {
            id: "memory.local.semantic".into(),
            provider: semantic.embedding_provider.clone(),
            model: semantic.model.clone(),
            dimensions: semantic.dimensions,
            metric: "cosine".into(),
            normalized: true,
        }),
        None,
    )
}

fn package_identity_for_harness_runtime(raw: &str) -> String {
    raw.rsplit_once('@')
        .filter(|(name, version)| !name.is_empty() && !version.is_empty())
        .map(|(name, _)| name.to_string())
        .unwrap_or_else(|| raw.to_string())
}

fn memory_runtime_readiness(
    plan: &ResolvedHarnessPlan,
    package_name: &str,
) -> (String, String, Option<String>) {
    if let Some(mapping) = plan.config.config.memory.packages.get(package_name) {
        return (mapping.runtime.clone(), "available".into(), None);
    }
    ("local".into(), "available".into(), None)
}

fn memory_record_type_snapshots(
    manifest: &MemoryManifest,
    contracts: &crate::harness_runtime::memory::ValidatedMemoryContracts,
    space_name: &str,
) -> Vec<MemoryRecordTypeRuntimeSnapshot> {
    let Some(space) = manifest.memory.spaces.get(space_name) else {
        return Vec::new();
    };
    space
        .record_types
        .iter()
        .filter_map(|record_type| {
            let metadata = manifest.memory.record_types.get(record_type)?;
            let content_schema = crate::harness_runtime::memory::generated_memory_content_schema(
                contracts,
                space_name,
                record_type,
            )
            .ok()?;
            Some(MemoryRecordTypeRuntimeSnapshot {
                name: record_type.clone(),
                schema_version: metadata.version.clone(),
                content_schema,
            })
        })
        .collect()
}

fn knowledge_runtime_readiness(
    plan: &ResolvedHarnessPlan,
    package: &ResolvedPackageInfo,
    manifest: &crate::manifest::KnowledgeManifest,
    candidate_state: &str,
) -> (String, String, Option<String>) {
    if candidate_state != "available" {
        return (
            "none".into(),
            candidate_state.to_string(),
            Some(format!(
                "Knowledge binding readiness state is {candidate_state}"
            )),
        );
    }
    if let Some(mapping) = plan.config.config.knowledge.packages.get(&package.name) {
        return configured_knowledge_runtime_readiness(plan, &mapping.runtime);
    }
    match manifest.knowledge.mode.as_str() {
        "context" => {
            match crate::commands::knowledge::build_context_mode(&package.root, manifest) {
                Ok(result) => {
                    let mut mismatches = Vec::new();
                    match manifest.knowledge.context.as_ref() {
                        Some(context) => {
                            if context.document_count != Some(result.document_count) {
                                mismatches.push("knowledge.context.document_count");
                            }
                            if context.total_bytes != Some(result.total_bytes) {
                                mismatches.push("knowledge.context.total_bytes");
                            }
                            if context.content_hash.as_deref() != Some(result.content_hash.as_str())
                            {
                                mismatches.push("knowledge.context.content_hash");
                            }
                        }
                        None => mismatches.push("knowledge.context"),
                    }
                    if mismatches.is_empty() {
                        ("local".into(), "available".into(), None)
                    } else {
                        (
                            "local".into(),
                            "unavailable".into(),
                            Some(format!(
                                "context Knowledge metadata is stale or malformed: {}",
                                mismatches.join(", ")
                            )),
                        )
                    }
                }
                Err(err) => ("local".into(), "unavailable".into(), Some(err.to_string())),
            }
        }
        "vector" => match crate::commands::knowledge::local_vector_readiness(&package.root) {
            Ok(readiness) => {
                let space = KnowledgeEmbeddingSnapshot {
                    id: readiness.embedding_id,
                    provider: readiness.provider,
                    model: readiness.model,
                    dimensions: readiness.dimensions,
                    metric: readiness.metric,
                    normalized: readiness.normalized,
                };
                match compatible_embedding_provider_id(plan, &space) {
                    Some(provider_id)
                        if configured_embedding_provider_is_realizable(plan, &provider_id) =>
                    {
                        ("local".into(), "available".into(), None)
                    }
                    Some(provider_id) => (
                        "local".into(),
                        "unavailable".into(),
                        Some(format!(
                            "installed vector artifacts are coherent but EmbeddingProvider `{provider_id}` is unavailable for the current execution surface"
                        )),
                    ),
                    None => (
                        "local".into(),
                        "unavailable".into(),
                        Some(format!(
                            "installed vector artifacts are coherent but no compatible EmbeddingProvider is configured for {}/{}/dimensions={}/normalized={}",
                            space.provider, space.model, space.dimensions, space.normalized
                        )),
                    ),
                }
            }
            Err(err) => (
                "local".into(),
                "unavailable".into(),
                Some(format!("installed vector artifact integrity failed: {err}")),
            ),
        },
        other => (
            "none".into(),
            "unavailable".into(),
            Some(format!("unsupported Knowledge mode `{other}`")),
        ),
    }
}

fn configured_knowledge_runtime_readiness(
    plan: &ResolvedHarnessPlan,
    runtime_id: &str,
) -> (String, String, Option<String>) {
    let Some(_entry) = plan.config.config.knowledge.runtimes.get(runtime_id) else {
        return (
            runtime_id.to_string(),
            "unavailable".into(),
            Some(format!(
                "knowledge.packages references undefined KnowledgeRuntime `{runtime_id}`"
            )),
        );
    };
    match configured_runtime_candidate_state(plan, "knowledge_runtime", runtime_id) {
        Some(
            CapabilityState::Unavailable
            | CapabilityState::Suppressed
            | CapabilityState::NotConfigured,
        ) => (
            runtime_id.to_string(),
            "unavailable".into(),
            Some(format!(
                "configured KnowledgeRuntime `{runtime_id}` is unavailable for the current execution surface"
            )),
        ),
        Some(CapabilityState::Available | CapabilityState::Pending) | None => {
            (runtime_id.to_string(), "available".into(), None)
        }
    }
}

fn configured_embedding_provider_is_realizable(
    plan: &ResolvedHarnessPlan,
    provider_id: &str,
) -> bool {
    matches!(
        configured_runtime_candidate_state(plan, "embedding_provider", provider_id),
        Some(CapabilityState::Available | CapabilityState::Pending) | None
    )
}

fn configured_runtime_candidate_state(
    plan: &ResolvedHarnessPlan,
    kind: &str,
    identity: &str,
) -> Option<CapabilityState> {
    plan.capabilities
        .iter()
        .find(|capability| capability.kind == kind && capability.identity == identity)
        .map(|capability| capability.state)
}

fn compatible_embedding_provider_id(
    plan: &ResolvedHarnessPlan,
    space: &KnowledgeEmbeddingSnapshot,
) -> Option<String> {
    plan.config
        .config
        .knowledge
        .embedding_matches
        .iter()
        .find(|item| crate::harness_runtime::knowledge::embedding_key_matches(space, &item.r#match))
        .map(|item| item.embedding_provider.clone())
}

fn capability_state_label(state: CapabilityState) -> &'static str {
    match state {
        CapabilityState::Available => "available",
        CapabilityState::Pending => "pending",
        CapabilityState::Unavailable => "unavailable",
        CapabilityState::Suppressed => "suppressed",
        CapabilityState::NotConfigured => "not_configured",
    }
}

fn package_snapshot(package: &ResolvedPackageInfo) -> PackageSnapshot {
    PackageSnapshot {
        kind: format!("{:?}", package.kind),
        name: package.name.clone(),
        version: package.version.clone(),
        root: Some(package.root.clone()),
    }
}
