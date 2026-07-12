use crate::manifest::{
    KnowledgeManifest, LintIssue, load_manifest_value, parse_knowledge_manifest,
    resolve_schema_source, validate_manifest_value, write_manifest_pretty_atomic,
};
use crate::prelude::*;
use crate::semver::types::{
    Lock, LockedPackage, PackageKind, parse_package_spec, resolve_declared_package_from_packages,
    split_package_ref,
};
use anyhow::{Context, anyhow, bail};
use chrono::{SecondsFormat, Utc};
use serde::Deserialize;
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use std::cmp::Ordering;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering as AtomicOrdering};
use std::thread;
use std::time::{Duration, Instant};

#[cfg(not(test))]
const EMBEDDING_ADAPTER_TIMEOUT_MS: u64 = 10_000;
#[cfg(test)]
const EMBEDDING_ADAPTER_TIMEOUT_MS: u64 = 1_000;
const EMBEDDING_ADAPTER_MAX_STDOUT_BYTES: usize = 1024 * 1024;
const EMBEDDING_ADAPTER_MAX_STDERR_BYTES: usize = 64 * 1024;

#[derive(Args, Debug)]
pub struct KnowledgeArgs {
    #[command(subcommand)]
    pub command: KnowledgeCmd,
}

#[derive(Subcommand, Debug)]
pub enum KnowledgeCmd {
    /// Validate and derive Knowledge metadata for publishing and local use
    Build(KnowledgeBuildArgs),

    /// Inspect a local or installed Knowledge package
    Inspect(KnowledgeInspectArgs),

    /// Query a vector-mode Knowledge package
    Query(KnowledgeQueryArgs),
}

#[derive(Args, Debug, Clone)]
pub struct KnowledgeBuildArgs {
    /// Path to the Knowledge manifest to build
    #[arg(long, default_value = "agent.json")]
    pub manifest: PathBuf,
}

#[derive(Args, Debug, Clone)]
pub struct KnowledgeInspectArgs {
    #[arg(value_name = "PATH_OR_PACKAGE")]
    pub target: String,

    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Debug, Clone)]
pub struct KnowledgeQueryArgs {
    #[arg(value_name = "PATH_OR_PACKAGE")]
    pub target: String,

    #[arg(value_name = "QUERY_TEXT")]
    pub query_text: Option<String>,

    #[arg(long)]
    pub top_k: Option<usize>,

    #[arg(long)]
    pub score_threshold: Option<f64>,

    #[arg(long)]
    pub json: bool,

    #[arg(long)]
    pub include_text: bool,

    #[arg(long)]
    pub include_metadata: bool,

    #[arg(long, value_name = "FILE|-")]
    pub vector_json: Option<String>,

    #[arg(long, value_name = "CMD")]
    pub embedding_command: Option<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct BuiltContextDocument {
    pub(crate) bytes: u64,
    pub(crate) sha256: String,
}

#[derive(Debug, Clone)]
pub(crate) struct ContextBuildResult {
    pub(crate) documents: Vec<BuiltContextDocument>,
    pub(crate) document_count: u64,
    pub(crate) total_bytes: u64,
    pub(crate) content_hash: String,
}

#[derive(Debug, Clone)]
struct ChunkRecord {
    id: String,
    source_id: String,
    text: String,
    metadata: Option<Value>,
}

#[derive(Debug, Clone)]
struct SourceRecord {
    id: String,
    object: Map<String, Value>,
}

#[derive(Debug, Clone)]
struct ResolvedKnowledgeTarget {
    manifest_path: PathBuf,
    package_root: PathBuf,
    display_target: String,
}

#[derive(Debug, Clone, Deserialize)]
struct LocalIndexMetadata {
    r#type: String,
    format_version: u64,
    algorithm: String,
    embedding_id: String,
    metric: String,
    normalized: bool,
    source_corpus_hash: String,
    source_chunks_hash: String,
    source_sources_hash: String,
    source_vectors_hash: String,
    chunks_path: String,
    sources_path: String,
    vectors_path: String,
    dimensions: u64,
    chunk_count: u64,
    source_count: u64,
    vector_count: u64,
    built_at: Option<String>,
    agentpm_version: Option<String>,
}

#[derive(Debug, Clone)]
struct ResolvedLocalIndexMetadata {
    declared_index_path: String,
    metadata_path: PathBuf,
    metadata: LocalIndexMetadata,
}

#[derive(Debug, Clone)]
struct ResolvedLocalIndexValidation {
    resolved: ResolvedLocalIndexMetadata,
    mismatches: Vec<String>,
}

#[derive(Debug, Clone)]
struct QueryVectorInput {
    values: Vec<f32>,
    provider: Option<String>,
    model: Option<String>,
    dimensions: Option<u64>,
}

#[derive(Debug, Clone)]
struct QueryRowMatch {
    row: usize,
    score: f64,
}

#[derive(Debug, Clone)]
struct QueryResultRow {
    row: usize,
    score: f64,
    chunk_id: String,
    source_id: String,
    source_title: Option<String>,
    source_uri: Option<String>,
    text: Option<String>,
    chunk_metadata: Option<Value>,
    source_metadata: Option<Value>,
}

#[derive(Debug, Clone, Copy)]
struct QueryExecutionOptions {
    top_k: usize,
    score_threshold: Option<f64>,
    include_text: bool,
    include_metadata: bool,
}

#[derive(Debug, Clone)]
struct InspectView {
    json: Value,
    text: String,
}

#[derive(Debug, Clone)]
struct QueryView {
    json: Value,
    text: String,
}

#[derive(Debug, Clone)]
pub(crate) struct VectorBuildResult {
    pub(crate) chunk_count: u64,
    pub(crate) source_count: u64,
    pub(crate) vector_count: u64,
    pub(crate) dimensions: u64,
    pub(crate) corpus_hash: String,
    pub(crate) chunks_hash: String,
    pub(crate) sources_hash: String,
    pub(crate) vectors_hash: String,
    pub(crate) embedding_id: String,
}

#[derive(Debug, Clone)]
pub(crate) struct LocalIndexInputs<'a> {
    package_root: &'a Path,
    chunks_path: &'a str,
    sources_path: &'a str,
    vectors_path: &'a str,
    embedding_id: &'a str,
    metric: &'a str,
    normalized: bool,
    dimensions: u64,
    chunk_count: u64,
    source_count: u64,
    vector_count: u64,
    corpus_hash: &'a str,
    chunks_hash: &'a str,
    sources_hash: &'a str,
    vectors_hash: &'a str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub(crate) enum KnowledgeBuildMode {
    Check,
    Write,
}

#[derive(Debug, Clone)]
pub(crate) enum KnowledgeBuildSummary {
    Context {
        name: String,
        version: String,
        result: ContextBuildResult,
    },
    Vector {
        name: String,
        version: String,
        result: VectorBuildResult,
    },
}

impl KnowledgeArgs {
    pub async fn run(self) -> Result<()> {
        match self.command {
            KnowledgeCmd::Build(args) => args.run().await,
            KnowledgeCmd::Inspect(args) => args.run().await,
            KnowledgeCmd::Query(args) => args.run().await,
        }
    }
}

impl KnowledgeBuildArgs {
    pub async fn run(self) -> Result<()> {
        let manifest_path = resolve_manifest_path(&self.manifest)?;
        let summary = execute_knowledge_build(&manifest_path, KnowledgeBuildMode::Write)?;
        print_build_summary(&summary);

        Ok(())
    }
}

impl KnowledgeInspectArgs {
    pub async fn run(self) -> Result<()> {
        let cwd = std::env::current_dir().context("reading current directory")?;
        let rendered = inspect_knowledge(&cwd, &self)?;
        if self.json {
            println!("{}", serde_json::to_string_pretty(&rendered.json)?);
        } else {
            println!("{}", rendered.text);
        }
        Ok(())
    }
}

impl KnowledgeQueryArgs {
    pub async fn run(self) -> Result<()> {
        let cwd = std::env::current_dir().context("reading current directory")?;
        let rendered = query_knowledge(&cwd, &self)?;
        if self.json {
            println!("{}", serde_json::to_string_pretty(&rendered.json)?);
        } else {
            println!("{}", rendered.text);
        }
        Ok(())
    }
}

pub(crate) fn execute_knowledge_build(
    manifest_path: &Path,
    mode: KnowledgeBuildMode,
) -> Result<KnowledgeBuildSummary> {
    let (_manifest, summary) = execute_knowledge_build_with_manifest(manifest_path, mode)?;
    Ok(summary)
}

pub(crate) fn execute_knowledge_build_with_manifest(
    manifest_path: &Path,
    mode: KnowledgeBuildMode,
) -> Result<(KnowledgeManifest, KnowledgeBuildSummary)> {
    let package_root = manifest_path
        .parent()
        .ok_or_else(|| anyhow!("manifest path has no parent: {}", manifest_path.display()))?
        .to_path_buf();

    let (mut manifest_value, _) = load_manifest_value(manifest_path)?;
    validate_manifest_or_bail(manifest_path, &mut manifest_value)?;

    let kind = manifest_value
        .get("kind")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("manifest must include kind"))?;
    if kind != "knowledge" {
        bail!(
            "`agentpm knowledge build` requires kind=\"knowledge\" (got kind=\"{}\")",
            kind
        );
    }

    let manifest = parse_knowledge_manifest(&manifest_value)?;
    validate_declared_knowledge_paths(&manifest)?;
    let summary_name = manifest.name.clone();
    let summary_version = manifest.version.clone();

    let summary = match manifest.knowledge.mode.as_str() {
        "context" => {
            let result = build_context_mode(&package_root, &manifest)?;
            if mode == KnowledgeBuildMode::Write {
                let knowledge_obj = manifest_value
                    .get_mut("knowledge")
                    .and_then(Value::as_object_mut)
                    .ok_or_else(|| {
                        anyhow!("knowledge manifest is missing the top-level knowledge object")
                    })?;
                apply_context_build(knowledge_obj, &result)?;
                write_manifest_pretty_atomic(manifest_path, &manifest_value)?;
            }
            KnowledgeBuildSummary::Context {
                name: summary_name.clone(),
                version: summary_version.clone(),
                result,
            }
        }
        "vector" => {
            let result = build_vector_mode(&package_root, &manifest, mode)?;
            if mode == KnowledgeBuildMode::Write {
                let knowledge_obj = manifest_value
                    .get_mut("knowledge")
                    .and_then(Value::as_object_mut)
                    .ok_or_else(|| {
                        anyhow!("knowledge manifest is missing the top-level knowledge object")
                    })?;
                apply_vector_build(knowledge_obj, &result)?;
                write_manifest_pretty_atomic(manifest_path, &manifest_value)?;
            }
            KnowledgeBuildSummary::Vector {
                name: summary_name,
                version: summary_version,
                result,
            }
        }
        other => bail!("unsupported knowledge mode: {}", other),
    };

    Ok((manifest, summary))
}

fn print_build_summary(summary: &KnowledgeBuildSummary) {
    match summary {
        KnowledgeBuildSummary::Context {
            name,
            version,
            result,
        } => {
            println!("Knowledge build complete: {}@{}", name, version);
            println!("Mode: context");
            println!("Documents: {}", result.document_count);
            println!("Total bytes: {}", result.total_bytes);
            println!("Content hash: {}", result.content_hash);
        }
        KnowledgeBuildSummary::Vector {
            name,
            version,
            result,
        } => {
            println!("Knowledge build complete: {}@{}", name, version);
            println!("Mode: vector");
            println!("Chunks: {}", result.chunk_count);
            println!("Sources: {}", result.source_count);
            println!("Vectors: {}", result.vector_count);
            println!("Dimensions: {}", result.dimensions);
            println!("Index: knowledge/indexes/default");
        }
    }
}

fn inspect_knowledge(base_dir: &Path, args: &KnowledgeInspectArgs) -> Result<InspectView> {
    let target = resolve_knowledge_target(base_dir, &args.target)?;
    let (manifest, summary) =
        execute_knowledge_build_with_manifest(&target.manifest_path, KnowledgeBuildMode::Check)?;
    let manifest_mismatches = manifest_summary_mismatches(&manifest, &summary);
    let vector_index = if matches!(summary, KnowledgeBuildSummary::Vector { .. }) {
        Some(load_local_index_validation(
            &target.package_root,
            &manifest,
            summary_vector_result(&summary)?,
        )?)
    } else {
        None
    };

    let json = build_inspect_json(
        &target,
        &manifest,
        &summary,
        &manifest_mismatches,
        vector_index.as_ref(),
    );
    let text = build_inspect_text(
        &target,
        &manifest,
        &summary,
        &manifest_mismatches,
        vector_index.as_ref(),
    );
    Ok(InspectView { json, text })
}

fn query_knowledge(base_dir: &Path, args: &KnowledgeQueryArgs) -> Result<QueryView> {
    let target = resolve_knowledge_target(base_dir, &args.target)?;
    let (manifest, summary) =
        execute_knowledge_build_with_manifest(&target.manifest_path, KnowledgeBuildMode::Check)?;

    if manifest.knowledge.mode != "vector" {
        bail!(
            "Knowledge package `{}` is mode=\"{}\" and has no vector index; it is intended for direct context loading",
            manifest.name,
            manifest.knowledge.mode
        );
    }

    let manifest_mismatches = manifest_summary_mismatches(&manifest, &summary);
    if !manifest_mismatches.is_empty() {
        bail!(
            "Knowledge manifest build metadata is stale for {}:\n- {}\nRun `agentpm knowledge build` to refresh it.",
            target.manifest_path.display(),
            manifest_mismatches.join("\n- ")
        );
    }

    let vector_result = summary_vector_result(&summary)?;
    let local_index = require_fresh_local_index(load_local_index_validation(
        &target.package_root,
        &manifest,
        vector_result,
    )?)?;
    let query_vector = resolve_query_vector(base_dir, args, &manifest)?;
    let top_k = args
        .top_k
        .or_else(|| {
            manifest
                .knowledge
                .retrieval
                .as_ref()
                .and_then(|r| r.default_top_k.map(|value| value as usize))
        })
        .unwrap_or(5);
    if top_k == 0 {
        bail!("--top-k must be greater than 0");
    }
    let score_threshold = args.score_threshold.or_else(|| {
        manifest
            .knowledge
            .retrieval
            .as_ref()
            .and_then(|r| r.default_score_threshold)
    });

    let rows = execute_exact_vector_query(
        &target.package_root,
        &manifest,
        &local_index.metadata,
        &query_vector.values,
        QueryExecutionOptions {
            top_k,
            score_threshold,
            include_text: args.include_text,
            include_metadata: args.include_metadata,
        },
    )?;

    let json = build_query_json(
        &target,
        &manifest,
        &local_index.metadata,
        &rows,
        args.include_text,
        args.include_metadata,
    );
    let text = build_query_text(&target, &manifest, &local_index.metadata, &rows);
    Ok(QueryView { json, text })
}

fn resolve_knowledge_target(base_dir: &Path, target: &str) -> Result<ResolvedKnowledgeTarget> {
    let candidate_path = Path::new(target);
    let candidate_abs = if candidate_path.is_absolute() {
        candidate_path.to_path_buf()
    } else {
        base_dir.join(candidate_path)
    };

    if candidate_abs.exists() {
        let manifest_path = if candidate_abs.is_dir() {
            candidate_abs.join("agent.json")
        } else {
            candidate_abs
        };
        if !manifest_path.exists() {
            bail!(
                "Knowledge target does not contain agent.json: {}",
                manifest_path.display()
            );
        }
        let package_root = manifest_path
            .parent()
            .ok_or_else(|| anyhow!("manifest path has no parent: {}", manifest_path.display()))?
            .to_path_buf();
        return Ok(ResolvedKnowledgeTarget {
            manifest_path,
            package_root,
            display_target: target.to_string(),
        });
    }

    let normalized = target
        .strip_prefix("knowledge:")
        .unwrap_or(target)
        .to_string();
    let requested = parse_package_spec(&normalized)
        .with_context(|| format!("resolving Knowledge target `{}`", target))?;

    let project_root = base_dir;
    let lock = crate::manifest::read_lock_or_default(project_root)?;
    let packages = match &lock {
        Lock::V2(lock) => lock.packages.clone(),
        Lock::V1(_) => BTreeMap::new(),
    };
    let resolved_pkg = if packages.is_empty() {
        None
    } else {
        resolve_declared_package_from_packages(
            &packages,
            &requested.name,
            &requested.range,
            PackageKind::Knowledge,
        )?
    };

    let (owner, name) = split_package_ref(&requested.name)?;
    let version = if let Some(pkg) = resolved_pkg {
        pkg.version
    } else {
        resolve_installed_knowledge_version(project_root, &owner, &name, &requested.range)?
    };
    let manifest_path = project_root
        .join(".agentpm")
        .join("knowledge")
        .join(&owner)
        .join(&name)
        .join(&version)
        .join("agent.json");
    if !manifest_path.exists() {
        bail!(
            "Installed Knowledge package not found for {} at {}",
            requested.name,
            manifest_path.display()
        );
    }
    let package_root = manifest_path
        .parent()
        .ok_or_else(|| anyhow!("manifest path has no parent: {}", manifest_path.display()))?
        .to_path_buf();

    Ok(ResolvedKnowledgeTarget {
        manifest_path,
        package_root,
        display_target: format!("{}@{}", requested.name, version),
    })
}

fn resolve_installed_knowledge_version(
    project_root: &Path,
    owner: &str,
    name: &str,
    range: &str,
) -> Result<String> {
    let base = project_root
        .join(".agentpm")
        .join("knowledge")
        .join(owner)
        .join(name);
    if !base.exists() {
        bail!("No installed Knowledge package found at {}", base.display());
    }

    let mut packages = BTreeMap::new();
    for entry in fs::read_dir(&base).with_context(|| format!("reading {}", base.display()))? {
        let entry = entry?;
        if !entry
            .file_type()
            .with_context(|| format!("reading {}", entry.path().display()))?
            .is_dir()
        {
            continue;
        }
        let version = entry.file_name().to_string_lossy().to_string();
        let key = crate::semver::types::package_key(
            PackageKind::Knowledge,
            &format!("@{owner}/{name}"),
            &version,
        );
        packages.insert(
            key,
            LockedPackage {
                kind: PackageKind::Knowledge,
                name: format!("@{owner}/{name}"),
                version,
                integrity: String::new(),
            },
        );
    }

    let pkg = resolve_declared_package_from_packages(
        &packages,
        &format!("@{owner}/{name}"),
        range,
        PackageKind::Knowledge,
    )?
    .ok_or_else(|| {
        anyhow!(
            "No installed Knowledge package version matched @{} / {} with range {}",
            owner,
            name,
            range
        )
    })?;
    Ok(pkg.version)
}

fn manifest_summary_mismatches(
    manifest: &KnowledgeManifest,
    summary: &KnowledgeBuildSummary,
) -> Vec<String> {
    match summary {
        KnowledgeBuildSummary::Context { result, .. } => {
            let mut mismatches = Vec::new();
            match manifest.knowledge.context.as_ref() {
                Some(context) => {
                    if context.document_count != Some(result.document_count) {
                        mismatches.push("knowledge.context.document_count".to_string());
                    }
                    if context.total_bytes != Some(result.total_bytes) {
                        mismatches.push("knowledge.context.total_bytes".to_string());
                    }
                    if context.content_hash.as_deref() != Some(result.content_hash.as_str()) {
                        mismatches.push("knowledge.context.content_hash".to_string());
                    }
                }
                None => mismatches.push("knowledge.context".to_string()),
            }

            if manifest.knowledge.documents.len() != result.documents.len() {
                mismatches.push("knowledge.documents length".to_string());
            } else {
                for (idx, (doc, built)) in manifest
                    .knowledge
                    .documents
                    .iter()
                    .zip(&result.documents)
                    .enumerate()
                {
                    if doc.bytes != Some(built.bytes) {
                        mismatches.push(format!("knowledge.documents[{idx}].bytes"));
                    }
                    if doc.sha256.as_deref() != Some(built.sha256.as_str()) {
                        mismatches.push(format!("knowledge.documents[{idx}].sha256"));
                    }
                }
            }
            mismatches
        }
        KnowledgeBuildSummary::Vector { result, .. } => {
            let mut mismatches = Vec::new();
            match manifest.knowledge.corpus.as_ref() {
                Some(corpus) => {
                    if corpus.chunk_count != Some(result.chunk_count) {
                        mismatches.push("knowledge.corpus.chunk_count".to_string());
                    }
                    if corpus.source_count != Some(result.source_count) {
                        mismatches.push("knowledge.corpus.source_count".to_string());
                    }
                    if corpus.content_hash.as_deref() != Some(result.corpus_hash.as_str()) {
                        mismatches.push("knowledge.corpus.content_hash".to_string());
                    }
                }
                None => mismatches.push("knowledge.corpus".to_string()),
            }
            match manifest.knowledge.embedding.as_ref() {
                Some(embedding) => {
                    if embedding.vector_count != Some(result.vector_count) {
                        mismatches.push("knowledge.embedding.vector_count".to_string());
                    }
                    if embedding.vectors_hash.as_deref() != Some(result.vectors_hash.as_str()) {
                        mismatches.push("knowledge.embedding.vectors_hash".to_string());
                    }
                    if embedding.dimensions != result.dimensions {
                        mismatches.push("knowledge.embedding.dimensions".to_string());
                    }
                }
                None => mismatches.push("knowledge.embedding".to_string()),
            }
            mismatches
        }
    }
}

fn summary_vector_result(summary: &KnowledgeBuildSummary) -> Result<&VectorBuildResult> {
    match summary {
        KnowledgeBuildSummary::Vector { result, .. } => Ok(result),
        _ => bail!("expected vector-mode Knowledge summary"),
    }
}

fn load_local_index_validation(
    package_root: &Path,
    manifest: &KnowledgeManifest,
    vector_result: &VectorBuildResult,
) -> Result<ResolvedLocalIndexValidation> {
    let index = manifest
        .knowledge
        .indexes
        .iter()
        .find(|index| index.id == "default" && index.r#type == "agentpm-local")
        .ok_or_else(|| anyhow!("vector-mode Knowledge requires a default agentpm-local index"))?;

    let index_dir = resolve_existing_dir(package_root, &index.path)?;
    let metadata_path = index_dir.join("metadata.json");
    if !metadata_path.exists() {
        bail!("index metadata is missing: {}", metadata_path.display());
    }
    let metadata_text = fs::read_to_string(&metadata_path)
        .with_context(|| format!("reading {}", metadata_path.display()))?;
    let metadata: LocalIndexMetadata = serde_json::from_str(&metadata_text)
        .with_context(|| format!("parsing {}", metadata_path.display()))?;

    let mut mismatches = Vec::new();
    if metadata.r#type != "agentpm-local" {
        mismatches.push("type".to_string());
    }
    if metadata.format_version != 1 {
        mismatches.push("format_version".to_string());
    }
    if metadata.algorithm != "exact" {
        mismatches.push("algorithm".to_string());
    }
    if metadata.embedding_id != vector_result.embedding_id {
        mismatches.push("embedding_id".to_string());
    }
    let embedding = manifest
        .knowledge
        .embedding
        .as_ref()
        .ok_or_else(|| anyhow!("vector-mode Knowledge requires knowledge.embedding"))?;
    if metadata.metric != embedding.metric {
        mismatches.push("metric".to_string());
    }
    if metadata.normalized != embedding.normalized {
        mismatches.push("normalized".to_string());
    }
    if metadata.dimensions != embedding.dimensions {
        mismatches.push("dimensions".to_string());
    }
    if metadata.vector_count != vector_result.vector_count {
        mismatches.push("vector_count".to_string());
    }
    if metadata.chunk_count != vector_result.chunk_count {
        mismatches.push("chunk_count".to_string());
    }
    if metadata.source_count != vector_result.source_count {
        mismatches.push("source_count".to_string());
    }
    let corpus = manifest
        .knowledge
        .corpus
        .as_ref()
        .ok_or_else(|| anyhow!("vector-mode Knowledge requires knowledge.corpus"))?;
    if metadata.chunks_path != corpus.chunks_path.clone().unwrap_or_default() {
        mismatches.push("chunks_path".to_string());
    }
    if metadata.sources_path != corpus.sources_path.clone().unwrap_or_default() {
        mismatches.push("sources_path".to_string());
    }
    if metadata.vectors_path != embedding.vectors_path {
        mismatches.push("vectors_path".to_string());
    }
    if metadata.source_corpus_hash != vector_result.corpus_hash {
        mismatches.push("source_corpus_hash".to_string());
    }
    if metadata.source_chunks_hash != vector_result.chunks_hash {
        mismatches.push("source_chunks_hash".to_string());
    }
    if metadata.source_sources_hash != vector_result.sources_hash {
        mismatches.push("source_sources_hash".to_string());
    }
    if metadata.source_vectors_hash != vector_result.vectors_hash {
        mismatches.push("source_vectors_hash".to_string());
    }

    Ok(ResolvedLocalIndexValidation {
        resolved: ResolvedLocalIndexMetadata {
            declared_index_path: index.path.clone(),
            metadata_path,
            metadata,
        },
        mismatches,
    })
}

fn require_fresh_local_index(
    validation: ResolvedLocalIndexValidation,
) -> Result<ResolvedLocalIndexMetadata> {
    if !validation.mismatches.is_empty() {
        bail!(
            "index metadata is stale or unsupported at {}:\n- {}\nRun `agentpm knowledge build` to refresh it.",
            validation.resolved.metadata_path.display(),
            validation.mismatches.join("\n- ")
        );
    }

    Ok(validation.resolved)
}

fn resolve_query_vector(
    base_dir: &Path,
    args: &KnowledgeQueryArgs,
    manifest: &KnowledgeManifest,
) -> Result<QueryVectorInput> {
    if let Some(vector_json) = &args.vector_json {
        return load_query_vector_json(vector_json, manifest);
    }
    if let Some(command) = &args.embedding_command {
        let query_text = args
            .query_text
            .as_deref()
            .ok_or_else(|| anyhow!("`--embedding-command` requires query text input"))?;
        return load_query_vector_from_adapter(base_dir, command, query_text, manifest);
    }
    if args.query_text.is_some() {
        let provider = manifest
            .knowledge
            .embedding
            .as_ref()
            .map(|embedding| format!("{}/{}", embedding.provider, embedding.model))
            .unwrap_or_else(|| "unknown provider/model".to_string());
        bail!(
            "This artifact uses {}.\nagentpm knowledge query cannot embed text automatically yet.\nProvide --vector-json, or use a runtime that supports this provider.",
            provider
        );
    }
    bail!("`agentpm knowledge query` requires `--vector-json <file|->`");
}

fn load_query_vector_json(
    path_or_dash: &str,
    manifest: &KnowledgeManifest,
) -> Result<QueryVectorInput> {
    let raw = if path_or_dash == "-" {
        use std::io::Read as _;
        let mut input = String::new();
        std::io::stdin()
            .read_to_string(&mut input)
            .context("reading query vector JSON from stdin")?;
        input
    } else {
        fs::read_to_string(path_or_dash)
            .with_context(|| format!("reading query vector JSON from {}", path_or_dash))?
    };
    let value: Value = serde_json::from_str(&raw).with_context(|| {
        if path_or_dash == "-" {
            "parsing query vector JSON from stdin".to_string()
        } else {
            format!("parsing query vector JSON from {}", path_or_dash)
        }
    })?;

    parse_query_vector_input(value, manifest)
}

fn load_query_vector_from_adapter(
    base_dir: &Path,
    command_line: &str,
    query_text: &str,
    manifest: &KnowledgeManifest,
) -> Result<QueryVectorInput> {
    let embedding = manifest
        .knowledge
        .embedding
        .as_ref()
        .ok_or_else(|| anyhow!("vector-mode Knowledge requires knowledge.embedding"))?;
    let input_payload = json!({
        "text": query_text,
        "embedding": {
            "provider": embedding.provider,
            "model": embedding.model,
            "dimensions": embedding.dimensions,
            "metric": embedding.metric,
            "normalized": embedding.normalized
        }
    });
    let stdout = execute_embedding_adapter(
        command_line,
        base_dir,
        &(serde_json::to_vec(&input_payload)?),
    )?;
    let value: Value =
        serde_json::from_slice(&stdout).context("adapter stdout was not valid JSON")?;
    parse_query_vector_input(value, manifest)
}

fn parse_query_vector_input(
    value: Value,
    manifest: &KnowledgeManifest,
) -> Result<QueryVectorInput> {
    let input = match value {
        Value::Array(values) => QueryVectorInput {
            values: parse_query_vector_values(&values)?,
            provider: None,
            model: None,
            dimensions: None,
        },
        Value::Object(map) => {
            let values = map
                .get("vector")
                .or_else(|| map.get("values"))
                .and_then(Value::as_array)
                .ok_or_else(|| {
                    anyhow!("query vector JSON object must contain a `vector` or `values` array")
                })?;
            let embedding_meta = map.get("embedding").and_then(Value::as_object);
            QueryVectorInput {
                values: parse_query_vector_values(values)?,
                provider: map
                    .get("provider")
                    .and_then(Value::as_str)
                    .or_else(|| {
                        embedding_meta
                            .and_then(|meta| meta.get("provider"))
                            .and_then(Value::as_str)
                    })
                    .map(str::to_string),
                model: map
                    .get("model")
                    .and_then(Value::as_str)
                    .or_else(|| {
                        embedding_meta
                            .and_then(|meta| meta.get("model"))
                            .and_then(Value::as_str)
                    })
                    .map(str::to_string),
                dimensions: map.get("dimensions").and_then(Value::as_u64).or_else(|| {
                    embedding_meta
                        .and_then(|meta| meta.get("dimensions"))
                        .and_then(Value::as_u64)
                }),
            }
        }
        _ => bail!("query vector JSON must be an array or object"),
    };

    validate_query_vector_input(&input, manifest)?;
    Ok(input)
}

fn validate_query_vector_input(
    input: &QueryVectorInput,
    manifest: &KnowledgeManifest,
) -> Result<()> {
    let embedding = manifest
        .knowledge
        .embedding
        .as_ref()
        .ok_or_else(|| anyhow!("vector-mode Knowledge requires knowledge.embedding"))?;
    if input.values.len() != embedding.dimensions as usize {
        bail!(
            "query vector length {} does not match knowledge.embedding.dimensions {}",
            input.values.len(),
            embedding.dimensions
        );
    }
    if let Some(dimensions) = input.dimensions
        && dimensions != embedding.dimensions
    {
        bail!(
            "query vector metadata dimensions {} does not match knowledge.embedding.dimensions {}",
            dimensions,
            embedding.dimensions
        );
    }
    if let Some(provider) = input.provider.as_deref()
        && provider != embedding.provider
    {
        bail!(
            "query vector metadata provider `{}` does not match knowledge.embedding.provider `{}`",
            provider,
            embedding.provider
        );
    }
    if let Some(model) = input.model.as_deref()
        && model != embedding.model
    {
        bail!(
            "query vector metadata model `{}` does not match knowledge.embedding.model `{}`",
            model,
            embedding.model
        );
    }
    Ok(())
}

fn execute_embedding_adapter(command_line: &str, cwd: &Path, input: &[u8]) -> Result<Vec<u8>> {
    let argv = parse_adapter_command_line(command_line)?;
    let program = argv
        .first()
        .ok_or_else(|| anyhow!("`--embedding-command` must not be empty"))?;
    let args = &argv[1..];

    let timed_out = Arc::new(AtomicBool::new(false));
    let stdout_over_limit = Arc::new(AtomicBool::new(false));
    let stderr_over_limit = Arc::new(AtomicBool::new(false));

    let mut cmd = Command::new(program);
    cmd.args(args)
        .current_dir(cwd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        unsafe {
            cmd.pre_exec(|| {
                if libc::setpgid(0, 0) != 0 {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }
    }

    let mut child = cmd
        .spawn()
        .with_context(|| format!("spawning embedding adapter `{}`", command_line))?;

    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| anyhow!("embedding adapter stdin was not piped"))?;
    stdin
        .write_all(input)
        .context("writing adapter request to stdin")?;
    drop(stdin);

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| anyhow!("embedding adapter stdout was not piped"))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| anyhow!("embedding adapter stderr was not piped"))?;

    let stdout_limit_flag = Arc::clone(&stdout_over_limit);
    let stdout_handle = thread::spawn(move || {
        read_stream_with_limit(
            stdout,
            EMBEDDING_ADAPTER_MAX_STDOUT_BYTES,
            &stdout_limit_flag,
        )
    });
    let stderr_limit_flag = Arc::clone(&stderr_over_limit);
    let stderr_handle = thread::spawn(move || {
        read_stream_with_limit(
            stderr,
            EMBEDDING_ADAPTER_MAX_STDERR_BYTES,
            &stderr_limit_flag,
        )
    });

    let start = Instant::now();
    let timeout = Duration::from_millis(EMBEDDING_ADAPTER_TIMEOUT_MS);
    loop {
        if child
            .try_wait()
            .context("waiting for embedding adapter")?
            .is_some()
        {
            break;
        }
        if start.elapsed() >= timeout {
            timed_out.store(true, AtomicOrdering::Relaxed);
            #[cfg(unix)]
            {
                let _ = kill_process_group(child.id());
            }
            let _ = child.kill();
            break;
        }
        if stdout_over_limit.load(AtomicOrdering::Relaxed)
            || stderr_over_limit.load(AtomicOrdering::Relaxed)
        {
            #[cfg(unix)]
            {
                let _ = kill_process_group(child.id());
            }
            let _ = child.kill();
            break;
        }
        thread::sleep(Duration::from_millis(10));
    }

    let status = child.wait().context("waiting for embedding adapter exit")?;
    let stdout = stdout_handle
        .join()
        .map_err(|_| anyhow!("embedding adapter stdout reader panicked"))??;
    let stderr = stderr_handle
        .join()
        .map_err(|_| anyhow!("embedding adapter stderr reader panicked"))??;

    if timed_out.load(AtomicOrdering::Relaxed) {
        bail!(
            "embedding adapter timed out after {} ms",
            EMBEDDING_ADAPTER_TIMEOUT_MS
        );
    }
    if stdout_over_limit.load(AtomicOrdering::Relaxed) {
        bail!(
            "embedding adapter stdout exceeded {} bytes",
            EMBEDDING_ADAPTER_MAX_STDOUT_BYTES
        );
    }
    if stderr_over_limit.load(AtomicOrdering::Relaxed) {
        bail!(
            "embedding adapter stderr exceeded {} bytes",
            EMBEDDING_ADAPTER_MAX_STDERR_BYTES
        );
    }
    if !status.success() {
        let stderr_text = String::from_utf8_lossy(&stderr);
        let suffix = if stderr_text.trim().is_empty() {
            String::new()
        } else {
            format!(": {}", stderr_text.trim())
        };
        bail!(
            "embedding adapter exited unsuccessfully (status {}){}",
            status,
            suffix
        );
    }

    Ok(stdout)
}

fn read_stream_with_limit<R: Read>(
    mut reader: R,
    limit: usize,
    over_limit: &AtomicBool,
) -> Result<Vec<u8>> {
    let mut buf = [0u8; 8192];
    let mut output = Vec::new();
    loop {
        let read = reader
            .read(&mut buf)
            .context("reading embedding adapter stream")?;
        if read == 0 {
            break;
        }
        let remaining = limit.saturating_sub(output.len());
        let to_take = remaining.min(read);
        output.extend_from_slice(&buf[..to_take]);
        if to_take < read {
            over_limit.store(true, AtomicOrdering::Relaxed);
            break;
        }
    }
    Ok(output)
}

fn parse_adapter_command_line(command_line: &str) -> Result<Vec<String>> {
    #[derive(Clone, Copy)]
    enum Quote {
        Single,
        Double,
    }

    let mut args = Vec::new();
    let mut current = String::new();
    let mut chars = command_line.chars().peekable();
    let mut quote = None;
    let mut arg_started = false;

    while let Some(ch) = chars.next() {
        match quote {
            Some(Quote::Single) => {
                if ch == '\'' {
                    quote = None;
                } else {
                    current.push(ch);
                }
                arg_started = true;
            }
            Some(Quote::Double) => {
                if ch == '"' {
                    quote = None;
                } else if ch == '\\' {
                    if let Some(next) = chars.next() {
                        current.push(next);
                    } else {
                        current.push('\\');
                    }
                } else {
                    current.push(ch);
                }
                arg_started = true;
            }
            None => match ch {
                '\'' => {
                    quote = Some(Quote::Single);
                    arg_started = true;
                }
                '"' => {
                    quote = Some(Quote::Double);
                    arg_started = true;
                }
                '\\' => {
                    if let Some(next) = chars.next() {
                        current.push(next);
                        arg_started = true;
                    }
                }
                ch if ch.is_whitespace() => {
                    if arg_started || !current.is_empty() {
                        args.push(std::mem::take(&mut current));
                        arg_started = false;
                    }
                }
                _ => {
                    current.push(ch);
                    arg_started = true;
                }
            },
        }
    }

    if quote.is_some() {
        bail!("`--embedding-command` contains an unterminated quote");
    }
    if arg_started || !current.is_empty() {
        args.push(current);
    }
    if args.is_empty() {
        bail!("`--embedding-command` must not be empty");
    }
    Ok(args)
}

#[cfg(unix)]
fn kill_process_group(pid: u32) -> Result<()> {
    let rc = unsafe { libc::killpg(pid as i32, libc::SIGKILL) };
    if rc != 0 {
        let err = std::io::Error::last_os_error();
        if err.kind() != std::io::ErrorKind::NotFound {
            return Err(err).context("killing timed-out adapter process group");
        }
    }
    Ok(())
}

fn parse_query_vector_values(values: &[Value]) -> Result<Vec<f32>> {
    let mut parsed = Vec::with_capacity(values.len());
    for (idx, value) in values.iter().enumerate() {
        let number = value
            .as_f64()
            .ok_or_else(|| anyhow!("query vector entry {} must be a number", idx))?;
        parsed.push(number as f32);
    }
    Ok(parsed)
}

fn execute_exact_vector_query(
    package_root: &Path,
    manifest: &KnowledgeManifest,
    index: &LocalIndexMetadata,
    query_vector: &[f32],
    options: QueryExecutionOptions,
) -> Result<Vec<QueryResultRow>> {
    if index.metric != "cosine" || !index.normalized {
        bail!("only metric=\"cosine\" with normalized=true is supported for local exact search");
    }

    let vectors_path = resolve_existing_file(package_root, &index.vectors_path)?;
    let chunk_path = resolve_existing_file(package_root, &index.chunks_path)?;
    let source_path = resolve_existing_file(package_root, &index.sources_path)?;

    let best_rows = score_vector_rows(
        &vectors_path,
        query_vector,
        index.dimensions as usize,
        options.top_k,
        options.score_threshold,
    )?;
    let chunks = read_chunks_jsonl(&chunk_path, &index.chunks_path)?;
    let sources = read_sources_jsonl(&source_path, &index.sources_path)?;
    let source_map = sources
        .into_iter()
        .map(|source| (source.id.clone(), source))
        .collect::<HashMap<_, _>>();

    let mut rows = Vec::new();
    for hit in best_rows {
        let chunk = chunks.get(hit.row).ok_or_else(|| {
            anyhow!(
                "query result row {} is out of bounds for {} chunk rows",
                hit.row,
                chunks.len()
            )
        })?;
        let source = source_map.get(&chunk.source_id).ok_or_else(|| {
            anyhow!(
                "chunk `{}` references missing source `{}` during hydration",
                chunk.id,
                chunk.source_id
            )
        })?;

        rows.push(QueryResultRow {
            row: hit.row,
            score: hit.score,
            chunk_id: chunk.id.clone(),
            source_id: chunk.source_id.clone(),
            source_title: source
                .object
                .get("title")
                .and_then(Value::as_str)
                .map(str::to_string),
            source_uri: source
                .object
                .get("uri")
                .and_then(Value::as_str)
                .map(str::to_string),
            text: options.include_text.then(|| chunk.text.clone()),
            chunk_metadata: options.include_metadata.then(|| {
                chunk
                    .metadata
                    .clone()
                    .unwrap_or_else(|| Value::Object(Map::new()))
            }),
            source_metadata: options
                .include_metadata
                .then(|| Value::Object(source.object.clone())),
        });
    }

    let _ = manifest;
    Ok(rows)
}

fn score_vector_rows(
    path: &Path,
    query_vector: &[f32],
    dimensions: usize,
    top_k: usize,
    score_threshold: Option<f64>,
) -> Result<Vec<QueryRowMatch>> {
    let mut file = fs::File::open(path).with_context(|| format!("reading {}", path.display()))?;
    let mut row_buffer = vec![0u8; dimensions * 4];
    let mut best = Vec::new();
    let mut row = 0usize;

    loop {
        match file.read_exact(&mut row_buffer) {
            Ok(()) => {
                let score = row_score_dot_product(&row_buffer, query_vector);
                if score_threshold.is_none_or(|threshold| score >= threshold) {
                    best.push(QueryRowMatch { row, score });
                    best.sort_by(|a, b| {
                        b.score
                            .partial_cmp(&a.score)
                            .unwrap_or(Ordering::Equal)
                            .then_with(|| a.row.cmp(&b.row))
                    });
                    if best.len() > top_k {
                        best.pop();
                    }
                }
                row += 1;
            }
            Err(err) if err.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(err) => {
                return Err(err).with_context(|| format!("reading {}", path.display()));
            }
        }
    }

    Ok(best)
}

fn row_score_dot_product(row_bytes: &[u8], query_vector: &[f32]) -> f64 {
    row_bytes
        .chunks_exact(4)
        .zip(query_vector)
        .map(|(chunk, query)| {
            let value = f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
            (value as f64) * (*query as f64)
        })
        .sum()
}

fn build_inspect_json(
    target: &ResolvedKnowledgeTarget,
    manifest: &KnowledgeManifest,
    summary: &KnowledgeBuildSummary,
    manifest_mismatches: &[String],
    local_index: Option<&ResolvedLocalIndexValidation>,
) -> Value {
    let mut root = json!({
        "target": target.display_target,
        "manifest_path": target.manifest_path,
        "package_root": target.package_root,
        "name": manifest.name,
        "version": manifest.version,
        "mode": manifest.knowledge.mode,
        "manifest_metadata_fresh": manifest_mismatches.is_empty(),
        "manifest_metadata_mismatches": manifest_mismatches,
    });

    match summary {
        KnowledgeBuildSummary::Context { result, .. } => {
            root["build"] = json!({
                "document_count": result.document_count,
                "total_bytes": result.total_bytes,
                "content_hash": result.content_hash
            });
        }
        KnowledgeBuildSummary::Vector { result, .. } => {
            root["build"] = json!({
                "chunk_count": result.chunk_count,
                "source_count": result.source_count,
                "vector_count": result.vector_count,
                "dimensions": result.dimensions,
                "corpus_hash": result.corpus_hash,
                "vectors_hash": result.vectors_hash
            });
            if let Some(index) = local_index {
                root["index"] = json!({
                    "path": index.resolved.declared_index_path,
                    "metadata_path": index.resolved.metadata_path,
                    "type": index.resolved.metadata.r#type,
                    "algorithm": index.resolved.metadata.algorithm,
                    "format_version": index.resolved.metadata.format_version,
                    "fresh": index.mismatches.is_empty(),
                    "mismatches": index.mismatches,
                    "built_at": index.resolved.metadata.built_at,
                    "agentpm_version": index.resolved.metadata.agentpm_version,
                });
            }
        }
    }

    root
}

fn build_inspect_text(
    target: &ResolvedKnowledgeTarget,
    manifest: &KnowledgeManifest,
    summary: &KnowledgeBuildSummary,
    manifest_mismatches: &[String],
    local_index: Option<&ResolvedLocalIndexValidation>,
) -> String {
    let mut lines = vec![
        format!("Knowledge: {}@{}", manifest.name, manifest.version),
        format!("Target: {}", target.display_target),
        format!("Manifest: {}", target.manifest_path.display()),
        format!("Mode: {}", manifest.knowledge.mode),
        format!(
            "Manifest metadata freshness: {}",
            if manifest_mismatches.is_empty() {
                "fresh"
            } else {
                "stale"
            }
        ),
    ];
    if !manifest_mismatches.is_empty() {
        lines.push(format!(
            "Manifest mismatches: {}",
            manifest_mismatches.join(", ")
        ));
    }

    match summary {
        KnowledgeBuildSummary::Context { result, .. } => {
            lines.push(format!("Documents: {}", result.document_count));
            lines.push(format!("Total bytes: {}", result.total_bytes));
            lines.push(format!("Content hash: {}", result.content_hash));
            for document in &manifest.knowledge.documents {
                lines.push(format!("Document: {}", document.path));
                if let Some(content_type) = &document.content_type {
                    lines.push(format!("  Content type: {}", content_type));
                }
                if let Some(role) = &document.role {
                    lines.push(format!("  Role: {}", role));
                }
                if let Some(bytes) = document.bytes {
                    lines.push(format!("  Bytes: {}", bytes));
                }
                if let Some(sha256) = &document.sha256 {
                    lines.push(format!("  SHA256: {}", sha256));
                }
            }
        }
        KnowledgeBuildSummary::Vector { result, .. } => {
            let embedding = manifest.knowledge.embedding.as_ref();
            lines.push(format!("Chunks: {}", result.chunk_count));
            lines.push(format!("Sources: {}", result.source_count));
            if let Some(embedding) = embedding {
                lines.push(format!(
                    "Embedding: {}/{}",
                    embedding.provider, embedding.model
                ));
                lines.push(format!("Dimensions: {}", embedding.dimensions));
                lines.push(format!("Metric: {}", embedding.metric));
                lines.push(format!("Normalized: {}", embedding.normalized));
                lines.push(format!("Vectors path: {}", embedding.vectors_path));
                if let Some(vectors_hash) = &embedding.vectors_hash {
                    lines.push(format!("Vectors hash: {}", vectors_hash));
                }
            }
            if let Some(index) = local_index {
                lines.push(format!(
                    "Index path: {}",
                    index.resolved.declared_index_path
                ));
                lines.push(format!("Index type: {}", index.resolved.metadata.r#type));
                lines.push(format!(
                    "Index algorithm: {}",
                    index.resolved.metadata.algorithm
                ));
                lines.push(format!(
                    "Index metadata freshness: {}",
                    if index.mismatches.is_empty() {
                        "fresh"
                    } else {
                        "stale"
                    }
                ));
                if !index.mismatches.is_empty() {
                    lines.push(format!("Index mismatches: {}", index.mismatches.join(", ")));
                }
            }
            if let Some(retrieval) = &manifest.knowledge.retrieval {
                if let Some(strategy) = &retrieval.strategy {
                    lines.push(format!("Retrieval strategy: {}", strategy));
                }
                if let Some(default_top_k) = retrieval.default_top_k {
                    lines.push(format!("Default top-k: {}", default_top_k));
                }
            }
        }
    }

    lines.join("\n")
}

fn build_query_json(
    target: &ResolvedKnowledgeTarget,
    manifest: &KnowledgeManifest,
    index: &LocalIndexMetadata,
    rows: &[QueryResultRow],
    include_text: bool,
    include_metadata: bool,
) -> Value {
    json!({
        "target": target.display_target,
        "name": manifest.name,
        "version": manifest.version,
        "mode": manifest.knowledge.mode,
        "query": {
            "algorithm": index.algorithm,
            "metric": index.metric,
            "normalized": index.normalized,
            "include_text": include_text,
            "include_metadata": include_metadata
        },
        "results": rows.iter().map(|row| {
            json!({
                "row": row.row,
                "score": row.score,
                "chunk_id": row.chunk_id,
                "source_id": row.source_id,
                "source_title": row.source_title,
                "source_uri": row.source_uri,
                "text": row.text,
                "chunk_metadata": row.chunk_metadata,
                "source_metadata": row.source_metadata
            })
        }).collect::<Vec<_>>()
    })
}

fn build_query_text(
    target: &ResolvedKnowledgeTarget,
    manifest: &KnowledgeManifest,
    index: &LocalIndexMetadata,
    rows: &[QueryResultRow],
) -> String {
    let mut lines = vec![
        format!("Knowledge query: {}@{}", manifest.name, manifest.version),
        format!("Target: {}", target.display_target),
        format!(
            "Search: {} local exact search (metric={}, normalized={})",
            index.r#type, index.metric, index.normalized
        ),
        format!("Results: {}", rows.len()),
    ];
    for (rank, row) in rows.iter().enumerate() {
        lines.push(format!(
            "{}. score={:.6} row={} chunk={} source={}",
            rank + 1,
            row.score,
            row.row,
            row.chunk_id,
            row.source_id
        ));
        if let Some(title) = &row.source_title {
            lines.push(format!("   title: {}", title));
        }
        if let Some(uri) = &row.source_uri {
            lines.push(format!("   uri: {}", uri));
        }
        if let Some(text) = &row.text {
            lines.push(format!("   text: {}", text));
        }
        if let Some(metadata) = &row.chunk_metadata {
            lines.push(format!("   chunk_metadata: {}", metadata));
        }
    }
    lines.join("\n")
}

fn resolve_manifest_path(manifest: &Path) -> Result<PathBuf> {
    if manifest.is_absolute() {
        Ok(manifest.to_path_buf())
    } else {
        Ok(std::env::current_dir()
            .context("reading current directory")?
            .join(manifest))
    }
}

fn validate_manifest_or_bail(manifest_path: &Path, value: &mut Value) -> Result<()> {
    let schema_source = resolve_schema_source(None);
    let (ok, issues) = validate_manifest_value(
        &schema_source,
        &manifest_path.to_string_lossy(),
        value,
        false,
    )?;

    if ok {
        return Ok(());
    }

    bail!(
        "manifest validation failed for {}:\n{}",
        manifest_path.display(),
        format_issues(&issues)
    );
}

fn format_issues(issues: &[LintIssue]) -> String {
    issues
        .iter()
        .map(|issue| {
            let mut line = format!("- [{}] {}", issue.level, issue.message);
            if !issue.instance_path.is_empty() {
                line.push_str(&format!(" (instance {})", issue.instance_path));
            }
            if !issue.schema_path.is_empty() {
                line.push_str(&format!(" (schema {})", issue.schema_path));
            }
            line
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn validate_declared_knowledge_paths(manifest: &KnowledgeManifest) -> Result<()> {
    for document in &manifest.knowledge.documents {
        let _ = parse_safe_relative_path(&document.path)?;
    }

    if let Some(corpus) = &manifest.knowledge.corpus {
        if let Some(chunks_path) = &corpus.chunks_path {
            let _ = parse_safe_relative_path(chunks_path)?;
        }
        if let Some(sources_path) = &corpus.sources_path {
            let _ = parse_safe_relative_path(sources_path)?;
        }
    }

    if let Some(embedding) = &manifest.knowledge.embedding {
        let _ = parse_safe_relative_path(&embedding.vectors_path)?;
    }

    for index in &manifest.knowledge.indexes {
        let _ = parse_safe_relative_path(&index.path)?;
    }

    if let Some(provenance) = &manifest.knowledge.provenance
        && let Some(sources_manifest_path) = &provenance.sources_manifest_path
    {
        let _ = parse_safe_relative_path(sources_manifest_path)?;
    }

    Ok(())
}

pub(crate) fn build_context_mode(
    package_root: &Path,
    manifest: &KnowledgeManifest,
) -> Result<ContextBuildResult> {
    if manifest.knowledge.documents.is_empty() {
        bail!("context-mode Knowledge requires at least one declared document");
    }

    let mut documents = Vec::new();
    let mut total_bytes = 0u64;
    let mut aggregate = Sha256::new();

    for doc in &manifest.knowledge.documents {
        let path = resolve_existing_file(package_root, &doc.path)?;
        let bytes = fs::read(&path).with_context(|| format!("reading {}", path.display()))?;
        let sha256 = sha256_prefixed(&bytes);
        let byte_len = bytes.len() as u64;
        total_bytes += byte_len;

        aggregate.update(doc.path.as_bytes());
        aggregate.update([0]);
        aggregate.update(&bytes);
        aggregate.update([0xff]);

        documents.push(BuiltContextDocument {
            bytes: byte_len,
            sha256,
        });
    }

    Ok(ContextBuildResult {
        document_count: documents.len() as u64,
        total_bytes,
        content_hash: format!("sha256:{:x}", aggregate.finalize()),
        documents,
    })
}

pub(crate) fn apply_context_build(
    knowledge_obj: &mut Map<String, Value>,
    result: &ContextBuildResult,
) -> Result<()> {
    let documents = knowledge_obj
        .get_mut("documents")
        .and_then(Value::as_array_mut)
        .ok_or_else(|| anyhow!("knowledge.documents must be an array for context mode"))?;

    for (doc_value, built) in documents.iter_mut().zip(&result.documents) {
        let doc_obj = doc_value
            .as_object_mut()
            .ok_or_else(|| anyhow!("knowledge.documents entries must be objects"))?;
        doc_obj.insert("bytes".into(), Value::Number(built.bytes.into()));
        doc_obj.insert("sha256".into(), Value::String(built.sha256.clone()));
    }

    knowledge_obj.insert(
        "context".into(),
        json!({
            "document_count": result.document_count,
            "total_bytes": result.total_bytes,
            "content_hash": result.content_hash,
        }),
    );
    Ok(())
}

pub(crate) fn build_vector_mode(
    package_root: &Path,
    manifest: &KnowledgeManifest,
    mode: KnowledgeBuildMode,
) -> Result<VectorBuildResult> {
    let corpus = manifest
        .knowledge
        .corpus
        .as_ref()
        .ok_or_else(|| anyhow!("vector-mode Knowledge requires knowledge.corpus"))?;
    let embedding = manifest
        .knowledge
        .embedding
        .as_ref()
        .ok_or_else(|| anyhow!("vector-mode Knowledge requires knowledge.embedding"))?;

    let chunks_path = corpus
        .chunks_path
        .as_ref()
        .ok_or_else(|| anyhow!("vector-mode Knowledge requires knowledge.corpus.chunks_path"))?;
    let sources_path = corpus
        .sources_path
        .as_ref()
        .ok_or_else(|| anyhow!("vector-mode Knowledge requires knowledge.corpus.sources_path"))?;

    let chunks_abs = resolve_existing_file(package_root, chunks_path)?;
    let sources_abs = resolve_existing_file(package_root, sources_path)?;
    let vectors_abs = resolve_existing_file(package_root, &embedding.vectors_path)?;

    let chunks = read_chunks_jsonl(&chunks_abs, chunks_path)?;
    let sources = read_sources_jsonl(&sources_abs, sources_path)?;
    validate_chunk_sources(&chunks, &sources)?;

    let dimensions = embedding.dimensions;
    if dimensions == 0 {
        bail!("knowledge.embedding.dimensions must be greater than 0");
    }

    let chunks_bytes =
        fs::read(&chunks_abs).with_context(|| format!("reading {}", chunks_abs.display()))?;
    let sources_bytes =
        fs::read(&sources_abs).with_context(|| format!("reading {}", sources_abs.display()))?;
    let vectors_bytes =
        fs::read(&vectors_abs).with_context(|| format!("reading {}", vectors_abs.display()))?;

    let vector_count = validate_vector_file(
        &vectors_abs,
        &vectors_bytes,
        dimensions,
        chunks.len() as u64,
    )?;

    let corpus_hash = aggregate_named_bytes(&[
        ("chunks", chunks_bytes.as_slice()),
        ("sources", sources_bytes.as_slice()),
    ]);
    let chunks_hash = sha256_prefixed(&chunks_bytes);
    let sources_hash = sha256_prefixed(&sources_bytes);
    let vectors_hash = sha256_prefixed(&vectors_bytes);

    if mode == KnowledgeBuildMode::Write {
        build_local_index(&LocalIndexInputs {
            package_root,
            chunks_path,
            sources_path,
            vectors_path: &embedding.vectors_path,
            embedding_id: &embedding.id,
            metric: &embedding.metric,
            normalized: embedding.normalized,
            dimensions,
            chunk_count: chunks.len() as u64,
            source_count: sources.len() as u64,
            vector_count,
            corpus_hash: &corpus_hash,
            chunks_hash: &chunks_hash,
            sources_hash: &sources_hash,
            vectors_hash: &vectors_hash,
        })?;
    }

    Ok(VectorBuildResult {
        chunk_count: chunks.len() as u64,
        source_count: sources.len() as u64,
        vector_count,
        dimensions,
        corpus_hash,
        chunks_hash,
        sources_hash,
        vectors_hash,
        embedding_id: embedding.id.clone(),
    })
}

pub(crate) fn apply_vector_build(
    knowledge_obj: &mut Map<String, Value>,
    result: &VectorBuildResult,
) -> Result<()> {
    let corpus = knowledge_obj
        .entry("corpus")
        .or_insert_with(|| Value::Object(Map::new()))
        .as_object_mut()
        .ok_or_else(|| anyhow!("knowledge.corpus must be an object for vector mode"))?;
    corpus.insert(
        "chunk_count".into(),
        Value::Number(result.chunk_count.into()),
    );
    corpus.insert(
        "source_count".into(),
        Value::Number(result.source_count.into()),
    );
    corpus.insert(
        "content_hash".into(),
        Value::String(result.corpus_hash.clone()),
    );

    let embedding = knowledge_obj
        .entry("embedding")
        .or_insert_with(|| Value::Object(Map::new()))
        .as_object_mut()
        .ok_or_else(|| anyhow!("knowledge.embedding must be an object for vector mode"))?;
    embedding.insert(
        "vector_count".into(),
        Value::Number(result.vector_count.into()),
    );
    embedding.insert(
        "vectors_hash".into(),
        Value::String(result.vectors_hash.clone()),
    );

    let mut preserved = knowledge_obj
        .get("indexes")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter(|entry| {
            entry
                .as_object()
                .and_then(|obj| obj.get("id"))
                .and_then(Value::as_str)
                != Some("default")
        })
        .collect::<Vec<_>>();
    preserved.push(json!({
        "id": "default",
        "type": "agentpm-local",
        "path": "knowledge/indexes/default",
        "embedding_id": result.embedding_id,
        "generated_by": "agentpm knowledge build"
    }));
    knowledge_obj.insert("indexes".into(), Value::Array(preserved));

    Ok(())
}

pub(crate) fn resolve_existing_file(package_root: &Path, relative: &str) -> Result<PathBuf> {
    let rel = parse_safe_relative_path(relative)?;
    let package_root = if package_root.as_os_str().is_empty() {
        Path::new(".")
    } else {
        package_root
    };
    let abs = package_root.join(&rel);
    let canon = fs::canonicalize(&abs)
        .with_context(|| format!("declared path does not exist: {}", abs.display()))?;
    let root_canon = fs::canonicalize(package_root)
        .with_context(|| format!("reading package root {}", package_root.display()))?;
    if !canon.starts_with(&root_canon) {
        bail!("declared path escapes the package root: {}", relative);
    }
    let md = fs::metadata(&canon).with_context(|| format!("reading {}", canon.display()))?;
    if !md.is_file() {
        bail!("declared path is not a file: {}", relative);
    }
    Ok(canon)
}

fn resolve_existing_dir(package_root: &Path, relative: &str) -> Result<PathBuf> {
    let rel = parse_safe_relative_path(relative)?;
    let package_root = if package_root.as_os_str().is_empty() {
        Path::new(".")
    } else {
        package_root
    };
    let abs = package_root.join(&rel);
    let canon = fs::canonicalize(&abs)
        .with_context(|| format!("declared path does not exist: {}", abs.display()))?;
    let root_canon = fs::canonicalize(package_root)
        .with_context(|| format!("reading package root {}", package_root.display()))?;
    if !canon.starts_with(&root_canon) {
        bail!("declared path escapes the package root: {}", relative);
    }
    let md = fs::metadata(&canon).with_context(|| format!("reading {}", canon.display()))?;
    if !md.is_dir() {
        bail!("declared path is not a directory: {}", relative);
    }
    Ok(canon)
}

pub(crate) fn parse_safe_relative_path(relative: &str) -> Result<PathBuf> {
    if relative.is_empty() {
        bail!("declared path must not be empty");
    }

    let path = Path::new(relative);
    if path.is_absolute() {
        bail!("declared path must be package-relative: {}", relative);
    }

    let mut clean = PathBuf::new();
    for comp in path.components() {
        match comp {
            Component::Normal(part) => clean.push(part),
            Component::CurDir => {}
            Component::ParentDir => {
                bail!("declared path must not contain `..`: {}", relative)
            }
            Component::RootDir | Component::Prefix(_) => {
                bail!("declared path must be package-relative: {}", relative)
            }
        }
    }

    if clean.as_os_str().is_empty() {
        bail!("declared path must not be empty");
    }

    Ok(clean)
}

fn read_chunks_jsonl(path: &Path, display_path: &str) -> Result<Vec<ChunkRecord>> {
    let text = fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    let mut chunks = Vec::new();
    let mut seen = HashSet::new();

    for (idx, line) in text.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let value: Value = serde_json::from_str(line)
            .with_context(|| format!("invalid JSON at {} line {}", display_path, idx + 1))?;
        let obj = value
            .as_object()
            .ok_or_else(|| anyhow!("{} line {} must be a JSON object", display_path, idx + 1))?;

        let id = obj
            .get("id")
            .and_then(Value::as_str)
            .filter(|v| !v.trim().is_empty())
            .ok_or_else(|| {
                anyhow!(
                    "{} line {} is missing a non-empty string id",
                    display_path,
                    idx + 1
                )
            })?
            .to_string();
        if !seen.insert(id.clone()) {
            bail!(
                "{} line {} has a duplicate chunk id `{}`",
                display_path,
                idx + 1,
                id
            );
        }

        let source_id = obj
            .get("source_id")
            .and_then(Value::as_str)
            .filter(|v| !v.trim().is_empty())
            .ok_or_else(|| {
                anyhow!(
                    "{} line {} is missing a non-empty string source_id",
                    display_path,
                    idx + 1
                )
            })?
            .to_string();
        let text_value = obj
            .get("text")
            .and_then(Value::as_str)
            .filter(|v| !v.trim().is_empty())
            .ok_or_else(|| {
                anyhow!(
                    "{} line {} is missing a non-empty string text",
                    display_path,
                    idx + 1
                )
            })?;

        if let Some(metadata) = obj.get("metadata")
            && !metadata.is_object()
        {
            bail!(
                "{} line {} has metadata that is not an object",
                display_path,
                idx + 1
            );
        }

        chunks.push(ChunkRecord {
            id,
            source_id,
            text: text_value.to_string(),
            metadata: obj.get("metadata").cloned(),
        });
    }

    Ok(chunks)
}

fn read_sources_jsonl(path: &Path, display_path: &str) -> Result<Vec<SourceRecord>> {
    let text = fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    let mut sources = Vec::new();
    let mut seen = HashSet::new();

    for (idx, line) in text.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let value: Value = serde_json::from_str(line)
            .with_context(|| format!("invalid JSON at {} line {}", display_path, idx + 1))?;
        let obj = value
            .as_object()
            .ok_or_else(|| anyhow!("{} line {} must be a JSON object", display_path, idx + 1))?;

        let id = obj
            .get("id")
            .and_then(Value::as_str)
            .filter(|v| !v.trim().is_empty())
            .ok_or_else(|| {
                anyhow!(
                    "{} line {} is missing a non-empty string id",
                    display_path,
                    idx + 1
                )
            })?
            .to_string();
        if !seen.insert(id.clone()) {
            bail!(
                "{} line {} has a duplicate source id `{}`",
                display_path,
                idx + 1,
                id
            );
        }

        sources.push(SourceRecord {
            id,
            object: obj.clone(),
        });
    }

    Ok(sources)
}

fn validate_chunk_sources(chunks: &[ChunkRecord], sources: &[SourceRecord]) -> Result<()> {
    let source_ids = sources
        .iter()
        .map(|source| source.id.as_str())
        .collect::<HashSet<_>>();
    for chunk in chunks {
        if !source_ids.contains(chunk.source_id.as_str()) {
            bail!(
                "chunk `{}` references unknown source_id `{}`",
                chunk.id,
                chunk.source_id
            );
        }
    }
    Ok(())
}

pub(crate) fn validate_vector_file(
    path: &Path,
    bytes: &[u8],
    dimensions: u64,
    expected_chunk_count: u64,
) -> Result<u64> {
    if bytes.is_empty() {
        bail!("vector file is empty: {}", path.display());
    }
    if !bytes.len().is_multiple_of(4) {
        bail!(
            "vector file byte length must be divisible by 4: {}",
            path.display()
        );
    }

    let float_count = (bytes.len() / 4) as u64;
    if !float_count.is_multiple_of(dimensions) {
        bail!(
            "vector file float count {} is not divisible by dimensions {}",
            float_count,
            dimensions
        );
    }

    let vector_count = float_count / dimensions;
    if vector_count != expected_chunk_count {
        bail!(
            "vector count {} does not match chunk count {}",
            vector_count,
            expected_chunk_count
        );
    }

    Ok(vector_count)
}

pub(crate) fn build_local_index(inputs: &LocalIndexInputs<'_>) -> Result<()> {
    let index_dir = inputs
        .package_root
        .join("knowledge")
        .join("indexes")
        .join("default");
    if index_dir.exists() {
        fs::remove_dir_all(&index_dir)
            .with_context(|| format!("removing {}", index_dir.display()))?;
    }
    fs::create_dir_all(&index_dir).with_context(|| format!("creating {}", index_dir.display()))?;

    let metadata = json!({
        "type": "agentpm-local",
        "format_version": 1,
        "algorithm": "exact",
        "embedding_id": inputs.embedding_id,
        "metric": inputs.metric,
        "normalized": inputs.normalized,
        "source_corpus_hash": inputs.corpus_hash,
        "source_chunks_hash": inputs.chunks_hash,
        "source_sources_hash": inputs.sources_hash,
        "source_vectors_hash": inputs.vectors_hash,
        "chunks_path": inputs.chunks_path,
        "sources_path": inputs.sources_path,
        "vectors_path": inputs.vectors_path,
        "dimensions": inputs.dimensions,
        "chunk_count": inputs.chunk_count,
        "source_count": inputs.source_count,
        "vector_count": inputs.vector_count,
        "built_at": Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true),
        "agentpm_version": env!("CARGO_PKG_VERSION")
    });
    write_manifest_pretty_atomic(&index_dir.join("metadata.json"), &metadata)
        .with_context(|| format!("writing {}", index_dir.join("metadata.json").display()))?;

    Ok(())
}

fn aggregate_named_bytes(entries: &[(&str, &[u8])]) -> String {
    let mut hasher = Sha256::new();
    for (name, bytes) in entries {
        hasher.update(name.as_bytes());
        hasher.update([0]);
        hasher.update(bytes);
        hasher.update([0xff]);
    }
    format!("sha256:{:x}", hasher.finalize())
}

fn sha256_prefixed(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("sha256:{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::{load_manifest_value, validate_manifest_value};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(label: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("agentpm-knowledge-{label}-{nanos}"));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn schema_path() -> String {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../schemas/agentpm.manifest.schema.json")
            .to_string_lossy()
            .into_owned()
    }

    fn write_manifest(root: &Path, value: &Value) {
        write_manifest_pretty_atomic(&root.join("agent.json"), value).unwrap();
    }

    fn assert_manifest_valid(path: &Path) {
        let (mut value, _) = load_manifest_value(path).unwrap();
        let (ok, issues) =
            validate_manifest_value(&schema_path(), &path.to_string_lossy(), &mut value, false)
                .unwrap();
        assert!(ok, "manifest should validate after build: {issues:#?}");
    }

    fn write_f32_vectors(path: &Path, rows: &[Vec<f32>]) {
        let mut bytes = Vec::new();
        for row in rows {
            for value in row {
                bytes.extend_from_slice(&value.to_le_bytes());
            }
        }
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, bytes).unwrap();
    }

    fn available_python() -> Option<String> {
        for candidate in ["python3", "python"] {
            let status = std::process::Command::new(candidate)
                .arg("--version")
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
            if matches!(status, Ok(status) if status.success()) {
                return Some(candidate.to_string());
            }
        }
        None
    }

    fn write_adapter_script(root: &Path, body: &str) -> PathBuf {
        let path = root.join("adapter.py");
        fs::write(&path, body).unwrap();
        path
    }

    fn quote_arg(arg: &str) -> String {
        format!("\"{}\"", arg.replace('\\', "\\\\").replace('"', "\\\""))
    }

    #[tokio::test]
    async fn build_context_mode_updates_manifest_metadata() {
        let root = temp_dir("context");
        fs::create_dir_all(root.join("knowledge/docs")).unwrap();
        fs::write(
            root.join("knowledge/docs/playbook.md"),
            "# Playbook\n\nUse this context.\n",
        )
        .unwrap();
        write_manifest(
            &root,
            &json!({
                "kind": "knowledge",
                "name": "engineering-playbook",
                "version": "0.1.0",
                "description": "Engineering playbook intended for direct context loading.",
                "knowledge": {
                    "mode": "context",
                    "documents": [
                        {
                            "path": "knowledge/docs/playbook.md",
                            "content_type": "text/markdown",
                            "role": "context"
                        }
                    ]
                }
            }),
        );

        KnowledgeBuildArgs {
            manifest: root.join("agent.json"),
        }
        .run()
        .await
        .unwrap();

        let manifest_path = root.join("agent.json");
        assert_manifest_valid(&manifest_path);
        let built: Value =
            serde_json::from_str(&fs::read_to_string(&manifest_path).unwrap()).unwrap();

        assert_eq!(built["knowledge"]["context"]["document_count"], 1);
        assert_eq!(
            built["knowledge"]["documents"][0]["bytes"]
                .as_u64()
                .unwrap(),
            fs::metadata(root.join("knowledge/docs/playbook.md"))
                .unwrap()
                .len()
        );
        assert!(
            built["knowledge"]["documents"][0]["sha256"]
                .as_str()
                .unwrap()
                .starts_with("sha256:")
        );
        assert!(
            built["knowledge"]["context"]["content_hash"]
                .as_str()
                .unwrap()
                .starts_with("sha256:")
        );

        let _ = fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn build_vector_mode_updates_manifest_and_generates_index() {
        let root = temp_dir("vector");
        fs::create_dir_all(root.join("knowledge/embeddings")).unwrap();
        fs::write(
            root.join("knowledge/chunks.jsonl"),
            concat!(
                "{\"id\":\"chunk_1\",\"source_id\":\"src_1\",\"text\":\"Alpha text\"}\n",
                "{\"id\":\"chunk_2\",\"source_id\":\"src_2\",\"text\":\"Beta text\",\"metadata\":{\"section\":\"beta\"}}\n"
            ),
        )
        .unwrap();
        fs::write(
            root.join("knowledge/sources.jsonl"),
            concat!(
                "{\"id\":\"src_1\",\"title\":\"Source One\"}\n",
                "{\"id\":\"src_2\",\"title\":\"Source Two\"}\n"
            ),
        )
        .unwrap();
        write_f32_vectors(
            &root.join("knowledge/embeddings/default.f32"),
            &[vec![0.1, 0.2, 0.3], vec![0.4, 0.5, 0.6]],
        );
        write_manifest(
            &root,
            &json!({
                "kind": "knowledge",
                "name": "python-docs",
                "version": "0.1.0",
                "description": "Prepared retrieval corpus for Python documentation.",
                "knowledge": {
                    "mode": "vector",
                    "corpus": {
                        "chunks_path": "knowledge/chunks.jsonl",
                        "sources_path": "knowledge/sources.jsonl"
                    },
                    "embedding": {
                        "id": "default",
                        "provider": "openai",
                        "model": "text-embedding-3-small",
                        "dimensions": 3,
                        "metric": "cosine",
                        "normalized": true,
                        "vectors_path": "knowledge/embeddings/default.f32"
                    }
                }
            }),
        );

        KnowledgeBuildArgs {
            manifest: root.join("agent.json"),
        }
        .run()
        .await
        .unwrap();

        let manifest_path = root.join("agent.json");
        assert_manifest_valid(&manifest_path);
        let built: Value =
            serde_json::from_str(&fs::read_to_string(&manifest_path).unwrap()).unwrap();

        assert_eq!(built["knowledge"]["corpus"]["chunk_count"], 2);
        assert_eq!(built["knowledge"]["corpus"]["source_count"], 2);
        assert!(
            built["knowledge"]["corpus"]["content_hash"]
                .as_str()
                .unwrap()
                .starts_with("sha256:")
        );
        assert_eq!(built["knowledge"]["embedding"]["vector_count"], 2);
        assert!(
            built["knowledge"]["embedding"]["vectors_hash"]
                .as_str()
                .unwrap()
                .starts_with("sha256:")
        );
        assert_eq!(built["knowledge"]["indexes"][0]["type"], "agentpm-local");
        assert_eq!(
            built["knowledge"]["indexes"][0]["path"],
            "knowledge/indexes/default"
        );
        assert!(
            root.join("knowledge/indexes/default/metadata.json")
                .exists()
        );
        assert!(!root.join("knowledge/indexes/default/vectors.f32").exists());
        assert!(
            !root
                .join("knowledge/indexes/default/chunk_ids.json")
                .exists()
        );

        let metadata: Value = serde_json::from_str(
            &fs::read_to_string(root.join("knowledge/indexes/default/metadata.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(metadata["type"], "agentpm-local");
        assert_eq!(metadata["format_version"], 1);
        assert_eq!(metadata["algorithm"], "exact");
        assert_eq!(metadata["embedding_id"], "default");
        assert_eq!(metadata["metric"], "cosine");
        assert_eq!(metadata["normalized"], true);
        assert_eq!(metadata["chunks_path"], "knowledge/chunks.jsonl");
        assert_eq!(metadata["sources_path"], "knowledge/sources.jsonl");
        assert_eq!(metadata["vectors_path"], "knowledge/embeddings/default.f32");
        assert_eq!(metadata["dimensions"], 3);
        assert_eq!(metadata["chunk_count"], 2);
        assert_eq!(metadata["source_count"], 2);
        assert_eq!(metadata["vector_count"], 2);
        assert!(
            metadata["source_corpus_hash"]
                .as_str()
                .unwrap()
                .starts_with("sha256:")
        );
        assert!(
            metadata["source_chunks_hash"]
                .as_str()
                .unwrap()
                .starts_with("sha256:")
        );
        assert!(
            metadata["source_sources_hash"]
                .as_str()
                .unwrap()
                .starts_with("sha256:")
        );
        assert!(
            metadata["source_vectors_hash"]
                .as_str()
                .unwrap()
                .starts_with("sha256:")
        );

        let _ = fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn build_vector_mode_rejects_vector_count_mismatch() {
        let root = temp_dir("vector-mismatch");
        fs::create_dir_all(root.join("knowledge/embeddings")).unwrap();
        fs::write(
            root.join("knowledge/chunks.jsonl"),
            concat!(
                "{\"id\":\"chunk_1\",\"source_id\":\"src_1\",\"text\":\"Alpha text\"}\n",
                "{\"id\":\"chunk_2\",\"source_id\":\"src_1\",\"text\":\"Beta text\"}\n"
            ),
        )
        .unwrap();
        fs::write(
            root.join("knowledge/sources.jsonl"),
            "{\"id\":\"src_1\",\"title\":\"Source One\"}\n",
        )
        .unwrap();
        write_f32_vectors(
            &root.join("knowledge/embeddings/default.f32"),
            &[vec![0.1, 0.2, 0.3]],
        );
        write_manifest(
            &root,
            &json!({
                "kind": "knowledge",
                "name": "python-docs",
                "version": "0.1.0",
                "description": "Prepared retrieval corpus for Python documentation.",
                "knowledge": {
                    "mode": "vector",
                    "corpus": {
                        "chunks_path": "knowledge/chunks.jsonl",
                        "sources_path": "knowledge/sources.jsonl"
                    },
                    "embedding": {
                        "id": "default",
                        "provider": "openai",
                        "model": "text-embedding-3-small",
                        "dimensions": 3,
                        "metric": "cosine",
                        "normalized": true,
                        "vectors_path": "knowledge/embeddings/default.f32"
                    }
                }
            }),
        );

        let err = KnowledgeBuildArgs {
            manifest: root.join("agent.json"),
        }
        .run()
        .await
        .unwrap_err();

        assert!(
            format!("{err:#}").contains("vector count 1 does not match chunk count 2"),
            "{err:#}"
        );

        let _ = fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn build_context_mode_rejects_missing_document_path() {
        let root = temp_dir("context-missing-doc");
        write_manifest(
            &root,
            &json!({
                "kind": "knowledge",
                "name": "engineering-playbook",
                "version": "0.1.0",
                "description": "Engineering playbook intended for direct context loading.",
                "knowledge": {
                    "mode": "context",
                    "documents": [
                        {
                            "path": "knowledge/docs/missing.md",
                            "content_type": "text/markdown",
                            "role": "context"
                        }
                    ]
                }
            }),
        );

        let err = KnowledgeBuildArgs {
            manifest: root.join("agent.json"),
        }
        .run()
        .await
        .unwrap_err();

        assert!(
            format!("{err:#}").contains("declared path does not exist"),
            "{err:#}"
        );

        let _ = fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn build_vector_mode_rejects_duplicate_chunk_ids() {
        let root = temp_dir("vector-dup-chunks");
        fs::create_dir_all(root.join("knowledge/embeddings")).unwrap();
        fs::write(
            root.join("knowledge/chunks.jsonl"),
            concat!(
                "{\"id\":\"chunk_1\",\"source_id\":\"src_1\",\"text\":\"Alpha text\"}\n",
                "{\"id\":\"chunk_1\",\"source_id\":\"src_2\",\"text\":\"Beta text\"}\n"
            ),
        )
        .unwrap();
        fs::write(
            root.join("knowledge/sources.jsonl"),
            concat!(
                "{\"id\":\"src_1\",\"title\":\"Source One\"}\n",
                "{\"id\":\"src_2\",\"title\":\"Source Two\"}\n"
            ),
        )
        .unwrap();
        write_f32_vectors(
            &root.join("knowledge/embeddings/default.f32"),
            &[vec![0.1, 0.2, 0.3], vec![0.4, 0.5, 0.6]],
        );
        write_manifest(
            &root,
            &json!({
                "kind": "knowledge",
                "name": "python-docs",
                "version": "0.1.0",
                "description": "Prepared retrieval corpus for Python documentation.",
                "knowledge": {
                    "mode": "vector",
                    "corpus": {
                        "chunks_path": "knowledge/chunks.jsonl",
                        "sources_path": "knowledge/sources.jsonl"
                    },
                    "embedding": {
                        "id": "default",
                        "provider": "openai",
                        "model": "text-embedding-3-small",
                        "dimensions": 3,
                        "metric": "cosine",
                        "normalized": true,
                        "vectors_path": "knowledge/embeddings/default.f32"
                    }
                }
            }),
        );

        let err = KnowledgeBuildArgs {
            manifest: root.join("agent.json"),
        }
        .run()
        .await
        .unwrap_err();

        assert!(format!("{err:#}").contains("duplicate chunk id"), "{err:#}");

        let _ = fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn build_vector_mode_rejects_duplicate_source_ids() {
        let root = temp_dir("vector-dup-sources");
        fs::create_dir_all(root.join("knowledge/embeddings")).unwrap();
        fs::write(
            root.join("knowledge/chunks.jsonl"),
            concat!(
                "{\"id\":\"chunk_1\",\"source_id\":\"src_1\",\"text\":\"Alpha text\"}\n",
                "{\"id\":\"chunk_2\",\"source_id\":\"src_1\",\"text\":\"Beta text\"}\n"
            ),
        )
        .unwrap();
        fs::write(
            root.join("knowledge/sources.jsonl"),
            concat!(
                "{\"id\":\"src_1\",\"title\":\"Source One\"}\n",
                "{\"id\":\"src_1\",\"title\":\"Source One Duplicate\"}\n"
            ),
        )
        .unwrap();
        write_f32_vectors(
            &root.join("knowledge/embeddings/default.f32"),
            &[vec![0.1, 0.2, 0.3], vec![0.4, 0.5, 0.6]],
        );
        write_manifest(
            &root,
            &json!({
                "kind": "knowledge",
                "name": "python-docs",
                "version": "0.1.0",
                "description": "Prepared retrieval corpus for Python documentation.",
                "knowledge": {
                    "mode": "vector",
                    "corpus": {
                        "chunks_path": "knowledge/chunks.jsonl",
                        "sources_path": "knowledge/sources.jsonl"
                    },
                    "embedding": {
                        "id": "default",
                        "provider": "openai",
                        "model": "text-embedding-3-small",
                        "dimensions": 3,
                        "metric": "cosine",
                        "normalized": true,
                        "vectors_path": "knowledge/embeddings/default.f32"
                    }
                }
            }),
        );

        let err = KnowledgeBuildArgs {
            manifest: root.join("agent.json"),
        }
        .run()
        .await
        .unwrap_err();

        assert!(
            format!("{err:#}").contains("duplicate source id"),
            "{err:#}"
        );

        let _ = fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn build_vector_mode_rejects_chunk_source_missing_from_sources() {
        let root = temp_dir("vector-missing-source-ref");
        fs::create_dir_all(root.join("knowledge/embeddings")).unwrap();
        fs::write(
            root.join("knowledge/chunks.jsonl"),
            concat!(
                "{\"id\":\"chunk_1\",\"source_id\":\"src_1\",\"text\":\"Alpha text\"}\n",
                "{\"id\":\"chunk_2\",\"source_id\":\"src_missing\",\"text\":\"Beta text\"}\n"
            ),
        )
        .unwrap();
        fs::write(
            root.join("knowledge/sources.jsonl"),
            "{\"id\":\"src_1\",\"title\":\"Source One\"}\n",
        )
        .unwrap();
        write_f32_vectors(
            &root.join("knowledge/embeddings/default.f32"),
            &[vec![0.1, 0.2, 0.3], vec![0.4, 0.5, 0.6]],
        );
        write_manifest(
            &root,
            &json!({
                "kind": "knowledge",
                "name": "python-docs",
                "version": "0.1.0",
                "description": "Prepared retrieval corpus for Python documentation.",
                "knowledge": {
                    "mode": "vector",
                    "corpus": {
                        "chunks_path": "knowledge/chunks.jsonl",
                        "sources_path": "knowledge/sources.jsonl"
                    },
                    "embedding": {
                        "id": "default",
                        "provider": "openai",
                        "model": "text-embedding-3-small",
                        "dimensions": 3,
                        "metric": "cosine",
                        "normalized": true,
                        "vectors_path": "knowledge/embeddings/default.f32"
                    }
                }
            }),
        );

        let err = KnowledgeBuildArgs {
            manifest: root.join("agent.json"),
        }
        .run()
        .await
        .unwrap_err();

        assert!(
            format!("{err:#}").contains("references unknown source_id"),
            "{err:#}"
        );

        let _ = fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn build_vector_mode_rejects_missing_vector_file() {
        let root = temp_dir("vector-missing-file");
        fs::create_dir_all(root.join("knowledge/embeddings")).unwrap();
        fs::write(
            root.join("knowledge/chunks.jsonl"),
            concat!(
                "{\"id\":\"chunk_1\",\"source_id\":\"src_1\",\"text\":\"Alpha text\"}\n",
                "{\"id\":\"chunk_2\",\"source_id\":\"src_2\",\"text\":\"Beta text\"}\n"
            ),
        )
        .unwrap();
        fs::write(
            root.join("knowledge/sources.jsonl"),
            concat!(
                "{\"id\":\"src_1\",\"title\":\"Source One\"}\n",
                "{\"id\":\"src_2\",\"title\":\"Source Two\"}\n"
            ),
        )
        .unwrap();
        write_manifest(
            &root,
            &json!({
                "kind": "knowledge",
                "name": "python-docs",
                "version": "0.1.0",
                "description": "Prepared retrieval corpus for Python documentation.",
                "knowledge": {
                    "mode": "vector",
                    "corpus": {
                        "chunks_path": "knowledge/chunks.jsonl",
                        "sources_path": "knowledge/sources.jsonl"
                    },
                    "embedding": {
                        "id": "default",
                        "provider": "openai",
                        "model": "text-embedding-3-small",
                        "dimensions": 3,
                        "metric": "cosine",
                        "normalized": true,
                        "vectors_path": "knowledge/embeddings/default.f32"
                    }
                }
            }),
        );

        let err = KnowledgeBuildArgs {
            manifest: root.join("agent.json"),
        }
        .run()
        .await
        .unwrap_err();

        assert!(
            format!("{err:#}").contains("declared path does not exist"),
            "{err:#}"
        );

        let _ = fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn build_vector_mode_rejects_vector_dimension_mismatch() {
        let root = temp_dir("vector-dimension-mismatch");
        fs::create_dir_all(root.join("knowledge/embeddings")).unwrap();
        fs::write(
            root.join("knowledge/chunks.jsonl"),
            concat!(
                "{\"id\":\"chunk_1\",\"source_id\":\"src_1\",\"text\":\"Alpha text\"}\n",
                "{\"id\":\"chunk_2\",\"source_id\":\"src_2\",\"text\":\"Beta text\"}\n"
            ),
        )
        .unwrap();
        fs::write(
            root.join("knowledge/sources.jsonl"),
            concat!(
                "{\"id\":\"src_1\",\"title\":\"Source One\"}\n",
                "{\"id\":\"src_2\",\"title\":\"Source Two\"}\n"
            ),
        )
        .unwrap();
        write_f32_vectors(
            &root.join("knowledge/embeddings/default.f32"),
            &[vec![0.1, 0.2, 0.3, 0.4], vec![0.5, 0.6, 0.7, 0.8]],
        );
        write_manifest(
            &root,
            &json!({
                "kind": "knowledge",
                "name": "python-docs",
                "version": "0.1.0",
                "description": "Prepared retrieval corpus for Python documentation.",
                "knowledge": {
                    "mode": "vector",
                    "corpus": {
                        "chunks_path": "knowledge/chunks.jsonl",
                        "sources_path": "knowledge/sources.jsonl"
                    },
                    "embedding": {
                        "id": "default",
                        "provider": "openai",
                        "model": "text-embedding-3-small",
                        "dimensions": 3,
                        "metric": "cosine",
                        "normalized": true,
                        "vectors_path": "knowledge/embeddings/default.f32"
                    }
                }
            }),
        );

        let err = KnowledgeBuildArgs {
            manifest: root.join("agent.json"),
        }
        .run()
        .await
        .unwrap_err();

        assert!(
            format!("{err:#}").contains("float count 8 is not divisible by dimensions 3"),
            "{err:#}"
        );

        let _ = fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn build_rejects_non_knowledge_manifest() {
        let root = temp_dir("non-knowledge");
        write_manifest(
            &root,
            &json!({
                "kind": "agent",
                "name": "research-agent",
                "version": "0.1.0",
                "description": "Not a knowledge package.",
                "tools": []
            }),
        );

        let err = KnowledgeBuildArgs {
            manifest: root.join("agent.json"),
        }
        .run()
        .await
        .unwrap_err();

        assert!(
            format!("{err:#}").contains("`agentpm knowledge build` requires kind=\"knowledge\""),
            "{err:#}"
        );

        let _ = fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn check_mode_does_not_rewrite_manifest_or_generate_index() {
        let root = temp_dir("check-only");
        fs::create_dir_all(root.join("knowledge/embeddings")).unwrap();
        fs::write(
            root.join("knowledge/chunks.jsonl"),
            concat!(
                "{\"id\":\"chunk_1\",\"source_id\":\"src_1\",\"text\":\"Alpha text\"}\n",
                "{\"id\":\"chunk_2\",\"source_id\":\"src_2\",\"text\":\"Beta text\"}\n"
            ),
        )
        .unwrap();
        fs::write(
            root.join("knowledge/sources.jsonl"),
            concat!(
                "{\"id\":\"src_1\",\"title\":\"Source One\"}\n",
                "{\"id\":\"src_2\",\"title\":\"Source Two\"}\n"
            ),
        )
        .unwrap();
        write_f32_vectors(
            &root.join("knowledge/embeddings/default.f32"),
            &[vec![0.1, 0.2, 0.3], vec![0.4, 0.5, 0.6]],
        );
        write_manifest(
            &root,
            &json!({
                "kind": "knowledge",
                "name": "python-docs",
                "version": "0.1.0",
                "description": "Prepared retrieval corpus for Python documentation.",
                "knowledge": {
                    "mode": "vector",
                    "corpus": {
                        "chunks_path": "knowledge/chunks.jsonl",
                        "sources_path": "knowledge/sources.jsonl"
                    },
                    "embedding": {
                        "id": "default",
                        "provider": "openai",
                        "model": "text-embedding-3-small",
                        "dimensions": 3,
                        "metric": "cosine",
                        "normalized": true,
                        "vectors_path": "knowledge/embeddings/default.f32"
                    }
                }
            }),
        );
        let before = fs::read_to_string(root.join("agent.json")).unwrap();

        let summary =
            execute_knowledge_build(&root.join("agent.json"), KnowledgeBuildMode::Check).unwrap();

        match summary {
            KnowledgeBuildSummary::Vector { result, .. } => {
                assert_eq!(result.chunk_count, 2);
                assert_eq!(result.source_count, 2);
                assert_eq!(result.vector_count, 2);
            }
            other => panic!("expected vector summary, got {other:?}"),
        }

        let after = fs::read_to_string(root.join("agent.json")).unwrap();
        assert_eq!(before, after, "check mode should not rewrite agent.json");
        assert!(
            !root.join("knowledge/indexes/default").exists(),
            "check mode should not generate the local index"
        );

        let _ = fs::remove_dir_all(root);
    }

    fn write_context_fixture(root: &Path) {
        fs::create_dir_all(root.join("knowledge/docs")).unwrap();
        fs::write(
            root.join("knowledge/docs/playbook.md"),
            "# Playbook\n\nUse this context.\n",
        )
        .unwrap();
        write_manifest(
            root,
            &json!({
                "kind": "knowledge",
                "name": "engineering-playbook",
                "version": "0.1.0",
                "description": "Engineering playbook intended for direct context loading.",
                "knowledge": {
                    "mode": "context",
                    "documents": [
                        {
                            "path": "knowledge/docs/playbook.md",
                            "content_type": "text/markdown",
                            "role": "context"
                        }
                    ]
                }
            }),
        );
        execute_knowledge_build(&root.join("agent.json"), KnowledgeBuildMode::Write).unwrap();
    }

    fn write_vector_fixture(root: &Path) {
        fs::create_dir_all(root.join("knowledge/embeddings")).unwrap();
        fs::write(
            root.join("knowledge/chunks.jsonl"),
            concat!(
                "{\"id\":\"chunk_1\",\"source_id\":\"src_1\",\"text\":\"Alpha text\"}\n",
                "{\"id\":\"chunk_2\",\"source_id\":\"src_2\",\"text\":\"Beta text\",\"metadata\":{\"section\":\"beta\"}}\n",
                "{\"id\":\"chunk_3\",\"source_id\":\"src_2\",\"text\":\"Gamma text\"}\n"
            ),
        )
        .unwrap();
        fs::write(
            root.join("knowledge/sources.jsonl"),
            concat!(
                "{\"id\":\"src_1\",\"title\":\"Source One\",\"uri\":\"https://example.com/one\"}\n",
                "{\"id\":\"src_2\",\"title\":\"Source Two\",\"uri\":\"https://example.com/two\"}\n"
            ),
        )
        .unwrap();
        write_f32_vectors(
            &root.join("knowledge/embeddings/default.f32"),
            &[
                vec![1.0, 0.0, 0.0],
                vec![0.0, 1.0, 0.0],
                vec![0.5, 0.5, 0.0],
            ],
        );
        write_manifest(
            root,
            &json!({
                "kind": "knowledge",
                "name": "python-docs",
                "version": "0.1.0",
                "description": "Prepared retrieval corpus for Python documentation.",
                "knowledge": {
                    "mode": "vector",
                    "corpus": {
                        "chunks_path": "knowledge/chunks.jsonl",
                        "sources_path": "knowledge/sources.jsonl"
                    },
                    "embedding": {
                        "id": "default",
                        "provider": "openai",
                        "model": "text-embedding-3-small",
                        "dimensions": 3,
                        "metric": "cosine",
                        "normalized": true,
                        "vectors_path": "knowledge/embeddings/default.f32"
                    },
                    "retrieval": {
                        "strategy": "vector",
                        "default_top_k": 2
                    }
                }
            }),
        );
        execute_knowledge_build(&root.join("agent.json"), KnowledgeBuildMode::Write).unwrap();
    }

    #[test]
    fn inspect_context_mode_local_package_outputs_metadata() {
        let root = temp_dir("inspect-context-local");
        write_context_fixture(&root);

        let view = inspect_knowledge(
            &root,
            &KnowledgeInspectArgs {
                target: ".".to_string(),
                json: true,
            },
        )
        .unwrap();

        assert_eq!(view.json["mode"], "context");
        assert_eq!(view.json["build"]["document_count"], 1);
        assert_eq!(view.json["manifest_metadata_fresh"], true);
        assert!(view.text.contains("Mode: context"));
        assert!(view.text.contains("Document: knowledge/docs/playbook.md"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn inspect_vector_mode_local_package_includes_index_freshness() {
        let root = temp_dir("inspect-vector-local");
        write_vector_fixture(&root);

        let view = inspect_knowledge(
            &root,
            &KnowledgeInspectArgs {
                target: ".".to_string(),
                json: true,
            },
        )
        .unwrap();

        assert_eq!(view.json["mode"], "vector");
        assert_eq!(view.json["index"]["algorithm"], "exact");
        assert_eq!(view.json["index"]["fresh"], true);
        assert!(view.text.contains("Index metadata freshness: fresh"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn inspect_vector_mode_reports_stale_index_instead_of_failing() {
        let root = temp_dir("inspect-vector-stale-index");
        write_vector_fixture(&root);
        let metadata_path = root.join("knowledge/indexes/default/metadata.json");
        let mut metadata: Value =
            serde_json::from_str(&fs::read_to_string(&metadata_path).unwrap()).unwrap();
        metadata["metric"] = Value::String("euclidean".to_string());
        write_manifest_pretty_atomic(&metadata_path, &metadata).unwrap();

        let view = inspect_knowledge(
            &root,
            &KnowledgeInspectArgs {
                target: ".".to_string(),
                json: true,
            },
        )
        .unwrap();

        assert_eq!(view.json["index"]["fresh"], false);
        assert_eq!(view.json["index"]["mismatches"], json!(["metric"]));
        assert!(view.text.contains("Index metadata freshness: stale"));
        assert!(view.text.contains("Index mismatches: metric"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn inspect_installed_vector_package_ref_resolves_via_lock_and_install_layout() {
        let root = temp_dir("inspect-installed-vector");
        let install_root = root.join(".agentpm/knowledge/zack/python-docs/0.1.0");
        write_vector_fixture(&install_root);
        crate::manifest::write_lock(
            &root,
            &Lock::V2(crate::semver::types::LockV2 {
                lockfile_version: 3,
                generated: Utc::now(),
                packages: BTreeMap::from([(
                    "knowledge:@zack/python-docs@0.1.0".to_string(),
                    crate::semver::types::LockedPackage {
                        kind: PackageKind::Knowledge,
                        name: "@zack/python-docs".to_string(),
                        version: "0.1.0".to_string(),
                        integrity: "sha256-knowledge".to_string(),
                    },
                )]),
                roots: BTreeMap::new(),
            }),
        )
        .unwrap();

        let view = inspect_knowledge(
            &root,
            &KnowledgeInspectArgs {
                target: "@zack/python-docs".to_string(),
                json: true,
            },
        )
        .unwrap();

        assert_eq!(view.json["name"], "python-docs");
        assert_eq!(view.json["index"]["algorithm"], "exact");

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn inspect_installed_context_package_ref_resolves_via_lock_and_install_layout() {
        let root = temp_dir("inspect-installed-context");
        let install_root = root.join(".agentpm/knowledge/zack/engineering-playbook/0.1.0");
        write_context_fixture(&install_root);
        crate::manifest::write_lock(
            &root,
            &Lock::V2(crate::semver::types::LockV2 {
                lockfile_version: 3,
                generated: Utc::now(),
                packages: BTreeMap::from([(
                    "knowledge:@zack/engineering-playbook@0.1.0".to_string(),
                    crate::semver::types::LockedPackage {
                        kind: PackageKind::Knowledge,
                        name: "@zack/engineering-playbook".to_string(),
                        version: "0.1.0".to_string(),
                        integrity: "sha256-knowledge".to_string(),
                    },
                )]),
                roots: BTreeMap::new(),
            }),
        )
        .unwrap();

        let view = inspect_knowledge(
            &root,
            &KnowledgeInspectArgs {
                target: "@zack/engineering-playbook".to_string(),
                json: true,
            },
        )
        .unwrap();

        assert_eq!(view.json["name"], "engineering-playbook");
        assert_eq!(view.json["mode"], "context");
        assert_eq!(view.json["build"]["document_count"], 1);
        assert_eq!(view.json["manifest_metadata_fresh"], true);

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn query_vector_json_returns_ranked_rows_and_source_metadata() {
        let root = temp_dir("query-vector");
        write_vector_fixture(&root);
        let vector_path = root.join("query.json");
        fs::write(&vector_path, "{\"vector\":[1.0,0.0,0.0]}").unwrap();

        let view = query_knowledge(
            &root,
            &KnowledgeQueryArgs {
                target: ".".to_string(),
                query_text: None,
                top_k: Some(2),
                score_threshold: None,
                json: true,
                include_text: true,
                include_metadata: true,
                vector_json: Some(vector_path.to_string_lossy().into_owned()),
                embedding_command: None,
            },
        )
        .unwrap();

        let results = view.json["results"].as_array().unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0]["chunk_id"], "chunk_1");
        assert_eq!(results[0]["row"], 0);
        assert_eq!(results[0]["source_id"], "src_1");
        assert_eq!(results[0]["source_title"], "Source One");
        assert_eq!(results[1]["chunk_id"], "chunk_3");
        assert_eq!(results[1]["row"], 2);
        assert!(view.text.contains("chunk=chunk_1"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn query_rejects_context_mode_packages_clearly() {
        let root = temp_dir("query-context-mode");
        write_context_fixture(&root);
        let vector_path = root.join("query.json");
        fs::write(&vector_path, "[1.0,0.0,0.0]").unwrap();

        let err = query_knowledge(
            &root,
            &KnowledgeQueryArgs {
                target: ".".to_string(),
                query_text: None,
                top_k: None,
                score_threshold: None,
                json: false,
                include_text: false,
                include_metadata: false,
                vector_json: Some(vector_path.to_string_lossy().into_owned()),
                embedding_command: None,
            },
        )
        .unwrap_err();

        assert!(format!("{err:#}").contains("intended for direct context loading"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn query_rejects_context_mode_before_embedding_command_executes() {
        let python = available_python().expect("python required for tests");
        let root = temp_dir("query-context-mode-embedding-command");
        write_context_fixture(&root);
        let marker = root.join("adapter-ran.txt");
        let script = write_adapter_script(
            &root,
            &format!(
                r#"from pathlib import Path
Path({}).write_text("ran", encoding="utf-8")
raise SystemExit(0)
"#,
                quote_arg(&marker.to_string_lossy())
            ),
        );
        let command = format!("{python} {}", quote_arg(&script.to_string_lossy()));

        let err = query_knowledge(
            &root,
            &KnowledgeQueryArgs {
                target: ".".to_string(),
                query_text: Some("How does handoff work?".to_string()),
                top_k: None,
                score_threshold: None,
                json: false,
                include_text: false,
                include_metadata: false,
                vector_json: None,
                embedding_command: Some(command),
            },
        )
        .unwrap_err();

        assert!(format!("{err:#}").contains("intended for direct context loading"));
        assert!(
            !marker.exists(),
            "embedding adapter should not execute for context-mode packages"
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn query_rejects_vector_length_mismatch_before_search() {
        let root = temp_dir("query-vector-length-mismatch");
        write_vector_fixture(&root);
        let vector_path = root.join("query.json");
        fs::write(&vector_path, "[1.0,0.0]").unwrap();

        let err = query_knowledge(
            &root,
            &KnowledgeQueryArgs {
                target: ".".to_string(),
                query_text: None,
                top_k: None,
                score_threshold: None,
                json: false,
                include_text: false,
                include_metadata: false,
                vector_json: Some(vector_path.to_string_lossy().into_owned()),
                embedding_command: None,
            },
        )
        .unwrap_err();

        assert!(format!("{err:#}").contains("query vector length 2 does not match"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn query_rejects_provider_metadata_mismatch() {
        let root = temp_dir("query-provider-mismatch");
        write_vector_fixture(&root);
        let vector_path = root.join("query.json");
        fs::write(
            &vector_path,
            "{\"vector\":[1.0,0.0,0.0],\"provider\":\"voyage\"}",
        )
        .unwrap();

        let err = query_knowledge(
            &root,
            &KnowledgeQueryArgs {
                target: ".".to_string(),
                query_text: None,
                top_k: None,
                score_threshold: None,
                json: false,
                include_text: false,
                include_metadata: false,
                vector_json: Some(vector_path.to_string_lossy().into_owned()),
                embedding_command: None,
            },
        )
        .unwrap_err();

        assert!(format!("{err:#}").contains("provider `voyage` does not match"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn query_rejects_missing_index_metadata() {
        let root = temp_dir("query-missing-index-metadata");
        write_vector_fixture(&root);
        fs::remove_file(root.join("knowledge/indexes/default/metadata.json")).unwrap();
        let vector_path = root.join("query.json");
        fs::write(&vector_path, "[1.0,0.0,0.0]").unwrap();

        let err = query_knowledge(
            &root,
            &KnowledgeQueryArgs {
                target: ".".to_string(),
                query_text: None,
                top_k: None,
                score_threshold: None,
                json: false,
                include_text: false,
                include_metadata: false,
                vector_json: Some(vector_path.to_string_lossy().into_owned()),
                embedding_command: None,
            },
        )
        .unwrap_err();

        assert!(format!("{err:#}").contains("index metadata is missing"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn query_rejects_unsupported_index_algorithm() {
        let root = temp_dir("query-unsupported-algorithm");
        write_vector_fixture(&root);
        let metadata_path = root.join("knowledge/indexes/default/metadata.json");
        let mut metadata: Value =
            serde_json::from_str(&fs::read_to_string(&metadata_path).unwrap()).unwrap();
        metadata["algorithm"] = Value::String("ann".to_string());
        write_manifest_pretty_atomic(&metadata_path, &metadata).unwrap();
        let vector_path = root.join("query.json");
        fs::write(&vector_path, "[1.0,0.0,0.0]").unwrap();

        let err = query_knowledge(
            &root,
            &KnowledgeQueryArgs {
                target: ".".to_string(),
                query_text: None,
                top_k: None,
                score_threshold: None,
                json: false,
                include_text: false,
                include_metadata: false,
                vector_json: Some(vector_path.to_string_lossy().into_owned()),
                embedding_command: None,
            },
        )
        .unwrap_err();

        assert!(format!("{err:#}").contains("algorithm"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn query_rejects_unsupported_index_format_version() {
        let root = temp_dir("query-unsupported-format-version");
        write_vector_fixture(&root);
        let metadata_path = root.join("knowledge/indexes/default/metadata.json");
        let mut metadata: Value =
            serde_json::from_str(&fs::read_to_string(&metadata_path).unwrap()).unwrap();
        metadata["format_version"] = Value::Number(2.into());
        write_manifest_pretty_atomic(&metadata_path, &metadata).unwrap();
        let vector_path = root.join("query.json");
        fs::write(&vector_path, "[1.0,0.0,0.0]").unwrap();

        let err = query_knowledge(
            &root,
            &KnowledgeQueryArgs {
                target: ".".to_string(),
                query_text: None,
                top_k: None,
                score_threshold: None,
                json: false,
                include_text: false,
                include_metadata: false,
                vector_json: Some(vector_path.to_string_lossy().into_owned()),
                embedding_command: None,
            },
        )
        .unwrap_err();

        assert!(format!("{err:#}").contains("format_version"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn query_rejects_stale_corpus_hash() {
        let root = temp_dir("query-stale-corpus");
        write_vector_fixture(&root);
        fs::write(
            root.join("knowledge/chunks.jsonl"),
            concat!(
                "{\"id\":\"chunk_1\",\"source_id\":\"src_1\",\"text\":\"Alpha text changed\"}\n",
                "{\"id\":\"chunk_2\",\"source_id\":\"src_2\",\"text\":\"Beta text\",\"metadata\":{\"section\":\"beta\"}}\n",
                "{\"id\":\"chunk_3\",\"source_id\":\"src_2\",\"text\":\"Gamma text\"}\n"
            ),
        )
        .unwrap();
        let vector_path = root.join("query.json");
        fs::write(&vector_path, "[1.0,0.0,0.0]").unwrap();

        let err = query_knowledge(
            &root,
            &KnowledgeQueryArgs {
                target: ".".to_string(),
                query_text: None,
                top_k: None,
                score_threshold: None,
                json: false,
                include_text: false,
                include_metadata: false,
                vector_json: Some(vector_path.to_string_lossy().into_owned()),
                embedding_command: None,
            },
        )
        .unwrap_err();

        assert!(format!("{err:#}").contains("manifest build metadata is stale"));
        assert!(format!("{err:#}").contains("knowledge.corpus.content_hash"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn query_rejects_stale_vector_hash() {
        let root = temp_dir("query-stale-vectors");
        write_vector_fixture(&root);
        write_f32_vectors(
            &root.join("knowledge/embeddings/default.f32"),
            &[
                vec![0.9, 0.1, 0.0],
                vec![0.0, 1.0, 0.0],
                vec![0.5, 0.5, 0.0],
            ],
        );
        let vector_path = root.join("query.json");
        fs::write(&vector_path, "[1.0,0.0,0.0]").unwrap();

        let err = query_knowledge(
            &root,
            &KnowledgeQueryArgs {
                target: ".".to_string(),
                query_text: None,
                top_k: None,
                score_threshold: None,
                json: false,
                include_text: false,
                include_metadata: false,
                vector_json: Some(vector_path.to_string_lossy().into_owned()),
                embedding_command: None,
            },
        )
        .unwrap_err();

        assert!(format!("{err:#}").contains("knowledge.embedding.vectors_hash"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn query_rejects_normalized_flag_mismatch() {
        let root = temp_dir("query-normalized-mismatch");
        write_vector_fixture(&root);
        let metadata_path = root.join("knowledge/indexes/default/metadata.json");
        let mut metadata: Value =
            serde_json::from_str(&fs::read_to_string(&metadata_path).unwrap()).unwrap();
        metadata["normalized"] = Value::Bool(false);
        write_manifest_pretty_atomic(&metadata_path, &metadata).unwrap();
        let vector_path = root.join("query.json");
        fs::write(&vector_path, "[1.0,0.0,0.0]").unwrap();

        let err = query_knowledge(
            &root,
            &KnowledgeQueryArgs {
                target: ".".to_string(),
                query_text: None,
                top_k: None,
                score_threshold: None,
                json: false,
                include_text: false,
                include_metadata: false,
                vector_json: Some(vector_path.to_string_lossy().into_owned()),
                embedding_command: None,
            },
        )
        .unwrap_err();

        assert!(format!("{err:#}").contains("normalized"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn query_rejects_metric_mismatch() {
        let root = temp_dir("query-metric-mismatch");
        write_vector_fixture(&root);
        let metadata_path = root.join("knowledge/indexes/default/metadata.json");
        let mut metadata: Value =
            serde_json::from_str(&fs::read_to_string(&metadata_path).unwrap()).unwrap();
        metadata["metric"] = Value::String("euclidean".to_string());
        write_manifest_pretty_atomic(&metadata_path, &metadata).unwrap();
        let vector_path = root.join("query.json");
        fs::write(&vector_path, "[1.0,0.0,0.0]").unwrap();

        let err = query_knowledge(
            &root,
            &KnowledgeQueryArgs {
                target: ".".to_string(),
                query_text: None,
                top_k: None,
                score_threshold: None,
                json: false,
                include_text: false,
                include_metadata: false,
                vector_json: Some(vector_path.to_string_lossy().into_owned()),
                embedding_command: None,
            },
        )
        .unwrap_err();

        assert!(format!("{err:#}").contains("metric"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn query_text_without_vector_or_adapter_fails_clearly() {
        let root = temp_dir("query-text-no-adapter");
        write_vector_fixture(&root);

        let err = query_knowledge(
            &root,
            &KnowledgeQueryArgs {
                target: ".".to_string(),
                query_text: Some("what is alpha?".to_string()),
                top_k: None,
                score_threshold: None,
                json: false,
                include_text: false,
                include_metadata: false,
                vector_json: None,
                embedding_command: None,
            },
        )
        .unwrap_err();

        assert!(format!("{err:#}").contains("cannot embed text automatically yet"));
        assert!(format!("{err:#}").contains("Provide --vector-json"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn query_embedding_command_returns_ranked_rows() {
        let python = available_python().expect("python required for tests");
        let root = temp_dir("query-embedding-command-success");
        write_vector_fixture(&root);
        let script = write_adapter_script(
            &root,
            r#"import json, sys
payload = json.load(sys.stdin)
assert payload["text"] == "find alpha"
assert payload["embedding"]["provider"] == "openai"
assert payload["embedding"]["model"] == "text-embedding-3-small"
assert payload["embedding"]["dimensions"] == 3
json.dump({"vector": [1.0, 0.0, 0.0], "provider": "openai", "model": "text-embedding-3-small", "dimensions": 3}, sys.stdout)
"#,
        );
        let command = format!("{python} {}", quote_arg(&script.to_string_lossy()));

        let view = query_knowledge(
            &root,
            &KnowledgeQueryArgs {
                target: ".".to_string(),
                query_text: Some("find alpha".to_string()),
                top_k: Some(2),
                score_threshold: None,
                json: true,
                include_text: true,
                include_metadata: false,
                vector_json: None,
                embedding_command: Some(command),
            },
        )
        .unwrap();

        let results = view.json["results"].as_array().unwrap();
        assert_eq!(results[0]["chunk_id"], "chunk_1");
        assert_eq!(results[1]["chunk_id"], "chunk_3");

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn query_embedding_command_non_zero_exit_fails_clearly() {
        let python = available_python().expect("python required for tests");
        let root = temp_dir("query-embedding-command-fail");
        write_vector_fixture(&root);
        let script = write_adapter_script(
            &root,
            r#"import sys
sys.stderr.write("adapter boom\n")
raise SystemExit(7)
"#,
        );
        let command = format!("{python} {}", quote_arg(&script.to_string_lossy()));

        let err = query_knowledge(
            &root,
            &KnowledgeQueryArgs {
                target: ".".to_string(),
                query_text: Some("find alpha".to_string()),
                top_k: None,
                score_threshold: None,
                json: false,
                include_text: false,
                include_metadata: false,
                vector_json: None,
                embedding_command: Some(command),
            },
        )
        .unwrap_err();

        assert!(format!("{err:#}").contains("embedding adapter exited unsuccessfully"));
        assert!(format!("{err:#}").contains("adapter boom"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn query_embedding_command_invalid_json_stdout_fails_clearly() {
        let python = available_python().expect("python required for tests");
        let root = temp_dir("query-embedding-command-invalid-json");
        write_vector_fixture(&root);
        let script = write_adapter_script(
            &root,
            r#"import sys
sys.stdout.write("not json")
"#,
        );
        let command = format!("{python} {}", quote_arg(&script.to_string_lossy()));

        let err = query_knowledge(
            &root,
            &KnowledgeQueryArgs {
                target: ".".to_string(),
                query_text: Some("find alpha".to_string()),
                top_k: None,
                score_threshold: None,
                json: false,
                include_text: false,
                include_metadata: false,
                vector_json: None,
                embedding_command: Some(command),
            },
        )
        .unwrap_err();

        assert!(format!("{err:#}").contains("adapter stdout was not valid JSON"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn query_embedding_command_timeout_fails_clearly() {
        let python = available_python().expect("python required for tests");
        let root = temp_dir("query-embedding-command-timeout");
        write_vector_fixture(&root);
        let script = write_adapter_script(
            &root,
            r#"import time
time.sleep(30)
"#,
        );
        let command = format!("{python} {}", quote_arg(&script.to_string_lossy()));

        let err = query_knowledge(
            &root,
            &KnowledgeQueryArgs {
                target: ".".to_string(),
                query_text: Some("find alpha".to_string()),
                top_k: None,
                score_threshold: None,
                json: false,
                include_text: false,
                include_metadata: false,
                vector_json: None,
                embedding_command: Some(command),
            },
        )
        .unwrap_err();

        assert!(format!("{err:#}").contains("embedding adapter timed out"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn query_embedding_command_provider_mismatch_fails_clearly() {
        let python = available_python().expect("python required for tests");
        let root = temp_dir("query-embedding-command-provider-mismatch");
        write_vector_fixture(&root);
        let script = write_adapter_script(
            &root,
            r#"import json, sys
json.dump({"vector": [1.0, 0.0, 0.0], "provider": "voyage", "model": "voyage-code-3", "dimensions": 3}, sys.stdout)
"#,
        );
        let command = format!("{python} {}", quote_arg(&script.to_string_lossy()));

        let err = query_knowledge(
            &root,
            &KnowledgeQueryArgs {
                target: ".".to_string(),
                query_text: Some("find alpha".to_string()),
                top_k: None,
                score_threshold: None,
                json: false,
                include_text: false,
                include_metadata: false,
                vector_json: None,
                embedding_command: Some(command),
            },
        )
        .unwrap_err();

        assert!(format!("{err:#}").contains("provider `voyage` does not match"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn query_embedding_command_model_mismatch_fails_clearly() {
        let python = available_python().expect("python required for tests");
        let root = temp_dir("query-embedding-command-model-mismatch");
        write_vector_fixture(&root);
        let script = write_adapter_script(
            &root,
            r#"import json, sys
json.dump({"vector": [1.0, 0.0, 0.0], "provider": "openai", "model": "text-embedding-3-large", "dimensions": 3}, sys.stdout)
"#,
        );
        let command = format!("{python} {}", quote_arg(&script.to_string_lossy()));

        let err = query_knowledge(
            &root,
            &KnowledgeQueryArgs {
                target: ".".to_string(),
                query_text: Some("find alpha".to_string()),
                top_k: None,
                score_threshold: None,
                json: false,
                include_text: false,
                include_metadata: false,
                vector_json: None,
                embedding_command: Some(command),
            },
        )
        .unwrap_err();

        assert!(format!("{err:#}").contains("model `text-embedding-3-large` does not match"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn query_embedding_command_dimensions_mismatch_fails_clearly() {
        let python = available_python().expect("python required for tests");
        let root = temp_dir("query-embedding-command-dimensions-mismatch");
        write_vector_fixture(&root);
        let script = write_adapter_script(
            &root,
            r#"import json, sys
json.dump({"vector": [1.0, 0.0, 0.0], "provider": "openai", "model": "text-embedding-3-small", "dimensions": 4}, sys.stdout)
"#,
        );
        let command = format!("{python} {}", quote_arg(&script.to_string_lossy()));

        let err = query_knowledge(
            &root,
            &KnowledgeQueryArgs {
                target: ".".to_string(),
                query_text: Some("find alpha".to_string()),
                top_k: None,
                score_threshold: None,
                json: false,
                include_text: false,
                include_metadata: false,
                vector_json: None,
                embedding_command: Some(command),
            },
        )
        .unwrap_err();

        assert!(format!("{err:#}").contains("metadata dimensions 4 does not match"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn query_embedding_command_requires_query_text() {
        let root = temp_dir("query-embedding-command-no-text");
        write_vector_fixture(&root);

        let err = query_knowledge(
            &root,
            &KnowledgeQueryArgs {
                target: ".".to_string(),
                query_text: None,
                top_k: None,
                score_threshold: None,
                json: false,
                include_text: false,
                include_metadata: false,
                vector_json: None,
                embedding_command: Some("python3 adapter.py".to_string()),
            },
        )
        .unwrap_err();

        assert!(format!("{err:#}").contains("requires query text input"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn parse_adapter_command_line_preserves_empty_quoted_args() {
        let parsed =
            parse_adapter_command_line(r#"python3 script.py --flag "" '' "value with spaces""#)
                .unwrap();

        assert_eq!(
            parsed,
            vec![
                "python3".to_string(),
                "script.py".to_string(),
                "--flag".to_string(),
                String::new(),
                String::new(),
                "value with spaces".to_string(),
            ]
        );
    }
}
