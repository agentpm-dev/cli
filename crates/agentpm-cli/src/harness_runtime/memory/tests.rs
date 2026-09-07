use super::custom::{
    CustomMemoryReadRequest, CustomMemoryWriteOperation, CustomMemoryWriteRequest,
};
use super::*;
use crate::harness_runtime::knowledge::KnowledgeRuntimeFailure;
use crate::harness_runtime::service::HostServiceInvoker;
use serde_json::json;
use std::sync::{Arc, Mutex};
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

    crate::commands::memory::execute_memory_build(&dir.join("agent.json"), MemoryBuildMode::Write)
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

    crate::commands::memory::execute_memory_build(&dir.join("agent.json"), MemoryBuildMode::Write)
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

fn total_vector_count(runtime: &LocalSqliteMemoryRuntime) -> u64 {
    runtime
        .connection
        .query_row("SELECT COUNT(*) FROM memory_vectors", [], |row| row.get(0))
        .unwrap()
}

fn enable_semantic_notes(manifest: &mut MemoryManifest) {
    let notes = manifest.memory.spaces.get_mut("notes").unwrap();
    if !notes
        .retrieval
        .modes
        .contains(&MemoryRetrievalMode::Semantic)
    {
        notes.retrieval.modes.push(MemoryRetrievalMode::Semantic);
    }
}

fn semantic_config() -> LocalMemorySemanticConfig {
    LocalMemorySemanticConfig {
        embedding_provider: "test".into(),
        embedding_model: "toy-2d".into(),
        dimensions: 2,
        normalized: true,
    }
}

#[derive(Debug, Default)]
struct TestEmbeddingProvider {
    calls: Vec<String>,
}

impl EmbeddingProvider for TestEmbeddingProvider {
    fn validate_space(&self, space: &KnowledgeEmbeddingSnapshot) -> Result<(), String> {
        if space.provider == "test"
            && space.model == "toy-2d"
            && space.dimensions == 2
            && space.metric == "cosine"
            && space.normalized
        {
            Ok(())
        } else {
            Err("unexpected embedding space".into())
        }
    }

    fn embed(
        &mut self,
        _space: &KnowledgeEmbeddingSnapshot,
        text: &str,
    ) -> std::result::Result<Vec<f32>, KnowledgeRuntimeFailure> {
        self.calls.push(text.to_string());
        if text.contains("alpha") {
            Ok(vec![1.0, 0.0])
        } else if text.contains("beta") {
            Ok(vec![0.0, 1.0])
        } else {
            Ok(vec![0.5, 0.5])
        }
    }
}

#[derive(Debug, Default)]
struct FailingEmbeddingProvider;

impl EmbeddingProvider for FailingEmbeddingProvider {
    fn validate_space(&self, _space: &KnowledgeEmbeddingSnapshot) -> Result<(), String> {
        Ok(())
    }

    fn embed(
        &mut self,
        _space: &KnowledgeEmbeddingSnapshot,
        _text: &str,
    ) -> std::result::Result<Vec<f32>, KnowledgeRuntimeFailure> {
        Err(KnowledgeRuntimeFailure::new(
            "embedding_provider_failed",
            "test embedder failed",
        ))
    }
}

#[derive(Debug)]
struct DeletingEmbeddingProvider {
    database_path: PathBuf,
    record_id: String,
    calls: Vec<String>,
    deleted: bool,
}

impl EmbeddingProvider for DeletingEmbeddingProvider {
    fn validate_space(&self, _space: &KnowledgeEmbeddingSnapshot) -> Result<(), String> {
        Ok(())
    }

    fn embed(
        &mut self,
        _space: &KnowledgeEmbeddingSnapshot,
        text: &str,
    ) -> std::result::Result<Vec<f32>, KnowledgeRuntimeFailure> {
        self.calls.push(text.to_string());
        if !self.deleted && text.contains("alpha durable note") {
            let connection = Connection::open(&self.database_path).map_err(|err| {
                KnowledgeRuntimeFailure::new("test_delete_failed", err.to_string())
            })?;
            connection
                .execute(
                    "DELETE FROM memory_records WHERE id = ?1",
                    params![&self.record_id],
                )
                .map_err(|err| {
                    KnowledgeRuntimeFailure::new("test_delete_failed", err.to_string())
                })?;
            self.deleted = true;
        }
        if text.contains("alpha") {
            Ok(vec![1.0, 0.0])
        } else {
            Ok(vec![0.5, 0.5])
        }
    }
}

fn vector_row(runtime: &LocalSqliteMemoryRuntime, record_id: &str) -> (String, Vec<u8>) {
    runtime
        .connection
        .query_row(
            "SELECT content_hash, vector FROM memory_vectors WHERE record_id = ?1",
            params![record_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
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

    let (ordered_json, ordered_hash) = LocalSqliteMemoryRuntime::scope_identity(&ordered).unwrap();
    let (reversed_json, reversed_hash) =
        LocalSqliteMemoryRuntime::scope_identity(&reversed).unwrap();

    assert_eq!(ordered_json, r#"{"thread":"t-456","user":"u-123"}"#);
    assert_eq!(ordered_json, reversed_json);
    assert_eq!(ordered_hash, reversed_hash);
    LocalSqliteMemoryRuntime::verify_scope_identity(&ordered_json, &ordered_hash).unwrap();
    assert!(LocalSqliteMemoryRuntime::verify_scope_identity(&ordered_json, "sha256:bad").is_err());
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
    array_equality_filter.filter = BTreeMap::from([("labels".into(), json!(["bug", "release"]))]);
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

    let mut conjunctive_filter = m14b_read_request(&manifest, "notes", LocalMemoryReadMode::Filter);
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

    let mut delete_read = m14b_read_request(&manifest, "delete_notes", LocalMemoryReadMode::Key);
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
fn local_sqlite_with_semantic_adds_semantic_retrieval() {
    let capabilities = MemoryRuntimeCapabilityDescriptor::local_sqlite_with_semantic();

    assert!(
        capabilities
            .retrieval_modes
            .contains(&MemoryRetrievalMode::Semantic)
    );
    assert!(capabilities.capacity);
    assert!(capabilities.atomic_batches);
}

#[test]
fn sqlite_semantic_read_backfills_vectors_and_ranks_exact_cosine() {
    let dir = temp_dir("m14d-semantic-read");
    let (mut manifest, contracts) = write_m14b_memory_package(&dir);
    enable_semantic_notes(&mut manifest);
    let mut runtime = LocalSqliteMemoryRuntime::open(&dir, None).unwrap();

    let alpha = runtime
        .write_record(m14b_write_request(
            &manifest,
            &contracts,
            "notes",
            "note",
            Some(json!({ "body": "alpha durable note" })),
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
            Some(json!({ "body": "beta durable note" })),
        ))
        .unwrap()
        .record
        .unwrap();

    runtime
            .connection
            .execute(
                r#"
                INSERT INTO memory_vectors (
                    record_id, package, package_version, space, record_type, scope_hash,
                    embedding_provider, embedding_model, dimensions, content_hash, vector, updated_at
                )
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'test', 'toy-2d', 2, 'sha256:stale', ?7, ?8)
                "#,
                params![
                    &beta.id,
                    &beta.package,
                    &beta.package_version,
                    &beta.space,
                    &beta.record_type,
                    &beta.scope_hash,
                    encode_f32_le_vector(&[1.0, 0.0], 2).unwrap(),
                    Utc::now().to_rfc3339(),
                ],
            )
            .unwrap();

    let mut request = m14b_read_request(&manifest, "notes", LocalMemoryReadMode::Semantic);
    request.record_type = Some("note".into());
    request.query = Some("alpha query".into());
    request.limit = Some(5);
    let mut embedder = TestEmbeddingProvider::default();
    let result = runtime
        .read_records_semantic(request, &semantic_config(), &mut embedder)
        .unwrap();

    assert_eq!(result.embedding_requests, 3);
    assert_eq!(result.records.len(), 2);
    assert_eq!(result.records[0].id, alpha.id);
    assert_eq!(result.records[1].id, beta.id);
    let (alpha_hash, alpha_blob) = vector_row(&runtime, &alpha.id);
    assert_eq!(alpha_blob, encode_f32_le_vector(&[1.0, 0.0], 2).unwrap());
    assert_eq!(
        alpha_hash,
        durable_memory_content_hash(&alpha.content).unwrap()
    );
    let (beta_hash, beta_blob) = vector_row(&runtime, &beta.id);
    assert_eq!(beta_blob, encode_f32_le_vector(&[0.0, 1.0], 2).unwrap());
    assert_eq!(
        beta_hash,
        durable_memory_content_hash(&beta.content).unwrap()
    );

    let mut cached_request = m14b_read_request(&manifest, "notes", LocalMemoryReadMode::Semantic);
    cached_request.record_type = Some("note".into());
    cached_request.query = Some("alpha query".into());
    let mut cached_embedder = TestEmbeddingProvider::default();
    let cached = runtime
        .read_records_semantic(cached_request, &semantic_config(), &mut cached_embedder)
        .unwrap();
    assert_eq!(cached.embedding_requests, 1);
    assert_eq!(cached_embedder.calls, vec!["alpha query"]);

    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn sqlite_semantic_write_generates_vector_best_effort() {
    let dir = temp_dir("m14d-semantic-write-vector");
    let (mut manifest, contracts) = write_m14b_memory_package(&dir);
    enable_semantic_notes(&mut manifest);
    let mut runtime = LocalSqliteMemoryRuntime::open(&dir, None).unwrap();

    let mut embedder = TestEmbeddingProvider::default();
    let result = runtime
        .write_record_with_semantic(
            m14b_write_request(
                &manifest,
                &contracts,
                "notes",
                "note",
                Some(json!({
                    "body": "alpha durable note",
                    "secret": "ephemeral-secret"
                })),
            ),
            &semantic_config(),
            &mut embedder,
        )
        .unwrap();
    assert_eq!(result.embedding_requests, 1);
    assert!(result.semantic_embedding_error.is_none());
    let record = result.record.unwrap();
    assert_eq!(vector_count(&runtime, &record.id), 1);
    assert_eq!(embedder.calls.len(), 1);
    assert!(!embedder.calls[0].contains("ephemeral-secret"));
    let (_hash, blob) = vector_row(&runtime, &record.id);
    assert_eq!(
        blob,
        vec![
            0x00, 0x00, 0x80, 0x3f, // 1.0f32 little-endian
            0x00, 0x00, 0x00, 0x00, // 0.0f32 little-endian
        ]
    );

    let mut failing_embedder = FailingEmbeddingProvider;
    let failed_embedding = runtime
        .write_record_with_semantic(
            m14b_write_request(
                &manifest,
                &contracts,
                "notes",
                "note",
                Some(json!({ "body": "beta durable note" })),
            ),
            &semantic_config(),
            &mut failing_embedder,
        )
        .unwrap();
    assert_eq!(failed_embedding.embedding_requests, 1);
    assert!(
        failed_embedding
            .semantic_embedding_error
            .as_deref()
            .is_some_and(|error| error.contains("test embedder failed"))
    );
    let failed_record = failed_embedding.record.unwrap();
    assert_eq!(vector_count(&runtime, &failed_record.id), 0);

    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn sqlite_semantic_mutations_delete_or_replace_vectors() {
    let dir = temp_dir("m14d-semantic-mutation-vectors");
    let (mut manifest, contracts) = write_m14b_memory_package(&dir);
    enable_semantic_notes(&mut manifest);
    let mut runtime = LocalSqliteMemoryRuntime::open(&dir, None).unwrap();

    let mut embedder = TestEmbeddingProvider::default();
    let record = runtime
        .write_record_with_semantic(
            m14b_write_request(
                &manifest,
                &contracts,
                "notes",
                "note",
                Some(json!({ "body": "alpha durable note" })),
            ),
            &semantic_config(),
            &mut embedder,
        )
        .unwrap()
        .record
        .unwrap();
    assert_eq!(vector_count(&runtime, &record.id), 1);

    let mut update = m14b_write_request(
        &manifest,
        &contracts,
        "notes",
        "note",
        Some(json!({ "body": "beta durable note" })),
    );
    update.operation = LocalMemoryWriteOperation::Update;
    update.record_id = Some(record.id.clone());
    let updated = runtime.write_record(update).unwrap().record.unwrap();
    assert_eq!(vector_count(&runtime, &updated.id), 0);

    let mut semantic_update = m14b_write_request(
        &manifest,
        &contracts,
        "notes",
        "note",
        Some(json!({ "body": "alpha durable note again" })),
    );
    semantic_update.operation = LocalMemoryWriteOperation::Update;
    semantic_update.record_id = Some(updated.id.clone());
    let mut update_embedder = TestEmbeddingProvider::default();
    let regenerated = runtime
        .write_record_with_semantic(semantic_update, &semantic_config(), &mut update_embedder)
        .unwrap()
        .record
        .unwrap();
    assert_eq!(vector_count(&runtime, &regenerated.id), 1);

    let mut delete = m14b_write_request(&manifest, &contracts, "notes", "note", None);
    delete.operation = LocalMemoryWriteOperation::Delete;
    delete.record_id = Some(regenerated.id.clone());
    runtime.write_record(delete).unwrap();
    assert_eq!(vector_count(&runtime, &regenerated.id), 0);

    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn sqlite_semantic_read_bounds_lazy_backfill_and_reports_pending() {
    let dir = temp_dir("m14d-semantic-bounded-backfill");
    let (mut manifest, contracts) = write_m14b_memory_package(&dir);
    enable_semantic_notes(&mut manifest);
    {
        let notes = manifest.memory.spaces.get_mut("notes").unwrap();
        notes.capacity = None;
        notes.retention = None;
    }
    let mut runtime = LocalSqliteMemoryRuntime::open(&dir, None).unwrap();

    let total_records = LOCAL_MEMORY_SEMANTIC_READ_BACKFILL_LIMIT + 3;
    for index in 0..total_records {
        runtime
            .write_record(m14b_write_request(
                &manifest,
                &contracts,
                "notes",
                "note",
                Some(json!({ "body": format!("gamma durable note {index}") })),
            ))
            .unwrap();
    }

    let mut request = m14b_read_request(&manifest, "notes", LocalMemoryReadMode::Semantic);
    request.record_type = Some("note".into());
    request.query = Some("alpha query".into());
    request.limit = Some(5);
    let mut embedder = TestEmbeddingProvider::default();
    let result = runtime
        .read_records_semantic(request, &semantic_config(), &mut embedder)
        .unwrap();

    assert_eq!(result.records.len(), 5);
    assert_eq!(
        result.vectors_materialized,
        LOCAL_MEMORY_SEMANTIC_READ_BACKFILL_LIMIT as u64
    );
    assert_eq!(result.vectors_pending, 3);
    assert_eq!(
        result.embedding_requests,
        1 + LOCAL_MEMORY_SEMANTIC_READ_BACKFILL_LIMIT as u64
    );
    assert_eq!(
        total_vector_count(&runtime),
        LOCAL_MEMORY_SEMANTIC_READ_BACKFILL_LIMIT as u64
    );

    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn sqlite_semantic_read_skips_backfill_upsert_when_snapshot_is_stale() {
    let dir = temp_dir("m14d-semantic-stale-backfill");
    let (mut manifest, contracts) = write_m14b_memory_package(&dir);
    enable_semantic_notes(&mut manifest);
    let mut runtime = LocalSqliteMemoryRuntime::open(&dir, None).unwrap();

    let record = runtime
        .write_record(m14b_write_request(
            &manifest,
            &contracts,
            "notes",
            "note",
            Some(json!({ "body": "alpha durable note" })),
        ))
        .unwrap()
        .record
        .unwrap();

    let mut request = m14b_read_request(&manifest, "notes", LocalMemoryReadMode::Semantic);
    request.record_type = Some("note".into());
    request.query = Some("lookup query".into());
    let mut embedder = DeletingEmbeddingProvider {
        database_path: runtime.database_path().to_path_buf(),
        record_id: record.id.clone(),
        calls: Vec::new(),
        deleted: false,
    };
    let result = runtime
        .read_records_semantic(request, &semantic_config(), &mut embedder)
        .unwrap();

    assert_eq!(result.embedding_requests, 2);
    assert_eq!(result.vectors_materialized, 0);
    assert_eq!(result.vectors_pending, 1);
    assert!(result.records.is_empty());
    assert_eq!(vector_count(&runtime, &record.id), 0);
    assert!(
        runtime
            .get_record(
                &record.package,
                &record.package_version,
                &record.space,
                &BTreeMap::from([("user".into(), "m14b-user".into())]),
                &record.id
            )
            .unwrap()
            .is_none()
    );

    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn sqlite_semantic_read_filters_candidates_and_maps_embedding_failures() {
    let dir = temp_dir("m14d-semantic-filter-failure");
    let (mut manifest, contracts) = write_m14b_memory_package(&dir);
    enable_semantic_notes(&mut manifest);
    let mut runtime = LocalSqliteMemoryRuntime::open(&dir, None).unwrap();

    let kept = runtime
        .write_record(m14b_write_request(
            &manifest,
            &contracts,
            "notes",
            "note",
            Some(json!({ "body": "alpha durable note", "tag": "keep" })),
        ))
        .unwrap()
        .record
        .unwrap();
    let skipped = runtime
        .write_record(m14b_write_request(
            &manifest,
            &contracts,
            "notes",
            "note",
            Some(json!({ "body": "beta durable note", "tag": "skip" })),
        ))
        .unwrap()
        .record
        .unwrap();

    let mut request = m14b_read_request(&manifest, "notes", LocalMemoryReadMode::Semantic);
    request.record_type = Some("note".into());
    request.filter = BTreeMap::from([("tag".into(), json!("keep"))]);
    request.query = Some("alpha query".into());
    let mut embedder = TestEmbeddingProvider::default();
    let result = runtime
        .read_records_semantic(request, &semantic_config(), &mut embedder)
        .unwrap();
    assert_eq!(result.records.len(), 1);
    assert_eq!(result.records[0].id, kept.id);
    assert_eq!(vector_count(&runtime, &kept.id), 1);
    assert_eq!(vector_count(&runtime, &skipped.id), 0);
    assert!(
        embedder
            .calls
            .iter()
            .all(|call| !call.contains("beta durable note"))
    );

    let mut failing_request = m14b_read_request(&manifest, "notes", LocalMemoryReadMode::Semantic);
    failing_request.record_type = Some("note".into());
    failing_request.query = Some("alpha query".into());
    let mut failing_embedder = FailingEmbeddingProvider;
    let failure = runtime
        .read_records_semantic(failing_request, &semantic_config(), &mut failing_embedder)
        .unwrap_err();
    assert_eq!(
        local_memory_error_code(&failure),
        Some("embedding_provider_failed")
    );

    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn sqlite_semantic_embedding_uses_persistable_projection_only() {
    let dir = temp_dir("m14d-semantic-projection");
    let (mut manifest, contracts) = write_m14b_memory_package(&dir);
    enable_semantic_notes(&mut manifest);
    let mut runtime = LocalSqliteMemoryRuntime::open(&dir, None).unwrap();

    let record = runtime
        .write_record(m14b_write_request(
            &manifest,
            &contracts,
            "notes",
            "note",
            Some(json!({
                "body": "alpha durable note",
                "secret": "ephemeral-secret",
                "nested": {
                    "visible": "kept",
                    "ephemeral": "ephemeral-nested-secret"
                }
            })),
        ))
        .unwrap()
        .record
        .unwrap();
    assert_eq!(
        record.content,
        json!({
            "body": "alpha durable note",
            "nested": { "visible": "kept" }
        })
    );

    let mut request = m14b_read_request(&manifest, "notes", LocalMemoryReadMode::Semantic);
    request.record_type = Some("note".into());
    request.query = Some("alpha query".into());
    let mut embedder = TestEmbeddingProvider::default();
    runtime
        .read_records_semantic(request, &semantic_config(), &mut embedder)
        .unwrap();

    assert!(embedder.calls.iter().all(
        |call| !call.contains("ephemeral-secret") && !call.contains("ephemeral-nested-secret")
    ));
    let (content_hash, _blob) = vector_row(&runtime, &record.id);
    assert_eq!(
        content_hash,
        durable_memory_content_hash(&record.content).unwrap()
    );

    fs::remove_dir_all(dir).unwrap();
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
    assert!(reasons.contains(&"no supported retrieval modes; declared modes: `Semantic`"));
}

#[test]
fn custom_memory_write_request_uses_durable_projection_before_dispatch() {
    let dir = temp_dir("custom-memory-durable-projection");
    let (manifest, contracts) = write_m14b_memory_package(&dir);
    let request = m14b_write_request(
        &manifest,
        &contracts,
        "notes",
        "note",
        Some(json!({
            "body": "alpha durable note",
            "secret": "ephemeral-secret",
            "nested": {
                "visible": "kept",
                "ephemeral": "ephemeral-nested-secret"
            }
        })),
    );

    let custom_request = custom_memory_write_request_from_local(&request).unwrap();

    assert_eq!(
        custom_request.content,
        Some(json!({
            "body": "alpha durable note",
            "nested": { "visible": "kept" }
        }))
    );
    let payload = serde_json::to_string(&custom_request).unwrap();
    assert!(!payload.contains("ephemeral-secret"));
    assert!(!payload.contains("ephemeral-nested-secret"));

    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn custom_memory_capability_validation_rejects_ready_false() {
    let spaces = Vec::new();
    let err = validate_memory_runtime_capabilities(
        &json!({
            "ready": false,
            "capabilities": {
                "space_models": ["collection"],
                "retrieval_modes": ["key"],
                "retention_actions": [],
                "constraints": [],
                "capacity": false,
                "durable_trigger_state": false,
                "atomic_batches": false
            }
        }),
        "remote-memory",
        &spaces,
    )
    .unwrap_err();

    assert!(err.to_string().contains("ready=false"));
}

#[test]
fn custom_memory_capability_validation_rejects_package_version_mismatch() {
    let spaces = vec![crate::harness_runtime::model::MemorySpaceRuntimeSnapshot {
        package: "memory-test".into(),
        package_version: "0.1.0".into(),
        space: "notes".into(),
        model: MemorySpaceModel::Collection,
        description: "Notes.".into(),
        root: None,
        runtime: "remote-memory".into(),
        source: "agent_binding".into(),
        state: "available".into(),
        readiness_reason: None,
        binding_scope: "global".into(),
        scope_keys: vec!["user".into()],
        retrieval_modes: vec![MemoryRetrievalMode::Key],
        semantic: None,
        append_only: false,
        record_types: Vec::new(),
    }];
    let err = validate_memory_runtime_capabilities(
        &json!({
            "ready": true,
            "capabilities": {
                "space_models": ["collection"],
                "retrieval_modes": ["key"],
                "retention_actions": [],
                "constraints": [],
                "capacity": false,
                "durable_trigger_state": false,
                "atomic_batches": false,
                "packages": [
                    { "package": "memory-test", "version": "0.2.0", "ready": true }
                ]
            }
        }),
        "remote-memory",
        &spaces,
    )
    .unwrap_err();

    assert!(
        err.to_string()
            .contains("does not attest memory-test@0.1.0 as ready")
    );
}

#[test]
fn custom_memory_capability_validation_rejects_not_ready_or_malformed_descriptor() {
    let spaces = Vec::new();
    let not_ready = validate_memory_runtime_capabilities(
        &json!({
            "ready": false,
            "capabilities": {
                "space_models": ["collection"],
                "retrieval_modes": ["key"],
                "retention_actions": [],
                "constraints": [],
                "capacity": false,
                "durable_trigger_state": false,
                "atomic_batches": false
            }
        }),
        "remote-memory",
        &spaces,
    )
    .unwrap_err();
    assert!(not_ready.to_string().contains("ready=false"));

    let malformed = validate_memory_runtime_capabilities(
        &json!({
            "space_models": ["collection"]
        }),
        "remote-memory",
        &spaces,
    )
    .unwrap_err();
    assert!(malformed.to_string().contains("validating MemoryRuntime"));
}

#[test]
fn custom_memory_runtime_dispatches_read_to_routed_service() {
    #[derive(Clone)]
    struct RecordingInvoker {
        calls: Arc<Mutex<Vec<(String, String, String, Value)>>>,
    }

    impl HostServiceInvoker for RecordingInvoker {
        fn invoke_host_service(
            &mut self,
            role: &str,
            registry_id: &str,
            method: &str,
            payload: Value,
            _timeout_ms: u64,
        ) -> Result<Value> {
            self.calls.lock().unwrap().push((
                role.to_string(),
                registry_id.to_string(),
                method.to_string(),
                payload,
            ));
            Ok(json!({
                "ok": true,
                "package": "memory-test",
                "package_version": "0.1.0",
                "space": "notes",
                "mode": "key",
                "records": [],
                "count": 0
            }))
        }
    }

    let calls = Arc::new(Mutex::new(Vec::new()));
    let mut runtime = CustomMemoryRuntime::new(
        vec![crate::harness_runtime::model::MemorySpaceRuntimeSnapshot {
            package: "memory-test".into(),
            package_version: "0.1.0".into(),
            space: "notes".into(),
            model: MemorySpaceModel::Collection,
            description: "Notes.".into(),
            root: None,
            runtime: "remote-memory".into(),
            source: "agent_binding".into(),
            state: "available".into(),
            readiness_reason: None,
            binding_scope: "global".into(),
            scope_keys: vec!["user".into()],
            retrieval_modes: vec![MemoryRetrievalMode::Key],
            semantic: None,
            append_only: false,
            record_types: Vec::new(),
        }],
        HashMap::from([(
            "remote-memory".into(),
            ServiceRuntime::host(
                Box::new(RecordingInvoker {
                    calls: calls.clone(),
                }),
                1_000,
            ),
        )]),
    );

    let result = runtime.dispatch_read(CustomMemoryReadRequest {
        package: "memory-test".into(),
        package_version: "0.1.0".into(),
        space: "notes".into(),
        scope: BTreeMap::from([("user".into(), "u-123".into())]),
        mode: MemoryRetrievalMode::Key,
        record_id: Some("rec-1".into()),
        record_type: None,
        filter: BTreeMap::new(),
        query: None,
        limit: None,
        now: Utc::now().to_rfc3339(),
    });

    assert!(result.ok);
    assert_eq!(result.output["ok"], json!(true));
    let calls = calls.lock().unwrap();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].0, "memory");
    assert_eq!(calls[0].1, "remote-memory");
    assert_eq!(calls[0].2, "read");
    assert_eq!(calls[0].3["request"]["package"], json!("memory-test"));
    assert_eq!(calls[0].3["request"]["space"], json!("notes"));
}

#[test]
fn custom_memory_runtime_rejects_malformed_read_result() {
    #[derive(Clone)]
    struct MalformedInvoker;

    impl HostServiceInvoker for MalformedInvoker {
        fn invoke_host_service(
            &mut self,
            _role: &str,
            _registry_id: &str,
            _method: &str,
            _payload: Value,
            _timeout_ms: u64,
        ) -> Result<Value> {
            Ok(json!({
                "ok": true,
                "package": "memory-test",
                "package_version": "0.1.0",
                "space": "notes",
                "records": [],
                "count": 0
            }))
        }
    }

    let mut runtime = CustomMemoryRuntime::new(
        vec![crate::harness_runtime::model::MemorySpaceRuntimeSnapshot {
            package: "memory-test".into(),
            package_version: "0.1.0".into(),
            space: "notes".into(),
            model: MemorySpaceModel::Collection,
            description: "Notes.".into(),
            root: None,
            runtime: "remote-memory".into(),
            source: "agent_binding".into(),
            state: "available".into(),
            readiness_reason: None,
            binding_scope: "global".into(),
            scope_keys: vec!["user".into()],
            retrieval_modes: vec![MemoryRetrievalMode::Key],
            semantic: None,
            append_only: false,
            record_types: Vec::new(),
        }],
        HashMap::from([(
            "remote-memory".into(),
            ServiceRuntime::host(Box::new(MalformedInvoker), 1_000),
        )]),
    );

    let result = runtime.dispatch_read(CustomMemoryReadRequest {
        package: "memory-test".into(),
        package_version: "0.1.0".into(),
        space: "notes".into(),
        scope: BTreeMap::from([("user".into(), "u-123".into())]),
        mode: MemoryRetrievalMode::Key,
        record_id: Some("rec-1".into()),
        record_type: None,
        filter: BTreeMap::new(),
        query: None,
        limit: None,
        now: Utc::now().to_rfc3339(),
    });

    assert!(result.ok);
    assert_eq!(result.output["ok"], json!(false));
    assert_eq!(
        result.output["error"]["code"],
        json!("malformed_memory_runtime_response")
    );
    assert!(
        result.output["error"]["message"]
            .as_str()
            .is_some_and(|message| message.contains("malformed read result"))
    );
}

#[test]
fn custom_memory_host_request_failure_emits_service_events() {
    #[derive(Clone)]
    struct FailingInvoker;

    impl HostServiceInvoker for FailingInvoker {
        fn invoke_host_service(
            &mut self,
            _role: &str,
            _registry_id: &str,
            _method: &str,
            _payload: Value,
            _timeout_ms: u64,
        ) -> Result<Value> {
            Err(anyhow::anyhow!("host memory read failed"))
        }
    }

    let mut service_events = crate::harness_runtime::ServiceLifecycleEvents::new();
    let mut runtime = CustomMemoryRuntime::with_lifecycle(
        vec![crate::harness_runtime::model::MemorySpaceRuntimeSnapshot {
            package: "memory-test".into(),
            package_version: "0.1.0".into(),
            space: "notes".into(),
            model: MemorySpaceModel::Collection,
            description: "Notes.".into(),
            root: None,
            runtime: "remote-memory".into(),
            source: "agent_binding".into(),
            state: "available".into(),
            readiness_reason: None,
            binding_scope: "global".into(),
            scope_keys: vec!["user".into()],
            retrieval_modes: vec![MemoryRetrievalMode::Key],
            semantic: None,
            append_only: false,
            record_types: Vec::new(),
        }],
        HashMap::from([(
            "remote-memory".into(),
            ServiceRuntime::host(Box::new(FailingInvoker), 1_000),
        )]),
        Some(service_events.emitter()),
    );

    let result = runtime.dispatch_read(CustomMemoryReadRequest {
        package: "memory-test".into(),
        package_version: "0.1.0".into(),
        space: "notes".into(),
        scope: BTreeMap::from([("user".into(), "u-123".into())]),
        mode: MemoryRetrievalMode::Key,
        record_id: Some("rec-1".into()),
        record_type: None,
        filter: BTreeMap::new(),
        query: None,
        limit: None,
        now: Utc::now().to_rfc3339(),
    });

    assert!(result.ok);
    assert_eq!(result.output["ok"], json!(false));
    assert_eq!(
        result.output["error"]["code"],
        json!("memory_runtime_failed")
    );

    let events = service_events.drain();
    assert!(events.iter().any(|event| {
        event.event_type == crate::harness_observability::HarnessEventType::ServiceUnhealthy
            && event.service == "memory"
            && event.registry_id == "remote-memory"
            && event.message.contains("host memory read failed")
    }));
    assert!(events.iter().any(|event| {
        event.event_type == crate::harness_observability::HarnessEventType::ServiceFailed
            && event.service == "memory"
            && event.registry_id == "remote-memory"
            && event.message.contains("host memory read failed")
    }));
}

#[test]
fn custom_memory_runtime_rejects_mismatched_write_result() {
    #[derive(Clone)]
    struct MismatchedInvoker;

    impl HostServiceInvoker for MismatchedInvoker {
        fn invoke_host_service(
            &mut self,
            _role: &str,
            _registry_id: &str,
            _method: &str,
            _payload: Value,
            _timeout_ms: u64,
        ) -> Result<Value> {
            Ok(json!({
                "ok": true,
                "package": "memory-test",
                "package_version": "0.1.0",
                "space": "notes",
                "operation": "delete",
                "record_id": "rec-1",
                "record": null
            }))
        }
    }

    let mut runtime = CustomMemoryRuntime::new(
        vec![crate::harness_runtime::model::MemorySpaceRuntimeSnapshot {
            package: "memory-test".into(),
            package_version: "0.1.0".into(),
            space: "notes".into(),
            model: MemorySpaceModel::Collection,
            description: "Notes.".into(),
            root: None,
            runtime: "remote-memory".into(),
            source: "agent_binding".into(),
            state: "available".into(),
            readiness_reason: None,
            binding_scope: "global".into(),
            scope_keys: vec!["user".into()],
            retrieval_modes: vec![MemoryRetrievalMode::Key],
            semantic: None,
            append_only: false,
            record_types: Vec::new(),
        }],
        HashMap::from([(
            "remote-memory".into(),
            ServiceRuntime::host(Box::new(MismatchedInvoker), 1_000),
        )]),
    );

    let result = runtime.dispatch_write(CustomMemoryWriteRequest {
        package: "memory-test".into(),
        package_version: "0.1.0".into(),
        space: "notes".into(),
        space_model: MemorySpaceModel::Collection,
        record_type: "note".into(),
        schema_version: "1.0.0".into(),
        scope: BTreeMap::from([("user".into(), "u-123".into())]),
        operation: CustomMemoryWriteOperation::Create,
        record_id: None,
        content: Some(json!({ "body": "alpha" })),
        provenance: json!({}),
        now: Utc::now().to_rfc3339(),
    });

    assert!(result.ok);
    assert_eq!(result.output["ok"], json!(false));
    assert_eq!(
        result.output["error"]["code"],
        json!("malformed_memory_runtime_response")
    );
    assert!(
        result.output["error"]["message"]
            .as_str()
            .is_some_and(|message| message.contains("returned operation `Delete`"))
    );
}

#[test]
fn custom_memory_failure_codes_map_to_local_error_kinds() {
    let not_found = custom_memory_action_error_from_output(&json!({
        "ok": false,
        "package": "memory-test",
        "package_version": "0.1.0",
        "space": "notes",
        "error": {
            "code": "not_found",
            "message": "missing record"
        }
    }))
    .expect("mapped not_found");
    assert_eq!(not_found.code(), "not_found");
    assert!(not_found.is_model_correctable());

    let capacity = custom_memory_action_error_from_output(&json!({
        "ok": false,
        "package": "memory-test",
        "package_version": "0.1.0",
        "space": "notes",
        "error": {
            "code": "capacity_exceeded",
            "message": "space is full"
        }
    }))
    .expect("mapped capacity_exceeded");
    assert_eq!(capacity.code(), "capacity_exceeded");
    assert!(!capacity.is_model_correctable());

    assert!(
        custom_memory_action_error_from_output(&json!({
            "ok": false,
            "package": "memory-test",
            "package_version": "0.1.0",
            "space": "notes",
            "error": {
                "code": "memory_runtime_failed",
                "message": "backend unavailable"
            }
        }))
        .is_none()
    );
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
