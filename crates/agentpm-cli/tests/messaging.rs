use serde_json::{Value, json};
use std::fs;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

static NEXT: AtomicU64 = AtomicU64::new(0);

struct Project(PathBuf);
impl Project {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "agentpm-messaging-{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn command(&self) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_agentpm"));
        command
            .env("AGENTPM_TELEMETRY", "0")
            .current_dir(&self.0)
            .env("NO_COLOR", "1")
            .env("RUST_LOG", "")
            .env("AGENTPM_TOKEN", "apm_fixture.secret")
            .env("HOME", &self.0)
            .env("USERPROFILE", &self.0)
            .env("XDG_DATA_HOME", self.0.join("data"))
            .env("XDG_CONFIG_HOME", self.0.join("config"))
            .env("APPDATA", self.0.join("data"));
        command
    }

    fn cli(&self, args: &[&str]) -> Output {
        self.command().args(args).output().unwrap()
    }

    fn success(&self, args: &[&str]) -> Output {
        let output = self.cli(args);
        assert_eq!(
            output.status.code(),
            Some(0),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        output
    }
}
impl Drop for Project {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn mock_response(status: u16, body: Value) -> (String, std::thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    listener.set_nonblocking(true).unwrap();
    let worker = std::thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(10);
        let mut stream = loop {
            match listener.accept() {
                Ok((stream, _)) => break stream,
                Err(err)
                    if err.kind() == std::io::ErrorKind::WouldBlock
                        && Instant::now() < deadline =>
                {
                    std::thread::sleep(Duration::from_millis(5))
                }
                Err(err) => panic!("mock request not received: {err}"),
            }
        };
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let mut request = Vec::new();
        let mut buffer = [0; 4096];
        loop {
            let count = stream.read(&mut buffer).unwrap();
            assert!(count > 0, "request ended before its body");
            request.extend_from_slice(&buffer[..count]);
            if let Some(end) = request.windows(4).position(|bytes| bytes == b"\r\n\r\n") {
                let headers = String::from_utf8_lossy(&request[..end]);
                let length = headers
                    .lines()
                    .filter_map(|line| line.split_once(':'))
                    .find(|(name, _)| name.eq_ignore_ascii_case("content-length"))
                    .map(|(_, value)| value.trim().parse::<usize>().unwrap())
                    .unwrap_or(0);
                if request.len() >= end + 4 + length {
                    break;
                }
            }
        }
        let body = body.to_string();
        write!(stream, "HTTP/1.1 {status} Fixture\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
    });
    (format!("http://{address}"), worker)
}

#[test]
fn registry_command_routes_share_friendly_transport_errors_and_runtime_exit_one() {
    let project = Project::new();
    project.success(&["init", "--kind", "tool", "--name", "fixture"]);
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    drop(listener);
    let cases: &[(&[&str], &str)] = &[
        (&["install", "@example/tool"], "install @example/tool"),
        (
            &["export", "--skill", "@example/tool"],
            "export Skill @example/tool",
        ),
        (
            &["new", "@example/template", "generated"],
            "create a project",
        ),
        (&["whoami"], "check authentication"),
        (&["publish", "--quiet"], "publish the package"),
        (&["login", "apm_fixture.secret", "--no-write"], "sign in"),
        (
            &[
                "namespace",
                "revoke-signer",
                "--namespace",
                "example",
                "--signer-id",
                "fixture",
            ],
            "manage namespace signers",
        ),
    ];
    for (args, action) in cases {
        let mut command = vec!["--base-url", &url];
        command.extend_from_slice(args);
        let output = project.cli(&command);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.code(), Some(1), "{args:?}: {stderr}");
        assert!(
            stderr.contains("Cannot connect")
                && stderr.contains(action)
                && stderr.contains("then retry"),
            "{args:?}: {stderr}"
        );
        assert!(
            !stderr.contains("/v1/")
                && !stderr.contains("/cli/device")
                && !stderr.contains("error sending request"),
            "{stderr}"
        );
        assert!(
            output.stdout.is_empty(),
            "failed command must not announce success: {args:?}"
        );
    }
    let debug = project
        .command()
        .env("RUST_LOG", "agentpm=debug")
        .args(["--base-url", &url, "whoami"])
        .output()
        .unwrap();
    assert_eq!(debug.status.code(), Some(1));
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&debug.stdout),
        String::from_utf8_lossy(&debug.stderr)
    );
    assert!(
        combined.contains("Registry transport failure") && combined.contains("Cannot connect"),
        "{combined}"
    );
}

#[test]
fn whoami_success_is_zero_and_auth_not_found_and_server_errors_stay_distinct() {
    let project = Project::new();
    let no_token = project
        .command()
        .env_remove("AGENTPM_TOKEN")
        .arg("whoami")
        .output()
        .unwrap();
    assert_eq!(no_token.status.code(), Some(0));
    assert!(String::from_utf8_lossy(&no_token.stdout).contains("Not authenticated"));
    let (url, server) = mock_response(
        200,
        json!({"kind":"pat","sub":"user","email":"reader@example.test","scopes":["packages:read"]}),
    );
    let output = project.success(&["--base-url", &url, "whoami"]);
    server.join().unwrap();
    assert!(String::from_utf8_lossy(&output.stdout).contains("reader@example.test"));
    let (url, server) = mock_response(200, json!({"unexpected":"shape"}));
    let output = project.cli(&["--base-url", &url, "whoami"]);
    server.join().unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(1));
    assert!(
        stderr.contains("Invalid registry response") && !stderr.contains("/v1/"),
        "{stderr}"
    );
    for status in [401, 403, 404, 500] {
        let (url, server) = mock_response(status, json!({"error":"fixture response"}));
        let output = project.cli(&["--base-url", &url, "install", "@private/hidden-tool"]);
        server.join().unwrap();
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.code(), Some(1), "{stderr}");
        if status == 401 {
            assert!(
                stderr.contains("Credentials rejected")
                    && stderr.contains("AGENTPM_TOKEN")
                    && !stderr.contains("not found"),
                "{stderr}"
            );
        } else {
            assert!(
                !stderr.contains("Credentials rejected") && !stderr.contains("Cannot connect"),
                "{stderr}"
            );
            assert!(
                stderr.contains(if status == 404 {
                    "not found"
                } else if status == 403 {
                    "403"
                } else {
                    "500"
                }),
                "{stderr}"
            );
        }
    }
}

#[test]
fn missing_mcp_is_clap_usage_error_and_unknown_key_is_concise_runtime_error() {
    let project = Project::new();
    let output = project.cli(&["serve"]);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(2));
    assert!(
        stderr.contains("--mcp") && stderr.contains("Usage:"),
        "{stderr}"
    );
    assert!(!stderr.contains("currently requires"));
    let id = format!(
        "missing-key-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    );
    let output = project.cli(&["keys", "export", "--key-id", &id]);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(1));
    assert!(
        stderr.contains(&format!("No local key with id `{id}`"))
            && stderr.contains("agentpm keys list"),
        "{stderr}"
    );
    assert!(
        !stderr.contains("Caused by:")
            && !stderr.contains("Application Support")
            && !stderr.contains("No such file"),
        "{stderr}"
    );
    project.success(&["serve", "--help"]);
}

#[test]
fn sibling_inspect_headers_are_relative_and_json_retains_full_paths() {
    for kind in ["knowledge", "memory"] {
        let project = Project::new();
        project.success(&["init", "--kind", kind, "--name", "fixture"]);
        project.success(&[kind, "build"]);
        let output = project.success(&[kind, "inspect", "."]);
        let text = String::from_utf8(output.stdout).unwrap();
        let title = if kind == "knowledge" {
            "Knowledge"
        } else {
            "Memory"
        };
        assert_eq!(
            text.lines().take(4).collect::<Vec<_>>(),
            [
                format!("{title}: fixture@0.1.0"),
                "Target: .".into(),
                "Status: fresh".into(),
                "Manifest: agent.json".into()
            ]
        );
        assert!(
            !text.contains("/./") && !text.contains(project.0.to_str().unwrap()),
            "{text}"
        );
        let output = project.success(&[kind, "inspect", ".", "--json"]);
        let value: Value = serde_json::from_slice(&output.stdout).unwrap();
        let manifest = if kind == "memory" {
            &value["target"]["manifest_path"]
        } else {
            &value["manifest_path"]
        };
        assert!(std::path::Path::new(manifest.as_str().unwrap()).is_absolute());
        if kind == "knowledge" {
            fs::write(
                project.0.join("knowledge/docs/context.md"),
                "# Changed source\n",
            )
            .unwrap();
            let output = project.success(&[kind, "inspect", "."]);
            assert!(String::from_utf8_lossy(&output.stdout).contains("Status: stale"));
        } else {
            fs::remove_file(project.0.join("schemas/user-preference.schema.json")).unwrap();
            let output = project.cli(&[kind, "inspect", "."]);
            let stderr = String::from_utf8_lossy(&output.stderr);
            assert_eq!(output.status.code(), Some(1));
            assert!(stderr.contains("references schema `schemas/user-preference.schema.json`, which could not be read") && stderr.contains("Restore the file"), "{stderr}");
            assert!(!stderr.contains("is invalid: reading"), "{stderr}");
        }
    }
}

#[test]
fn new_reports_created_project_and_readme_next_step_when_noninteractive() {
    let project = Project::new();
    project.success(&["init", "--kind", "template", "--name", "starter"]);
    let output = project.success(&["new", ".", "./generated", "--quiet"]);
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(
        text.starts_with(
            "Created project: generated\n\
             Next: review generated/README.md before running the generated entrypoints.\n"
        ) && text.contains("Review generated scaffold: cat README.md"),
        "{text}"
    );
    assert!(!text.contains("/./"));
    assert!(project.0.join("generated/README.md").exists());
}

#[test]
fn new_uses_generic_next_step_without_readme_and_preserves_authored_entrypoints() {
    for readme_is_directory in [false, true] {
        let project = Project::new();
        project.success(&["init", "--kind", "template", "--name", "starter"]);
        let readme = project.0.join("template/README.md");
        fs::remove_file(&readme).unwrap();
        if readme_is_directory {
            fs::create_dir(&readme).unwrap();
        }
        let manifest_path = project.0.join("agent.json");
        let mut manifest: Value =
            serde_json::from_slice(&fs::read(&manifest_path).unwrap()).unwrap();
        manifest["template"]["entrypoints"] = json!([{"label": "Run", "command": "echo ready"}]);
        fs::write(
            &manifest_path,
            serde_json::to_vec_pretty(&manifest).unwrap(),
        )
        .unwrap();
        project.success(&["lint", "--strict"]);

        let output = project.success(&["new", ".", "./generated", "--quiet"]);
        let text = String::from_utf8(output.stdout).unwrap();
        assert_eq!(
            text,
            "Created project: generated\n\
             Next: review the generated project before running its entrypoints.\n\
             Run: echo ready\n"
        );
        assert!(project.0.join("generated/agent.json").is_file());
        assert!(!project.0.join("generated/README.md").is_file());
    }
}
