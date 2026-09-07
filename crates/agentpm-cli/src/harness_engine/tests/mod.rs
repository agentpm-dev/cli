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
use crate::harness_runtime::knowledge::{
    EmbeddingProvider, KnowledgeRuntimeFailure, ServiceRuntime,
};
use crate::harness_runtime::model::{
    KnowledgeEmbeddingSnapshot, MemoryRecordTypeRuntimeSnapshot, MemorySpaceRuntimeSnapshot,
    ModelProviderSelection, ModelRuntimeFailure, ModelTurn, RuntimeCapabilitySnapshot,
    SUCCESSFUL_ACTION_RESULT_CONTROL, ScriptedModelRuntime, SkillResourceSnapshot,
    SkillRuntimeSnapshot, ToolRuntimeSnapshot,
};
use crate::harness_runtime::service::HostServiceInvoker;
use crate::manifest::{
    LoopAccessMemory, LoopCheckpoint, LoopErrorPolicy, LoopLimits, LoopMetadata, LoopOutcome,
    LoopPhaseAccess, LoopPhaseFailurePolicy, LoopToolFailurePolicy, LoopTransition,
    MemoryRetrievalMode, MemorySpaceModel,
};
use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;

#[derive(Debug, Default)]
struct TestMemoryEmbeddingProvider {
    calls: Vec<String>,
}

impl EmbeddingProvider for TestMemoryEmbeddingProvider {
    fn validate_space(&self, space: &KnowledgeEmbeddingSnapshot) -> Result<(), String> {
        if space.provider == "test"
            && space.model == "toy-2d"
            && space.dimensions == 2
            && space.metric == "cosine"
            && space.normalized
        {
            Ok(())
        } else {
            Err("unexpected Memory embedding space".into())
        }
    }

    fn embed(
        &mut self,
        _space: &KnowledgeEmbeddingSnapshot,
        text: &str,
    ) -> std::result::Result<Vec<f32>, KnowledgeRuntimeFailure> {
        self.calls.push(text.to_string());
        if text.contains("Alpha") || text.contains("alpha") {
            Ok(vec![1.0, 0.0])
        } else {
            Ok(vec![0.0, 1.0])
        }
    }
}

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

fn enable_m14c_memory_package_semantic(root: &std::path::Path) {
    let manifest_path = root.join("agent.json");
    let mut manifest: Value =
        serde_json::from_slice(&std::fs::read(&manifest_path).unwrap()).unwrap();
    manifest["memory"]["spaces"]["notes"]["retrieval"]["modes"] =
        json!(["key", "filter", "chronological", "full_text", "semantic"]);
    std::fs::write(
        &manifest_path,
        serde_json::to_vec_pretty(&manifest).unwrap(),
    )
    .unwrap();
    crate::commands::memory::execute_memory_build(
        &manifest_path,
        crate::commands::memory::MemoryBuildMode::Write,
    )
    .unwrap();
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
        semantic: None,
        append_only: false,
        record_types: vec![record_type],
    });
    runtime
}

fn session_with_tool_and_skill() -> HarnessSession {
    HarnessSession::with_runtime_snapshot(runtime_with_tool_and_skill())
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

mod core;
mod hooks;
mod memory;
