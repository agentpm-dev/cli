use super::*;
use serde_json::json;

fn fixture() -> Value {
    serde_json::from_str(include_str!(
        "../../contracts/registry-provenance-v1.fixtures.json"
    ))
    .unwrap()
}

fn trusted() -> (Value, VerifiedKeySet) {
    let data = fixture();
    let roots: Vec<TrustRoot> = serde_json::from_value(data["roots"].clone()).unwrap();
    let signed: SignedKeySet = serde_json::from_value(data["keyset"].clone()).unwrap();
    assert_eq!(
        signed.statement.canonical_bytes().unwrap(),
        data["canonical"].as_str().unwrap().as_bytes()
    );
    let verified = signed
        .verify(
            &roots,
            &roots[0].registry,
            timestamp(data["now"].as_str().unwrap()).unwrap(),
        )
        .unwrap();
    (data, verified)
}

#[test]
fn authenticated_provenance_verifies_new_and_existing_legacy_releases() {
    let (data, keys) = trusted();
    for vector in data["vectors"].as_array().unwrap() {
        let artifact: PackageArtifact = serde_json::from_value(vector["artifact"].clone()).unwrap();
        verify_artifact(
            &artifact,
            Some(&keys),
            VerificationPolicy {
                require_signature: true,
                require_attestation: true,
            },
        )
        .unwrap();
        // All golden authors are currently revoked; historical acceptance still
        // verifies, including the retired Registry key's pre-retirement v2 rows.
        assert!(!artifact.verification.as_ref().unwrap().authors[0].is_active);
    }
}

#[test]
fn original_v1_registry_types_verify_without_rewriting_signed_bytes() {
    use ed25519_dalek::{Signer, SigningKey};
    let (data, keys) = trusted();
    let key = SigningKey::from_bytes(&std::array::from_fn(|i| i as u8));
    let policy = VerificationPolicy {
        require_signature: true,
        require_attestation: true,
    };
    for statement_type in [
        "agentpm.registry.attestation.v1",
        "agentpm.package.attestation.v1",
    ] {
        let mut artifact: PackageArtifact =
            serde_json::from_value(data["vectors"][2]["artifact"].clone()).unwrap();
        let att = artifact
            .verification
            .as_mut()
            .unwrap()
            .registry_attestation
            .as_mut()
            .unwrap();
        att.statement_json["type"] = json!(statement_type);
        if statement_type == "agentpm.registry.attestation.v1" {
            att.statement_json.as_object_mut().unwrap().remove("kind");
        }
        let bytes = legacy_statement_bytes(&att.statement_json).unwrap();
        att.signature_b64 = B64.encode(key.sign(&bytes).to_bytes());
        att.statement_bytes_b64 = Some(B64.encode(&bytes));
        verify_artifact(&artifact, Some(&keys), policy).unwrap();

        for field in ["type", "namespace", "kind", "artifactDigest"] {
            let mut changed = artifact.clone();
            let att = changed
                .verification
                .as_mut()
                .unwrap()
                .registry_attestation
                .as_mut()
                .unwrap();
            att.statement_json[field] = json!("unsupported");
            // Re-sign honestly: even a valid signature cannot bypass binding or
            // turn an unrecognized future type into a supported legacy format.
            let bytes = legacy_statement_bytes(&att.statement_json).unwrap();
            att.signature_b64 = B64.encode(key.sign(&bytes).to_bytes());
            att.statement_bytes_b64 = Some(B64.encode(bytes));
            assert!(
                verify_artifact(&changed, Some(&keys), policy).is_err(),
                "{statement_type}: {field}"
            );
        }
        let mut changed = artifact;
        changed
            .verification
            .as_mut()
            .unwrap()
            .registry_attestation
            .as_mut()
            .unwrap()
            .signature_b64 = B64.encode([0; 64]);
        assert!(verify_artifact(&changed, Some(&keys), policy).is_err());
    }
}

#[test]
fn original_tool_v1_authors_verify_without_weakening_identity_or_format_checks() {
    use ed25519_dalek::{Signer, SigningKey};

    fn sign(author: &mut AuthorProof, key: &SigningKey) {
        let bytes = legacy_statement_bytes(&author.statement_json).unwrap();
        author.signature_b64 = B64.encode(key.sign(&bytes).to_bytes());
        author.statement_bytes_b64 = Some(B64.encode(bytes));
    }

    let (data, keys) = trusted();
    let key = SigningKey::from_bytes(&std::array::from_fn(|i| i as u8));
    let policy = VerificationPolicy {
        require_signature: true,
        require_attestation: true,
    };
    for include_kind in [false, true] {
        let mut artifact: PackageArtifact =
            serde_json::from_value(data["vectors"][2]["artifact"].clone()).unwrap();
        let author = &mut artifact.verification.as_mut().unwrap().authors[0];
        assert_eq!(
            author.public_key_b64,
            B64.encode(key.verifying_key().as_bytes())
        );
        author.statement_json["type"] = json!("agentpm.tool.signature.v1");
        if !include_kind {
            author
                .statement_json
                .as_object_mut()
                .unwrap()
                .remove("kind");
        }
        // Keep original timestamp spelling and every extension field signed.
        author.statement_json["historicalExtension"] = json!("é😀");
        sign(author, &key);
        verify_artifact(&artifact, Some(&keys), policy).unwrap();

        for field in ["name", "version", "kind", "artifactDigest", "type"] {
            let mut changed = artifact.clone();
            let author = &mut changed.verification.as_mut().unwrap().authors[0];
            author.statement_json[field] = json!("unsupported");
            // A genuinely signed but wrongly bound or unknown statement fails.
            sign(author, &key);
            assert!(
                verify_artifact(&changed, Some(&keys), policy).is_err(),
                "{field}"
            );
        }
        for field in ["createdAt", "historicalExtension"] {
            let mut changed = artifact.clone();
            let author = &mut changed.verification.as_mut().unwrap().authors[0];
            author.statement_json[field] = json!("changed after signing");
            author.statement_bytes_b64 =
                Some(B64.encode(legacy_statement_bytes(&author.statement_json).unwrap()));
            assert!(
                verify_artifact(&changed, Some(&keys), policy).is_err(),
                "{field}"
            );
        }
        let mut changed = artifact.clone();
        changed.verification.as_mut().unwrap().authors[0].signature_b64 = B64.encode([0; 64]);
        assert!(verify_artifact(&changed, Some(&keys), policy).is_err());

        // The historical Tool label must not become a signature type for Agents.
        let mut changed = artifact;
        changed.kind = crate::models::install::PackageKind::Agent;
        let proof = changed.verification.as_mut().unwrap();
        proof.manifest["kind"] = json!("agent");
        proof.authors[0].statement_json["kind"] = json!("agent");
        sign(&mut proof.authors[0], &key);
        assert!(
            verify_artifact(
                &changed,
                None,
                VerificationPolicy {
                    require_signature: true,
                    require_attestation: false
                },
            )
            .is_err()
        );
    }
}

#[test]
fn keyset_root_origin_signature_and_validity_are_required() {
    let data = fixture();
    let roots: Vec<TrustRoot> = serde_json::from_value(data["roots"].clone()).unwrap();
    let signed: SignedKeySet = serde_json::from_value(data["keyset"].clone()).unwrap();
    let now = timestamp(data["now"].as_str().unwrap()).unwrap();
    assert!(signed.verify(&[], &roots[0].registry, now).is_err());
    assert!(
        signed
            .verify(&roots, "https://api.attacker.invalid", now)
            .is_err()
    );
    assert!(
        signed
            .verify(&roots, &roots[0].registry, now + 91 * 86400)
            .is_err()
    );
    let mut changed = signed.clone();
    changed.statement.keys[0].public_key_b64 = B64.encode([0; 32]);
    assert!(changed.verify(&roots, &roots[0].registry, now).is_err());
    let mut null_retirement = data["keyset"].clone();
    null_retirement["statement"]["keys"][1]["retiredAt"] = Value::Null;
    assert!(serde_json::from_value::<SignedKeySet>(null_retirement).is_err());
    let mut changed = signed;
    changed.signature_b64 = B64.encode([0; 64]);
    assert!(changed.verify(&roots, &roots[0].registry, now).is_err());
}

#[test]
fn mutations_of_every_release_binding_fail_verification() {
    let (data, keys) = trusted();
    let original = &data["vectors"][1]["artifact"];
    let policy = VerificationPolicy {
        require_signature: true,
        require_attestation: true,
    };
    for path in [
        vec!["integrity"],
        vec!["verification", "release", "manifestDigest"],
        vec!["verification", "release", "pythonResolutionDigest"],
        vec!["verification", "manifest", "description"],
        vec!["verification", "registry_attestation", "signature_b64"],
        vec!["verification", "registry_attestation", "registry_key_id"],
    ] {
        let mut changed = original.clone();
        let mut location = &mut changed;
        for name in &path {
            location = &mut location[*name];
        }
        *location = json!("changed");
        let result = serde_json::from_value::<PackageArtifact>(changed)
            .map(|artifact| verify_artifact(&artifact, Some(&keys), policy).is_err())
            .unwrap_or(true);
        assert!(result, "{path:?}");
    }
    let original: PackageArtifact = serde_json::from_value(original.clone()).unwrap();
    let mut changed = original.clone();
    changed
        .verification
        .as_mut()
        .unwrap()
        .release
        .as_mut()
        .unwrap()
        .artifacts
        .pop();
    // Even updating the outer digest cannot forge either release signature.
    changed.integrity = changed
        .verification
        .as_ref()
        .unwrap()
        .release
        .as_ref()
        .unwrap()
        .digest_hex()
        .unwrap();
    assert!(verify_artifact(&changed, Some(&keys), policy).is_err());
    let mut changed = original.clone();
    changed
        .verification
        .as_mut()
        .unwrap()
        .release
        .as_mut()
        .unwrap()
        .artifacts[0]
        .target = crate::target::Target::Any;
    changed.integrity = changed
        .verification
        .as_ref()
        .unwrap()
        .release
        .as_ref()
        .unwrap()
        .digest_hex()
        .unwrap();
    assert!(verify_artifact(&changed, Some(&keys), policy).is_err());
    let mut changed = original.clone();
    changed.verification.as_mut().unwrap().authors[0].signature_b64 = B64.encode([0; 64]);
    assert!(verify_artifact(&changed, Some(&keys), policy).is_err());
    let mut changed = original.clone();
    changed.verification.as_mut().unwrap().authors[0].statement_json["releaseDigest"] =
        json!(format!("sha256:{}", "f".repeat(64)));
    assert!(verify_artifact(&changed, Some(&keys), policy).is_err());
    let mut changed = original;
    changed
        .verification
        .as_mut()
        .unwrap()
        .registry_attestation
        .as_mut()
        .unwrap()
        .statement_json["releaseDigest"] = json!(format!("sha256:{}", "f".repeat(64)));
    assert!(verify_artifact(&changed, Some(&keys), policy).is_err());

    let mut legacy: PackageArtifact =
        serde_json::from_value(data["vectors"][2]["artifact"].clone()).unwrap();
    legacy
        .verification
        .as_mut()
        .unwrap()
        .registry_attestation
        .as_mut()
        .unwrap()
        .statement_bytes_b64 = Some(B64.encode(b"{}"));
    assert!(verify_artifact(&legacy, Some(&keys), policy).is_err());
}

#[test]
fn policy_requires_crypto_instead_of_counts_or_booleans() {
    let (data, keys) = trusted();
    let mut artifact: PackageArtifact =
        serde_json::from_value(data["vectors"][2]["artifact"].clone()).unwrap();
    for mode in ["off", "optional", "required"] {
        artifact.signing.as_mut().unwrap().mode = mode.into();
        artifact.verification.as_mut().unwrap().mode = mode.into();
        verify_artifact(
            &artifact,
            Some(&keys),
            VerificationPolicy {
                require_signature: true,
                require_attestation: true,
            },
        )
        .unwrap();
    }
    artifact.verification.as_mut().unwrap().authors.clear();
    assert!(verify_artifact(&artifact, Some(&keys), VerificationPolicy::default()).is_err());
    artifact.signing.as_mut().unwrap().mode = "optional".into();
    artifact.verification.as_mut().unwrap().mode = "optional".into();
    assert!(
        verify_artifact(
            &artifact,
            Some(&keys),
            VerificationPolicy {
                require_signature: true,
                require_attestation: false
            }
        )
        .is_err()
    );
    artifact.verification.as_mut().unwrap().registry_attestation = None;
    assert!(
        verify_artifact(
            &artifact,
            Some(&keys),
            VerificationPolicy {
                require_signature: false,
                require_attestation: true
            }
        )
        .is_err()
    );
    artifact.verification = None;
    assert!(
        verify_artifact(
            &artifact,
            Some(&keys),
            VerificationPolicy {
                require_signature: false,
                require_attestation: true
            }
        )
        .is_err()
    );
    verify_artifact(&artifact, None, VerificationPolicy::default()).unwrap();
}

#[test]
#[ignore = "requires the public proof exported by the disposable PostgreSQL backfill test"]
fn persisted_legacy_v2_attestation_verifies_through_the_consumer_path() {
    let path = std::env::var("AGENTPM_M14C_PERSISTED_PROOF")
        .expect("export the historical PostgreSQL proof first");
    let data: Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    let roots: Vec<TrustRoot> = serde_json::from_value(data["roots"].clone()).unwrap();
    let signed: SignedKeySet = serde_json::from_value(data["keyset"].clone()).unwrap();
    let keys = signed
        .verify(
            &roots,
            &roots[0].registry,
            timestamp(data["now"].as_str().unwrap()).unwrap(),
        )
        .unwrap();
    let artifact: PackageArtifact = serde_json::from_value(data["artifact"].clone()).unwrap();
    verify_artifact(
        &artifact,
        Some(&keys),
        VerificationPolicy {
            require_signature: false,
            require_attestation: true,
        },
    )
    .unwrap();
    assert_eq!(
        artifact
            .verification
            .unwrap()
            .registry_attestation
            .unwrap()
            .statement_json["type"],
        "agentpm.registry.attestation.v2"
    );
}
