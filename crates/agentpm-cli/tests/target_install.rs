//! Shipping CLI noninteractive compatibility errors must be structured and final.
use agentpm_sdk::target::Target;
use serde_json::{Value, json};
use std::{
    fs,
    io::{Read, Write},
    net::TcpListener,
    process::{Command, Stdio},
    time::Duration,
};

#[test]
fn machine_incompatible_install_does_not_change_version_create_lock_or_finalize() {
    let root =
        std::env::temp_dir().join(format!("agentpm-m15-cli-{:032x}", rand::random::<u128>()));
    for directory in ["home/AppData/Roaming", "home/AppData/Local"] {
        fs::create_dir_all(root.join(directory)).unwrap();
    }
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let target = Target::current().unwrap().as_str();
    let server = std::thread::spawn(move || {
        for step in 0..2 {
            let (mut stream, _) = listener.accept().unwrap();
            stream.set_nonblocking(false).unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(10)))
                .unwrap();
            let mut headers = Vec::new();
            while !headers.ends_with(b"\r\n\r\n") {
                let mut byte = [0];
                stream.read_exact(&mut byte).unwrap();
                headers.push(byte[0]);
            }
            let headers = String::from_utf8(headers).unwrap();
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
            let mut bytes = vec![0; length];
            stream.read_exact(&mut bytes).unwrap();
            let request: Value = serde_json::from_slice(&bytes).unwrap();
            assert_eq!(request["release_format"], "agentpm.package.release.v1");
            let (status, body) = if step == 0 {
                assert!(headers.starts_with("POST /v1/tools/install/resolve"));
                assert_eq!(request["items"][0]["range"], "1.0.0");
                (
                    "200 OK",
                    json!({"items":[{"kind":"tool","name":"@fixture/example","version":"1.0.0","integrity":"a".repeat(64)}]}),
                )
            } else {
                assert!(headers.starts_with("POST /v1/tools/install/init"));
                assert_eq!(request["consumer"]["target"], target);
                (
                    "422 Unprocessable Entity",
                    json!({"code":"no_compatible_artifact","message":"No compatible artifact", "details":{"name":"@fixture/example","version":"1.0.0","target":target,"available_targets":["other"],"compatible_newer_versions":["1.1.0"]}}),
                )
            };
            let body = body.to_string();
            write!(stream,"HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len()).unwrap();
        }
    });
    let output = Command::new(env!("CARGO_BIN_EXE_agentpm"))
        .current_dir(&root)
        .env("HOME", root.join("home"))
        .env("USERPROFILE", root.join("home"))
        .env("APPDATA", root.join("home/AppData/Roaming"))
        .env("LOCALAPPDATA", root.join("home/AppData/Local"))
        .env("XDG_CONFIG_HOME", root.join("home/config"))
        .env("XDG_DATA_HOME", root.join("home/data"))
        .env("AGENTPM_TELEMETRY", "0")
        .env("AGENTPM_TOKEN", "apm_m15_fixture")
        .args([
            "--base-url",
            &origin,
            "install",
            "@fixture/example@1.0.0",
            "--machine",
        ])
        .stdin(Stdio::null())
        .output()
        .unwrap();
    server.join().unwrap();
    assert_eq!(output.status.code(), Some(1));
    let result: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(result["status"], "error");
    assert_eq!(result["error"]["category"], "no_compatible_artifact");
    assert_eq!(result["error"]["details"]["target"], target);
    assert!(!root.join("agent.lock").exists());
    assert!(!root.join(".agentpm").exists());
    fs::remove_dir_all(root).unwrap();
}
