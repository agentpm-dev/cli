use crate::commands::memory::{
    GeneratedMemoryContract, MemoryBuildMetadata, MemoryBuildMode, MemoryContractIndex,
    execute_memory_build_with_output,
};
use crate::harness_config::HarnessImplementationEntry;
use crate::harness_runtime::action::{ActionDispatchResult, ActionFailureCategory};
use crate::harness_runtime::knowledge::{
    EmbeddingProvider, KnowledgeRuntimeFailure, ServiceRuntime,
};
use crate::harness_runtime::model::KnowledgeEmbeddingSnapshot;
use crate::manifest::{
    MemoryManifest, MemoryRetentionAction, MemoryRetrievalMode, MemorySpace, MemorySpaceModel,
    resolve_existing_relative_file,
};
use anyhow::{Context, Result, anyhow, bail};
use chrono::{DateTime, Duration as ChronoDuration, Utc};
use jsonschema::{Draft, JSONSchema};
use rusqlite::{Connection, OptionalExtension, Row, Transaction, params};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap};
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, UNIX_EPOCH};

mod custom;
mod semantic;

pub use custom::CustomMemoryRuntime;
pub(crate) use custom::{
    CustomMemoryCountRequest, CustomMemoryOperationStateRequest,
    custom_memory_action_error_from_output, custom_memory_lifecycle_commit_request_from_local,
    custom_memory_read_request_from_local, custom_memory_write_request_from_local,
    emit_memory_host_service_failure, process_memory_runtime_service,
    validate_memory_runtime_capabilities,
};
pub(crate) use semantic::durable_memory_content_hash;
#[cfg(test)]
use semantic::encode_f32_le_vector;
use semantic::{
    LocalMemoryWriteEmbeddingResult, delete_memory_vectors_for_record,
    materialize_memory_vector_best_effort,
};

const LOCAL_MEMORY_SCHEMA_VERSION: u64 = 2;
const LOCAL_MEMORY_DB_NAME: &str = "memory.sqlite3";
const LOCAL_MEMORY_BUSY_TIMEOUT_MS: u64 = 5_000;
const LOCAL_MEMORY_SEMANTIC_READ_BACKFILL_LIMIT: usize = 32;
static MEMORY_RECORD_ID_COUNTER: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocalMemoryActionErrorKind {
    NotFound,
    CapacityExceeded,
    ConstraintViolation,
    ContractViolation,
    EmbeddingProviderUnavailable,
    EmbeddingProviderFailed,
    Backend,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalMemoryActionError {
    kind: LocalMemoryActionErrorKind,
    message: String,
}

impl LocalMemoryActionError {
    fn new(kind: LocalMemoryActionErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }

    fn not_found(message: impl Into<String>) -> Self {
        Self::new(LocalMemoryActionErrorKind::NotFound, message)
    }

    fn capacity_exceeded(message: impl Into<String>) -> Self {
        Self::new(LocalMemoryActionErrorKind::CapacityExceeded, message)
    }

    fn constraint_violation(message: impl Into<String>) -> Self {
        Self::new(LocalMemoryActionErrorKind::ConstraintViolation, message)
    }

    pub(crate) fn contract_violation(message: impl Into<String>) -> Self {
        Self::new(LocalMemoryActionErrorKind::ContractViolation, message)
    }

    pub(crate) fn backend(message: impl Into<String>) -> Self {
        Self::new(LocalMemoryActionErrorKind::Backend, message)
    }

    fn embedding_provider_unavailable(message: impl Into<String>) -> Self {
        Self::new(
            LocalMemoryActionErrorKind::EmbeddingProviderUnavailable,
            message,
        )
    }

    fn embedding_provider_failed(message: impl Into<String>) -> Self {
        Self::new(LocalMemoryActionErrorKind::EmbeddingProviderFailed, message)
    }

    pub fn code(&self) -> &'static str {
        match self.kind {
            LocalMemoryActionErrorKind::NotFound => "not_found",
            LocalMemoryActionErrorKind::CapacityExceeded => "capacity_exceeded",
            LocalMemoryActionErrorKind::ConstraintViolation => "constraint_violation",
            LocalMemoryActionErrorKind::ContractViolation => "contract_violation",
            LocalMemoryActionErrorKind::EmbeddingProviderUnavailable => {
                "embedding_provider_unavailable"
            }
            LocalMemoryActionErrorKind::EmbeddingProviderFailed => "embedding_provider_failed",
            LocalMemoryActionErrorKind::Backend => "memory_runtime_failed",
        }
    }

    pub fn is_model_correctable(&self) -> bool {
        matches!(
            self.kind,
            LocalMemoryActionErrorKind::NotFound
                | LocalMemoryActionErrorKind::ConstraintViolation
                | LocalMemoryActionErrorKind::ContractViolation
        )
    }
}

impl fmt::Display for LocalMemoryActionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for LocalMemoryActionError {}

fn has_local_memory_schema_error(error: &anyhow::Error) -> bool {
    error.chain().any(|cause| {
        cause
            .downcast_ref::<LocalMemoryActionError>()
            .is_some_and(LocalMemoryActionError::is_model_correctable)
    })
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MemoryRuntimeConstraintCapability {
    AppendOnly,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MemoryRuntimeCapabilityDescriptor {
    pub space_models: Vec<MemorySpaceModel>,
    pub retrieval_modes: Vec<MemoryRetrievalMode>,
    pub retention_actions: Vec<MemoryRetentionAction>,
    pub constraints: Vec<MemoryRuntimeConstraintCapability>,
    pub capacity: bool,
    pub durable_trigger_state: bool,
    pub atomic_batches: bool,
}

impl MemoryRuntimeCapabilityDescriptor {
    pub fn local_sqlite() -> Self {
        Self {
            space_models: vec![
                MemorySpaceModel::Document,
                MemorySpaceModel::Collection,
                MemorySpaceModel::Sequence,
            ],
            retrieval_modes: vec![
                MemoryRetrievalMode::Key,
                MemoryRetrievalMode::Filter,
                MemoryRetrievalMode::Chronological,
                MemoryRetrievalMode::FullText,
            ],
            retention_actions: vec![
                MemoryRetentionAction::Delete,
                MemoryRetentionAction::Archive,
            ],
            constraints: vec![MemoryRuntimeConstraintCapability::AppendOnly],
            capacity: true,
            durable_trigger_state: true,
            atomic_batches: true,
        }
    }

    pub fn local_sqlite_with_semantic() -> Self {
        let mut descriptor = Self::local_sqlite();
        descriptor
            .retrieval_modes
            .push(MemoryRetrievalMode::Semantic);
        descriptor
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemorySpaceReadinessDiagnostic {
    pub space: String,
    pub reason: String,
}

pub fn unrealizable_memory_spaces(
    manifest: &MemoryManifest,
    capabilities: &MemoryRuntimeCapabilityDescriptor,
) -> Vec<MemorySpaceReadinessDiagnostic> {
    let mut diagnostics = Vec::new();

    for (space_name, space) in &manifest.memory.spaces {
        if !capabilities.space_models.contains(&space.model) {
            diagnostics.push(MemorySpaceReadinessDiagnostic {
                space: space_name.clone(),
                reason: format!("space model `{:?}` is not supported", space.model),
            });
        }

        let has_supported_retrieval_mode = space
            .retrieval
            .modes
            .iter()
            .any(|mode| capabilities.retrieval_modes.contains(mode));
        if !has_supported_retrieval_mode {
            let declared_modes = space
                .retrieval
                .modes
                .iter()
                .map(|mode| format!("`{mode:?}`"))
                .collect::<Vec<_>>()
                .join(", ");
            diagnostics.push(MemorySpaceReadinessDiagnostic {
                space: space_name.clone(),
                reason: format!("no supported retrieval modes; declared modes: {declared_modes}"),
            });
        }

        if let Some(retention) = &space.retention
            && !capabilities
                .retention_actions
                .contains(&retention.on_expire)
        {
            diagnostics.push(MemorySpaceReadinessDiagnostic {
                space: space_name.clone(),
                reason: format!(
                    "retention action `{:?}` is not supported",
                    retention.on_expire
                ),
            });
        }

        if space.capacity.is_some() && !capabilities.capacity {
            diagnostics.push(MemorySpaceReadinessDiagnostic {
                space: space_name.clone(),
                reason: "capacity checks are not supported".into(),
            });
        }

        if space
            .constraints
            .as_ref()
            .and_then(|constraints| constraints.append_only)
            .unwrap_or(false)
            && !capabilities
                .constraints
                .contains(&MemoryRuntimeConstraintCapability::AppendOnly)
        {
            diagnostics.push(MemorySpaceReadinessDiagnostic {
                space: space_name.clone(),
                reason: "append-only constraints are not supported".into(),
            });
        }
    }

    diagnostics
}

#[derive(Debug, Clone)]
pub struct LocalMemoryRecordRow {
    pub id: String,
    pub package: String,
    pub package_version: String,
    pub space: String,
    pub space_model: MemorySpaceModel,
    pub record_type: String,
    pub schema_version: String,
    pub scope: BTreeMap<String, String>,
    pub content: Value,
    pub provenance: Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
    pub archived_at: Option<DateTime<Utc>>,
    pub ordinal: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StoredMemoryRecord {
    pub id: String,
    pub package: String,
    pub package_version: String,
    pub space: String,
    pub space_model: String,
    pub record_type: String,
    pub schema_version: String,
    pub scope_json: String,
    pub scope_hash: String,
    pub content: Value,
    pub provenance: Value,
    pub created_at: String,
    pub updated_at: String,
    pub expires_at: Option<String>,
    pub archived_at: Option<String>,
    pub ordinal: Option<i64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocalMemoryWriteOperation {
    Create,
    Upsert,
    Update,
    Delete,
    Archive,
}

#[derive(Debug, Clone)]
pub struct LocalMemoryWriteRequest<'a> {
    pub package: &'a str,
    pub package_version: &'a str,
    pub manifest: &'a MemoryManifest,
    pub contracts: &'a ValidatedMemoryContracts,
    pub space: &'a str,
    pub record_type: &'a str,
    pub scope: BTreeMap<String, String>,
    pub operation: LocalMemoryWriteOperation,
    pub record_id: Option<String>,
    pub content: Option<Value>,
    pub provenance: Value,
    pub now: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LocalMemoryWriteResult {
    pub operation: LocalMemoryWriteOperation,
    pub record: Option<StoredMemoryRecord>,
    pub affected_record_id: Option<String>,
    pub embedding_requests: u64,
    pub embedding_request_duration_ms: Option<u64>,
    pub semantic_embedding_error: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocalMemoryReadMode {
    Key,
    Filter,
    Chronological,
    FullText,
    Semantic,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalMemorySemanticConfig {
    pub embedding_provider: String,
    pub embedding_model: String,
    pub dimensions: u64,
    pub normalized: bool,
}

impl LocalMemorySemanticConfig {
    pub fn embedding_space(&self) -> KnowledgeEmbeddingSnapshot {
        KnowledgeEmbeddingSnapshot {
            id: "memory.local.semantic".into(),
            provider: self.embedding_provider.clone(),
            model: self.embedding_model.clone(),
            dimensions: self.dimensions,
            metric: "cosine".into(),
            normalized: self.normalized,
        }
    }
}

#[derive(Debug, Clone)]
pub struct LocalMemoryReadRequest<'a> {
    pub package: &'a str,
    pub package_version: &'a str,
    pub manifest: &'a MemoryManifest,
    pub space: &'a str,
    pub scope: BTreeMap<String, String>,
    pub mode: LocalMemoryReadMode,
    pub record_id: Option<String>,
    pub record_type: Option<String>,
    pub filter: BTreeMap<String, Value>,
    pub query: Option<String>,
    pub limit: Option<usize>,
    pub now: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LocalMemoryReadResult {
    pub records: Vec<StoredMemoryRecord>,
    pub embedding_requests: u64,
    pub embedding_request_duration_ms: Option<u64>,
    pub vectors_materialized: u64,
    pub vectors_pending: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LocalMemoryOperationStateRow {
    pub package: String,
    pub package_version: String,
    pub operation: String,
    pub scope: BTreeMap<String, String>,
    pub trigger_type: String,
    pub armed: bool,
    pub baseline_at: Option<DateTime<Utc>>,
    pub last_completed_at: Option<DateTime<Utc>>,
    pub last_failed_at: Option<DateTime<Utc>>,
    pub next_eligible_at: Option<DateTime<Utc>>,
    pub last_observed_value: Option<i64>,
    pub last_failure: Option<Value>,
    pub watermark: Option<Value>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalMemoryLifecycleSourceSnapshot {
    pub package: String,
    pub package_version: String,
    pub space: String,
    pub scope: BTreeMap<String, String>,
    pub record_id: String,
    pub content_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum LocalMemoryLifecycleTriggerPrecondition {
    ActiveCountAtLeast {
        package: String,
        package_version: String,
        space: String,
        scope: BTreeMap<String, String>,
        threshold: u64,
    },
    ActiveCountAtCapacity {
        package: String,
        package_version: String,
        space: String,
        scope: BTreeMap<String, String>,
        max_records: u64,
    },
}

#[derive(Debug)]
pub struct LocalMemoryLifecycleCommitRequest<'a> {
    pub trigger_precondition: Option<LocalMemoryLifecycleTriggerPrecondition>,
    pub expected_sources: Vec<LocalMemoryLifecycleSourceSnapshot>,
    pub output_writes: Vec<LocalMemoryWriteRequest<'a>>,
    pub source_mutations: Vec<LocalMemoryWriteRequest<'a>>,
    pub operation_state: LocalMemoryOperationStateRow,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalMemoryLifecycleCommitResult {
    pub output_record_ids: Vec<String>,
    pub source_record_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StoredMemoryOperationState {
    pub package: String,
    pub package_version: String,
    pub operation: String,
    pub scope_json: String,
    pub scope_hash: String,
    pub trigger_type: String,
    pub armed: bool,
    pub baseline_at: Option<String>,
    pub last_completed_at: Option<String>,
    pub last_failed_at: Option<String>,
    pub next_eligible_at: Option<String>,
    pub last_observed_value: Option<i64>,
    pub last_failure: Option<Value>,
    pub watermark: Option<Value>,
}

pub struct LocalSqliteMemoryRuntime {
    database_path: PathBuf,
    connection: Connection,
}

pub struct LocalSqliteMemoryBatch<'conn> {
    transaction: Transaction<'conn>,
}

#[derive(Debug, Clone)]
pub struct ValidatedMemoryContracts {
    pub index: MemoryContractIndex,
    pub contracts: Vec<GeneratedMemoryContract>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct MemoryContractCacheIdentity {
    package_root: PathBuf,
    manifest_hash: String,
    build_source_manifest_hash: String,
    source_schemas: Vec<(String, String)>,
    source_schemas_hash: String,
    source_contract_inputs_hash: String,
    build_contracts_index_hash: String,
    actual_contracts_index_hash: String,
    contracts_hash: String,
    contract_count: u64,
    contract_hashes: Vec<(String, String)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct MemoryContractArtifactFingerprint {
    artifacts: Vec<MemoryContractArtifactStat>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct MemoryContractArtifactStat {
    path: String,
    len: u64,
    modified_nanos: Option<u128>,
}

#[derive(Debug, Clone)]
struct CachedMemoryContracts {
    identity: MemoryContractCacheIdentity,
    artifact_paths: Vec<String>,
    artifact_fingerprint: MemoryContractArtifactFingerprint,
    contracts: ValidatedMemoryContracts,
}

#[derive(Debug, Default)]
pub struct MemoryContractCache {
    entries: BTreeMap<PathBuf, CachedMemoryContracts>,
}

impl MemoryContractCache {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn validate_and_load(&mut self, package_root: &Path) -> Result<ValidatedMemoryContracts> {
        let canonical_root = package_root.canonicalize().with_context(|| {
            format!(
                "resolving Memory package root for contract cache {}",
                package_root.display()
            )
        })?;

        if let Some(cached) = self.entries.get(&canonical_root)
            && let Ok(fingerprint) =
                memory_contract_artifact_fingerprint(&canonical_root, &cached.artifact_paths)
            && fingerprint == cached.artifact_fingerprint
        {
            return Ok(cached.contracts.clone());
        }

        let contracts = validate_and_load_memory_contracts(&canonical_root)?;
        let identity = memory_contract_cache_identity(&canonical_root)?;
        let artifact_paths = memory_contract_artifact_paths(&identity);
        let artifact_fingerprint =
            memory_contract_artifact_fingerprint(&canonical_root, &artifact_paths)?;
        self.entries.insert(
            canonical_root,
            CachedMemoryContracts {
                identity,
                artifact_paths,
                artifact_fingerprint,
                contracts: contracts.clone(),
            },
        );
        Ok(contracts)
    }

    #[cfg(test)]
    fn len(&self) -> usize {
        self.entries.len()
    }
}

impl LocalSqliteMemoryRuntime {
    pub fn open(workspace_root: &Path, state_dir: Option<&Path>) -> Result<Self> {
        let resolved_state_dir = match state_dir {
            Some(path) if path.is_absolute() => path.to_path_buf(),
            Some(path) => workspace_root.join(path),
            None => workspace_root.join(".agentpm-state"),
        };
        fs::create_dir_all(&resolved_state_dir).with_context(|| {
            format!(
                "creating Memory runtime state dir {}",
                resolved_state_dir.display()
            )
        })?;

        let database_path = resolved_state_dir.join(LOCAL_MEMORY_DB_NAME);
        let connection = Connection::open(&database_path)
            .with_context(|| format!("opening Memory SQLite store {}", database_path.display()))?;
        Self::configure_connection(&connection)?;
        let runtime = Self {
            database_path,
            connection,
        };
        runtime.initialize_schema()?;
        Ok(runtime)
    }

    pub fn database_path(&self) -> &Path {
        &self.database_path
    }

    pub fn capabilities(&self) -> MemoryRuntimeCapabilityDescriptor {
        MemoryRuntimeCapabilityDescriptor::local_sqlite()
    }

    fn configure_connection(connection: &Connection) -> Result<()> {
        connection
            .busy_timeout(Duration::from_millis(LOCAL_MEMORY_BUSY_TIMEOUT_MS))
            .context("configuring Memory SQLite busy timeout")?;
        connection
            .execute_batch(
                r#"
                PRAGMA foreign_keys = ON;
                PRAGMA journal_mode = WAL;
                "#,
            )
            .context("configuring Memory SQLite connection")?;

        Ok(())
    }

    pub fn schema_version(&self) -> Result<u64> {
        let value: String = self
            .connection
            .query_row(
                "SELECT value FROM memory_meta WHERE key = 'schema_version'",
                [],
                |row| row.get(0),
            )
            .context("reading Memory SQLite schema version")?;
        value
            .parse::<u64>()
            .with_context(|| format!("invalid Memory SQLite schema version `{value}`"))
    }

    pub fn canonical_scope_json(scope: &BTreeMap<String, String>) -> Result<String> {
        serde_json::to_string(scope).context("serializing canonical Memory scope JSON")
    }

    pub fn scope_hash_for_json(scope_json: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(scope_json.as_bytes());
        format!("sha256:{}", hex::encode(hasher.finalize()))
    }

    pub fn scope_identity(scope: &BTreeMap<String, String>) -> Result<(String, String)> {
        let scope_json = Self::canonical_scope_json(scope)?;
        let scope_hash = Self::scope_hash_for_json(&scope_json);
        Ok((scope_json, scope_hash))
    }

    pub fn verify_scope_identity(scope_json: &str, scope_hash: &str) -> Result<()> {
        let expected = Self::scope_hash_for_json(scope_json);
        if scope_hash != expected {
            bail!("Memory scope hash/content mismatch: expected `{expected}`, got `{scope_hash}`");
        }
        if scope_hash != scope_hash.to_ascii_lowercase() {
            bail!("Memory scope hash must be lowercase");
        }
        Ok(())
    }

    pub fn insert_record(&self, record: &LocalMemoryRecordRow) -> Result<()> {
        insert_memory_record(&self.connection, record)
    }

    pub fn get_record(
        &self,
        package: &str,
        package_version: &str,
        space: &str,
        scope: &BTreeMap<String, String>,
        record_id: &str,
    ) -> Result<Option<StoredMemoryRecord>> {
        get_memory_record(
            &self.connection,
            package,
            package_version,
            space,
            scope,
            record_id,
            Utc::now(),
        )
    }

    pub fn active_record_count(
        &self,
        package: &str,
        package_version: &str,
        space: &str,
        scope: &BTreeMap<String, String>,
        record_type: Option<&str>,
    ) -> Result<u64> {
        active_memory_record_count(
            &self.connection,
            package,
            package_version,
            space,
            scope,
            record_type,
            Utc::now(),
        )
    }

    pub fn write_record(
        &mut self,
        request: LocalMemoryWriteRequest<'_>,
    ) -> Result<LocalMemoryWriteResult> {
        self.write_record_inner(request, None, true)
    }

    pub fn write_record_with_semantic(
        &mut self,
        request: LocalMemoryWriteRequest<'_>,
        semantic: &LocalMemorySemanticConfig,
        embedder: &mut dyn EmbeddingProvider,
    ) -> Result<LocalMemoryWriteResult> {
        self.write_record_inner(request, Some((semantic, embedder)), true)
    }

    pub fn write_record_for_lifecycle(
        &mut self,
        request: LocalMemoryWriteRequest<'_>,
    ) -> Result<LocalMemoryWriteResult> {
        self.write_record_inner(request, None, false)
    }

    pub fn source_snapshot_for_lifecycle(
        &self,
        record: &StoredMemoryRecord,
    ) -> Result<LocalMemoryLifecycleSourceSnapshot> {
        let scope: BTreeMap<String, String> = serde_json::from_str(&record.scope_json)
            .context("parsing Memory source record scope JSON")?;
        Ok(LocalMemoryLifecycleSourceSnapshot {
            package: record.package.clone(),
            package_version: record.package_version.clone(),
            space: record.space.clone(),
            scope,
            record_id: record.id.clone(),
            content_hash: durable_memory_content_hash(&record.content)?,
        })
    }

    pub fn commit_lifecycle_operation(
        &mut self,
        request: LocalMemoryLifecycleCommitRequest<'_>,
    ) -> Result<LocalMemoryLifecycleCommitResult> {
        self.atomic_batch(|batch| {
            let mut operation_state = request.operation_state;
            let mut expired_spaces = BTreeSetLike::new();
            for write in request
                .output_writes
                .iter()
                .chain(request.source_mutations.iter())
            {
                if expired_spaces.insert(format!(
                    "{}\x1f{}\x1f{}",
                    write.package, write.package_version, write.space
                )) {
                    let space = memory_space(write.manifest, write.space)?;
                    expire_memory_records_for_space(
                        &batch.transaction,
                        write.package,
                        write.package_version,
                        write.space,
                        space,
                        write.now,
                    )?;
                }
            }

            if let Some(precondition) = &request.trigger_precondition {
                validate_lifecycle_trigger_precondition(
                    &batch.transaction,
                    precondition,
                    operation_state.updated_at,
                )?;
            }

            for source in &request.expected_sources {
                let current = get_memory_record(
                    &batch.transaction,
                    &source.package,
                    &source.package_version,
                    &source.space,
                    &source.scope,
                    &source.record_id,
                    operation_state.updated_at,
                )?
                .ok_or_else(|| {
                    LocalMemoryActionError::constraint_violation(format!(
                        "Memory lifecycle source `{}` changed before commit",
                        source.record_id
                    ))
                })?;
                let current_hash = durable_memory_content_hash(&current.content)?;
                if current_hash != source.content_hash {
                    return Err(LocalMemoryActionError::constraint_violation(format!(
                        "Memory lifecycle source `{}` changed before commit",
                        source.record_id
                    ))
                    .into());
                }
            }

            let mut output_record_ids = Vec::new();
            for write in request.output_writes {
                if let Some(record_id) = apply_lifecycle_write_in_transaction(batch, write)? {
                    output_record_ids.push(record_id);
                }
            }

            let mut source_record_ids = Vec::new();
            for write in request.source_mutations {
                if let Some(record_id) = apply_lifecycle_write_in_transaction(batch, write)? {
                    source_record_ids.push(record_id);
                }
            }

            operation_state.watermark = Some(lifecycle_watermark_with_commit_records(
                operation_state.watermark.take(),
                &output_record_ids,
                &source_record_ids,
            ));
            batch.store_operation_state(&operation_state)?;
            Ok(LocalMemoryLifecycleCommitResult {
                output_record_ids,
                source_record_ids,
            })
        })
    }

    fn write_record_inner(
        &mut self,
        request: LocalMemoryWriteRequest<'_>,
        mut semantic_write: Option<(&LocalMemorySemanticConfig, &mut dyn EmbeddingProvider)>,
        enforce_direct_append_only: bool,
    ) -> Result<LocalMemoryWriteResult> {
        validate_memory_scope(request.manifest, request.space, &request.scope)?;
        let space = memory_space(request.manifest, request.space)?;
        if !space
            .record_types
            .contains(&request.record_type.to_string())
        {
            return Err(LocalMemoryActionError::contract_violation(format!(
                "record type `{}` is not permitted in Memory space `{}`",
                request.record_type, request.space
            ))
            .into());
        }

        if enforce_direct_append_only
            && append_only_enabled(space)
            && matches!(
                request.operation,
                LocalMemoryWriteOperation::Update
                    | LocalMemoryWriteOperation::Delete
                    | LocalMemoryWriteOperation::Archive
            )
        {
            return Err(LocalMemoryActionError::constraint_violation(format!(
                "append_only Memory space `{}` rejects direct mutation",
                request.space
            ))
            .into());
        }

        if matches!(
            request.operation,
            LocalMemoryWriteOperation::Create | LocalMemoryWriteOperation::Upsert
        ) && request.record_id.is_some()
        {
            return Err(LocalMemoryActionError::constraint_violation(
                "Memory create/upsert cannot assign an authoritative record id",
            )
            .into());
        }

        let prepared = match request.operation {
            LocalMemoryWriteOperation::Create | LocalMemoryWriteOperation::Upsert => {
                Some(prepare_local_memory_record(&request, None).map_err(|err| {
                    if has_local_memory_schema_error(&err) {
                        err
                    } else {
                        err.context("preparing Memory create/upsert")
                    }
                })?)
            }
            LocalMemoryWriteOperation::Update => {
                let record_id = request.record_id.as_deref().ok_or_else(|| {
                    LocalMemoryActionError::constraint_violation(
                        "Memory update requires an existing record id",
                    )
                })?;
                Some(
                    prepare_local_memory_record(&request, Some(record_id)).map_err(|err| {
                        if has_local_memory_schema_error(&err) {
                            err
                        } else {
                            err.context("preparing Memory update")
                        }
                    })?,
                )
            }
            LocalMemoryWriteOperation::Delete | LocalMemoryWriteOperation::Archive => None,
        };

        self.atomic_batch(|batch| {
            expire_memory_records_for_space(
                &batch.transaction,
                request.package,
                request.package_version,
                request.space,
                space,
                request.now,
            )?;

            match request.operation {
                LocalMemoryWriteOperation::Create | LocalMemoryWriteOperation::Upsert => {
                    let mut record = prepared.expect("record prepared for create/upsert");
                    let existing_id = if matches!(space.model, MemorySpaceModel::Document) {
                        find_current_document_id(
                            &batch.transaction,
                            request.package,
                            request.package_version,
                            request.space,
                            &request.scope,
                            request.now,
                        )?
                    } else {
                        None
                    };
                    if existing_id.is_some()
                        && matches!(request.operation, LocalMemoryWriteOperation::Create)
                    {
                        return Err(LocalMemoryActionError::constraint_violation(format!(
                            "Memory document create for space `{}` requires no current document for the resolved scope; use upsert to replace the current document",
                            request.space
                        ))
                        .into());
                    }
                    let creates_new_active = existing_id.is_none();
                    enforce_memory_capacity(
                        &batch.transaction,
                        &request,
                        space,
                        creates_new_active,
                    )?;

                    if let Some(existing_id) = existing_id {
                        record.id = existing_id.clone();
                        validate_memory_record_envelope(request.contracts, &record)?;
                        update_memory_record(&batch.transaction, &record)?;
                    } else {
                        if matches!(space.model, MemorySpaceModel::Sequence) {
                            record.ordinal = Some(batch.allocate_sequence_ordinal(
                                request.package,
                                request.package_version,
                                request.space,
                                &request.scope,
                            )?);
                        }
                        validate_memory_record_envelope(request.contracts, &record)?;
                        insert_memory_record(&batch.transaction, &record)?;
                    }
                    delete_memory_vectors_for_record(
                        &batch.transaction,
                        request.package,
                        request.package_version,
                        request.space,
                        &request.scope,
                        &record.id,
                    )?;
                    let stored = get_memory_record(
                        &batch.transaction,
                        request.package,
                        request.package_version,
                        request.space,
                        &request.scope,
                        &record.id,
                        request.now,
                    )?
                    .context("Memory write did not return the committed record")?;
                    let semantic_result =
                        if let Some((semantic, embedder)) = semantic_write.as_mut() {
                            materialize_memory_vector_best_effort(
                                &batch.transaction,
                                &stored,
                                semantic,
                                &mut **embedder,
                            )
                        } else {
                            LocalMemoryWriteEmbeddingResult::default()
                        };
                    Ok(LocalMemoryWriteResult {
                        operation: request.operation,
                        affected_record_id: Some(stored.id.clone()),
                        record: Some(stored),
                        embedding_requests: semantic_result.embedding_requests,
                        embedding_request_duration_ms: semantic_result.duration_ms,
                        semantic_embedding_error: semantic_result.error,
                    })
                }
                LocalMemoryWriteOperation::Update => {
                    let mut record = prepared.expect("record prepared for update");
                    let record_id = request.record_id.as_deref().unwrap();
                    let existing = get_memory_record(
                        &batch.transaction,
                        request.package,
                        request.package_version,
                        request.space,
                        &request.scope,
                        record_id,
                        request.now,
                    )?
                    .ok_or_else(|| {
                        LocalMemoryActionError::not_found(format!(
                            "Memory record `{record_id}` was not found"
                        ))
                    })?;
                    if !matches!(space.model, MemorySpaceModel::Document)
                        && existing.record_type != request.record_type
                    {
                        return Err(LocalMemoryActionError::constraint_violation(format!(
                            "Memory update target `{record_id}` has record type `{}` not `{}`",
                            existing.record_type, request.record_type
                        ))
                        .into());
                    }
                    record.id = existing.id;
                    record.created_at = parse_rfc3339_utc(&existing.created_at)?;
                    record.ordinal = existing.ordinal;
                    validate_memory_record_envelope(request.contracts, &record)?;
                    update_memory_record(&batch.transaction, &record)?;
                    delete_memory_vectors_for_record(
                        &batch.transaction,
                        request.package,
                        request.package_version,
                        request.space,
                        &request.scope,
                        &record.id,
                    )?;
                    let stored = get_memory_record(
                        &batch.transaction,
                        request.package,
                        request.package_version,
                        request.space,
                        &request.scope,
                        &record.id,
                        request.now,
                    )?
                    .context("Memory update did not return the committed record")?;
                    let semantic_result =
                        if let Some((semantic, embedder)) = semantic_write.as_mut() {
                            materialize_memory_vector_best_effort(
                                &batch.transaction,
                                &stored,
                                semantic,
                                &mut **embedder,
                            )
                        } else {
                            LocalMemoryWriteEmbeddingResult::default()
                        };
                    Ok(LocalMemoryWriteResult {
                        operation: request.operation,
                        affected_record_id: Some(stored.id.clone()),
                        record: Some(stored),
                        embedding_requests: semantic_result.embedding_requests,
                        embedding_request_duration_ms: semantic_result.duration_ms,
                        semantic_embedding_error: semantic_result.error,
                    })
                }
                LocalMemoryWriteOperation::Delete => {
                    let record_id = request.record_id.as_deref().ok_or_else(|| {
                        LocalMemoryActionError::constraint_violation(
                            "Memory delete requires an existing record id",
                        )
                    })?;
                    delete_memory_record(
                        &batch.transaction,
                        request.package,
                        request.package_version,
                        request.space,
                        &request.scope,
                        record_id,
                    )?;
                    Ok(LocalMemoryWriteResult {
                        operation: request.operation,
                        affected_record_id: Some(record_id.to_string()),
                        record: None,
                        embedding_requests: 0,
                        embedding_request_duration_ms: None,
                        semantic_embedding_error: None,
                    })
                }
                LocalMemoryWriteOperation::Archive => {
                    let record_id = request.record_id.as_deref().ok_or_else(|| {
                        LocalMemoryActionError::constraint_violation(
                            "Memory archive requires an existing record id",
                        )
                    })?;
                    archive_memory_record(
                        &batch.transaction,
                        request.package,
                        request.package_version,
                        request.space,
                        &request.scope,
                        record_id,
                        request.now,
                    )?;
                    Ok(LocalMemoryWriteResult {
                        operation: request.operation,
                        affected_record_id: Some(record_id.to_string()),
                        record: None,
                        embedding_requests: 0,
                        embedding_request_duration_ms: None,
                        semantic_embedding_error: None,
                    })
                }
            }
        })
    }

    pub fn read_records(
        &mut self,
        request: LocalMemoryReadRequest<'_>,
    ) -> Result<Vec<StoredMemoryRecord>> {
        validate_memory_scope(request.manifest, request.space, &request.scope)?;
        let space = memory_space(request.manifest, request.space)?;

        self.atomic_batch(|batch| {
            expire_memory_records_for_space(
                &batch.transaction,
                request.package,
                request.package_version,
                request.space,
                space,
                request.now,
            )?;

            match request.mode {
                LocalMemoryReadMode::Key => {
                    read_memory_records_by_key(&batch.transaction, &request)
                }
                LocalMemoryReadMode::Filter => {
                    read_memory_records_by_filter(&batch.transaction, &request)
                }
                LocalMemoryReadMode::Chronological => {
                    read_memory_records_chronological(&batch.transaction, &request)
                }
                LocalMemoryReadMode::FullText => {
                    read_memory_records_full_text(&batch.transaction, &request)
                }
                LocalMemoryReadMode::Semantic => Err(LocalMemoryActionError::constraint_violation(
                    "semantic Memory retrieval requires an EmbeddingProvider",
                )
                .into()),
            }
        })
    }

    pub fn read_active_records_for_lifecycle(
        &mut self,
        request: LocalMemoryReadRequest<'_>,
    ) -> Result<Vec<StoredMemoryRecord>> {
        validate_memory_scope(request.manifest, request.space, &request.scope)?;
        let space = memory_space(request.manifest, request.space)?;
        self.atomic_batch(|batch| {
            expire_memory_records_for_space(
                &batch.transaction,
                request.package,
                request.package_version,
                request.space,
                space,
                request.now,
            )?;
            query_active_memory_records(
                &batch.transaction,
                &request,
                "updated_at ASC, created_at ASC, ordinal ASC, id ASC",
                None,
            )
        })
    }

    pub fn allocate_sequence_ordinal(
        &mut self,
        package: &str,
        package_version: &str,
        space: &str,
        scope: &BTreeMap<String, String>,
    ) -> Result<i64> {
        self.atomic_batch(|batch| {
            batch.allocate_sequence_ordinal(package, package_version, space, scope)
        })
    }

    pub fn store_operation_state(&self, state: &LocalMemoryOperationStateRow) -> Result<()> {
        store_memory_operation_state(&self.connection, state)
    }

    pub fn load_operation_state(
        &self,
        package: &str,
        package_version: &str,
        operation: &str,
        scope: &BTreeMap<String, String>,
    ) -> Result<Option<StoredMemoryOperationState>> {
        load_memory_operation_state(&self.connection, package, package_version, operation, scope)
    }

    pub fn atomic_batch<T>(
        &mut self,
        execute: impl FnOnce(&mut LocalSqliteMemoryBatch<'_>) -> Result<T>,
    ) -> Result<T> {
        let transaction = self
            .connection
            .transaction()
            .context("starting Memory SQLite atomic batch")?;
        let mut batch = LocalSqliteMemoryBatch { transaction };
        match execute(&mut batch) {
            Ok(value) => {
                batch
                    .transaction
                    .commit()
                    .context("committing Memory SQLite atomic batch")?;
                Ok(value)
            }
            Err(err) => {
                if let Err(rollback_err) = batch.transaction.rollback() {
                    return Err(err).context(format!(
                        "rolling back Memory SQLite atomic batch failed: {rollback_err}"
                    ));
                }
                Err(err)
            }
        }
    }

    fn initialize_schema(&self) -> Result<()> {
        self.connection.execute_batch(
            r#"
            PRAGMA foreign_keys = ON;

            CREATE TABLE IF NOT EXISTS memory_meta (
                key TEXT PRIMARY KEY NOT NULL,
                value TEXT NOT NULL
            );
            "#,
        )?;

        let existing_version = self
            .connection
            .query_row(
                "SELECT value FROM memory_meta WHERE key = 'schema_version'",
                [],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .context("checking Memory SQLite schema version")?;

        let schema_version = match existing_version {
            Some(version) => version
                .parse::<u64>()
                .with_context(|| format!("invalid Memory SQLite schema version `{version}`"))?,
            None => 0,
        };

        if schema_version > LOCAL_MEMORY_SCHEMA_VERSION {
            bail!(
                "unsupported Memory SQLite schema version {schema_version}; this AgentPM supports up to {LOCAL_MEMORY_SCHEMA_VERSION}"
            );
        }

        self.migrate_schema_from(schema_version)
    }

    fn migrate_schema_from(&self, mut schema_version: u64) -> Result<()> {
        while schema_version < LOCAL_MEMORY_SCHEMA_VERSION {
            schema_version = match schema_version {
                0 => {
                    self.create_schema_v1()?;
                    self.set_schema_version(1)?;
                    1
                }
                1 => {
                    self.create_schema_v2()?;
                    self.set_schema_version(2)?;
                    2
                }
                unsupported => bail!(
                    "no Memory SQLite migration path from schema version {unsupported} to {LOCAL_MEMORY_SCHEMA_VERSION}"
                ),
            };
        }

        Ok(())
    }

    fn set_schema_version(&self, schema_version: u64) -> Result<()> {
        self.connection.execute(
            r#"
            INSERT INTO memory_meta (key, value) VALUES ('schema_version', ?1)
            ON CONFLICT(key) DO UPDATE SET value = excluded.value
            "#,
            params![schema_version.to_string()],
        )?;

        Ok(())
    }

    fn create_schema_v1(&self) -> Result<()> {
        self.connection.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS memory_records (
                id TEXT NOT NULL,
                package TEXT NOT NULL,
                package_version TEXT NOT NULL,
                space TEXT NOT NULL,
                space_model TEXT NOT NULL,
                record_type TEXT NOT NULL,
                schema_version TEXT NOT NULL,
                scope_json TEXT NOT NULL,
                scope_hash TEXT NOT NULL,
                content_json TEXT NOT NULL,
                provenance_json TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                expires_at TEXT,
                archived_at TEXT,
                ordinal INTEGER,
                PRIMARY KEY(package, package_version, space, scope_hash, id)
            );

            CREATE INDEX IF NOT EXISTS idx_memory_records_active_lookup
                ON memory_records(package, package_version, space, scope_hash, record_type, archived_at);

            CREATE INDEX IF NOT EXISTS idx_memory_records_sequence
                ON memory_records(package, package_version, space, scope_hash, ordinal);

            CREATE INDEX IF NOT EXISTS idx_memory_records_expires_at
                ON memory_records(expires_at);

            CREATE UNIQUE INDEX IF NOT EXISTS idx_memory_records_document_current
                ON memory_records(package, package_version, space, scope_hash)
                WHERE space_model = 'document' AND archived_at IS NULL;

            CREATE TABLE IF NOT EXISTS memory_sequence_state (
                package TEXT NOT NULL,
                package_version TEXT NOT NULL,
                space TEXT NOT NULL,
                scope_json TEXT NOT NULL,
                scope_hash TEXT NOT NULL,
                next_ordinal INTEGER NOT NULL,
                updated_at TEXT NOT NULL,
                PRIMARY KEY(package, package_version, space, scope_hash)
            );

            CREATE TABLE IF NOT EXISTS memory_operation_state (
                package TEXT NOT NULL,
                package_version TEXT NOT NULL,
                operation TEXT NOT NULL,
                scope_json TEXT NOT NULL,
                scope_hash TEXT NOT NULL,
                trigger_type TEXT NOT NULL,
                armed INTEGER NOT NULL,
                baseline_at TEXT,
                last_completed_at TEXT,
                next_eligible_at TEXT,
                last_observed_value INTEGER,
                watermark_json TEXT,
                updated_at TEXT NOT NULL,
                PRIMARY KEY(package, package_version, operation, scope_hash)
            );

            CREATE TABLE IF NOT EXISTS memory_vectors (
                record_id TEXT NOT NULL,
                package TEXT NOT NULL,
                package_version TEXT NOT NULL,
                space TEXT NOT NULL,
                record_type TEXT NOT NULL,
                scope_hash TEXT NOT NULL,
                embedding_provider TEXT NOT NULL,
                embedding_model TEXT NOT NULL,
                dimensions INTEGER NOT NULL,
                content_hash TEXT NOT NULL,
                vector BLOB NOT NULL,
                updated_at TEXT NOT NULL,
                PRIMARY KEY(package, package_version, space, scope_hash, record_id, embedding_provider, embedding_model),
                FOREIGN KEY(package, package_version, space, scope_hash, record_id)
                    REFERENCES memory_records(package, package_version, space, scope_hash, id)
            );

            CREATE INDEX IF NOT EXISTS idx_memory_vectors_lookup
                ON memory_vectors(package, package_version, space, record_type, scope_hash, embedding_provider, embedding_model);
            "#,
        )?;

        Ok(())
    }

    fn create_schema_v2(&self) -> Result<()> {
        self.connection
            .execute_batch(
                r#"
                ALTER TABLE memory_operation_state ADD COLUMN last_failed_at TEXT;
                ALTER TABLE memory_operation_state ADD COLUMN last_failure_json TEXT;
                "#,
            )
            .context("migrating Memory SQLite schema to v2")
    }
}

fn lifecycle_watermark_with_commit_records(
    watermark: Option<Value>,
    output_record_ids: &[String],
    source_record_ids: &[String],
) -> Value {
    let mut object = match watermark {
        Some(Value::Object(object)) => object,
        Some(value) => serde_json::Map::from_iter([("previous".into(), value)]),
        None => serde_json::Map::new(),
    };
    object.insert(
        "last_output_record_ids".into(),
        json!(output_record_ids.to_vec()),
    );
    object.insert(
        "last_mutated_source_record_ids".into(),
        json!(source_record_ids.to_vec()),
    );
    Value::Object(object)
}

fn validate_lifecycle_trigger_precondition(
    connection: &Connection,
    precondition: &LocalMemoryLifecycleTriggerPrecondition,
    now: DateTime<Utc>,
) -> Result<()> {
    match precondition {
        LocalMemoryLifecycleTriggerPrecondition::ActiveCountAtLeast {
            package,
            package_version,
            space,
            scope,
            threshold,
        } => {
            let count = active_memory_record_count(
                connection,
                package,
                package_version,
                space,
                scope,
                None,
                now,
            )?;
            if count < *threshold {
                return Err(LocalMemoryActionError::constraint_violation(format!(
                    "Memory lifecycle trigger for space `{space}` no longer has active_count >= threshold {threshold} before commit"
                ))
                .into());
            }
        }
        LocalMemoryLifecycleTriggerPrecondition::ActiveCountAtCapacity {
            package,
            package_version,
            space,
            scope,
            max_records,
        } => {
            let count = active_memory_record_count(
                connection,
                package,
                package_version,
                space,
                scope,
                None,
                now,
            )?;
            if count < *max_records {
                return Err(LocalMemoryActionError::constraint_violation(format!(
                    "Memory lifecycle capacity trigger for space `{space}` no longer has active_count >= max_records {max_records} before commit"
                ))
                .into());
            }
        }
    }
    Ok(())
}

impl LocalSqliteMemoryBatch<'_> {
    pub fn insert_record(&self, record: &LocalMemoryRecordRow) -> Result<()> {
        insert_memory_record(&self.transaction, record)
    }

    pub fn get_record(
        &self,
        package: &str,
        package_version: &str,
        space: &str,
        scope: &BTreeMap<String, String>,
        record_id: &str,
    ) -> Result<Option<StoredMemoryRecord>> {
        get_memory_record(
            &self.transaction,
            package,
            package_version,
            space,
            scope,
            record_id,
            Utc::now(),
        )
    }

    pub fn active_record_count(
        &self,
        package: &str,
        package_version: &str,
        space: &str,
        scope: &BTreeMap<String, String>,
        record_type: Option<&str>,
    ) -> Result<u64> {
        active_memory_record_count(
            &self.transaction,
            package,
            package_version,
            space,
            scope,
            record_type,
            Utc::now(),
        )
    }

    pub fn allocate_sequence_ordinal(
        &self,
        package: &str,
        package_version: &str,
        space: &str,
        scope: &BTreeMap<String, String>,
    ) -> Result<i64> {
        allocate_memory_sequence_ordinal(&self.transaction, package, package_version, space, scope)
    }

    pub fn store_operation_state(&self, state: &LocalMemoryOperationStateRow) -> Result<()> {
        store_memory_operation_state(&self.transaction, state)
    }

    pub fn load_operation_state(
        &self,
        package: &str,
        package_version: &str,
        operation: &str,
        scope: &BTreeMap<String, String>,
    ) -> Result<Option<StoredMemoryOperationState>> {
        load_memory_operation_state(
            &self.transaction,
            package,
            package_version,
            operation,
            scope,
        )
    }
}

fn apply_lifecycle_write_in_transaction(
    batch: &mut LocalSqliteMemoryBatch<'_>,
    request: LocalMemoryWriteRequest<'_>,
) -> Result<Option<String>> {
    validate_memory_scope(request.manifest, request.space, &request.scope)?;
    let space = memory_space(request.manifest, request.space)?;
    if !space
        .record_types
        .contains(&request.record_type.to_string())
    {
        return Err(LocalMemoryActionError::contract_violation(format!(
            "record type `{}` is not permitted in Memory space `{}`",
            request.record_type, request.space
        ))
        .into());
    }
    if matches!(
        request.operation,
        LocalMemoryWriteOperation::Create | LocalMemoryWriteOperation::Upsert
    ) && request.record_id.is_some()
    {
        return Err(LocalMemoryActionError::constraint_violation(
            "Memory create/upsert cannot assign an authoritative record id",
        )
        .into());
    }

    let prepared = match request.operation {
        LocalMemoryWriteOperation::Create | LocalMemoryWriteOperation::Upsert => {
            Some(prepare_local_memory_record(&request, None)?)
        }
        LocalMemoryWriteOperation::Update => {
            let record_id = request.record_id.as_deref().ok_or_else(|| {
                LocalMemoryActionError::constraint_violation(
                    "Memory update requires an existing record id",
                )
            })?;
            Some(prepare_local_memory_record(&request, Some(record_id))?)
        }
        LocalMemoryWriteOperation::Delete | LocalMemoryWriteOperation::Archive => None,
    };

    match request.operation {
        LocalMemoryWriteOperation::Create | LocalMemoryWriteOperation::Upsert => {
            let mut record = prepared.expect("record prepared for create/upsert");
            let existing_id = if matches!(space.model, MemorySpaceModel::Document) {
                find_current_document_id(
                    &batch.transaction,
                    request.package,
                    request.package_version,
                    request.space,
                    &request.scope,
                    request.now,
                )?
            } else {
                None
            };
            if existing_id.is_some()
                && matches!(request.operation, LocalMemoryWriteOperation::Create)
            {
                return Err(LocalMemoryActionError::constraint_violation(format!(
                    "Memory document create for space `{}` requires no current document for the resolved scope; use upsert to replace the current document",
                    request.space
                ))
                .into());
            }
            enforce_memory_capacity(&batch.transaction, &request, space, existing_id.is_none())?;
            if let Some(existing_id) = existing_id {
                record.id = existing_id;
                validate_memory_record_envelope(request.contracts, &record)?;
                update_memory_record(&batch.transaction, &record)?;
            } else {
                if matches!(space.model, MemorySpaceModel::Sequence) {
                    record.ordinal = Some(batch.allocate_sequence_ordinal(
                        request.package,
                        request.package_version,
                        request.space,
                        &request.scope,
                    )?);
                }
                validate_memory_record_envelope(request.contracts, &record)?;
                insert_memory_record(&batch.transaction, &record)?;
            }
            delete_memory_vectors_for_record(
                &batch.transaction,
                request.package,
                request.package_version,
                request.space,
                &request.scope,
                &record.id,
            )?;
            Ok(Some(record.id))
        }
        LocalMemoryWriteOperation::Update => {
            let mut record = prepared.expect("record prepared for update");
            let record_id = request.record_id.as_deref().unwrap();
            let existing = get_memory_record(
                &batch.transaction,
                request.package,
                request.package_version,
                request.space,
                &request.scope,
                record_id,
                request.now,
            )?
            .ok_or_else(|| {
                LocalMemoryActionError::not_found(format!(
                    "Memory record `{record_id}` was not found"
                ))
            })?;
            if !matches!(space.model, MemorySpaceModel::Document)
                && existing.record_type != request.record_type
            {
                return Err(LocalMemoryActionError::constraint_violation(format!(
                    "Memory update target `{record_id}` has record type `{}` not `{}`",
                    existing.record_type, request.record_type
                ))
                .into());
            }
            record.id = existing.id;
            record.created_at = parse_rfc3339_utc(&existing.created_at)?;
            record.ordinal = existing.ordinal;
            validate_memory_record_envelope(request.contracts, &record)?;
            update_memory_record(&batch.transaction, &record)?;
            delete_memory_vectors_for_record(
                &batch.transaction,
                request.package,
                request.package_version,
                request.space,
                &request.scope,
                &record.id,
            )?;
            Ok(Some(record.id))
        }
        LocalMemoryWriteOperation::Delete => {
            let record_id = request.record_id.as_deref().ok_or_else(|| {
                LocalMemoryActionError::constraint_violation(
                    "Memory delete requires an existing record id",
                )
            })?;
            delete_memory_record(
                &batch.transaction,
                request.package,
                request.package_version,
                request.space,
                &request.scope,
                record_id,
            )?;
            Ok(Some(record_id.to_string()))
        }
        LocalMemoryWriteOperation::Archive => {
            let record_id = request.record_id.as_deref().ok_or_else(|| {
                LocalMemoryActionError::constraint_violation(
                    "Memory archive requires an existing record id",
                )
            })?;
            archive_memory_record(
                &batch.transaction,
                request.package,
                request.package_version,
                request.space,
                &request.scope,
                record_id,
                request.now,
            )?;
            Ok(Some(record_id.to_string()))
        }
    }
}

fn insert_memory_record(connection: &Connection, record: &LocalMemoryRecordRow) -> Result<()> {
    let (scope_json, scope_hash) = LocalSqliteMemoryRuntime::scope_identity(&record.scope)?;
    connection
        .execute(
            r#"
            INSERT INTO memory_records (
                id, package, package_version, space, space_model, record_type,
                schema_version, scope_json, scope_hash, content_json, provenance_json,
                created_at, updated_at, expires_at, archived_at, ordinal
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)
            "#,
            params![
                &record.id,
                &record.package,
                &record.package_version,
                &record.space,
                memory_space_model_name(&record.space_model),
                &record.record_type,
                &record.schema_version,
                scope_json,
                scope_hash,
                serde_json::to_string(&record.content)?,
                serde_json::to_string(&record.provenance)?,
                record.created_at.to_rfc3339(),
                record.updated_at.to_rfc3339(),
                record.expires_at.as_ref().map(DateTime::to_rfc3339),
                record.archived_at.as_ref().map(DateTime::to_rfc3339),
                record.ordinal,
            ],
        )
        .context("inserting Memory record")?;
    Ok(())
}

fn get_memory_record(
    connection: &Connection,
    package: &str,
    package_version: &str,
    space: &str,
    scope: &BTreeMap<String, String>,
    record_id: &str,
    now: DateTime<Utc>,
) -> Result<Option<StoredMemoryRecord>> {
    let (_scope_json, scope_hash) = LocalSqliteMemoryRuntime::scope_identity(scope)?;
    connection
        .query_row(
            r#"
            SELECT id, package, package_version, space, space_model, record_type,
                   schema_version, scope_json, scope_hash, content_json, provenance_json,
                   created_at, updated_at, expires_at, archived_at, ordinal
            FROM memory_records
            WHERE package = ?1 AND package_version = ?2 AND space = ?3
              AND scope_hash = ?4 AND id = ?5 AND archived_at IS NULL
              AND (expires_at IS NULL OR expires_at > ?6)
            "#,
            params![
                package,
                package_version,
                space,
                scope_hash,
                record_id,
                now.to_rfc3339()
            ],
            stored_memory_record_from_row,
        )
        .optional()
        .context("reading Memory record")
}

fn stored_memory_record_from_row(row: &Row<'_>) -> rusqlite::Result<StoredMemoryRecord> {
    let content_json: String = row.get(9)?;
    let provenance_json: String = row.get(10)?;
    Ok(StoredMemoryRecord {
        id: row.get(0)?,
        package: row.get(1)?,
        package_version: row.get(2)?,
        space: row.get(3)?,
        space_model: row.get(4)?,
        record_type: row.get(5)?,
        schema_version: row.get(6)?,
        scope_json: row.get(7)?,
        scope_hash: row.get(8)?,
        content: serde_json::from_str(&content_json).map_err(|err| {
            rusqlite::Error::FromSqlConversionFailure(9, rusqlite::types::Type::Text, Box::new(err))
        })?,
        provenance: serde_json::from_str(&provenance_json).map_err(|err| {
            rusqlite::Error::FromSqlConversionFailure(
                10,
                rusqlite::types::Type::Text,
                Box::new(err),
            )
        })?,
        created_at: row.get(11)?,
        updated_at: row.get(12)?,
        expires_at: row.get(13)?,
        archived_at: row.get(14)?,
        ordinal: row.get(15)?,
    })
}

fn update_memory_record(connection: &Connection, record: &LocalMemoryRecordRow) -> Result<()> {
    let (_scope_json, scope_hash) = LocalSqliteMemoryRuntime::scope_identity(&record.scope)?;
    let rows = connection
        .execute(
            r#"
            UPDATE memory_records
            SET record_type = ?6,
                schema_version = ?7,
                content_json = ?8,
                provenance_json = ?9,
                updated_at = ?10,
                expires_at = ?11,
                archived_at = NULL,
                ordinal = ?12
            WHERE package = ?1 AND package_version = ?2 AND space = ?3
              AND scope_hash = ?4 AND id = ?5 AND archived_at IS NULL
            "#,
            params![
                &record.package,
                &record.package_version,
                &record.space,
                scope_hash,
                &record.id,
                &record.record_type,
                &record.schema_version,
                serde_json::to_string(&record.content)?,
                serde_json::to_string(&record.provenance)?,
                record.updated_at.to_rfc3339(),
                record.expires_at.as_ref().map(DateTime::to_rfc3339),
                record.ordinal,
            ],
        )
        .context("updating Memory record")?;
    if rows == 0 {
        bail!("Memory record `{}` was not found", record.id);
    }
    Ok(())
}

fn find_current_document_id(
    connection: &Connection,
    package: &str,
    package_version: &str,
    space: &str,
    scope: &BTreeMap<String, String>,
    now: DateTime<Utc>,
) -> Result<Option<String>> {
    let (_scope_json, scope_hash) = LocalSqliteMemoryRuntime::scope_identity(scope)?;
    connection
        .query_row(
            r#"
            SELECT id
            FROM memory_records
            WHERE package = ?1 AND package_version = ?2 AND space = ?3
              AND scope_hash = ?4 AND space_model = 'document'
              AND archived_at IS NULL AND (expires_at IS NULL OR expires_at > ?5)
            ORDER BY updated_at DESC, id ASC
            LIMIT 1
            "#,
            params![
                package,
                package_version,
                space,
                scope_hash,
                now.to_rfc3339()
            ],
            |row| row.get(0),
        )
        .optional()
        .context("finding current Memory document")
}

fn delete_memory_record(
    connection: &Connection,
    package: &str,
    package_version: &str,
    space: &str,
    scope: &BTreeMap<String, String>,
    record_id: &str,
) -> Result<()> {
    delete_memory_vectors_for_record(
        connection,
        package,
        package_version,
        space,
        scope,
        record_id,
    )?;
    let (_scope_json, scope_hash) = LocalSqliteMemoryRuntime::scope_identity(scope)?;
    let rows = connection
        .execute(
            r#"
            DELETE FROM memory_records
            WHERE package = ?1 AND package_version = ?2 AND space = ?3
              AND scope_hash = ?4 AND id = ?5 AND archived_at IS NULL
            "#,
            params![package, package_version, space, scope_hash, record_id],
        )
        .context("deleting Memory record")?;
    if rows == 0 {
        return Err(LocalMemoryActionError::not_found(format!(
            "Memory record `{record_id}` was not found"
        ))
        .into());
    }
    Ok(())
}

fn archive_memory_record(
    connection: &Connection,
    package: &str,
    package_version: &str,
    space: &str,
    scope: &BTreeMap<String, String>,
    record_id: &str,
    now: DateTime<Utc>,
) -> Result<()> {
    delete_memory_vectors_for_record(
        connection,
        package,
        package_version,
        space,
        scope,
        record_id,
    )?;
    let (_scope_json, scope_hash) = LocalSqliteMemoryRuntime::scope_identity(scope)?;
    let rows = connection
        .execute(
            r#"
            UPDATE memory_records
            SET archived_at = ?6, updated_at = ?6
            WHERE package = ?1 AND package_version = ?2 AND space = ?3
              AND scope_hash = ?4 AND id = ?5 AND archived_at IS NULL
            "#,
            params![
                package,
                package_version,
                space,
                scope_hash,
                record_id,
                now.to_rfc3339()
            ],
        )
        .context("archiving Memory record")?;
    if rows == 0 {
        return Err(LocalMemoryActionError::not_found(format!(
            "Memory record `{record_id}` was not found"
        ))
        .into());
    }
    Ok(())
}

fn active_memory_record_count(
    connection: &Connection,
    package: &str,
    package_version: &str,
    space: &str,
    scope: &BTreeMap<String, String>,
    record_type: Option<&str>,
    now: DateTime<Utc>,
) -> Result<u64> {
    let (_scope_json, scope_hash) = LocalSqliteMemoryRuntime::scope_identity(scope)?;
    let count = match record_type {
        Some(record_type) => connection.query_row(
            r#"
            SELECT COUNT(*)
            FROM memory_records
            WHERE package = ?1 AND package_version = ?2 AND space = ?3
              AND scope_hash = ?4 AND record_type = ?5 AND archived_at IS NULL
              AND (expires_at IS NULL OR expires_at > ?6)
            "#,
            params![
                package,
                package_version,
                space,
                scope_hash,
                record_type,
                now.to_rfc3339()
            ],
            |row| row.get::<_, u64>(0),
        ),
        None => connection.query_row(
            r#"
            SELECT COUNT(*)
            FROM memory_records
            WHERE package = ?1 AND package_version = ?2 AND space = ?3
              AND scope_hash = ?4 AND archived_at IS NULL
              AND (expires_at IS NULL OR expires_at > ?5)
            "#,
            params![
                package,
                package_version,
                space,
                scope_hash,
                now.to_rfc3339()
            ],
            |row| row.get::<_, u64>(0),
        ),
    }
    .context("counting active Memory records")?;
    Ok(count)
}

fn prepare_local_memory_record(
    request: &LocalMemoryWriteRequest<'_>,
    existing_record_id: Option<&str>,
) -> Result<LocalMemoryRecordRow> {
    let content = request.content.as_ref().ok_or_else(|| {
        LocalMemoryActionError::constraint_violation("Memory create/update requires record content")
    })?;
    let space = memory_space(request.manifest, request.space)?;
    let schema_version = request
        .manifest
        .memory
        .record_types
        .get(request.record_type)
        .ok_or_else(|| {
            LocalMemoryActionError::contract_violation(format!(
                "unknown Memory record type `{}`",
                request.record_type
            ))
        })?
        .version
        .clone();
    let durable_content = durable_memory_content_projection_for_record(
        request.contracts,
        request.space,
        request.record_type,
        content,
    )
    .map_err(|err| {
        if has_local_memory_schema_error(&err) {
            err
        } else {
            err.context("projecting durable Memory content")
        }
    })?;

    let id = existing_record_id
        .map(ToString::to_string)
        .unwrap_or_else(allocate_memory_record_id);
    let expires_at = memory_record_expires_at(space, request.now)?;
    let provenance = match &request.provenance {
        Value::Null => json!({}),
        other => other.clone(),
    };
    let record = LocalMemoryRecordRow {
        id,
        package: request.package.to_string(),
        package_version: request.package_version.to_string(),
        space: request.space.to_string(),
        space_model: space.model.clone(),
        record_type: request.record_type.to_string(),
        schema_version,
        scope: request.scope.clone(),
        content: durable_content,
        provenance,
        created_at: request.now,
        updated_at: request.now,
        expires_at,
        archived_at: None,
        ordinal: None,
    };
    Ok(record)
}

pub fn durable_memory_content_projection_for_record(
    contracts: &ValidatedMemoryContracts,
    space: &str,
    record_type: &str,
    content: &Value,
) -> Result<Value> {
    let schemas = memory_contract_schemas(contracts, space, record_type)?;
    validate_json_schema(
        &schemas.content_schema,
        content,
        "full proposed Memory content",
    )
    .map_err(|err| LocalMemoryActionError::contract_violation(err.to_string()))?;
    let durable_content = durable_content_projection(content, &schemas.content_schema)?;
    validate_json_schema(
        &schemas.content_schema,
        &durable_content,
        "durable Memory content projection",
    )
    .map_err(|err| LocalMemoryActionError::contract_violation(err.to_string()))?;
    Ok(durable_content)
}

fn validate_memory_record_envelope(
    contracts: &ValidatedMemoryContracts,
    record: &LocalMemoryRecordRow,
) -> Result<()> {
    let schemas = memory_contract_schemas(contracts, &record.space, &record.record_type)?;
    let mut envelope_schema = schemas.envelope_schema;
    allow_harness_memory_provenance(&mut envelope_schema);
    validate_json_schema(
        &envelope_schema,
        &memory_record_contract_envelope(record),
        "Memory record envelope",
    )
    .map_err(|err| LocalMemoryActionError::contract_violation(err.to_string()).into())
}

fn allow_harness_memory_provenance(envelope_schema: &mut Value) {
    let Some(properties) = envelope_schema
        .get_mut("properties")
        .and_then(Value::as_object_mut)
    else {
        return;
    };
    let Some(provenance) = properties
        .get_mut("provenance")
        .and_then(Value::as_object_mut)
    else {
        return;
    };
    let provenance_properties = provenance.entry("properties").or_insert_with(|| json!({}));
    let Some(provenance_properties) = provenance_properties.as_object_mut() else {
        return;
    };
    provenance_properties
        .entry("harness")
        .or_insert_with(harness_memory_provenance_schema);
    provenance_properties
        .entry("harness_lifecycle")
        .or_insert_with(harness_lifecycle_memory_provenance_schema);
}

fn harness_memory_provenance_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": [
            "kind",
            "run_id",
            "phase_execution_id",
            "phase_id",
            "action_kind",
            "operation"
        ],
        "properties": {
            "kind": {
                "type": "string",
                "minLength": 1
            },
            "run_id": {
                "type": "string",
                "minLength": 1
            },
            "phase_execution_id": {
                "type": "string",
                "minLength": 1
            },
            "phase_id": {
                "type": "string",
                "minLength": 1
            },
            "action_kind": {
                "const": "memory_write"
            },
            "operation": {
                "type": "string",
                "enum": ["create", "upsert", "update", "delete", "archive"]
            },
            "source": {
                "type": "string",
                "minLength": 1
            },
            "model_provider": {
                "type": "string",
                "minLength": 1
            },
            "model_id": {
                "type": "string",
                "minLength": 1
            }
        }
    })
}

fn harness_lifecycle_memory_provenance_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": true,
        "required": [
            "kind",
            "operation",
            "operation_identity",
            "source_record_ids",
            "source_records"
        ],
        "properties": {
            "kind": {
                "const": "harness_memory_lifecycle_operation"
            },
            "operation": {
                "type": "string",
                "minLength": 1
            },
            "operation_identity": {
                "type": "string",
                "minLength": 1
            },
            "source_record_ids": {
                "type": "array",
                "items": { "type": "string", "minLength": 1 }
            },
            "source_records": {
                "type": "array",
                "items": {
                    "type": "object",
                    "additionalProperties": true,
                    "required": ["package", "package_version", "space", "record_type", "id", "scope_hash"],
                    "properties": {
                        "package": { "type": "string", "minLength": 1 },
                        "package_version": { "type": "string", "minLength": 1 },
                        "space": { "type": "string", "minLength": 1 },
                        "record_type": { "type": "string", "minLength": 1 },
                        "id": { "type": "string", "minLength": 1 },
                        "scope_hash": { "type": "string", "minLength": 1 }
                    }
                }
            }
        }
    })
}

fn memory_contract_schemas(
    contracts: &ValidatedMemoryContracts,
    space: &str,
    record_type: &str,
) -> Result<MemoryContractSchemas> {
    let index_entry = contracts
        .index
        .contracts
        .iter()
        .find(|entry| entry.space == space && entry.record_type == record_type)
        .with_context(|| {
            format!(
                "generated Memory contract missing for space `{space}` record type `{record_type}`"
            )
        })?;
    let contract = contracts
        .contracts
        .iter()
        .find(|contract| contract.path == index_entry.path)
        .with_context(|| {
            format!(
                "generated Memory contract file `{}` was not loaded",
                index_entry.path
            )
        })?;
    let envelope_schema: Value = serde_json::from_slice(&contract.schema_bytes)
        .with_context(|| format!("parsing generated Memory contract `{}`", contract.path))?;
    let content_schema = envelope_schema
        .pointer("/properties/content")
        .cloned()
        .with_context(|| {
            format!(
                "generated Memory contract `{}` has no content schema",
                contract.path
            )
        })?;
    Ok(MemoryContractSchemas {
        envelope_schema,
        content_schema,
    })
}

pub fn generated_memory_content_schema(
    contracts: &ValidatedMemoryContracts,
    space: &str,
    record_type: &str,
) -> Result<Value> {
    Ok(memory_contract_schemas(contracts, space, record_type)?.content_schema)
}

#[derive(Debug, Clone)]
struct MemoryContractSchemas {
    envelope_schema: Value,
    content_schema: Value,
}

fn validate_json_schema(schema: &Value, instance: &Value, label: &str) -> Result<()> {
    let compile_schema = schema_for_standalone_compile(schema);
    let compiled = JSONSchema::options()
        .with_draft(Draft::Draft202012)
        .compile(&compile_schema)
        .map_err(|err| anyhow!("compiling {label} schema failed: {err}"))?;
    if let Err(errors) = compiled.validate(instance) {
        let details = errors
            .take(5)
            .map(|err| err.to_string())
            .collect::<Vec<_>>()
            .join("; ");
        bail!("{label} validation failed: {details}");
    }
    Ok(())
}

fn schema_for_standalone_compile(schema: &Value) -> Value {
    let mut schema = schema.clone();
    if let Some(object) = schema.as_object_mut()
        && let Some(Value::String(id)) = object.get("$id")
        && !id.contains(':')
    {
        object.remove("$id");
    }
    schema
}

fn durable_content_projection(content: &Value, content_schema: &Value) -> Result<Value> {
    project_non_persistable_value(content, content_schema, content_schema, 0)
        .map(|projection| projection.unwrap_or(Value::Null))
}

fn project_non_persistable_value(
    value: &Value,
    schema: &Value,
    root_schema: &Value,
    depth: usize,
) -> Result<Option<Value>> {
    if depth > 128 {
        bail!("Memory persistence governance schema traversal exceeded recursion limit");
    }
    let resolved_schema = resolve_local_schema_ref(schema, root_schema).unwrap_or(schema);
    if schema_has_persist_false(resolved_schema) {
        return Ok(None);
    }

    let mut projected = value.clone();
    for keyword in ["allOf", "anyOf", "oneOf"] {
        if let Some(subschemas) = resolved_schema.get(keyword).and_then(Value::as_array) {
            for subschema in subschemas {
                projected = match project_non_persistable_value(
                    &projected,
                    subschema,
                    root_schema,
                    depth + 1,
                )? {
                    Some(projected) => projected,
                    None => return Ok(None),
                };
            }
        }
    }

    match (&projected, resolved_schema) {
        (Value::Object(object), Value::Object(schema_object)) => {
            let properties = schema_object.get("properties").and_then(Value::as_object);
            let additional_properties = schema_object.get("additionalProperties");
            let mut durable = Map::new();
            for (key, child_value) in object {
                let child_schema = properties
                    .and_then(|properties| properties.get(key))
                    .or_else(|| additional_properties.filter(|value| value.is_object()));
                let projected_child = match child_schema {
                    Some(child_schema) => project_non_persistable_value(
                        child_value,
                        child_schema,
                        root_schema,
                        depth + 1,
                    )?,
                    None => Some(child_value.clone()),
                };
                if let Some(projected_child) = projected_child {
                    durable.insert(key.clone(), projected_child);
                }
            }
            Ok(Some(Value::Object(durable)))
        }
        (Value::Array(items), Value::Object(schema_object)) => {
            if let Some(item_schema) = schema_object.get("items") {
                let mut durable = Vec::new();
                for item in items {
                    if let Some(projected_item) =
                        project_non_persistable_value(item, item_schema, root_schema, depth + 1)?
                    {
                        durable.push(projected_item);
                    }
                }
                Ok(Some(Value::Array(durable)))
            } else {
                Ok(Some(projected))
            }
        }
        _ => Ok(Some(projected)),
    }
}

fn schema_has_persist_false(schema: &Value) -> bool {
    schema
        .as_object()
        .and_then(|object| object.get("x-agentpm-persist"))
        .and_then(Value::as_bool)
        == Some(false)
}

fn resolve_local_schema_ref<'a>(schema: &'a Value, root_schema: &'a Value) -> Option<&'a Value> {
    let reference = schema.as_object()?.get("$ref")?.as_str()?;
    let pointer = reference.strip_prefix('#')?;
    root_schema.pointer(pointer)
}

fn memory_record_contract_envelope(record: &LocalMemoryRecordRow) -> Value {
    let mut envelope = Map::new();
    envelope.insert("id".into(), Value::String(record.id.clone()));
    envelope.insert(
        "record_type".into(),
        Value::String(record.record_type.clone()),
    );
    envelope.insert("space".into(), Value::String(record.space.clone()));
    envelope.insert(
        "scope".into(),
        serde_json::to_value(&record.scope).unwrap_or_else(|_| json!({})),
    );
    envelope.insert(
        "schema_version".into(),
        Value::String(record.schema_version.clone()),
    );
    envelope.insert(
        "created_at".into(),
        Value::String(record.created_at.to_rfc3339()),
    );
    envelope.insert(
        "updated_at".into(),
        Value::String(record.updated_at.to_rfc3339()),
    );
    if let Some(expires_at) = record.expires_at.as_ref() {
        envelope.insert("expires_at".into(), Value::String(expires_at.to_rfc3339()));
    }
    if let Some(ordinal) = record.ordinal {
        envelope.insert("ordinal".into(), Value::Number(ordinal.into()));
    }
    envelope.insert("provenance".into(), record.provenance.clone());
    envelope.insert("content".into(), record.content.clone());
    Value::Object(envelope)
}

fn memory_space<'a>(manifest: &'a MemoryManifest, space: &str) -> Result<&'a MemorySpace> {
    manifest
        .memory
        .spaces
        .get(space)
        .with_context(|| format!("unknown Memory space `{space}`"))
}

fn validate_memory_scope(
    manifest: &MemoryManifest,
    space_name: &str,
    scope: &BTreeMap<String, String>,
) -> Result<()> {
    let space = memory_space(manifest, space_name)?;
    let expected = space.scope.iter().cloned().collect::<BTreeSetLike>();
    let actual = scope.keys().cloned().collect::<BTreeSetLike>();
    if actual != expected {
        bail!("Memory scope for space `{space_name}` must match declared complete scope tuple");
    }
    if scope.values().any(String::is_empty) {
        bail!("Memory scope for space `{space_name}` contains an empty value");
    }
    Ok(())
}

type BTreeSetLike = std::collections::BTreeSet<String>;

fn append_only_enabled(space: &MemorySpace) -> bool {
    space
        .constraints
        .as_ref()
        .and_then(|constraints| constraints.append_only)
        .unwrap_or(false)
}

fn memory_record_expires_at(
    space: &MemorySpace,
    updated_at: DateTime<Utc>,
) -> Result<Option<DateTime<Utc>>> {
    match &space.retention {
        Some(retention) => Ok(Some(updated_at + parse_memory_ttl(&retention.ttl)?)),
        None => Ok(None),
    }
}

fn parse_memory_ttl(ttl: &str) -> Result<ChronoDuration> {
    let Some(rest) = ttl.strip_prefix('P') else {
        bail!("Memory retention ttl `{ttl}` must be an ISO-8601 duration starting with `P`");
    };
    if rest.is_empty() {
        bail!("Memory retention ttl `{ttl}` is empty");
    }

    let mut in_time = false;
    let mut number = String::new();
    let mut duration = ChronoDuration::zero();
    for ch in rest.chars() {
        if ch == 'T' {
            if in_time {
                bail!("Memory retention ttl `{ttl}` has duplicate time marker");
            }
            in_time = true;
            continue;
        }
        if ch.is_ascii_digit() {
            number.push(ch);
            continue;
        }
        if number.is_empty() {
            bail!("Memory retention ttl `{ttl}` has a unit without a value");
        }
        let value = number
            .parse::<i64>()
            .with_context(|| format!("parsing Memory retention ttl `{ttl}`"))?;
        number.clear();
        duration = match (in_time, ch) {
            (false, 'D') => duration + ChronoDuration::days(value),
            (true, 'H') => duration + ChronoDuration::hours(value),
            (true, 'M') => duration + ChronoDuration::minutes(value),
            (true, 'S') => duration + ChronoDuration::seconds(value),
            _ => bail!("Memory retention ttl `{ttl}` uses unsupported unit `{ch}`"),
        };
    }
    if !number.is_empty() || duration <= ChronoDuration::zero() {
        bail!("Memory retention ttl `{ttl}` is not a positive supported duration");
    }
    Ok(duration)
}

fn enforce_memory_capacity(
    connection: &Connection,
    request: &LocalMemoryWriteRequest<'_>,
    space: &MemorySpace,
    creates_new_active: bool,
) -> Result<()> {
    if !creates_new_active {
        return Ok(());
    }
    let Some(capacity) = &space.capacity else {
        return Ok(());
    };
    let active_count = active_memory_record_count(
        connection,
        request.package,
        request.package_version,
        request.space,
        &request.scope,
        None,
        request.now,
    )?;
    if active_count >= capacity.max_records {
        return Err(LocalMemoryActionError::capacity_exceeded(format!(
            "Memory space `{}` scope already has {} active records; max_records is {}",
            request.space, active_count, capacity.max_records
        ))
        .into());
    }
    Ok(())
}

fn expire_memory_records_for_space(
    connection: &Connection,
    package: &str,
    package_version: &str,
    space: &str,
    space_manifest: &MemorySpace,
    now: DateTime<Utc>,
) -> Result<()> {
    let Some(retention) = &space_manifest.retention else {
        return Ok(());
    };
    match retention.on_expire {
        MemoryRetentionAction::Archive => {
            connection
                .execute(
                    r#"
                    DELETE FROM memory_vectors
                    WHERE package = ?1 AND package_version = ?2 AND space = ?3
                      AND record_id IN (
                          SELECT id
                          FROM memory_records
                          WHERE package = ?1 AND package_version = ?2 AND space = ?3
                            AND archived_at IS NULL AND expires_at IS NOT NULL AND expires_at <= ?4
                      )
                    "#,
                    params![package, package_version, space, now.to_rfc3339()],
                )
                .context("deleting vectors for expired archived Memory records")?;
            connection
                .execute(
                    r#"
                    UPDATE memory_records
                    SET archived_at = ?4, updated_at = ?4
                    WHERE package = ?1 AND package_version = ?2 AND space = ?3
                      AND archived_at IS NULL AND expires_at IS NOT NULL AND expires_at <= ?4
                    "#,
                    params![package, package_version, space, now.to_rfc3339()],
                )
                .context("archiving expired Memory records")?;
        }
        MemoryRetentionAction::Delete => {
            connection
                .execute(
                    r#"
                    DELETE FROM memory_vectors
                    WHERE package = ?1 AND package_version = ?2 AND space = ?3
                      AND record_id IN (
                          SELECT id
                          FROM memory_records
                          WHERE package = ?1 AND package_version = ?2 AND space = ?3
                            AND archived_at IS NULL AND expires_at IS NOT NULL AND expires_at <= ?4
                      )
                    "#,
                    params![package, package_version, space, now.to_rfc3339()],
                )
                .context("deleting vectors for expired Memory records")?;
            connection
                .execute(
                    r#"
                    DELETE FROM memory_records
                    WHERE package = ?1 AND package_version = ?2 AND space = ?3
                      AND archived_at IS NULL AND expires_at IS NOT NULL AND expires_at <= ?4
                    "#,
                    params![package, package_version, space, now.to_rfc3339()],
                )
                .context("deleting expired Memory records")?;
        }
    }
    Ok(())
}

fn read_memory_records_by_key(
    connection: &Connection,
    request: &LocalMemoryReadRequest<'_>,
) -> Result<Vec<StoredMemoryRecord>> {
    ensure_retrieval_mode(request.manifest, request.space, MemoryRetrievalMode::Key)?;
    if let Some(record_id) = &request.record_id {
        return Ok(get_memory_record(
            connection,
            request.package,
            request.package_version,
            request.space,
            &request.scope,
            record_id,
            request.now,
        )?
        .into_iter()
        .collect());
    }

    let space = memory_space(request.manifest, request.space)?;
    if !matches!(space.model, MemorySpaceModel::Document) {
        bail!("Memory key read requires a record id for non-document spaces");
    }
    let Some(record_id) = find_current_document_id(
        connection,
        request.package,
        request.package_version,
        request.space,
        &request.scope,
        request.now,
    )?
    else {
        return Ok(Vec::new());
    };
    Ok(get_memory_record(
        connection,
        request.package,
        request.package_version,
        request.space,
        &request.scope,
        &record_id,
        request.now,
    )?
    .into_iter()
    .collect())
}

fn read_memory_records_by_filter(
    connection: &Connection,
    request: &LocalMemoryReadRequest<'_>,
) -> Result<Vec<StoredMemoryRecord>> {
    ensure_retrieval_mode(request.manifest, request.space, MemoryRetrievalMode::Filter)?;
    let mut records = query_active_memory_records(
        connection,
        request,
        "updated_at DESC, created_at DESC, id ASC",
        None,
    )?;
    records.retain(|record| content_matches_filter(&record.content, &request.filter));
    if let Some(limit) = request.limit {
        records.truncate(limit);
    }
    Ok(records)
}

fn read_memory_records_chronological(
    connection: &Connection,
    request: &LocalMemoryReadRequest<'_>,
) -> Result<Vec<StoredMemoryRecord>> {
    ensure_retrieval_mode(
        request.manifest,
        request.space,
        MemoryRetrievalMode::Chronological,
    )?;
    query_active_memory_records(
        connection,
        request,
        "ordinal ASC, created_at ASC, id ASC",
        request.limit,
    )
}

fn read_memory_records_full_text(
    connection: &Connection,
    request: &LocalMemoryReadRequest<'_>,
) -> Result<Vec<StoredMemoryRecord>> {
    ensure_retrieval_mode(
        request.manifest,
        request.space,
        MemoryRetrievalMode::FullText,
    )?;
    let query = request
        .query
        .as_deref()
        .context("Memory full_text read requires a query")?
        .to_lowercase();
    let mut records = query_active_memory_records(
        connection,
        request,
        "updated_at DESC, created_at DESC, id ASC",
        None,
    )?;
    records.retain(|record| content_contains_text(&record.content, &query));
    if let Some(limit) = request.limit {
        records.truncate(limit);
    }
    Ok(records)
}

fn ensure_retrieval_mode(
    manifest: &MemoryManifest,
    space: &str,
    mode: MemoryRetrievalMode,
) -> Result<()> {
    let space_manifest = memory_space(manifest, space)?;
    if !space_manifest.retrieval.modes.contains(&mode) {
        bail!(
            "Memory space `{space}` does not declare retrieval mode `{:?}`",
            mode
        );
    }
    Ok(())
}

fn content_matches_filter(content: &Value, filter: &BTreeMap<String, Value>) -> bool {
    filter.iter().all(|(key, expected)| {
        content_matches_filter_path(content, &filter_path_segments(key), expected)
    })
}

fn filter_path_segments(path: &str) -> Vec<&str> {
    path.split('.').collect()
}

fn content_matches_filter_path(value: &Value, path: &[&str], expected: &Value) -> bool {
    if path.is_empty() && value == expected {
        return true;
    }

    match value {
        Value::Array(items) => items
            .iter()
            .any(|item| content_matches_filter_path(item, path, expected)),
        Value::Object(object) if !path.is_empty() => object
            .get(path[0])
            .is_some_and(|child| content_matches_filter_path(child, &path[1..], expected)),
        _ => false,
    }
}

fn content_contains_text(value: &Value, query: &str) -> bool {
    match value {
        Value::String(text) => text.to_lowercase().contains(query),
        Value::Array(items) => items.iter().any(|item| content_contains_text(item, query)),
        Value::Object(object) => object
            .values()
            .any(|item| content_contains_text(item, query)),
        _ => false,
    }
}

fn query_active_memory_records(
    connection: &Connection,
    request: &LocalMemoryReadRequest<'_>,
    order_by: &str,
    limit: Option<usize>,
) -> Result<Vec<StoredMemoryRecord>> {
    let (_scope_json, scope_hash) = LocalSqliteMemoryRuntime::scope_identity(&request.scope)?;
    let now = request.now.to_rfc3339();
    let mut sql = r#"
        SELECT id, package, package_version, space, space_model, record_type,
               schema_version, scope_json, scope_hash, content_json, provenance_json,
               created_at, updated_at, expires_at, archived_at, ordinal
        FROM memory_records
        WHERE package = ?1 AND package_version = ?2 AND space = ?3
          AND scope_hash = ?4 AND archived_at IS NULL
          AND (expires_at IS NULL OR expires_at > ?5)
        "#
    .to_string();
    if request.record_type.is_some() {
        sql.push_str(" AND record_type = ?6");
    }
    sql.push_str(" ORDER BY ");
    sql.push_str(order_by);
    if limit.is_some() && request.record_type.is_none() {
        sql.push_str(" LIMIT ?6");
    } else if limit.is_some() {
        sql.push_str(" LIMIT ?7");
    }

    let mut statement = connection
        .prepare(&sql)
        .context("preparing Memory read query")?;
    let records = match (request.record_type.as_deref(), limit) {
        (Some(record_type), Some(limit)) => statement
            .query_map(
                params![
                    request.package,
                    request.package_version,
                    request.space,
                    scope_hash,
                    now,
                    record_type,
                    limit as i64
                ],
                stored_memory_record_from_row,
            )?
            .collect::<rusqlite::Result<Vec<_>>>()?,
        (Some(record_type), None) => statement
            .query_map(
                params![
                    request.package,
                    request.package_version,
                    request.space,
                    scope_hash,
                    now,
                    record_type
                ],
                stored_memory_record_from_row,
            )?
            .collect::<rusqlite::Result<Vec<_>>>()?,
        (None, Some(limit)) => statement
            .query_map(
                params![
                    request.package,
                    request.package_version,
                    request.space,
                    scope_hash,
                    now,
                    limit as i64
                ],
                stored_memory_record_from_row,
            )?
            .collect::<rusqlite::Result<Vec<_>>>()?,
        (None, None) => statement
            .query_map(
                params![
                    request.package,
                    request.package_version,
                    request.space,
                    scope_hash,
                    now
                ],
                stored_memory_record_from_row,
            )?
            .collect::<rusqlite::Result<Vec<_>>>()?,
    };
    Ok(records)
}

fn allocate_memory_record_id() -> String {
    let counter = MEMORY_RECORD_ID_COUNTER.fetch_add(1, Ordering::Relaxed);
    let timestamp = Utc::now()
        .timestamp_nanos_opt()
        .unwrap_or_else(|| Utc::now().timestamp_micros() * 1_000);
    format!("mem-{timestamp:x}-{counter:x}")
}

fn parse_rfc3339_utc(value: &str) -> Result<DateTime<Utc>> {
    Ok(DateTime::parse_from_rfc3339(value)
        .with_context(|| format!("parsing Memory record timestamp `{value}`"))?
        .with_timezone(&Utc))
}

fn allocate_memory_sequence_ordinal(
    connection: &Connection,
    package: &str,
    package_version: &str,
    space: &str,
    scope: &BTreeMap<String, String>,
) -> Result<i64> {
    let (scope_json, scope_hash) = LocalSqliteMemoryRuntime::scope_identity(scope)?;
    let next_ordinal = connection
        .query_row(
            r#"
            SELECT next_ordinal
            FROM memory_sequence_state
            WHERE package = ?1 AND package_version = ?2 AND space = ?3 AND scope_hash = ?4
            "#,
            params![package, package_version, space, scope_hash],
            |row| row.get::<_, i64>(0),
        )
        .optional()
        .context("reading Memory sequence state")?
        .unwrap_or(0);
    connection
        .execute(
            r#"
            INSERT INTO memory_sequence_state (
                package, package_version, space, scope_json, scope_hash, next_ordinal, updated_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            ON CONFLICT(package, package_version, space, scope_hash)
            DO UPDATE SET next_ordinal = excluded.next_ordinal, updated_at = excluded.updated_at
            "#,
            params![
                package,
                package_version,
                space,
                scope_json,
                scope_hash,
                next_ordinal + 1,
                Utc::now().to_rfc3339(),
            ],
        )
        .context("updating Memory sequence state")?;
    Ok(next_ordinal)
}

fn store_memory_operation_state(
    connection: &Connection,
    state: &LocalMemoryOperationStateRow,
) -> Result<()> {
    let (scope_json, scope_hash) = LocalSqliteMemoryRuntime::scope_identity(&state.scope)?;
    connection
        .execute(
            r#"
            INSERT INTO memory_operation_state (
                package, package_version, operation, scope_json, scope_hash,
                trigger_type, armed, baseline_at, last_completed_at, last_failed_at,
                next_eligible_at, last_observed_value, last_failure_json, watermark_json, updated_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)
            ON CONFLICT(package, package_version, operation, scope_hash)
            DO UPDATE SET
                trigger_type = excluded.trigger_type,
                armed = excluded.armed,
                baseline_at = excluded.baseline_at,
                last_completed_at = excluded.last_completed_at,
                last_failed_at = excluded.last_failed_at,
                next_eligible_at = excluded.next_eligible_at,
                last_observed_value = excluded.last_observed_value,
                last_failure_json = excluded.last_failure_json,
                watermark_json = excluded.watermark_json,
                updated_at = excluded.updated_at
            "#,
            params![
                &state.package,
                &state.package_version,
                &state.operation,
                scope_json,
                scope_hash,
                &state.trigger_type,
                i64::from(state.armed),
                state.baseline_at.as_ref().map(DateTime::to_rfc3339),
                state.last_completed_at.as_ref().map(DateTime::to_rfc3339),
                state.last_failed_at.as_ref().map(DateTime::to_rfc3339),
                state.next_eligible_at.as_ref().map(DateTime::to_rfc3339),
                state.last_observed_value,
                state
                    .last_failure
                    .as_ref()
                    .map(serde_json::to_string)
                    .transpose()?,
                state
                    .watermark
                    .as_ref()
                    .map(serde_json::to_string)
                    .transpose()?,
                state.updated_at.to_rfc3339(),
            ],
        )
        .context("storing Memory operation state")?;
    Ok(())
}

fn load_memory_operation_state(
    connection: &Connection,
    package: &str,
    package_version: &str,
    operation: &str,
    scope: &BTreeMap<String, String>,
) -> Result<Option<StoredMemoryOperationState>> {
    let (_scope_json, scope_hash) = LocalSqliteMemoryRuntime::scope_identity(scope)?;
    connection
        .query_row(
            r#"
            SELECT package, package_version, operation, scope_json, scope_hash,
                   trigger_type, armed, baseline_at, last_completed_at, last_failed_at,
                   next_eligible_at, last_observed_value, last_failure_json, watermark_json
            FROM memory_operation_state
            WHERE package = ?1 AND package_version = ?2 AND operation = ?3 AND scope_hash = ?4
            "#,
            params![package, package_version, operation, scope_hash],
            |row| {
                let last_failure_json: Option<String> = row.get(12)?;
                let watermark_json: Option<String> = row.get(13)?;
                Ok(StoredMemoryOperationState {
                    package: row.get(0)?,
                    package_version: row.get(1)?,
                    operation: row.get(2)?,
                    scope_json: row.get(3)?,
                    scope_hash: row.get(4)?,
                    trigger_type: row.get(5)?,
                    armed: row.get::<_, i64>(6)? != 0,
                    baseline_at: row.get(7)?,
                    last_completed_at: row.get(8)?,
                    last_failed_at: row.get(9)?,
                    next_eligible_at: row.get(10)?,
                    last_observed_value: row.get(11)?,
                    last_failure: last_failure_json
                        .map(|value| {
                            serde_json::from_str(&value).map_err(|err| {
                                rusqlite::Error::FromSqlConversionFailure(
                                    12,
                                    rusqlite::types::Type::Text,
                                    Box::new(err),
                                )
                            })
                        })
                        .transpose()?,
                    watermark: watermark_json
                        .map(|value| {
                            serde_json::from_str(&value).map_err(|err| {
                                rusqlite::Error::FromSqlConversionFailure(
                                    13,
                                    rusqlite::types::Type::Text,
                                    Box::new(err),
                                )
                            })
                        })
                        .transpose()?,
                })
            },
        )
        .optional()
        .context("reading Memory operation state")
}

pub trait MemoryRuntime {
    fn capabilities(&self) -> MemoryRuntimeCapabilityDescriptor;
}

impl MemoryRuntime for LocalSqliteMemoryRuntime {
    fn capabilities(&self) -> MemoryRuntimeCapabilityDescriptor {
        MemoryRuntimeCapabilityDescriptor::local_sqlite()
    }
}

pub fn default_local_memory_state_dir(workspace_root: &Path) -> PathBuf {
    workspace_root.join(".agentpm-state")
}

pub fn ensure_canonical_scope_identity(scope_json: &str, scope_hash: &str) -> Result<()> {
    if !matches!(
        serde_json::from_str::<Value>(scope_json),
        Ok(Value::Object(_))
    ) {
        return Err(anyhow!("Memory scope JSON must be an object"));
    }
    LocalSqliteMemoryRuntime::verify_scope_identity(scope_json, scope_hash)
}

pub fn validate_and_load_memory_contracts(package_root: &Path) -> Result<ValidatedMemoryContracts> {
    let manifest_path = package_root.join("agent.json");
    let executed = execute_memory_build_with_output(&manifest_path, MemoryBuildMode::Check)
        .with_context(|| {
            format!(
                "validating generated Memory contracts for {}",
                package_root.display()
            )
        })?;

    if let Some(check) = executed.check
        && !check.mismatches.is_empty()
    {
        let details = check
            .mismatches
            .iter()
            .map(|mismatch| format!("{}: {}", mismatch.path, mismatch.detail))
            .collect::<Vec<_>>()
            .join("; ");
        bail!("generated Memory contracts are not current: {details}");
    }

    Ok(ValidatedMemoryContracts {
        index: executed.output.index,
        contracts: executed.output.contracts,
    })
}

fn memory_contract_cache_identity(package_root: &Path) -> Result<MemoryContractCacheIdentity> {
    let manifest_hash = sha256_prefixed_bytes(
        &fs::read(package_root.join("agent.json"))
            .with_context(|| format!("reading {}", package_root.join("agent.json").display()))?,
    );

    let build_path = resolve_existing_relative_file(package_root, "memory/build.json")?;
    let build_bytes =
        fs::read(&build_path).with_context(|| format!("reading {}", build_path.display()))?;
    let build_metadata: MemoryBuildMetadata =
        serde_json::from_slice(&build_bytes).context("parsing memory/build.json")?;

    let mut source_schemas = Vec::new();
    for source_schema in &build_metadata.source_schemas {
        let path = resolve_existing_relative_file(package_root, &source_schema.path)?;
        let hash = sha256_prefixed_bytes(
            &fs::read(&path).with_context(|| format!("reading {}", path.display()))?,
        );
        source_schemas.push((source_schema.path.clone(), hash));
    }
    source_schemas.sort();

    let index_path = resolve_existing_relative_file(package_root, "memory/contracts/index.json")?;
    let index_bytes =
        fs::read(&index_path).with_context(|| format!("reading {}", index_path.display()))?;
    let index: MemoryContractIndex =
        serde_json::from_slice(&index_bytes).context("parsing memory/contracts/index.json")?;
    let mut contract_hashes = Vec::new();
    for contract in &index.contracts {
        let path = resolve_existing_relative_file(package_root, &contract.path)?;
        let hash = sha256_prefixed_bytes(
            &fs::read(&path).with_context(|| format!("reading {}", path.display()))?,
        );
        contract_hashes.push((contract.path.clone(), hash));
    }
    contract_hashes.sort();

    Ok(MemoryContractCacheIdentity {
        package_root: package_root.to_path_buf(),
        manifest_hash,
        build_source_manifest_hash: build_metadata.source_manifest_hash,
        source_schemas,
        source_schemas_hash: build_metadata.source_schemas_hash,
        source_contract_inputs_hash: build_metadata.source_contract_inputs_hash,
        build_contracts_index_hash: build_metadata.contracts_index_hash,
        actual_contracts_index_hash: sha256_prefixed_bytes(&index_bytes),
        contracts_hash: build_metadata.contracts_hash,
        contract_count: build_metadata.contract_count,
        contract_hashes,
    })
}

fn memory_contract_artifact_paths(identity: &MemoryContractCacheIdentity) -> Vec<String> {
    let mut paths = vec![
        "agent.json".to_string(),
        "memory/build.json".to_string(),
        "memory/contracts/index.json".to_string(),
    ];
    paths.extend(
        identity
            .source_schemas
            .iter()
            .map(|(path, _hash)| path.clone()),
    );
    paths.extend(
        identity
            .contract_hashes
            .iter()
            .map(|(path, _hash)| path.clone()),
    );
    paths.sort();
    paths.dedup();
    paths
}

fn memory_contract_artifact_fingerprint(
    package_root: &Path,
    artifact_paths: &[String],
) -> Result<MemoryContractArtifactFingerprint> {
    let mut artifacts = Vec::new();
    for artifact_path in artifact_paths {
        let path = resolve_existing_relative_file(package_root, artifact_path)?;
        let metadata = fs::metadata(&path)
            .with_context(|| format!("reading metadata for {}", path.display()))?;
        let modified_nanos = metadata
            .modified()
            .ok()
            .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
            .map(|duration| duration.as_nanos());
        artifacts.push(MemoryContractArtifactStat {
            path: artifact_path.clone(),
            len: metadata.len(),
            modified_nanos,
        });
    }
    artifacts.sort_by(|left, right| left.path.cmp(&right.path));

    Ok(MemoryContractArtifactFingerprint { artifacts })
}

fn memory_space_model_name(model: &MemorySpaceModel) -> &'static str {
    match model {
        MemorySpaceModel::Document => "document",
        MemorySpaceModel::Collection => "collection",
        MemorySpaceModel::Sequence => "sequence",
    }
}

fn sha256_prefixed_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("sha256:{}", hex::encode(hasher.finalize()))
}

#[cfg(test)]
mod tests;
