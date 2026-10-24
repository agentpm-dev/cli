use super::*;
use serde_json::json;
use std::{net::TcpListener, thread, time::Instant};

struct Workspace(PathBuf);
impl Workspace {
    fn new() -> Self {
        let path =
            std::env::temp_dir().join(format!("agentpm-m11a-test-{:032x}", rand::random::<u128>()));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}
impl Drop for Workspace {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn pylock() -> String {
    format!(
        r#"lock-version = "1.0"
[[packages]]
name = "foo"
version = "1.2.0"
[[packages.wheels]]
url = "https://index.test/foo-1.2.0-py3-none-any.whl"
hashes = {{ sha256 = "{}" }}
[[packages.wheels]]
url = "https://index.test/foo-1.2.0-cp311-cp311-win_amd64.whl"
hashes = {{ sha256 = "{}" }}
[packages.sdist]
url = "https://index.test/foo-1.2.0.tar.gz"
hashes = {{ sha256 = "{}" }}
"#,
        "a".repeat(64),
        "b".repeat(64),
        "c".repeat(64)
    )
}

#[test]
fn structured_resolution_preserves_all_distribution_hashes_without_urls_or_targets() {
    let inputs = ResolutionInputs::new(&["foo>=1".into()], "3.11", UV_VERSION).unwrap();
    let resolved = from_pylock(&pylock(), &inputs).unwrap();
    assert_eq!(resolved.packages()[0].hashes.len(), 3);
    let bytes = String::from_utf8(resolved.canonical_bytes().unwrap()).unwrap();
    assert!(!bytes.contains("index.test"));
    assert!(!bytes.contains("win_amd64"));
    assert!(from_pylock(&pylock().replace("sha256 =", "sha512 ="), &inputs).is_err());
    assert!(
        from_pylock(
            &pylock().replace("lock-version = \"1.0\"", "lock-version = \"2.0\""),
            &inputs
        )
        .is_err()
    );
}

#[test]
fn parser_preserves_windows_only_transitive_packages_and_their_markers() {
    let inputs = ResolutionInputs::new(&["foo>=1".into()], "3.11", UV_VERSION).unwrap();
    let raw = format!(
        "{}\n[[packages]]\nname = 'colorama'\nversion = '0.4.6'\nmarker = \"sys_platform == 'win32'\"\n[[packages.wheels]]\nhashes = {{sha256 = '{}'}}\n",
        pylock(),
        "d".repeat(64)
    );
    let result = from_pylock(&raw, &inputs).unwrap();
    let transitive = result
        .packages()
        .iter()
        .find(|p| p.name == "colorama")
        .unwrap();
    assert_eq!(transitive.version, "0.4.6");
    assert_eq!(transitive.hashes, [format!("sha256:{}", "d".repeat(64))]);
    assert!(transitive.marker.as_deref().unwrap().contains("win32"));
    assert_eq!(result.packages().len(), 2);
}

#[test]
fn http_storage_failures_are_not_reported_as_proxy_failures() {
    for storage in [false, true] {
        for status in [403, 404, 500] {
            let workspace = Workspace::new();
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            listener.set_nonblocking(true).unwrap();
            let base = format!("http://{}", listener.local_addr().unwrap());
            let binary_url = format!("{base}/binary?signature=PRIVATE_SIGNED_URL");
            let worker = thread::spawn(move || {
                for index in 0..if storage { 2 } else { 1 } {
                    let deadline = Instant::now() + Duration::from_secs(10);
                    let (mut stream, _) = loop {
                        match listener.accept() {
                            Ok(connection) => break connection,
                            Err(error)
                                if error.kind() == std::io::ErrorKind::WouldBlock
                                    && Instant::now() < deadline =>
                            {
                                thread::sleep(Duration::from_millis(5))
                            }
                            Err(error) => panic!("missing expected resolver request: {error}"),
                        }
                    };
                    stream.set_nonblocking(false).unwrap();
                    stream
                        .set_read_timeout(Some(Duration::from_secs(2)))
                        .unwrap();
                    let mut buffer = [0; 8192];
                    let size = stream.read(&mut buffer).unwrap();
                    assert!(size > 0, "expected a resolver HTTP request");
                    let (code, body) = if storage && index == 0 {
                        (
                            200,
                            serde_json::to_vec(&json!({"presigned_url":binary_url})).unwrap(),
                        )
                    } else {
                        (status, b"fixture failure".to_vec())
                    };
                    write!(stream, "HTTP/1.1 {code} Fixture\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len()).unwrap();
                    stream.write_all(&body).unwrap();
                }
            });
            let error = ManagedUv::acquire_at(&base, &workspace.0, Target::current_triple(), None)
                .unwrap_err();
            worker.join().unwrap();
            let message = error.to_string();
            assert!(message.contains(&format!("HTTP {status}")), "{message}");
            assert!(!message.contains("network/proxy failure"), "{message}");
            assert!(!message.contains("PRIVATE_SIGNED_URL"), "{message}");
            if status == 404 {
                assert!(
                    message.contains("API resolver-route deployment"),
                    "{message}"
                );
            }
            assert!(
                message.contains(if status == 500 {
                    "storage HTTP failure"
                } else {
                    "not mirrored"
                }),
                "{message}"
            );
            assert!(fs::read_dir(&workspace.0).unwrap().next().is_none());
        }
    }
}

#[test]
fn no_dependency_path_is_lazy_and_does_not_acquire_any_resolver() {
    let workspace = Workspace::new();
    for manifest in [
        json!({"kind":"tool"}),
        json!({"kind":"tool","runtime":{"type":"python","version":"3.11","dependencies":[]}}),
        json!({"kind":"agent"}),
    ] {
        assert!(
            prepare_with(&manifest, &workspace.0, false, || panic!(
                "must not acquire uv"
            ))
            .unwrap()
            .is_none()
        );
    }
    assert!(!workspace.0.join(".agentpm").exists());
    fs::write(
        workspace.0.join("agent.lock"),
        serde_json::to_vec(
            &json!({"lockfile_version":2,"generated":"2026-10-23T00:00:00Z","dependencies":{}}),
        )
        .unwrap(),
    )
    .unwrap();
    let manifest = json!({"kind":"tool","name":"fixture","version":"1.0.0","runtime":{"type":"python","version":"3.11","dependencies":["foo"]}});
    let error = prepare_with(&manifest, &workspace.0, false, || {
        panic!("legacy lock must fail before acquisition")
    })
    .unwrap_err();
    assert!(error.to_string().contains("modern agent.lock"));
}

#[test]
fn non_tool_publication_ignores_unrelated_invalid_locks() {
    let workspace = Workspace::new();
    let lock_path = workspace.0.join("agent.lock");
    for kind in ["agent", "skill"] {
        let manifest = json!({"kind":kind});
        for bytes in [
            "not valid JSON",
            r#"{"lockfile_version":99,"packages":{},"roots":{}}"#,
            r#"{"lockfile_version":2,"generated":"2026-10-23T00:00:00Z","packages":{},"roots":{},"unknown_field":true}"#,
        ] {
            fs::write(&lock_path, bytes).unwrap();
            for refresh in [false, true] {
                assert!(
                    prepare_with(&manifest, &workspace.0, refresh, || panic!(
                        "non-Tool publication must not acquire uv"
                    ))
                    .unwrap()
                    .is_none()
                );
            }
            require_publish_resolution(&manifest, &workspace.0).unwrap();
            assert_eq!(fs::read(&lock_path).unwrap(), bytes.as_bytes());
            assert!(!workspace.0.join("cache").exists());
        }
    }
}

#[test]
fn unsupported_target_fails_before_network_and_names_override() {
    let workspace = Workspace::new();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let registry = format!("http://{}", listener.local_addr().unwrap());
    let error = ManagedUv::acquire_at(&registry, &workspace.0, "x86_64-unknown-linux-musl", None)
        .unwrap_err();
    let message = format!("{error:#}");
    assert!(message.contains("no network was attempted"));
    assert!(message.contains("AGENTPM_UV_PATH"));
    assert!(message.contains("aarch64-apple-darwin"));
    assert_eq!(
        listener.accept().unwrap_err().kind(),
        std::io::ErrorKind::WouldBlock
    );
    assert!(fs::read_dir(&workspace.0).unwrap().next().is_none());
}

#[cfg(unix)]
fn fake_uv(root: &Path, version: &str) -> PathBuf {
    use std::os::unix::fs::PermissionsExt;
    let path = root.join("uv-test");
    let script = format!(
        "#!/bin/sh\nif [ \"$#\" = 1 ] && [ \"$1\" = --version ]; then echo 'uv {version} (fixture)'; exit 0; fi\nif [ \"$#\" = 5 ] && [ \"$*\" = 'lock --no-build --no-python-downloads --no-sources --no-progress' ]; then exit 0; fi\nif [ \"$#\" != 11 ] || [ \"$*\" != 'export --locked --format pylock.toml --output-file pylock.toml --no-dev --no-emit-project --no-python-downloads --no-sources --no-progress' ]; then echo 'unexpected uv argv' >&2; exit 64; fi\necho resolve >> '{}'\ncat > pylock.toml <<'PYLOCK'\n{}\nPYLOCK\n",
        root.join("calls").display(),
        pylock()
    );
    fs::write(&path, script).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
    path
}

#[cfg(unix)]
#[test]
fn fake_resolver_rejects_missing_export_safety_flags_and_unknown_commands() {
    let workspace = Workspace::new();
    let path = fake_uv(&workspace.0, UV_VERSION);
    let commands: Vec<Vec<String>> = serde_json::from_str(include_str!(
        "../../../../platforms/uv-resolution-commands.json"
    ))
    .unwrap();
    for missing in ["--locked", "--no-emit-project", "--no-build"] {
        let original = commands
            .iter()
            .find(|args| args.iter().any(|s| s == missing))
            .unwrap();
        let output = Command::new(&path)
            .args(original.iter().filter(|s| *s != missing))
            .current_dir(&workspace.0)
            .output()
            .unwrap();
        assert!(!output.status.success(), "fake accepted missing {missing}");
        assert!(String::from_utf8_lossy(&output.stderr).contains("unexpected uv argv"));
    }
    assert!(
        !Command::new(path)
            .arg("unexpected")
            .status()
            .unwrap()
            .success()
    );
    assert!(!workspace.0.join("pylock.toml").exists());
}

#[cfg(unix)]
#[test]
fn override_is_exact_and_version_checked_before_each_execution() {
    let workspace = Workspace::new();
    let path = fake_uv(&workspace.0, UV_VERSION);
    let resolver = ManagedUv::acquire_at(
        "http://127.0.0.1:1",
        &workspace.0,
        "unsupported",
        Some(path.clone()),
    )
    .unwrap();
    resolver.command().unwrap();
    fake_uv(&workspace.0, "0.0.0");
    let error = resolver.command().unwrap_err();
    assert!(error.to_string().contains("version mismatch"));
    assert!(error.to_string().contains(UV_VERSION));
    assert!(
        ManagedUv::acquire_at(
            "http://127.0.0.1:1",
            &workspace.0,
            "unsupported",
            Some(path)
        )
        .is_err()
    );
}

#[cfg(unix)]
#[test]
fn cached_binary_reuses_without_network_and_tampering_fails() {
    let workspace = Workspace::new();
    let source = fake_uv(&workspace.0, UV_VERSION);
    let target = Target::current().unwrap().to_string();
    let table = PinTable {
        version: UV_VERSION.into(),
        artifacts: BTreeMap::from([(
            target.clone(),
            Pin {
                sha256: binary_digest(&source).unwrap(),
            },
        )]),
    };
    let cache = workspace
        .0
        .join("runtimes/uv")
        .join(UV_VERSION)
        .join(&target);
    fs::create_dir_all(&cache).unwrap();
    fs::copy(&source, cache.join("uv")).unwrap();
    let uv =
        ManagedUv::acquire_pinned("http://127.0.0.1:1", &workspace.0, &target, &table).unwrap();
    uv.command().unwrap();
    fs::write(cache.join("uv"), "tampered").unwrap();
    assert!(
        uv.command()
            .unwrap_err()
            .to_string()
            .contains("digest mismatch")
    );
    assert!(
        ManagedUv::acquire_pinned("http://127.0.0.1:1", &workspace.0, &target, &table)
            .unwrap_err()
            .to_string()
            .contains("digest mismatch")
    );
}

#[cfg(unix)]
#[test]
fn committed_author_lock_reuses_exact_inputs_without_cache_and_refresh_reexecutes() {
    let workspace = Workspace::new();
    let path = fake_uv(&workspace.0, UV_VERSION);
    let manifest = json!({"kind":"tool","name":"fixture","version":"0.1.0","runtime":{"type":"python","version":"3.11","dependencies":["foo>=1"]}});
    let first = prepare_with(&manifest, &workspace.0, false, || {
        Ok(ManagedUv {
            path: path.clone(),
            digest: None,
        })
    })
    .unwrap()
    .unwrap();
    fs::remove_dir_all(workspace.0.join("cache")).unwrap();
    let second = prepare_with(&manifest, &workspace.0, false, || {
        panic!("must reuse matching state")
    })
    .unwrap()
    .unwrap();
    assert_eq!(first, second);
    prepare_with(&manifest, &workspace.0, true, || {
        Ok(ManagedUv {
            path: path.clone(),
            digest: None,
        })
    })
    .unwrap();
    assert_eq!(
        fs::read_to_string(workspace.0.join("calls"))
            .unwrap()
            .lines()
            .count(),
        2
    );
    let lock = crate::manifest::read_lock_or_default(&workspace.0).unwrap();
    assert_eq!(lock.lockfile_version(), 4);
    let crate::semver::types::Lock::V2(lock) = lock else {
        panic!()
    };
    assert!(lock.packages.is_empty());
    assert_eq!(
        lock.roots["local:tool"].python_resolution.as_ref(),
        Some(&first)
    );
    let mut changed = manifest.clone();
    changed["runtime"]["dependencies"] = json!(["foo>=1.1"]);
    prepare_with(&changed, &workspace.0, false, || {
        Ok(ManagedUv { path, digest: None })
    })
    .unwrap();
    assert_eq!(
        fs::read_to_string(workspace.0.join("calls"))
            .unwrap()
            .lines()
            .count(),
        3
    );
}

#[cfg(unix)]
#[test]
fn stale_author_lock_cannot_publish_and_corrupt_disposable_cache_fails_closed() {
    let workspace = Workspace::new();
    let path = fake_uv(&workspace.0, UV_VERSION);
    let manifest = json!({"kind":"tool","name":"fixture","version":"0.1.0","runtime":{"type":"python","version":"3.11","dependencies":["foo>=1"]}});
    let acquire = || {
        Ok(ManagedUv {
            path: path.clone(),
            digest: None,
        })
    };
    prepare_with(&manifest, &workspace.0, false, acquire).unwrap();
    let cache = fs::read_dir(workspace.0.join("cache/python-resolutions"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    for fingerprint in [json!(null), json!("old-fingerprint")] {
        let path = workspace.0.join("agent.lock");
        let mut lock: serde_json::Value =
            serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        lock["roots"]["local:tool"]["python_input_fingerprint"] = fingerprint;
        fs::write(path, serde_json::to_vec(&lock).unwrap()).unwrap();
        assert!(
            require_publish_resolution(&manifest, &workspace.0)
                .unwrap_err()
                .to_string()
                .contains("missing or stale")
        );
        prepare_with(&manifest, &workspace.0, true, acquire).unwrap();
    }
    assert_eq!(
        fs::read_to_string(workspace.0.join("calls"))
            .unwrap()
            .lines()
            .count(),
        3
    );
    let mut record: serde_json::Value = serde_json::from_slice(&fs::read(&cache).unwrap()).unwrap();
    record["resolution_digest"] = json!("tampered");
    fs::write(&cache, serde_json::to_vec(&record).unwrap()).unwrap();
    // The committed lock is authoritative even if a disposable cache is corrupt.
    prepare_with(&manifest, &workspace.0, false, || panic!("must reuse lock")).unwrap();
    fs::remove_file(workspace.0.join("agent.lock")).unwrap();
    assert!(
        prepare_with(&manifest, &workspace.0, false, || panic!(
            "corrupt state must fail before acquisition"
        ))
        .unwrap_err()
        .to_string()
        .contains("corrupted")
    );
}

#[test]
fn empty_lock_is_valid_only_when_every_root_is_inapplicable() {
    let inapplicable = ResolutionInputs::new(
        &["foo; sys_platform == 'freebsd'".into()],
        "3.11",
        UV_VERSION,
    )
    .unwrap();
    assert!(
        from_pylock("lock-version = '1.0'", &inapplicable)
            .unwrap()
            .packages()
            .is_empty()
    );
    let applicable = ResolutionInputs::new(&["foo".into()], "3.11", UV_VERSION).unwrap();
    assert!(from_pylock("lock-version = '1.0'", &applicable).is_err());
}

fn serve_two(
    responder: impl Fn(usize, &str, &str) -> Vec<u8> + Send + 'static,
) -> (String, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let server_base = base.clone();
    let worker = thread::spawn(move || {
        let until = Instant::now() + Duration::from_secs(10);
        let mut count = 0;
        while count < 2 && Instant::now() < until {
            let Ok((mut stream, _)) = listener.accept() else {
                thread::sleep(Duration::from_millis(5));
                continue;
            };
            stream.set_nonblocking(false).unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let mut bytes = [0; 8192];
            let read = stream.read(&mut bytes).unwrap();
            let request = String::from_utf8_lossy(&bytes[..read]);
            assert!(!request.to_ascii_lowercase().contains("authorization:"));
            let response = responder(count, &request, &server_base);
            write!(stream, "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n", response.len()).unwrap();
            stream.write_all(&response).unwrap();
            count += 1;
        }
        assert_eq!(count, 2);
    });
    (base, worker)
}

#[test]
fn downloaded_digest_mismatch_is_not_cached_or_executed() {
    let workspace = Workspace::new();
    let (base, server) = serve_two(|count, request, base| {
        if count == 0 {
            assert!(request.starts_with("GET /v1/tools/install/resolvers/uv/"));
            serde_json::to_vec(&json!({"presigned_url":format!("{base}/binary")})).unwrap()
        } else {
            b"tampered binary".to_vec()
        }
    });
    let error =
        ManagedUv::acquire_at(&base, &workspace.0, Target::current_triple(), None).unwrap_err();
    server.join().unwrap();
    assert!(error.to_string().contains("digest mismatch"));
    assert!(fs::read_dir(&workspace.0).unwrap().next().is_none());
}

#[test]
fn network_failure_has_its_own_operator_message() {
    let workspace = Workspace::new();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    drop(listener);
    let error =
        ManagedUv::acquire_at(&endpoint, &workspace.0, Target::current_triple(), None).unwrap_err();
    assert!(error.to_string().contains("network/proxy failure"));
}

#[cfg(unix)]
#[test]
fn valid_download_is_verified_cached_and_reused_offline() {
    let workspace = Workspace::new();
    let source = fake_uv(&workspace.0, UV_VERSION);
    let bytes = fs::read(source).unwrap();
    let target = Target::current().unwrap().to_string();
    let table = PinTable {
        version: UV_VERSION.into(),
        artifacts: BTreeMap::from([(
            target.clone(),
            Pin {
                sha256: format!("{:x}", Sha256::digest(&bytes)),
            },
        )]),
    };
    let (endpoint, server) = serve_two(move |count, _, base| {
        if count == 0 {
            serde_json::to_vec(&json!({"presigned_url":format!("{base}/binary")})).unwrap()
        } else {
            bytes.clone()
        }
    });
    let cache = workspace.0.join("cache");
    ManagedUv::acquire_pinned(&endpoint, &cache, &target, &table)
        .unwrap_or_else(|error| panic!("managed resolver acquisition failed: {error:#}"));
    server.join().unwrap();
    // The server is gone, so successful acquisition here must use the verified cache.
    ManagedUv::acquire_pinned(&endpoint, &cache, &target, &table)
        .unwrap()
        .command()
        .unwrap();
}

#[cfg(unix)]
#[test]
fn resolver_execution_failure_is_not_a_transport_or_version_error() {
    use std::os::unix::fs::PermissionsExt;
    let workspace = Workspace::new();
    let path = workspace.0.join("failing-uv");
    fs::write(&path, format!("#!/bin/sh\nif [ \"$1\" = --version ]; then echo 'uv {UV_VERSION}'; exit 0; fi\necho 'fixture dependency conflict' >&2\nexit 42\n")).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
    let inputs = ResolutionInputs::new(&["foo".into()], "3.11", UV_VERSION).unwrap();
    let error = resolve(&ManagedUv { path, digest: None }, &inputs).unwrap_err();
    assert!(error.to_string().contains("resolver executed but failed"));
    assert!(error.to_string().contains("fixture dependency conflict"));
}
