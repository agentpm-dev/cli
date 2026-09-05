use crate::commands::memory::{
    GeneratedMemoryContract, MemoryBuildMetadata, MemoryBuildMode, MemoryContractIndex,
    execute_memory_build_with_output,
};
use crate::manifest::{
    MemoryManifest, MemoryRetentionAction, MemoryRetrievalMode, MemorySpaceModel,
    resolve_existing_relative_file,
};
use anyhow::{Context, Result, anyhow, bail};
use chrono::{DateTime, Utc};
use rusqlite::{Connection, OptionalExtension, Transaction, params};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, UNIX_EPOCH};

const LOCAL_MEMORY_SCHEMA_VERSION: u64 = 1;
const LOCAL_MEMORY_DB_NAME: &str = "memory.sqlite3";
const LOCAL_MEMORY_BUSY_TIMEOUT_MS: u64 = 5_000;

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
            retrieval_modes: vec![MemoryRetrievalMode::Key],
            retention_actions: Vec::new(),
            constraints: Vec::new(),
            capacity: false,
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

#[derive(Debug, Clone, PartialEq)]
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
    pub ordinal: Option<i64>,
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
        )
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
) -> Result<Option<StoredMemoryRecord>> {
    let (_scope_json, scope_hash) = LocalSqliteMemoryRuntime::scope_identity(scope)?;
    connection
        .query_row(
            r#"
            SELECT id, package, package_version, space, space_model, record_type,
                   schema_version, scope_json, scope_hash, content_json, provenance_json,
                   ordinal
            FROM memory_records
            WHERE package = ?1 AND package_version = ?2 AND space = ?3
              AND scope_hash = ?4 AND id = ?5 AND archived_at IS NULL
            "#,
            params![package, package_version, space, scope_hash, record_id],
            |row| {
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
                        rusqlite::Error::FromSqlConversionFailure(
                            9,
                            rusqlite::types::Type::Text,
                            Box::new(err),
                        )
                    })?,
                    provenance: serde_json::from_str(&provenance_json).map_err(|err| {
                        rusqlite::Error::FromSqlConversionFailure(
                            10,
                            rusqlite::types::Type::Text,
                            Box::new(err),
                        )
                    })?,
                    ordinal: row.get(11)?,
                })
            },
        )
        .optional()
        .context("reading Memory record")
}

fn active_memory_record_count(
    connection: &Connection,
    package: &str,
    package_version: &str,
    space: &str,
    scope: &BTreeMap<String, String>,
    record_type: Option<&str>,
) -> Result<u64> {
    let (_scope_json, scope_hash) = LocalSqliteMemoryRuntime::scope_identity(scope)?;
    let count = match record_type {
        Some(record_type) => connection.query_row(
            r#"
            SELECT COUNT(*)
            FROM memory_records
            WHERE package = ?1 AND package_version = ?2 AND space = ?3
              AND scope_hash = ?4 AND record_type = ?5 AND archived_at IS NULL
            "#,
            params![package, package_version, space, scope_hash, record_type],
            |row| row.get::<_, u64>(0),
        ),
        None => connection.query_row(
            r#"
            SELECT COUNT(*)
            FROM memory_records
            WHERE package = ?1 AND package_version = ?2 AND space = ?3
              AND scope_hash = ?4 AND archived_at IS NULL
            "#,
            params![package, package_version, space, scope_hash],
            |row| row.get::<_, u64>(0),
        ),
    }
    .context("counting active Memory records")?;
    Ok(count)
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
    fn local_sqlite_capabilities_advertise_only_m14a_primitives() {
        let capabilities = MemoryRuntimeCapabilityDescriptor::local_sqlite();

        assert_eq!(
            capabilities.space_models,
            vec![
                MemorySpaceModel::Document,
                MemorySpaceModel::Collection,
                MemorySpaceModel::Sequence
            ]
        );
        assert_eq!(capabilities.retrieval_modes, vec![MemoryRetrievalMode::Key]);
        assert!(capabilities.retention_actions.is_empty());
        assert!(capabilities.constraints.is_empty());
        assert!(!capabilities.capacity);
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
                        "retrieval": { "modes": ["filter", "full_text", "semantic"] },
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

        assert_eq!(diagnostics.len(), 6);
        let reasons = diagnostics
            .iter()
            .map(|diagnostic| diagnostic.reason.as_str())
            .collect::<Vec<_>>();
        assert!(
            diagnostics
                .iter()
                .all(|diagnostic| diagnostic.space == "notes")
        );
        assert!(reasons.contains(&"retrieval mode `Filter` is not supported"));
        assert!(reasons.contains(&"retrieval mode `FullText` is not supported"));
        assert!(reasons.contains(&"retrieval mode `Semantic` is not supported"));
        assert!(reasons.contains(&"retention action `Archive` is not supported"));
        assert!(reasons.contains(&"capacity checks are not supported"));
        assert!(reasons.contains(&"append-only constraints are not supported"));
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
