//! D13 experiment only: candidate injection is confined to this model adapter.
use super::*;
use crate::harness_runtime::model::PromptSection;

struct DescriptionComparison {
    include: bool,
    description: String,
    model: ScriptedModelRuntime,
}
impl ModelRuntime for DescriptionComparison {
    fn generate(&mut self, mut request: ModelRequest) -> Result<ModelTurn, ModelRuntimeFailure> {
        if self.include {
            request.prompt.sections.push(PromptSection {
                number: 12,
                title: "Agent description (experiment)".into(),
                content: self.description.clone(),
            });
        }
        self.model.generate(request)
    }
}

#[test]
fn description_prompt_ab_preserves_current_behavior_without_evidence_of_benefit() {
    let mut rows = Vec::new();
    for phases in [1, 3] {
        let mut outcomes = Vec::new();
        for include in [false, true] {
            let mut workflow = base_loop();
            workflow.r#loop.phases.truncate(phases);
            workflow.r#loop.checkpoints.clear();
            workflow.r#loop.transitions = workflow
                .r#loop
                .phases
                .iter()
                .enumerate()
                .map(|(i, phase)| LoopTransition {
                    from: phase.id.clone(),
                    on: "complete".into(),
                    to: workflow
                        .r#loop
                        .phases
                        .get(i + 1)
                        .map(|p| p.id.clone())
                        .unwrap_or_else(|| "$end".into()),
                })
                .collect();
            let description = "Summarize incidents and escalate unresolved risks.";
            for phase in &mut workflow.r#loop.phases {
                phase.objective = description.into();
                phase.outcomes.clear();
            }
            let turns: Vec<_> = workflow
                .r#loop
                .phases
                .iter()
                .map(|phase| completion(&phase.id, "complete"))
                .collect();
            let mut model = DescriptionComparison {
                include,
                description: description.into(),
                model: ScriptedModelRuntime::new(turns),
            };
            let mut engine = HarnessEngine::new(workflow, HarnessEngineOptions::new(limits()));
            let mut runtime = RuntimeSnapshot::empty("description-ab".into());
            if phases > 1 {
                runtime.profiles = vec![profile_snapshot(
                    "@fixture/style",
                    "1.0.0",
                    "Incident reviewer",
                )];
                runtime.profile_bindings.global = vec!["@fixture/style".into()];
            }
            let mut session = HarnessSession::with_runtime_snapshot(runtime);
            let result = engine
                .execute_run(
                    &mut session,
                    "Review the incident.",
                    &mut model,
                    &mut ScriptedActionDispatcher::default(),
                    &mut ScriptedApprovalController::default(),
                )
                .unwrap();
            let HarnessRunResult::Terminal(result) = result else {
                panic!("expected completed Run")
            };
            assert_eq!(result.status, HarnessTerminalStatus::Ended);
            outcomes.push(result.output.clone());
            assert_eq!(model.model.requests.len(), phases);
            for request in &model.model.requests {
                let prompt = request.prompt.render_text();
                assert_eq!(prompt.contains("Agent description (experiment)"), include);
                assert!(prompt.contains(description)); // Already present in phase objective.
                if phases > 1 {
                    assert_eq!(prompt.matches("Profile: @fixture/style@1.0.0").count(), 1);
                    assert!(prompt.contains("Do not invent evidence."));
                }
            }
            rows.push(json!({
                "phases": phases, "include_description": include,
                "terminal_status": result.status, "output": result.output,
                "prompts": model.model.requests.iter().map(|r| json!({"phase": r.phase_id, "text": r.prompt.render_text(), "utf8_bytes": r.prompt.render_text().len(), "whitespace_words": r.prompt.render_text().split_whitespace().count()})).collect::<Vec<_>>(),
                "provider": "deterministic scripted model", "paid_requests": 0,
                "provider_tokens": null, "provider_cost": null,
                "quality_assessment": "Control-flow/output equivalence only; scripted responses cannot establish LLM fidelity or quality benefit.",
                "duplication": if include {"Repeats authored phase objective."} else {"No additional description section."},
            }));
        }
        assert_eq!(outcomes[0], outcomes[1]);
    }
    let report = json!({"decision":"defer_injection_keep_current_behavior", "reason":"Added prompt content and duplicated intent; no measured model-quality benefit. No APDS change.", "rows":rows});
    if let Ok(path) = std::env::var("AGENTPM_DESCRIPTION_AB_RESULTS") {
        let path = std::path::PathBuf::from(path);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(path, serde_json::to_vec_pretty(&report).unwrap()).unwrap();
    }
}
