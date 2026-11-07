use super::*;
use std::io::{Read, Write};
use std::net::TcpListener;

fn root() -> PathBuf {
    let root = std::env::temp_dir().join(format!("agentpm-m15-{:032x}", rand::random::<u128>()));
    std::fs::create_dir(&root).unwrap();
    root
}

fn archive(entries: &[(&str, &[u8], tar::EntryType)]) -> Vec<u8> {
    let mut tar = tar::Builder::new(flate2::write::GzEncoder::new(
        Vec::new(),
        flate2::Compression::default(),
    ));
    for (name, bytes, kind) in entries {
        let mut header = tar::Header::new_gnu();
        header.set_mode(0o644);
        header.set_entry_type(*kind);
        header.set_size(bytes.len() as u64);
        if kind.is_symlink() || kind.is_hard_link() {
            header.set_link_name("elsewhere").unwrap();
        }
        header.set_cksum();
        tar.append_data(&mut header, name, *bytes).unwrap();
    }
    tar.into_inner().unwrap().finish().unwrap()
}

#[tokio::test]
async fn bad_archive_never_replaces_previous_install_and_new_failure_leaves_no_directory() {
    let root = root();
    let source = root.join("artifact.tgz");
    let dest = root.join("1.0.0");
    std::fs::create_dir(&dest).unwrap();
    std::fs::write(dest.join("original"), "valid").unwrap();
    for kind in [
        tar::EntryType::Symlink,
        tar::EntryType::Link,
        tar::EntryType::Fifo,
    ] {
        std::fs::write(&source, archive(&[("bad", b"", kind)])).unwrap();
        assert!(
            extract_verified(&source, &dest, None, 10000, 10)
                .await
                .is_err()
        );
        assert_eq!(
            std::fs::read_to_string(dest.join("original")).unwrap(),
            "valid"
        );
        let fresh = root.join("fresh");
        assert!(
            extract_verified(&source, &fresh, None, 10000, 10)
                .await
                .is_err()
        );
        assert!(!fresh.exists());
    }
    std::fs::write(
        &source,
        archive(&[
            ("one", b"content", tar::EntryType::Regular),
            ("two", b"content", tar::EntryType::Regular),
        ]),
    )
    .unwrap();
    assert!(
        extract_verified(&source, &dest, None, 10000, 1)
            .await
            .is_err()
    );
    assert!(
        extract_verified(&source, &dest, None, 100, 10)
            .await
            .is_err()
    );
    assert!(dest.join("original").exists());
    assert!(!std::fs::read_dir(&root).unwrap().any(|p| {
        p.unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with(".extract-")
    }));
    for path in ["../escape", "/absolute", "C:/outside", "x\\..\\outside"] {
        assert!(sanitize_entry_path(Path::new(path)).is_err());
    }
    std::fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn selected_artifact_is_local_and_cache_changes_with_target_and_digest() {
    let root = root();
    let source = root.join("artifact.tgz");
    let dest = root.join("1.0.0");
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../agentpm-sdk/contracts/registry-provenance-v1.fixtures.json"
    ))
    .unwrap();
    let mut artifact: sdkm::PackageArtifact =
        serde_json::from_value(fixture["vectors"][0]["artifact"].clone()).unwrap();
    let selected = artifact
        .verification
        .as_ref()
        .unwrap()
        .release
        .as_ref()
        .unwrap()
        .artifacts[0]
        .clone();
    artifact.selected_artifact = Some(selected.clone());
    let manifest = serde_json::to_vec(&artifact.verification.as_ref().unwrap().manifest).unwrap();
    let descriptor = serde_json::to_vec(&agentpm_sdk::release::ArtifactMetadata {
        target: selected.target,
        format: selected.format,
    })
    .unwrap();
    std::fs::write(
        &source,
        archive(&[
            ("agent.json", &manifest, tar::EntryType::Regular),
            (
                "agentpm-artifact.json",
                &descriptor,
                tar::EntryType::Regular,
            ),
        ]),
    )
    .unwrap();
    extract_verified(&source, &dest, Some(&artifact), 100000, 10)
        .await
        .unwrap();
    assert!(local_selection_matches(&dest, &artifact).unwrap());
    validate_local_selection(&dest).unwrap();
    let original_cache = cache_filename(&artifact);
    let mut other = artifact.clone();
    other.selected_artifact.as_mut().unwrap().target =
        agentpm_sdk::target::Target::current().unwrap();
    assert_ne!(original_cache, cache_filename(&other));
    other = artifact.clone();
    other.selected_artifact.as_mut().unwrap().digest = format!("sha256:{}", "d".repeat(64));
    assert_ne!(original_cache, cache_filename(&other));
    assert!(!local_selection_matches(&dest, &other).unwrap());
    let marker = selection_path(&dest).unwrap();
    let mut state: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&marker).unwrap()).unwrap();
    state["target"] = serde_json::json!("different-host");
    std::fs::write(marker, serde_json::to_vec(&state).unwrap()).unwrap();
    assert!(validate_local_selection(&dest).is_err());
    // A signed inventory can still describe a malformed archive; reject its embedded descriptor.
    let before = std::fs::read(dest.join("agent.json")).unwrap();
    std::fs::write(
        &source,
        archive(&[
            ("agent.json", &manifest, tar::EntryType::Regular),
            ("agentpm-artifact.json", b"{}", tar::EntryType::Regular),
        ]),
    )
    .unwrap();
    assert!(
        extract_verified(&source, &dest, Some(&artifact), 100000, 10)
            .await
            .is_err()
    );
    assert_eq!(std::fs::read(dest.join("agent.json")).unwrap(), before);
    std::fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn expired_url_renews_only_same_inventory_and_cache_is_reverified() {
    let root = root();
    let bytes = archive(&[("agent.json", b"{}", tar::EntryType::Regular)]);
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let init = sdkm::InstallInitResponse {
        session_id: "fixture".into(),
        expires_at: "2030-01-01T00:00:00Z".into(),
        artifacts: vec![sdkm::PackageArtifact {
            kind: sdkm::PackageKind::Tool,
            name: "@fixture/example".into(),
            version: "1.0.0".into(),
            integrity: format!("{:x}", Sha256::digest(&bytes)),
            presigned_url: format!("{base}/expired"),
            size: Some(bytes.len() as u64),
            content_type: Some("application/gzip".into()),
            signing: None,
            verification: None,
            runtime: None,
            python_resolution: None,
            selected_artifact: None,
        }],
    };
    let mut renewed: serde_json::Value = serde_json::to_value(&init).unwrap();
    renewed["artifacts"][0]["presigned_url"] = serde_json::json!(format!("{base}/valid"));
    let server_bytes = bytes.clone();
    let server = std::thread::spawn(move || {
        for index in 0..3 {
            let (mut stream, _) = listener.accept().unwrap();
            stream.set_nonblocking(false).unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(10)))
                .unwrap();
            let mut request = Vec::new();
            while !request.ends_with(b"\r\n\r\n") {
                let mut byte = [0];
                stream.read_exact(&mut byte).unwrap();
                request.push(byte[0]);
            }
            let headers = String::from_utf8(request).unwrap();
            let (status, body) = match index {
                0 => {
                    assert!(headers.starts_with("GET /expired"));
                    assert!(!headers.to_lowercase().contains("authorization:"));
                    ("403 Forbidden", Vec::new())
                }
                1 => {
                    assert!(headers.starts_with("POST /v1/tools/install/renew"));
                    let length: usize = headers
                        .lines()
                        .find_map(|line| {
                            line.to_lowercase()
                                .strip_prefix("content-length:")
                                .map(str::trim)
                                .map(str::parse)
                        })
                        .unwrap()
                        .unwrap();
                    let mut body = vec![0; length];
                    stream.read_exact(&mut body).unwrap();
                    assert_eq!(
                        serde_json::from_slice::<serde_json::Value>(&body).unwrap()["session_id"],
                        "fixture"
                    );
                    ("200 OK", serde_json::to_vec(&renewed).unwrap())
                }
                _ => {
                    assert!(headers.starts_with("GET /valid"));
                    assert!(!headers.to_lowercase().contains("authorization:"));
                    ("200 OK", server_bytes.clone())
                }
            };
            write!(stream, "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len()).unwrap();
            stream.write_all(&body).unwrap();
        }
    });
    let cache = root.join("cache");
    let tools = root.join("tools");
    let roots = InstallRoots {
        tools_dir: &tools,
        agents_dir: &tools,
        skills_dir: &tools,
        knowledge_dir: &tools,
        memory_dir: &tools,
        profiles_dir: &tools,
        loops_dir: &tools,
    };
    let registry = agentpm_sdk::AgentPmClient::new(base)
        .unwrap()
        .with_token("apm_fixture_only");
    download_and_extract_with_client(&init, &cache, roots, false, true, Some(&registry))
        .await
        .unwrap();
    server.join().unwrap();
    // Verified cache succeeds even after the fixture server is gone.
    download_and_extract_with_client(&init, &cache, roots, false, true, Some(&registry))
        .await
        .unwrap();
    let cached = cache.join(cache_filename(&init.artifacts[0]));
    std::fs::write(&cached, b"tampered").unwrap();
    assert!(
        download_and_extract_with_client(&init, &cache, roots, false, true, Some(&registry))
            .await
            .is_err()
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn size_hash_truncation_and_idle_failures_preserve_previous_cache_and_remove_parts() {
    let root = root();
    let cache = root.join("archive.tgz");
    for mode in 0..4 {
        std::fs::write(&cache, b"previous-verified-cache").unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!(
            "http://{}/artifact?signature=do-not-log",
            listener.local_addr().unwrap()
        );
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream.set_nonblocking(false).unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(10)))
                .unwrap();
            let mut request = Vec::new();
            while !request.ends_with(b"\r\n\r\n") {
                let mut byte = [0];
                stream.read_exact(&mut byte).unwrap();
                request.push(byte[0]);
            }
            if mode == 3 {
                std::thread::sleep(Duration::from_millis(250));
                return;
            }
            let size = if mode == 2 { 20 } else { 3 };
            let _ = write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Length: {size}\r\nConnection: close\r\n\r\nbad"
            );
        });
        let expected_size = if mode == 0 || mode == 2 {
            Some(20)
        } else {
            Some(3)
        };
        let result = download_with_timeout(
            &Client::new(),
            &url,
            &cache,
            expected_size,
            Some(&"a".repeat(64)),
            Duration::from_millis(100),
        )
        .await;
        assert!(result.is_err());
        assert!(!format!("{:#}", result.unwrap_err()).contains("do-not-log"));
        server.join().unwrap();
        assert_eq!(std::fs::read(&cache).unwrap(), b"previous-verified-cache");
        assert_eq!(std::fs::read_dir(&root).unwrap().count(), 1);
    }
    std::fs::remove_dir_all(root).unwrap();
}
