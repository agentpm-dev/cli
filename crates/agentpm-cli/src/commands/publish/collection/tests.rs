use super::*;
use crate::{
    commands::publish::{package_tool_build, tests::tar_entries},
    manifest::ToolManifest,
};
use agentpm_sdk::{artifact::PayloadBuildFailure, target::Target};
use serde_json::json;

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "agentpm-collection-{:032x}",
            rand::random::<u128>()
        ));
        fs::create_dir_all(&root).unwrap();
        let fixture = Self(root);
        fixture.add("tool.py", b"print('{}')\n");
        fixture
    }
    fn add(&self, name: &str, bytes: &[u8]) {
        let path = self.0.join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, bytes).unwrap();
    }
    fn package(
        &self,
        files: &[&str],
        entrypoint: &str,
    ) -> Result<(PathBuf, agentpm_sdk::artifact::ToolPayloadDescriptor)> {
        let value = json!({"kind":"tool","name":"collection-check","version":"0.1.0",
            "entrypoint":{"command":"python3","args":[entrypoint]}, "runtime":{"type":"python","version":"3.11"}, "files":files});
        let manifest: ToolManifest = serde_json::from_value(value.clone()).unwrap();
        let path = self.0.join("agent.json");
        fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
        package_tool_build(&manifest, &path, None)
            .map(|(path, descriptor)| (path, descriptor.unwrap()))
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn directory_traversal_prunes_shared_state_and_reports_roots_without_narrowing() {
    let fixture = Fixture::new();
    let policy = PayloadFormats::embedded().unwrap();
    for name in &policy.installed_state.directory_basenames {
        // A malformed portable binary and blocked archive must never be opened.
        fixture.add(&format!("{name}/bad.png"), b"not a PNG");
        fixture.add(&format!("{name}/blocked.whl"), b"not a wheel");
    }
    fixture.add(".git/objects/blocked.whl", b"excluded git object");
    for name in &policy.collection.file_basenames {
        fixture.add(name, b"OS data");
    }
    for name in [
        ".env",
        ".env.local",
        ".env.production",
        ".env.sample",
        ".env.template",
        ".env.dist",
        ".env.example.local",
        ".ENV.UPPER",
    ] {
        fixture.add(name, b"private-looking fixture configuration");
    }
    fixture.add(".env.example", b"NAME=placeholder\n");
    let (archive, descriptor) = fixture.package(&["."], "tool.py").unwrap();
    assert_eq!(descriptor.target, Target::Any);
    let members = tar_entries(&archive);
    assert_eq!(members, vec!["agent.json", "tool.py", ".env.example"]);
    assert!(!descriptor.diagnostics.is_empty());
    assert!(
        descriptor
            .diagnostics
            .iter()
            .all(|d| d.severity == PayloadSeverity::Info && d.target.is_none())
    );
    assert!(
        descriptor
            .diagnostics
            .iter()
            .all(|d| !d.path.ends_with("bad.png") && !d.path.ends_with("blocked.whl"))
    );
    assert!(
        descriptor
            .diagnostics
            .iter()
            .any(|d| d.path == ".git" && d.message.contains("traversal"))
    );
    let worktree = Fixture::new();
    worktree.add(".GIT", b"gitdir: an-unvisited-worktree-location\n");
    worktree.add(".VeNv/bad.png", b"not a PNG");
    worktree.add(".ENV", b"FIXTURE=1\n");
    worktree.add(".DS_STORE", b"OS fixture");
    worktree.add(".ENV.EXAMPLE", b"NAME=placeholder\n");
    let (archive, descriptor) = worktree.package(&["."], "tool.py").unwrap();
    assert_eq!(
        tar_entries(&archive),
        vec!["agent.json", "tool.py", ".ENV.EXAMPLE"]
    );
    assert_eq!(descriptor.target, Target::Any);
    assert!(
        descriptor
            .diagnostics
            .iter()
            .any(|d| d.path == ".GIT" && d.rule.as_deref() == Some("version_control"))
    );
}

#[test]
fn custom_environment_markers_prune_whole_roots_case_insensitively() {
    let fixture = Fixture::new();
    fixture.add(
        "dev-python/PYVENV.CFG",
        b"marker contents are not interpreted",
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(
            fixture.0.join("dev-python/PYVENV.CFG"),
            fs::Permissions::from_mode(0o000),
        )
        .unwrap();
    }
    fixture.add("dev-python/deep/blocked.whl", b"must not be visited");
    fixture.add("dev-python/sibling.py", b"x = 1\n");
    let (archive, descriptor) = fixture.package(&["."], "tool.py").unwrap();
    assert_eq!(tar_entries(&archive), vec!["agent.json", "tool.py"]);
    assert_eq!(descriptor.target, Target::Any);
    assert!(
        descriptor
            .diagnostics
            .iter()
            .any(|d| d.path == "dev-python" && d.rule.as_deref() == Some("environment_root"))
    );
    // The same marker-detected root can be included deliberately, including
    // descendants which ordinary traversal would omit.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(
            fixture.0.join("dev-python/PYVENV.CFG"),
            fs::Permissions::from_mode(0o644),
        )
        .unwrap();
    }
    fs::remove_file(fixture.0.join("dev-python/deep/blocked.whl")).unwrap();
    fixture.add("dev-python/.env", b"EXPLICIT=1\n");
    fixture.add("dev-python/venv/keep.py", b"source\n");
    let (archive, descriptor) = fixture.package(&["dev-python"], "tool.py").unwrap();
    let members = tar_entries(&archive);
    assert!(
        members.contains(&"dev-python/.env".into())
            && members.contains(&"dev-python/venv/keep.py".into())
    );
    assert_eq!(descriptor.target, Target::current().unwrap());
    assert!(descriptor.diagnostics.iter().any(|d| d.code
        == PayloadDiagnosticCode::CollectionExplicitOverride
        && d.rule.as_deref() == Some("environment_root")));
}

#[test]
fn explicit_excluded_root_keeps_full_subtree_and_entry_order_is_irrelevant() {
    let fixture = Fixture::new();
    fixture.add("mypkg/__pycache__/cache.pyc", b"compiled fixture");
    fixture.add("mypkg/__pycache__/.env", b"EXPLICIT=1\n");
    fixture.add("mypkg/__pycache__/venv/inner.py", b"x = 1\n");
    let (plain, plain_descriptor) = fixture.package(&["mypkg"], "tool.py").unwrap();
    assert_eq!(plain_descriptor.target, Target::Any);
    assert_eq!(tar_entries(&plain), vec!["agent.json", "tool.py"]);
    let mut first = None;
    for files in [
        ["mypkg", "mypkg/__pycache__"],
        ["mypkg/__pycache__", "mypkg"],
    ] {
        let (archive, descriptor) = fixture.package(&files, "tool.py").unwrap();
        let members = tar_entries(&archive);
        assert!(members.contains(&"mypkg/__pycache__/.env".into()));
        assert!(members.contains(&"mypkg/__pycache__/venv/inner.py".into()));
        assert_eq!(descriptor.target, Target::current().unwrap());
        assert!(
            descriptor
                .diagnostics
                .iter()
                .any(|d| d.path == "mypkg/__pycache__"
                    && d.code == PayloadDiagnosticCode::CollectionExplicitOverride)
        );
        if let Some((expected_members, expected_diagnostics)) = &first {
            assert_eq!(&members, expected_members);
            assert_eq!(&descriptor.diagnostics, expected_diagnostics);
        } else {
            first = Some((members, descriptor.diagnostics));
        }
    }
}

#[test]
fn separate_explicit_children_and_required_entrypoint_survive_pruned_parents() {
    let fixture = Fixture::new();
    fixture.add(".git/config", b"git fixture");
    fixture.add(".git/unrequested.bin", b"must not ship");
    fixture.add("venv/tool.py", b"print('{}')\n");
    fixture.add("venv/unrequested.py", b"must not ship");
    let mut first = None;
    for files in [[".", ".git/config"], [".git/config", "."]] {
        let (archive, descriptor) = fixture.package(&files, "venv/tool.py").unwrap();
        let members = tar_entries(&archive);
        assert!(members.contains(&".git/config".into()));
        assert!(members.contains(&"venv/tool.py".into()));
        assert!(!members.contains(&".git/unrequested.bin".into()));
        assert!(!members.contains(&"venv/unrequested.py".into()));
        assert!(
            descriptor
                .diagnostics
                .iter()
                .any(|d| d.path == "venv/tool.py"
                    && d.code == PayloadDiagnosticCode::CollectionExplicitOverride)
        );
        assert!(
            descriptor
                .diagnostics
                .iter()
                .any(|d| d.path == "venv" && d.message.contains("separate explicit selections"))
        );
        if let Some((expected_members, expected_diagnostics)) = &first {
            assert_eq!(&members, expected_members);
            assert_eq!(&descriptor.diagnostics, expected_diagnostics);
        } else {
            first = Some((members, descriptor.diagnostics));
        }
    }
}

#[test]
fn explicit_dotenv_and_case_variants_are_kept_with_info() {
    let fixture = Fixture::new();
    for name in [".ENV", ".DS_STORE", ".env.template"] {
        fixture.add(name, b"text fixture\n");
        let (archive, descriptor) = fixture.package(&[name], "tool.py").unwrap();
        assert!(tar_entries(&archive).contains(&name.to_owned()));
        assert!(
            descriptor
                .diagnostics
                .iter()
                .any(|d| d.path == name && d.severity == PayloadSeverity::Info && d.rule.is_some())
        );
    }
    assert!(fixture.package(&["missing/.env"], "tool.py").is_err());
}

#[test]
fn intentional_runtime_metadata_and_development_templates_are_not_stripped() {
    let fixture = Fixture::new();
    for name in [
        "_vendor/native.so",
        "pkg.egg-info/PKG-INFO",
        "pkg.dist-info/METADATA",
        "Makefile",
        "Dockerfile",
        "CODEOWNERS",
        ".gitignore",
        ".editorconfig",
        ".dockerignore",
        "venv",
    ] {
        fixture.add(name, b"payload text\n");
    }
    let (archive, descriptor) = fixture.package(&["."], "tool.py").unwrap();
    let members = tar_entries(&archive);
    for name in [
        "_vendor/native.so",
        "pkg.egg-info/PKG-INFO",
        "pkg.dist-info/METADATA",
        "Makefile",
        "Dockerfile",
        "CODEOWNERS",
        ".gitignore",
        ".editorconfig",
        ".dockerignore",
        "venv",
    ] {
        assert!(members.contains(&name.to_owned()), "{name}");
    }
    let before = members;
    fixture.add(".gitignore", b"*\n!none\n");
    assert_eq!(
        tar_entries(&fixture.package(&["."], "tool.py").unwrap().0),
        before
    );
    assert_eq!(descriptor.target, Target::current().unwrap());
}

#[test]
fn explicit_overrides_do_not_bypass_fatal_rules_and_preserve_old_archive() {
    let fixture = Fixture::new();
    let (archive, _) = fixture.package(&["tool.py"], "tool.py").unwrap();
    let before = fs::read(&archive).unwrap();
    fixture.add("venv/blocked.whl", b"not allowed");
    assert!(
        fixture
            .package(&["venv"], "tool.py")
            .unwrap_err()
            .to_string()
            .contains("embedded")
    );
    assert_eq!(fs::read(&archive).unwrap(), before);
    fs::remove_file(fixture.0.join("venv/blocked.whl")).unwrap();
    fixture.add("venv/bad.png", b"GIF89a wrong format");
    let error = fixture.package(&["venv"], "tool.py").unwrap_err();
    let failure = error.downcast_ref::<PayloadBuildFailure>().unwrap();
    assert!(
        failure
            .diagnostics
            .iter()
            .any(|d| d.code == PayloadDiagnosticCode::CollectionExplicitOverride)
    );
    assert!(
        failure
            .diagnostics
            .iter()
            .any(|d| d.code == PayloadDiagnosticCode::FormatMismatch)
    );
    assert_eq!(fs::read(&archive).unwrap(), before);
}

#[test]
fn a_large_excluded_environment_does_not_consume_the_archive_entry_budget() {
    let fixture = Fixture::new();
    let environment = fixture.0.join("venv");
    fs::create_dir_all(&environment).unwrap();
    for index in 0..=crate::commands::publish::MAX_TAR_ENTRIES {
        fs::write(environment.join(format!("module-{index}.py")), b"").unwrap();
    }
    let (archive, descriptor) = fixture.package(&["."], "tool.py").unwrap();
    assert_eq!(tar_entries(&archive), vec!["agent.json", "tool.py"]);
    assert_eq!(descriptor.target, Target::Any);
    let before = fs::read(&archive).unwrap();
    let error = fixture.package(&["venv"], "tool.py").unwrap_err();
    assert!(error.to_string().contains("too many files"), "{error:#}");
    assert_eq!(fs::read(&archive).unwrap(), before);
}

#[cfg(unix)]
#[test]
fn links_are_not_followed_and_explicit_paths_cannot_traverse_them() {
    use std::os::unix::fs::symlink;
    let fixture = Fixture::new();
    let external = Fixture::new();
    external.add("blocked.whl", b"not part of this project");
    symlink(&external.0, fixture.0.join(".venv")).unwrap();
    symlink(&external.0, fixture.0.join("other-link")).unwrap();
    let (archive, descriptor) = fixture.package(&["."], "tool.py").unwrap();
    assert!(
        !tar_entries(&archive)
            .iter()
            .any(|name| name.starts_with(".venv") || name.contains("blocked.whl"))
    );
    assert!(tar_entries(&archive).contains(&"other-link".into()));
    assert!(
        descriptor
            .diagnostics
            .iter()
            .any(|d| d.path == ".venv" && d.rule.as_deref() == Some("environment_directory"))
    );
    let error = fixture
        .package(&["other-link/tool.py"], "tool.py")
        .unwrap_err();
    assert!(format!("{error:#}").contains("symlink"));
}

#[cfg(unix)]
#[test]
fn inspection_errors_name_the_path_and_never_silently_bypass_checks() {
    use std::os::unix::fs::PermissionsExt;
    let fixture = Fixture::new();
    fixture.add("private/source.py", b"source\n");
    let directory = fixture.0.join("private");
    fs::set_permissions(&directory, fs::Permissions::from_mode(0o000)).unwrap();
    let result = fixture.package(&["."], "tool.py");
    fs::set_permissions(&directory, fs::Permissions::from_mode(0o755)).unwrap();
    // Root can inspect mode-000 directories; native CI uses ordinary users.
    if unsafe { libc::geteuid() } != 0 {
        let message = format!("{:#}", result.unwrap_err());
        assert!(
            message.contains("private") && message.contains("inspecting directory"),
            "{message}"
        );
    }
}
