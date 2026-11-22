use serde_json::{Value, json};
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);

struct Project(PathBuf);
impl Project {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "agentpm-authoring-{}-{}-{}",
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

    fn cli(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_agentpm"))
            .env("AGENTPM_TELEMETRY", "0")
            .current_dir(&self.0)
            .env("NO_COLOR", "1")
            .args(args)
            .output()
            .unwrap()
    }

    fn manifest(&self) -> Value {
        serde_json::from_slice(&fs::read(self.0.join("agent.json")).unwrap()).unwrap()
    }

    fn write(&self, value: &Value) {
        fs::write(
            self.0.join("agent.json"),
            serde_json::to_vec_pretty(value).unwrap(),
        )
        .unwrap();
    }

    fn init(&self, kind: &str) {
        success(self.cli(&["init", "--kind", kind, "--name", "starter"]));
    }
}
impl Drop for Project {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn success(output: Output) -> Output {
    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

const KINDS: [&str; 8] = [
    "agent",
    "tool",
    "skill",
    "knowledge",
    "memory",
    "profile",
    "loop",
    "template",
];

fn scaffold_files(root: &std::path::Path) -> Value {
    fn collect(
        root: &std::path::Path,
        dir: &std::path::Path,
        files: &mut serde_json::Map<String, Value>,
    ) {
        for entry in fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                collect(root, &path, files);
            } else {
                let name = path
                    .strip_prefix(root)
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .replace('\\', "/");
                // Text template checkout endings may be CRLF on Windows; the
                // snapshot records content rather than a Git checkout setting.
                let text = fs::read_to_string(&path).unwrap().replace("\r\n", "\n");
                files.insert(
                    name.clone(),
                    if name == "agent.json" {
                        serde_json::from_str(&text).unwrap()
                    } else {
                        json!(text)
                    },
                );
            }
        }
    }
    let mut files = serde_json::Map::new();
    collect(root, root, &mut files);
    Value::Object(files)
}

#[test]
fn default_and_explicit_scaffolds_match_snapshots_and_lint_offline() {
    let help = success(Project::new().cli(&["init", "--help"]));
    let help = String::from_utf8(help.stdout).unwrap();
    assert!(help.contains("[default: agent]"));
    assert!(help.contains("my-agent"));
    let implicit = Project::new();
    let output = success(implicit.cli(&["init"]));
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains("Agent Package created"));
    assert!(text.contains("add a Loop dependency and configure a model"));
    assert!(!implicit.0.join("main.py").exists());
    for kind in KINDS.into_iter().chain(["knowledge-vector"]) {
        let project = Project::new();
        if kind == "knowledge-vector" {
            success(project.cli(&["init", "--kind", "knowledge", "--mode", "vector"]));
        } else {
            success(project.cli(&["init", "--kind", kind]));
        }
        if kind == "agent" {
            assert_eq!(scaffold_files(&project.0), scaffold_files(&implicit.0));
        }
        let snapshot = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(format!("tests/fixtures/scaffolds/{kind}.snapshot.json"));
        let expected: Value = serde_json::from_slice(&fs::read(snapshot).unwrap()).unwrap();
        assert_eq!(scaffold_files(&project.0), expected, "{kind}");
        let manifest = project.manifest();
        let expected_kind = if kind == "knowledge-vector" {
            "knowledge"
        } else {
            kind
        };
        assert_eq!(manifest["name"], format!("my-{expected_kind}"));
        assert_eq!(manifest["kind"], expected_kind);
        let raw = fs::read_to_string(project.0.join("agent.json")).unwrap();
        assert!(raw.find("\"name\"").unwrap() < raw.find("\"description\"").unwrap());
        assert!(raw.find("\"version\": \"0.1.0\"").unwrap() < raw.find("\"description\"").unwrap());
        let lint = success(project.cli(&["lint", "--strict", "--format", "json"]));
        assert!(!String::from_utf8_lossy(&lint.stdout).contains("Missing $schema"));
        let apds = success(project.cli(&["lint", "--apds", "standalone", "--format", "json"]));
        let reports: Value = serde_json::from_slice(&apds.stdout).unwrap();
        assert_eq!(
            reports[0]["apds"]["status"], "conformant",
            "{kind}: {reports}"
        );
    }
    let manifest = implicit.manifest();
    for key in ["tools", "skills", "knowledge", "memory", "profiles"] {
        assert_eq!(manifest[key], json!([]));
    }
    assert!(manifest.get("loop").is_none());
}

#[test]
fn descriptions_round_trip_without_replacing_user_placeholders() {
    let description =
        "Quotes \" : backslash \\ / Unicode 😀 café\nnext\tline {{AGENT_NAME}} {{SKILL_TITLE}}";
    for kind in KINDS {
        let project = Project::new();
        success(project.cli(&["init", "--kind", kind, "--description", description]));
        assert_eq!(project.manifest()["description"], description, "{kind}");
        success(project.cli(&["lint", "--strict"]));
        if kind == "skill" {
            let markdown = fs::read_to_string(project.0.join("SKILL.md")).unwrap();
            let scalar = markdown
                .lines()
                .find_map(|line| line.strip_prefix("description: "))
                .unwrap();
            assert_eq!(serde_json::from_str::<String>(scalar).unwrap(), description);
        }
    }
}

#[test]
fn invalid_input_leaves_output_uncreated_and_existing_files_untouched() {
    let duplicate = Project::new();
    assert!(
        !duplicate
            .cli(&[
                "init",
                "--name",
                "first",
                "--name",
                "second",
                "--out-dir",
                "new-package"
            ])
            .status
            .success()
    );
    assert!(!duplicate.0.join("new-package").exists());
    for kind in KINDS {
        for name in [
            "",
            "@namespace/name",
            "../escape",
            "two words",
            "quote\"",
            "line\nfeed",
            "café",
            &"a".repeat(65),
        ] {
            let project = Project::new();
            let result = project.cli(&[
                "init",
                "--kind",
                kind,
                "--name",
                name,
                "--out-dir",
                "new-package",
            ]);
            assert!(!result.status.success(), "{kind} accepted {name:?}");
            assert!(!project.0.join("new-package").exists(), "{kind}: {name:?}");
        }
        let project = Project::new();
        assert!(
            !project
                .cli(&[
                    "init",
                    "--kind",
                    kind,
                    "--description",
                    " \n\t",
                    "--out-dir",
                    "new-package"
                ])
                .status
                .success()
        );
        assert!(!project.0.join("new-package").exists());
        success(project.cli(&["init", "--kind", kind]));
        let before = scaffold_files(&project.0);
        assert!(!project.cli(&["init", "--kind", kind]).status.success());
        assert_eq!(scaffold_files(&project.0), before);
    }
}

#[test]
fn secondary_file_and_staging_collisions_fail_before_creating_manifest() {
    for (kind, relative) in [
        ("skill", "SKILL.md"),
        ("template", "template/README.md"),
        ("knowledge", "knowledge/docs/context.md"),
        ("memory", "schemas/user-preference.schema.json"),
        ("profile", "README.md"),
        ("loop", "README.md"),
        ("agent", "agent.tmp"),
        ("tool", "main.tmp"),
    ] {
        let project = Project::new();
        let path = project.0.join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, "keep existing content").unwrap();
        let before = scaffold_files(&project.0);
        assert!(
            !project.cli(&["init", "--kind", kind]).status.success(),
            "{relative}"
        );
        assert_eq!(scaffold_files(&project.0), before, "{relative}");
        assert!(!project.0.join("agent.json").exists());
    }
    let project = Project::new();
    fs::write(project.0.join("knowledge"), "not a directory").unwrap();
    assert!(
        !project
            .cli(&["init", "--kind", "knowledge", "--mode", "vector"])
            .status
            .success()
    );
    assert!(!project.0.join("agent.json").exists());
}

#[cfg(unix)]
#[test]
fn scaffold_does_not_follow_existing_generated_file_or_directory_symlinks() {
    use std::os::unix::fs::symlink;
    let project = Project::new();
    let elsewhere = Project::new();
    symlink(
        elsewhere.0.join("missing.json"),
        project.0.join("agent.json"),
    )
    .unwrap();
    assert!(!project.cli(&["init"]).status.success());
    assert!(!elsewhere.0.join("missing.json").exists());
    fs::remove_file(project.0.join("agent.json")).unwrap();
    symlink(&elsewhere.0, project.0.join("schemas")).unwrap();
    assert!(!project.cli(&["init", "--kind", "memory"]).status.success());
    assert!(!project.0.join("agent.json").exists());
    assert!(fs::read_dir(&elsewhere.0).unwrap().next().is_none());
}

#[test]
fn fresh_agent_is_valid_but_harness_reports_missing_loop() {
    let project = Project::new();
    success(project.cli(&["init"]));
    success(project.cli(&["lint"]));
    // Supply only the supported local-root context; no Registry or model call.
    fs::write(
        project.0.join("agent.lock"),
        serde_json::to_vec_pretty(&json!({
            "lockfile_version": 2, "generated": "2026-11-15T00:00:00Z", "packages": {},
            "roots": {"local:agent": {"name": "my-agent", "version": "0.1.0", "tools": []}}
        }))
        .unwrap(),
    )
    .unwrap();
    let output = project.cli(&["harness", "--json"]);
    assert!(!output.status.success());
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(
        text.contains("agent_missing_loop"),
        "stdout: {text}, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(text.contains("A Loop is required by Harness, not by APDS"));
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["apds"]["status"], "conformant");
    assert_eq!(report["harness_support"], "not_runnable");
    assert_eq!(report["local_readiness"], "not_evaluated");
}

#[test]
fn all_eight_init_commands_lint_clean_and_validate_the_standard_selector() {
    let schema: Value = serde_json::from_str(include_str!(
        "../../../schemas/agentpm.manifest.schema.json"
    ))
    .unwrap();
    assert_eq!(
        schema["properties"]["standard"]["properties"]["version"],
        json!({"const":"1.0.0"})
    );
    assert_eq!(schema["additionalProperties"], json!(false));
    for kind in [
        "tool",
        "agent",
        "template",
        "skill",
        "knowledge",
        "memory",
        "profile",
        "loop",
    ] {
        let project = Project::new();
        project.init(kind);
        let output = success(project.cli(&["lint", "--strict", "--format", "json"]));
        let reports: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(reports[0]["issues"], json!([]), "{kind}: {reports}");
        let original = project.manifest();
        assert_eq!(
            original["standard"],
            json!({"id":"agentpm", "version":"1.0.0"})
        );
        let mut declared = original.clone();
        declared["standard"] = json!({"id":"agentpm", "version":"1.0.0"});
        project.write(&declared);
        success(project.cli(&["lint", "--strict"]));
        for standard in [
            json!({"future":"value"}),
            json!("unrecognized"),
            json!(null),
            json!(true),
            json!([1, 2]),
            json!(42),
        ] {
            let mut invalid = original.clone();
            invalid["standard"] = standard;
            project.write(&invalid);
            assert_eq!(project.cli(&["lint"]).status.code(), Some(1));
        }
        assert_eq!(
            original["$schema"], schema["$id"],
            "scaffolds pin the editor schema"
        );
        for typo in ["standrad", "tool"] {
            let mut value = original.clone();
            value[typo] = json!({});
            project.write(&value);
            let output = project.cli(&["lint", "--format", "json"]);
            assert_eq!(output.status.code(), Some(1), "{kind} accepted {typo}");
            let reports: Value = serde_json::from_slice(&output.stdout).unwrap();
            assert!(
                reports[0]["issues"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|issue| issue["rule_id"] == "APDS-GRAPH-001"
                        && issue["instance_path"] == format!("/{typo}"))
            );
        }
        let mut invalid = original;
        invalid["standard"] = json!({"id":"agentpm", "version":"1.0.0"});
        let required_field = match kind {
            "agent" => "name",
            "tool" => "entrypoint",
            _ => kind,
        };
        invalid.as_object_mut().unwrap().remove(required_field);
        project.write(&invalid);
        assert_eq!(
            project.cli(&["lint"]).status.code(),
            Some(1),
            "standard must not mask missing {required_field} on {kind}"
        );
    }
}

#[test]
fn tool_init_creates_a_runnable_minimal_json_echo_and_packages_its_source() {
    let project = Project::new();
    project.init("tool");
    let manifest = project.manifest();
    assert_eq!(manifest["runtime"], json!({"type":"python", "version":"3"}));
    assert_eq!(manifest["files"], json!(["main.py"]));
    assert_eq!(manifest["entrypoint"]["command"], "python3");
    assert_eq!(manifest["entrypoint"]["args"], json!(["main.py"]));
    let python = ["python3", "python"]
        .into_iter()
        .find(|command| {
            Command::new(command)
                .arg("--version")
                .output()
                .is_ok_and(|output| output.status.success())
        })
        .expect("Python required for scaffold execution fixture");
    let mut child = Command::new(python)
        .arg("main.py")
        .current_dir(&project.0)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let request = json!({"message":"hello", "nested":{"count":2}});
    child
        .stdin
        .take()
        .unwrap()
        .write_all(serde_json::to_string(&request).unwrap().as_bytes())
        .unwrap();
    let result = success(child.wait_with_output().unwrap());
    assert_eq!(
        serde_json::from_slice::<Value>(&result.stdout).unwrap(),
        request
    );
    success(project.cli(&[
        "publish",
        "--dry-run",
        "--token",
        "fixture-token",
        "--quiet",
    ]));
    assert!(
        project
            .0
            .join("target/agentpm/starter-0.1.0-python3.tar.gz")
            .is_file()
    );
}

#[test]
fn tool_init_keeps_existing_source_content() {
    let project = Project::new();
    fs::write(project.0.join("main.py"), "# existing application code\n").unwrap();
    project.init("tool");
    assert_eq!(
        fs::read_to_string(project.0.join("main.py")).unwrap(),
        "# existing application code\n"
    );
}

#[test]
fn lint_and_publish_share_human_context_ordering_and_preserve_machine_report_fields() {
    let project = Project::new();
    project.write(&serde_json::from_str(include_str!("fixtures/lint/invalid-loop.json")).unwrap());
    let pretty = project.cli(&["lint"]);
    assert_eq!(pretty.status.code(), Some(1));
    let human = String::from_utf8(pretty.stdout).unwrap();
    assert!(human.starts_with("✗ agent.json\n"));
    assert!(human.contains("at instance /loop/transitions/0/to"));
    assert!(!human.contains("\"phases\":"), "{human}");
    let publish = project.cli(&[
        "publish",
        "--dry-run",
        "--token",
        "fixture-token",
        "--quiet",
    ]);
    assert_eq!(publish.status.code(), Some(1));
    let stderr = String::from_utf8(publish.stderr).unwrap();
    assert!(
        stderr.contains(&human),
        "publish did not reuse lint's contextual report:\n{stderr}\nexpected:\n{human}"
    );
    for format in ["json", "ndjson"] {
        let output = project.cli(&["lint", "--format", format]);
        assert_eq!(output.status.code(), Some(1));
        let json: Value = serde_json::from_slice(&output.stdout).unwrap();
        let report = if format == "json" { &json[0] } else { &json };
        assert_eq!(report.as_object().unwrap().len(), 4);
        assert_eq!(report["apds"]["status"], "nonconformant");
        assert_eq!(report["file"], "agent.json");
        assert_eq!(report["ok"], false);
        for issue in report["issues"].as_array().unwrap() {
            assert_eq!(
                issue.as_object().unwrap().len(),
                6,
                "unexpected machine field: {issue}"
            );
            for key in ["file", "level", "message", "instance_path", "schema_path"] {
                assert!(issue[key].is_string(), "missing {key}: {issue}");
            }
        }
    }
}

#[test]
fn missing_literal_tool_files_block_publish_without_a_partial_artifact() {
    for missing in ["missing.txt", "missing-dir", "*.py"] {
        let project = Project::new();
        project.init("tool");
        let mut manifest = project.manifest();
        manifest["files"] = json!(["main.py", missing]);
        project.write(&manifest);
        let output = project.cli(&[
            "publish",
            "--dry-run",
            "--token",
            "fixture-token",
            "--quiet",
        ]);
        assert_eq!(output.status.code(), Some(1));
        let error = String::from_utf8_lossy(&output.stderr);
        assert!(
            error.contains("manifest.files entry") && error.contains(missing),
            "{error}"
        );
        assert!(
            !project
                .0
                .join("target/agentpm/starter-0.1.0-python3.tar.gz")
                .exists()
        );
    }
}

#[test]
fn knowledge_write_preserves_standard_and_schema_fields_outside_the_typed_manifest() {
    let project = Project::new();
    project.init("knowledge");
    let mut value = project.manifest();
    value["standard"] = json!({"id":"agentpm", "version":"1.0.0"});
    // This existing root field is schema-valid but absent from KnowledgeManifest.
    value["environment"] = json!({"vars":{"EXAMPLE":{"required":false,"description":"Opaque to the Knowledge writer", "default":"value"}}});
    project.write(&value);
    success(project.cli(&["knowledge", "build"]));
    let mut written = project.manifest();
    written["knowledge"] = value["knowledge"].clone();
    assert_eq!(written, value, "only Knowledge metadata may change");
    let first_bytes = fs::read(project.0.join("agent.json")).unwrap();
    success(project.cli(&["knowledge", "build"]));
    assert_eq!(fs::read(project.0.join("agent.json")).unwrap(), first_bytes);
}

#[test]
fn standard_survives_content_builds_and_dry_run_archives_for_all_eight_kinds() {
    for kind in [
        "tool",
        "agent",
        "template",
        "skill",
        "knowledge",
        "memory",
        "profile",
        "loop",
    ] {
        let project = Project::new();
        project.init(kind);
        let mut manifest = project.manifest();
        let standard = json!({"id":"agentpm", "version":"1.0.0"});
        manifest["standard"] = standard.clone();
        project.write(&manifest);
        if kind == "knowledge" || kind == "memory" {
            let before = fs::read(project.0.join("agent.json")).unwrap();
            success(project.cli(&[kind, "build"]));
            assert_eq!(project.manifest()["standard"], standard);
            if kind == "memory" {
                assert_eq!(fs::read(project.0.join("agent.json")).unwrap(), before);
                assert!(project.0.join("memory/build.json").is_file());
                assert!(project.0.join("memory/contracts/index.json").is_file());
            }
        }
        let before = fs::read(project.0.join("agent.json")).unwrap();
        success(project.cli(&[
            "publish",
            "--dry-run",
            "--strict",
            "--token",
            "fixture-token",
            "--quiet",
        ]));
        assert_eq!(fs::read(project.0.join("agent.json")).unwrap(), before);
        let archive = fs::read_dir(project.0.join("target/agentpm"))
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .find(|path| path.to_string_lossy().ends_with(".tar.gz"))
            .expect("dry run must produce an archive");
        let mut archive = tar::Archive::new(flate2::read::GzDecoder::new(
            fs::File::open(archive).unwrap(),
        ));
        let mut manifests = Vec::new();
        for entry in archive.entries().unwrap() {
            let mut entry = entry.unwrap();
            if entry.path().unwrap().as_ref() == std::path::Path::new("agent.json") {
                manifests.push(serde_json::from_reader::<_, Value>(&mut entry).unwrap());
            }
        }
        assert_eq!(
            manifests,
            vec![project.manifest()],
            "{kind} archive manifest"
        );
    }
}

#[test]
fn new_preserves_standard_in_template_agents_and_still_blocks_unknown_keys() {
    let project = Project::new();
    project.init("template");
    let mut template = project.manifest();
    template["standard"] = json!({"id":"agentpm", "version":"1.0.0"});
    project.write(&template);
    let agent = Project::new();
    agent.init("agent");
    let mut manifest = agent.manifest();
    manifest["standard"] = json!({"id":"agentpm", "version":"1.0.0"});
    let agents = project.0.join("template/agents");
    fs::create_dir_all(&agents).unwrap();
    let source = agents.join("starter.agent.json");
    fs::write(&source, serde_json::to_vec_pretty(&manifest).unwrap()).unwrap();
    let created = success(project.cli(&[
        "new",
        ".",
        "generated",
        "--var",
        "project_name=generated",
        "--quiet",
    ]));
    assert!(
        !String::from_utf8_lossy(&created.stdout).contains("Local manifests missing `standard`:")
    );
    let generated = project.0.join("generated");
    let written: Value =
        serde_json::from_slice(&fs::read(generated.join("agents/starter.agent.json")).unwrap())
            .unwrap();
    assert_eq!(written, manifest, "generated Agent metadata must survive");
    success(project.cli(&["lint", "generated/agents/starter.agent.json", "--strict"]));
    let root: Value =
        serde_json::from_slice(&fs::read(generated.join("agent.json")).unwrap()).unwrap();
    assert_eq!(root["standard"], json!({"id":"agentpm", "version":"1.0.0"}));
    success(project.cli(&["lint", "generated/agent.json", "--strict"]));
    for path in ["agentpm.workspace.json", ".agentpm/template.json"] {
        let value: Value =
            serde_json::from_slice(&fs::read(generated.join(path)).unwrap()).unwrap();
        assert!(
            value.get("standard").is_none(),
            "new must not invent standard on {path}"
        );
    }
    manifest["standrad"] = json!({});
    fs::write(&source, serde_json::to_vec_pretty(&manifest).unwrap()).unwrap();
    let rejected = project.cli(&[
        "new",
        ".",
        "rejected",
        "--var",
        "project_name=rejected",
        "--quiet",
    ]);
    assert_eq!(rejected.status.code(), Some(1));
    let error = String::from_utf8_lossy(&rejected.stderr);
    assert!(
        error.contains("generated manifest agents/starter.agent.json"),
        "{error}"
    );
    assert!(error.contains("standrad"), "{error}");
    assert!(
        !project.0.join("rejected").exists(),
        "failed new must clean its output"
    );
}

#[test]
fn new_declares_root_and_guides_migration_without_rewriting_template_agents() {
    let project = Project::new();
    project.init("template");
    let agent = Project::new();
    agent.init("agent");
    let mut manifest = agent.manifest();
    manifest.as_object_mut().unwrap().remove("standard");
    manifest["description"] = json!("An Agent for {{ project_name }}.");
    let agents = project.0.join("template/agents");
    fs::create_dir_all(&agents).unwrap();
    let source = agents.join("legacy.agent.json");
    let before = serde_json::to_string_pretty(&manifest).unwrap();
    fs::write(&source, &before).unwrap();
    manifest["name"] = json!("secondary");
    let second_before = serde_json::to_string_pretty(&manifest).unwrap();
    fs::write(agents.join("secondary.agent.json"), &second_before).unwrap();
    let output = success(project.cli(&[
        "new",
        ".",
        "generated",
        "--var",
        "project_name=generated",
        "--quiet",
    ]));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("run `agentpm lint`"), "{stdout}");
    assert_eq!(stdout.matches("Next:").count(), 1, "{stdout}");
    assert_eq!(
        stdout
            .matches("Local manifests missing `standard`:")
            .count(),
        1,
        "{stdout}"
    );
    assert_eq!(
        stdout
            .matches(r#""standard":{"id":"agentpm","version":"1.0.0"}"#)
            .count(),
        1,
        "{stdout}"
    );
    assert!(
        stdout.contains("agentpm lint agents/legacy.agent.json"),
        "{stdout}"
    );
    assert!(
        stdout.contains("agents/secondary.agent.json: agentpm lint agents/secondary.agent.json"),
        "{stdout}"
    );
    assert_eq!(fs::read_to_string(&source).unwrap(), before);
    assert_eq!(
        fs::read_to_string(agents.join("secondary.agent.json")).unwrap(),
        second_before
    );
    let generated = Project(project.0.join("generated"));
    assert_eq!(
        generated.manifest()["standard"],
        json!({"id":"agentpm", "version":"1.0.0"})
    );
    let lint = success(generated.cli(&["lint", "--strict", "--format", "json"]));
    let reports: Value = serde_json::from_slice(&lint.stdout).unwrap();
    assert_eq!(reports[0]["apds"]["status"], "conformant");
    assert_eq!(reports[0]["apds"]["runner_evaluated"], false);
    let nested = generated.0.join("agents/legacy.agent.json");
    let rendered = before.replace("{{ project_name }}", "generated");
    assert_eq!(fs::read_to_string(&nested).unwrap(), rendered);
    assert_eq!(
        fs::read_to_string(generated.0.join("agents/secondary.agent.json")).unwrap(),
        second_before.replace("{{ project_name }}", "generated")
    );
    let rejected = generated.cli(&["lint", "agents/legacy.agent.json", "--format", "json"]);
    assert_eq!(rejected.status.code(), Some(1));
    let reports: Value = serde_json::from_slice(&rejected.stdout).unwrap();
    assert!(reports[0]["issues"].as_array().unwrap().iter().any(|issue| issue["rule_id"] == "APDS-ID-001" && issue["instance_path"] == "/standard"));
    success(generated.cli(&["lint", "--legacy", "agents/legacy.agent.json", "--strict"]));
    assert_eq!(fs::read_to_string(&nested).unwrap(), rendered);
}

#[test]
fn export_preserves_source_and_declares_standard_in_fresh_skills() {
    let project = Project::new();
    project.init("tool");
    let mut manifest = project.manifest();
    manifest["standard"] = json!({"id":"agentpm", "version":"1.0.0"});
    project.write(&manifest);
    let installed = project.0.join(".agentpm/tools/fixture/starter/0.1.0");
    fs::create_dir_all(&installed).unwrap();
    fs::copy(project.0.join("agent.json"), installed.join("agent.json")).unwrap();
    fs::copy(project.0.join("main.py"), installed.join("main.py")).unwrap();
    fs::write(
        project.0.join("agent.lock"),
        serde_json::to_vec_pretty(&json!({
            "lockfile_version": 2,
            "generated": "2026-11-13T00:00:00Z",
            "packages": {
                "tool:@fixture/starter@0.1.0": {
                    "kind": "tool", "name": "@fixture/starter", "version": "0.1.0",
                    "integrity": "fixture-not-downloaded"
                }
            },
            "roots": {}
        }))
        .unwrap(),
    )
    .unwrap();
    let before = fs::read(installed.join("agent.json")).unwrap();
    success(project.cli(&[
        "export",
        "--skill",
        "@fixture/starter@0.1.0",
        "--manifest",
        "--output",
        "exported",
    ]));
    let skill: Value =
        serde_json::from_slice(&fs::read(project.0.join("exported/agent.json")).unwrap()).unwrap();
    assert_eq!(skill["kind"], "skill");
    assert_eq!(
        skill["standard"],
        json!({"id":"agentpm", "version":"1.0.0"})
    );
    assert_eq!(fs::read(installed.join("agent.json")).unwrap(), before);
    success(project.cli(&["lint", "exported/agent.json"]));
}
