//! Python Tool selection. Exclusions apply to incidental traversal; explicit
//! selections are resolved first and collected independently of manifest order.
use agentpm_sdk::artifact::{
    PayloadDiagnostic, PayloadDiagnosticCode, PayloadSeverity,
    policy::{CollectionRule, PayloadFormats},
    sort_diagnostics,
};
use anyhow::{Context, Result, bail, ensure};
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    fs,
    path::{Component, Path, PathBuf},
};

pub(super) struct Collection {
    pub files: BTreeMap<String, PathBuf>,
    pub diagnostics: Vec<PayloadDiagnostic>,
}

struct Selection {
    path: PathBuf,
    kind: fs::FileType,
    direct_rule: Option<CollectionRule>,
    effective_rule: Option<CollectionRule>,
}

struct Collector<'a> {
    root: PathBuf,
    policy: &'a PayloadFormats,
    explicit: HashSet<PathBuf>,
    preserved_trees: Vec<PathBuf>,
    visited: HashSet<PathBuf>,
    directory_rules: HashMap<PathBuf, Option<CollectionRule>>,
    skip_author_lock: bool,
    result: Collection,
}

pub(super) fn collect(
    root: &Path,
    files: &[String],
    entrypoint: &str,
    skip_author_lock: bool,
) -> Result<Collection> {
    let root = if root.as_os_str().is_empty() {
        Path::new(".")
    } else {
        root
    };
    let root = fs::canonicalize(root)
        .with_context(|| format!("reading package root {}", root.display()))?;
    let policy = PayloadFormats::embedded()?;
    let mut collector = Collector {
        root,
        policy,
        explicit: HashSet::new(),
        preserved_trees: Vec::new(),
        visited: HashSet::new(),
        directory_rules: HashMap::new(),
        skip_author_lock,
        result: Collection {
            files: BTreeMap::new(),
            diagnostics: Vec::new(),
        },
    };
    let mut selections = BTreeMap::new();
    for request in files
        .iter()
        .map(String::as_str)
        .chain(std::iter::once(entrypoint))
    {
        let relative = normalize_request(request)?;
        collector
            .check_ancestors(&relative)
            .with_context(|| format!("inspecting manifest.files entry `{request}`"))?;
        let path = collector.root.join(relative);
        let kind = match fs::symlink_metadata(&path) {
            Ok(metadata) => metadata.file_type(),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                bail!(
                    "manifest.files entry `{request}` does not exist at {}. Create it or correct the literal file/directory path before publishing (globs are not expanded).",
                    path.display()
                );
            }
            Err(error) => {
                return Err(error).with_context(|| {
                    format!(
                        "inspecting manifest.files entry `{request}` at {}",
                        path.display()
                    )
                });
            }
        };
        let direct_rule = collector.rule(&path, kind)?;
        let effective_rule = if direct_rule.is_some() {
            direct_rule
        } else {
            collector.ancestor_rule(&path)?
        };
        if direct_rule.is_some() && kind.is_dir() {
            collector.preserved_trees.push(path.clone());
        }
        collector.explicit.insert(path.clone());
        selections.insert(
            path.clone(),
            Selection {
                path,
                kind,
                direct_rule,
                effective_rule,
            },
        );
    }
    for selection in selections.values() {
        if let Some(rule) = selection.effective_rule {
            collector.report(
                &selection.path,
                rule,
                true,
                selection.kind.is_dir() && selection.direct_rule.is_some(),
            );
        }
    }
    for selection in selections.values() {
        collector.visit(&selection.path, selection.kind, false)?;
    }
    sort_diagnostics(&mut collector.result.diagnostics);
    Ok(collector.result)
}

fn normalize_request(request: &str) -> Result<PathBuf> {
    let mut relative = PathBuf::new();
    for component in Path::new(request).components() {
        match component {
            Component::Normal(part) => relative.push(part),
            Component::CurDir => {}
            _ => {
                bail!("manifest.files/entrypoint path must stay within the package root: {request}")
            }
        }
    }
    Ok(relative)
}

impl Collector<'_> {
    fn relative(&self, path: &Path) -> String {
        let relative = path
            .strip_prefix(&self.root)
            .expect("collector paths stay within root");
        let name = super::rel_to_tar_name(relative);
        if name.is_empty() { ".".into() } else { name }
    }

    fn check_ancestors(&self, relative: &Path) -> Result<()> {
        let mut path = self.root.clone();
        let parts: Vec<_> = relative.components().collect();
        for part in parts.iter().take(parts.len().saturating_sub(1)) {
            path.push(part.as_os_str());
            let kind = fs::symlink_metadata(&path)
                .with_context(|| format!("inspecting explicit path ancestor {}", path.display()))?
                .file_type();
            ensure!(
                !kind.is_symlink(),
                "explicit path traverses a symlink at {}; list the link itself instead",
                path.display()
            );
        }
        Ok(())
    }

    fn ancestor_rule(&mut self, path: &Path) -> Result<Option<CollectionRule>> {
        for ancestor in path.ancestors().skip(1) {
            if !ancestor.starts_with(&self.root) {
                break;
            }
            let kind = fs::symlink_metadata(ancestor)
                .with_context(|| format!("inspecting explicit ancestor {}", ancestor.display()))?
                .file_type();
            if let Some(rule) = self.rule(ancestor, kind)? {
                return Ok(Some(rule));
            }
        }
        Ok(None)
    }

    fn rule(&mut self, path: &Path, kind: fs::FileType) -> Result<Option<CollectionRule>> {
        // Explicit sibling requests share ancestor probes. Inspect a directory
        // once per collection instead of rescanning it for every requested file.
        if kind.is_dir()
            && let Some(rule) = self.directory_rules.get(path)
        {
            return Ok(*rule);
        }
        let rule = self.inspect_rule(path, kind)?;
        if kind.is_dir() {
            self.directory_rules.insert(path.to_path_buf(), rule);
        }
        Ok(rule)
    }

    fn inspect_rule(&self, path: &Path, kind: fs::FileType) -> Result<Option<CollectionRule>> {
        let name = path.file_name().unwrap_or_default().to_string_lossy();
        if let Some(rule) = self
            .policy
            .collection_rule(&name, kind.is_dir(), kind.is_symlink())
        {
            return Ok(Some(rule));
        }
        if kind.is_dir() {
            for entry in fs::read_dir(path).with_context(|| {
                format!(
                    "inspecting directory for environment markers: {}",
                    path.display()
                )
            })? {
                let entry = entry
                    .with_context(|| format!("reading directory entry in {}", path.display()))?;
                if self
                    .policy
                    .installed_state
                    .marker_basenames
                    .iter()
                    .any(|marker| {
                        entry
                            .file_name()
                            .to_string_lossy()
                            .eq_ignore_ascii_case(marker)
                    })
                    && entry
                        .file_type()
                        .with_context(|| {
                            format!("inspecting environment marker {}", entry.path().display())
                        })?
                        .is_file()
                {
                    return Ok(Some(CollectionRule::EnvironmentRoot));
                }
            }
        }
        Ok(None)
    }

    fn visit(&mut self, path: &Path, kind: fs::FileType, inherited_preserve: bool) -> Result<()> {
        // Keep traversal on a heap stack, not the process call stack: directory
        // depth must not turn an otherwise valid source tree into a stack overflow.
        let mut pending = vec![(path.to_path_buf(), kind, inherited_preserve)];
        while let Some((path, kind, inherited_preserve)) = pending.pop() {
            let name = self.relative(&path);
            // Existing build-output and author-lock exclusions remain independent
            // of the incidental policy and cannot include the archive being built.
            if name == "target/agentpm"
                || name.starts_with("target/agentpm/")
                || name == "agent.json"
                || (self.skip_author_lock
                    && (name == "agent.lock"
                        || name == agentpm_sdk::python::RESOLUTION_METADATA_FILE))
            {
                continue;
            }
            if !self.visited.insert(path.to_path_buf()) {
                continue;
            }
            let preserve = inherited_preserve
                || self
                    .preserved_trees
                    .iter()
                    .any(|root| path.starts_with(root));
            if !preserve
                && !self.explicit.contains(&path)
                && let Some(rule) = self.rule(&path, kind)?
            {
                self.report(&path, rule, false, kind.is_dir());
                continue;
            }
            if kind.is_dir() {
                let mut entries = fs::read_dir(&path)
                    .with_context(|| format!("collecting directory {}", path.display()))?
                    .collect::<std::io::Result<Vec<_>>>()
                    .with_context(|| format!("reading directory {}", path.display()))?;
                entries.sort_by_key(|entry| entry.file_name());
                for entry in entries.into_iter().rev() {
                    let kind = entry
                        .file_type()
                        .with_context(|| format!("inspecting {}", entry.path().display()))?;
                    pending.push((entry.path(), kind, preserve));
                }
            } else {
                self.result.files.insert(name, path.to_path_buf());
                ensure!(
                    self.result.files.len() <= super::MAX_TAR_ENTRIES,
                    "too many files in package (> {})",
                    super::MAX_TAR_ENTRIES
                );
            }
        }
        Ok(())
    }

    fn report(&mut self, path: &Path, rule: CollectionRule, explicit: bool, directory: bool) {
        let action = if explicit {
            if directory {
                "explicit selection preserves this directory and its subtree"
            } else {
                "explicit selection preserves this entry"
            }
        } else if directory {
            "skipped incidental directory traversal; separate explicit selections are still collected"
        } else {
            "omitted incidental entry"
        };
        self.result.diagnostics.push(PayloadDiagnostic {
            code: if explicit { PayloadDiagnosticCode::CollectionExplicitOverride } else { PayloadDiagnosticCode::CollectionOmitted },
            severity: PayloadSeverity::Info,
            path: self.relative(path),
            message: format!("{action}: {}.", rule.reason()),
            help: if explicit { None } else { Some("To include intentionally, list this exact file or directory in manifest.files.".into()) },
            target: None, expected_format: None, rule: Some(rule.id().into()),
        });
    }
}

#[cfg(test)]
mod tests;
