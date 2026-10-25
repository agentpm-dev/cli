use super::*;
use serde_json::json;

#[test]
fn manifest_integer_domain_matches_the_registry_jcs_library() {
    for value in [json!(9007199254740992u64), json!(-9007199254740992i64)] {
        assert!(manifest_digest(&json!({"nested": [value]})).is_err());
    }
}

#[test]
fn shared_release_vectors_have_identical_canonical_bytes_and_digests() {
    let fixtures: Value = serde_json::from_str(include_str!(
        "../../contracts/package-release-v1.fixtures.json"
    ))
    .unwrap();
    for vector in fixtures["vectors"].as_array().unwrap() {
        let release: ReleaseManifest = serde_json::from_value(vector["release"].clone()).unwrap();
        assert_eq!(
            release.canonical_bytes().unwrap(),
            vector["canonical"].as_str().unwrap().as_bytes()
        );
        assert_eq!(release.digest().unwrap(), vector["digest"]);
        assert_eq!(
            manifest_digest(&vector["manifest"]).unwrap(),
            release.manifest_digest
        );
        assert_eq!(
            serde_jcs::to_vec(&vector["manifest"]).unwrap(),
            vector["manifestCanonical"].as_str().unwrap().as_bytes()
        );
        let mut reordered = release.clone();
        reordered.artifacts.reverse();
        assert_eq!(release.digest().unwrap(), reordered.digest().unwrap());
    }
}

#[test]
fn invalid_release_metadata_and_duplicate_targets_are_rejected() {
    let fixtures: Value = serde_json::from_str(include_str!(
        "../../contracts/package-release-v1.fixtures.json"
    ))
    .unwrap();
    let original = fixtures["vectors"][0]["release"].clone();
    for (field, bad) in [
        ("type", json!("future")),
        ("name", json!("@../bad")),
        ("version", json!("1.0")),
        ("manifestDigest", json!("sha256:BAD")),
        ("pythonResolutionDigest", json!("local-path")),
        ("pythonResolutionDigest", json!(null)),
        ("uploadId", json!("private")),
        ("artifacts", json!([])),
    ] {
        let mut value = original.clone();
        value[field] = bad;
        assert!(
            serde_json::from_value::<ReleaseManifest>(value).is_err(),
            "{field}"
        );
    }
    for (field, bad) in [
        ("target", json!("macos-arm64")),
        ("digest", json!("a".repeat(64))),
        ("size", json!(0)),
        ("size", json!(true)),
        ("size", json!(9007199254740992u64)),
        ("contentType", json!("application/zip")),
        ("format", json!("zip")),
        ("tmpKey", json!("uploads/private")),
    ] {
        let mut value = original.clone();
        value["artifacts"][0][field] = bad;
        assert!(
            serde_json::from_value::<ReleaseManifest>(value).is_err(),
            "{field}"
        );
    }
    let mut value = original;
    let first = value["artifacts"][0].clone();
    value["artifacts"].as_array_mut().unwrap().push(first);
    assert!(serde_json::from_value::<ReleaseManifest>(value).is_err());
}

#[test]
fn schema_target_inventory_matches_the_canonical_target_table() {
    let schema: Value = serde_json::from_str(include_str!(
        "../../contracts/package-release-v1.schema.json"
    ))
    .unwrap();
    let actual = schema["$defs"]["artifact"]["properties"]["target"]["enum"]
        .as_array()
        .unwrap();
    let expected = Target::SUPPORTED
        .iter()
        .chain([Target::Any].iter())
        .map(|t| json!(t.as_str()))
        .collect::<Vec<_>>();
    assert_eq!(actual.len(), expected.len());
    assert!(expected.iter().all(|t| actual.contains(t)));
}
