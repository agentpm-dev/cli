use super::*;
use sha2::{Digest, Sha256};

fn fixtures() -> Value {
    serde_json::from_str(include_str!(
        "../../contracts/signing-canonicalization-v1.fixtures.json"
    ))
    .unwrap()
}

#[test]
fn shared_rfc8785_vectors_cover_unicode_utf16_sorting_numbers_and_whitespace() {
    for vector in fixtures()["jcs"].as_array().unwrap() {
        let input: Value = serde_json::from_str(vector["inputJson"].as_str().unwrap()).unwrap();
        let bytes = canonical_bytes(&input).unwrap();
        assert_eq!(
            bytes,
            vector["canonical"].as_str().unwrap().as_bytes(),
            "{}",
            vector["name"]
        );
        assert_eq!(format!("{:x}", Sha256::digest(&bytes)), vector["sha256"]);
        if input.is_object() {
            assert_eq!(
                crate::release::manifest_digest(&input).unwrap(),
                format!("sha256:{}", vector["sha256"].as_str().unwrap())
            );
        }
        let pretty = serde_json::to_string_pretty(&input).unwrap();
        let reordered: Value = serde_json::from_str(&pretty).unwrap();
        assert_eq!(bytes, canonical_bytes(&reordered).unwrap());
    }
    // JCS preserves ordinary array order; schema normalization is a separate step.
    assert_ne!(
        canonical_bytes(&serde_json::json!([1, 2])).unwrap(),
        canonical_bytes(&serde_json::json!([2, 1])).unwrap()
    );
}

#[test]
fn shared_contract_number_domain_and_timestamp_normalization() {
    let vectors = fixtures();
    for vector in vectors["timestamps"].as_array().unwrap() {
        let normalized = normalize_timestamp(vector["input"].as_str().unwrap()).unwrap();
        assert_eq!(normalized, vector["normalized"]);
        assert_eq!(normalize_timestamp(&normalized).unwrap(), normalized);
    }
    for value in vectors["invalidTimestamps"].as_array().unwrap() {
        assert!(
            normalize_timestamp(value.as_str().unwrap()).is_err(),
            "{value}"
        );
    }
    for value in vectors["invalidContracts"].as_array().unwrap() {
        let value: Value = serde_json::from_str(value.as_str().unwrap()).unwrap();
        assert!(contract_bytes(&value).is_err());
    }
    assert!(serde_json::from_str::<Value>(r#"{"invalid":"\udead"}"#).is_err());
}

#[test]
fn shared_python_resolution_arrays_normalize_before_jcs() {
    let vector = fixtures()["pythonResolution"].clone();
    for key in ["input", "normalized"] {
        let resolution: crate::python::PythonResolution =
            serde_json::from_value(vector[key].clone()).unwrap();
        assert_eq!(
            resolution.canonical_bytes().unwrap(),
            vector["canonical"].as_str().unwrap().as_bytes()
        );
        assert_eq!(resolution.digest().unwrap(), vector["sha256"]);
    }
}
