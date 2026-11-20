use super::*;
use serde_json::json;

fn wire() -> Value {
    json!({"lockfile_version":4,"generated":"2026-11-20T00:00:00Z",
        "packages":{"tool:@fixture/shared@1.0.0":{"kind":"tool","name":"@fixture/shared","version":"1.0.0","integrity":"a".repeat(64),"standard":{"id":"agentpm","version":"1.0.0"},"standard_origin":"legacy_inferred"}},
        "roots":{"local:agent":{"name":"consumer","version":"1.0.0","tools":["tool:@fixture/shared@1.0.0"],"standard":{"id":"agentpm","version":"1.0.0"},"standard_origin":"declared"}}})
}

#[test]
fn apds_v4_roundtrip_frozen_plan_and_regeneration_preserve_origin_and_digest() {
    let original = wire();
    let lock: Lock = serde_json::from_value(original.clone()).unwrap();
    assert_eq!(serde_json::to_value(&lock).unwrap(), original);
    let desired = DesiredSet {
        items: vec![PackageRequirement::tool("@fixture/shared", "1.0.0")],
    };
    let plan = ResolvePlan::from_lock(&desired, &lock).unwrap();
    let transport = crate::semver::adapt::plan_to_sdk_resolve(&plan);
    let plan: ResolvePlan = transport.into();
    let Lock::V2(regenerated) = lock_from_plan(&plan, &[]) else {
        panic!()
    };
    assert_eq!(regenerated.lockfile_version, 4);
    assert_eq!(
        serde_json::to_value(regenerated.packages).unwrap(),
        original["packages"]
    );
}

#[test]
fn apds_fields_require_v4_and_reject_unknown_unsupported_and_partial_metadata() {
    for path in [
        "/packages/tool:@fixture~1shared@1.0.0",
        "/roots/local:agent",
    ] {
        for version in [2, 3] {
            let mut value = wire();
            value["lockfile_version"] = json!(version);
            assert!(
                serde_json::from_value::<Lock>(value)
                    .unwrap_err()
                    .to_string()
                    .contains("requires agent.lock version 4")
            );
        }
        for (field, replacement) in [
            ("standard", json!({"id":"agentpm","version":"2.0.0"})),
            ("standard_origin", json!("verified")),
            ("future_required", json!(true)),
        ] {
            let mut value = wire();
            value.pointer_mut(path).unwrap()[field] = replacement;
            assert!(serde_json::from_value::<Lock>(value).is_err());
        }
        for field in ["standard", "standard_origin"] {
            let mut value = wire();
            value
                .pointer_mut(path)
                .unwrap()
                .as_object_mut()
                .unwrap()
                .remove(field);
            assert!(serde_json::from_value::<Lock>(value).is_err());
        }
    }
}

#[test]
fn apds_local_root_is_refreshed_from_authored_manifest_including_legacy_inference() {
    let mut lock: Lock = serde_json::from_value(wire()).unwrap();
    if let Lock::V2(value) = &mut lock {
        value
            .roots
            .get_mut("local:agent")
            .unwrap()
            .python_input_fingerprint = Some("stale-local-identity".into());
    }
    let mut authored = json!({"kind":"agent","name":"renamed","version":"2.0.0"});
    record_root_standard(&mut lock, "local:agent", &authored).unwrap();
    let value = serde_json::to_value(&lock).unwrap();
    assert_eq!(
        value["roots"]["local:agent"]["standard_origin"],
        "legacy_inferred"
    );
    assert_eq!(value["roots"]["local:agent"]["name"], "renamed");
    assert_eq!(value["roots"]["local:agent"]["version"], "2.0.0");
    assert!(
        value["roots"]["local:agent"]
            .get("python_input_fingerprint")
            .is_none()
    );
    authored["standard"] = json!({"id":"agentpm","version":"1.0.0"});
    record_root_standard(&mut lock, "local:agent", &authored).unwrap();
    assert_eq!(
        serde_json::to_value(lock).unwrap()["roots"]["local:agent"]["standard_origin"],
        "declared"
    );
}

#[test]
fn apds_exact_identity_collisions_remain_separate_without_host_state() {
    let mut value = wire();
    for (kind, version, origin) in [
        ("skill", "1.0.0", "declared"),
        ("tool", "2.0.0", "legacy_inferred"),
    ] {
        value["packages"][format!("{kind}:@fixture/shared@{version}")] = json!({"kind":kind,"name":"@fixture/shared","version":version,"integrity":"b".repeat(64),"standard":{"id":"agentpm","version":"1.0.0"},"standard_origin":origin});
    }
    let lock: Lock = serde_json::from_value(value.clone()).unwrap();
    assert_eq!(serde_json::to_value(lock).unwrap(), value);
}
