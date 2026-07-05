use crate::manifest::{
    KnowledgeManifest, LintIssue, load_manifest_value, parse_knowledge_manifest,
    resolve_schema_source, validate_manifest_value, write_manifest_pretty_atomic,
};
use crate::prelude::*;
use anyhow::{Context, anyhow, bail};
use chrono::{SecondsFormat, Utc};
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::fs;
use std::path::{Component, Path, PathBuf};

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
}

#[derive(Args, Debug, Clone)]
pub struct KnowledgeQueryArgs {
    #[arg(value_name = "PATH_OR_PACKAGE")]
    pub target: String,
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
}

#[derive(Debug, Clone)]
struct SourceRecord {
    id: String,
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
            KnowledgeCmd::Inspect(_) => {
                bail!("`agentpm knowledge inspect` is not implemented yet")
            }
            KnowledgeCmd::Query(_) => {
                bail!("`agentpm knowledge query` is not implemented yet")
            }
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

        let _ = text_value;
        chunks.push(ChunkRecord { id, source_id });
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

        sources.push(SourceRecord { id });
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
}
