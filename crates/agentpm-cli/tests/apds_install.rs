//! Real install commands against a loopback Registry; no production requests.
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{Read, Write},
    net::TcpListener,
    path::PathBuf,
    process::{Command, Output, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

type RegistryState = Arc<Mutex<(Value, Vec<u8>, Vec<String>, Option<Value>)>>;

struct Registry {
    root: PathBuf,
    url: String,
    state: RegistryState,
    stop: Arc<AtomicBool>,
    worker: Option<std::thread::JoinHandle<()>>,
}

impl Registry {
    fn new(declared: bool, advertise: bool) -> Self {
        let root = std::env::temp_dir().join(format!(
            "agentpm-apds-install-{:032x}",
            rand::random::<u128>()
        ));
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("agent.json"), json!({"kind":"agent","name":"consumer","version":"1.0.0","tools":["@fixture/example@1.0.0"]}).to_string()).unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let mut manifest = json!({"kind":"tool","name":"example","version":"1.0.0","description":"Fixture","runtime":{"type":"node"},"entrypoint":{"command":"node","args":["tool.js"]},"inputs":{"type":"object"},"outputs":{"type":"object"}});
        if declared {
            manifest["standard"] = json!({"id":"agentpm","version":"1.0.0"});
        }
        let bytes = archive(&manifest);
        let mut item = json!({"kind":"tool","name":"@fixture/example","version":"1.0.0","integrity":format!("{:x}",Sha256::digest(&bytes))});
        if advertise {
            item["standard"] = json!({"id":"agentpm","version":"1.0.0"});
            item["standard_origin"] = json!(if declared {
                "declared"
            } else {
                "legacy_inferred"
            });
        }
        let state = Arc::new(Mutex::new((item, bytes, vec![], None)));
        let stop = Arc::new(AtomicBool::new(false));
        let (shared, stopping, origin) = (state.clone(), stop.clone(), url.clone());
        let worker = std::thread::spawn(move || {
            while !stopping.load(Ordering::Relaxed) {
                let (mut stream, _) = match listener.accept() {
                    Ok(v) => v,
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(2));
                        continue;
                    }
                    Err(e) => panic!("{e}"),
                };
                stream.set_nonblocking(false).unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(10)))
                    .unwrap();
                let mut headers = vec![];
                while !headers.ends_with(b"\r\n\r\n") {
                    let mut b = [0];
                    stream.read_exact(&mut b).unwrap();
                    headers.push(b[0]);
                }
                let headers = String::from_utf8(headers).unwrap();
                let path = headers.split_whitespace().nth(1).unwrap();
                let length: usize = headers
                    .lines()
                    .find_map(|l| {
                        l.to_lowercase()
                            .strip_prefix("content-length:")
                            .map(|v| v.trim().parse().unwrap())
                    })
                    .unwrap_or(0);
                let mut body = vec![0; length];
                stream.read_exact(&mut body).unwrap();
                let mut shared = shared.lock().unwrap();
                shared.2.push(path.to_owned());
                let body = match path {
                    "/v1/tools/install/resolve" => {
                        serde_json::to_vec(&json!({"items":[shared.0.clone()]})).unwrap()
                    }
                    "/v1/tools/install/init" => {
                        let mut art = shared.3.clone().unwrap_or_else(|| shared.0.clone());
                        art["presigned_url"] = json!(format!("{origin}/artifact"));
                        serde_json::to_vec(&json!({"session_id":"fixture","expires_at":"2099-01-01T00:00:00Z","artifacts":[art]})).unwrap()
                    }
                    "/artifact" => shared.1.clone(),
                    "/v1/tools/install/finalize" => b"{}".to_vec(),
                    _ => panic!("unexpected fixture request {path}"),
                };
                write!(stream,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",body.len()).unwrap();
                stream.write_all(&body).unwrap();
            }
        });
        Self {
            root,
            url,
            state,
            stop,
            worker: Some(worker),
        }
    }
    fn install(&self, frozen: bool) -> Output {
        let mut command = Command::new(env!("CARGO_BIN_EXE_agentpm"));
        command
            .current_dir(&self.root)
            .stdin(Stdio::null())
            .args(["--base-url", &self.url, "install", "--machine"])
            .env("AGENTPM_TOKEN", "apm_fixture")
            .env("AGENTPM_TELEMETRY", "0")
            .env("XDG_CONFIG_HOME", self.root.join("config"))
            .env("APPDATA", self.root.join("config"))
            .env("LOCALAPPDATA", self.root.join("cache"));
        if frozen {
            command.arg("--frozen");
        }
        command.output().unwrap()
    }
    fn lock(&self) -> Value {
        serde_json::from_slice(&fs::read(self.root.join("agent.lock")).unwrap()).unwrap()
    }
    fn installed(&self) -> PathBuf {
        self.root
            .join(".agentpm/tools/fixture/example/1.0.0/agent.json")
    }
    fn replace_manifest(&self, manifest: &Value) {
        let mut s = self.state.lock().unwrap();
        s.1 = archive(manifest);
        s.0["integrity"] = json!(format!("{:x}", Sha256::digest(&s.1)));
    }
}
impl Drop for Registry {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        let _ = self.worker.take().unwrap().join();
        let _ = fs::remove_dir_all(&self.root);
    }
}
fn archive(manifest: &Value) -> Vec<u8> {
    let mut tar = tar::Builder::new(flate2::write::GzEncoder::new(
        vec![],
        flate2::Compression::default(),
    ));
    for (name, bytes) in [
        ("agent.json", serde_json::to_vec(manifest).unwrap()),
        (
            "tool.js",
            b"process.stdin.on('data', b => process.stdout.write(b));".to_vec(),
        ),
    ] {
        let mut header = tar::Header::new_gnu();
        header.set_size(bytes.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        tar.append_data(&mut header, name, bytes.as_slice())
            .unwrap();
    }
    tar.into_inner().unwrap().finish().unwrap()
}
fn success(out: &Output) {
    assert!(
        out.status.success(),
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn ordinary_regeneration_and_clean_frozen_replay_preserve_declared_and_legacy_metadata() {
    for declared in [false, true] {
        for advertise in [false, true] {
            let r = Registry::new(declared, advertise);
            success(&r.install(false));
            let lock = r.lock();
            assert_eq!(lock["lockfile_version"], 4);
            let entry = &lock["packages"]["tool:@fixture/example@1.0.0"];
            assert_eq!(
                entry["standard_origin"],
                if declared {
                    "declared"
                } else {
                    "legacy_inferred"
                }
            );
            assert_eq!(entry["standard"], json!({"id":"agentpm","version":"1.0.0"}));
            assert_eq!(
                lock["roots"]["local:agent"]["standard_origin"],
                "legacy_inferred"
            );
            success(&r.install(false));
            assert_eq!(r.lock()["packages"], lock["packages"]);
            let before = fs::read(r.root.join("agent.lock")).unwrap();
            fs::remove_dir_all(r.root.join(".agentpm")).unwrap();
            success(&r.install(true));
            assert_eq!(before, fs::read(r.root.join("agent.lock")).unwrap());
            assert!(r.installed().is_file());
        }
    }
}

#[test]
fn unsupported_malformed_wrong_identity_and_false_provenance_never_finalize_or_write_lock() {
    for change in [
        "unsupported",
        "malformed",
        "kind",
        "name",
        "version",
        "origin",
    ] {
        let r = Registry::new(true, true);
        let mut manifest: Value = serde_json::from_slice(&{
            let s = r.state.lock().unwrap();
            let gz = flate2::read::GzDecoder::new(s.1.as_slice());
            let mut a = tar::Archive::new(gz);
            let mut bytes = vec![];
            a.entries()
                .unwrap()
                .next()
                .unwrap()
                .unwrap()
                .read_to_end(&mut bytes)
                .unwrap();
            bytes
        })
        .unwrap();
        match change {
            "unsupported" => manifest["standard"]["version"] = json!("9.0.0"),
            "malformed" => manifest["standard"] = Value::Null,
            "kind" => manifest["kind"] = json!("agent"),
            "name" => manifest["name"] = json!("other"),
            "version" => manifest["version"] = json!("2.0.0"),
            "origin" => r.state.lock().unwrap().0["standard_origin"] = json!("legacy_inferred"),
            _ => unreachable!(),
        };
        r.replace_manifest(&manifest);
        let out = r.install(false);
        assert!(!out.status.success(), "{change}");
        assert!(!r.root.join("agent.lock").exists());
        assert!(!r.installed().exists());
        assert!(
            !r.state
                .lock()
                .unwrap()
                .2
                .iter()
                .any(|p| p.ends_with("finalize"))
        );
    }
}

#[test]
fn tampered_cache_is_reverified_and_bad_download_cannot_replace_installed_bytes_or_lock() {
    let r = Registry::new(true, true);
    success(&r.install(false));
    let before = fs::read(r.root.join("agent.lock")).unwrap();
    let installed = fs::read(r.installed()).unwrap();
    let cache = fs::read_dir(r.root.join(".agentpm/cache"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    fs::write(&cache, b"tampered").unwrap();
    r.state.lock().unwrap().1 = b"bad remote bytes".to_vec();
    let out = r.install(false);
    assert!(!out.status.success());
    assert_eq!(before, fs::read(r.root.join("agent.lock")).unwrap());
    assert_eq!(installed, fs::read(r.installed()).unwrap());
}

#[test]
fn tampered_installed_manifest_is_repaired_from_verified_cache() {
    let r = Registry::new(true, true);
    success(&r.install(false));
    let before = fs::read(r.installed()).unwrap();
    fs::write(r.installed(), b"{}").unwrap();
    let gets = r
        .state
        .lock()
        .unwrap()
        .2
        .iter()
        .filter(|p| *p == "/artifact")
        .count();
    success(&r.install(false));
    assert_eq!(fs::read(r.installed()).unwrap(), before);
    assert_eq!(
        gets,
        r.state
            .lock()
            .unwrap()
            .2
            .iter()
            .filter(|p| *p == "/artifact")
            .count()
    );
}

#[test]
fn apds_plan_to_init_drift_and_unsupported_resolve_fail_before_download() {
    let r = Registry::new(true, true);
    {
        let mut state = r.state.lock().unwrap();
        let mut changed = state.0.clone();
        changed["standard_origin"] = json!("legacy_inferred");
        state.3 = Some(changed);
    }
    assert!(!r.install(false).status.success());
    assert!(!r.root.join("agent.lock").exists());
    assert!(
        !r.state
            .lock()
            .unwrap()
            .2
            .iter()
            .any(|path| path == "/artifact")
    );
    let r = Registry::new(true, true);
    r.state.lock().unwrap().0["standard"]["version"] = json!("9.0.0");
    let out = r.install(false);
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stdout).contains("Unsupported APDS standard"));
    assert_eq!(r.state.lock().unwrap().2, vec!["/v1/tools/install/resolve"]);
}

#[test]
fn local_declaration_changes_refresh_on_normal_install_and_conflict_in_frozen_mode() {
    let r = Registry::new(true, true);
    success(&r.install(false));
    let manifest = r.root.join("agent.json");
    let mut value: Value = serde_json::from_slice(&fs::read(&manifest).unwrap()).unwrap();
    value["standard"] = json!({"id":"agentpm","version":"1.0.0"});
    value["name"] = json!("renamed");
    value["version"] = json!("2.0.0");
    fs::write(&manifest, value.to_string()).unwrap();
    let before = fs::read(r.root.join("agent.lock")).unwrap();
    assert!(!r.install(true).status.success());
    assert_eq!(before, fs::read(r.root.join("agent.lock")).unwrap());
    success(&r.install(false));
    let lock = r.lock();
    assert_eq!(lock["roots"]["local:agent"]["standard_origin"], "declared");
    assert_eq!(lock["roots"]["local:agent"]["name"], "renamed");
    assert_eq!(lock["roots"]["local:agent"]["version"], "2.0.0");
}

#[test]
fn old_registry_cannot_promote_a_modified_installed_selector_to_lock_truth() {
    let r = Registry::new(false, false);
    success(&r.install(false));
    let mut manifest: Value = serde_json::from_slice(&fs::read(r.installed()).unwrap()).unwrap();
    manifest["standard"] = json!({"id":"agentpm","version":"1.0.0"});
    fs::write(r.installed(), manifest.to_string()).unwrap();
    success(&r.install(false));
    assert_eq!(
        r.lock()["packages"]["tool:@fixture/example@1.0.0"]["standard_origin"],
        "legacy_inferred"
    );
    let manifest: Value = serde_json::from_slice(&fs::read(r.installed()).unwrap()).unwrap();
    assert!(manifest.get("standard").is_none());
}
