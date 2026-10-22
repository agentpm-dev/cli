use crate::harness_observability::HarnessTerminalStatus;
use rand::TryRngCore;
use serde::{Deserialize, Serialize};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{OnceLock, mpsc};
use std::time::Duration;

pub const DISCLOSURE: &str = "AgentPM sends minimal anonymous CLI/Harness usage events by default. No prompts, Tool I/O, paths, credentials, or package identities. Disable with AGENTPM_TELEMETRY=0 or [telemetry] enabled = false in config.toml. Privacy: https://www.agentpackagemanager.com/privacy";
static CLIENT: OnceLock<Telemetry> = OnceLock::new();

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeMode {
    Tui,
    Headless,
    Machine,
}

// Only typed, content-free facts can reach the serializer. Never accept maps
// or Harness envelopes here: those contain private input and output.
#[derive(Clone, Copy, Debug, Serialize)]
#[serde(tag = "event", content = "properties", rename_all = "snake_case")]
enum Event {
    CliFirstUsed(Platform),
    HarnessStarted {
        #[serde(flatten)]
        platform: Platform,
        runtime_mode: RuntimeMode,
    },
    HarnessCompleted {
        #[serde(flatten)]
        platform: Platform,
        runtime_mode: RuntimeMode,
        terminal_outcome: HarnessTerminalStatus,
    },
}

#[derive(Clone, Copy, Debug, Serialize)]
struct Platform {
    cli_version: &'static str,
    os_family: &'static str,
    architecture: &'static str,
}

impl Platform {
    fn current() -> Self {
        Self {
            cli_version: env!("CARGO_PKG_VERSION"),
            os_family: match std::env::consts::OS {
                "macos" => "macos",
                "windows" => "windows",
                "linux" => "linux",
                _ => "other",
            },
            architecture: match std::env::consts::ARCH {
                "x86_64" => "x86_64",
                "aarch64" => "aarch64",
                "x86" => "x86",
                "arm" => "arm",
                _ => "other",
            },
        }
    }
}

#[derive(Serialize)]
struct Payload<'a> {
    install_id: &'a str,
    #[serde(flatten)]
    event: Event,
}

#[derive(Clone)]
struct Telemetry {
    config_dir: PathBuf,
    sender: mpsc::SyncSender<Event>,
    mode: Option<RuntimeMode>,
}

#[derive(Deserialize, Default)]
struct FileSettings {
    telemetry: Option<Settings>,
}

#[derive(Deserialize)]
struct Settings {
    enabled: bool,
}

fn enabled(config_dir: &Path) -> bool {
    policy_enabled(config_dir, std::env::var_os("AGENTPM_TELEMETRY").as_deref())
}

fn policy_enabled(config_dir: &Path, setting: Option<&std::ffi::OsStr>) -> bool {
    if setting == Some(std::ffi::OsStr::new("0")) {
        return false;
    }
    match fs::read_to_string(config_dir.join("config.toml")) {
        Ok(text) => toml::from_str::<FileSettings>(&text)
            .map(|file| file.telemetry.is_none_or(|settings| settings.enabled))
            .unwrap_or(false),
        Err(error) => error.kind() == std::io::ErrorKind::NotFound,
    }
}

fn random_id() -> Option<String> {
    let mut bytes = [0; 16];
    rand::rngs::OsRng.try_fill_bytes(&mut bytes).ok()?;
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    let encoded = hex::encode(bytes);
    Some(format!(
        "{}-{}-{}-{}-{}",
        &encoded[..8],
        &encoded[8..12],
        &encoded[12..16],
        &encoded[16..20],
        &encoded[20..]
    ))
}

fn valid_id(value: &str) -> bool {
    value.len() == 36
        && value.bytes().enumerate().all(|(index, byte)| {
            if [8, 13, 18, 23].contains(&index) {
                byte == b'-'
            } else {
                byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)
            }
        })
        && value.as_bytes()[14] == b'4'
        && matches!(value.as_bytes()[19], b'8' | b'9' | b'a' | b'b')
}

fn install_id(config_dir: &Path) -> Option<String> {
    fs::create_dir_all(config_dir).ok()?;
    let path = config_dir.join("telemetry-id");
    if !path.exists() {
        let id = random_id()?;
        let temporary = config_dir.join(format!(".telemetry-{id}"));
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(&temporary).ok()?;
        let written = file.write_all(id.as_bytes());
        drop(file);
        // Publish a complete ID without ever replacing another process's ID.
        let linked = written.and_then(|()| fs::hard_link(&temporary, &path));
        let _ = fs::remove_file(temporary);
        if let Err(error) = linked
            && error.kind() != std::io::ErrorKind::AlreadyExists
        {
            return None;
        }
    }
    let id = fs::read_to_string(path).ok()?;
    valid_id(&id).then_some(id)
}

fn endpoint(base_url: &str) -> Option<reqwest::Url> {
    let mut url = reqwest::Url::parse(base_url).ok()?;
    if !matches!(url.scheme(), "https" | "http")
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return None;
    }
    url.set_path(&format!(
        "{}/analytics/cli",
        url.path().trim_end_matches('/')
    ));
    Some(url)
}

impl Telemetry {
    fn start(config_dir: PathBuf, base_url: &str, mode: Option<RuntimeMode>) -> Option<Self> {
        if !enabled(&config_dir) {
            return None;
        }
        let endpoint = endpoint(base_url)?;
        let id = install_id(&config_dir)?;
        let (sender, receiver) = mpsc::sync_channel::<Event>(32);
        let worker_dir = config_dir.clone();
        // Deliberately not a Tokio blocking task: runtime shutdown joins those.
        // Dropping this JoinHandle detaches; there is no flush or exit hook.
        std::thread::Builder::new()
            .name("agentpm-telemetry".into())
            .spawn(move || {
                let Ok(client) = reqwest::blocking::Client::builder()
                    .timeout(Duration::from_secs(1))
                    .connect_timeout(Duration::from_millis(300))
                    .redirect(reqwest::redirect::Policy::none())
                    .no_proxy()
                    .build()
                else {
                    return;
                };
                for event in receiver {
                    if enabled(&worker_dir) {
                        let _ = client
                            .post(endpoint.clone())
                            .json(&Payload {
                                install_id: &id,
                                event,
                            })
                            .send();
                    }
                }
            })
            .ok()?;
        Some(Self {
            config_dir,
            sender,
            mode,
        })
    }

    fn send(&self, event: Event) -> bool {
        if enabled(&self.config_dir) {
            return self.sender.try_send(event).is_ok();
        }
        false
    }

    fn first_use(&self) {
        if enabled(&self.config_dir)
            && OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(self.config_dir.join("telemetry-first-used"))
                .is_ok()
        {
            use is_terminal::IsTerminal;
            if std::io::stderr().is_terminal() {
                // Disclosure is stderr-only; never corrupt JSON or SDK stdout.
                let _ = writeln!(std::io::stderr(), "{DISCLOSURE}");
            }
            self.send(Event::CliFirstUsed(Platform::current()));
        }
    }
}

pub fn initialize(base_url: &str, mode: Option<RuntimeMode>) {
    if let Some(dirs) = crate::config::project_dirs()
        && let Some(client) = Telemetry::start(dirs.config_dir().into(), base_url, mode)
        && CLIENT.set(client).is_ok()
        && let Some(client) = CLIENT.get()
    {
        client.first_use();
    }
}

#[derive(Default)]
pub struct RunTelemetry {
    client: Option<Telemetry>,
    active: bool,
}

impl RunTelemetry {
    pub fn from_global() -> Self {
        Self {
            client: CLIENT.get().cloned(),
            active: false,
        }
    }

    pub fn started(&mut self) {
        if self.active {
            return;
        }
        if let Some(client) = &self.client
            && let Some(runtime_mode) = client.mode
        {
            self.active = client.send(Event::HarnessStarted {
                platform: Platform::current(),
                runtime_mode,
            });
        }
    }

    pub fn completed(&mut self, terminal_outcome: HarnessTerminalStatus) {
        if !std::mem::take(&mut self.active) {
            return;
        }
        if let Some(client) = &self.client
            && let Some(runtime_mode) = client.mode
        {
            client.send(Event::HarnessCompleted {
                platform: Platform::current(),
                runtime_mode,
                terminal_outcome,
            });
        }
    }

    pub fn abandon(&mut self) {
        self.active = false;
    }
}

#[cfg(test)]
pub(crate) mod tests;
