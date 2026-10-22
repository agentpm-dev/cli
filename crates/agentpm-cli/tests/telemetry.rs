use serde_json::{Value, json};
use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::process::{Command, Output, Stdio};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use std::time::{Duration, Instant};

const FIXTURES: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../specs/2026-10-06-immediate-hardening/manual-tests/fixtures/m10b"
);

struct Project(PathBuf);
impl Project {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "agentpm-cli-telemetry-{}-{}",
            std::process::id(),
            rand::random::<u64>()
        ));
        fs::create_dir_all(&path).unwrap();
        let installed = path.join(".agentpm/loops/fixture/telemetry-loop/0.1.0");
        fs::create_dir_all(&installed).unwrap();
        for (source, target) in [
            ("agent.fixture.json", path.join("agent.json")),
            ("lock.fixture.json", path.join("agent.lock")),
            ("harness.fixture.json", path.join("agentpm.harness.json")),
            ("loop.fixture.json", installed.join("agent.json")),
        ] {
            fs::copy(PathBuf::from(FIXTURES).join(source), target).unwrap();
        }
        Self(path)
    }

    fn command(&self, server: &Server, telemetry: bool) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_agentpm"));
        command
            .current_dir(&self.0)
            .env("HOME", self.0.join("home"))
            .env("USERPROFILE", self.0.join("home"))
            .env("XDG_CONFIG_HOME", self.0.join("home/config"))
            .env("XDG_DATA_HOME", self.0.join("home/data"))
            .env("APPDATA", self.0.join("home/data"))
            .env("AGENTPM_TELEMETRY", if telemetry { "1" } else { "0" })
            .env("AGENTPM_BASE_URL", &server.url)
            .env("AGENTPM_TOKEN", "PRIVATE_PAT")
            .env("OPENAI_API_KEY", "PRIVATE_MODEL_KEY")
            .env("OPENAI_BASE_URL", format!("{}/model", server.url))
            .env("RUST_LOG", "")
            .args([
                "harness",
                "--headless",
                "--input",
                "PRIVATE_PROMPT /private/path @private/package",
            ]);
        command
    }

    fn state(&self) -> Option<PathBuf> {
        walkdir::WalkDir::new(self.0.join("home"))
            .into_iter()
            .filter_map(Result::ok)
            .find(|entry| entry.file_name() == "telemetry-id")
            .map(|entry| entry.path().parent().unwrap().to_path_buf())
    }
}
impl Drop for Project {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

struct Request {
    path: String,
    authorization: Option<String>,
    body: Value,
}

struct Server {
    url: String,
    requests: Arc<Mutex<Vec<Request>>>,
    stop: Arc<AtomicBool>,
    worker: Option<std::thread::JoinHandle<()>>,
}
impl Server {
    fn new(analytics_delay: Duration, model_delay: Duration, model_status: u16) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        listener.set_nonblocking(true).unwrap();
        let requests = Arc::new(Mutex::new(Vec::new()));
        let stop = Arc::new(AtomicBool::new(false));
        let recorded = Arc::clone(&requests);
        let stopped = Arc::clone(&stop);
        let worker = std::thread::spawn(move || {
            let mut workers = Vec::new();
            while !stopped.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((stream, _)) => {
                        let recorded = Arc::clone(&recorded);
                        let stopped = Arc::clone(&stopped);
                        workers.push(std::thread::spawn(move || {
                            handle(
                                stream,
                                recorded,
                                stopped,
                                analytics_delay,
                                model_delay,
                                model_status,
                            )
                        }));
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(2))
                    }
                    Err(error) => panic!("fixture server: {error}"),
                }
            }
            for worker in workers {
                worker.join().unwrap();
            }
        });
        Self {
            url,
            requests,
            stop,
            worker: Some(worker),
        }
    }

    fn events(&self) -> Vec<Value> {
        let requests = self.requests.lock().unwrap();
        requests
            .iter()
            .filter(|request| request.path == "/analytics/cli")
            .map(|request| {
                assert!(request.authorization.is_none());
                assert!(!request.body.to_string().contains("PRIVATE"));
                request.body.clone()
            })
            .collect()
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        self.worker.take().unwrap().join().unwrap();
    }
}

fn handle(
    mut stream: TcpStream,
    recorded: Arc<Mutex<Vec<Request>>>,
    stop: Arc<AtomicBool>,
    analytics_delay: Duration,
    model_delay: Duration,
    model_status: u16,
) {
    stream
        .set_read_timeout(Some(Duration::from_secs(3)))
        .unwrap();
    let mut reader = BufReader::new(stream.try_clone().unwrap());
    let mut line = String::new();
    if reader.read_line(&mut line).is_err() || line.is_empty() {
        return;
    }
    let path = line.split_whitespace().nth(1).unwrap().to_string();
    let mut length = 0;
    let mut authorization = None;
    loop {
        line.clear();
        reader.read_line(&mut line).unwrap();
        if line == "\r\n" || line.is_empty() {
            break;
        }
        let (key, value) = line.split_once(':').unwrap();
        if key.eq_ignore_ascii_case("content-length") {
            length = value.trim().parse::<usize>().unwrap();
        }
        if key.eq_ignore_ascii_case("authorization") {
            authorization = Some(value.trim().to_string());
        }
    }
    let mut bytes = vec![0; length];
    reader.read_exact(&mut bytes).unwrap();
    let body: Value = serde_json::from_slice(&bytes).unwrap();
    let is_model = path == "/model";
    recorded.lock().unwrap().push(Request {
        path,
        authorization,
        body,
    });
    let deadline = Instant::now()
        + if is_model {
            model_delay
        } else {
            analytics_delay
        };
    while Instant::now() < deadline && !stop.load(Ordering::Relaxed) {
        std::thread::sleep(Duration::from_millis(5));
    }
    let (status, response) = if is_model {
        (model_status, json!({"choices":[{"message":{"role":"assistant","content":null,"tool_calls":[{
            "id":"fixture-call","type":"function","function":{"name":"phase_complete","arguments":"{\"outcome\":\"complete\",\"output\":{\"answer\":\"fixture-ok\"}}"}
        }]},"finish_reason":"tool_calls"}]}).to_string())
    } else {
        (204, String::new())
    };
    let _ = write!(
        stream,
        "HTTP/1.1 {status} OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{response}",
        response.len()
    );
}

fn successful(output: &Output) {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        serde_json::from_slice::<Value>(&output.stdout).unwrap(),
        json!({"answer":"fixture-ok"})
    );
}

#[test]
fn real_cli_first_use_is_once_stable_and_harness_events_exclude_credentials_and_input() {
    let project = Project::new();
    let server = Server::new(Duration::ZERO, Duration::from_millis(300), 200);
    for _ in 0..2 {
        successful(&project.command(&server, true).output().unwrap());
    }
    let events = server.events();
    assert_eq!(
        events
            .iter()
            .filter(|event| event["event"] == "cli_first_used")
            .count(),
        1
    );
    assert_eq!(
        events
            .iter()
            .filter(|event| event["event"] == "harness_started")
            .count(),
        2
    );
    let state = project.state().unwrap();
    let identity = fs::read_to_string(state.join("telemetry-id")).unwrap();
    assert!(events.iter().all(|event| event["install_id"] == identity));
    for event in events
        .iter()
        .filter(|event| event["event"] != "cli_first_used")
    {
        assert_eq!(event["properties"]["runtime_mode"], "headless");
    }
    // Completed is best-effort at process exit; the engine-level tests assert
    // the exact pair before transport, not guaranteed network delivery.
    assert!(
        events
            .iter()
            .filter(|event| event["event"] == "harness_completed")
            .count()
            <= 2
    );
    fs::write(state.join("config.toml"), "[telemetry]\nenabled = false\n").unwrap();
    // A separate endpoint cannot be confused with an already in-flight event
    // from a previous process that opted out only after it finished sending.
    let disabled_server = Server::new(Duration::ZERO, Duration::from_millis(300), 200);
    successful(&project.command(&disabled_server, true).output().unwrap());
    assert!(disabled_server.events().is_empty());
}

#[test]
fn opt_out_creates_no_identity_and_preserves_output_and_exit_during_outage() {
    let project = Project::new();
    let server = Server::new(Duration::from_millis(2500), Duration::from_millis(300), 200);
    let disabled = project.command(&server, false).output().unwrap();
    successful(&disabled);
    assert!(project.state().is_none());
    assert!(server.events().is_empty());
    let start = Instant::now();
    let enabled = project.command(&server, true).output().unwrap();
    assert!(
        start.elapsed() < Duration::from_secs(2),
        "telemetry blocked exit"
    );
    successful(&enabled);
    assert_eq!(enabled.status.code(), disabled.status.code());
    assert_eq!(enabled.stdout, disabled.stdout);
    assert_eq!(enabled.stderr, disabled.stderr);
}

#[test]
fn telemetry_does_not_change_failed_harness_exit_or_error_output() {
    let project = Project::new();
    let server = Server::new(Duration::ZERO, Duration::from_millis(300), 500);
    let disabled = project.command(&server, false).output().unwrap();
    let enabled = project.command(&server, true).output().unwrap();
    assert_eq!(disabled.status.code(), Some(1));
    assert_eq!(enabled.status.code(), disabled.status.code());
    assert_eq!(enabled.stdout, disabled.stdout);
    assert_eq!(enabled.stderr, disabled.stderr);
}

#[test]
fn killed_run_has_no_completion_and_no_retry() {
    let project = Project::new();
    let server = Server::new(Duration::ZERO, Duration::from_secs(30), 200);
    let mut child = project
        .command(&server, true)
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    while !server
        .events()
        .iter()
        .any(|event| event["event"] == "harness_started")
    {
        if let Some(status) = child.try_wait().unwrap() {
            let mut stderr = String::new();
            child
                .stderr
                .take()
                .unwrap()
                .read_to_string(&mut stderr)
                .unwrap();
            panic!(
                "Harness exited before start capture: {status}: {stderr}; events={:?}",
                server.events()
            );
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let output = child.wait_with_output().unwrap();
            panic!(
                "start event not received: {}; events={:?}",
                String::from_utf8_lossy(&output.stderr),
                server.events()
            );
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    child.kill().unwrap();
    assert!(!child.wait().unwrap().success());
    let before = server.events();
    assert!(
        !before
            .iter()
            .any(|event| event["event"] == "harness_completed")
    );
    std::thread::sleep(Duration::from_millis(100));
    assert_eq!(server.events(), before);
}

#[test]
fn help_discloses_default_collection_and_opt_out_without_creating_state() {
    let project = Project::new();
    let output = Command::new(env!("CARGO_BIN_EXE_agentpm"))
        .env("HOME", project.0.join("home"))
        .arg("--help")
        .output()
        .unwrap();
    assert!(output.status.success());
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains("by default"));
    assert!(text.contains("AGENTPM_TELEMETRY=0"));
    assert!(project.state().is_none());
}

#[test]
fn manual_fixture_manifests_lint_with_the_real_cli() {
    let project = Project::new();
    for path in [
        project.0.clone(),
        project
            .0
            .join(".agentpm/loops/fixture/telemetry-loop/0.1.0"),
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_agentpm"))
            .env("AGENTPM_TELEMETRY", "0")
            .current_dir(path)
            .args(["lint", "--strict"])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
