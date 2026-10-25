//! New-format artifact preparation primitive. Legacy publish does not call this;
//! M13B/M16 will connect the release transaction/build command separately.
use super::{MAX_TAR_ENTRIES, ensure_safe_tar_name, write_artifact_atomically};
use agentpm_sdk::release::{ARTIFACT_METADATA_FILE, ArtifactMetadata};
use anyhow::{Result, ensure};
use std::{collections::HashSet, fs::File, path::Path};

#[allow(dead_code)] // Internal release-building primitive; no new publish workflow in M13A.
pub(super) fn write_release_artifact(
    source: &Path,
    destination: &Path,
) -> Result<ArtifactMetadata> {
    ensure!(
        source != destination,
        "release artifact destination must be separate from its source"
    );
    let descriptor = super::payload::describe(source)?;
    let metadata = ArtifactMetadata {
        target: descriptor.target,
        format: descriptor.format,
    };
    let bytes = serde_json::to_vec(&metadata)?;
    write_artifact_atomically(destination, |tar| {
        let mut archive = tar::Archive::new(flate2::read::GzDecoder::new(File::open(source)?));
        let mut names = HashSet::new();
        for entry in archive.entries()? {
            let mut entry = entry?;
            let name = entry.path()?.to_string_lossy().into_owned();
            ensure_safe_tar_name(&name)?;
            ensure!(
                name != ARTIFACT_METADATA_FILE,
                "reserved generated artifact descriptor already exists"
            );
            ensure!(
                names.insert(name.clone()),
                "duplicate artifact member: {name}"
            );
            ensure!(
                names.len() < MAX_TAR_ENTRIES,
                "too many release artifact members"
            );
            let mut header = entry.header().clone();
            tar.append_data(&mut header, name, &mut entry)?;
        }
        ensure!(
            names.contains("agent.json"),
            "release artifact requires root agent.json"
        );
        let mut header = tar::Header::new_gnu();
        header.set_size(bytes.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        tar.append_data(&mut header, ARTIFACT_METADATA_FILE, bytes.as_slice())?;
        Ok(())
    })?;
    Ok(metadata)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn new_format_artifact_is_self_describing_without_changing_legacy_archive() {
        let root = std::env::temp_dir().join(format!(
            "agentpm-release-artifact-{:032x}",
            rand::random::<u128>()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let source = root.join("legacy.tar.gz");
        let destination = root.join("release.tar.gz");
        let manifest =
            serde_json::to_vec(&json!({"kind":"tool","name":"fixture","version":"1.0.0"})).unwrap();
        write_artifact_atomically(&source, |tar| {
            let mut header = tar::Header::new_gnu();
            header.set_size(manifest.len() as u64);
            header.set_mode(0o644);
            header.set_cksum();
            tar.append_data(&mut header, "agent.json", manifest.as_slice())?;
            Ok(())
        })
        .unwrap();
        let before_source = std::fs::read(&source).unwrap();
        let metadata = write_release_artifact(&source, &destination).unwrap();
        assert_eq!(metadata.target, agentpm_sdk::target::Target::Any);
        assert_eq!(std::fs::read(&source).unwrap(), before_source);
        let mut archive = tar::Archive::new(flate2::read::GzDecoder::new(
            File::open(&destination).unwrap(),
        ));
        let names = archive
            .entries()
            .unwrap()
            .map(|e| e.unwrap().path().unwrap().into_owned())
            .collect::<Vec<_>>();
        assert!(names.contains(&"agent.json".into()));
        assert!(names.contains(&ARTIFACT_METADATA_FILE.into()));
        let before = std::fs::read(&destination).unwrap();
        assert!(write_release_artifact(&destination, &source).is_err());
        assert_eq!(std::fs::read(&source).unwrap(), before_source);
        assert_eq!(std::fs::read(&destination).unwrap(), before);
        std::fs::remove_dir_all(root).unwrap();
    }
}
