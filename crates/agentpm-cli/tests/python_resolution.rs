use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
};

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root =
            std::env::temp_dir().join(format!("agentpm-m11a-cli-{:032x}", rand::random::<u128>()));
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("tool.py"), "print('{}')\n").unwrap();
        Self(root)
    }
    fn manifest(&self, runtime: Value) {
        let value = json!({"kind":"tool","name":"python-dependency-test","version":"0.1.0","description":"Python dependency test.",
            "entrypoint":{"command":if runtime["type"] == "node" {"node"} else {"python3"},"args":["tool.py"]},
            "runtime":runtime,"files":["tool.py"],"inputs":{"type":"object"},"outputs":{"type":"object"}});
        fs::write(
            self.0.join("agent.json"),
            serde_json::to_vec(&value).unwrap(),
        )
        .unwrap();
    }
    fn command(&self, verb: &str) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_agentpm"));
        command
            .current_dir(&self.0)
            .env("HOME", self.0.join("home"))
            .env("USERPROFILE", self.0.join("home"))
            .env("XDG_CONFIG_HOME", self.0.join("home/config"))
            .env("XDG_DATA_HOME", self.0.join("home/data"))
            .env("XDG_CACHE_HOME", self.0.join("cache"))
            .env("AGENTPM_TELEMETRY", "0")
            .arg(verb);
        if matches!(verb, "lint" | "publish") {
            command.args(["--schema", &schema().to_string_lossy()]);
        }
        command
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn schema() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../schemas/agentpm.manifest.schema.json")
}
fn text(output: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

#[test]
fn agent_and_skill_dry_runs_ignore_invalid_or_future_locks() {
    for kind in ["agent", "skill"] {
        let fixture = Fixture::new();
        let init = fixture
            .command("init")
            .args(["--kind", kind, "--name", "non-tool-publish"])
            .output()
            .unwrap();
        assert!(init.status.success(), "{}", text(&init));
        let lint = fixture.command("lint").arg("--strict").output().unwrap();
        assert!(lint.status.success(), "{}", text(&lint));
        let lock_path = fixture.0.join("agent.lock");
        for bytes in [
            "not valid JSON",
            r#"{"lockfile_version":99,"packages":{},"roots":{}}"#,
        ] {
            fs::write(&lock_path, bytes).unwrap();
            let output = fixture
                .command("publish")
                .env("AGENTPM_UV_PATH", fixture.0.join("must-not-execute"))
                .args(["--dry-run", "--token", "fixture-only", "--quiet"])
                .output()
                .unwrap();
            assert!(output.status.success(), "{}", text(&output));
            assert_eq!(fs::read(&lock_path).unwrap(), bytes.as_bytes());
        }
    }
}

#[test]
fn cold_live_publish_rejects_dependencies_before_any_acquisition_or_preparation() {
    use std::net::TcpListener;
    let fixture = Fixture::new();
    fixture.manifest(json!({"type":"python","version":"3.11","dependencies":["foo>=1"]}));
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    for override_path in [None, Some(fixture.0.join("must-not-execute"))] {
        let mut command = fixture.command("publish");
        command
            .env("HOME", fixture.0.join("home"))
            .env("USERPROFILE", fixture.0.join("home"))
            .env("XDG_CACHE_HOME", fixture.0.join("cache"))
            .env(
                "AGENTPM_BASE_URL",
                format!("http://{}", listener.local_addr().unwrap()),
            )
            .env_remove("AGENTPM_UV_PATH")
            .args(["--token", "fixture-only", "--quiet"]);
        if let Some(path) = override_path {
            command.env("AGENTPM_UV_PATH", path);
        }
        let output = command.output().unwrap();
        assert!(!output.status.success());
        assert!(
            text(&output).contains("missing or stale"),
            "{}",
            text(&output)
        );
        assert!(!text(&output).contains("Python dependencies resolved"));
        assert!(!fixture.0.join("target/agentpm/python-resolutions").exists());
        assert!(!fixture.0.join("agent.lock").exists());
        assert_eq!(
            listener.accept().unwrap_err().kind(),
            std::io::ErrorKind::WouldBlock
        );
    }
}

#[test]
fn live_refresh_rejects_a_valid_lock_before_resolution_mutation_or_upload() {
    use agentpm_sdk::python::ResolutionInputs;
    use std::net::TcpListener;

    let fixture = Fixture::new();
    fixture.manifest(json!({"type":"python","version":"3.11","dependencies":["foo>=1"]}));
    let resolution: Value = serde_json::from_str(include_str!(
        "../../agentpm-sdk/contracts/python-resolution-v1.json"
    ))
    .unwrap();
    let fingerprint = ResolutionInputs::new(&["foo>=1".into()], "3.11", env!("AGENTPM_UV_VERSION"))
        .unwrap()
        .fingerprint()
        .unwrap();
    let lock = json!({"lockfile_version":4,"generated":"2026-10-23T00:00:00Z","packages":{},
        "roots":{"local:tool":{"name":"python-dependency-test","version":"0.1.0","tools":[],
            "python_resolution":resolution,"python_input_fingerprint":fingerprint}}});
    let bytes = serde_json::to_vec_pretty(&lock).unwrap();
    let lock_path = fixture.0.join("agent.lock");
    fs::write(&lock_path, &bytes).unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let output = fixture
        .command("publish")
        .env(
            "AGENTPM_BASE_URL",
            format!("http://{}", listener.local_addr().unwrap()),
        )
        .env("AGENTPM_UV_PATH", fixture.0.join("must-not-execute"))
        .args(["--token", "fixture-only", "--quiet", "--refresh-python"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(
        text(&output).contains("--refresh-python requires --dry-run"),
        "{}",
        text(&output)
    );
    assert_eq!(fs::read(&lock_path).unwrap(), bytes);
    assert!(!fixture.0.join("target").exists());
    assert!(!fixture.0.join("home").exists());
    assert!(!fixture.0.join("cache").exists());
    assert_eq!(
        listener.accept().unwrap_err().kind(),
        std::io::ErrorKind::WouldBlock
    );
}

#[test]
fn lint_accepts_python_dependencies_and_rejects_invalid_or_node_declarations() {
    let fixture = Fixture::new();
    for runtime in [
        json!({"type":"python","version":"3.11"}),
        json!({"type":"python","version":"3.11","dependencies":["pydantic>=2,<3"]}),
    ] {
        fixture.manifest(runtime);
        let output = fixture.command("lint").output().unwrap();
        assert!(output.status.success(), "{}", text(&output));
    }
    for dependencies in [
        json!(["bad ** requirement"]),
        json!(["foo>=2,<1"]),
        json!(["foo", "FOO>=1"]),
        json!(["-r requirements.txt"]),
    ] {
        fixture.manifest(json!({"type":"python","version":"3.11","dependencies":dependencies}));
        let output = fixture.command("lint").output().unwrap();
        assert!(!output.status.success());
        assert!(
            text(&output).contains("/runtime/dependencies"),
            "{}",
            text(&output)
        );
    }
    fixture.manifest(json!({"type":"node","version":"18","dependencies":[]}));
    assert!(!fixture.command("lint").output().unwrap().status.success());
}

#[cfg(unix)]
#[test]
fn publish_prepares_committed_resolution_and_reuses_it_with_a_cold_cache() {
    use std::os::unix::fs::PermissionsExt;
    let fixture = Fixture::new();
    fixture.manifest(json!({"type":"python","version":"3.11","dependencies":["foo>=1"]}));
    let uv = fixture.0.join("fixture-uv");
    let script = format!(
        "#!/bin/sh\nif [ \"$#\" = 1 ] && [ \"$1\" = --version ]; then echo 'uv {}'; exit 0; fi\nif [ \"$#\" = 5 ] && [ \"$*\" = 'lock --no-build --no-python-downloads --no-sources --no-progress' ]; then exit 0; fi\nif [ \"$#\" != 11 ] || [ \"$*\" != 'export --locked --format pylock.toml --output-file pylock.toml --no-dev --no-emit-project --no-python-downloads --no-sources --no-progress' ]; then echo 'unexpected uv argv' >&2; exit 64; fi\ncat > pylock.toml <<'LOCK'\nlock-version = \"1.0\"\n[[packages]]\nname = \"foo\"\nversion = \"1.2.0\"\n[[packages.wheels]]\nhashes = {{sha256 = \"{}\"}}\nLOCK\n",
        env!("AGENTPM_UV_VERSION"),
        "a".repeat(64)
    );
    fs::write(&uv, script).unwrap();
    fs::set_permissions(&uv, fs::Permissions::from_mode(0o755)).unwrap();
    let dry = fixture
        .command("publish")
        .env("AGENTPM_UV_PATH", &uv)
        .args([
            "--token",
            "fixture-only",
            "--dry-run",
            "--refresh-python",
            "--quiet",
        ])
        .output()
        .unwrap();
    assert!(dry.status.success(), "{}", text(&dry));
    assert!(text(&dry).contains("Python dependencies resolved: 1"));
    let stderr = String::from_utf8_lossy(&dry.stderr);
    let summary = stderr
        .lines()
        .find(|line| line.starts_with("✓ Python dependencies resolved: 1 distributions, sha256:"))
        .expect("quiet/captured output must retain the checkmarked resolution summary");
    let digest = summary.split_once("sha256:").unwrap().1;
    assert_eq!(digest.len(), 64);
    assert!(
        digest
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    );
    // A fresh machine has the committed lock but no author cache or resolver.
    let lock_path = fixture.0.join("agent.lock");
    let prepared_lock = fs::read(&lock_path).unwrap();
    for path in [fixture.0.join("home"), fixture.0.join("cache")] {
        // Which directories get created depends on the OS's config/cache layout.
        match fs::remove_dir_all(&path) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => panic!("could not clear {}: {error}", path.display()),
        }
        assert!(!path.exists(), "{} must be cold", path.display());
    }
    fs::remove_file(&uv).unwrap();
    let cold = fixture
        .command("publish")
        .env("AGENTPM_UV_PATH", "/must-not-execute-on-cache-reuse")
        .args(["--token", "fixture-only", "--dry-run", "--quiet"])
        .output()
        .unwrap();
    assert!(cold.status.success(), "{}", text(&cold));
    assert_eq!(fs::read(&lock_path).unwrap(), prepared_lock);
    let lock: Value =
        serde_json::from_slice(&fs::read(fixture.0.join("agent.lock")).unwrap()).unwrap();
    assert_eq!(lock["lockfile_version"], 4);
    assert_eq!(lock["packages"], json!({}));
    assert_eq!(
        lock["roots"]["local:tool"]["python_resolution"]["packages"][0]["name"],
        "foo"
    );
    assert!(!fixture.0.join("target/agentpm/python-resolutions").exists());
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let endpoint = base.clone();
    let expected_digest = digest.to_owned();
    let worker = std::thread::spawn(move || {
        use std::io::{Read, Write};
        let mut digests = Vec::new();
        for index in 0..6 {
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
            let (mut stream, _) = loop {
                match listener.accept() {
                    Ok(connection) => break connection,
                    Err(e)
                        if e.kind() == std::io::ErrorKind::WouldBlock
                            && std::time::Instant::now() < deadline =>
                    {
                        std::thread::sleep(std::time::Duration::from_millis(5))
                    }
                    Err(e) => panic!("Missing publish request: {e}"),
                }
            };
            stream.set_nonblocking(false).unwrap();
            stream
                .set_read_timeout(Some(std::time::Duration::from_secs(3)))
                .unwrap();
            let mut bytes = Vec::new();
            let header_end = loop {
                let mut buffer = [0; 8192];
                let n = stream.read(&mut buffer).unwrap();
                assert!(n > 0);
                bytes.extend_from_slice(&buffer[..n]);
                if let Some(i) = bytes.windows(4).position(|v| v == b"\r\n\r\n") {
                    break i + 4;
                }
            };
            let headers = String::from_utf8_lossy(&bytes[..header_end]).to_string();
            let length: usize = headers
                .lines()
                .find_map(|l| {
                    l.to_ascii_lowercase()
                        .strip_prefix("content-length:")
                        .map(|s| s.trim().parse().unwrap())
                })
                .unwrap_or(0);
            while bytes.len() < header_end + length {
                let mut buffer = [0; 8192];
                let n = stream.read(&mut buffer).unwrap();
                assert!(n > 0);
                bytes.extend_from_slice(&buffer[..n]);
            }
            let response = match index % 3 {
                0 => {
                    assert!(headers.starts_with("POST /v1/tools/publish/init "));
                    let metadata: Value = serde_json::from_slice(&bytes[header_end..]).unwrap();
                    assert_eq!(metadata["python_resolution_digest"], expected_digest);
                    assert_eq!(
                        metadata["artifact"],
                        json!({"target":"any","format":"tar.gz","diagnostics":[]})
                    );
                    // Publisher-machine telemetry is not payload compatibility.
                    assert_eq!(metadata["client"]["os"], std::env::consts::OS);
                    assert_eq!(metadata["client"]["arch"], std::env::consts::ARCH);
                    digests.push(metadata["python_resolution_digest"].clone());
                    json!({"upload_id":"fixture-upload","put_url":format!("{endpoint}/object"),"headers":{},"final_key":"tool.tar.gz","expires_in":900}).to_string()
                }
                1 => {
                    assert!(headers.starts_with("PUT /object "));
                    let mut archive =
                        tar::Archive::new(flate2::read::GzDecoder::new(&bytes[header_end..]));
                    let mut found = false;
                    for entry in archive.entries().unwrap() {
                        let mut entry = entry.unwrap();
                        let name = entry.path().unwrap().to_string_lossy().to_string();
                        assert_ne!(name, "agent.lock");
                        if name == agentpm_sdk::python::RESOLUTION_METADATA_FILE {
                            let metadata: agentpm_sdk::python::PythonReleaseMetadata =
                                serde_json::from_reader(&mut entry).unwrap();
                            metadata.validate().unwrap();
                            assert_eq!(metadata.python_resolution_digest, expected_digest);
                            found = true;
                        }
                    }
                    assert!(found);
                    String::new()
                }
                _ => {
                    assert!(headers.starts_with("POST /v1/tools/publish/finalize "));
                    json!({"id":"fixture","name":"fixture","version":"0.1.0","url":"https://fixture.invalid"}).to_string()
                }
            };
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                response.len(),
                response
            )
            .unwrap();
        }
        assert_eq!(digests[0], digests[1]);
    });
    for _ in 0..2 {
        let live = fixture
            .command("publish")
            .env("AGENTPM_BASE_URL", &base)
            .env("AGENTPM_UV_PATH", "/must-not-execute-with-committed-lock")
            .args(["--token", "fixture-only", "--quiet"])
            .output()
            .unwrap();
        assert!(live.status.success(), "{}", text(&live));
    }
    worker.join().unwrap();
    // Full regeneration must keep author state while dropping packages outside the plan.
    let lock_path = fixture.0.join("agent.lock");
    let mut before: Value = serde_json::from_slice(&fs::read(&lock_path).unwrap()).unwrap();
    before["packages"]["tool:@old/tool@1.0.0"] =
        json!({"kind":"tool","name":"@old/tool","version":"1.0.0","integrity":"a".repeat(64)});
    fs::write(&lock_path, serde_json::to_vec(&before).unwrap()).unwrap();
    let installed = fixture
        .command("install")
        .args(["--quiet"])
        .output()
        .unwrap();
    assert!(installed.status.success(), "{}", text(&installed));
    let after: Value = serde_json::from_slice(&fs::read(&lock_path).unwrap()).unwrap();
    assert_eq!(after["roots"]["local:tool"], before["roots"]["local:tool"]);
    assert_eq!(after["packages"], json!({}));
    let unchanged = fs::read(&lock_path).unwrap();
    assert!(
        fixture
            .command("install")
            .args(["--quiet"])
            .output()
            .unwrap()
            .status
            .success()
    );
    assert_eq!(
        fs::read(&lock_path).unwrap(),
        unchanged,
        "unchanged install must not rewrite generated or reorder keys"
    );
    let mut manifest: Value =
        serde_json::from_slice(&fs::read(fixture.0.join("agent.json")).unwrap()).unwrap();
    manifest["runtime"]
        .as_object_mut()
        .unwrap()
        .remove("dependencies");
    fs::write(
        fixture.0.join("agent.json"),
        serde_json::to_vec(&manifest).unwrap(),
    )
    .unwrap();
    assert!(
        fixture
            .command("install")
            .args(["--quiet"])
            .output()
            .unwrap()
            .status
            .success()
    );
    let removed: Value = serde_json::from_slice(&fs::read(&lock_path).unwrap()).unwrap();
    assert!(
        removed["roots"]["local:tool"]
            .get("python_resolution")
            .is_none()
    );
    assert!(
        removed["roots"]["local:tool"]
            .get("python_input_fingerprint")
            .is_none()
    );
    assert_eq!(
        removed["lockfile_version"], 4,
        "regeneration must not downgrade an existing lock"
    );
    // A pre-v4 Tool project may have been mislabeled local:agent. Migrate, don't duplicate it.
    let mut legacy = after.clone();
    let mut root = legacy["roots"]
        .as_object_mut()
        .unwrap()
        .remove("local:tool")
        .unwrap();
    root.as_object_mut().unwrap().remove("python_resolution");
    root.as_object_mut()
        .unwrap()
        .remove("python_input_fingerprint");
    legacy["roots"]["local:agent"] = root;
    legacy["lockfile_version"] = json!(2);
    fs::write(&lock_path, serde_json::to_vec(&legacy).unwrap()).unwrap();
    let migrated = fixture
        .command("install")
        .args(["--quiet"])
        .output()
        .unwrap();
    assert!(migrated.status.success(), "{}", text(&migrated));
    let migrated: Value = serde_json::from_slice(&fs::read(&lock_path).unwrap()).unwrap();
    assert!(migrated["roots"].get("local:agent").is_none());
    assert_eq!(
        migrated["roots"]["local:tool"]["name"],
        "python-dependency-test"
    );
}

#[test]
fn future_lock_fails_before_publish_or_install_mutation() {
    let fixture = Fixture::new();
    fixture.manifest(json!({"type":"python","version":"3.11","dependencies":["foo>=1"]}));
    let bytes = br#"{"lockfile_version":99,"packages":{},"future_state":{"keep":"all"}}"#;
    fs::write(fixture.0.join("agent.lock"), bytes).unwrap();
    for verb in ["publish", "install"] {
        let mut command = fixture.command(verb);
        if verb == "publish" {
            command.args(["--token", "fixture-only", "--dry-run"]);
        }
        let result = command.output().unwrap();
        assert!(!result.status.success());
        assert!(
            text(&result).contains("Unsupported agent.lock version 99"),
            "{}",
            text(&result)
        );
        assert_eq!(fs::read(fixture.0.join("agent.lock")).unwrap(), bytes);
    }
    assert!(!fixture.0.join("target").exists());
}

#[test]
fn frozen_python_tool_rejects_pre_v4_lock_with_regeneration_guidance() {
    let fixture = Fixture::new();
    fixture.manifest(json!({"type":"python","version":"3.11","dependencies":["foo>=1"]}));
    for version in [2, 3] {
        let bytes=serde_json::to_vec(&json!({"lockfile_version":version,"generated":"2026-10-23T00:00:00Z","packages":{},"roots":{}})).unwrap();
        fs::write(fixture.0.join("agent.lock"), &bytes).unwrap();
        let result = fixture
            .command("install")
            .env("AGENTPM_BASE_URL", "http://127.0.0.1:1")
            .args(["--frozen", "--quiet"])
            .output()
            .unwrap();
        assert!(!result.status.success());
        assert!(
            text(&result).contains("run `agentpm install` without `--frozen`"),
            "{}",
            text(&result)
        );
        assert_eq!(fs::read(fixture.0.join("agent.lock")).unwrap(), bytes);
    }
}

#[test]
fn frozen_consumer_rejects_old_lock_when_registry_reports_python_dependencies() {
    use std::io::{Read, Write};
    use std::net::TcpListener;
    for snapshot in [
        include_bytes!("fixtures/locks/pre11b-v2.json").as_slice(),
        include_bytes!("fixtures/locks/pre11b-v3.json").as_slice(),
    ] {
        let fixture = Fixture::new();
        assert!(
            fixture
                .command("init")
                .args(["--kind", "agent", "--name", "frozen-consumer"])
                .output()
                .unwrap()
                .status
                .success()
        );
        let manifest_path = fixture.0.join("agent.json");
        let mut manifest: Value =
            serde_json::from_slice(&fs::read(&manifest_path).unwrap()).unwrap();
        manifest["tools"] = json!(["@fixture/component@0.1.0"]);
        fs::write(&manifest_path, serde_json::to_vec(&manifest).unwrap()).unwrap();
        let lint = fixture.command("lint").args(["--strict"]).output().unwrap();
        assert!(lint.status.success(), "{}", text(&lint));
        fs::write(fixture.0.join("agent.lock"), snapshot).unwrap();
        let lock: Value = serde_json::from_slice(snapshot).unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let artifact = json!({"kind":"tool","name":"@fixture/component","version":"0.1.0",
            "integrity":lock["packages"]["tool:@fixture/component@0.1.0"]["integrity"],
            "presigned_url":format!("{base}/must-not-download"),
            "python_resolution":serde_json::from_str::<Value>(include_str!("../../agentpm-sdk/contracts/python-resolution-v1.json")).unwrap()});
        let worker = std::thread::spawn(move || {
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
            let (mut stream, _) = loop {
                match listener.accept() {
                    Ok(connection) => break connection,
                    Err(e)
                        if e.kind() == std::io::ErrorKind::WouldBlock
                            && std::time::Instant::now() < deadline =>
                    {
                        std::thread::sleep(std::time::Duration::from_millis(5))
                    }
                    Err(e) => panic!("missing install:init request: {e}"),
                }
            };
            stream.set_nonblocking(false).unwrap();
            stream
                .set_read_timeout(Some(std::time::Duration::from_secs(3)))
                .unwrap();
            let mut bytes = Vec::new();
            let end = loop {
                let mut buffer = [0; 8192];
                let n = stream.read(&mut buffer).unwrap();
                assert!(n > 0);
                bytes.extend_from_slice(&buffer[..n]);
                if let Some(i) = bytes.windows(4).position(|v| v == b"\r\n\r\n") {
                    break i + 4;
                }
            };
            let headers = String::from_utf8_lossy(&bytes[..end]);
            assert!(headers.starts_with("POST /v1/tools/install/init "));
            let length: usize = headers
                .lines()
                .find_map(|l| {
                    l.to_ascii_lowercase()
                        .strip_prefix("content-length:")
                        .map(|s| s.trim().parse().unwrap())
                })
                .unwrap();
            while bytes.len() < end + length {
                let mut buffer = [0; 8192];
                let n = stream.read(&mut buffer).unwrap();
                assert!(n > 0);
                bytes.extend_from_slice(&buffer[..n]);
            }
            let request: Value = serde_json::from_slice(&bytes[end..]).unwrap();
            assert!(request["items"][0].get("python_resolution").is_none());
            let response=json!({"session_id":"fixture-session","expires_at":"2099-01-01T00:00:00Z","artifacts":[artifact]}).to_string();
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                response.len(),
                response
            )
            .unwrap();
            listener
        });
        let result = fixture
            .command("install")
            .env("AGENTPM_BASE_URL", base)
            .args(["--frozen", "--quiet"])
            .output()
            .unwrap();
        assert!(!result.status.success());
        assert!(
            text(&result).contains("run `agentpm install` without `--frozen`"),
            "{}",
            text(&result)
        );
        assert_eq!(fs::read(fixture.0.join("agent.lock")).unwrap(), snapshot);
        let listener = worker.join().unwrap();
        assert_eq!(
            listener.accept().unwrap_err().kind(),
            std::io::ErrorKind::WouldBlock
        );
    }
}

#[cfg(unix)]
#[test]
fn arbitrary_path_uv_is_never_used_as_a_fallback() {
    use std::os::unix::fs::PermissionsExt;
    let fixture = Fixture::new();
    fixture.manifest(json!({"type":"python","version":"3.11","dependencies":["foo"]}));
    let binaries = fixture.0.join("bin");
    fs::create_dir(&binaries).unwrap();
    let uv = binaries.join("uv");
    fs::write(
        &uv,
        format!(
            "#!/bin/sh\necho UNEXPECTED > '{}'\necho 'uv 0.0.0'\n",
            fixture.0.join("path-uv-executed").display()
        ),
    )
    .unwrap();
    fs::set_permissions(&uv, fs::Permissions::from_mode(0o755)).unwrap();
    let output = fixture
        .command("publish")
        .env("PATH", &binaries)
        .env_remove("AGENTPM_UV_PATH")
        .env("HOME", fixture.0.join("home"))
        .env("USERPROFILE", fixture.0.join("home"))
        .env("XDG_CACHE_HOME", fixture.0.join("cache"))
        .env("NO_PROXY", "*")
        .env("AGENTPM_BASE_URL", "http://127.0.0.1:1")
        .args(["--token", "fixture-only", "--dry-run", "--quiet"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(!fixture.0.join("path-uv-executed").exists());
    assert!(
        text(&output).contains("network/proxy failure"),
        "{}",
        text(&output)
    );
    let bare_override = fixture
        .command("publish")
        .env("PATH", &binaries)
        .env("AGENTPM_UV_PATH", "uv")
        .args(["--token", "fixture-only", "--dry-run", "--quiet"])
        .output()
        .unwrap();
    assert!(!bare_override.status.success());
    assert!(
        text(&bare_override).contains("PATH lookup is not used"),
        "{}",
        text(&bare_override)
    );
    assert!(!fixture.0.join("path-uv-executed").exists());
}

#[test]
fn resolver_download_honors_http_proxy_without_forwarding_publish_credentials() {
    use std::{
        io::{Read, Write},
        net::TcpListener,
        thread,
        time::{Duration, Instant},
    };
    let fixture = Fixture::new();
    fixture.manifest(json!({"type":"python","version":"3.11","dependencies":["foo"]}));
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let proxy = format!("http://{}", listener.local_addr().unwrap());
    let storage = format!("{proxy}/binary");
    let server = thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(10);
        let mut count = 0;
        while count < 2 && Instant::now() < deadline {
            let Ok((mut stream, _)) = listener.accept() else {
                thread::sleep(Duration::from_millis(5));
                continue;
            };
            stream.set_nonblocking(false).unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let mut buffer = [0; 8192];
            let size = stream.read(&mut buffer).unwrap();
            let request = String::from_utf8_lossy(&buffer[..size]);
            assert!(!request.to_ascii_lowercase().contains("authorization:"));
            let response = if count == 0 {
                assert!(request.starts_with(
                    "GET http://registry-m11a.invalid/v1/tools/install/resolvers/uv/"
                ));
                serde_json::to_vec(&json!({"presigned_url":storage})).unwrap()
            } else {
                b"tampered binary".to_vec()
            };
            write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", response.len()).unwrap();
            stream.write_all(&response).unwrap();
            count += 1;
        }
        assert_eq!(count, 2);
    });
    let output = fixture
        .command("publish")
        .env_remove("AGENTPM_UV_PATH")
        .env("AGENTPM_BASE_URL", "http://registry-m11a.invalid")
        .env("HOME", fixture.0.join("home"))
        .env("USERPROFILE", fixture.0.join("home"))
        .env("XDG_CACHE_HOME", fixture.0.join("cache"))
        .env("HTTP_PROXY", &proxy)
        .env("http_proxy", &proxy)
        .env("HTTPS_PROXY", &proxy)
        .env("https_proxy", &proxy)
        .env("ALL_PROXY", &proxy)
        .env("all_proxy", &proxy)
        .env("NO_PROXY", "")
        .env("no_proxy", "")
        .env("AGENTPM_TOKEN", "PRIVATE_PUBLISH_TOKEN")
        .args(["--token", "PRIVATE_PUBLISH_TOKEN", "--dry-run", "--quiet"])
        .output()
        .unwrap();
    server.join().unwrap();
    assert!(!output.status.success());
    assert!(
        text(&output).contains("downloaded Python resolver digest mismatch"),
        "{}",
        text(&output)
    );
}
