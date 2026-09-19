use super::action::{ActionDispatchResult, ActionFailureCategory};
use super::model::{McpImportRuntimeSnapshot, RuntimeCapabilitySnapshot};
use crate::harness_config::{
    HarnessMcpHeaderValue, HarnessMcpImport, HarnessMcpScope, HarnessRestartPolicy,
};
use anyhow::{Context, Result, anyhow, bail};
use reqwest::blocking::Client;
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fmt;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::time::Duration;

const ERR_MCP_STDIO_RESPONSE_TIMED_OUT: &str = "MCP stdio response timed out";
const ERR_MCP_STDIO_SERVER_CLOSED_STDOUT: &str = "MCP stdio server closed stdout";
const ERR_MCP_STDIO_CLOSED_STDOUT_INITIALIZE: &str = "closed stdout during initialize";
const ERR_MCP_STDIO_CLOSED_STDOUT_RESTART_INITIALIZE: &str =
    "closed stdout during restart initialize";
const ERR_MCP_STDIO_WRITE_REQUEST: &str = "writing MCP stdio JSON-RPC request";
const ERR_MCP_STDIO_FLUSH_REQUEST: &str = "flushing MCP stdio request";

#[derive(Debug)]
pub struct McpImportRuntimeActivation {
    pub runtime: ConfiguredMcpImportRuntime,
    pub snapshots: Vec<McpImportRuntimeSnapshot>,
    pub capability_candidates: Vec<RuntimeCapabilitySnapshot>,
}

#[derive(Debug, Default)]
pub struct ConfiguredMcpImportRuntime {
    clients: BTreeMap<String, McpImportClient>,
}

impl ConfiguredMcpImportRuntime {
    pub fn start(
        workspace_root: &Path,
        imports: &HashMap<String, HarnessMcpImport>,
    ) -> McpImportRuntimeActivation {
        let env: HashMap<String, String> = std::env::vars().collect();
        Self::start_with_env(workspace_root, imports, &env)
    }

    pub(crate) fn start_with_env(
        workspace_root: &Path,
        imports: &HashMap<String, HarnessMcpImport>,
        env: &HashMap<String, String>,
    ) -> McpImportRuntimeActivation {
        let mut runtime = Self::default();
        let mut snapshots = Vec::new();
        let mut capability_candidates = Vec::new();

        for (server_id, import) in imports {
            let transport = mcp_import_transport(import);
            let endpoint = mcp_import_safe_endpoint(import);
            let scopes = mcp_import_scope_labels(import);
            match start_import_server(workspace_root, server_id, import, env) {
                Ok(mut client) => {
                    let tools = match client.list_tools() {
                        Ok(tools) => tools,
                        Err(err) => {
                            snapshots.push(failed_import_snapshot(
                                server_id,
                                transport,
                                &scopes,
                                endpoint.as_deref(),
                                format!("MCP import `{server_id}` tools/list failed: {err:#}"),
                            ));
                            continue;
                        }
                    };
                    let allowed = import_tools(import);
                    let filtered = filter_tools(
                        server_id,
                        allowed.as_ref(),
                        tools,
                        transport,
                        &scopes,
                        endpoint.as_deref(),
                        &mut snapshots,
                    );
                    if filtered.is_empty() {
                        snapshots.push(failed_import_snapshot(
                            server_id,
                            transport,
                            &scopes,
                            endpoint.as_deref(),
                            format!("MCP import `{server_id}` did not expose any eligible Tools"),
                        ));
                        continue;
                    }

                    for tool in filtered {
                        let identity = imported_mcp_identity(server_id, &tool.name);
                        snapshots.push(McpImportRuntimeSnapshot {
                            server_id: server_id.clone(),
                            tool_name: tool.name.clone(),
                            identity: identity.clone(),
                            description: tool.description.clone().unwrap_or_else(|| {
                                format!("Imported MCP Tool `{}` from `{server_id}`.", tool.name)
                            }),
                            input_schema: tool.input_schema.unwrap_or_else(default_input_schema),
                            transport: transport.into(),
                            scopes: scopes.clone(),
                            endpoint: endpoint.clone(),
                            state: "available".into(),
                            readiness_reason: None,
                            source: "harness_config".into(),
                        });
                        for scope in &scopes {
                            capability_candidates.push(RuntimeCapabilitySnapshot {
                                kind: "mcp_import_tool".into(),
                                identity: identity.clone(),
                                scope: scope.clone(),
                                source: "harness_config".into(),
                                state: "available".into(),
                            });
                        }
                    }
                    runtime.clients.insert(server_id.clone(), client);
                }
                Err(err) => {
                    snapshots.push(failed_import_snapshot(
                        server_id,
                        transport,
                        &scopes,
                        endpoint.as_deref(),
                        format!("MCP import `{server_id}` failed to initialize: {err:#}"),
                    ));
                }
            }
        }

        McpImportRuntimeActivation {
            runtime,
            snapshots,
            capability_candidates,
        }
    }

    pub fn call_tool(
        &mut self,
        server: &str,
        tool: &str,
        arguments: &Value,
    ) -> ActionDispatchResult {
        let Some(client) = self.clients.get_mut(server) else {
            return ActionDispatchResult::failure_with_category(
                ActionFailureCategory::Runtime,
                format!("MCP import `{server}` is not available"),
            );
        };
        match client.call_tool(tool, arguments) {
            Ok(result) => ActionDispatchResult::success(result),
            Err(err) => {
                let category = mcp_import_call_failure_category(&err);
                ActionDispatchResult::failure_with_category(
                    category,
                    format!("MCP Tool `{server}/{tool}` failed: {err:#}"),
                )
            }
        }
    }

    pub fn stop(&mut self) {
        for client in self.clients.values_mut() {
            if let McpImportClient::Stdio { child, .. } = client {
                let _ = child.kill();
                let _ = child.wait();
            }
        }
        self.clients.clear();
    }
}

fn mcp_import_call_failure_category(err: &anyhow::Error) -> ActionFailureCategory {
    if let Some(classified) = err
        .chain()
        .find_map(|cause| cause.downcast_ref::<ClassifiedMcpImportError>())
    {
        return classified.kind;
    }

    let rendered = format!("{err:#}");
    if rendered.contains(ERR_MCP_STDIO_RESPONSE_TIMED_OUT) {
        return ActionFailureCategory::Timeout;
    }
    if rendered.contains(ERR_MCP_STDIO_SERVER_CLOSED_STDOUT)
        || rendered.contains(ERR_MCP_STDIO_CLOSED_STDOUT_RESTART_INITIALIZE)
        || rendered.contains(ERR_MCP_STDIO_CLOSED_STDOUT_INITIALIZE)
        || rendered.contains(ERR_MCP_STDIO_WRITE_REQUEST)
        || rendered.contains(ERR_MCP_STDIO_FLUSH_REQUEST)
    {
        return ActionFailureCategory::SubprocessFailure;
    }
    ActionFailureCategory::Runtime
}

#[derive(Debug)]
struct ClassifiedMcpImportError {
    kind: ActionFailureCategory,
    message: String,
}

impl fmt::Display for ClassifiedMcpImportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for ClassifiedMcpImportError {}

fn classified_mcp_import_error(
    kind: ActionFailureCategory,
    message: impl Into<String>,
) -> anyhow::Error {
    anyhow!(ClassifiedMcpImportError {
        kind,
        message: message.into(),
    })
}

impl Drop for ConfiguredMcpImportRuntime {
    fn drop(&mut self) {
        self.stop();
    }
}

#[derive(Debug)]
enum McpImportClient {
    Http {
        url: String,
        headers: BTreeMap<String, String>,
        client: Client,
        next_id: u64,
    },
    Stdio {
        server_id: String,
        config: StdioImportConfig,
        child: Child,
        stdin: ChildStdin,
        stdout: BufReader<ChildStdout>,
        next_id: u64,
        restart_attempts: u32,
    },
}

#[derive(Debug, Clone)]
struct StdioImportConfig {
    command: String,
    args: Vec<String>,
    cwd: PathBuf,
    env: BTreeMap<String, String>,
    startup_timeout_ms: u64,
    request_timeout_ms: u64,
    restart: HarnessRestartPolicy,
}

impl McpImportClient {
    fn initialize(&mut self) -> Result<()> {
        match self {
            Self::Http { .. } => {
                self.rpc_request("initialize", json!({}))?;
            }
            Self::Stdio {
                server_id,
                config,
                stdin,
                stdout,
                next_id,
                ..
            } => {
                let id = *next_id;
                *next_id += 1;
                write_stdio_json_rpc(stdin, id, "initialize", json!({}))?;
                wait_for_stdio_readable(stdout.get_ref(), config.startup_timeout_ms)
                    .context("waiting for MCP stdio initialize response")?;
                let mut line = String::new();
                stdout
                    .read_line(&mut line)
                    .context("reading MCP stdio initialize response")?;
                if line.trim().is_empty() {
                    return Err(classified_mcp_import_error(
                        ActionFailureCategory::SubprocessFailure,
                        format!(
                            "MCP import `{server_id}` {ERR_MCP_STDIO_CLOSED_STDOUT_INITIALIZE}"
                        ),
                    ));
                }
                parse_json_rpc_response(
                    serde_json::from_str(&line).context("parsing MCP stdio initialize JSON")?,
                )?;
            }
        }
        Ok(())
    }

    fn list_tools(&mut self) -> Result<Vec<DiscoveredMcpTool>> {
        let result = self.rpc_request("tools/list", json!({}))?;
        serde_json::from_value::<ToolsListResult>(result)
            .map(|result| result.tools)
            .context("parsing MCP tools/list response")
    }

    fn call_tool(&mut self, name: &str, arguments: &Value) -> Result<Value> {
        match self.rpc_request(
            "tools/call",
            json!({
                "name": name,
                "arguments": arguments,
            }),
        ) {
            Ok(result) => Ok(result),
            Err(err) => {
                if let Err(restart_err) = self.restart_after_failed_request() {
                    bail!("{err:#}; MCP import restart failed: {restart_err:#}");
                }
                Err(err)
            }
        }
    }

    fn rpc_request(&mut self, method: &str, params: Value) -> Result<Value> {
        match self {
            Self::Http {
                url,
                headers,
                client,
                next_id,
            } => {
                let id = *next_id;
                *next_id += 1;
                let mut request = client.post(url.as_str()).json(&json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "method": method,
                    "params": params,
                }));
                for (name, value) in headers.iter() {
                    request = request.header(name.as_str(), value.as_str());
                }
                let response = request
                    .send()
                    .map_err(|err| {
                        sanitized_http_mcp_error("sending MCP HTTP JSON-RPC request", url, err)
                    })?
                    .error_for_status()
                    .map_err(|err| {
                        sanitized_http_mcp_error("receiving MCP HTTP JSON-RPC response", url, err)
                    })?;
                let body = response
                    .json()
                    .map_err(|err| sanitized_http_mcp_error("parsing MCP HTTP JSON", url, err))?;
                parse_json_rpc_response(body)
            }
            Self::Stdio {
                stdin,
                stdout,
                next_id,
                config,
                ..
            } => {
                let id = *next_id;
                *next_id += 1;
                write_stdio_json_rpc(stdin, id, method, params)?;
                wait_for_stdio_readable(stdout.get_ref(), config.request_timeout_ms)
                    .context("waiting for MCP stdio response")?;
                let mut line = String::new();
                stdout
                    .read_line(&mut line)
                    .context("reading MCP stdio JSON-RPC response")?;
                if line.trim().is_empty() {
                    return Err(classified_mcp_import_error(
                        ActionFailureCategory::SubprocessFailure,
                        ERR_MCP_STDIO_SERVER_CLOSED_STDOUT,
                    ));
                }
                parse_json_rpc_response(
                    serde_json::from_str(&line).context("parsing MCP stdio JSON")?,
                )
            }
        }
    }

    fn restart_after_failed_request(&mut self) -> Result<()> {
        let Self::Stdio {
            server_id,
            config,
            child,
            stdin,
            stdout,
            next_id,
            restart_attempts,
            ..
        } = self
        else {
            return Ok(());
        };
        if *restart_attempts >= config.restart.max_attempts {
            bail!(
                "MCP import `{server_id}` restart attempts exhausted after {} attempt(s)",
                restart_attempts
            );
        }
        *restart_attempts += 1;
        std::thread::sleep(Duration::from_millis(config.restart.backoff_ms));
        stop_stdio_child(child);
        let (mut new_child, new_stdin, new_stdout) = spawn_stdio_process(server_id, config)?;
        *stdin = new_stdin;
        *stdout = BufReader::new(new_stdout);
        std::mem::swap(child, &mut new_child);
        *next_id = 1;
        let id = *next_id;
        *next_id += 1;
        write_stdio_json_rpc(stdin, id, "initialize", json!({}))?;
        wait_for_stdio_readable(stdout.get_ref(), config.startup_timeout_ms)
            .context("waiting for restarted MCP stdio initialize response")?;
        let mut line = String::new();
        stdout
            .read_line(&mut line)
            .context("reading restarted MCP stdio initialize response")?;
        if line.trim().is_empty() {
            return Err(classified_mcp_import_error(
                ActionFailureCategory::SubprocessFailure,
                format!(
                    "MCP import `{server_id}` {ERR_MCP_STDIO_CLOSED_STDOUT_RESTART_INITIALIZE}"
                ),
            ));
        }
        parse_json_rpc_response(
            serde_json::from_str(&line)
                .context("parsing restarted MCP stdio initialize JSON-RPC response")?,
        )?;
        Ok(())
    }
}

fn start_import_server(
    workspace_root: &Path,
    server_id: &str,
    import: &HarnessMcpImport,
    env_values: &HashMap<String, String>,
) -> Result<McpImportClient> {
    match import {
        HarnessMcpImport::Http { url, headers, .. } => {
            let headers = resolve_headers(server_id, headers, env_values)?;
            let client = Client::builder()
                .timeout(Duration::from_millis(120_000))
                .build()
                .context("building MCP HTTP client")?;
            let mut import = McpImportClient::Http {
                url: url.clone(),
                headers,
                client,
                next_id: 1,
            };
            import.initialize()?;
            Ok(import)
        }
        HarnessMcpImport::Stdio {
            command,
            args,
            cwd,
            env,
            startup_timeout_ms,
            request_timeout_ms,
            restart,
            ..
        } => {
            let cwd = resolve_import_cwd(workspace_root, cwd.as_deref());
            let mut resolved_env = BTreeMap::new();
            for key in env {
                let value = env_values
                    .get(key)
                    .cloned()
                    .with_context(|| format!("MCP import `{server_id}` missing env `{key}`"))?;
                resolved_env.insert(key.clone(), value);
            }
            let config = StdioImportConfig {
                command: command.clone(),
                args: args.clone(),
                cwd,
                env: resolved_env,
                startup_timeout_ms: *startup_timeout_ms,
                request_timeout_ms: *request_timeout_ms,
                restart: restart.clone(),
            };
            let (child, stdin, stdout) = spawn_stdio_process(server_id, &config)?;
            let mut import = McpImportClient::Stdio {
                server_id: server_id.into(),
                config,
                child,
                stdin,
                stdout: BufReader::new(stdout),
                next_id: 1,
                restart_attempts: 0,
            };
            import.initialize()?;
            Ok(import)
        }
    }
}

fn spawn_stdio_process(
    server_id: &str,
    config: &StdioImportConfig,
) -> Result<(Child, ChildStdin, ChildStdout)> {
    let mut process = Command::new(&config.command);
    process.args(&config.args);
    process.current_dir(&config.cwd);
    for (key, value) in &config.env {
        process.env(key, value);
    }
    let mut child = process
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .with_context(|| format!("starting MCP import `{server_id}`"))?;
    let stdin = child
        .stdin
        .take()
        .ok_or_else(|| anyhow!("MCP import `{server_id}` stdin unavailable"))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| anyhow!("MCP import `{server_id}` stdout unavailable"))?;
    Ok((child, stdin, stdout))
}

fn write_stdio_json_rpc(
    stdin: &mut ChildStdin,
    id: u64,
    method: &str,
    params: Value,
) -> Result<()> {
    writeln!(
        stdin,
        "{}",
        json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        })
    )
    .map_err(|err| {
        classified_mcp_import_error(
            ActionFailureCategory::SubprocessFailure,
            format!("{ERR_MCP_STDIO_WRITE_REQUEST}: {err}"),
        )
    })?;
    stdin.flush().map_err(|err| {
        classified_mcp_import_error(
            ActionFailureCategory::SubprocessFailure,
            format!("{ERR_MCP_STDIO_FLUSH_REQUEST}: {err}"),
        )
    })
}

fn stop_stdio_child(child: &mut Child) {
    if child.try_wait().ok().flatten().is_none() {
        let _ = child.kill();
    }
    let _ = child.wait();
}

#[cfg(unix)]
fn wait_for_stdio_readable(stdout: &ChildStdout, timeout_ms: u64) -> Result<()> {
    use std::os::fd::AsRawFd;
    let timeout_ms = timeout_ms.min(i32::MAX as u64) as i32;
    let mut poll_fd = libc::pollfd {
        fd: stdout.as_raw_fd(),
        events: libc::POLLIN,
        revents: 0,
    };
    let result = unsafe { libc::poll(&mut poll_fd, 1, timeout_ms) };
    if result < 0 {
        return Err(std::io::Error::last_os_error()).context("polling MCP stdio stdout");
    }
    if result == 0 {
        return Err(classified_mcp_import_error(
            ActionFailureCategory::Timeout,
            format!("{ERR_MCP_STDIO_RESPONSE_TIMED_OUT} after {timeout_ms}ms"),
        ));
    }
    Ok(())
}

#[cfg(not(unix))]
fn wait_for_stdio_readable(_stdout: &ChildStdout, _timeout_ms: u64) -> Result<()> {
    Ok(())
}

fn parse_json_rpc_response(value: Value) -> Result<Value> {
    if let Some(error) = value.get("error") {
        bail!("MCP JSON-RPC error: {error}");
    }
    value
        .get("result")
        .cloned()
        .ok_or_else(|| anyhow!("MCP JSON-RPC response missing result"))
}

fn filter_tools(
    server_id: &str,
    allowed: Option<&BTreeSet<String>>,
    tools: Vec<DiscoveredMcpTool>,
    transport: &str,
    scopes: &[String],
    endpoint: Option<&str>,
    snapshots: &mut Vec<McpImportRuntimeSnapshot>,
) -> Vec<DiscoveredMcpTool> {
    if let Some(allowed) = allowed {
        let advertised = tools
            .iter()
            .map(|tool| tool.name.clone())
            .collect::<BTreeSet<_>>();
        for name in allowed {
            if !advertised.contains(name) {
                snapshots.push(unavailable_tool_snapshot(
                    server_id,
                    name,
                    transport,
                    scopes,
                    endpoint,
                    "unavailable",
                    format!("MCP import `{server_id}` configured Tool `{name}` was not advertised"),
                ));
            }
        }
        tools
            .into_iter()
            .filter_map(|tool| {
                if allowed.contains(&tool.name) {
                    Some(tool)
                } else {
                    snapshots.push(unavailable_tool_snapshot(
                        server_id,
                        &tool.name,
                        transport,
                        scopes,
                        endpoint,
                        "suppressed",
                        format!(
                            "MCP import `{server_id}` Tool `{}` is not selected by its configured tools filter",
                            tool.name
                        ),
                    ));
                    None
                }
            })
            .collect()
    } else {
        tools
    }
}

fn import_tools(import: &HarnessMcpImport) -> Option<BTreeSet<String>> {
    match import {
        HarnessMcpImport::Stdio { tools, .. } | HarnessMcpImport::Http { tools, .. } => {
            tools.as_ref().map(|tools| tools.iter().cloned().collect())
        }
    }
}

fn mcp_import_scope_labels(import: &HarnessMcpImport) -> Vec<String> {
    match import {
        HarnessMcpImport::Stdio { scope, .. } | HarnessMcpImport::Http { scope, .. } => match scope
        {
            HarnessMcpScope::Global => vec!["global".into()],
            HarnessMcpScope::Phases { phases } => phases
                .iter()
                .map(|phase| format!("phase:{phase}"))
                .collect(),
        },
    }
}

fn resolve_headers(
    server_id: &str,
    headers: &HashMap<String, HarnessMcpHeaderValue>,
    env_values: &HashMap<String, String>,
) -> Result<BTreeMap<String, String>> {
    headers
        .iter()
        .map(|(name, value)| {
            let value = match value {
                HarnessMcpHeaderValue::Value { value } => value.clone(),
                HarnessMcpHeaderValue::Env { env } => {
                    env_values.get(env).cloned().with_context(|| {
                        format!("MCP import `{server_id}` missing header env `{env}`")
                    })?
                }
            };
            Ok((name.clone(), value))
        })
        .collect()
}

fn resolve_import_cwd(workspace_root: &Path, cwd: Option<&str>) -> PathBuf {
    cwd.map(|cwd| workspace_root.join(cwd))
        .unwrap_or_else(|| workspace_root.to_path_buf())
}

fn mcp_import_transport(import: &HarnessMcpImport) -> &'static str {
    match import {
        HarnessMcpImport::Stdio { .. } => "stdio",
        HarnessMcpImport::Http { .. } => "http",
    }
}

fn mcp_import_safe_endpoint(import: &HarnessMcpImport) -> Option<String> {
    match import {
        HarnessMcpImport::Http { url, .. } => Some(safe_http_endpoint(url)),
        HarnessMcpImport::Stdio { .. } => None,
    }
}

fn safe_http_endpoint(url: &str) -> String {
    if let Ok(mut parsed) = reqwest::Url::parse(url) {
        let _ = parsed.set_username("");
        let _ = parsed.set_password(None);
        parsed.set_query(None);
        parsed.set_fragment(None);
        return parsed.to_string();
    }
    url.split(['?', '#']).next().unwrap_or(url).to_string()
}

fn sanitized_http_mcp_error(context: &str, url: &str, err: reqwest::Error) -> anyhow::Error {
    let category = http_mcp_failure_category(&err);
    let mut details = Vec::new();
    if let Some(status) = err.status() {
        details.push(format!("status {status}"));
    }
    if err.is_timeout() {
        details.push("timeout".into());
    }
    if err.is_connect() {
        details.push("connection failed".into());
    }
    if err.is_decode() {
        details.push("decode failed".into());
    }
    if err.is_body() {
        details.push("body read failed".into());
    }
    if err.is_request() {
        details.push("request failed".into());
    }
    let detail = if details.is_empty() {
        "request failed".to_string()
    } else {
        details.join(", ")
    };
    classified_mcp_import_error(
        category,
        format!(
            "{context} for MCP endpoint `{}` failed: {detail}",
            safe_http_endpoint(url)
        ),
    )
}

fn http_mcp_failure_category(err: &reqwest::Error) -> ActionFailureCategory {
    if err.is_timeout() {
        ActionFailureCategory::Timeout
    } else if err.is_decode() || err.is_body() {
        ActionFailureCategory::MalformedOutput
    } else {
        ActionFailureCategory::Runtime
    }
}

fn failed_import_snapshot(
    server_id: &str,
    transport: &str,
    scopes: &[String],
    endpoint: Option<&str>,
    reason: String,
) -> McpImportRuntimeSnapshot {
    McpImportRuntimeSnapshot {
        server_id: server_id.into(),
        tool_name: String::new(),
        identity: format!("mcp:{server_id}"),
        description: format!("Unavailable MCP import `{server_id}`."),
        input_schema: default_input_schema(),
        transport: transport.into(),
        scopes: scopes.to_vec(),
        endpoint: endpoint.map(str::to_string),
        state: "unavailable".into(),
        readiness_reason: Some(reason),
        source: "harness_config".into(),
    }
}

fn unavailable_tool_snapshot(
    server_id: &str,
    tool_name: &str,
    transport: &str,
    scopes: &[String],
    endpoint: Option<&str>,
    state: &str,
    reason: String,
) -> McpImportRuntimeSnapshot {
    McpImportRuntimeSnapshot {
        server_id: server_id.into(),
        tool_name: tool_name.into(),
        identity: imported_mcp_identity(server_id, tool_name),
        description: format!("Unavailable MCP Tool `{tool_name}` from `{server_id}`."),
        input_schema: default_input_schema(),
        transport: transport.into(),
        scopes: scopes.to_vec(),
        endpoint: endpoint.map(str::to_string),
        state: state.into(),
        readiness_reason: Some(reason),
        source: "harness_config".into(),
    }
}

pub fn imported_mcp_identity(server_id: &str, tool_name: &str) -> String {
    format!("mcp:{server_id}/{tool_name}")
}

fn default_input_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": true
    })
}

#[derive(Debug, Deserialize)]
struct ToolsListResult {
    tools: Vec<DiscoveredMcpTool>,
}

#[derive(Debug, Deserialize)]
struct DiscoveredMcpTool {
    name: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(rename = "inputSchema", default)]
    input_schema: Option<Value>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Read;
    use std::net::{TcpListener, TcpStream};
    use std::sync::Arc;
    use std::thread;

    #[test]
    fn stdio_import_discovers_filtered_tool_and_dispatches_call() {
        let temp =
            std::env::temp_dir().join(format!("agentpm-mcp-import-test-{}", std::process::id()));
        fs::create_dir_all(&temp).unwrap();
        let script = temp.join("mcp_fixture.py");
        fs::write(&script, MCP_FIXTURE).unwrap();
        let mut imports = HashMap::new();
        imports.insert(
            "search".into(),
            HarnessMcpImport::Stdio {
                command: std::env::var("AGENTPM_TEST_PYTHON").unwrap_or_else(|_| "python3".into()),
                args: vec![script.display().to_string()],
                cwd: None,
                env: Vec::new(),
                scope: HarnessMcpScope::Phases {
                    phases: vec!["investigate".into()],
                },
                tools: Some(vec!["lookup".into()]),
                startup_timeout_ms: 5_000,
                request_timeout_ms: 5_000,
                restart: Default::default(),
            },
        );

        let activation = ConfiguredMcpImportRuntime::start(Path::new("."), &imports);

        assert_eq!(activation.snapshots.len(), 2);
        let available = activation
            .snapshots
            .iter()
            .find(|snapshot| snapshot.identity == "mcp:search/lookup")
            .unwrap();
        assert_eq!(available.state, "available");
        let suppressed = activation
            .snapshots
            .iter()
            .find(|snapshot| snapshot.identity == "mcp:search/ignored")
            .unwrap();
        assert_eq!(suppressed.state, "suppressed");
        assert_eq!(
            activation.capability_candidates[0].scope,
            "phase:investigate"
        );
        let runtime = Arc::new(std::sync::Mutex::new(activation.runtime));
        let result =
            runtime
                .lock()
                .unwrap()
                .call_tool("search", "lookup", &json!({ "query": "launch" }));
        assert!(result.ok, "{result:?}");
        assert_eq!(result.output["structuredContent"]["query"], json!("launch"));

        fs::remove_dir_all(temp).unwrap();
    }

    #[test]
    fn stdio_import_reports_missing_filtered_tool_without_suppressing_present_tool() {
        let temp = std::env::temp_dir().join(format!(
            "agentpm-mcp-import-missing-test-{}",
            std::process::id()
        ));
        fs::create_dir_all(&temp).unwrap();
        let script = temp.join("mcp_fixture.py");
        fs::write(&script, MCP_FIXTURE).unwrap();
        let mut imports = HashMap::new();
        imports.insert(
            "search".into(),
            HarnessMcpImport::Stdio {
                command: std::env::var("AGENTPM_TEST_PYTHON").unwrap_or_else(|_| "python3".into()),
                args: vec![script.display().to_string()],
                cwd: None,
                env: Vec::new(),
                scope: HarnessMcpScope::Global,
                tools: Some(vec!["lookup".into(), "missing".into()]),
                startup_timeout_ms: 5_000,
                request_timeout_ms: 5_000,
                restart: Default::default(),
            },
        );

        let activation = ConfiguredMcpImportRuntime::start(Path::new("."), &imports);

        assert!(
            activation
                .snapshots
                .iter()
                .any(|snapshot| snapshot.identity == "mcp:search/lookup"
                    && snapshot.state == "available")
        );
        let missing = activation
            .snapshots
            .iter()
            .find(|snapshot| snapshot.identity == "mcp:search/missing")
            .unwrap();
        assert_eq!(missing.state, "unavailable");
        assert!(
            missing
                .readiness_reason
                .as_deref()
                .unwrap_or_default()
                .contains("was not advertised")
        );
        assert_eq!(activation.capability_candidates.len(), 1);

        fs::remove_dir_all(temp).unwrap();
    }

    #[test]
    fn stdio_import_call_timeout_reports_timeout_category() {
        let temp = std::env::temp_dir().join(format!(
            "agentpm-mcp-import-timeout-test-{}",
            std::process::id()
        ));
        fs::create_dir_all(&temp).unwrap();
        let script = temp.join("mcp_timeout_fixture.py");
        fs::write(&script, MCP_TIMEOUT_FIXTURE).unwrap();
        let mut imports = HashMap::new();
        imports.insert(
            "search".into(),
            HarnessMcpImport::Stdio {
                command: std::env::var("AGENTPM_TEST_PYTHON").unwrap_or_else(|_| "python3".into()),
                args: vec![script.display().to_string()],
                cwd: None,
                env: Vec::new(),
                scope: HarnessMcpScope::Global,
                tools: Some(vec!["lookup".into()]),
                startup_timeout_ms: 5_000,
                request_timeout_ms: 25,
                restart: HarnessRestartPolicy {
                    max_attempts: 1,
                    backoff_ms: 0,
                },
            },
        );

        let activation = ConfiguredMcpImportRuntime::start(Path::new("."), &imports);
        let mut runtime = activation.runtime;
        let result = runtime.call_tool("search", "lookup", &json!({ "query": "launch" }));

        assert!(!result.ok, "{result:?}");
        assert_eq!(
            result.failure_category,
            Some(ActionFailureCategory::Timeout)
        );
        fs::remove_dir_all(temp).unwrap();
    }

    #[test]
    fn stdio_import_stop_cleans_up_session_owned_clients() {
        let temp = std::env::temp_dir().join(format!(
            "agentpm-mcp-import-cleanup-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&temp).unwrap();
        let script = temp.join("mcp_fixture.py");
        fs::write(&script, MCP_FIXTURE).unwrap();
        let mut imports = HashMap::new();
        imports.insert(
            "search".into(),
            HarnessMcpImport::Stdio {
                command: std::env::var("AGENTPM_TEST_PYTHON").unwrap_or_else(|_| "python3".into()),
                args: vec![script.display().to_string()],
                cwd: None,
                env: Vec::new(),
                scope: HarnessMcpScope::Global,
                tools: Some(vec!["lookup".into()]),
                startup_timeout_ms: 5_000,
                request_timeout_ms: 5_000,
                restart: Default::default(),
            },
        );

        let activation = ConfiguredMcpImportRuntime::start(Path::new("."), &imports);
        let mut runtime = activation.runtime;
        assert_eq!(runtime.clients.len(), 1);

        runtime.stop();

        assert!(runtime.clients.is_empty());
        let result = runtime.call_tool("search", "lookup", &json!({ "query": "launch" }));
        assert!(!result.ok);
        assert!(
            result
                .error
                .as_deref()
                .unwrap_or_default()
                .contains("is not available")
        );

        fs::remove_dir_all(temp).unwrap();
    }

    #[test]
    fn header_env_values_resolve_from_scoped_environment_map() {
        let mut headers = HashMap::new();
        headers.insert(
            "Authorization".into(),
            HarnessMcpHeaderValue::Env {
                env: "MCP_AUTHORIZATION".into(),
            },
        );
        headers.insert(
            "X-Workspace".into(),
            HarnessMcpHeaderValue::Value {
                value: "support".into(),
            },
        );
        let env = HashMap::from([("MCP_AUTHORIZATION".into(), "Bearer secret-token".into())]);

        let resolved = resolve_headers("company-search", &headers, &env).unwrap();

        assert_eq!(
            resolved.get("Authorization").map(String::as_str),
            Some("Bearer secret-token")
        );
        assert_eq!(
            resolved.get("X-Workspace").map(String::as_str),
            Some("support")
        );
    }

    #[test]
    fn missing_header_env_reports_name_without_secret_value() {
        let mut headers = HashMap::new();
        headers.insert(
            "Authorization".into(),
            HarnessMcpHeaderValue::Env {
                env: "MCP_AUTHORIZATION".into(),
            },
        );

        let err = resolve_headers("company-search", &headers, &HashMap::new()).unwrap_err();
        let text = format!("{err:#}");

        assert!(text.contains("MCP_AUTHORIZATION"));
        assert!(!text.contains("Bearer"));
        assert!(!text.contains("secret-token"));
    }

    #[test]
    fn http_import_safe_endpoint_strips_credentials_query_and_fragment() {
        assert_eq!(
            safe_http_endpoint("https://user:secret@mcp.example.com/mcp?token=secret#frag"),
            "https://mcp.example.com/mcp"
        );
    }

    #[test]
    fn http_import_failure_readiness_reason_omits_url_secrets() {
        let mut imports = HashMap::new();
        imports.insert(
            "search".into(),
            HarnessMcpImport::Http {
                url: "http://user:secret@127.0.0.1:1/mcp?token=secret-token#frag".into(),
                headers: HashMap::new(),
                scope: HarnessMcpScope::Global,
                tools: None,
            },
        );

        let activation =
            ConfiguredMcpImportRuntime::start_with_env(Path::new("."), &imports, &HashMap::new());

        assert_eq!(activation.snapshots.len(), 1);
        let snapshot = &activation.snapshots[0];
        assert_eq!(snapshot.endpoint.as_deref(), Some("http://127.0.0.1:1/mcp"));
        let reason = snapshot.readiness_reason.as_deref().unwrap_or_default();
        assert!(reason.contains("http://127.0.0.1:1/mcp"));
        assert!(!reason.contains("user"));
        assert!(!reason.contains("secret"));
        assert!(!reason.contains("token"));
        assert!(!reason.contains("frag"));
    }

    #[test]
    fn http_import_discovers_filtered_tool_with_env_header_and_dispatches_call() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/mcp", listener.local_addr().unwrap());
        let server = thread::spawn(move || {
            for _ in 0..3 {
                let (mut stream, _) = listener.accept().unwrap();
                let (headers, body) = read_http_request(&mut stream);
                assert!(headers.contains("authorization: bearer secret-token"));
                let request: Value = serde_json::from_str(&body).unwrap();
                let method = request["method"].as_str().unwrap_or_default();
                let result = match method {
                    "initialize" => json!({ "protocolVersion": "2025-06-18" }),
                    "tools/list" => json!({
                        "tools": [
                            {
                                "name": "lookup",
                                "description": "Lookup launch readiness.",
                                "inputSchema": {
                                    "type": "object",
                                    "additionalProperties": false,
                                    "properties": { "query": { "type": "string" } },
                                    "required": ["query"]
                                }
                            }
                        ]
                    }),
                    "tools/call" => json!({
                        "content": [{ "type": "text", "text": "ok" }],
                        "structuredContent": {
                            "query": request["params"]["arguments"]["query"].clone()
                        },
                        "isError": false
                    }),
                    _ => json!({}),
                };
                write_http_json(
                    &mut stream,
                    &json!({
                        "jsonrpc": "2.0",
                        "id": request["id"].clone(),
                        "result": result
                    }),
                );
            }
        });

        let mut imports = HashMap::new();
        imports.insert(
            "search".into(),
            HarnessMcpImport::Http {
                url: url.clone(),
                headers: HashMap::from([(
                    "Authorization".into(),
                    HarnessMcpHeaderValue::Env {
                        env: "MCP_AUTHORIZATION".into(),
                    },
                )]),
                scope: HarnessMcpScope::Global,
                tools: Some(vec!["lookup".into()]),
            },
        );
        let env = HashMap::from([("MCP_AUTHORIZATION".into(), "Bearer secret-token".into())]);

        let activation = ConfiguredMcpImportRuntime::start_with_env(Path::new("."), &imports, &env);

        assert_eq!(activation.snapshots.len(), 1);
        assert_eq!(activation.snapshots[0].transport, "http");
        assert_eq!(activation.snapshots[0].identity, "mcp:search/lookup");
        assert_eq!(activation.snapshots[0].scopes, vec!["global".to_string()]);
        assert_eq!(
            activation.snapshots[0].endpoint.as_deref(),
            Some(url.as_str())
        );
        let mut runtime = activation.runtime;
        let result = runtime.call_tool("search", "lookup", &json!({ "query": "launch" }));
        assert!(result.ok, "{result:?}");
        assert_eq!(result.output["structuredContent"]["query"], json!("launch"));
        server.join().unwrap();
    }

    #[test]
    fn http_import_call_decode_failure_reports_malformed_output_category() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/mcp", listener.local_addr().unwrap());
        let server = thread::spawn(move || {
            for _ in 0..3 {
                let (mut stream, _) = listener.accept().unwrap();
                let (_headers, body) = read_http_request(&mut stream);
                let request: Value = serde_json::from_str(&body).unwrap();
                let method = request["method"].as_str().unwrap_or_default();
                match method {
                    "initialize" => write_http_json(
                        &mut stream,
                        &json!({
                            "jsonrpc": "2.0",
                            "id": request["id"].clone(),
                            "result": { "protocolVersion": "2025-06-18" }
                        }),
                    ),
                    "tools/list" => write_http_json(
                        &mut stream,
                        &json!({
                            "jsonrpc": "2.0",
                            "id": request["id"].clone(),
                            "result": {
                                "tools": [
                                    {
                                        "name": "lookup",
                                        "description": "Lookup launch readiness.",
                                        "inputSchema": {
                                            "type": "object",
                                            "additionalProperties": false,
                                            "properties": { "query": { "type": "string" } },
                                            "required": ["query"]
                                        }
                                    }
                                ]
                            }
                        }),
                    ),
                    "tools/call" => {
                        let payload = b"{not-json";
                        write!(
                            stream,
                            "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\n\r\n",
                            payload.len()
                        )
                        .unwrap();
                        stream.write_all(payload).unwrap();
                    }
                    _ => unreachable!("unexpected method {method}"),
                }
            }
        });

        let mut imports = HashMap::new();
        imports.insert(
            "search".into(),
            HarnessMcpImport::Http {
                url: url.clone(),
                headers: HashMap::new(),
                scope: HarnessMcpScope::Global,
                tools: Some(vec!["lookup".into()]),
            },
        );

        let activation =
            ConfiguredMcpImportRuntime::start_with_env(Path::new("."), &imports, &HashMap::new());
        let mut runtime = activation.runtime;
        let result = runtime.call_tool("search", "lookup", &json!({ "query": "launch" }));

        assert!(!result.ok, "{result:?}");
        assert_eq!(
            result.failure_category,
            Some(ActionFailureCategory::MalformedOutput)
        );
        server.join().unwrap();
    }

    #[test]
    fn http_import_timeout_error_reports_timeout_category() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/mcp", listener.local_addr().unwrap());
        let server = thread::spawn(move || {
            let (_stream, _) = listener.accept().unwrap();
            thread::sleep(Duration::from_millis(150));
        });
        let client = Client::builder()
            .timeout(Duration::from_millis(20))
            .build()
            .unwrap();
        let err = client.post(&url).json(&json!({})).send().unwrap_err();

        let sanitized = sanitized_http_mcp_error("sending MCP HTTP JSON-RPC request", &url, err);

        assert_eq!(
            mcp_import_call_failure_category(&sanitized),
            ActionFailureCategory::Timeout
        );
        server.join().unwrap();
    }

    #[test]
    fn stdio_import_restart_does_not_replay_failed_in_flight_tool_call() {
        let temp = std::env::temp_dir().join(format!(
            "agentpm-mcp-import-restart-test-{}",
            std::process::id()
        ));
        fs::create_dir_all(&temp).unwrap();
        let script = temp.join("mcp_restart_fixture.py");
        let marker = temp.join("failed-once");
        fs::write(&script, MCP_RESTART_FIXTURE).unwrap();
        let mut imports = HashMap::new();
        imports.insert(
            "search".into(),
            HarnessMcpImport::Stdio {
                command: std::env::var("AGENTPM_TEST_PYTHON").unwrap_or_else(|_| "python3".into()),
                args: vec![script.display().to_string(), marker.display().to_string()],
                cwd: None,
                env: Vec::new(),
                scope: HarnessMcpScope::Global,
                tools: Some(vec!["lookup".into()]),
                startup_timeout_ms: 5_000,
                request_timeout_ms: 5_000,
                restart: HarnessRestartPolicy {
                    max_attempts: 1,
                    backoff_ms: 0,
                },
            },
        );

        let activation = ConfiguredMcpImportRuntime::start(Path::new("."), &imports);
        let mut runtime = activation.runtime;

        let failed = runtime.call_tool("search", "lookup", &json!({ "query": "first" }));
        assert!(!failed.ok, "{failed:?}");
        assert_eq!(
            failed.failure_category,
            Some(ActionFailureCategory::SubprocessFailure)
        );
        assert!(
            failed
                .error
                .as_deref()
                .unwrap_or_default()
                .contains("MCP stdio server closed stdout"),
            "{failed:?}"
        );

        let succeeded = runtime.call_tool("search", "lookup", &json!({ "query": "second" }));
        assert!(succeeded.ok, "{succeeded:?}");
        assert_eq!(
            succeeded.output["structuredContent"]["query"],
            json!("second")
        );

        fs::remove_dir_all(temp).unwrap();
    }

    fn read_http_request(stream: &mut TcpStream) -> (String, String) {
        let mut buffer = Vec::new();
        let mut temp = [0; 1024];
        loop {
            let read = stream.read(&mut temp).unwrap();
            assert_ne!(read, 0, "HTTP client closed before headers");
            buffer.extend_from_slice(&temp[..read]);
            if buffer.windows(4).any(|window| window == b"\r\n\r\n") {
                break;
            }
        }
        let header_end = buffer
            .windows(4)
            .position(|window| window == b"\r\n\r\n")
            .unwrap()
            + 4;
        let headers = String::from_utf8_lossy(&buffer[..header_end]).to_lowercase();
        let content_length = headers
            .lines()
            .find_map(|line| line.strip_prefix("content-length: "))
            .and_then(|value| value.trim().parse::<usize>().ok())
            .unwrap_or(0);
        while buffer.len() < header_end + content_length {
            let read = stream.read(&mut temp).unwrap();
            assert_ne!(read, 0, "HTTP client closed before body");
            buffer.extend_from_slice(&temp[..read]);
        }
        let body =
            String::from_utf8(buffer[header_end..header_end + content_length].to_vec()).unwrap();
        (headers, body)
    }

    fn write_http_json(stream: &mut TcpStream, value: &Value) {
        let body = value.to_string();
        write!(
            stream,
            "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\n\r\n{}",
            body.len(),
            body
        )
        .unwrap();
        stream.flush().unwrap();
    }

    const MCP_FIXTURE: &str = r#"
import json
import sys

for line in sys.stdin:
    request = json.loads(line)
    method = request.get("method")
    if method == "initialize":
        result = {"protocolVersion": "2025-06-18"}
    elif method == "tools/list":
        result = {"tools": [
            {
                "name": "lookup",
                "description": "Lookup launch readiness.",
                "inputSchema": {
                    "type": "object",
                    "additionalProperties": False,
                    "properties": {"query": {"type": "string"}},
                    "required": ["query"],
                },
            },
            {"name": "ignored", "inputSchema": {"type": "object"}},
        ]}
    elif method == "tools/call":
        result = {
            "content": [{"type": "text", "text": "ok"}],
            "structuredContent": {
                "query": request.get("params", {}).get("arguments", {}).get("query")
            },
            "isError": False,
        }
    else:
        result = {}
    print(json.dumps({"jsonrpc": "2.0", "id": request.get("id"), "result": result}), flush=True)
"#;

    const MCP_RESTART_FIXTURE: &str = r#"
import json
import sys
from pathlib import Path

marker = Path(sys.argv[1])

for line in sys.stdin:
    request = json.loads(line)
    method = request.get("method")
    if method == "initialize":
        result = {"protocolVersion": "2025-06-18"}
    elif method == "tools/list":
        result = {"tools": [
            {
                "name": "lookup",
                "description": "Lookup launch readiness.",
                "inputSchema": {
                    "type": "object",
                    "additionalProperties": False,
                    "properties": {"query": {"type": "string"}},
                    "required": ["query"],
                },
            }
        ]}
    elif method == "tools/call":
        if not marker.exists():
            marker.write_text("failed")
            sys.exit(1)
        result = {
            "content": [{"type": "text", "text": "ok"}],
            "structuredContent": {
                "query": request.get("params", {}).get("arguments", {}).get("query")
            },
            "isError": False,
        }
    else:
        result = {}
    print(json.dumps({"jsonrpc": "2.0", "id": request.get("id"), "result": result}), flush=True)
"#;

    const MCP_TIMEOUT_FIXTURE: &str = r#"
import json
import sys
import time

for line in sys.stdin:
    request = json.loads(line)
    method = request.get("method")
    if method == "initialize":
        result = {"protocolVersion": "2025-06-18"}
    elif method == "tools/list":
        result = {"tools": [
            {
                "name": "lookup",
                "description": "Lookup launch readiness.",
                "inputSchema": {
                    "type": "object",
                    "additionalProperties": False,
                    "properties": {"query": {"type": "string"}},
                    "required": ["query"],
                },
            }
        ]}
    elif method == "tools/call":
        time.sleep(0.2)
        result = {"content": [], "isError": False}
    else:
        result = {}
    print(json.dumps({"jsonrpc": "2.0", "id": request.get("id"), "result": result}), flush=True)
"#;
}
