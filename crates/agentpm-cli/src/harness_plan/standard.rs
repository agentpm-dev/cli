//! Harness's read-only APDS boundary. This does not execute or certify Runner behavior.
use super::*;
use crate::manifest::apds::{self, Context, Level, ResolvedGraph, Status};
use agentpm_sdk::apds::{Standard, StandardOrigin};
use serde_json::json;

pub fn supported_standards() -> Vec<Standard> {
    vec![Standard {
        id: "agentpm".into(),
        version: "1.0.0".into(),
    }]
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DefinitionStatus {
    #[default]
    NotEvaluated,
    Conformant,
    LegacyInferred,
    Nonconformant,
    Incomplete,
    Unsupported,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DefinitionReport {
    pub status: DefinitionStatus,
    pub checked_levels: Vec<String>,
    pub artifacts: Vec<ArtifactStandard>,
    /// Only definition checks are performed here, never execution conformance.
    pub runner_evaluated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactStandard {
    #[serde(rename = "artifact_id")]
    pub key: String,
    pub file: String,
    pub standard: Option<Standard>,
    pub standard_origin: Option<StandardOrigin>,
    pub status: DefinitionStatus,
    pub evaluated_rules: BTreeSet<String>,
    pub unevaluated: BTreeSet<String>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HarnessSupport {
    #[default]
    NotEvaluated,
    Supported,
    Limited,
    Unsupported,
    NotRunnable,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LocalReadiness {
    #[default]
    NotEvaluated,
    Ready,
    Limited,
    NotReady,
    BootstrapRequired,
}

impl DefinitionReport {
    pub fn permits_runtime_checks(&self) -> bool {
        matches!(
            self.status,
            DefinitionStatus::Conformant | DefinitionStatus::LegacyInferred
        )
    }
}

struct Document {
    key: String,
    path: PathBuf,
    value: Value,
    authored: Value,
    standard: Option<Standard>,
    origin: Option<StandardOrigin>,
}

/// The graph is built only from selected lock edges. Other workspace roots are not inspected.
pub(super) fn check_definition(
    workspace: &Path,
    agent: &ResolvedAgentRoot,
    lock: &LockV2,
    packages: &BTreeMap<String, ResolvedPackageInfo>,
    diagnostics: &mut Vec<PreflightDiagnostic>,
) -> Result<DefinitionReport> {
    let mut report = DefinitionReport {
        status: DefinitionStatus::Conformant,
        ..Default::default()
    };
    let mut pending = vec![(
        "root".to_owned(),
        agent.root_key.clone(),
        agent.manifest_path.clone(),
    )];
    let mut documents = BTreeMap::new();
    let mut artifacts = Vec::new();
    let mut edges = Vec::new();
    let mut seen = BTreeSet::new();
    while let Some((id, key, path)) = pending.pop() {
        if !seen.insert(id.clone()) {
            continue;
        }
        let (mut value, _) = match load_manifest_value(&path) {
            Ok(value) => value,
            Err(error) => {
                let malformed = error.downcast_ref::<serde_json::Error>().is_some();
                report.status = if malformed {
                    DefinitionStatus::Nonconformant
                } else {
                    DefinitionStatus::Incomplete
                };
                push_diag(
                    diagnostics,
                    PreflightDiagnosticSeverity::Fatal,
                    if malformed {
                        "apds_nonconformant"
                    } else {
                        "apds_context_unavailable"
                    },
                    format!(
                        "Cannot inspect selected manifest {}: {error}",
                        path.display()
                    ),
                    Some(path_for_report(workspace, &path)),
                );
                continue;
            }
        };
        let locked_package = lock.packages.get(&key);
        let locked_root = lock.roots.get(&key);
        if let Some(package) = locked_package
            && key
                != format!(
                    "{}:{}@{}",
                    package.kind.as_str(),
                    package.name,
                    package.version
                )
        {
            report.status = DefinitionStatus::Nonconformant;
            push_diag(
                diagnostics,
                PreflightDiagnosticSeverity::Fatal,
                "apds_identity_mismatch",
                format!(
                    "Locked package key disagrees with its kind/name/version at {key}; run agentpm install to reconcile it."
                ),
                Some("agent.lock"),
            );
        }
        let (expected, expected_origin) = if let Some(package) = locked_package {
            (package.standard.as_ref(), package.standard_origin)
        } else {
            (
                locked_root.and_then(|r| r.standard.as_ref()),
                locked_root.and_then(|r| r.standard_origin),
            )
        };
        let observed = agentpm_sdk::apds::from_manifest(&value);
        let (standard, origin) = match observed {
            Ok((standard, origin)) => {
                if let Err(error) = agentpm_sdk::apds::reconcile(&value, expected, expected_origin)
                {
                    report.status = DefinitionStatus::Nonconformant;
                    push_diag(
                        diagnostics,
                        PreflightDiagnosticSeverity::Fatal,
                        "apds_provenance_mismatch",
                        error.to_string(),
                        Some(path_for_report(workspace, &path)),
                    );
                }
                (Some(standard), Some(origin))
            }
            Err(error) => {
                let unsupported = value.get("standard").is_some_and(|s| {
                    s["id"].as_str().is_some_and(|s| !s.is_empty())
                        && s["version"].as_str().is_some_and(|s| !s.is_empty())
                        && (s["id"] != "agentpm" || s["version"] != "1.0.0")
                });
                report.status = if unsupported {
                    DefinitionStatus::Unsupported
                } else {
                    DefinitionStatus::Nonconformant
                };
                let reported_path = path_for_report(workspace, &path);
                push_diag(
                    diagnostics,
                    PreflightDiagnosticSeverity::Fatal,
                    if unsupported {
                        "unsupported_standard"
                    } else {
                        "apds_invalid_standard"
                    },
                    format!(
                        "{reported_path}: {error}. Harness supports agentpm / 1.0.0; no substitute standard was applied."
                    ),
                    Some(reported_path.clone()),
                );
                report.artifacts.push(ArtifactStandard {
                    key,
                    file: reported_path,
                    standard: serde_json::from_value(value["standard"].clone()).ok(),
                    standard_origin: Some(StandardOrigin::Declared),
                    status: report.status,
                    evaluated_rules: BTreeSet::new(),
                    unevaluated: BTreeSet::new(),
                });
                continue;
            }
        };
        let legacy = origin == Some(StandardOrigin::LegacyInferred);
        let authored = value.clone();
        // Historical scoped names/selectors are interpretation views, never rewritten bytes.
        if legacy {
            value["standard"] = json!({"id":"agentpm", "version":"1.0.0"});
            if let Some(name) = value["name"].as_str().filter(|n| n.starts_with('@')) {
                value["name"] = json!(name.rsplit('/').next().unwrap_or(name));
            }
        }
        let (kind, name, version) = if let Some(package) = locked_package {
            (
                package.kind.as_str(),
                Some(package.name.as_str()),
                Some(package.version.as_str()),
            )
        } else {
            (
                "agent",
                locked_root.and_then(|r| r.name.as_deref()),
                locked_root.and_then(|r| r.version.as_deref()),
            )
        };
        let name_matches = name.is_none_or(|name| {
            value["name"] == name || value["name"].as_str() == name.rsplit('/').next()
        });
        if value["kind"] != kind || !name_matches || version.is_some_and(|v| value["version"] != v)
        {
            report.status = DefinitionStatus::Nonconformant;
            push_diag(
                diagnostics,
                PreflightDiagnosticSeverity::Fatal,
                "apds_identity_mismatch",
                format!(
                    "Selected manifest identity disagrees with agent.lock at {key}; run agentpm install to reconcile it."
                ),
                Some(path_for_report(workspace, &path)),
            );
        }
        let groups = if value["kind"] == "agent" {
            vec![
                ("tools", PackageKind::Tool),
                ("skills", PackageKind::Skill),
                ("knowledge", PackageKind::Knowledge),
                ("memory", PackageKind::Memory),
                ("profiles", PackageKind::Profile),
                ("loop", PackageKind::Loop),
            ]
        } else if value["kind"] == "skill" {
            vec![("tools", PackageKind::Tool)]
        } else {
            Vec::new()
        };
        let mut selected_keys = BTreeSet::new();
        for (field, kind) in &groups {
            let (field, kind) = (*field, *kind);
            let declarations: Vec<Value> = if field == "loop" {
                value.get(field).cloned().into_iter().collect()
            } else {
                value[field].as_array().cloned().unwrap_or_default()
            };
            for (index, reference) in declarations.iter().enumerate() {
                let pointer = if field == "loop" {
                    "/loop".into()
                } else {
                    format!("/{field}/{index}")
                };
                let Some((identity, selector)) = reference_parts(reference) else {
                    continue;
                };
                let root_keys = lock.roots.get(&key).map(|r| keys_for(r, kind));
                let matches: Vec<_> = packages
                    .values()
                    .filter(|p| {
                        p.kind == kind
                            && p.name == identity
                            && root_keys
                                .as_ref()
                                .is_none_or(|keys| keys.contains(&p.key.as_str()))
                            && crate::semver::types::locked_version_matches(&p.version, &selector)
                                .unwrap_or(false)
                    })
                    .collect();
                if matches.len() != 1 {
                    report.status = DefinitionStatus::Incomplete;
                    push_diag(
                        diagnostics,
                        PreflightDiagnosticSeverity::Fatal,
                        "apds_unresolved_composition",
                        format!(
                            "{key}{pointer}: expected one exact locked {kind:?} selection for {identity}@{selector}; run agentpm install to reconcile the graph."
                        ),
                        Some(format!("{}#{pointer}", path_for_report(workspace, &path))),
                    );
                    continue;
                }
                let selected = matches[0];
                selected_keys.insert(selected.key.as_str());
                // The APDS graph adapter takes concrete selections, after checking the original selector.
                *value.pointer_mut(&pointer).expect("existing declaration") =
                    json!(format!("{}@{}", selected.name, selected.version));
                edges.push(json!({"from":id,"field":pointer,"to":selected.key}));
                pending.push((
                    selected.key.clone(),
                    selected.key.clone(),
                    selected.root.join("agent.json"),
                ));
            }
        }
        if !legacy && let Some(locked_root) = locked_root {
            let recorded: BTreeSet<_> = groups
                .iter()
                .flat_map(|(_, kind)| keys_for(locked_root, *kind))
                .collect();
            if recorded != selected_keys {
                report.status = DefinitionStatus::Incomplete;
                push_diag(
                    diagnostics,
                    PreflightDiagnosticSeverity::Fatal,
                    "apds_unresolved_composition",
                    format!(
                        "Recorded dependency edges differ from the selected authored graph at {key}; run agentpm install to reconcile it."
                    ),
                    Some("agent.lock"),
                );
            }
        }
        if id != "root" {
            let package = &packages[&key];
            artifacts.push(json!({"id":id,"identity":package.name,"kind":package.kind.as_str(),"version":package.version,"manifest":path}));
        }
        documents.insert(
            id,
            Document {
                key,
                path,
                value,
                authored,
                standard,
                origin,
            },
        );
    }
    // Never normalize away a structural error in the authored bytes. In particular,
    // replacing a dependency selector with its pinned version must not hide bad fields.
    let mut evaluations = BTreeMap::new();
    for document in documents.values() {
        let own = apds::evaluate(
            &document.authored,
            &document.path.to_string_lossy(),
            Level::Standalone,
            Context::default(),
        )?;
        if document.origin != Some(StandardOrigin::LegacyInferred)
            && own.status != Status::Conformant
        {
            report.status = DefinitionStatus::Nonconformant;
        }
        evaluations.insert(document.key.clone(), own);
    }
    if !documents.is_empty() {
        report.checked_levels.push("standalone".into());
    }
    // Unsupported/context failures must not fall through to a different interpretation.
    let resolved = if report.permits_runtime_checks() {
        let graph: ResolvedGraph =
            serde_json::from_value(json!({"complete":true,"artifacts":artifacts,"edges":edges}))?;
        let graph = graph.with_documents(
            documents
                .values()
                .map(|d| (d.path.clone(), d.value.clone()))
                .collect(),
        );
        let root = &documents["root"];
        let evaluation = apds::evaluate(
            &root.value,
            &root.path.to_string_lossy(),
            Level::Resolved,
            Context {
                graph: Some(&graph),
                package_root: None,
            },
        )?;
        // Unevaluated context is not a successful definition check, even when
        // the evaluator has no violation to attach to a particular document.
        if matches!(evaluation.status, Status::Incomplete | Status::Unsupported) {
            report.status = if evaluation.status == Status::Unsupported {
                DefinitionStatus::Unsupported
            } else {
                DefinitionStatus::Incomplete
            };
            push_diag(
                diagnostics,
                PreflightDiagnosticSeverity::Fatal,
                if evaluation.status == Status::Unsupported {
                    "unsupported_standard"
                } else {
                    "apds_unresolved_composition"
                },
                format!(
                    "Selected graph could not be fully interpreted; unevaluated rules: {}. Reconcile the installation before running Harness.",
                    evaluation
                        .unevaluated
                        .iter()
                        .cloned()
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
                Some(path_for_report(workspace, &root.path)),
            );
        }
        report.checked_levels.push("resolved".into());
        Some(evaluation)
    } else {
        None
    };
    for (id, document) in &documents {
        let legacy = document.origin == Some(StandardOrigin::LegacyInferred);
        let own = &evaluations[&document.key];
        let mut issues = own
            .issues
            .iter()
            .chain(resolved.iter().flat_map(|e| &e.issues))
            .filter(|i| {
                i.file == document.path.to_string_lossy()
                    && !(legacy
                        && i.rule_id.as_deref() == Some("APDS-ID-001")
                        && i.instance_path == "/standard")
            })
            .collect::<Vec<_>>();
        issues.sort_by_key(|i| (&i.instance_path, &i.rule_id));
        issues.dedup_by_key(|i| (&i.instance_path, &i.rule_id));
        for issue in &issues {
            push_diag(
                diagnostics,
                if legacy {
                    PreflightDiagnosticSeverity::Warning
                } else {
                    PreflightDiagnosticSeverity::Fatal
                },
                if legacy {
                    "legacy_apds_issue"
                } else {
                    "apds_nonconformant"
                },
                format!(
                    "{} at {}: {}",
                    issue.rule_id.as_deref().unwrap_or("APDS"),
                    issue.instance_path,
                    issue.message
                ),
                Some(format!(
                    "{}#{}",
                    path_for_report(workspace, &document.path),
                    issue.instance_path
                )),
            );
            if !legacy {
                report.status = DefinitionStatus::Nonconformant;
            }
        }
        let coverage = if id == "root" {
            resolved.as_ref().unwrap_or(own)
        } else {
            own
        };
        report.artifacts.push(ArtifactStandard {
            key: document.key.clone(),
            file: path_for_report(workspace, &document.path),
            standard: document.standard.clone(),
            standard_origin: document.origin,
            status: if legacy {
                DefinitionStatus::LegacyInferred
            } else if own.status == Status::Conformant && issues.is_empty() {
                DefinitionStatus::Conformant
            } else {
                DefinitionStatus::Nonconformant
            },
            evaluated_rules: if legacy {
                BTreeSet::new()
            } else {
                coverage.evaluated_rules.clone()
            },
            unevaluated: if legacy {
                BTreeSet::from(["APDS-ID-001".into()])
            } else {
                coverage.unevaluated.clone()
            },
        });
    }
    if report.status == DefinitionStatus::Conformant
        && documents
            .values()
            .any(|d| d.origin == Some(StandardOrigin::LegacyInferred))
    {
        report.status = DefinitionStatus::LegacyInferred;
        push_diag(
            diagnostics,
            PreflightDiagnosticSeverity::Info,
            "legacy_standard_inferred",
            "Legacy interpretation: agentpm / 1.0.0 inferred for undeclared manifests; this is not verified APDS conformance. Source unchanged.",
            None::<String>,
        );
    }
    Ok(report)
}

fn reference_parts(value: &Value) -> Option<(String, String)> {
    if let Some(raw) = value.as_str() {
        Some(package_ref_string_parts(raw))
    } else {
        Some((
            value["name"].as_str()?.into(),
            value["version"].as_str().unwrap_or("*").into(),
        ))
    }
}

fn keys_for(root: &LockedRoot, kind: PackageKind) -> Vec<&str> {
    match kind {
        PackageKind::Tool => root.tools.iter().map(String::as_str).collect(),
        PackageKind::Skill => root.skills.iter().map(String::as_str).collect(),
        PackageKind::Knowledge => root.knowledge.iter().map(String::as_str).collect(),
        PackageKind::Memory => root.memory.iter().map(String::as_str).collect(),
        PackageKind::Profile => root.profiles.iter().map(String::as_str).collect(),
        PackageKind::Loop => root.r#loop.as_deref().into_iter().collect(),
        _ => Vec::new(),
    }
}

pub fn status_lines(report: &PreflightReport) -> Vec<String> {
    fn label(value: impl Serialize) -> String {
        serde_json::to_value(value)
            .expect("serializable status")
            .as_str()
            .expect("string status")
            .to_owned()
    }
    let standards = report
        .supported_standards
        .iter()
        .map(|s| format!("{} / {}", s.id, s.version))
        .collect::<Vec<_>>()
        .join(", ");
    let scope = if report.apds.checked_levels.is_empty() {
        "not evaluated".into()
    } else {
        report.apds.checked_levels.join("/")
    };
    vec![
        format!("Harness supported APDS standards: {standards}"),
        format!(
            "APDS definition ({scope} checks): {}; Runner behavior not evaluated.",
            label(report.apds.status)
        ),
        format!(
            "Harness support: {}; local runtime readiness: {}.",
            label(report.harness_support),
            label(report.local_readiness)
        ),
    ]
}
