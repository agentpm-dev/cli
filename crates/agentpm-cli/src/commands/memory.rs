use crate::manifest::{
    LintIssue, MemoryManifest, MemorySpaceModel, load_manifest_value, parse_memory_manifest,
    resolve_existing_relative_file, resolve_schema_source, validate_manifest_value,
    write_manifest_pretty_atomic,
};
use crate::prelude::*;
use anyhow::{Context, anyhow, bail};
use chrono::{SecondsFormat, Utc};
use jsonschema::{Draft, JSONSchema};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const MEMORY_BUILD_METADATA_TYPE: &str = "agentpm-memory-contracts";
const MEMORY_BUILD_METADATA_FORMAT_VERSION: u64 = 1;
const MEMORY_CONTRACT_INDEX_TYPE: &str = "agentpm-memory-contract-index";
const MEMORY_CONTRACT_INDEX_FORMAT_VERSION: u64 = 1;

#[derive(Args, Debug)]
pub struct MemoryArgs {
    #[command(subcommand)]
    pub command: MemoryCmd,
}

#[derive(Subcommand, Debug)]
pub enum MemoryCmd {
    /// Validate and generate resolved contracts for a Memory Blueprint
    Build(MemoryBuildArgs),
}

#[derive(Args, Debug, Clone)]
pub struct MemoryBuildArgs {
    /// Path to the Memory manifest to build
    #[arg(long, default_value = "agent.json")]
    pub manifest: PathBuf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub(crate) enum MemoryBuildMode {
    Check,
    Write,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct MemoryContractIndexEntry {
    pub space: String,
    pub record_type: String,
    pub schema_version: String,
    pub model: String,
    pub source_schema: String,
    pub path: String,
    pub sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct MemoryContractIndex {
    pub r#type: String,
    pub format_version: u64,
    pub contracts: Vec<MemoryContractIndexEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GeneratedMemoryContract {
    pub path: String,
    pub sha256: String,
    pub schema_bytes: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GeneratedMemorySourceSchema {
    pub path: String,
    pub sha256: String,
    pub bytes: Vec<u8>,
    pub canonical_bytes: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GeneratedMemoryBuild {
    pub source_schemas: Vec<GeneratedMemorySourceSchema>,
    pub index: MemoryContractIndex,
    pub index_bytes: Vec<u8>,
    pub contracts: Vec<GeneratedMemoryContract>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct MemoryBuildSourceSchemaEntry {
    pub path: String,
    pub sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct MemoryBuildMetadata {
    pub r#type: String,
    pub format_version: u64,
    pub built_at: String,
    pub agentpm_version: String,
    pub manifest_path: String,
    pub source_manifest_hash: String,
    pub source_schemas: Vec<MemoryBuildSourceSchemaEntry>,
    pub source_schemas_hash: String,
    pub source_contract_inputs_hash: String,
    pub contracts_index_hash: String,
    pub contracts_hash: String,
    pub contract_count: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum MemoryBuildMismatchKind {
    MissingBuild,
    UnsupportedFormat,
    StaleSourceInput,
    MissingOutput,
    ModifiedOutput,
    UnexpectedOutput,
    InconsistentMetadata,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MemoryBuildMismatch {
    pub kind: MemoryBuildMismatchKind,
    pub path: String,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MemoryBuildCheckResult {
    pub mismatches: Vec<MemoryBuildMismatch>,
}

#[allow(dead_code)]
#[derive(Debug)]
pub(crate) struct ExecutedMemoryBuild {
    pub manifest: MemoryManifest,
    pub summary: MemoryBuildSummary,
    pub output: GeneratedMemoryBuild,
    pub build_metadata: MemoryBuildMetadata,
    pub check: Option<MemoryBuildCheckResult>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MemoryBuildSummary {
    pub name: String,
    pub version: String,
    pub scope_count: usize,
    pub record_type_count: usize,
    pub space_count: usize,
    pub operation_count: usize,
    pub contract_count: usize,
    pub contracts_dir: String,
}

impl MemoryArgs {
    pub async fn run(self) -> Result<()> {
        match self.command {
            MemoryCmd::Build(args) => args.run().await,
        }
    }
}

impl MemoryBuildArgs {
    pub async fn run(self) -> Result<()> {
        let manifest_path = resolve_manifest_path(&self.manifest)?;
        let summary = execute_memory_build(&manifest_path, MemoryBuildMode::Write)?;
        print_build_summary(&summary);
        Ok(())
    }
}

pub(crate) fn execute_memory_build(
    manifest_path: &Path,
    mode: MemoryBuildMode,
) -> Result<MemoryBuildSummary> {
    Ok(execute_memory_build_with_output(manifest_path, mode)?.summary)
}

pub(crate) fn execute_memory_build_with_output(
    manifest_path: &Path,
    mode: MemoryBuildMode,
) -> Result<ExecutedMemoryBuild> {
    let manifest_path = resolve_manifest_path(manifest_path)?;
    let package_root = manifest_path
        .parent()
        .ok_or_else(|| anyhow!("manifest path has no parent: {}", manifest_path.display()))?
        .to_path_buf();

    let (mut manifest_value, _) = load_manifest_value(&manifest_path)?;
    validate_manifest_or_bail(&manifest_path, &mut manifest_value)?;

    let kind = manifest_value
        .get("kind")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("manifest must include kind"))?;
    if kind != "memory" {
        bail!(
            "`agentpm memory build` requires kind=\"memory\" (got kind=\"{}\")",
            kind
        );
    }

    let manifest = parse_memory_manifest(&manifest_value)?;
    let output = generate_memory_build(&package_root, &manifest)?;
    let build_metadata = generate_memory_build_metadata(&manifest_path, &manifest_value, &output)?;
    let check = if mode == MemoryBuildMode::Check {
        Some(check_memory_build_freshness(
            &package_root,
            &output,
            &build_metadata,
        )?)
    } else {
        None
    };

    if mode == MemoryBuildMode::Write {
        write_generated_memory_build(&package_root, &output, &build_metadata)?;
    }

    let summary = MemoryBuildSummary {
        name: manifest.name.clone(),
        version: manifest.version.clone(),
        scope_count: manifest.memory.scopes.len(),
        record_type_count: manifest.memory.record_types.len(),
        space_count: manifest.memory.spaces.len(),
        operation_count: manifest.memory.operations.len(),
        contract_count: output.contracts.len(),
        contracts_dir: "memory/contracts".to_string(),
    };

    Ok(ExecutedMemoryBuild {
        manifest,
        summary,
        output,
        build_metadata,
        check,
    })
}

fn print_build_summary(summary: &MemoryBuildSummary) {
    println!(
        "Memory build complete: {}@{}",
        summary.name, summary.version
    );
    println!("Scopes: {}", summary.scope_count);
    println!("Record types: {}", summary.record_type_count);
    println!("Spaces: {}", summary.space_count);
    println!("Operations: {}", summary.operation_count);
    println!("Contracts: {}", summary.contract_count);
    println!("Output: {}", summary.contracts_dir);
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

fn generate_memory_build(
    package_root: &Path,
    manifest: &MemoryManifest,
) -> Result<GeneratedMemoryBuild> {
    let mut contracts = Vec::new();
    let mut index_entries = Vec::new();
    let mut source_schemas = BTreeMap::new();
    let mut seen_paths = BTreeSet::new();
    let mut seen_identities = BTreeSet::new();

    let mut space_keys = manifest.memory.spaces.keys().cloned().collect::<Vec<_>>();
    space_keys.sort();

    for space_key in space_keys {
        let space = manifest
            .memory
            .spaces
            .get(&space_key)
            .ok_or_else(|| anyhow!("space `{space_key}` missing during generation"))?;

        let mut record_types = space.record_types.clone();
        record_types.sort();

        for record_type_key in record_types {
            let record_type = manifest
                .memory
                .record_types
                .get(&record_type_key)
                .ok_or_else(|| {
                    anyhow!("record type `{record_type_key}` missing during generation")
                })?;

            let identity = format!("{space_key}\0{record_type_key}");
            if !seen_identities.insert(identity) {
                bail!(
                    "duplicate generated contract identity for space `{space_key}` and record type `{record_type_key}`"
                );
            }

            let file_name = format!("{space_key}.{record_type_key}.schema.json");
            let contract_path = format!("memory/contracts/{file_name}");
            if !seen_paths.insert(contract_path.clone()) {
                bail!("duplicate generated contract path `{contract_path}`");
            }

            let source_schema_path =
                resolve_existing_relative_file(package_root, &record_type.schema)?;
            let source_schema_bytes = fs::read(&source_schema_path)
                .with_context(|| format!("reading {}", source_schema_path.display()))?;
            let source_schema: Value = serde_json::from_slice(&source_schema_bytes)
                .with_context(|| format!("parsing JSON from {}", source_schema_path.display()))?;
            source_schemas
                .entry(record_type.schema.clone())
                .or_insert_with(|| GeneratedMemorySourceSchema {
                    path: record_type.schema.clone(),
                    sha256: sha256_prefixed(&source_schema_bytes),
                    canonical_bytes: canonical_json_bytes(&source_schema),
                    bytes: source_schema_bytes.clone(),
                });

            let contract = generate_contract_schema(
                manifest,
                &space_key,
                space,
                &record_type_key,
                record_type.version.as_str(),
                &source_schema,
            )?;

            JSONSchema::options()
                .with_draft(Draft::Draft202012)
                .compile(&contract)
                .map_err(|err| {
                    anyhow!(
                        "generated contract for space `{space_key}` and record type `{record_type_key}` is not valid JSON Schema Draft 2020-12: {err}"
                    )
                })?;

            let schema_bytes = pretty_json_bytes(&contract)?;
            let contract_sha256 = sha256_prefixed(&schema_bytes);
            contracts.push(GeneratedMemoryContract {
                path: contract_path.clone(),
                sha256: contract_sha256.clone(),
                schema_bytes,
            });
            index_entries.push(MemoryContractIndexEntry {
                space: space_key.clone(),
                record_type: record_type_key,
                schema_version: record_type.version.clone(),
                model: memory_space_model_name(&space.model).to_string(),
                source_schema: record_type.schema.clone(),
                path: contract_path,
                sha256: contract_sha256,
            });
        }
    }

    index_entries.sort_by(|a, b| {
        a.space
            .cmp(&b.space)
            .then(a.record_type.cmp(&b.record_type))
    });
    contracts.sort_by(|a, b| a.path.cmp(&b.path));

    let index = MemoryContractIndex {
        r#type: MEMORY_CONTRACT_INDEX_TYPE.to_string(),
        format_version: MEMORY_CONTRACT_INDEX_FORMAT_VERSION,
        contracts: index_entries,
    };
    let index_bytes = pretty_json_bytes(&serde_json::to_value(&index)?)?;

    Ok(GeneratedMemoryBuild {
        source_schemas: source_schemas.into_values().collect(),
        index,
        index_bytes,
        contracts,
    })
}

fn generate_contract_schema(
    manifest: &MemoryManifest,
    space_key: &str,
    space: &crate::manifest::MemorySpace,
    record_type_key: &str,
    schema_version: &str,
    source_schema: &Value,
) -> Result<Value> {
    let content_schema = embed_content_schema(source_schema);
    let mut scope_properties = Map::new();
    let mut scope_required = Vec::new();
    for scope_key in &space.scope {
        scope_properties.insert(
            scope_key.clone(),
            json!({
                "type": "string",
                "minLength": 1
            }),
        );
        scope_required.push(Value::String(scope_key.clone()));
    }

    let mut provenance_properties = Map::new();
    provenance_properties.insert(
        "source_record_ids".into(),
        json!({
            "type": "array",
            "uniqueItems": true,
            "items": {
                "type": "string",
                "minLength": 1
            }
        }),
    );
    if !manifest.memory.operations.is_empty() {
        let mut operation_keys = manifest
            .memory
            .operations
            .keys()
            .cloned()
            .collect::<Vec<_>>();
        operation_keys.sort();
        provenance_properties.insert(
            "operation".into(),
            json!({
                "type": "string",
                "enum": operation_keys
            }),
        );
    }

    let scope_schema = json!({
        "type": "object",
        "additionalProperties": false,
        "required": scope_required,
        "properties": scope_properties
    });

    let mut properties = Map::new();
    properties.insert(
        "id".into(),
        json!({
            "type": "string",
            "minLength": 1
        }),
    );
    properties.insert("record_type".into(), json!({ "const": record_type_key }));
    properties.insert("space".into(), json!({ "const": space_key }));
    properties.insert("scope".into(), scope_schema);
    properties.insert("schema_version".into(), json!({ "const": schema_version }));
    properties.insert(
        "created_at".into(),
        json!({
            "type": "string",
            "format": "date-time"
        }),
    );
    properties.insert(
        "updated_at".into(),
        json!({
            "type": "string",
            "format": "date-time"
        }),
    );
    properties.insert(
        "expires_at".into(),
        json!({
            "type": "string",
            "format": "date-time"
        }),
    );
    if matches!(space.model, MemorySpaceModel::Sequence) {
        properties.insert(
            "ordinal".into(),
            json!({
                "type": "integer",
                "minimum": 0
            }),
        );
    }
    properties.insert(
        "provenance".into(),
        json!({
            "type": "object",
            "additionalProperties": false,
            "properties": provenance_properties
        }),
    );
    properties.insert("content".into(), content_schema);

    let mut required = vec![
        Value::String("id".into()),
        Value::String("record_type".into()),
        Value::String("space".into()),
        Value::String("scope".into()),
        Value::String("schema_version".into()),
        Value::String("created_at".into()),
        Value::String("content".into()),
    ];
    if matches!(space.model, MemorySpaceModel::Sequence) {
        required.push(Value::String("ordinal".into()));
    }

    Ok(json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$id": format!(
            "agentpm://memory/{}/{}/{}/{}/{}",
            encode_uri_component(&manifest.name),
            encode_uri_component(&manifest.version),
            encode_uri_component(space_key),
            encode_uri_component(record_type_key),
            encode_uri_component(schema_version),
        ),
        "title": format!("{} {} {} record contract", manifest.name, space_key, record_type_key),
        "description": format!(
            "Resolved Memory record contract for package `{}` space `{}` and record type `{}`.",
            manifest.name, space_key, record_type_key
        ),
        "type": "object",
        "additionalProperties": false,
        "required": required,
        "properties": properties
    }))
}

fn embed_content_schema(source_schema: &Value) -> Value {
    match source_schema {
        Value::Object(map) if !map.contains_key("$id") => {
            let mut cloned = map.clone();
            cloned.insert("$id".into(), Value::String("content".into()));
            Value::Object(cloned)
        }
        _ => source_schema.clone(),
    }
}

fn pretty_json_bytes(value: &impl Serialize) -> Result<Vec<u8>> {
    let mut bytes = serde_json::to_vec_pretty(value)?;
    if !bytes.ends_with(b"\n") {
        bytes.push(b'\n');
    }
    Ok(bytes)
}

fn canonical_json_bytes(value: &Value) -> Vec<u8> {
    let mut out = Vec::new();
    write_canonical_json(value, &mut out);
    out
}

fn write_canonical_json(value: &Value, out: &mut Vec<u8>) {
    match value {
        Value::Null => out.extend_from_slice(b"null"),
        Value::Bool(value) => {
            if *value {
                out.extend_from_slice(b"true");
            } else {
                out.extend_from_slice(b"false");
            }
        }
        Value::Number(value) => out.extend_from_slice(value.to_string().as_bytes()),
        Value::String(value) => out.extend_from_slice(
            serde_json::to_string(value)
                .expect("serializing JSON string")
                .as_bytes(),
        ),
        Value::Array(values) => {
            out.push(b'[');
            for (idx, item) in values.iter().enumerate() {
                if idx > 0 {
                    out.push(b',');
                }
                write_canonical_json(item, out);
            }
            out.push(b']');
        }
        Value::Object(map) => {
            out.push(b'{');
            let mut entries = map.iter().collect::<Vec<_>>();
            entries.sort_by(|(a, _), (b, _)| a.cmp(b));
            for (idx, (key, item)) in entries.into_iter().enumerate() {
                if idx > 0 {
                    out.push(b',');
                }
                out.extend_from_slice(
                    serde_json::to_string(key)
                        .expect("serializing JSON object key")
                        .as_bytes(),
                );
                out.push(b':');
                write_canonical_json(item, out);
            }
            out.push(b'}');
        }
    }
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

fn generate_memory_build_metadata(
    manifest_path: &Path,
    manifest_value: &Value,
    output: &GeneratedMemoryBuild,
) -> Result<MemoryBuildMetadata> {
    let manifest_bytes =
        fs::read(manifest_path).with_context(|| format!("reading {}", manifest_path.display()))?;
    let manifest_sha256 = sha256_prefixed(&manifest_bytes);
    let memory_value = manifest_value
        .get("memory")
        .ok_or_else(|| anyhow!("manifest is missing top-level memory object"))?;
    let memory_canonical = canonical_json_bytes(memory_value);

    let source_schemas_hash = aggregate_named_bytes(
        &output
            .source_schemas
            .iter()
            .map(|source| (source.path.as_str(), source.bytes.as_slice()))
            .collect::<Vec<_>>(),
    );
    let source_schemas = output
        .source_schemas
        .iter()
        .map(|source| MemoryBuildSourceSchemaEntry {
            path: source.path.clone(),
            sha256: source.sha256.clone(),
        })
        .collect::<Vec<_>>();

    let mut contract_input_entries = Vec::with_capacity(output.source_schemas.len() + 1);
    contract_input_entries.push(("memory", memory_canonical.as_slice()));
    let source_canonical_entries = output
        .source_schemas
        .iter()
        .map(|source| (source.path.as_str(), source.canonical_bytes.as_slice()))
        .collect::<Vec<_>>();
    contract_input_entries.extend(source_canonical_entries);
    let source_contract_inputs_hash = aggregate_named_bytes(&contract_input_entries);

    let contracts_index_hash = sha256_prefixed(&output.index_bytes);
    let contracts_hash = aggregate_named_bytes(
        &output
            .contracts
            .iter()
            .map(|contract| (contract.path.as_str(), contract.schema_bytes.as_slice()))
            .collect::<Vec<_>>(),
    );

    Ok(MemoryBuildMetadata {
        r#type: MEMORY_BUILD_METADATA_TYPE.to_string(),
        format_version: MEMORY_BUILD_METADATA_FORMAT_VERSION,
        built_at: Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true),
        agentpm_version: env!("CARGO_PKG_VERSION").to_string(),
        manifest_path: "agent.json".to_string(),
        source_manifest_hash: manifest_sha256,
        source_schemas,
        source_schemas_hash,
        source_contract_inputs_hash,
        contracts_index_hash,
        contracts_hash,
        contract_count: output.contracts.len() as u64,
    })
}

fn check_memory_build_freshness(
    package_root: &Path,
    output: &GeneratedMemoryBuild,
    expected_metadata: &MemoryBuildMetadata,
) -> Result<MemoryBuildCheckResult> {
    let mut mismatches = Vec::new();
    let memory_dir = package_root.join("memory");
    let build_path = memory_dir.join("build.json");
    let contracts_dir = memory_dir.join("contracts");
    let index_path = contracts_dir.join("index.json");
    let expected_contracts = output
        .contracts
        .iter()
        .map(|contract| (contract.path.clone(), contract))
        .collect::<BTreeMap<_, _>>();

    let build_metadata = load_memory_build_metadata(&build_path, &mut mismatches)?;

    if let Some(metadata) = build_metadata.as_ref() {
        compare_build_metadata(metadata, expected_metadata, &mut mismatches);
    }

    if !contracts_dir.exists() {
        mismatches.push(MemoryBuildMismatch {
            kind: MemoryBuildMismatchKind::MissingOutput,
            path: "memory/contracts".to_string(),
            detail: "generated contracts directory is missing".to_string(),
        });
        return Ok(MemoryBuildCheckResult { mismatches });
    }

    let actual_index = load_contract_index(&index_path, &mut mismatches)?;
    let actual_index_hash = fs::read(&index_path)
        .ok()
        .map(|bytes| sha256_prefixed(&bytes));
    if let (Some(actual_hash), Some(metadata)) =
        (actual_index_hash.as_ref(), build_metadata.as_ref())
        && metadata.contracts_index_hash != *actual_hash
    {
        mismatches.push(MemoryBuildMismatch {
            kind: MemoryBuildMismatchKind::InconsistentMetadata,
            path: "memory/build.json".to_string(),
            detail: "contracts_index_hash does not match memory/contracts/index.json".to_string(),
        });
    }

    if let Some(index) = actual_index.as_ref() {
        if index.contracts.len() as u64 != expected_metadata.contract_count {
            mismatches.push(MemoryBuildMismatch {
                kind: MemoryBuildMismatchKind::InconsistentMetadata,
                path: "memory/contracts/index.json".to_string(),
                detail: format!(
                    "contract count {} does not match expected {}",
                    index.contracts.len(),
                    expected_metadata.contract_count
                ),
            });
        }
        if index.contracts != output.index.contracts {
            mismatches.push(MemoryBuildMismatch {
                kind: MemoryBuildMismatchKind::ModifiedOutput,
                path: "memory/contracts/index.json".to_string(),
                detail: "contract index entries do not match the expected generated output"
                    .to_string(),
            });
        }
    }

    let mut actual_contract_hash_entries = Vec::new();
    for (contract_path, expected_contract) in &expected_contracts {
        let relative = contract_path
            .strip_prefix("memory/contracts/")
            .unwrap_or(contract_path.as_str());
        let file_path = contracts_dir.join(relative);
        if !file_path.exists() {
            mismatches.push(MemoryBuildMismatch {
                kind: MemoryBuildMismatchKind::MissingOutput,
                path: contract_path.clone(),
                detail: "generated contract file is missing".to_string(),
            });
            continue;
        }
        let bytes =
            fs::read(&file_path).with_context(|| format!("reading {}", file_path.display()))?;
        let actual_hash = sha256_prefixed(&bytes);
        actual_contract_hash_entries.push((contract_path.as_str(), bytes));
        if actual_hash != expected_contract.sha256 {
            mismatches.push(MemoryBuildMismatch {
                kind: MemoryBuildMismatchKind::ModifiedOutput,
                path: contract_path.clone(),
                detail: format!(
                    "generated contract hash {} does not match expected {}",
                    actual_hash, expected_contract.sha256
                ),
            });
        }
    }

    let actual_generated_contracts_hash = aggregate_named_bytes(
        &actual_contract_hash_entries
            .iter()
            .map(|(path, bytes)| (*path, bytes.as_slice()))
            .collect::<Vec<_>>(),
    );
    if let Some(metadata) = build_metadata.as_ref()
        && metadata.contracts_hash != actual_generated_contracts_hash
    {
        mismatches.push(MemoryBuildMismatch {
            kind: MemoryBuildMismatchKind::InconsistentMetadata,
            path: "memory/build.json".to_string(),
            detail: "contracts_hash does not match the current generated contracts".to_string(),
        });
    }

    let expected_files = expected_contracts
        .keys()
        .map(|path| {
            path.strip_prefix("memory/contracts/")
                .unwrap_or(path.as_str())
                .to_string()
        })
        .chain(std::iter::once("index.json".to_string()))
        .collect::<BTreeSet<_>>();
    for entry in fs::read_dir(&contracts_dir)
        .with_context(|| format!("reading {}", contracts_dir.display()))?
    {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().to_string();
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            mismatches.push(MemoryBuildMismatch {
                kind: MemoryBuildMismatchKind::UnexpectedOutput,
                path: format!("memory/contracts/{name}"),
                detail: "unexpected generated subdirectory".to_string(),
            });
            continue;
        }
        if !expected_files.contains(&name) {
            mismatches.push(MemoryBuildMismatch {
                kind: MemoryBuildMismatchKind::UnexpectedOutput,
                path: format!("memory/contracts/{name}"),
                detail: "unexpected generated file".to_string(),
            });
        }
    }

    Ok(MemoryBuildCheckResult { mismatches })
}

fn load_memory_build_metadata(
    build_path: &Path,
    mismatches: &mut Vec<MemoryBuildMismatch>,
) -> Result<Option<MemoryBuildMetadata>> {
    if !build_path.exists() {
        mismatches.push(MemoryBuildMismatch {
            kind: MemoryBuildMismatchKind::MissingBuild,
            path: "memory/build.json".to_string(),
            detail: "build metadata file is missing".to_string(),
        });
        return Ok(None);
    }
    let bytes =
        fs::read(build_path).with_context(|| format!("reading {}", build_path.display()))?;
    let metadata: MemoryBuildMetadata = match serde_json::from_slice(&bytes) {
        Ok(value) => value,
        Err(err) => {
            mismatches.push(MemoryBuildMismatch {
                kind: MemoryBuildMismatchKind::InconsistentMetadata,
                path: "memory/build.json".to_string(),
                detail: format!("invalid build metadata JSON: {err}"),
            });
            return Ok(None);
        }
    };
    if metadata.r#type != MEMORY_BUILD_METADATA_TYPE {
        mismatches.push(MemoryBuildMismatch {
            kind: MemoryBuildMismatchKind::UnsupportedFormat,
            path: "memory/build.json".to_string(),
            detail: format!("unsupported build metadata type `{}`", metadata.r#type),
        });
    }
    if metadata.format_version != MEMORY_BUILD_METADATA_FORMAT_VERSION {
        mismatches.push(MemoryBuildMismatch {
            kind: MemoryBuildMismatchKind::UnsupportedFormat,
            path: "memory/build.json".to_string(),
            detail: format!(
                "unsupported build metadata format_version `{}`",
                metadata.format_version
            ),
        });
    }
    Ok(Some(metadata))
}

fn load_contract_index(
    index_path: &Path,
    mismatches: &mut Vec<MemoryBuildMismatch>,
) -> Result<Option<MemoryContractIndex>> {
    if !index_path.exists() {
        mismatches.push(MemoryBuildMismatch {
            kind: MemoryBuildMismatchKind::MissingOutput,
            path: "memory/contracts/index.json".to_string(),
            detail: "generated contract index is missing".to_string(),
        });
        return Ok(None);
    }
    let bytes =
        fs::read(index_path).with_context(|| format!("reading {}", index_path.display()))?;
    let index: MemoryContractIndex = match serde_json::from_slice(&bytes) {
        Ok(value) => value,
        Err(err) => {
            mismatches.push(MemoryBuildMismatch {
                kind: MemoryBuildMismatchKind::ModifiedOutput,
                path: "memory/contracts/index.json".to_string(),
                detail: format!("invalid contract index JSON: {err}"),
            });
            return Ok(None);
        }
    };
    if index.r#type != MEMORY_CONTRACT_INDEX_TYPE {
        mismatches.push(MemoryBuildMismatch {
            kind: MemoryBuildMismatchKind::UnsupportedFormat,
            path: "memory/contracts/index.json".to_string(),
            detail: format!("unsupported contract index type `{}`", index.r#type),
        });
    }
    if index.format_version != MEMORY_CONTRACT_INDEX_FORMAT_VERSION {
        mismatches.push(MemoryBuildMismatch {
            kind: MemoryBuildMismatchKind::UnsupportedFormat,
            path: "memory/contracts/index.json".to_string(),
            detail: format!(
                "unsupported contract index format_version `{}`",
                index.format_version
            ),
        });
    }
    Ok(Some(index))
}

fn compare_build_metadata(
    actual: &MemoryBuildMetadata,
    expected: &MemoryBuildMetadata,
    mismatches: &mut Vec<MemoryBuildMismatch>,
) {
    if actual.manifest_path != expected.manifest_path {
        mismatches.push(MemoryBuildMismatch {
            kind: MemoryBuildMismatchKind::InconsistentMetadata,
            path: "memory/build.json".to_string(),
            detail: "manifest_path is incorrect".to_string(),
        });
    }
    if actual.source_manifest_hash != expected.source_manifest_hash {
        mismatches.push(MemoryBuildMismatch {
            kind: MemoryBuildMismatchKind::StaleSourceInput,
            path: "agent.json".to_string(),
            detail: "source_manifest_hash does not match the current manifest bytes".to_string(),
        });
    }
    compare_source_schema_entries(&actual.source_schemas, &expected.source_schemas, mismatches);
    if actual.source_schemas_hash != expected.source_schemas_hash {
        mismatches.push(MemoryBuildMismatch {
            kind: MemoryBuildMismatchKind::StaleSourceInput,
            path: "memory/build.json".to_string(),
            detail: "source_schemas_hash does not match the current source schema contents"
                .to_string(),
        });
    }
    if actual.source_contract_inputs_hash != expected.source_contract_inputs_hash {
        mismatches.push(MemoryBuildMismatch {
            kind: MemoryBuildMismatchKind::StaleSourceInput,
            path: "memory/build.json".to_string(),
            detail: "source_contract_inputs_hash does not match the current contract inputs"
                .to_string(),
        });
    }
    if actual.contract_count != expected.contract_count {
        mismatches.push(MemoryBuildMismatch {
            kind: MemoryBuildMismatchKind::InconsistentMetadata,
            path: "memory/build.json".to_string(),
            detail: format!(
                "contract_count {} does not match expected {}",
                actual.contract_count, expected.contract_count
            ),
        });
    }
    if actual.contracts_index_hash != expected.contracts_index_hash {
        mismatches.push(MemoryBuildMismatch {
            kind: MemoryBuildMismatchKind::InconsistentMetadata,
            path: "memory/build.json".to_string(),
            detail: "contracts_index_hash does not match the expected contract index bytes"
                .to_string(),
        });
    }
    if actual.contracts_hash != expected.contracts_hash {
        mismatches.push(MemoryBuildMismatch {
            kind: MemoryBuildMismatchKind::InconsistentMetadata,
            path: "memory/build.json".to_string(),
            detail: "contracts_hash does not match the expected generated contracts".to_string(),
        });
    }
}

fn compare_source_schema_entries(
    actual: &[MemoryBuildSourceSchemaEntry],
    expected: &[MemoryBuildSourceSchemaEntry],
    mismatches: &mut Vec<MemoryBuildMismatch>,
) {
    let actual_map = actual
        .iter()
        .map(|entry| (entry.path.as_str(), entry.sha256.as_str()))
        .collect::<BTreeMap<_, _>>();
    let expected_map = expected
        .iter()
        .map(|entry| (entry.path.as_str(), entry.sha256.as_str()))
        .collect::<BTreeMap<_, _>>();

    for expected_entry in expected {
        match actual_map.get(expected_entry.path.as_str()) {
            None => mismatches.push(MemoryBuildMismatch {
                kind: MemoryBuildMismatchKind::StaleSourceInput,
                path: expected_entry.path.clone(),
                detail: "source schema entry is missing from build metadata".to_string(),
            }),
            Some(actual_sha) if *actual_sha != expected_entry.sha256 => {
                mismatches.push(MemoryBuildMismatch {
                    kind: MemoryBuildMismatchKind::StaleSourceInput,
                    path: expected_entry.path.clone(),
                    detail: format!(
                        "source schema sha256 {} does not match expected {}",
                        actual_sha, expected_entry.sha256
                    ),
                });
            }
            _ => {}
        }
    }
    for actual_entry in actual {
        if !expected_map.contains_key(actual_entry.path.as_str()) {
            mismatches.push(MemoryBuildMismatch {
                kind: MemoryBuildMismatchKind::InconsistentMetadata,
                path: actual_entry.path.clone(),
                detail: "unexpected source schema entry in build metadata".to_string(),
            });
        }
    }
}

fn memory_space_model_name(model: &MemorySpaceModel) -> &'static str {
    match model {
        MemorySpaceModel::Document => "document",
        MemorySpaceModel::Collection => "collection",
        MemorySpaceModel::Sequence => "sequence",
    }
}

fn encode_uri_component(value: &str) -> String {
    let mut out = String::new();
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char)
            }
            _ => out.push_str(&format!("%{:02X}", byte)),
        }
    }
    out
}

fn write_generated_memory_build(
    package_root: &Path,
    output: &GeneratedMemoryBuild,
    build_metadata: &MemoryBuildMetadata,
) -> Result<()> {
    let memory_dir = package_root.join("memory");
    fs::create_dir_all(&memory_dir)
        .with_context(|| format!("creating {}", memory_dir.display()))?;

    let contracts_dir = memory_dir.join("contracts");
    let stage_dir = memory_dir.join(format!(".contracts-stage-{}", unique_suffix()));
    fs::create_dir_all(&stage_dir).with_context(|| format!("creating {}", stage_dir.display()))?;

    write_bytes_atomic(&stage_dir.join("index.json"), &output.index_bytes)?;

    for contract in &output.contracts {
        let filename = Path::new(&contract.path).file_name().ok_or_else(|| {
            anyhow!(
                "generated contract path has no file name: {}",
                contract.path
            )
        })?;
        let file_path = stage_dir.join(filename);
        write_bytes_atomic(&file_path, &contract.schema_bytes)?;
    }

    replace_dir_atomically(&stage_dir, &contracts_dir)?;
    write_manifest_pretty_atomic(
        &memory_dir.join("build.json"),
        &serde_json::to_value(build_metadata)?,
    )
    .with_context(|| format!("writing {}", memory_dir.join("build.json").display()))?;
    Ok(())
}

fn write_bytes_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;
    }

    let tmp = path.with_extension("tmp");
    {
        let mut f = fs::File::create(&tmp)
            .with_context(|| format!("opening temp file {}", tmp.display()))?;
        use std::io::Write as _;
        f.write_all(bytes)
            .with_context(|| format!("writing {}", tmp.display()))?;
        let _ = f.sync_all();
    }

    if path.exists() {
        let _ = fs::remove_file(path);
    }
    fs::rename(&tmp, path)
        .with_context(|| format!("renaming {} -> {}", tmp.display(), path.display()))?;

    if let Some(parent) = path.parent()
        && let Ok(dirf) = fs::File::open(parent)
    {
        let _ = dirf.sync_all();
    }

    Ok(())
}

fn replace_dir_atomically(stage_dir: &Path, target_dir: &Path) -> Result<()> {
    let backup_dir = target_dir
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(format!(".contracts-backup-{}", unique_suffix()));

    if target_dir.exists() {
        fs::rename(target_dir, &backup_dir).with_context(|| {
            format!(
                "moving existing generated contracts {} to {}",
                target_dir.display(),
                backup_dir.display()
            )
        })?;
    }

    if let Err(err) = fs::rename(stage_dir, target_dir) {
        if backup_dir.exists() {
            let _ = fs::rename(&backup_dir, target_dir);
        }
        return Err(err).with_context(|| {
            format!(
                "moving staged generated contracts {} to {}",
                stage_dir.display(),
                target_dir.display()
            )
        });
    }

    if backup_dir.exists() {
        fs::remove_dir_all(&backup_dir)
            .with_context(|| format!("removing {}", backup_dir.display()))?;
    }

    Ok(())
}

fn unique_suffix() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    nanos.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::write_manifest_pretty;

    fn temp_dir(label: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("agentpm-memory-{label}-{nanos}"));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write_fixture_file(dir: &Path, relative: &str, contents: &str) {
        let path = dir.join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, contents).unwrap();
    }

    fn write_manifest(dir: &Path, manifest: Value) -> PathBuf {
        let path = dir.join("agent.json");
        write_manifest_pretty(&path, &manifest).unwrap();
        path
    }

    fn simple_memory_manifest() -> Value {
        json!({
            "kind": "memory",
            "name": "conversation-continuity",
            "version": "0.1.0",
            "description": "Portable structure for conversational continuity memory.",
            "memory": {
                "scopes": {
                    "user": {
                        "description": "The user whose memory is being retained."
                    }
                },
                "record_types": {
                    "user_preference": {
                        "version": "1.0.0",
                        "description": "A durable user preference record.",
                        "schema": "schemas/user-preference.schema.json"
                    }
                },
                "spaces": {
                    "profile": {
                        "description": "One profile document per user.",
                        "model": "document",
                        "record_types": ["user_preference"],
                        "scope": ["user"],
                        "retrieval": {
                            "modes": ["key"]
                        }
                    }
                }
            }
        })
    }

    fn interaction_schema() -> &'static str {
        r#"{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "type": "object",
  "properties": {
    "speaker": {
      "type": "string",
      "enum": ["user", "assistant", "tool"]
    },
    "text": {
      "type": "string",
      "x-agentpm-data-class": "operational",
      "x-agentpm-sensitivity": "moderate",
      "x-agentpm-persist": true,
      "x-agentpm-shareable": true
    }
  },
  "required": ["speaker", "text"],
  "additionalProperties": false
}
"#
    }

    fn preference_schema() -> &'static str {
        r##"{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "type": "object",
  "$defs": {
    "profileNote": {
      "type": "object",
      "properties": {
        "body": {
          "type": "string",
          "x-agentpm-data-class": "personal",
          "x-agentpm-sensitivity": "moderate",
          "x-agentpm-persist": true,
          "x-agentpm-shareable": false
        }
      },
      "required": ["body"],
      "additionalProperties": false
    }
  },
  "properties": {
    "favorite_color": {
      "type": "string",
      "x-agentpm-data-class": "personal",
      "x-agentpm-sensitivity": "low",
      "x-agentpm-persist": true,
      "x-agentpm-shareable": false
    },
    "notes": {
      "type": "array",
      "items": {
        "$ref": "#/$defs/profileNote"
      }
    }
  },
  "required": ["favorite_color"],
  "additionalProperties": false
}
"##
    }

    fn compile_schema(value: &Value) {
        JSONSchema::options()
            .with_draft(Draft::Draft202012)
            .compile(value)
            .unwrap();
    }

    fn assert_instance_valid(schema: &Value, instance: &Value) {
        let compiled = JSONSchema::options()
            .with_draft(Draft::Draft202012)
            .compile(schema)
            .unwrap();
        if let Err(errs) = compiled.validate(instance) {
            let messages = errs
                .map(|err| err.to_string())
                .collect::<Vec<_>>()
                .join("; ");
            panic!("expected instance to validate, got: {messages}");
        }
    }

    fn assert_instance_invalid(schema: &Value, instance: &Value) {
        let compiled = JSONSchema::options()
            .with_draft(Draft::Draft202012)
            .compile(schema)
            .unwrap();
        assert!(
            compiled.validate(instance).is_err(),
            "expected instance to fail validation"
        );
    }

    fn mismatch_paths(
        check: &MemoryBuildCheckResult,
        kind: MemoryBuildMismatchKind,
    ) -> Vec<String> {
        check
            .mismatches
            .iter()
            .filter(|mismatch| mismatch.kind == kind)
            .map(|mismatch| mismatch.path.clone())
            .collect()
    }

    #[test]
    fn memory_build_writes_simple_document_contract_and_index() {
        let dir = temp_dir("simple-document");
        write_fixture_file(
            &dir,
            "schemas/user-preference.schema.json",
            preference_schema(),
        );
        let manifest_path = write_manifest(&dir, simple_memory_manifest());
        let manifest_before = fs::read(&manifest_path).unwrap();
        let schema_before = fs::read(dir.join("schemas/user-preference.schema.json")).unwrap();

        let summary = execute_memory_build(&manifest_path, MemoryBuildMode::Write).unwrap();
        assert_eq!(summary.contract_count, 1);
        assert_eq!(summary.scope_count, 1);
        assert_eq!(summary.record_type_count, 1);
        assert_eq!(summary.space_count, 1);
        assert_eq!(summary.operation_count, 0);

        let index_value: Value = serde_json::from_str(
            &fs::read_to_string(dir.join("memory/contracts/index.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(index_value["type"], MEMORY_CONTRACT_INDEX_TYPE);
        assert_eq!(index_value["format_version"], 1);
        assert_eq!(index_value["contracts"].as_array().unwrap().len(), 1);
        assert_eq!(
            index_value["contracts"][0]["source_schema"],
            "schemas/user-preference.schema.json"
        );
        assert_eq!(
            index_value["contracts"][0]["path"],
            "memory/contracts/profile.user_preference.schema.json"
        );

        let contract_value: Value = serde_json::from_str(
            &fs::read_to_string(dir.join("memory/contracts/profile.user_preference.schema.json"))
                .unwrap(),
        )
        .unwrap();
        compile_schema(&contract_value);
        assert_eq!(
            contract_value["$id"],
            "agentpm://memory/conversation-continuity/0.1.0/profile/user_preference/1.0.0"
        );
        assert!(
            contract_value["required"]
                .as_array()
                .unwrap()
                .contains(&Value::String("content".into()))
        );
        assert!(
            contract_value["required"]
                .as_array()
                .unwrap()
                .contains(&Value::String("created_at".into()))
        );
        assert_eq!(contract_value["properties"]["space"]["const"], "profile");
        assert_eq!(
            contract_value["properties"]["record_type"]["const"],
            "user_preference"
        );
        assert_eq!(
            contract_value["properties"]["scope"]["required"],
            json!(["user"])
        );
        assert!(contract_value["properties"]["ordinal"].is_null());
        assert!(
            contract_value["properties"]["content"]["properties"]["favorite_color"]["x-agentpm-data-class"]
                == "personal"
        );

        assert_eq!(fs::read(&manifest_path).unwrap(), manifest_before);
        assert_eq!(
            fs::read(dir.join("schemas/user-preference.schema.json")).unwrap(),
            schema_before
        );
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn memory_build_generates_sequence_and_collection_model_specific_fields() {
        let dir = temp_dir("model-specific");
        write_fixture_file(
            &dir,
            "schemas/interaction.schema.json",
            interaction_schema(),
        );
        let manifest_path = write_manifest(
            &dir,
            json!({
                "kind": "memory",
                "name": "history-memory",
                "version": "0.1.0",
                "description": "Sequence and collection test.",
                "memory": {
                    "scopes": {
                        "user": { "description": "The user." },
                        "conversation": { "description": "The conversation." }
                    },
                    "record_types": {
                        "interaction": {
                            "version": "1.0.0",
                            "description": "One interaction.",
                            "schema": "schemas/interaction.schema.json"
                        }
                    },
                    "spaces": {
                        "history": {
                            "description": "Ordered interaction history.",
                            "model": "sequence",
                            "record_types": ["interaction"],
                            "scope": ["user", "conversation"],
                            "retrieval": { "modes": ["chronological"] },
                            "constraints": { "append_only": true }
                        },
                        "saved_notes": {
                            "description": "Saved notes collection.",
                            "model": "collection",
                            "record_types": ["interaction"],
                            "scope": ["user"],
                            "retrieval": { "modes": ["filter"] },
                            "constraints": { "append_only": true }
                        }
                    }
                }
            }),
        );

        let executed =
            execute_memory_build_with_output(&manifest_path, MemoryBuildMode::Check).unwrap();
        let output = executed.output;
        assert_eq!(output.contracts.len(), 2);

        let history: Value = serde_json::from_slice(
            &output
                .contracts
                .iter()
                .find(|c| c.path.ends_with("history.interaction.schema.json"))
                .unwrap()
                .schema_bytes,
        )
        .unwrap();
        let saved_notes: Value = serde_json::from_slice(
            &output
                .contracts
                .iter()
                .find(|c| c.path.ends_with("saved_notes.interaction.schema.json"))
                .unwrap()
                .schema_bytes,
        )
        .unwrap();

        assert!(
            history["required"]
                .as_array()
                .unwrap()
                .contains(&Value::String("ordinal".into()))
        );
        assert_eq!(saved_notes["properties"]["ordinal"], Value::Null);
        assert_eq!(
            history["properties"]["scope"]["required"],
            json!(["user", "conversation"])
        );
        assert_eq!(
            saved_notes["properties"]["scope"]["required"],
            json!(["user"])
        );

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn memory_build_generates_sorted_multi_space_multi_record_type_index() {
        let dir = temp_dir("sorted-index");
        write_fixture_file(&dir, "schemas/a.schema.json", preference_schema());
        write_fixture_file(&dir, "schemas/b.schema.json", interaction_schema());
        let manifest_path = write_manifest(
            &dir,
            json!({
                "kind": "memory",
                "name": "multi-memory",
                "version": "0.1.0",
                "description": "Multiple spaces and record types.",
                "memory": {
                    "scopes": {
                        "user": { "description": "The user." }
                    },
                    "record_types": {
                        "b_type": {
                            "version": "2.0.0",
                            "description": "B type.",
                            "schema": "schemas/b.schema.json"
                        },
                        "a_type": {
                            "version": "1.0.0",
                            "description": "A type.",
                            "schema": "schemas/a.schema.json"
                        }
                    },
                    "spaces": {
                        "z_space": {
                            "description": "Z space.",
                            "model": "collection",
                            "record_types": ["b_type", "a_type"],
                            "scope": ["user"],
                            "retrieval": { "modes": ["filter"] }
                        },
                        "a_space": {
                            "description": "A space.",
                            "model": "document",
                            "record_types": ["a_type"],
                            "scope": ["user"],
                            "retrieval": { "modes": ["key"] }
                        }
                    }
                }
            }),
        );

        let output = execute_memory_build_with_output(&manifest_path, MemoryBuildMode::Check)
            .unwrap()
            .output;
        assert_eq!(output.index.contracts.len(), 3);
        let entries = &output.index.contracts;
        assert_eq!(entries[0].space, "a_space");
        assert_eq!(entries[0].record_type, "a_type");
        assert_eq!(entries[1].space, "z_space");
        assert_eq!(entries[1].record_type, "a_type");
        assert_eq!(entries[2].space, "z_space");
        assert_eq!(entries[2].record_type, "b_type");

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn memory_build_contracts_are_self_contained_after_source_schemas_are_removed() {
        let dir = temp_dir("self-contained");
        write_fixture_file(
            &dir,
            "schemas/user-preference.schema.json",
            preference_schema(),
        );
        let manifest_path = write_manifest(&dir, simple_memory_manifest());
        execute_memory_build(&manifest_path, MemoryBuildMode::Write).unwrap();

        fs::remove_dir_all(dir.join("schemas")).unwrap();

        let contract_value: Value = serde_json::from_str(
            &fs::read_to_string(dir.join("memory/contracts/profile.user_preference.schema.json"))
                .unwrap(),
        )
        .unwrap();
        compile_schema(&contract_value);
        assert!(contract_value["properties"]["content"]["$defs"]["profileNote"].is_object());
        assert_eq!(contract_value["properties"]["content"]["$id"], "content");

        let valid_instance = json!({
            "id": "mem_123",
            "record_type": "user_preference",
            "space": "profile",
            "scope": {
                "user": "user_123"
            },
            "schema_version": "1.0.0",
            "created_at": "2026-07-20T18:00:00Z",
            "content": {
                "favorite_color": "blue",
                "notes": [
                    {
                        "body": "hello"
                    }
                ]
            }
        });
        let invalid_instance = json!({
            "id": "mem_123",
            "record_type": "user_preference",
            "space": "profile",
            "scope": {
                "user": "user_123"
            },
            "schema_version": "1.0.0",
            "created_at": "2026-07-20T18:00:00Z",
            "content": {
                "favorite_color": "blue",
                "notes": [
                    {
                        "body": "hello",
                        "extra": true
                    }
                ]
            }
        });
        assert_instance_valid(&contract_value, &valid_instance);
        assert_instance_invalid(&contract_value, &invalid_instance);

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn memory_build_write_removes_stale_generated_contracts() {
        let dir = temp_dir("remove-stale");
        write_fixture_file(
            &dir,
            "schemas/user-preference.schema.json",
            preference_schema(),
        );
        let manifest_path = write_manifest(&dir, simple_memory_manifest());
        execute_memory_build(&manifest_path, MemoryBuildMode::Write).unwrap();

        let mut manifest_value: Value =
            serde_json::from_str(&fs::read_to_string(&manifest_path).unwrap()).unwrap();
        manifest_value["memory"]["spaces"] = json!({});
        write_manifest_pretty(&manifest_path, &manifest_value).unwrap();

        execute_memory_build(&manifest_path, MemoryBuildMode::Write).unwrap();
        let index_value: Value = serde_json::from_str(
            &fs::read_to_string(dir.join("memory/contracts/index.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(index_value["contracts"], json!([]));
        assert!(
            !dir.join("memory/contracts/profile.user_preference.schema.json")
                .exists()
        );

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn memory_build_unchanged_rebuild_produces_identical_bytes() {
        let dir = temp_dir("identical-rebuild");
        write_fixture_file(
            &dir,
            "schemas/user-preference.schema.json",
            preference_schema(),
        );
        let manifest_path = write_manifest(&dir, simple_memory_manifest());

        execute_memory_build(&manifest_path, MemoryBuildMode::Write).unwrap();
        let first_index = fs::read(dir.join("memory/contracts/index.json")).unwrap();
        let first_contract =
            fs::read(dir.join("memory/contracts/profile.user_preference.schema.json")).unwrap();

        execute_memory_build(&manifest_path, MemoryBuildMode::Write).unwrap();
        let second_index = fs::read(dir.join("memory/contracts/index.json")).unwrap();
        let second_contract =
            fs::read(dir.join("memory/contracts/profile.user_preference.schema.json")).unwrap();

        assert_eq!(first_index, second_index);
        assert_eq!(first_contract, second_contract);

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn memory_build_failed_generation_preserves_previous_successful_output() {
        let dir = temp_dir("preserve-on-failure");
        write_fixture_file(
            &dir,
            "schemas/user-preference.schema.json",
            preference_schema(),
        );
        let manifest_path = write_manifest(&dir, simple_memory_manifest());

        execute_memory_build(&manifest_path, MemoryBuildMode::Write).unwrap();
        let previous_index = fs::read(dir.join("memory/contracts/index.json")).unwrap();
        let previous_contract =
            fs::read(dir.join("memory/contracts/profile.user_preference.schema.json")).unwrap();

        let mut manifest_value: Value =
            serde_json::from_str(&fs::read_to_string(&manifest_path).unwrap()).unwrap();
        manifest_value["memory"]["record_types"]["user_preference"]["schema"] =
            json!("schemas/missing.schema.json");
        write_manifest_pretty(&manifest_path, &manifest_value).unwrap();

        let err = execute_memory_build(&manifest_path, MemoryBuildMode::Write).unwrap_err();
        assert!(
            err.to_string()
                .contains("schema `schemas/missing.schema.json`")
        );
        assert_eq!(
            fs::read(dir.join("memory/contracts/index.json")).unwrap(),
            previous_index
        );
        assert_eq!(
            fs::read(dir.join("memory/contracts/profile.user_preference.schema.json")).unwrap(),
            previous_contract
        );

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn memory_build_check_mode_performs_no_writes() {
        let dir = temp_dir("check-no-write");
        write_fixture_file(
            &dir,
            "schemas/user-preference.schema.json",
            preference_schema(),
        );
        let manifest_path = write_manifest(&dir, simple_memory_manifest());

        let executed =
            execute_memory_build_with_output(&manifest_path, MemoryBuildMode::Check).unwrap();
        assert_eq!(executed.summary.contract_count, 1);
        assert_eq!(executed.output.contracts.len(), 1);
        let check = executed.check.unwrap();
        assert_eq!(
            mismatch_paths(&check, MemoryBuildMismatchKind::MissingBuild),
            vec!["memory/build.json".to_string()]
        );
        assert!(!dir.join("memory/contracts/index.json").exists());
        assert!(
            !dir.join("memory/contracts/profile.user_preference.schema.json")
                .exists()
        );
        assert!(!dir.join("memory/build.json").exists());

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn memory_build_write_writes_build_metadata_and_contract_hashes() {
        let dir = temp_dir("build-metadata");
        write_fixture_file(
            &dir,
            "schemas/user-preference.schema.json",
            preference_schema(),
        );
        let manifest_path = write_manifest(&dir, simple_memory_manifest());

        let executed =
            execute_memory_build_with_output(&manifest_path, MemoryBuildMode::Write).unwrap();
        let build_json: Value =
            serde_json::from_str(&fs::read_to_string(dir.join("memory/build.json")).unwrap())
                .unwrap();
        let index_json: Value = serde_json::from_str(
            &fs::read_to_string(dir.join("memory/contracts/index.json")).unwrap(),
        )
        .unwrap();

        assert_eq!(build_json["type"], MEMORY_BUILD_METADATA_TYPE);
        assert_eq!(build_json["format_version"], 1);
        assert_eq!(build_json["manifest_path"], "agent.json");
        assert_eq!(build_json["contract_count"], Value::from(1));
        assert!(
            build_json["source_manifest_hash"]
                .as_str()
                .unwrap()
                .starts_with("sha256:")
        );
        assert!(
            build_json["source_schemas"][0]["sha256"]
                .as_str()
                .unwrap()
                .starts_with("sha256:")
        );
        assert_eq!(
            build_json["source_schemas"][0]["path"],
            "schemas/user-preference.schema.json"
        );
        assert!(
            build_json["source_schemas_hash"]
                .as_str()
                .unwrap()
                .starts_with("sha256:")
        );
        assert!(
            build_json["source_contract_inputs_hash"]
                .as_str()
                .unwrap()
                .starts_with("sha256:")
        );
        assert!(
            build_json["contracts_index_hash"]
                .as_str()
                .unwrap()
                .starts_with("sha256:")
        );
        assert!(
            build_json["contracts_hash"]
                .as_str()
                .unwrap()
                .starts_with("sha256:")
        );
        assert_eq!(
            index_json["contracts"][0]["sha256"],
            Value::String(executed.output.contracts[0].sha256.clone())
        );

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn memory_build_check_mode_reports_fresh_after_successful_build() {
        let dir = temp_dir("check-fresh");
        write_fixture_file(
            &dir,
            "schemas/user-preference.schema.json",
            preference_schema(),
        );
        let manifest_path = write_manifest(&dir, simple_memory_manifest());
        execute_memory_build(&manifest_path, MemoryBuildMode::Write).unwrap();

        let executed =
            execute_memory_build_with_output(&manifest_path, MemoryBuildMode::Check).unwrap();
        let check = executed.check.unwrap();
        assert!(check.mismatches.is_empty(), "{:#?}", check.mismatches);

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn memory_build_check_mode_reports_stale_manifest_bytes() {
        let dir = temp_dir("check-stale-manifest");
        write_fixture_file(
            &dir,
            "schemas/user-preference.schema.json",
            preference_schema(),
        );
        let manifest_path = write_manifest(&dir, simple_memory_manifest());
        execute_memory_build(&manifest_path, MemoryBuildMode::Write).unwrap();

        let original = fs::read_to_string(&manifest_path).unwrap();
        fs::write(&manifest_path, original.replace("  ", "    ")).unwrap();

        let executed =
            execute_memory_build_with_output(&manifest_path, MemoryBuildMode::Check).unwrap();
        let check = executed.check.unwrap();
        assert!(
            mismatch_paths(&check, MemoryBuildMismatchKind::StaleSourceInput)
                .contains(&"agent.json".to_string())
        );

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn memory_build_check_mode_reports_stale_source_schema() {
        let dir = temp_dir("check-stale-source");
        write_fixture_file(
            &dir,
            "schemas/user-preference.schema.json",
            preference_schema(),
        );
        let manifest_path = write_manifest(&dir, simple_memory_manifest());
        execute_memory_build(&manifest_path, MemoryBuildMode::Write).unwrap();

        write_fixture_file(
            &dir,
            "schemas/user-preference.schema.json",
            preference_schema()
                .replace(
                    "\"x-agentpm-sensitivity\": \"low\"",
                    "\"x-agentpm-sensitivity\": \"critical\"",
                )
                .as_str(),
        );

        let executed =
            execute_memory_build_with_output(&manifest_path, MemoryBuildMode::Check).unwrap();
        let check = executed.check.unwrap();
        assert!(
            mismatch_paths(&check, MemoryBuildMismatchKind::StaleSourceInput)
                .contains(&"schemas/user-preference.schema.json".to_string()),
            "{:#?}",
            check.mismatches
        );

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn memory_build_check_mode_reports_modified_contract() {
        let dir = temp_dir("check-modified-contract");
        write_fixture_file(
            &dir,
            "schemas/user-preference.schema.json",
            preference_schema(),
        );
        let manifest_path = write_manifest(&dir, simple_memory_manifest());
        execute_memory_build(&manifest_path, MemoryBuildMode::Write).unwrap();

        fs::write(
            dir.join("memory/contracts/profile.user_preference.schema.json"),
            "{\n  \"tampered\": true\n}\n",
        )
        .unwrap();

        let executed =
            execute_memory_build_with_output(&manifest_path, MemoryBuildMode::Check).unwrap();
        let check = executed.check.unwrap();
        assert!(
            mismatch_paths(&check, MemoryBuildMismatchKind::ModifiedOutput)
                .contains(&"memory/contracts/profile.user_preference.schema.json".to_string())
        );

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn memory_build_check_mode_reports_tampered_per_contract_hash_in_index() {
        let dir = temp_dir("check-index-contract-hash");
        write_fixture_file(
            &dir,
            "schemas/user-preference.schema.json",
            preference_schema(),
        );
        let manifest_path = write_manifest(&dir, simple_memory_manifest());
        execute_memory_build(&manifest_path, MemoryBuildMode::Write).unwrap();

        let mut index_json: Value = serde_json::from_str(
            &fs::read_to_string(dir.join("memory/contracts/index.json")).unwrap(),
        )
        .unwrap();
        index_json["contracts"][0]["sha256"] = Value::String("sha256:deadbeef".to_string());
        write_manifest_pretty(&dir.join("memory/contracts/index.json"), &index_json).unwrap();

        let executed =
            execute_memory_build_with_output(&manifest_path, MemoryBuildMode::Check).unwrap();
        let check = executed.check.unwrap();
        assert!(
            mismatch_paths(&check, MemoryBuildMismatchKind::ModifiedOutput)
                .contains(&"memory/contracts/index.json".to_string()),
            "{:#?}",
            check.mismatches
        );

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn memory_build_check_mode_reports_tampered_index_identity_fields() {
        let dir = temp_dir("check-index-identity");
        write_fixture_file(
            &dir,
            "schemas/user-preference.schema.json",
            preference_schema(),
        );
        let manifest_path = write_manifest(&dir, simple_memory_manifest());
        execute_memory_build(&manifest_path, MemoryBuildMode::Write).unwrap();

        let mut index_json: Value = serde_json::from_str(
            &fs::read_to_string(dir.join("memory/contracts/index.json")).unwrap(),
        )
        .unwrap();
        index_json["contracts"][0]["space"] = Value::String("profile_renamed".to_string());
        write_manifest_pretty(&dir.join("memory/contracts/index.json"), &index_json).unwrap();

        let executed =
            execute_memory_build_with_output(&manifest_path, MemoryBuildMode::Check).unwrap();
        let check = executed.check.unwrap();
        assert!(
            mismatch_paths(&check, MemoryBuildMismatchKind::ModifiedOutput)
                .contains(&"memory/contracts/index.json".to_string()),
            "{:#?}",
            check.mismatches
        );

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn memory_build_check_mode_reports_tampered_aggregate_contract_hash() {
        let dir = temp_dir("check-aggregate-contract-hash");
        write_fixture_file(
            &dir,
            "schemas/user-preference.schema.json",
            preference_schema(),
        );
        let manifest_path = write_manifest(&dir, simple_memory_manifest());
        execute_memory_build(&manifest_path, MemoryBuildMode::Write).unwrap();

        let mut build_json: Value =
            serde_json::from_str(&fs::read_to_string(dir.join("memory/build.json")).unwrap())
                .unwrap();
        build_json["contracts_hash"] = Value::String("sha256:deadbeef".to_string());
        write_manifest_pretty(&dir.join("memory/build.json"), &build_json).unwrap();

        let executed =
            execute_memory_build_with_output(&manifest_path, MemoryBuildMode::Check).unwrap();
        let check = executed.check.unwrap();
        assert!(
            mismatch_paths(&check, MemoryBuildMismatchKind::InconsistentMetadata)
                .contains(&"memory/build.json".to_string()),
            "{:#?}",
            check.mismatches
        );

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn memory_build_check_mode_reports_missing_and_unexpected_outputs() {
        let dir = temp_dir("check-missing-extra");
        write_fixture_file(
            &dir,
            "schemas/user-preference.schema.json",
            preference_schema(),
        );
        let manifest_path = write_manifest(&dir, simple_memory_manifest());
        execute_memory_build(&manifest_path, MemoryBuildMode::Write).unwrap();

        fs::remove_file(dir.join("memory/contracts/profile.user_preference.schema.json")).unwrap();
        fs::write(dir.join("memory/contracts/extra.schema.json"), "{}\n").unwrap();
        fs::create_dir_all(dir.join("memory/contracts/unexpected-dir")).unwrap();

        let executed =
            execute_memory_build_with_output(&manifest_path, MemoryBuildMode::Check).unwrap();
        let check = executed.check.unwrap();
        assert!(
            mismatch_paths(&check, MemoryBuildMismatchKind::MissingOutput)
                .contains(&"memory/contracts/profile.user_preference.schema.json".to_string())
        );
        assert!(
            mismatch_paths(&check, MemoryBuildMismatchKind::UnexpectedOutput)
                .contains(&"memory/contracts/extra.schema.json".to_string())
        );
        assert!(
            mismatch_paths(&check, MemoryBuildMismatchKind::UnexpectedOutput)
                .contains(&"memory/contracts/unexpected-dir".to_string())
        );

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn memory_build_check_mode_reports_missing_build_metadata_file() {
        let dir = temp_dir("check-missing-build-json");
        write_fixture_file(
            &dir,
            "schemas/user-preference.schema.json",
            preference_schema(),
        );
        let manifest_path = write_manifest(&dir, simple_memory_manifest());
        execute_memory_build(&manifest_path, MemoryBuildMode::Write).unwrap();

        fs::remove_file(dir.join("memory/build.json")).unwrap();

        let executed =
            execute_memory_build_with_output(&manifest_path, MemoryBuildMode::Check).unwrap();
        let check = executed.check.unwrap();
        assert!(
            mismatch_paths(&check, MemoryBuildMismatchKind::MissingBuild)
                .contains(&"memory/build.json".to_string()),
            "{:#?}",
            check.mismatches
        );

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn memory_build_check_mode_reports_incomplete_build_metadata_json() {
        let dir = temp_dir("check-incomplete-build-json");
        write_fixture_file(
            &dir,
            "schemas/user-preference.schema.json",
            preference_schema(),
        );
        let manifest_path = write_manifest(&dir, simple_memory_manifest());
        execute_memory_build(&manifest_path, MemoryBuildMode::Write).unwrap();

        fs::write(
            dir.join("memory/build.json"),
            "{\"type\":\"agentpm-memory-contracts\"}\n",
        )
        .unwrap();

        let executed =
            execute_memory_build_with_output(&manifest_path, MemoryBuildMode::Check).unwrap();
        let check = executed.check.unwrap();
        assert!(
            mismatch_paths(&check, MemoryBuildMismatchKind::InconsistentMetadata)
                .contains(&"memory/build.json".to_string()),
            "{:#?}",
            check.mismatches
        );

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn memory_build_check_mode_reports_unsupported_build_metadata_and_index_formats() {
        let dir = temp_dir("check-unsupported-format");
        write_fixture_file(
            &dir,
            "schemas/user-preference.schema.json",
            preference_schema(),
        );
        let manifest_path = write_manifest(&dir, simple_memory_manifest());
        execute_memory_build(&manifest_path, MemoryBuildMode::Write).unwrap();

        let mut build_json: Value =
            serde_json::from_str(&fs::read_to_string(dir.join("memory/build.json")).unwrap())
                .unwrap();
        build_json["format_version"] = Value::from(99);
        write_manifest_pretty(&dir.join("memory/build.json"), &build_json).unwrap();

        let mut index_json: Value = serde_json::from_str(
            &fs::read_to_string(dir.join("memory/contracts/index.json")).unwrap(),
        )
        .unwrap();
        index_json["format_version"] = Value::from(99);
        write_manifest_pretty(&dir.join("memory/contracts/index.json"), &index_json).unwrap();

        let executed =
            execute_memory_build_with_output(&manifest_path, MemoryBuildMode::Check).unwrap();
        let check = executed.check.unwrap();
        let unsupported = mismatch_paths(&check, MemoryBuildMismatchKind::UnsupportedFormat);
        assert!(unsupported.contains(&"memory/build.json".to_string()));
        assert!(unsupported.contains(&"memory/contracts/index.json".to_string()));

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn memory_build_check_mode_ignores_informational_metadata_changes() {
        let dir = temp_dir("check-informational-metadata");
        write_fixture_file(
            &dir,
            "schemas/user-preference.schema.json",
            preference_schema(),
        );
        let manifest_path = write_manifest(&dir, simple_memory_manifest());
        execute_memory_build(&manifest_path, MemoryBuildMode::Write).unwrap();

        let original_contract =
            fs::read(dir.join("memory/contracts/profile.user_preference.schema.json")).unwrap();
        let original_index = fs::read(dir.join("memory/contracts/index.json")).unwrap();

        let mut build_json: Value =
            serde_json::from_str(&fs::read_to_string(dir.join("memory/build.json")).unwrap())
                .unwrap();
        build_json["built_at"] = Value::String("2099-01-01T00:00:00Z".to_string());
        build_json["agentpm_version"] = Value::String("9.9.9".to_string());
        write_manifest_pretty(&dir.join("memory/build.json"), &build_json).unwrap();

        let executed =
            execute_memory_build_with_output(&manifest_path, MemoryBuildMode::Check).unwrap();
        let check = executed.check.unwrap();
        assert!(check.mismatches.is_empty(), "{:#?}", check.mismatches);
        assert_eq!(
            fs::read(dir.join("memory/contracts/profile.user_preference.schema.json")).unwrap(),
            original_contract
        );
        assert_eq!(
            fs::read(dir.join("memory/contracts/index.json")).unwrap(),
            original_index
        );

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn memory_build_check_mode_reports_inconsistent_contract_count_metadata() {
        let dir = temp_dir("check-count-mismatch");
        write_fixture_file(
            &dir,
            "schemas/user-preference.schema.json",
            preference_schema(),
        );
        let manifest_path = write_manifest(&dir, simple_memory_manifest());
        execute_memory_build(&manifest_path, MemoryBuildMode::Write).unwrap();

        let mut build_json: Value =
            serde_json::from_str(&fs::read_to_string(dir.join("memory/build.json")).unwrap())
                .unwrap();
        build_json["contract_count"] = Value::from(99);
        write_manifest_pretty(&dir.join("memory/build.json"), &build_json).unwrap();

        let executed =
            execute_memory_build_with_output(&manifest_path, MemoryBuildMode::Check).unwrap();
        let check = executed.check.unwrap();
        assert!(
            mismatch_paths(&check, MemoryBuildMismatchKind::InconsistentMetadata)
                .contains(&"memory/build.json".to_string())
        );

        let _ = fs::remove_dir_all(dir);
    }
}
