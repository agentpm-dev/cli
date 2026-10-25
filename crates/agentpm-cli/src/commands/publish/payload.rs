//! Conservative classification of the *completed archive*, not the workspace
//! or the resolver's distribution set. No dependency wheels are inspected here.

use agentpm_sdk::{
    artifact::{
        ArtifactFormat, PayloadBuildFailure, PayloadDiagnostic, PayloadDiagnosticCode,
        PayloadSeverity, ToolPayloadDescriptor, policy::PayloadFormats,
    },
    target::Target,
};
use anyhow::{Context, Result};
use std::{fs::File, io::Read, path::Path};

pub(super) fn describe(path: &Path) -> Result<ToolPayloadDescriptor> {
    let policy = PayloadFormats::embedded()?;
    let header_bytes = policy
        .binary_formats
        .iter()
        .map(|format| format.min_header_bytes)
        .max()
        .unwrap_or(8)
        .max(8);
    let mut diagnostics = Vec::new();
    let mut archive = tar::Archive::new(flate2::read::GzDecoder::new(File::open(path)?));
    for entry in archive.entries().context("reading Tool payload entries")? {
        let mut entry = entry.context("reading Tool payload entry")?;
        let display_path = entry.path()?.to_string_lossy().into_owned();
        let name = display_path.to_ascii_lowercase();
        let basename = name.rsplit('/').next().unwrap_or(&name);
        let reason = if !entry.header().entry_type().is_file() {
            // Never follow archive links to inspect external files.
            Some((
                PayloadDiagnosticCode::UnsupportedMemberType,
                "unsupported archive member type; compatibility is unknown",
            ))
        } else {
            let mode = entry.header().mode()?;
            let mut prefix = Vec::new();
            entry
                .by_ref()
                .take(header_bytes as u64)
                .read_to_end(&mut prefix)?;
            let binary = policy.binary_for(basename);
            // Always inspect portable binary candidates, even after an earlier
            // member narrowed the target or when this path is installed state.
            if let Some(format) = binary
                && !format.recognizes(&prefix)
            {
                diagnostics.push(PayloadDiagnostic {
                    code: PayloadDiagnosticCode::FormatMismatch,
                    severity: PayloadSeverity::Error,
                    path: display_path,
                    message: format!("does not match the required {} signature; packaging failed.", format.name),
                    help: Some(format!("Rename the file to match its actual format, or replace it with valid {} data. If renamed, update manifest.files and any code/template references, then publish again. Renaming to an unrecognized format can still make the payload target-specific.", format.name)),
                    target: None,
                    expected_format: Some(format.name.clone()),
                    rule: None,
                });
                continue;
            }
            if policy.is_installed_state(&name) {
                Some((
                    PayloadDiagnosticCode::InstalledState,
                    "contains installed Python environment state",
                ))
            } else if policy.is_native_filename(basename) || native_signature(&prefix) {
                Some((
                    PayloadDiagnosticCode::NativeContent,
                    "contains native or compiled payload content",
                ))
            } else if mode & 0o111 != 0 && !basename.ends_with(".py") {
                Some((
                    PayloadDiagnosticCode::ExecutableMember,
                    "is an executable member without a portable Python source role",
                ))
            } else if binary.is_some() {
                None
            } else if !policy.is_text_candidate(&name) {
                Some((
                    PayloadDiagnosticCode::UnknownFormat,
                    "is not a recognized portable format",
                ))
            } else if !portable_text(&mut std::io::Cursor::new(prefix).chain(&mut entry))? {
                Some((
                    PayloadDiagnosticCode::UnknownFormat,
                    "does not contain recognized portable UTF-8 text/data",
                ))
            } else {
                None
            }
        };
        if let Some((code, reason)) = reason {
            diagnostics.push(PayloadDiagnostic {
                code,
                severity: PayloadSeverity::Warning,
                path: display_path,
                message: reason.into(),
                help: Some("Review the shipped file and its classification before publishing for other platforms.".into()),
                target: Some(Target::current()?),
                expected_format: None,
                rule: None,
            });
        }
    }
    diagnostics.sort_by(|a, b| a.path.cmp(&b.path).then_with(|| a.code.cmp(&b.code)));
    diagnostics.dedup_by(|a, b| a.path == b.path && a.code == b.code);
    if diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity == PayloadSeverity::Error)
    {
        return Err(PayloadBuildFailure { diagnostics }.into());
    }
    Ok(ToolPayloadDescriptor {
        target: if diagnostics.is_empty() {
            Target::Any
        } else {
            Target::current()?
        },
        format: ArtifactFormat::TarGz,
        diagnostics,
    })
}

fn native_signature(bytes: &[u8]) -> bool {
    // ELF, PE/DOS, Mach-O (both byte orders, 32/64 bit, universal), WebAssembly,
    // and static archives. Check bytes even when a binary is renamed *.py.
    bytes.starts_with(b"\x7fELF")
        || bytes.starts_with(b"MZ")
        || bytes.starts_with(b"!<arch>\n")
        || bytes.starts_with(b"\0asm")
        || [
            [0xfe, 0xed, 0xfa, 0xce],
            [0xce, 0xfa, 0xed, 0xfe],
            [0xfe, 0xed, 0xfa, 0xcf],
            [0xcf, 0xfa, 0xed, 0xfe],
            [0xca, 0xfe, 0xba, 0xbe],
            [0xbe, 0xba, 0xfe, 0xca],
            [0xca, 0xfe, 0xba, 0xbf],
            [0xbf, 0xba, 0xfe, 0xca],
        ]
        .iter()
        .any(|magic| bytes.starts_with(magic))
}

fn portable_text(reader: &mut impl Read) -> Result<bool> {
    // Inspect the entire file with bounded memory. Preserve up to three trailing
    // UTF-8 bytes between reads, so neither Unicode nor binary suffixes depend
    // on where an I/O chunk happens to end.
    let mut buffer = [0u8; 8192];
    let mut pending = 0;
    let mut first = true;
    loop {
        let n = reader.read(&mut buffer[pending..])?;
        if n == 0 {
            if first {
                return Ok(!native_signature(&buffer[..pending])
                    && std::str::from_utf8(&buffer[..pending]).is_ok()
                    && !has_binary_controls(&buffer[..pending]));
            }
            return Ok(pending == 0);
        }
        let length = pending + n;
        // Read::read can return short reads. Wait for enough bytes to inspect
        // every supported magic prefix without assuming one read fills a block.
        if first && length < 8 {
            pending = length;
            continue;
        }
        if first && native_signature(&buffer[..length]) {
            return Ok(false);
        }
        first = false;
        if has_binary_controls(&buffer[..length]) {
            return Ok(false);
        }
        match std::str::from_utf8(&buffer[..length]) {
            Ok(_) => pending = 0,
            Err(error) if error.error_len().is_none() => {
                let valid = error.valid_up_to();
                pending = length - valid;
                buffer.copy_within(valid..length, 0);
            }
            Err(_) => return Ok(false),
        }
    }
}

fn has_binary_controls(bytes: &[u8]) -> bool {
    bytes
        .iter()
        .any(|b| b.is_ascii_control() && !matches!(b, b'\n' | b'\r' | b'\t'))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::publish::{
        package_tool_with_resolution, tests::tar_entries, write_artifact_atomically,
    };
    use crate::manifest::ToolManifest;
    use agentpm_sdk::python::{PythonReleaseMetadata, PythonResolution};
    use serde_json::json;
    use std::{fs, io::Cursor, path::PathBuf};

    struct Fixture(PathBuf);

    impl Fixture {
        fn new() -> Self {
            let root = std::env::temp_dir()
                .join(format!("agentpm-payload-{:032x}", rand::random::<u128>()));
            fs::create_dir_all(&root).unwrap();
            fs::write(root.join("tool.py"), "print('{}')\n").unwrap();
            Self(root)
        }

        fn manifest(&self, files: &[&str], dependencies: bool) -> ToolManifest {
            let mut value = json!({
                "kind":"tool", "name":"payload-check", "version":"0.1.0",
                "entrypoint":{"command":"python3","args":["tool.py"]},
                "runtime":{"type":"python","version":"3.11"}, "files":files
            });
            if dependencies {
                value["runtime"]["dependencies"] = json!(["foo>=1"]);
            }
            fs::write(
                self.0.join("agent.json"),
                serde_json::to_vec_pretty(&value).unwrap(),
            )
            .unwrap();
            serde_json::from_value(value).unwrap()
        }

        fn add(&self, name: &str, content: &[u8]) {
            let path = self.0.join(name);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, content).unwrap();
        }

        fn package(&self, manifest: &ToolManifest) -> PathBuf {
            package_tool_with_resolution(manifest, &self.0.join("agent.json"), None).unwrap()
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn pure_source_and_text_data_are_any_even_with_executable_python() {
        let fixture = Fixture::new();
        fixture.add("data/settings.json", b"{\"hello\":\"world\"}\n");
        fixture.add("README", "Hello, 世界!\n".as_bytes());
        fixture.add("_vendor/pure.py", b"x = 1\n");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(fixture.0.join("tool.py"), fs::Permissions::from_mode(0o755))
                .unwrap();
        }
        let manifest = fixture.manifest(&["tool.py", "data", "README", "_vendor"], false);
        let path = fixture.package(&manifest);
        let descriptor = describe(&path).unwrap();
        assert_eq!(descriptor.target, Target::Any);
        assert_eq!(descriptor.format, ArtifactFormat::TarGz);
        assert!(tar_entries(&path).contains(&"data/settings.json".to_owned()));
        // M12 adds build metadata, not a new member or a new release format.
        assert!(!tar_entries(&path).contains(&"agentpm-artifact.json".to_owned()));
    }

    #[test]
    fn declared_dependencies_and_generated_resolution_do_not_narrow_payload() {
        let fixture = Fixture::new();
        fixture.add("agent.lock", b"author state must not enter payload\n");
        let manifest = fixture.manifest(&["tool.py", "agent.lock"], true);
        let resolution: PythonResolution = serde_json::from_str(include_str!(
            "../../../../agentpm-sdk/contracts/python-resolution-v1.json"
        ))
        .unwrap();
        let metadata = PythonReleaseMetadata::new(resolution).unwrap();
        let path =
            package_tool_with_resolution(&manifest, &fixture.0.join("agent.json"), Some(&metadata))
                .unwrap();
        assert_eq!(describe(&path).unwrap().target, Target::Any);
        let entries = tar_entries(&path);
        assert!(entries.contains(&"agentpm-python.json".to_owned()));
        assert!(!entries.contains(&"agent.lock".to_owned()));
    }

    #[test]
    fn every_native_extension_and_nested_vendor_payload_uses_current_target() {
        let policy = PayloadFormats::embedded().unwrap();
        let mut names: Vec<String> = policy
            .native_filenames
            .suffixes
            .iter()
            .flat_map(|suffix| {
                [
                    format!("native{suffix}"),
                    format!("NATIVE{}", suffix.to_ascii_uppercase()),
                ]
            })
            .collect();
        for fragment in &policy.native_filenames.basename_contains {
            names.push(format!("native{fragment}1.2.3"));
            // Valid portable text with a portable final suffix would become
            // false `any` if the embedded literal-fragment rule were ignored.
            names.push(format!("native{fragment}notes.json"));
        }
        names.extend([
            "_vendor/pkg/lib/native.so".into(),
            "_vendor/pkg.libs/native.dylib".into(),
        ]);
        for name in names {
            let fixture = Fixture::new();
            fixture.add(&name, b"{}\n");
            let manifest = fixture.manifest(&[&name], false);
            let path = fixture.package(&manifest);
            let descriptor = describe(&path).unwrap();
            assert_eq!(descriptor.target, Target::current().unwrap(), "{name}");
            assert!(
                descriptor
                    .diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.path == name
                        && diagnostic.code == PayloadDiagnosticCode::NativeContent),
                "{name}: {:?}",
                descriptor.diagnostics
            );
        }
    }

    #[test]
    fn environment_snapshots_and_installed_metadata_are_not_portable_source() {
        for name in [
            ".venv/pyvenv.cfg",
            "custom-environment/pyvenv.cfg",
            "venv/bin/activate",
            "lib/python3.11/site-packages/example.py",
            "lib/python3/dist-packages/example.py",
            "_vendor/pkg-1.0.dist-info/WHEEL",
        ] {
            let fixture = Fixture::new();
            fixture.add(name, b"# text-only snapshot\n");
            let manifest = fixture.manifest(&[name], false);
            assert_eq!(
                describe(&fixture.package(&manifest)).unwrap().target,
                Target::current().unwrap(),
                "{name}"
            );
        }
    }

    #[test]
    fn native_signatures_cannot_hide_under_source_filenames() {
        for bytes in [
            b"\x7fELFfixture".as_slice(),
            b"MZfixture",
            b"\xfe\xed\xfa\xcefixture",
            b"\xce\xfa\xed\xfefixture",
            b"\xfe\xed\xfa\xcffixture",
            b"\xcf\xfa\xed\xfefixture",
            b"\xca\xfe\xba\xbefixture",
            b"\xbe\xba\xfe\xcafixture",
            b"\xca\xfe\xba\xbffixture",
            b"\xbf\xba\xfe\xcafixture",
            b"\0asmfixture",
            b"!<arch>\nfixture",
        ] {
            let fixture = Fixture::new();
            fixture.add("disguised.py", bytes);
            let manifest = fixture.manifest(&["disguised.py"], false);
            assert_eq!(
                describe(&fixture.package(&manifest)).unwrap().target,
                Target::current().unwrap()
            );
        }
    }

    #[test]
    fn unknown_executables_and_binary_content_default_to_current_target() {
        for (name, bytes) in [
            ("helper", b"unrecognized executable\n".as_slice()),
            ("helper.sh", b"#!/bin/sh\ntrue\n"),
            ("helper.cmd", b"@echo off\r\n"),
            ("unknown.dat", b"opaque data"),
            ("fake.txt", b"text\0binary"),
            ("fake.py", b"\xff\xfe\x01"),
        ] {
            let fixture = Fixture::new();
            fixture.add(name, bytes);
            let manifest = fixture.manifest(&[name], false);
            assert_eq!(
                describe(&fixture.package(&manifest)).unwrap().target,
                Target::current().unwrap(),
                "{name}"
            );
        }
    }

    #[test]
    fn only_final_collected_files_affect_classification() {
        let fixture = Fixture::new();
        fixture.add("unshipped.so", b"native\n");
        fixture.add("target/agentpm/old.exe", b"native\n");
        let manifest = fixture.manifest(&["tool.py"], false);
        assert_eq!(
            describe(&fixture.package(&manifest)).unwrap().target,
            Target::Any
        );
        let manifest = fixture.manifest(&["."], false);
        assert_eq!(
            describe(&fixture.package(&manifest)).unwrap().target,
            Target::current().unwrap()
        );
        fs::remove_file(fixture.0.join("unshipped.so")).unwrap();
        let path = fixture.package(&manifest);
        assert_eq!(describe(&path).unwrap().target, Target::Any);
        assert!(
            !tar_entries(&path)
                .iter()
                .any(|name| name.starts_with("target/agentpm/"))
        );
    }

    #[test]
    fn embedded_wheels_stay_blocked_and_missing_files_fail_before_replacing_archive() {
        let fixture = Fixture::new();
        let manifest = fixture.manifest(&["tool.py"], false);
        let path = fixture.package(&manifest);
        let original = fs::read(&path).unwrap();
        fixture.add("_vendor/pkg.whl", b"wheel fixture\n");
        for name in ["_vendor/pkg.whl", "missing.py"] {
            let manifest = fixture.manifest(&[name], false);
            let error =
                package_tool_with_resolution(&manifest, &fixture.0.join("agent.json"), None)
                    .unwrap_err()
                    .to_string();
            assert!(
                error.contains(if name.ends_with(".whl") {
                    "embedded"
                } else {
                    "does not exist"
                }),
                "{error}"
            );
            assert_eq!(fs::read(&path).unwrap(), original);
        }
    }

    #[test]
    fn classification_streams_past_prefixes_and_handles_utf8_chunk_boundaries() {
        let mut bytes = vec![b'a'; 8191];
        bytes.extend_from_slice("世界\n".as_bytes());
        assert!(portable_text(&mut Cursor::new(&bytes)).unwrap());
        bytes.push(0);
        assert!(!portable_text(&mut Cursor::new(&bytes)).unwrap());
        bytes.pop();
        bytes.push(0xe4); // truncated Unicode is not portable text
        assert!(!portable_text(&mut Cursor::new(&bytes)).unwrap());
        struct ShortReads<'a>(&'a [u8]);
        impl Read for ShortReads<'_> {
            fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
                self.0.read(&mut out[..1])
            }
        }
        for text in ["", "x", "世界", "print('Hello, 世界!')\n"] {
            assert!(portable_text(&mut ShortReads(text.as_bytes())).unwrap());
        }
        assert!(!portable_text(&mut ShortReads(b"MZfixture")).unwrap());
        assert!(!portable_text(&mut ShortReads(b"valid text\0")).unwrap());
    }

    #[test]
    fn links_never_gain_a_portable_claim_and_archive_is_not_rewritten() {
        let fixture = Fixture::new();
        let path = fixture.0.join("legacy.tar.gz");
        write_artifact_atomically(&path, |tar| {
            let mut header = tar::Header::new_gnu();
            header.set_entry_type(tar::EntryType::Symlink);
            header.set_size(0);
            header.set_mode(0o644);
            header.set_link_name("tool.py")?;
            header.set_cksum();
            tar.append_data(&mut header, "link.py", std::io::empty())?;
            Ok(())
        })
        .unwrap();
        let before = fs::read(&path).unwrap();
        assert_eq!(describe(&path).unwrap().target, Target::current().unwrap());
        assert_eq!(fs::read(&path).unwrap(), before);
    }

    #[test]
    fn executable_text_without_a_known_python_source_role_is_target_specific() {
        let fixture = Fixture::new();
        let path = fixture.0.join("executable.tar.gz");
        write_artifact_atomically(&path, |tar| {
            let bytes = b"unknown executable text\n";
            let mut header = tar::Header::new_gnu();
            header.set_size(bytes.len() as u64);
            header.set_mode(0o755);
            header.set_cksum();
            tar.append_data(&mut header, "helper.txt", bytes.as_slice())?;
            Ok(())
        })
        .unwrap();
        assert_eq!(describe(&path).unwrap().target, Target::current().unwrap());
    }

    #[test]
    fn every_shared_text_candidate_can_ship_as_any() {
        let policy = PayloadFormats::embedded().unwrap();
        let mut names: Vec<String> = policy
            .text_groups
            .iter()
            .flat_map(|group| &group.suffixes)
            .map(|suffix| format!("input{suffix}"))
            .collect();
        names.extend(policy.text_basenames.iter().cloned());
        names.extend(
            policy
                .text_basename_prefixes
                .iter()
                .map(|prefix| format!("{prefix}example")),
        );
        for name in names {
            let fixture = Fixture::new();
            fixture.add(&name, b"# portable text\n");
            let manifest = fixture.manifest(&[&name], false);
            let descriptor = describe(&fixture.package(&manifest)).unwrap();
            assert_eq!(descriptor.target, Target::Any, "{name}");
            assert!(
                descriptor
                    .diagnostics
                    .iter()
                    .all(|d| d.severity == PayloadSeverity::Info)
            );
        }
    }

    #[test]
    fn dotenv_prefix_and_egg_metadata_are_scoped_text_candidates() {
        for name in [
            ".env",
            ".env.local",
            ".env.example",
            ".ENV.LOCAL",
            "config/.env.production",
            "MANIFEST.in",
            "requirements.in",
            "input.IN",
            "pkg.egg-info",
            "pkg.egg-info/PKG-INFO",
            "pkg.egg-info/SOURCES.txt",
            "pkg.egg-info/requires.txt",
        ] {
            let fixture = Fixture::new();
            fixture.add(name, b"portable text\n");
            let manifest = fixture.manifest(&[name], false);
            let archive = fixture.package(&manifest);
            assert_eq!(describe(&archive).unwrap().target, Target::Any, "{name}");
            assert!(tar_entries(&archive).contains(&name.to_owned()));
        }
        for name in [
            "config.env.local",
            ".envrc",
            ".environment",
            ".gitignore",
            ".env.local/unknown.data",
            "pkg.egg-info/unknown.data",
            "pkg.egg-info/native.so",
            ".env.so",
        ] {
            let fixture = Fixture::new();
            fixture.add(name, b"text\n");
            let manifest = fixture.manifest(&[name], false);
            let descriptor = describe(&fixture.package(&manifest)).unwrap();
            assert_eq!(descriptor.target, Target::current().unwrap(), "{name}");
            assert_eq!(descriptor.diagnostics[0].path, name);
        }
    }

    #[test]
    fn portable_binary_aliases_bypass_text_checks_but_mismatches_fail_atomically() {
        let policy = PayloadFormats::embedded().unwrap();
        for format in &policy.binary_formats {
            let mut header = vec![0; format.min_header_bytes];
            for signature in &format.signatures {
                let bytes = hex::decode(&signature.hex).unwrap();
                header[signature.offset..signature.offset + bytes.len()].copy_from_slice(&bytes);
            }
            for suffix in &format.suffixes {
                let fixture = Fixture::new();
                let name = format!("asset{}", suffix.to_ascii_uppercase());
                fixture.add(&name, &header);
                let manifest = fixture.manifest(&[&name], false);
                let archive = fixture.package(&manifest);
                assert_eq!(describe(&archive).unwrap().target, Target::Any, "{name}");
                let before = fs::read(&archive).unwrap();
                for invalid in [
                    b"MZ renamed executable".as_slice(),
                    &header[..header.len() - 1],
                ] {
                    fixture.add(&name, invalid);
                    let error = package_tool_with_resolution(
                        &manifest,
                        &fixture.0.join("agent.json"),
                        None,
                    )
                    .unwrap_err();
                    let failure = error.downcast_ref::<PayloadBuildFailure>().unwrap();
                    assert_eq!(failure.diagnostics[0].severity, PayloadSeverity::Error);
                    assert_eq!(
                        failure.diagnostics[0].code,
                        PayloadDiagnosticCode::FormatMismatch
                    );
                    assert_eq!(
                        failure.diagnostics[0].expected_format.as_deref(),
                        Some(format.name.as_str())
                    );
                    assert!(
                        failure.diagnostics[0]
                            .help
                            .as_ref()
                            .unwrap()
                            .contains("Rename")
                    );
                    assert_eq!(fs::read(&archive).unwrap(), before);
                }
            }
        }
    }

    #[test]
    fn narrowing_never_hides_a_mismatch_regardless_of_member_order() {
        for names in [["native.so", "bad.png"], ["bad.png", "native.so"]] {
            let fixture = Fixture::new();
            fixture.add("native.so", b"native fixture");
            fixture.add("bad.png", b"GIF89a wrong format");
            let manifest = fixture.manifest(&names, false);
            let error =
                package_tool_with_resolution(&manifest, &fixture.0.join("agent.json"), None)
                    .unwrap_err();
            let failure = error.downcast_ref::<PayloadBuildFailure>().unwrap();
            assert_eq!(failure.diagnostics.len(), 2);
            assert_eq!(failure.diagnostics[0].path, "bad.png");
            assert_eq!(
                failure.diagnostics[0].code,
                PayloadDiagnosticCode::FormatMismatch
            );
            assert_eq!(failure.diagnostics[1].path, "native.so");
        }
    }

    #[test]
    fn binary_data_keeps_executable_and_installed_state_restrictions() {
        let fixture = Fixture::new();
        let path = fixture.0.join("binary-modes.tar.gz");
        write_artifact_atomically(&path, |tar| {
            for name in ["z.pdf", ".dist-info/asset.pdf"] {
                let bytes = b"%PDF-1.7\n";
                let mut header = tar::Header::new_gnu();
                header.set_size(bytes.len() as u64);
                header.set_mode(0o755);
                header.set_cksum();
                tar.append_data(&mut header, name, bytes.as_slice())?;
            }
            Ok(())
        })
        .unwrap();
        let descriptor = describe(&path).unwrap();
        assert_eq!(descriptor.target, Target::current().unwrap());
        assert_eq!(
            descriptor.diagnostics[0].code,
            PayloadDiagnosticCode::InstalledState
        );
        assert_eq!(
            descriptor.diagnostics[1].code,
            PayloadDiagnosticCode::ExecutableMember
        );
    }

    #[tokio::test]
    async fn legacy_extraction_does_not_reclassify_previously_published_assets() {
        let fixture = Fixture::new();
        let archive = fixture.0.join("legacy.tar.gz");
        let bytes = b"GIF89a historical asset labeled PNG";
        // Model bytes accepted before the publisher's signature agreement rule.
        write_artifact_atomically(&archive, |tar| {
            let mut header = tar::Header::new_gnu();
            header.set_size(bytes.len() as u64);
            header.set_mode(0o644);
            header.set_cksum();
            tar.append_data(&mut header, "icon.png", bytes.as_slice())?;
            Ok(())
        })
        .unwrap();
        let before = fs::read(&archive).unwrap();
        let installed = fixture.0.join("installed");
        crate::io::download::extract_tar_gz(&archive, &installed)
            .await
            .unwrap();
        assert_eq!(fs::read(installed.join("icon.png")).unwrap(), bytes);
        assert_eq!(fs::read(&archive).unwrap(), before);
        assert!(!installed.join("agentpm-artifact.json").exists());
    }
}
