//! The format inventory is embedded build input, never remote/user policy.
use anyhow::{Context, Result, ensure};
use serde::Deserialize;
use std::{collections::HashSet, sync::OnceLock};

pub const FORMAT_POLICY_JSON: &str = include_str!("../../contracts/tool-payload-formats-v1.json");
const MAX_HEADER_BYTES: usize = 4096;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PayloadFormats {
    #[serde(rename = "type")]
    policy_type: String,
    pub text_groups: Vec<TextGroup>,
    pub text_basenames: Vec<String>,
    pub text_basename_prefixes: Vec<String>,
    pub text_metadata: TextMetadata,
    pub binary_formats: Vec<BinaryFormat>,
    pub installed_state: InstalledState,
    pub collection: CollectionPolicy,
    pub native_filenames: NativeFilenames,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeFilenames {
    pub suffixes: Vec<String>,
    pub basename_contains: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InstalledState {
    pub directory_basenames: Vec<String>,
    pub marker_basenames: Vec<String>,
    pub metadata_directory_suffixes: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CollectionPolicy {
    pub any_basenames: Vec<String>,
    pub file_basenames: Vec<String>,
    pub dotenv_basename: String,
    pub dotenv_prefix: String,
    pub dotenv_exceptions: Vec<String>,
}

#[derive(Debug, Clone, Copy)]
pub enum CollectionRule {
    VersionControl,
    EnvironmentDirectory,
    EnvironmentRoot,
    EnvironmentMarker,
    OsMetadata,
    Dotenv,
}

impl CollectionRule {
    pub fn id(self) -> &'static str {
        match self {
            Self::VersionControl => "version_control",
            Self::EnvironmentDirectory => "environment_directory",
            Self::EnvironmentRoot => "environment_root",
            Self::EnvironmentMarker => "environment_marker",
            Self::OsMetadata => "os_metadata",
            Self::Dotenv => "dotenv",
        }
    }

    pub fn reason(self) -> &'static str {
        match self {
            Self::VersionControl => "version-control metadata",
            Self::EnvironmentDirectory => "Python environment/cache directory",
            Self::EnvironmentRoot => "Python environment root with a configuration marker",
            Self::EnvironmentMarker => "Python environment configuration marker",
            Self::OsMetadata => "OS metadata",
            Self::Dotenv => "incidental dotenv configuration",
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TextGroup {
    pub name: String,
    pub suffixes: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TextMetadata {
    pub file_suffixes: Vec<String>,
    pub directory_suffixes: Vec<String>,
    pub basenames: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BinaryFormat {
    pub name: String,
    pub suffixes: Vec<String>,
    pub min_header_bytes: usize,
    pub signatures: Vec<Signature>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Signature {
    pub offset: usize,
    pub hex: String,
}

impl PayloadFormats {
    pub fn from_json(json: &str) -> Result<Self> {
        let policy: Self =
            serde_json::from_str(json).context("invalid Tool payload format inventory")?;
        ensure!(
            policy.policy_type == "agentpm.tool-payload-formats.v1",
            "unsupported Tool payload format inventory type/version"
        );
        ensure!(
            !policy.text_groups.is_empty() && !policy.binary_formats.is_empty(),
            "empty format inventory"
        );
        for values in [
            &policy.installed_state.directory_basenames,
            &policy.installed_state.marker_basenames,
            &policy.collection.any_basenames,
            &policy.collection.file_basenames,
            &policy.collection.dotenv_exceptions,
        ] {
            let mut seen = HashSet::new();
            ensure!(!values.is_empty(), "empty collection/state name list");
            for value in values {
                ensure!(
                    valid_name(value) && value == &value.to_ascii_lowercase() && seen.insert(value),
                    "invalid or duplicate collection/state basename: {value}"
                );
            }
        }
        let mut roles = HashSet::new();
        for name in policy
            .installed_state
            .directory_basenames
            .iter()
            .chain(&policy.installed_state.marker_basenames)
            .chain(&policy.collection.any_basenames)
            .chain(&policy.collection.file_basenames)
        {
            ensure!(
                roles.insert(name),
                "duplicate/conflicting installed-state or collection role: {name}"
            );
        }
        ensure!(
            !policy
                .collection
                .dotenv_exceptions
                .iter()
                .any(|name| roles.contains(name)),
            "dotenv exception conflicts with another exclusion role"
        );
        let mut metadata_suffixes = HashSet::new();
        validate_suffixes(
            &policy.installed_state.metadata_directory_suffixes,
            &mut metadata_suffixes,
        )?;
        for value in [
            &policy.collection.dotenv_basename,
            &policy.collection.dotenv_prefix,
        ] {
            ensure!(
                valid_name(value) && value == &value.to_ascii_lowercase(),
                "invalid dotenv name/prefix"
            );
        }
        ensure!(
            policy
                .text_basenames
                .iter()
                .any(|name| name.eq_ignore_ascii_case(&policy.collection.dotenv_basename))
                && policy
                    .text_basename_prefixes
                    .contains(&policy.collection.dotenv_prefix),
            "collection dotenv matching disagrees with portable-text inventory"
        );
        ensure!(
            policy
                .collection
                .dotenv_exceptions
                .iter()
                .all(|name| name.starts_with(&policy.collection.dotenv_prefix)),
            "dotenv exception does not match the excluded prefix"
        );
        ensure!(
            !policy
                .collection
                .any_basenames
                .iter()
                .chain(&policy.collection.file_basenames)
                .any(
                    |name| policy.installed_state.directory_basenames.contains(name)
                        || policy.installed_state.marker_basenames.contains(name)
                        || policy
                            .installed_state
                            .metadata_directory_suffixes
                            .iter()
                            .any(|suffix| name.ends_with(suffix))
                ),
            "collection rules conflict with installed-state roles"
        );
        let mut suffixes = HashSet::new();
        let mut names = HashSet::new();
        for group in &policy.text_groups {
            ensure!(
                !group.name.is_empty() && names.insert(group.name.to_ascii_lowercase()),
                "empty or duplicate format group"
            );
            validate_suffixes(&group.suffixes, &mut suffixes)?;
        }
        validate_suffixes(&policy.text_metadata.file_suffixes, &mut suffixes)?;
        let mut directory_suffixes = HashSet::new();
        validate_suffixes(
            &policy.text_metadata.directory_suffixes,
            &mut directory_suffixes,
        )?;
        let mut basenames = HashSet::new();
        for name in policy
            .text_basenames
            .iter()
            .chain(&policy.text_metadata.basenames)
        {
            ensure!(
                valid_name(name) && basenames.insert(name.to_ascii_lowercase()),
                "invalid or duplicate text basename: {name}"
            );
        }
        let mut prefixes = HashSet::new();
        for prefix in &policy.text_basename_prefixes {
            ensure!(
                valid_name(prefix)
                    && prefix == &prefix.to_ascii_lowercase()
                    && prefixes.insert(prefix),
                "invalid or duplicate basename prefix: {prefix}"
            );
        }
        let text_suffixes = suffixes.clone();
        for format in &policy.binary_formats {
            ensure!(
                !format.name.is_empty() && names.insert(format.name.to_ascii_lowercase()),
                "empty or duplicate binary format name"
            );
            validate_suffixes(&format.suffixes, &mut suffixes)?;
            ensure!(
                !format.suffixes.iter().any(|binary| text_suffixes
                    .iter()
                    .any(|text| binary.ends_with(text) || text.ends_with(binary))),
                "overlapping text/binary suffixes: {}",
                format.name
            );
            ensure!(
                (1..=MAX_HEADER_BYTES).contains(&format.min_header_bytes)
                    && !format.signatures.is_empty(),
                "invalid binary header bound/signatures: {}",
                format.name
            );
            let mut checks = HashSet::new();
            let mut required_bytes = std::collections::BTreeMap::new();
            for signature in &format.signatures {
                let bytes = signature.bytes()?;
                ensure!(
                    !bytes.is_empty()
                        && signature
                            .offset
                            .checked_add(bytes.len())
                            .is_some_and(|end| end <= format.min_header_bytes),
                    "signature exceeds binary header bound: {}",
                    format.name
                );
                ensure!(
                    checks.insert((signature.offset, &signature.hex)),
                    "duplicate binary signature: {}",
                    format.name
                );
                for (index, byte) in bytes.into_iter().enumerate() {
                    if let Some(previous) = required_bytes.insert(signature.offset + index, byte) {
                        ensure!(
                            previous == byte,
                            "contradicting binary signatures: {}",
                            format.name
                        );
                    }
                }
            }
            // Text aliases/prefixes may not register a binary format too. An
            // explicit prefix exception such as .env. still cannot bypass a
            // binary suffix on a longer basename such as .env.png.
            ensure!(
                !basenames.iter().any(|name| format.matches_name(name)),
                "text basename conflicts with binary format: {}",
                format.name
            );
        }
        let portable_suffixes = suffixes.clone();
        validate_suffixes(&policy.native_filenames.suffixes, &mut suffixes)?;
        ensure!(
            !policy
                .native_filenames
                .suffixes
                .iter()
                .any(|native| portable_suffixes
                    .iter()
                    .any(|portable| native.ends_with(portable) || portable.ends_with(native))),
            "native suffix conflicts with a portable text/binary suffix"
        );
        ensure!(
            !policy.native_filenames.basename_contains.is_empty(),
            "empty native basename-contains list"
        );
        let mut native_fragments = HashSet::new();
        for fragment in &policy.native_filenames.basename_contains {
            ensure!(
                valid_name(fragment)
                    && fragment == &fragment.to_ascii_lowercase()
                    && native_fragments.insert(fragment),
                "invalid or duplicate native basename fragment: {fragment}"
            );
            ensure!(
                fragment.strip_suffix('.').is_some_and(|suffix| policy
                    .native_filenames
                    .suffixes
                    .iter()
                    .any(|native| suffix == native)),
                "native versioned-name fragment must reference a declared native suffix followed by a dot: {fragment}"
            );
        }
        ensure!(
            !basenames.iter().any(|name| policy.is_native_filename(name)),
            "text basename conflicts with native filename matching"
        );
        for alias in suffixes {
            ensure!(
                !basenames.contains(&alias),
                "basename conflicts with suffix: {alias}"
            );
        }
        Ok(policy)
    }

    pub fn embedded() -> Result<&'static Self> {
        static POLICY: OnceLock<std::result::Result<PayloadFormats, String>> = OnceLock::new();
        POLICY
            .get_or_init(|| {
                Self::from_json(FORMAT_POLICY_JSON).map_err(|error| format!("{error:#}"))
            })
            .as_ref()
            .map_err(|error| anyhow::anyhow!("{error}"))
    }

    /// `name` is the normalized archive path; matching considers only its
    /// basename except the explicitly scoped metadata-directory exception.
    pub fn is_text_candidate(&self, name: &str) -> bool {
        let (parent, basename) = name.rsplit_once('/').unwrap_or(("", name));
        self.text_groups
            .iter()
            .flat_map(|group| &group.suffixes)
            .any(|suffix| basename.ends_with(suffix))
            || self
                .text_basenames
                .iter()
                .any(|candidate| basename.eq_ignore_ascii_case(candidate))
            || self
                .text_basename_prefixes
                .iter()
                .any(|prefix| basename.starts_with(prefix))
            || self
                .text_metadata
                .file_suffixes
                .iter()
                .any(|suffix| basename.ends_with(suffix))
            || (parent.split('/').any(|part| {
                self.text_metadata
                    .directory_suffixes
                    .iter()
                    .any(|suffix| part.ends_with(suffix))
            }) && self
                .text_metadata
                .basenames
                .iter()
                .any(|candidate| basename.eq_ignore_ascii_case(candidate)))
    }

    pub fn binary_for(&self, basename: &str) -> Option<&BinaryFormat> {
        self.binary_formats
            .iter()
            .find(|format| format.matches_name(basename))
    }

    /// Match an archive basename, ignoring parent components separated by `/`.
    pub fn is_native_filename(&self, path: &str) -> bool {
        let basename = path.rsplit('/').next().unwrap_or(path).to_ascii_lowercase();
        self.native_filenames
            .suffixes
            .iter()
            .any(|suffix| basename.ends_with(suffix))
            || self
                .native_filenames
                .basename_contains
                .iter()
                .any(|fragment| basename.contains(fragment))
    }

    pub fn is_installed_state(&self, path: &str) -> bool {
        let (parent, basename) = path.rsplit_once('/').unwrap_or(("", path));
        parent.split('/').any(|part| {
            self.installed_state
                .directory_basenames
                .iter()
                .any(|name| part.eq_ignore_ascii_case(name))
                || self
                    .installed_state
                    .metadata_directory_suffixes
                    .iter()
                    .any(|suffix| part.ends_with(suffix))
        }) || self
            .installed_state
            .marker_basenames
            .iter()
            .any(|name| basename.eq_ignore_ascii_case(name))
    }

    /// Directory and marker identities are shared with classification. No
    /// second collection environment list can drift from this inventory.
    pub fn collection_rule(
        &self,
        name: &str,
        directory: bool,
        symlink: bool,
    ) -> Option<CollectionRule> {
        let name = name.to_ascii_lowercase();
        if self.collection.any_basenames.contains(&name) {
            Some(CollectionRule::VersionControl)
        } else if (directory || symlink) && self.installed_state.directory_basenames.contains(&name)
        {
            Some(CollectionRule::EnvironmentDirectory)
        } else if !directory && self.installed_state.marker_basenames.contains(&name) {
            Some(CollectionRule::EnvironmentMarker)
        } else if !directory && self.collection.file_basenames.contains(&name) {
            Some(CollectionRule::OsMetadata)
        } else if !directory
            && (name == self.collection.dotenv_basename
                || name.starts_with(&self.collection.dotenv_prefix))
            && !self.collection.dotenv_exceptions.contains(&name)
        {
            Some(CollectionRule::Dotenv)
        } else {
            None
        }
    }
}

fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.trim() == name
        && !name.chars().any(char::is_whitespace)
        && !name.contains(['/', '\\', '*'])
        && name != "."
        && name != ".."
}

fn validate_suffixes(values: &[String], seen: &mut HashSet<String>) -> Result<()> {
    ensure!(!values.is_empty(), "empty suffix list");
    for value in values {
        ensure!(
            valid_name(value)
                && value.starts_with('.')
                && value.len() > 1
                && value == &value.to_ascii_lowercase()
                && seen.insert(value.clone()),
            "invalid, duplicate or conflicting suffix: {value}"
        );
    }
    Ok(())
}

impl Signature {
    fn bytes(&self) -> Result<Vec<u8>> {
        ensure!(
            !self.hex.is_empty()
                && self.hex.len().is_multiple_of(2)
                && self
                    .hex
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
            "invalid signature hex"
        );
        (0..self.hex.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&self.hex[i..i + 2], 16).context("invalid signature byte"))
            .collect()
    }
}

impl BinaryFormat {
    fn matches_name(&self, name: &str) -> bool {
        self.suffixes.iter().any(|suffix| name.ends_with(suffix))
    }

    pub fn recognizes(&self, bytes: &[u8]) -> bool {
        bytes.len() >= self.min_header_bytes
            && self.signatures.iter().all(|signature| {
                signature.bytes().is_ok_and(|expected| {
                    signature
                        .offset
                        .checked_add(expected.len())
                        .is_some_and(|end| {
                            bytes.get(signature.offset..end) == Some(expected.as_slice())
                        })
                })
            })
    }
}

#[cfg(test)]
mod tests;
