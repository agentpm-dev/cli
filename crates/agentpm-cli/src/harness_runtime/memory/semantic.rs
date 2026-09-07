use super::*;

impl LocalSqliteMemoryRuntime {
    pub fn read_records_semantic(
        &mut self,
        request: LocalMemoryReadRequest<'_>,
        semantic: &LocalMemorySemanticConfig,
        embedder: &mut dyn EmbeddingProvider,
    ) -> Result<LocalMemoryReadResult> {
        validate_memory_scope(request.manifest, request.space, &request.scope)?;
        ensure_retrieval_mode(
            request.manifest,
            request.space,
            MemoryRetrievalMode::Semantic,
        )?;
        let query = request
            .query
            .as_deref()
            .filter(|query| !query.trim().is_empty())
            .ok_or_else(|| {
                LocalMemoryActionError::constraint_violation(
                    "Memory semantic read requires a non-empty query",
                )
            })?;
        let embedding_space = semantic.embedding_space();
        embedder
            .validate_space(&embedding_space)
            .map_err(LocalMemoryActionError::embedding_provider_unavailable)?;

        let (query_vector, query_duration_ms) =
            embed_text_timed(embedder, &embedding_space, query)?;
        validate_vector_dimensions(&query_vector, semantic.dimensions)?;

        let space = memory_space(request.manifest, request.space)?;
        let (mut ranked, backfill_candidates, mut vectors_pending) =
            self.atomic_batch(|batch| {
                expire_memory_records_for_space(
                    &batch.transaction,
                    request.package,
                    request.package_version,
                    request.space,
                    space,
                    request.now,
                )?;
                let mut records = query_active_memory_records(
                    &batch.transaction,
                    &request,
                    "updated_at DESC, created_at DESC, id ASC",
                    None,
                )?;
                records.retain(|record| content_matches_filter(&record.content, &request.filter));

                let mut ranked = Vec::new();
                let mut backfill_candidates = Vec::new();
                let mut vectors_pending = 0_u64;
                for record in records.drain(..) {
                    let content_hash = durable_memory_content_hash(&record.content)?;
                    let vector = match read_memory_vector(&batch.transaction, &record, semantic)? {
                        Some(row) if row.content_hash == content_hash => {
                            Some(decode_f32_le_vector(&row.vector, semantic.dimensions)?)
                        }
                        _ => {
                            if backfill_candidates.len()
                                >= LOCAL_MEMORY_SEMANTIC_READ_BACKFILL_LIMIT
                            {
                                vectors_pending += 1;
                                None
                            } else {
                                backfill_candidates.push(SemanticMemoryBackfillCandidate {
                                    record,
                                    content_hash,
                                });
                                continue;
                            }
                        }
                    };
                    if let Some(vector) = vector {
                        let score = cosine_similarity(&query_vector, &vector)?;
                        ranked.push((score, record));
                    }
                }

                Ok((ranked, backfill_candidates, vectors_pending))
            })?;

        let mut embedding_requests = 1;
        let mut embedding_request_duration_ms = query_duration_ms;
        let mut vectors_materialized = 0_u64;
        for candidate in backfill_candidates {
            let (vector, duration_ms) = embed_memory_vector_for_record(
                &candidate.record,
                semantic,
                &embedding_space,
                embedder,
            )?;
            embedding_requests += 1;
            embedding_request_duration_ms =
                embedding_request_duration_ms.saturating_add(duration_ms);
            let materialized = self.atomic_batch(|batch| {
                upsert_memory_vector_if_current(
                    &batch.transaction,
                    &candidate.record,
                    semantic,
                    &candidate.content_hash,
                    &vector,
                    request.now,
                )
            })?;
            if materialized {
                let score = cosine_similarity(&query_vector, &vector)?;
                ranked.push((score, candidate.record));
                vectors_materialized += 1;
            } else {
                vectors_pending += 1;
            }
        }

        ranked.sort_by(|(left_score, left), (right_score, right)| {
            right_score
                .total_cmp(left_score)
                .then_with(|| left.updated_at.cmp(&right.updated_at))
                .then_with(|| left.created_at.cmp(&right.created_at))
                .then_with(|| left.id.cmp(&right.id))
        });
        let mut records = ranked
            .into_iter()
            .map(|(_score, record)| record)
            .collect::<Vec<_>>();
        if let Some(limit) = request.limit {
            records.truncate(limit);
        }
        Ok(LocalMemoryReadResult {
            records,
            embedding_requests,
            embedding_request_duration_ms: Some(embedding_request_duration_ms),
            vectors_materialized,
            vectors_pending,
        })
    }
}

pub(super) fn delete_memory_vectors_for_record(
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

#[derive(Debug, Clone)]
struct StoredMemoryVector {
    content_hash: String,
    vector: Vec<u8>,
}

#[derive(Debug)]
struct SemanticMemoryBackfillCandidate {
    record: StoredMemoryRecord,
    content_hash: String,
}

#[derive(Debug, Default)]
pub(super) struct LocalMemoryWriteEmbeddingResult {
    pub(super) embedding_requests: u64,
    pub(super) duration_ms: Option<u64>,
    pub(super) error: Option<String>,
}

fn embed_text_timed(
    embedder: &mut dyn EmbeddingProvider,
    embedding_space: &KnowledgeEmbeddingSnapshot,
    text: &str,
) -> Result<(Vec<f32>, u64)> {
    embed_text_timed_raw(embedder, embedding_space, text).map_err(|(err, _duration_ms)| {
        LocalMemoryActionError::embedding_provider_failed(format!("{}: {}", err.code, err.message))
            .into()
    })
}

fn embed_text_timed_raw(
    embedder: &mut dyn EmbeddingProvider,
    embedding_space: &KnowledgeEmbeddingSnapshot,
    text: &str,
) -> std::result::Result<(Vec<f32>, u64), (KnowledgeRuntimeFailure, u64)> {
    let started = std::time::Instant::now();
    match embedder.embed(embedding_space, text) {
        Ok(vector) => {
            let duration_ms = started.elapsed().as_millis().try_into().unwrap_or(u64::MAX);
            Ok((vector, duration_ms))
        }
        Err(err) => {
            let duration_ms = started.elapsed().as_millis().try_into().unwrap_or(u64::MAX);
            Err((err, duration_ms))
        }
    }
}

fn embed_memory_vector_for_record(
    record: &StoredMemoryRecord,
    semantic: &LocalMemorySemanticConfig,
    embedding_space: &KnowledgeEmbeddingSnapshot,
    embedder: &mut dyn EmbeddingProvider,
) -> Result<(Vec<f32>, u64)> {
    let input = semantic_memory_embedding_input(&record.content)?;
    let (vector, duration_ms) = embed_text_timed(embedder, embedding_space, &input)?;
    validate_vector_dimensions(&vector, semantic.dimensions)?;
    Ok((vector, duration_ms))
}

pub(super) fn materialize_memory_vector_best_effort(
    connection: &Connection,
    record: &StoredMemoryRecord,
    semantic: &LocalMemorySemanticConfig,
    embedder: &mut dyn EmbeddingProvider,
) -> LocalMemoryWriteEmbeddingResult {
    let embedding_space = semantic.embedding_space();
    if let Err(err) = embedder.validate_space(&embedding_space) {
        return LocalMemoryWriteEmbeddingResult {
            embedding_requests: 0,
            duration_ms: None,
            error: Some(format!("embedding_provider_unavailable: {err}")),
        };
    }
    let content_hash = match durable_memory_content_hash(&record.content) {
        Ok(hash) => hash,
        Err(err) => {
            return LocalMemoryWriteEmbeddingResult {
                embedding_requests: 0,
                duration_ms: None,
                error: Some(err.to_string()),
            };
        }
    };
    let input = match semantic_memory_embedding_input(&record.content) {
        Ok(input) => input,
        Err(err) => {
            return LocalMemoryWriteEmbeddingResult {
                embedding_requests: 0,
                duration_ms: None,
                error: Some(err.to_string()),
            };
        }
    };
    let (vector, duration_ms) = match embed_text_timed_raw(embedder, &embedding_space, &input) {
        Ok(result) => result,
        Err((err, duration_ms)) => {
            return LocalMemoryWriteEmbeddingResult {
                embedding_requests: 1,
                duration_ms: Some(duration_ms),
                error: Some(format!("{}: {}", err.code, err.message)),
            };
        }
    };
    if let Err(err) = validate_vector_dimensions(&vector, semantic.dimensions)
        .and_then(|_| upsert_memory_vector(connection, record, semantic, &content_hash, &vector))
    {
        return LocalMemoryWriteEmbeddingResult {
            embedding_requests: 1,
            duration_ms: Some(duration_ms),
            error: Some(err.to_string()),
        };
    }
    LocalMemoryWriteEmbeddingResult {
        embedding_requests: 1,
        duration_ms: Some(duration_ms),
        error: None,
    }
}

fn read_memory_vector(
    connection: &Connection,
    record: &StoredMemoryRecord,
    semantic: &LocalMemorySemanticConfig,
) -> Result<Option<StoredMemoryVector>> {
    connection
        .query_row(
            r#"
            SELECT content_hash, vector
            FROM memory_vectors
            WHERE package = ?1 AND package_version = ?2 AND space = ?3
              AND scope_hash = ?4 AND record_id = ?5
              AND embedding_provider = ?6 AND embedding_model = ?7
              AND dimensions = ?8
            "#,
            params![
                &record.package,
                &record.package_version,
                &record.space,
                &record.scope_hash,
                &record.id,
                &semantic.embedding_provider,
                &semantic.embedding_model,
                semantic.dimensions as i64,
            ],
            |row| {
                Ok(StoredMemoryVector {
                    content_hash: row.get(0)?,
                    vector: row.get(1)?,
                })
            },
        )
        .optional()
        .context("reading Memory semantic vector")
}

fn upsert_memory_vector(
    connection: &Connection,
    record: &StoredMemoryRecord,
    semantic: &LocalMemorySemanticConfig,
    content_hash: &str,
    vector: &[f32],
) -> Result<()> {
    let blob = encode_f32_le_vector(vector, semantic.dimensions)?;
    connection
        .execute(
            r#"
            INSERT INTO memory_vectors (
                record_id, package, package_version, space, record_type, scope_hash,
                embedding_provider, embedding_model, dimensions, content_hash, vector, updated_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
            ON CONFLICT(package, package_version, space, scope_hash, record_id, embedding_provider, embedding_model)
            DO UPDATE SET
                record_type = excluded.record_type,
                dimensions = excluded.dimensions,
                content_hash = excluded.content_hash,
                vector = excluded.vector,
                updated_at = excluded.updated_at
            "#,
            params![
                &record.id,
                &record.package,
                &record.package_version,
                &record.space,
                &record.record_type,
                &record.scope_hash,
                &semantic.embedding_provider,
                &semantic.embedding_model,
                semantic.dimensions as i64,
                content_hash,
                blob,
                Utc::now().to_rfc3339(),
            ],
        )
        .context("upserting Memory semantic vector")?;
    Ok(())
}

fn upsert_memory_vector_if_current(
    connection: &Connection,
    snapshot: &StoredMemoryRecord,
    semantic: &LocalMemorySemanticConfig,
    expected_content_hash: &str,
    vector: &[f32],
    now: DateTime<Utc>,
) -> Result<bool> {
    let current_content_json = connection
        .query_row(
            r#"
            SELECT content_json
            FROM memory_records
            WHERE package = ?1 AND package_version = ?2 AND space = ?3
              AND scope_hash = ?4 AND id = ?5 AND archived_at IS NULL
              AND (expires_at IS NULL OR expires_at > ?6)
            "#,
            params![
                &snapshot.package,
                &snapshot.package_version,
                &snapshot.space,
                &snapshot.scope_hash,
                &snapshot.id,
                now.to_rfc3339(),
            ],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .context("checking current Memory record before vector upsert")?;
    let Some(current_content_json) = current_content_json else {
        return Ok(false);
    };
    let current_content: Value = serde_json::from_str(&current_content_json)
        .context("parsing current Memory record content before vector upsert")?;
    if durable_memory_content_hash(&current_content)? != expected_content_hash {
        return Ok(false);
    }
    upsert_memory_vector(
        connection,
        snapshot,
        semantic,
        expected_content_hash,
        vector,
    )?;
    Ok(true)
}

pub(super) fn durable_memory_content_hash(content: &Value) -> Result<String> {
    let input = semantic_memory_embedding_input(content)?;
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    Ok(format!("sha256:{}", hex::encode(hasher.finalize())))
}

fn semantic_memory_embedding_input(content: &Value) -> Result<String> {
    serde_json::to_string(content).context("serializing durable Memory content for embedding")
}

pub(super) fn encode_f32_le_vector(vector: &[f32], dimensions: u64) -> Result<Vec<u8>> {
    validate_vector_dimensions(vector, dimensions)?;
    let mut blob = Vec::with_capacity(vector.len() * 4);
    for value in vector {
        blob.extend_from_slice(&value.to_le_bytes());
    }
    Ok(blob)
}

fn decode_f32_le_vector(blob: &[u8], dimensions: u64) -> Result<Vec<f32>> {
    let expected_len = usize::try_from(dimensions)
        .ok()
        .and_then(|dimensions| dimensions.checked_mul(4))
        .ok_or_else(|| LocalMemoryActionError::contract_violation("invalid vector dimensions"))?;
    if blob.len() != expected_len {
        return Err(LocalMemoryActionError::backend(format!(
            "stored Memory vector has {} bytes; expected {expected_len}",
            blob.len()
        ))
        .into());
    }
    let mut vector = Vec::with_capacity(expected_len / 4);
    let (chunks, remainder) = blob.as_chunks::<4>();
    if !remainder.is_empty() {
        return Err(LocalMemoryActionError::backend(format!(
            "stored Memory vector has {} bytes; expected {expected_len}",
            blob.len()
        ))
        .into());
    }
    for chunk in chunks {
        vector.push(f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]));
    }
    validate_vector_dimensions(&vector, dimensions)?;
    Ok(vector)
}

fn validate_vector_dimensions(vector: &[f32], dimensions: u64) -> Result<()> {
    if dimensions == 0 {
        return Err(LocalMemoryActionError::contract_violation(
            "Memory semantic dimensions must be greater than 0",
        )
        .into());
    }
    if vector.len() != dimensions as usize {
        return Err(LocalMemoryActionError::backend(format!(
            "embedding vector has {} dimensions; expected {dimensions}",
            vector.len()
        ))
        .into());
    }
    if vector.iter().any(|value| !value.is_finite()) {
        return Err(
            LocalMemoryActionError::backend("embedding vector contains non-finite values").into(),
        );
    }
    Ok(())
}

fn cosine_similarity(left: &[f32], right: &[f32]) -> Result<f32> {
    if left.len() != right.len() {
        return Err(LocalMemoryActionError::backend(
            "cannot compare embedding vectors with different dimensions",
        )
        .into());
    }
    let mut dot = 0.0_f32;
    let mut left_norm = 0.0_f32;
    let mut right_norm = 0.0_f32;
    for (left, right) in left.iter().zip(right.iter()) {
        dot += left * right;
        left_norm += left * left;
        right_norm += right * right;
    }
    if left_norm == 0.0 || right_norm == 0.0 {
        return Ok(0.0);
    }
    Ok(dot / (left_norm.sqrt() * right_norm.sqrt()))
}
