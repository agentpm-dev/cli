use crate::commands::memory::{
    GeneratedMemoryContract, MemoryBuildMetadata, MemoryBuildMode, MemoryContractIndex,
    execute_memory_build_with_output,
};
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
use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, UNIX_EPOCH};

const LOCAL_MEMORY_SCHEMA_VERSION: u64 = 1;
const LOCAL_MEMORY_DB_NAME: &str = "memory.sqlite3";
const LOCAL_MEMORY_BUSY_TIMEOUT_MS: u64 = 5_000;
static MEMORY_RECORD_ID_COUNTER: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocalMemoryActionErrorKind {
    NotFound,
    CapacityExceeded,
    ConstraintViolation,
    ContractViolation,
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

    fn contract_violation(message: impl Into<String>) -> Self {
        Self::new(LocalMemoryActionErrorKind::ContractViolation, message)
    }

    pub fn code(&self) -> &'static str {
        match self.kind {
            LocalMemoryActionErrorKind::NotFound => "not_found",
            LocalMemoryActionErrorKind::CapacityExceeded => "capacity_exceeded",
            LocalMemoryActionErrorKind::ConstraintViolation => "constraint_violation",
            LocalMemoryActionErrorKind::ContractViolation => "contract_violation",
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

        for retrieval_mode in &space.retrieval.modes {
            if !capabilities.retrieval_modes.contains(retrieval_mode) {
                diagnostics.push(MemorySpaceReadinessDiagnostic {
                    space: space_name.clone(),
                    reason: format!("retrieval mode `{:?}` is not supported", retrieval_mode),
                });
            }
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

#[derive(Debug, Clone, PartialEq, Serialize)]
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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocalMemoryReadMode {
    Key,
    Filter,
    Chronological,
    FullText,
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

#[derive(Debug, Clone)]
pub struct LocalMemoryOperationStateRow {
    pub package: String,
    pub package_version: String,
    pub operation: String,
    pub scope: BTreeMap<String, String>,
    pub trigger_type: String,
    pub armed: bool,
    pub baseline_at: Option<DateTime<Utc>>,
    pub last_completed_at: Option<DateTime<Utc>>,
    pub next_eligible_at: Option<DateTime<Utc>>,
    pub last_observed_value: Option<i64>,
    pub watermark: Option<Value>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq)]
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
    pub next_eligible_at: Option<String>,
    pub last_observed_value: Option<i64>,
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

        if append_only_enabled(space)
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
                    Ok(LocalMemoryWriteResult {
                        operation: request.operation,
                        affected_record_id: Some(stored.id.clone()),
                        record: Some(stored),
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
                    Ok(LocalMemoryWriteResult {
                        operation: request.operation,
                        affected_record_id: Some(stored.id.clone()),
                        record: Some(stored),
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
            }
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

fn delete_memory_vectors_for_record(
    connection: &Connection,
    package: &str,
    package_version: &str,
    space: &str,
    scope: &BTreeMap<String, String>,
    record_id: &str,
) -> Result<()> {
    let (_scope_json, scope_hash) = LocalSqliteMemoryRuntime::scope_identity(scope)?;
    connection
        .execute(
            r#"
            DELETE FROM memory_vectors
            WHERE package = ?1 AND package_version = ?2 AND space = ?3
              AND scope_hash = ?4 AND record_id = ?5
            "#,
            params![package, package_version, space, scope_hash, record_id],
        )
        .context("deleting stale Memory vectors")?;
    Ok(())
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
                trigger_type, armed, baseline_at, last_completed_at, next_eligible_at,
                last_observed_value, watermark_json, updated_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
            ON CONFLICT(package, package_version, operation, scope_hash)
            DO UPDATE SET
                trigger_type = excluded.trigger_type,
                armed = excluded.armed,
                baseline_at = excluded.baseline_at,
                last_completed_at = excluded.last_completed_at,
                next_eligible_at = excluded.next_eligible_at,
                last_observed_value = excluded.last_observed_value,
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
                state.next_eligible_at.as_ref().map(DateTime::to_rfc3339),
                state.last_observed_value,
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
                   trigger_type, armed, baseline_at, last_completed_at, next_eligible_at,
                   last_observed_value, watermark_json
            FROM memory_operation_state
            WHERE package = ?1 AND package_version = ?2 AND operation = ?3 AND scope_hash = ?4
            "#,
            params![package, package_version, operation, scope_hash],
            |row| {
                let watermark_json: Option<String> = row.get(11)?;
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
                    next_eligible_at: row.get(9)?,
                    last_observed_value: row.get(10)?,
                    watermark: watermark_json
                        .map(|value| {
                            serde_json::from_str(&value).map_err(|err| {
                                rusqlite::Error::FromSqlConversionFailure(
                                    11,
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
mod tests {
    use super::*;
    use serde_json::json;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct TreeEntrySnapshot {
        is_dir: bool,
        len: u64,
        modified_nanos: Option<u128>,
        content_hash: Option<String>,
    }

    fn temp_dir(label: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("agentpm-memory-{label}-{nanos}"));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn scope() -> BTreeMap<String, String> {
        BTreeMap::from([
            ("thread".to_string(), "t-456".to_string()),
            ("user".to_string(), "u-123".to_string()),
        ])
    }

    fn local_memory_error_code(error: &anyhow::Error) -> Option<&'static str> {
        error
            .chain()
            .find_map(|cause| cause.downcast_ref::<LocalMemoryActionError>())
            .map(LocalMemoryActionError::code)
    }

    fn test_record(id: &str) -> LocalMemoryRecordRow {
        LocalMemoryRecordRow {
            id: id.to_string(),
            package: "@zack/memory".into(),
            package_version: "0.1.0".into(),
            space: "notes".into(),
            space_model: MemorySpaceModel::Collection,
            record_type: "note".into(),
            schema_version: "1.0.0".into(),
            scope: scope(),
            content: json!({ "body": "remember this" }),
            provenance: json!({ "source": "test" }),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            expires_at: None,
            archived_at: None,
            ordinal: Some(1),
        }
    }

    fn test_document_record(id: &str, record_type: &str) -> LocalMemoryRecordRow {
        LocalMemoryRecordRow {
            id: id.to_string(),
            package: "@zack/memory".into(),
            package_version: "0.1.0".into(),
            space: "profile".into(),
            space_model: MemorySpaceModel::Document,
            record_type: record_type.into(),
            schema_version: "1.0.0".into(),
            scope: scope(),
            content: json!({ "body": id }),
            provenance: json!({ "source": "test" }),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            expires_at: None,
            archived_at: None,
            ordinal: None,
        }
    }

    fn test_operation_state() -> LocalMemoryOperationStateRow {
        LocalMemoryOperationStateRow {
            package: "@zack/memory".into(),
            package_version: "0.1.0".into(),
            operation: "rollup".into(),
            scope: scope(),
            trigger_type: "record_count".into(),
            armed: true,
            baseline_at: None,
            last_completed_at: None,
            next_eligible_at: None,
            last_observed_value: Some(1),
            watermark: Some(json!({ "cursor": "rec-1" })),
            updated_at: Utc::now(),
        }
    }

    fn table_columns(runtime: &LocalSqliteMemoryRuntime, table: &str) -> Vec<(String, i64)> {
        let mut statement = runtime
            .connection
            .prepare(&format!("PRAGMA table_info({table})"))
            .unwrap();
        statement
            .query_map([], |row| {
                Ok((row.get::<_, String>(1)?, row.get::<_, i64>(5)?))
            })
            .unwrap()
            .map(Result::unwrap)
            .collect()
    }

    fn package_tree_snapshot(root: &Path) -> BTreeMap<String, TreeEntrySnapshot> {
        let mut snapshot = BTreeMap::new();
        collect_tree_snapshot(root, root, &mut snapshot);
        snapshot
    }

    fn collect_tree_snapshot(
        root: &Path,
        path: &Path,
        snapshot: &mut BTreeMap<String, TreeEntrySnapshot>,
    ) {
        for entry in fs::read_dir(path).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            let metadata = entry.metadata().unwrap();
            let relative_path = path
                .strip_prefix(root)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/");
            snapshot.insert(
                relative_path,
                TreeEntrySnapshot {
                    is_dir: metadata.is_dir(),
                    len: metadata.len(),
                    modified_nanos: metadata
                        .modified()
                        .ok()
                        .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
                        .map(|duration| duration.as_nanos()),
                    content_hash: metadata
                        .is_file()
                        .then(|| sha256_prefixed_bytes(&fs::read(&path).unwrap())),
                },
            );
            if metadata.is_dir() {
                collect_tree_snapshot(root, &path, snapshot);
            }
        }
    }

    fn write_built_memory_package(dir: &Path) {
        fs::write(
            dir.join("agent.json"),
            r#"{
  "kind": "memory",
  "name": "generated-contract-test",
  "version": "0.1.0",
  "description": "Generated contract loader test.",
  "memory": {
    "scopes": {
      "user": { "description": "User scope." }
    },
    "record_types": {
      "note": {
        "version": "1.0.0",
        "description": "Durable note.",
        "schema": "schemas/note.schema.json"
      }
    },
    "spaces": {
      "notes": {
        "description": "Notes.",
        "model": "collection",
        "record_types": ["note"],
        "scope": ["user"],
        "retrieval": { "modes": ["key", "filter"] }
      }
    }
  }
}
"#,
        )
        .unwrap();
        fs::create_dir_all(dir.join("schemas")).unwrap();
        fs::write(
            dir.join("schemas/note.schema.json"),
            r#"{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "type": "object",
  "properties": {
    "body": { "type": "string" }
  },
  "required": ["body"],
  "additionalProperties": false
}
"#,
        )
        .unwrap();

        crate::commands::memory::execute_memory_build(
            &dir.join("agent.json"),
            MemoryBuildMode::Write,
        )
        .unwrap();
    }

    fn write_m14b_memory_package(dir: &Path) -> (MemoryManifest, ValidatedMemoryContracts) {
        fs::write(
            dir.join("agent.json"),
            r#"{
  "kind": "memory",
  "name": "m14b-memory-test",
  "version": "0.1.0",
  "description": "M14b direct Memory runtime test package.",
  "memory": {
    "scopes": {
      "user": { "description": "User scope." }
    },
    "record_types": {
      "note": {
        "version": "1.0.0",
        "description": "Durable note.",
        "schema": "schemas/note.schema.json"
      },
      "profile_a": {
        "version": "1.0.0",
        "description": "Profile A.",
        "schema": "schemas/profile-a.schema.json"
      },
      "profile_b": {
        "version": "1.0.0",
        "description": "Profile B.",
        "schema": "schemas/profile-b.schema.json"
      },
      "event": {
        "version": "1.0.0",
        "description": "Event.",
        "schema": "schemas/event.schema.json"
      },
      "volatile_note": {
        "version": "1.0.0",
        "description": "Complex non-persistable note.",
        "schema": "schemas/volatile-note.schema.json"
      }
    },
    "spaces": {
      "notes": {
        "description": "Notes.",
        "model": "collection",
        "record_types": ["note"],
        "scope": ["user"],
        "retrieval": { "modes": ["key", "filter", "full_text"] },
        "capacity": { "max_records": 2 },
        "retention": { "ttl": "PT1S", "on_expire": "archive" }
      },
      "delete_notes": {
        "description": "Delete-on-expiry notes.",
        "model": "collection",
        "record_types": ["note"],
        "scope": ["user"],
        "retrieval": { "modes": ["key"] },
        "retention": { "ttl": "PT1S", "on_expire": "delete" }
      },
      "profile": {
        "description": "Single current profile.",
        "model": "document",
        "record_types": ["profile_a", "profile_b"],
        "scope": ["user"],
        "retrieval": { "modes": ["key"] }
      },
      "events": {
        "description": "Append-only events.",
        "model": "sequence",
        "record_types": ["event"],
        "scope": ["user"],
        "retrieval": { "modes": ["chronological", "key"] },
        "constraints": { "append_only": true }
      },
      "volatile_notes": {
        "description": "Volatile projection validation.",
        "model": "collection",
        "record_types": ["volatile_note"],
        "scope": ["user"],
        "retrieval": { "modes": ["key"] }
      }
    }
  }
}
"#,
        )
        .unwrap();
        fs::create_dir_all(dir.join("schemas")).unwrap();
        fs::write(
            dir.join("schemas/note.schema.json"),
            r#"{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "type": "object",
  "properties": {
    "body": {
      "type": "string",
      "minLength": 1,
      "x-agentpm-persist": true,
      "x-agentpm-shareable": true
    },
	    "tag": {
	      "type": "string",
	      "x-agentpm-persist": true
	    },
	    "labels": {
	      "type": "array",
	      "items": { "type": "string" },
	      "x-agentpm-persist": true
	    },
	    "assignee": {
	      "type": "object",
	      "properties": {
	        "team": { "type": "string" },
	        "user": { "type": "string" }
	      },
	      "additionalProperties": false,
	      "x-agentpm-persist": true
	    },
	    "items": {
	      "type": "array",
	      "items": {
	        "type": "object",
	        "properties": {
	          "name": { "type": "string" }
	        },
	        "additionalProperties": false
	      },
	      "x-agentpm-persist": true
	    },
	    "secret": {
      "type": "string",
      "x-agentpm-persist": false
    },
    "nested": {
      "type": "object",
      "properties": {
        "visible": { "type": "string" },
        "ephemeral": { "type": "string", "x-agentpm-persist": false }
      },
      "additionalProperties": false
    }
  },
  "required": ["body"],
  "additionalProperties": false
}
"#,
        )
        .unwrap();
        fs::write(
            dir.join("schemas/profile-a.schema.json"),
            r#"{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "type": "object",
  "properties": { "name": { "type": "string", "minLength": 1 } },
  "required": ["name"],
  "additionalProperties": false
}
"#,
        )
        .unwrap();
        fs::write(
            dir.join("schemas/profile-b.schema.json"),
            r#"{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "type": "object",
  "properties": { "display": { "type": "string", "minLength": 1 } },
  "required": ["display"],
  "additionalProperties": false
}
"#,
        )
        .unwrap();
        fs::write(
            dir.join("schemas/event.schema.json"),
            r#"{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "type": "object",
  "properties": { "body": { "type": "string", "minLength": 1 } },
  "required": ["body"],
  "additionalProperties": false
}
"#,
        )
        .unwrap();
        fs::write(
            dir.join("schemas/volatile-note.schema.json"),
            r#"{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "type": "object",
  "properties": {
    "durable": { "type": "string" },
    "volatile": { "type": "string", "x-agentpm-persist": false }
  },
  "anyOf": [
    { "required": ["durable"] },
    { "required": ["volatile"] }
  ],
  "additionalProperties": false
}
"#,
        )
        .unwrap();

        crate::commands::memory::execute_memory_build(
            &dir.join("agent.json"),
            MemoryBuildMode::Write,
        )
        .unwrap();
        let manifest: MemoryManifest =
            serde_json::from_str(&fs::read_to_string(dir.join("agent.json")).unwrap()).unwrap();
        let contracts = validate_and_load_memory_contracts(dir).unwrap();
        (manifest, contracts)
    }

    fn m14b_write_request<'a>(
        manifest: &'a MemoryManifest,
        contracts: &'a ValidatedMemoryContracts,
        space: &'a str,
        record_type: &'a str,
        content: Option<Value>,
    ) -> LocalMemoryWriteRequest<'a> {
        LocalMemoryWriteRequest {
            package: &manifest.name,
            package_version: &manifest.version,
            manifest,
            contracts,
            space,
            record_type,
            scope: BTreeMap::from([("user".to_string(), "u-123".to_string())]),
            operation: LocalMemoryWriteOperation::Create,
            record_id: None,
            content,
            provenance: json!({ "source_record_ids": [] }),
            now: Utc::now(),
        }
    }

    fn m14b_read_request<'a>(
        manifest: &'a MemoryManifest,
        space: &'a str,
        mode: LocalMemoryReadMode,
    ) -> LocalMemoryReadRequest<'a> {
        LocalMemoryReadRequest {
            package: &manifest.name,
            package_version: &manifest.version,
            manifest,
            space,
            scope: BTreeMap::from([("user".to_string(), "u-123".to_string())]),
            mode,
            record_id: None,
            record_type: None,
            filter: BTreeMap::new(),
            query: None,
            limit: None,
            now: Utc::now(),
        }
    }

    fn insert_fake_vector(runtime: &LocalSqliteMemoryRuntime, record: &StoredMemoryRecord) {
        runtime
            .connection
            .execute(
                r#"
                INSERT INTO memory_vectors (
                    record_id, package, package_version, space, record_type, scope_hash,
                    embedding_provider, embedding_model, dimensions, content_hash, vector, updated_at
                )
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'test', 'test-model', 1, 'sha256:test', ?7, ?8)
                "#,
                params![
                    &record.id,
                    &record.package,
                    &record.package_version,
                    &record.space,
                    &record.record_type,
                    &record.scope_hash,
                    vec![0_u8, 0, 0, 0],
                    Utc::now().to_rfc3339(),
                ],
            )
            .unwrap();
    }

    fn vector_count(runtime: &LocalSqliteMemoryRuntime, record_id: &str) -> u64 {
        runtime
            .connection
            .query_row(
                "SELECT COUNT(*) FROM memory_vectors WHERE record_id = ?1",
                params![record_id],
                |row| row.get(0),
            )
            .unwrap()
    }

    #[test]
    fn sqlite_memory_store_initializes_schema_version_one() {
        let dir = temp_dir("schema");
        let runtime = LocalSqliteMemoryRuntime::open(&dir, None).unwrap();

        assert_eq!(runtime.schema_version().unwrap(), 1);
        assert_eq!(
            runtime.database_path(),
            dir.join(".agentpm-state").join("memory.sqlite3")
        );
        assert!(!dir.join(".agentpm").join("memory.sqlite3").exists());

        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn sqlite_memory_store_configures_wal_and_busy_timeout() {
        let dir = temp_dir("connection-pragmas");
        let runtime = LocalSqliteMemoryRuntime::open(&dir, None).unwrap();

        let journal_mode: String = runtime
            .connection
            .query_row("PRAGMA journal_mode", [], |row| row.get(0))
            .unwrap();
        let busy_timeout: i64 = runtime
            .connection
            .query_row("PRAGMA busy_timeout", [], |row| row.get(0))
            .unwrap();
        let foreign_keys: i64 = runtime
            .connection
            .query_row("PRAGMA foreign_keys", [], |row| row.get(0))
            .unwrap();

        assert_eq!(journal_mode, "wal");
        assert_eq!(busy_timeout, LOCAL_MEMORY_BUSY_TIMEOUT_MS as i64);
        assert_eq!(foreign_keys, 1);

        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn sqlite_memory_store_migrates_schema_zero_to_current_version() {
        let dir = temp_dir("schema-migration");
        let state_dir = dir.join(".agentpm-state");
        fs::create_dir_all(&state_dir).unwrap();
        {
            let connection = Connection::open(state_dir.join(LOCAL_MEMORY_DB_NAME)).unwrap();
            connection
                .execute_batch(
                    r#"
                    CREATE TABLE memory_meta (
                        key TEXT PRIMARY KEY NOT NULL,
                        value TEXT NOT NULL
                    );

                    INSERT INTO memory_meta (key, value) VALUES ('schema_version', '0');
                    "#,
                )
                .unwrap();
        }

        let runtime = LocalSqliteMemoryRuntime::open(&dir, None).unwrap();

        assert_eq!(
            runtime.schema_version().unwrap(),
            LOCAL_MEMORY_SCHEMA_VERSION
        );
        assert!(
            table_columns(&runtime, "memory_records")
                .iter()
                .any(|(name, _pk)| name == "content_json")
        );
        assert!(
            table_columns(&runtime, "memory_operation_state")
                .iter()
                .any(|(name, _pk)| name == "watermark_json")
        );

        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn sqlite_memory_schema_uses_spec_primary_keys_and_columns() {
        let dir = temp_dir("schema-keys");
        let runtime = LocalSqliteMemoryRuntime::open(&dir, None).unwrap();

        let record_pk = table_columns(&runtime, "memory_records")
            .into_iter()
            .filter(|(_name, pk)| *pk > 0)
            .collect::<BTreeMap<_, _>>()
            .into_iter()
            .map(|(name, pk)| (pk, name))
            .collect::<BTreeMap<_, _>>()
            .into_iter()
            .map(|(pk, name)| (name, pk))
            .collect::<Vec<_>>();
        assert_eq!(
            record_pk,
            vec![
                ("package".into(), 1),
                ("package_version".into(), 2),
                ("space".into(), 3),
                ("scope_hash".into(), 4),
                ("id".into(), 5),
            ]
        );

        let operation_columns = table_columns(&runtime, "memory_operation_state")
            .into_iter()
            .map(|(name, _pk)| name)
            .collect::<Vec<_>>();
        for expected in [
            "trigger_type",
            "armed",
            "baseline_at",
            "last_completed_at",
            "next_eligible_at",
            "last_observed_value",
            "watermark_json",
        ] {
            assert!(operation_columns.contains(&expected.to_string()));
        }

        let vector_pk = table_columns(&runtime, "memory_vectors")
            .into_iter()
            .filter(|(_name, pk)| *pk > 0)
            .collect::<BTreeMap<_, _>>()
            .into_iter()
            .map(|(name, pk)| (pk, name))
            .collect::<BTreeMap<_, _>>()
            .into_iter()
            .map(|(pk, name)| (name, pk))
            .collect::<Vec<_>>();
        assert_eq!(
            vector_pk,
            vec![
                ("package".into(), 1),
                ("package_version".into(), 2),
                ("space".into(), 3),
                ("scope_hash".into(), 4),
                ("record_id".into(), 5),
                ("embedding_provider".into(), 6),
                ("embedding_model".into(), 7),
            ]
        );

        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn sqlite_memory_store_rejects_newer_schema_version() {
        let dir = temp_dir("newer-schema");
        {
            let runtime = LocalSqliteMemoryRuntime::open(&dir, None).unwrap();
            runtime
                .connection
                .execute(
                    "UPDATE memory_meta SET value = '999' WHERE key = 'schema_version'",
                    [],
                )
                .unwrap();
        }

        let err = match LocalSqliteMemoryRuntime::open(&dir, None) {
            Ok(_) => panic!("expected newer schema version to fail"),
            Err(err) => err,
        };
        assert!(
            err.to_string()
                .contains("unsupported Memory SQLite schema version 999"),
            "unexpected error: {err:?}"
        );

        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn canonical_scope_json_and_hash_are_stable_and_verified() {
        let ordered = BTreeMap::from([
            ("thread".to_string(), "t-456".to_string()),
            ("user".to_string(), "u-123".to_string()),
        ]);
        let reversed = BTreeMap::from([
            ("user".to_string(), "u-123".to_string()),
            ("thread".to_string(), "t-456".to_string()),
        ]);

        let (ordered_json, ordered_hash) =
            LocalSqliteMemoryRuntime::scope_identity(&ordered).unwrap();
        let (reversed_json, reversed_hash) =
            LocalSqliteMemoryRuntime::scope_identity(&reversed).unwrap();

        assert_eq!(ordered_json, r#"{"thread":"t-456","user":"u-123"}"#);
        assert_eq!(ordered_json, reversed_json);
        assert_eq!(ordered_hash, reversed_hash);
        LocalSqliteMemoryRuntime::verify_scope_identity(&ordered_json, &ordered_hash).unwrap();
        assert!(
            LocalSqliteMemoryRuntime::verify_scope_identity(&ordered_json, "sha256:bad").is_err()
        );
    }

    #[test]
    fn sqlite_memory_store_persists_records_across_restart() {
        let dir = temp_dir("restart-records");
        {
            let runtime = LocalSqliteMemoryRuntime::open(&dir, None).unwrap();
            runtime.insert_record(&test_record("rec-1")).unwrap();
            runtime
                .store_operation_state(&test_operation_state())
                .unwrap();
        }

        let runtime = LocalSqliteMemoryRuntime::open(&dir, None).unwrap();
        let record = runtime
            .get_record("@zack/memory", "0.1.0", "notes", &scope(), "rec-1")
            .unwrap()
            .unwrap();
        assert_eq!(record.content, json!({ "body": "remember this" }));
        let operation_state = runtime
            .load_operation_state("@zack/memory", "0.1.0", "rollup", &scope())
            .unwrap()
            .unwrap();
        assert_eq!(operation_state.trigger_type, "record_count");
        assert!(operation_state.armed);
        assert_eq!(operation_state.last_observed_value, Some(1));
        assert_eq!(
            operation_state.watermark,
            Some(json!({ "cursor": "rec-1" }))
        );
        assert_eq!(
            runtime
                .active_record_count("@zack/memory", "0.1.0", "notes", &scope(), None)
                .unwrap(),
            1
        );

        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn sqlite_memory_get_record_excludes_archived_rows_from_active_key_reads() {
        let dir = temp_dir("archived-key-read");
        let runtime = LocalSqliteMemoryRuntime::open(&dir, None).unwrap();
        let mut record = test_record("rec-archived");
        record.archived_at = Some(Utc::now());
        runtime.insert_record(&record).unwrap();

        assert!(
            runtime
                .get_record("@zack/memory", "0.1.0", "notes", &scope(), "rec-archived")
                .unwrap()
                .is_none()
        );
        assert_eq!(
            runtime
                .active_record_count("@zack/memory", "0.1.0", "notes", &scope(), None)
                .unwrap(),
            0
        );

        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn sqlite_memory_store_enforces_one_current_document_per_space_and_scope() {
        let dir = temp_dir("document-current");
        let runtime = LocalSqliteMemoryRuntime::open(&dir, None).unwrap();

        runtime
            .insert_record(&test_document_record("doc-1", "summary"))
            .unwrap();
        let err = runtime
            .insert_record(&test_document_record("doc-2", "profile"))
            .unwrap_err();

        assert!(
            err.to_string().contains("inserting Memory record"),
            "unexpected error: {err:?}"
        );

        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn sqlite_memory_store_allocates_sequence_ordinals_deterministically() {
        let dir = temp_dir("sequence");
        let mut runtime = LocalSqliteMemoryRuntime::open(&dir, None).unwrap();

        assert_eq!(
            runtime
                .allocate_sequence_ordinal("@zack/memory", "0.1.0", "events", &scope())
                .unwrap(),
            0
        );
        assert_eq!(
            runtime
                .allocate_sequence_ordinal("@zack/memory", "0.1.0", "events", &scope())
                .unwrap(),
            1
        );

        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn sqlite_memory_atomic_batch_commits_record_sequence_and_operation_state() {
        let dir = temp_dir("atomic-commit");
        let mut runtime = LocalSqliteMemoryRuntime::open(&dir, None).unwrap();

        let ordinal = runtime
            .atomic_batch(|batch| {
                let ordinal =
                    batch.allocate_sequence_ordinal("@zack/memory", "0.1.0", "events", &scope())?;
                let mut record = test_record("rec-batch");
                record.space = "events".into();
                record.space_model = MemorySpaceModel::Sequence;
                record.ordinal = Some(ordinal);
                batch.insert_record(&record)?;
                let mut operation_state = test_operation_state();
                operation_state.last_observed_value = Some(42);
                batch.store_operation_state(&operation_state)?;
                assert_eq!(
                    batch.active_record_count("@zack/memory", "0.1.0", "events", &scope(), None)?,
                    1
                );
                Ok(ordinal)
            })
            .unwrap();

        assert_eq!(ordinal, 0);
        assert_eq!(
            runtime
                .get_record("@zack/memory", "0.1.0", "events", &scope(), "rec-batch")
                .unwrap()
                .unwrap()
                .ordinal,
            Some(0)
        );
        assert_eq!(
            runtime
                .load_operation_state("@zack/memory", "0.1.0", "rollup", &scope())
                .unwrap()
                .unwrap()
                .last_observed_value,
            Some(42)
        );
        assert_eq!(
            runtime
                .allocate_sequence_ordinal("@zack/memory", "0.1.0", "events", &scope())
                .unwrap(),
            1
        );

        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn sqlite_memory_atomic_batch_rolls_back_all_primitive_mutations() {
        let dir = temp_dir("atomic-rollback");
        let mut runtime = LocalSqliteMemoryRuntime::open(&dir, None).unwrap();

        let err = runtime
            .atomic_batch(|batch| {
                let ordinal =
                    batch.allocate_sequence_ordinal("@zack/memory", "0.1.0", "events", &scope())?;
                let mut record = test_record("rec-rollback");
                record.space = "events".into();
                record.space_model = MemorySpaceModel::Sequence;
                record.ordinal = Some(ordinal);
                batch.insert_record(&record)?;
                batch.store_operation_state(&test_operation_state())?;
                Err::<(), _>(anyhow::anyhow!("abort batch"))
            })
            .unwrap_err();

        assert!(err.to_string().contains("abort batch"));
        assert!(
            runtime
                .get_record("@zack/memory", "0.1.0", "events", &scope(), "rec-rollback")
                .unwrap()
                .is_none()
        );
        assert_eq!(
            runtime
                .active_record_count("@zack/memory", "0.1.0", "events", &scope(), None)
                .unwrap(),
            0
        );
        assert!(
            runtime
                .load_operation_state("@zack/memory", "0.1.0", "rollup", &scope())
                .unwrap()
                .is_none()
        );
        assert_eq!(
            runtime
                .allocate_sequence_ordinal("@zack/memory", "0.1.0", "events", &scope())
                .unwrap(),
            0
        );

        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn sqlite_direct_document_replace_allows_cross_record_type_replacement() {
        let dir = temp_dir("m14b-document-replace");
        let (manifest, contracts) = write_m14b_memory_package(&dir);
        let mut runtime = LocalSqliteMemoryRuntime::open(&dir, None).unwrap();
        let scoped_user = BTreeMap::from([("user".to_string(), "u-123".to_string())]);

        let first = runtime
            .write_record(m14b_write_request(
                &manifest,
                &contracts,
                "profile",
                "profile_a",
                Some(json!({ "name": "A" })),
            ))
            .unwrap()
            .record
            .unwrap();
        let duplicate_create = runtime
            .write_record(m14b_write_request(
                &manifest,
                &contracts,
                "profile",
                "profile_b",
                Some(json!({ "display": "B" })),
            ))
            .unwrap_err();
        let duplicate_error = duplicate_create
            .downcast_ref::<LocalMemoryActionError>()
            .expect("duplicate document create should return typed Memory error");
        assert_eq!(duplicate_error.code(), "constraint_violation");
        assert!(
            duplicate_error
                .to_string()
                .contains("use upsert to replace")
        );

        let second = {
            let mut request = m14b_write_request(
                &manifest,
                &contracts,
                "profile",
                "profile_b",
                Some(json!({ "display": "B" })),
            );
            request.operation = LocalMemoryWriteOperation::Upsert;
            runtime.write_record(request).unwrap().record.unwrap()
        };

        assert_eq!(first.id, second.id);
        assert_eq!(second.record_type, "profile_b");
        assert_eq!(second.content, json!({ "display": "B" }));
        assert_eq!(
            runtime
                .active_record_count(
                    &manifest.name,
                    &manifest.version,
                    "profile",
                    &scoped_user,
                    None
                )
                .unwrap(),
            1
        );

        let current = runtime
            .read_records(m14b_read_request(
                &manifest,
                "profile",
                LocalMemoryReadMode::Key,
            ))
            .unwrap();
        assert_eq!(current.len(), 1);
        assert_eq!(current[0].record_type, "profile_b");

        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn sqlite_direct_collection_supports_crud_filter_full_text_and_capacity() {
        let dir = temp_dir("m14b-collection");
        let (manifest, contracts) = write_m14b_memory_package(&dir);
        let mut runtime = LocalSqliteMemoryRuntime::open(&dir, None).unwrap();
        let now = Utc::now();
        let scoped_user = BTreeMap::from([("user".to_string(), "u-123".to_string())]);

        let alpha = runtime
            .write_record(m14b_write_request(
                &manifest,
                &contracts,
                "notes",
                "note",
                Some(json!({
                    "body": "Alpha launch checklist Änderung am Release Straße",
                    "tag": "alpha",
                    "labels": ["bug", "release"],
                    "assignee": { "team": "platform", "user": "ada" },
                    "items": [{ "name": "smoke-test" }, { "name": "rollback" }]
                })),
            ))
            .unwrap()
            .record
            .unwrap();
        let beta = runtime
            .write_record(m14b_write_request(
                &manifest,
                &contracts,
                "notes",
                "note",
                Some(json!({
                    "body": "Beta support note",
                    "tag": "beta",
                    "labels": ["support"],
                    "assignee": { "team": "support", "user": "lin" },
                    "items": [{ "name": "triage" }]
                })),
            ))
            .unwrap()
            .record
            .unwrap();

        let mut filter_request = m14b_read_request(&manifest, "notes", LocalMemoryReadMode::Filter);
        filter_request.filter = BTreeMap::from([("tag".into(), json!("alpha"))]);
        let filtered = runtime.read_records(filter_request).unwrap();
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].id, alpha.id);

        let mut label_filter = m14b_read_request(&manifest, "notes", LocalMemoryReadMode::Filter);
        label_filter.filter = BTreeMap::from([("labels".into(), json!("bug"))]);
        let label_matches = runtime.read_records(label_filter).unwrap();
        assert_eq!(label_matches.len(), 1);
        assert_eq!(label_matches[0].id, alpha.id);

        let mut array_equality_filter =
            m14b_read_request(&manifest, "notes", LocalMemoryReadMode::Filter);
        array_equality_filter.filter =
            BTreeMap::from([("labels".into(), json!(["bug", "release"]))]);
        let array_equality_matches = runtime.read_records(array_equality_filter).unwrap();
        assert_eq!(array_equality_matches.len(), 1);
        assert_eq!(array_equality_matches[0].id, alpha.id);

        let mut nested_filter = m14b_read_request(&manifest, "notes", LocalMemoryReadMode::Filter);
        nested_filter.filter = BTreeMap::from([("assignee.team".into(), json!("platform"))]);
        let nested_matches = runtime.read_records(nested_filter).unwrap();
        assert_eq!(nested_matches.len(), 1);
        assert_eq!(nested_matches[0].id, alpha.id);

        let mut array_object_filter =
            m14b_read_request(&manifest, "notes", LocalMemoryReadMode::Filter);
        array_object_filter.filter = BTreeMap::from([("items.name".into(), json!("rollback"))]);
        let array_object_matches = runtime.read_records(array_object_filter).unwrap();
        assert_eq!(array_object_matches.len(), 1);
        assert_eq!(array_object_matches[0].id, alpha.id);

        let mut conjunctive_filter =
            m14b_read_request(&manifest, "notes", LocalMemoryReadMode::Filter);
        conjunctive_filter.filter = BTreeMap::from([
            ("labels".into(), json!("release")),
            ("assignee.team".into(), json!("platform")),
        ]);
        let conjunctive_matches = runtime.read_records(conjunctive_filter).unwrap();
        assert_eq!(conjunctive_matches.len(), 1);
        assert_eq!(conjunctive_matches[0].id, alpha.id);

        let mut non_matching_filter =
            m14b_read_request(&manifest, "notes", LocalMemoryReadMode::Filter);
        non_matching_filter.filter = BTreeMap::from([("labels".into(), json!("security"))]);
        assert!(
            runtime
                .read_records(non_matching_filter)
                .unwrap()
                .is_empty()
        );

        let mut full_text_request =
            m14b_read_request(&manifest, "notes", LocalMemoryReadMode::FullText);
        full_text_request.query = Some("support".into());
        let searched = runtime.read_records(full_text_request).unwrap();
        assert_eq!(searched.len(), 1);
        assert_eq!(searched[0].id, beta.id);

        let mut unicode_full_text_request =
            m14b_read_request(&manifest, "notes", LocalMemoryReadMode::FullText);
        unicode_full_text_request.query = Some("änderung".into());
        let unicode_searched = runtime.read_records(unicode_full_text_request).unwrap();
        assert_eq!(unicode_searched.len(), 1);
        assert_eq!(unicode_searched[0].id, alpha.id);

        let mut normalization_limited_request =
            m14b_read_request(&manifest, "notes", LocalMemoryReadMode::FullText);
        normalization_limited_request.query = Some("strasse".into());
        assert!(
            runtime
                .read_records(normalization_limited_request)
                .unwrap()
                .is_empty()
        );

        let overflow = runtime
            .write_record(m14b_write_request(
                &manifest,
                &contracts,
                "notes",
                "note",
                Some(json!({ "body": "Overflow", "tag": "gamma" })),
            ))
            .unwrap_err();
        assert_eq!(
            local_memory_error_code(&overflow),
            Some("capacity_exceeded")
        );

        let updated = {
            let mut request = m14b_write_request(
                &manifest,
                &contracts,
                "notes",
                "note",
                Some(json!({ "body": "Alpha launch checklist updated", "tag": "alpha" })),
            );
            request.operation = LocalMemoryWriteOperation::Update;
            request.record_id = Some(alpha.id.clone());
            request.now = now;
            runtime.write_record(request).unwrap().record.unwrap()
        };
        assert_eq!(updated.id, alpha.id);
        assert_eq!(updated.content["body"], "Alpha launch checklist updated");

        insert_fake_vector(&runtime, &beta);
        assert_eq!(vector_count(&runtime, &beta.id), 1);
        {
            let mut request = m14b_write_request(&manifest, &contracts, "notes", "note", None);
            request.operation = LocalMemoryWriteOperation::Archive;
            request.record_id = Some(beta.id.clone());
            request.now = now;
            runtime.write_record(request).unwrap();
        }
        assert!(
            runtime
                .get_record(
                    &manifest.name,
                    &manifest.version,
                    "notes",
                    &scoped_user,
                    &beta.id
                )
                .unwrap()
                .is_none()
        );
        assert_eq!(vector_count(&runtime, &beta.id), 0);

        {
            let mut request = m14b_write_request(&manifest, &contracts, "notes", "note", None);
            request.operation = LocalMemoryWriteOperation::Delete;
            request.record_id = Some(alpha.id.clone());
            request.now = now;
            runtime.write_record(request).unwrap();
        }
        assert_eq!(
            runtime
                .active_record_count(
                    &manifest.name,
                    &manifest.version,
                    "notes",
                    &scoped_user,
                    None
                )
                .unwrap(),
            0
        );

        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn sqlite_direct_sequence_orders_chronologically_and_never_reuses_ordinals() {
        let dir = temp_dir("m14b-sequence");
        let (manifest, contracts) = write_m14b_memory_package(&dir);
        let mut runtime = LocalSqliteMemoryRuntime::open(&dir, None).unwrap();
        let now = Utc::now();

        let first = runtime
            .write_record(m14b_write_request(
                &manifest,
                &contracts,
                "events",
                "event",
                Some(json!({ "body": "first" })),
            ))
            .unwrap()
            .record
            .unwrap();
        let second = runtime
            .write_record(m14b_write_request(
                &manifest,
                &contracts,
                "events",
                "event",
                Some(json!({ "body": "second" })),
            ))
            .unwrap()
            .record
            .unwrap();
        assert_eq!(first.ordinal, Some(0));
        assert_eq!(second.ordinal, Some(1));

        let append_only_update = {
            let mut request = m14b_write_request(
                &manifest,
                &contracts,
                "events",
                "event",
                Some(json!({ "body": "forbidden" })),
            );
            request.operation = LocalMemoryWriteOperation::Update;
            request.record_id = Some(first.id.clone());
            request.now = now;
            runtime.write_record(request).unwrap_err()
        };
        assert!(append_only_update.to_string().contains("append_only"));

        let third = runtime
            .write_record(m14b_write_request(
                &manifest,
                &contracts,
                "events",
                "event",
                Some(json!({ "body": "third" })),
            ))
            .unwrap()
            .record
            .unwrap();
        assert_eq!(third.ordinal, Some(2));

        let ordered = runtime
            .read_records(m14b_read_request(
                &manifest,
                "events",
                LocalMemoryReadMode::Chronological,
            ))
            .unwrap();
        assert_eq!(
            ordered
                .iter()
                .map(|record| record.content["body"].as_str().unwrap())
                .collect::<Vec<_>>(),
            vec!["first", "second", "third"]
        );

        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn sqlite_direct_write_enforces_contract_and_persistence_projection_before_mutation() {
        let dir = temp_dir("m14b-projection");
        let (manifest, contracts) = write_m14b_memory_package(&dir);
        let mut runtime = LocalSqliteMemoryRuntime::open(&dir, None).unwrap();
        let scoped_user = BTreeMap::from([("user".to_string(), "u-123".to_string())]);

        let invalid_full_content = runtime
            .write_record(m14b_write_request(
                &manifest,
                &contracts,
                "notes",
                "note",
                Some(json!({ "tag": "missing-body" })),
            ))
            .unwrap_err();
        assert!(
            format!("{invalid_full_content:?}")
                .contains("full proposed Memory content validation failed")
        );

        let persisted = runtime
            .write_record(m14b_write_request(
                &manifest,
                &contracts,
                "notes",
                "note",
                Some(json!({
                    "body": "Keep this",
                    "secret": "drop this",
                    "nested": { "visible": "yes", "ephemeral": "drop nested" }
                })),
            ))
            .unwrap()
            .record
            .unwrap();
        assert_eq!(
            persisted.content,
            json!({ "body": "Keep this", "nested": { "visible": "yes" } })
        );

        drop(runtime);
        let runtime = LocalSqliteMemoryRuntime::open(&dir, None).unwrap();
        let read_back = runtime
            .get_record(
                &manifest.name,
                &manifest.version,
                "notes",
                &scoped_user,
                &persisted.id,
            )
            .unwrap()
            .unwrap();
        assert_eq!(
            read_back.content,
            json!({ "body": "Keep this", "nested": { "visible": "yes" } })
        );

        let mut runtime = runtime;
        let invalid_projection = runtime
            .write_record(m14b_write_request(
                &manifest,
                &contracts,
                "volatile_notes",
                "volatile_note",
                Some(json!({ "volatile": "full content is initially valid" })),
            ))
            .unwrap_err();
        assert!(
            format!("{invalid_projection:?}")
                .contains("durable Memory content projection validation failed")
        );
        assert_eq!(
            runtime
                .active_record_count(
                    &manifest.name,
                    &manifest.version,
                    "volatile_notes",
                    &scoped_user,
                    None
                )
                .unwrap(),
            0
        );

        let schemas = memory_contract_schemas(&contracts, "notes", "note").unwrap();
        assert_eq!(
            schemas.content_schema["properties"]["body"]["x-agentpm-shareable"],
            true
        );

        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn sqlite_direct_ttl_lazy_expiry_archives_or_deletes_before_active_reads() {
        let dir = temp_dir("m14b-ttl");
        let (manifest, contracts) = write_m14b_memory_package(&dir);
        let mut runtime = LocalSqliteMemoryRuntime::open(&dir, None).unwrap();
        let now = Utc::now();
        let scoped_user = BTreeMap::from([("user".to_string(), "u-123".to_string())]);

        let archived = runtime
            .write_record(m14b_write_request(
                &manifest,
                &contracts,
                "notes",
                "note",
                Some(json!({ "body": "expires by archive" })),
            ))
            .unwrap()
            .record
            .unwrap();
        let deleted = runtime
            .write_record(m14b_write_request(
                &manifest,
                &contracts,
                "delete_notes",
                "note",
                Some(json!({ "body": "expires by delete" })),
            ))
            .unwrap()
            .record
            .unwrap();

        insert_fake_vector(&runtime, &archived);
        assert_eq!(vector_count(&runtime, &archived.id), 1);
        let mut archive_read = m14b_read_request(&manifest, "notes", LocalMemoryReadMode::Filter);
        archive_read.now = now + ChronoDuration::seconds(2);
        runtime.read_records(archive_read).unwrap();
        assert!(
            runtime
                .get_record(
                    &manifest.name,
                    &manifest.version,
                    "notes",
                    &scoped_user,
                    &archived.id
                )
                .unwrap()
                .is_none()
        );
        let archived_at: Option<String> = runtime
            .connection
            .query_row(
                "SELECT archived_at FROM memory_records WHERE id = ?1",
                params![archived.id],
                |row| row.get(0),
            )
            .unwrap();
        assert!(archived_at.is_some());
        assert_eq!(vector_count(&runtime, &archived.id), 0);

        let mut delete_read =
            m14b_read_request(&manifest, "delete_notes", LocalMemoryReadMode::Key);
        delete_read.record_id = Some(deleted.id.clone());
        delete_read.now = now + ChronoDuration::seconds(2);
        assert!(runtime.read_records(delete_read).unwrap().is_empty());
        let deleted_count: u64 = runtime
            .connection
            .query_row(
                "SELECT COUNT(*) FROM memory_records WHERE id = ?1",
                params![deleted.id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(deleted_count, 0);

        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn sqlite_direct_read_rolls_back_lazy_expiry_when_read_fails() {
        let dir = temp_dir("m14b-read-expiry-rollback");
        let (manifest, contracts) = write_m14b_memory_package(&dir);
        let mut runtime = LocalSqliteMemoryRuntime::open(&dir, None).unwrap();
        let now = Utc::now();

        let record = runtime
            .write_record(m14b_write_request(
                &manifest,
                &contracts,
                "notes",
                "note",
                Some(json!({ "body": "expires by archive" })),
            ))
            .unwrap()
            .record
            .unwrap();
        insert_fake_vector(&runtime, &record);

        let mut failing_read = m14b_read_request(&manifest, "notes", LocalMemoryReadMode::FullText);
        failing_read.now = now + ChronoDuration::seconds(2);
        let err = runtime.read_records(failing_read).unwrap_err();
        assert!(err.to_string().contains("requires a query"));

        let archived_at: Option<String> = runtime
            .connection
            .query_row(
                "SELECT archived_at FROM memory_records WHERE id = ?1",
                params![record.id],
                |row| row.get(0),
            )
            .unwrap();
        assert!(archived_at.is_none());
        assert_eq!(vector_count(&runtime, &record.id), 1);

        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn sqlite_direct_read_and_capacity_filters_honor_request_clock() {
        let dir = temp_dir("m14b-request-clock");
        let (manifest, contracts) = write_m14b_memory_package(&dir);
        let mut runtime = LocalSqliteMemoryRuntime::open(&dir, None).unwrap();
        let old_now = Utc::now() - ChronoDuration::seconds(10);

        let mut first_request = m14b_write_request(
            &manifest,
            &contracts,
            "notes",
            "note",
            Some(json!({ "body": "first old note", "tag": "clock" })),
        );
        first_request.now = old_now;
        let first = runtime.write_record(first_request).unwrap().record.unwrap();

        let mut key_read = m14b_read_request(&manifest, "notes", LocalMemoryReadMode::Key);
        key_read.record_id = Some(first.id.clone());
        key_read.now = old_now;
        assert_eq!(runtime.read_records(key_read).unwrap().len(), 1);

        let mut filter_read = m14b_read_request(&manifest, "notes", LocalMemoryReadMode::Filter);
        filter_read.filter = BTreeMap::from([("tag".into(), json!("clock"))]);
        filter_read.now = old_now;
        assert_eq!(runtime.read_records(filter_read).unwrap().len(), 1);

        let mut second_request = m14b_write_request(
            &manifest,
            &contracts,
            "notes",
            "note",
            Some(json!({ "body": "second old note", "tag": "clock" })),
        );
        second_request.now = old_now;
        runtime.write_record(second_request).unwrap();

        let mut overflow_request = m14b_write_request(
            &manifest,
            &contracts,
            "notes",
            "note",
            Some(json!({ "body": "third old note", "tag": "clock" })),
        );
        overflow_request.now = old_now;
        let overflow = runtime.write_record(overflow_request).unwrap_err();
        assert_eq!(
            local_memory_error_code(&overflow),
            Some("capacity_exceeded")
        );

        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn local_sqlite_capabilities_advertise_only_current_primitives() {
        let capabilities = MemoryRuntimeCapabilityDescriptor::local_sqlite();

        assert_eq!(
            capabilities.space_models,
            vec![
                MemorySpaceModel::Document,
                MemorySpaceModel::Collection,
                MemorySpaceModel::Sequence
            ]
        );
        assert_eq!(
            capabilities.retrieval_modes,
            vec![
                MemoryRetrievalMode::Key,
                MemoryRetrievalMode::Filter,
                MemoryRetrievalMode::Chronological,
                MemoryRetrievalMode::FullText,
            ]
        );
        assert_eq!(
            capabilities.retention_actions,
            vec![
                MemoryRetentionAction::Delete,
                MemoryRetentionAction::Archive
            ]
        );
        assert_eq!(
            capabilities.constraints,
            vec![MemoryRuntimeConstraintCapability::AppendOnly]
        );
        assert!(capabilities.capacity);
        assert!(capabilities.durable_trigger_state);
        assert!(capabilities.atomic_batches);
    }

    #[test]
    fn runtime_capability_comparison_reports_unrealizable_space_requirements() {
        let manifest: MemoryManifest = serde_json::from_value(json!({
            "kind": "memory",
            "name": "capability-test",
            "version": "0.1.0",
            "memory": {
                "scopes": {
                    "user": { "description": "User scope." }
                },
                "record_types": {
                    "note": {
                        "version": "1.0.0",
                        "description": "Note.",
                        "schema": "schemas/note.schema.json"
                    }
                },
                "spaces": {
                    "notes": {
                        "description": "Notes.",
                        "model": "collection",
                        "record_types": ["note"],
                        "scope": ["user"],
                        "retrieval": { "modes": ["semantic"] },
                        "capacity": { "max_records": 10 },
                        "retention": { "ttl": "P30D", "on_expire": "archive" },
                        "constraints": { "append_only": true }
                    }
                }
            }
        }))
        .unwrap();

        let diagnostics = unrealizable_memory_spaces(
            &manifest,
            &MemoryRuntimeCapabilityDescriptor::local_sqlite(),
        );

        assert_eq!(diagnostics.len(), 1);
        let reasons = diagnostics
            .iter()
            .map(|diagnostic| diagnostic.reason.as_str())
            .collect::<Vec<_>>();
        assert!(
            diagnostics
                .iter()
                .all(|diagnostic| diagnostic.space == "notes")
        );
        assert!(reasons.contains(&"retrieval mode `Semantic` is not supported"));
    }

    #[test]
    fn generated_memory_contract_loader_validates_current_package_artifacts() {
        let dir = temp_dir("generated-contracts");
        write_built_memory_package(&dir);

        let loaded = validate_and_load_memory_contracts(&dir).unwrap();
        assert_eq!(loaded.index.contracts.len(), 1);
        assert_eq!(loaded.contracts.len(), 1);

        fs::write(dir.join("memory/contracts/index.json"), "{}").unwrap();
        let err = validate_and_load_memory_contracts(&dir).unwrap_err();
        assert!(
            err.to_string()
                .contains("generated Memory contracts are not current"),
            "unexpected error: {err:?}"
        );

        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn generated_memory_contract_loader_does_not_write_into_package_root() {
        let dir = temp_dir("generated-contracts-read-only");
        write_built_memory_package(&dir);
        let before = package_tree_snapshot(&dir);
        std::thread::sleep(Duration::from_millis(20));

        validate_and_load_memory_contracts(&dir).unwrap();

        let after = package_tree_snapshot(&dir);
        assert_eq!(after, before);

        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn memory_contract_cache_reuses_validated_contracts_until_source_changes() {
        let dir = temp_dir("contract-cache-source");
        write_built_memory_package(&dir);
        let mut cache = MemoryContractCache::new();

        let loaded = cache.validate_and_load(&dir).unwrap();
        assert_eq!(loaded.contracts.len(), 1);
        assert_eq!(cache.len(), 1);
        let cached = cache.validate_and_load(&dir).unwrap();
        assert_eq!(cached.contracts.len(), 1);
        assert_eq!(cache.len(), 1);

        fs::write(
            dir.join("schemas/note.schema.json"),
            r#"{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "type": "object",
  "properties": {
    "body": { "type": "string" },
    "extra": { "type": "string" }
  },
  "required": ["body"],
  "additionalProperties": false
}
"#,
        )
        .unwrap();

        let err = cache.validate_and_load(&dir).unwrap_err();
        assert!(
            err.to_string()
                .contains("generated Memory contracts are not current"),
            "unexpected error: {err:?}"
        );

        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn memory_contract_cache_revalidates_when_build_metadata_identity_changes() {
        let dir = temp_dir("contract-cache-metadata");
        write_built_memory_package(&dir);
        let mut cache = MemoryContractCache::new();
        cache.validate_and_load(&dir).unwrap();

        let build_path = dir.join("memory/build.json");
        let mut build_metadata: Value =
            serde_json::from_slice(&fs::read(&build_path).unwrap()).unwrap();
        build_metadata["contracts_hash"] = json!("sha256:0000");
        fs::write(
            &build_path,
            serde_json::to_vec_pretty(&build_metadata).unwrap(),
        )
        .unwrap();

        let err = cache.validate_and_load(&dir).unwrap_err();
        assert!(
            err.to_string()
                .contains("generated Memory contracts are not current"),
            "unexpected error: {err:?}"
        );

        fs::remove_dir_all(dir).unwrap();
    }
}
