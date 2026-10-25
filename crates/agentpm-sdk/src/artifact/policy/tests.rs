use super::*;
use serde_json::{Value, json};

#[test]
fn embedded_inventory_is_valid_and_exhaustively_matchable() {
    let policy = PayloadFormats::embedded().unwrap();
    for suffix in policy.text_groups.iter().flat_map(|group| &group.suffixes) {
        assert!(policy.is_text_candidate(&format!("example{suffix}")));
    }
    for basename in &policy.text_basenames {
        assert!(policy.is_text_candidate(&basename.to_ascii_lowercase()));
    }
    for prefix in &policy.text_basename_prefixes {
        assert!(policy.is_text_candidate(&format!("{prefix}example")));
    }
    for format in &policy.binary_formats {
        for suffix in &format.suffixes {
            assert_eq!(
                policy.binary_for(&format!("asset{suffix}")).unwrap().name,
                format.name
            );
        }
    }
}

#[test]
fn invalid_inventory_fails_closed() {
    let original: Value = serde_json::from_str(FORMAT_POLICY_JSON).unwrap();
    for (pointer, replacement) in [
        ("/type", json!("agentpm.tool-payload-formats.v2")),
        ("/text_groups", json!([])),
        ("/text_groups/0/suffixes", json!([".py", ".py"])),
        ("/text_groups/0/suffixes", json!([".png"])),
        ("/text_groups/0/suffixes", json!([".other.png"])),
        ("/text_groups/0/suffixes", json!([".PY"])),
        ("/text_groups/0/suffixes", json!(["py"])),
        ("/text_basenames", json!(["README", "readme"])),
        ("/text_basenames", json!(["asset.png"])),
        ("/text_basenames", json!(["asset.so.1.json"])),
        ("/native_filenames/suffixes", json!([])),
        ("/native_filenames/suffixes", json!([".node", ".node"])),
        ("/native_filenames/suffixes", json!([".SO"])),
        ("/native_filenames/suffixes", json!([".py"])),
        ("/native_filenames/suffixes", json!([".custom.png"])),
        ("/native_filenames/basename_contains", json!([])),
        ("/native_filenames/basename_contains", json!([".SO."])),
        (
            "/native_filenames/basename_contains",
            json!([".so.", ".so."]),
        ),
        ("/native_filenames/basename_contains", json!([".missing."])),
        ("/native_filenames/basename_contains", json!([".so"])),
        ("/text_basename_prefixes", json!([".env.*"])),
        ("/text_basename_prefixes", json!(["nested/.env."])),
        (
            "/installed_state/directory_basenames",
            json!(["venv", "venv"]),
        ),
        ("/installed_state/marker_basenames", json!(["venv"])),
        ("/collection/any_basenames", json!(["pkg.dist-info"])),
        ("/collection/file_basenames", json!([".git"])),
        ("/collection/dotenv_prefix", json!(".different.")),
        (
            "/collection/dotenv_exceptions",
            json!(["not-a-dotenv-file"]),
        ),
        ("/binary_formats/0/min_header_bytes", json!(0)),
        ("/binary_formats/0/min_header_bytes", json!(5000)),
        ("/binary_formats/0/min_header_bytes", json!(7)),
        ("/binary_formats/0/signatures", json!([])),
        ("/binary_formats/0/signatures/0/hex", json!("f")),
        ("/binary_formats/0/signatures/0/hex", json!("zz")),
        ("/binary_formats/0/signatures/0/offset", json!(-1)),
        ("/binary_formats/0/signatures/0/offset", json!(u64::MAX)),
        (
            "/binary_formats/0/signatures",
            json!([{"offset":0,"hex":"89504e470d0a1a0a"},{"offset":0,"hex":"ff"}]),
        ),
    ] {
        let mut value = original.clone();
        *value.pointer_mut(pointer).unwrap() = replacement;
        assert!(
            PayloadFormats::from_json(&value.to_string()).is_err(),
            "{pointer}: {value}"
        );
    }
    let mut extra = original;
    extra["typo"] = json!(true);
    assert!(PayloadFormats::from_json(&extra.to_string()).is_err());
    assert!(PayloadFormats::from_json("{}").is_err());
}

#[test]
fn independent_binary_signatures_require_all_checks() {
    let policy = PayloadFormats::embedded().unwrap();
    for (filename, header) in [
        ("asset.png", b"\x89PNG\r\n\x1a\n".as_slice()),
        ("asset.jpg", b"\xff\xd8\xff"),
        ("asset.webp", b"RIFF\x04\0\0\0WEBP"),
        ("asset.pdf", b"%PDF-1.7\n"),
        ("asset.woff2", b"wOF2"),
    ] {
        let format = policy.binary_for(filename).unwrap();
        assert!(format.recognizes(header), "{filename}");
        assert!(!format.recognizes(&header[..format.min_header_bytes - 1]));
        assert!(!format.recognizes(b"MZ native binary renamed as data"));
    }
    let webp = policy.binary_for("asset.webp").unwrap();
    assert!(!webp.recognizes(b"RIFF\x04\0\0\0WAVE"));
    assert!(!webp.recognizes(b"XXXX\x04\0\0\0WEBP"));
}

#[test]
fn collection_and_classification_share_environment_vocabulary_and_kind_rules() {
    let policy = PayloadFormats::embedded().unwrap();
    for name in &policy.installed_state.directory_basenames {
        assert!(matches!(
            policy.collection_rule(name, true, false),
            Some(CollectionRule::EnvironmentDirectory)
        ));
        assert!(matches!(
            policy.collection_rule(&name.to_ascii_uppercase(), false, true),
            Some(CollectionRule::EnvironmentDirectory)
        ));
        assert!(policy.collection_rule(name, false, false).is_none());
        assert!(policy.is_installed_state(&format!("{name}/pkg.py")));
    }
    for marker in &policy.installed_state.marker_basenames {
        assert!(matches!(
            policy.collection_rule(marker, false, false),
            Some(CollectionRule::EnvironmentMarker)
        ));
        assert!(policy.is_installed_state(&format!("custom/{marker}")));
    }
    for suffix in &policy.installed_state.metadata_directory_suffixes {
        assert!(
            policy
                .collection_rule(&format!("package{suffix}"), true, false)
                .is_none()
        );
        assert!(policy.is_installed_state(&format!("package{suffix}/METADATA")));
    }
    for name in [
        &policy.collection.dotenv_basename,
        &format!("{}local", policy.collection.dotenv_prefix),
    ] {
        assert!(matches!(
            policy.collection_rule(name, false, false),
            Some(CollectionRule::Dotenv)
        ));
        assert!(policy.is_text_candidate(name));
    }
    for exception in &policy.collection.dotenv_exceptions {
        assert!(
            policy
                .collection_rule(&exception.to_ascii_uppercase(), false, false)
                .is_none()
        );
        assert!(policy.is_text_candidate(exception));
    }
}

#[test]
fn native_filename_matching_uses_the_shared_suffix_and_literal_fragment_modes() {
    let policy = PayloadFormats::embedded().unwrap();
    for suffix in &policy.native_filenames.suffixes {
        assert!(policy.is_native_filename(&format!("vendor/payload{suffix}")));
        assert!(policy.is_native_filename(&format!("PAYLOAD{}", suffix.to_ascii_uppercase())));
    }
    for fragment in &policy.native_filenames.basename_contains {
        for tail in ["1", "1.2.3", "notes.json", ""] {
            assert!(policy.is_native_filename(&format!("payload{fragment}{tail}")));
        }
        assert!(!policy.is_native_filename(&format!("directory{fragment}1/source.py")));
    }
    // Representative independent contract cases, not another exhaustive list.
    assert!(policy.is_native_filename("payload.node"));
    assert!(policy.is_native_filename("libfoo.so.1.2.3"));
    assert!(!policy.is_native_filename("payload.software.json"));
    assert!(!policy.is_native_filename("payload.json"));
    let mut changed: Value = serde_json::from_str(FORMAT_POLICY_JSON).unwrap();
    changed["native_filenames"]["suffixes"]
        .as_array_mut()
        .unwrap()
        .push(json!(".review-fixture"));
    let changed = PayloadFormats::from_json(&changed.to_string()).unwrap();
    assert!(changed.is_native_filename("payload.review-fixture"));
    assert!(!policy.is_native_filename("payload.review-fixture"));
}
