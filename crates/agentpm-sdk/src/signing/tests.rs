use super::*;
use serde_json::{Value, json};

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
