use crate::adapter::{AdapterToolDescriptor, invoke_descriptor, list_locked_tool_descriptors};
use crate::prelude::*;
use crate::runner::{RunOptions, RunnerErrorKind, classify_runner_error};
use anyhow::{Context, bail};
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::{BTreeSet, HashMap};
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;

const DEFAULT_MCP_HOST: &str = "127.0.0.1";
const DEFAULT_MCP_PORT: u16 = 7331;
const MCP_PROTOCOL_VERSION: &str = "2025-03-26";

#[derive(Args, Debug, Clone)]
pub struct ServeArgs {
    /// Start the built-in local MCP server
    #[arg(long)]
    pub mcp: bool,

    /// Host address for the MCP server
    #[arg(long, default_value = DEFAULT_MCP_HOST)]
    pub host: String,

    /// Port for the MCP server
    #[arg(long, default_value_t = DEFAULT_MCP_PORT)]
    pub port: u16,

    /// Restrict exposure to a specific package ref; repeatable
    #[arg(long = "tool", value_name = "PACKAGE_REF")]
    pub tool: Vec<String>,

    /// Restrict exposure to a comma-separated list of package refs
    #[arg(long, value_name = "PACKAGE_REFS")]
    pub tools: Option<String>,
}

impl ServeArgs {
    pub async fn run(self, _base_url: String) -> Result<()> {
        if !self.mcp {
            bail!("serve currently requires --mcp");
        }

        let project_dir = std::env::current_dir().context("reading current directory")?;
        let registry = build_registry(&project_dir, &self.selected_tools()?)?;
        let addr = SocketAddr::new(
            self.host
                .parse()
                .with_context(|| format!("invalid host address: {}", self.host))?,
            self.port,
        );
        let listener = tokio::net::TcpListener::bind(addr)
            .await
            .with_context(|| format!("binding MCP server to {}", addr))?;
        let bound = listener.local_addr().context("reading bound MCP address")?;

        eprintln!("AgentPM MCP server listening on http://{bound}");
        axum::serve(
            listener,
            build_router(Arc::new(AppState {
                project_dir,
                registry,
            })),
        )
        .await
        .context("running MCP server")
    }

    fn selected_tools(&self) -> Result<Option<BTreeSet<String>>> {
        let mut selected = BTreeSet::new();
        for tool in &self.tool {
            let trimmed = tool.trim();
            if trimmed.is_empty() {
                bail!("--tool values must not be empty");
            }
            selected.insert(trimmed.to_string());
        }
        if let Some(tools) = &self.tools {
            for tool in tools.split(',') {
                let trimmed = tool.trim();
                if trimmed.is_empty() {
                    bail!("--tools must not contain empty entries");
                }
                selected.insert(trimmed.to_string());
            }
        }

        if selected.is_empty() {
            Ok(None)
        } else {
            Ok(Some(selected))
        }
    }
}

#[derive(Clone)]
struct AppState {
    project_dir: PathBuf,
    registry: ToolRegistry,
}

#[derive(Clone)]
struct ToolRegistry {
    tools: Arc<Vec<McpToolRegistration>>,
    by_name: Arc<HashMap<String, McpToolRegistration>>,
}

#[derive(Debug, Clone)]
struct McpToolRegistration {
    mcp_name: String,
    descriptor: AdapterToolDescriptor,
}

impl ToolRegistry {
    fn from_descriptors(descriptors: Vec<AdapterToolDescriptor>) -> Result<Self> {
        let mut tools = Vec::with_capacity(descriptors.len());
        let mut by_name = HashMap::with_capacity(descriptors.len());

        for descriptor in descriptors {
            let mcp_name = package_ref_to_mcp_name(&descriptor.package_ref);
            if by_name.contains_key(&mcp_name) {
                bail!(
                    "tool name collision after MCP normalization: {}",
                    descriptor.package_ref
                );
            }
            let registration = McpToolRegistration {
                mcp_name: mcp_name.clone(),
                descriptor,
            };
            by_name.insert(mcp_name, registration.clone());
            tools.push(registration);
        }

        Ok(Self {
            tools: Arc::new(tools),
            by_name: Arc::new(by_name),
        })
    }

    fn tools_list(&self) -> Vec<Value> {
        self.tools
            .iter()
            .map(|tool| {
                json!({
                    "name": tool.mcp_name,
                    "description": tool.descriptor.description.clone().unwrap_or_default(),
                    "inputSchema": tool.descriptor.input_schema.clone(),
                })
            })
            .collect()
    }

    fn find(&self, name: &str) -> Option<&McpToolRegistration> {
        self.by_name.get(name)
    }
}

fn build_registry(
    project_dir: &Path,
    selected_tools: &Option<BTreeSet<String>>,
) -> Result<ToolRegistry> {
    let descriptors = list_locked_tool_descriptors(project_dir)?;
    let descriptors = if let Some(selected) = selected_tools {
        let available: BTreeSet<_> = descriptors
            .iter()
            .map(|descriptor| descriptor.package_ref.clone())
            .collect();
        let missing: Vec<_> = selected
            .iter()
            .filter(|tool| !available.contains(*tool))
            .cloned()
            .collect();
        if !missing.is_empty() {
            bail!(
                "requested tool(s) not found in agent.lock: {}",
                missing.join(", ")
            );
        }

        descriptors
            .into_iter()
            .filter(|descriptor| selected.contains(&descriptor.package_ref))
            .collect()
    } else {
        descriptors
    };

    ToolRegistry::from_descriptors(descriptors)
}

fn build_router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/", get(root_get).post(mcp_post))
        .route("/mcp", post(mcp_post))
        .with_state(state)
}

async fn root_get() -> &'static str {
    "AgentPM MCP server"
}

async fn mcp_post(State(state): State<Arc<AppState>>, Json(request): Json<Value>) -> Response {
    match dispatch_request(state, request).await {
        DispatchOutcome::Response(body) => (StatusCode::OK, Json(body)).into_response(),
        DispatchOutcome::Notification => StatusCode::NO_CONTENT.into_response(),
    }
}

enum DispatchOutcome {
    Response(Value),
    Notification,
}

#[derive(Debug, Deserialize)]
struct JsonRpcRequest {
    #[serde(default)]
    jsonrpc: Option<String>,
    #[serde(default)]
    id: Option<Value>,
    method: String,
    #[serde(default)]
    params: Option<Value>,
}

async fn dispatch_request(state: Arc<AppState>, request: Value) -> DispatchOutcome {
    let parsed: JsonRpcRequest = match serde_json::from_value(request) {
        Ok(request) => request,
        Err(err) => {
            return DispatchOutcome::Response(json_rpc_error(
                None,
                -32600,
                format!("invalid JSON-RPC request: {err}"),
            ));
        }
    };

    if parsed.jsonrpc.as_deref() != Some("2.0") {
        return DispatchOutcome::Response(json_rpc_error(
            parsed.id,
            -32600,
            "jsonrpc must be \"2.0\"",
        ));
    }

    let id = parsed.id.clone();
    match parsed.method.as_str() {
        "initialize" => DispatchOutcome::Response(json_rpc_result(
            id,
            json!({
                "protocolVersion": MCP_PROTOCOL_VERSION,
                "capabilities": {
                    "tools": {
                        "listChanged": false
                    }
                },
                "serverInfo": {
                    "name": "agentpm",
                    "version": env!("CARGO_PKG_VERSION")
                }
            }),
        )),
        "notifications/initialized" => DispatchOutcome::Notification,
        "tools/list" => DispatchOutcome::Response(json_rpc_result(
            id,
            json!({
                "tools": state.registry.tools_list()
            }),
        )),
        "tools/call" => match handle_tools_call(state, parsed.params).await {
            Ok(result) => DispatchOutcome::Response(json_rpc_result(id, result)),
            Err(error) => DispatchOutcome::Response(json_rpc_error(id, error.code, error.message)),
        },
        _ => DispatchOutcome::Response(json_rpc_error(
            id,
            -32601,
            format!("unsupported MCP method: {}", parsed.method),
        )),
    }
}

#[derive(Debug)]
struct RpcError {
    code: i64,
    message: String,
}

async fn handle_tools_call(state: Arc<AppState>, params: Option<Value>) -> Result<Value, RpcError> {
    let params = params.ok_or_else(|| RpcError {
        code: -32602,
        message: "tools/call requires params".to_string(),
    })?;
    let params = params.as_object().ok_or_else(|| RpcError {
        code: -32602,
        message: "tools/call params must be an object".to_string(),
    })?;

    let name = params
        .get("name")
        .and_then(Value::as_str)
        .ok_or_else(|| RpcError {
            code: -32602,
            message: "tools/call params.name must be a string".to_string(),
        })?;
    let arguments = params
        .get("arguments")
        .cloned()
        .unwrap_or_else(|| Value::Object(Default::default()));

    let registration = state.registry.find(name).cloned().ok_or_else(|| RpcError {
        code: -32602,
        message: format!("unknown MCP tool: {name}"),
    })?;
    let project_dir = state.project_dir.clone();
    let descriptor = registration.descriptor.clone();

    let result = tokio::task::spawn_blocking(move || {
        invoke_descriptor(
            &project_dir,
            &descriptor,
            &arguments,
            &RunOptions::default(),
        )
    })
    .await
    .map_err(|err| RpcError {
        code: -32603,
        message: format!("failed to join MCP tool worker: {err}"),
    })?
    .map_err(map_runner_error)?;

    Ok(json!({
        "content": [{
            "type": "text",
            "text": serde_json::to_string(&result.output).map_err(|err| RpcError {
                code: -32603,
                message: format!("failed to serialize MCP tool output: {err}"),
            })?
        }],
        "structuredContent": result.output,
        "isError": false
    }))
}

fn map_runner_error(err: anyhow::Error) -> RpcError {
    let message = format!("{err:#}");
    let code = match classify_runner_error(&err) {
        RunnerErrorKind::Resolution => -32602,
        RunnerErrorKind::Runtime => -32001,
        RunnerErrorKind::Timeout => -32002,
        RunnerErrorKind::MalformedOutput => -32003,
        RunnerErrorKind::SubprocessFailure => -32004,
        RunnerErrorKind::Other => -32000,
    };

    RpcError { code, message }
}

fn json_rpc_result(id: Option<Value>, result: Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id.unwrap_or(Value::Null),
        "result": result
    })
}

fn json_rpc_error(id: Option<Value>, code: i64, message: impl Into<String>) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id.unwrap_or(Value::Null),
        "error": {
            "code": code,
            "message": message.into()
        }
    })
}

fn package_ref_to_mcp_name(package_ref: &str) -> String {
    let without_scope = package_ref.trim_start_matches('@');
    let (namespace, name) = without_scope
        .split_once('/')
        .unwrap_or(("tool", without_scope));
    format!(
        "{}__{}",
        normalize_mcp_component(namespace),
        normalize_mcp_component(name)
    )
}

fn normalize_mcp_component(component: &str) -> String {
    component
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::{Body, to_bytes};
    use axum::http::Request;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::process::{Command, Stdio};
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};
    use tower::util::ServiceExt;

    static NEXT_ID: AtomicU64 = AtomicU64::new(1);

    #[tokio::test]
    async fn lists_locked_tools_over_http_mcp() {
        let python = available_command(&["python3", "python"]).expect("python required for tests");
        let root = TestProject::new();
        root.write_lock(lock_for("@zack/echo-json", "0.1.0"));
        root.write_tool(
            "@zack/echo-json",
            "0.1.0",
            python_tool_manifest("echo-json", "0.1.0", python.as_str()),
            python_echo_script("0.1.0"),
        );

        let app = test_app(root.path());

        let initialize = app
            .clone()
            .oneshot(json_request(
                "/",
                json!({
                    "jsonrpc": "2.0",
                    "id": 1,
                    "method": "initialize",
                    "params": {}
                }),
            ))
            .await
            .unwrap();
        assert!(initialize.status().is_success());

        let response = app
            .oneshot(json_request(
                "/",
                json!({
                    "jsonrpc": "2.0",
                    "id": 2,
                    "method": "tools/list",
                    "params": {}
                }),
            ))
            .await
            .unwrap();
        let body = json_body(response).await;
        let tools = body["result"]["tools"].as_array().unwrap();

        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0]["name"], "zack__echo_json");
        assert_eq!(tools[0]["description"], "Echo tool for MCP tests");
        assert_eq!(tools[0]["inputSchema"]["type"], "object");
    }

    #[tokio::test]
    async fn calls_locked_tool_over_http_mcp() {
        let python = available_command(&["python3", "python"]).expect("python required for tests");
        let root = TestProject::new();
        root.write_lock(lock_for("@zack/echo-json", "0.1.0"));
        root.write_tool(
            "@zack/echo-json",
            "0.1.0",
            python_tool_manifest("echo-json", "0.1.0", python.as_str()),
            python_echo_script("0.1.0"),
        );
        root.write_tool(
            "@zack/echo-json",
            "0.2.0",
            python_tool_manifest("echo-json", "0.2.0", python.as_str()),
            python_echo_script("0.2.0"),
        );

        let app = test_app(root.path());

        let response = app
            .oneshot(json_request(
                "/mcp",
                json!({
                    "jsonrpc": "2.0",
                    "id": 3,
                    "method": "tools/call",
                    "params": {
                        "name": "zack__echo_json",
                        "arguments": {
                            "message": "hello"
                        }
                    }
                }),
            ))
            .await
            .unwrap();
        let body = json_body(response).await;

        assert_eq!(body["result"]["structuredContent"]["toolVersion"], "0.1.0");
        assert_eq!(
            body["result"]["structuredContent"]["input"]["message"],
            "hello"
        );
        assert_eq!(body["result"]["isError"], false);
    }

    #[tokio::test]
    async fn returns_invalid_params_for_unknown_tool_name() {
        let python = available_command(&["python3", "python"]).expect("python required for tests");
        let root = TestProject::new();
        root.write_lock(lock_for("@zack/echo-json", "0.1.0"));
        root.write_tool(
            "@zack/echo-json",
            "0.1.0",
            python_tool_manifest("echo-json", "0.1.0", python.as_str()),
            python_echo_script("0.1.0"),
        );

        let app = test_app(root.path());
        let response = app
            .oneshot(json_request(
                "/mcp",
                json!({
                    "jsonrpc": "2.0",
                    "id": 4,
                    "method": "tools/call",
                    "params": {
                        "name": "zack__missing_tool",
                        "arguments": {}
                    }
                }),
            ))
            .await
            .unwrap();
        let body = json_body(response).await;

        assert_eq!(body["error"]["code"], -32602);
        assert!(
            body["error"]["message"]
                .as_str()
                .unwrap()
                .contains("unknown MCP tool")
        );
    }

    #[tokio::test]
    async fn returns_runtime_error_for_missing_required_env() {
        let python = available_command(&["python3", "python"]).expect("python required for tests");
        let root = TestProject::new();
        root.write_lock(lock_for("@zack/requires-env", "0.1.0"));
        root.write_tool(
            "@zack/requires-env",
            "0.1.0",
            python_required_env_manifest("requires-env", "0.1.0", python.as_str()),
            python_echo_script("0.1.0"),
        );

        let app = test_app(root.path());
        let response = app
            .oneshot(json_request(
                "/mcp",
                json!({
                    "jsonrpc": "2.0",
                    "id": 5,
                    "method": "tools/call",
                    "params": {
                        "name": "zack__requires_env",
                        "arguments": {}
                    }
                }),
            ))
            .await
            .unwrap();
        let body = json_body(response).await;

        assert_eq!(body["error"]["code"], -32001);
        assert!(
            body["error"]["message"]
                .as_str()
                .unwrap()
                .contains("missing required environment variables")
        );
    }

    #[tokio::test]
    async fn returns_timeout_error_for_slow_tool() {
        let python = available_command(&["python3", "python"]).expect("python required for tests");
        let root = TestProject::new();
        root.write_lock(lock_for("@zack/slow-tool", "0.1.0"));
        root.write_tool(
            "@zack/slow-tool",
            "0.1.0",
            python_timeout_manifest("slow-tool", "0.1.0", python.as_str(), 50),
            python_sleep_script(200),
        );

        let app = test_app(root.path());
        let response = app
            .oneshot(json_request(
                "/mcp",
                json!({
                    "jsonrpc": "2.0",
                    "id": 6,
                    "method": "tools/call",
                    "params": {
                        "name": "zack__slow_tool",
                        "arguments": {}
                    }
                }),
            ))
            .await
            .unwrap();
        let body = json_body(response).await;

        assert_eq!(body["error"]["code"], -32002);
        assert!(
            body["error"]["message"]
                .as_str()
                .unwrap()
                .contains("tool execution timed out")
        );
    }

    #[tokio::test]
    async fn initialized_notification_returns_204_without_body() {
        let root = TestProject::new();
        let app = test_app(root.path());

        let response = app
            .clone()
            .oneshot(json_request(
                "/",
                json!({
                    "jsonrpc": "2.0",
                    "id": 7,
                    "method": "tools/list",
                    "params": {}
                }),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let notification = app
            .oneshot(json_request(
                "/",
                json!({
                    "jsonrpc": "2.0",
                    "method": "notifications/initialized",
                    "params": {}
                }),
            ))
            .await
            .unwrap();
        let status = notification.status();
        let body = to_bytes(notification.into_body(), usize::MAX)
            .await
            .unwrap();

        assert_eq!(status, StatusCode::NO_CONTENT);
        assert!(body.is_empty(), "expected empty body for 204 response");
    }

    fn available_command(candidates: &[&str]) -> Option<String> {
        candidates.iter().find_map(|candidate| {
            Command::new(candidate)
                .arg("--version")
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .ok()
                .filter(|status| status.success())
                .map(|_| (*candidate).to_string())
        })
    }

    fn lock_for(package: &str, version: &str) -> String {
        format!(
            r#"{{
  "lockfile_version": 1,
  "generated": "2026-05-03T00:00:00Z",
  "dependencies": {{
    "{package}": {{
      "version": "{version}",
      "integrity": "abc"
    }}
  }}
}}"#
        )
    }

    fn python_tool_manifest(name: &str, version: &str, command: &str) -> String {
        format!(
            r#"{{
  "kind": "tool",
  "name": "{name}",
  "version": "{version}",
  "description": "Echo tool for MCP tests",
  "entrypoint": {{
    "command": "{command}",
    "args": ["script.py"],
    "cwd": ".",
    "timeout_ms": 5000,
    "env": {{}}
  }},
  "runtime": {{
    "type": "python",
    "version": ">=3.10"
  }},
  "inputs": {{
    "type": "object",
    "properties": {{
      "message": {{
        "type": "string"
      }}
    }},
    "required": ["message"]
  }},
  "outputs": {{
    "type": "object",
    "properties": {{
      "toolVersion": {{
        "type": "string"
      }},
      "input": {{
        "type": "object"
      }}
    }},
    "required": ["toolVersion", "input"]
  }}
}}"#
        )
    }

    fn python_required_env_manifest(name: &str, version: &str, command: &str) -> String {
        format!(
            r#"{{
  "kind": "tool",
  "name": "{name}",
  "version": "{version}",
  "description": "Env-dependent tool for MCP tests",
  "entrypoint": {{
    "command": "{command}",
    "args": ["script.py"],
    "cwd": ".",
    "timeout_ms": 5000,
    "env": {{}}
  }},
  "runtime": {{
    "type": "python"
  }},
  "environment": {{
    "vars": {{
      "API_TOKEN": {{
        "required": true
      }}
    }}
  }},
  "inputs": {{
    "type": "object"
  }},
  "outputs": {{
    "type": "object"
  }}
}}"#
        )
    }

    fn python_timeout_manifest(
        name: &str,
        version: &str,
        command: &str,
        timeout_ms: u64,
    ) -> String {
        format!(
            r#"{{
  "kind": "tool",
  "name": "{name}",
  "version": "{version}",
  "description": "Slow tool for MCP timeout tests",
  "entrypoint": {{
    "command": "{command}",
    "args": ["script.py"],
    "cwd": ".",
    "timeout_ms": {timeout_ms},
    "env": {{}}
  }},
  "runtime": {{
    "type": "python"
  }},
  "inputs": {{
    "type": "object"
  }},
  "outputs": {{
    "type": "object"
  }}
}}"#
        )
    }

    fn python_echo_script(version: &str) -> String {
        format!(
            r#"import json
import sys

payload = json.load(sys.stdin)
json.dump(
    {{
        "toolVersion": "{version}",
        "input": payload
    }},
    sys.stdout,
)
"#
        )
    }

    fn python_sleep_script(sleep_ms: u64) -> String {
        format!(
            r#"import json
import sys
import time

json.load(sys.stdin)
time.sleep({sleep_ms} / 1000.0)
json.dump({{"ok": True}}, sys.stdout)
"#
        )
    }

    struct TestProject {
        root: PathBuf,
    }

    impl TestProject {
        fn new() -> Self {
            let mut root = std::env::temp_dir();
            let unique = format!(
                "agentpm-serve-test-{}-{}",
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_millis(),
                NEXT_ID.fetch_add(1, Ordering::Relaxed)
            );
            root.push(unique);
            fs::create_dir_all(&root).unwrap();
            Self { root }
        }

        fn path(&self) -> &Path {
            &self.root
        }

        fn write_lock(&self, lock: String) {
            fs::write(self.root.join("agent.lock"), lock).unwrap();
        }

        fn write_tool(&self, package: &str, version: &str, manifest: String, script: String) {
            let dir = self.tool_dir(package, version);
            fs::create_dir_all(&dir).unwrap();
            fs::write(dir.join("agent.json"), manifest).unwrap();
            fs::write(dir.join("script.py"), script).unwrap();
        }

        fn tool_dir(&self, package: &str, version: &str) -> PathBuf {
            let trimmed = package.trim_start_matches('@');
            let (namespace, name) = trimmed.split_once('/').unwrap();
            self.root
                .join(".agentpm")
                .join("tools")
                .join(namespace)
                .join(name)
                .join(version)
        }
    }

    impl Drop for TestProject {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    fn test_app(project_dir: &Path) -> Router {
        let registry = build_registry(project_dir, &None).unwrap();
        let state = Arc::new(AppState {
            project_dir: project_dir.to_path_buf(),
            registry,
        });
        build_router(state)
    }

    fn json_request(path: &str, body: Value) -> Request<Body> {
        Request::builder()
            .method("POST")
            .uri(path)
            .header("content-type", "application/json")
            .body(Body::from(body.to_string()))
            .unwrap()
    }

    async fn json_body(response: axum::response::Response) -> Value {
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        serde_json::from_slice(&body).unwrap()
    }
}
