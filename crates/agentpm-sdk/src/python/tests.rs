use super::*;
use serde_json::json;

#[test]
fn recorded_set_is_selected_by_consumer_markers_without_solving() {
    let resolution = PythonResolution::new(
        ">=3.11".into(),
        vec!["foo>=1".into()],
        vec![
            package("foo", "1.0", Some("python_version < '3.12'")),
            package("foo", "2.0", Some("python_version >= '3.12'")),
            package("colorama", "0.4.6", Some("sys_platform == 'win32'")),
        ],
    )
    .unwrap();
    let mut markers = json!({"implementation_name":"cpython", "implementation_version":"3.11.9",
        "os_name":"posix", "platform_machine":"arm64", "platform_python_implementation":"CPython",
        "platform_release":"", "platform_system":"Darwin", "platform_version":"",
        "python_full_version":"3.11.9", "python_version":"3.11", "sys_platform":"darwin"});
    let selected = resolution.packages_for_environment(&markers).unwrap();
    assert_eq!(
        selected
            .iter()
            .map(|p| (p.name.as_str(), p.version.as_str()))
            .collect::<Vec<_>>(),
        vec![("foo", "1.0")]
    );
    markers["python_version"] = json!("3.12");
    markers["python_full_version"] = json!("3.12.1");
    markers["sys_platform"] = json!("win32");
    let selected = resolution.packages_for_environment(&markers).unwrap();
    assert_eq!(
        selected
            .iter()
            .map(|p| (p.name.as_str(), p.version.as_str()))
            .collect::<Vec<_>>(),
        vec![("colorama", "0.4.6"), ("foo", "2.0")]
    );
    markers["python_full_version"] = json!("3.10.9");
    assert!(resolution.packages_for_environment(&markers).is_err());
    markers["python_full_version"] = json!("3.12.1");
    markers["sys_platform"] = json!("linux");
    markers["platform_machine"] = json!("aarch64");
    let supported = crate::target::Target::SUPPORTED
        .iter()
        .map(|target| format!("({})", target.marker()))
        .collect::<Vec<_>>()
        .join(" or ");
    let resolution = PythonResolution::new(
        ">=3.11".into(),
        vec!["foo>=1".into()],
        vec![package("foo", "1.0", Some(&supported))],
    )
    .unwrap();
    assert!(
        resolution
            .packages_for_environment(&markers)
            .unwrap_err()
            .to_string()
            .contains("active requirement")
    );
}

fn package(name: &str, version: &str, marker: Option<&str>) -> PythonPackage {
    PythonPackage {
        name: name.into(),
        version: version.into(),
        hashes: vec![format!("sha256:{}", "a".repeat(64))],
        marker: marker.map(str::to_owned),
    }
}

#[test]
fn requirements_use_pep508_and_reject_duplicates_conflicts_and_paths() {
    let roots = vec![
        "Pydantic_Core >=2,<3".into(),
        "colorama; sys_platform == 'win32'".into(),
    ];
    let normalized = normalize_requirements(&roots).unwrap();
    assert!(normalized.iter().any(|r| r.starts_with("pydantic-core")));
    for values in [
        vec!["not valid **"],
        vec!["foo>=2,<1"],
        vec!["foo", "FOO>=1"],
        vec!["foo @ file:///private/local"],
        vec!["-r requirements.txt"],
        vec!["foo @ https://example.test/foo.whl"],
    ] {
        assert!(
            normalize_requirements(&values.iter().map(|v| v.to_string()).collect::<Vec<_>>())
                .is_err(),
            "{values:?}"
        );
    }
    assert!(
        normalize_requirements(&[
            "foo==1; sys_platform == 'win32'".into(),
            "foo==2; sys_platform != 'win32'".into()
        ])
        .is_ok()
    );
    assert_eq!(minimum_python("3.11").unwrap(), ">=3.11");
    assert!(minimum_python("3.11; malicious").is_err());
}

#[test]
fn resolution_is_normalized_and_canonical() {
    let first = PythonResolution::new(
        ">=3.11".into(),
        vec!["Zebra>=1".into(), "ALPHA".into()],
        vec![package("Zebra", "1.0", None), package("ALPHA", "2.0", None)],
    )
    .unwrap();
    let second = PythonResolution::new(
        ">=3.11".into(),
        vec!["alpha".into(), "zebra>=1".into()],
        vec![package("alpha", "2.0", None), package("zebra", "1.0", None)],
    )
    .unwrap();
    assert_eq!(
        first.canonical_bytes().unwrap(),
        second.canonical_bytes().unwrap()
    );
    assert_eq!(first.digest().unwrap(), second.digest().unwrap());
    assert_eq!(first.packages()[0].name, "alpha");
    assert_eq!(
        serde_json::from_slice::<PythonResolution>(&first.canonical_bytes().unwrap()).unwrap(),
        first
    );
    let mut wire = serde_json::to_value(first).unwrap();
    for field in ["target", "wheel", "path", "environment_id", "interpreter"] {
        wire[field] = json!("PRIVATE");
        assert!(serde_json::from_value::<PythonResolution>(wire.clone()).is_err());
        wire.as_object_mut().unwrap().remove(field);
    }
}

#[test]
fn resolution_rejects_missing_digests_and_overlapping_branches() {
    let base = PythonResolution::new(
        ">=3.11".into(),
        vec!["foo".into()],
        vec![package("foo", "1", None)],
    )
    .unwrap();
    let wire = serde_json::to_value(base).unwrap();
    for hashes in [
        json!([]),
        json!(["sha256:short"]),
        json!([format!("sha256:{}", "A".repeat(64))]),
    ] {
        let mut value = wire.clone();
        value["packages"][0]["hashes"] = hashes;
        assert!(serde_json::from_value::<PythonResolution>(value).is_err());
    }
    assert!(
        PythonResolution::new(
            ">=3.11".into(),
            vec!["foo".into()],
            vec![
                package("foo", "1", None),
                package("foo", "2", Some("sys_platform == 'win32'"))
            ]
        )
        .is_err()
    );
    assert!(
        PythonResolution::new(
            ">=3.11".into(),
            vec!["foo>=2".into()],
            vec![package("foo", "1", None)]
        )
        .is_err()
    );
    assert!(PythonResolution::new(">=3.11".into(), vec!["foo".into()], vec![]).is_err());
}

#[test]
fn portable_marker_branches_and_all_hashes_are_preserved() {
    let mut windows = package("foo", "1", Some("sys_platform == 'win32'"));
    windows.hashes.push(format!("sha256:{}", "0".repeat(64)));
    let result = PythonResolution::new(
        ">=3.11".into(),
        vec!["foo".into()],
        vec![
            windows,
            package("foo", "2", Some("sys_platform != 'win32'")),
        ],
    )
    .unwrap();
    assert_eq!(result.packages().len(), 2);
    assert_eq!(
        result.packages()[0].hashes.len() + result.packages()[1].hashes.len(),
        3
    );
    assert!(
        !String::from_utf8(result.canonical_bytes().unwrap())
            .unwrap()
            .contains("aarch64")
    );
}

#[test]
fn root_coverage_cannot_infer_missing_transitives_without_dependency_edges() {
    let root = package("click", "8.1.8", None);
    let windows_transitive = package("colorama", "0.4.6", Some("sys_platform == 'win32'"));
    let complete = PythonResolution::new(
        ">=3.11".into(),
        vec!["click==8.1.8".into()],
        vec![root.clone(), windows_transitive],
    )
    .unwrap();
    // This limitation is intentional in graph-free v1, not proof that Click needs no Colorama.
    let roots_only =
        PythonResolution::new(">=3.11".into(), vec!["click==8.1.8".into()], vec![root]).unwrap();
    assert_ne!(complete.digest().unwrap(), roots_only.digest().unwrap());
    assert_eq!(complete.packages().len(), 2);
    assert_eq!(roots_only.packages().len(), 1);
}

#[test]
fn fingerprint_records_every_resolution_input() {
    let inputs = ResolutionInputs::new(&["foo>=1".into()], "3.11", "0.12.6").unwrap();
    let fingerprint = inputs.fingerprint().unwrap();
    for mutated in [
        ResolutionInputs {
            uv_version: "0.12.7".into(),
            ..inputs.clone()
        },
        ResolutionInputs {
            python_requires: ">=3.12".into(),
            ..inputs.clone()
        },
        ResolutionInputs {
            requirements: vec!["foo>=2".into()],
            ..inputs.clone()
        },
        ResolutionInputs {
            targets: vec!["aarch64-apple-darwin".into()],
            ..inputs.clone()
        },
    ] {
        assert_ne!(fingerprint, mutated.fingerprint().unwrap());
    }
}

#[test]
fn canonical_contract_fixture_matches_cross_language_vector() {
    let resolution: PythonResolution =
        serde_json::from_str(include_str!("../../contracts/python-resolution-v1.json")).unwrap();
    assert_eq!(
        String::from_utf8(resolution.canonical_bytes().unwrap()).unwrap(),
        concat!(
            "{\"packages\":[{\"hashes\":[\"sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\"],",
            "\"name\":\"foo\",\"version\":\"1.2.0\"}],\"python\":{\"requires\":\">=3.11\"},",
            "\"requirements\":[\"foo>=1\"],\"type\":\"agentpm.python-resolution.v1\"}"
        )
    );
}
