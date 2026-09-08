use super::*;
use crate::harness_config::{
    HarnessApprovalController, HarnessConfig, HarnessConfigSource, HarnessHookBinding,
    HarnessHookFailurePolicy, HarnessHookId, HarnessImplementation, HarnessImplementationEntry,
    HarnessMemorySemanticConfig, HarnessRuntimeMapping, HarnessTraceConfig, HarnessTraceContent,
    HarnessTraceLevel, ResolvedHarnessConfig,
};
use crate::harness_observability::{
    HarnessTerminalStatus, MemoryWriteReviewReportSummary, ReportPackageIdentity, RunReport,
    RunUsage,
};
use crate::harness_runtime::SemanticAction;
use crate::harness_runtime::action::ScriptedActionDispatcher;
use crate::harness_runtime::action::SemanticActionProposal;
use crate::harness_runtime::model::{
    ModelCapabilityAdvertisement, ModelRuntimeFailure, ModelTurn, ScriptedModelRuntime,
};
use crate::semver::types::PackageKind;
use serde_json::json;
use std::fs;
use std::path::Path;

#[test]
fn parse_scope_requires_key_value_pair() {
    assert_eq!(
        parse_scope("user=user-1").unwrap(),
        ("user".to_string(), "user-1".to_string())
    );
    assert!(parse_scope("user").is_err());
    assert!(parse_scope("=user-1").is_err());
    assert!(parse_scope("user=").is_err());
}

#[test]
fn capability_counts_describe_pending_runtime_activation() {
    let root = temp_dir("capability-count-labels");
    let mut plan = minimal_plan(&root);
    plan.capabilities
        .push(crate::harness_plan::StaticCapabilityCandidate {
            kind: "knowledge".into(),
            identity: "@zack/manual-context".into(),
            scope: "global".into(),
            source: "agent_binding".into(),
            state: CapabilityState::Available,
        });
    plan.capabilities
        .push(crate::harness_plan::StaticCapabilityCandidate {
            kind: "model_provider".into(),
            identity: "openai/gpt-4o-mini".into(),
            scope: "session".into(),
            source: "harness_config".into(),
            state: CapabilityState::Pending,
        });

    let counts = capability_counts(&plan);
    assert_eq!(counts.get("available"), Some(&1));
    assert_eq!(counts.get("pending runtime activation"), Some(&1));
    assert!(!counts.contains_key("pending"));
}

#[test]
fn verbose_static_capability_details_list_available_and_pending_identities() {
    let root = temp_dir("verbose-static-capability-details");
    let mut plan = minimal_plan(&root);
    plan.capabilities
        .push(crate::harness_plan::StaticCapabilityCandidate {
            kind: "knowledge".into(),
            identity: "@zack/manual-vector".into(),
            scope: "global".into(),
            source: "agent_binding".into(),
            state: CapabilityState::Available,
        });
    plan.capabilities
        .push(crate::harness_plan::StaticCapabilityCandidate {
            kind: "embedding_provider".into(),
            identity: "toy-embedder".into(),
            scope: "session".into(),
            source: "harness_config".into(),
            state: CapabilityState::Pending,
        });

    let details = static_capability_detail_lines(&plan);
    assert_eq!(
        details,
        vec![
            "  - available: knowledge `@zack/manual-vector` (scopes: global, sources: agent_binding)",
            "  - pending runtime activation: embedding_provider `toy-embedder` (scopes: session, sources: harness_config)",
        ]
    );
}

#[test]
fn memory_write_review_failures_render_human_warning_lines() {
    let mut report = minimal_run_report("run-review-warning");
    report.memory_write_review_summaries = vec![
        MemoryWriteReviewReportSummary {
            point: "run_end".into(),
            phase_execution_id: "phase-exec-1".into(),
            status: "failed".into(),
            reason: "structured_output_repair_limit".into(),
            model_calls: 4,
            memory_reads_attempted: 0,
            memory_reads_completed: 0,
            memory_writes_attempted: 0,
            memory_writes_completed: 0,
        },
        MemoryWriteReviewReportSummary {
            point: "phase_end".into(),
            phase_execution_id: "phase-exec-2".into(),
            status: "completed".into(),
            reason: "completed".into(),
            model_calls: 2,
            memory_reads_attempted: 0,
            memory_reads_completed: 0,
            memory_writes_attempted: 1,
            memory_writes_completed: 1,
        },
    ];

    assert_eq!(
        memory_write_review_warning_lines(&report),
        vec![
            "Warning: Memory write review at run_end failed: structured_output_repair_limit. Memory writes attempted/completed: 0/0. Intended Memory may not have been written."
        ]
    );
}

#[test]
fn memory_write_review_skipped_summaries_do_not_render_human_warning_lines() {
    let mut report = minimal_run_report("run-review-skipped-warning");
    report.memory_write_review_summaries = vec![MemoryWriteReviewReportSummary {
        point: "phase_end".into(),
        phase_execution_id: "phase-exec-3".into(),
        status: "skipped".into(),
        reason: "no_writable_memory_surface".into(),
        model_calls: 0,
        memory_reads_attempted: 0,
        memory_reads_completed: 0,
        memory_writes_attempted: 0,
        memory_writes_completed: 0,
    }];

    assert!(memory_write_review_warning_lines(&report).is_empty());
}

#[test]
fn memory_write_review_failed_warning_lines_are_available_for_aborted_terminals() {
    let mut report = minimal_run_report("run-review-aborted-warning");
    report.terminal_status = HarnessTerminalStatus::Aborted;
    report.memory_write_review_summaries = vec![MemoryWriteReviewReportSummary {
        point: "run_end".into(),
        phase_execution_id: "phase-exec-1".into(),
        status: "failed".into(),
        reason: "structured_output_repair_limit".into(),
        model_calls: 4,
        memory_reads_attempted: 0,
        memory_reads_completed: 0,
        memory_writes_attempted: 0,
        memory_writes_completed: 0,
    }];
    let terminal = RuntimeTerminalResult {
        status: HarnessTerminalStatus::Aborted,
        output: Some(json!({ "summary": "authored abort" })),
        report,
    };

    assert_eq!(
        terminal_memory_write_review_warning_lines(&terminal),
        vec![
            "Warning: Memory write review at run_end failed: structured_output_repair_limit. Memory writes attempted/completed: 0/0. Intended Memory may not have been written."
        ]
    );
}

#[test]
fn memory_write_review_mixed_summaries_warn_only_for_failures() {
    let mut report = minimal_run_report("run-review-mixed-warning");
    report.memory_write_review_summaries = vec![
        MemoryWriteReviewReportSummary {
            point: "run_end".into(),
            phase_execution_id: "phase-exec-1".into(),
            status: "failed".into(),
            reason: "structured_output_repair_limit".into(),
            model_calls: 4,
            memory_reads_attempted: 0,
            memory_reads_completed: 0,
            memory_writes_attempted: 0,
            memory_writes_completed: 0,
        },
        MemoryWriteReviewReportSummary {
            point: "phase_end".into(),
            phase_execution_id: "phase-exec-2".into(),
            status: "completed".into(),
            reason: "completed".into(),
            model_calls: 2,
            memory_reads_attempted: 0,
            memory_reads_completed: 0,
            memory_writes_attempted: 1,
            memory_writes_completed: 1,
        },
        MemoryWriteReviewReportSummary {
            point: "phase_end".into(),
            phase_execution_id: "phase-exec-3".into(),
            status: "skipped".into(),
            reason: "no_writable_memory_surface".into(),
            model_calls: 0,
            memory_reads_attempted: 0,
            memory_reads_completed: 0,
            memory_writes_attempted: 0,
            memory_writes_completed: 0,
        },
    ];

    assert_eq!(
        memory_write_review_warning_lines(&report),
        vec![
            "Warning: Memory write review at run_end failed: structured_output_repair_limit. Memory writes attempted/completed: 0/0. Intended Memory may not have been written."
        ]
    );
}

#[test]
fn verbose_static_capability_details_combine_scopes_for_counted_identity() {
    let root = temp_dir("verbose-static-capability-combined-scopes");
    let mut plan = minimal_plan(&root);
    for scope in ["phase:no-knowledge", "phase:research"] {
        plan.capabilities
            .push(crate::harness_plan::StaticCapabilityCandidate {
                kind: "knowledge".into(),
                identity: "@zack/manual-context".into(),
                scope: scope.into(),
                source: "agent_binding".into(),
                state: CapabilityState::Available,
            });
    }

    let counts = capability_counts(&plan);
    let details = static_capability_detail_lines(&plan);
    assert_eq!(counts.get("available"), Some(&1));
    assert_eq!(
        details,
        vec![
            "  - available: knowledge `@zack/manual-context` (scopes: phase:no-knowledge, phase:research, sources: agent_binding)",
        ]
    );
}

#[test]
fn trace_verbose_enables_human_preflight_verbose_details() {
    let root = temp_dir("trace-verbose-human-preflight");
    let mut plan = minimal_plan(&root);

    assert!(!human_preflight_verbose_enabled(false, &plan));
    assert!(human_preflight_verbose_enabled(true, &plan));

    plan.config.config.trace.level = HarnessTraceLevel::Verbose;
    assert!(human_preflight_verbose_enabled(false, &plan));
}

#[test]
fn default_surface_is_tui_with_explicit_headless_and_machine_modes() {
    let default_args = HarnessArgs {
        agent: None,
        config: None,
        state_dir: None,
        scopes: Vec::new(),
        machine: false,
        headless: false,
        json: false,
        verbose: false,
        input: None,
        input_file: None,
        report: None,
    };
    assert_eq!(default_args.surface(), HarnessExecutionSurface::Tui);

    let headless_args = HarnessArgs {
        headless: true,
        ..default_args.clone()
    };
    assert_eq!(headless_args.surface(), HarnessExecutionSurface::Headless);

    let machine_args = HarnessArgs {
        machine: true,
        ..default_args
    };
    assert_eq!(machine_args.surface(), HarnessExecutionSurface::Machine);
}

#[test]
fn headless_and_machine_preflight_avoid_stdout_reserved_for_payloads() {
    assert_eq!(
        PreflightOutputStream::for_surface(HarnessExecutionSurface::Headless),
        PreflightOutputStream::Stderr
    );
    assert_eq!(
        PreflightOutputStream::for_surface(HarnessExecutionSurface::Tui),
        PreflightOutputStream::Stdout
    );
    assert_eq!(
        PreflightOutputStream::for_surface(HarnessExecutionSurface::Machine),
        PreflightOutputStream::Stderr
    );
    let args = HarnessArgs {
        agent: None,
        config: None,
        state_dir: None,
        scopes: Vec::new(),
        machine: false,
        headless: true,
        json: true,
        verbose: false,
        input: None,
        input_file: None,
        report: None,
    };
    let err = validate_surface_flags(HarnessExecutionSurface::Headless, &args).unwrap_err();
    assert!(err.to_string().contains("--json cannot be combined"));
}

#[test]
fn machine_protocol_rejects_wrong_version_and_non_request_input() {
    let mut request = MachineEnvelope {
        protocol: AGENTPM_HARNESS_MACHINE_PROTOCOL.into(),
        version: AGENTPM_HARNESS_MACHINE_VERSION,
        kind: MachineFrameKind::Request,
        id: Some("req-1".into()),
        method: Some("initialize".into()),
        payload: json!({}),
        error: None,
    };
    assert!(validate_machine_request(&request).is_ok());
    request.version = 2;
    assert!(
        validate_machine_request(&request)
            .unwrap_err()
            .contains("unsupported protocol version")
    );
    request.version = AGENTPM_HARNESS_MACHINE_VERSION;
    request.kind = MachineFrameKind::Event;
    assert!(
        validate_machine_request(&request)
            .unwrap_err()
            .contains("kind `request`")
    );
}

#[test]
fn machine_json_flag_is_rejected_because_stdout_is_protocol_only() {
    let args = HarnessArgs {
        agent: None,
        config: None,
        state_dir: None,
        scopes: Vec::new(),
        machine: true,
        headless: false,
        json: true,
        verbose: false,
        input: None,
        input_file: None,
        report: None,
    };
    let err = validate_surface_flags(HarnessExecutionSurface::Machine, &args).unwrap_err();
    assert!(err.to_string().contains("protocol frames"));
}

#[test]
fn host_service_requirements_include_selected_model_hooks_and_approval() {
    let root = temp_dir("host-service-requirements");
    let mut plan = minimal_plan(&root);
    plan.config.config.model = Some(crate::harness_config::HarnessModelConfig {
        provider: "host-model".into(),
        model: "model-1".into(),
        options: json!({}),
    });
    plan.config.config.providers.models.insert(
        "host-model".into(),
        HarnessImplementationEntry {
            implementation: HarnessImplementation::Host {
                request_timeout_ms: 1_000,
            },
        },
    );
    plan.config.config.hooks.implementations.insert(
        "host-hooks".into(),
        HarnessImplementationEntry {
            implementation: HarnessImplementation::Host {
                request_timeout_ms: 1_000,
            },
        },
    );
    plan.config.config.hooks.bindings.push(HarnessHookBinding {
        hook: HarnessHookId::BeforeToolCall,
        implementation: "host-hooks".into(),
        failure_policy: HarnessHookFailurePolicy::Closed,
    });
    plan.config.config.approvals.controller = Some(HarnessApprovalController {
        implementation: HarnessImplementation::Host {
            request_timeout_ms: 1_000,
        },
    });

    let required = required_host_services(&plan);
    assert!(required.contains(&host_service("model", "host-model")));
    assert!(required.contains(&host_service("hook", "host-hooks")));
    assert!(required.contains(&host_service("approval", "controller")));
}

#[test]
fn host_service_requirements_include_mapped_memory_runtime() {
    let root = temp_dir("host-memory-service-requirements");
    let mut plan = minimal_plan(&root);
    write_semantic_memory_fixture_with_modes(&root, &mut plan, json!(["key", "semantic"]));
    plan.config.config.memory.runtimes.insert(
        "remote-memory".into(),
        HarnessImplementationEntry {
            implementation: HarnessImplementation::Host {
                request_timeout_ms: 1_000,
            },
        },
    );
    plan.config.config.memory.packages.insert(
        "semantic-memory-test".into(),
        HarnessRuntimeMapping {
            runtime: "remote-memory".into(),
        },
    );

    let required = required_host_services(&plan);
    assert!(required.contains(&host_service("memory", "remote-memory")));
}

#[test]
fn custom_memory_activation_failure_suppresses_mapped_space_without_local_fallback() {
    let root = temp_dir("custom-memory-activation-failure");
    let mut plan = minimal_plan(&root);
    write_semantic_memory_fixture_with_modes(&root, &mut plan, json!(["key"]));
    plan.config.config.memory.runtimes.insert(
        "remote-memory".into(),
        HarnessImplementationEntry {
            implementation: HarnessImplementation::Process {
                command: "missing-agentpm-memory-runtime".into(),
                args: Vec::new(),
                cwd: None,
                env: Vec::new(),
                startup_timeout_ms: 100,
                request_timeout_ms: 100,
                restart: Default::default(),
            },
        },
    );
    plan.config.config.memory.packages.insert(
        "semantic-memory-test".into(),
        HarnessRuntimeMapping {
            runtime: "remote-memory".into(),
        },
    );

    let mut runtime = runtime_snapshot_from_plan(&plan);
    assert_eq!(runtime.memory[0].runtime, "remote-memory");
    assert_eq!(runtime.memory[0].state, "available");

    let activation = activate_custom_memory_runtime_for_plan(&plan, &runtime, None, None);
    assert!(activation.runtime.is_none());
    apply_custom_memory_activation_to_runtime(&mut runtime, &activation);

    assert_eq!(runtime.memory[0].runtime, "remote-memory");
    assert_eq!(runtime.memory[0].state, "unavailable");
    assert!(
        runtime.memory[0]
            .readiness_reason
            .as_deref()
            .unwrap_or_default()
            .contains("configured MemoryRuntime `remote-memory` could not start")
    );
}

#[test]
fn custom_memory_activation_filters_retrieval_modes_from_runtime_capabilities() {
    let root = temp_dir("custom-memory-retrieval-filter");
    let mut plan = minimal_plan(&root);
    write_semantic_memory_fixture_with_modes(&root, &mut plan, json!(["key", "semantic"]));
    plan.config.config.memory.runtimes.insert(
        "remote-memory".into(),
        HarnessImplementationEntry {
            implementation: HarnessImplementation::Host {
                request_timeout_ms: 1_000,
            },
        },
    );
    plan.config.config.memory.packages.insert(
        "semantic-memory-test".into(),
        HarnessRuntimeMapping {
            runtime: "remote-memory".into(),
        },
    );
    let (bridge, _, _) = buffered_machine_bridge();
    bridge.register_host_service(
        &host_service("memory", "remote-memory"),
        custom_memory_capabilities("semantic-memory-test", "0.1.0", json!(["key"])),
    );

    let mut runtime = runtime_snapshot_from_plan(&plan);
    let activation = activate_custom_memory_runtime_for_plan(&plan, &runtime, Some(bridge), None);
    assert!(activation.runtime.is_some());
    apply_custom_memory_activation_to_runtime(&mut runtime, &activation);

    assert_eq!(runtime.memory[0].state, "available");
    assert_eq!(
        runtime.memory[0].retrieval_modes,
        vec![MemoryRetrievalMode::Key]
    );
}

#[test]
fn custom_memory_activation_rejects_package_version_mismatch() {
    let root = temp_dir("custom-memory-package-version-mismatch");
    let mut plan = minimal_plan(&root);
    write_semantic_memory_fixture_with_modes(&root, &mut plan, json!(["key"]));
    plan.config.config.memory.runtimes.insert(
        "remote-memory".into(),
        HarnessImplementationEntry {
            implementation: HarnessImplementation::Host {
                request_timeout_ms: 1_000,
            },
        },
    );
    plan.config.config.memory.packages.insert(
        "semantic-memory-test".into(),
        HarnessRuntimeMapping {
            runtime: "remote-memory".into(),
        },
    );
    let (bridge, _, _) = buffered_machine_bridge();
    bridge.register_host_service(
        &host_service("memory", "remote-memory"),
        custom_memory_capabilities("semantic-memory-test", "9.9.9", json!(["key"])),
    );

    let mut runtime = runtime_snapshot_from_plan(&plan);
    let activation = activate_custom_memory_runtime_for_plan(&plan, &runtime, Some(bridge), None);
    assert!(activation.runtime.is_none());
    apply_custom_memory_activation_to_runtime(&mut runtime, &activation);

    assert_eq!(runtime.memory[0].runtime, "remote-memory");
    assert_eq!(runtime.memory[0].state, "unavailable");
    assert!(
        runtime.memory[0]
            .readiness_reason
            .as_deref()
            .unwrap_or_default()
            .contains("does not attest semantic-memory-test@0.1.0 as ready")
    );
}

#[test]
fn custom_memory_activation_suppresses_blueprint_space_mismatch() {
    let root = temp_dir("custom-memory-blueprint-space-mismatch");
    let mut plan = minimal_plan(&root);
    write_semantic_memory_fixture_with_modes(&root, &mut plan, json!(["key"]));
    plan.config.config.memory.runtimes.insert(
        "remote-memory".into(),
        HarnessImplementationEntry {
            implementation: HarnessImplementation::Host {
                request_timeout_ms: 1_000,
            },
        },
    );
    plan.config.config.memory.packages.insert(
        "semantic-memory-test".into(),
        HarnessRuntimeMapping {
            runtime: "remote-memory".into(),
        },
    );
    let (bridge, _, _) = buffered_machine_bridge();
    let mut capabilities =
        custom_memory_capabilities("semantic-memory-test", "0.1.0", json!(["key"]));
    capabilities["space_models"] = json!(["document"]);
    bridge.register_host_service(&host_service("memory", "remote-memory"), capabilities);

    let mut runtime = runtime_snapshot_from_plan(&plan);
    let activation = activate_custom_memory_runtime_for_plan(&plan, &runtime, Some(bridge), None);
    assert!(activation.runtime.is_none());
    apply_custom_memory_activation_to_runtime(&mut runtime, &activation);

    assert_eq!(runtime.memory[0].runtime, "remote-memory");
    assert_eq!(runtime.memory[0].state, "unavailable");
    assert!(
        runtime.memory[0]
            .readiness_reason
            .as_deref()
            .unwrap_or_default()
            .contains("space model")
    );
}

#[test]
fn custom_memory_lifecycle_only_capability_gaps_do_not_suppress_direct_access() {
    let root = temp_dir("custom-memory-lifecycle-only-gaps");
    let mut plan = minimal_plan(&root);
    write_semantic_memory_fixture_with_modes(&root, &mut plan, json!(["key"]));
    plan.config.config.memory.runtimes.insert(
        "remote-memory".into(),
        HarnessImplementationEntry {
            implementation: HarnessImplementation::Host {
                request_timeout_ms: 1_000,
            },
        },
    );
    plan.config.config.memory.packages.insert(
        "semantic-memory-test".into(),
        HarnessRuntimeMapping {
            runtime: "remote-memory".into(),
        },
    );
    let (bridge, _, _) = buffered_machine_bridge();
    bridge.register_host_service(
        &host_service("memory", "remote-memory"),
        custom_memory_capabilities("semantic-memory-test", "0.1.0", json!(["key"])),
    );

    let mut runtime = runtime_snapshot_from_plan(&plan);
    let activation = activate_custom_memory_runtime_for_plan(&plan, &runtime, Some(bridge), None);
    assert!(activation.runtime.is_some());
    apply_custom_memory_activation_to_runtime(&mut runtime, &activation);

    assert_eq!(runtime.memory[0].runtime, "remote-memory");
    assert_eq!(runtime.memory[0].state, "available");
    assert_eq!(
        runtime.memory[0].retrieval_modes,
        vec![MemoryRetrievalMode::Key]
    );
    let descriptor = activation.capabilities.get("remote-memory").unwrap();
    assert!(!descriptor.durable_trigger_state);
    assert!(!descriptor.atomic_batches);
}

#[test]
fn custom_memory_one_bad_runtime_does_not_disable_healthy_sibling() {
    let root = temp_dir("custom-memory-one-bad-runtime");
    let mut plan = minimal_plan(&root);
    write_multi_memory_agent(&root, &["healthy-memory", "broken-memory"]);
    write_key_memory_package(&root, &mut plan, "healthy-memory");
    write_key_memory_package(&root, &mut plan, "broken-memory");
    plan.config.config.memory.runtimes.insert(
        "healthy-runtime".into(),
        HarnessImplementationEntry {
            implementation: HarnessImplementation::Host {
                request_timeout_ms: 1_000,
            },
        },
    );
    plan.config.config.memory.runtimes.insert(
        "broken-runtime".into(),
        HarnessImplementationEntry {
            implementation: HarnessImplementation::Host {
                request_timeout_ms: 1_000,
            },
        },
    );
    plan.config.config.memory.packages.insert(
        "healthy-memory".into(),
        HarnessRuntimeMapping {
            runtime: "healthy-runtime".into(),
        },
    );
    plan.config.config.memory.packages.insert(
        "broken-memory".into(),
        HarnessRuntimeMapping {
            runtime: "broken-runtime".into(),
        },
    );
    let (bridge, _, _) = buffered_machine_bridge();
    bridge.register_host_service(
        &host_service("memory", "healthy-runtime"),
        custom_memory_capabilities("healthy-memory", "0.1.0", json!(["key"])),
    );

    let mut runtime = runtime_snapshot_from_plan(&plan);
    let activation = activate_custom_memory_runtime_for_plan(&plan, &runtime, Some(bridge), None);
    assert!(activation.runtime.is_some());
    apply_custom_memory_activation_to_runtime(&mut runtime, &activation);

    let healthy = runtime
        .memory
        .iter()
        .find(|space| space.package == "healthy-memory")
        .unwrap();
    assert_eq!(healthy.runtime, "healthy-runtime");
    assert_eq!(healthy.state, "available");
    assert!(healthy.readiness_reason.is_none());

    let broken = runtime
        .memory
        .iter()
        .find(|space| space.package == "broken-memory")
        .unwrap();
    assert_eq!(broken.runtime, "broken-runtime");
    assert_eq!(broken.state, "unavailable");
    assert!(
        broken
            .readiness_reason
            .as_deref()
            .unwrap_or_default()
            .contains("host service is not registered")
    );
}

#[test]
fn custom_memory_host_registration_failure_emits_service_events() {
    let root = temp_dir("custom-memory-host-registration-events");
    let mut plan = minimal_plan(&root);
    write_semantic_memory_fixture_with_modes(&root, &mut plan, json!(["key"]));
    plan.config.config.memory.runtimes.insert(
        "remote-memory".into(),
        HarnessImplementationEntry {
            implementation: HarnessImplementation::Host {
                request_timeout_ms: 1_000,
            },
        },
    );
    plan.config.config.memory.packages.insert(
        "semantic-memory-test".into(),
        HarnessRuntimeMapping {
            runtime: "remote-memory".into(),
        },
    );
    let (bridge, _, _) = buffered_machine_bridge();
    let mut runtime = runtime_snapshot_from_plan(&plan);
    let mut service_events = ServiceLifecycleEvents::new();

    let activation = activate_custom_memory_runtime_for_plan(
        &plan,
        &runtime,
        Some(bridge),
        Some(&service_events),
    );
    assert!(activation.runtime.is_none());
    apply_custom_memory_activation_to_runtime(&mut runtime, &activation);
    assert_eq!(runtime.memory[0].state, "unavailable");
    assert!(
        runtime.memory[0]
            .readiness_reason
            .as_deref()
            .unwrap_or_default()
            .contains("host service is not registered")
    );

    let events = service_events.drain();
    assert!(events.iter().any(|event| {
        event.event_type == crate::harness_observability::HarnessEventType::ServiceUnhealthy
            && event.service == "memory"
            && event.registry_id == "remote-memory"
            && event.message.contains("host service is not registered")
    }));
    assert!(events.iter().any(|event| {
        event.event_type == crate::harness_observability::HarnessEventType::ServiceFailed
            && event.service == "memory"
            && event.registry_id == "remote-memory"
            && event.message.contains("host service is not registered")
    }));
}

#[test]
fn custom_memory_host_capability_failure_emits_service_events() {
    let root = temp_dir("custom-memory-host-capability-events");
    let mut plan = minimal_plan(&root);
    write_semantic_memory_fixture_with_modes(&root, &mut plan, json!(["key"]));
    plan.config.config.memory.runtimes.insert(
        "remote-memory".into(),
        HarnessImplementationEntry {
            implementation: HarnessImplementation::Host {
                request_timeout_ms: 1_000,
            },
        },
    );
    plan.config.config.memory.packages.insert(
        "semantic-memory-test".into(),
        HarnessRuntimeMapping {
            runtime: "remote-memory".into(),
        },
    );
    let (bridge, _, _) = buffered_machine_bridge();
    bridge.register_host_service(
        &host_service("memory", "remote-memory"),
        json!({
            "ready": false,
            "capabilities": {
                "space_models": ["collection"],
                "retrieval_modes": ["key"],
                "retention_actions": [],
                "constraints": [],
                "capacity": false,
                "durable_trigger_state": false,
                "atomic_batches": false
            }
        }),
    );
    let mut runtime = runtime_snapshot_from_plan(&plan);
    let mut service_events = ServiceLifecycleEvents::new();

    let activation = activate_custom_memory_runtime_for_plan(
        &plan,
        &runtime,
        Some(bridge),
        Some(&service_events),
    );
    assert!(activation.runtime.is_none());
    apply_custom_memory_activation_to_runtime(&mut runtime, &activation);
    assert_eq!(runtime.memory[0].state, "unavailable");
    assert!(
        runtime.memory[0]
            .readiness_reason
            .as_deref()
            .unwrap_or_default()
            .contains("ready=false")
    );

    let events = service_events.drain();
    assert!(events.iter().any(|event| {
        event.event_type == crate::harness_observability::HarnessEventType::ServiceUnhealthy
            && event.service == "memory"
            && event.registry_id == "remote-memory"
            && event.message.contains("ready=false")
    }));
    assert!(events.iter().any(|event| {
        event.event_type == crate::harness_observability::HarnessEventType::ServiceFailed
            && event.service == "memory"
            && event.registry_id == "remote-memory"
            && event.message.contains("ready=false")
    }));
}

#[test]
fn memory_host_registration_accepts_root_level_capability_descriptor() {
    let root = temp_dir("memory-host-root-capabilities");
    let mut plan = minimal_plan(&root);
    write_semantic_memory_fixture_with_modes(&root, &mut plan, json!(["key"]));
    plan.config.config.memory.runtimes.insert(
        "remote-memory".into(),
        HarnessImplementationEntry {
            implementation: HarnessImplementation::Host {
                request_timeout_ms: 1_000,
            },
        },
    );
    plan.config.config.memory.packages.insert(
        "semantic-memory-test".into(),
        HarnessRuntimeMapping {
            runtime: "remote-memory".into(),
        },
    );
    let (bridge, _, _) = buffered_machine_bridge();
    let mut payload = custom_memory_capabilities("semantic-memory-test", "0.1.0", json!(["key"]));
    payload["role"] = json!("memory");
    payload["registry_id"] = json!("remote-memory");

    let service = register_host_service(&plan, &bridge, &payload).unwrap();
    assert_eq!(service, host_service("memory", "remote-memory"));
    let mut runtime = runtime_snapshot_from_plan(&plan);
    let activation = activate_custom_memory_runtime_for_plan(&plan, &runtime, Some(bridge), None);
    assert!(activation.runtime.is_some());
    apply_custom_memory_activation_to_runtime(&mut runtime, &activation);
    assert_eq!(runtime.memory[0].state, "available");
}

#[test]
fn custom_memory_process_activation_accepts_ready_descriptor() {
    let root = temp_dir("custom-memory-process-ready");
    let mut plan = minimal_plan(&root);
    write_semantic_memory_fixture_with_modes(&root, &mut plan, json!(["key"]));
    let script = root.join("memory_service.py");
    fs::write(
        &script,
        r#"
import json, sys
for line in sys.stdin:
    msg = json.loads(line)
    assert msg["service"] == "memory"
    assert msg["payload"]["role"] == "memory"
    assert msg["payload"]["registry_id"] == "remote-memory"
    result = {
        "ready": True,
        "registry_id": "remote-memory",
        "protocol_version": 1,
        "capabilities": {
            "space_models": ["collection"],
            "retrieval_modes": ["key"],
            "retention_actions": [],
            "constraints": [],
            "capacity": False,
            "durable_trigger_state": False,
            "atomic_batches": False,
            "packages": [
                { "package": "semantic-memory-test", "version": "0.1.0", "ready": True }
            ]
        }
    }
    print(json.dumps({
        "protocol": "agentpm-service",
        "version": 1,
        "kind": "initialized",
        "id": msg.get("id"),
        "service": "memory",
        "result": result
    }), flush=True)
"#,
    )
    .unwrap();
    plan.config.config.memory.runtimes.insert(
        "remote-memory".into(),
        HarnessImplementationEntry {
            implementation: HarnessImplementation::Process {
                command: "python3".into(),
                args: vec![script.display().to_string()],
                cwd: None,
                env: Vec::new(),
                startup_timeout_ms: 1_000,
                request_timeout_ms: 1_000,
                restart: Default::default(),
            },
        },
    );
    plan.config.config.memory.packages.insert(
        "semantic-memory-test".into(),
        HarnessRuntimeMapping {
            runtime: "remote-memory".into(),
        },
    );

    let mut runtime = runtime_snapshot_from_plan(&plan);
    let activation = activate_custom_memory_runtime_for_plan(&plan, &runtime, None, None);
    assert!(activation.runtime.is_some());
    apply_custom_memory_activation_to_runtime(&mut runtime, &activation);

    assert_eq!(runtime.memory[0].state, "available");
    assert_eq!(runtime.memory[0].runtime, "remote-memory");
}

#[test]
fn mapped_knowledge_runtime_readiness_requires_realizable_runtime() {
    let root = temp_dir("mapped-knowledge-runtime-readiness");
    let knowledge_root = root.join(".agentpm/knowledge/@zack/guide/0.1.0");
    write_json(
        &knowledge_root.join("agent.json"),
        json!({
            "kind": "knowledge",
            "name": "@zack/guide",
            "version": "0.1.0",
            "description": "Guide.",
            "knowledge": {
                "mode": "context",
                "content_type": "text/markdown",
                "documents": [
                    { "path": "knowledge/docs/guide.md", "content_type": "text/markdown" }
                ]
            }
        }),
    );
    let mut plan = minimal_plan(&root);
    plan.package_graph.insert(
        "knowledge:@zack/guide@0.1.0".into(),
        ResolvedPackageInfo {
            key: "knowledge:@zack/guide@0.1.0".into(),
            kind: PackageKind::Knowledge,
            name: "@zack/guide".into(),
            version: "0.1.0".into(),
            root: knowledge_root,
        },
    );
    plan.capabilities
        .push(crate::harness_plan::StaticCapabilityCandidate {
            kind: "knowledge".into(),
            identity: "@zack/guide".into(),
            scope: "global".into(),
            source: "agent_binding".into(),
            state: CapabilityState::Available,
        });
    plan.capabilities
        .push(crate::harness_plan::StaticCapabilityCandidate {
            kind: "knowledge_runtime".into(),
            identity: "remote-knowledge".into(),
            scope: "session".into(),
            source: "harness_config".into(),
            state: CapabilityState::Unavailable,
        });
    plan.config.config.knowledge.runtimes.insert(
        "remote-knowledge".into(),
        HarnessImplementationEntry {
            implementation: HarnessImplementation::Host {
                request_timeout_ms: 1_000,
            },
        },
    );
    plan.config.config.knowledge.packages.insert(
        "@zack/guide".into(),
        HarnessRuntimeMapping {
            runtime: "remote-knowledge".into(),
        },
    );

    let runtime = runtime_snapshot_from_plan(&plan);
    assert_eq!(runtime.knowledge.len(), 1);
    assert_eq!(runtime.knowledge[0].runtime, "remote-knowledge");
    assert_eq!(runtime.knowledge[0].state, "unavailable");
    assert!(
        runtime.knowledge[0]
            .readiness_reason
            .as_deref()
            .unwrap_or_default()
            .contains("configured KnowledgeRuntime `remote-knowledge` is unavailable")
    );
}

#[test]
fn custom_knowledge_activation_failure_suppresses_mapped_package() {
    let root = temp_dir("custom-knowledge-activation-failure");
    let knowledge_root = root.join(".agentpm/knowledge/@zack/guide/0.1.0");
    write_json(
        &knowledge_root.join("agent.json"),
        json!({
            "kind": "knowledge",
            "name": "@zack/guide",
            "version": "0.1.0",
            "description": "Guide.",
            "knowledge": {
                "mode": "context",
                "content_type": "text/markdown",
                "documents": [
                    { "path": "knowledge/docs/guide.md", "content_type": "text/markdown" }
                ]
            }
        }),
    );
    let mut plan = minimal_plan(&root);
    plan.package_graph.insert(
        "knowledge:@zack/guide@0.1.0".into(),
        ResolvedPackageInfo {
            key: "knowledge:@zack/guide@0.1.0".into(),
            kind: PackageKind::Knowledge,
            name: "@zack/guide".into(),
            version: "0.1.0".into(),
            root: knowledge_root,
        },
    );
    plan.capabilities
        .push(crate::harness_plan::StaticCapabilityCandidate {
            kind: "knowledge".into(),
            identity: "@zack/guide".into(),
            scope: "global".into(),
            source: "agent_binding".into(),
            state: CapabilityState::Available,
        });
    plan.capabilities
        .push(crate::harness_plan::StaticCapabilityCandidate {
            kind: "knowledge_runtime".into(),
            identity: "remote-knowledge".into(),
            scope: "session".into(),
            source: "harness_config".into(),
            state: CapabilityState::Available,
        });
    plan.config.config.knowledge.runtimes.insert(
        "remote-knowledge".into(),
        HarnessImplementationEntry {
            implementation: HarnessImplementation::Process {
                command: "__agentpm_missing_knowledge_runtime__".into(),
                args: Vec::new(),
                cwd: None,
                env: Vec::new(),
                startup_timeout_ms: 100,
                request_timeout_ms: 100,
                restart: Default::default(),
            },
        },
    );
    plan.config.config.knowledge.packages.insert(
        "@zack/guide".into(),
        HarnessRuntimeMapping {
            runtime: "remote-knowledge".into(),
        },
    );

    let mut runtime = runtime_snapshot_from_plan(&plan);
    assert_eq!(runtime.knowledge.len(), 1);
    assert_eq!(runtime.knowledge[0].state, "available");

    let activation = activate_custom_knowledge_runtime_for_plan(&plan, &runtime, None, None);
    assert!(activation.runtime.is_none());
    apply_custom_knowledge_activation_to_runtime(&mut runtime, &activation);

    assert_eq!(runtime.knowledge[0].runtime, "remote-knowledge");
    assert_eq!(runtime.knowledge[0].state, "unavailable");
    assert!(
        runtime.knowledge[0]
            .readiness_reason
            .as_deref()
            .unwrap_or_default()
            .contains("configured KnowledgeRuntime `remote-knowledge` could not start")
    );
}

#[test]
fn custom_knowledge_activation_isolates_unhealthy_runtime() {
    let root = temp_dir("custom-knowledge-activation-isolates-runtime");
    let healthy_root = root.join(".agentpm/knowledge/@zack/healthy/0.1.0");
    let unhealthy_root = root.join(".agentpm/knowledge/@zack/unhealthy/0.1.0");
    for (package_root, name, description) in [
        (&healthy_root, "@zack/healthy", "Healthy guide."),
        (&unhealthy_root, "@zack/unhealthy", "Unhealthy guide."),
    ] {
        write_json(
            &package_root.join("agent.json"),
            json!({
                "kind": "knowledge",
                "name": name,
                "version": "0.1.0",
                "description": description,
                "knowledge": {
                    "mode": "context",
                    "content_type": "text/markdown",
                    "documents": [
                        { "path": "knowledge/docs/guide.md", "content_type": "text/markdown" }
                    ]
                }
            }),
        );
    }
    let mut plan = minimal_plan(&root);
    for (name, package_root) in [
        ("@zack/healthy", healthy_root),
        ("@zack/unhealthy", unhealthy_root),
    ] {
        plan.package_graph.insert(
            format!("knowledge:{name}@0.1.0"),
            ResolvedPackageInfo {
                key: format!("knowledge:{name}@0.1.0"),
                kind: PackageKind::Knowledge,
                name: name.into(),
                version: "0.1.0".into(),
                root: package_root,
            },
        );
        plan.capabilities
            .push(crate::harness_plan::StaticCapabilityCandidate {
                kind: "knowledge".into(),
                identity: name.into(),
                scope: "global".into(),
                source: "agent_binding".into(),
                state: CapabilityState::Available,
            });
    }
    for runtime_id in ["healthy-knowledge", "unhealthy-knowledge"] {
        plan.capabilities
            .push(crate::harness_plan::StaticCapabilityCandidate {
                kind: "knowledge_runtime".into(),
                identity: runtime_id.into(),
                scope: "session".into(),
                source: "harness_config".into(),
                state: CapabilityState::Available,
            });
    }
    plan.config.config.knowledge.runtimes.insert(
        "healthy-knowledge".into(),
        HarnessImplementationEntry {
            implementation: HarnessImplementation::Host {
                request_timeout_ms: 1_000,
            },
        },
    );
    plan.config.config.knowledge.runtimes.insert(
        "unhealthy-knowledge".into(),
        HarnessImplementationEntry {
            implementation: HarnessImplementation::Process {
                command: "__agentpm_missing_knowledge_runtime__".into(),
                args: Vec::new(),
                cwd: None,
                env: Vec::new(),
                startup_timeout_ms: 100,
                request_timeout_ms: 100,
                restart: Default::default(),
            },
        },
    );
    plan.config.config.knowledge.packages.insert(
        "@zack/healthy".into(),
        HarnessRuntimeMapping {
            runtime: "healthy-knowledge".into(),
        },
    );
    plan.config.config.knowledge.packages.insert(
        "@zack/unhealthy".into(),
        HarnessRuntimeMapping {
            runtime: "unhealthy-knowledge".into(),
        },
    );
    let (bridge, _, _) = buffered_machine_bridge();
    bridge.register_host_service(
        &host_service("knowledge", "healthy-knowledge"),
        json!({
            "ready": true,
            "registry_id": "healthy-knowledge",
            "modes": ["context_document"],
            "features": [],
            "packages": [
                {
                    "package": "@zack/healthy",
                    "version": "0.1.0",
                    "ready": true
                }
            ]
        }),
    );

    let mut runtime = runtime_snapshot_from_plan(&plan);
    let activation =
        activate_custom_knowledge_runtime_for_plan(&plan, &runtime, Some(bridge), None);
    assert!(activation.runtime.is_some());
    apply_custom_knowledge_activation_to_runtime(&mut runtime, &activation);

    let healthy = runtime
        .knowledge
        .iter()
        .find(|package| package.name == "@zack/healthy")
        .unwrap();
    assert_eq!(healthy.runtime, "healthy-knowledge");
    assert_eq!(healthy.state, "available");
    assert!(healthy.readiness_reason.is_none());

    let unhealthy = runtime
        .knowledge
        .iter()
        .find(|package| package.name == "@zack/unhealthy")
        .unwrap();
    assert_eq!(unhealthy.runtime, "unhealthy-knowledge");
    assert_eq!(unhealthy.state, "unavailable");
    assert!(
        unhealthy
            .readiness_reason
            .as_deref()
            .unwrap_or_default()
            .contains("configured KnowledgeRuntime `unhealthy-knowledge` could not start")
    );
}

#[test]
fn memory_semantic_retrieval_advertises_only_when_embedding_provider_ready() {
    let root = temp_dir("memory-semantic-readiness");
    let mut plan = minimal_plan(&root);
    write_semantic_memory_fixture(&root, &mut plan);

    let runtime = runtime_snapshot_from_plan(&plan);
    assert_eq!(runtime.memory.len(), 1);
    assert_eq!(runtime.memory[0].state, "unavailable");
    assert!(runtime.memory[0].semantic.is_none());
    assert!(runtime.memory[0].retrieval_modes.is_empty());
    assert!(
        runtime.memory[0]
            .readiness_reason
            .as_deref()
            .unwrap_or_default()
            .contains("memory.local.semantic configuration")
    );

    plan.config.config.memory.local.semantic = Some(HarnessMemorySemanticConfig {
        embedding_provider: "test-embedder".into(),
        model: "toy-2d".into(),
        dimensions: 2,
    });
    let runtime = runtime_snapshot_from_plan(&plan);
    assert_eq!(runtime.memory[0].state, "unavailable");
    assert!(runtime.memory[0].semantic.is_none());
    assert!(
        runtime.memory[0]
            .readiness_reason
            .as_deref()
            .unwrap_or_default()
            .contains("undefined EmbeddingProvider `test-embedder`")
    );

    plan.config.config.providers.embeddings.insert(
        "test-embedder".into(),
        HarnessImplementationEntry {
            implementation: HarnessImplementation::Host {
                request_timeout_ms: 1_000,
            },
        },
    );
    plan.capabilities
        .push(crate::harness_plan::StaticCapabilityCandidate {
            kind: "embedding_provider".into(),
            identity: "test-embedder".into(),
            scope: "session".into(),
            source: "harness_config".into(),
            state: CapabilityState::Available,
        });
    let runtime = runtime_snapshot_from_plan(&plan);
    assert_eq!(runtime.memory[0].state, "available");
    assert_eq!(
        runtime.memory[0].retrieval_modes,
        vec![MemoryRetrievalMode::Semantic]
    );
    let semantic = runtime.memory[0].semantic.as_ref().unwrap();
    assert_eq!(semantic.provider, "test-embedder");
    assert_eq!(semantic.model, "toy-2d");
    assert_eq!(semantic.dimensions, 2);
    assert_eq!(semantic.metric, "cosine");
    assert!(semantic.normalized);
}

#[test]
fn memory_semantic_retrieval_degrades_to_supported_modes_without_embedding_provider() {
    let root = temp_dir("memory-semantic-degrades-to-key");
    let mut plan = minimal_plan(&root);
    write_semantic_memory_fixture_with_modes(&root, &mut plan, json!(["key", "semantic"]));

    let runtime = runtime_snapshot_from_plan(&plan);
    assert_eq!(runtime.memory.len(), 1);
    assert_eq!(runtime.memory[0].state, "available");
    assert!(runtime.memory[0].readiness_reason.is_none());
    assert!(runtime.memory[0].semantic.is_none());
    assert_eq!(
        runtime.memory[0].retrieval_modes,
        vec![MemoryRetrievalMode::Key]
    );
}

#[test]
fn knowledge_snapshot_uses_resolved_package_identity_for_scoped_installs() {
    let root = temp_dir("knowledge-snapshot-scoped-installed-package");
    let knowledge_root = root.join(".agentpm/knowledge/zack/guide/0.1.0");
    fs::create_dir_all(knowledge_root.join("knowledge/docs")).unwrap();
    fs::write(
        knowledge_root.join("knowledge/docs/guide.md"),
        "# Guide\n\nScoped installed Knowledge package.\n",
    )
    .unwrap();
    write_json(
        &knowledge_root.join("agent.json"),
        json!({
            "kind": "knowledge",
            "name": "guide",
            "version": "0.1.0",
            "description": "Schema-valid unscoped Knowledge manifest.",
            "knowledge": {
                "mode": "context",
                "documents": [
                    { "path": "knowledge/docs/guide.md", "content_type": "text/markdown" }
                ]
            }
        }),
    );
    crate::commands::knowledge::execute_knowledge_build(
        &knowledge_root.join("agent.json"),
        crate::commands::knowledge::KnowledgeBuildMode::Write,
    )
    .unwrap();

    let mut plan = minimal_plan(&root);
    plan.package_graph.insert(
        "knowledge:@zack/guide@0.1.0".into(),
        crate::harness_plan::ResolvedPackageInfo {
            key: "knowledge:@zack/guide@0.1.0".into(),
            kind: PackageKind::Knowledge,
            name: "@zack/guide".into(),
            version: "0.1.0".into(),
            root: knowledge_root,
        },
    );
    plan.capabilities
        .push(crate::harness_plan::StaticCapabilityCandidate {
            kind: "knowledge".into(),
            identity: "@zack/guide".into(),
            scope: "phase:research".into(),
            source: "agent_binding".into(),
            state: CapabilityState::Available,
        });

    let snapshots = knowledge_snapshots_from_plan(&plan);
    assert_eq!(snapshots.len(), 1);
    assert_eq!(snapshots[0].name, "@zack/guide");
    assert_eq!(snapshots[0].version, "0.1.0");
    assert_eq!(snapshots[0].state, "available");
    assert_eq!(snapshots[0].mode, "context");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn machine_registration_accepts_unconfigured_sdk_hooks_and_approval() {
    let root = temp_dir("sdk-host-service-registration");
    let plan = minimal_plan(&root);
    let (bridge, _, _) = buffered_machine_bridge();

    let hook_service = register_host_service(
        &plan,
        &bridge,
        &json!({
            "role": "hook",
            "registry_id": "sdk-hooks",
            "hooks": ["before_tool_call", "before_model_request"]
        }),
    )
    .unwrap();
    let approval_service = register_host_service(
        &plan,
        &bridge,
        &json!({
            "role": "approval",
            "registry_id": "controller"
        }),
    )
    .unwrap();

    assert_eq!(hook_service, host_service("hook", "sdk-hooks"));
    assert_eq!(approval_service, host_service("approval", "controller"));
    assert!(bridge.has_host_service(&hook_service));
    assert!(bridge.has_host_service(&approval_service));
    assert!(bridge.has_sdk_approval_controller());
    let hooks = bridge.sdk_host_hooks();
    assert_eq!(hooks.len(), 2);
    assert!(
        hooks
            .iter()
            .any(|hook| hook.registry_id == "sdk-hooks"
                && hook.hook == HarnessHookId::BeforeToolCall)
    );
    assert!(
        hooks.iter().any(|hook| hook.registry_id == "sdk-hooks"
            && hook.hook == HarnessHookId::BeforeModelRequest)
    );
}

#[test]
fn machine_registration_rejects_unconfigured_host_provider() {
    let root = temp_dir("unconfigured-host-provider-registration");
    let plan = minimal_plan(&root);
    let (bridge, _, _) = buffered_machine_bridge();

    let err = register_host_service(
        &plan,
        &bridge,
        &json!({
            "role": "model",
            "registry_id": "sdk-model"
        }),
    )
    .unwrap_err();

    assert!(err.contains("is not configured"));
}

#[test]
fn host_registration_response_marks_runtime_roles_active() {
    let embedding = host_service_registration_response(&host_service("embedding", "embedder"));
    assert_eq!(embedding["registered"], json!(true));
    assert_eq!(embedding["active"], json!(true));
    assert!(embedding["reason"].is_null());

    let knowledge = host_service_registration_response(&host_service("knowledge", "kb"));
    assert_eq!(knowledge["active"], json!(true));
    assert!(knowledge["reason"].is_null());

    let memory = host_service_registration_response(&host_service("memory", "store"));
    assert_eq!(memory["active"], json!(true));
    assert!(memory["reason"].is_null());

    let model = host_service_registration_response(&host_service("model", "host-model"));
    assert_eq!(model["active"], json!(true));
    assert!(model["reason"].is_null());
}

#[test]
fn host_model_runtime_uses_machine_host_service_contract() {
    let selection = ModelProviderSelection {
        provider: "host-model".into(),
        model: "model-1".into(),
        options: json!({}),
    };
    let expected_turn = ModelTurn {
        assistant_content: Some("from host".into()),
        actions: Vec::new(),
        usage: RunUsage::default(),
        finish_reason: Some("stop".into()),
        provider_metadata: BTreeMap::new(),
    };
    let mut runtime = HostModelRuntime {
        selection: selection.clone(),
        invoker: Box::new(FakeHostInvoker {
            response: serde_json::to_value(&expected_turn).unwrap(),
            capabilities: None,
        }),
        capabilities: host_model_capabilities_from_registration(
            &host_model_capabilities(),
            "host-model",
            "model-1",
        )
        .unwrap(),
        request_timeout_ms: 1_000,
    };

    let turn = runtime.generate(empty_model_request(selection)).unwrap();
    assert_eq!(turn, expected_turn);
}

#[test]
fn host_model_runtime_defaults_missing_or_partial_usage() {
    let selection = ModelProviderSelection {
        provider: "host-model".into(),
        model: "model-1".into(),
        options: json!({}),
    };
    let capabilities = host_model_capabilities_from_registration(
        &host_model_capabilities(),
        "host-model",
        "model-1",
    )
    .unwrap();
    let mut missing_usage = HostModelRuntime {
        selection: selection.clone(),
        invoker: Box::new(FakeHostInvoker {
            response: json!({
                "assistant_content": "from host",
                "actions": [],
                "finish_reason": "stop",
                "provider_metadata": {}
            }),
            capabilities: None,
        }),
        capabilities: capabilities.clone(),
        request_timeout_ms: 1_000,
    };
    let turn = missing_usage
        .generate(empty_model_request(selection.clone()))
        .unwrap();
    assert_eq!(turn.usage, RunUsage::default());

    let mut partial_usage = HostModelRuntime {
        selection: selection.clone(),
        invoker: Box::new(FakeHostInvoker {
            response: json!({
                "assistant_content": "from host",
                "actions": [],
                "usage": {
                    "tokens": {
                        "input_tokens": 7
                    },
                    "embedding_requests": 2
                }
            }),
            capabilities: None,
        }),
        capabilities,
        request_timeout_ms: 1_000,
    };
    let turn = partial_usage
        .generate(empty_model_request(selection.clone()))
        .unwrap();
    assert_eq!(turn.usage.tokens.input_tokens, Some(7));
    assert_eq!(turn.usage.tokens.output_tokens, None);
    assert_eq!(turn.usage.tokens.total_tokens, None);
    assert_eq!(turn.usage.embedding_requests, 2);
    assert_eq!(turn.usage.knowledge_requests, 0);
    assert_eq!(
        turn.usage.cost,
        crate::harness_observability::CostUsage::default()
    );
}

#[test]
fn host_model_runtime_uses_registered_capability_advertisement() {
    let root = temp_dir("host-model-capability-advertisement");
    let mut plan = minimal_plan(&root);
    plan.config.config.model = Some(crate::harness_config::HarnessModelConfig {
        provider: "host-model".into(),
        model: "model-1".into(),
        options: json!({}),
    });
    plan.config.config.providers.models.insert(
        "host-model".into(),
        HarnessImplementationEntry {
            implementation: HarnessImplementation::Host {
                request_timeout_ms: 1_000,
            },
        },
    );
    let (bridge, _, _) = buffered_machine_bridge();
    register_host_service(
        &plan,
        &bridge,
        &json!({
            "role": "model",
            "registry_id": "host-model",
            "capabilities": {
                "provider": "host-model",
                "model": "model-1",
                "semantic_actions": false,
                "structured_output": true,
                "multimodal_input": false,
                "usage_reporting": true
            }
        }),
    )
    .unwrap();

    let runtime = model_runtime_from_plan(
        &plan,
        ModelProviderSelection {
            provider: "host-model".into(),
            model: "model-1".into(),
            options: json!({}),
        },
        Some(Box::new(bridge)),
        None,
    )
    .unwrap();
    let err = validate_model_capabilities(runtime.as_ref()).unwrap_err();
    assert!(err.to_string().contains("semantic action support"));
}

#[test]
fn host_model_capabilities_reject_mismatched_model_identity() {
    let err = host_model_capabilities_from_registration(
        &json!({
            "provider": "host-model",
            "model": "other-model",
            "semantic_actions": true,
            "structured_output": true,
            "multimodal_input": false,
            "usage_reporting": true
        }),
        "host-model",
        "model-1",
    )
    .unwrap_err();
    assert!(err.to_string().contains("expected `model-1`"));
}

#[test]
fn host_service_registration_rejects_not_ready() {
    let root = temp_dir("host-service-not-ready");
    let plan = minimal_plan(&root);
    let (bridge, _sender, _output) = buffered_machine_bridge();

    let err = register_host_service(
        &plan,
        &bridge,
        &json!({
            "role": "approval",
            "registry_id": "controller",
            "ready": false
        }),
    )
    .unwrap_err();

    assert!(err.contains("reported not ready"));
}

#[test]
fn configured_host_hook_registration_validates_advertised_hooks() {
    let root = temp_dir("configured-host-hook-registration");
    let mut plan = minimal_plan(&root);
    plan.config.config.hooks.implementations.insert(
        "host-hooks".into(),
        HarnessImplementationEntry {
            implementation: HarnessImplementation::Host {
                request_timeout_ms: 1_000,
            },
        },
    );
    plan.config.config.hooks.bindings.push(HarnessHookBinding {
        hook: HarnessHookId::BeforeToolCall,
        implementation: "host-hooks".into(),
        failure_policy: HarnessHookFailurePolicy::Closed,
    });
    let (bridge, _sender, _output) = buffered_machine_bridge();

    let err = register_host_service(
        &plan,
        &bridge,
        &json!({
            "role": "hook",
            "registry_id": "host-hooks",
            "hooks": ["before_model_request"]
        }),
    )
    .unwrap_err();
    assert!(err.contains("does not advertise configured hook `before_tool_call`"));

    register_host_service(
        &plan,
        &bridge,
        &json!({
            "role": "hook",
            "registry_id": "host-hooks",
            "capabilities": {
                "hooks": ["before_tool_call"]
            }
        }),
    )
    .unwrap();
}

#[test]
fn configured_host_approval_rejects_missing_request_capability() {
    let controller = HarnessApprovalController {
        implementation: HarnessImplementation::Host {
            request_timeout_ms: 1_000,
        },
    };

    let err = match ConfiguredApprovalController::host(
        &controller,
        None,
        Box::new(FakeHostInvoker {
            response: json!({ "decision": "approve" }),
            capabilities: Some(json!({
                "approval": false
            })),
        }),
    ) {
        Ok(_) => panic!("host approval controller should reject missing request capability"),
        Err(err) => err,
    };

    assert!(
        err.to_string()
            .contains("does not advertise request_approval support")
    );
}

#[test]
fn host_service_request_frames_bypass_trace_content_redaction() {
    let writer = MachineProtocolWriter::stdout(HarnessTraceContent::Redacted);
    let host_request = MachineEnvelope {
        protocol: AGENTPM_HARNESS_MACHINE_PROTOCOL.into(),
        version: AGENTPM_HARNESS_MACHINE_VERSION,
        kind: MachineFrameKind::Request,
        id: Some("host-hook-1".into()),
        method: Some("host_service".into()),
        payload: json!({
            "role": "hook",
            "registry_id": "host-hooks",
            "method": "before_tool_call",
            "payload": {
                "hook": "before_tool_call",
                "input": {
                    "phase_id": "classify",
                    "tool": "@zack/search",
                    "arguments": {
                        "query": "visible to host implementation"
                    }
                }
            }
        }),
        error: None,
    };

    let redacted = writer.frame_value(host_request.clone(), true).unwrap();
    assert_eq!(redacted["payload"]["payload"]["input"], json!("[redacted]"));

    let unredacted = writer.frame_value(host_request, false).unwrap();
    assert_eq!(
        unredacted["payload"]["payload"]["input"]["arguments"]["query"],
        json!("visible to host implementation")
    );
}

#[test]
fn machine_bridge_rejects_start_run_while_active_without_blocking_host_service_response() {
    let (bridge, sender, output) = buffered_machine_bridge();
    bridge.register_host_service(
        &host_service("model", "host-model"),
        host_model_capabilities(),
    );
    bridge.set_active_run(true);
    let mut bridge_for_thread = bridge.clone();
    let waiter = std::thread::spawn(move || {
        bridge_for_thread.invoke_host_service(
            "model",
            "host-model",
            "generate",
            json!({ "input": "visible" }),
            1_000,
        )
    });

    sender
        .send(Ok(machine_request(
            "start-while-active",
            "start_run",
            json!({ "input": "second run" }),
        )))
        .unwrap();
    sender
        .send(Ok(machine_response(
            "host-model-host-model-1",
            json!({ "ok": true }),
        )))
        .unwrap();

    assert_eq!(waiter.join().unwrap().unwrap(), json!({ "ok": true }));
    let frames = machine_frames_from_buffer(&output);
    assert!(frames.iter().any(|frame| {
        frame["id"] == "start-while-active"
            && frame["kind"] == "error"
            && frame["error"]["code"] == "session_busy"
    }));
}

#[test]
fn machine_bridge_cancel_run_interrupts_active_host_service_wait() {
    let (bridge, sender, output) = buffered_machine_bridge();
    bridge.register_host_service(
        &host_service("model", "host-model"),
        host_model_capabilities(),
    );
    bridge.set_active_run(true);
    let mut bridge_for_thread = bridge.clone();
    let waiter = std::thread::spawn(move || {
        bridge_for_thread.invoke_host_service(
            "model",
            "host-model",
            "generate",
            json!({ "input": "visible" }),
            1_000,
        )
    });

    sender
        .send(Ok(machine_request("cancel-1", "cancel_run", json!({}))))
        .unwrap();

    let err = waiter.join().unwrap().unwrap_err();
    assert!(err.to_string().contains("run cancellation requested"));
    assert!(bridge.cancellation_token().load(Ordering::SeqCst));
    let frames = machine_frames_from_buffer(&output);
    assert!(frames.iter().any(|frame| {
        frame["id"] == "cancel-1"
            && frame["kind"] == "response"
            && frame["payload"]["accepted"] == true
    }));
}

#[test]
fn machine_bridge_emits_host_service_failure_events() {
    let (bridge, sender, _output) = buffered_machine_bridge();
    bridge.register_host_service(
        &host_service("model", "host-model"),
        host_model_capabilities(),
    );
    let mut service_events = ServiceLifecycleEvents::new();
    bridge.set_host_service_lifecycle_emitter(service_events.emitter());
    let mut bridge_for_thread = bridge.clone();
    let waiter = std::thread::spawn(move || {
        bridge_for_thread.invoke_host_service(
            "model",
            "host-model",
            "generate",
            json!({ "input": "visible" }),
            1_000,
        )
    });

    sender
        .send(Ok(machine_error(
            "host-model-host-model-1",
            "host_failure",
            "host model failed",
        )))
        .unwrap();

    let err = waiter.join().unwrap().unwrap_err();
    assert!(err.to_string().contains("host model failed"));
    let events = service_events.drain();
    assert!(events.iter().any(|event| {
        event.event_type == crate::harness_observability::HarnessEventType::ServiceUnhealthy
            && event.service == "model"
            && event.registry_id == "host-model"
    }));
    assert!(events.iter().any(|event| {
        event.event_type == crate::harness_observability::HarnessEventType::ServiceFailed
            && event.service == "model"
            && event.registry_id == "host-model"
    }));
}

#[test]
fn machine_bridge_accepts_shutdown_control_request() {
    let (bridge, sender, output) = buffered_machine_bridge();
    sender
        .send(Ok(machine_request("shutdown-1", "shutdown", json!({}))))
        .unwrap();

    let request = bridge.recv_control_request().unwrap().unwrap();
    assert_eq!(request.method.as_deref(), Some("shutdown"));
    bridge
        .write_response(request.id.as_deref(), json!({ "shutdown": true }))
        .unwrap();

    let frames = machine_frames_from_buffer(&output);
    assert!(frames.iter().any(|frame| {
        frame["id"] == "shutdown-1"
            && frame["kind"] == "response"
            && frame["payload"]["shutdown"] == true
    }));
}

#[test]
fn host_approval_controller_decodes_machine_host_decision() {
    let controller = HarnessApprovalController {
        implementation: HarnessImplementation::Host {
            request_timeout_ms: 1_000,
        },
    };
    let mut runtime = ConfiguredApprovalController::host(
        &controller,
        None,
        Box::new(FakeHostInvoker {
            response: json!({ "decision": "deny" }),
            capabilities: None,
        }),
    )
    .unwrap()
    .unwrap();
    let decision = runtime.request_approval(&crate::manifest::LoopCheckpoint {
        id: "approve-review".into(),
        r#type: "approval".into(),
        before_phase: "review".into(),
        on_reject: "$handoff".into(),
    });
    assert_eq!(decision, crate::harness_runtime::ApprovalDecision::Deny);
}

#[test]
fn model_selection_requires_configured_model_for_headless_execution() {
    let root = temp_dir("missing-model-selection");
    let plan = minimal_plan(&root);
    let err = model_selection(&plan).unwrap_err();
    assert!(err.to_string().contains("requires model.provider"));
}

#[test]
fn failed_headless_terminal_status_includes_terminal_error_detail() {
    let terminal = RuntimeTerminalResult {
        status: HarnessTerminalStatus::Failed,
        output: Some(json!({ "error": "OPENAI_API_KEY is required for provider `openai`" })),
        report: minimal_run_report("run-1"),
    };
    let message = terminal_status_error_message(&terminal, HarnessTerminalStatus::Failed).unwrap();
    assert!(message.contains("terminal status Failed"));
    assert!(message.contains("OPENAI_API_KEY"));
}

#[test]
fn model_capability_validation_rejects_missing_semantic_or_structured_support() {
    let runtime = UnsupportedModelRuntime {
        semantic_actions: false,
        structured_output: true,
    };
    let err = validate_model_capabilities(&runtime).unwrap_err();
    assert!(err.to_string().contains("semantic action support"));

    let runtime = UnsupportedModelRuntime {
        semantic_actions: true,
        structured_output: false,
    };
    let err = validate_model_capabilities(&runtime).unwrap_err();
    assert!(err.to_string().contains("structured output support"));
}

#[tokio::test]
async fn headless_worker_constructs_blocking_provider_outside_tokio_runtime() {
    run_headless_worker(|| {
        let _runtime = BuiltInModelRuntime::from_selection(ModelProviderSelection {
            provider: "openai".into(),
            model: "gpt-4o-mini".into(),
            options: json!({}),
        })
        .map_err(|err| anyhow!(err.message))?;
        Ok(())
    })
    .unwrap();
}

#[test]
fn headless_execution_runs_one_engine_run_and_writes_report() {
    let root = temp_dir("headless-exec");
    let loop_root = root.join(".agentpm/loops/zack/review-loop/0.1.0");
    write_json(
        &loop_root.join("agent.json"),
        json!({
            "kind": "loop",
            "name": "@zack/review-loop",
            "version": "0.1.0",
            "loop": {
                "entry_phase": "respond",
                "phases": [
                    { "id": "respond", "objective": "Respond to the request." }
                ],
                "transitions": [
                    { "from": "respond", "on": "complete", "to": "$end" }
                ]
            }
        }),
    );
    let mut plan = minimal_plan(&root);
    plan.config.config.model = Some(crate::harness_config::HarnessModelConfig {
        provider: "ollama".into(),
        model: "test-model".into(),
        options: json!({}),
    });
    plan.config.config.trace = HarnessTraceConfig {
        enabled: true,
        level: HarnessTraceLevel::Verbose,
        content: HarnessTraceContent::Full,
    };
    plan.loop_package = Some(ResolvedPackageInfo {
        key: "loop:@zack/review-loop@0.1.0".into(),
        kind: PackageKind::Loop,
        name: "@zack/review-loop".into(),
        version: "0.1.0".into(),
        root: loop_root,
    });
    let report_path = root.join("custom-report.json");
    let mut model = ScriptedModelRuntime::new(vec![ModelTurn {
        assistant_content: Some("final response".into()),
        actions: Vec::new(),
        usage: RunUsage::default(),
        finish_reason: Some("stop".into()),
        provider_metadata: BTreeMap::new(),
    }]);
    let mut dispatcher = ScriptedActionDispatcher::default();
    let result = execute_headless_plan(
        &plan,
        "write a response".into(),
        Some(&report_path),
        &mut model,
        &mut dispatcher,
    )
    .unwrap();
    assert_eq!(result.status, HarnessTerminalStatus::Ended);
    assert_eq!(result.output, Some(json!("final response")));
    assert!(report_path.exists());
    let events_path = plan
        .state_dir
        .join("runs")
        .join(&result.report.run_id)
        .join("events.jsonl");
    assert_eq!(
        result.report.trace_path.as_deref(),
        Some(events_path.to_string_lossy().as_ref())
    );
    let report_json: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&report_path).unwrap()).unwrap();
    assert_eq!(
        report_json["trace_path"],
        events_path.to_string_lossy().as_ref()
    );
    let events = fs::read_to_string(&events_path).unwrap();
    assert!(events.contains("\"event_type\":\"run_started\""));
    assert!(events.contains("\"event_type\":\"run_completed\""));
    let parsed_events = events
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect::<Vec<serde_json::Value>>();
    let prompt_event = parsed_events
        .iter()
        .find(|event: &&serde_json::Value| event["event_type"] == "prompt_prepared")
        .unwrap();
    let prompt = prompt_event["payload"]["fields"]["prompt"]
        .as_str()
        .unwrap();
    assert!(prompt.contains("Harness authority"));
    assert!(prompt.contains("write a response"));
    let runtime_request = parsed_events
        .iter()
        .find(|event: &&serde_json::Value| event["event_type"] == "model_runtime_request_prepared")
        .unwrap();
    assert_eq!(
        runtime_request["payload"]["fields"]["request_kind"],
        "canonical_model_request"
    );
    assert_eq!(
        runtime_request["payload"]["fields"]["runtime_kind"],
        "scripted"
    );
    assert_eq!(runtime_request["payload"]["fields"]["provider"], "ollama");
    let runtime_prompt = runtime_request["payload"]["fields"]["prompt"]
        .as_str()
        .unwrap();
    assert!(runtime_prompt.contains("Harness authority"));
    assert!(runtime_prompt.contains("write a response"));
    let model_completed = parsed_events
        .iter()
        .find(|event: &&serde_json::Value| event["event_type"] == "model_request_completed")
        .unwrap();
    assert_eq!(
        model_completed["payload"]["fields"]["assistant_content"],
        "final response"
    );
    assert_eq!(
        model_completed["payload"]["fields"]["finish_reason"],
        "stop"
    );
    let phase_result = parsed_events
        .iter()
        .find(|event: &&serde_json::Value| event["event_type"] == "phase_result_ready")
        .unwrap();
    assert_eq!(phase_result["payload"]["output"], "final response");
    assert_eq!(model.requests.len(), 1);
    assert_eq!(model.requests[0].prompt.sections.len(), 6);
}

#[test]
fn headless_execution_runs_three_phase_loop_and_writes_report() {
    let root = temp_dir("headless-three-phase");
    let loop_root = root.join(".agentpm/loops/zack/review-loop/0.1.0");
    write_json(
        &loop_root.join("agent.json"),
        json!({
            "kind": "loop",
            "name": "@zack/review-loop",
            "version": "0.1.0",
            "loop": {
                "entry_phase": "assess",
                "phases": [
                    {
                        "id": "assess",
                        "objective": "Assess the request.",
                        "outcomes": [
                            { "id": "draft", "description": "Draft a response." }
                        ]
                    },
                    {
                        "id": "draft",
                        "objective": "Draft the response.",
                        "outcomes": [
                            { "id": "review", "description": "Review the response." }
                        ]
                    },
                    { "id": "review", "objective": "Review the response." }
                ],
                "transitions": [
                    { "from": "assess", "on": "draft", "to": "draft" },
                    { "from": "draft", "on": "review", "to": "review" },
                    { "from": "review", "on": "complete", "to": "$end" }
                ]
            }
        }),
    );
    let mut plan = minimal_plan(&root);
    plan.config.config.model = Some(crate::harness_config::HarnessModelConfig {
        provider: "ollama".into(),
        model: "test-model".into(),
        options: json!({}),
    });
    plan.loop_package = Some(ResolvedPackageInfo {
        key: "loop:@zack/review-loop@0.1.0".into(),
        kind: PackageKind::Loop,
        name: "@zack/review-loop".into(),
        version: "0.1.0".into(),
        root: loop_root,
    });
    let mut model = ScriptedModelRuntime::new(vec![
        phase_completion_turn(Some("draft"), Some(json!({ "assessment": "ok" }))),
        phase_completion_turn(Some("review"), Some(json!({ "draft": "ready" }))),
        phase_completion_turn(None, Some(json!({ "final": "approved" }))),
    ]);
    let mut dispatcher = ScriptedActionDispatcher::default();
    let result = execute_headless_plan(
        &plan,
        "prepare a response".into(),
        None,
        &mut model,
        &mut dispatcher,
    )
    .unwrap();

    assert_eq!(result.status, HarnessTerminalStatus::Ended);
    assert_eq!(result.output, Some(json!({ "final": "approved" })));
    assert_eq!(result.report.phase_summaries.len(), 3);
    assert_eq!(model.requests.len(), 3);
}

#[test]
fn headless_execution_reports_approval_required_terminal_status() {
    let root = temp_dir("headless-approval-required");
    let loop_root = root.join(".agentpm/loops/zack/review-loop/0.1.0");
    write_json(
        &loop_root.join("agent.json"),
        json!({
            "kind": "loop",
            "name": "@zack/review-loop",
            "version": "0.1.0",
            "loop": {
                "entry_phase": "assess",
                "checkpoints": [
                    {
                        "id": "approve-review",
                        "type": "approval",
                        "before_phase": "review",
                        "on_reject": "$handoff"
                    }
                ],
                "phases": [
                    {
                        "id": "assess",
                        "objective": "Assess the request.",
                        "outcomes": [
                            { "id": "review", "description": "Review the response." }
                        ]
                    },
                    { "id": "review", "objective": "Review the response." }
                ],
                "transitions": [
                    { "from": "assess", "on": "review", "to": "review" },
                    { "from": "review", "on": "complete", "to": "$end" }
                ]
            }
        }),
    );
    let mut plan = minimal_plan(&root);
    plan.config.config.model = Some(crate::harness_config::HarnessModelConfig {
        provider: "ollama".into(),
        model: "test-model".into(),
        options: json!({}),
    });
    plan.loop_package = Some(ResolvedPackageInfo {
        key: "loop:@zack/review-loop@0.1.0".into(),
        kind: PackageKind::Loop,
        name: "@zack/review-loop".into(),
        version: "0.1.0".into(),
        root: loop_root,
    });
    let report_path = root.join("approval-report.json");
    let mut model = ScriptedModelRuntime::new(vec![phase_completion_turn(
        Some("review"),
        Some(json!({ "assessment": "needs review" })),
    )]);
    let mut dispatcher = ScriptedActionDispatcher::default();
    let result = execute_headless_plan(
        &plan,
        "prepare a response".into(),
        Some(&report_path),
        &mut model,
        &mut dispatcher,
    )
    .unwrap();

    assert_eq!(result.status, HarnessTerminalStatus::ApprovalRequired);
    assert!(report_path.exists());
    assert_eq!(model.requests.len(), 1);
}

fn phase_completion_turn(outcome: Option<&str>, output: Option<serde_json::Value>) -> ModelTurn {
    ModelTurn {
        assistant_content: None,
        actions: vec![SemanticActionProposal::new(
            "complete",
            SemanticAction::PhaseCompletion {
                outcome: outcome.map(str::to_string),
                output,
            },
        )],
        usage: RunUsage::default(),
        finish_reason: Some("stop".into()),
        provider_metadata: BTreeMap::new(),
    }
}

fn temp_dir(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "agentpm-harness-command-{name}-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&path);
    fs::create_dir_all(&path).unwrap();
    path
}

fn minimal_run_report(run_id: &str) -> RunReport {
    RunReport {
        report_version: crate::harness_observability::HARNESS_REPORT_SCHEMA_VERSION,
        session_id: "session-1".into(),
        run_id: run_id.into(),
        agent: ReportPackageIdentity {
            name: "@zack/test-agent".into(),
            version: "0.1.0".into(),
        },
        loop_package: ReportPackageIdentity {
            name: "@zack/review-loop".into(),
            version: "0.1.0".into(),
        },
        started_at: chrono::Utc::now(),
        ended_at: None,
        duration_ms: None,
        terminal_status: HarnessTerminalStatus::Failed,
        terminal_output: None,
        preflight_status: PreflightStatus::Ready,
        diagnostics: Vec::new(),
        runtime: Default::default(),
        runtime_sources: BTreeMap::new(),
        consumer_context: None,
        scope_summaries: Vec::new(),
        phase_summaries: Vec::new(),
        checkpoint_summaries: Vec::new(),
        action_summaries: Vec::new(),
        tool_summaries: Vec::new(),
        mcp_summaries: Vec::new(),
        knowledge_summaries: Vec::new(),
        memory_summaries: Vec::new(),
        memory_write_review_summaries: Vec::new(),
        usage: RunUsage::default(),
        retry_count: 0,
        repair_count: 0,
        error_count: 0,
        approval_summary: BTreeMap::new(),
        cancellation_summary: BTreeMap::new(),
        trace_path: None,
    }
}

fn minimal_plan(root: &Path) -> ResolvedHarnessPlan {
    let config = HarnessConfig {
        version: 1,
        ..HarnessConfig::default()
    };
    ResolvedHarnessPlan {
        workspace_root: root.to_path_buf(),
        lock_path: root.join("agent.lock"),
        state_dir: root.join(".agentpm-state"),
        config: ResolvedHarnessConfig {
            workspace_root: root.to_path_buf(),
            config_path: None,
            config,
            state_dir: root.join(".agentpm-state"),
            state_dir_source: HarnessConfigSource::cli_override(),
        },
        selected_agent: Some(crate::harness_plan::ResolvedAgentRoot {
            root_key: "local:agent:agent.json".into(),
            name: "@zack/test-agent".into(),
            version: "0.1.0".into(),
            manifest_path: root.join("agent.json"),
            package_key: None,
            tools: Vec::new(),
            skills: Vec::new(),
            knowledge: Vec::new(),
            memory: Vec::new(),
            profiles: Vec::new(),
            loop_key: "loop:@zack/review-loop@0.1.0".into(),
        }),
        loop_package: None,
        package_graph: BTreeMap::new(),
        runtime_scopes: BTreeMap::new(),
        consumer_context: crate::harness_plan::ConsumerContextReadiness {
            state: CapabilityState::NotConfigured,
            file: None,
            path: None,
            byte_size: None,
            approximate_tokens: None,
            sha256: None,
        },
        profile_bindings: crate::harness_runtime::model::ProfileBindingSnapshot::default(),
        profiles: BTreeMap::new(),
        capabilities: Vec::new(),
        report: crate::harness_plan::PreflightReport {
            status: PreflightStatus::Ready,
            diagnostics: Vec::new(),
        },
    }
}

fn write_json(path: &Path, value: serde_json::Value) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, serde_json::to_vec_pretty(&value).unwrap()).unwrap();
}

fn write_semantic_memory_fixture(root: &Path, plan: &mut ResolvedHarnessPlan) {
    write_semantic_memory_fixture_with_modes(root, plan, json!(["semantic"]));
}

fn write_multi_memory_agent(root: &Path, packages: &[&str]) {
    write_json(
        &root.join("agent.json"),
        json!({
            "kind": "agent",
            "name": "@zack/test-agent",
            "version": "0.1.0",
            "memory": packages
                .iter()
                .map(|package| format!("{package}@0.1.0"))
                .collect::<Vec<_>>(),
            "bindings": {
                "global": {
                    "memory": packages
                        .iter()
                        .map(|package| json!({
                            "package": format!("{package}@0.1.0"),
                            "spaces": ["notes"]
                        }))
                        .collect::<Vec<_>>()
                }
            }
        }),
    );
}

fn write_key_memory_package(root: &Path, plan: &mut ResolvedHarnessPlan, package_name: &str) {
    let memory_root = root
        .join(".agentpm/memory")
        .join(package_name)
        .join("0.1.0");
    write_json(
        &memory_root.join("agent.json"),
        json!({
            "kind": "memory",
            "name": package_name,
            "version": "0.1.0",
            "description": "Key Memory test package.",
            "memory": {
                "scopes": {
                    "user": { "description": "User scope." }
                },
                "record_types": {
                    "note": {
                        "version": "1.0.0",
                        "description": "Note.",
                        "schema": "schemas/note.schema.json"
                    }
                },
                "spaces": {
                    "notes": {
                        "description": "Notes.",
                        "model": "collection",
                        "record_types": ["note"],
                        "scope": ["user"],
                        "retrieval": { "modes": ["key"] }
                    }
                }
            }
        }),
    );
    write_json(
        &memory_root.join("schemas/note.schema.json"),
        json!({
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "type": "object",
            "properties": {
                "body": { "type": "string", "minLength": 1 }
            },
            "required": ["body"],
            "additionalProperties": false
        }),
    );
    crate::commands::memory::execute_memory_build(
        &memory_root.join("agent.json"),
        crate::commands::memory::MemoryBuildMode::Write,
    )
    .unwrap();
    plan.package_graph.insert(
        format!("memory:{package_name}@0.1.0"),
        ResolvedPackageInfo {
            key: format!("memory:{package_name}@0.1.0"),
            kind: PackageKind::Memory,
            name: package_name.into(),
            version: "0.1.0".into(),
            root: memory_root,
        },
    );
}

fn write_semantic_memory_fixture_with_modes(
    root: &Path,
    plan: &mut ResolvedHarnessPlan,
    retrieval_modes: Value,
) {
    write_json(
        &root.join("agent.json"),
        json!({
            "kind": "agent",
            "name": "@zack/test-agent",
            "version": "0.1.0",
            "memory": ["semantic-memory-test@0.1.0"],
            "bindings": {
                "global": {
                    "memory": [
                        {
                            "package": "semantic-memory-test@0.1.0",
                            "spaces": ["notes"]
                        }
                    ]
                }
            }
        }),
    );
    let memory_root = root.join(".agentpm/memory/semantic-memory-test/0.1.0");
    write_json(
        &memory_root.join("agent.json"),
        json!({
            "kind": "memory",
            "name": "semantic-memory-test",
            "version": "0.1.0",
            "description": "Semantic Memory test package.",
            "memory": {
                "scopes": {
                    "user": { "description": "User scope." }
                },
                "record_types": {
                    "note": {
                        "version": "1.0.0",
                        "description": "Note.",
                        "schema": "schemas/note.schema.json"
                    }
                },
                "spaces": {
                    "notes": {
                        "description": "Semantic notes.",
                        "model": "collection",
                        "record_types": ["note"],
                        "scope": ["user"],
                        "retrieval": { "modes": retrieval_modes }
                    }
                }
            }
        }),
    );
    write_json(
        &memory_root.join("schemas/note.schema.json"),
        json!({
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "type": "object",
            "properties": {
                "body": { "type": "string", "minLength": 1 }
            },
            "required": ["body"],
            "additionalProperties": false
        }),
    );
    crate::commands::memory::execute_memory_build(
        &memory_root.join("agent.json"),
        crate::commands::memory::MemoryBuildMode::Write,
    )
    .unwrap();
    plan.package_graph.insert(
        "memory:semantic-memory-test@0.1.0".into(),
        ResolvedPackageInfo {
            key: "memory:semantic-memory-test@0.1.0".into(),
            kind: PackageKind::Memory,
            name: "semantic-memory-test".into(),
            version: "0.1.0".into(),
            root: memory_root,
        },
    );
}

fn host_service(role: &str, registry_id: &str) -> HostServiceRegistration {
    HostServiceRegistration {
        role: role.into(),
        registry_id: registry_id.into(),
    }
}

fn host_model_capabilities() -> Value {
    json!({
        "provider": "host-model",
        "model": "model-1",
        "semantic_actions": true,
        "structured_output": true,
        "multimodal_input": false,
        "usage_reporting": true
    })
}

fn custom_memory_capabilities(package: &str, version: &str, retrieval_modes: Value) -> Value {
    json!({
        "space_models": ["collection"],
        "retrieval_modes": retrieval_modes,
        "retention_actions": [],
        "constraints": [],
        "capacity": false,
        "durable_trigger_state": false,
        "atomic_batches": false,
        "packages": [
            {
                "package": package,
                "version": version,
                "ready": true
            }
        ]
    })
}

type MachineBridgeFixture = (
    MachineHostBridgeHandle,
    mpsc::Sender<std::result::Result<MachineEnvelope, String>>,
    Arc<Mutex<Vec<u8>>>,
);

fn buffered_machine_bridge() -> MachineBridgeFixture {
    let (writer, output) = MachineProtocolWriter::buffer(HarnessTraceContent::Full);
    let (sender, receiver) = mpsc::channel();
    let cancellation_requested = Arc::new(AtomicBool::new(false));
    let active_run = Arc::new(AtomicBool::new(false));
    (
        MachineHostBridgeHandle::new(writer, receiver, cancellation_requested, active_run),
        sender,
        output,
    )
}

fn machine_request(id: &str, method: &str, payload: Value) -> MachineEnvelope {
    MachineEnvelope {
        protocol: AGENTPM_HARNESS_MACHINE_PROTOCOL.into(),
        version: AGENTPM_HARNESS_MACHINE_VERSION,
        kind: MachineFrameKind::Request,
        id: Some(id.into()),
        method: Some(method.into()),
        payload,
        error: None,
    }
}

fn machine_response(id: &str, payload: Value) -> MachineEnvelope {
    MachineEnvelope {
        protocol: AGENTPM_HARNESS_MACHINE_PROTOCOL.into(),
        version: AGENTPM_HARNESS_MACHINE_VERSION,
        kind: MachineFrameKind::Response,
        id: Some(id.into()),
        method: None,
        payload,
        error: None,
    }
}

fn machine_error(id: &str, code: &str, message: &str) -> MachineEnvelope {
    MachineEnvelope {
        protocol: AGENTPM_HARNESS_MACHINE_PROTOCOL.into(),
        version: AGENTPM_HARNESS_MACHINE_VERSION,
        kind: MachineFrameKind::Error,
        id: Some(id.into()),
        method: None,
        payload: Value::Null,
        error: Some(MachineError {
            code: code.into(),
            message: message.into(),
        }),
    }
}

fn machine_frames_from_buffer(output: &Arc<Mutex<Vec<u8>>>) -> Vec<Value> {
    let output = output.lock().unwrap();
    String::from_utf8_lossy(&output)
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

fn empty_model_request(selection: ModelProviderSelection) -> ModelRequest {
    ModelRequest {
        runtime: RuntimeSnapshot::empty("session-1".into()),
        model: Some(selection),
        prompt: crate::harness_runtime::model::LogicalPrompt {
            sections: Vec::new(),
            action_aliases: Vec::new(),
            completion: crate::harness_runtime::model::CompletionContract {
                phase_id: "respond".into(),
                explicit_outcomes: Vec::new(),
                implicit_complete: true,
            },
            diagnostics: Vec::new(),
        },
        run_id: "run-1".into(),
        phase_execution_id: "phase-exec-1".into(),
        phase_id: "respond".into(),
        phase_objective: "Respond.".into(),
        run_input: "input".into(),
        prior_phase_results: Vec::new(),
        transcript: Vec::new(),
        effective_phase: crate::harness_engine::EffectivePhase {
            phase_id: "respond".into(),
            tools_allowed: Some(false),
            knowledge_allowed: None,
            memory_read_allowed: None,
            memory_write_allowed: None,
            authored_profile_candidates: Vec::new(),
            active_profiles: Vec::new(),
            active_tools: Vec::new(),
            active_skills: Vec::new(),
            active_knowledge: Vec::new(),
            active_memory: Vec::new(),
            capability_catalog: Vec::new(),
            suppressed_capabilities: Vec::new(),
        },
        repair_feedback: None,
    }
}

struct FakeHostInvoker {
    response: Value,
    capabilities: Option<Value>,
}

impl HostServiceInvoker for FakeHostInvoker {
    fn invoke_host_service(
        &mut self,
        _role: &str,
        _registry_id: &str,
        _method: &str,
        _payload: Value,
        _timeout_ms: u64,
    ) -> Result<Value> {
        Ok(self.response.clone())
    }

    fn host_service_capabilities(&self, _role: &str, _registry_id: &str) -> Option<Value> {
        self.capabilities.clone()
    }
}

struct UnsupportedModelRuntime {
    semantic_actions: bool,
    structured_output: bool,
}

impl ModelRuntime for UnsupportedModelRuntime {
    fn capabilities(&self) -> ModelCapabilityAdvertisement {
        ModelCapabilityAdvertisement {
            semantic_actions: self.semantic_actions,
            structured_output: self.structured_output,
            multimodal_input: false,
            context_window_tokens: None,
            usage_reporting: false,
        }
    }

    fn generate(
        &mut self,
        _request: crate::harness_runtime::ModelRequest,
    ) -> std::result::Result<ModelTurn, ModelRuntimeFailure> {
        unreachable!("capability validation should fail before generation")
    }
}
