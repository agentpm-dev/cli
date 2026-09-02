#![allow(dead_code)]

use super::model::{
    KnowledgeDocumentSnapshot, KnowledgeEmbeddingSnapshot, KnowledgeRetrievalSnapshot,
    KnowledgeRuntimeSnapshot, RuntimeSnapshot,
};
use super::service::{
    HostServiceInvoker, ProcessServiceClient, ProcessServiceConfig, ServiceLifecycleEmitter,
};
use crate::commands::knowledge::{
    LocalKnowledgeQueryOptions, local_vector_readiness, query_local_vector_knowledge,
    resolve_existing_file,
};
use crate::harness_config::{
    HarnessEmbeddingMatchKey, HarnessImplementation, HarnessImplementationEntry,
};
use crate::harness_observability::{HarnessEventType, RunUsage};
use crate::harness_runtime::action::{ActionDispatchResult, ActionFailureCategory, SemanticAction};
use crate::manifest::{KnowledgeManifest, load_manifest_value, parse_knowledge_manifest};
use anyhow::{Context, Result, anyhow, bail};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, HashMap};
use std::path::Path;
use std::time::Instant;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KnowledgeRequestMode {
    ContextDocument,
    VectorQuery,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KnowledgeRuntimeRequest {
    pub package: String,
    pub version: String,
    pub mode: KnowledgeRequestMode,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub document: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub query: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_k: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub score_threshold: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub return_citations: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KnowledgeRuntimeResult {
    pub ok: bool,
    pub package: String,
    pub version: String,
    pub mode: KnowledgeRequestMode,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub document: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub query: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub results: Vec<KnowledgeRetrievalResult>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub citations: Vec<KnowledgeCitation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<KnowledgeRuntimeFailure>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KnowledgeRetrievalResult {
    pub rank: usize,
    pub score: f64,
    pub chunk_id: String,
    pub source_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_uri: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chunk_metadata: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_metadata: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KnowledgeCitation {
    pub chunk_id: String,
    pub source_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uri: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KnowledgeRuntimeFailure {
    pub code: String,
    pub message: String,
    #[serde(default)]
    pub retryable: bool,
}

impl KnowledgeRuntimeFailure {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            retryable: false,
        }
    }
}

pub trait KnowledgeRuntime {
    fn dispatch(&mut self, action: &SemanticAction) -> ActionDispatchResult;
}

pub struct NoopKnowledgeRuntime;

impl KnowledgeRuntime for NoopKnowledgeRuntime {
    fn dispatch(&mut self, action: &SemanticAction) -> ActionDispatchResult {
        ActionDispatchResult::failure_with_category(
            ActionFailureCategory::Resolution,
            format!(
                "Knowledge runtime is not available for `{}`",
                action.identity()
            ),
        )
    }
}

pub trait EmbeddingProvider {
    fn validate_space(&self, space: &KnowledgeEmbeddingSnapshot) -> Result<(), String>;
    fn embed(
        &mut self,
        space: &KnowledgeEmbeddingSnapshot,
        text: &str,
    ) -> std::result::Result<Vec<f32>, KnowledgeRuntimeFailure>;
}

pub struct LocalKnowledgeRuntime {
    packages: BTreeMap<String, KnowledgeRuntimeSnapshot>,
    embedding_provider: Option<Box<dyn EmbeddingProvider>>,
}

struct LocalKnowledgeRetrieval {
    result: KnowledgeRuntimeResult,
    embedding_requests: u64,
    embedding_request_duration_ms: Option<u64>,
}

impl LocalKnowledgeRetrieval {
    fn without_embedding(result: KnowledgeRuntimeResult) -> Self {
        Self {
            result,
            embedding_requests: 0,
            embedding_request_duration_ms: None,
        }
    }

    fn with_embedding(result: KnowledgeRuntimeResult, duration_ms: u64) -> Self {
        Self {
            result,
            embedding_requests: 1,
            embedding_request_duration_ms: Some(duration_ms),
        }
    }
}

impl LocalKnowledgeRuntime {
    pub fn from_runtime(
        runtime: &RuntimeSnapshot,
        embedding_provider: Option<Box<dyn EmbeddingProvider>>,
    ) -> Self {
        Self {
            packages: runtime
                .knowledge
                .iter()
                .cloned()
                .map(|package| (package.name.clone(), package))
                .collect(),
            embedding_provider,
        }
    }

    fn retrieve_context(
        &mut self,
        package: &KnowledgeRuntimeSnapshot,
        document: &str,
    ) -> KnowledgeRuntimeResult {
        let Some(declared) = package
            .documents
            .iter()
            .find(|candidate| candidate.path == document)
        else {
            return failed_result(
                package,
                KnowledgeRequestMode::ContextDocument,
                Some(document.to_string()),
                None,
                "undeclared_document",
                format!(
                    "document `{document}` is not declared by `{}`",
                    package.name
                ),
            );
        };
        let Some(root) = &package.root else {
            return failed_result(
                package,
                KnowledgeRequestMode::ContextDocument,
                Some(document.to_string()),
                None,
                "missing_package_root",
                format!("Knowledge package `{}` root is unavailable", package.name),
            );
        };
        match resolve_existing_file(root, &declared.path).and_then(|path| {
            std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))
        }) {
            Ok(content) => KnowledgeRuntimeResult {
                ok: true,
                package: package.name.clone(),
                version: package.version.clone(),
                mode: KnowledgeRequestMode::ContextDocument,
                document: Some(document.to_string()),
                query: None,
                content: Some(content),
                results: Vec::new(),
                citations: Vec::new(),
                error: None,
            },
            Err(err) => failed_result(
                package,
                KnowledgeRequestMode::ContextDocument,
                Some(document.to_string()),
                None,
                "context_load_failed",
                err.to_string(),
            ),
        }
    }

    fn retrieve_vector(
        &mut self,
        package: &KnowledgeRuntimeSnapshot,
        query: &str,
        top_k: Option<usize>,
        score_threshold: Option<f64>,
        return_citations: Option<bool>,
    ) -> LocalKnowledgeRetrieval {
        let Some(root) = &package.root else {
            return LocalKnowledgeRetrieval::without_embedding(failed_result(
                package,
                KnowledgeRequestMode::VectorQuery,
                None,
                Some(query.to_string()),
                "missing_package_root",
                format!("Knowledge package `{}` root is unavailable", package.name),
            ));
        };
        let Some(space) = &package.embedding else {
            return LocalKnowledgeRetrieval::without_embedding(failed_result(
                package,
                KnowledgeRequestMode::VectorQuery,
                None,
                Some(query.to_string()),
                "missing_embedding_metadata",
                format!(
                    "Knowledge package `{}` has no embedding metadata",
                    package.name
                ),
            ));
        };
        let Some(embedder) = self.embedding_provider.as_mut() else {
            return LocalKnowledgeRetrieval::without_embedding(failed_result(
                package,
                KnowledgeRequestMode::VectorQuery,
                None,
                Some(query.to_string()),
                "embedding_provider_unavailable",
                "no compatible EmbeddingProvider is configured for this vector Knowledge package",
            ));
        };
        if let Err(err) = embedder.validate_space(space) {
            return LocalKnowledgeRetrieval::without_embedding(failed_result(
                package,
                KnowledgeRequestMode::VectorQuery,
                None,
                Some(query.to_string()),
                "embedding_space_mismatch",
                err,
            ));
        }
        let embedding_started = Instant::now();
        let vector = match embedder.embed(space, query) {
            Ok(vector) => vector,
            Err(err) => {
                let duration_ms = elapsed_ms(embedding_started);
                return LocalKnowledgeRetrieval::with_embedding(
                    KnowledgeRuntimeResult {
                        ok: false,
                        package: package.name.clone(),
                        version: package.version.clone(),
                        mode: KnowledgeRequestMode::VectorQuery,
                        document: None,
                        query: Some(query.to_string()),
                        content: None,
                        results: Vec::new(),
                        citations: Vec::new(),
                        error: Some(err),
                    },
                    duration_ms,
                );
            }
        };
        let embedding_duration_ms = elapsed_ms(embedding_started);
        LocalKnowledgeRetrieval::with_embedding(
            match query_local_vector_knowledge(
                root,
                vector,
                LocalKnowledgeQueryOptions {
                    top_k,
                    score_threshold,
                    include_text: true,
                    include_metadata: true,
                },
            ) {
                Ok(value) => vector_result_from_query_json(package, query, return_citations, value),
                Err(err) => failed_result(
                    package,
                    KnowledgeRequestMode::VectorQuery,
                    None,
                    Some(query.to_string()),
                    "local_vector_query_failed",
                    err.to_string(),
                ),
            },
            embedding_duration_ms,
        )
    }
}

impl KnowledgeRuntime for LocalKnowledgeRuntime {
    fn dispatch(&mut self, action: &SemanticAction) -> ActionDispatchResult {
        let SemanticAction::KnowledgeRequest {
            package,
            mode,
            document,
            query,
            top_k,
            score_threshold,
            return_citations,
        } = action
        else {
            return ActionDispatchResult::failure("KnowledgeRuntime received non-Knowledge action");
        };
        let Some(snapshot) = self.packages.get(package).cloned() else {
            return ActionDispatchResult::failure_with_category(
                ActionFailureCategory::Resolution,
                format!(
                    "Knowledge package `{package}` is not available in the current EffectivePhase"
                ),
            );
        };
        if snapshot.state != "available" {
            return ActionDispatchResult::success(json!(failed_result(
                &snapshot,
                mode.clone().unwrap_or(KnowledgeRequestMode::VectorQuery),
                document.clone(),
                query.clone(),
                "knowledge_unavailable",
                snapshot.readiness_reason.clone().unwrap_or_else(|| format!(
                    "Knowledge package `{}` is unavailable",
                    snapshot.name
                )),
            )));
        }
        let request_mode = match (mode, document, query) {
            (Some(mode), _, _) => mode.clone(),
            (None, Some(_), _) => KnowledgeRequestMode::ContextDocument,
            (None, None, Some(_)) => KnowledgeRequestMode::VectorQuery,
            (None, None, None) => {
                return ActionDispatchResult::failure_with_category(
                    ActionFailureCategory::Schema,
                    "Knowledge request must include document or query",
                );
            }
        };
        let retrieval = match request_mode {
            KnowledgeRequestMode::ContextDocument => {
                LocalKnowledgeRetrieval::without_embedding(match document {
                    Some(document) => self.retrieve_context(&snapshot, document),
                    None => failed_result(
                        &snapshot,
                        KnowledgeRequestMode::ContextDocument,
                        None,
                        None,
                        "missing_document",
                        "context-document Knowledge request requires document",
                    ),
                })
            }
            KnowledgeRequestMode::VectorQuery => match query {
                Some(query) => self.retrieve_vector(
                    &snapshot,
                    query,
                    *top_k,
                    *score_threshold,
                    *return_citations,
                ),
                None => LocalKnowledgeRetrieval::without_embedding(failed_result(
                    &snapshot,
                    KnowledgeRequestMode::VectorQuery,
                    None,
                    None,
                    "missing_query",
                    "vector-query Knowledge request requires query",
                )),
            },
        };
        let usage = RunUsage {
            embedding_requests: retrieval.embedding_requests,
            ..Default::default()
        };
        let mut result = ActionDispatchResult::success(json!(retrieval.result)).with_usage(usage);
        if let Some(duration_ms) = retrieval.embedding_request_duration_ms {
            result = result.with_embedding_request_duration_ms(duration_ms);
        }
        result
    }
}

fn elapsed_ms(started: Instant) -> u64 {
    started.elapsed().as_millis().try_into().unwrap_or(u64::MAX)
}

pub struct ServiceEmbeddingProvider {
    registry_id: String,
    capabilities: EmbeddingCapabilities,
    runtime: ServiceRuntime,
    lifecycle_events: Option<ServiceLifecycleEmitter>,
}

pub struct RoutingEmbeddingProvider {
    routes: BTreeMap<String, String>,
    providers: BTreeMap<String, Box<dyn EmbeddingProvider>>,
}

impl RoutingEmbeddingProvider {
    pub fn new(
        routes: BTreeMap<String, String>,
        providers: BTreeMap<String, Box<dyn EmbeddingProvider>>,
    ) -> Self {
        Self { routes, providers }
    }
}

impl EmbeddingProvider for RoutingEmbeddingProvider {
    fn validate_space(&self, space: &KnowledgeEmbeddingSnapshot) -> Result<(), String> {
        let key = embedding_space_key(space);
        let Some(provider_id) = self.routes.get(&key) else {
            return Err(format!(
                "no EmbeddingProvider route for {}/{}/dimensions={}/normalized={}",
                space.provider, space.model, space.dimensions, space.normalized
            ));
        };
        let Some(provider) = self.providers.get(provider_id) else {
            return Err(format!("EmbeddingProvider `{provider_id}` is not active"));
        };
        provider.validate_space(space)
    }

    fn embed(
        &mut self,
        space: &KnowledgeEmbeddingSnapshot,
        text: &str,
    ) -> std::result::Result<Vec<f32>, KnowledgeRuntimeFailure> {
        let key = embedding_space_key(space);
        let Some(provider_id) = self.routes.get(&key).cloned() else {
            return Err(KnowledgeRuntimeFailure::new(
                "embedding_provider_unavailable",
                format!(
                    "no EmbeddingProvider route for {}/{}/dimensions={}/normalized={}",
                    space.provider, space.model, space.dimensions, space.normalized
                ),
            ));
        };
        let Some(provider) = self.providers.get_mut(&provider_id) else {
            return Err(KnowledgeRuntimeFailure::new(
                "embedding_provider_unavailable",
                format!("EmbeddingProvider `{provider_id}` is not active"),
            ));
        };
        provider.embed(space, text)
    }
}

pub(crate) enum ServiceRuntime {
    Process(Box<ProcessServiceClient>),
    Host {
        invoker: Box<dyn HostServiceInvoker>,
        request_timeout_ms: u64,
    },
}

impl ServiceRuntime {
    pub(crate) fn process(
        service: &str,
        registry_id: &str,
        entry: &HarnessImplementationEntry,
        workspace_root: &Path,
        initialize_payload: Map<String, Value>,
        lifecycle_events: Option<ServiceLifecycleEmitter>,
    ) -> Result<Self> {
        let client = ProcessServiceClient::start(ProcessServiceConfig {
            service: service.into(),
            registry_id: registry_id.to_string(),
            initialize_payload,
            implementation: entry.implementation.clone(),
            workspace_root: workspace_root.to_path_buf(),
            lifecycle_events,
        })?;
        Ok(Self::Process(Box::new(client)))
    }

    pub(crate) fn host(invoker: Box<dyn HostServiceInvoker>, request_timeout_ms: u64) -> Self {
        Self::Host {
            invoker,
            request_timeout_ms,
        }
    }

    pub(crate) fn initialization_result(&self) -> Option<&Value> {
        match self {
            Self::Process(client) => Some(client.initialization_result()),
            Self::Host { .. } => None,
        }
    }
}

#[derive(Debug, Clone, Default)]
struct EmbeddingCapabilities {
    spaces: Vec<KnowledgeEmbeddingSnapshot>,
}

impl ServiceEmbeddingProvider {
    pub fn process(
        workspace_root: &Path,
        registry_id: &str,
        entry: &HarnessImplementationEntry,
        lifecycle_events: Option<ServiceLifecycleEmitter>,
    ) -> Result<Self> {
        let client = ProcessServiceClient::start(ProcessServiceConfig {
            service: "embedding".into(),
            registry_id: registry_id.to_string(),
            initialize_payload: Map::new(),
            implementation: entry.implementation.clone(),
            workspace_root: workspace_root.to_path_buf(),
            lifecycle_events,
        })?;
        let capabilities = embedding_capabilities_from_value(client.initialization_result())
            .with_context(|| {
                format!("validating EmbeddingProvider `{registry_id}` capabilities")
            })?;
        Ok(Self {
            registry_id: registry_id.to_string(),
            capabilities,
            runtime: ServiceRuntime::Process(Box::new(client)),
            lifecycle_events: None,
        })
    }

    pub fn host(
        registry_id: &str,
        entry: &HarnessImplementationEntry,
        invoker: Box<dyn HostServiceInvoker>,
        lifecycle_events: Option<ServiceLifecycleEmitter>,
    ) -> Result<Self> {
        let capabilities = match invoker.host_service_capabilities("embedding", registry_id) {
            Some(capabilities) => capabilities,
            None => {
                emit_embedding_host_service_failure(
                    lifecycle_events.as_ref(),
                    registry_id,
                    format!("host EmbeddingProvider `{registry_id}` is not registered"),
                );
                bail!("host EmbeddingProvider `{registry_id}` is not registered");
            }
        };
        let capabilities =
            match embedding_capabilities_from_value(&capabilities).with_context(|| {
                format!("validating host EmbeddingProvider `{registry_id}` capabilities")
            }) {
                Ok(capabilities) => capabilities,
                Err(err) => {
                    emit_embedding_host_service_failure(
                        lifecycle_events.as_ref(),
                        registry_id,
                        err.to_string(),
                    );
                    return Err(err);
                }
            };
        let HarnessImplementation::Host { request_timeout_ms } = entry.implementation else {
            bail!("host EmbeddingProvider requires host implementation");
        };
        Ok(Self {
            registry_id: registry_id.to_string(),
            capabilities,
            runtime: ServiceRuntime::Host {
                invoker,
                request_timeout_ms,
            },
            lifecycle_events,
        })
    }
}

impl EmbeddingProvider for ServiceEmbeddingProvider {
    fn validate_space(&self, space: &KnowledgeEmbeddingSnapshot) -> Result<(), String> {
        if self.capabilities.spaces.iter().any(|candidate| {
            candidate.provider == space.provider
                && candidate.model == space.model
                && candidate.dimensions == space.dimensions
                && candidate.normalized == space.normalized
        }) {
            Ok(())
        } else {
            Err(format!(
                "EmbeddingProvider `{}` does not advertise {}/{}/dimensions={}/normalized={}",
                self.registry_id, space.provider, space.model, space.dimensions, space.normalized
            ))
        }
    }

    fn embed(
        &mut self,
        space: &KnowledgeEmbeddingSnapshot,
        text: &str,
    ) -> std::result::Result<Vec<f32>, KnowledgeRuntimeFailure> {
        let payload = json!({
            "provider": space.provider,
            "model": space.model,
            "dimensions": space.dimensions,
            "normalized": space.normalized,
            "text": text,
        });
        let registry_id = self.registry_id.clone();
        let lifecycle_events = self.lifecycle_events.clone();
        let value = match &mut self.runtime {
            ServiceRuntime::Process(client) => client.request("embed", payload),
            ServiceRuntime::Host {
                invoker,
                request_timeout_ms,
            } => {
                let emits_lifecycle_events = invoker.emits_lifecycle_events();
                let result = invoker.invoke_host_service(
                    "embedding",
                    &registry_id,
                    "embed",
                    payload,
                    *request_timeout_ms,
                );
                if let Err(err) = &result
                    && !emits_lifecycle_events
                {
                    emit_embedding_host_service_failure(
                        lifecycle_events.as_ref(),
                        &registry_id,
                        err.to_string(),
                    );
                }
                result
            }
        }
        .map_err(|err| {
            KnowledgeRuntimeFailure::new("embedding_provider_failed", err.to_string())
        })?;
        parse_embedding_response(value, space)
            .map_err(|err| KnowledgeRuntimeFailure::new("malformed_embedding_response", err))
    }
}

fn emit_embedding_host_service_failure(
    lifecycle_events: Option<&ServiceLifecycleEmitter>,
    registry_id: &str,
    message: impl Into<String>,
) {
    let Some(events) = lifecycle_events else {
        return;
    };
    let message = message.into();
    events.emit(
        HarnessEventType::ServiceUnhealthy,
        "embedding",
        registry_id,
        "unhealthy",
        format!("Host EmbeddingProvider request failed: {message}"),
    );
    events.emit(
        HarnessEventType::ServiceFailed,
        "embedding",
        registry_id,
        "failed",
        format!("Host EmbeddingProvider request failed: {message}"),
    );
}

pub struct CustomKnowledgeRuntime {
    packages: BTreeMap<String, KnowledgeRuntimeSnapshot>,
    runtimes: HashMap<String, ServiceRuntime>,
    package_routes: BTreeMap<String, String>,
}

impl CustomKnowledgeRuntime {
    pub(crate) fn new(
        packages: Vec<KnowledgeRuntimeSnapshot>,
        runtimes: HashMap<String, ServiceRuntime>,
        package_routes: BTreeMap<String, String>,
    ) -> Self {
        Self {
            packages: packages
                .into_iter()
                .map(|package| (package.name.clone(), package))
                .collect(),
            runtimes,
            package_routes,
        }
    }
}

impl KnowledgeRuntime for CustomKnowledgeRuntime {
    fn dispatch(&mut self, action: &SemanticAction) -> ActionDispatchResult {
        let SemanticAction::KnowledgeRequest { package, .. } = action else {
            return ActionDispatchResult::failure("KnowledgeRuntime received non-Knowledge action");
        };
        let Some(snapshot) = self.packages.get(package).cloned() else {
            return ActionDispatchResult::failure_with_category(
                ActionFailureCategory::Resolution,
                format!(
                    "Knowledge package `{package}` is not available in the current EffectivePhase"
                ),
            );
        };
        let Some(registry_id) = self.package_routes.get(package).cloned() else {
            return ActionDispatchResult::failure_with_category(
                ActionFailureCategory::Resolution,
                format!("Knowledge package `{package}` has no custom runtime route"),
            );
        };
        let request = match knowledge_request_from_action(action, &snapshot) {
            Ok(request) => request,
            Err(err) => {
                return ActionDispatchResult::failure_with_category(
                    ActionFailureCategory::Schema,
                    err,
                );
            }
        };
        let payload = json!({ "request": request });
        let started = Instant::now();
        let result = match self.runtimes.get_mut(&registry_id) {
            Some(ServiceRuntime::Process(client)) => client.request("retrieve", payload),
            Some(ServiceRuntime::Host {
                invoker,
                request_timeout_ms,
            }) => invoker.invoke_host_service(
                "knowledge",
                &registry_id,
                "retrieve",
                payload,
                *request_timeout_ms,
            ),
            None => Err(anyhow!("KnowledgeRuntime `{registry_id}` is not active")),
        };
        match result {
            Ok(value) => {
                let mut output = value;
                if let Value::Object(map) = &mut output {
                    map.entry("duration_ms")
                        .or_insert_with(|| json!(started.elapsed().as_millis() as u64));
                }
                ActionDispatchResult::success(output)
            }
            Err(err) => ActionDispatchResult::success(json!(failed_result(
                &snapshot,
                request.mode,
                request.document,
                request.query,
                "knowledge_runtime_failed",
                err.to_string(),
            ))),
        }
    }
}

pub struct CompositeKnowledgeRuntime {
    local: LocalKnowledgeRuntime,
    custom: Option<CustomKnowledgeRuntime>,
    custom_routes: BTreeMap<String, String>,
}

impl CompositeKnowledgeRuntime {
    pub fn new(
        local: LocalKnowledgeRuntime,
        custom: Option<CustomKnowledgeRuntime>,
        custom_routes: BTreeMap<String, String>,
    ) -> Self {
        Self {
            local,
            custom,
            custom_routes,
        }
    }
}

impl KnowledgeRuntime for CompositeKnowledgeRuntime {
    fn dispatch(&mut self, action: &SemanticAction) -> ActionDispatchResult {
        if self.custom_routes.contains_key(&action.identity()) {
            if let Some(custom) = self.custom.as_mut() {
                custom.dispatch(action)
            } else {
                ActionDispatchResult::failure_with_category(
                    ActionFailureCategory::Resolution,
                    format!(
                        "custom KnowledgeRuntime route for `{}` is configured but not active",
                        action.identity()
                    ),
                )
            }
        } else {
            self.local.dispatch(action)
        }
    }
}

pub fn knowledge_request_from_action(
    action: &SemanticAction,
    package: &KnowledgeRuntimeSnapshot,
) -> Result<KnowledgeRuntimeRequest, String> {
    let SemanticAction::KnowledgeRequest {
        mode,
        document,
        query,
        top_k,
        score_threshold,
        return_citations,
        ..
    } = action
    else {
        return Err("not a Knowledge request".into());
    };
    let request_mode = mode.clone().unwrap_or_else(|| {
        if document.is_some() {
            KnowledgeRequestMode::ContextDocument
        } else {
            KnowledgeRequestMode::VectorQuery
        }
    });
    Ok(KnowledgeRuntimeRequest {
        package: package.name.clone(),
        version: package.version.clone(),
        mode: request_mode,
        document: document.clone(),
        query: query.clone(),
        top_k: *top_k,
        score_threshold: *score_threshold,
        return_citations: *return_citations,
    })
}

pub fn knowledge_snapshot_from_manifest(
    package_root: &Path,
    manifest: &KnowledgeManifest,
    source: String,
    runtime: String,
    state: String,
    readiness_reason: Option<String>,
) -> KnowledgeRuntimeSnapshot {
    KnowledgeRuntimeSnapshot {
        name: manifest.name.clone(),
        version: manifest.version.clone(),
        mode: manifest.knowledge.mode.clone(),
        description: manifest
            .description
            .clone()
            .unwrap_or_else(|| "AgentPM Knowledge surface.".into()),
        root: Some(package_root.to_path_buf()),
        source,
        state,
        runtime,
        readiness_reason,
        documents: manifest
            .knowledge
            .documents
            .iter()
            .map(|document| KnowledgeDocumentSnapshot {
                path: document.path.clone(),
                content_type: document.content_type.clone(),
                role: document.role.clone(),
                description: document.description.clone(),
                bytes: document.bytes,
                sha256: document.sha256.clone(),
            })
            .collect(),
        embedding: manifest.knowledge.embedding.as_ref().map(|embedding| {
            KnowledgeEmbeddingSnapshot {
                id: embedding.id.clone(),
                provider: embedding.provider.clone(),
                model: embedding.model.clone(),
                dimensions: embedding.dimensions,
                metric: embedding.metric.clone(),
                normalized: embedding.normalized,
            }
        }),
        retrieval: manifest.knowledge.retrieval.as_ref().map(|retrieval| {
            KnowledgeRetrievalSnapshot {
                strategy: retrieval.strategy.clone(),
                default_top_k: retrieval.default_top_k,
                default_score_threshold: retrieval.default_score_threshold,
                return_citations: retrieval.return_citations,
            }
        }),
    }
}

pub fn load_knowledge_snapshot(
    package_root: &Path,
    source: String,
    runtime: String,
    state: String,
    readiness_reason: Option<String>,
) -> Result<KnowledgeRuntimeSnapshot> {
    let manifest_path = package_root.join("agent.json");
    let (value, _) = load_manifest_value(&manifest_path)?;
    let manifest = parse_knowledge_manifest(&value)?;
    Ok(knowledge_snapshot_from_manifest(
        package_root,
        &manifest,
        source,
        runtime,
        state,
        readiness_reason,
    ))
}

pub fn local_vector_artifact_reason(package_root: &Path) -> Option<String> {
    local_vector_readiness(package_root)
        .err()
        .map(|err| err.to_string())
}

pub fn embedding_key_matches(
    space: &KnowledgeEmbeddingSnapshot,
    key: &HarnessEmbeddingMatchKey,
) -> bool {
    space.provider == key.provider
        && space.model == key.model
        && space.dimensions == key.dimensions
        && space.normalized == key.normalized
}

pub fn embedding_space_key(space: &KnowledgeEmbeddingSnapshot) -> String {
    format!(
        "{}\n{}\n{}\n{}",
        space.provider, space.model, space.dimensions, space.normalized
    )
}

fn failed_result(
    package: &KnowledgeRuntimeSnapshot,
    mode: KnowledgeRequestMode,
    document: Option<String>,
    query: Option<String>,
    code: impl Into<String>,
    message: impl Into<String>,
) -> KnowledgeRuntimeResult {
    KnowledgeRuntimeResult {
        ok: false,
        package: package.name.clone(),
        version: package.version.clone(),
        mode,
        document,
        query,
        content: None,
        results: Vec::new(),
        citations: Vec::new(),
        error: Some(KnowledgeRuntimeFailure::new(code, message)),
    }
}

fn vector_result_from_query_json(
    package: &KnowledgeRuntimeSnapshot,
    query: &str,
    return_citations: Option<bool>,
    value: Value,
) -> KnowledgeRuntimeResult {
    let results = value
        .get("results")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .enumerate()
        .filter_map(|(idx, value)| {
            Some(KnowledgeRetrievalResult {
                rank: idx + 1,
                score: value.get("score")?.as_f64()?,
                chunk_id: value.get("chunk_id")?.as_str()?.to_string(),
                source_id: value.get("source_id")?.as_str()?.to_string(),
                source_title: value
                    .get("source_title")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                source_uri: value
                    .get("source_uri")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                text: value
                    .get("text")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                chunk_metadata: value
                    .get("chunk_metadata")
                    .cloned()
                    .filter(|v| !v.is_null()),
                source_metadata: value
                    .get("source_metadata")
                    .cloned()
                    .filter(|v| !v.is_null()),
            })
        })
        .collect::<Vec<_>>();
    let include_citations = return_citations
        .or_else(|| {
            package
                .retrieval
                .as_ref()
                .and_then(|retrieval| retrieval.return_citations)
        })
        .unwrap_or(true);
    let citations = if include_citations {
        results
            .iter()
            .map(|result| KnowledgeCitation {
                chunk_id: result.chunk_id.clone(),
                source_id: result.source_id.clone(),
                title: result.source_title.clone(),
                uri: result.source_uri.clone(),
            })
            .collect()
    } else {
        Vec::new()
    };
    KnowledgeRuntimeResult {
        ok: true,
        package: package.name.clone(),
        version: package.version.clone(),
        mode: KnowledgeRequestMode::VectorQuery,
        document: None,
        query: Some(query.to_string()),
        content: None,
        results,
        citations,
        error: None,
    }
}

fn embedding_capabilities_from_value(value: &Value) -> Result<EmbeddingCapabilities> {
    if value
        .get("ready")
        .and_then(Value::as_bool)
        .is_some_and(|ready| !ready)
    {
        bail!("EmbeddingProvider reported ready=false");
    }
    let spaces_value = value
        .get("embedding_spaces")
        .or_else(|| value.get("spaces"))
        .or_else(|| {
            value
                .get("capabilities")
                .and_then(|v| v.get("embedding_spaces"))
        })
        .ok_or_else(|| anyhow!("EmbeddingProvider must advertise embedding_spaces"))?;
    let spaces = spaces_value
        .as_array()
        .ok_or_else(|| anyhow!("EmbeddingProvider embedding_spaces must be an array"))?
        .iter()
        .map(|space| {
            let provider = required_string(space, "provider")?;
            let model = required_string(space, "model")?;
            let dimensions = space
                .get("dimensions")
                .and_then(Value::as_u64)
                .ok_or_else(|| anyhow!("embedding space requires dimensions"))?;
            let normalized = space
                .get("normalized")
                .and_then(Value::as_bool)
                .ok_or_else(|| anyhow!("embedding space requires normalized"))?;
            Ok(KnowledgeEmbeddingSnapshot {
                id: space
                    .get("id")
                    .and_then(Value::as_str)
                    .unwrap_or("default")
                    .to_string(),
                provider,
                model,
                dimensions,
                metric: space
                    .get("metric")
                    .and_then(Value::as_str)
                    .unwrap_or("cosine")
                    .to_string(),
                normalized,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    if spaces.is_empty() {
        bail!("EmbeddingProvider advertised no embedding spaces");
    }
    Ok(EmbeddingCapabilities { spaces })
}

pub fn validate_embedding_provider_capabilities(value: &Value, registry_id: &str) -> Result<()> {
    embedding_capabilities_from_value(value)
        .map(|_| ())
        .with_context(|| format!("validating EmbeddingProvider `{registry_id}` capabilities"))
}

pub fn validate_knowledge_runtime_capabilities(
    value: &Value,
    registry_id: &str,
    packages: &[KnowledgeRuntimeSnapshot],
) -> Result<()> {
    if value
        .get("ready")
        .and_then(Value::as_bool)
        .is_some_and(|ready| !ready)
    {
        bail!("KnowledgeRuntime `{registry_id}` reported ready=false");
    }
    let advertised_modes = required_string_array_capability(value, "modes")
        .with_context(|| format!("validating KnowledgeRuntime `{registry_id}` modes"))?;
    let _advertised_features = required_string_array_capability(value, "features")
        .with_context(|| format!("validating KnowledgeRuntime `{registry_id}` features"))?;
    if let Some(advertised_registry_id) = value.get("registry_id").and_then(Value::as_str)
        && advertised_registry_id != registry_id
    {
        bail!(
            "KnowledgeRuntime initialized as `{advertised_registry_id}`, expected `{registry_id}`"
        );
    }
    let Some(advertised_packages) = value
        .get("packages")
        .or_else(|| value.get("attestations"))
        .or_else(|| value.get("capabilities").and_then(|v| v.get("packages")))
    else {
        if !packages.is_empty() {
            bail!("KnowledgeRuntime `{registry_id}` must attest routed Knowledge packages");
        }
        return Ok(());
    };
    let advertised_packages = advertised_packages
        .as_array()
        .ok_or_else(|| anyhow!("KnowledgeRuntime packages capability must be an array"))?;
    for package in packages {
        let required_mode = knowledge_runtime_request_mode_for_package(package);
        if !advertised_modes
            .iter()
            .any(|mode| mode == required_mode || mode == package.mode.as_str())
        {
            bail!(
                "KnowledgeRuntime `{registry_id}` does not advertise mode `{required_mode}` for {}@{}",
                package.name,
                package.version
            );
        }
        let expected_corpus = package_corpus_hash(package)?;
        let matched = advertised_packages.iter().any(|value| {
            let Some(obj) = value.as_object() else {
                return false;
            };
            let name_matches = obj.get("package").and_then(Value::as_str)
                == Some(package.name.as_str())
                || obj.get("name").and_then(Value::as_str) == Some(package.name.as_str());
            let version_matches =
                obj.get("version").and_then(Value::as_str) == Some(package.version.as_str());
            let ready_matches = obj.get("ready").and_then(Value::as_bool) == Some(true);
            let corpus_matches = expected_corpus.as_ref().is_none_or(|expected| {
                obj.get("corpus").and_then(Value::as_str) == Some(expected.as_str())
                    || obj.get("corpus_hash").and_then(Value::as_str) == Some(expected.as_str())
            });
            name_matches && version_matches && ready_matches && corpus_matches
        });
        if !matched {
            match expected_corpus {
                Some(corpus) => bail!(
                    "KnowledgeRuntime `{registry_id}` does not attest {}@{} corpus {} as ready",
                    package.name,
                    package.version,
                    corpus
                ),
                None => bail!(
                    "KnowledgeRuntime `{registry_id}` does not attest {}@{} as ready",
                    package.name,
                    package.version
                ),
            }
        }
    }
    Ok(())
}

fn required_string_array_capability(value: &Value, key: &str) -> Result<Vec<String>> {
    let values = value
        .get(key)
        .or_else(|| value.get("capabilities").and_then(|v| v.get(key)))
        .ok_or_else(|| anyhow!("KnowledgeRuntime must advertise {key}"))?;
    let values = values
        .as_array()
        .ok_or_else(|| anyhow!("KnowledgeRuntime {key} capability must be an array"))?;
    values
        .iter()
        .map(|value| {
            value
                .as_str()
                .map(str::to_string)
                .ok_or_else(|| anyhow!("KnowledgeRuntime {key} entries must be strings"))
        })
        .collect::<Result<Vec<_>>>()
}

fn knowledge_runtime_request_mode_for_package(package: &KnowledgeRuntimeSnapshot) -> &'static str {
    match package.mode.as_str() {
        "context" => "context_document",
        "vector" => "vector_query",
        _ => "unknown",
    }
}

fn package_corpus_hash(package: &KnowledgeRuntimeSnapshot) -> Result<Option<String>> {
    let Some(root) = &package.root else {
        return Ok(None);
    };
    let manifest_path = root.join("agent.json");
    let (value, _) = load_manifest_value(&manifest_path)?;
    let manifest = parse_knowledge_manifest(&value)?;
    Ok(manifest
        .knowledge
        .corpus
        .and_then(|corpus| corpus.content_hash))
}

fn parse_embedding_response(
    value: Value,
    space: &KnowledgeEmbeddingSnapshot,
) -> Result<Vec<f32>, String> {
    let values = match &value {
        Value::Array(values) => values,
        Value::Object(map) => map
            .get("vector")
            .or_else(|| map.get("values"))
            .and_then(Value::as_array)
            .ok_or_else(|| "embedding response must contain vector or values array".to_string())?,
        _ => return Err("embedding response must be an array or object".into()),
    };
    let mut vector = Vec::with_capacity(values.len());
    for (idx, value) in values.iter().enumerate() {
        let number = value
            .as_f64()
            .ok_or_else(|| format!("embedding vector entry {idx} is not a number"))?;
        if !number.is_finite() {
            return Err(format!("embedding vector entry {idx} is not finite"));
        }
        vector.push(number as f32);
    }
    if vector.len() != space.dimensions as usize {
        return Err(format!(
            "embedding vector length {} does not match requested dimensions {}",
            vector.len(),
            space.dimensions
        ));
    }
    if space.normalized {
        let norm = vector
            .iter()
            .map(|value| (*value as f64) * (*value as f64))
            .sum::<f64>()
            .sqrt();
        if (norm - 1.0).abs() > 0.01 {
            return Err(format!(
                "embedding vector norm {:.6} is incompatible with normalized=true",
                norm
            ));
        }
    }
    Ok(vector)
}

fn required_string(value: &Value, key: &str) -> Result<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(str::to_string)
        .ok_or_else(|| anyhow!("{key} is required"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::knowledge::{KnowledgeBuildMode, execute_knowledge_build};
    use crate::harness_runtime::ServiceLifecycleEvents;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    struct StaticEmbeddingProvider {
        space: KnowledgeEmbeddingSnapshot,
        vector: Vec<f32>,
    }

    impl EmbeddingProvider for StaticEmbeddingProvider {
        fn validate_space(&self, space: &KnowledgeEmbeddingSnapshot) -> Result<(), String> {
            if &self.space == space {
                Ok(())
            } else {
                Err("space mismatch".into())
            }
        }

        fn embed(
            &mut self,
            _space: &KnowledgeEmbeddingSnapshot,
            _text: &str,
        ) -> std::result::Result<Vec<f32>, KnowledgeRuntimeFailure> {
            Ok(self.vector.clone())
        }
    }

    struct FailingHostEmbeddingInvoker {
        capabilities: Value,
    }

    impl HostServiceInvoker for FailingHostEmbeddingInvoker {
        fn invoke_host_service(
            &mut self,
            _role: &str,
            _registry_id: &str,
            _method: &str,
            _payload: Value,
            _timeout_ms: u64,
        ) -> Result<Value> {
            Err(anyhow!("host embedding failed"))
        }

        fn host_service_capabilities(&self, _role: &str, _registry_id: &str) -> Option<Value> {
            Some(self.capabilities.clone())
        }
    }

    #[test]
    fn context_document_loads_only_declared_document() {
        let root = temp_dir("context");
        std::fs::create_dir_all(root.join("knowledge/docs")).unwrap();
        std::fs::write(
            root.join("knowledge/docs/guide.md"),
            "# Guide\nUse the harness.",
        )
        .unwrap();
        let manifest = json!({
            "kind": "knowledge",
            "name": "guide",
            "version": "0.1.0",
            "description": "Guide docs.",
            "knowledge": {
                "mode": "context",
                "documents": [
                    {
                        "path": "knowledge/docs/guide.md",
                        "content_type": "text/markdown",
                        "role": "guide"
                    }
                ]
            }
        });
        std::fs::write(
            root.join("agent.json"),
            serde_json::to_vec_pretty(&manifest).unwrap(),
        )
        .unwrap();
        execute_knowledge_build(&root.join("agent.json"), KnowledgeBuildMode::Write).unwrap();
        let snapshot = load_knowledge_snapshot(
            &root,
            "agent_binding".into(),
            "local".into(),
            "available".into(),
            None,
        )
        .unwrap();
        let mut runtime =
            LocalKnowledgeRuntime::from_runtime(&runtime_with_knowledge(snapshot), None);

        let result = runtime.dispatch(&SemanticAction::KnowledgeRequest {
            package: "guide".into(),
            mode: Some(KnowledgeRequestMode::ContextDocument),
            document: Some("knowledge/docs/guide.md".into()),
            query: None,
            top_k: None,
            score_threshold: None,
            return_citations: None,
        });

        assert!(result.ok);
        assert_eq!(result.output["ok"], json!(true));
        assert!(
            result.output["content"]
                .as_str()
                .unwrap()
                .contains("Use the harness.")
        );

        let denied = runtime.dispatch(&SemanticAction::KnowledgeRequest {
            package: "guide".into(),
            mode: Some(KnowledgeRequestMode::ContextDocument),
            document: Some("knowledge/docs/secret.md".into()),
            query: None,
            top_k: None,
            score_threshold: None,
            return_citations: None,
        });
        assert!(denied.ok);
        assert_eq!(denied.output["ok"], json!(false));
        assert_eq!(denied.output["error"]["code"], json!("undeclared_document"));

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn composite_runtime_never_falls_back_to_local_for_mapped_package() {
        let root = temp_dir("custom-no-fallback");
        std::fs::create_dir_all(root.join("knowledge/docs")).unwrap();
        std::fs::write(
            root.join("knowledge/docs/guide.md"),
            "# Guide\nLocal content must not be used.",
        )
        .unwrap();
        let manifest = json!({
            "kind": "knowledge",
            "name": "guide",
            "version": "0.1.0",
            "description": "Guide docs.",
            "knowledge": {
                "mode": "context",
                "documents": [
                    {
                        "path": "knowledge/docs/guide.md",
                        "content_type": "text/markdown"
                    }
                ]
            }
        });
        std::fs::write(
            root.join("agent.json"),
            serde_json::to_vec_pretty(&manifest).unwrap(),
        )
        .unwrap();
        execute_knowledge_build(&root.join("agent.json"), KnowledgeBuildMode::Write).unwrap();
        let snapshot = load_knowledge_snapshot(
            &root,
            "agent_binding".into(),
            "custom-kb".into(),
            "available".into(),
            None,
        )
        .unwrap();
        let runtime_snapshot = runtime_with_knowledge(snapshot);
        let local = LocalKnowledgeRuntime::from_runtime(&runtime_snapshot, None);
        let mut runtime = CompositeKnowledgeRuntime::new(
            local,
            None,
            std::collections::BTreeMap::from([("guide".into(), "custom-kb".into())]),
        );

        let result = runtime.dispatch(&SemanticAction::KnowledgeRequest {
            package: "guide".into(),
            mode: Some(KnowledgeRequestMode::ContextDocument),
            document: Some("knowledge/docs/guide.md".into()),
            query: None,
            top_k: None,
            score_threshold: None,
            return_citations: None,
        });

        assert!(!result.ok);
        assert_eq!(
            result.failure_category,
            Some(ActionFailureCategory::Resolution)
        );
        assert!(
            result
                .error
                .as_deref()
                .unwrap_or_default()
                .contains("custom KnowledgeRuntime route for `guide` is configured but not active")
        );
        assert_eq!(result.output, Value::Null);

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn local_vector_query_uses_embedding_provider_and_returns_citations() {
        let root = temp_dir("vector");
        write_vector_package(&root);
        execute_knowledge_build(&root.join("agent.json"), KnowledgeBuildMode::Write).unwrap();
        let snapshot = load_knowledge_snapshot(
            &root,
            "agent_binding".into(),
            "local".into(),
            "available".into(),
            None,
        )
        .unwrap();
        let space = snapshot.embedding.clone().unwrap();
        let runtime_snapshot = runtime_with_knowledge(snapshot);
        let mut runtime = LocalKnowledgeRuntime::from_runtime(
            &runtime_snapshot,
            Some(Box::new(StaticEmbeddingProvider {
                space,
                vector: vec![1.0, 0.0, 0.0],
            })),
        );

        let result = runtime.dispatch(&SemanticAction::KnowledgeRequest {
            package: "vector-docs".into(),
            mode: Some(KnowledgeRequestMode::VectorQuery),
            document: None,
            query: Some("alpha".into()),
            top_k: Some(1),
            score_threshold: None,
            return_citations: Some(true),
        });

        assert!(result.ok);
        assert_eq!(result.output["ok"], json!(true));
        assert_eq!(
            result.output["results"][0]["chunk_id"],
            json!("chunk_alpha")
        );
        assert_eq!(
            result.output["citations"][0]["source_id"],
            json!("src_alpha")
        );
        assert_eq!(result.usage.embedding_requests, 1);
        assert!(result.embedding_request_duration_ms.is_some());

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn host_embedding_provider_failure_emits_service_health_events() {
        let space = KnowledgeEmbeddingSnapshot {
            id: "default".into(),
            provider: "bring-your-own".into(),
            model: "test-embedding".into(),
            dimensions: 3,
            metric: "cosine".into(),
            normalized: true,
        };
        let entry = HarnessImplementationEntry {
            implementation: HarnessImplementation::Host {
                request_timeout_ms: 100,
            },
        };
        let mut lifecycle_events = ServiceLifecycleEvents::new();
        let mut provider = ServiceEmbeddingProvider::host(
            "host-embedder",
            &entry,
            Box::new(FailingHostEmbeddingInvoker {
                capabilities: json!({
                    "ready": true,
                    "embedding_spaces": [
                        {
                            "id": "default",
                            "provider": "bring-your-own",
                            "model": "test-embedding",
                            "dimensions": 3,
                            "metric": "cosine",
                            "normalized": true
                        }
                    ]
                }),
            }),
            Some(lifecycle_events.emitter()),
        )
        .unwrap();

        let err = provider.embed(&space, "alpha").unwrap_err();

        assert_eq!(err.code, "embedding_provider_failed");
        assert!(err.message.contains("host embedding failed"));
        let events = lifecycle_events.drain();
        assert!(events.iter().any(|event| {
            event.event_type == HarnessEventType::ServiceUnhealthy
                && event.service == "embedding"
                && event.registry_id == "host-embedder"
                && event.message.contains("host embedding failed")
        }));
        assert!(events.iter().any(|event| {
            event.event_type == HarnessEventType::ServiceFailed
                && event.service == "embedding"
                && event.registry_id == "host-embedder"
                && event.message.contains("host embedding failed")
        }));
    }

    #[test]
    fn local_vector_query_rejects_incompatible_embedding_provider() {
        let root = temp_dir("vector-mismatch");
        write_vector_package(&root);
        execute_knowledge_build(&root.join("agent.json"), KnowledgeBuildMode::Write).unwrap();
        let snapshot = load_knowledge_snapshot(
            &root,
            "agent_binding".into(),
            "local".into(),
            "available".into(),
            None,
        )
        .unwrap();
        let mut wrong_space = snapshot.embedding.clone().unwrap();
        wrong_space.model = "wrong".into();
        let runtime_snapshot = runtime_with_knowledge(snapshot);
        let mut runtime = LocalKnowledgeRuntime::from_runtime(
            &runtime_snapshot,
            Some(Box::new(StaticEmbeddingProvider {
                space: wrong_space,
                vector: vec![1.0, 0.0, 0.0],
            })),
        );

        let result = runtime.dispatch(&SemanticAction::KnowledgeRequest {
            package: "vector-docs".into(),
            mode: Some(KnowledgeRequestMode::VectorQuery),
            document: None,
            query: Some("alpha".into()),
            top_k: Some(1),
            score_threshold: None,
            return_citations: Some(true),
        });

        assert!(result.ok);
        assert_eq!(result.output["ok"], json!(false));
        assert_eq!(
            result.output["error"]["code"],
            json!("embedding_space_mismatch")
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn embedding_response_must_be_finite_dimensioned_and_normalized() {
        let space = KnowledgeEmbeddingSnapshot {
            id: "default".into(),
            provider: "bring-your-own".into(),
            model: "three-d".into(),
            dimensions: 3,
            metric: "cosine".into(),
            normalized: true,
        };
        assert!(parse_embedding_response(json!([1.0, 0.0, 0.0]), &space).is_ok());
        assert!(parse_embedding_response(json!([2.0, 0.0, 0.0]), &space).is_err());
        assert!(parse_embedding_response(json!([1.0, 0.0]), &space).is_err());
        assert!(parse_embedding_response(json!([1.0, "nan", 0.0]), &space).is_err());
    }

    #[test]
    fn golden_knowledge_protocol_fixtures_parse() {
        #[derive(Deserialize)]
        struct RequestFixture {
            request: KnowledgeRuntimeRequest,
        }

        let request: RequestFixture = serde_json::from_str(include_str!(
            "../../fixtures/harness/knowledge/knowledge_runtime_request.json"
        ))
        .unwrap();
        assert_eq!(request.request.package, "@zack/docs");
        assert_eq!(request.request.mode, KnowledgeRequestMode::VectorQuery);

        let result: KnowledgeRuntimeResult = serde_json::from_str(include_str!(
            "../../fixtures/harness/knowledge/knowledge_runtime_result.json"
        ))
        .unwrap();
        assert!(result.ok);
        assert_eq!(result.results[0].chunk_id, "chunk_001");

        let embedding_request: Value = serde_json::from_str(include_str!(
            "../../fixtures/harness/knowledge/embedding_provider_request.json"
        ))
        .unwrap();
        assert_eq!(embedding_request["dimensions"], json!(3));

        let embedding_result: Value = serde_json::from_str(include_str!(
            "../../fixtures/harness/knowledge/embedding_provider_result.json"
        ))
        .unwrap();
        assert_eq!(embedding_result["vector"][0], json!(1.0));

        let _: Value = serde_json::from_str(include_str!(
            "../../fixtures/harness/knowledge/knowledge_hooks.json"
        ))
        .unwrap();
        let _: Value = serde_json::from_str(include_str!(
            "../../fixtures/harness/knowledge/typed_failures.json"
        ))
        .unwrap();
    }

    #[test]
    fn knowledge_runtime_capabilities_require_modes_features_ready_and_corpus_attestation() {
        let root = temp_dir("custom-runtime-attestation");
        write_vector_package(&root);
        execute_knowledge_build(&root.join("agent.json"), KnowledgeBuildMode::Write).unwrap();
        let snapshot = load_knowledge_snapshot(
            &root,
            "agent_binding".into(),
            "custom-kb".into(),
            "available".into(),
            None,
        )
        .unwrap();
        let corpus =
            parse_knowledge_manifest(&load_manifest_value(&root.join("agent.json")).unwrap().0)
                .unwrap()
                .knowledge
                .corpus
                .unwrap()
                .content_hash
                .unwrap();

        validate_knowledge_runtime_capabilities(
            &json!({
                "registry_id": "custom-kb",
                "ready": true,
                "modes": ["vector_query"],
                "features": [],
                "packages": [{
                    "package": "vector-docs",
                    "version": "0.1.0",
                    "corpus": corpus,
                    "ready": true
                }]
            }),
            "custom-kb",
            std::slice::from_ref(&snapshot),
        )
        .unwrap();

        let err = validate_knowledge_runtime_capabilities(
            &json!({
                "registry_id": "custom-kb",
                "ready": true,
                "features": [],
                "packages": [{
                    "package": "vector-docs",
                    "version": "0.1.0",
                    "corpus": "sha256:wrong",
                    "ready": true
                }]
            }),
            "custom-kb",
            std::slice::from_ref(&snapshot),
        )
        .unwrap_err();
        assert!(err.to_string().contains("modes"));

        let err = validate_knowledge_runtime_capabilities(
            &json!({
                "registry_id": "custom-kb",
                "ready": true,
                "modes": ["context_document"],
                "features": [],
                "packages": [{
                    "package": "vector-docs",
                    "version": "0.1.0",
                    "corpus": "sha256:wrong",
                    "ready": true
                }]
            }),
            "custom-kb",
            std::slice::from_ref(&snapshot),
        )
        .unwrap_err();
        assert!(err.to_string().contains("mode `vector_query`"));

        let err = validate_knowledge_runtime_capabilities(
            &json!({
                "registry_id": "custom-kb",
                "ready": true,
                "modes": ["vector_query"],
                "features": [],
                "packages": [{
                    "package": "vector-docs",
                    "version": "0.1.0",
                    "corpus": "sha256:wrong",
                    "ready": true
                }]
            }),
            "custom-kb",
            std::slice::from_ref(&snapshot),
        )
        .unwrap_err();
        assert!(err.to_string().contains("corpus sha256:"));

        let err = validate_knowledge_runtime_capabilities(
            &json!({
                "registry_id": "custom-kb",
                "ready": true,
                "modes": ["vector_query"],
                "features": [],
                "packages": [{
                    "package": "vector-docs",
                    "version": "0.1.0",
                    "ready": false
                }]
            }),
            "custom-kb",
            &[snapshot],
        )
        .unwrap_err();
        assert!(err.to_string().contains("as ready"));

        let _ = std::fs::remove_dir_all(root);
    }

    fn runtime_with_knowledge(knowledge: KnowledgeRuntimeSnapshot) -> RuntimeSnapshot {
        let mut runtime = RuntimeSnapshot::empty("session".into());
        runtime.knowledge.push(knowledge);
        runtime
    }

    fn temp_dir(label: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("agentpm-harness-knowledge-{label}-{nanos}"));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write_vector_package(root: &Path) {
        std::fs::create_dir_all(root.join("knowledge/embeddings")).unwrap();
        std::fs::write(
            root.join("knowledge/chunks.jsonl"),
            concat!(
                "{\"id\":\"chunk_alpha\",\"source_id\":\"src_alpha\",\"text\":\"Alpha text\"}\n",
                "{\"id\":\"chunk_beta\",\"source_id\":\"src_beta\",\"text\":\"Beta text\"}\n"
            ),
        )
        .unwrap();
        std::fs::write(
            root.join("knowledge/sources.jsonl"),
            concat!(
                "{\"id\":\"src_alpha\",\"title\":\"Alpha\",\"uri\":\"file://alpha\"}\n",
                "{\"id\":\"src_beta\",\"title\":\"Beta\",\"uri\":\"file://beta\"}\n"
            ),
        )
        .unwrap();
        write_f32_vectors(
            &root.join("knowledge/embeddings/default.f32"),
            &[vec![1.0, 0.0, 0.0], vec![0.0, 1.0, 0.0]],
        );
        std::fs::write(
            root.join("agent.json"),
            serde_json::to_vec_pretty(&json!({
                "kind": "knowledge",
                "name": "vector-docs",
                "version": "0.1.0",
                "description": "Vector docs.",
                "knowledge": {
                    "mode": "vector",
                    "corpus": {
                        "chunks_path": "knowledge/chunks.jsonl",
                        "sources_path": "knowledge/sources.jsonl"
                    },
                    "embedding": {
                        "id": "default",
                        "provider": "bring-your-own",
                        "model": "three-d",
                        "dimensions": 3,
                        "metric": "cosine",
                        "normalized": true,
                        "vectors_path": "knowledge/embeddings/default.f32"
                    },
                    "retrieval": {
                        "strategy": "exact",
                        "default_top_k": 2,
                        "return_citations": true
                    }
                }
            }))
            .unwrap(),
        )
        .unwrap();
    }

    fn write_f32_vectors(path: &Path, rows: &[Vec<f32>]) {
        let mut bytes = Vec::new();
        for row in rows {
            for value in row {
                bytes.extend_from_slice(&value.to_le_bytes());
            }
        }
        std::fs::write(path, bytes).unwrap();
    }
}
