//! Real CLI build/sign/upload paths; network calls stay on a local fixture server.
use base64::{Engine, engine::general_purpose::STANDARD as B64};
use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
};

struct Fixture(PathBuf);
impl Fixture {
    fn new(kind: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "agentpm-ci-publish-{:032x}",
            rand::random::<u128>()
        ));
        for dir in ["home/AppData/Roaming", "home/AppData/Local"] {
            fs::create_dir_all(root.join(dir)).unwrap();
        }
        let manifest = if kind == "tool" {
            json!({"kind":"tool","name":"ci-example","version":"0.1.0","description":"Automated publication example.","runtime":{"type":"python","version":"3.11"},"entrypoint":{"command":"python","args":["tool.py"]},"files":["tool.py"],"inputs":{"type":"object"},"outputs":{"type":"object"}})
        } else {
            json!({"kind":"agent","name":"ci-example","version":"0.1.0","description":"Automated Agent publication example.","tools":[]})
        };
        fs::write(
            root.join("agent.json"),
            serde_json::to_vec(&manifest).unwrap(),
        )
        .unwrap();
        fs::write(root.join("tool.py"), "print('{}')\n").unwrap();
        Self(root)
    }
    fn command(&self, verb: &str) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_agentpm"));
        command
            .current_dir(&self.0)
            .stdin(Stdio::null())
            .env("HOME", self.0.join("home"))
            .env("USERPROFILE", self.0.join("home"))
            .env("APPDATA", self.0.join("home/AppData/Roaming"))
            .env("LOCALAPPDATA", self.0.join("home/AppData/Local"))
            .env("XDG_CONFIG_HOME", self.0.join("home/config"))
            .env("XDG_DATA_HOME", self.0.join("home/data"))
            .env("AGENTPM_TELEMETRY", "0")
            .env_remove("AGENTPM_TOKEN")
            .env_remove("AGENTPM_KEY_PASSPHRASE")
            .env("AGENTPM_BASE_URL", "http://127.0.0.1:1")
            .arg(verb);
        command
    }
    fn package(&self) -> Value {
        let output = self
            .command("package")
            .args(["--machine", "--schema"])
            .arg(schema())
            .output()
            .unwrap();
        assert!(output.status.success(), "{}", text(&output));
        serde_json::from_slice(&output.stdout).unwrap()
    }
    fn encrypted_key(&self) -> PathBuf {
        use argon2::{Argon2, PasswordHasher, password_hash::SaltString};
        use chacha20poly1305::{
            XChaCha20Poly1305, XNonce,
            aead::{Aead, KeyInit},
        };
        let salt = SaltString::encode_b64(b"fixture-salt-1234").unwrap();
        let hash = Argon2::default()
            .hash_password(b"fixture-passphrase", &salt)
            .unwrap();
        let hash = hash.hash.unwrap();
        let nonce = [9; 24];
        let secret = [7; 32];
        let public = ed25519_dalek::SigningKey::from_bytes(&secret)
            .verifying_key()
            .to_bytes();
        let cipher = XChaCha20Poly1305::new_from_slice(hash.as_bytes())
            .unwrap()
            .encrypt(XNonce::from_slice(&nonce), secret.as_ref())
            .unwrap();
        let path = self.0.join("encrypted-key.json");
        fs::write(&path,serde_json::to_vec(&json!({"version":1,"label":"CI fixture","created_at":"2026-11-08T00:00:00Z","public_key_b64":B64.encode(public),"kdf":"argon2id","salt_b64":salt.as_str(),"nonce_b64":B64.encode(nonce),"ciphertext_b64":B64.encode(cipher)})).unwrap()).unwrap();
        path
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
fn machine_bundle_is_relocatable_and_changed_bytes_fail_before_network() {
    let fixture = Fixture::new("tool");
    let before = fs::read(fixture.0.join("agent.json")).unwrap();
    let value = fixture.package();
    assert_eq!(value["build"]["target"], "any");
    assert_eq!(
        value["build"]["build_target"],
        agentpm_sdk::target::Target::current().unwrap().as_str()
    );
    assert!(value["build"]["python_resolution_digest"].is_null());
    assert_eq!(before, fs::read(fixture.0.join("agent.json")).unwrap());
    assert!(!fixture.0.join("agent.lock").exists());
    let directory = fixture.0.join("relocated");
    fs::create_dir(&directory).unwrap();
    for entry in fs::read_dir(fixture.0.join("target/agentpm/build")).unwrap() {
        let entry = entry.unwrap();
        fs::copy(entry.path(), directory.join(entry.file_name())).unwrap();
    }
    let descriptor = directory.join("package-build.json");
    let output = fixture
        .command("publish")
        .arg("--build")
        .arg(&descriptor)
        .args(["--namespace", "fixture", "--token", "fixture-only"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(!text(&output).contains("differs"), "{}", text(&output));
    let archive = directory.join(value["build"]["artifact_path"].as_str().unwrap());
    fs::write(archive, b"changed").unwrap();
    let output = fixture
        .command("publish")
        .arg("--build")
        .arg(&descriptor)
        .args(["--namespace", "fixture", "--token", "fixture-only"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(
        text(&output).contains("artifact digest/size differs"),
        "{}",
        text(&output)
    );
}

#[test]
fn machine_failure_preserves_typed_payload_diagnostics() {
    let fixture = Fixture::new("tool");
    let path = fixture.0.join("agent.json");
    let mut manifest: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    manifest["files"] = json!(["tool.py", "icon.png"]);
    fs::write(path, serde_json::to_vec(&manifest).unwrap()).unwrap();
    fs::write(fixture.0.join("icon.png"), b"GIF89a").unwrap();
    let output = fixture
        .command("package")
        .args(["--machine", "--schema"])
        .arg(schema())
        .output()
        .unwrap();
    assert!(!output.status.success());
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["status"], "error");
    assert_eq!(value["error"]["diagnostics"][0]["code"], "format_mismatch");
}

#[test]
fn python_package_binds_discovered_readme_and_declared_license_even_outside_files() {
    let fixture = Fixture::new("tool");
    let path = fixture.0.join("agent.json");
    let mut manifest: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    manifest["license"] = json!({"spdx":"MIT","file":"LICENSE"});
    let authored = serde_json::to_vec(&manifest).unwrap();
    fs::write(&path, &authored).unwrap();
    fs::write(fixture.0.join("README.md"), "Discovered README\n").unwrap();
    fs::write(fixture.0.join("LICENSE"), "Declared license\n").unwrap();
    let output = fixture.package();
    let archive = fixture
        .0
        .join("target/agentpm/build")
        .join(output["build"]["artifact_path"].as_str().unwrap());
    let mut archive = tar::Archive::new(flate2::read::GzDecoder::new(
        fs::File::open(archive).unwrap(),
    ));
    let names: Vec<_> = archive
        .entries()
        .unwrap()
        .map(|e| e.unwrap().path().unwrap().into_owned())
        .collect();
    assert!(
        names.contains(&PathBuf::from("README.md")) && names.contains(&PathBuf::from("LICENSE"))
    );
    assert_eq!(fs::read(path).unwrap(), authored);
    assert_eq!(manifest["files"], json!(["tool.py"]));
    assert_eq!(output["build"]["target"], "any");
}

#[test]
fn native_packaging_records_platform_permission_behavior_without_relabeling() {
    let fixture = Fixture::new("tool");
    let manifest_path = fixture.0.join("agent.json");
    let mut manifest: Value = serde_json::from_slice(&fs::read(&manifest_path).unwrap()).unwrap();
    manifest["files"] = json!(["tool.py", "example.csv"]);
    fs::write(manifest_path, serde_json::to_vec(&manifest).unwrap()).unwrap();
    let path = fixture.0.join("example.csv");
    fs::write(&path, b"value\n1\n").unwrap();
    let portable = fixture.package();
    assert_eq!(portable["build"]["target"], "any");
    fs::rename(
        fixture.0.join("target/agentpm/build"),
        fixture.0.join("portable"),
    )
    .unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
    }
    let second = fixture.package();
    #[cfg(unix)]
    {
        assert_eq!(
            second["build"]["target"],
            agentpm_sdk::target::Target::current().unwrap().as_str()
        );
        let diagnostics = second["build"]["diagnostics"].as_array().unwrap();
        assert!(
            diagnostics
                .iter()
                .any(|d| d["code"] == "executable_member" && d["path"] == "example.csv")
        );
        assert_eq!(
            portable["build"]["manifest_digest"],
            second["build"]["manifest_digest"]
        );
        assert_eq!(
            portable["build"]["python_resolution_digest"],
            second["build"]["python_resolution_digest"]
        );
    }
    #[cfg(windows)]
    assert_eq!(second["build"]["target"], "any");
    let output = fixture
        .command("publish")
        .args([
            "--build",
            "portable/package-build.json",
            "--build",
            "target/agentpm/build/package-build.json",
            "--namespace",
            "fixture",
            "--token",
            "fixture-only",
        ])
        .output()
        .unwrap();
    assert!(!output.status.success());
    #[cfg(unix)]
    {
        assert!(
            text(&output).contains("Mixed any/concrete"),
            "{}",
            text(&output)
        );
        assert!(text(&output).contains("example.csv") && text(&output).contains("claims any"));
    }
    #[cfg(windows)]
    assert!(
        text(&output).contains("duplicate artifact target"),
        "{}",
        text(&output)
    );
}

#[test]
fn headless_signing_errors_are_actionable_and_do_not_leak_secrets_or_key_paths() {
    let fixture = Fixture::new("tool");
    fixture.package();
    let key = fixture.encrypted_key();
    for (pass, message) in [
        (None, "set AGENTPM_KEY_PASSPHRASE"),
        (Some("wrong-private-passphrase"), "Cannot decrypt"),
        (Some(""), "is empty"),
    ] {
        let mut command = fixture.command("publish");
        command
            .args([
                "--build",
                "target/agentpm/build/package-build.json",
                "--sign",
                "--key-file",
            ])
            .arg(&key)
            .args(["--token", "private-pat-fixture", "--namespace", "fixture"]);
        if let Some(pass) = pass {
            command.env("AGENTPM_KEY_PASSPHRASE", pass);
        }
        let output = command.output().unwrap();
        let text = text(&output);
        assert!(!output.status.success());
        assert!(text.contains(message), "{text}");
        assert!(
            !text.contains("private-pat-fixture")
                && !text.contains("wrong-private-passphrase")
                && !text.contains(key.to_str().unwrap()),
            "{text}"
        );
    }
    let original: Value = serde_json::from_slice(&fs::read(&key).unwrap()).unwrap();
    for (field, value, message) in [
        (
            "version",
            json!(2),
            "Unsupported encrypted signing key envelope",
        ),
        (
            "kdf",
            json!("unknown"),
            "Unsupported encrypted signing key envelope",
        ),
        (
            "public_key_b64",
            json!(B64.encode([8; 32])),
            "public/private identity mismatch",
        ),
    ] {
        let mut changed = original.clone();
        changed[field] = value;
        fs::write(&key, serde_json::to_vec(&changed).unwrap()).unwrap();
        let output = fixture
            .command("publish")
            .args([
                "--build",
                "target/agentpm/build/package-build.json",
                "--namespace",
                "fixture",
                "--token",
                "fixture-only",
                "--sign",
                "--key-file",
            ])
            .arg(&key)
            .env("AGENTPM_KEY_PASSPHRASE", "fixture-passphrase")
            .output()
            .unwrap();
        assert!(!output.status.success());
        assert!(text(&output).contains(message), "{}", text(&output));
        assert!(!text(&output).contains(key.to_str().unwrap()));
    }
    fs::remove_file(&key).unwrap();
    let output = fixture
        .command("publish")
        .args([
            "--build",
            "target/agentpm/build/package-build.json",
            "--namespace",
            "fixture",
            "--token",
            "fixture-only",
            "--sign",
            "--key-file",
        ])
        .arg(&key)
        .env("AGENTPM_KEY_PASSPHRASE", "fixture-passphrase")
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(text(&output).contains("Cannot read encrypted signing key file"));
    assert!(!text(&output).contains(key.to_str().unwrap()));
}

// Synthetic cross-target inventories exercise the CLI transport and signatures.
// Native compilation/execution is separately exercised by the hosted fixture.
fn synthetic_build(
    fixture: &Fixture,
    template: &Value,
    target: agentpm_sdk::target::Target,
) -> PathBuf {
    use sha2::{Digest, Sha256};
    let directory = fixture.0.join(target.as_str());
    fs::create_dir(&directory).unwrap();
    let archive_path = directory.join("fixture.tar.gz");
    let source = fixture
        .0
        .join("target/agentpm/build")
        .join(template["artifact_path"].as_str().unwrap());
    let mut source = tar::Archive::new(flate2::read::GzDecoder::new(
        fs::File::open(source).unwrap(),
    ));
    let mut archive = tar::Builder::new(flate2::write::GzEncoder::new(
        fs::File::create(&archive_path).unwrap(),
        flate2::Compression::default(),
    ));
    for entry in source.entries().unwrap() {
        use std::io::Read;
        let mut entry = entry.unwrap();
        let name = entry.path().unwrap().into_owned();
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes).unwrap();
        if name == Path::new(agentpm_sdk::release::ARTIFACT_METADATA_FILE) {
            bytes = serde_json::to_vec(&json!({"target":target,"format":"tar.gz"})).unwrap();
        }
        let mut header = entry.header().clone();
        header.set_size(bytes.len() as u64);
        header.set_cksum();
        archive
            .append_data(&mut header, name, bytes.as_slice())
            .unwrap();
    }
    archive.into_inner().unwrap().finish().unwrap();
    let bytes = fs::read(archive_path).unwrap();
    let sha = format!("{:x}", Sha256::digest(&bytes));
    let mut descriptor = template.clone();
    descriptor["artifact_path"] = "fixture.tar.gz".into();
    descriptor["target"] = json!(target);
    descriptor["build_target"] = json!(target);
    descriptor["artifact_size"] = json!(bytes.len());
    descriptor["artifact_sha256"] = format!("sha256:{sha}").into();
    descriptor["metadata"]["sha256"] = sha.into();
    descriptor["metadata"]["size"] = json!(bytes.len());
    descriptor["metadata"]["artifact"]["target"] = json!(target);
    for diagnostic in descriptor["diagnostics"].as_array_mut().unwrap() {
        diagnostic["target"] = json!(target);
    }
    descriptor["metadata"]["artifact"]["diagnostics"] = descriptor["diagnostics"].clone();
    let path = directory.join("package-build.json");
    fs::write(&path, serde_json::to_vec(&descriptor).unwrap()).unwrap();
    path
}

#[derive(Clone)]
struct Registry {
    base: String,
    release: std::sync::Arc<std::sync::Mutex<Option<agentpm_sdk::release::ReleaseManifest>>>,
    signatures: std::sync::Arc<std::sync::Mutex<Vec<String>>>,
    uploaded: std::sync::Arc<std::sync::Mutex<Vec<String>>>,
}

fn receipt() -> axum::Json<Value> {
    axum::Json(
        json!({"id":"fixture","name":"ci-example","version":"0.1.0","url":"https://fixture.invalid","author_signatures_accepted":1}),
    )
}

fn verify_signature(public: &str, signature: &str, bytes: &[u8]) {
    use ed25519_dalek::Verifier;
    let public: [u8; 32] = B64.decode(public).unwrap().try_into().unwrap();
    ed25519_dalek::VerifyingKey::from_bytes(&public)
        .unwrap()
        .verify(
            bytes,
            &ed25519_dalek::Signature::from_slice(&B64.decode(signature).unwrap()).unwrap(),
        )
        .unwrap();
}

async fn init_release(
    axum::extract::State(state): axum::extract::State<Registry>,
    axum::Json(body): axum::Json<Value>,
) -> axum::Json<Value> {
    let release: agentpm_sdk::release::ReleaseManifest =
        serde_json::from_value(body["release"].clone()).unwrap();
    release.validate().unwrap();
    assert_eq!(release.name, "@fixture/ci-example");
    assert_eq!(
        release.artifacts.len(),
        agentpm_sdk::target::Target::SUPPORTED.len()
    );
    let uploads:Vec<_> = release.artifacts.iter().map(|a|json!({"artifact_id":a.target,"target":a.target,
        "put_url":format!("{}/object/{}",state.base,a.target),"headers":{},"expires_at":"tomorrow","status":"pending","max_bytes":1000000})).collect();
    *state.release.lock().unwrap() = Some(release);
    axum::Json(
        json!({"release_id":"fixture","resumed":false,"expires_at":"tomorrow","max_bytes":1000000,"artifacts":uploads}),
    )
}

async fn init_legacy(
    axum::extract::State(state): axum::extract::State<Registry>,
    axum::Json(body): axum::Json<Value>,
) -> axum::Json<Value> {
    assert_eq!(body["manifest"]["kind"], "agent");
    assert!(body.get("release").is_none());
    axum::Json(
        json!({"upload_id":"legacy","put_url":format!("{}/object/legacy",state.base),"headers":{},"final_key":"fixture","expires_in":900}),
    )
}

async fn upload(
    axum::extract::State(state): axum::extract::State<Registry>,
    axum::extract::Path(target): axum::extract::Path<String>,
    headers: axum::http::HeaderMap,
    body: axum::body::Bytes,
) -> axum::http::StatusCode {
    use sha2::{Digest, Sha256};
    assert!(headers.get("authorization").is_none());
    assert!(!body.is_empty());
    if target != "legacy" {
        let release = state.release.lock().unwrap();
        let artifact = release
            .as_ref()
            .unwrap()
            .artifacts
            .iter()
            .find(|a| a.target.as_str() == target)
            .unwrap();
        assert_eq!(
            artifact.digest,
            format!("sha256:{:x}", Sha256::digest(&body))
        );
        assert_eq!(artifact.size, body.len() as u64);
    }
    state.uploaded.lock().unwrap().push(target);
    axum::http::StatusCode::OK
}

async fn finalize_release(
    axum::extract::State(state): axum::extract::State<Registry>,
    axum::Json(body): axum::Json<Value>,
) -> axum::Json<Value> {
    assert_eq!(body["require_signature"], true);
    let signature: agentpm_sdk::signing::ReleaseAuthorSignature =
        serde_json::from_value(body["author_signatures"][0].clone()).unwrap();
    let release = state.release.lock().unwrap();
    let release = release.as_ref().unwrap();
    assert_eq!(
        signature.statement_json.release_digest,
        release.digest().unwrap()
    );
    assert_eq!(
        state.uploaded.lock().unwrap().len(),
        release.artifacts.len()
    );
    verify_signature(
        &signature.public_key_b64,
        &signature.signature_b64,
        &signature.statement_json.canonical_bytes().unwrap(),
    );
    state.signatures.lock().unwrap().push("v2".into());
    receipt()
}

async fn finalize_legacy(
    axum::extract::State(state): axum::extract::State<Registry>,
    axum::Json(body): axum::Json<Value>,
) -> axum::Json<Value> {
    assert_eq!(body["require_signature"], true);
    let signature = &body["author_signatures"][0];
    let statement: agentpm_sdk::signing::AuthorSignatureV1 =
        serde_json::from_value(signature["statement_json"].clone()).unwrap();
    assert_eq!(statement.kind, "agent");
    verify_signature(
        signature["public_key_b64"].as_str().unwrap(),
        signature["signature_b64"].as_str().unwrap(),
        &statement.legacy_bytes().unwrap(),
    );
    state.signatures.lock().unwrap().push("v1".into());
    receipt()
}

#[tokio::test]
async fn real_cli_headless_signing_publishes_complete_v2_inventory_and_legacy_v1() {
    use axum::{
        Router,
        routing::{post, put},
    };
    use std::sync::{Arc, Mutex};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let state = Registry {
        base: format!("http://{}", listener.local_addr().unwrap()),
        release: Arc::new(Mutex::new(None)),
        signatures: Arc::new(Mutex::new(Vec::new())),
        uploaded: Arc::new(Mutex::new(Vec::new())),
    };
    let app = Router::new()
        .route("/v1/tools/publish/releases/init", post(init_release))
        .route("/v1/tools/publish/init", post(init_legacy))
        .route("/object/{target}", put(upload))
        .route(
            "/v1/tools/publish/releases/fixture/finalize",
            post(finalize_release),
        )
        .route("/v1/tools/publish/finalize", post(finalize_legacy))
        .with_state(state.clone());
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    for kind in ["tool", "agent"] {
        let fixture = Fixture::new(kind);
        let key = fixture.encrypted_key();
        if kind == "tool" {
            let path = fixture.0.join("agent.json");
            let mut manifest: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
            manifest["files"] = json!(["tool.py", "native.dat"]);
            fs::write(path, serde_json::to_vec(&manifest).unwrap()).unwrap();
            fs::write(fixture.0.join("native.dat"), "synthetic target fixture").unwrap();
        }
        let build = fixture.package();
        let mut command = fixture.command("publish");
        command
            .args([
                "--base-url",
                &state.base,
                "--namespace",
                "fixture",
                "--token",
                "fixture-only",
                "--sign",
                "--key-file",
            ])
            .arg(key)
            .env("AGENTPM_KEY_PASSPHRASE", "fixture-passphrase");
        if kind == "tool" {
            for target in agentpm_sdk::target::Target::SUPPORTED {
                let descriptor = synthetic_build(&fixture, &build["build"], *target);
                command
                    .arg("--build")
                    .arg(descriptor)
                    .args(["--expect-build-target", target.as_str()]);
            }
        } else {
            command.args(["--build", "target/agentpm/build/package-build.json"]);
        }
        let output = tokio::task::spawn_blocking(move || command.output().unwrap())
            .await
            .unwrap();
        assert!(output.status.success(), "{}", text(&output));
    }
    assert_eq!(*state.signatures.lock().unwrap(), ["v2", "v1"]);
    server.abort();
}
