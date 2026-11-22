//! Shared local Knowledge readiness for preflight and execution snapshots.
use super::*;
use crate::harness_runtime::model::KnowledgeEmbeddingSnapshot;

pub(super) fn check_knowledge_backends(plan: &mut ResolvedHarnessPlan) {
    if !plan.report.apds.permits_runtime_checks() {
        return;
    }
    let Some(agent) = &plan.selected_agent else {
        return;
    };
    let active = plan
        .capabilities
        .iter()
        .filter(|c| c.kind == "knowledge" && c.state == CapabilityState::Available)
        .map(|c| c.identity.as_str())
        .collect::<BTreeSet<_>>();
    let mut diagnostics = Vec::new();
    for key in &agent.knowledge {
        let Some(package) = plan
            .package_graph
            .get(key)
            .filter(|p| active.contains(p.name.as_str()))
        else {
            continue;
        };
        let Ok((value, _)) = load_manifest_value(&package.root.join("agent.json")) else {
            continue;
        };
        let Ok(manifest) = parse_knowledge_manifest(&value) else {
            continue;
        };
        let (_, state, reason) = knowledge_runtime_readiness(plan, package, &manifest, "available");
        if state != "available" {
            push_diag(
                &mut diagnostics,
                PreflightDiagnosticSeverity::Suppressed,
                "knowledge_backend_unavailable",
                format!(
                    "Knowledge `{}` is unavailable in this local runtime: {}. This is not an APDS definition verdict.",
                    package.name,
                    reason.unwrap_or(state)
                ),
                Some(path_for_report(
                    &plan.workspace_root,
                    &package.root.join("agent.json"),
                )),
            );
        }
    }
    if !diagnostics.is_empty() {
        if plan.report.harness_support == HarnessSupport::Supported {
            plan.report.harness_support = HarnessSupport::Limited;
        }
        if plan.report.local_readiness == LocalReadiness::Ready {
            plan.report.local_readiness = LocalReadiness::Limited;
        }
        plan.report.diagnostics.extend(diagnostics);
    }
}

pub(crate) fn knowledge_runtime_readiness(
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

pub(crate) fn configured_embedding_provider_is_realizable(
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
