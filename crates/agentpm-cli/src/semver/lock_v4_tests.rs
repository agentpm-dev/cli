use super::*;
use serde_json::json;

fn resolution() -> PythonResolution {
    serde_json::from_str(include_str!(
        "../../../agentpm-sdk/contracts/python-resolution-v1.json"
    ))
    .unwrap()
}

#[test]
fn future_versions_and_unknown_fields_are_rejected_before_shape_conversion() {
    for shape in [json!({"dependencies":{}}), json!({"packages":{}})] {
        let mut value = shape;
        value["lockfile_version"] = json!(5);
        value["future_state"] = json!({"must":"survive"});
        let error = serde_json::from_value::<Lock>(value)
            .unwrap_err()
            .to_string();
        assert!(
            error.contains("Unsupported agent.lock version 5"),
            "{error}"
        );
    }
    let mut value = serde_json::to_value(Lock::empty_v2()).unwrap();
    value["future_state"] = json!(true);
    assert!(serde_json::from_value::<Lock>(value).is_err());
}

#[test]
fn legacy_versions_roundtrip_without_new_fields_or_format_changes() {
    for version in [2, 3] {
        let lock = Lock::V2(LockV2 {
            lockfile_version: version,
            generated: Utc::now(),
            packages: BTreeMap::new(),
            roots: BTreeMap::new(),
        });
        let before = serde_json::to_vec_pretty(&lock).unwrap();
        let after: Lock = serde_json::from_slice(&before).unwrap();
        assert_eq!(before, serde_json::to_vec_pretty(&after).unwrap());
        assert!(!String::from_utf8(before).unwrap().contains("python_"));
        let kind = if version == 2 { "tool" } else { "skill" };
        let key = format!("{kind}:@fixture/component@1.0.0");
        let mut populated = json!({"lockfile_version":version,"generated":"2026-10-23T00:00:00Z","packages":{},"roots":{}});
        populated["packages"][&key] = json!({"kind":kind,"name":"@fixture/component","version":"1.0.0","integrity":"a".repeat(64)});
        populated["roots"][if version == 2 {
            "local:agent"
        } else {
            "local:skill"
        }] = json!({"name":"fixture","version":"1.0.0","tools":[]});
        let before = serde_json::to_vec_pretty(&populated).unwrap();
        let after: Lock = serde_json::from_slice(&before).unwrap();
        assert_eq!(
            before,
            serde_json::to_vec_pretty(&serde_json::to_value(after).unwrap()).unwrap()
        );
    }
    let legacy = json!({"lockfile_version":2,"generated":"2026-10-23T00:00:00Z","dependencies":{"@fixture/t":{"version":"1.0.0","integrity":"a"}}});
    assert!(matches!(
        serde_json::from_value::<Lock>(legacy).unwrap(),
        Lock::V1(_)
    ));
}

#[test]
fn real_pre_11b_snapshots_roundtrip_and_unchanged_writes_preserve_every_byte() {
    for bytes in [
        include_bytes!("../../tests/fixtures/locks/pre11b-v2.json").as_slice(),
        include_bytes!("../../tests/fixtures/locks/pre11b-v3.json").as_slice(),
    ] {
        let mut lock: Lock = serde_json::from_slice(bytes).unwrap();
        let mut serialized =
            serde_json::to_vec_pretty(&serde_json::to_value(&lock).unwrap()).unwrap();
        serialized.push(b'\n');
        assert_eq!(bytes, serialized);
        let directory = std::env::temp_dir().join(format!(
            "agentpm-pre11b-snapshot-{:032x}",
            rand::random::<u128>()
        ));
        std::fs::create_dir(&directory).unwrap();
        let path = directory.join("agent.lock");
        std::fs::write(&path, bytes).unwrap();
        if let Lock::V2(value) = &mut lock {
            value.generated = Utc::now();
        }
        crate::manifest::write_lock(&directory, &lock).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
        std::fs::remove_dir_all(directory).unwrap();
    }
}

#[test]
fn consumer_resolution_survives_sdk_plan_lock_and_build_metadata_with_one_digest() {
    let resolution = resolution();
    let digest = resolution.digest().unwrap();
    let response: agentpm_sdk::models::install::ResolveResponse = serde_json::from_value(json!({"items":[{
        "kind":"tool","name":"@fixture/tool","version":"1.0.0","integrity":"a".repeat(64),"python_resolution":resolution
    }]})).unwrap();
    let plan: ResolvePlan = response.into();
    let wire = crate::semver::adapt::plan_to_sdk_resolve(&plan);
    assert_eq!(
        wire.items[0]
            .python_resolution
            .as_ref()
            .unwrap()
            .digest()
            .unwrap(),
        digest
    );
    let lock = lock_from_plan(&plan, &[]);
    assert_eq!(lock.lockfile_version(), 4);
    let desired = DesiredSet {
        items: vec![PackageRequirement::tool("@fixture/tool", "*")],
    };
    let frozen = ResolvePlan::from_lock(&desired, &lock).unwrap();
    assert_eq!(
        serde_json::to_value(crate::semver::adapt::plan_to_sdk_resolve(&frozen)).unwrap(),
        serde_json::to_value(&wire).unwrap()
    );
    let value = lock
        .find_unique_locked_package("@fixture/tool", PackageKind::Tool)
        .unwrap()
        .unwrap();
    assert_eq!(
        value.python_resolution.as_ref().unwrap().digest().unwrap(),
        digest
    );
    let metadata =
        agentpm_sdk::python::PythonReleaseMetadata::new(value.python_resolution.unwrap()).unwrap();
    metadata.validate().unwrap();
    assert_eq!(metadata.python_resolution_digest, digest);
    assert_eq!(
        metadata.release_manifest_digest().unwrap(),
        format!("sha256:{digest}")
    );
    assert_eq!(
        serde_json::to_value(&metadata).unwrap()["python_resolution"],
        serde_json::to_value(resolution).unwrap()
    );
}

#[test]
fn local_tool_state_survives_regeneration_but_absent_packages_do_not() {
    let resolution = resolution();
    let mut existing = lock_from_plan(
        &ResolvePlan { items: Vec::new() },
        &[LockRoot::LocalTool {
            name: "fixture".into(),
            version: "1.0.0".into(),
        }],
    );
    assert_eq!(
        existing.lockfile_version(),
        2,
        "a local Tool with no Python state does not require v4"
    );
    if let Lock::V2(value) = &mut existing {
        value.roots.get_mut("local:tool").unwrap().python_resolution = Some(resolution.clone());
        value
            .roots
            .get_mut("local:tool")
            .unwrap()
            .python_input_fingerprint = Some("stale-but-preserved".into());
        value.packages.insert(
            "tool:@old/tool@1.0.0".into(),
            LockedPackage {
                kind: PackageKind::Tool,
                name: "@old/tool".into(),
                version: "1.0.0".into(),
                integrity: "a".repeat(64),
                python_resolution: None,
            },
        );
    }
    let mut next = lock_from_plan(
        &ResolvePlan { items: Vec::new() },
        &[LockRoot::LocalTool {
            name: "fixture".into(),
            version: "1.0.0".into(),
        }],
    );
    preserve_local_tool_resolution(&existing, &mut next, true);
    let Lock::V2(next) = next else { panic!() };
    assert!(next.packages.is_empty());
    assert_eq!(
        next.roots["local:tool"].python_resolution.as_ref(),
        Some(&resolution)
    );
    assert_eq!(
        next.roots["local:tool"].python_input_fingerprint.as_deref(),
        Some("stale-but-preserved")
    );
    assert_eq!(next.lockfile_version, 4);
}

#[test]
fn local_tool_resolution_is_not_preserved_when_dependencies_are_removed() {
    let roots = [LockRoot::LocalTool {
        name: "fixture".into(),
        version: "1.0.0".into(),
    }];
    let mut existing = lock_from_plan(&ResolvePlan { items: Vec::new() }, &roots);
    let Lock::V2(value) = &mut existing else {
        panic!()
    };
    value.lockfile_version = 4;
    let root = value.roots.get_mut("local:tool").unwrap();
    root.python_resolution = Some(resolution());
    root.python_input_fingerprint = Some("previous-resolution".into());
    let original = serde_json::to_value(&existing).unwrap();

    for already_carried in [false, true] {
        let mut next = if already_carried {
            existing.clone()
        } else {
            lock_from_plan(&ResolvePlan { items: Vec::new() }, &roots)
        };
        preserve_local_tool_resolution(&existing, &mut next, false);
        let serialized = serde_json::to_value(&next).unwrap();
        assert_eq!(serialized["lockfile_version"], 4);
        assert_eq!(serialized["packages"], json!({}));
        assert_eq!(
            serialized["roots"],
            json!({"local:tool":{"name":"fixture","version":"1.0.0","tools":[]}}),
            "both Python fields must be omitted without losing the local Tool identity"
        );
        assert_eq!(serde_json::to_value(&existing).unwrap(), original);
    }
}

#[test]
fn both_cli_and_sdk_use_the_same_package_kind_type_including_template() {
    for value in [
        "tool",
        "agent",
        "skill",
        "knowledge",
        "memory",
        "profile",
        "loop",
        "template",
    ] {
        let cli: PackageKind = serde_json::from_value(json!(value)).unwrap();
        let sdk: agentpm_sdk::models::install::PackageKind = cli;
        assert_eq!(sdk.as_str(), value);
        assert_eq!(serde_json::to_value(sdk).unwrap(), json!(value));
    }
}

#[test]
fn realistic_twenty_tool_lock_fits_metadata_limits_and_roundtrips_through_resolve_dtos() {
    let resolution: PythonResolution = serde_json::from_str(include_str!(
        "../../../agentpm-sdk/contracts/python-resolution-realistic.json"
    ))
    .unwrap();
    assert_eq!(resolution.packages().len(), 14);
    assert_eq!(
        resolution
            .packages()
            .iter()
            .map(|p| p.hashes.len())
            .sum::<usize>(),
        113
    );
    let metadata = agentpm_sdk::python::PythonReleaseMetadata::new(resolution.clone()).unwrap();
    let metadata_bytes = serde_json::to_vec_pretty(&metadata).unwrap();
    assert!(metadata_bytes.len() < agentpm_sdk::python::MAX_RESOLUTION_METADATA_BYTES);
    let plan = ResolvePlan {
        items: (0..20)
            .map(|i| ResolvedPackage {
                kind: PackageKind::Tool,
                name: format!("@fixture/tool-{i}"),
                version: "1.0.0".into(),
                integrity: "a".repeat(64),
                python_resolution: Some(resolution.clone()),
            })
            .collect(),
    };
    let sdk_wire = crate::semver::adapt::plan_to_sdk_resolve(&plan);
    let bytes = serde_json::to_vec(&sdk_wire).unwrap();
    let restored: agentpm_sdk::models::install::ResolveResponse =
        serde_json::from_slice(&bytes).unwrap();
    let lock = lock_from_plan(&restored.into(), &[]);
    // Use the same Value/pretty serialization as write_lock; keep reviewable diffs.
    let lock_bytes = serde_json::to_vec_pretty(&serde_json::to_value(&lock).unwrap()).unwrap();
    // Fixture growth budget only, not a Registry request/response or lockfile cap.
    // The Registry separately enforces/tests its aggregate resolve-response budget.
    assert!(lock_bytes.len() < 512 * 1024);
    let restored: Lock = serde_json::from_slice(&lock_bytes).unwrap();
    let Lock::V2(restored) = restored else {
        panic!()
    };
    assert_eq!(restored.packages.len(), 20);
    for package in restored.packages.values() {
        assert_eq!(package.python_resolution.as_ref(), Some(&resolution));
    }
    eprintln!(
        "M11B scale: Tool metadata {} bytes; 20-Tool lock {} bytes; resolve DTO {} bytes",
        metadata_bytes.len(),
        lock_bytes.len(),
        bytes.len()
    );
}
