use super::*;
use serde_json::{Value, json};

#[test]
fn release_statements_match_shared_jcs_and_bind_one_logical_release() {
    let fixtures: Value = serde_json::from_str(include_str!(
        "../../contracts/release-signing-v1.fixtures.json"
    ))
    .unwrap();
    for vector in fixtures["vectors"].as_array().unwrap() {
        let release: crate::release::ReleaseManifest =
            serde_json::from_value(vector["release"].clone()).unwrap();
        let bytes = if vector["statement"]["type"] == AUTHOR_SIGNATURE_V2 {
            let value: AuthorSignatureV2 =
                serde_json::from_value(vector["statement"].clone()).unwrap();
            let constructed =
                AuthorSignatureV2::new(&release, "2026-11-06T06:00:00-07:00").unwrap();
            assert_eq!(value, constructed);
            value.canonical_bytes().unwrap()
        } else {
            let value: RegistryAttestationV3 =
                serde_json::from_value(vector["statement"].clone()).unwrap();
            let constructed = RegistryAttestationV3::new(
                &release,
                "2026-11-06T06:00:00-07:00",
                "fixture-é😀",
                "fixture-registry-key",
            )
            .unwrap();
            assert_eq!(value, constructed);
            value.canonical_bytes().unwrap()
        };
        assert_eq!(bytes, vector["canonical"].as_str().unwrap().as_bytes());
    }
}

#[test]
fn release_author_rejects_invalid_wire_before_signing() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../contracts/release-signing-v1.fixtures.json"
    ))
    .unwrap();
    for (field, value) in [
        ("type", json!("future")),
        ("name", json!("example")),
        ("version", json!("1.0")),
        ("releaseDigest", json!("bad")),
        ("createdAt", json!("2026-11-06T13:00:00+00:00")),
        ("createdAt", json!(null)),
        ("unknown", json!(true)),
    ] {
        let mut wire = fixture["vectors"][0]["statement"].clone();
        wire[field] = value;
        assert!(
            serde_json::from_value::<AuthorSignatureV2>(wire)
                .map(|s| s.canonical_bytes().is_err())
                .unwrap_or(true),
            "{field}"
        );
    }
}

#[test]
fn typed_author_v1_is_byte_identical_to_original_inline_statement() {
    let vectors: Value = serde_json::from_str(include_str!(
        "../../contracts/signing-canonicalization-v1.fixtures.json"
    ))
    .unwrap();
    let vector = &vectors["legacy"][0];
    let typed: AuthorSignatureV1 = serde_json::from_value(vector["statement"].clone()).unwrap();
    assert_eq!(
        typed.legacy_bytes().unwrap(),
        vector["rustCanonical"].as_str().unwrap().as_bytes()
    );
    assert_eq!(
        typed.legacy_bytes().unwrap(),
        serde_json::to_vec(&vector["statement"]).unwrap()
    );
    let constructed = AuthorSignatureV1::new(
        typed.kind.clone(),
        typed.name.clone(),
        typed.version.clone(),
        typed.artifact_digest.clone(),
        typed.created_at.clone(),
    );
    assert_eq!(constructed, typed);
    // Exercise every current publish kind, Unicode and a non-UTC historical timestamp.
    for kind in [
        "tool",
        "agent",
        "template",
        "skill",
        "knowledge",
        "memory",
        "profile",
        "loop",
    ] {
        let statement = AuthorSignatureV1::new(
            kind.into(),
            "café 😀".into(),
            "1.2.0".into(),
            typed.artifact_digest.clone(),
            "2026-10-26T06:34:56-06:00".into(),
        );
        let old = json!({"type":AUTHOR_SIGNATURE_V1,"kind":kind,"name":"café 😀","version":"1.2.0","artifactDigest":typed.artifact_digest,"createdAt":"2026-10-26T06:34:56-06:00"});
        assert_eq!(
            statement.legacy_bytes().unwrap(),
            serde_json::to_vec(&old).unwrap()
        );
        assert_eq!(
            serde_json::from_slice::<AuthorSignatureV1>(&statement.legacy_bytes().unwrap())
                .unwrap(),
            statement
        );
    }
}
