use super::*;
use serde_json::{Value, json};

pub(crate) struct Directory(pub PathBuf);

impl Directory {
    pub fn new() -> Self {
        let path =
            std::env::temp_dir().join(format!("agentpm-telemetry-test-{}", random_id().unwrap()));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}

impl Drop for Directory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

pub(crate) fn recorder(
    directory: &Path,
    mode: RuntimeMode,
) -> (RunTelemetry, impl Fn() -> Vec<Value>) {
    let (sender, receiver) = mpsc::sync_channel(32);
    let telemetry = RunTelemetry {
        client: Some(Telemetry {
            config_dir: directory.into(),
            sender,
            mode: Some(mode),
        }),
        active: false,
    };
    let drain = move || {
        receiver
            .try_iter()
            .map(|event| serde_json::to_value(event).unwrap())
            .collect()
    };
    (telemetry, drain)
}

#[test]
fn install_id_is_random_stable_and_atomic_under_concurrent_initialization() {
    let directory = Directory::new();
    let handles: Vec<_> = (0..12)
        .map(|_| {
            let path = directory.0.clone();
            std::thread::spawn(move || install_id(&path).unwrap())
        })
        .collect();
    let ids: Vec<_> = handles
        .into_iter()
        .map(|thread| thread.join().unwrap())
        .collect();
    assert!(valid_id(&ids[0]));
    assert!(ids.iter().all(|id| id == &ids[0]));
    assert_eq!(install_id(&directory.0).unwrap(), ids[0]);
    let other = Directory::new();
    assert_ne!(install_id(&other.0).unwrap(), ids[0]);
    fs::write(directory.0.join("telemetry-id"), "PRIVATE_PATH_OR_USERNAME").unwrap();
    assert!(install_id(&directory.0).is_none());
}

#[test]
fn disabled_invalid_or_unreadable_config_fails_closed_without_state_creation() {
    let directory = Directory::new();
    assert!(policy_enabled(&directory.0, None));
    assert!(!policy_enabled(
        &directory.0,
        Some(std::ffi::OsStr::new("0"))
    ));
    let path = directory.0.join("config.toml");
    fs::write(
        &path,
        "base_url = 'https://example.test'\n[telemetry]\nenabled = false\n",
    )
    .unwrap();
    assert!(!policy_enabled(&directory.0, None));
    assert!(Telemetry::start(directory.0.clone(), "http://127.0.0.1:9", None).is_none());
    assert!(!directory.0.join("telemetry-id").exists());
    assert!(!directory.0.join("telemetry-first-used").exists());
    fs::write(&path, "not = [valid toml").unwrap();
    assert!(!policy_enabled(&directory.0, None));
    fs::remove_file(&path).unwrap();
    fs::create_dir(&path).unwrap();
    assert!(!policy_enabled(&directory.0, None));
}

#[test]
fn first_use_is_attempted_once_per_installation_even_across_versions_and_processes() {
    let directory = Directory::new();
    let (run, drain) = recorder(&directory.0, RuntimeMode::Headless);
    let client = run.client.unwrap();
    let threads: Vec<_> = (0..12)
        .map(|_| {
            let client = client.clone();
            std::thread::spawn(move || client.first_use())
        })
        .collect();
    for thread in threads {
        thread.join().unwrap();
    }
    assert_eq!(drain().len(), 1);
    client.first_use();
    assert!(drain().is_empty());
    // Claim state has no CLI version: an upgrade does not reset adoption.
    assert_eq!(
        fs::read(directory.0.join("telemetry-first-used")).unwrap(),
        b""
    );
}

#[test]
fn closed_payload_and_per_run_events_are_identical_across_surfaces() {
    let mut baseline = None;
    for mode in [
        RuntimeMode::Tui,
        RuntimeMode::Headless,
        RuntimeMode::Machine,
    ] {
        let directory = Directory::new();
        let (mut run, drain) = recorder(&directory.0, mode);
        run.completed(HarnessTerminalStatus::Ended);
        for outcome in [HarnessTerminalStatus::Ended, HarnessTerminalStatus::Failed] {
            run.started();
            run.started();
            run.completed(outcome);
            run.completed(outcome);
        }
        let mut events = drain();
        assert_eq!(events.len(), 4);
        for event in &mut events {
            assert_eq!(event.as_object().unwrap().len(), 2);
            let props = event["properties"].as_object_mut().unwrap();
            assert_eq!(props.remove("runtime_mode"), Some(json!(mode)));
            let expected = if props.contains_key("terminal_outcome") {
                4
            } else {
                3
            };
            assert_eq!(props.len(), expected);
            assert_eq!(props["cli_version"], env!("CARGO_PKG_VERSION"));
        }
        if let Some(expected) = &baseline {
            assert_eq!(&events, expected);
        } else {
            baseline = Some(events);
        }
    }
}

#[test]
fn all_terminal_outcomes_are_typed_and_mid_session_opt_out_suppresses_events() {
    let directory = Directory::new();
    let (mut run, drain) = recorder(&directory.0, RuntimeMode::Tui);
    for outcome in [
        HarnessTerminalStatus::Ended,
        HarnessTerminalStatus::Aborted,
        HarnessTerminalStatus::HandedOff,
        HarnessTerminalStatus::Failed,
        HarnessTerminalStatus::Cancelled,
        HarnessTerminalStatus::LimitReached,
        HarnessTerminalStatus::ApprovalRequired,
    ] {
        run.started();
        run.completed(outcome);
    }
    assert_eq!(drain().len(), 14);
    run.started();
    fs::write(
        directory.0.join("config.toml"),
        "[telemetry]\nenabled = false\n",
    )
    .unwrap();
    run.completed(HarnessTerminalStatus::Ended);
    run.started();
    run.completed(HarnessTerminalStatus::Ended);
    assert_eq!(drain().len(), 1);
}

#[test]
fn panic_or_drop_leaves_a_start_without_completion_and_full_queue_never_blocks() {
    let directory = Directory::new();
    let (mut run, drain) = recorder(&directory.0, RuntimeMode::Machine);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        run.started();
        panic!("simulated private runtime failure");
    }));
    assert!(result.is_err());
    drop(run);
    let events = drain();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0]["event"], "harness_started");
    let (mut run, drain) = recorder(&directory.0, RuntimeMode::Machine);
    for _ in 0..1000 {
        run.started();
        run.completed(HarnessTerminalStatus::Ended);
    }
    assert_eq!(drain().len(), 32);
    drop(drain);
    run.started(); // A disconnected sender is also harmless.
    run.completed(HarnessTerminalStatus::Failed);
}

#[test]
fn telemetry_endpoint_preserves_registry_prefix_and_rejects_credentials_or_queries() {
    assert_eq!(
        endpoint("https://example.test/api/").unwrap().as_str(),
        "https://example.test/api/analytics/cli"
    );
    for base in [
        "file:///tmp/private",
        "https://token@example.test",
        "https://example.test?secret=a",
        "https://example.test#private",
    ] {
        assert!(endpoint(base).is_none());
    }
}
