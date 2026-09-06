use super::*;
use crate::harness_observability::{HarnessEventEnvelope, InMemoryEventSink};
use crate::harness_runtime::action::{
    ActionDispatchResult, ActionFailureCategory, MemoryReadMode, MemoryWriteOperation,
    ScriptedActionDispatcher, SemanticActionProposal,
};
use crate::harness_runtime::approval::ScriptedApprovalController;
use crate::harness_runtime::hook::{
    BeforeKnowledgeRequestDecision, BeforeKnowledgeRequestHook, BeforeModelRequestContextSection,
    BeforeModelRequestDecision, BeforeModelRequestHook, BeforeToolCallDecision,
    BeforeToolSelectionDecision, BeforeToolSelectionHook, HookRuntimeFailure,
};
use crate::harness_runtime::model::{
    KnowledgeEmbeddingSnapshot, MemoryRecordTypeRuntimeSnapshot, MemorySpaceRuntimeSnapshot,
    ModelProviderSelection, ModelRuntimeFailure, ModelTurn, RuntimeCapabilitySnapshot,
    SUCCESSFUL_ACTION_RESULT_CONTROL, ScriptedModelRuntime, SkillResourceSnapshot,
    SkillRuntimeSnapshot, ToolRuntimeSnapshot,
};
use crate::manifest::{
    LoopAccessMemory, LoopCheckpoint, LoopErrorPolicy, LoopLimits, LoopMetadata, LoopOutcome,
    LoopPhaseAccess, LoopPhaseFailurePolicy, LoopToolFailurePolicy, LoopTransition,
    MemoryRetrievalMode, MemorySpaceModel,
};
use std::collections::VecDeque;
use std::path::PathBuf;

fn hook_event_fields_for(
    events: &[HarnessEventEnvelope],
    event_type: HarnessEventType,
    hook: &str,
) -> BTreeMap<String, Value> {
    for event in events {
        if event.event_type != event_type {
            continue;
        }
        let HarnessEventPayload::Lifecycle { fields, .. } = &event.payload else {
            continue;
        };
        if fields.get("hook") == Some(&json!(hook)) {
            return fields.clone();
        }
    }
    panic!("missing {event_type:?} event for {hook}");
}

fn limits() -> HarnessRuntimeLimits {
    HarnessRuntimeLimits {
        max_steps: 8,
        max_model_calls_per_phase: 8,
        max_tool_calls_per_phase: 8,
        max_actions_per_phase: 16,
        max_tool_call_repairs: 2,
        max_structured_output_repairs: 2,
        max_memory_operation_repairs: 2,
    }
}

fn base_loop() -> LoopManifest {
    LoopManifest {
        kind: "loop".into(),
        name: "@zack/test-loop".into(),
        version: "0.1.0".into(),
        description: None,
        readme: None,
        license: None,
        r#loop: LoopMetadata {
            archetype: Some("test".into()),
            entry_phase: "assess".into(),
            limits: None,
            phases: vec![
                LoopPhase {
                    id: "assess".into(),
                    objective: "Assess request.".into(),
                    access: None,
                    outcomes: vec![
                        LoopOutcome {
                            id: "execute".into(),
                            description: "Execute.".into(),
                        },
                        LoopOutcome {
                            id: "handoff".into(),
                            description: "Hand off.".into(),
                        },
                    ],
                },
                LoopPhase {
                    id: "execute".into(),
                    objective: "Execute.".into(),
                    access: None,
                    outcomes: vec![LoopOutcome {
                        id: "review".into(),
                        description: "Review.".into(),
                    }],
                },
                LoopPhase {
                    id: "review".into(),
                    objective: "Review.".into(),
                    access: None,
                    outcomes: vec![
                        LoopOutcome {
                            id: "again".into(),
                            description: "Try again.".into(),
                        },
                        LoopOutcome {
                            id: "ready".into(),
                            description: "Ready.".into(),
                        },
                    ],
                },
            ],
            transitions: vec![
                LoopTransition {
                    from: "assess".into(),
                    on: "execute".into(),
                    to: "execute".into(),
                },
                LoopTransition {
                    from: "assess".into(),
                    on: "handoff".into(),
                    to: "$handoff".into(),
                },
                LoopTransition {
                    from: "execute".into(),
                    on: "review".into(),
                    to: "review".into(),
                },
                LoopTransition {
                    from: "review".into(),
                    on: "again".into(),
                    to: "execute".into(),
                },
                LoopTransition {
                    from: "review".into(),
                    on: "ready".into(),
                    to: "$end".into(),
                },
            ],
            checkpoints: Vec::new(),
            error_policy: None,
        },
    }
}

fn completion(id: &str, outcome: &str) -> ModelTurn {
    ModelTurn {
        assistant_content: Some(format!("content for {outcome}")),
        actions: vec![SemanticActionProposal::new(
            id,
            SemanticAction::PhaseCompletion {
                outcome: Some(outcome.into()),
                output: Some(json!({ "outcome": outcome })),
            },
        )],
        usage: RunUsage::default(),
        finish_reason: None,
        provider_metadata: BTreeMap::new(),
    }
}

fn profile_snapshot(name: &str, version: &str, role: &str) -> ProfileSnapshot {
    ProfileSnapshot {
        name: name.into(),
        version: version.into(),
        profile: serde_json::from_value(json!({
            "identity": {
                "role": role,
                "description": "Profile identity description.",
                "expertise": ["support operations"]
            },
            "objectives": ["Keep the answer actionable."],
            "principles": ["Prefer evidence over speculation."],
            "audience": {
                "description": "Operators.",
                "assumed_knowledge": "Basic incident terminology.",
                "adaptation": ["Use direct next steps."]
            },
            "communication": {
                "tone": ["calm", "precise"],
                "verbosity": "concise",
                "guidelines": ["Lead with the decision."],
                "formatting": ["Use compact bullets."],
                "vocabulary": {
                    "prefer": ["next checkpoint"],
                    "avoid": ["obviously"]
                }
            },
            "boundaries": ["Do not invent evidence."],
            "constraints": [
                {
                    "id": "cite-evidence",
                    "strength": "required",
                    "instruction": "Tie recommendations to observed evidence."
                },
                {
                    "id": "state-risk",
                    "strength": "preferred",
                    "instruction": "Name residual risk when present."
                }
            ]
        }))
        .unwrap(),
    }
}

fn temp_context_file(label: &str, content: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "agentpm-harness-engine-{label}-{}-context.md",
        std::process::id()
    ));
    std::fs::write(&path, content).unwrap();
    path
}

fn temp_workspace_dir(label: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "agentpm-harness-engine-{label}-{}-{}",
        std::process::id(),
        Utc::now().timestamp_nanos_opt().unwrap_or_default()
    ));
    std::fs::create_dir_all(&path).unwrap();
    path
}

fn runtime_with_tool_and_skill() -> RuntimeSnapshot {
    let mut runtime = RuntimeSnapshot::empty("session-test".into());
    runtime.tools.push(ToolRuntimeSnapshot {
        name: "@zack/search".into(),
        version: "0.1.0".into(),
        description: "Search incident records.".into(),
        root: None,
        input_schema: json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "query": { "type": "string" }
            },
            "required": ["query"]
        }),
        state: "available".into(),
        source: "agent_binding".into(),
    });
    runtime.skills.push(SkillRuntimeSnapshot {
        name: "@zack/skill".into(),
        version: "0.1.0".into(),
        description: "Use procedural guidance.".into(),
        root: None,
        resources: vec![
            SkillResourceSnapshot {
                id: "entrypoint".into(),
                path: "SKILL.md".into(),
                kind: "entrypoint".into(),
            },
            SkillResourceSnapshot {
                id: "references/handoff-template.md".into(),
                path: "references/handoff-template.md".into(),
                kind: "reference".into(),
            },
        ],
        state: "available".into(),
        source: "agent_binding".into(),
    });
    runtime.capability_candidates = vec![
        RuntimeCapabilitySnapshot {
            kind: "tool".into(),
            identity: "@zack/search".into(),
            scope: "global".into(),
            source: "agent_binding".into(),
            state: "available".into(),
        },
        RuntimeCapabilitySnapshot {
            kind: "skill".into(),
            identity: "@zack/skill".into(),
            scope: "global".into(),
            source: "agent_binding".into(),
            state: "available".into(),
        },
    ];
    runtime
}

fn vector_knowledge_snapshot(name: &str) -> KnowledgeRuntimeSnapshot {
    KnowledgeRuntimeSnapshot {
        name: name.into(),
        version: "0.1.0".into(),
        mode: "vector".into(),
        description: format!("{name} search corpus."),
        root: None,
        source: "agent_binding".into(),
        state: "available".into(),
        runtime: "local".into(),
        readiness_reason: None,
        documents: Vec::new(),
        embedding: None,
        retrieval: None,
    }
}

fn runtime_with_knowledge_packages(packages: &[&str]) -> RuntimeSnapshot {
    let mut runtime = RuntimeSnapshot::empty("session-test".into());
    for package in packages {
        runtime.knowledge.push(vector_knowledge_snapshot(package));
        runtime
            .capability_candidates
            .push(RuntimeCapabilitySnapshot {
                kind: "knowledge".into(),
                identity: (*package).into(),
                scope: "global".into(),
                source: "agent_binding".into(),
                state: "available".into(),
            });
    }
    runtime
}

fn one_phase_memory_loop(access: Option<LoopPhaseAccess>) -> LoopManifest {
    LoopManifest {
        kind: "loop".into(),
        name: "m14c-memory-loop".into(),
        version: "0.1.0".into(),
        description: None,
        readme: None,
        license: None,
        r#loop: LoopMetadata {
            archetype: None,
            entry_phase: "remember".into(),
            limits: None,
            phases: vec![LoopPhase {
                id: "remember".into(),
                objective: "Use direct Memory when useful.".into(),
                access,
                outcomes: vec![LoopOutcome {
                    id: "done".into(),
                    description: "Done.".into(),
                }],
            }],
            transitions: vec![LoopTransition {
                from: "remember".into(),
                on: "done".into(),
                to: "$end".into(),
            }],
            checkpoints: Vec::new(),
            error_policy: None,
        },
    }
}

fn write_m14c_memory_package(root: &std::path::Path) -> MemoryRecordTypeRuntimeSnapshot {
    std::fs::create_dir_all(root.join("schemas")).unwrap();
    std::fs::write(
        root.join("agent.json"),
        r#"{
  "kind": "memory",
  "name": "m14c-memory-test",
  "version": "0.1.0",
  "description": "M14c direct Memory engine test package.",
  "memory": {
    "scopes": {
      "user": { "description": "User scope." }
    },
    "record_types": {
      "note": {
        "version": "1.0.0",
        "description": "Durable note.",
        "schema": "schemas/note.schema.json"
      },
      "task": {
        "version": "1.0.0",
        "description": "Durable task.",
        "schema": "schemas/task.schema.json"
      },
      "profile_a": {
        "version": "1.0.0",
        "description": "Profile shape A.",
        "schema": "schemas/profile-a.schema.json"
      },
      "profile_b": {
        "version": "1.0.0",
        "description": "Profile shape B.",
        "schema": "schemas/profile-b.schema.json"
      }
    },
    "spaces": {
      "notes": {
        "description": "Direct notes.",
        "model": "collection",
        "record_types": ["note", "task"],
        "scope": ["user"],
        "retrieval": { "modes": ["key", "filter", "chronological", "full_text"] },
        "capacity": { "max_records": 1 }
      },
      "profile": {
        "description": "Single current profile.",
        "model": "document",
        "record_types": ["profile_a", "profile_b"],
        "scope": ["user"],
        "retrieval": { "modes": ["key"] }
      }
    }
  }
}
"#,
    )
    .unwrap();
    std::fs::write(
        root.join("schemas/note.schema.json"),
        r#"{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "type": "object",
  "properties": {
    "body": { "type": "string", "minLength": 1 },
    "labels": {
      "type": "array",
      "items": { "type": "string", "minLength": 1 }
    },
    "assignee": {
      "type": "object",
      "properties": {
        "team": { "type": "string", "minLength": 1 }
      },
      "additionalProperties": false
    }
  },
  "required": ["body"],
  "additionalProperties": false
}
"#,
    )
    .unwrap();
    std::fs::write(
        root.join("schemas/task.schema.json"),
        r#"{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "type": "object",
  "properties": {
    "title": { "type": "string", "minLength": 1 }
  },
  "required": ["title"],
  "additionalProperties": false
}
"#,
    )
    .unwrap();
    std::fs::write(
        root.join("schemas/profile-a.schema.json"),
        r#"{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "type": "object",
  "properties": {
    "name": { "type": "string", "minLength": 1 }
  },
  "required": ["name"],
  "additionalProperties": false
}
"#,
    )
    .unwrap();
    std::fs::write(
        root.join("schemas/profile-b.schema.json"),
        r#"{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "type": "object",
  "properties": {
    "display": { "type": "string", "minLength": 1 }
  },
  "required": ["display"],
  "additionalProperties": false
}
"#,
    )
    .unwrap();
    crate::commands::memory::execute_memory_build(
        &root.join("agent.json"),
        crate::commands::memory::MemoryBuildMode::Write,
    )
    .unwrap();
    let contracts =
        crate::harness_runtime::memory::validate_and_load_memory_contracts(root).unwrap();
    MemoryRecordTypeRuntimeSnapshot {
        name: "note".into(),
        schema_version: "1.0.0".into(),
        content_schema: crate::harness_runtime::memory::generated_memory_content_schema(
            &contracts, "notes", "note",
        )
        .unwrap(),
    }
}

fn m14c_task_record_type_snapshot(root: &std::path::Path) -> MemoryRecordTypeRuntimeSnapshot {
    let contracts =
        crate::harness_runtime::memory::validate_and_load_memory_contracts(root).unwrap();
    MemoryRecordTypeRuntimeSnapshot {
        name: "task".into(),
        schema_version: "1.0.0".into(),
        content_schema: crate::harness_runtime::memory::generated_memory_content_schema(
            &contracts, "notes", "task",
        )
        .unwrap(),
    }
}

fn m14c_profile_record_type_snapshot(
    root: &std::path::Path,
    record_type: &str,
) -> MemoryRecordTypeRuntimeSnapshot {
    let contracts =
        crate::harness_runtime::memory::validate_and_load_memory_contracts(root).unwrap();
    MemoryRecordTypeRuntimeSnapshot {
        name: record_type.into(),
        schema_version: "1.0.0".into(),
        content_schema: crate::harness_runtime::memory::generated_memory_content_schema(
            &contracts,
            "profile",
            record_type,
        )
        .unwrap(),
    }
}

fn runtime_with_m14c_memory(
    workspace: &std::path::Path,
    package_root: &std::path::Path,
    binding_scope: &str,
    state: &str,
    include_scope: bool,
) -> RuntimeSnapshot {
    let record_type = write_m14c_memory_package(package_root);
    let mut runtime = RuntimeSnapshot::empty("session-test".into());
    runtime.workspace_root = workspace.to_path_buf();
    runtime.state_dir = workspace.join(".agentpm-state");
    if include_scope {
        runtime
            .runtime_scopes
            .insert("user".into(), "user-123".into());
    }
    runtime.memory.push(MemorySpaceRuntimeSnapshot {
        package: "m14c-memory-test".into(),
        package_version: "0.1.0".into(),
        space: "notes".into(),
        model: MemorySpaceModel::Collection,
        description: "Direct notes.".into(),
        root: Some(package_root.to_path_buf()),
        runtime: "local".into(),
        source: "agent_binding".into(),
        state: state.into(),
        readiness_reason: (state != "available").then(|| "test unavailable".into()),
        binding_scope: binding_scope.into(),
        scope_keys: vec!["user".into()],
        retrieval_modes: vec![
            MemoryRetrievalMode::Key,
            MemoryRetrievalMode::Filter,
            MemoryRetrievalMode::Chronological,
            MemoryRetrievalMode::FullText,
        ],
        append_only: false,
        record_types: vec![record_type],
    });
    runtime
}

fn session_with_tool_and_skill() -> HarnessSession {
    HarnessSession::with_runtime_snapshot(runtime_with_tool_and_skill())
}

#[test]
fn memory_descriptors_require_bound_ready_space_and_trusted_scope() {
    let temp = temp_workspace_dir("m14c-descriptors");
    let package_root = temp.join("memory-package");
    std::fs::create_dir_all(&package_root).unwrap();
    let runtime = runtime_with_m14c_memory(&temp, &package_root, "global", "available", true);
    let effective =
        EffectivePhase::from_phase(&one_phase_memory_loop(None).r#loop.phases[0], &runtime);
    let notes_read = effective
        .capability_catalog
        .iter()
        .find(|descriptor| {
            descriptor.action_kind == "memory_read"
                && descriptor.identity == "m14c-memory-test/notes"
        })
        .expect("notes read descriptor");
    assert!(notes_read.description.contains("For collection spaces, key requires record_id; use filter, chronological, or full_text to find/list records when available."));
    assert!(effective.capability_catalog.iter().any(|descriptor| {
        descriptor.action_kind == "memory_write" && descriptor.identity == "m14c-memory-test/notes"
    }));

    let missing_scope_runtime =
        runtime_with_m14c_memory(&temp, &package_root, "global", "available", false);
    let missing_scope = EffectivePhase::from_phase(
        &one_phase_memory_loop(None).r#loop.phases[0],
        &missing_scope_runtime,
    );
    assert!(
        !missing_scope
            .capability_catalog
            .iter()
            .any(|descriptor| descriptor.action_kind == "memory_read")
    );
    assert!(
        missing_scope
            .suppressed_capabilities
            .iter()
            .any(|suppressed| {
                suppressed.kind == "memory_read"
                    && suppressed.reason.contains("unresolved Memory scope keys")
            })
    );

    let read_disabled_loop = one_phase_memory_loop(Some(LoopPhaseAccess {
        tools: None,
        knowledge: None,
        memory: Some(LoopAccessMemory {
            read: Some(false),
            write: Some(true),
        }),
    }));
    let write_only = EffectivePhase::from_phase(&read_disabled_loop.r#loop.phases[0], &runtime);
    assert!(
        !write_only
            .capability_catalog
            .iter()
            .any(|descriptor| descriptor.action_kind == "memory_read")
    );
    assert!(
        write_only
            .capability_catalog
            .iter()
            .any(|descriptor| descriptor.action_kind == "memory_write")
    );
}

#[test]
fn memory_descriptors_union_global_and_phase_scoped_bindings() {
    let temp = temp_workspace_dir("m14c-memory-union");
    let package_root = temp.join("memory-package");
    std::fs::create_dir_all(&package_root).unwrap();
    let mut runtime = runtime_with_m14c_memory(&temp, &package_root, "global", "available", true);
    let mut phase_memory = runtime.memory[0].clone();
    phase_memory.space = "phase_notes".into();
    phase_memory.description = "Phase notes.".into();
    phase_memory.binding_scope = "phase:remember".into();
    let mut other_phase_memory = runtime.memory[0].clone();
    other_phase_memory.space = "other_phase_notes".into();
    other_phase_memory.description = "Other phase notes.".into();
    other_phase_memory.binding_scope = "phase:other".into();
    runtime.memory.push(phase_memory);
    runtime.memory.push(other_phase_memory);

    let effective =
        EffectivePhase::from_phase(&one_phase_memory_loop(None).r#loop.phases[0], &runtime);
    let identities = effective
        .capability_catalog
        .iter()
        .filter(|descriptor| descriptor.action_kind == "memory_read")
        .map(|descriptor| descriptor.identity.as_str())
        .collect::<Vec<_>>();

    assert_eq!(
        identities,
        vec!["m14c-memory-test/notes", "m14c-memory-test/phase_notes"]
    );
    assert!(effective.active_memory.iter().any(|memory| {
        memory.package == "m14c-memory-test"
            && memory.space == "notes"
            && memory.binding_scope == "global"
    }));
    assert!(effective.active_memory.iter().any(|memory| {
        memory.package == "m14c-memory-test"
            && memory.space == "phase_notes"
            && memory.binding_scope == "phase:remember"
    }));
    assert!(
        !effective
            .active_memory
            .iter()
            .any(|memory| memory.space == "other_phase_notes")
    );
}

#[test]
fn memory_read_descriptors_explain_key_mode_by_space_model() {
    let temp = temp_workspace_dir("m14c-read-key-descriptors");
    let package_root = temp.join("memory-package");
    std::fs::create_dir_all(&package_root).unwrap();
    let record_type = write_m14c_memory_package(&package_root);
    let mut runtime = RuntimeSnapshot::empty("session-test".into());
    runtime.workspace_root = temp.to_path_buf();
    runtime.state_dir = temp.join(".agentpm-state");
    runtime
        .runtime_scopes
        .insert("user".into(), "user-123".into());
    runtime.memory = vec![
        MemorySpaceRuntimeSnapshot {
            package: "m14c-memory-test".into(),
            package_version: "0.1.0".into(),
            space: "session".into(),
            model: MemorySpaceModel::Document,
            description: "Current session.".into(),
            root: Some(package_root.to_path_buf()),
            runtime: "local".into(),
            source: "agent_binding".into(),
            state: "available".into(),
            readiness_reason: None,
            binding_scope: "global".into(),
            scope_keys: vec!["user".into()],
            retrieval_modes: vec![MemoryRetrievalMode::Key],
            append_only: false,
            record_types: vec![record_type.clone()],
        },
        MemorySpaceRuntimeSnapshot {
            package: "m14c-memory-test".into(),
            package_version: "0.1.0".into(),
            space: "log".into(),
            model: MemorySpaceModel::Sequence,
            description: "Ordered log.".into(),
            root: Some(package_root.to_path_buf()),
            runtime: "local".into(),
            source: "agent_binding".into(),
            state: "available".into(),
            readiness_reason: None,
            binding_scope: "global".into(),
            scope_keys: vec!["user".into()],
            retrieval_modes: vec![MemoryRetrievalMode::Key, MemoryRetrievalMode::Chronological],
            append_only: false,
            record_types: vec![record_type],
        },
    ];
    let effective =
        EffectivePhase::from_phase(&one_phase_memory_loop(None).r#loop.phases[0], &runtime);

    let document_read = effective
        .capability_catalog
        .iter()
        .find(|descriptor| {
            descriptor.action_kind == "memory_read"
                && descriptor.identity == "m14c-memory-test/session"
        })
        .expect("document read descriptor");
    assert!(document_read.description.contains(
        "For document spaces, key reads the current scoped document and does not require record_id."
    ));
    let sequence_read = effective
        .capability_catalog
        .iter()
        .find(|descriptor| {
            descriptor.action_kind == "memory_read" && descriptor.identity == "m14c-memory-test/log"
        })
        .expect("sequence read descriptor");
    assert!(sequence_read.description.contains(
        "For sequence spaces, key requires record_id; use chronological to find/list records when available."
    ));
    assert!(!sequence_read.description.contains("filter"));
    assert!(!sequence_read.description.contains("full_text"));
}

#[test]
fn direct_memory_actions_route_to_local_runtime_and_phase_transcript() {
    let temp = temp_workspace_dir("m14c-direct");
    let package_root = temp.join("memory-package");
    std::fs::create_dir_all(&package_root).unwrap();
    let runtime = runtime_with_m14c_memory(&temp, &package_root, "global", "available", true);
    let mut session = HarnessSession::with_runtime_snapshot(runtime);
    let memory = InMemoryEventSink::default();
    let handle = memory.clone();
    session.emitter.add_sink(Box::new(memory));
    let mut model = ScriptedModelRuntime::new(vec![
        ModelTurn {
            assistant_content: None,
            actions: vec![SemanticActionProposal::new(
                "write",
                SemanticAction::MemoryWrite {
                    package: "m14c-memory-test".into(),
                    space: "notes".into(),
                    operation: MemoryWriteOperation::Create,
                    record_type: "note".into(),
                    record_id: None,
                    content: Some(json!({
                        "body": "Alpha launch checklist",
                        "labels": ["alpha", "release"],
                        "assignee": { "team": "platform" }
                    })),
                },
            )],
            usage: RunUsage::default(),
            finish_reason: Some("tool_calls".into()),
            provider_metadata: BTreeMap::new(),
        },
        ModelTurn {
            assistant_content: None,
            actions: vec![SemanticActionProposal::new(
                "read",
                SemanticAction::MemoryRead {
                    package: "m14c-memory-test".into(),
                    space: "notes".into(),
                    mode: MemoryReadMode::Filter,
                    record_id: None,
                    record_type: Some("note".into()),
                    filter: BTreeMap::from([("labels".into(), json!("release"))]),
                    query: None,
                    limit: Some(1),
                },
            )],
            usage: RunUsage::default(),
            finish_reason: Some("tool_calls".into()),
            provider_metadata: BTreeMap::new(),
        },
        completion("done", "done"),
    ]);
    let mut dispatcher = ScriptedActionDispatcher::default();
    let mut approvals = ScriptedApprovalController::default();
    let mut engine = HarnessEngine::new(
        one_phase_memory_loop(None),
        HarnessEngineOptions::new(limits()),
    );
    let result = engine
        .execute_run(
            &mut session,
            "remember this",
            &mut model,
            &mut dispatcher,
            &mut approvals,
        )
        .unwrap();
    let HarnessRunResult::Terminal(result) = result else {
        panic!("expected terminal result");
    };
    assert_eq!(result.report.terminal_status, HarnessTerminalStatus::Ended);
    assert_eq!(result.report.usage.memory_requests, 2);
    assert_eq!(result.report.memory_summaries.len(), 2);
    assert!(
        dispatcher.dispatched.is_empty(),
        "Memory must not use fake dispatcher path"
    );

    let request_after_write = &model.requests[1];
    let transcript_text = request_after_write.prompt.render_text();
    assert!(transcript_text.contains("ActionResult [memory_write m14c-memory-test/notes]"));
    assert!(transcript_text.contains(SUCCESSFUL_ACTION_RESULT_CONTROL));
    let request_after_read = &model.requests[2];
    let transcript_text = request_after_read.prompt.render_text();
    assert!(transcript_text.contains("ActionResult [memory_read m14c-memory-test/notes]"));
    assert!(transcript_text.contains("Alpha launch checklist"));

    let events = handle.events();
    assert!(
        events
            .iter()
            .any(|event| event.event_type == HarnessEventType::MemoryWriteStarted)
    );
    assert!(
        events
            .iter()
            .any(|event| event.event_type == HarnessEventType::MemoryWriteCompleted)
    );
    assert!(
        events
            .iter()
            .any(|event| event.event_type == HarnessEventType::MemoryReadStarted)
    );
    assert!(
        events
            .iter()
            .any(|event| event.event_type == HarnessEventType::MemoryReadCompleted)
    );
    let write_completed = events
        .iter()
        .find(|event| event.event_type == HarnessEventType::MemoryWriteCompleted)
        .expect("memory write completed event");
    let HarnessEventPayload::Action { fields, .. } = &write_completed.payload else {
        panic!("expected action payload");
    };
    let provenance = &fields["result"]["record"]["provenance"]["harness"];
    assert_eq!(provenance["kind"], json!("harness_direct_memory_write"));
    assert_eq!(provenance["run_id"], json!(result.report.run_id));
    assert_eq!(provenance["phase_execution_id"], json!("phase-exec-1"));
    assert_eq!(provenance["phase_id"], json!("remember"));
    assert_eq!(provenance["action_kind"], json!("memory_write"));
    assert_eq!(provenance["operation"], json!("create"));
    assert_eq!(provenance["source"], json!("agent_binding"));
}

#[test]
fn simplified_provider_memory_content_still_receives_authoritative_validation() {
    let temp = temp_workspace_dir("m14c-schema-simplification-validation");
    let package_root = temp.join("memory-package");
    std::fs::create_dir_all(&package_root).unwrap();
    let mut runtime = runtime_with_m14c_memory(&temp, &package_root, "global", "available", true);
    runtime.memory[0]
        .record_types
        .push(m14c_task_record_type_snapshot(&package_root));
    let mut session = HarnessSession::with_runtime_snapshot(runtime);
    let memory = InMemoryEventSink::default();
    let handle = memory.clone();
    session.emitter.add_sink(Box::new(memory));
    let mut model = ScriptedModelRuntime::new(vec![
        ModelTurn {
            assistant_content: None,
            actions: vec![SemanticActionProposal::new(
                "provider-compatible-but-contract-invalid",
                SemanticAction::MemoryWrite {
                    package: "m14c-memory-test".into(),
                    space: "notes".into(),
                    operation: MemoryWriteOperation::Create,
                    record_type: "note".into(),
                    record_id: None,
                    content: Some(json!({
                        "title": "valid for task, invalid for note"
                    })),
                },
            )],
            usage: RunUsage::default(),
            finish_reason: Some("tool_calls".into()),
            provider_metadata: BTreeMap::new(),
        },
        completion("done", "done"),
    ]);
    let mut dispatcher = ScriptedActionDispatcher::default();
    let mut approvals = ScriptedApprovalController::default();
    let mut engine = HarnessEngine::new(
        one_phase_memory_loop(None),
        HarnessEngineOptions::new(limits()),
    );
    let result = engine
        .execute_run(
            &mut session,
            "invalid memory",
            &mut model,
            &mut dispatcher,
            &mut approvals,
        )
        .unwrap();

    let HarnessRunResult::Terminal(result) = result else {
        panic!("expected terminal result");
    };
    assert_eq!(result.report.terminal_status, HarnessTerminalStatus::Ended);
    assert_eq!(result.report.usage.memory_requests, 0);
    assert!(dispatcher.dispatched.is_empty());
    assert!(
        !handle
            .events()
            .iter()
            .any(|event| event.event_type == HarnessEventType::MemoryWriteStarted)
    );
    assert!(handle.events().iter().any(|event| {
        event.event_type == HarnessEventType::SemanticActionRejected
            && matches!(
                &event.payload,
                HarnessEventPayload::Action { status, .. } if status == "invalid_arguments"
            )
    }));
    assert!(
        model.requests[1]
            .prompt
            .render_text()
            .contains("Memory content")
    );
}

#[test]
fn memory_capacity_overflow_returns_typed_structured_failure() {
    let temp = temp_workspace_dir("m14c-memory-capacity");
    let package_root = temp.join("memory-package");
    std::fs::create_dir_all(&package_root).unwrap();
    let runtime = runtime_with_m14c_memory(&temp, &package_root, "global", "available", true);
    let mut session = HarnessSession::with_runtime_snapshot(runtime);
    let memory = InMemoryEventSink::default();
    let handle = memory.clone();
    session.emitter.add_sink(Box::new(memory));
    let mut model = ScriptedModelRuntime::new(vec![
        ModelTurn {
            assistant_content: None,
            actions: vec![SemanticActionProposal::new(
                "write-1",
                SemanticAction::MemoryWrite {
                    package: "m14c-memory-test".into(),
                    space: "notes".into(),
                    operation: MemoryWriteOperation::Create,
                    record_type: "note".into(),
                    record_id: None,
                    content: Some(json!({ "body": "first note" })),
                },
            )],
            usage: RunUsage::default(),
            finish_reason: Some("tool_calls".into()),
            provider_metadata: BTreeMap::new(),
        },
        ModelTurn {
            assistant_content: None,
            actions: vec![SemanticActionProposal::new(
                "write-2",
                SemanticAction::MemoryWrite {
                    package: "m14c-memory-test".into(),
                    space: "notes".into(),
                    operation: MemoryWriteOperation::Create,
                    record_type: "note".into(),
                    record_id: None,
                    content: Some(json!({ "body": "second note" })),
                },
            )],
            usage: RunUsage::default(),
            finish_reason: Some("tool_calls".into()),
            provider_metadata: BTreeMap::new(),
        },
        completion("done", "done"),
    ]);
    let mut dispatcher = ScriptedActionDispatcher::default();
    let mut approvals = ScriptedApprovalController::default();
    let mut engine = HarnessEngine::new(
        one_phase_memory_loop(None),
        HarnessEngineOptions::new(limits()),
    );
    let result = engine
        .execute_run(
            &mut session,
            "overflow memory capacity",
            &mut model,
            &mut dispatcher,
            &mut approvals,
        )
        .unwrap();

    let HarnessRunResult::Terminal(result) = result else {
        panic!("expected terminal result");
    };
    assert_eq!(result.report.terminal_status, HarnessTerminalStatus::Ended);
    assert_eq!(result.report.usage.memory_requests, 2);
    assert!(dispatcher.dispatched.is_empty());
    assert!(
        !handle
            .events()
            .iter()
            .any(|event| event.event_type == HarnessEventType::SemanticActionRejected)
    );
    assert!(handle.events().iter().any(|event| {
        if event.event_type != HarnessEventType::MemoryWriteFailed {
            return false;
        }
        let HarnessEventPayload::Action { fields, .. } = &event.payload else {
            return false;
        };
        fields
            .get("result")
            .and_then(|result| result.get("error"))
            .and_then(|error| error.get("code"))
            .and_then(Value::as_str)
            == Some("capacity_exceeded")
    }));
    assert!(
        model.requests[2]
            .prompt
            .render_text()
            .contains("\"code\":\"capacity_exceeded\"")
    );
}

#[test]
fn memory_actions_count_against_action_limit_not_tool_limit() {
    let temp = temp_workspace_dir("m14c-memory-action-limit-tool-limit");
    let package_root = temp.join("memory-package");
    std::fs::create_dir_all(&package_root).unwrap();
    let runtime = runtime_with_m14c_memory(&temp, &package_root, "global", "available", true);
    let mut session = HarnessSession::with_runtime_snapshot(runtime);
    let mut model = ScriptedModelRuntime::new(vec![
        ModelTurn {
            assistant_content: None,
            actions: vec![SemanticActionProposal::new(
                "write",
                SemanticAction::MemoryWrite {
                    package: "m14c-memory-test".into(),
                    space: "notes".into(),
                    operation: MemoryWriteOperation::Create,
                    record_type: "note".into(),
                    record_id: None,
                    content: Some(json!({ "body": "allowed despite zero tool calls" })),
                },
            )],
            usage: RunUsage::default(),
            finish_reason: Some("tool_calls".into()),
            provider_metadata: BTreeMap::new(),
        },
        completion("done", "done"),
    ]);
    let mut dispatcher = ScriptedActionDispatcher::default();
    let mut approvals = ScriptedApprovalController::default();
    let mut runtime_limits = limits();
    runtime_limits.max_tool_calls_per_phase = 0;
    let mut engine = HarnessEngine::new(
        one_phase_memory_loop(None),
        HarnessEngineOptions::new(runtime_limits),
    );
    let result = engine
        .execute_run(
            &mut session,
            "write memory",
            &mut model,
            &mut dispatcher,
            &mut approvals,
        )
        .unwrap();
    let HarnessRunResult::Terminal(result) = result else {
        panic!("expected terminal result");
    };
    assert_eq!(result.report.terminal_status, HarnessTerminalStatus::Ended);
    assert_eq!(result.report.usage.memory_requests, 1);
    assert_eq!(result.report.usage.tool_calls, 0);
    assert!(dispatcher.dispatched.is_empty());

    let temp = temp_workspace_dir("m14c-memory-action-limit-exhaustion");
    let package_root = temp.join("memory-package");
    std::fs::create_dir_all(&package_root).unwrap();
    let runtime = runtime_with_m14c_memory(&temp, &package_root, "global", "available", true);
    let mut session = HarnessSession::with_runtime_snapshot(runtime);
    let memory = InMemoryEventSink::default();
    let handle = memory.clone();
    session.emitter.add_sink(Box::new(memory));
    let mut model = ScriptedModelRuntime::new(vec![ModelTurn {
        assistant_content: None,
        actions: vec![
            SemanticActionProposal::new(
                "write",
                SemanticAction::MemoryWrite {
                    package: "m14c-memory-test".into(),
                    space: "notes".into(),
                    operation: MemoryWriteOperation::Create,
                    record_type: "note".into(),
                    record_id: None,
                    content: Some(json!({ "body": "first memory action" })),
                },
            ),
            SemanticActionProposal::new(
                "read",
                SemanticAction::MemoryRead {
                    package: "m14c-memory-test".into(),
                    space: "notes".into(),
                    mode: MemoryReadMode::Filter,
                    record_id: None,
                    record_type: Some("note".into()),
                    filter: BTreeMap::from([("body".into(), json!("first memory action"))]),
                    query: None,
                    limit: Some(1),
                },
            ),
        ],
        usage: RunUsage::default(),
        finish_reason: Some("tool_calls".into()),
        provider_metadata: BTreeMap::new(),
    }]);
    let mut runtime_limits = limits();
    runtime_limits.max_actions_per_phase = 1;
    let mut engine = HarnessEngine::new(
        one_phase_memory_loop(None),
        HarnessEngineOptions::new(runtime_limits),
    );
    let result = engine
        .execute_run(
            &mut session,
            "write then read memory",
            &mut model,
            &mut dispatcher,
            &mut approvals,
        )
        .unwrap();
    let HarnessRunResult::Terminal(result) = result else {
        panic!("expected terminal limit result");
    };
    assert_eq!(result.status, HarnessTerminalStatus::LimitReached);
    assert_eq!(result.report.usage.memory_requests, 1);
    assert_eq!(result.report.usage.tool_calls, 0);
    assert_eq!(
        handle
            .events()
            .iter()
            .filter(|event| event.event_type == HarnessEventType::MemoryWriteCompleted)
            .count(),
        1
    );
    assert!(
        !handle
            .events()
            .iter()
            .any(|event| event.event_type == HarnessEventType::MemoryReadStarted)
    );
}

#[test]
fn invalid_memory_write_and_unknown_filter_path_request_repair_before_dispatch() {
    let temp = temp_workspace_dir("m14c-repair");
    let package_root = temp.join("memory-package");
    std::fs::create_dir_all(&package_root).unwrap();
    let runtime = runtime_with_m14c_memory(&temp, &package_root, "global", "available", true);
    let mut session = HarnessSession::with_runtime_snapshot(runtime);
    let memory = InMemoryEventSink::default();
    let handle = memory.clone();
    session.emitter.add_sink(Box::new(memory));
    let mut model = ScriptedModelRuntime::new(vec![
        ModelTurn {
            assistant_content: None,
            actions: vec![SemanticActionProposal::new(
                "bad-write",
                SemanticAction::MemoryWrite {
                    package: "m14c-memory-test".into(),
                    space: "notes".into(),
                    operation: MemoryWriteOperation::Create,
                    record_type: "note".into(),
                    record_id: None,
                    content: Some(json!({ "labels": ["missing-body"] })),
                },
            )],
            usage: RunUsage::default(),
            finish_reason: Some("tool_calls".into()),
            provider_metadata: BTreeMap::new(),
        },
        ModelTurn {
            assistant_content: None,
            actions: vec![SemanticActionProposal::new(
                "bad-read",
                SemanticAction::MemoryRead {
                    package: "m14c-memory-test".into(),
                    space: "notes".into(),
                    mode: MemoryReadMode::Filter,
                    record_id: None,
                    record_type: Some("note".into()),
                    filter: BTreeMap::from([("unknown.path".into(), json!("x"))]),
                    query: None,
                    limit: None,
                },
            )],
            usage: RunUsage::default(),
            finish_reason: Some("tool_calls".into()),
            provider_metadata: BTreeMap::new(),
        },
        completion("done", "done"),
    ]);
    let mut dispatcher = ScriptedActionDispatcher::default();
    let mut approvals = ScriptedApprovalController::default();
    let mut engine = HarnessEngine::new(
        one_phase_memory_loop(None),
        HarnessEngineOptions::new(limits()),
    );
    let result = engine
        .execute_run(
            &mut session,
            "invalid memory",
            &mut model,
            &mut dispatcher,
            &mut approvals,
        )
        .unwrap();

    let HarnessRunResult::Terminal(result) = result else {
        panic!("expected terminal result");
    };
    assert_eq!(result.report.usage.memory_requests, 0);
    assert!(dispatcher.dispatched.is_empty());
    let rejected = handle
        .events()
        .into_iter()
        .filter(|event| event.event_type == HarnessEventType::SemanticActionRejected)
        .count();
    assert_eq!(rejected, 2);
    assert!(
        model.requests[1]
            .prompt
            .render_text()
            .contains("Memory content")
    );
    assert!(
        model.requests[2]
            .prompt
            .render_text()
            .contains("Memory filter path `unknown.path`")
    );
}

#[test]
fn append_only_memory_write_rejects_mutation_before_dispatch() {
    let temp = temp_workspace_dir("m14c-append-only-repair");
    let package_root = temp.join("memory-package");
    std::fs::create_dir_all(&package_root).unwrap();
    let mut runtime = runtime_with_m14c_memory(&temp, &package_root, "global", "available", true);
    runtime.memory[0].append_only = true;
    let mut session = HarnessSession::with_runtime_snapshot(runtime);
    let memory = InMemoryEventSink::default();
    let handle = memory.clone();
    session.emitter.add_sink(Box::new(memory));
    let mut model = ScriptedModelRuntime::new(vec![
        ModelTurn {
            assistant_content: None,
            actions: vec![SemanticActionProposal::new(
                "bad-update",
                SemanticAction::MemoryWrite {
                    package: "m14c-memory-test".into(),
                    space: "notes".into(),
                    operation: MemoryWriteOperation::Update,
                    record_type: "note".into(),
                    record_id: Some("mem_existing".into()),
                    content: Some(json!({ "body": "updated body" })),
                },
            )],
            usage: RunUsage::default(),
            finish_reason: Some("tool_calls".into()),
            provider_metadata: BTreeMap::new(),
        },
        completion("done", "done"),
    ]);
    let mut dispatcher = ScriptedActionDispatcher::default();
    let mut approvals = ScriptedApprovalController::default();
    let mut engine = HarnessEngine::new(
        one_phase_memory_loop(None),
        HarnessEngineOptions::new(limits()),
    );
    let result = engine
        .execute_run(
            &mut session,
            "invalid append-only memory",
            &mut model,
            &mut dispatcher,
            &mut approvals,
        )
        .unwrap();

    let HarnessRunResult::Terminal(result) = result else {
        panic!("expected terminal result");
    };
    assert_eq!(result.report.usage.memory_requests, 0);
    assert!(dispatcher.dispatched.is_empty());
    assert!(handle.events().iter().any(|event| {
        if event.event_type != HarnessEventType::SemanticActionRejected {
            return false;
        }
        let HarnessEventPayload::Action { fields, .. } = &event.payload else {
            return false;
        };
        fields
            .get("error")
            .and_then(Value::as_str)
            .is_some_and(|error| error.contains("append-only"))
    }));
    assert!(
        model.requests[1]
            .prompt
            .render_text()
            .contains("append-only")
    );
}

#[test]
fn duplicate_document_create_requests_repair_after_runtime_lookup() {
    let temp = temp_workspace_dir("m14c-duplicate-document-create-repair");
    let package_root = temp.join("memory-package");
    std::fs::create_dir_all(&package_root).unwrap();
    let mut runtime = runtime_with_m14c_memory(&temp, &package_root, "global", "available", true);
    runtime.memory.push(MemorySpaceRuntimeSnapshot {
        package: "m14c-memory-test".into(),
        package_version: "0.1.0".into(),
        space: "profile".into(),
        model: MemorySpaceModel::Document,
        description: "Single current profile.".into(),
        root: Some(package_root.to_path_buf()),
        runtime: "local".into(),
        source: "agent_binding".into(),
        state: "available".into(),
        readiness_reason: None,
        binding_scope: "global".into(),
        scope_keys: vec!["user".into()],
        retrieval_modes: vec![MemoryRetrievalMode::Key],
        append_only: false,
        record_types: vec![
            m14c_profile_record_type_snapshot(&package_root, "profile_a"),
            m14c_profile_record_type_snapshot(&package_root, "profile_b"),
        ],
    });
    let mut session = HarnessSession::with_runtime_snapshot(runtime);
    let memory = InMemoryEventSink::default();
    let handle = memory.clone();
    session.emitter.add_sink(Box::new(memory));
    let mut model = ScriptedModelRuntime::new(vec![
        ModelTurn {
            assistant_content: None,
            actions: vec![SemanticActionProposal::new(
                "create-profile-a",
                SemanticAction::MemoryWrite {
                    package: "m14c-memory-test".into(),
                    space: "profile".into(),
                    operation: MemoryWriteOperation::Create,
                    record_type: "profile_a".into(),
                    record_id: None,
                    content: Some(json!({ "name": "A" })),
                },
            )],
            usage: RunUsage::default(),
            finish_reason: Some("tool_calls".into()),
            provider_metadata: BTreeMap::new(),
        },
        ModelTurn {
            assistant_content: None,
            actions: vec![SemanticActionProposal::new(
                "duplicate-create-profile-b",
                SemanticAction::MemoryWrite {
                    package: "m14c-memory-test".into(),
                    space: "profile".into(),
                    operation: MemoryWriteOperation::Create,
                    record_type: "profile_b".into(),
                    record_id: None,
                    content: Some(json!({ "display": "B" })),
                },
            )],
            usage: RunUsage::default(),
            finish_reason: Some("tool_calls".into()),
            provider_metadata: BTreeMap::new(),
        },
        completion("done", "done"),
    ]);
    let mut dispatcher = ScriptedActionDispatcher::default();
    let mut approvals = ScriptedApprovalController::default();
    let mut engine = HarnessEngine::new(
        one_phase_memory_loop(None),
        HarnessEngineOptions::new(limits()),
    );
    let result = engine
        .execute_run(
            &mut session,
            "duplicate document create",
            &mut model,
            &mut dispatcher,
            &mut approvals,
        )
        .unwrap();

    let HarnessRunResult::Terminal(result) = result else {
        panic!("expected terminal result");
    };
    assert_eq!(result.report.terminal_status, HarnessTerminalStatus::Ended);
    assert_eq!(result.report.usage.memory_requests, 2);
    assert_eq!(result.report.repair_count, 1);
    assert!(dispatcher.dispatched.is_empty());
    let expected = "Memory document create for space `profile` requires no current document for the resolved scope";
    assert!(handle.events().iter().any(|event| {
        if event.event_type != HarnessEventType::SemanticActionRejected {
            return false;
        }
        let HarnessEventPayload::Action { fields, .. } = &event.payload else {
            return false;
        };
        fields
            .get("error")
            .and_then(Value::as_str)
            .is_some_and(|error| error.contains(expected))
    }));
    assert!(model.requests[2].prompt.render_text().contains(expected));

    let (manifest_value, _) = load_manifest_value(&package_root.join("agent.json")).unwrap();
    let manifest = parse_memory_manifest(&manifest_value).unwrap();
    let records = session
        .local_memory_runtime()
        .unwrap()
        .read_records(LocalMemoryReadRequest {
            package: "m14c-memory-test",
            package_version: "0.1.0",
            manifest: &manifest,
            space: "profile",
            scope: BTreeMap::from([("user".into(), "user-123".into())]),
            mode: LocalMemoryReadMode::Key,
            record_id: None,
            record_type: None,
            filter: BTreeMap::new(),
            query: None,
            limit: None,
            now: Utc::now(),
        })
        .unwrap();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].record_type, "profile_a");
    assert_eq!(records[0].content, json!({ "name": "A" }));
}

#[test]
fn missing_memory_write_target_requests_repair_after_runtime_lookup() {
    let temp = temp_workspace_dir("m14c-missing-memory-target-repair");
    let package_root = temp.join("memory-package");
    std::fs::create_dir_all(&package_root).unwrap();
    let runtime = runtime_with_m14c_memory(&temp, &package_root, "global", "available", true);
    let mut session = HarnessSession::with_runtime_snapshot(runtime);
    let memory = InMemoryEventSink::default();
    let handle = memory.clone();
    session.emitter.add_sink(Box::new(memory));
    let mut model = ScriptedModelRuntime::new(vec![
        ModelTurn {
            assistant_content: None,
            actions: vec![SemanticActionProposal::new(
                "missing-update",
                SemanticAction::MemoryWrite {
                    package: "m14c-memory-test".into(),
                    space: "notes".into(),
                    operation: MemoryWriteOperation::Update,
                    record_type: "note".into(),
                    record_id: Some("mem_missing".into()),
                    content: Some(json!({ "body": "updated body" })),
                },
            )],
            usage: RunUsage::default(),
            finish_reason: Some("tool_calls".into()),
            provider_metadata: BTreeMap::new(),
        },
        completion("done", "done"),
    ]);
    let mut dispatcher = ScriptedActionDispatcher::default();
    let mut approvals = ScriptedApprovalController::default();
    let mut engine = HarnessEngine::new(
        one_phase_memory_loop(None),
        HarnessEngineOptions::new(limits()),
    );
    let result = engine
        .execute_run(
            &mut session,
            "missing memory target",
            &mut model,
            &mut dispatcher,
            &mut approvals,
        )
        .unwrap();

    let HarnessRunResult::Terminal(result) = result else {
        panic!("expected terminal result");
    };
    assert_eq!(result.report.terminal_status, HarnessTerminalStatus::Ended);
    assert_eq!(result.report.usage.memory_requests, 1);
    assert!(dispatcher.dispatched.is_empty());
    assert!(handle.events().iter().any(|event| {
        if event.event_type != HarnessEventType::SemanticActionRejected {
            return false;
        }
        let HarnessEventPayload::Action { fields, .. } = &event.payload else {
            return false;
        };
        fields
            .get("error")
            .and_then(Value::as_str)
            .is_some_and(|error| error.contains("Memory record `mem_missing` was not found"))
    }));
    assert!(
        model.requests[1]
            .prompt
            .render_text()
            .contains("Memory record `mem_missing` was not found")
    );
}

#[test]
fn memory_write_target_record_type_mismatch_requests_repair_after_runtime_lookup() {
    let temp = temp_workspace_dir("m14c-memory-target-type-repair");
    let package_root = temp.join("memory-package");
    std::fs::create_dir_all(&package_root).unwrap();
    let mut runtime = runtime_with_m14c_memory(&temp, &package_root, "global", "available", true);
    runtime.memory[0]
        .record_types
        .push(m14c_task_record_type_snapshot(&package_root));
    let mut session = HarnessSession::with_runtime_snapshot(runtime);
    let memory = InMemoryEventSink::default();
    let handle = memory.clone();
    session.emitter.add_sink(Box::new(memory));

    let (manifest_value, _) = load_manifest_value(&package_root.join("agent.json")).unwrap();
    let manifest = parse_memory_manifest(&manifest_value).unwrap();
    let contracts =
        crate::harness_runtime::memory::validate_and_load_memory_contracts(&package_root).unwrap();
    let seeded = session
        .local_memory_runtime()
        .unwrap()
        .write_record(LocalMemoryWriteRequest {
            package: "m14c-memory-test",
            package_version: "0.1.0",
            manifest: &manifest,
            contracts: &contracts,
            space: "notes",
            record_type: "note",
            scope: BTreeMap::from([("user".into(), "user-123".into())]),
            operation: LocalMemoryWriteOperation::Create,
            record_id: None,
            content: Some(json!({ "body": "seed note" })),
            provenance: json!({}),
            now: Utc::now(),
        })
        .unwrap();
    let seeded_id = seeded.affected_record_id.unwrap();

    let mut model = ScriptedModelRuntime::new(vec![
        ModelTurn {
            assistant_content: None,
            actions: vec![SemanticActionProposal::new(
                "wrong-type-update",
                SemanticAction::MemoryWrite {
                    package: "m14c-memory-test".into(),
                    space: "notes".into(),
                    operation: MemoryWriteOperation::Update,
                    record_type: "task".into(),
                    record_id: Some(seeded_id.clone()),
                    content: Some(json!({ "title": "retitled as task" })),
                },
            )],
            usage: RunUsage::default(),
            finish_reason: Some("tool_calls".into()),
            provider_metadata: BTreeMap::new(),
        },
        completion("done", "done"),
    ]);
    let mut dispatcher = ScriptedActionDispatcher::default();
    let mut approvals = ScriptedApprovalController::default();
    let mut engine = HarnessEngine::new(
        one_phase_memory_loop(None),
        HarnessEngineOptions::new(limits()),
    );
    let result = engine
        .execute_run(
            &mut session,
            "wrong memory target type",
            &mut model,
            &mut dispatcher,
            &mut approvals,
        )
        .unwrap();

    let HarnessRunResult::Terminal(result) = result else {
        panic!("expected terminal result");
    };
    assert_eq!(result.report.terminal_status, HarnessTerminalStatus::Ended);
    assert_eq!(result.report.usage.memory_requests, 1);
    assert!(dispatcher.dispatched.is_empty());
    let expected = format!("Memory update target `{seeded_id}` has record type `note` not `task`");
    assert!(handle.events().iter().any(|event| {
        if event.event_type != HarnessEventType::SemanticActionRejected {
            return false;
        }
        let HarnessEventPayload::Action { fields, .. } = &event.payload else {
            return false;
        };
        fields
            .get("error")
            .and_then(Value::as_str)
            .is_some_and(|error| error == expected)
    }));
    assert!(model.requests[1].prompt.render_text().contains(&expected));
}

#[test]
fn memory_runtime_failure_returns_structured_action_result_without_fake_dispatch() {
    let temp = temp_workspace_dir("m14c-runtime-failure");
    let package_root = temp.join("memory-package");
    std::fs::create_dir_all(&package_root).unwrap();
    let mut runtime = runtime_with_m14c_memory(&temp, &package_root, "global", "available", true);
    runtime.memory[0].runtime = "process-memory-fixture".into();
    runtime.memory[0].readiness_reason = Some("M14e custom runtime dispatch is not active".into());
    let mut session = HarnessSession::with_runtime_snapshot(runtime);
    let memory = InMemoryEventSink::default();
    let handle = memory.clone();
    session.emitter.add_sink(Box::new(memory));
    let mut model = ScriptedModelRuntime::new(vec![
        ModelTurn {
            assistant_content: None,
            actions: vec![SemanticActionProposal::new(
                "read",
                SemanticAction::MemoryRead {
                    package: "m14c-memory-test".into(),
                    space: "notes".into(),
                    mode: MemoryReadMode::Key,
                    record_id: Some("mem_missing".into()),
                    record_type: Some("note".into()),
                    filter: BTreeMap::new(),
                    query: None,
                    limit: None,
                },
            )],
            usage: RunUsage::default(),
            finish_reason: Some("tool_calls".into()),
            provider_metadata: BTreeMap::new(),
        },
        completion("done", "done"),
    ]);
    let mut dispatcher = ScriptedActionDispatcher::default();
    let mut approvals = ScriptedApprovalController::default();
    let mut engine = HarnessEngine::new(
        one_phase_memory_loop(None),
        HarnessEngineOptions::new(limits()),
    );
    let result = engine
        .execute_run(
            &mut session,
            "read unavailable memory",
            &mut model,
            &mut dispatcher,
            &mut approvals,
        )
        .unwrap();

    let HarnessRunResult::Terminal(result) = result else {
        panic!("expected terminal result");
    };
    assert_eq!(result.report.terminal_status, HarnessTerminalStatus::Ended);
    assert_eq!(result.report.usage.memory_requests, 1);
    assert!(dispatcher.dispatched.is_empty());
    assert_eq!(result.report.memory_summaries.len(), 1);
    assert_eq!(result.report.memory_summaries[0].status, "failed");
    assert_eq!(result.report.action_summaries.len(), 1);
    assert_eq!(
        result.report.action_summaries[0].error.as_deref(),
        Some("M14e custom runtime dispatch is not active")
    );
    assert!(
        model.requests[1]
            .prompt
            .render_text()
            .contains("memory_runtime_unavailable")
    );
    let events = handle.events();
    assert!(
        events
            .iter()
            .any(|event| event.event_type == HarnessEventType::MemoryReadStarted)
    );
    assert!(
        events
            .iter()
            .any(|event| event.event_type == HarnessEventType::MemoryReadFailed)
    );
}

#[derive(Default)]
struct TestHookRuntime {
    tool_selection: Option<BeforeToolSelectionDecision>,
    tool_selection_hooks: Vec<BeforeToolSelectionHook>,
    nonfatal_before_tool_selection: Option<String>,
    model_request: Option<BeforeModelRequestDecision>,
    tool_call: Option<BeforeToolCallDecision>,
    tool_call_hooks: Vec<BeforeToolCallHook>,
    knowledge_request: Option<BeforeKnowledgeRequestDecision>,
    knowledge_request_hooks: Vec<BeforeKnowledgeRequestHook>,
    fail_before_tool_call: Option<String>,
    reject_before_tool_call: Option<String>,
    nonfatal_before_tool_call: Option<String>,
    nonfatal_failures: Vec<HookRuntimeFailure>,
    fail_before_model: Option<String>,
    nonfatal_before_model: Option<String>,
    active_hooks: Vec<HarnessHookId>,
    before_model_hooks: Vec<BeforeModelRequestHook>,
    before_model_calls: usize,
}

impl HookRuntime for TestHookRuntime {
    fn has_hook(&self, hook: HarnessHookId) -> bool {
        self.active_hooks.contains(&hook)
    }

    fn before_tool_selection(
        &mut self,
        hook: BeforeToolSelectionHook,
    ) -> std::result::Result<BeforeToolSelectionDecision, HookRuntimeFailure> {
        self.tool_selection_hooks.push(hook);
        if let Some(message) = &self.nonfatal_before_tool_selection {
            self.nonfatal_failures.push(HookRuntimeFailure::new(
                crate::harness_config::HarnessHookId::BeforeToolSelection,
                message.clone(),
            ));
        }
        Ok(self.tool_selection.clone().unwrap_or_default())
    }

    fn before_model_request(
        &mut self,
        hook: BeforeModelRequestHook,
    ) -> std::result::Result<BeforeModelRequestDecision, HookRuntimeFailure> {
        self.before_model_calls += 1;
        self.before_model_hooks.push(hook);
        if let Some(message) = &self.nonfatal_before_model {
            self.nonfatal_failures.push(HookRuntimeFailure::new(
                crate::harness_config::HarnessHookId::BeforeModelRequest,
                message.clone(),
            ));
        }
        if let Some(message) = &self.fail_before_model {
            return Err(HookRuntimeFailure::new(
                crate::harness_config::HarnessHookId::BeforeModelRequest,
                message.clone(),
            ));
        }
        Ok(self.model_request.clone().unwrap_or_default())
    }

    fn before_tool_call(
        &mut self,
        hook: BeforeToolCallHook,
    ) -> std::result::Result<BeforeToolCallDecision, HookRuntimeFailure> {
        self.tool_call_hooks.push(hook);
        if let Some(message) = &self.nonfatal_before_tool_call {
            self.nonfatal_failures.push(HookRuntimeFailure::new(
                crate::harness_config::HarnessHookId::BeforeToolCall,
                message.clone(),
            ));
        }
        if let Some(message) = &self.reject_before_tool_call {
            return Err(HookRuntimeFailure::rejection(
                crate::harness_config::HarnessHookId::BeforeToolCall,
                message.clone(),
            ));
        }
        if let Some(message) = &self.fail_before_tool_call {
            return Err(HookRuntimeFailure::new(
                crate::harness_config::HarnessHookId::BeforeToolCall,
                message.clone(),
            ));
        }
        Ok(self.tool_call.clone().unwrap_or_default())
    }

    fn before_knowledge_request(
        &mut self,
        hook: BeforeKnowledgeRequestHook,
    ) -> std::result::Result<BeforeKnowledgeRequestDecision, HookRuntimeFailure> {
        self.knowledge_request_hooks.push(hook);
        Ok(self.knowledge_request.clone().unwrap_or_default())
    }

    fn drain_nonfatal_failures(&mut self) -> Vec<HookRuntimeFailure> {
        std::mem::take(&mut self.nonfatal_failures)
    }
}

struct UsageKnowledgeRuntime {
    result: ActionDispatchResult,
}

impl KnowledgeRuntime for UsageKnowledgeRuntime {
    fn dispatch(&mut self, _action: &SemanticAction) -> ActionDispatchResult {
        self.result.clone()
    }
}

struct RecordingKnowledgeRuntime {
    result: ActionDispatchResult,
    dispatched: Vec<SemanticAction>,
}

impl RecordingKnowledgeRuntime {
    fn new(result: ActionDispatchResult) -> Self {
        Self {
            result,
            dispatched: Vec::new(),
        }
    }
}

impl KnowledgeRuntime for RecordingKnowledgeRuntime {
    fn dispatch(&mut self, action: &SemanticAction) -> ActionDispatchResult {
        self.dispatched.push(action.clone());
        self.result.clone()
    }
}

fn runtime_with_two_tools_and_skill() -> RuntimeSnapshot {
    let mut runtime = runtime_with_tool_and_skill();
    runtime.tools.push(ToolRuntimeSnapshot {
        name: "@zack/comment".into(),
        version: "0.1.0".into(),
        description: "Draft a comment.".into(),
        root: None,
        input_schema: json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "body": { "type": "string" }
            },
            "required": ["body"]
        }),
        state: "available".into(),
        source: "agent_binding".into(),
    });
    runtime.capability_candidates.insert(
        1,
        RuntimeCapabilitySnapshot {
            kind: "tool".into(),
            identity: "@zack/comment".into(),
            scope: "global".into(),
            source: "agent_binding".into(),
            state: "available".into(),
        },
    );
    runtime
}

#[test]
fn before_tool_selection_hook_subsets_model_visible_tool_catalog() {
    let mut session = HarnessSession::with_runtime_snapshot(runtime_with_two_tools_and_skill());
    let memory = InMemoryEventSink::default();
    let handle = memory.clone();
    session.emitter.add_sink(Box::new(memory));
    let mut model = ScriptedModelRuntime::new([completion("done", "handoff")]);
    let mut dispatcher = ScriptedActionDispatcher::default();
    let mut approvals = ScriptedApprovalController::default();
    let mut hooks = TestHookRuntime {
        tool_selection: Some(BeforeToolSelectionDecision {
            candidate_ids: Some(vec!["@zack/comment".into()]),
        }),
        active_hooks: vec![HarnessHookId::BeforeToolSelection],
        ..TestHookRuntime::default()
    };
    let mut engine = HarnessEngine::new(base_loop(), HarnessEngineOptions::new(limits()));
    let result = {
        let mut knowledge = NoopKnowledgeRuntime;
        let mut services = HarnessRuntimeServices {
            model: &mut model,
            dispatcher: &mut dispatcher,
            knowledge: &mut knowledge,
            approvals: &mut approvals,
            hooks: &mut hooks,
            service_events: None,
        };
        engine
            .execute_run_with_id(
                &mut session,
                "run-hooks-selection".into(),
                "input",
                &mut services,
            )
            .unwrap()
    };

    assert!(matches!(result, HarnessRunResult::Terminal(_)));
    let prompt = model.requests[0].prompt.render_text();
    assert!(prompt.contains("@zack/comment"));
    assert!(!prompt.contains("@zack/search"));
    let hook_input = hooks.tool_selection_hooks.first().expect("hook input");
    assert_eq!(hook_input.phase.phase_id, "assess");
    assert_eq!(
        hook_input
            .candidates
            .iter()
            .map(|candidate| candidate.canonical_id.as_str())
            .collect::<Vec<_>>(),
        vec!["@zack/search", "@zack/comment"]
    );
    let event_types: Vec<_> = handle
        .events()
        .iter()
        .map(|event| event.event_type)
        .collect();
    assert!(event_types.contains(&HarnessEventType::HookStarted));
    assert!(event_types.contains(&HarnessEventType::HookCompleted));
    let phase_enter_position = event_types
        .iter()
        .position(|event_type| *event_type == HarnessEventType::PhaseEnterRequested)
        .expect("phase enter event");
    let hook_started_position = event_types
        .iter()
        .position(|event_type| *event_type == HarnessEventType::HookStarted)
        .expect("hook started event");
    let hook_completed_position = event_types
        .iter()
        .position(|event_type| *event_type == HarnessEventType::HookCompleted)
        .expect("hook completed event");
    let effective_phase_position = event_types
        .iter()
        .position(|event_type| *event_type == HarnessEventType::EffectivePhaseComputed)
        .expect("effective phase event");
    assert!(phase_enter_position < hook_started_position);
    assert!(hook_completed_position < effective_phase_position);
    let fields = hook_event_fields_for(
        &handle.events(),
        HarnessEventType::HookCompleted,
        "before_tool_selection",
    );
    assert_eq!(fields["binding_count"], json!(1));
    assert_eq!(fields["candidate_count_before"], json!(2));
    assert_eq!(fields["candidate_count_after"], json!(1));
    assert_eq!(fields["candidate_ids_after"], json!(["@zack/comment"]));
    assert_eq!(fields["patched"], json!(true));
}

#[test]
fn before_tool_selection_invalid_patch_still_reports_queued_nonfatal_failures() {
    let mut session = HarnessSession::with_runtime_snapshot(runtime_with_two_tools_and_skill());
    let memory = InMemoryEventSink::default();
    let handle = memory.clone();
    session.emitter.add_sink(Box::new(memory));
    let mut model = ScriptedModelRuntime::new([completion("done", "handoff")]);
    let mut dispatcher = ScriptedActionDispatcher::default();
    let mut approvals = ScriptedApprovalController::default();
    let mut hooks = TestHookRuntime {
        tool_selection: Some(BeforeToolSelectionDecision {
            candidate_ids: Some(vec!["@zack/introduced".into()]),
        }),
        nonfatal_before_tool_selection: Some("advisory hook failed".into()),
        active_hooks: vec![HarnessHookId::BeforeToolSelection],
        ..TestHookRuntime::default()
    };
    let mut engine = HarnessEngine::new(base_loop(), HarnessEngineOptions::new(limits()));
    let result = {
        let mut knowledge = NoopKnowledgeRuntime;
        let mut services = HarnessRuntimeServices {
            model: &mut model,
            dispatcher: &mut dispatcher,
            knowledge: &mut knowledge,
            approvals: &mut approvals,
            hooks: &mut hooks,
            service_events: None,
        };
        engine
            .execute_run_with_id(
                &mut session,
                "run-hooks-selection-invalid-patch".into(),
                "input",
                &mut services,
            )
            .unwrap()
    };

    let HarnessRunResult::Terminal(result) = result else {
        panic!("expected terminal result");
    };
    assert_eq!(result.status, HarnessTerminalStatus::Failed);
    assert!(model.requests.is_empty());
    let events = handle.events();
    let event_types = events
        .iter()
        .map(|event| event.event_type)
        .collect::<Vec<_>>();
    let first_hook_failed_position = event_types
        .iter()
        .position(|event_type| *event_type == HarnessEventType::HookFailed)
        .expect("nonfatal hook failed event");
    let phase_failed_position = event_types
        .iter()
        .position(|event_type| *event_type == HarnessEventType::PhaseFailed)
        .expect("phase failed event");
    assert!(first_hook_failed_position < phase_failed_position);
    let hook_failed_count = event_types
        .iter()
        .filter(|event_type| **event_type == HarnessEventType::HookFailed)
        .count();
    assert_eq!(hook_failed_count, 2);
    assert_eq!(
        hook_event_fields_for(
            &events,
            HarnessEventType::HookFailed,
            "before_tool_selection"
        )["nonfatal"],
        json!(true)
    );
}

#[test]
fn before_tool_call_hook_patches_arguments_and_revalidates() {
    let mut session = session_with_tool_and_skill();
    let memory = InMemoryEventSink::default();
    let handle = memory.clone();
    session.emitter.add_sink(Box::new(memory));
    let mut model =
        ScriptedModelRuntime::new([tool_turn("@zack/search"), completion("done", "handoff")]);
    let mut dispatcher = ScriptedActionDispatcher::default();
    let mut approvals = ScriptedApprovalController::default();
    let mut hooks = TestHookRuntime {
        tool_call: Some(BeforeToolCallDecision {
            arguments: Some(json!({ "query": "patched" })),
        }),
        active_hooks: vec![HarnessHookId::BeforeToolCall],
        ..TestHookRuntime::default()
    };
    let mut engine = HarnessEngine::new(base_loop(), HarnessEngineOptions::new(limits()));
    let result = {
        let mut knowledge = NoopKnowledgeRuntime;
        let mut services = HarnessRuntimeServices {
            model: &mut model,
            dispatcher: &mut dispatcher,
            knowledge: &mut knowledge,
            approvals: &mut approvals,
            hooks: &mut hooks,
            service_events: None,
        };
        engine
            .execute_run_with_id(
                &mut session,
                "run-hooks-tool-call".into(),
                "input",
                &mut services,
            )
            .unwrap()
    };

    assert!(matches!(result, HarnessRunResult::Terminal(_)));
    assert_eq!(
        dispatcher.dispatched[0],
        SemanticAction::AgentPmTool {
            tool: "@zack/search".into(),
            arguments: json!({ "query": "patched" }),
        }
    );
    let hook_input = hooks.tool_call_hooks.first().expect("hook input");
    assert_eq!(hook_input.phase_id, "assess");
    assert_eq!(hook_input.tool, "@zack/search");
    assert_eq!(hook_input.arguments, json!({ "query": "x" }));
    let event_types: Vec<_> = handle
        .events()
        .iter()
        .map(|event| event.event_type)
        .collect();
    assert!(event_types.contains(&HarnessEventType::HookStarted));
    assert!(event_types.contains(&HarnessEventType::HookCompleted));
    let fields = hook_event_fields_for(
        &handle.events(),
        HarnessEventType::HookCompleted,
        "before_tool_call",
    );
    assert_eq!(fields["binding_count"], json!(1));
    assert_eq!(fields["tool"], json!("@zack/search"));
    assert_eq!(fields["argument_keys_before"], json!(["query"]));
    assert_eq!(fields["argument_keys_after"], json!(["query"]));
    assert_eq!(fields["arguments_patched"], json!(true));
}

#[test]
fn before_tool_call_continue_failure_is_reported_before_completed() {
    let mut session = session_with_tool_and_skill();
    let memory = InMemoryEventSink::default();
    let handle = memory.clone();
    session.emitter.add_sink(Box::new(memory));
    let mut model =
        ScriptedModelRuntime::new([tool_turn("@zack/search"), completion("done", "handoff")]);
    let mut dispatcher = ScriptedActionDispatcher::default();
    let mut approvals = ScriptedApprovalController::default();
    let mut hooks = TestHookRuntime {
        nonfatal_before_tool_call: Some("invalid hook patch".into()),
        active_hooks: vec![HarnessHookId::BeforeToolCall],
        ..TestHookRuntime::default()
    };
    let mut engine = HarnessEngine::new(base_loop(), HarnessEngineOptions::new(limits()));
    let result = {
        let mut knowledge = NoopKnowledgeRuntime;
        let mut services = HarnessRuntimeServices {
            model: &mut model,
            dispatcher: &mut dispatcher,
            knowledge: &mut knowledge,
            approvals: &mut approvals,
            hooks: &mut hooks,
            service_events: None,
        };
        engine
            .execute_run_with_id(
                &mut session,
                "run-hooks-tool-call-continue-failure".into(),
                "input",
                &mut services,
            )
            .unwrap()
    };

    assert!(matches!(result, HarnessRunResult::Terminal(_)));
    assert_eq!(dispatcher.dispatched.len(), 1);
    let event_types = handle
        .events()
        .iter()
        .map(|event| event.event_type)
        .collect::<Vec<_>>();
    let hook_started_position = event_types
        .iter()
        .position(|event_type| *event_type == HarnessEventType::HookStarted)
        .expect("hook started event");
    let hook_failed_position = event_types
        .iter()
        .position(|event_type| *event_type == HarnessEventType::HookFailed)
        .expect("hook failed event");
    let hook_completed_position = event_types
        .iter()
        .position(|event_type| *event_type == HarnessEventType::HookCompleted)
        .expect("hook completed event");
    assert!(hook_started_position < hook_failed_position);
    assert!(hook_failed_position < hook_completed_position);
    let fields = hook_event_fields_for(
        &handle.events(),
        HarnessEventType::HookFailed,
        "before_tool_call",
    );
    assert_eq!(fields["nonfatal"], json!(true));
}

#[test]
fn before_tool_call_hook_revalidates_patched_arguments_before_dispatch() {
    let mut session = session_with_tool_and_skill();
    let memory = InMemoryEventSink::default();
    let handle = memory.clone();
    session.emitter.add_sink(Box::new(memory));
    let mut model =
        ScriptedModelRuntime::new([tool_turn("@zack/search"), completion("done", "handoff")]);
    let mut dispatcher = ScriptedActionDispatcher::default();
    let mut approvals = ScriptedApprovalController::default();
    let mut hooks = TestHookRuntime {
        tool_call: Some(BeforeToolCallDecision {
            arguments: Some(json!({})),
        }),
        nonfatal_before_tool_call: Some("advisory hook failed".into()),
        active_hooks: vec![HarnessHookId::BeforeToolCall],
        ..TestHookRuntime::default()
    };
    let mut engine = HarnessEngine::new(base_loop(), HarnessEngineOptions::new(limits()));
    let result = {
        let mut knowledge = NoopKnowledgeRuntime;
        let mut services = HarnessRuntimeServices {
            model: &mut model,
            dispatcher: &mut dispatcher,
            knowledge: &mut knowledge,
            approvals: &mut approvals,
            hooks: &mut hooks,
            service_events: None,
        };
        engine
            .execute_run_with_id(
                &mut session,
                "run-hooks-tool-call-invalid-args".into(),
                "input",
                &mut services,
            )
            .unwrap()
    };

    let HarnessRunResult::Terminal(result) = result else {
        panic!("expected terminal result");
    };
    assert_eq!(result.status, HarnessTerminalStatus::Failed);
    assert!(dispatcher.dispatched.is_empty());
    let events = handle.events();
    let event_types: Vec<_> = events.iter().map(|event| event.event_type).collect();
    assert!(event_types.contains(&HarnessEventType::HookFailed));
    let hook_failed_count = event_types
        .iter()
        .filter(|event_type| **event_type == HarnessEventType::HookFailed)
        .count();
    assert_eq!(hook_failed_count, 2);
    let hook_failed_position = event_types
        .iter()
        .position(|event_type| *event_type == HarnessEventType::HookFailed)
        .expect("nonfatal hook failed event");
    let phase_failed_position = event_types
        .iter()
        .position(|event_type| *event_type == HarnessEventType::PhaseFailed)
        .expect("phase failed event");
    assert!(hook_failed_position < phase_failed_position);
    assert_eq!(
        hook_event_fields_for(&events, HarnessEventType::HookFailed, "before_tool_call")["nonfatal"],
        json!(true)
    );
    assert!(!event_types.contains(&HarnessEventType::ToolInvoked));
}

#[test]
fn before_tool_call_hook_rejection_blocks_dispatch_and_emits_rejected() {
    let mut session = session_with_tool_and_skill();
    let memory = InMemoryEventSink::default();
    let handle = memory.clone();
    session.emitter.add_sink(Box::new(memory));
    let mut model = ScriptedModelRuntime::new([tool_turn("@zack/search")]);
    let mut dispatcher = ScriptedActionDispatcher::default();
    let mut approvals = ScriptedApprovalController::default();
    let mut hooks = TestHookRuntime {
        reject_before_tool_call: Some("blocked by policy".into()),
        active_hooks: vec![HarnessHookId::BeforeToolCall],
        ..TestHookRuntime::default()
    };
    let mut engine = HarnessEngine::new(base_loop(), HarnessEngineOptions::new(limits()));
    let result = {
        let mut knowledge = NoopKnowledgeRuntime;
        let mut services = HarnessRuntimeServices {
            model: &mut model,
            dispatcher: &mut dispatcher,
            knowledge: &mut knowledge,
            approvals: &mut approvals,
            hooks: &mut hooks,
            service_events: None,
        };
        engine
            .execute_run_with_id(
                &mut session,
                "run-hooks-tool-call-reject".into(),
                "input",
                &mut services,
            )
            .unwrap()
    };

    let HarnessRunResult::Terminal(result) = result else {
        panic!("expected terminal result");
    };
    assert_eq!(result.status, HarnessTerminalStatus::Failed);
    assert!(dispatcher.dispatched.is_empty());
    let event_types: Vec<_> = handle
        .events()
        .iter()
        .map(|event| event.event_type)
        .collect();
    assert!(event_types.contains(&HarnessEventType::HookRejected));
    assert!(!event_types.contains(&HarnessEventType::ToolInvoked));
}

#[test]
fn before_tool_call_rejection_still_reports_queued_nonfatal_failures() {
    let mut session = session_with_tool_and_skill();
    let memory = InMemoryEventSink::default();
    let handle = memory.clone();
    session.emitter.add_sink(Box::new(memory));
    let mut model = ScriptedModelRuntime::new([tool_turn("@zack/search")]);
    let mut dispatcher = ScriptedActionDispatcher::default();
    let mut approvals = ScriptedApprovalController::default();
    let mut hooks = TestHookRuntime {
        nonfatal_before_tool_call: Some("advisory hook failed".into()),
        reject_before_tool_call: Some("blocked by policy".into()),
        active_hooks: vec![HarnessHookId::BeforeToolCall],
        ..TestHookRuntime::default()
    };
    let mut engine = HarnessEngine::new(base_loop(), HarnessEngineOptions::new(limits()));
    let result = {
        let mut knowledge = NoopKnowledgeRuntime;
        let mut services = HarnessRuntimeServices {
            model: &mut model,
            dispatcher: &mut dispatcher,
            knowledge: &mut knowledge,
            approvals: &mut approvals,
            hooks: &mut hooks,
            service_events: None,
        };
        engine
            .execute_run_with_id(
                &mut session,
                "run-hooks-tool-call-nonfatal-then-reject".into(),
                "input",
                &mut services,
            )
            .unwrap()
    };

    let HarnessRunResult::Terminal(result) = result else {
        panic!("expected terminal result");
    };
    assert_eq!(result.status, HarnessTerminalStatus::Failed);
    assert!(dispatcher.dispatched.is_empty());
    let events = handle.events();
    let event_types = events
        .iter()
        .map(|event| event.event_type)
        .collect::<Vec<_>>();
    let hook_failed_position = event_types
        .iter()
        .position(|event_type| *event_type == HarnessEventType::HookFailed)
        .expect("hook failed event");
    let hook_rejected_position = event_types
        .iter()
        .position(|event_type| *event_type == HarnessEventType::HookRejected)
        .expect("hook rejected event");
    assert!(hook_failed_position < hook_rejected_position);
    assert_eq!(
        hook_event_fields_for(&events, HarnessEventType::HookFailed, "before_tool_call")["nonfatal"],
        json!(true)
    );
    assert!(!event_types.contains(&HarnessEventType::ToolInvoked));
}

#[test]
fn before_tool_call_hook_failure_emits_failed_not_rejected() {
    let mut session = session_with_tool_and_skill();
    let memory = InMemoryEventSink::default();
    let handle = memory.clone();
    session.emitter.add_sink(Box::new(memory));
    let mut model = ScriptedModelRuntime::new([tool_turn("@zack/search")]);
    let mut dispatcher = ScriptedActionDispatcher::default();
    let mut approvals = ScriptedApprovalController::default();
    let mut hooks = TestHookRuntime {
        fail_before_tool_call: Some("transport timeout".into()),
        active_hooks: vec![HarnessHookId::BeforeToolCall],
        ..TestHookRuntime::default()
    };
    let mut engine = HarnessEngine::new(base_loop(), HarnessEngineOptions::new(limits()));
    let result = {
        let mut knowledge = NoopKnowledgeRuntime;
        let mut services = HarnessRuntimeServices {
            model: &mut model,
            dispatcher: &mut dispatcher,
            knowledge: &mut knowledge,
            approvals: &mut approvals,
            hooks: &mut hooks,
            service_events: None,
        };
        engine
            .execute_run_with_id(
                &mut session,
                "run-hooks-tool-call-failure".into(),
                "input",
                &mut services,
            )
            .unwrap()
    };

    let HarnessRunResult::Terminal(result) = result else {
        panic!("expected terminal result");
    };
    assert_eq!(result.status, HarnessTerminalStatus::Failed);
    assert!(dispatcher.dispatched.is_empty());
    let event_types: Vec<_> = handle
        .events()
        .iter()
        .map(|event| event.event_type)
        .collect();
    assert!(event_types.contains(&HarnessEventType::HookFailed));
    assert!(!event_types.contains(&HarnessEventType::HookRejected));
    assert!(!event_types.contains(&HarnessEventType::ToolInvoked));
}

#[test]
fn before_knowledge_request_hook_shapes_request_before_dispatch() {
    let mut session =
        HarnessSession::with_runtime_snapshot(runtime_with_knowledge_packages(&["@zack/guide"]));
    let memory = InMemoryEventSink::default();
    let handle = memory.clone();
    session.emitter.add_sink(Box::new(memory));
    let mut model = ScriptedModelRuntime::new(vec![
        knowledge_query_turn("@zack/guide"),
        completion("assess-complete", "execute"),
        completion("execute-complete", "review"),
        completion("review-complete", "ready"),
    ]);
    let mut dispatcher = ScriptedActionDispatcher::default();
    let mut approvals = ScriptedApprovalController::default();
    let mut hooks = TestHookRuntime {
        knowledge_request: Some(BeforeKnowledgeRequestDecision {
            document: None,
            query: Some("patched query".into()),
            top_k: Some(3),
            score_threshold: Some(0.42),
            return_citations: Some(false),
        }),
        active_hooks: vec![HarnessHookId::BeforeKnowledgeRequest],
        ..TestHookRuntime::default()
    };
    let mut knowledge = RecordingKnowledgeRuntime::new(ActionDispatchResult::success(json!({
        "ok": true,
        "package": "@zack/guide",
        "version": "0.1.0",
        "mode": "vector_query",
        "query": "patched query",
        "results": [],
        "citations": []
    })));
    let mut engine = HarnessEngine::new(base_loop(), HarnessEngineOptions::new(limits()));
    let result = {
        let mut services = HarnessRuntimeServices {
            model: &mut model,
            dispatcher: &mut dispatcher,
            knowledge: &mut knowledge,
            approvals: &mut approvals,
            hooks: &mut hooks,
            service_events: None,
        };
        engine
            .execute_run_with_id(
                &mut session,
                "run-hooks-knowledge-request".into(),
                "input",
                &mut services,
            )
            .unwrap()
    };

    assert!(matches!(result, HarnessRunResult::Terminal(_)));
    assert_eq!(knowledge.dispatched.len(), 1);
    assert_eq!(
        knowledge.dispatched[0],
        SemanticAction::KnowledgeRequest {
            package: "@zack/guide".into(),
            mode: None,
            document: None,
            query: Some("patched query".into()),
            top_k: Some(3),
            score_threshold: Some(0.42),
            return_citations: Some(false),
        }
    );
    let hook_input = hooks.knowledge_request_hooks.first().expect("hook input");
    assert_eq!(hook_input.phase_id, "assess");
    assert_eq!(hook_input.request.package, "@zack/guide");
    assert_eq!(hook_input.request.query.as_deref(), Some("alpha"));
    assert_eq!(hook_input.request.top_k, Some(1));
    let fields = hook_event_fields_for(
        &handle.events(),
        HarnessEventType::HookCompleted,
        "before_knowledge_request",
    );
    assert_eq!(fields["binding_count"], json!(1));
    assert_eq!(fields["query"], json!("patched query"));
    assert_eq!(fields["top_k"], json!(3));
    assert_eq!(fields["score_threshold"], json!(0.42));
    assert_eq!(fields["return_citations"], json!(false));
    assert_eq!(fields["patched"], json!(true));
}

#[test]
fn before_model_request_hook_appends_context_and_merges_provider_options() {
    let mut runtime = runtime_with_tool_and_skill();
    runtime.model = Some(ModelProviderSelection {
        provider: "test-provider".into(),
        model: "test-model".into(),
        options: json!({ "temperature": 0.1, "existing": true }),
    });
    let mut session = HarnessSession::with_runtime_snapshot(runtime);
    let memory = InMemoryEventSink::default();
    let handle = memory.clone();
    session.emitter.add_sink(Box::new(memory));
    let mut model = ScriptedModelRuntime::new([completion("done", "handoff")]);
    let mut dispatcher = ScriptedActionDispatcher::default();
    let mut approvals = ScriptedApprovalController::default();
    let mut provider_options = serde_json::Map::new();
    provider_options.insert("temperature".into(), json!(0.2));
    provider_options.insert("metadata".into(), json!({ "hook": true }));
    let mut hooks = TestHookRuntime {
        model_request: Some(BeforeModelRequestDecision {
            context_sections: vec![BeforeModelRequestContextSection {
                title: "Policy Note".into(),
                content: "Prefer the safest concise answer.".into(),
            }],
            provider_options,
        }),
        active_hooks: vec![HarnessHookId::BeforeModelRequest],
        ..TestHookRuntime::default()
    };
    let mut engine = HarnessEngine::new(base_loop(), HarnessEngineOptions::new(limits()));
    let result = {
        let mut knowledge = NoopKnowledgeRuntime;
        let mut services = HarnessRuntimeServices {
            model: &mut model,
            dispatcher: &mut dispatcher,
            knowledge: &mut knowledge,
            approvals: &mut approvals,
            hooks: &mut hooks,
            service_events: None,
        };
        engine
            .execute_run_with_id(
                &mut session,
                "run-hooks-model-patch".into(),
                "input",
                &mut services,
            )
            .unwrap()
    };

    assert!(matches!(result, HarnessRunResult::Terminal(_)));
    let request = &model.requests[0];
    assert!(request.prompt.render_text().contains("Hook Context:"));
    assert!(
        request
            .prompt
            .render_text()
            .contains("Prefer the safest concise answer.")
    );
    let model = request.model.as_ref().expect("selected model");
    assert_eq!(model.provider, "test-provider");
    assert_eq!(model.model, "test-model");
    assert_eq!(model.options["temperature"], json!(0.2));
    assert_eq!(model.options["existing"], json!(true));
    assert_eq!(
        request.runtime.model.as_ref().unwrap().options,
        model.options
    );

    let hook_input = hooks.before_model_hooks.first().expect("hook input");
    assert_eq!(hook_input.phase.phase_id, "assess");
    assert_eq!(
        hook_input.phase.completion.explicit_outcomes,
        vec!["execute", "handoff"]
    );
    assert!(
        hook_input
            .sections
            .iter()
            .any(|section| section.title == CONSUMER_RUN_CONTEXT_SECTION_TITLE && section.mutable)
    );
    assert!(
        hook_input
            .sections
            .iter()
            .any(|section| section.title == "HARNESS CONTROL" && !section.mutable)
    );
    let fields = hook_event_fields_for(
        &handle.events(),
        HarnessEventType::HookCompleted,
        "before_model_request",
    );
    assert_eq!(fields["binding_count"], json!(1));
    assert_eq!(fields["model_provider"], json!("test-provider"));
    assert_eq!(fields["model_id"], json!("test-model"));
    assert_eq!(fields["context_sections_added"], json!(1));
    assert_eq!(
        fields["provider_option_patch_keys"],
        json!(["metadata", "temperature"])
    );
    assert_eq!(fields["patched"], json!(true));
}

#[test]
fn before_model_request_hook_fails_closed_before_model_runtime() {
    let mut session = session_with_tool_and_skill();
    let memory = InMemoryEventSink::default();
    let handle = memory.clone();
    session.emitter.add_sink(Box::new(memory));
    let mut model = ScriptedModelRuntime::new([completion("done", "handoff")]);
    let mut dispatcher = ScriptedActionDispatcher::default();
    let mut approvals = ScriptedApprovalController::default();
    let mut hooks = TestHookRuntime {
        fail_before_model: Some("blocked by policy".into()),
        active_hooks: vec![HarnessHookId::BeforeModelRequest],
        ..TestHookRuntime::default()
    };
    let mut engine = HarnessEngine::new(base_loop(), HarnessEngineOptions::new(limits()));
    let result = {
        let mut knowledge = NoopKnowledgeRuntime;
        let mut services = HarnessRuntimeServices {
            model: &mut model,
            dispatcher: &mut dispatcher,
            knowledge: &mut knowledge,
            approvals: &mut approvals,
            hooks: &mut hooks,
            service_events: None,
        };
        engine
            .execute_run_with_id(
                &mut session,
                "run-hooks-model".into(),
                "input",
                &mut services,
            )
            .unwrap()
    };

    let HarnessRunResult::Terminal(result) = result else {
        panic!("expected terminal result");
    };
    assert_eq!(result.status, HarnessTerminalStatus::Failed);
    assert_eq!(hooks.before_model_calls, 1);
    assert!(model.requests.is_empty());
    let event_types: Vec<_> = handle
        .events()
        .iter()
        .map(|event| event.event_type)
        .collect();
    assert!(event_types.contains(&HarnessEventType::HookStarted));
    assert!(event_types.contains(&HarnessEventType::HookFailed));
}

#[test]
fn before_model_request_invalid_patch_still_reports_queued_nonfatal_failures() {
    let mut session = session_with_tool_and_skill();
    let memory = InMemoryEventSink::default();
    let handle = memory.clone();
    session.emitter.add_sink(Box::new(memory));
    let mut model = ScriptedModelRuntime::new([completion("done", "handoff")]);
    let mut dispatcher = ScriptedActionDispatcher::default();
    let mut approvals = ScriptedApprovalController::default();
    let mut provider_options = serde_json::Map::new();
    provider_options.insert("temperature".into(), json!(0.2));
    let mut hooks = TestHookRuntime {
        model_request: Some(BeforeModelRequestDecision {
            context_sections: Vec::new(),
            provider_options,
        }),
        nonfatal_before_model: Some("advisory model hook failed".into()),
        active_hooks: vec![HarnessHookId::BeforeModelRequest],
        ..TestHookRuntime::default()
    };
    let mut engine = HarnessEngine::new(base_loop(), HarnessEngineOptions::new(limits()));
    let result = {
        let mut knowledge = NoopKnowledgeRuntime;
        let mut services = HarnessRuntimeServices {
            model: &mut model,
            dispatcher: &mut dispatcher,
            knowledge: &mut knowledge,
            approvals: &mut approvals,
            hooks: &mut hooks,
            service_events: None,
        };
        engine
            .execute_run_with_id(
                &mut session,
                "run-hooks-model-invalid-patch".into(),
                "input",
                &mut services,
            )
            .unwrap()
    };

    let HarnessRunResult::Terminal(result) = result else {
        panic!("expected terminal result");
    };
    assert_eq!(result.status, HarnessTerminalStatus::Failed);
    assert!(model.requests.is_empty());
    let events = handle.events();
    let event_types = events
        .iter()
        .map(|event| event.event_type)
        .collect::<Vec<_>>();
    let hook_failed_position = event_types
        .iter()
        .position(|event_type| *event_type == HarnessEventType::HookFailed)
        .expect("nonfatal hook failed event");
    let phase_failed_position = event_types
        .iter()
        .position(|event_type| *event_type == HarnessEventType::PhaseFailed)
        .expect("phase failed event");
    assert!(hook_failed_position < phase_failed_position);
    assert_eq!(
        hook_event_fields_for(
            &events,
            HarnessEventType::HookFailed,
            "before_model_request"
        )["nonfatal"],
        json!(true)
    );
}

struct MutatingModelRuntime {
    turns: VecDeque<ModelTurn>,
    requests: Vec<ModelRequest>,
    mutate_path: PathBuf,
    replacement: String,
}

impl MutatingModelRuntime {
    fn new(turns: Vec<ModelTurn>, mutate_path: PathBuf, replacement: &str) -> Self {
        Self {
            turns: turns.into(),
            requests: Vec::new(),
            mutate_path,
            replacement: replacement.into(),
        }
    }
}

impl ModelRuntime for MutatingModelRuntime {
    fn generate(
        &mut self,
        request: ModelRequest,
    ) -> std::result::Result<ModelTurn, ModelRuntimeFailure> {
        self.requests.push(request);
        if self.requests.len() == 1 {
            std::fs::write(&self.mutate_path, &self.replacement).unwrap();
        }
        self.turns
            .pop_front()
            .ok_or_else(|| ModelRuntimeFailure::new("no scripted turn"))
    }
}

fn tool_turn(tool: &str) -> ModelTurn {
    ModelTurn {
        assistant_content: None,
        actions: vec![SemanticActionProposal::new(
            "tool",
            SemanticAction::AgentPmTool {
                tool: tool.into(),
                arguments: json!({ "query": "x" }),
            },
        )],
        usage: RunUsage::default(),
        finish_reason: None,
        provider_metadata: BTreeMap::new(),
    }
}

fn tool_turn_with_arguments(tool: &str, arguments: Value) -> ModelTurn {
    ModelTurn {
        assistant_content: None,
        actions: vec![SemanticActionProposal::new(
            "tool",
            SemanticAction::AgentPmTool {
                tool: tool.into(),
                arguments,
            },
        )],
        usage: RunUsage::default(),
        finish_reason: None,
        provider_metadata: BTreeMap::new(),
    }
}

fn knowledge_query_turn(package: &str) -> ModelTurn {
    ModelTurn {
        assistant_content: None,
        actions: vec![SemanticActionProposal::new(
            "knowledge",
            SemanticAction::KnowledgeRequest {
                package: package.into(),
                mode: None,
                document: None,
                query: Some("alpha".into()),
                top_k: Some(1),
                score_threshold: None,
                return_citations: Some(true),
            },
        )],
        usage: RunUsage::default(),
        finish_reason: None,
        provider_metadata: BTreeMap::new(),
    }
}

fn skill_read_turn(skill: &str, resource: &str) -> ModelTurn {
    ModelTurn {
        assistant_content: None,
        actions: vec![SemanticActionProposal::new(
            "skill",
            SemanticAction::SkillResourceRead {
                skill: skill.into(),
                resource: resource.into(),
            },
        )],
        usage: RunUsage::default(),
        finish_reason: None,
        provider_metadata: BTreeMap::new(),
    }
}

fn run_engine(
    loop_manifest: LoopManifest,
    turns: Vec<ModelTurn>,
) -> (HarnessRunResult, HarnessSession, ScriptedModelRuntime) {
    let mut engine = HarnessEngine::new(loop_manifest, HarnessEngineOptions::new(limits()));
    let mut session = HarnessSession::new();
    let memory = InMemoryEventSink::default();
    session.emitter.add_sink(Box::new(memory));
    let mut model = ScriptedModelRuntime::new(turns);
    let mut dispatcher = ScriptedActionDispatcher::default();
    let mut approvals = ScriptedApprovalController::default();
    let result = engine
        .execute_run(
            &mut session,
            "hello",
            &mut model,
            &mut dispatcher,
            &mut approvals,
        )
        .unwrap();
    (result, session, model)
}

fn run_tool_failure_policy(
    tool_failure: LoopToolFailurePolicy,
    dispatcher_results: Vec<ActionDispatchResult>,
) -> (RuntimeTerminalResult, ScriptedActionDispatcher) {
    let mut loop_manifest = base_loop();
    loop_manifest.r#loop.error_policy = Some(LoopErrorPolicy {
        tool_failure: Some(tool_failure),
        phase_failure: None,
    });
    let mut engine = HarnessEngine::new(loop_manifest, HarnessEngineOptions::new(limits()));
    let mut session = session_with_tool_and_skill();
    let mut model = ScriptedModelRuntime::new(vec![tool_turn("@zack/search")]);
    let mut dispatcher = ScriptedActionDispatcher::default();
    for result in dispatcher_results {
        dispatcher.push_result("@zack/search", result);
    }
    let mut approvals = ScriptedApprovalController::default();
    let result = engine
        .execute_run(
            &mut session,
            "hello",
            &mut model,
            &mut dispatcher,
            &mut approvals,
        )
        .unwrap();
    let HarnessRunResult::Terminal(result) = result else {
        panic!("expected terminal result");
    };
    (*result, dispatcher)
}

#[test]
fn executes_multi_phase_loop_and_accumulates_session_usage() {
    let (result, session, model) = run_engine(
        base_loop(),
        vec![
            completion("a", "execute"),
            completion("b", "review"),
            completion("c", "ready"),
        ],
    );
    let HarnessRunResult::Terminal(result) = result else {
        panic!("expected terminal result");
    };
    assert_eq!(result.status, HarnessTerminalStatus::Ended);
    assert_eq!(result.report.phase_summaries.len(), 3);
    assert_eq!(session.usage.started_runs, 1);
    assert_eq!(session.usage.completed_runs, 1);
    assert_eq!(model.requests.len(), 3);
    assert_eq!(model.requests[1].prior_phase_results.len(), 1);
    assert_eq!(model.requests[1].prior_phase_results[0].loop_step_number, 1);
    assert_eq!(model.requests[2].prior_phase_results[1].loop_step_number, 2);
}

#[test]
fn supports_cycles_and_phase_reentry() {
    let (result, _, _) = run_engine(
        base_loop(),
        vec![
            completion("a", "execute"),
            completion("b", "review"),
            completion("c", "again"),
            completion("d", "review"),
            completion("e", "ready"),
        ],
    );
    let HarnessRunResult::Terminal(result) = result else {
        panic!("expected terminal result");
    };
    assert_eq!(result.status, HarnessTerminalStatus::Ended);
    let phases: Vec<_> = result
        .report
        .phase_summaries
        .iter()
        .map(|phase| phase.phase_id.as_str())
        .collect();
    assert_eq!(
        phases,
        vec!["assess", "execute", "review", "execute", "review"]
    );
}

#[test]
fn rejects_starting_second_run_while_pending_approval_without_mutating_active_run() {
    let mut loop_manifest = base_loop();
    loop_manifest.r#loop.checkpoints = vec![LoopCheckpoint {
        id: "approve-assess".into(),
        r#type: "approval".into(),
        before_phase: "assess".into(),
        on_reject: "$handoff".into(),
    }];
    let mut engine = HarnessEngine::new(
        loop_manifest,
        HarnessEngineOptions {
            runtime_limits: limits(),
            retain_active_on_approval_required: true,
        },
    );
    let mut session = HarnessSession::new();
    let mut model = ScriptedModelRuntime::new(vec![completion("a", "execute")]);
    let mut dispatcher = ScriptedActionDispatcher::default();
    let mut approvals = ScriptedApprovalController::default();
    approvals.push("approve-assess", ApprovalDecision::Pending);
    let result = engine
        .execute_run(
            &mut session,
            "first",
            &mut model,
            &mut dispatcher,
            &mut approvals,
        )
        .unwrap();
    let HarnessRunResult::PendingApproval { run_id, .. } = result else {
        panic!("expected pending approval");
    };
    let err = session.start_run("second".into()).unwrap_err();
    assert!(err.to_string().contains(&run_id));
    assert_eq!(session.active_run().unwrap().run_id(), run_id);
    assert_eq!(session.usage.started_runs, 1);
}

#[test]
fn keeps_phase_transcripts_isolated() {
    let (_, _, model) = run_engine(
        base_loop(),
        vec![
            completion("a", "execute"),
            completion("b", "review"),
            completion("c", "ready"),
        ],
    );
    assert_eq!(model.requests[0].transcript.len(), 1);
    assert_eq!(model.requests[1].transcript.len(), 1);
    assert_eq!(model.requests[2].transcript.len(), 1);
    assert_eq!(model.requests[2].prior_phase_results.len(), 2);
}

#[test]
fn model_request_contains_canonical_prompt_sections_and_runtime_snapshot() {
    let mut snapshot = RuntimeSnapshot::empty("session-test".into());
    snapshot.workspace_root = PathBuf::from("/workspace");
    snapshot.state_dir = PathBuf::from("/workspace/.agentpm-state");
    snapshot.runtime_scopes = BTreeMap::from([("user".into(), "user-1".into())]);
    snapshot.consumer_context = Some(crate::harness_runtime::ConsumerContextSnapshot {
        state: "Available".into(),
        file: Some("ops-context.md".into()),
        path: Some(PathBuf::from("/workspace/ops-context.md")),
        content: None,
        byte_size: None,
        approximate_tokens: None,
        sha256: None,
    });
    let mut engine = HarnessEngine::new(base_loop(), HarnessEngineOptions::new(limits()));
    let mut session = HarnessSession::with_runtime_snapshot(snapshot);
    let mut model = ScriptedModelRuntime::new(vec![
        completion("a", "execute"),
        completion("b", "review"),
        completion("c", "ready"),
    ]);
    let mut dispatcher = ScriptedActionDispatcher::default();
    let mut approvals = ScriptedApprovalController::default();
    let result = engine
        .execute_run(
            &mut session,
            "hello",
            &mut model,
            &mut dispatcher,
            &mut approvals,
        )
        .unwrap();
    assert!(matches!(result, HarnessRunResult::Terminal(_)));
    let request = &model.requests[0];
    let titles: Vec<_> = request
        .prompt
        .sections
        .iter()
        .map(|section| section.title.as_str())
        .collect();
    assert_eq!(
        titles,
        vec![
            "HARNESS CONTROL",
            "AUTHORED PHASE + BEHAVIOR",
            "CONSUMER / RUN CONTEXT",
            "CROSS-PHASE STATE",
            "EFFECTIVE CAPABILITY CATALOG",
            "CURRENT PHASE-LOCAL TRANSCRIPT"
        ]
    );
    assert_eq!(request.runtime.runtime_scopes["user"], "user-1");
    assert_eq!(request.prompt.action_aliases.len(), 1);
    assert!(request.prompt.render_text().contains("Harness authority"));
}

#[test]
fn effective_phase_injects_profiles_global_then_phase_and_dedupes() {
    let mut snapshot = RuntimeSnapshot::empty("session-test".into());
    snapshot.profiles = vec![
        profile_snapshot("@zack/global-style", "0.1.0", "Global role"),
        profile_snapshot("@zack/phase-style", "0.2.0", "Phase role"),
    ];
    snapshot.profile_bindings = crate::harness_runtime::model::ProfileBindingSnapshot {
        global: vec!["@zack/global-style".into(), "@zack/phase-style".into()],
        phases: BTreeMap::from([(
            "assess".into(),
            vec![
                "@zack/phase-style".into(),
                "@zack/global-style".into(),
                "@zack/missing-style".into(),
            ],
        )]),
    };
    let mut engine = HarnessEngine::new(base_loop(), HarnessEngineOptions::new(limits()));
    let mut session = HarnessSession::with_runtime_snapshot(snapshot);
    let memory = InMemoryEventSink::default();
    let handle = memory.clone();
    session.emitter.add_sink(Box::new(memory));
    let mut model = ScriptedModelRuntime::new(vec![
        completion("a", "execute"),
        completion("b", "review"),
        completion("c", "ready"),
    ]);
    let mut dispatcher = ScriptedActionDispatcher::default();
    let mut approvals = ScriptedApprovalController::default();

    engine
        .execute_run(
            &mut session,
            "hello",
            &mut model,
            &mut dispatcher,
            &mut approvals,
        )
        .unwrap();
    let effective = &model.requests[0].effective_phase;
    assert_eq!(
        effective.authored_profile_candidates,
        vec![
            "@zack/global-style",
            "@zack/phase-style",
            "@zack/missing-style"
        ]
    );
    assert_eq!(effective.active_profiles.len(), 2);
    assert_eq!(effective.suppressed_capabilities.len(), 1);
    let prompt = model.requests[0].prompt.render_text();
    let global_index = prompt.find("Profile: @zack/global-style@0.1.0").unwrap();
    let phase_index = prompt.find("Profile: @zack/phase-style@0.2.0").unwrap();
    assert!(global_index < phase_index);
    assert_eq!(prompt.matches("Profile: @zack/global-style").count(), 1);
    assert!(prompt.contains("[required] cite-evidence"));
    assert!(prompt.contains("Preferred vocabulary"));
    let effective_event = handle
        .events()
        .into_iter()
        .find(|event| event.event_type == HarnessEventType::EffectivePhaseComputed)
        .unwrap();
    let HarnessEventPayload::Lifecycle { fields, .. } = effective_event.payload else {
        panic!("expected lifecycle payload");
    };
    assert_eq!(
        fields["suppressed_profiles"][0]["identity"],
        "@zack/missing-style"
    );
}

#[test]
fn consumer_context_is_snapshotted_per_run_and_reloaded_next_run() {
    let path = temp_context_file("reload", "first run context\n");
    let mut snapshot = RuntimeSnapshot::empty("session-test".into());
    snapshot.consumer_context = Some(crate::harness_runtime::ConsumerContextSnapshot {
        state: "Available".into(),
        file: Some("context.md".into()),
        path: Some(path.clone()),
        content: None,
        byte_size: None,
        approximate_tokens: None,
        sha256: None,
    });
    let mut engine = HarnessEngine::new(base_loop(), HarnessEngineOptions::new(limits()));
    let mut session = HarnessSession::with_runtime_snapshot(snapshot);
    let mut first_model = MutatingModelRuntime::new(
        vec![
            completion("a", "execute"),
            completion("b", "review"),
            completion("c", "ready"),
        ],
        path.clone(),
        "edited during first run\n",
    );
    let mut dispatcher = ScriptedActionDispatcher::default();
    let mut approvals = ScriptedApprovalController::default();
    let first_result = engine
        .execute_run(
            &mut session,
            "hello",
            &mut first_model,
            &mut dispatcher,
            &mut approvals,
        )
        .unwrap();
    let HarnessRunResult::Terminal(first_result) = first_result else {
        panic!("expected terminal result");
    };
    let report_context = first_result.report.consumer_context.as_ref().unwrap();
    assert_eq!(report_context.status, "loaded");
    assert!(report_context.byte_size.is_some());
    assert!(report_context.approximate_tokens.is_some());
    assert!(
        first_model
            .requests
            .iter()
            .all(|request| request.prompt.render_text().contains("first run context"))
    );
    assert!(first_model.requests.iter().all(|request| {
        !request
            .prompt
            .render_text()
            .contains("edited during first run")
    }));

    session.clear_terminal_active_run();
    let mut second_model = ScriptedModelRuntime::new(vec![
        completion("d", "execute"),
        completion("e", "review"),
        completion("f", "ready"),
    ]);
    engine
        .execute_run(
            &mut session,
            "hello",
            &mut second_model,
            &mut dispatcher,
            &mut approvals,
        )
        .unwrap();
    assert!(
        second_model.requests[0]
            .prompt
            .render_text()
            .contains("edited during first run")
    );
    let context = second_model.requests[0]
        .runtime
        .consumer_context
        .as_ref()
        .unwrap();
    assert!(context.byte_size.is_some());
    assert!(context.approximate_tokens.is_some());
    assert!(context.sha256.as_deref().unwrap().starts_with("sha256:"));
}

#[test]
fn missing_consumer_context_without_resolved_path_is_evented_and_non_fatal() {
    let mut snapshot = RuntimeSnapshot::empty("session-test".into());
    snapshot.consumer_context = Some(crate::harness_runtime::ConsumerContextSnapshot {
        state: "Unavailable".into(),
        file: Some("missing-context.md".into()),
        path: None,
        content: None,
        byte_size: None,
        approximate_tokens: None,
        sha256: None,
    });
    let mut engine = HarnessEngine::new(base_loop(), HarnessEngineOptions::new(limits()));
    let mut session = HarnessSession::with_runtime_snapshot(snapshot);
    let memory = InMemoryEventSink::default();
    let handle = memory.clone();
    session.emitter.add_sink(Box::new(memory));
    let mut model = ScriptedModelRuntime::new(vec![
        completion("a", "execute"),
        completion("b", "review"),
        completion("c", "ready"),
    ]);
    let mut dispatcher = ScriptedActionDispatcher::default();
    let mut approvals = ScriptedApprovalController::default();

    let result = engine
        .execute_run(
            &mut session,
            "hello",
            &mut model,
            &mut dispatcher,
            &mut approvals,
        )
        .unwrap();

    assert!(matches!(result, HarnessRunResult::Terminal(_)));
    assert!(
        model.requests[0]
            .prompt
            .diagnostics
            .iter()
            .any(|diagnostic| { diagnostic.contains("consumer context content is not loaded") })
    );
    assert_consumer_context_unavailable_event(&handle);
}

#[test]
fn consumer_context_deleted_before_run_start_is_evented_and_non_fatal() {
    let path = temp_context_file("deleted", "deleted before run\n");
    std::fs::remove_file(&path).unwrap();
    let mut snapshot = RuntimeSnapshot::empty("session-test".into());
    snapshot.consumer_context = Some(crate::harness_runtime::ConsumerContextSnapshot {
        state: "Available".into(),
        file: Some("context.md".into()),
        path: Some(path),
        content: None,
        byte_size: Some(19),
        approximate_tokens: Some(5),
        sha256: Some("sha256:preflight".into()),
    });
    let mut engine = HarnessEngine::new(base_loop(), HarnessEngineOptions::new(limits()));
    let mut session = HarnessSession::with_runtime_snapshot(snapshot);
    let memory = InMemoryEventSink::default();
    let handle = memory.clone();
    session.emitter.add_sink(Box::new(memory));
    let mut model = ScriptedModelRuntime::new(vec![
        completion("a", "execute"),
        completion("b", "review"),
        completion("c", "ready"),
    ]);
    let mut dispatcher = ScriptedActionDispatcher::default();
    let mut approvals = ScriptedApprovalController::default();

    let result = engine
        .execute_run(
            &mut session,
            "hello",
            &mut model,
            &mut dispatcher,
            &mut approvals,
        )
        .unwrap();

    let HarnessRunResult::Terminal(result) = result else {
        panic!("expected terminal result");
    };
    assert_eq!(
        result.report.consumer_context.as_ref().unwrap().status,
        "unavailable"
    );
    assert!(
        !model.requests[0]
            .prompt
            .render_text()
            .contains("deleted before run")
    );
    assert_consumer_context_unavailable_event(&handle);
}

fn assert_consumer_context_unavailable_event(handle: &InMemoryEventSink) {
    assert!(
        handle
            .events()
            .iter()
            .any(|event| event.event_type == HarnessEventType::ConsumerContextUnavailable)
    );
}

#[test]
fn multi_turn_phase_processes_multiple_ordered_actions() {
    let mut engine = HarnessEngine::new(base_loop(), HarnessEngineOptions::new(limits()));
    let mut session = session_with_tool_and_skill();
    let mut model = ScriptedModelRuntime::new(vec![
        ModelTurn {
            assistant_content: Some("I will act.".into()),
            actions: vec![
                SemanticActionProposal::new(
                    "tool-1",
                    SemanticAction::AgentPmTool {
                        tool: "@zack/search".into(),
                        arguments: json!({ "query": "incident" }),
                    },
                ),
                SemanticActionProposal::new(
                    "skill-1",
                    SemanticAction::SkillResourceRead {
                        skill: "@zack/skill".into(),
                        resource: "entrypoint".into(),
                    },
                ),
            ],
            usage: RunUsage::default(),
            finish_reason: None,
            provider_metadata: BTreeMap::new(),
        },
        completion("a", "execute"),
        completion("b", "review"),
        completion("c", "ready"),
    ]);
    let mut dispatcher = ScriptedActionDispatcher::default();
    let mut approvals = ScriptedApprovalController::default();
    let result = engine
        .execute_run(
            &mut session,
            "hello",
            &mut model,
            &mut dispatcher,
            &mut approvals,
        )
        .unwrap();
    assert!(matches!(result, HarnessRunResult::Terminal(_)));
    assert_eq!(dispatcher.dispatched.len(), 2);
    assert_eq!(dispatcher.dispatched[0].identity(), "@zack/search");
    assert_eq!(
        dispatcher.dispatched[1].identity(),
        "@zack/skill/entrypoint"
    );
    assert_eq!(model.requests.len(), 4);
    assert!(model.requests[1].transcript.len() > model.requests[0].transcript.len());
}

#[test]
fn invalid_tool_arguments_request_repair_before_dispatch() {
    let mut runtime_limits = limits();
    runtime_limits.max_structured_output_repairs = 0;
    runtime_limits.max_tool_call_repairs = 1;
    let mut engine = HarnessEngine::new(base_loop(), HarnessEngineOptions::new(runtime_limits));
    let mut session = session_with_tool_and_skill();
    let mut model = ScriptedModelRuntime::new(vec![
        ModelTurn {
            assistant_content: None,
            actions: vec![SemanticActionProposal::new(
                "bad-tool",
                SemanticAction::AgentPmTool {
                    tool: "@zack/search".into(),
                    arguments: json!({ "query": 42 }),
                },
            )],
            usage: RunUsage::default(),
            finish_reason: None,
            provider_metadata: BTreeMap::new(),
        },
        tool_turn("@zack/search"),
        completion("a", "execute"),
        completion("b", "review"),
        completion("c", "ready"),
    ]);
    let mut dispatcher = ScriptedActionDispatcher::default();
    let mut approvals = ScriptedApprovalController::default();

    let result = engine
        .execute_run(
            &mut session,
            "hello",
            &mut model,
            &mut dispatcher,
            &mut approvals,
        )
        .unwrap();

    let HarnessRunResult::Terminal(result) = result else {
        panic!("expected terminal result");
    };
    assert_eq!(result.status, HarnessTerminalStatus::Ended);
    assert_eq!(result.report.repair_count, 1);
    assert_eq!(dispatcher.dispatched.len(), 1);
    assert_eq!(dispatcher.dispatched[0].identity(), "@zack/search");
    assert!(
        model.requests[1]
            .prompt
            .render_text()
            .contains("arguments are invalid")
    );
}

#[test]
fn tool_call_repair_limit_exhaustion_fails_before_dispatch() {
    let mut runtime_limits = limits();
    runtime_limits.max_tool_call_repairs = 0;
    let mut engine = HarnessEngine::new(base_loop(), HarnessEngineOptions::new(runtime_limits));
    let mut session = session_with_tool_and_skill();
    let mut model = ScriptedModelRuntime::new(vec![ModelTurn {
        assistant_content: None,
        actions: vec![SemanticActionProposal::new(
            "bad-tool",
            SemanticAction::AgentPmTool {
                tool: "@zack/search".into(),
                arguments: json!({ "query": 42 }),
            },
        )],
        usage: RunUsage::default(),
        finish_reason: None,
        provider_metadata: BTreeMap::new(),
    }]);
    let mut dispatcher = ScriptedActionDispatcher::default();
    let mut approvals = ScriptedApprovalController::default();

    let result = engine
        .execute_run(
            &mut session,
            "hello",
            &mut model,
            &mut dispatcher,
            &mut approvals,
        )
        .unwrap();

    let HarnessRunResult::Terminal(result) = result else {
        panic!("expected terminal result");
    };
    assert_eq!(result.status, HarnessTerminalStatus::Failed);
    assert_eq!(result.report.repair_count, 0);
    assert_eq!(
        result.output,
        Some(json!({ "error": "tool call repair limit exhausted" }))
    );
    assert!(dispatcher.dispatched.is_empty());
}

#[test]
fn two_tools_preserve_order_and_validate_against_each_tool_schema() {
    let mut runtime_limits = limits();
    runtime_limits.max_tool_call_repairs = 1;
    let mut engine = HarnessEngine::new(base_loop(), HarnessEngineOptions::new(runtime_limits));
    let mut session = HarnessSession::with_runtime_snapshot(runtime_with_two_tools_and_skill());
    let mut model = ScriptedModelRuntime::new(vec![
        tool_turn_with_arguments("@zack/comment", json!({ "query": "wrong schema" })),
        tool_turn("@zack/search"),
        tool_turn_with_arguments("@zack/comment", json!({ "body": "ready" })),
        completion("a", "execute"),
        completion("b", "review"),
        completion("c", "ready"),
    ]);
    let mut dispatcher = ScriptedActionDispatcher::default();
    let mut approvals = ScriptedApprovalController::default();

    let result = engine
        .execute_run(
            &mut session,
            "hello",
            &mut model,
            &mut dispatcher,
            &mut approvals,
        )
        .unwrap();

    let HarnessRunResult::Terminal(result) = result else {
        panic!("expected terminal result");
    };
    assert_eq!(result.status, HarnessTerminalStatus::Ended);
    assert_eq!(result.report.repair_count, 1);
    let tool_descriptors = model.requests[0]
        .effective_phase
        .capability_catalog
        .iter()
        .filter(|descriptor| descriptor.action_kind == "agentpm_tool")
        .map(|descriptor| descriptor.identity.as_str())
        .collect::<Vec<_>>();
    assert_eq!(tool_descriptors, vec!["@zack/search", "@zack/comment"]);
    assert_eq!(dispatcher.dispatched.len(), 2);
    assert_eq!(dispatcher.dispatched[0].identity(), "@zack/search");
    assert_eq!(dispatcher.dispatched[1].identity(), "@zack/comment");
    assert!(
        model.requests[1]
            .prompt
            .render_text()
            .contains("Tool `@zack/comment` arguments are invalid")
    );
}

#[test]
fn phase_scoped_tool_candidates_are_active_only_for_matching_phase() {
    let mut runtime = runtime_with_two_tools_and_skill();
    runtime.capability_candidates = vec![
        RuntimeCapabilitySnapshot {
            kind: "tool".into(),
            identity: "@zack/search".into(),
            scope: "phase:assess".into(),
            source: "agent_binding".into(),
            state: "available".into(),
        },
        RuntimeCapabilitySnapshot {
            kind: "tool".into(),
            identity: "@zack/comment".into(),
            scope: "phase:execute".into(),
            source: "agent_binding".into(),
            state: "available".into(),
        },
    ];

    let loop_manifest = base_loop();
    let assess = EffectivePhase::from_phase(&loop_manifest.r#loop.phases[0], &runtime);
    let execute = EffectivePhase::from_phase(&loop_manifest.r#loop.phases[1], &runtime);

    assert!(
        assess
            .capability_catalog
            .iter()
            .any(|descriptor| descriptor.action_kind == "agentpm_tool"
                && descriptor.identity == "@zack/search")
    );
    assert!(
        !assess
            .capability_catalog
            .iter()
            .any(|descriptor| descriptor.action_kind == "agentpm_tool"
                && descriptor.identity == "@zack/comment")
    );
    assert!(
        execute
            .capability_catalog
            .iter()
            .any(|descriptor| descriptor.action_kind == "agentpm_tool"
                && descriptor.identity == "@zack/comment")
    );
}

#[test]
fn schema_valid_tool_domain_failure_is_returned_to_phase_transcript() {
    let mut engine = HarnessEngine::new(base_loop(), HarnessEngineOptions::new(limits()));
    let mut session = session_with_tool_and_skill();
    let mut model = ScriptedModelRuntime::new(vec![
        tool_turn("@zack/search"),
        completion("a", "execute"),
        completion("b", "review"),
        completion("c", "ready"),
    ]);
    let mut dispatcher = ScriptedActionDispatcher::default();
    dispatcher.push_result(
        "@zack/search",
        ActionDispatchResult::success(json!({
            "ok": false,
            "error": "domain-level failure",
            "reason": "manual_review_required"
        })),
    );
    let mut approvals = ScriptedApprovalController::default();

    let result = engine
        .execute_run(
            &mut session,
            "hello",
            &mut model,
            &mut dispatcher,
            &mut approvals,
        )
        .unwrap();

    let HarnessRunResult::Terminal(result) = result else {
        panic!("expected terminal result");
    };
    assert_eq!(result.status, HarnessTerminalStatus::Ended);
    assert_eq!(dispatcher.dispatched.len(), 1);
    let next_prompt = model.requests[1].prompt.render_text();
    assert!(next_prompt.contains("ActionResult [agentpm_tool @zack/search]"));
    assert!(next_prompt.contains("\"ok\":false"));
    assert!(next_prompt.contains("domain-level failure"));
}

#[test]
fn knowledge_backend_failure_is_returned_to_phase_transcript() {
    let mut engine = HarnessEngine::new(base_loop(), HarnessEngineOptions::new(limits()));
    let mut session =
        HarnessSession::with_runtime_snapshot(runtime_with_knowledge_packages(&["@zack/guide"]));
    let memory = InMemoryEventSink::default();
    let handle = memory.clone();
    session.emitter.add_sink(Box::new(memory));
    let mut model = ScriptedModelRuntime::new(vec![
        knowledge_query_turn("@zack/guide"),
        completion("assess-complete", "execute"),
        completion("execute-complete", "review"),
        completion("review-complete", "ready"),
    ]);
    let mut dispatcher = ScriptedActionDispatcher::default();
    let mut knowledge = RecordingKnowledgeRuntime::new(ActionDispatchResult::success(json!({
        "ok": false,
        "package": "@zack/guide",
        "version": "0.1.0",
        "mode": "vector_query",
        "query": "alpha",
        "error": {
            "code": "knowledge_backend_down",
            "message": "backend unavailable",
            "retryable": true
        }
    })));
    let mut approvals = ScriptedApprovalController::default();
    let mut hooks = NoopHookRuntime;
    let mut services = HarnessRuntimeServices {
        model: &mut model,
        dispatcher: &mut dispatcher,
        knowledge: &mut knowledge,
        approvals: &mut approvals,
        hooks: &mut hooks,
        service_events: None,
    };

    let result = engine
        .execute_run_with_id(
            &mut session,
            "run-knowledge-backend-failure".into(),
            "hello",
            &mut services,
        )
        .unwrap();

    let HarnessRunResult::Terminal(result) = result else {
        panic!("expected terminal result");
    };
    assert_eq!(result.status, HarnessTerminalStatus::Ended);
    assert_eq!(knowledge.dispatched.len(), 1);
    assert!(model.requests.len() >= 2);
    let next_prompt = model.requests[1].prompt.render_text();
    assert!(next_prompt.contains("ActionResult [knowledge_request @zack/guide]"));
    assert!(next_prompt.contains("\"ok\":false"));
    assert!(next_prompt.contains("knowledge_backend_down"));
    assert!(next_prompt.contains("backend unavailable"));
    let event_types = handle
        .events()
        .iter()
        .map(|event| event.event_type)
        .collect::<Vec<_>>();
    assert!(event_types.contains(&HarnessEventType::KnowledgeFailed));
    assert!(!event_types.contains(&HarnessEventType::PhaseFailed));
}

#[test]
fn skill_resource_content_is_loaded_on_demand_and_phase_local() {
    let mut engine = HarnessEngine::new(base_loop(), HarnessEngineOptions::new(limits()));
    let mut session = session_with_tool_and_skill();
    let memory = InMemoryEventSink::default();
    let handle = memory.clone();
    session.emitter.add_sink(Box::new(memory));
    let mut model = ScriptedModelRuntime::new(vec![
        skill_read_turn("@zack/skill", "entrypoint"),
        skill_read_turn("@zack/skill", "references/handoff-template.md"),
        completion("a", "execute"),
        completion("b", "review"),
        completion("c", "ready"),
    ]);
    let mut dispatcher = ScriptedActionDispatcher::default();
    dispatcher.push_result(
        "@zack/skill/entrypoint",
        ActionDispatchResult::success(json!({
            "action_kind": "skill_resource_read",
            "ok": true,
            "skill": "@zack/skill",
            "resource": "entrypoint",
            "content": "Use concise handoff guidance."
        })),
    );
    dispatcher.push_result(
        "@zack/skill/references/handoff-template.md",
        ActionDispatchResult::success(json!({
            "action_kind": "skill_resource_read",
            "ok": true,
            "skill": "@zack/skill",
            "resource": "references/handoff-template.md",
            "content": "Use the handoff template."
        })),
    );
    let mut approvals = ScriptedApprovalController::default();

    let result = engine
        .execute_run(
            &mut session,
            "hello",
            &mut model,
            &mut dispatcher,
            &mut approvals,
        )
        .unwrap();

    assert!(matches!(result, HarnessRunResult::Terminal(_)));
    let prompt_with_one_resource = model.requests[1].prompt.render_text();
    assert!(prompt_with_one_resource.contains("Loaded resource: entrypoint"));
    assert!(prompt_with_one_resource.contains("Use concise handoff guidance."));
    assert!(
        prompt_with_one_resource
            .contains("ActionResult [skill_resource_read @zack/skill/entrypoint]")
    );
    let prompt_with_grouped_resources = model.requests[2].prompt.render_text();
    assert_eq!(
        prompt_with_grouped_resources
            .matches("Skill: @zack/skill")
            .count(),
        1
    );
    assert!(prompt_with_grouped_resources.contains("Loaded resource: entrypoint"));
    assert!(
        prompt_with_grouped_resources.contains("Loaded resource: references/handoff-template.md")
    );
    assert!(prompt_with_grouped_resources.contains("Use concise handoff guidance."));
    assert!(prompt_with_grouped_resources.contains("Use the handoff template."));
    assert!(
        !model.requests[3]
            .prompt
            .render_text()
            .contains("Use concise handoff guidance.")
    );
    let event_types: Vec<_> = handle
        .events()
        .iter()
        .map(|event| event.event_type)
        .collect();
    assert!(event_types.contains(&HarnessEventType::SkillActivated));
    assert!(event_types.contains(&HarnessEventType::SkillResourceRequested));
    assert!(event_types.contains(&HarnessEventType::SkillResourceLoaded));
    let skill_requested = handle
        .events()
        .into_iter()
        .find(|event| event.event_type == HarnessEventType::SkillResourceRequested)
        .unwrap();
    let HarnessEventPayload::Action { fields, .. } = skill_requested.payload else {
        panic!("expected action payload");
    };
    assert_eq!(fields["source"], "agent_binding");
}

#[test]
fn unavailable_tool_is_suppressed_from_effective_phase() {
    let mut runtime = runtime_with_tool_and_skill();
    runtime.tools[0].state = "unavailable".into();
    runtime.capability_candidates[0].state = "unavailable".into();
    let phase = &base_loop().r#loop.phases[0];
    let effective = EffectivePhase::from_phase(phase, &runtime);

    assert!(
        !effective
            .capability_catalog
            .iter()
            .any(|descriptor| descriptor.action_kind == "agentpm_tool")
    );
    assert!(
        effective
            .suppressed_capabilities
            .iter()
            .any(|capability| capability.kind == "tool" && capability.identity == "@zack/search")
    );
}

#[test]
fn knowledge_packages_remain_distinct_model_visible_surfaces() {
    let runtime = runtime_with_knowledge_packages(&["@zack/alpha", "@zack/beta"]);
    let phase = &base_loop().r#loop.phases[0];
    let effective = EffectivePhase::from_phase(phase, &runtime);

    let knowledge_descriptors = effective
        .capability_catalog
        .iter()
        .filter(|descriptor| descriptor.action_kind == "knowledge_request")
        .map(|descriptor| descriptor.identity.as_str())
        .collect::<Vec<_>>();
    assert_eq!(knowledge_descriptors, vec!["@zack/alpha", "@zack/beta"]);
    assert_eq!(
        effective
            .active_knowledge
            .iter()
            .map(|knowledge| knowledge.name.as_str())
            .collect::<Vec<_>>(),
        vec!["@zack/alpha", "@zack/beta"]
    );
    assert!(
        !effective
            .capability_catalog
            .iter()
            .any(|descriptor| descriptor.action_kind == "knowledge_request"
                && descriptor.identity == "knowledge")
    );
}

#[test]
fn loop_access_suppresses_skill_inherited_tools_but_not_skill_resources() {
    let mut runtime = runtime_with_tool_and_skill();
    runtime.capability_candidates = vec![
        RuntimeCapabilitySnapshot {
            kind: "skill".into(),
            identity: "@zack/skill".into(),
            scope: "global".into(),
            source: "agent_binding".into(),
            state: "available".into(),
        },
        RuntimeCapabilitySnapshot {
            kind: "tool".into(),
            identity: "@zack/search".into(),
            scope: "global".into(),
            source: "skill:@zack/skill".into(),
            state: "available".into(),
        },
    ];
    let mut loop_manifest = base_loop();
    loop_manifest.r#loop.phases[0].access = Some(LoopPhaseAccess {
        tools: Some(false),
        knowledge: None,
        memory: None,
    });

    let effective = EffectivePhase::from_phase(&loop_manifest.r#loop.phases[0], &runtime);

    assert!(
        effective
            .capability_catalog
            .iter()
            .any(|descriptor| descriptor.action_kind == "skill_resource_read"
                && descriptor.identity == "@zack/skill")
    );
    assert!(
        !effective
            .capability_catalog
            .iter()
            .any(|descriptor| descriptor.action_kind == "agentpm_tool")
    );
    assert!(
        effective
            .suppressed_capabilities
            .iter()
            .any(|capability| capability.kind == "tool"
                && capability.identity == "@zack/search"
                && capability.source == "skill:@zack/skill"
                && capability.reason == "Loop access.tools=false for this phase")
    );
}

#[test]
fn ambiguous_completion_plus_action_requests_repair_without_executing_action() {
    let mut engine = HarnessEngine::new(base_loop(), HarnessEngineOptions::new(limits()));
    let mut session = HarnessSession::new();
    let mut model = ScriptedModelRuntime::new(vec![
        ModelTurn {
            assistant_content: None,
            actions: vec![
                SemanticActionProposal::new(
                    "tool",
                    SemanticAction::AgentPmTool {
                        tool: "@zack/search".into(),
                        arguments: json!({}),
                    },
                ),
                SemanticActionProposal::new(
                    "complete",
                    SemanticAction::PhaseCompletion {
                        outcome: Some("execute".into()),
                        output: None,
                    },
                ),
            ],
            usage: RunUsage::default(),
            finish_reason: None,
            provider_metadata: BTreeMap::new(),
        },
        completion("repair", "execute"),
        completion("b", "review"),
        completion("c", "ready"),
    ]);
    let mut dispatcher = ScriptedActionDispatcher::default();
    let mut approvals = ScriptedApprovalController::default();
    let result = engine
        .execute_run(
            &mut session,
            "hello",
            &mut model,
            &mut dispatcher,
            &mut approvals,
        )
        .unwrap();
    let HarnessRunResult::Terminal(result) = result else {
        panic!("expected terminal result");
    };
    assert_eq!(result.report.repair_count, 1);
    assert!(dispatcher.dispatched.is_empty());
    assert_eq!(
        model.requests[1].repair_feedback.as_deref(),
        Some("A phase completion proposal cannot be combined with executable actions.")
    );
}

#[test]
fn loop_access_gates_tools_knowledge_and_memory_but_not_skill_resources() {
    let mut loop_manifest = base_loop();
    loop_manifest.r#loop.phases[0].access = Some(LoopPhaseAccess {
        tools: Some(false),
        knowledge: Some(false),
        memory: Some(LoopAccessMemory {
            read: Some(false),
            write: Some(false),
        }),
    });
    let mut engine = HarnessEngine::new(loop_manifest, HarnessEngineOptions::new(limits()));
    let mut session = session_with_tool_and_skill();
    let memory = InMemoryEventSink::default();
    let handle = memory.clone();
    session.emitter.add_sink(Box::new(memory));
    let mut model = ScriptedModelRuntime::new(vec![
        tool_turn("@zack/search"),
        ModelTurn {
            assistant_content: None,
            actions: vec![SemanticActionProposal::new(
                "skill",
                SemanticAction::SkillResourceRead {
                    skill: "@zack/skill".into(),
                    resource: "entrypoint".into(),
                },
            )],
            usage: RunUsage::default(),
            finish_reason: None,
            provider_metadata: BTreeMap::new(),
        },
        completion("a", "execute"),
        completion("b", "review"),
        completion("c", "ready"),
    ]);
    let mut dispatcher = ScriptedActionDispatcher::default();
    let mut approvals = ScriptedApprovalController::default();
    let result = engine
        .execute_run(
            &mut session,
            "hello",
            &mut model,
            &mut dispatcher,
            &mut approvals,
        )
        .unwrap();
    assert!(matches!(result, HarnessRunResult::Terminal(_)));
    assert_eq!(dispatcher.dispatched.len(), 1);
    assert!(matches!(
        dispatcher.dispatched[0],
        SemanticAction::SkillResourceRead { .. }
    ));
    let events = handle.events();
    let tool_candidates = events
        .iter()
        .find(|event| event.event_type == HarnessEventType::ToolCandidatesComputed)
        .unwrap();
    let HarnessEventPayload::Lifecycle { fields, .. } = &tool_candidates.payload else {
        panic!("expected lifecycle payload");
    };
    assert_eq!(fields["suppressed"][0]["identity"], "@zack/search");
    assert_eq!(fields["suppressed"][0]["source"], "agent_binding");
    assert_eq!(
        fields["suppressed"][0]["reason"],
        "Loop access.tools=false for this phase"
    );
}

#[test]
fn approval_checkpoints_are_ordered_and_first_rejection_routes() {
    let mut loop_manifest = base_loop();
    loop_manifest.r#loop.checkpoints = vec![
        LoopCheckpoint {
            id: "first".into(),
            r#type: "approval".into(),
            before_phase: "execute".into(),
            on_reject: "$abort".into(),
        },
        LoopCheckpoint {
            id: "second".into(),
            r#type: "approval".into(),
            before_phase: "execute".into(),
            on_reject: "$handoff".into(),
        },
    ];
    let mut engine = HarnessEngine::new(loop_manifest, HarnessEngineOptions::new(limits()));
    let mut session = HarnessSession::new();
    let mut model = ScriptedModelRuntime::new(vec![completion("a", "execute")]);
    let mut dispatcher = ScriptedActionDispatcher::default();
    let mut approvals = ScriptedApprovalController::default();
    approvals.push("first", ApprovalDecision::Approve);
    approvals.push("second", ApprovalDecision::Deny);
    let result = engine
        .execute_run(
            &mut session,
            "hello",
            &mut model,
            &mut dispatcher,
            &mut approvals,
        )
        .unwrap();
    let HarnessRunResult::Terminal(result) = result else {
        panic!("expected terminal result");
    };
    assert_eq!(result.status, HarnessTerminalStatus::HandedOff);
    let checkpoints: Vec<_> = result
        .report
        .checkpoint_summaries
        .iter()
        .map(|checkpoint| (&checkpoint.checkpoint_id, &checkpoint.status))
        .collect();
    assert_eq!(
        checkpoints,
        vec![
            (&"first".into(), &"approved".into()),
            (&"second".into(), &"denied".into())
        ]
    );
}

#[test]
fn implicit_and_invalid_explicit_outcomes_are_handled_with_repair() {
    let mut loop_manifest = base_loop();
    loop_manifest.r#loop.phases[2].outcomes.clear();
    loop_manifest.r#loop.transitions.push(LoopTransition {
        from: "review".into(),
        on: "complete".into(),
        to: "$end".into(),
    });
    let (result, _, model) = run_engine(
        loop_manifest,
        vec![
            completion("a", "bogus"),
            completion("repair", "execute"),
            completion("b", "review"),
            ModelTurn {
                assistant_content: Some("implicit".into()),
                actions: Vec::new(),
                usage: RunUsage::default(),
                finish_reason: None,
                provider_metadata: BTreeMap::new(),
            },
        ],
    );
    let HarnessRunResult::Terminal(result) = result else {
        panic!("expected terminal result");
    };
    assert_eq!(result.report.repair_count, 1);
    assert!(
        model.requests[1]
            .repair_feedback
            .as_deref()
            .unwrap_or_default()
            .contains("not declared")
    );
}

#[test]
fn max_step_exhaustion_returns_limit_reached() {
    let mut loop_manifest = base_loop();
    loop_manifest.r#loop.limits = Some(LoopLimits { max_steps: Some(2) });
    let (result, _, _) = run_engine(
        loop_manifest,
        vec![
            completion("a", "execute"),
            completion("b", "review"),
            completion("c", "again"),
        ],
    );
    let HarnessRunResult::Terminal(result) = result else {
        panic!("expected terminal result");
    };
    assert_eq!(result.status, HarnessTerminalStatus::LimitReached);
}

#[test]
fn tool_retry_counts_additional_attempts_after_initial_failure() {
    let mut loop_manifest = base_loop();
    loop_manifest.r#loop.error_policy = Some(LoopErrorPolicy {
        tool_failure: Some(LoopToolFailurePolicy {
            action: LoopToolFailureAction::Retry,
            max_retries: Some(2),
            on_exhausted: Some(LoopToolFailureExhaustedAction::FailPhase),
        }),
        phase_failure: None,
    });
    let mut engine = HarnessEngine::new(loop_manifest, HarnessEngineOptions::new(limits()));
    let mut session = session_with_tool_and_skill();
    let mut model = ScriptedModelRuntime::new(vec![
        tool_turn("@zack/search"),
        completion("a", "execute"),
        completion("b", "review"),
        completion("c", "ready"),
    ]);
    let mut dispatcher = ScriptedActionDispatcher::default();
    dispatcher.push_result("@zack/search", ActionDispatchResult::failure("temporary"));
    dispatcher.push_result(
        "@zack/search",
        ActionDispatchResult::success(json!({"results": ["cached hit"]})),
    );
    let mut approvals = ScriptedApprovalController::default();
    let result = engine
        .execute_run(
            &mut session,
            "hello",
            &mut model,
            &mut dispatcher,
            &mut approvals,
        )
        .unwrap();
    let HarnessRunResult::Terminal(result) = result else {
        panic!("expected terminal result");
    };
    assert_eq!(result.status, HarnessTerminalStatus::Ended);
    assert_eq!(result.report.retry_count, 1);
    assert_eq!(result.report.usage.tool_retries, 1);
    assert_eq!(dispatcher.dispatched.len(), 2);
    let next_prompt = model.requests[1].prompt.render_text();
    assert!(next_prompt.contains("ActionResult [agentpm_tool @zack/search]"));
    assert!(next_prompt.contains(SUCCESSFUL_ACTION_RESULT_CONTROL));
}

#[test]
fn default_action_failure_becomes_runtime_failed() {
    let mut engine = HarnessEngine::new(base_loop(), HarnessEngineOptions::new(limits()));
    let mut session = session_with_tool_and_skill();
    let mut model = ScriptedModelRuntime::new(vec![tool_turn("@zack/search")]);
    let mut dispatcher = ScriptedActionDispatcher::default();
    dispatcher.push_result("@zack/search", ActionDispatchResult::failure("boom"));
    let mut approvals = ScriptedApprovalController::default();
    let result = engine
        .execute_run(
            &mut session,
            "hello",
            &mut model,
            &mut dispatcher,
            &mut approvals,
        )
        .unwrap();
    let HarnessRunResult::Terminal(result) = result else {
        panic!("expected terminal result");
    };
    assert_eq!(result.status, HarnessTerminalStatus::Failed);
    assert_eq!(result.report.error_count, 1);
}

#[test]
fn direct_tool_failure_policies_route_with_structured_terminal_status() {
    let (fail_phase_result, _) = run_tool_failure_policy(
        LoopToolFailurePolicy {
            action: LoopToolFailureAction::FailPhase,
            max_retries: None,
            on_exhausted: None,
        },
        vec![ActionDispatchResult::failure("$abort")],
    );
    assert_eq!(fail_phase_result.status, HarnessTerminalStatus::Failed);
    assert_eq!(fail_phase_result.output, Some(json!({ "error": "$abort" })));

    let (abort_result, _) = run_tool_failure_policy(
        LoopToolFailurePolicy {
            action: LoopToolFailureAction::Abort,
            max_retries: None,
            on_exhausted: None,
        },
        vec![ActionDispatchResult::failure("tool failed")],
    );
    assert_eq!(abort_result.status, HarnessTerminalStatus::Aborted);

    let (handoff_result, _) = run_tool_failure_policy(
        LoopToolFailurePolicy {
            action: LoopToolFailureAction::Handoff,
            max_retries: None,
            on_exhausted: None,
        },
        vec![ActionDispatchResult::failure("tool failed")],
    );
    assert_eq!(handoff_result.status, HarnessTerminalStatus::HandedOff);
}

#[test]
fn retry_exhaustion_tool_policies_route_terminal_status() {
    let (fail_phase_result, fail_phase_dispatcher) = run_tool_failure_policy(
        LoopToolFailurePolicy {
            action: LoopToolFailureAction::Retry,
            max_retries: Some(1),
            on_exhausted: Some(LoopToolFailureExhaustedAction::FailPhase),
        },
        vec![
            ActionDispatchResult::failure("temporary"),
            ActionDispatchResult::failure("still failing"),
        ],
    );
    assert_eq!(fail_phase_result.status, HarnessTerminalStatus::Failed);
    assert_eq!(fail_phase_result.report.retry_count, 1);
    assert_eq!(fail_phase_dispatcher.dispatched.len(), 2);

    let (abort_result, abort_dispatcher) = run_tool_failure_policy(
        LoopToolFailurePolicy {
            action: LoopToolFailureAction::Retry,
            max_retries: Some(1),
            on_exhausted: Some(LoopToolFailureExhaustedAction::Abort),
        },
        vec![
            ActionDispatchResult::failure("temporary"),
            ActionDispatchResult::failure("still failing"),
        ],
    );
    assert_eq!(abort_result.status, HarnessTerminalStatus::Aborted);
    assert_eq!(abort_result.report.retry_count, 1);
    assert_eq!(abort_dispatcher.dispatched.len(), 2);

    let (handoff_result, handoff_dispatcher) = run_tool_failure_policy(
        LoopToolFailurePolicy {
            action: LoopToolFailureAction::Retry,
            max_retries: Some(1),
            on_exhausted: Some(LoopToolFailureExhaustedAction::Handoff),
        },
        vec![
            ActionDispatchResult::failure("temporary"),
            ActionDispatchResult::failure("still failing"),
        ],
    );
    assert_eq!(handoff_result.status, HarnessTerminalStatus::HandedOff);
    assert_eq!(handoff_result.report.retry_count, 1);
    assert_eq!(handoff_dispatcher.dispatched.len(), 2);
}

#[test]
fn deterministic_tool_failure_categories_do_not_retry() {
    let mut loop_manifest = base_loop();
    loop_manifest.r#loop.error_policy = Some(LoopErrorPolicy {
        tool_failure: Some(LoopToolFailurePolicy {
            action: LoopToolFailureAction::Retry,
            max_retries: Some(2),
            on_exhausted: Some(LoopToolFailureExhaustedAction::FailPhase),
        }),
        phase_failure: None,
    });
    let mut engine = HarnessEngine::new(loop_manifest, HarnessEngineOptions::new(limits()));
    let mut session = session_with_tool_and_skill();
    let mut model = ScriptedModelRuntime::new(vec![tool_turn("@zack/search")]);
    let mut dispatcher = ScriptedActionDispatcher::default();
    dispatcher.push_result(
        "@zack/search",
        ActionDispatchResult::failure_with_category(
            ActionFailureCategory::Schema,
            "authoritative Tool input schema rejected arguments",
        ),
    );
    let mut approvals = ScriptedApprovalController::default();

    let result = engine
        .execute_run(
            &mut session,
            "hello",
            &mut model,
            &mut dispatcher,
            &mut approvals,
        )
        .unwrap();

    let HarnessRunResult::Terminal(result) = result else {
        panic!("expected terminal result");
    };
    assert_eq!(result.status, HarnessTerminalStatus::Failed);
    assert_eq!(result.report.retry_count, 0);
    assert_eq!(dispatcher.dispatched.len(), 1);
}

#[test]
fn terminal_tool_failure_status_does_not_retry() {
    let (result, dispatcher) = run_tool_failure_policy(
        LoopToolFailurePolicy {
            action: LoopToolFailureAction::Retry,
            max_retries: Some(2),
            on_exhausted: Some(LoopToolFailureExhaustedAction::FailPhase),
        },
        vec![ActionDispatchResult::terminal_failure(
            HarnessTerminalStatus::Cancelled,
            "ToolRuntime cancelled agentpm run for `@zack/search`",
        )],
    );

    assert_eq!(result.status, HarnessTerminalStatus::Cancelled);
    assert_eq!(result.report.retry_count, 0);
    assert_eq!(result.report.usage.tool_retries, 0);
    assert_eq!(dispatcher.dispatched.len(), 1);
}

#[test]
fn multiple_sequential_runs_reuse_session_and_reset_run_state() {
    let mut engine = HarnessEngine::new(base_loop(), HarnessEngineOptions::new(limits()));
    let mut session = HarnessSession::new();
    let mut dispatcher = ScriptedActionDispatcher::default();
    let mut approvals = ScriptedApprovalController::default();

    let mut first_model = ScriptedModelRuntime::new(vec![
        completion("a", "execute"),
        completion("b", "review"),
        completion("c", "ready"),
    ]);
    let first = engine
        .execute_run(
            &mut session,
            "first",
            &mut first_model,
            &mut dispatcher,
            &mut approvals,
        )
        .unwrap();
    let HarnessRunResult::Terminal(first) = first else {
        panic!("expected terminal first run");
    };

    let mut second_model = ScriptedModelRuntime::new(vec![
        completion("d", "execute"),
        completion("e", "review"),
        completion("f", "ready"),
    ]);
    let second = engine
        .execute_run(
            &mut session,
            "second",
            &mut second_model,
            &mut dispatcher,
            &mut approvals,
        )
        .unwrap();
    let HarnessRunResult::Terminal(second) = second else {
        panic!("expected terminal second run");
    };

    assert_ne!(first.report.run_id, second.report.run_id);
    assert_eq!(session.usage.started_runs, 2);
    assert_eq!(session.usage.completed_runs, 2);
    assert_eq!(first_model.requests[0].prior_phase_results.len(), 0);
    assert_eq!(second_model.requests[0].prior_phase_results.len(), 0);
}

#[test]
fn unresolved_approval_terminalizes_as_approval_required_when_not_retained() {
    let mut loop_manifest = base_loop();
    loop_manifest.r#loop.checkpoints = vec![LoopCheckpoint {
        id: "approve-assess".into(),
        r#type: "approval".into(),
        before_phase: "assess".into(),
        on_reject: "$handoff".into(),
    }];
    let mut engine = HarnessEngine::new(loop_manifest, HarnessEngineOptions::new(limits()));
    let mut session = HarnessSession::new();
    let mut model = ScriptedModelRuntime::new(vec![completion("a", "execute")]);
    let mut dispatcher = ScriptedActionDispatcher::default();
    let mut approvals = ScriptedApprovalController::default();
    approvals.push("approve-assess", ApprovalDecision::Pending);

    let result = engine
        .execute_run(
            &mut session,
            "hello",
            &mut model,
            &mut dispatcher,
            &mut approvals,
        )
        .unwrap();
    let HarnessRunResult::Terminal(result) = result else {
        panic!("expected terminal approval-required");
    };
    assert_eq!(result.status, HarnessTerminalStatus::ApprovalRequired);
    assert_eq!(result.report.checkpoint_summaries[0].status, "pending");
    assert!(session.active_run().is_none());
}

#[test]
fn authored_abort_and_cancellation_have_distinct_terminal_statuses() {
    let mut abort_loop = base_loop();
    abort_loop.r#loop.transitions[1].to = "$abort".into();
    let (abort_result, _, _) = run_engine(abort_loop, vec![completion("a", "handoff")]);
    let HarnessRunResult::Terminal(abort_result) = abort_result else {
        panic!("expected terminal abort");
    };
    assert_eq!(abort_result.status, HarnessTerminalStatus::Aborted);

    let mut engine = HarnessEngine::new(
        base_loop(),
        HarnessEngineOptions {
            runtime_limits: limits(),
            retain_active_on_approval_required: true,
        },
    );
    let mut session = HarnessSession::new();
    session.start_run("cancel me".into()).unwrap();
    let cancelled = engine
        .cancel_active_run(&mut session, "test cancellation")
        .unwrap();
    assert_eq!(cancelled.status, HarnessTerminalStatus::Cancelled);
    assert_eq!(session.usage.completed_runs, 1);
}

#[test]
fn phase_failure_policy_can_abort_or_handoff() {
    let mut abort_loop = base_loop();
    abort_loop.r#loop.error_policy = Some(LoopErrorPolicy {
        tool_failure: None,
        phase_failure: Some(LoopPhaseFailurePolicy {
            action: LoopPhaseFailureAction::Abort,
        }),
    });
    let (abort_result, _, _) = {
        let mut engine = HarnessEngine::new(abort_loop, HarnessEngineOptions::new(limits()));
        let mut session = HarnessSession::new();
        let mut model =
            ScriptedModelRuntime::with_results(vec![Err(ModelRuntimeFailure::new("model down"))]);
        let mut dispatcher = ScriptedActionDispatcher::default();
        let mut approvals = ScriptedApprovalController::default();
        let result = engine
            .execute_run(
                &mut session,
                "hello",
                &mut model,
                &mut dispatcher,
                &mut approvals,
            )
            .unwrap();
        (result, session, model)
    };
    let HarnessRunResult::Terminal(abort_result) = abort_result else {
        panic!("expected terminal abort");
    };
    assert_eq!(abort_result.status, HarnessTerminalStatus::Aborted);

    let mut handoff_loop = base_loop();
    handoff_loop.r#loop.error_policy = Some(LoopErrorPolicy {
        tool_failure: None,
        phase_failure: Some(LoopPhaseFailurePolicy {
            action: LoopPhaseFailureAction::Handoff,
        }),
    });
    let (handoff_result, _, _) = {
        let mut engine = HarnessEngine::new(handoff_loop, HarnessEngineOptions::new(limits()));
        let mut session = HarnessSession::new();
        let mut model =
            ScriptedModelRuntime::with_results(vec![Err(ModelRuntimeFailure::new("model down"))]);
        let mut dispatcher = ScriptedActionDispatcher::default();
        let mut approvals = ScriptedApprovalController::default();
        let result = engine
            .execute_run(
                &mut session,
                "hello",
                &mut model,
                &mut dispatcher,
                &mut approvals,
            )
            .unwrap();
        (result, session, model)
    };
    let HarnessRunResult::Terminal(handoff_result) = handoff_result else {
        panic!("expected terminal handoff");
    };
    assert_eq!(handoff_result.status, HarnessTerminalStatus::HandedOff);
}

#[test]
fn runtime_limits_cover_model_action_tool_and_repair_exhaustion() {
    let mut model_limit = limits();
    model_limit.max_model_calls_per_phase = 1;
    let mut engine = HarnessEngine::new(base_loop(), HarnessEngineOptions::new(model_limit));
    let mut session = session_with_tool_and_skill();
    let mut model =
        ScriptedModelRuntime::new(vec![tool_turn("@zack/search"), completion("a", "execute")]);
    let mut dispatcher = ScriptedActionDispatcher::default();
    let mut approvals = ScriptedApprovalController::default();
    let result = engine
        .execute_run(
            &mut session,
            "hello",
            &mut model,
            &mut dispatcher,
            &mut approvals,
        )
        .unwrap();
    let HarnessRunResult::Terminal(result) = result else {
        panic!("expected terminal limit");
    };
    assert_eq!(result.status, HarnessTerminalStatus::LimitReached);

    let mut action_limit = limits();
    action_limit.max_actions_per_phase = 0;
    let mut engine = HarnessEngine::new(base_loop(), HarnessEngineOptions::new(action_limit));
    let mut session = session_with_tool_and_skill();
    let mut model = ScriptedModelRuntime::new(vec![completion("a", "execute")]);
    let result = engine
        .execute_run(
            &mut session,
            "hello",
            &mut model,
            &mut dispatcher,
            &mut approvals,
        )
        .unwrap();
    let HarnessRunResult::Terminal(result) = result else {
        panic!("expected terminal limit");
    };
    assert_eq!(result.status, HarnessTerminalStatus::LimitReached);

    let mut tool_limit = limits();
    tool_limit.max_tool_calls_per_phase = 0;
    let mut engine = HarnessEngine::new(base_loop(), HarnessEngineOptions::new(tool_limit));
    let mut session = session_with_tool_and_skill();
    let mut model = ScriptedModelRuntime::new(vec![tool_turn("@zack/search")]);
    let result = engine
        .execute_run(
            &mut session,
            "hello",
            &mut model,
            &mut dispatcher,
            &mut approvals,
        )
        .unwrap();
    let HarnessRunResult::Terminal(result) = result else {
        panic!("expected terminal limit");
    };
    assert_eq!(result.status, HarnessTerminalStatus::LimitReached);

    let mut repair_limit = limits();
    repair_limit.max_structured_output_repairs = 0;
    let mut engine = HarnessEngine::new(base_loop(), HarnessEngineOptions::new(repair_limit));
    let mut session = HarnessSession::new();
    let mut model = ScriptedModelRuntime::new(vec![completion("bad", "missing")]);
    let result = engine
        .execute_run(
            &mut session,
            "hello",
            &mut model,
            &mut dispatcher,
            &mut approvals,
        )
        .unwrap();
    let HarnessRunResult::Terminal(result) = result else {
        panic!("expected terminal failed");
    };
    assert_eq!(result.status, HarnessTerminalStatus::Failed);
    assert_eq!(result.report.repair_count, 0);
}

#[test]
fn report_and_events_include_phase_transition_action_and_usage_data() {
    let mut engine = HarnessEngine::new(base_loop(), HarnessEngineOptions::new(limits()));
    let mut session = session_with_tool_and_skill();
    let memory = InMemoryEventSink::default();
    let handle = memory.clone();
    session.emitter.add_sink(Box::new(memory));
    let mut model = ScriptedModelRuntime::new(vec![
        tool_turn("@zack/search"),
        completion("a", "execute"),
        completion("b", "review"),
        completion("c", "ready"),
    ]);
    let mut dispatcher = ScriptedActionDispatcher::default();
    let mut approvals = ScriptedApprovalController::default();
    let result = engine
        .execute_run(
            &mut session,
            "hello",
            &mut model,
            &mut dispatcher,
            &mut approvals,
        )
        .unwrap();
    let HarnessRunResult::Terminal(result) = result else {
        panic!("expected terminal result");
    };
    assert_eq!(result.report.action_summaries.len(), 1);
    assert_eq!(result.report.tool_summaries.len(), 1);
    assert_eq!(result.report.usage.accepted_semantic_actions, 4);
    let events = handle.events();
    let event_types: Vec<_> = events.iter().map(|event| event.event_type).collect();
    assert!(event_types.contains(&HarnessEventType::RunStarted));
    assert!(event_types.contains(&HarnessEventType::ToolCandidatesComputed));
    assert!(event_types.contains(&HarnessEventType::PhaseStarted));
    assert!(event_types.contains(&HarnessEventType::SemanticActionProposed));
    assert!(event_types.contains(&HarnessEventType::ToolInvoked));
    assert!(event_types.contains(&HarnessEventType::ToolCompleted));
    assert!(event_types.contains(&HarnessEventType::TransitionSelected));
    assert!(event_types.contains(&HarnessEventType::RunCompleted));
    assert!(event_types.contains(&HarnessEventType::SessionUsageUpdated));
    let tool_invoked = events
        .iter()
        .find(|event| event.event_type == HarnessEventType::ToolInvoked)
        .unwrap();
    assert_eq!(
        tool_invoked.phase_execution_id.as_deref(),
        Some("phase-exec-1")
    );
    let HarnessEventPayload::Action { fields, .. } = &tool_invoked.payload else {
        panic!("expected action payload");
    };
    assert_eq!(fields["source"], "agent_binding");
}

#[test]
fn knowledge_dispatch_usage_is_reported_and_rolled_up() {
    let mut engine = HarnessEngine::new(base_loop(), HarnessEngineOptions::new(limits()));
    let mut session = HarnessSession::new();
    let memory = InMemoryEventSink::default();
    let handle = memory.clone();
    session.emitter.add_sink(Box::new(memory));
    session
        .runtime_snapshot
        .knowledge
        .push(KnowledgeRuntimeSnapshot {
            name: "@zack/guide".into(),
            version: "0.1.0".into(),
            mode: "vector".into(),
            description: "Guide".into(),
            root: None,
            source: "agent_binding".into(),
            state: "available".into(),
            runtime: "local".into(),
            readiness_reason: None,
            documents: Vec::new(),
            embedding: Some(KnowledgeEmbeddingSnapshot {
                id: "default".into(),
                provider: "manual".into(),
                model: "toy-3d".into(),
                dimensions: 3,
                metric: "cosine".into(),
                normalized: true,
            }),
            retrieval: None,
        });
    session
        .runtime_snapshot
        .capability_candidates
        .push(RuntimeCapabilitySnapshot {
            kind: "knowledge".into(),
            identity: "@zack/guide".into(),
            scope: "global".into(),
            source: "agent_binding".into(),
            state: "available".into(),
        });
    let usage = RunUsage {
        embedding_requests: 1,
        ..Default::default()
    };
    let mut knowledge = UsageKnowledgeRuntime {
        result: ActionDispatchResult::success(json!({
            "ok": true,
            "package": "@zack/guide",
            "version": "0.1.0",
            "mode": "vector_query",
            "query": "alpha",
            "results": [],
            "citations": []
        }))
        .with_usage(usage),
    };
    let mut model = ScriptedModelRuntime::new(vec![
        knowledge_query_turn("@zack/guide"),
        completion("assess-complete", "execute"),
        completion("execute-complete", "review"),
        completion("review-complete", "ready"),
    ]);
    let mut dispatcher = ScriptedActionDispatcher::default();
    let mut approvals = ScriptedApprovalController::default();
    let mut hooks = NoopHookRuntime;
    let mut services = HarnessRuntimeServices {
        model: &mut model,
        dispatcher: &mut dispatcher,
        knowledge: &mut knowledge,
        approvals: &mut approvals,
        hooks: &mut hooks,
        service_events: None,
    };

    let result = engine
        .execute_run_with_id(&mut session, "run-usage".into(), "hello", &mut services)
        .unwrap();
    let HarnessRunResult::Terminal(result) = result else {
        panic!("expected terminal result");
    };

    assert_eq!(result.report.usage.knowledge_requests, 1);
    assert_eq!(result.report.usage.embedding_requests, 1);
    assert_eq!(session.usage.knowledge_requests, 1);
    assert_eq!(session.usage.embedding_requests, 1);
    let events = handle.events();
    let event_types: Vec<_> = events.iter().map(|event| event.event_type).collect();
    assert!(event_types.contains(&HarnessEventType::EmbeddingRequestStarted));
    assert!(event_types.contains(&HarnessEventType::EmbeddingRequestCompleted));
    let embedding_started = events
        .iter()
        .find(|event| event.event_type == HarnessEventType::EmbeddingRequestStarted)
        .unwrap();
    let HarnessEventPayload::Action { fields, .. } = &embedding_started.payload else {
        panic!("expected embedding action payload");
    };
    assert_eq!(fields["package"], "@zack/guide");
    assert_eq!(fields["provider"], "manual");
    assert_eq!(fields["model"], "toy-3d");
    assert_eq!(fields["dimensions"], 3);
    assert_eq!(fields["normalized"], true);
    assert!(fields.get("duration_ms").is_none());
    let embedding_completed = events
        .iter()
        .find(|event| event.event_type == HarnessEventType::EmbeddingRequestCompleted)
        .unwrap();
    let HarnessEventPayload::Action { fields, .. } = &embedding_completed.payload else {
        panic!("expected embedding action payload");
    };
    assert!(fields["duration_ms"].as_u64().is_some());
}

#[test]
fn embedding_provider_failures_emit_embedding_failed_event() {
    let mut engine = HarnessEngine::new(base_loop(), HarnessEngineOptions::new(limits()));
    let mut session = HarnessSession::new();
    let memory = InMemoryEventSink::default();
    let handle = memory.clone();
    session.emitter.add_sink(Box::new(memory));
    session
        .runtime_snapshot
        .knowledge
        .push(KnowledgeRuntimeSnapshot {
            name: "@zack/guide".into(),
            version: "0.1.0".into(),
            mode: "vector".into(),
            description: "Guide".into(),
            root: None,
            source: "agent_binding".into(),
            state: "available".into(),
            runtime: "local".into(),
            readiness_reason: None,
            documents: Vec::new(),
            embedding: Some(KnowledgeEmbeddingSnapshot {
                id: "default".into(),
                provider: "manual".into(),
                model: "toy-3d".into(),
                dimensions: 3,
                metric: "cosine".into(),
                normalized: true,
            }),
            retrieval: None,
        });
    session
        .runtime_snapshot
        .capability_candidates
        .push(RuntimeCapabilitySnapshot {
            kind: "knowledge".into(),
            identity: "@zack/guide".into(),
            scope: "global".into(),
            source: "agent_binding".into(),
            state: "available".into(),
        });
    let usage = RunUsage {
        embedding_requests: 1,
        ..Default::default()
    };
    let mut knowledge = UsageKnowledgeRuntime {
        result: ActionDispatchResult::success(json!({
            "ok": false,
            "package": "@zack/guide",
            "version": "0.1.0",
            "mode": "vector_query",
            "query": "alpha",
            "results": [],
            "citations": [],
            "error": {
                "code": "embedding_provider_failed",
                "message": "provider unavailable",
                "retryable": false
            }
        }))
        .with_usage(usage),
    };
    let mut model = ScriptedModelRuntime::new(vec![
        knowledge_query_turn("@zack/guide"),
        completion("assess-complete", "execute"),
        completion("execute-complete", "review"),
        completion("review-complete", "ready"),
    ]);
    let mut dispatcher = ScriptedActionDispatcher::default();
    let mut approvals = ScriptedApprovalController::default();
    let mut hooks = NoopHookRuntime;
    let mut services = HarnessRuntimeServices {
        model: &mut model,
        dispatcher: &mut dispatcher,
        knowledge: &mut knowledge,
        approvals: &mut approvals,
        hooks: &mut hooks,
        service_events: None,
    };

    let result = engine
        .execute_run_with_id(
            &mut session,
            "run-embedding-failed".into(),
            "hello",
            &mut services,
        )
        .unwrap();
    assert!(matches!(result, HarnessRunResult::Terminal(_)));

    let events = handle.events();
    let event_types: Vec<_> = events.iter().map(|event| event.event_type).collect();
    assert!(event_types.contains(&HarnessEventType::EmbeddingRequestStarted));
    assert!(event_types.contains(&HarnessEventType::EmbeddingRequestFailed));
    assert!(event_types.contains(&HarnessEventType::KnowledgeFailed));
    assert!(!event_types.contains(&HarnessEventType::EmbeddingRequestCompleted));
    let embedding_failed = events
        .iter()
        .find(|event| event.event_type == HarnessEventType::EmbeddingRequestFailed)
        .unwrap();
    let HarnessEventPayload::Action { fields, .. } = &embedding_failed.payload else {
        panic!("expected embedding action payload");
    };
    assert_eq!(fields["error_code"], "embedding_provider_failed");
    assert!(fields["duration_ms"].as_u64().is_some());
}

#[test]
fn non_tool_action_failures_emit_specific_failed_event_types() {
    let mut engine = HarnessEngine::new(base_loop(), HarnessEngineOptions::new(limits()));
    let mut session = HarnessSession::new();
    let memory = InMemoryEventSink::default();
    let handle = memory.clone();
    session.emitter.add_sink(Box::new(memory));
    session
        .runtime_snapshot
        .knowledge
        .push(KnowledgeRuntimeSnapshot {
            name: "@zack/guide".into(),
            version: "0.1.0".into(),
            mode: "vector".into(),
            description: "Guide".into(),
            root: None,
            source: "agent_binding".into(),
            state: "available".into(),
            runtime: "local".into(),
            readiness_reason: None,
            documents: Vec::new(),
            embedding: None,
            retrieval: None,
        });
    session
        .runtime_snapshot
        .knowledge
        .push(KnowledgeRuntimeSnapshot {
            name: "@zack/stale-guide".into(),
            version: "0.1.0".into(),
            mode: "vector".into(),
            description: "Stale guide".into(),
            root: None,
            source: "agent_binding".into(),
            state: "unavailable".into(),
            runtime: "local".into(),
            readiness_reason: Some("vector index is stale".into()),
            documents: Vec::new(),
            embedding: None,
            retrieval: None,
        });
    session
        .runtime_snapshot
        .capability_candidates
        .push(RuntimeCapabilitySnapshot {
            kind: "knowledge".into(),
            identity: "@zack/guide".into(),
            scope: "global".into(),
            source: "agent_binding".into(),
            state: "available".into(),
        });
    session
        .runtime_snapshot
        .capability_candidates
        .push(RuntimeCapabilitySnapshot {
            kind: "knowledge".into(),
            identity: "@zack/stale-guide".into(),
            scope: "global".into(),
            source: "agent_binding".into(),
            state: "unavailable".into(),
        });
    let mut model = ScriptedModelRuntime::new(vec![ModelTurn {
        assistant_content: None,
        actions: vec![SemanticActionProposal::new(
            "knowledge",
            SemanticAction::KnowledgeRequest {
                package: "@zack/guide".into(),
                mode: None,
                document: None,
                query: Some("x".into()),
                top_k: None,
                score_threshold: None,
                return_citations: None,
            },
        )],
        usage: RunUsage::default(),
        finish_reason: None,
        provider_metadata: BTreeMap::new(),
    }]);
    let mut dispatcher = ScriptedActionDispatcher::default();
    dispatcher.push_result("@zack/guide", ActionDispatchResult::failure("unavailable"));
    let mut approvals = ScriptedApprovalController::default();
    let result = engine
        .execute_run(
            &mut session,
            "hello",
            &mut model,
            &mut dispatcher,
            &mut approvals,
        )
        .unwrap();
    let HarnessRunResult::Terminal(result) = result else {
        panic!("expected terminal result");
    };
    assert_eq!(result.status, HarnessTerminalStatus::Failed);
    assert_eq!(
        result.report.knowledge_summaries,
        vec![OperationReportSummary {
            operation_kind: "knowledge_request".into(),
            identity: "@zack/guide".into(),
            status: "failed".into(),
            count: 1,
        }]
    );
    let events = handle.events();
    let event_types: Vec<_> = events.iter().map(|event| event.event_type).collect();
    assert!(event_types.contains(&HarnessEventType::KnowledgeSurfaceReady));
    assert!(event_types.contains(&HarnessEventType::KnowledgeSurfaceUnavailable));
    assert!(event_types.contains(&HarnessEventType::KnowledgeFailed));
    assert!(!event_types.contains(&HarnessEventType::SemanticActionCompleted));
    let unavailable = events
        .iter()
        .find(|event| event.event_type == HarnessEventType::KnowledgeSurfaceUnavailable)
        .unwrap();
    let HarnessEventPayload::Action { fields, .. } = &unavailable.payload else {
        panic!("expected action payload");
    };
    assert_eq!(fields["reason"], "vector index is stale");
}
