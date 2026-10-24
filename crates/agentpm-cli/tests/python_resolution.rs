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
            .env("AGENTPM_TELEMETRY", "0")
            .arg(verb)
            .args(["--schema", &schema().to_string_lossy()]);
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
            .args(["--token", "fixture-only", "--quiet", "--refresh-python"]);
        if let Some(path) = override_path {
            command.env("AGENTPM_UV_PATH", path);
        }
        let output = command.output().unwrap();
        assert!(!output.status.success());
        assert!(text(&output).contains("Milestone 11B"), "{}", text(&output));
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
fn publish_prepares_resolution_but_cannot_upload_before_lock_integration() {
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
        .args(["--token", "fixture-only", "--dry-run", "--quiet"])
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
    let live = fixture
        .command("publish")
        .env("AGENTPM_UV_PATH", "/must-not-execute-on-cache-reuse")
        .args(["--token", "fixture-only", "--quiet"])
        .output()
        .unwrap();
    assert!(!live.status.success());
    assert!(text(&live).contains("Milestone 11B"), "{}", text(&live));
    assert!(!fixture.0.join("agent.lock").exists());
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
