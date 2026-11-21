//! Real CLI replay against a loopback Registry, with visible immutable fixture manifests.
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs,
    io::{Read, Write},
    net::TcpListener,
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

#[derive(Clone)]
struct Package {
    item: Value,
    manifest: Value,
    archive: Vec<u8>,
}
#[derive(Default)]
struct State {
    packages: BTreeMap<String, Package>,
    requests: Vec<(String, Value)>,
    bad_digest: bool,
    corrupt_download: bool,
    unusable_urls: bool,
    omit_resolved: Option<String>,
}
struct Registry {
    root: PathBuf,
    url: String,
    state: Arc<Mutex<State>>,
    stop: Arc<AtomicBool>,
    worker: Option<std::thread::JoinHandle<()>>,
}
fn key(item: &Value) -> String {
    format!(
        "{}:{}@{}",
        item["kind"].as_str().unwrap(),
        item["name"].as_str().unwrap(),
        item["version"].as_str().unwrap()
    )
}
fn package(kind: &str, name: &str, version: &str) -> Package {
    let snapshot: Value = serde_json::from_slice(
        &fs::read(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join(format!("tests/fixtures/scaffolds/{kind}.snapshot.json")),
        )
        .unwrap(),
    )
    .unwrap();
    let mut manifest = snapshot["agent.json"].clone();
    manifest["name"] = json!(name);
    manifest["version"] = json!(version);
    if kind == "tool" {
        manifest["runtime"] = json!({"type":"node", "version":"18"});
        manifest["entrypoint"] = json!({"command":"node", "args":["tool.js"]});
        manifest["files"] = json!(["tool.js"]);
    }
    let mut result = Package {
        item: json!({"kind":kind,"name":format!("@fixture/{name}"),"version":version,
        "standard":{"id":"agentpm","version":"1.0.0"},"standard_origin":"declared"}),
        manifest,
        archive: vec![],
    };
    result.rebuild();
    result
}
impl Package {
    fn rebuild(&mut self) {
        let kind = self.item["kind"].as_str().unwrap();
        let snapshot: Value = serde_json::from_slice(
            &fs::read(
                Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join(format!("tests/fixtures/scaffolds/{kind}.snapshot.json")),
            )
            .unwrap(),
        )
        .unwrap();
        let mut tar = tar::Builder::new(flate2::write::GzEncoder::new(
            vec![],
            flate2::Compression::default(),
        ));
        let mut files = BTreeMap::new();
        for (path, content) in snapshot.as_object().unwrap() {
            if let Some(content) = content.as_str() {
                files.insert(path.clone(), content.as_bytes().to_vec());
            }
        }
        files.insert(
            "agent.json".into(),
            serde_json::to_vec(&self.manifest).unwrap(),
        );
        files.insert(
            "tool.js".into(),
            b"process.stdin.on('data', b => process.stdout.write(b));".to_vec(),
        );
        for (path, data) in files {
            let mut header = tar::Header::new_gnu();
            header.set_size(data.len() as u64);
            header.set_mode(0o644);
            header.set_cksum();
            tar.append_data(&mut header, path, data.as_slice()).unwrap();
        }
        self.archive = tar.into_inner().unwrap().finish().unwrap();
        self.item["integrity"] = json!(format!("{:x}", Sha256::digest(&self.archive)));
    }
}
impl Registry {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "agentpm-frozen-install-{:032x}",
            rand::random::<u128>()
        ));
        fs::create_dir_all(&root).unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let state = Arc::new(Mutex::new(State::default()));
        let stop = Arc::new(AtomicBool::new(false));
        let (shared, stopping, origin) = (state.clone(), stop.clone(), url.clone());
        let worker = std::thread::spawn(move || {
            while !stopping.load(Ordering::Relaxed) {
                let (mut stream, _) = match listener.accept() {
                    Ok(value) => value,
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(2));
                        continue;
                    }
                    Err(error) => panic!("{error}"),
                };
                stream.set_nonblocking(false).unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(15)))
                    .unwrap();
                let mut headers = vec![];
                while !headers.ends_with(b"\r\n\r\n") {
                    let mut byte = [0];
                    if stream.read_exact(&mut byte).is_err() {
                        break;
                    }
                    headers.push(byte[0]);
                }
                let headers = String::from_utf8(headers).unwrap();
                let Some(path) = headers.split_whitespace().nth(1).map(str::to_owned) else {
                    continue;
                };
                let length: usize = headers
                    .lines()
                    .find_map(|line| {
                        line.to_ascii_lowercase()
                            .strip_prefix("content-length:")
                            .map(|v| v.trim().parse().unwrap())
                    })
                    .unwrap_or(0);
                let mut data = vec![0; length];
                stream.read_exact(&mut data).unwrap();
                let request: Value = if data.is_empty() {
                    Value::Null
                } else {
                    serde_json::from_slice(&data).unwrap()
                };
                let mut state = shared.lock().unwrap();
                state.requests.push((path.clone(), request.clone()));
                let body = if path == "/v1/tools/install/resolve" {
                    let mut pending = request["items"].as_array().unwrap().clone();
                    let mut items = BTreeMap::new();
                    while let Some(req) = pending.pop() {
                        let selected = state
                            .packages
                            .values()
                            .find(|p| {
                                p.item["name"] == req["name"]
                                    && matches_version(
                                        p.item["version"].as_str().unwrap(),
                                        req["range"].as_str().unwrap(),
                                    )
                            })
                            .unwrap();
                        if items
                            .insert(key(&selected.item), selected.item.clone())
                            .is_some()
                        {
                            continue;
                        }
                        pending.extend(requirements(&selected.manifest));
                    }
                    if let Some(omitted) = &state.omit_resolved {
                        items.remove(omitted);
                    }
                    serde_json::to_vec(&json!({"items":items.into_values().collect::<Vec<_>>()}))
                        .unwrap()
                } else if path == "/v1/tools/install/init" {
                    let artifacts = request["items"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|item| {
                            let mut art = state.packages[&key(item)].item.clone();
                            if state.bad_digest {
                                art["integrity"] = json!("0".repeat(64));
                            }
                            let path = if state.unusable_urls {
                                "/offline".to_owned()
                            } else {
                                format!("/artifact/{}", key(item))
                            };
                            art["presigned_url"] = json!(format!("{origin}{path}"));
                            art
                        })
                        .collect::<Vec<_>>();
                    serde_json::to_vec(&json!({"session_id":"fixture","expires_at":"2099-01-01T00:00:00Z","artifacts":artifacts})).unwrap()
                } else if let Some(key) = path.strip_prefix("/artifact/") {
                    if state.corrupt_download {
                        b"not the pinned archive".to_vec()
                    } else {
                        state.packages[key].archive.clone()
                    }
                } else if path == "/v1/tools/install/finalize" {
                    b"{}".to_vec()
                } else {
                    panic!("unexpected fixture request {path}")
                };
                write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len()).unwrap();
                let _ = stream.write_all(&body);
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
    fn add(&self, package: Package) {
        // Positive authored manifests must pass the shipping lint path.
        let path = self.root.join(format!(
            "lint-{}-{}.json",
            package.item["name"].as_str().unwrap().replace('/', "-"),
            package.item["version"].as_str().unwrap()
        ));
        fs::write(&path, package.manifest.to_string()).unwrap();
        let mut args = vec!["lint", path.to_str().unwrap()];
        if package.manifest.get("standard").is_none() {
            args.push("--legacy");
        }
        success(&self.command(&args));
        fs::remove_file(path).unwrap();
        self.state
            .lock()
            .unwrap()
            .packages
            .insert(key(&package.item), package);
    }
    fn graph(&self) {
        self.add(package("tool", "echo", "1.0.0"));
        let mut legacy = package("tool", "echo", "2.0.0");
        legacy.manifest.as_object_mut().unwrap().remove("standard");
        legacy.item["standard_origin"] = json!("legacy_inferred");
        legacy.rebuild();
        self.add(legacy);
        let mut skill = package("skill", "skill", "1.0.0");
        skill.manifest["tools"] = json!(["@fixture/echo@1.0.0"]);
        skill.rebuild();
        self.add(skill);
        for kind in ["knowledge", "memory", "profile", "loop"] {
            self.add(package(kind, kind, "1.0.0"));
        }
        let mut agent = package("agent", "agent", "1.0.0");
        agent.manifest["tools"] = json!(["@fixture/echo@2.0.0"]);
        agent.manifest["skills"] = json!(["@fixture/skill@1.0.0"]);
        agent.manifest["knowledge"] = json!(["@fixture/knowledge@1.0.0"]);
        agent.manifest["memory"] = json!(["@fixture/memory@1.0.0"]);
        agent.manifest["profiles"] = json!(["@fixture/profile@1.0.0"]);
        agent.manifest["loop"] = json!("@fixture/loop@1.0.0");
        agent.rebuild();
        self.add(agent);
    }
    fn command(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_agentpm"))
            .current_dir(&self.root)
            .stdin(Stdio::null())
            .args(["--base-url", &self.url])
            .args(args)
            .env("AGENTPM_TOKEN", "apm_fixture")
            .env("AGENTPM_TELEMETRY", "0")
            .env("XDG_CONFIG_HOME", self.root.join("config"))
            .env("APPDATA", self.root.join("config"))
            .env("LOCALAPPDATA", self.root.join("cache"))
            .output()
            .unwrap()
    }
    fn lock(&self) -> Value {
        read(&self.root.join("agent.lock"))
    }
    fn write_lock(&self, value: &Value) {
        fs::write(self.root.join("agent.lock"), value.to_string()).unwrap();
    }
    fn calls(&self) -> Vec<String> {
        self.state
            .lock()
            .unwrap()
            .requests
            .iter()
            .map(|(path, _)| path.clone())
            .collect()
    }
    fn clear_calls(&self) {
        self.state.lock().unwrap().requests.clear();
    }
    fn clean_install(&self) {
        fs::remove_dir_all(self.root.join(".agentpm")).unwrap();
        self.clear_calls();
    }
    fn installed(&self) -> BTreeMap<String, Vec<u8>> {
        fn walk(path: &Path, base: &Path, values: &mut BTreeMap<String, Vec<u8>>) {
            if !path.is_dir() {
                return;
            }
            for entry in fs::read_dir(path).unwrap() {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    walk(&path, base, values);
                } else {
                    values.insert(
                        path.strip_prefix(base)
                            .unwrap()
                            .to_string_lossy()
                            .into_owned(),
                        fs::read(&path).unwrap(),
                    );
                }
            }
        }
        let mut values = BTreeMap::new();
        for kind in [
            "tools",
            "agents",
            "skills",
            "knowledge",
            "memory",
            "profiles",
            "loops",
        ] {
            walk(
                &self.root.join(".agentpm").join(kind),
                &self.root,
                &mut values,
            );
        }
        values
    }
}
impl Drop for Registry {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
        let _ = fs::remove_dir_all(&self.root);
    }
}
fn matches_version(version: &str, range: &str) -> bool {
    let version = semver::Version::parse(version).unwrap();
    if let Ok(exact) = semver::Version::parse(range) {
        version == exact
    } else {
        semver::VersionReq::parse(range).unwrap().matches(&version)
    }
}
fn requirements(manifest: &Value) -> Vec<Value> {
    if !matches!(manifest["kind"].as_str(), Some("agent" | "skill")) {
        return vec![];
    }
    let mut out = vec![];
    for (field, kind) in [
        ("tools", "tool"),
        ("skills", "skill"),
        ("knowledge", "knowledge"),
        ("memory", "memory"),
        ("profiles", "profile"),
        ("loop", "loop"),
    ] {
        let entries = if field == "loop" {
            manifest.get(field).into_iter().collect()
        } else {
            manifest[field]
                .as_array()
                .map(|a| a.iter().collect::<Vec<_>>())
                .unwrap_or_default()
        };
        for entry in entries {
            let (name, range) = if let Some(value) = entry.as_str() {
                match value.rfind('@').filter(|i| *i > 0) {
                    Some(i) => (&value[..i], &value[i + 1..]),
                    None => (value, "*"),
                }
            } else {
                (
                    entry["name"].as_str().unwrap(),
                    entry["version"].as_str().unwrap(),
                )
            };
            out.push(json!({"kind":kind,"name":name,"range":range}));
        }
    }
    out
}
fn read(path: &Path) -> Value {
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}
fn success(out: &Output) {
    assert!(
        out.status.success(),
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
}
fn failure(out: &Output, message: &str) {
    assert_eq!(
        out.status.code(),
        Some(1),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let report: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(report["status"], "error");
    assert!(
        report["error"]["message"]
            .as_str()
            .unwrap()
            .contains(message),
        "{report}"
    );
}
fn local_agent(r: &Registry, deps: Value) {
    let mut manifest = package("agent", "consumer", "1.0.0").manifest;
    manifest["tools"] = deps;
    fs::write(r.root.join("agent.json"), manifest.to_string()).unwrap();
}

#[test]
fn direct_agent_clean_replay_restores_all_kinds_and_two_exact_tool_versions() {
    let r = Registry::new();
    r.graph();
    success(&r.command(&["install", "@fixture/agent@1.0.0", "--machine"]));
    let lock = fs::read(r.root.join("agent.lock")).unwrap();
    let installed = r.installed();
    assert_eq!(r.lock()["packages"].as_object().unwrap().len(), 8);
    assert_eq!(
        r.lock()["packages"]["tool:@fixture/echo@2.0.0"]["standard_origin"],
        "legacy_inferred"
    );
    for _ in 0..2 {
        r.clean_install();
        success(&r.command(&["install", "@fixture/agent@1.0.0", "--frozen", "--machine"]));
        assert_eq!(fs::read(r.root.join("agent.lock")).unwrap(), lock);
        assert_eq!(r.installed(), installed);
        assert!(!r.calls().iter().any(|p| p.ends_with("/resolve")));
        assert_eq!(
            r.calls()
                .iter()
                .filter(|p| p.starts_with("/artifact/"))
                .count(),
            8
        );
    }
}
#[test]
fn direct_skill_replay_preserves_kind_and_inherited_tool() {
    let r = Registry::new();
    r.graph();
    success(&r.command(&["install", "@fixture/skill@1.0.0", "--machine"]));
    let before = r.installed();
    let lock = fs::read(r.root.join("agent.lock")).unwrap();
    r.clean_install();
    success(&r.command(&["install", "@fixture/skill@1.0.0", "--frozen", "--machine"]));
    assert_eq!(r.installed(), before);
    assert_eq!(fs::read(r.root.join("agent.lock")).unwrap(), lock);
    assert_eq!(r.lock()["packages"].as_object().unwrap().len(), 2);
}
#[test]
fn local_agent_replay_restores_transitive_skill_tool_and_preserves_sources() {
    let r = Registry::new();
    r.graph();
    local_agent(&r, json!(["@fixture/echo@2.0.0"]));
    let mut manifest = read(&r.root.join("agent.json"));
    manifest["skills"] = json!(["@fixture/skill@1.0.0"]);
    fs::write(r.root.join("agent.json"), manifest.to_string()).unwrap();
    success(&r.command(&["install", "--machine"]));
    let original = fs::read(r.root.join("agent.json")).unwrap();
    let installed = r.installed();
    r.clean_install();
    success(&r.command(&["install", "--frozen", "--machine"]));
    assert_eq!(r.installed(), installed);
    assert_eq!(fs::read(r.root.join("agent.json")).unwrap(), original);
}
#[test]
fn changed_authored_range_fails_before_init_and_keeps_installed_state() {
    let r = Registry::new();
    r.graph();
    local_agent(&r, json!(["@fixture/echo@1.0.0"]));
    success(&r.command(&["install", "--machine"]));
    let installed = r.installed();
    let lock = fs::read(r.root.join("agent.lock")).unwrap();
    local_agent(&r, json!(["@fixture/echo@^2"]));
    r.clear_calls();
    failure(
        &r.command(&["install", "--frozen", "--machine"]),
        "No locked tool",
    );
    assert!(r.calls().is_empty());
    assert_eq!(r.installed(), installed);
    assert_eq!(fs::read(r.root.join("agent.lock")).unwrap(), lock);
}
#[test]
fn dangling_edge_wrong_kind_and_key_identity_fail_before_network() {
    let r = Registry::new();
    r.graph();
    success(&r.command(&["install", "@fixture/agent@1.0.0", "--machine"]));
    let original = r.lock();
    let installed = r.installed();
    for fault in ["missing", "wrong-kind", "wrong-key"] {
        let mut lock = original.clone();
        match fault {
            "missing" => {
                lock["packages"]
                    .as_object_mut()
                    .unwrap()
                    .remove("skill:@fixture/skill@1.0.0");
            }
            "wrong-kind" => {
                lock["roots"]["agent:@fixture/agent@1.0.0"]["tools"] =
                    json!(["skill:@fixture/skill@1.0.0"])
            }
            _ => lock["packages"]["skill:@fixture/skill@1.0.0"]["kind"] = json!("tool"),
        }
        r.write_lock(&lock);
        let saved = fs::read(r.root.join("agent.lock")).unwrap();
        r.clear_calls();
        failure(
            &r.command(&["install", "@fixture/agent@1.0.0", "--frozen", "--machine"]),
            match fault {
                "missing" => "Missing locked dependency",
                "wrong-kind" => "Wrong kind",
                _ => "Locked package key differs",
            },
        );
        assert!(r.calls().is_empty());
        assert_eq!(r.installed(), installed);
        assert_eq!(fs::read(r.root.join("agent.lock")).unwrap(), saved);
    }
}
#[test]
fn hidden_transitive_dependency_fails_before_install_replacement_or_finalize() {
    let r = Registry::new();
    r.graph();
    success(&r.command(&["install", "@fixture/agent@1.0.0", "--machine"]));
    // Remove the Skill's Tool from the lock; no recorded root edge names it.
    let mut lock = r.lock();
    lock["packages"]
        .as_object_mut()
        .unwrap()
        .remove("tool:@fixture/echo@1.0.0");
    r.write_lock(&lock);
    let saved = fs::read(r.root.join("agent.lock")).unwrap();
    let installed = r.installed();
    r.clear_calls();
    failure(
        &r.command(&[
            "install",
            "@fixture/agent@1.0.0",
            "--frozen",
            "--refresh",
            "--machine",
        ]),
        "No locked tool",
    );
    assert!(
        !r.calls()
            .iter()
            .any(|p| p.ends_with("/resolve") || p.ends_with("/finalize"))
    );
    assert_eq!(r.installed(), installed);
    assert_eq!(fs::read(r.root.join("agent.lock")).unwrap(), saved);
}
#[test]
fn init_digest_drift_and_corrupt_payload_cannot_change_existing_install() {
    for corrupt in [false, true] {
        let r = Registry::new();
        r.graph();
        success(&r.command(&["install", "@fixture/skill@1.0.0", "--machine"]));
        let saved = fs::read(r.root.join("agent.lock")).unwrap();
        let installed = r.installed();
        {
            let mut s = r.state.lock().unwrap();
            s.bad_digest = !corrupt;
            s.corrupt_download = corrupt;
        }
        r.clear_calls();
        failure(
            &r.command(&[
                "install",
                "@fixture/skill@1.0.0",
                "--frozen",
                "--refresh",
                "--machine",
            ]),
            if corrupt {
                "sha256 mismatch"
            } else {
                "differs from the lock/resolve plan"
            },
        );
        assert_eq!(r.installed(), installed);
        assert_eq!(fs::read(r.root.join("agent.lock")).unwrap(), saved);
        assert!(!r.calls().iter().any(|p| p.ends_with("/finalize")));
    }
}
#[test]
fn cache_reuse_requires_registry_session_but_not_artifact_get_and_repairs_tampering() {
    let mut r = Registry::new();
    r.graph();
    success(&r.command(&["install", "@fixture/skill@1.0.0", "--machine"]));
    r.state.lock().unwrap().unusable_urls = true;
    r.clear_calls();
    success(&r.command(&["install", "@fixture/skill@1.0.0", "--frozen", "--machine"]));
    assert_eq!(
        r.calls(),
        vec!["/v1/tools/install/init", "/v1/tools/install/finalize"]
    );
    r.state.lock().unwrap().unusable_urls = false;
    for entry in fs::read_dir(r.root.join(".agentpm/cache")).unwrap() {
        fs::write(entry.unwrap().path(), b"tampered").unwrap();
    }
    r.clear_calls();
    success(&r.command(&["install", "@fixture/skill@1.0.0", "--frozen", "--machine"]));
    assert_eq!(
        r.calls()
            .iter()
            .filter(|p| p.starts_with("/artifact/"))
            .count(),
        2
    );
    r.stop.store(true, Ordering::Relaxed);
    let _ = r.worker.take().unwrap().join();
    let saved = fs::read(r.root.join("agent.lock")).unwrap();
    let installed = r.installed();
    failure(
        &r.command(&["install", "@fixture/skill@1.0.0", "--frozen", "--machine"]),
        "http error",
    );
    assert_eq!(r.installed(), installed);
    assert_eq!(fs::read(r.root.join("agent.lock")).unwrap(), saved);
}
#[test]
fn empty_local_agent_is_stable_noop_and_empty_published_agent_is_still_installed() {
    let r = Registry::new();
    local_agent(&r, json!([]));
    success(&r.command(&["install", "--machine"]));
    assert!(r.calls().is_empty());
    let lock = fs::read(r.root.join("agent.lock")).unwrap();
    success(&r.command(&["install", "--machine"]));
    assert!(r.calls().is_empty());
    assert_eq!(fs::read(r.root.join("agent.lock")).unwrap(), lock);
    success(&r.command(&["install", "--frozen", "--machine"]));
    assert!(r.calls().is_empty());
    assert_eq!(fs::read(r.root.join("agent.lock")).unwrap(), lock);
    r.add(package("agent", "empty", "1.0.0"));
    success(&r.command(&["install", "@fixture/empty@1.0.0", "--machine"]));
    assert!(
        r.root
            .join(".agentpm/agents/fixture/empty/1.0.0/agent.json")
            .exists()
    );
    let published_lock = fs::read(r.root.join("agent.lock")).unwrap();
    r.clear_calls();
    success(&r.command(&["install", "@fixture/empty@1.0.0", "--machine"]));
    assert_eq!(fs::read(r.root.join("agent.lock")).unwrap(), published_lock);
    let state = r.state.lock().unwrap();
    for (path, request) in &state.requests {
        if path.ends_with("/resolve") {
            assert_eq!(request["items"].as_array().unwrap().len(), 1);
        }
    }
    drop(state);
    r.clean_install();
    success(&r.command(&["install", "@fixture/empty@1.0.0", "--frozen", "--machine"]));
    assert_eq!(r.lock()["packages"].as_object().unwrap().len(), 1);
}
#[test]
fn frozen_without_lock_fails_and_templates_still_use_new() {
    let r = Registry::new();
    local_agent(&r, json!([]));
    failure(
        &r.command(&["install", "--frozen", "--machine"]),
        "--frozen requires agent.lock",
    );
    assert!(r.calls().is_empty());
    assert!(!r.root.join("agent.lock").exists());
    r.add(package("template", "starter", "1.0.0"));
    failure(
        &r.command(&["install", "@fixture/starter@1.0.0", "--machine"]),
        "use `agentpm new`",
    );
    assert!(!r.calls().iter().any(|p| p.ends_with("/init")));
}

#[test]
fn multi_agent_workspace_keeps_distinct_edges_and_restores_both_tool_versions() {
    let r = Registry::new();
    r.graph();
    local_agent(&r, json!(["@fixture/echo@1.0.0"]));
    let mut second = package("agent", "second", "1.0.0").manifest;
    second["tools"] = json!(["@fixture/echo@2.0.0"]);
    fs::create_dir_all(r.root.join("agents")).unwrap();
    fs::write(r.root.join("agents/second.agent.json"), second.to_string()).unwrap();
    let workspace = json!({"schema_version":1,"manifests":["agent.json","agents/second.agent.json"],"package_roots":{}});
    fs::write(r.root.join("agentpm.workspace.json"), workspace.to_string()).unwrap();
    success(&r.command(&["install", "--machine"]));
    let lock = r.lock();
    assert_eq!(
        lock["roots"]["local:agent"]["tools"],
        json!(["tool:@fixture/echo@1.0.0"])
    );
    assert_eq!(
        lock["roots"]["local:agent:agents/second.agent.json"]["tools"],
        json!(["tool:@fixture/echo@2.0.0"])
    );
    let saved = fs::read(r.root.join("agent.lock")).unwrap();
    let installed = r.installed();
    r.clean_install();
    success(&r.command(&["install", "--frozen", "--machine"]));
    assert_eq!(r.installed(), installed);
    assert_eq!(fs::read(r.root.join("agent.lock")).unwrap(), saved);
    assert!(!r.calls().iter().any(|p| p.ends_with("/resolve")));
    // Regeneration preserves every standard/origin and exact edge.
    success(&r.command(&["install", "--machine"]));
    assert_eq!(r.lock()["packages"], lock["packages"]);
    assert_eq!(r.lock()["roots"], lock["roots"]);
}
#[test]
fn empty_workspace_records_all_local_roots_without_network() {
    let r = Registry::new();
    local_agent(&r, json!([]));
    fs::create_dir_all(r.root.join("agents")).unwrap();
    fs::write(
        r.root.join("agents/second.agent.json"),
        package("agent", "second", "1.0.0").manifest.to_string(),
    )
    .unwrap();
    fs::write(
        r.root.join("agentpm.workspace.json"),
        json!({"schema_version":1,"manifests":["agent.json","agents/second.agent.json"]})
            .to_string(),
    )
    .unwrap();
    success(&r.command(&["install", "--machine"]));
    assert_eq!(r.lock()["roots"].as_object().unwrap().len(), 2);
    assert!(r.calls().is_empty());
    let saved = fs::read(r.root.join("agent.lock")).unwrap();
    success(&r.command(&["install", "--frozen", "--machine"]));
    assert_eq!(fs::read(r.root.join("agent.lock")).unwrap(), saved);
    assert!(r.calls().is_empty());
}
#[test]
fn local_skill_and_metadata_free_v2_v3_locks_replay_without_rewrite() {
    for (kind, version) in [("tool", 2), ("skill", 3)] {
        let r = Registry::new();
        r.graph();
        let spec = if kind == "tool" {
            "@fixture/echo@1.0.0"
        } else {
            "@fixture/skill@1.0.0"
        };
        success(&r.command(&["install", spec, "--machine"]));
        let mut lock = r.lock();
        for collection in ["packages", "roots"] {
            if let Some(entries) = lock.get_mut(collection).and_then(Value::as_object_mut) {
                for value in entries.values_mut() {
                    value.as_object_mut().unwrap().remove("standard");
                    value.as_object_mut().unwrap().remove("standard_origin");
                }
            }
        }
        lock["lockfile_version"] = json!(version);
        r.write_lock(&lock);
        let saved = fs::read(r.root.join("agent.lock")).unwrap();
        r.clean_install();
        success(&r.command(&["install", spec, "--frozen", "--machine"]));
        assert_eq!(fs::read(r.root.join("agent.lock")).unwrap(), saved);
    }
    let r = Registry::new();
    r.graph();
    let mut manifest = package("skill", "consumer", "1.0.0").manifest;
    manifest["tools"] = json!(["@fixture/echo@1.0.0"]);
    fs::write(r.root.join("agent.json"), manifest.to_string()).unwrap();
    success(&r.command(&["install", "--machine"]));
    let saved = fs::read(r.root.join("agent.lock")).unwrap();
    r.clean_install();
    success(&r.command(&["install", "--frozen", "--machine"]));
    assert_eq!(fs::read(r.root.join("agent.lock")).unwrap(), saved);
    assert_eq!(
        r.lock()["roots"]["local:skill"]["standard_origin"],
        "declared"
    );
}
#[test]
fn immutable_transitive_range_is_checked_without_resolving_new_versions() {
    let r = Registry::new();
    r.graph();
    // An honest immutable Skill asks for a version absent from this fabricated lock.
    {
        let mut state = r.state.lock().unwrap();
        let skill = state
            .packages
            .get_mut("skill:@fixture/skill@1.0.0")
            .unwrap();
        skill.manifest["tools"] = json!(["@fixture/echo@^3"]);
        skill.rebuild();
    }
    // Seed only the lock via a successful direct Tool install, then pin the
    // immutable Skill manually to simulate a stale/incomplete external lock.
    success(&r.command(&["install", "@fixture/echo@1.0.0", "--machine"]));
    let mut lock = r.lock();
    let skill = r.state.lock().unwrap().packages["skill:@fixture/skill@1.0.0"]
        .item
        .clone();
    lock["packages"]["skill:@fixture/skill@1.0.0"] = skill;
    r.write_lock(&lock);
    let saved = fs::read(r.root.join("agent.lock")).unwrap();
    let installed = r.installed();
    r.clear_calls();
    failure(
        &r.command(&["install", "@fixture/skill@1.0.0", "--frozen", "--machine"]),
        "No locked tool @fixture/echo satisfies ^3",
    );
    assert_eq!(r.installed(), installed);
    assert_eq!(fs::read(r.root.join("agent.lock")).unwrap(), saved);
    assert!(
        !r.calls()
            .iter()
            .any(|p| p.ends_with("/resolve") || p.ends_with("/finalize"))
    );
}
#[test]
fn stored_registry_edge_must_satisfy_immutable_manifest_selector() {
    let r = Registry::new();
    r.graph();
    success(&r.command(&["install", "@fixture/agent@1.0.0", "--machine"]));
    let mut lock = r.lock();
    lock["roots"]["agent:@fixture/agent@1.0.0"]["tools"] = json!(["tool:@fixture/echo@1.0.0"]);
    r.write_lock(&lock);
    let saved = fs::read(r.root.join("agent.lock")).unwrap();
    let installed = r.installed();
    r.clear_calls();
    failure(
        &r.command(&["install", "@fixture/agent@1.0.0", "--frozen", "--machine"]),
        "No locked tool @fixture/echo satisfies 2.0.0",
    );
    assert_eq!(r.installed(), installed);
    assert_eq!(fs::read(r.root.join("agent.lock")).unwrap(), saved);
    assert!(
        !r.calls()
            .iter()
            .any(|p| p.ends_with("/resolve") || p.ends_with("/finalize"))
    );
}
#[test]
fn direct_frozen_spec_enforces_range_and_exact_version_with_same_named_packages() {
    let r = Registry::new();
    r.graph();
    success(&r.command(&["install", "@fixture/agent@1.0.0", "--machine"]));
    let saved = fs::read(r.root.join("agent.lock")).unwrap();
    success(&r.command(&["install", "@fixture/echo@1.0.0", "--frozen", "--machine"]));
    success(&r.command(&["install", "@fixture/echo@^2", "--frozen", "--machine"]));
    for spec in ["@fixture/echo@^3", "@fixture/echo"] {
        r.clear_calls();
        failure(
            &r.command(&["install", spec, "--frozen", "--machine"]),
            "must identify exactly one locked kind/version",
        );
        assert!(r.calls().is_empty());
    }
    assert_eq!(fs::read(r.root.join("agent.lock")).unwrap(), saved);
}

#[test]
fn ordinary_incomplete_registry_graph_fails_before_replacing_existing_packages() {
    let r = Registry::new();
    r.graph();
    success(&r.command(&["install", "@fixture/echo@2.0.0", "--machine"]));
    let installed = r.installed();
    let lock = fs::read(r.root.join("agent.lock")).unwrap();
    r.clear_calls();
    r.state.lock().unwrap().omit_resolved = Some("tool:@fixture/echo@1.0.0".into());
    failure(
        &r.command(&["install", "@fixture/skill@1.0.0", "--machine"]),
        "missing from the resolved package set",
    );
    assert_eq!(r.installed(), installed);
    assert_eq!(fs::read(r.root.join("agent.lock")).unwrap(), lock);
    assert!(!r.calls().iter().any(|p| p.ends_with("/finalize")));
}
#[test]
fn legacy_v1_tool_pin_replays_and_incompatible_range_fails_without_rewrite() {
    let r = Registry::new();
    r.add(package("tool", "echo", "1.0.0"));
    let item = r.state.lock().unwrap().packages["tool:@fixture/echo@1.0.0"]
        .item
        .clone();
    let lock = json!({"lockfile_version":1,"generated":"2026-11-20T00:00:00Z","dependencies":{
        "@fixture/echo":{"version":"1.0.0","integrity":item["integrity"]}}});
    r.write_lock(&lock);
    let saved = fs::read(r.root.join("agent.lock")).unwrap();
    success(&r.command(&["install", "@fixture/echo@^1", "--frozen", "--machine"]));
    assert_eq!(fs::read(r.root.join("agent.lock")).unwrap(), saved);
    r.clear_calls();
    failure(
        &r.command(&["install", "@fixture/echo@^2", "--frozen", "--machine"]),
        "No locked tool",
    );
    assert!(r.calls().is_empty());
    assert_eq!(fs::read(r.root.join("agent.lock")).unwrap(), saved);
}

#[test]
fn changed_local_root_identity_and_removed_dependencies_require_regeneration() {
    let r = Registry::new();
    r.graph();
    local_agent(&r, json!(["@fixture/echo@1.0.0"]));
    success(&r.command(&["install", "--machine"]));
    let original = read(&r.root.join("agent.json"));
    let saved = fs::read(r.root.join("agent.lock")).unwrap();
    let installed = r.installed();
    for field in ["name", "version", "tools"] {
        let mut manifest = original.clone();
        manifest[field] = match field {
            "name" => json!("renamed"),
            "version" => json!("2.0.0"),
            _ => json!([]),
        };
        fs::write(r.root.join("agent.json"), manifest.to_string()).unwrap();
        r.clear_calls();
        failure(
            &r.command(&["install", "--frozen", "--machine"]),
            if field == "tools" {
                "Locked dependency edges differ"
            } else {
                "Local root identity differs"
            },
        );
        assert!(r.calls().is_empty());
        assert_eq!(r.installed(), installed);
        assert_eq!(fs::read(r.root.join("agent.lock")).unwrap(), saved);
    }
}

#[test]
fn registry_root_provenance_cannot_contradict_verified_manifest() {
    let r = Registry::new();
    r.graph();
    success(&r.command(&["install", "@fixture/agent@1.0.0", "--machine"]));
    let mut lock = r.lock();
    lock["roots"]["agent:@fixture/agent@1.0.0"]["standard"] =
        json!({"id":"agentpm","version":"1.0.0"});
    lock["roots"]["agent:@fixture/agent@1.0.0"]["standard_origin"] = json!("legacy_inferred");
    r.write_lock(&lock);
    let saved = fs::read(r.root.join("agent.lock")).unwrap();
    let installed = r.installed();
    r.clear_calls();
    failure(
        &r.command(&["install", "@fixture/agent@1.0.0", "--frozen", "--machine"]),
        "APDS standard/origin differs from the embedded package manifest",
    );
    assert_eq!(r.installed(), installed);
    assert_eq!(fs::read(r.root.join("agent.lock")).unwrap(), saved);
    assert!(
        !r.calls()
            .iter()
            .any(|p| p.ends_with("/resolve") || p.ends_with("/finalize"))
    );
}

#[test]
fn frozen_refresh_repairs_modified_payload_without_downloading_it_twice() {
    let r = Registry::new();
    r.graph();
    success(&r.command(&["install", "@fixture/skill@1.0.0", "--machine"]));
    let installed = r.installed();
    let saved = fs::read(r.root.join("agent.lock")).unwrap();
    let script = r.root.join(".agentpm/tools/fixture/echo/1.0.0/tool.js");
    fs::write(&script, b"modified after installation").unwrap();
    r.clear_calls();
    success(&r.command(&[
        "install",
        "@fixture/skill@1.0.0",
        "--frozen",
        "--refresh",
        "--machine",
    ]));
    assert_eq!(r.installed(), installed);
    assert_eq!(fs::read(r.root.join("agent.lock")).unwrap(), saved);
    assert_eq!(
        r.calls()
            .iter()
            .filter(|p| p.starts_with("/artifact/"))
            .count(),
        2
    );
    assert!(!r.calls().iter().any(|p| p.ends_with("/resolve")));
}
