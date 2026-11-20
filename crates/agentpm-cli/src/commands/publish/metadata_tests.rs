//! Declared display files must exist in artifacts, independently of page metadata.
use super::*;
use crate::manifest::apds::{self, Context, Level, Status};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);

fn fixture(kind: &str) -> (PathBuf, JsonValue) {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let sequence = NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!(
        "agentpm-publish-metadata-{kind}-{nonce}-{sequence}"
    ));
    fs::create_dir_all(root.join("docs")).unwrap();
    fs::create_dir_all(root.join("template")).unwrap();
    fs::write(root.join("docs/README.md"), "# Authored README\n").unwrap();
    fs::write(root.join("LICENSE"), "Authored license text\n").unwrap();
    fs::write(root.join("template/starter.txt"), "starter content\n").unwrap();
    fs::write(root.join("main.js"), "console.log('{}');\n").unwrap();
    let mut value = serde_json::json!({
        "$schema": crate::manifest::DEFAULT_MANIFEST_SCHEMA_URL,
        "standard": {"id":"agentpm", "version":"1.0.0"},
        "kind": if kind == "python-tool" { "tool" } else { kind },
        "name": "metadata-package",
        "version": "0.1.0",
        "description": "Declared package metadata fixture.",
        "readme": "docs/README.md",
        "license": {"spdx":"MIT", "file":"LICENSE"}
    });
    match kind {
        "tool" | "python-tool" => {
            if kind == "python-tool" {
                fs::write(root.join("tool.py"), "print('{}')\n").unwrap();
                value["runtime"] = serde_json::json!({"type":"python", "version":"3.11"});
                value["entrypoint"] = serde_json::json!({"command":"python", "args":["tool.py"]});
                value["files"] = serde_json::json!(["tool.py"]);
            } else {
                value["runtime"] = serde_json::json!({"type":"node", "version":"20"});
                value["entrypoint"] = serde_json::json!({"command":"node", "args":["main.js"]});
                value["files"] = serde_json::json!(["main.js"]);
            }
            value["inputs"] = serde_json::json!({"type":"object"});
            value["outputs"] = serde_json::json!({"type":"object"});
        }
        "template" => {
            value["template"] = serde_json::json!({
                "display_name":"Metadata fixture", "use_case":"testing",
                "execution_surfaces":["node-sdk"], "stack":["node"],
                "files_root":"template", "variables":[],
                "dependencies":{"tools":[],"agents":[]},
                "entrypoints":[{"label":"Run", "command":"node main.js"}]
            });
        }
        "agent" => {}
        _ => unreachable!(),
    }
    (root, value)
}

fn archive_path(root: &Path, kind: &str) -> PathBuf {
    let filename = match kind {
        "tool" => "metadata-package-0.1.0-node20.tar.gz",
        "python-tool" => "metadata-package-0.1.0-python311.tar.gz",
        _ => "metadata-package-0.1.0.tar.gz",
    };
    root.join("target/agentpm").join(filename)
}

async fn build(root: &Path, value: &JsonValue) -> Result<()> {
    let path = root.join("agent.json");
    fs::write(&path, serde_json::to_vec_pretty(value)?)?;
    PublishArgs {
        manifest: path.to_string_lossy().into_owned(),
        dry_run: true,
        quiet: true,
        token: Some("dummy-token".into()),
        ..Default::default()
    }
    .run("https://example.com".into())
    .await
}

#[tokio::test]
async fn declared_metadata_is_archived_and_passes_packaged_apds_for_all_missing_paths() {
    for kind in ["agent", "template", "tool", "python-tool"] {
        let (root, value) = fixture(kind);
        build(&root, &value).await.unwrap();
        let path = archive_path(&root, kind);
        let mut archive =
            tar::Archive::new(flate2::read::GzDecoder::new(File::open(path).unwrap()));
        let extracted = root.join("extracted");
        archive.unpack(&extracted).unwrap();
        assert_eq!(
            fs::read_to_string(extracted.join("docs/README.md")).unwrap(),
            "# Authored README\n",
            "{kind}"
        );
        assert_eq!(
            fs::read_to_string(extracted.join("LICENSE")).unwrap(),
            "Authored license text\n",
            "{kind}"
        );
        assert_eq!(
            fs::read(extracted.join("agent.json")).unwrap(),
            fs::read(root.join("agent.json")).unwrap()
        );
        let result = apds::evaluate(
            &value,
            "agent.json",
            Level::Packaged,
            Context {
                package_root: Some(&extracted),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(
            result.status,
            Status::Conformant,
            "{kind}: {:?}",
            result.issues
        );
    }
}

#[tokio::test]
async fn missing_declared_metadata_fails_without_replacing_previous_artifact() {
    for kind in ["agent", "template", "tool", "python-tool"] {
        for (relative, pointer) in [("docs/README.md", "/readme"), ("LICENSE", "/license/file")] {
            let (root, value) = fixture(kind);
            build(&root, &value).await.unwrap();
            let artifact = archive_path(&root, kind);
            let before = fs::read(&artifact).unwrap();
            fs::remove_file(root.join(relative)).unwrap();
            fs::write(
                root.join("README.md"),
                "Discovery cannot replace a declaration\n",
            )
            .unwrap();
            let error = format!("{:#}", build(&root, &value).await.unwrap_err());
            assert!(
                error.contains("APDS-ID-004") && error.contains(pointer),
                "{kind}: {error}"
            );
            assert_eq!(fs::read(&artifact).unwrap(), before, "{kind}");
            assert!(!artifact_temp_path(&artifact).exists());
        }
    }
}

#[tokio::test]
async fn selected_metadata_is_not_duplicated_by_explicit_declarations() {
    for kind in ["agent", "template", "tool", "python-tool"] {
        let (root, mut value) = fixture(kind);
        value["readme"] = "./docs/README.md".into();
        value["license"]["file"] = "docs/README.md".into();
        if kind == "tool" {
            value["files"] = serde_json::json!(["main.js", "docs"]);
        }
        if kind == "python-tool" {
            value["files"] = serde_json::json!(["tool.py", "docs"]);
        }
        if kind == "template" {
            value["template"]["files_root"] = "docs".into();
        }
        build(&root, &value).await.unwrap();
        let entries = super::tests::tar_entries(&archive_path(&root, kind));
        assert_eq!(
            entries
                .iter()
                .filter(|name| name.as_str() == "docs/README.md")
                .count(),
            1,
            "{kind}"
        );
    }
}

#[cfg(unix)]
#[tokio::test]
async fn declared_metadata_cannot_follow_a_symlink_outside_the_package() {
    for kind in ["agent", "template", "tool", "python-tool"] {
        let (root, value) = fixture(kind);
        let outside = root.with_extension("outside.md");
        fs::write(&outside, "private external content\n").unwrap();
        fs::remove_file(root.join("docs/README.md")).unwrap();
        std::os::unix::fs::symlink(&outside, root.join("docs/README.md")).unwrap();
        let error = format!("{:#}", build(&root, &value).await.unwrap_err());
        assert!(
            error.contains("APDS-ID-004") && error.contains("escapes the package root"),
            "{kind}: {error}"
        );
        assert!(!archive_path(&root, kind).exists());
    }
}

#[tokio::test]
async fn python_declared_display_caps_fail_atomically_and_accept_the_boundary() {
    for (relative, pointer, limit) in [
        ("docs/README.md", "/readme", MAX_README_BYTES),
        ("LICENSE", "/license/file", MAX_LICENSE_BYTES),
    ] {
        let (root, value) = fixture("python-tool");
        build(&root, &value).await.unwrap();
        let artifact = archive_path(&root, "python-tool");
        let before = fs::read(&artifact).unwrap();
        fs::write(root.join(relative), vec![b'x'; limit + 1]).unwrap();
        let error = format!("{:#}", build(&root, &value).await.unwrap_err());
        assert!(
            error.contains("APDS-ID-004")
                && error.contains(pointer)
                && error.contains(&format!("{limit} bytes")),
            "{error}"
        );
        assert_eq!(fs::read(&artifact).unwrap(), before);
        assert!(!artifact_temp_path(&artifact).exists());
        fs::write(root.join(relative), vec![b'x'; limit]).unwrap();
        build(&root, &value).await.unwrap();
        assert!(super::tests::tar_entries(&artifact).contains(&relative.to_owned()));
    }
}

#[tokio::test]
async fn python_invalid_utf8_display_files_are_required_errors() {
    for (relative, pointer) in [("docs/README.md", "/readme"), ("LICENSE", "/license/file")] {
        let (root, value) = fixture("python-tool");
        fs::write(root.join(relative), [0xff, 0xfe]).unwrap();
        let error = format!("{:#}", build(&root, &value).await.unwrap_err());
        assert!(
            error.contains("APDS-ID-004") && error.contains(pointer) && error.contains("UTF-8"),
            "{error}"
        );
        assert!(!archive_path(&root, "python-tool").exists());
    }
}

#[tokio::test]
async fn python_undeclared_oversized_readme_remains_optional_discovery() {
    let (root, mut value) = fixture("python-tool");
    value.as_object_mut().unwrap().remove("readme");
    fs::write(root.join("README.md"), vec![b'x'; MAX_README_BYTES + 1]).unwrap();
    build(&root, &value).await.unwrap();
    let entries = super::tests::tar_entries(&archive_path(&root, "python-tool"));
    assert!(!entries.contains(&"README.md".to_owned()));
    assert!(entries.contains(&"LICENSE".to_owned()));
}

#[tokio::test]
async fn python_declared_readme_cannot_be_silently_excluded_as_build_output() {
    let (root, mut value) = fixture("python-tool");
    value["readme"] = "target/agentpm/README.md".into();
    fs::create_dir_all(root.join("target/agentpm")).unwrap();
    fs::write(root.join("target/agentpm/README.md"), "Required content\n").unwrap();
    let error = format!("{:#}", build(&root, &value).await.unwrap_err());
    assert!(
        error.contains("APDS-ID-004 /readme") && error.contains("excluded"),
        "{error}"
    );
    assert!(!archive_path(&root, "python-tool").exists());
}

#[tokio::test]
async fn python_declared_readme_can_reference_the_already_embedded_manifest() {
    let (root, mut value) = fixture("python-tool");
    value["readme"] = "agent.json".into();
    build(&root, &value).await.unwrap();
    let entries = super::tests::tar_entries(&archive_path(&root, "python-tool"));
    assert_eq!(
        entries
            .iter()
            .filter(|name| name.as_str() == "agent.json")
            .count(),
        1
    );
}
