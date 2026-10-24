use super::*;
use agentpm_sdk::python::PythonPackage;
use serde_json::json;

fn sample_interpreter() -> Interpreter {
    serde_json::from_value(json!({
        "executable":"/fixture/python3", "base_executable":"/fixture/python3",
        "prefix":"/fixture", "base_prefix":"/fixture", "abi":"cpython-311",
        "version":"3.11.9", "packages":[], "markers":{
            "implementation_name":"cpython", "implementation_version":"3.11.9",
            "os_name":"posix", "platform_machine":"arm64", "platform_python_implementation":"CPython",
            "platform_release":"fixture", "platform_system":"Darwin", "platform_version":"fixture",
            "python_full_version":"3.11.9", "python_version":"3.11", "sys_platform":"darwin"
        }
    })).unwrap()
}

fn sample_resolution() -> PythonResolution {
    PythonResolution::new(
        ">=3.11".into(),
        vec!["foo>=1".into()],
        vec![
            PythonPackage {
                name: "foo".into(),
                version: "1.0.0".into(),
                hashes: vec![format!("sha256:{}", "a".repeat(64))],
                marker: None,
            },
            PythonPackage {
                name: "colorama".into(),
                version: "0.4.6".into(),
                hashes: vec![format!("sha256:{}", "b".repeat(64))],
                marker: Some("sys_platform == 'win32'".into()),
            },
        ],
    )
    .unwrap()
}

fn sample_identity() -> Fingerprint {
    Fingerprint {
        tool: "@fixture/tool".into(),
        version: "1.0.0".into(),
        target: "aarch64-apple-darwin".into(),
        resolution_digest: sample_resolution().digest().unwrap(),
        uv_version: UV_VERSION.into(),
        interpreter: sample_interpreter().identity().unwrap(),
    }
}

#[test]
fn replay_contains_only_exact_applicable_recorded_packages_and_hashes() {
    let text = requirements(&sample_resolution(), &sample_interpreter()).unwrap();
    assert_eq!(
        text,
        format!("foo==1.0.0 --hash=sha256:{}\n", "a".repeat(64))
    );
    assert_eq!(
        expected_packages(&sample_resolution(), &sample_interpreter()).unwrap(),
        vec![("foo".into(), "1.0.0".into())]
    );
}

#[test]
fn environment_identity_changes_with_tool_version_target_python_and_resolution() {
    let identity = sample_identity();
    let original = directory(Path::new("/fixture"), &identity).unwrap();
    let mut values = Vec::new();
    let mut version = identity.clone();
    version.version = "2.0.0".into();
    values.push(version);
    let mut target = identity.clone();
    target.target = "x86_64-unknown-linux-gnu".into();
    values.push(target);
    let mut python = identity.clone();
    python.interpreter.python_version = "3.12".into();
    values.push(python);
    let mut abi = identity.clone();
    abi.interpreter.abi.push_str("-changed");
    values.push(abi);
    let mut implementation = identity.clone();
    implementation.interpreter.implementation = "pypy".into();
    values.push(implementation);
    let mut executable = identity.clone();
    executable.interpreter.executable = "/other/python3".into();
    values.push(executable);
    let mut resolution = identity.clone();
    resolution.resolution_digest = "c".repeat(64);
    values.push(resolution);
    for value in values {
        assert_ne!(directory(Path::new("/fixture"), &value).unwrap(), original);
    }
    let mut unsafe_identity = identity.clone();
    unsafe_identity.tool = "@../outside".into();
    assert!(directory(Path::new("/fixture"), &unsafe_identity).is_err());
}

#[test]
fn kernel_and_compatible_python_patch_updates_keep_the_environment_directory() {
    let resolution = sample_resolution();
    let before = sample_interpreter();
    let original = fingerprint("@fixture/tool", "1.0.0", &resolution, &before).unwrap();
    let mut updated = before.clone();
    updated.markers["platform_release"] = json!("26.0.0");
    updated.markers["platform_version"] = json!("new-kernel");
    let kernel = fingerprint("@fixture/tool", "1.0.0", &resolution, &updated).unwrap();
    assert_eq!(kernel, original);
    updated.version = "3.11.10".into();
    updated.markers["python_full_version"] = json!("3.11.10");
    updated.markers["implementation_version"] = json!("3.11.10");
    // Package managers may move the physical installation on a patch update,
    // while the user's selected executable and ABI stay the same.
    updated.base_executable = "/new-cellar/python3.11".into();
    updated.base_prefix = "/new-cellar".into();
    updated.prefix = "/new-cellar".into();
    let patch = fingerprint("@fixture/tool", "1.0.0", &resolution, &updated).unwrap();
    assert_eq!(patch, original);
    assert_eq!(
        directory(Path::new("/fixture"), &patch).unwrap(),
        directory(Path::new("/fixture"), &original).unwrap()
    );
    assert_eq!(
        expected_packages(&resolution, &updated).unwrap(),
        expected_packages(&resolution, &before).unwrap()
    );
}

#[test]
fn live_kernel_and_patch_markers_still_invalidate_a_changed_package_set() {
    for (marker, old, new) in [
        ("platform_release", "25.5.0", "26.0.0"),
        ("platform_version", "old-kernel", "new-kernel"),
        ("python_full_version", "3.11.9", "3.11.10"),
        ("implementation_version", "3.11.9", "3.11.10"),
    ] {
        let mut packages = sample_resolution().packages().to_vec();
        packages.push(PythonPackage {
            name: "conditional".into(),
            version: "1.0.0".into(),
            hashes: vec![format!("sha256:{}", "c".repeat(64))],
            marker: Some(format!("{marker} == '{new}'")),
        });
        let resolution =
            PythonResolution::new(">=3.11".into(), vec!["foo>=1".into()], packages).unwrap();
        let mut before = sample_interpreter();
        before.markers[marker] = json!(old);
        let mut after = before.clone();
        after.markers[marker] = json!(new);
        let original = fingerprint("@fixture/tool", "1.0.0", &resolution, &before).unwrap();
        let updated = fingerprint("@fixture/tool", "1.0.0", &resolution, &after).unwrap();
        assert_eq!(original, updated);
        let previous = expected_packages(&resolution, &before).unwrap();
        let expected = expected_packages(&resolution, &after).unwrap();
        assert!(!previous.iter().any(|(name, _)| name == "conditional"));
        assert!(expected.contains(&("conditional".into(), "1.0.0".into())));
        let workspace = Workspace::new();
        let directory = directory(&workspace.0, &original).unwrap();
        fs::create_dir_all(&directory).unwrap();
        fs::write(
            directory.join("ready.json"),
            serde_json::to_vec(&ReadyState {
                version: 1,
                fingerprint: original,
                packages: previous,
            })
            .unwrap(),
        )
        .unwrap();
        let error = ready(&directory, &updated, &after, &expected).unwrap_err();
        assert!(
            error.to_string().contains("fingerprint changed"),
            "{error:#}"
        );
    }
}

#[test]
fn install_failure_categories_are_distinct_and_name_the_target() {
    for (text, expected) in [
        ("Hash mismatch for foo==1.0.0", "digest mismatch"),
        (
            "No wheels are available for foo==1.0.0",
            "No compatible Python wheel",
        ),
        (
            "foo==1.0.0 was not found in the package registry",
            "unavailable from the index",
        ),
        (
            "HTTP status client error (404 Not Found) fetching foo==1.0.0",
            "unavailable from the index",
        ),
        (
            "HTTP status client error (410 Gone) fetching foo==1.0.0",
            "unavailable from the index",
        ),
    ] {
        let message = provisioning_failure(text.as_bytes(), &sample_identity()).to_string();
        assert!(message.contains(expected), "{message}");
        assert!(message.contains("aarch64-apple-darwin"));
        assert!(message.contains("foo==1.0.0"));
    }
}

struct Workspace(PathBuf);
impl Workspace {
    fn new() -> Self {
        let path =
            std::env::temp_dir().join(format!("agentpm-m11c-{:032x}", rand::random::<u128>()));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
    fn engine(&self, wheels: Option<&Path>) -> Result<ProvisionEngine> {
        let source = std::env::var_os("AGENTPM_TEST_UV")
            .context("set AGENTPM_TEST_UV to the verified pinned uv binary")?;
        let cache = self.0.join("uv-cache");
        let binary = cache
            .join("runtimes/uv")
            .join(UV_VERSION)
            .join(Target::current_triple())
            .join(if cfg!(windows) { "uv.exe" } else { "uv" });
        if !binary.exists() {
            fs::create_dir_all(binary.parent().unwrap())?;
            fs::copy(source, &binary)?;
        }
        let uv =
            ManagedUv::acquire_at("http://127.0.0.1:1", &cache, Target::current_triple(), None)?;
        let index_arguments = wheels
            .map(|path| {
                vec![
                    "--no-index".into(),
                    "--find-links".into(),
                    path.to_string_lossy().into_owned(),
                    "--offline".into(),
                ]
            })
            .unwrap_or_default();
        Ok(ProvisionEngine {
            uv,
            cache: self.0.join("wheel-cache"),
            index_arguments,
        })
    }
}
impl Drop for Workspace {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn python() -> String {
    for candidate in std::env::var("AGENTPM_TEST_PYTHON").ok().into_iter().chain(
        [
            "python3.13",
            "python3.12",
            "python3.11",
            "python3",
            "python",
        ]
        .map(str::to_owned),
    ) {
        if let Ok(output) = Command::new(&candidate)
            .args([
                "-I",
                "-c",
                "import sys; assert sys.version_info >= (3,11); print(sys.executable)",
            ])
            .output()
            && output.status.success()
        {
            return String::from_utf8(output.stdout).unwrap().trim().to_owned();
        }
    }
    panic!("Python >=3.11 is required for the native environment tests");
}

fn manifest(python: &str, requirements: Vec<String>) -> RunnerManifest {
    let python = Path::new(python).file_name().unwrap().to_string_lossy();
    serde_json::from_value(json!({"kind":"tool","name":"tool","version":"1.0.0",
        "entrypoint":{"command":python,"args":["tool.py"]},
        "runtime":{"type":"python","version":"3.11","dependencies":requirements},
        "inputs":{"type":"object"},"outputs":{"type":"object"}}))
    .unwrap()
}

fn wheel(workspace: &Workspace, name: &str, tag: &str, dependency: &str) -> (PathBuf, String) {
    let wheels = workspace.0.join("wheels");
    fs::create_dir_all(&wheels).unwrap();
    let output = Command::new(python()).args(["-I", "-c", r#"
import base64, hashlib, json, pathlib, sys, zipfile
directory, name, tag, dependency = sys.argv[1:]
module = name.replace('-', '_')
dist = module + '-1.0.0.dist-info'
metadata = 'Metadata-Version: 2.3\nName: ' + name + '\nVersion: 1.0.0\n'
if dependency:
    metadata += 'Requires-Dist: ' + dependency + '\n'
files = {
    module + '/__init__.py': b'VALUE = "installed-from-locked-wheel"\n',
    dist + '/METADATA': metadata.encode(),
    dist + '/WHEEL': ('Wheel-Version: 1.0\nGenerator: AgentPM-test\nRoot-Is-Purelib: true\nTag: '+tag+'\n').encode(),
}
record = ''.join(path + ',sha256=' + base64.urlsafe_b64encode(hashlib.sha256(data).digest()).decode().rstrip('=') + ',' + str(len(data)) + '\n' for path, data in files.items())
files[dist+'/RECORD'] = (record + dist + '/RECORD,,\n').encode()
path = pathlib.Path(directory) / (module + '-1.0.0-' + tag + '.whl')
with zipfile.ZipFile(path, 'w') as archive:
    for name, data in files.items():
        archive.writestr(name, data)
print(json.dumps([str(path), 'sha256:'+hashlib.sha256(path.read_bytes()).hexdigest()]))
"#]).arg(&wheels).args([name, tag, dependency]).output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let values: Vec<String> = serde_json::from_slice(&output.stdout).unwrap();
    (values[0].clone().into(), values[1].clone())
}

fn locked(name: &str, hash: &str) -> PythonResolution {
    PythonResolution::new(
        ">=3.11".into(),
        vec![format!("{name}==1.0.0")],
        vec![PythonPackage {
            name: name.into(),
            version: "1.0.0".into(),
            hashes: vec![hash.into()],
            marker: None,
        }],
    )
    .unwrap()
}

fn provision(
    workspace: &Workspace,
    _manifest: &RunnerManifest,
    resolution: &PythonResolution,
    wheels: Option<&Path>,
) -> Result<PathBuf> {
    provision_with(
        &workspace.0,
        "@fixture/tool",
        "1.0.0",
        &python(),
        resolution,
        false,
        || workspace.engine(wheels),
    )
}

fn install_fixture(
    workspace: &Workspace,
    manifest: &RunnerManifest,
    resolution: &PythonResolution,
    script: &str,
) -> ResolvedTool {
    let tool = workspace.0.join(".agentpm/tools/fixture/tool/1.0.0");
    fs::create_dir_all(&tool).unwrap();
    let value = json!({"kind":"tool","name":"tool","version":"1.0.0",
        "entrypoint":{"command":manifest.entrypoint.command,"args":["tool.py"]},
        "runtime":{"type":"python","version":"3.11","dependencies":resolution.requirements()},
        "inputs":{"type":"object"},"outputs":{"type":"object"}});
    fs::write(tool.join("agent.json"), serde_json::to_vec(&value).unwrap()).unwrap();
    fs::write(tool.join("tool.py"), script).unwrap();
    fs::write(
        tool.join("agentpm-python.json"),
        serde_json::to_vec(&PythonReleaseMetadata::new(resolution.clone()).unwrap()).unwrap(),
    )
    .unwrap();
    fs::write(workspace.0.join("agent.lock"), serde_json::to_vec(&json!({"lockfile_version":4,"generated":"2026-10-24T00:00:00Z",
        "packages":{"tool:@fixture/tool@1.0.0":{"kind":"tool","name":"@fixture/tool","version":"1.0.0","integrity":"a".repeat(64),"python_resolution":resolution}},"roots":{}})).unwrap()).unwrap();
    crate::runner::resolve_installed_tool(
        &workspace.0,
        &crate::runner::parse_tool_spec("@fixture/tool@1.0.0").unwrap(),
    )
    .unwrap()
}

fn cli_binary() -> PathBuf {
    let binary = std::env::var_os("AGENTPM_TEST_CLI")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            let test = std::env::current_exe().unwrap();
            test.parent()
                .unwrap()
                .parent()
                .unwrap()
                .join(format!("agentpm{}", std::env::consts::EXE_SUFFIX))
        });
    assert!(
        binary.is_file(),
        "build the CLI before the native tests: {}",
        binary.display()
    );
    binary
}

fn dispatch_through_harness(workspace: &Workspace) -> Value {
    use crate::harness_runtime::tool::AgentPmActionDispatcher;
    use crate::harness_runtime::{
        ActionDispatcher, RuntimeSnapshot, SemanticAction, ToolRuntimeSnapshot,
    };
    let mut runtime = RuntimeSnapshot::empty("m11c-fixture".into());
    runtime.workspace_root = workspace.0.clone();
    runtime.tools.push(ToolRuntimeSnapshot {
        name: "@fixture/tool".into(),
        version: "1.0.0".into(),
        description: "M11C dependency Tool".into(),
        root: Some(workspace.0.join(".agentpm/tools/fixture/tool/1.0.0")),
        input_schema: json!({"type":"object"}),
        state: "available".into(),
        source: "agent_binding".into(),
    });
    let mut dispatcher =
        AgentPmActionDispatcher::with_agentpm_binary(&runtime, cli_binary()).unwrap();
    let result = dispatcher.dispatch(&SemanticAction::AgentPmTool {
        tool: "@fixture/tool".into(),
        arguments: json!({"quantity":2,"unit_price":4.5}),
    });
    assert!(result.ok, "{:?}", result.error);
    result.output
}

#[test]
#[ignore = "requires verified AGENTPM_TEST_UV; run on each native target"]
fn real_uv_replay_run_harness_and_interrupted_repair() {
    let workspace = Workspace::new();
    let (file, hash) = wheel(&workspace, "m11c-dep", "py3-none-any", "");
    let base_resolution = locked("m11c-dep", &hash);
    let (_, windows_hash) = wheel(&workspace, "m11c-windows", "py3-none-any", "");
    let mut packages = base_resolution.packages().to_vec();
    packages.push(PythonPackage {
        name: "m11c-windows".into(),
        version: "1.0.0".into(),
        hashes: vec![windows_hash],
        marker: Some("sys_platform == 'win32'".into()),
    });
    let resolution = PythonResolution::new(
        ">=3.11".into(),
        base_resolution.requirements().to_vec(),
        packages,
    )
    .unwrap();
    let manifest = manifest(&python(), resolution.requirements().to_vec());
    let resolved = install_fixture(
        &workspace,
        &manifest,
        &resolution,
        "import json,sys\nfrom pathlib import Path\nsys.path.insert(0,str(Path(__file__).parent/'_vendor'))\nimport authored_helper\nfrom m11c_dep import VALUE\nprint(json.dumps({'value':VALUE,'vendor':authored_helper.VALUE}))\n",
    );
    let vendor = resolved.tool_dir.join("_vendor");
    fs::create_dir_all(&vendor).unwrap();
    fs::write(vendor.join("authored_helper.py"), "VALUE='author-vendor'\n").unwrap();
    let unfinished = directory(
        &workspace.0,
        &fingerprint(
            "@fixture/tool",
            "1.0.0",
            &resolution,
            &inspect(&manifest.entrypoint.command, false).unwrap(),
        )
        .unwrap(),
    )
    .unwrap()
    .join("venv");
    fs::create_dir_all(&unfinished).unwrap();
    fs::write(
        unfinished.join("interrupted-creation.txt"),
        "process stopped before creating Python\n",
    )
    .unwrap();
    let interpreter = provision(
        &workspace,
        &manifest,
        &resolution,
        Some(file.parent().unwrap()),
    )
    .unwrap();
    let environment = interpreter
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    assert!(environment.join("ready.json").is_file());
    let identity = fingerprint(
        "@fixture/tool",
        "1.0.0",
        &resolution,
        &inspect(&manifest.entrypoint.command, false).unwrap(),
    )
    .unwrap();
    let state_path = directory(&workspace.0, &identity)
        .unwrap()
        .join("ready.json");
    let state = fs::read(&state_path).unwrap();
    let run = crate::runner::run_installed_tool(
        &workspace.0,
        &crate::runner::parse_tool_spec("@fixture/tool").unwrap(),
        &json!({}),
        &RunOptions::default(),
    )
    .unwrap();
    assert_eq!(run.output["value"], "installed-from-locked-wheel");
    assert_eq!(run.output["vendor"], "author-vendor");
    assert_eq!(
        dispatch_through_harness(&workspace)["value"],
        "installed-from-locked-wheel"
    );
    assert!(check_tool_ready(&workspace.0, "@fixture/tool", "1.0.0").is_ok());

    let shadow = workspace.0.join("pythonpath-shadow");
    fs::create_dir_all(&shadow).unwrap();
    fs::write(shadow.join("m11c_dep.py"), "VALUE='ambient-shadow'\n").unwrap();
    // Prove this path really would override the locked module without isolation.
    let output = Command::new(python())
        .env("PYTHONPATH", &shadow)
        .args(["-c", "from m11c_dep import VALUE; print(VALUE)"])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "ambient-shadow"
    );
    let options = RunOptions {
        env_overrides: HashMap::from([
            ("PYTHONPATH".into(), shadow.to_string_lossy().into_owned()),
            ("PYTHONHOME".into(), "/not-a-python-home".into()),
        ]),
        ..RunOptions::default()
    };
    let result = crate::runner::run_installed_tool(
        &workspace.0,
        &crate::runner::parse_tool_spec("@fixture/tool").unwrap(),
        &json!({}),
        &options,
    )
    .unwrap();
    assert_eq!(result.output["value"], "installed-from-locked-wheel");
    assert_eq!(result.output["vendor"], "author-vendor");
    // Child-process env is ambient to the real CLI, without mutating this test
    // process's environment. Exercise normal and Harness's machine run surface.
    for machine in [false, true] {
        let mut command = Command::new(cli_binary());
        command
            .current_dir(&workspace.0)
            .env("AGENTPM_TELEMETRY", "0")
            .env("AGENTPM_PYTHON", python())
            .env("PYTHONPATH", &shadow)
            .args(["run", "@fixture/tool", "--input", "{}"]);
        if machine {
            command.arg("--machine");
        }
        let output = command.output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let value: Value = serde_json::from_slice(&output.stdout).unwrap();
        let result = if machine {
            assert_eq!(value["status"], "success", "{value}");
            &value["output"]
        } else {
            &value
        };
        assert_eq!(result["value"], "installed-from-locked-wheel");
        assert_eq!(result["vendor"], "author-vendor");
    }

    // A second install needs neither the index nor the wheel; the environment and
    // pinned engine already exist. No HTTP is available in this test.
    let original_wheel = fs::read(&file).unwrap();
    fs::remove_file(&file).unwrap();
    provision(
        &workspace,
        &manifest,
        &resolution,
        Some(file.parent().unwrap()),
    )
    .unwrap();
    assert_eq!(fs::read(&state_path).unwrap(), state);
    // An interrupted sync has no completion record. Retry removes an unrecorded
    // package, rather than treating a partial environment as complete.
    fs::remove_file(&state_path).unwrap();
    assert!(for_run(&workspace.0, &resolved, &manifest.entrypoint.command).is_err());
    let info = inspect(&interpreter.to_string_lossy(), true).unwrap();
    let site = Command::new(&interpreter)
        .args([
            "-I",
            "-c",
            "import sysconfig; print(sysconfig.get_path('purelib'))",
        ])
        .output()
        .unwrap();
    let site = PathBuf::from(String::from_utf8(site.stdout).unwrap().trim());
    let extra = site.join("unrecorded-1.0.0.dist-info");
    fs::create_dir_all(&extra).unwrap();
    fs::write(
        extra.join("METADATA"),
        "Metadata-Version: 2.3\nName: unrecorded\nVersion: 1.0.0\n",
    )
    .unwrap();
    fs::write(
        extra.join("RECORD"),
        "unrecorded-1.0.0.dist-info/METADATA,,\nunrecorded-1.0.0.dist-info/RECORD,,\n",
    )
    .unwrap();
    assert!(info.packages.contains(&("m11c-dep".into(), "1.0.0".into())));
    assert_eq!(
        info.packages.iter().any(|(name, _)| name == "m11c-windows"),
        cfg!(windows)
    );
    provision(
        &workspace,
        &manifest,
        &resolution,
        Some(file.parent().unwrap()),
    )
    .unwrap();
    assert!(!extra.exists());
    assert!(for_run(&workspace.0, &resolved, &manifest.entrypoint.command).is_ok());
    // A failed forced reinstall must invalidate an already-ready environment.
    let (_, changed_hash) = wheel(&workspace, "m11c-dep", "py3-none-any", "unrecorded==1.0.0");
    assert_ne!(
        changed_hash, hash,
        "tampered distribution remains a structurally valid wheel"
    );
    let error = provision_with(
        &workspace.0,
        "@fixture/tool",
        "1.0.0",
        &python(),
        &resolution,
        true,
        || workspace.engine(Some(file.parent().unwrap())),
    )
    .unwrap_err();
    assert!(
        format!("{error:#}").contains("digest mismatch"),
        "{error:#}"
    );
    assert!(!state_path.exists());
    assert!(for_run(&workspace.0, &resolved, &manifest.entrypoint.command).is_err());
    fs::write(&file, &original_wheel).unwrap();
    provision(
        &workspace,
        &manifest,
        &resolution,
        Some(file.parent().unwrap()),
    )
    .unwrap();
    // Changing the selected interpreter identity cannot reuse a ready marker.
    let mut changed = identity.clone();
    changed.interpreter.executable.push_str("-changed");
    assert!(
        ready(
            environment,
            &changed,
            &inspect(&manifest.entrypoint.command, false).unwrap(),
            &info.packages
        )
        .is_err()
    );

    let engine = workspace.engine(None).unwrap();
    let other_base = workspace.0.join("other-python");
    let output = engine
        .uv
        .command()
        .unwrap()
        .args([
            "--no-config",
            "venv",
            "--no-python-downloads",
            "--cache-dir",
        ])
        .arg(&engine.cache)
        .arg("--python")
        .arg(python())
        .arg(&other_base)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let other_python = other_base.join(if cfg!(windows) {
        "Scripts/python.exe"
    } else {
        "bin/python"
    });
    let mut options = RunOptions::default();
    options.env_overrides.insert(
        "AGENTPM_PYTHON".into(),
        other_python.to_string_lossy().into_owned(),
    );
    let spec = crate::runner::parse_tool_spec("@fixture/tool").unwrap();
    let error =
        crate::runner::run_installed_tool(&workspace.0, &spec, &json!({}), &options).unwrap_err();
    assert!(format!("{error:#}").contains(ERR_ENVIRONMENT), "{error:#}");
    // Restore the exact original distribution bytes for the explicit reinstall.
    fs::write(&file, original_wheel).unwrap();
    provision_with(
        &workspace.0,
        "@fixture/tool",
        "1.0.0",
        &other_python.to_string_lossy(),
        &resolution,
        false,
        || workspace.engine(Some(file.parent().unwrap())),
    )
    .unwrap();
    assert!(crate::runner::run_installed_tool(&workspace.0, &spec, &json!({}), &options).is_ok());
    let mut value: Value =
        serde_json::from_slice(&fs::read(&resolved.manifest_path).unwrap()).unwrap();
    value["runtime"]["version"] = json!("99.0");
    fs::write(&resolved.manifest_path, serde_json::to_vec(&value).unwrap()).unwrap();
    let error =
        crate::runner::run_installed_tool(&workspace.0, &spec, &json!({}), &options).unwrap_err();
    assert!(
        format!("{error:#}").contains("runtime minimum version not satisfied"),
        "{error:#}"
    );
}

#[test]
#[ignore = "requires verified AGENTPM_TEST_UV; run on each native target"]
fn real_uv_hash_missing_wheel_and_unrecorded_dependency_fail_closed() {
    let workspace = Workspace::new();
    let (file, hash) = wheel(&workspace, "m11c-dep", "py3-none-any", "");
    let wheels = file.parent().unwrap();
    let manifest = manifest(&python(), vec!["m11c-dep==1.0.0".into()]);
    let bad = locked("m11c-dep", &format!("sha256:{}", "0".repeat(64)));
    let error = provision(&workspace, &manifest, &bad, Some(wheels)).unwrap_err();
    assert!(
        format!("{error:#}").contains("digest mismatch"),
        "{error:#}"
    );
    let identity = fingerprint(
        "@fixture/tool",
        "1.0.0",
        &bad,
        &inspect(&manifest.entrypoint.command, false).unwrap(),
    )
    .unwrap();
    assert!(
        !directory(&workspace.0, &identity)
            .unwrap()
            .join("ready.json")
            .exists()
    );
    fs::remove_file(&file).unwrap();
    let error = provision(
        &workspace,
        &manifest,
        &locked("m11c-dep", &hash),
        Some(wheels),
    )
    .unwrap_err();
    assert!(
        format!("{error:#}").contains("unavailable from the index"),
        "{error:#}"
    );

    let (_, hash) = wheel(&workspace, "m11c-dep", "cp39-cp39-win_amd64", "");
    let error = provision(
        &workspace,
        &manifest,
        &locked("m11c-dep", &hash),
        Some(wheels),
    )
    .unwrap_err();
    assert!(
        format!("{error:#}").contains("No compatible Python wheel"),
        "{error:#}"
    );
    // An sdist with a valid recorded hash still must not run its build code.
    for file in fs::read_dir(wheels).unwrap() {
        fs::remove_file(file.unwrap().path()).unwrap();
    }
    let source = wheels.join("m11c_dep-1.0.0.tar.gz");
    let mut archive = tar::Builder::new(flate2::write::GzEncoder::new(
        fs::File::create(&source).unwrap(),
        flate2::Compression::default(),
    ));
    let script = b"raise RuntimeError('M11C SOURCE BUILD EXECUTED')\n";
    let mut header = tar::Header::new_gnu();
    header.set_size(script.len() as u64);
    header.set_mode(0o644);
    header.set_cksum();
    archive
        .append_data(&mut header, "m11c_dep-1.0.0/setup.py", &script[..])
        .unwrap();
    archive.into_inner().unwrap().finish().unwrap();
    let hash = format!("sha256:{:x}", Sha256::digest(fs::read(&source).unwrap()));
    let error = provision(
        &workspace,
        &manifest,
        &locked("m11c-dep", &hash),
        Some(wheels),
    )
    .unwrap_err();
    assert!(
        format!("{error:#}").contains("No compatible Python wheel"),
        "{error:#}"
    );
    assert!(!format!("{error:#}").contains("M11C SOURCE BUILD EXECUTED"));
    // A dependency's own METADATA requests a package absent from the resolution.
    // Sync must not expand the set; the completeness check must then fail.
    let (_, hash) = wheel(&workspace, "m11c-dep", "py3-none-any", "unrecorded==1.0.0");
    wheel(&workspace, "unrecorded", "py3-none-any", "");
    let resolution = locked("m11c-dep", &hash);
    let error = provision(&workspace, &manifest, &resolution, Some(wheels)).unwrap_err();
    assert!(
        format!("{error:#}").contains("no unrecorded packages were added"),
        "{error:#}"
    );
    let identity = fingerprint(
        "@fixture/tool",
        "1.0.0",
        &resolution,
        &inspect(&manifest.entrypoint.command, false).unwrap(),
    )
    .unwrap();
    let dir = directory(&workspace.0, &identity).unwrap();
    assert!(!dir.join("ready.json").exists());
    assert!(
        inspect(&python_path(&dir).to_string_lossy(), true)
            .unwrap()
            .packages
            .iter()
            .all(|(name, _)| name != "unrecorded")
    );
}

#[test]
#[ignore = "requires verified AGENTPM_TEST_UV; run on each native target"]
fn real_uv_venv_does_not_require_ensurepip() {
    let workspace = Workspace::new();
    let engine = workspace.engine(None).unwrap();
    let base = workspace.0.join("base");
    let output = engine
        .uv
        .command()
        .unwrap()
        .args([
            "--no-config",
            "venv",
            "--no-python-downloads",
            "--cache-dir",
        ])
        .arg(workspace.0.join("base-cache"))
        .arg("--python")
        .arg(python())
        .arg(&base)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let base_python = base.join(if cfg!(windows) {
        "Scripts/python.exe"
    } else {
        "bin/python"
    });
    let site = Command::new(&base_python)
        .args([
            "-I",
            "-c",
            "import sysconfig; print(sysconfig.get_path('purelib'))",
        ])
        .output()
        .unwrap();
    let site = PathBuf::from(String::from_utf8(site.stdout).unwrap().trim());
    assert!(
        site.canonicalize()
            .unwrap()
            .starts_with(workspace.0.canonicalize().unwrap())
    );
    fs::write(site.join("block_ensurepip.pth"), "import sys; sys.meta_path.insert(0, type('BlockEnsurepip', (), {'find_spec': lambda self, name, *args: (_ for _ in ()).throw(ImportError('fixture: ensurepip/venv unavailable')) if name.split('.')[0] in ('ensurepip', 'venv') else None})())\n").unwrap();
    assert!(
        !Command::new(&base_python)
            .args(["-I", "-m", "ensurepip", "--version"])
            .output()
            .unwrap()
            .status
            .success()
    );
    let (file, hash) = wheel(&workspace, "m11c-dep", "py3-none-any", "");
    let resolution = locked("m11c-dep", &hash);
    provision_with(
        &workspace.0,
        "@fixture/tool",
        "1.0.0",
        &base_python.to_string_lossy(),
        &resolution,
        false,
        || workspace.engine(Some(file.parent().unwrap())),
    )
    .unwrap();
}

#[cfg(unix)]
#[test]
#[ignore = "requires verified AGENTPM_TEST_UV and local HTTP fixtures"]
fn real_cli_install_finalizes_only_after_provisioning_and_keeps_frozen_lock() {
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::os::unix::fs::PermissionsExt;
    use std::sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    };
    use std::time::{Duration, Instant};
    for bad_hash in [true, false] {
        let workspace = Workspace::new();
        let (file, hash) = wheel(&workspace, "m11c-dep", "py3-none-any", "");
        let resolution = locked(
            "m11c-dep",
            if bad_hash {
                "sha256:0000000000000000000000000000000000000000000000000000000000000000"
            } else {
                &hash
            },
        );
        let manifest = manifest(&python(), resolution.requirements().to_vec());
        let tool = install_fixture(
            &workspace,
            &manifest,
            &resolution,
            "import json\nfrom m11c_dep import VALUE\nprint(json.dumps({'value':VALUE}))\n",
        );
        let mut archive = tar::Builder::new(flate2::write::GzEncoder::new(
            Vec::new(),
            flate2::Compression::default(),
        ));
        for name in ["agent.json", "agentpm-python.json", "tool.py"] {
            let bytes = fs::read(tool.tool_dir.join(name)).unwrap();
            let mut header = tar::Header::new_gnu();
            header.set_size(bytes.len() as u64);
            header.set_mode(0o644);
            header.set_cksum();
            archive.append_data(&mut header, name, &bytes[..]).unwrap();
        }
        let bytes = archive.into_inner().unwrap().finish().unwrap();
        let integrity = format!("{:x}", Sha256::digest(&bytes));
        fs::remove_dir_all(workspace.0.join(".agentpm")).unwrap();
        fs::remove_file(workspace.0.join("agent.lock")).unwrap();
        fs::write(workspace.0.join("agent.json"),serde_json::to_vec(&json!({"kind":"agent","name":"consumer","version":"1.0.0","tools":["@fixture/tool@1.0.0"]})).unwrap()).unwrap();
        let wrapper = workspace.0.join("uv-wrapper");
        let source = PathBuf::from(std::env::var_os("AGENTPM_TEST_UV").unwrap());
        // Test-only index injection after the normal managed command scrubs UV_*.
        // The actual pinned binary performs venv, hash verification and sync.
        let quote = |path: &Path| format!("'{}'", path.to_string_lossy().replace('\'', "'\\''"));
        fs::write(
            &wrapper,
            format!(
                "#!/bin/sh\nexport UV_NO_INDEX=1\nexport UV_FIND_LINKS={}\nexec {} \"$@\"\n",
                quote(file.parent().unwrap()),
                quote(&source)
            ),
        )
        .unwrap();
        fs::set_permissions(&wrapper, fs::Permissions::from_mode(0o755)).unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let endpoint = url.clone();
        let done = Arc::new(AtomicBool::new(false));
        let stop = done.clone();
        let finalized = Arc::new(AtomicUsize::new(0));
        let count = finalized.clone();
        let worker = std::thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(30);
            while !stop.load(Ordering::SeqCst) && Instant::now() < deadline {
                let (mut stream, _) = match listener.accept() {
                    Ok(pair) => pair,
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(5));
                        continue;
                    }
                    Err(e) => panic!("{e}"),
                };
                // Accepted sockets can inherit O_NONBLOCK on macOS/BSD.
                stream.set_nonblocking(false).unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                let mut request = Vec::new();
                let mut chunk = [0; 4096];
                let end = loop {
                    let n = stream.read(&mut chunk).unwrap();
                    assert!(n > 0);
                    request.extend_from_slice(&chunk[..n]);
                    if let Some(p) = request.windows(4).position(|v| v == b"\r\n\r\n") {
                        break p + 4;
                    }
                };
                let header = String::from_utf8_lossy(&request[..end]).to_string();
                let length = header
                    .lines()
                    .find_map(|line| {
                        line.split_once(':')
                            .filter(|(name, _)| name.eq_ignore_ascii_case("content-length"))
                            .map(|(_, value)| value.trim().parse::<usize>().unwrap())
                    })
                    .unwrap_or(0);
                while request.len() < end + length {
                    let n = stream.read(&mut chunk).unwrap();
                    assert!(n > 0);
                    request.extend_from_slice(&chunk[..n]);
                }
                let body = if header.starts_with("POST /v1/tools/install/resolve ") {
                    serde_json::to_vec(&json!({"items":[{"kind":"tool","name":"@fixture/tool","version":"1.0.0","integrity":integrity,"python_resolution":resolution}]})).unwrap()
                } else if header.starts_with("POST /v1/tools/install/init ") {
                    serde_json::to_vec(&json!({"session_id":"fixture-session","expires_at":"2099-01-01T00:00:00Z","artifacts":[{
                        "kind":"tool","name":"@fixture/tool","version":"1.0.0","integrity":integrity,"presigned_url":format!("{endpoint}/artifact"),"python_resolution":resolution
                    }]})).unwrap()
                } else if header.starts_with("GET /artifact ") {
                    bytes.clone()
                } else {
                    assert!(
                        header.starts_with("POST /v1/tools/install/finalize "),
                        "{header}"
                    );
                    count.fetch_add(1, Ordering::SeqCst);
                    b"{}".to_vec()
                };
                write!(
                    stream,
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                )
                .unwrap();
                stream.write_all(&body).unwrap();
            }
        });
        let test = std::env::current_exe().unwrap();
        let binary = test.parent().unwrap().parent().unwrap().join("agentpm");
        let invoke = |frozen: bool| {
            let mut command = Command::new(&binary);
            command
                .current_dir(&workspace.0)
                .env("AGENTPM_TELEMETRY", "0")
                .env("HOME", workspace.0.join("home"))
                .env("XDG_CONFIG_HOME", workspace.0.join("config"))
                .env("XDG_CACHE_HOME", workspace.0.join("cache"))
                .env("AGENTPM_UV_PATH", &wrapper)
                .env("AGENTPM_PYTHON", python())
                .args(["--base-url", &url, "install", "--quiet"]);
            if frozen {
                command.arg("--frozen");
            }
            command.output().unwrap()
        };
        let output = invoke(false);
        let initial_text = String::from_utf8_lossy(&output.stderr).to_string();
        let lock = workspace.0.join("agent.lock");
        let success = output.status.success();
        let mut frozen_result = None;
        if success && !bad_hash {
            let before = fs::read(&lock).unwrap();
            frozen_result = Some((invoke(true), before));
        }
        done.store(true, Ordering::SeqCst);
        worker.join().unwrap();
        if bad_hash {
            assert!(!success, "must reject the wheel digest");
            assert!(initial_text.contains("digest mismatch"), "{initial_text}");
            assert_eq!(finalized.load(Ordering::SeqCst), 0);
            assert!(!lock.exists());
            assert!(
                !walkdir::WalkDir::new(workspace.0.join(".agentpm/runtimes"))
                    .into_iter()
                    .filter_map(|e| e.ok())
                    .any(|e| e.file_name() == "ready.json")
            );
        } else {
            assert!(success, "{initial_text}");
            let (frozen, before) = frozen_result.unwrap();
            assert!(
                frozen.status.success(),
                "{}",
                String::from_utf8_lossy(&frozen.stderr)
            );
            assert_eq!(fs::read(&lock).unwrap(), before);
            assert_eq!(finalized.load(Ordering::SeqCst), 2);
            let lock: Value = serde_json::from_slice(&before).unwrap();
            assert_eq!(lock["lockfile_version"], 4);
            assert!(
                lock["packages"]["tool:@fixture/tool@1.0.0"]
                    .get("python_resolution")
                    .is_some()
            );
        }
    }
}

#[test]
#[ignore = "requires verified AGENTPM_TEST_UV and PyPI access; run on each native target"]
fn real_pydantic_native_wheel_runs_through_cli_and_harness() {
    let workspace = Workspace::new();
    let resolution: PythonResolution = serde_json::from_str(include_str!(
        "../../../agentpm-sdk/contracts/python-resolution-pydantic-v1.json"
    ))
    .unwrap();
    let manifest = manifest(&python(), resolution.requirements().to_vec());
    let resolved = install_fixture(
        &workspace,
        &manifest,
        &resolution,
        "import json,sys\nfrom pydantic import BaseModel,Field\nclass Order(BaseModel):\n quantity:int=Field(gt=0)\n unit_price:float=Field(gt=0)\norder=Order.model_validate(json.load(sys.stdin))\nprint(json.dumps({'total':order.quantity*order.unit_price,'prefix':sys.prefix}))\n",
    );
    let interpreter = provision(&workspace, &manifest, &resolution, None).unwrap();
    let actual = inspect(&interpreter.to_string_lossy(), true).unwrap();
    assert!(
        actual
            .packages
            .iter()
            .any(|(name, _)| name == "pydantic-core")
    );
    assert_eq!(
        actual.packages.len(),
        resolution
            .packages_for_environment(&actual.markers)
            .unwrap()
            .len()
    );
    let expected_prefix = interpreter
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .canonicalize()
        .unwrap();
    let output = crate::runner::run_installed_tool(
        &workspace.0,
        &crate::runner::parse_tool_spec("@fixture/tool").unwrap(),
        &json!({"quantity":2,"unit_price":4.5}),
        &RunOptions::default(),
    )
    .unwrap();
    assert_eq!(output.output["total"], 9.0);
    assert_eq!(
        Path::new(output.output["prefix"].as_str().unwrap())
            .canonicalize()
            .unwrap(),
        expected_prefix
    );
    let harness = dispatch_through_harness(&workspace);
    assert_eq!(harness["total"], 9.0);
    assert_eq!(
        Path::new(harness["prefix"].as_str().unwrap())
            .canonicalize()
            .unwrap(),
        expected_prefix
    );
    let marker = for_run(&workspace.0, &resolved, &manifest.entrypoint.command)
        .unwrap()
        .unwrap();
    drop(marker);
    // A future lock is rejected even when an exact selector could otherwise bypass
    // lock-based version selection in the existing runner.
    let lock_path = workspace.0.join("agent.lock");
    let mut lock: Value = serde_json::from_slice(&fs::read(&lock_path).unwrap()).unwrap();
    lock["lockfile_version"] = json!(99);
    fs::write(&lock_path, serde_json::to_vec(&lock).unwrap()).unwrap();
    assert!(
        format!(
            "{:#}",
            for_run(&workspace.0, &resolved, &manifest.entrypoint.command).unwrap_err()
        )
        .contains("Unsupported agent.lock version 99")
    );
}
