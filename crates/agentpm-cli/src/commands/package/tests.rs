use super::*;

fn build(host: Target, target: Target) -> (PackageBuild, PathBuf) {
    (
        PackageBuild {
            build_type: BUILD_TYPE.into(),
            artifact_path: "fixture.tar.gz".into(),
            build_target: host,
            target: Some(target),
            format: ArtifactFormat::TarGz,
            artifact_sha256: format!("sha256:{}", "a".repeat(64)),
            artifact_size: 100,
            manifest_digest: format!("sha256:{}", "b".repeat(64)),
            python_resolution_digest: None,
            diagnostics: Vec::new(),
            metadata: json!({"manifest":{"kind":"tool","name":"fixture","version":"1.0.0"}}),
        },
        PathBuf::from(format!("{host}/package-build.json")),
    )
}

#[test]
fn fan_in_accepts_distinct_targets_and_rejects_missing_duplicate_or_changed_inputs() {
    let hosts = Target::SUPPORTED;
    let builds: Vec<_> = hosts.iter().map(|&host| build(host, host)).collect();
    validate_set(&builds, hosts).unwrap();
    assert!(
        validate_set(&builds[..builds.len() - 1], hosts)
            .unwrap_err()
            .to_string()
            .contains("missing")
    );
    let mut changed = builds.clone();
    changed[1].0.target = changed[0].0.target;
    assert!(
        validate_set(&changed, hosts)
            .unwrap_err()
            .to_string()
            .contains("duplicate artifact")
    );
    for field in ["name", "version"] {
        let mut changed = builds.clone();
        changed[1].0.metadata["manifest"][field] = "different".into();
        assert!(validate_set(&changed, hosts).is_err());
    }
    let mut changed = builds.clone();
    changed[1].0.manifest_digest = "different".into();
    assert!(validate_set(&changed, hosts).is_err());
    let mut changed = builds.clone();
    changed[1].0.python_resolution_digest = Some("different".into());
    assert!(validate_set(&changed, hosts).is_err());
}

#[test]
fn mixed_claims_report_both_builds_and_typed_permission_path_without_inventing_a_warning() {
    let mut concrete = build(Target::SUPPORTED[0], Target::SUPPORTED[0]);
    concrete.0.diagnostics.push(PayloadDiagnostic {
        code: agentpm_sdk::artifact::PayloadDiagnosticCode::ExecutableMember,
        severity: agentpm_sdk::artifact::PayloadSeverity::Warning,
        path: "assets/example.csv".into(),
        message: "executable archive member".into(),
        help: None,
        target: Some(Target::SUPPORTED[0]),
        expected_format: None,
        rule: None,
    });
    let portable = build(Target::SUPPORTED[1], Target::Any);
    let message = validate_set(&[concrete, portable], &[])
        .unwrap_err()
        .to_string();
    for expected in [
        "Mixed any/concrete",
        "assets/example.csv",
        "claims any",
        Target::SUPPORTED[0].as_str(),
        Target::SUPPORTED[1].as_str(),
    ] {
        assert!(message.contains(expected), "{message}");
    }
    assert!(validate_set(&[build(Target::SUPPORTED[0], Target::Any)], &[]).is_ok());
    assert!(
        validate_set(
            &[
                build(Target::SUPPORTED[0], Target::Any),
                build(Target::SUPPORTED[1], Target::Any)
            ],
            &[]
        )
        .is_err()
    );
}
