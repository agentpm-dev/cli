use super::*;
use crate::telemetry::{
    RuntimeMode,
    tests::{Directory, recorder},
};

#[test]
fn telemetry_tracks_real_runs_not_sessions_and_never_projects_private_content() {
    let mut baseline = None;
    for mode in [
        RuntimeMode::Tui,
        RuntimeMode::Headless,
        RuntimeMode::Machine,
    ] {
        let directory = Directory::new();
        let (telemetry, drain) = recorder(&directory.0, mode);
        let mut session = HarnessSession::new();
        session.telemetry = telemetry;
        let mut manifest = base_loop();
        manifest.name = "@private/SECRET_PACKAGE".into();
        manifest.r#loop.transitions[0].to = "$end".into();
        let mut engine = HarnessEngine::new(manifest, HarnessEngineOptions::new(limits()));
        for _ in 0..2 {
            let mut model =
                ScriptedModelRuntime::new(vec![completion("PRIVATE_ACTION_ID", "execute")]);
            let mut dispatcher = ScriptedActionDispatcher::default();
            let mut approvals = ScriptedApprovalController::default();
            let result = engine
                .execute_run(
                    &mut session,
                    "PRIVATE_PROMPT /private/path API_KEY=secret",
                    &mut model,
                    &mut dispatcher,
                    &mut approvals,
                )
                .unwrap();
            assert!(matches!(result, HarnessRunResult::Terminal(_)));
        }
        let mut events = drain();
        assert_eq!(events.len(), 4);
        assert_eq!(session.usage.started_runs, 2);
        assert_eq!(events[0]["event"], "harness_started");
        assert_eq!(events[1]["event"], "harness_completed");
        assert_eq!(events[1]["properties"]["terminal_outcome"], "ended");
        let wire = serde_json::to_string(&events).unwrap();
        for forbidden in [
            "PRIVATE",
            "SECRET",
            "secret",
            "/private",
            "run_id",
            "session_id",
            "output",
            "prompt",
        ] {
            assert!(!wire.contains(forbidden), "leaked {forbidden}");
        }
        for event in &mut events {
            assert_eq!(event["properties"]["runtime_mode"], json!(mode));
            event["properties"]
                .as_object_mut()
                .unwrap()
                .remove("runtime_mode");
        }
        if let Some(expected) = &baseline {
            assert_eq!(&events, expected);
        } else {
            baseline = Some(events);
        }
    }
}

#[test]
fn handled_engine_error_emits_one_failed_completion_without_changing_error() {
    let directory = Directory::new();
    let (telemetry, drain) = recorder(&directory.0, RuntimeMode::Headless);
    let mut session = HarnessSession::new();
    session.telemetry = telemetry;
    let mut manifest = base_loop();
    manifest.r#loop.checkpoints.push(LoopCheckpoint {
        id: "gate".into(),
        r#type: "approval".into(),
        before_phase: "assess".into(),
        on_reject: "$abort".into(),
    });
    let mut engine = HarnessEngine::new(manifest, HarnessEngineOptions::new(limits()));
    let mut model = ScriptedModelRuntime::new(vec![completion("a", "execute")]);
    let mut approvals = ScriptedApprovalController::default();
    approvals.push("gate", ApprovalDecision::Failure("PRIVATE_ERROR".into()));
    let result = engine.execute_run(
        &mut session,
        "PRIVATE_PROMPT",
        &mut model,
        &mut ScriptedActionDispatcher::default(),
        &mut approvals,
    );
    assert!(result.unwrap_err().to_string().contains("PRIVATE_ERROR"));
    let events = drain();
    assert_eq!(events.len(), 2);
    assert_eq!(events[1]["properties"]["terminal_outcome"], "failed");
}

#[test]
fn panicking_engine_run_never_synthesizes_a_completion_during_unwind() {
    struct PanickingModel;
    impl ModelRuntime for PanickingModel {
        fn generate(&mut self, _request: ModelRequest) -> Result<ModelTurn, ModelRuntimeFailure> {
            panic!("PRIVATE_RUNTIME_PANIC");
        }
    }
    let directory = Directory::new();
    let (telemetry, drain) = recorder(&directory.0, RuntimeMode::Headless);
    let mut session = HarnessSession::new();
    session.telemetry = telemetry;
    let mut engine = HarnessEngine::new(base_loop(), HarnessEngineOptions::new(limits()));
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        engine.execute_run(
            &mut session,
            "PRIVATE_PROMPT",
            &mut PanickingModel,
            &mut ScriptedActionDispatcher::default(),
            &mut ScriptedApprovalController::default(),
        )
    }));
    assert!(result.is_err());
    drop(session);
    let events = drain();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0]["event"], "harness_started");
    assert!(!serde_json::to_string(&events).unwrap().contains("PRIVATE"));
}

#[test]
fn interactive_approval_pause_is_not_completion_but_cancellation_is() {
    let directory = Directory::new();
    let (telemetry, drain) = recorder(&directory.0, RuntimeMode::Tui);
    let mut session = HarnessSession::new();
    session.telemetry = telemetry;
    let mut manifest = base_loop();
    manifest.r#loop.checkpoints.push(LoopCheckpoint {
        id: "gate".into(),
        r#type: "approval".into(),
        before_phase: "assess".into(),
        on_reject: "$abort".into(),
    });
    let mut options = HarnessEngineOptions::new(limits());
    options.retain_active_on_approval_required = true;
    let mut engine = HarnessEngine::new(manifest, options);
    let mut approvals = ScriptedApprovalController::default();
    approvals.push("gate", ApprovalDecision::Pending);
    let result = engine
        .execute_run(
            &mut session,
            "PRIVATE_PROMPT",
            &mut ScriptedModelRuntime::new(vec![]),
            &mut ScriptedActionDispatcher::default(),
            &mut approvals,
        )
        .unwrap();
    assert!(matches!(result, HarnessRunResult::PendingApproval { .. }));
    let events = drain();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0]["event"], "harness_started");
    engine
        .cancel_active_run(&mut session, "PRIVATE_CANCEL_REASON")
        .unwrap();
    let events = drain();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0]["properties"]["terminal_outcome"], "cancelled");
}
