use super::*;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EffectivePhase {
    pub phase_id: String,
    pub tools_allowed: Option<bool>,
    pub knowledge_allowed: Option<bool>,
    pub memory_read_allowed: Option<bool>,
    pub memory_write_allowed: Option<bool>,
    pub authored_profile_candidates: Vec<String>,
    pub active_profiles: Vec<ActiveProfile>,
    pub active_tools: Vec<ToolRuntimeSnapshot>,
    pub active_skills: Vec<SkillRuntimeSnapshot>,
    pub active_knowledge: Vec<KnowledgeRuntimeSnapshot>,
    pub active_memory: Vec<MemorySpaceRuntimeSnapshot>,
    pub capability_catalog: Vec<CapabilityDescriptor>,
    pub suppressed_capabilities: Vec<SuppressedCapability>,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn runtime_capability_descriptors(
    phase: &LoopPhase,
    runtime: &RuntimeSnapshot,
    tools_allowed: Option<bool>,
    knowledge_allowed: Option<bool>,
    memory_read_allowed: Option<bool>,
    memory_write_allowed: Option<bool>,
    suppressed_capabilities: &mut Vec<SuppressedCapability>,
    active_tools: &mut Vec<ToolRuntimeSnapshot>,
    active_skills: &mut Vec<SkillRuntimeSnapshot>,
    active_knowledge: &mut Vec<KnowledgeRuntimeSnapshot>,
    active_memory: &mut Vec<MemorySpaceRuntimeSnapshot>,
) -> Vec<CapabilityDescriptor> {
    let mut descriptors = Vec::new();
    let mut seen_tools = BTreeSet::new();
    let mut seen_skills = BTreeSet::new();
    let mut seen_knowledge = BTreeSet::new();
    let mut seen_memory_read = BTreeSet::new();
    let mut seen_memory_write = BTreeSet::new();
    for candidate in runtime
        .capability_candidates
        .iter()
        .filter(|candidate| candidate_scope_matches_phase(&candidate.scope, &phase.id))
    {
        match candidate.kind.as_str() {
            "tool" => {
                if !seen_tools.insert(candidate.identity.clone()) {
                    continue;
                }
                if tools_allowed == Some(false) {
                    suppressed_capabilities.push(SuppressedCapability {
                        kind: "tool".into(),
                        identity: candidate.identity.clone(),
                        source: candidate.source.clone(),
                        reason: "Loop access.tools=false for this phase".into(),
                    });
                    continue;
                }
                if !is_available_candidate(candidate) {
                    suppressed_capabilities.push(SuppressedCapability {
                        kind: "tool".into(),
                        identity: candidate.identity.clone(),
                        source: candidate.source.clone(),
                        reason: format!("Tool readiness state is {}", candidate.state),
                    });
                    continue;
                }
                let Some(tool) = runtime
                    .tools
                    .iter()
                    .find(|tool| tool.name == candidate.identity)
                else {
                    suppressed_capabilities.push(SuppressedCapability {
                        kind: "tool".into(),
                        identity: candidate.identity.clone(),
                        source: candidate.source.clone(),
                        reason: "resolved Tool metadata unavailable".into(),
                    });
                    continue;
                };
                active_tools.push(tool.clone());
                descriptors.push(CapabilityDescriptor {
                    action_kind: "agentpm_tool".into(),
                    identity: tool.name.clone(),
                    description: tool.description.clone(),
                    source: candidate.source.clone(),
                });
            }
            "skill" => {
                if !seen_skills.insert(candidate.identity.clone()) {
                    continue;
                }
                if !is_available_candidate(candidate) {
                    suppressed_capabilities.push(SuppressedCapability {
                        kind: "skill".into(),
                        identity: candidate.identity.clone(),
                        source: candidate.source.clone(),
                        reason: format!("Skill readiness state is {}", candidate.state),
                    });
                    continue;
                }
                let Some(skill) = runtime
                    .skills
                    .iter()
                    .find(|skill| skill.name == candidate.identity)
                else {
                    suppressed_capabilities.push(SuppressedCapability {
                        kind: "skill".into(),
                        identity: candidate.identity.clone(),
                        source: candidate.source.clone(),
                        reason: "resolved Skill metadata unavailable".into(),
                    });
                    continue;
                };
                active_skills.push(skill.clone());
                descriptors.push(CapabilityDescriptor {
                    action_kind: "skill_resource_read".into(),
                    identity: skill.name.clone(),
                    description: skill_resource_descriptor_description(skill),
                    source: candidate.source.clone(),
                });
            }
            "knowledge" => {
                if !seen_knowledge.insert(candidate.identity.clone()) {
                    continue;
                }
                if knowledge_allowed == Some(false) {
                    suppressed_capabilities.push(SuppressedCapability {
                        kind: "knowledge".into(),
                        identity: candidate.identity.clone(),
                        source: candidate.source.clone(),
                        reason: "Loop access.knowledge=false for this phase".into(),
                    });
                    continue;
                }
                let Some(knowledge) = runtime
                    .knowledge
                    .iter()
                    .find(|knowledge| knowledge.name == candidate.identity)
                else {
                    suppressed_capabilities.push(SuppressedCapability {
                        kind: "knowledge".into(),
                        identity: candidate.identity.clone(),
                        source: candidate.source.clone(),
                        reason: "resolved Knowledge metadata unavailable".into(),
                    });
                    continue;
                };
                if !is_available_candidate(candidate) || knowledge.state != "available" {
                    suppressed_capabilities.push(SuppressedCapability {
                        kind: "knowledge".into(),
                        identity: candidate.identity.clone(),
                        source: candidate.source.clone(),
                        reason: knowledge.readiness_reason.clone().unwrap_or_else(|| {
                            format!("Knowledge readiness state is {}", knowledge.state)
                        }),
                    });
                    continue;
                }
                active_knowledge.push(knowledge.clone());
                descriptors.push(CapabilityDescriptor {
                    action_kind: "knowledge_request".into(),
                    identity: knowledge.name.clone(),
                    description: knowledge_descriptor_description(knowledge),
                    source: candidate.source.clone(),
                });
            }
            _ => {}
        }
    }
    for memory in runtime
        .memory
        .iter()
        .filter(|memory| candidate_scope_matches_phase(&memory.binding_scope, &phase.id))
    {
        let identity = memory_action_identity(&memory.package, &memory.space);
        let mut suppress_memory = |kind: &str, reason: String| {
            suppressed_capabilities.push(SuppressedCapability {
                kind: kind.into(),
                identity: identity.clone(),
                source: memory.source.clone(),
                reason,
            });
        };
        if memory.state != "available" {
            let reason = memory
                .readiness_reason
                .clone()
                .unwrap_or_else(|| format!("Memory readiness state is {}", memory.state));
            if memory_read_allowed != Some(false) {
                suppress_memory("memory_read", reason.clone());
            }
            if memory_write_allowed != Some(false) {
                suppress_memory("memory_write", reason);
            }
            continue;
        }
        let missing_scope_keys = memory
            .scope_keys
            .iter()
            .filter(|key| {
                runtime
                    .runtime_scopes
                    .get(*key)
                    .is_none_or(|value| value.is_empty())
            })
            .cloned()
            .collect::<Vec<_>>();
        if !missing_scope_keys.is_empty() {
            let reason = format!(
                "unresolved Memory scope keys for space `{}`: {}",
                memory.space,
                missing_scope_keys.join(", ")
            );
            if memory_read_allowed != Some(false) {
                suppress_memory("memory_read", reason.clone());
            }
            if memory_write_allowed != Some(false) {
                suppress_memory("memory_write", reason);
            }
            continue;
        }
        if memory.record_types.is_empty() {
            let reason = format!(
                "generated Memory contracts are unavailable for space `{}`",
                memory.space
            );
            if memory_read_allowed != Some(false) {
                suppress_memory("memory_read", reason.clone());
            }
            if memory_write_allowed != Some(false) {
                suppress_memory("memory_write", reason);
            }
            continue;
        }
        if (memory_read_allowed != Some(false) || memory_write_allowed != Some(false))
            && !active_memory
                .iter()
                .any(|active| active.package == memory.package && active.space == memory.space)
        {
            active_memory.push(memory.clone());
        }
        if memory_read_allowed == Some(false) {
            suppress_memory(
                "memory_read",
                "Loop access.memory.read=false for this phase".into(),
            );
        } else if seen_memory_read.insert(identity.clone()) {
            descriptors.push(CapabilityDescriptor {
                action_kind: "memory_read".into(),
                identity: identity.clone(),
                description: memory_read_descriptor_description(memory),
                source: memory.source.clone(),
            });
        }
        if memory_write_allowed == Some(false) {
            suppress_memory(
                "memory_write",
                "Loop access.memory.write=false for this phase".into(),
            );
        } else if seen_memory_write.insert(identity.clone()) {
            descriptors.push(CapabilityDescriptor {
                action_kind: "memory_write".into(),
                identity,
                description: memory_write_descriptor_description(memory),
                source: memory.source.clone(),
            });
        }
    }
    descriptors
}

pub(super) fn memory_action_identity(package: &str, space: &str) -> String {
    format!("{package}/{space}")
}

pub(super) fn candidate_scope_matches_phase(scope: &str, phase_id: &str) -> bool {
    scope == "global" || scope.strip_prefix("phase:") == Some(phase_id)
}

pub(super) fn is_available_candidate(candidate: &RuntimeCapabilitySnapshot) -> bool {
    candidate.state == "available"
}

pub(super) fn skill_resource_descriptor_description(
    skill: &crate::harness_runtime::SkillRuntimeSnapshot,
) -> String {
    let resources = skill
        .resources
        .iter()
        .map(|resource| resource.id.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "{} Available resources: {}.",
        skill.description,
        if resources.is_empty() {
            "none".into()
        } else {
            resources
        }
    )
}

pub(super) fn knowledge_descriptor_description(knowledge: &KnowledgeRuntimeSnapshot) -> String {
    match knowledge.mode.as_str() {
        "context" => {
            let documents = knowledge
                .documents
                .iter()
                .map(|document| {
                    if let Some(role) = &document.role {
                        format!("{} ({role})", document.path)
                    } else {
                        document.path.clone()
                    }
                })
                .collect::<Vec<_>>()
                .join(", ");
            format!(
                "{} Context Knowledge. Request one declared document when needed. Documents: {}.",
                knowledge.description,
                if documents.is_empty() {
                    "none".into()
                } else {
                    documents
                }
            )
        }
        "vector" => {
            let defaults = knowledge
                .retrieval
                .as_ref()
                .and_then(|retrieval| retrieval.default_top_k)
                .map(|top_k| format!(" Default top_k hint: {top_k}."))
                .unwrap_or_default();
            format!(
                "{} Vector Knowledge. Submit a text query; retrieval returns chunks, sources, scores, and citation metadata.{}",
                knowledge.description, defaults
            )
        }
        _ => knowledge.description.clone(),
    }
}

pub(super) fn memory_read_descriptor_description(memory: &MemorySpaceRuntimeSnapshot) -> String {
    let record_types = memory
        .record_types
        .iter()
        .map(|record_type| format!("{}@{}", record_type.name, record_type.schema_version))
        .collect::<Vec<_>>()
        .join(", ");
    let retrieval_modes = memory
        .retrieval_modes
        .iter()
        .map(memory_retrieval_mode_label)
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "{} Memory read for `{}`/`{}`. Modes: {}. Record types: {}. Scope is resolved by Harness.",
        memory.description,
        memory.package,
        memory.space,
        if retrieval_modes.is_empty() {
            "none".into()
        } else {
            retrieval_modes
        },
        if record_types.is_empty() {
            "none".into()
        } else {
            record_types
        }
    )
}

pub(super) fn memory_write_descriptor_description(memory: &MemorySpaceRuntimeSnapshot) -> String {
    let record_types = memory
        .record_types
        .iter()
        .map(|record_type| format!("{}@{}", record_type.name, record_type.schema_version))
        .collect::<Vec<_>>()
        .join(", ");
    let operations = if matches!(memory.model, MemorySpaceModel::Document) {
        "create, upsert, update, delete, archive"
    } else {
        "create, upsert, update, delete, archive where permitted by constraints"
    };
    format!(
        "{} Memory write for `{}`/`{}`. Operations: {}. Record types: {}. Provide record content only; Harness owns id, scope, envelope, timestamps, ordinal, and provenance.",
        memory.description,
        memory.package,
        memory.space,
        operations,
        if record_types.is_empty() {
            "none".into()
        } else {
            record_types
        }
    )
}

pub(super) fn memory_retrieval_mode_label(mode: &MemoryRetrievalMode) -> &'static str {
    match mode {
        MemoryRetrievalMode::Key => "key",
        MemoryRetrievalMode::Filter => "filter",
        MemoryRetrievalMode::Chronological => "chronological",
        MemoryRetrievalMode::FullText => "full_text",
        MemoryRetrievalMode::Semantic => "semantic",
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ActiveProfile {
    pub name: String,
    pub version: String,
    pub profile: ProfileMetadata,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SuppressedCapability {
    pub kind: String,
    pub identity: String,
    pub source: String,
    pub reason: String,
}

pub(super) fn profile_candidates_for_phase(
    phase: &LoopPhase,
    runtime: &RuntimeSnapshot,
) -> Vec<String> {
    let mut seen = BTreeSet::new();
    let mut candidates = Vec::new();
    for profile in runtime.profile_bindings.global.iter().chain(
        runtime
            .profile_bindings
            .phases
            .get(&phase.id)
            .into_iter()
            .flatten(),
    ) {
        if seen.insert(profile.clone()) {
            candidates.push(profile.clone());
        }
    }
    candidates
}

pub(super) fn active_memory_space<'a>(
    phase: &'a EffectivePhase,
    package: &str,
    space: &str,
) -> Option<&'a MemorySpaceRuntimeSnapshot> {
    phase
        .active_memory
        .iter()
        .find(|candidate| candidate.package == package && candidate.space == space)
}

pub(super) fn memory_read_mode_matches(
    candidate: &MemoryRetrievalMode,
    mode: MemoryReadMode,
) -> bool {
    matches!(
        (candidate, mode),
        (MemoryRetrievalMode::Key, MemoryReadMode::Key)
            | (MemoryRetrievalMode::Filter, MemoryReadMode::Filter)
            | (
                MemoryRetrievalMode::Chronological,
                MemoryReadMode::Chronological
            )
            | (MemoryRetrievalMode::FullText, MemoryReadMode::FullText)
            | (MemoryRetrievalMode::Semantic, MemoryReadMode::Semantic)
    )
}

pub(super) fn memory_read_mode_label(mode: MemoryReadMode) -> &'static str {
    match mode {
        MemoryReadMode::Key => "key",
        MemoryReadMode::Filter => "filter",
        MemoryReadMode::Chronological => "chronological",
        MemoryReadMode::FullText => "full_text",
        MemoryReadMode::Semantic => "semantic",
    }
}

pub(super) fn memory_write_operation_label(operation: MemoryWriteOperation) -> &'static str {
    match operation {
        MemoryWriteOperation::Create => "create",
        MemoryWriteOperation::Upsert => "upsert",
        MemoryWriteOperation::Update => "update",
        MemoryWriteOperation::Delete => "delete",
        MemoryWriteOperation::Archive => "archive",
    }
}

pub(super) fn phase_completion_descriptors(phase: &LoopPhase) -> Vec<CapabilityDescriptor> {
    let description = if phase.outcomes.is_empty() {
        "Complete the phase with implicit outcome `complete`.".to_string()
    } else {
        format!(
            "Complete the phase with one authored outcome: {}.",
            phase
                .outcomes
                .iter()
                .map(|outcome| outcome.id.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        )
    };
    vec![CapabilityDescriptor {
        action_kind: "phase_completion".into(),
        identity: format!("{}/completion", phase.id),
        description,
        source: "loop".into(),
    }]
}

impl EffectivePhase {
    pub(super) fn from_phase(phase: &LoopPhase, runtime: &RuntimeSnapshot) -> Self {
        let access = phase.access.as_ref();
        let authored_profile_candidates = profile_candidates_for_phase(phase, runtime);
        let mut active_profiles = Vec::new();
        let mut suppressed_capabilities = Vec::new();
        for candidate in &authored_profile_candidates {
            if let Some(profile) = runtime
                .profiles
                .iter()
                .find(|profile| profile.name == *candidate)
            {
                active_profiles.push(ActiveProfile::from_snapshot(profile));
            } else {
                suppressed_capabilities.push(SuppressedCapability {
                    kind: "profile".into(),
                    identity: candidate.clone(),
                    source: "agent_binding".into(),
                    reason: "resolved profile metadata unavailable".into(),
                });
            }
        }
        let mut active_tools = Vec::new();
        let mut active_skills = Vec::new();
        let mut active_knowledge = Vec::new();
        let mut active_memory = Vec::new();
        let mut capability_catalog = phase_completion_descriptors(phase);
        capability_catalog.extend(runtime_capability_descriptors(
            phase,
            runtime,
            access.and_then(|access| access.tools),
            access.and_then(|access| access.knowledge),
            access
                .and_then(|access| access.memory.as_ref())
                .and_then(|memory| memory.read),
            access
                .and_then(|access| access.memory.as_ref())
                .and_then(|memory| memory.write),
            &mut suppressed_capabilities,
            &mut active_tools,
            &mut active_skills,
            &mut active_knowledge,
            &mut active_memory,
        ));
        Self {
            phase_id: phase.id.clone(),
            tools_allowed: access.and_then(|access| access.tools),
            knowledge_allowed: access.and_then(|access| access.knowledge),
            memory_read_allowed: access
                .and_then(|access| access.memory.as_ref())
                .and_then(|memory| memory.read),
            memory_write_allowed: access
                .and_then(|access| access.memory.as_ref())
                .and_then(|memory| memory.write),
            authored_profile_candidates,
            active_profiles,
            active_tools,
            active_skills,
            active_knowledge,
            active_memory,
            capability_catalog,
            suppressed_capabilities,
        }
    }

    pub(super) fn permits(&self, action: &SemanticAction) -> bool {
        match action {
            SemanticAction::AgentPmTool { .. } | SemanticAction::ExternalMcpTool { .. } => {
                self.tools_allowed != Some(false)
            }
            SemanticAction::KnowledgeRequest { .. } => self.knowledge_allowed != Some(false),
            SemanticAction::MemoryRead { .. } => self.memory_read_allowed != Some(false),
            SemanticAction::MemoryWrite { .. } => self.memory_write_allowed != Some(false),
            SemanticAction::SkillResourceRead { .. } | SemanticAction::PhaseCompletion { .. } => {
                true
            }
        }
    }
}

impl ActiveProfile {
    fn from_snapshot(snapshot: &ProfileSnapshot) -> Self {
        Self {
            name: snapshot.name.clone(),
            version: snapshot.version.clone(),
            profile: snapshot.profile.clone(),
        }
    }
}
