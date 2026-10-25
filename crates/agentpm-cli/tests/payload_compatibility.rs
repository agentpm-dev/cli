//! Exercise the shipping CLI packaging path on every native contract runner.
use agentpm_sdk::target::Target;
use serde_json::json;
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
};

struct Fixture(PathBuf);

impl Fixture {
    fn new(runtime: &str, files: &[&str]) -> Self {
        let root = std::env::temp_dir().join(format!(
            "agentpm-payload-cli-{:032x}",
            rand::random::<u128>()
        ));
        fs::create_dir_all(&root).unwrap();
        // Windows known-folder lookup needs a real isolated profile/app-data
        // tree before CLI startup; pointing USERPROFILE at a missing home can
        // prevent ProjectDirs from resolving any config directories.
        for directory in ["home/AppData/Roaming", "home/AppData/Local"] {
            fs::create_dir_all(root.join(directory)).unwrap();
        }
        fs::write(root.join("tool.py"), "print('{}')\n").unwrap();
        let manifest = json!({
            "$schema":"https://raw.githubusercontent.com/agentpm-dev/cli/main/schemas/agentpm.manifest.schema.json",
            "kind":"tool", "name":"payload-fixture", "version":"0.1.0",
            "description":"Tool payload compatibility fixture.",
            "entrypoint":{"command":if runtime == "python" {"python3"} else {"node"},"args":["tool.py"]},
            "runtime":{"type":runtime,"version":if runtime == "python" {"3.11"} else {"20"}},
            "files":files,"inputs":{"type":"object"},"outputs":{"type":"object"}
        });
        fs::write(
            root.join("agent.json"),
            serde_json::to_vec(&manifest).unwrap(),
        )
        .unwrap();
        Self(root)
    }

    fn command(&self) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_agentpm"));
        command
            .current_dir(&self.0)
            .env("HOME", self.0.join("home"))
            .env("USERPROFILE", self.0.join("home"))
            .env("APPDATA", self.0.join("home/AppData/Roaming"))
            .env("LOCALAPPDATA", self.0.join("home/AppData/Local"))
            .env("XDG_CONFIG_HOME", self.0.join("home/config"))
            .env("XDG_DATA_HOME", self.0.join("home/data"))
            .env("AGENTPM_TELEMETRY", "0")
            .env("AGENTPM_BASE_URL", "http://127.0.0.1:1")
            .args(["publish", "--token", "fixture-only", "--schema"])
            .arg(
                Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("../../schemas/agentpm.manifest.schema.json"),
            );
        command
    }

    fn add(&self, name: &str, content: &[u8]) {
        let path = self.0.join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, content).unwrap();
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn text(output: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

#[test]
fn dry_run_classifies_the_archive_without_changing_authored_contracts() {
    for (name, bytes, expected) in [
        ("data.json", b"{\"fixture\":true}\n".as_slice(), Target::Any),
        ("_vendor/native.so", b"native", Target::current().unwrap()),
        ("lib/native.dylib", b"native", Target::current().unwrap()),
        ("pkg/native.pyd", b"native", Target::current().unwrap()),
        ("pkg/native.dll", b"native", Target::current().unwrap()),
        ("bin/helper.exe", b"native", Target::current().unwrap()),
        ("helper", b"unknown executable", Target::current().unwrap()),
    ] {
        let fixture = Fixture::new("python", &["tool.py", name]);
        fixture.add(name, bytes);
        fixture.add("not-collected.so", b"unshipped binary");
        fixture.add(
            "agent.lock",
            br#"{"lockfile_version":2,"generated":"2026-10-24T00:00:00Z","packages":{},"roots":{}}"#,
        );
        let manifest_before = fs::read(fixture.0.join("agent.json")).unwrap();
        let lock_before = fs::read(fixture.0.join("agent.lock")).unwrap();
        let output = fixture.command().arg("--dry-run").output().unwrap();
        assert!(output.status.success(), "{name}: {}", text(&output));
        assert!(
            text(&output).contains(&format!("Tool payload: {expected} (format: tar.gz)")),
            "{name}: {}",
            text(&output)
        );
        assert!(text(&output).contains("Dry-run: artifact created at"));
        assert_eq!(
            fs::read(fixture.0.join("agent.json")).unwrap(),
            manifest_before
        );
        assert_eq!(fs::read(fixture.0.join("agent.lock")).unwrap(), lock_before);
        assert!(
            fixture
                .0
                .join("target/agentpm/payload-fixture-0.1.0-python311.tar.gz")
                .is_file()
        );
    }
}

#[test]
fn other_tool_runtimes_keep_their_existing_packaging_behavior() {
    let fixture = Fixture::new("node", &["tool.py"]);
    let output = fixture.command().arg("--dry-run").output().unwrap();
    assert!(output.status.success(), "{}", text(&output));
    assert!(!text(&output).contains("Tool payload:"));
}

#[test]
fn missing_declared_paths_and_embedded_wheels_fail_packaging() {
    for name in ["missing.py", "vendor/example.whl"] {
        let fixture = Fixture::new("python", &[name]);
        if name.ends_with(".whl") {
            fixture.add(name, b"embedded wheel fixture");
        }
        let output = fixture.command().arg("--dry-run").output().unwrap();
        assert!(!output.status.success(), "{}", text(&output));
        let expected = if name.ends_with(".whl") {
            "embedded archive not allowed in package: vendor/example.whl"
        } else {
            "manifest.files entry `missing.py` does not exist"
        };
        assert!(text(&output).contains(expected), "{}", text(&output));
        assert!(!text(&output).contains("Tool payload:"));
        assert!(
            !fixture
                .0
                .join("target/agentpm/payload-fixture-0.1.0-python311.tar.gz")
                .exists()
        );
    }
}

#[tokio::test]
async fn live_publish_sends_descriptor_separately_from_publisher_telemetry() {
    use axum::{
        Json, Router,
        http::StatusCode,
        routing::{post, put},
    };
    use serde_json::Value;
    use std::sync::{Arc, Mutex};

    let received = Arc::new(Mutex::new(Vec::<Value>::new()));
    let uploaded = Arc::new(Mutex::new(Vec::<Vec<String>>::new()));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let app = Router::new()
        .route("/v1/tools/publish/init", post({
            let received = Arc::clone(&received);
            let base = base.clone();
            move |Json(metadata): Json<Value>| {
                let received = Arc::clone(&received);
                let base = base.clone();
                async move {
                    received.lock().unwrap().push(metadata);
                    Json(json!({"upload_id":"fixture","put_url":format!("{base}/object"),
                        "headers":{},"final_key":"tool.tar.gz","expires_in":900}))
                }
            }
        }))
        .route("/object", put({
            let uploaded = Arc::clone(&uploaded);
            move |body: axum::body::Bytes| {
                let uploaded = Arc::clone(&uploaded);
                async move {
                    let mut archive = tar::Archive::new(flate2::read::GzDecoder::new(body.as_ref()));
                    let members = archive.entries().unwrap().map(|entry| entry.unwrap().path().unwrap().to_string_lossy().into_owned()).collect();
                    uploaded.lock().unwrap().push(members);
                    StatusCode::OK
                }
            }
        }))
        .route("/v1/tools/publish/finalize", post(|| async {
            Json(json!({"id":"fixture","name":"payload-fixture","version":"0.1.0","url":"https://fixture.invalid"}))
        }));
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    for name in [
        "data.json",
        "_vendor/native.so",
        "directory",
        "explicit-dotenv",
    ] {
        let requested = match name {
            "directory" => vec!["."],
            "explicit-dotenv" => vec![".", ".env"],
            _ => vec![name],
        };
        let fixture = Fixture::new("python", &requested);
        if matches!(name, "directory" | "explicit-dotenv") {
            fixture.add(".env", b"PRIVATE_FIXTURE=not-a-real-secret\n");
            fixture.add(".env.local", b"PRIVATE_FIXTURE=not-a-real-secret\n");
            fixture.add(".env.example", b"NAME=placeholder\n");
            fixture.add(".git/object.bin", b"not payload");
            fixture.add(".venv/bad.png", b"not a png");
        } else {
            fixture.add(name, b"{}\n");
        }
        let mut command = fixture.command();
        command
            .env("AGENTPM_BASE_URL", &base)
            .args(["--quiet", "--strict"]);
        let output = tokio::task::spawn_blocking(move || command.output().unwrap())
            .await
            .unwrap();
        assert!(output.status.success(), "{}", text(&output));
        if matches!(name, "directory" | "explicit-dotenv") {
            assert!(String::from_utf8_lossy(&output.stderr).contains("Info:"));
            assert!(!String::from_utf8_lossy(&output.stdout).contains("Info:"));
        }
    }
    server.abort();
    let received = received.lock().unwrap();
    assert_eq!(received.len(), 4);
    for (index, (metadata, target)) in received
        .iter()
        .zip([
            Target::Any,
            Target::current().unwrap(),
            Target::Any,
            Target::Any,
        ])
        .enumerate()
    {
        assert_eq!(metadata["artifact"]["target"], json!(target));
        assert_eq!(metadata["artifact"]["format"], "tar.gz");
        let diagnostics = metadata["artifact"]["diagnostics"].as_array().unwrap();
        if index >= 2 {
            assert!(!diagnostics.is_empty());
            assert!(
                diagnostics
                    .iter()
                    .all(|d| d["severity"] == "info" && d.get("target").is_none())
            );
            assert!(
                diagnostics
                    .iter()
                    .any(|d| d["code"] == "collection_omitted" && d["rule"] == "dotenv")
            );
            if index == 3 {
                assert!(diagnostics.iter().any(|d| d["code"] == "collection_explicit_override" && d["path"] == ".env"));
            }
        } else if target == Target::Any {
            assert!(diagnostics.is_empty());
        } else {
            assert_eq!(diagnostics[0]["severity"], "warning");
            assert_eq!(diagnostics[0]["code"], "native_content");
            assert_eq!(diagnostics[0]["path"], "_vendor/native.so");
            assert_eq!(diagnostics[0]["target"], json!(target));
        }
        assert_eq!(metadata["client"]["os"], std::env::consts::OS);
        assert_eq!(metadata["client"]["arch"], std::env::consts::ARCH);
        assert!(metadata["manifest"].get("artifact").is_none());
        assert!(metadata["manifest"].get("target").is_none());
    }
    let uploaded = uploaded.lock().unwrap();
    for (index, members) in uploaded.iter().enumerate().skip(2) {
        assert!(members.contains(&".env.example".into()));
        assert!(!members.contains(&".env.local".into()));
        assert!(
            !members
                .iter()
                .any(|name| name.starts_with(".git/") || name.starts_with(".venv/"))
        );
        assert_eq!(members.contains(&".env".into()), index == 3);
    }
}

#[test]
fn non_python_directory_collection_keeps_the_existing_behavior() {
    let fixture = Fixture::new("node", &["."]);
    fixture.add(".env", b"FIXTURE=1\n");
    fixture.add(".git/readme.txt", b"historical node payload\n");
    let output = fixture.command().arg("--dry-run").output().unwrap();
    assert!(output.status.success(), "{}", text(&output));
    let file = fs::File::open(
        fixture
            .0
            .join("target/agentpm/payload-fixture-0.1.0-node20.tar.gz"),
    )
    .unwrap();
    let mut archive = tar::Archive::new(flate2::read::GzDecoder::new(file));
    let members: Vec<_> = archive
        .entries()
        .unwrap()
        .map(|entry| {
            entry
                .unwrap()
                .path()
                .unwrap()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    assert!(members.contains(&".env".into()) && members.contains(&".git/readme.txt".into()));
    assert!(!text(&output).contains("Tool payload:") && !text(&output).contains("Info:"));
}

#[test]
fn non_python_directory_collection_normalizes_names_and_skips_generated_outputs() {
    use std::collections::HashSet;

    for manifest_form in ["default", "dotted", "absolute", "nested-relative"] {
        let fixture = Fixture::new(
            "node",
            &[
                ".",
                "./agent.json",
                "./tool.py",
                "./assets",
                "assets/data.json",
                "./target/agentpm/unrelated.tar.gz.tmp",
            ],
        );
        fixture.add("assets/data.json", b"{}\n");
        fixture.add(".env", b"FIXTURE=1\n");
        fixture.add(".git/readme.txt", b"intentional Node payload\n");
        fixture.add(".venv/data.txt", b"intentional Node payload\n");
        fixture.add("target/agentpm/unrelated.tar.gz.tmp", b"generated output\n");
        fixture.add(
            "target/agentpm/previous.tar.gz",
            b"previous generated archive\n",
        );
        let manifest_path = fixture.0.join("agent.json");
        let mut manifest: serde_json::Value =
            serde_json::from_slice(&fs::read(&manifest_path).unwrap()).unwrap();
        manifest["entrypoint"]["args"] = json!(["./tool.py"]);
        let manifest_before = serde_json::to_vec(&manifest).unwrap();
        fs::write(&manifest_path, &manifest_before).unwrap();

        let run = || {
            let mut command = fixture.command();
            match manifest_form {
                "dotted" => {
                    command.args(["--manifest", "./agent.json"]);
                }
                "absolute" => {
                    command.arg("--manifest").arg(&manifest_path);
                }
                "nested-relative" => {
                    command
                        .current_dir(fixture.0.parent().unwrap())
                        .arg("--manifest")
                        .arg(
                            Path::new(".")
                                .join(fixture.0.file_name().unwrap())
                                .join("agent.json"),
                        );
                }
                _ => {}
            }
            command.arg("--dry-run").output().unwrap()
        };
        let artifact = fixture
            .0
            .join("target/agentpm/payload-fixture-0.1.0-node20.tar.gz");
        for attempt in 0..2 {
            let output = run();
            assert!(
                output.status.success(),
                "{manifest_form}, attempt {attempt}: {}",
                text(&output)
            );
            assert!(!text(&output).contains("Tool payload:") && !text(&output).contains("Info:"));
            let mut archive = tar::Archive::new(flate2::read::GzDecoder::new(
                fs::File::open(&artifact).unwrap(),
            ));
            let names: Vec<String> = archive
                .entries()
                .unwrap()
                .map(|entry| {
                    entry
                        .unwrap()
                        .path()
                        .unwrap()
                        .to_string_lossy()
                        .into_owned()
                })
                .collect();
            assert_eq!(
                names.len(),
                names.iter().collect::<HashSet<_>>().len(),
                "{names:?}"
            );
            assert!(
                names
                    .iter()
                    .all(|name| !name.starts_with("./") && !name.starts_with("target/agentpm/")),
                "{names:?}"
            );
            for name in [
                "agent.json",
                "tool.py",
                "assets/data.json",
                ".env",
                ".git/readme.txt",
                ".venv/data.txt",
            ] {
                assert!(
                    names.iter().any(|member| member == name),
                    "{manifest_form}: {names:?}"
                );
            }
        }
        assert_eq!(fs::read(&manifest_path).unwrap(), manifest_before);

        // Generated-output filtering must not exempt ordinary payload archives.
        let valid_archive = fs::read(&artifact).unwrap();
        fixture.add("vendor/must-not-ship.whl", b"blocked payload archive\n");
        let output = run();
        assert!(!output.status.success(), "{}", text(&output));
        assert!(
            text(&output)
                .contains("embedded archive not allowed in package: vendor/must-not-ship.whl"),
            "{}",
            text(&output)
        );
        assert_eq!(fs::read(&artifact).unwrap(), valid_archive);
        assert_eq!(fs::read(&manifest_path).unwrap(), manifest_before);
    }
}

#[test]
fn mismatches_block_dry_and_live_republication_without_replacing_valid_artifact() {
    use std::net::TcpListener;
    for names in [["native.so", "icon.png"], ["icon.png", "native.so"]] {
        let fixture = Fixture::new("python", &names);
        fixture.add("native.so", b"native fixture");
        fixture.add("icon.png", b"\x89PNG\r\n\x1a\n");
        fixture.add("agent.lock", br#"{"lockfile_version":2,"generated":"2026-10-24T00:00:00Z","packages":{},"roots":{}}"#);
        let valid = fixture.command().arg("--dry-run").output().unwrap();
        assert!(valid.status.success(), "{}", text(&valid));
        let archive = fixture
            .0
            .join("target/agentpm/payload-fixture-0.1.0-python311.tar.gz");
        let before = fs::read(&archive).unwrap();
        let manifest_before = fs::read(fixture.0.join("agent.json")).unwrap();
        let lock_before = fs::read(fixture.0.join("agent.lock")).unwrap();
        fixture.add("icon.png", b"GIF89a previously accepted mismatched asset");
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        for dry in [true, false] {
            let mut command = fixture.command();
            command
                .env(
                    "AGENTPM_BASE_URL",
                    format!("http://{}", listener.local_addr().unwrap()),
                )
                .arg("--quiet");
            if dry {
                command.arg("--dry-run");
            }
            let output = command.output().unwrap();
            assert!(!output.status.success(), "{}", text(&output));
            let stderr = String::from_utf8_lossy(&output.stderr);
            assert!(
                stderr.contains("icon.png") && stderr.contains("PNG signature"),
                "{stderr}"
            );
            assert!(
                stderr.contains("Rename") && stderr.contains("manifest.files"),
                "{stderr}"
            );
            assert!(!text(&output).contains("Dry-run: artifact created"));
            assert!(!text(&output).contains("✓ Tool payload:"));
            assert_eq!(fs::read(&archive).unwrap(), before);
            assert_eq!(
                fs::read(fixture.0.join("agent.json")).unwrap(),
                manifest_before
            );
            assert_eq!(fs::read(fixture.0.join("agent.lock")).unwrap(), lock_before);
            assert!(
                !archive
                    .with_file_name("payload-fixture-0.1.0-python311.tar.gz.tmp")
                    .exists()
            );
            assert_eq!(
                listener.accept().unwrap_err().kind(),
                std::io::ErrorKind::WouldBlock
            );
        }
        // Correcting the label fixes the error, not the unknown format policy.
        fs::rename(fixture.0.join("icon.png"), fixture.0.join("icon.gif")).unwrap();
        let path = fixture.0.join("agent.json");
        let mut manifest: serde_json::Value =
            serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        manifest["files"] = json!(["icon.gif", "native.so"]);
        fs::write(&path, serde_json::to_vec(&manifest).unwrap()).unwrap();
        let renamed = fixture.command().arg("--dry-run").output().unwrap();
        assert!(renamed.status.success(), "{}", text(&renamed));
        assert!(text(&renamed).contains("Warning: icon.gif: is not a recognized portable format"));
    }
}

#[test]
fn quiet_strict_and_non_tty_keep_sorted_narrowing_warnings_on_stderr() {
    let fixture = Fixture::new("python", &["z.rtf", "a.ts", "a.ts"]);
    fixture.add("z.rtf", b"{\\rtf1 text}");
    fixture.add("a.ts", b"static source text");
    let output = fixture
        .command()
        .args(["--dry-run", "--quiet", "--strict"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", text(&output));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.find("Warning: a.ts:").unwrap() < stderr.find("Warning: z.rtf:").unwrap());
    assert_eq!(stderr.matches("Warning: a.ts:").count(), 1);
    assert!(!String::from_utf8_lossy(&output.stdout).contains("Warning:"));
}
