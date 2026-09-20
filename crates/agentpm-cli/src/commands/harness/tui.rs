use super::HarnessArgs;
use crate::harness_config::{
    HarnessModelConfig, HarnessTraceContent, HarnessTraceLevel, is_built_in_model_provider,
};
use crate::harness_observability::{
    HarnessEventEnvelope, HarnessEventPayload, HarnessEventType, HarnessTerminalStatus, RunReport,
    allocate_harness_run_id,
};
use crate::harness_plan::{CapabilityState, PreflightDiagnosticSeverity, PreflightStatus};
use crate::prelude::*;
use anyhow::{Context, bail};
use chrono::Utc;
use crossterm::{
    cursor::{Hide, Show},
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Frame, Terminal,
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout, Margin, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
};
use std::{
    io::{IsTerminal, Stdout, stdout},
    panic::{self, PanicHookInfo},
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc::{Receiver, TryRecvError},
    },
    thread::{self, ThreadId},
    time::Duration,
};

const DEFAULT_ACCENT: Color = Color::Rgb(14, 165, 233);
const SURFACE_BG: Color = Color::Rgb(3, 7, 7);
const PANEL_BORDER: Color = Color::Rgb(64, 72, 82);
const PANEL_BORDER_SUBTLE: Color = Color::Rgb(42, 48, 56);
const TEXT_PRIMARY: Color = Color::Rgb(220, 226, 234);
const TEXT_PANEL_TITLE: Color = Color::Rgb(190, 199, 211);
const TEXT_MUTED: Color = Color::Rgb(118, 126, 137);
const TEXT_DIM: Color = Color::Rgb(79, 87, 97);
const STATUS_READY: Color = Color::Rgb(34, 197, 94);
const STATUS_WARNING: Color = Color::Rgb(250, 204, 21);
const PRODUCT_TITLE: &str = "AgentPM Harness";
const PREFLIGHT_LINE_WIDTH: usize = 34;
const BAR_HORIZONTAL_PADDING: u16 = 1;
type PageCursor = i64;

mod state;
use state::*;

type PanicHook = Box<dyn Fn(&PanicHookInfo<'_>) + Sync + Send + 'static>;

pub(super) fn run_tui_surface(args: HarnessArgs, workspace_root: PathBuf) -> Result<()> {
    ensure_tui_terminal_available()?;

    let mut terminal = TuiTerminal::enter()?;
    let mut app = TuiApp::loading(args.clone());
    let bootstrap = spawn_bootstrap_worker(args, workspace_root.clone(), None);

    run_shell_loop(&mut terminal, &mut app, workspace_root, bootstrap)
}

fn ensure_tui_terminal_available() -> Result<()> {
    let term = std::env::var("TERM").ok();
    if !terminal_supports_tui(
        term.as_deref(),
        std::io::stdout().is_terminal(),
        std::io::stderr().is_terminal(),
    ) {
        bail!(
            "agentpm harness default TUI requires an interactive terminal; rerun with --headless for script output or --machine for SDK/application protocol output"
        );
    }
    Ok(())
}

fn run_shell_loop(
    terminal: &mut TuiTerminal,
    app: &mut TuiApp,
    workspace_root: PathBuf,
    bootstrap: Receiver<BootstrapMessage>,
) -> Result<()> {
    let mut bootstrap = Some(bootstrap);
    loop {
        poll_bootstrap_result(app, &mut bootstrap);
        poll_run_result(app);
        terminal.draw(|frame| render_app(frame, app))?;
        if event::poll(Duration::from_millis(250)).context("polling terminal input")? {
            let Event::Key(key) = event::read().context("reading terminal input")? else {
                continue;
            };
            if key.kind != KeyEventKind::Press {
                continue;
            }
            if handle_output_viewer_key(app, key.code) {
                continue;
            }
            if handle_resolution_prompt_key(
                app,
                key.code,
                key.modifiers,
                &workspace_root,
                &mut bootstrap,
            )? {
                continue;
            }
            app.reconcile_focus();
            match key.code {
                KeyCode::Esc if app.focus == TuiFocus::Composer => {
                    app.focus = TuiFocus::Panel(VisiblePanel::Run)
                }
                _ if shell_quit_requested(app, key.code, key.modifiers) => break,
                KeyCode::Enter if app.can_send_message() => start_run_from_composer(app),
                KeyCode::Enter if app.composer_available() => app.focus = TuiFocus::Composer,
                KeyCode::Backspace if app.can_send_message() => {
                    app.composer_input.pop();
                }
                KeyCode::Char(ch)
                    if app.can_send_message() && !key.modifiers.contains(KeyModifiers::CONTROL) =>
                {
                    app.composer_input.push(ch);
                }
                KeyCode::BackTab => app.focus_previous(),
                KeyCode::Tab if key.modifiers.contains(KeyModifiers::SHIFT) => app.focus_previous(),
                KeyCode::Tab => app.focus_next(),
                _ if app.focus == TuiFocus::Composer => {}
                KeyCode::PageUp => app.page_focused_panel(PanelPageDirection::Next),
                KeyCode::PageDown => app.page_focused_panel(PanelPageDirection::Previous),
                KeyCode::Char('c') | KeyCode::Char('C') if app.can_cancel_run() => {
                    app.request_run_cancel();
                }
                KeyCode::Char('o') | KeyCode::Char('O') if app.has_latest_output() => {
                    app.open_output_viewer();
                }
                KeyCode::Char('d') | KeyCode::Char('D') if app.has_workspace_details() => {
                    app.workspace_detail_expanded = !app.workspace_detail_expanded
                }
                KeyCode::Char('a') | KeyCode::Char('A') if app.can_prompt_agent_selector() => {
                    app.open_resolution_prompt(ResolutionPromptKind::AgentSelector)
                }
                KeyCode::Char('p') | KeyCode::Char('P') if app.can_prompt_model() => {
                    app.open_resolution_prompt(ResolutionPromptKind::Model)
                }
                KeyCode::Char('s') | KeyCode::Char('S') if app.can_prompt_scope() => {
                    app.open_resolution_prompt(ResolutionPromptKind::Scope)
                }
                KeyCode::Char('1') if app.layout_mode == LayoutMode::Single => {
                    app.focus_panel(VisiblePanel::Workspace)
                }
                KeyCode::Char('2') => app.focus_panel(VisiblePanel::Run),
                KeyCode::Char('3') => app.focus_panel(VisiblePanel::Trace),
                KeyCode::Char('4') => app.focus_panel(VisiblePanel::Memory),
                KeyCode::Char('5') => app.focus_panel(VisiblePanel::Reports),
                KeyCode::Char('6') if app.layout_mode != LayoutMode::Wide => {
                    app.focus_panel(VisiblePanel::EventStream)
                }
                KeyCode::Char('w') | KeyCode::Char('W')
                    if app.layout_mode == LayoutMode::Single =>
                {
                    app.focus_panel(VisiblePanel::Workspace)
                }
                KeyCode::Char('r') | KeyCode::Char('R') => app.focus_panel(VisiblePanel::Run),
                KeyCode::Char('t') | KeyCode::Char('T') => app.focus_panel(VisiblePanel::Trace),
                KeyCode::Char('m') | KeyCode::Char('M') => app.focus_panel(VisiblePanel::Memory),
                _ => {}
            }
        }
    }
    Ok(())
}

fn shell_quit_requested(app: &TuiApp, code: KeyCode, modifiers: KeyModifiers) -> bool {
    if code == KeyCode::Char('c') && modifiers.contains(KeyModifiers::CONTROL) {
        return true;
    }
    match code {
        KeyCode::Esc => app.focus != TuiFocus::Composer,
        KeyCode::Char('q') => app.focus != TuiFocus::Composer,
        _ => false,
    }
}

fn poll_run_result(app: &mut TuiApp) {
    let mut finished = None;
    if let TuiState::Running {
        snapshot,
        receiver,
        progress,
        events,
        ..
    } = &mut app.state
    {
        snapshot.trace.events = events.events();
        snapshot.run.transcript = events.transcript();
        snapshot.run.latest_output = events.latest_phase_output();
        snapshot.run.usage = events.live_run_usage();
        if let Some(phase_id) = events.current_phase_id()
            && snapshot.run.phase_id.as_deref() != Some(phase_id.as_str())
        {
            snapshot.run.phase_id = Some(phase_id);
            snapshot.run.phase_objective = None;
        }
        loop {
            match receiver.try_recv() {
                Ok(TuiRunMessage::Progress(item)) => progress.push(item),
                Ok(TuiRunMessage::Finished(result)) => {
                    finished = Some(*result);
                    break;
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    app.run_error = Some("run worker exited before reporting completion".into());
                    break;
                }
            }
        }
    }
    if let Some(result) = finished {
        app.run_error = result.error;
        app.state = TuiState::Ready {
            controller: Box::new(result.controller),
        };
        app.panel = VisiblePanel::Run;
        app.focus = TuiFocus::Composer;
        app.composer_input.clear();
        app.reconcile_focus();
    }
}

fn start_run_from_composer(app: &mut TuiApp) {
    let input = app.composer_input.trim().to_string();
    if input.is_empty() {
        return;
    }
    let placeholder = TuiState::Failed {
        message: "starting Run".into(),
    };
    let state = std::mem::replace(&mut app.state, placeholder);
    let TuiState::Ready { controller } = state else {
        app.state = state;
        return;
    };
    let run_id = allocate_harness_run_id();
    let cancel = Arc::clone(&controller.cancellation_requested);
    let events = controller.events.clone();
    events.reset_run_output();
    let mut snapshot = controller.snapshot().clone();
    snapshot.run.status = TuiRunStatus::Active;
    snapshot.run.run_id = Some(run_id.clone());
    snapshot.run.phase_id = Some("starting".into());
    snapshot.run.started_at = Some(Utc::now());
    snapshot.run.phase_objective = Some("Preparing the Harness services for this Run.".into());
    snapshot.run.terminal_status = None;
    snapshot.run.latest_output = None;
    snapshot.run.transcript.clear();
    snapshot.run.usage = Default::default();
    snapshot.run.approval = None;
    snapshot.reports.current_report_path = None;
    snapshot.reports.current_trace_path = None;
    snapshot.reports.current_report = None;
    snapshot.reports.current_trace_events.clear();
    snapshot.reports.current_report_error = None;
    snapshot.reports.current_trace_error = None;
    app.assistant_output_page = 0;
    let receiver = spawn_tui_run_worker(*controller, run_id, input.clone());
    app.state = TuiState::Running {
        snapshot: Box::new(snapshot),
        receiver,
        progress: vec![TuiRunProgress {
            message: "Starting Run.".into(),
        }],
        events,
        cancel,
    };
    app.panel = VisiblePanel::Run;
    app.focus = TuiFocus::Panel(VisiblePanel::Run);
    app.run_error = None;
    app.output_viewer = None;
}

fn handle_output_viewer_key(app: &mut TuiApp, code: KeyCode) -> bool {
    let Some(viewer) = &mut app.output_viewer else {
        return false;
    };
    match code {
        KeyCode::Esc | KeyCode::Char('o') | KeyCode::Char('O') | KeyCode::Char('q') => {
            app.output_viewer = None;
        }
        KeyCode::Up => viewer.scroll = viewer.scroll.saturating_sub(1),
        KeyCode::Down => viewer.scroll = viewer.scroll.saturating_add(1),
        KeyCode::PageUp => viewer.scroll = viewer.scroll.saturating_sub(10),
        KeyCode::PageDown => viewer.scroll = viewer.scroll.saturating_add(10),
        KeyCode::Home => viewer.scroll = 0,
        _ => {}
    }
    true
}

fn poll_bootstrap_result(app: &mut TuiApp, bootstrap: &mut Option<Receiver<BootstrapMessage>>) {
    let Some(receiver) = bootstrap else {
        return;
    };
    loop {
        match receiver.try_recv() {
            Ok(BootstrapMessage::Progress(progress)) => app.push_bootstrap_progress(progress),
            Ok(BootstrapMessage::Ready(result)) => match *result {
                Ok(plan) => {
                    app.push_bootstrap_progress(TuiBootstrapProgress::new(
                        TuiBootstrapStage::Runtime,
                        "Preparing TUI runtime controller.",
                    ));
                    match TuiSessionController::new(plan) {
                        Ok(controller) => {
                            let args = app.bootstrap_args.clone();
                            let model = app.model_override.clone();
                            *app = TuiApp::ready_with_runtime_inputs(controller, args, model);
                            *bootstrap = None;
                            return;
                        }
                        Err(err) => {
                            *app = TuiApp::failed(format!("{err:#}"));
                            *bootstrap = None;
                            return;
                        }
                    }
                }
                Err(err) => {
                    *app = TuiApp::failed(format!("{err:#}"));
                    *bootstrap = None;
                    return;
                }
            },
            Err(TryRecvError::Empty) => return,
            Err(TryRecvError::Disconnected) => {
                *app = TuiApp::failed("bootstrap worker exited before reporting readiness".into());
                *bootstrap = None;
                return;
            }
        }
    }
}

fn handle_resolution_prompt_key(
    app: &mut TuiApp,
    code: KeyCode,
    modifiers: KeyModifiers,
    workspace_root: &Path,
    bootstrap: &mut Option<Receiver<BootstrapMessage>>,
) -> Result<bool> {
    if app.resolution_prompt.is_none() {
        return Ok(false);
    }
    if matches!(code, KeyCode::Char('c')) && modifiers.contains(KeyModifiers::CONTROL) {
        return Ok(false);
    }
    match code {
        KeyCode::Esc => app.resolution_prompt = None,
        KeyCode::Backspace => {
            if let Some(prompt) = &mut app.resolution_prompt {
                prompt.value.pop();
                prompt.error = None;
            }
        }
        KeyCode::Enter => {
            if let Some(receiver) = app.submit_resolution_prompt(workspace_root)? {
                *bootstrap = Some(receiver);
            }
        }
        KeyCode::Char(ch) if !modifiers.contains(KeyModifiers::CONTROL) => {
            if let Some(prompt) = &mut app.resolution_prompt {
                prompt.value.push(ch);
                prompt.error = None;
            }
        }
        _ => {}
    }
    Ok(true)
}

struct TuiTerminal {
    terminal: Terminal<CrosstermBackend<Stdout>>,
    _guard: TerminalGuard,
}

impl TuiTerminal {
    fn enter() -> Result<Self> {
        let guard = TerminalGuard::enter()?;
        let backend = CrosstermBackend::new(stdout());
        let mut terminal = Terminal::new(backend).context("starting TUI terminal")?;
        terminal.clear().context("clearing TUI terminal")?;
        Ok(Self {
            terminal,
            _guard: guard,
        })
    }

    fn draw(&mut self, render: impl FnOnce(&mut Frame<'_>)) -> Result<()> {
        self.terminal.draw(render).context("drawing TUI frame")?;
        Ok(())
    }
}

struct TerminalGuard {
    previous_hook: Arc<Mutex<Option<PanicHook>>>,
    owner_thread_id: ThreadId,
}

impl TerminalGuard {
    fn enter() -> Result<Self> {
        enable_raw_mode().context("enabling terminal raw mode")?;
        let mut out = stdout();
        execute!(out, EnterAlternateScreen, Hide).context("entering alternate terminal screen")?;

        let previous_hook: Arc<Mutex<Option<PanicHook>>> =
            Arc::new(Mutex::new(Some(panic::take_hook())));
        let hook_ref = Arc::clone(&previous_hook);
        let owner_thread_id = thread::current().id();
        panic::set_hook(Box::new(move |info| {
            if thread::current().id() == owner_thread_id {
                restore_terminal_state();
                if let Ok(guard) = hook_ref.lock()
                    && let Some(previous) = guard.as_ref()
                {
                    previous(info);
                }
            }
        }));

        Ok(Self {
            previous_hook,
            owner_thread_id,
        })
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        restore_terminal_state();
        if thread::current().id() == self.owner_thread_id
            && let Ok(mut previous) = self.previous_hook.lock()
            && let Some(previous) = previous.take()
        {
            panic::set_hook(previous);
        }
    }
}

fn restore_terminal_state() {
    let _ = disable_raw_mode();
    let mut out = stdout();
    let _ = execute!(out, Show, LeaveAlternateScreen);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum VisiblePanel {
    Workspace,
    Run,
    Trace,
    Memory,
    Reports,
    EventStream,
}

impl VisiblePanel {
    fn label(self) -> &'static str {
        match self {
            Self::Workspace => "Workspace",
            Self::Run => "Run",
            Self::Trace => "Trace",
            Self::Memory => "Memory",
            Self::Reports => "Reports",
            Self::EventStream => "Event Stream",
        }
    }

    fn next_single(self) -> Self {
        match self {
            Self::Workspace => Self::Run,
            Self::Run => Self::Trace,
            Self::Trace => Self::Memory,
            Self::Memory => Self::Reports,
            Self::Reports => Self::EventStream,
            Self::EventStream => Self::Workspace,
        }
    }
}

fn next_focus_panel(panel: VisiblePanel, layout: LayoutMode) -> VisiblePanel {
    match layout {
        LayoutMode::Single => panel.next_single(),
        LayoutMode::Medium => match panel {
            VisiblePanel::Workspace => VisiblePanel::Run,
            VisiblePanel::Run => VisiblePanel::Trace,
            VisiblePanel::Trace => VisiblePanel::Memory,
            VisiblePanel::Memory => VisiblePanel::Reports,
            VisiblePanel::Reports => VisiblePanel::EventStream,
            VisiblePanel::EventStream => VisiblePanel::Workspace,
        },
        LayoutMode::Wide => match panel {
            VisiblePanel::Workspace => VisiblePanel::Run,
            VisiblePanel::Run => VisiblePanel::Trace,
            VisiblePanel::Trace => VisiblePanel::Memory,
            VisiblePanel::Memory => VisiblePanel::Reports,
            VisiblePanel::Reports => VisiblePanel::EventStream,
            VisiblePanel::EventStream => VisiblePanel::Workspace,
        },
    }
}

fn previous_focus_panel(panel: VisiblePanel, layout: LayoutMode) -> VisiblePanel {
    match layout {
        LayoutMode::Single => match panel {
            VisiblePanel::Workspace => VisiblePanel::EventStream,
            VisiblePanel::Run => VisiblePanel::Workspace,
            VisiblePanel::Trace => VisiblePanel::Run,
            VisiblePanel::Memory => VisiblePanel::Trace,
            VisiblePanel::Reports => VisiblePanel::Memory,
            VisiblePanel::EventStream => VisiblePanel::Reports,
        },
        LayoutMode::Medium | LayoutMode::Wide => match panel {
            VisiblePanel::Workspace => VisiblePanel::EventStream,
            VisiblePanel::Run => VisiblePanel::Workspace,
            VisiblePanel::Trace => VisiblePanel::Run,
            VisiblePanel::Memory => VisiblePanel::Trace,
            VisiblePanel::Reports => VisiblePanel::Memory,
            VisiblePanel::EventStream => VisiblePanel::Reports,
        },
    }
}

struct TuiApp {
    state: TuiState,
    panel: VisiblePanel,
    focus: TuiFocus,
    accent: Color,
    warnings: Vec<String>,
    layout_mode: LayoutMode,
    workspace_page: PageCursor,
    trace_page: PageCursor,
    assistant_output_page: PageCursor,
    workspace_detail_expanded: bool,
    resolution_prompt: Option<ResolutionPrompt>,
    bootstrap_args: HarnessArgs,
    model_override: Option<HarnessModelConfig>,
    composer_input: String,
    run_error: Option<String>,
    output_viewer: Option<OutputViewerState>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TuiFocus {
    Composer,
    Panel(VisiblePanel),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PanelPageDirection {
    Next,
    Previous,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ResolutionPromptKind {
    AgentSelector,
    Model,
    Scope,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ResolutionPrompt {
    kind: ResolutionPromptKind,
    label: String,
    value: String,
    error: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LayoutMode {
    Wide,
    Medium,
    Single,
}

enum TuiState {
    Loading {
        args: HarnessArgs,
        progress: Vec<TuiBootstrapProgress>,
    },
    Ready {
        controller: Box<TuiSessionController>,
    },
    Running {
        snapshot: Box<TuiSessionSnapshot>,
        receiver: Receiver<TuiRunMessage>,
        progress: Vec<TuiRunProgress>,
        events: TuiEventBuffer,
        cancel: Arc<AtomicBool>,
    },
    Failed {
        message: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct OutputViewerState {
    scroll: usize,
}

impl TuiApp {
    fn loading(args: HarnessArgs) -> Self {
        Self {
            state: TuiState::Loading {
                args: args.clone(),
                progress: Vec::new(),
            },
            panel: VisiblePanel::Run,
            focus: TuiFocus::Panel(VisiblePanel::Run),
            accent: DEFAULT_ACCENT,
            warnings: Vec::new(),
            layout_mode: LayoutMode::Single,
            workspace_page: 0,
            trace_page: 0,
            assistant_output_page: 0,
            workspace_detail_expanded: false,
            resolution_prompt: None,
            bootstrap_args: args,
            model_override: None,
            composer_input: String::new(),
            run_error: None,
            output_viewer: None,
        }
    }

    fn ready_with_runtime_inputs(
        controller: TuiSessionController,
        bootstrap_args: HarnessArgs,
        model_override: Option<HarnessModelConfig>,
    ) -> Self {
        let (accent, warning) = branding_accent(
            controller
                .plan()
                .config
                .config
                .ui
                .branding
                .accent
                .as_deref(),
        );
        let warnings = warning.into_iter().collect();
        Self {
            state: TuiState::Ready {
                controller: Box::new(controller),
            },
            panel: VisiblePanel::Run,
            focus: TuiFocus::Composer,
            accent,
            warnings,
            layout_mode: LayoutMode::Single,
            workspace_page: 0,
            trace_page: 0,
            assistant_output_page: 0,
            workspace_detail_expanded: false,
            resolution_prompt: None,
            bootstrap_args,
            model_override,
            composer_input: String::new(),
            run_error: None,
            output_viewer: None,
        }
    }

    fn push_bootstrap_progress(&mut self, progress: TuiBootstrapProgress) {
        if let TuiState::Loading {
            progress: items, ..
        } = &mut self.state
        {
            items.push(progress);
        }
    }

    fn failed(message: String) -> Self {
        Self {
            state: TuiState::Failed {
                message: message.clone(),
            },
            panel: VisiblePanel::Workspace,
            focus: TuiFocus::Panel(VisiblePanel::Workspace),
            accent: DEFAULT_ACCENT,
            warnings: Vec::new(),
            layout_mode: LayoutMode::Single,
            workspace_page: 0,
            trace_page: 0,
            assistant_output_page: 0,
            workspace_detail_expanded: false,
            resolution_prompt: None,
            bootstrap_args: HarnessArgs::default(),
            model_override: None,
            composer_input: String::new(),
            run_error: Some(message),
            output_viewer: None,
        }
    }

    fn snapshot(&self) -> Option<&TuiSessionSnapshot> {
        match &self.state {
            TuiState::Ready { controller } => Some(controller.snapshot()),
            TuiState::Running { snapshot, .. } => Some(snapshot),
            TuiState::Loading { .. } | TuiState::Failed { .. } => None,
        }
    }

    fn run_snapshot(&self) -> Option<&TuiRunSnapshot> {
        self.snapshot().map(|snapshot| &snapshot.run)
    }

    fn is_run_active(&self) -> bool {
        matches!(
            self.run_snapshot().map(|run| run.status),
            Some(TuiRunStatus::Active | TuiRunStatus::PendingApproval)
        ) || matches!(self.state, TuiState::Running { .. })
    }

    fn composer_available(&self) -> bool {
        self.output_viewer.is_none()
            && self.resolution_prompt.is_none()
            && self.panel == VisiblePanel::Run
            && matches!(
                self.run_snapshot().map(|run| run.status),
                Some(TuiRunStatus::Idle | TuiRunStatus::Terminal)
            )
            && matches!(self.state, TuiState::Ready { .. })
    }

    fn can_send_message(&self) -> bool {
        self.focus == TuiFocus::Composer && self.composer_available()
    }

    fn can_cancel_run(&self) -> bool {
        self.output_viewer.is_none()
            && matches!(self.focus, TuiFocus::Panel(VisiblePanel::Run))
            && self.panel == VisiblePanel::Run
            && self.is_run_active()
    }

    fn focus_panel(&mut self, panel: VisiblePanel) {
        if !matches!(
            (self.layout_mode, panel),
            (
                LayoutMode::Wide | LayoutMode::Medium,
                VisiblePanel::Workspace
            ) | (LayoutMode::Wide, VisiblePanel::EventStream)
        ) {
            self.panel = panel;
        }
        self.focus = TuiFocus::Panel(panel);
    }

    fn focus_next(&mut self) {
        if self.focus == TuiFocus::Composer {
            self.focus = TuiFocus::Panel(VisiblePanel::Run);
            return;
        }
        let panel = match self.focus {
            TuiFocus::Composer => VisiblePanel::Run,
            TuiFocus::Panel(panel) => next_focus_panel(panel, self.layout_mode),
        };
        self.focus_panel(panel);
    }

    fn focus_previous(&mut self) {
        if self.focus == TuiFocus::Composer {
            self.focus = TuiFocus::Panel(VisiblePanel::Run);
            return;
        }
        let panel = match self.focus {
            TuiFocus::Composer => VisiblePanel::Run,
            TuiFocus::Panel(panel) => previous_focus_panel(panel, self.layout_mode),
        };
        self.focus_panel(panel);
    }

    fn page_focused_panel(&mut self, direction: PanelPageDirection) {
        match (self.focus, direction) {
            (TuiFocus::Panel(VisiblePanel::Workspace), PanelPageDirection::Next) => {
                self.workspace_page += 1
            }
            (TuiFocus::Panel(VisiblePanel::Workspace), PanelPageDirection::Previous) => {
                self.workspace_page -= 1
            }
            (TuiFocus::Panel(VisiblePanel::Trace), PanelPageDirection::Next) => {
                self.trace_page += 1
            }
            (TuiFocus::Panel(VisiblePanel::Trace), PanelPageDirection::Previous) => {
                self.trace_page -= 1
            }
            (TuiFocus::Panel(VisiblePanel::Run), PanelPageDirection::Next) => {
                self.assistant_output_page += 1
            }
            (TuiFocus::Panel(VisiblePanel::Run), PanelPageDirection::Previous) => {
                self.assistant_output_page -= 1
            }
            _ => {}
        }
    }

    fn reconcile_focus(&mut self) {
        if self.output_viewer.is_some() || self.resolution_prompt.is_some() {
            return;
        }
        if self.layout_mode != LayoutMode::Single && self.panel == VisiblePanel::Workspace {
            self.panel = VisiblePanel::Run;
        }
        if self.layout_mode == LayoutMode::Wide && self.panel == VisiblePanel::EventStream {
            self.panel = VisiblePanel::Run;
        }
        match self.focus {
            TuiFocus::Composer if !self.composer_available() => {
                self.focus = TuiFocus::Panel(self.panel);
            }
            TuiFocus::Panel(VisiblePanel::Workspace) if self.layout_mode != LayoutMode::Single => {}
            TuiFocus::Panel(VisiblePanel::EventStream) if self.layout_mode == LayoutMode::Wide => {}
            TuiFocus::Panel(_) => {}
            TuiFocus::Composer => {}
        }
    }

    fn has_latest_output(&self) -> bool {
        assistant_output_text(self).is_some()
    }

    fn open_output_viewer(&mut self) {
        if self.has_latest_output() {
            self.output_viewer = Some(OutputViewerState { scroll: 0 });
        }
    }

    fn request_run_cancel(&mut self) {
        match &mut self.state {
            TuiState::Ready { controller } => {
                if let Err(err) = controller.request_cancel() {
                    self.run_error = Some(format!("{err:#}"));
                }
            }
            TuiState::Running {
                progress, cancel, ..
            } => {
                cancel.store(true, Ordering::SeqCst);
                progress.push(TuiRunProgress {
                    message: "Cancellation requested.".into(),
                });
            }
            TuiState::Loading { .. } | TuiState::Failed { .. } => {}
        }
    }

    fn has_workspace_details(&self) -> bool {
        let TuiState::Ready { controller } = &self.state else {
            return false;
        };
        !self.warnings.is_empty()
            || controller
                .snapshot()
                .workspace
                .diagnostics
                .iter()
                .any(|diagnostic| {
                    matches!(
                        diagnostic.severity,
                        PreflightDiagnosticSeverity::Fatal
                            | PreflightDiagnosticSeverity::Warning
                            | PreflightDiagnosticSeverity::Suppressed
                            | PreflightDiagnosticSeverity::Pending
                    )
                })
    }

    fn has_resolution_actions(&self) -> bool {
        self.can_prompt_agent_selector() || self.can_prompt_model() || self.can_prompt_scope()
    }

    fn can_prompt_agent_selector(&self) -> bool {
        matches!(
            &self.state,
            TuiState::Ready { controller }
                if controller
                    .snapshot()
                    .workspace
                    .diagnostics
                    .iter()
                    .any(|diagnostic| matches!(
                        diagnostic.code.as_str(),
                        "agent_selection_required" | "agent_not_found"
                    ))
        )
    }

    fn can_prompt_model(&self) -> bool {
        matches!(
            &self.state,
            TuiState::Ready { controller } if controller.plan().config.config.model.is_none()
        )
    }

    fn can_prompt_scope(&self) -> bool {
        self.first_unresolved_scope_key().is_some()
    }

    fn first_unresolved_scope_key(&self) -> Option<String> {
        let TuiState::Ready { controller } = &self.state else {
            return None;
        };
        controller
            .snapshot()
            .workspace
            .diagnostics
            .iter()
            .find(|diagnostic| diagnostic.code == "unresolved_runtime_scope")
            .and_then(|diagnostic| unresolved_scope_key_from_message(&diagnostic.message))
    }

    fn open_resolution_prompt(&mut self, kind: ResolutionPromptKind) {
        let (label, value) = match kind {
            ResolutionPromptKind::AgentSelector => ("Agent selector".into(), String::new()),
            ResolutionPromptKind::Model => ("Model provider/model".into(), "openai/".into()),
            ResolutionPromptKind::Scope => {
                let key = self.first_unresolved_scope_key().unwrap_or_default();
                ("Runtime scope KEY=VALUE".into(), format!("{key}="))
            }
        };
        self.panel = VisiblePanel::Workspace;
        self.resolution_prompt = Some(ResolutionPrompt {
            kind,
            label,
            value,
            error: None,
        });
    }

    fn submit_resolution_prompt(
        &mut self,
        workspace_root: &Path,
    ) -> Result<Option<Receiver<BootstrapMessage>>> {
        let Some(prompt) = self.resolution_prompt.clone() else {
            bail!("no TUI resolution prompt is active");
        };
        let value = prompt.value.trim();
        if value.is_empty() {
            self.set_resolution_prompt_error("Value is required.");
            return Ok(None);
        }
        match prompt.kind {
            ResolutionPromptKind::AgentSelector => self.bootstrap_args.agent = Some(value.into()),
            ResolutionPromptKind::Model => {
                let Some((provider, model)) = value.split_once('/') else {
                    self.set_resolution_prompt_error("Use provider/model.");
                    return Ok(None);
                };
                if provider.trim().is_empty() || model.trim().is_empty() {
                    self.set_resolution_prompt_error("Use non-empty provider/model.");
                    return Ok(None);
                }
                let provider = provider.trim();
                if !self.model_provider_available(provider) {
                    self.set_resolution_prompt_error(format!(
                        "Unknown model provider `{provider}`."
                    ));
                    return Ok(None);
                }
                self.model_override = Some(HarnessModelConfig {
                    provider: provider.into(),
                    model: model.trim().into(),
                    options: serde_json::Value::Object(Default::default()),
                });
            }
            ResolutionPromptKind::Scope => {
                let Some((key, scope_value)) = value.split_once('=') else {
                    self.set_resolution_prompt_error("Use KEY=VALUE.");
                    return Ok(None);
                };
                if key.trim().is_empty() || scope_value.trim().is_empty() {
                    self.set_resolution_prompt_error("Use non-empty KEY=VALUE.");
                    return Ok(None);
                }
                self.bootstrap_args
                    .scopes
                    .retain(|(existing, _)| existing != key.trim());
                self.bootstrap_args
                    .scopes
                    .push((key.trim().into(), scope_value.trim().into()));
            }
        }
        self.resolution_prompt = None;
        let args = self.bootstrap_args.clone();
        let model = self.model_override.clone();
        *self = TuiApp::loading(args.clone());
        self.model_override = model.clone();
        Ok(Some(spawn_bootstrap_worker(
            args,
            workspace_root.to_path_buf(),
            model,
        )))
    }

    fn set_resolution_prompt_error(&mut self, message: impl Into<String>) {
        if let Some(prompt) = &mut self.resolution_prompt {
            prompt.error = Some(message.into());
        }
    }

    fn model_provider_available(&self, provider: &str) -> bool {
        if is_built_in_model_provider(provider) {
            return true;
        }
        let TuiState::Ready { controller } = &self.state else {
            return false;
        };
        controller
            .plan()
            .config
            .config
            .providers
            .models
            .contains_key(provider)
    }
}

fn unresolved_scope_key_from_message(message: &str) -> Option<String> {
    let rest = message.strip_prefix("Memory runtime scope `")?;
    let (key, _) = rest.split_once('`')?;
    Some(key.to_string())
}

fn branding_accent(configured: Option<&str>) -> (Color, Option<String>) {
    let Some(value) = configured else {
        return (DEFAULT_ACCENT, None);
    };
    if terminal_color_fallback_requested() {
        return (
            DEFAULT_ACCENT,
            Some(format!(
                "Configured accent `{value}` is valid, but this terminal requested reduced color; using default accent."
            )),
        );
    }
    match parse_hex_color(value) {
        Some(color) => (color, None),
        None => (DEFAULT_ACCENT, None),
    }
}

fn terminal_color_fallback_requested() -> bool {
    std::env::var_os("NO_COLOR").is_some()
}

fn parse_hex_color(value: &str) -> Option<Color> {
    let hex = value.strip_prefix('#')?;
    if hex.len() != 6 {
        return None;
    }
    let red = u8::from_str_radix(&hex[0..2], 16).ok()?;
    let green = u8::from_str_radix(&hex[2..4], 16).ok()?;
    let blue = u8::from_str_radix(&hex[4..6], 16).ok()?;
    Some(Color::Rgb(red, green, blue))
}

fn render_app(frame: &mut Frame<'_>, app: &mut TuiApp) {
    let area = frame.area();
    frame.render_widget(Clear, area);
    frame.render_widget(
        Block::default().style(Style::default().bg(SURFACE_BG)),
        area,
    );
    let area = area.inner(Margin {
        horizontal: 1,
        vertical: 0,
    });
    app.layout_mode = layout_mode_for_width(area.width);
    app.reconcile_focus();

    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(5),
            Constraint::Length(4),
        ])
        .split(area);

    render_top_bar(frame, layout[0], app);
    let divider_lanes = render_body(frame, layout[1], app);
    render_keybar(frame, layout[2], app);
    render_shell_dividers(frame, layout[0], layout[2], &divider_lanes);
    render_output_viewer(frame, layout[1], app);
}

fn layout_mode_for_width(width: u16) -> LayoutMode {
    if width >= 120 {
        LayoutMode::Wide
    } else if width >= 88 {
        LayoutMode::Medium
    } else {
        LayoutMode::Single
    }
}

fn trace_level_label(level: &HarnessTraceLevel) -> &'static str {
    match level {
        HarnessTraceLevel::Minimal => "minimal",
        HarnessTraceLevel::Normal => "normal",
        HarnessTraceLevel::Verbose => "verbose",
    }
}

fn trace_content_label(content: &HarnessTraceContent) -> &'static str {
    match content {
        HarnessTraceContent::None => "none",
        HarnessTraceContent::Redacted => "redacted",
        HarnessTraceContent::Full => "full",
    }
}

fn preflight_status_label(status: PreflightStatus) -> &'static str {
    match status {
        PreflightStatus::Ready => "ready",
        PreflightStatus::ReadyWithWarnings => "ready_with_warnings",
        PreflightStatus::SelectionRequired => "selection_required",
        PreflightStatus::Failed => "failed",
    }
}

fn diagnostic_severity_label(severity: PreflightDiagnosticSeverity) -> &'static str {
    match severity {
        PreflightDiagnosticSeverity::Fatal => "fatal",
        PreflightDiagnosticSeverity::Warning => "warning",
        PreflightDiagnosticSeverity::Suppressed => "suppressed",
        PreflightDiagnosticSeverity::Pending => "pending",
        PreflightDiagnosticSeverity::Info => "info",
    }
}

fn bootstrap_stage_label(stage: TuiBootstrapStage) -> &'static str {
    match stage {
        TuiBootstrapStage::Bootstrap => "bootstrap",
        TuiBootstrapStage::Preflight => "preflight",
        TuiBootstrapStage::Runtime => "runtime",
    }
}

fn terminal_status_label(status: HarnessTerminalStatus) -> &'static str {
    match status {
        HarnessTerminalStatus::Ended => "ended",
        HarnessTerminalStatus::HandedOff => "handed_off",
        HarnessTerminalStatus::Aborted => "aborted",
        HarnessTerminalStatus::Failed => "failed",
        HarnessTerminalStatus::Cancelled => "cancelled",
        HarnessTerminalStatus::LimitReached => "limit_reached",
        HarnessTerminalStatus::ApprovalRequired => "approval_required",
    }
}

fn event_trace_line(event: &HarnessEventEnvelope) -> String {
    let event_type = serde_json::to_value(event.event_type)
        .ok()
        .and_then(|value| value.as_str().map(str::to_string))
        .unwrap_or_else(|| "unknown_event".into());
    match &event.run_id {
        Some(run_id) => format!(
            "{}  {} · {}",
            event.timestamp.format("%H:%M:%S"),
            event_type,
            run_id
        ),
        None => format!("{}  {}", event.timestamp.format("%H:%M:%S"), event_type),
    }
}

fn render_top_bar(frame: &mut Frame<'_>, area: Rect, app: &TuiApp) {
    let (branding, trace_label) = match &app.state {
        TuiState::Ready { controller } => {
            let plan = controller.plan();
            let branding = &plan.config.config.ui.branding;
            let mut parts = Vec::new();
            if branding.name != PRODUCT_TITLE {
                parts.push(branding.name.clone());
            }
            if let Some(subtitle) = &branding.subtitle
                && !subtitle.trim().is_empty()
            {
                parts.push(subtitle.clone());
            }
            let trace = format!(
                "[Trace: {}]  [Content: {}]",
                trace_level_label(&plan.config.config.trace.level),
                trace_content_label(&plan.config.config.trace.content)
            );
            (parts.join(" · "), trace)
        }
        TuiState::Loading { .. } => (String::new(), "[Trace: Pending]".into()),
        TuiState::Running { .. } => (String::new(), "[Trace: Normal]".into()),
        TuiState::Failed { .. } => (String::new(), "[Trace: Unavailable]".into()),
    };

    let block = Block::default()
        .borders(Borders::BOTTOM)
        .border_style(Style::default().fg(PANEL_BORDER_SUBTLE));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let content_area = if inner.height > 1 {
        Rect {
            y: inner.y + 1,
            height: 1,
            ..inner
        }
    } else {
        inner
    };

    let content_area = content_area.inner(Margin {
        horizontal: BAR_HORIZONTAL_PADDING,
        vertical: 0,
    });
    let right_width = (trace_label.chars().count() + "● Session Active  ".len() + 2)
        .min(content_area.width.saturating_sub(8) as usize) as u16;
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(12), Constraint::Length(right_width)])
        .split(content_area);

    let mut left_spans = vec![Span::styled(
        PRODUCT_TITLE,
        Style::default()
            .fg(Color::White)
            .add_modifier(Modifier::BOLD),
    )];
    if !branding.is_empty() {
        left_spans.push(Span::raw("  "));
        left_spans.push(Span::styled(
            branding,
            Style::default().fg(app.accent).add_modifier(Modifier::BOLD),
        ));
    }

    let right_line = Line::from(vec![
        Span::styled("●", Style::default().fg(STATUS_READY)),
        Span::styled(
            " Session Active",
            Style::default()
                .fg(STATUS_READY)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("  "),
        Span::styled(trace_label, Style::default().fg(TEXT_MUTED)),
    ]);

    frame.render_widget(
        Paragraph::new(Line::from(left_spans)).alignment(Alignment::Left),
        chunks[0],
    );
    frame.render_widget(
        Paragraph::new(right_line).alignment(Alignment::Right),
        chunks[1],
    );
}

fn render_body(frame: &mut Frame<'_>, area: Rect, app: &TuiApp) -> Vec<Rect> {
    let content_area = if area.height > 1 {
        Rect {
            y: area.y + 1,
            height: area.height - 1,
            ..area
        }
    } else {
        area
    };

    match app.layout_mode {
        LayoutMode::Wide => render_wide_body(frame, area, content_area, app),
        LayoutMode::Medium => render_medium_body(frame, area, content_area, app),
        LayoutMode::Single => {
            match app.panel {
                VisiblePanel::Workspace => render_workspace_panel(frame, content_area, app),
                VisiblePanel::EventStream => render_trace_rail(frame, content_area, app),
                _ => render_selected_center_panel(frame, content_area, app),
            }
            Vec::new()
        }
    }
}

fn render_wide_body(
    frame: &mut Frame<'_>,
    area: Rect,
    content_area: Rect,
    app: &TuiApp,
) -> Vec<Rect> {
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(38),
            Constraint::Length(3),
            Constraint::Min(50),
            Constraint::Length(3),
            Constraint::Length(38),
        ])
        .split(area);
    let content_columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(38),
            Constraint::Length(3),
            Constraint::Min(50),
            Constraint::Length(3),
            Constraint::Length(38),
        ])
        .split(content_area);
    render_workspace_panel(frame, content_columns[0], app);
    render_selected_center_panel(frame, content_columns[2], app);
    render_trace_rail(frame, content_columns[4], app);
    vec![columns[1], columns[3]]
}

fn render_medium_body(
    frame: &mut Frame<'_>,
    area: Rect,
    content_area: Rect,
    app: &TuiApp,
) -> Vec<Rect> {
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(38),
            Constraint::Length(3),
            Constraint::Min(45),
        ])
        .split(area);
    let content_columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(38),
            Constraint::Length(3),
            Constraint::Min(45),
        ])
        .split(content_area);
    render_workspace_panel(frame, content_columns[0], app);
    render_selected_center_panel(frame, content_columns[2], app);
    vec![columns[1]]
}

fn render_shell_dividers(
    frame: &mut Frame<'_>,
    top_bar_area: Rect,
    keybar_area: Rect,
    divider_lanes: &[Rect],
) {
    let top_joint_y = top_bar_area
        .y
        .saturating_add(top_bar_area.height.saturating_sub(1));
    let bottom_joint_y = keybar_area.y;
    let body_y = top_joint_y.saturating_add(1);
    let body_height = bottom_joint_y.saturating_sub(body_y);
    for lane in divider_lanes {
        let x = lane.x + lane.width / 2;
        render_rule_glyph(frame, x, top_joint_y, "┬");
        render_vertical_rule(
            frame,
            Rect {
                x,
                y: body_y,
                width: 1,
                height: body_height,
            },
        );
        render_rule_glyph(frame, x, bottom_joint_y, "┴");
    }
}

fn render_vertical_rule(frame: &mut Frame<'_>, area: Rect) {
    frame.render_widget(
        Block::default()
            .borders(Borders::LEFT)
            .border_style(Style::default().fg(PANEL_BORDER_SUBTLE)),
        area,
    );
}

fn render_rule_glyph(frame: &mut Frame<'_>, x: u16, y: u16, glyph: &'static str) {
    frame.render_widget(
        Paragraph::new(glyph).style(Style::default().fg(PANEL_BORDER_SUBTLE)),
        Rect {
            x,
            y,
            width: 1,
            height: 1,
        },
    );
}

fn render_selected_center_panel(frame: &mut Frame<'_>, area: Rect, app: &TuiApp) {
    match app.panel {
        VisiblePanel::Workspace | VisiblePanel::Run => render_run_panel(frame, area, app),
        VisiblePanel::Trace => render_center_trace_panel(frame, area, app),
        VisiblePanel::Memory => render_center_content(frame, area, memory_lines(app)),
        VisiblePanel::Reports => render_center_content(frame, area, report_lines(app)),
        VisiblePanel::EventStream => render_trace_rail(frame, area, app),
    }
}

fn render_workspace_panel(frame: &mut Frame<'_>, area: Rect, app: &TuiApp) {
    let block = Block::default()
        .title(" Preflight - Workspace Readiness ")
        .title_style(panel_title_style())
        .borders(Borders::ALL)
        .border_style(Style::default().fg(PANEL_BORDER));
    let block_inner = block.inner(area);
    let inner = panel_inner(&block, area);
    let footer = Rect {
        x: block_inner.x.saturating_add(1),
        y: block_inner.y + block_inner.height.saturating_sub(1),
        width: block_inner.width.saturating_sub(2),
        height: if block_inner.height > 0 { 1 } else { 0 },
    };
    let body_height = if footer.height > 0 {
        footer.y.saturating_sub(inner.y)
    } else {
        inner.height
    };
    let page = workspace_page(app, body_height as usize, inner.width as usize);
    let body = Rect {
        height: body_height,
        ..inner
    };
    frame.render_widget(block, area);
    frame.render_widget(
        Paragraph::new(page.lines)
            .style(Style::default().fg(TEXT_PRIMARY))
            .wrap(Wrap { trim: false }),
        body,
    );
    frame.render_widget(
        Paragraph::new(page.footer)
            .style(Style::default().fg(TEXT_MUTED))
            .alignment(Alignment::Right),
        footer,
    );
}

fn render_run_panel(frame: &mut Frame<'_>, area: Rect, app: &TuiApp) {
    let inner = area.inner(Margin {
        horizontal: 1,
        vertical: 0,
    });
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(1),
        ])
        .split(inner);
    frame.render_widget(Paragraph::new(center_header_line(app)), chunks[0]);
    frame.render_widget(Paragraph::new(center_tabs_line(app)), chunks[2]);

    match run_visual_state(app) {
        RunVisualState::NoRun => render_no_run_panel(frame, chunks[4], app),
        RunVisualState::Active => render_active_run_panel(frame, chunks[4], app),
        RunVisualState::Terminal => render_terminal_run_panel(frame, chunks[4], app),
        RunVisualState::Loading => {
            render_run_section(
                frame,
                chunks[4],
                " Bootstrap ",
                vec![Line::from(
                    "Resolving workspace, config, lockfile, and Agent graph...",
                )],
                app.accent,
            );
        }
        RunVisualState::Failed => {
            render_run_section(
                frame,
                chunks[4],
                " Run Unavailable ",
                vec![Line::from(styled(
                    "Fix preflight errors, then restart the Harness.",
                    Color::Red,
                ))],
                Color::Red,
            );
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RunVisualState {
    Loading,
    Failed,
    NoRun,
    Active,
    Terminal,
}

fn run_visual_state(app: &TuiApp) -> RunVisualState {
    match &app.state {
        TuiState::Loading { .. } => RunVisualState::Loading,
        TuiState::Failed { .. } => RunVisualState::Failed,
        TuiState::Running { .. } => RunVisualState::Active,
        TuiState::Ready { controller } => match controller.snapshot().run.status {
            TuiRunStatus::Active | TuiRunStatus::PendingApproval => RunVisualState::Active,
            TuiRunStatus::Terminal => RunVisualState::Terminal,
            TuiRunStatus::Idle => RunVisualState::NoRun,
        },
    }
}

fn render_no_run_panel(frame: &mut Frame<'_>, area: Rect, app: &TuiApp) {
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5),
            Constraint::Length(1),
            Constraint::Length(5),
            Constraint::Min(1),
        ])
        .split(area);
    render_run_section(
        frame,
        layout[0],
        " Ready ",
        vec![
            Line::from(styled("No Run yet", app.accent)),
            Line::from("Type a request below to start the first Run in this Session."),
        ],
        PANEL_BORDER,
    );
    render_message_section(frame, layout[2], app, app.accent);
}

fn render_active_run_panel(frame: &mut Frame<'_>, area: Rect, app: &TuiApp) {
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5),
            Constraint::Length(1),
            Constraint::Length(6),
            Constraint::Length(1),
            Constraint::Length(5),
            Constraint::Length(1),
            Constraint::Min(6),
            Constraint::Length(1),
            Constraint::Length(5),
        ])
        .split(area);
    render_run_section(
        frame,
        layout[0],
        " ⊙ Phase Objective ",
        phase_objective_content(app),
        app.accent,
    );
    render_run_section(
        frame,
        layout[2],
        " ◇ Effective Capabilities ",
        effective_capabilities_content(app),
        PANEL_BORDER,
    );
    render_run_section(
        frame,
        layout[4],
        " Σ Usage ",
        usage_content(app),
        PANEL_BORDER,
    );
    render_assistant_output_section(frame, layout[6], " ⊙ Assistant Output ", app);
    render_working_section(frame, layout[8], app);
}

fn render_terminal_run_panel(frame: &mut Frame<'_>, area: Rect, app: &TuiApp) {
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(8),
            Constraint::Length(1),
            Constraint::Length(5),
            Constraint::Length(1),
            Constraint::Min(6),
            Constraint::Length(1),
            Constraint::Length(5),
        ])
        .split(area);
    render_run_section(
        frame,
        layout[0],
        " ■ Run Summary ",
        run_summary_content(app),
        PANEL_BORDER,
    );
    render_run_section(
        frame,
        layout[2],
        " Σ Usage ",
        usage_content(app),
        PANEL_BORDER,
    );
    render_assistant_output_section(frame, layout[4], " ⊙ Assistant Output (Latest) ", app);
    render_message_section(frame, layout[6], app, app.accent);
}

fn render_run_section(
    frame: &mut Frame<'_>,
    area: Rect,
    title: &'static str,
    lines: Vec<Line<'static>>,
    border: Color,
) {
    if area.height == 0 || area.width == 0 {
        return;
    }
    let block = Block::default()
        .title(title)
        .title_style(panel_title_style())
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border));
    let inner = panel_inner(&block, area);
    frame.render_widget(block, area);
    frame.render_widget(
        Paragraph::new(lines)
            .style(Style::default().fg(TEXT_PRIMARY))
            .wrap(Wrap { trim: false }),
        inner,
    );
}

fn render_assistant_output_section(
    frame: &mut Frame<'_>,
    area: Rect,
    title: &'static str,
    app: &TuiApp,
) {
    if area.height == 0 || area.width == 0 {
        return;
    }
    let block = Block::default()
        .title(title)
        .title_style(panel_title_style())
        .borders(Borders::ALL)
        .border_style(Style::default().fg(PANEL_BORDER));
    let block_inner = block.inner(area);
    let inner = panel_inner(&block, area);
    let footer = Rect {
        x: block_inner.x.saturating_add(1),
        y: block_inner.y + block_inner.height.saturating_sub(1),
        width: block_inner.width.saturating_sub(2),
        height: if block_inner.height > 0 { 1 } else { 0 },
    };
    let body_height = if footer.height > 0 {
        footer.y.saturating_sub(inner.y)
    } else {
        inner.height
    };
    let body = Rect {
        height: body_height,
        ..inner
    };
    let page = assistant_output_page(app, body_height as usize, body.width as usize);
    frame.render_widget(block, area);
    frame.render_widget(
        Paragraph::new(page.lines)
            .style(Style::default().fg(TEXT_PRIMARY))
            .wrap(Wrap { trim: false }),
        body,
    );
    frame.render_widget(
        Paragraph::new(page.footer)
            .style(Style::default().fg(TEXT_MUTED))
            .alignment(Alignment::Right),
        footer,
    );
}

fn render_message_section(frame: &mut Frame<'_>, area: Rect, app: &TuiApp, border: Color) {
    let mut lines = Vec::new();
    let text = if app.composer_input.is_empty() {
        let placeholder = if app.focus == TuiFocus::Composer {
            "Type a message to start the next Run..."
        } else {
            "Press Enter to compose the next Run..."
        };
        Span::styled(placeholder, Style::default().fg(TEXT_DIM))
    } else {
        Span::styled(
            app.composer_input.clone(),
            Style::default().fg(TEXT_PRIMARY),
        )
    };
    lines.push(Line::from(text));
    if app.focus == TuiFocus::Composer {
        lines.push(Line::from(vec![
            key_span("Enter", app.accent),
            Span::raw(" Send"),
        ]));
    } else {
        lines.push(Line::from(vec![
            key_span("Enter", app.accent),
            Span::raw(" Compose"),
        ]));
    }
    render_run_section(frame, area, " Message ", lines, border);
}

fn render_working_section(frame: &mut Frame<'_>, area: Rect, app: &TuiApp) {
    let phase = app
        .run_snapshot()
        .and_then(|run| run.phase_id.as_deref())
        .unwrap_or("starting");
    render_run_section(
        frame,
        area,
        " Run In Progress ",
        vec![
            Line::from(vec![
                styled(
                    format!("Run is active - phase {phase} in progress"),
                    STATUS_WARNING,
                ),
                Span::raw("    "),
                key_span("C", app.accent),
                Span::raw(" Cancel Run"),
            ]),
            Line::from(Span::styled(
                "Composer reopens when this Run reaches a terminal state.",
                Style::default().fg(TEXT_MUTED),
            )),
        ],
        STATUS_WARNING,
    );
}

fn render_center_content(frame: &mut Frame<'_>, area: Rect, lines: Vec<Line<'_>>) {
    let inner = area.inner(Margin {
        horizontal: 1,
        vertical: 0,
    });
    frame.render_widget(
        Paragraph::new(lines)
            .style(Style::default().fg(TEXT_PRIMARY))
            .wrap(Wrap { trim: false }),
        inner,
    );
}

fn render_center_trace_panel(frame: &mut Frame<'_>, area: Rect, app: &TuiApp) {
    let inner = area.inner(Margin {
        horizontal: 1,
        vertical: 0,
    });
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(1),
            Constraint::Length(1),
        ])
        .split(inner);
    let page = trace_page(app, chunks[4].height as usize);
    frame.render_widget(Paragraph::new(center_header_line(app)), chunks[0]);
    frame.render_widget(Paragraph::new(center_tabs_line(app)), chunks[2]);
    frame.render_widget(
        Paragraph::new(page.lines)
            .style(Style::default().fg(TEXT_PRIMARY))
            .wrap(Wrap { trim: false }),
        chunks[4],
    );
    frame.render_widget(
        Paragraph::new(page.footer)
            .style(Style::default().fg(TEXT_MUTED))
            .alignment(Alignment::Right),
        chunks[5],
    );
}

fn render_output_viewer(frame: &mut Frame<'_>, area: Rect, app: &TuiApp) {
    let Some(viewer) = &app.output_viewer else {
        return;
    };
    let Some(output) = assistant_output_text(app) else {
        return;
    };
    let overlay = centered_overlay(area, app.layout_mode);
    frame.render_widget(Clear, overlay);
    let block = Block::default()
        .title(" Assistant Output ")
        .title_style(panel_title_style())
        .borders(Borders::ALL)
        .border_style(Style::default().fg(app.accent));
    let inner = panel_inner(&block, overlay);
    frame.render_widget(block, overlay);
    let mut lines = Vec::new();
    if let Some(snapshot) = app.snapshot() {
        if let Some(path) = &snapshot.reports.current_report_path {
            lines.push(Line::from(format!("report: {}", path.display())));
        }
        if let Some(path) = &snapshot.reports.current_trace_path {
            lines.push(Line::from(format!("trace: {}", path.display())));
        }
        if !lines.is_empty() {
            lines.push(Line::from(""));
        }
    }
    lines.extend(output.lines().map(|line| Line::from(line.to_string())));
    frame.render_widget(
        Paragraph::new(lines)
            .style(Style::default().fg(TEXT_PRIMARY))
            .scroll((viewer.scroll as u16, 0))
            .wrap(Wrap { trim: false }),
        inner,
    );
}

fn centered_overlay(area: Rect, layout: LayoutMode) -> Rect {
    match layout {
        LayoutMode::Single => area.inner(Margin {
            horizontal: 1,
            vertical: 1,
        }),
        LayoutMode::Medium | LayoutMode::Wide => {
            let width = area.width.saturating_sub(8).max(area.width / 2);
            let height = area.height.saturating_sub(4).max(area.height / 2);
            Rect {
                x: area.x + (area.width.saturating_sub(width)) / 2,
                y: area.y + (area.height.saturating_sub(height)) / 2,
                width,
                height,
            }
        }
    }
}

fn render_trace_rail(frame: &mut Frame<'_>, area: Rect, app: &TuiApp) {
    let block = Block::default()
        .title(" Trace - Event Stream ")
        .title_style(panel_title_style())
        .borders(Borders::ALL)
        .border_style(Style::default().fg(PANEL_BORDER));
    let inner = panel_inner(&block, area);
    frame.render_widget(block, area);
    frame.render_widget(
        Paragraph::new(trace_lines(app))
            .style(Style::default().fg(TEXT_PRIMARY))
            .wrap(Wrap { trim: false }),
        inner,
    );
}

fn panel_inner(block: &Block<'_>, area: Rect) -> Rect {
    block.inner(area).inner(Margin {
        horizontal: 1,
        vertical: 1,
    })
}

fn panel_title_style() -> Style {
    Style::default()
        .fg(TEXT_PANEL_TITLE)
        .add_modifier(Modifier::BOLD)
}

fn render_keybar(frame: &mut Frame<'_>, area: Rect, app: &TuiApp) {
    let block = Block::default()
        .borders(Borders::TOP)
        .border_style(Style::default().fg(PANEL_BORDER_SUBTLE));
    let inner = block.inner(area);
    let content_area = if inner.height > 1 {
        Rect {
            y: inner.y + inner.height / 2,
            height: 1,
            ..inner
        }
    } else {
        inner
    };
    let content_area = content_area.inner(Margin {
        horizontal: BAR_HORIZONTAL_PADDING,
        vertical: 0,
    });
    frame.render_widget(block, area);

    let mut spans = if app.output_viewer.is_some() {
        vec![
            key_span("↑/↓", app.accent),
            Span::raw(" Scroll  "),
            key_span("PgUp/PgDn", app.accent),
            Span::raw(" Page  "),
            key_span("Esc", app.accent),
            Span::raw(" Close"),
        ]
    } else {
        vec![
            key_span("Q", app.accent),
            Span::raw(" Quit  "),
            key_span("Tab", app.accent),
            Span::raw(" Next  "),
            key_span("Shift+Tab", app.accent),
            Span::raw(" Prev  "),
        ]
    };
    if app.output_viewer.is_some() {
        let line = Line::from(spans);
        frame.render_widget(
            Paragraph::new(line)
                .alignment(Alignment::Left)
                .style(Style::default().fg(TEXT_MUTED)),
            content_area,
        );
        return;
    }
    if app.focus == TuiFocus::Composer {
        spans = vec![
            key_span("Enter", app.accent),
            Span::raw(" Send  "),
            key_span("Esc", app.accent),
            Span::raw(" Navigation  "),
            Span::raw("Focus: Composer"),
        ];
        let line = Line::from(spans);
        frame.render_widget(
            Paragraph::new(line)
                .alignment(Alignment::Left)
                .style(Style::default().fg(TEXT_MUTED)),
            content_area,
        );
        return;
    }
    if app.composer_available() {
        spans.extend([key_span("Enter", app.accent), Span::raw(" Compose  ")]);
    }
    if app.can_cancel_run() {
        spans.extend([key_span("C", app.accent), Span::raw(" Cancel Run  ")]);
    }
    if app.focus == TuiFocus::Panel(VisiblePanel::Workspace) {
        spans.extend([key_span("PgUp", app.accent), Span::raw(" Next Page  ")]);
        spans.extend([key_span("PgDn", app.accent), Span::raw(" Prev Page  ")]);
    }
    if app.has_latest_output() {
        spans.extend([key_span("O", app.accent), Span::raw(" Output  ")]);
    }
    if app.layout_mode == LayoutMode::Single {
        spans.extend([key_span("1", app.accent), Span::raw(" Workspace  ")]);
    }
    if app.has_workspace_details() {
        spans.extend([key_span("D", app.accent), Span::raw(" Details  ")]);
    }
    if app.can_prompt_agent_selector() {
        spans.extend([key_span("A", app.accent), Span::raw(" Agent  ")]);
    }
    if app.can_prompt_model() {
        spans.extend([key_span("P", app.accent), Span::raw(" Model  ")]);
    }
    if app.can_prompt_scope() {
        spans.extend([key_span("S", app.accent), Span::raw(" Scope  ")]);
    }
    spans.extend([
        key_span("2", app.accent),
        Span::raw(" Run  "),
        key_span("3", app.accent),
        Span::raw(" Trace  "),
        key_span("4", app.accent),
        Span::raw(" Memory  "),
        key_span("5", app.accent),
        Span::raw(" Reports"),
    ]);
    if app.layout_mode != LayoutMode::Wide {
        spans.extend([
            Span::raw("  "),
            key_span("6", app.accent),
            Span::raw(" Events"),
        ]);
    }
    spans.push(Span::raw(format!("    Focus: {}", app.focus.label())));
    let status_text = bottom_run_status_text(app);
    let status_width = status_text
        .as_ref()
        .map(|text| text.chars().count() as u16)
        .unwrap_or(0);
    let left_area = status_text
        .as_ref()
        .filter(|_| content_area.width > status_width.saturating_add(48))
        .map(|_| Rect {
            width: content_area
                .width
                .saturating_sub(status_width.saturating_add(2)),
            ..content_area
        })
        .unwrap_or(content_area);
    let line = Line::from(spans);
    frame.render_widget(
        Paragraph::new(line)
            .alignment(Alignment::Left)
            .style(Style::default().fg(TEXT_MUTED)),
        left_area,
    );
    if let Some(status_text) = status_text
        && content_area.width > status_width.saturating_add(48)
    {
        let right_area = Rect {
            x: content_area
                .x
                .saturating_add(content_area.width.saturating_sub(status_width)),
            width: status_width,
            ..content_area
        };
        frame.render_widget(
            Paragraph::new(Line::from(status_text))
                .alignment(Alignment::Right)
                .style(Style::default().fg(TEXT_MUTED)),
            right_area,
        );
    }
}

fn key_span(label: &'static str, accent: Color) -> Span<'static> {
    Span::styled(
        label,
        Style::default().fg(accent).add_modifier(Modifier::BOLD),
    )
}

fn bottom_run_status_text(app: &TuiApp) -> Option<String> {
    let snapshot = app.snapshot()?;
    let run_id = run_header_text(snapshot.run.run_id.as_deref());
    match snapshot.run.status {
        TuiRunStatus::Active | TuiRunStatus::PendingApproval => {
            let phase = snapshot.run.phase_id.as_deref().unwrap_or("starting");
            let status = match snapshot.run.status {
                TuiRunStatus::PendingApproval => "approval",
                _ => "active",
            };
            let elapsed = snapshot
                .run
                .started_at
                .map(|started| {
                    let duration_ms = Utc::now()
                        .signed_duration_since(started)
                        .num_milliseconds()
                        .max(0) as u64;
                    format_clock_duration_ms(duration_ms)
                })
                .unwrap_or_else(|| "--:--:--".into());
            Some(format!("{run_id} · {phase} · {status} · {elapsed}"))
        }
        TuiRunStatus::Terminal => {
            let report = snapshot.reports.current_report.as_ref();
            let terminal_status = report
                .map(|report| report.terminal_status)
                .or(snapshot.run.terminal_status);
            let status = terminal_status
                .map(terminal_status_label)
                .unwrap_or("terminal");
            let target = report
                .and_then(report_terminal_target)
                .unwrap_or_else(|| status.to_string());
            let duration = report
                .and_then(|report| report.duration_ms)
                .or(snapshot.run.usage.duration_ms)
                .map(format_clock_duration_ms)
                .unwrap_or_else(|| "--:--:--".into());
            Some(format!("{run_id} · {target} -> {status} · {duration}"))
        }
        TuiRunStatus::Idle => None,
    }
}

impl TuiFocus {
    fn label(self) -> &'static str {
        match self {
            Self::Composer => "Composer",
            Self::Panel(panel) => panel.label(),
        }
    }
}

struct WorkspacePage {
    lines: Vec<Line<'static>>,
    footer: Line<'static>,
}

fn workspace_page(app: &TuiApp, max_lines: usize, line_width: usize) -> WorkspacePage {
    match &app.state {
        TuiState::Loading { args, progress } => {
            let mut lines = vec![
                Line::from(styled("Loading Harness workspace...", app.accent)),
                Line::from(""),
                Line::from(format!(
                    "Agent selector: {}",
                    args.agent.as_deref().unwrap_or("auto")
                )),
                Line::from(format!(
                    "Config: {}",
                    args.config
                        .as_ref()
                        .map(|path| path.display().to_string())
                        .unwrap_or_else(|| "agentpm.harness.json or defaults".into())
                )),
                Line::from(""),
            ];
            if progress.is_empty() {
                lines.push(Line::from("Preflight readiness will appear here."));
            } else {
                for item in progress.iter().rev().take(5).rev() {
                    lines.push(Line::from(vec![
                        styled(bootstrap_stage_label(item.stage), app.accent),
                        Span::raw(format!(" - {}", item.message)),
                    ]));
                }
            }
            WorkspacePage {
                lines,
                footer: workspace_page_footer(0, 1),
            }
        }
        TuiState::Failed { message } => WorkspacePage {
            lines: vec![
                Line::from(styled("Preflight failed", Color::Red)),
                Line::from(""),
                Line::from(message.clone()),
                Line::from(""),
                Line::from("Press Q to exit."),
            ],
            footer: workspace_page_footer(0, 1),
        },
        TuiState::Ready { controller } => {
            workspace_ready_page(app, controller.snapshot(), max_lines, line_width)
        }
        TuiState::Running { snapshot, .. } => {
            workspace_ready_page(app, snapshot, max_lines, line_width)
        }
    }
}

fn workspace_ready_page(
    app: &TuiApp,
    snapshot: &TuiSessionSnapshot,
    max_lines: usize,
    line_width: usize,
) -> WorkspacePage {
    let mut groups = Vec::new();
    groups.push(vec![state_line(
        "Readiness",
        readiness_state(app, snapshot),
    )]);
    for category in &snapshot.workspace.categories {
        let mut group = vec![
            state_line(&category.label, category.state),
            info_line("  ", category.summary.clone()),
        ];
        if let Some(source) = &category.source {
            group.push(info_line("  ", readiness_source_text(category, source)));
        }
        groups.push(group);
    }
    let warnings = warning_lines(app, snapshot);
    if !warnings.is_empty() {
        groups.push(vec![
            preflight_divider_line(),
            Line::from(""),
            Line::from(styled(diagnostics_header(snapshot), STATUS_WARNING)),
        ]);
        for warning in warnings {
            groups.push(vec![warning]);
        }
    }
    if let Some(prompt) = &app.resolution_prompt {
        let mut prompt_group = vec![
            Line::from(styled("Resolve", app.accent)),
            info_line("  ", prompt.label.clone()),
            Line::from(vec![
                Span::styled("> ", Style::default().fg(app.accent)),
                Span::styled(prompt.value.clone(), Style::default().fg(TEXT_PRIMARY)),
            ]),
        ];
        if let Some(error) = &prompt.error {
            prompt_group.push(Line::from(vec![
                styled("  error - ", Color::Red),
                Span::styled(error.clone(), Style::default().fg(Color::Red)),
            ]));
        }
        prompt_group.push(info_line("  ", "Enter applies · Esc cancels"));
        groups.push(prompt_group);
    } else {
        let actions = workspace_resolution_actions(app);
        if !actions.is_empty() {
            let mut action_group = vec![Line::from(styled("Resolve", app.accent))];
            action_group.extend(actions.into_iter().map(|action| info_line("  ", action)));
            groups.push(action_group);
        }
    }
    paginate_workspace_groups(groups, app.workspace_page, max_lines, line_width)
}

fn paginate_workspace_groups(
    groups: Vec<Vec<Line<'static>>>,
    requested_page: PageCursor,
    max_lines: usize,
    line_width: usize,
) -> WorkspacePage {
    if groups.is_empty() {
        return WorkspacePage {
            lines: Vec::new(),
            footer: workspace_page_footer(0, 1),
        };
    }
    let max_body_lines = max_lines.max(1);
    let line_width = line_width.max(1);
    let mut pages: Vec<Vec<Line<'static>>> = Vec::new();
    let mut current = Vec::new();
    let mut current_height = 0;
    for group in groups {
        let separator = usize::from(!current.is_empty());
        let group_height = visual_lines_height(&group, line_width);
        let needed = separator + group_height;
        if !current.is_empty() && current_height + needed > max_body_lines {
            pages.push(current);
            current = Vec::new();
            current_height = 0;
        }
        if !current.is_empty() {
            current.push(Line::from(""));
            current_height += 1;
        }
        current_height += group_height;
        current.extend(group);
    }
    if !current.is_empty() {
        pages.push(current);
    }
    let page_count = pages.len().max(1);
    let page_index = page_index_for_cursor(requested_page, page_count);
    let lines = pages.into_iter().nth(page_index).unwrap_or_default();
    WorkspacePage {
        lines,
        footer: workspace_page_footer(page_index, page_count),
    }
}

fn workspace_page_footer(page_index: usize, page_count: usize) -> Line<'static> {
    if page_count > 1 {
        info_line(
            "",
            format!(
                "Page {}/{} · PgUp next · PgDn prev",
                page_index + 1,
                page_count
            ),
        )
    } else {
        info_line("", "Page 1/1")
    }
}

fn visual_lines_height(lines: &[Line<'static>], width: usize) -> usize {
    lines
        .iter()
        .map(|line| visual_line_height(&line.to_string(), width))
        .sum()
}

fn visual_line_height(text: &str, width: usize) -> usize {
    if text.is_empty() {
        return 1;
    }
    text.lines()
        .map(|line| {
            let chars = line.chars().count().max(1);
            chars.div_ceil(width)
        })
        .sum::<usize>()
        .max(1)
}

fn preflight_divider_line() -> Line<'static> {
    Line::from(Span::styled(
        "─".repeat(PREFLIGHT_LINE_WIDTH),
        Style::default().fg(PANEL_BORDER_SUBTLE),
    ))
}

fn phase_objective_content(app: &TuiApp) -> Vec<Line<'static>> {
    let objective = app
        .run_snapshot()
        .and_then(|run| run.phase_objective.clone())
        .unwrap_or_else(|| "Preparing the next Harness step.".into());
    vec![Line::from(objective)]
}

fn effective_capabilities_content(app: &TuiApp) -> Vec<Line<'static>> {
    if let TuiState::Ready { controller } = &app.state {
        let plan = controller.plan();
        return vec![
            label_value_line(
                "Tools",
                format!(
                    "{} ready",
                    grouped_capability_counts(plan, "tool").available
                ),
            ),
            label_value_line(
                "Skills",
                format!(
                    "{} ready",
                    grouped_capability_counts(plan, "skill").available
                ),
            ),
            label_value_line(
                "Memory Spaces",
                format!(
                    "{} ready",
                    grouped_capability_counts(plan, "memory").available
                ),
            ),
            label_value_line(
                "Knowledge",
                format!(
                    "{} ready",
                    grouped_capability_counts(plan, "knowledge").available
                ),
            ),
        ];
    }
    let Some(snapshot) = app.snapshot() else {
        return vec![Line::from("Capabilities pending.")];
    };
    let mut lines = Vec::new();
    for (label, category_label) in [
        ("Tools", "Tools"),
        ("Skills", "Skills"),
        ("Memory Spaces", "Memory"),
        ("Knowledge", "Knowledge"),
    ] {
        let summary = snapshot
            .workspace
            .categories
            .iter()
            .find(|category| category.label == category_label)
            .map(|category| category.summary.clone())
            .unwrap_or_else(|| "pending".into());
        lines.push(label_value_line(label, summary));
    }
    lines
}

fn usage_content(app: &TuiApp) -> Vec<Line<'static>> {
    let Some(snapshot) = app.snapshot() else {
        return vec![Line::from("Usage pending.")];
    };
    vec![
        Line::from(vec![
            Span::styled("Run: ", Style::default().fg(TEXT_MUTED)),
            Span::styled(
                format!("{} model calls", snapshot.run.usage.model_calls),
                Style::default()
                    .fg(TEXT_PRIMARY)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!(
                    " · {} tok",
                    token_usage_text(snapshot.run.usage.tokens.total_tokens)
                ),
                Style::default().fg(TEXT_MUTED),
            ),
            Span::raw("    "),
            Span::styled("Session: ", Style::default().fg(TEXT_MUTED)),
            Span::styled(
                format!("{} runs total", snapshot.usage.started_runs),
                Style::default().fg(TEXT_PRIMARY),
            ),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            format!(
                "cost: {}",
                cost_usage_text(
                    snapshot.run.usage.cost.amount,
                    snapshot.run.usage.cost.currency.as_deref()
                )
            ),
            Style::default().fg(TEXT_MUTED),
        )),
    ]
}

struct AssistantOutputPage {
    lines: Vec<Line<'static>>,
    footer: Line<'static>,
}

fn assistant_output_page(app: &TuiApp, max_lines: usize, line_width: usize) -> AssistantOutputPage {
    paginate_assistant_output_lines(
        assistant_output_lines(app),
        app.assistant_output_page,
        max_lines,
        line_width,
    )
}

fn assistant_output_lines(app: &TuiApp) -> Vec<Line<'static>> {
    let Some(snapshot) = app.snapshot() else {
        return assistant_output_empty_lines();
    };
    let visible_items = visible_transcript_items(snapshot);
    let mut lines = Vec::new();
    let mut current_phase: Option<String> = None;
    let mut phase_item_count = 0usize;
    for item in visible_items {
        if item.phase_label != current_phase {
            if !lines.is_empty() {
                lines.push(Line::from(""));
            }
            let label = item.phase_label.clone().unwrap_or_else(|| "phase".into());
            lines.push(Line::from(styled(label, app.accent)));
            lines.push(Line::from(""));
            current_phase = item.phase_label.clone();
            phase_item_count = 0;
        } else if phase_item_count > 0 {
            lines.push(Line::from(""));
        }
        lines.extend(transcript_item_lines(item));
        phase_item_count += 1;
    }
    if run_visual_state(app) == RunVisualState::Terminal
        && let Some(output) = terminal_output_text(app)
    {
        if !lines.is_empty() {
            lines.push(Line::from(""));
        }
        lines.push(Line::from(styled("Terminal output", app.accent)));
        for line in output.lines() {
            lines.push(Line::from(line.to_string()));
        }
    }
    if lines.is_empty()
        && let Some(output) = latest_output_text(app)
    {
        lines.push(Line::from(styled("Output", app.accent)));
        for line in output.lines() {
            lines.push(Line::from(line.to_string()));
        }
    }
    if lines.is_empty() {
        assistant_output_empty_lines()
    } else {
        lines
    }
}

fn assistant_output_empty_lines() -> Vec<Line<'static>> {
    vec![Line::from(Span::styled(
        "No assistant or PhaseResult output yet.",
        Style::default().fg(TEXT_MUTED),
    ))]
}

fn visible_transcript_items(snapshot: &TuiSessionSnapshot) -> Vec<&TuiRunTranscriptItem> {
    if matches!(
        snapshot.run.status,
        TuiRunStatus::Active | TuiRunStatus::PendingApproval
    ) {
        let current_phase = active_transcript_phase_label(snapshot);
        return snapshot
            .run
            .transcript
            .iter()
            .filter(|item| {
                current_phase
                    .as_ref()
                    .is_none_or(|phase| item.phase_label.as_deref() == Some(phase.as_str()))
            })
            .collect();
    }
    snapshot.run.transcript.iter().collect()
}

fn active_transcript_phase_label(snapshot: &TuiSessionSnapshot) -> Option<String> {
    snapshot
        .run
        .phase_id
        .as_ref()
        .filter(|phase_id| phase_id.as_str() != "starting")
        .cloned()
        .or_else(|| {
            snapshot
                .run
                .transcript
                .iter()
                .rev()
                .find_map(|item| item.phase_label.clone())
        })
}

fn transcript_item_lines(item: &TuiRunTranscriptItem) -> Vec<Line<'static>> {
    match &item.kind {
        TuiRunTranscriptKind::Assistant { content } => {
            let mut lines = vec![Line::from(styled("Assistant", TEXT_MUTED)), Line::from("")];
            lines.extend(
                content
                    .trim()
                    .lines()
                    .map(|line| Line::from(line.to_string())),
            );
            lines
        }
        TuiRunTranscriptKind::Repair { message } => vec![
            Line::from(styled("Repair", STATUS_WARNING)),
            Line::from(message.clone()),
        ],
        TuiRunTranscriptKind::PhaseResult { outcome, output } => {
            let mut lines = vec![Line::from(styled("PhaseResult", TEXT_MUTED))];
            if let Some(outcome) = outcome {
                lines.push(Line::from(format!("outcome: {outcome}")));
            }
            if let Some(output) = output {
                lines.push(Line::from(format!("output: {}", pretty_json_text(output))));
            }
            lines
        }
    }
}

fn paginate_assistant_output_lines(
    lines: Vec<Line<'static>>,
    requested_page: PageCursor,
    max_lines: usize,
    line_width: usize,
) -> AssistantOutputPage {
    let max_body_lines = max_lines.max(1);
    let wrapped_lines = wrap_lines_for_width(lines, line_width.max(1));
    let mut pages: Vec<Vec<Line<'static>>> = wrapped_lines
        .chunks(max_body_lines)
        .map(|chunk| chunk.to_vec())
        .collect();
    if pages.is_empty() {
        pages.push(assistant_output_empty_lines());
    }
    let page_count = pages.len().max(1);
    let page_index = page_index_for_cursor(requested_page, page_count);
    AssistantOutputPage {
        lines: pages.into_iter().nth(page_index).unwrap_or_default(),
        footer: assistant_output_footer(page_index, page_count),
    }
}

fn wrap_lines_for_width(lines: Vec<Line<'static>>, width: usize) -> Vec<Line<'static>> {
    let width = width.max(1);
    let mut wrapped = Vec::new();
    for line in lines {
        if visual_lines_height(std::slice::from_ref(&line), width) <= 1 {
            wrapped.push(line);
            continue;
        }
        let text = line.to_string();
        if text.is_empty() {
            wrapped.push(Line::from(""));
            continue;
        }
        let chars = text.chars().collect::<Vec<_>>();
        for chunk in chars.chunks(width) {
            wrapped.push(Line::from(chunk.iter().collect::<String>()));
        }
    }
    wrapped
}

fn assistant_output_footer(page_index: usize, page_count: usize) -> Line<'static> {
    if page_count > 1 {
        info_line(
            "",
            format!(
                "Page {}/{} · PgUp next · PgDn prev · O full",
                page_index + 1,
                page_count
            ),
        )
    } else {
        info_line("", "Page 1/1 · O full")
    }
}

fn run_summary_content(app: &TuiApp) -> Vec<Line<'static>> {
    let Some(snapshot) = app.snapshot() else {
        return vec![Line::from("Run summary unavailable.")];
    };
    match (
        snapshot.reports.current_report.as_ref(),
        snapshot.reports.current_report_error.as_ref(),
    ) {
        (Some(report), _) => vec![
            label_value_line(
                "Terminal status",
                terminal_status_label(report.terminal_status).to_string(),
            ),
            label_value_line(
                "Duration",
                report
                    .duration_ms
                    .map(format_duration_ms)
                    .unwrap_or_else(|| "unknown".into()),
            ),
            label_value_line("Checkpoints", checkpoint_summary_text(report)),
            label_value_line("Phase path", phase_path_text(report)),
        ],
        (None, None) => vec![
            label_value_line(
                "Terminal status",
                snapshot
                    .run
                    .terminal_status
                    .map(terminal_status_label)
                    .unwrap_or("unknown")
                    .to_string(),
            ),
            label_value_line("Duration", "unknown".into()),
            label_value_line("Checkpoints", "unknown".into()),
            label_value_line("Phase path", "unknown".into()),
        ],
        (None, Some(err)) => vec![
            Line::from(styled("Report unavailable", STATUS_WARNING)),
            Line::from(err.clone()),
        ],
    }
}

fn label_value_line(label: &'static str, value: String) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("{label:<16}"), Style::default().fg(TEXT_MUTED)),
        Span::styled(
            value,
            Style::default()
                .fg(TEXT_PRIMARY)
                .add_modifier(Modifier::BOLD),
        ),
    ])
}

fn latest_output_text(app: &TuiApp) -> Option<String> {
    if let Some(value) = app
        .run_snapshot()
        .and_then(|run| run.latest_output.as_ref())
    {
        return Some(pretty_json_text(value));
    }
    let snapshot = app.snapshot()?;
    if snapshot.run.status != TuiRunStatus::Terminal {
        return latest_phase_result_output_text(snapshot);
    }
    match snapshot.reports.current_report.as_ref() {
        Some(report) => report
            .terminal_output
            .as_ref()
            .map(pretty_json_text)
            .or_else(|| latest_phase_result_output_text(snapshot)),
        None => latest_phase_result_output_text(snapshot),
    }
}

fn assistant_output_text(app: &TuiApp) -> Option<String> {
    let snapshot = app.snapshot()?;
    if snapshot.run.transcript.is_empty()
        && terminal_output_text(app).is_none()
        && latest_output_text(app).is_none()
    {
        return None;
    }
    Some(
        assistant_output_lines(app)
            .iter()
            .map(Line::to_string)
            .collect::<Vec<_>>()
            .join("\n"),
    )
}

fn terminal_output_text(app: &TuiApp) -> Option<String> {
    let snapshot = app.snapshot()?;
    if snapshot.run.status != TuiRunStatus::Terminal {
        return None;
    }
    snapshot
        .reports
        .current_report
        .as_ref()
        .and_then(|report| report.terminal_output.as_ref().map(pretty_json_text))
}

fn latest_phase_result_output_text(snapshot: &TuiSessionSnapshot) -> Option<String> {
    snapshot.trace.events.iter().rev().find_map(|event| {
        if snapshot.run.run_id.as_deref().is_some()
            && event.run_id.as_deref() != snapshot.run.run_id.as_deref()
        {
            return None;
        }
        if event.event_type != HarnessEventType::PhaseResultReady {
            return None;
        }
        let HarnessEventPayload::Phase {
            output: Some(output),
            ..
        } = &event.payload
        else {
            return None;
        };
        Some(pretty_json_text(output))
    })
}

fn pretty_json_text(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(text) => text.clone(),
        _ => serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string()),
    }
}

fn token_usage_text(tokens: Option<u64>) -> String {
    tokens
        .map(compact_count_for_tui)
        .unwrap_or_else(|| "unknown".into())
}

fn compact_count_for_tui(value: u64) -> String {
    if value >= 1000 {
        format!("{:.1}k", value as f64 / 1000.0)
    } else {
        value.to_string()
    }
}

fn cost_usage_text(amount: Option<f64>, currency: Option<&str>) -> String {
    match (amount, currency) {
        (Some(amount), Some(currency)) => format!("{amount:.4} {currency}"),
        (Some(amount), None) => format!("{amount:.4}"),
        _ => "unknown (provider does not report pricing)".into(),
    }
}

fn format_duration_ms(duration_ms: u64) -> String {
    if duration_ms >= 60_000 {
        format!("{:.1}m", duration_ms as f64 / 60_000.0)
    } else if duration_ms >= 1_000 {
        format!("{:.1}s", duration_ms as f64 / 1_000.0)
    } else {
        format!("{duration_ms}ms")
    }
}

fn format_clock_duration_ms(duration_ms: u64) -> String {
    let total_seconds = duration_ms / 1_000;
    let hours = total_seconds / 3_600;
    let minutes = (total_seconds % 3_600) / 60;
    let seconds = total_seconds % 60;
    format!("{hours:02}:{minutes:02}:{seconds:02}")
}

fn checkpoint_summary_text(report: &RunReport) -> String {
    if report.checkpoint_summaries.is_empty() {
        return "none".into();
    }
    let approved = report
        .checkpoint_summaries
        .iter()
        .filter(|checkpoint| checkpoint.status == "approved")
        .count();
    let denied = report
        .checkpoint_summaries
        .iter()
        .filter(|checkpoint| checkpoint.status == "denied")
        .count();
    format!(
        "{} total · {} approved · {} denied",
        report.checkpoint_summaries.len(),
        approved,
        denied
    )
}

fn phase_path_text(report: &RunReport) -> String {
    if report.phase_summaries.is_empty() {
        return "unknown".into();
    }
    let mut parts = Vec::new();
    for phase in &report.phase_summaries {
        parts.push(phase.phase_id.clone());
        if let Some(transition) = &phase.transition_to
            && (transition == "$end" || transition.starts_with('$'))
        {
            parts.push(transition.clone());
        }
    }
    parts.dedup();
    parts.join(" -> ")
}

fn trace_lines(app: &TuiApp) -> Vec<Line<'static>> {
    match &app.state {
        TuiState::Loading { progress, .. } => {
            let mut lines = vec![Line::from("bootstrap_started")];
            for item in progress.iter().rev().take(8).rev() {
                lines.push(Line::from(format!(
                    "{}: {}",
                    bootstrap_stage_label(item.stage),
                    item.message
                )));
            }
            lines
        }
        TuiState::Failed { .. } => vec![Line::from("preflight_failed")],
        TuiState::Running {
            snapshot, progress, ..
        } => {
            let mut lines = snapshot
                .trace
                .events
                .iter()
                .rev()
                .take(24)
                .rev()
                .map(|event| Line::from(event_trace_line(event)))
                .collect::<Vec<_>>();
            for item in progress.iter().rev().take(4).rev() {
                lines.push(Line::from(format!("run_starting  {}", item.message)));
            }
            if lines.is_empty() {
                lines.push(Line::from("Run starting."));
            }
            lines
        }
        TuiState::Ready { controller } => {
            let mut lines = Vec::new();
            for event in controller
                .snapshot()
                .trace
                .events
                .iter()
                .rev()
                .take(28)
                .rev()
            {
                lines.push(Line::from(event_trace_line(event)));
            }
            if lines.is_empty() {
                lines.push(Line::from("No session events yet."));
            }
            lines
        }
    }
}

struct TracePage {
    lines: Vec<Line<'static>>,
    footer: Line<'static>,
}

fn trace_page(app: &TuiApp, max_lines: usize) -> TracePage {
    let lines = center_trace_event_lines(app);
    paginate_trace_lines(lines, app.trace_page, max_lines)
}

fn center_trace_event_lines(app: &TuiApp) -> Vec<Line<'static>> {
    if let Some(snapshot) = app.snapshot() {
        if !snapshot.reports.current_trace_events.is_empty() {
            return snapshot
                .reports
                .current_trace_events
                .iter()
                .map(|event| Line::from(event_trace_line(event)))
                .collect();
        }
        if let Some(err) = &snapshot.reports.current_trace_error {
            return vec![
                Line::from(styled("Trace unavailable", STATUS_WARNING)),
                Line::from(err.clone()),
            ];
        }
    }
    trace_lines(app)
}

fn paginate_trace_lines(
    lines: Vec<Line<'static>>,
    requested_page: PageCursor,
    max_lines: usize,
) -> TracePage {
    let max_body_lines = max_lines.max(1);
    let mut pages: Vec<Vec<Line<'static>>> = lines
        .chunks(max_body_lines)
        .map(|chunk| chunk.to_vec())
        .collect();
    if pages.is_empty() {
        pages.push(vec![Line::from("No trace events yet.")]);
    }
    let page_count = pages.len().max(1);
    let page_index = page_index_for_cursor(requested_page, page_count);
    TracePage {
        lines: pages.into_iter().nth(page_index).unwrap_or_default(),
        footer: trace_page_footer(page_index, page_count),
    }
}

fn page_index_for_cursor(cursor: PageCursor, page_count: usize) -> usize {
    cursor.rem_euclid(page_count.max(1) as PageCursor) as usize
}

fn trace_page_footer(page_index: usize, page_count: usize) -> Line<'static> {
    if page_count > 1 {
        info_line(
            "",
            format!(
                "Page {}/{} · PgUp next · PgDn prev",
                page_index + 1,
                page_count
            ),
        )
    } else {
        info_line("", "Page 1/1")
    }
}

fn memory_lines(app: &TuiApp) -> Vec<Line<'_>> {
    match &app.state {
        TuiState::Ready { controller } => {
            let plan = controller.plan();
            let memory = grouped_capability_counts(plan, "memory");
            vec![
                center_header_line(app),
                Line::from(""),
                center_tabs_line(app),
                Line::from(""),
                Line::from(styled("Memory", app.accent)),
                Line::from(format!("available: {}", memory.available)),
                Line::from(format!("pending: {}", memory.pending)),
                Line::from(format!("suppressed: {}", memory.suppressed)),
                Line::from(format!("unavailable: {}", memory.unavailable)),
            ]
        }
        TuiState::Running { snapshot, .. } => vec![
            center_header_line(app),
            Line::from(""),
            center_tabs_line(app),
            Line::from(""),
            Line::from(styled("Memory", app.accent)),
            Line::from(format!(
                "Run active: {}",
                snapshot.run.run_id.as_deref().unwrap_or("unknown")
            )),
            Line::from("Memory activity detail lands in the Memory panel milestone."),
        ],
        _ => vec![
            center_header_line(app),
            Line::from(""),
            center_tabs_line(app),
            Line::from(""),
            Line::from("Memory state pending preflight."),
        ],
    }
}

fn report_lines(app: &TuiApp) -> Vec<Line<'_>> {
    match &app.state {
        TuiState::Ready { controller } => {
            let plan = controller.plan();
            let mut lines = vec![
                center_header_line(app),
                Line::from(""),
                center_tabs_line(app),
                Line::from(""),
            ];
            match (
                controller.snapshot().reports.current_report.as_ref(),
                controller.snapshot().reports.current_report_error.as_ref(),
            ) {
                (Some(report), _) => {
                    lines.push(Line::from(styled("Run Report", app.accent)));
                    lines.push(Line::from(format!("run_id: {}", report.run_id)));
                    lines.push(Line::from(format!(
                        "status: {}",
                        terminal_status_label(report.terminal_status)
                    )));
                    lines.push(Line::from(format!(
                        "trace: {}",
                        report.trace_path.as_deref().unwrap_or("not recorded")
                    )));
                }
                (None, None) => {
                    lines.push(Line::from(styled("Preflight Report", app.accent)));
                    lines.push(Line::from(format!(
                        "status: {}",
                        preflight_status_label(plan.report.status)
                    )));
                    lines.push(Line::from(format!(
                        "diagnostics: {}",
                        plan.report.diagnostics.len()
                    )));
                    lines.push(Line::from(format!(
                        "workspace: {}",
                        plan.workspace_root.display()
                    )));
                    lines.push(Line::from(format!(
                        "state_dir: {}",
                        plan.state_dir.display()
                    )));
                }
                (None, Some(err)) => {
                    lines.push(Line::from(styled("Report unavailable", STATUS_WARNING)));
                    lines.push(Line::from(err.clone()));
                }
            }
            lines
        }
        TuiState::Running { snapshot, .. } => vec![
            center_header_line(app),
            Line::from(""),
            center_tabs_line(app),
            Line::from(""),
            Line::from(styled("Run Report", app.accent)),
            Line::from(format!(
                "run_id: {}",
                snapshot.run.run_id.as_deref().unwrap_or("pending")
            )),
            Line::from("Report will be written when the Run reaches a terminal status."),
        ],
        TuiState::Failed { message } => vec![
            center_header_line(app),
            Line::from(""),
            center_tabs_line(app),
            Line::from(""),
            Line::from(styled(
                "Preflight failed before report was available",
                Color::Red,
            )),
            Line::from(message.clone()),
        ],
        TuiState::Loading { .. } => vec![
            center_header_line(app),
            Line::from(""),
            center_tabs_line(app),
            Line::from(""),
            Line::from("Report pending preflight."),
        ],
    }
}

fn center_header_line(app: &TuiApp) -> Line<'static> {
    if let TuiState::Ready { controller } = &app.state {
        let snapshot = controller.snapshot();
        if snapshot.run.status == TuiRunStatus::Terminal {
            return terminal_header_line(snapshot, app.accent);
        }
    }
    let (run, phase, status, status_color) = match &app.state {
        TuiState::Loading { .. } => (
            "Run --".to_string(),
            "bootstrap",
            "Loading".to_string(),
            STATUS_WARNING,
        ),
        TuiState::Ready { controller } => {
            let snapshot = controller.snapshot();
            (
                run_header_text(snapshot.run.run_id.as_deref()),
                snapshot.run.phase_id.as_deref().unwrap_or("idle"),
                run_status_display_text(&snapshot.run),
                run_status_display_color(&snapshot.run),
            )
        }
        TuiState::Running { snapshot, .. } => (
            run_header_text(snapshot.run.run_id.as_deref()),
            snapshot.run.phase_id.as_deref().unwrap_or("starting"),
            "In Progress".into(),
            STATUS_WARNING,
        ),
        TuiState::Failed { .. } => (
            "Run --".to_string(),
            "preflight",
            "Failed".to_string(),
            Color::Red,
        ),
    };
    active_header_line(app.accent, run, phase, status, status_color)
}

fn active_header_line(
    accent: Color,
    run: String,
    phase: &str,
    status: String,
    status_color: Color,
) -> Line<'static> {
    Line::from(vec![
        Span::styled(
            run,
            Style::default()
                .fg(TEXT_PRIMARY)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("  "),
        Span::styled("Phase: ", Style::default().fg(TEXT_MUTED)),
        Span::styled(
            phase.to_string(),
            Style::default().fg(accent).add_modifier(Modifier::BOLD),
        ),
        Span::raw("  "),
        Span::styled("Status: ", Style::default().fg(TEXT_MUTED)),
        Span::styled(
            status,
            Style::default()
                .fg(status_color)
                .add_modifier(Modifier::BOLD),
        ),
    ])
}

fn terminal_header_line(snapshot: &TuiSessionSnapshot, accent: Color) -> Line<'static> {
    let report = snapshot.reports.current_report.as_ref();
    let terminal_status = report
        .map(|report| report.terminal_status)
        .or(snapshot.run.terminal_status);
    let status_text = terminal_status
        .map(terminal_status_label)
        .unwrap_or("terminal")
        .to_string();
    let status_color = terminal_status
        .map(terminal_status_color)
        .unwrap_or(TEXT_PRIMARY);
    let terminal_target = report
        .and_then(report_terminal_target)
        .or_else(|| snapshot.run.phase_id.clone())
        .unwrap_or_else(|| status_text.clone());
    Line::from(vec![
        Span::styled(
            run_header_text(snapshot.run.run_id.as_deref()),
            Style::default()
                .fg(TEXT_PRIMARY)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("  "),
        Span::styled("Terminal: ", Style::default().fg(TEXT_MUTED)),
        Span::styled(
            terminal_target,
            Style::default().fg(accent).add_modifier(Modifier::BOLD),
        ),
        Span::styled(" -> ", Style::default().fg(TEXT_MUTED)),
        Span::styled(
            status_text.clone(),
            Style::default()
                .fg(status_color)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("  "),
        Span::styled(
            terminal_status_badge_text(terminal_status),
            Style::default()
                .fg(status_color)
                .add_modifier(Modifier::BOLD),
        ),
    ])
}

fn report_terminal_target(report: &RunReport) -> Option<String> {
    let phase = report.phase_summaries.last()?;
    phase
        .transition_to
        .clone()
        .or_else(|| phase.outcome.clone())
        .or_else(|| Some(phase.phase_id.clone()))
}

fn terminal_status_badge_text(status: Option<HarnessTerminalStatus>) -> String {
    let label = status
        .map(terminal_status_label)
        .unwrap_or("terminal")
        .to_ascii_uppercase();
    let mark = match status {
        Some(HarnessTerminalStatus::Ended | HarnessTerminalStatus::HandedOff) => "✓",
        Some(_) => "×",
        None => "•",
    };
    format!("[{mark} {label}]")
}

fn terminal_status_color(status: HarnessTerminalStatus) -> Color {
    match status {
        HarnessTerminalStatus::Ended | HarnessTerminalStatus::HandedOff => STATUS_READY,
        _ => Color::Red,
    }
}

fn run_status_display_text(run: &TuiRunSnapshot) -> String {
    match run.status {
        TuiRunStatus::Idle => "Ready".into(),
        TuiRunStatus::Active => "In Progress".into(),
        TuiRunStatus::PendingApproval => "Approval Required".into(),
        TuiRunStatus::Terminal => run
            .terminal_status
            .map(terminal_status_label)
            .unwrap_or("terminal")
            .to_string(),
    }
}

fn run_status_display_color(run: &TuiRunSnapshot) -> Color {
    match run.status {
        TuiRunStatus::Idle => STATUS_READY,
        TuiRunStatus::Active | TuiRunStatus::PendingApproval => STATUS_WARNING,
        TuiRunStatus::Terminal => match run.terminal_status {
            Some(HarnessTerminalStatus::Ended | HarnessTerminalStatus::HandedOff) => STATUS_READY,
            Some(_) => Color::Red,
            None => TEXT_PRIMARY,
        },
    }
}

fn run_header_text(run_id: Option<&str>) -> String {
    run_id
        .map(|run_id| format!("Run {}", run_id.rsplit('-').next().unwrap_or(run_id)))
        .unwrap_or_else(|| "Run --".into())
}

fn center_tabs_line(app: &TuiApp) -> Line<'static> {
    Line::from(vec![
        tab_span("Run", app.panel == VisiblePanel::Run, app.accent),
        tab_sep(),
        tab_span("Trace", app.panel == VisiblePanel::Trace, app.accent),
        tab_sep(),
        tab_span("Memory", app.panel == VisiblePanel::Memory, app.accent),
        tab_sep(),
        tab_span("Reports", app.panel == VisiblePanel::Reports, app.accent),
    ])
}

fn tab_span(label: &'static str, selected: bool, accent: Color) -> Span<'static> {
    let text = format!(" {label} ");
    if selected {
        Span::styled(
            text,
            Style::default()
                .fg(accent)
                .add_modifier(Modifier::BOLD | Modifier::REVERSED),
        )
    } else {
        Span::styled(text, Style::default().fg(TEXT_MUTED))
    }
}

fn tab_sep() -> Span<'static> {
    Span::styled(" | ", Style::default().fg(TEXT_DIM))
}

fn readiness_state(app: &TuiApp, snapshot: &TuiSessionSnapshot) -> CapabilityState {
    if snapshot
        .workspace
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity == PreflightDiagnosticSeverity::Fatal)
    {
        CapabilityState::Unavailable
    } else if app.has_resolution_actions()
        || snapshot
            .workspace
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == PreflightDiagnosticSeverity::Pending)
    {
        CapabilityState::Pending
    } else if snapshot.workspace.diagnostics.iter().any(|diagnostic| {
        matches!(
            diagnostic.severity,
            PreflightDiagnosticSeverity::Warning | PreflightDiagnosticSeverity::Suppressed
        )
    }) {
        CapabilityState::Suppressed
    } else {
        CapabilityState::Available
    }
}

fn workspace_resolution_actions(app: &TuiApp) -> Vec<String> {
    let mut actions = Vec::new();
    if app.can_prompt_agent_selector() {
        actions.push("A select Agent".into());
    }
    if app.can_prompt_model() {
        actions.push("P set model provider/model".into());
    }
    if app.can_prompt_scope() {
        actions.push("S set missing runtime scope".into());
    }
    actions
}

fn readiness_source_text(category: &TuiReadinessCategory, source: &str) -> String {
    if category.label == "Agent" {
        "selected from workspace".into()
    } else {
        format!("from {source}")
    }
}

fn diagnostics_header(snapshot: &TuiSessionSnapshot) -> &'static str {
    if snapshot
        .workspace
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity == PreflightDiagnosticSeverity::Fatal)
    {
        "Diagnostics"
    } else {
        "Warnings"
    }
}

fn warning_lines(app: &TuiApp, snapshot: &TuiSessionSnapshot) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    for warning in &app.warnings {
        lines.push(Line::from(vec![
            styled("warning", STATUS_WARNING),
            Span::raw(format!(" - {warning}")),
        ]));
    }
    for diagnostic in &snapshot.workspace.diagnostics {
        if matches!(
            diagnostic.severity,
            PreflightDiagnosticSeverity::Fatal
                | PreflightDiagnosticSeverity::Warning
                | PreflightDiagnosticSeverity::Suppressed
                | PreflightDiagnosticSeverity::Pending
        ) {
            let detail = if app.workspace_detail_expanded {
                format!("{}: {}", diagnostic.code, diagnostic.message)
            } else {
                diagnostic.code.clone()
            };
            let color = if diagnostic.severity == PreflightDiagnosticSeverity::Fatal {
                Color::Red
            } else {
                STATUS_WARNING
            };
            lines.push(Line::from(vec![
                styled(diagnostic_severity_label(diagnostic.severity), color),
                Span::raw(format!(" - {detail}")),
            ]));
        }
    }
    lines
}

fn state_line(label: &str, state: CapabilityState) -> Line<'static> {
    let (mark, color, text) = match state {
        CapabilityState::Available => ("✓", STATUS_READY, "Ready"),
        CapabilityState::Pending => ("•", STATUS_WARNING, "Pending"),
        CapabilityState::Unavailable => ("×", Color::Red, "Unavailable"),
        CapabilityState::Suppressed => ("!", STATUS_WARNING, "Suppressed"),
        CapabilityState::NotConfigured => ("-", Color::DarkGray, "Not configured"),
    };
    let prefix_width = 2 + label.chars().count();
    let gap = PREFLIGHT_LINE_WIDTH
        .saturating_sub(prefix_width + text.chars().count())
        .max(1);
    Line::from(vec![
        styled(mark, color),
        Span::raw(format!(" {label}{}", " ".repeat(gap))),
        styled(text, color),
    ])
}

fn info_line(prefix: &str, value: impl Into<String>) -> Line<'static> {
    Line::from(Span::styled(
        format!("{prefix}{}", value.into()),
        Style::default().fg(Color::DarkGray),
    ))
}

fn styled<'a>(text: impl Into<std::borrow::Cow<'a, str>>, color: Color) -> Span<'a> {
    Span::styled(text, Style::default().fg(color))
}

fn terminal_supports_tui(
    term: Option<&str>,
    stdout_is_terminal: bool,
    stderr_is_terminal: bool,
) -> bool {
    stdout_is_terminal
        && stderr_is_terminal
        && term
            .map(|term| {
                let normalized = term.trim().to_ascii_lowercase();
                !normalized.is_empty() && normalized != "dumb"
            })
            .unwrap_or(true)
}

#[cfg(test)]
mod tests {
    use super::super::{harness_engine_options_from_plan, runtime_snapshot_from_plan};
    use super::*;
    use crate::harness_config::HarnessConfigSourceKind;
    use crate::harness_engine::{
        HarnessEngine, HarnessRuntimeServices, HarnessSession, RuntimeTerminalResult,
    };
    use crate::harness_observability::{
        HarnessEventBuilder, HarnessEventPayload, HarnessEventSink, HarnessEventType,
        RunOutputPaths, RunReport, RunUsage,
    };
    use crate::harness_plan::{HarnessPlanProgress, HarnessPlanProgressStage, ResolvedHarnessPlan};
    use crate::harness_runtime::action::ScriptedActionDispatcher;
    use crate::harness_runtime::approval::ScriptedApprovalController;
    use crate::harness_runtime::model::ScriptedModelRuntime;
    use std::collections::BTreeMap;
    use std::sync::{atomic::AtomicBool, mpsc};

    #[test]
    fn terminal_support_requires_interactive_streams_and_non_dumb_term() {
        assert!(terminal_supports_tui(Some("xterm-256color"), true, true));
        assert!(!terminal_supports_tui(Some("dumb"), true, true));
        assert!(!terminal_supports_tui(Some(""), true, true));
        assert!(!terminal_supports_tui(Some("xterm-256color"), false, true));
        assert!(!terminal_supports_tui(Some("xterm-256color"), true, false));
    }

    #[test]
    fn parses_configured_hex_accent_and_falls_back_for_invalid_internal_value() {
        assert_eq!(
            parse_hex_color("#2563EB"),
            Some(Color::Rgb(0x25, 0x63, 0xeb))
        );
        assert_eq!(parse_hex_color("blue"), None);
        assert_eq!(parse_hex_color("#GGGGGG"), None);
    }

    #[test]
    fn focus_cycles_through_single_panel_tabs() {
        assert_eq!(
            next_focus_panel(VisiblePanel::Workspace, LayoutMode::Single),
            VisiblePanel::Run
        );
        assert_eq!(
            next_focus_panel(VisiblePanel::Run, LayoutMode::Single),
            VisiblePanel::Trace
        );
        assert_eq!(
            next_focus_panel(VisiblePanel::Trace, LayoutMode::Single),
            VisiblePanel::Memory
        );
        assert_eq!(
            next_focus_panel(VisiblePanel::Memory, LayoutMode::Single),
            VisiblePanel::Reports
        );
        assert_eq!(
            next_focus_panel(VisiblePanel::Reports, LayoutMode::Single),
            VisiblePanel::EventStream
        );
        assert_eq!(
            next_focus_panel(VisiblePanel::EventStream, LayoutMode::Single),
            VisiblePanel::Workspace
        );
        assert_eq!(
            previous_focus_panel(VisiblePanel::Workspace, LayoutMode::Single),
            VisiblePanel::EventStream
        );
        assert_eq!(
            previous_focus_panel(VisiblePanel::Run, LayoutMode::Single),
            VisiblePanel::Workspace
        );
    }

    #[test]
    fn focus_cycles_medium_layout_through_workspace_and_center_tabs() {
        assert_eq!(
            next_focus_panel(VisiblePanel::Workspace, LayoutMode::Medium),
            VisiblePanel::Run
        );
        assert_eq!(
            next_focus_panel(VisiblePanel::Run, LayoutMode::Medium),
            VisiblePanel::Trace
        );
        assert_eq!(
            next_focus_panel(VisiblePanel::Trace, LayoutMode::Medium),
            VisiblePanel::Memory
        );
        assert_eq!(
            next_focus_panel(VisiblePanel::Memory, LayoutMode::Medium),
            VisiblePanel::Reports
        );
        assert_eq!(
            next_focus_panel(VisiblePanel::Reports, LayoutMode::Medium),
            VisiblePanel::EventStream
        );
        assert_eq!(
            next_focus_panel(VisiblePanel::EventStream, LayoutMode::Medium),
            VisiblePanel::Workspace
        );
        assert_eq!(
            previous_focus_panel(VisiblePanel::Workspace, LayoutMode::Medium),
            VisiblePanel::EventStream
        );
        assert_eq!(
            previous_focus_panel(VisiblePanel::Trace, LayoutMode::Medium),
            VisiblePanel::Run
        );
    }

    #[test]
    fn focus_cycles_wide_layout_through_visible_rails_and_center_tabs() {
        assert_eq!(
            next_focus_panel(VisiblePanel::Workspace, LayoutMode::Wide),
            VisiblePanel::Run
        );
        assert_eq!(
            next_focus_panel(VisiblePanel::Run, LayoutMode::Wide),
            VisiblePanel::Trace
        );
        assert_eq!(
            next_focus_panel(VisiblePanel::Trace, LayoutMode::Wide),
            VisiblePanel::Memory
        );
        assert_eq!(
            next_focus_panel(VisiblePanel::Memory, LayoutMode::Wide),
            VisiblePanel::Reports
        );
        assert_eq!(
            next_focus_panel(VisiblePanel::Reports, LayoutMode::Wide),
            VisiblePanel::EventStream
        );
        assert_eq!(
            next_focus_panel(VisiblePanel::EventStream, LayoutMode::Wide),
            VisiblePanel::Workspace
        );
        assert_eq!(
            previous_focus_panel(VisiblePanel::Workspace, LayoutMode::Wide),
            VisiblePanel::EventStream
        );
        assert_eq!(
            previous_focus_panel(VisiblePanel::Memory, LayoutMode::Wide),
            VisiblePanel::Trace
        );
    }

    #[test]
    fn layout_mode_breakpoints_are_single_source_of_truth() {
        assert_eq!(layout_mode_for_width(120), LayoutMode::Wide);
        assert_eq!(layout_mode_for_width(119), LayoutMode::Medium);
        assert_eq!(layout_mode_for_width(88), LayoutMode::Medium);
        assert_eq!(layout_mode_for_width(87), LayoutMode::Single);
    }

    #[test]
    fn composer_focus_preserves_printable_navigation_characters() {
        let mut app =
            TuiApp::ready_with_runtime_inputs(test_controller(), HarnessArgs::default(), None);
        assert_eq!(app.focus, TuiFocus::Composer);
        assert!(app.can_send_message());

        app.composer_input.push('!');
        app.composer_input.push('3');

        assert_eq!(app.panel, VisiblePanel::Run);
        assert_eq!(app.composer_input, "!3");
        assert!(!shell_quit_requested(
            &app,
            KeyCode::Char('q'),
            KeyModifiers::empty()
        ));
        assert!(shell_quit_requested(
            &app,
            KeyCode::Char('c'),
            KeyModifiers::CONTROL
        ));

        app.focus = TuiFocus::Panel(VisiblePanel::Run);
        assert!(shell_quit_requested(
            &app,
            KeyCode::Char('q'),
            KeyModifiers::empty()
        ));
    }

    #[test]
    fn panel_focus_uses_page_keys_for_focused_panel_pagination() {
        let mut app =
            TuiApp::ready_with_runtime_inputs(test_controller(), HarnessArgs::default(), None);
        app.layout_mode = LayoutMode::Wide;
        app.focus = TuiFocus::Panel(VisiblePanel::Workspace);
        app.workspace_page = 1;
        app.trace_page = 2;
        app.assistant_output_page = 4;

        app.page_focused_panel(PanelPageDirection::Previous);
        assert_eq!(app.workspace_page, 0);
        assert_eq!(app.trace_page, 2);
        assert_eq!(app.assistant_output_page, 4);

        app.page_focused_panel(PanelPageDirection::Previous);
        assert_eq!(app.workspace_page, -1);
        assert_eq!(app.trace_page, 2);
        assert_eq!(app.assistant_output_page, 4);

        app.page_focused_panel(PanelPageDirection::Next);
        assert_eq!(app.workspace_page, 0);
        assert_eq!(app.trace_page, 2);
        assert_eq!(app.assistant_output_page, 4);

        app.focus = TuiFocus::Panel(VisiblePanel::Trace);
        app.page_focused_panel(PanelPageDirection::Next);
        assert_eq!(app.workspace_page, 0);
        assert_eq!(app.trace_page, 3);
        assert_eq!(app.assistant_output_page, 4);

        app.page_focused_panel(PanelPageDirection::Previous);
        assert_eq!(app.workspace_page, 0);
        assert_eq!(app.trace_page, 2);
        assert_eq!(app.assistant_output_page, 4);

        app.focus = TuiFocus::Panel(VisiblePanel::Run);
        app.page_focused_panel(PanelPageDirection::Next);
        assert_eq!(app.workspace_page, 0);
        assert_eq!(app.trace_page, 2);
        assert_eq!(app.assistant_output_page, 5);
    }

    #[test]
    fn pagination_cursors_wrap_in_both_directions() {
        let lines = vec![Line::from("one"), Line::from("two"), Line::from("three")];
        let previous = paginate_trace_lines(lines.clone(), -1, 1);
        assert_eq!(previous.lines[0].to_string(), "three");
        assert!(previous.footer.to_string().contains("Page 3/3"));

        let next = paginate_trace_lines(lines, 3, 1);
        assert_eq!(next.lines[0].to_string(), "one");
        assert!(next.footer.to_string().contains("Page 1/3"));
    }

    #[test]
    fn trace_pagination_slices_event_lines_and_reports_footer() {
        let lines = (1..=5)
            .map(|index| Line::from(format!("event {index}")))
            .collect::<Vec<_>>();

        let first = paginate_trace_lines(lines.clone(), 0, 2);
        assert_eq!(
            first.lines.iter().map(Line::to_string).collect::<Vec<_>>(),
            vec!["event 1", "event 2"]
        );
        assert_eq!(first.footer.to_string(), "Page 1/3 · PgUp next · PgDn prev");

        let third = paginate_trace_lines(lines, 2, 2);
        assert_eq!(
            third.lines.iter().map(Line::to_string).collect::<Vec<_>>(),
            vec!["event 5"]
        );
        assert_eq!(third.footer.to_string(), "Page 3/3 · PgUp next · PgDn prev");
    }

    #[test]
    fn assistant_output_pagination_accounts_for_wrapped_lines() {
        let lines = vec![Line::from("abcdefghijkl")];

        let first = paginate_assistant_output_lines(lines.clone(), 0, 2, 5);
        assert_eq!(
            first.lines.iter().map(Line::to_string).collect::<Vec<_>>(),
            vec!["abcde", "fghij"]
        );
        assert_eq!(
            first.footer.to_string(),
            "Page 1/2 · PgUp next · PgDn prev · O full"
        );

        let second = paginate_assistant_output_lines(lines, 1, 2, 5);
        assert_eq!(
            second.lines.iter().map(Line::to_string).collect::<Vec<_>>(),
            vec!["kl"]
        );
        assert_eq!(
            second.footer.to_string(),
            "Page 2/2 · PgUp next · PgDn prev · O full"
        );
    }

    #[test]
    fn trace_labels_are_stable_ui_copy() {
        assert_eq!(trace_level_label(&HarnessTraceLevel::Minimal), "minimal");
        assert_eq!(trace_level_label(&HarnessTraceLevel::Normal), "normal");
        assert_eq!(trace_level_label(&HarnessTraceLevel::Verbose), "verbose");
        assert_eq!(trace_content_label(&HarnessTraceContent::None), "none");
        assert_eq!(
            trace_content_label(&HarnessTraceContent::Redacted),
            "redacted"
        );
        assert_eq!(trace_content_label(&HarnessTraceContent::Full), "full");
    }

    #[test]
    fn preflight_labels_are_stable_ui_copy() {
        assert_eq!(preflight_status_label(PreflightStatus::Ready), "ready");
        assert_eq!(
            preflight_status_label(PreflightStatus::ReadyWithWarnings),
            "ready_with_warnings"
        );
        assert_eq!(
            preflight_status_label(PreflightStatus::SelectionRequired),
            "selection_required"
        );
        assert_eq!(preflight_status_label(PreflightStatus::Failed), "failed");
        assert_eq!(
            diagnostic_severity_label(PreflightDiagnosticSeverity::Fatal),
            "fatal"
        );
        assert_eq!(
            diagnostic_severity_label(PreflightDiagnosticSeverity::Warning),
            "warning"
        );
        assert_eq!(
            diagnostic_severity_label(PreflightDiagnosticSeverity::Suppressed),
            "suppressed"
        );
        assert_eq!(
            diagnostic_severity_label(PreflightDiagnosticSeverity::Pending),
            "pending"
        );
        assert_eq!(
            diagnostic_severity_label(PreflightDiagnosticSeverity::Info),
            "info"
        );
    }

    #[test]
    fn config_source_labels_are_stable_ui_copy() {
        assert_eq!(
            config_source_label(HarnessConfigSourceKind::HarnessDefault),
            "default"
        );
        assert_eq!(
            config_source_label(HarnessConfigSourceKind::ConfigFile),
            "config_file"
        );
        assert_eq!(
            config_source_label(HarnessConfigSourceKind::CliOverride),
            "cli_override"
        );
        assert_eq!(
            config_source_label(HarnessConfigSourceKind::SdkOverride),
            "sdk_override"
        );
        assert_eq!(
            config_source_label(HarnessConfigSourceKind::InteractiveOverride),
            "interactive"
        );
        assert_eq!(
            config_source_label(HarnessConfigSourceKind::Environment),
            "environment"
        );
    }

    #[test]
    fn pending_bootstrap_result_keeps_loading_state() {
        let (_sender, receiver) = mpsc::channel();
        let mut bootstrap = Some(receiver);
        let mut app = TuiApp::loading(test_harness_args());

        poll_bootstrap_result(&mut app, &mut bootstrap);

        assert!(matches!(app.state, TuiState::Loading { .. }));
        assert!(bootstrap.is_some());
    }

    #[test]
    fn disconnected_bootstrap_result_becomes_failed_state() {
        let (sender, receiver) = mpsc::channel();
        drop(sender);
        let mut bootstrap = Some(receiver);
        let mut app = TuiApp::loading(test_harness_args());

        poll_bootstrap_result(&mut app, &mut bootstrap);

        assert!(matches!(app.state, TuiState::Failed { .. }));
        assert!(bootstrap.is_none());
    }

    #[test]
    fn bootstrap_progress_updates_loading_state() {
        let (sender, receiver) = mpsc::channel();
        sender
            .send(BootstrapMessage::Progress(TuiBootstrapProgress::new(
                TuiBootstrapStage::Preflight,
                "checking readiness",
            )))
            .unwrap();
        let mut bootstrap = Some(receiver);
        let mut app = TuiApp::loading(test_harness_args());

        poll_bootstrap_result(&mut app, &mut bootstrap);

        let TuiState::Loading { progress, .. } = &app.state else {
            panic!("app should still be loading");
        };
        assert_eq!(progress.len(), 1);
        assert_eq!(progress[0].stage, TuiBootstrapStage::Preflight);
        assert!(bootstrap.is_some());
    }

    #[test]
    fn resolver_progress_maps_to_tui_bootstrap_stages() {
        let config = TuiBootstrapProgress::from_plan_progress(HarnessPlanProgress {
            stage: HarnessPlanProgressStage::Config,
            message: "loading config".into(),
        });
        assert_eq!(config.stage, TuiBootstrapStage::Bootstrap);
        assert_eq!(config.message, "loading config");

        let validation = TuiBootstrapProgress::from_plan_progress(HarnessPlanProgress {
            stage: HarnessPlanProgressStage::Validation,
            message: "validating agent".into(),
        });
        assert_eq!(validation.stage, TuiBootstrapStage::Preflight);
        assert_eq!(validation.message, "validating agent");
    }

    #[test]
    fn workspace_readiness_summary_uses_stable_categories_and_counts() {
        let mut plan = test_plan();
        plan.config.config.model = Some(crate::harness_config::HarnessModelConfig {
            provider: "openai".into(),
            model: "gpt-4o-mini".into(),
            options: serde_json::Value::Object(Default::default()),
        });
        plan.config.model_source =
            crate::harness_config::HarnessConfigSource::interactive_override();
        let readiness = workspace_readiness_from_plan(&plan);

        let profiles = readiness
            .categories
            .iter()
            .find(|category| category.label == "Profiles")
            .expect("profiles category");
        assert_eq!(profiles.summary, "1 profile ready");
        let skills = readiness
            .categories
            .iter()
            .find(|category| category.label == "Skills")
            .expect("skills category");
        assert_eq!(skills.summary, "1 skill ready");
        let tools = readiness
            .categories
            .iter()
            .find(|category| category.label == "Tools")
            .expect("tools category");
        assert_eq!(tools.summary, "1 ready, 1 suppressed");
        let memory = readiness
            .categories
            .iter()
            .find(|category| category.label == "Memory")
            .expect("memory category");
        assert_eq!(memory.state, CapabilityState::Available);
        assert_eq!(memory.summary, "1 space ready");
        let model = readiness
            .categories
            .iter()
            .find(|category| category.label == "Model")
            .expect("model category");
        assert_eq!(
            readiness_source_text(model, model.source.as_deref().unwrap()),
            "from interactive"
        );

        let agent = readiness
            .categories
            .iter()
            .find(|category| category.label == "Agent")
            .expect("agent category");
        assert_eq!(
            readiness_source_text(agent, agent.source.as_deref().unwrap()),
            "selected from workspace"
        );
    }

    #[test]
    fn workspace_readiness_paginates_without_splitting_groups() {
        let mut app =
            TuiApp::ready_with_runtime_inputs(test_controller(), HarnessArgs::default(), None);

        let first_page = workspace_page(&app, 8, PREFLIGHT_LINE_WIDTH);
        assert!(first_page.footer.to_string().contains("Page 1/"));
        assert!(first_page.footer.to_string().contains("PgUp next"));
        assert!(first_page.footer.to_string().contains("PgDn prev"));
        assert!(
            first_page
                .lines
                .iter()
                .any(|line| line.to_string().contains("Readiness"))
        );

        app.focus = TuiFocus::Panel(VisiblePanel::Workspace);
        app.page_focused_panel(PanelPageDirection::Next);
        let second_page = workspace_page(&app, 8, PREFLIGHT_LINE_WIDTH);
        assert!(second_page.footer.to_string().contains("Page 2/"));
        assert!(
            !second_page
                .lines
                .iter()
                .any(|line| line.to_string().contains("Readiness"))
        );

        let full_page = workspace_page(&app, 80, PREFLIGHT_LINE_WIDTH);
        assert_eq!(full_page.footer.to_string(), "Page 1/1");
        assert!(!full_page.footer.to_string().contains("PgUp next"));
    }

    #[test]
    fn workspace_resolution_prompt_renders_model_override_input() {
        let mut app =
            TuiApp::ready_with_runtime_inputs(test_controller(), HarnessArgs::default(), None);
        assert!(app.can_prompt_model());

        app.open_resolution_prompt(ResolutionPromptKind::Model);
        let lines = workspace_page(&app, 80, PREFLIGHT_LINE_WIDTH).lines;
        assert!(
            lines
                .iter()
                .any(|line| line.to_string().contains("Model provider/model"))
        );
        assert!(
            lines
                .iter()
                .any(|line| line.to_string().contains("Enter applies"))
        );
    }

    #[test]
    fn resolution_prompt_does_not_swallow_ctrl_c() {
        let mut app =
            TuiApp::ready_with_runtime_inputs(test_controller(), HarnessArgs::default(), None);
        app.open_resolution_prompt(ResolutionPromptKind::Model);
        let mut bootstrap = None;

        let handled = handle_resolution_prompt_key(
            &mut app,
            KeyCode::Char('c'),
            KeyModifiers::CONTROL,
            &std::env::temp_dir(),
            &mut bootstrap,
        )
        .expect("prompt key handling");

        assert!(!handled);
    }

    #[test]
    fn invalid_resolution_prompt_input_stays_inline() {
        let mut app =
            TuiApp::ready_with_runtime_inputs(test_controller(), HarnessArgs::default(), None);

        app.open_resolution_prompt(ResolutionPromptKind::Scope);
        let Some(prompt) = &mut app.resolution_prompt else {
            panic!("resolution prompt expected");
        };
        prompt.value = "user".into();
        let submitted = app
            .submit_resolution_prompt(&std::env::temp_dir())
            .expect("inline validation should not crash TUI");
        assert!(submitted.is_none());
        assert!(app.resolution_prompt.is_some());

        let lines = workspace_page(&app, 80, PREFLIGHT_LINE_WIDTH).lines;
        assert!(
            lines
                .iter()
                .any(|line| line.to_string().contains("Use KEY=VALUE."))
        );
    }

    #[test]
    fn invalid_model_provider_stays_inline() {
        let mut app =
            TuiApp::ready_with_runtime_inputs(test_controller(), HarnessArgs::default(), None);

        app.open_resolution_prompt(ResolutionPromptKind::Model);
        let Some(prompt) = &mut app.resolution_prompt else {
            panic!("resolution prompt expected");
        };
        prompt.value = "not-a-provider/model".into();
        let submitted = app
            .submit_resolution_prompt(&std::env::temp_dir())
            .expect("inline validation should not crash TUI");
        assert!(submitted.is_none());
        assert!(app.resolution_prompt.is_some());

        let lines = workspace_page(&app, 80, PREFLIGHT_LINE_WIDTH).lines;
        assert!(
            lines
                .iter()
                .any(|line| line.to_string().contains("Unknown model provider"))
        );
    }

    #[test]
    fn workspace_diagnostics_default_to_codes_and_expand_to_messages() {
        let mut app =
            TuiApp::ready_with_runtime_inputs(test_controller(), HarnessArgs::default(), None);
        assert!(!app.has_workspace_details());
        let TuiState::Ready { controller } = &mut app.state else {
            panic!("ready app expected");
        };
        controller
            .snapshot
            .workspace
            .diagnostics
            .push(TuiDiagnosticSummary {
                severity: PreflightDiagnosticSeverity::Warning,
                code: "example_warning".into(),
                message: "expanded diagnostic detail".into(),
            });
        assert!(app.has_workspace_details());

        let compact = workspace_page(&app, 80, PREFLIGHT_LINE_WIDTH).lines;
        assert!(
            compact
                .iter()
                .any(|line| line.to_string().contains("example_warning"))
        );
        assert!(
            !compact
                .iter()
                .any(|line| line.to_string().contains("expanded diagnostic detail"))
        );

        app.workspace_detail_expanded = true;
        let expanded = workspace_page(&app, 80, PREFLIGHT_LINE_WIDTH).lines;
        assert!(
            expanded
                .iter()
                .any(|line| line.to_string().contains("expanded diagnostic detail"))
        );
    }

    #[test]
    fn workspace_pagination_accounts_for_wrapped_expanded_diagnostics() {
        let mut app =
            TuiApp::ready_with_runtime_inputs(test_controller(), HarnessArgs::default(), None);
        let TuiState::Ready { controller } = &mut app.state else {
            panic!("ready app expected");
        };
        controller
            .snapshot
            .workspace
            .diagnostics
            .push(TuiDiagnosticSummary {
                severity: PreflightDiagnosticSeverity::Warning,
                code: "first_warning".into(),
                message: "This diagnostic has a long expanded message that wraps across several visual rows in the preflight rail.".into(),
            });
        controller
            .snapshot
            .workspace
            .diagnostics
            .push(TuiDiagnosticSummary {
                severity: PreflightDiagnosticSeverity::Warning,
                code: "second_warning".into(),
                message: "A second diagnostic proves that warning entries can move onto the next page when details are expanded.".into(),
            });
        app.workspace_detail_expanded = true;

        let mut first_warning_page = None;
        let mut second_warning_page = None;
        for page_index in 0..10 {
            app.workspace_page = page_index.into();
            let page = workspace_page(&app, 10, 24);
            if page
                .lines
                .iter()
                .any(|line| line.to_string().contains("first_warning"))
            {
                first_warning_page = Some(page_index);
            }
            if page
                .lines
                .iter()
                .any(|line| line.to_string().contains("second_warning"))
            {
                second_warning_page = Some(page_index);
            }
        }
        let first_warning_page = first_warning_page.expect("first warning page");
        let second_warning_page = second_warning_page.expect("second warning page");
        assert!(
            second_warning_page > first_warning_page,
            "expected second warning to paginate after first warning"
        );
    }

    #[test]
    fn workspace_readiness_is_pending_when_resolution_actions_remain() {
        let mut app =
            TuiApp::ready_with_runtime_inputs(test_controller(), HarnessArgs::default(), None);
        assert_eq!(
            readiness_state(&app, ready_snapshot(&app)),
            CapabilityState::Pending
        );

        let TuiState::Ready { controller } = &mut app.state else {
            panic!("ready app expected");
        };
        controller.plan.config.config.model = Some(crate::harness_config::HarnessModelConfig {
            provider: "openai".into(),
            model: "gpt-4o-mini".into(),
            options: serde_json::Value::Object(Default::default()),
        });
        assert_eq!(
            readiness_state(&app, ready_snapshot(&app)),
            CapabilityState::Available
        );

        let TuiState::Ready { controller } = &mut app.state else {
            panic!("ready app expected");
        };
        controller
            .snapshot
            .workspace
            .diagnostics
            .push(TuiDiagnosticSummary {
                severity: PreflightDiagnosticSeverity::Warning,
                code: "unresolved_runtime_scope".into(),
                message: "Memory runtime scope `user` is required by active Memory bindings but has no configured value.".into(),
            });
        assert_eq!(
            readiness_state(&app, ready_snapshot(&app)),
            CapabilityState::Pending
        );
    }

    #[test]
    fn selection_required_plan_builds_preflight_only_controller() {
        let mut plan = test_plan();
        plan.selected_agent = None;
        plan.loop_package = None;
        plan.report.status = PreflightStatus::SelectionRequired;
        plan.report
            .diagnostics
            .push(crate::harness_plan::PreflightDiagnostic {
                severity: PreflightDiagnosticSeverity::Fatal,
                code: "agent_selection_required".into(),
                message: "multiple runnable Agents are available; pass `agentpm harness <agent>` to select one.".into(),
                path: Some("agent.lock".into()),
            });

        let app = TuiApp::ready_with_runtime_inputs(
            TuiSessionController::new(plan).expect("preflight-only controller"),
            HarnessArgs::default(),
            None,
        );

        assert!(app.can_prompt_agent_selector());
        assert_eq!(
            readiness_state(&app, ready_snapshot(&app)),
            CapabilityState::Unavailable
        );
        let page = workspace_page(&app, 80, PREFLIGHT_LINE_WIDTH);
        assert!(
            page.lines
                .iter()
                .any(|line| line.to_string().contains("Diagnostics"))
        );
        assert!(
            page.lines
                .iter()
                .any(|line| line.to_string().contains("agent_selection_required"))
        );
    }

    #[test]
    fn agent_not_found_keeps_agent_resolution_available() {
        let mut controller = test_controller();
        controller
            .snapshot
            .workspace
            .diagnostics
            .push(TuiDiagnosticSummary {
                severity: PreflightDiagnosticSeverity::Fatal,
                code: "agent_not_found".into(),
                message: "no runnable Agent in agent.lock/install state matches `missing-agent`."
                    .into(),
            });
        let app = TuiApp::ready_with_runtime_inputs(
            controller,
            HarnessArgs {
                agent: Some("missing-agent".into()),
                ..HarnessArgs::default()
            },
            None,
        );

        assert!(app.can_prompt_agent_selector());
        assert!(
            workspace_resolution_actions(&app)
                .iter()
                .any(|action| action.contains("select Agent"))
        );
    }

    #[test]
    fn tui_event_buffer_applies_trace_content_policy() {
        let buffer = TuiEventBuffer::new(HarnessTraceContent::None, 8);
        let mut sink = buffer.sink();
        sink.record(&HarnessEventEnvelope {
            schema_version: 1,
            event_id: "evt-test".into(),
            session_id: "sess-test".into(),
            run_id: None,
            session_sequence: 1,
            run_sequence: None,
            timestamp: chrono::Utc::now(),
            event_type: HarnessEventType::PromptPrepared,
            phase_execution_id: None,
            correlation_id: None,
            parent_event_id: None,
            payload: HarnessEventPayload::Content {
                label: "prompt".into(),
                content: serde_json::json!({ "secret": "should-not-render" }),
            },
        })
        .unwrap();

        let events = buffer.events();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].payload, HarnessEventPayload::Empty);
    }

    #[test]
    fn tui_event_buffer_resets_run_output_without_clearing_trace() {
        let buffer = TuiEventBuffer::new(HarnessTraceContent::Full, 8);
        let mut sink = buffer.sink();
        let mut fields = BTreeMap::new();
        fields.insert(
            "assistant_content".into(),
            serde_json::json!("previous run"),
        );
        sink.record(&HarnessEventEnvelope {
            schema_version: 1,
            event_id: "evt-test".into(),
            session_id: "sess-test".into(),
            run_id: Some("run-old".into()),
            session_sequence: 1,
            run_sequence: Some(1),
            timestamp: chrono::Utc::now(),
            event_type: HarnessEventType::ModelRequestCompleted,
            phase_execution_id: Some("phase-exec-1".into()),
            correlation_id: None,
            parent_event_id: None,
            payload: HarnessEventPayload::Lifecycle {
                message: "Model request completed.".into(),
                fields,
            },
        })
        .unwrap();

        assert_eq!(buffer.events().len(), 1);
        assert_eq!(buffer.transcript().len(), 1);

        buffer.reset_run_output();

        assert_eq!(buffer.events().len(), 1);
        assert!(buffer.transcript().is_empty());
        assert!(buffer.latest_phase_output().is_none());
    }

    #[test]
    fn terminal_result_preserves_output_artifacts_and_usage_snapshot() {
        let mut controller = test_controller();
        let paths = RunOutputPaths::resolve(
            &std::env::temp_dir().join("agentpm-tui-test-state"),
            "run-test",
            None,
        )
        .unwrap();
        let mut report = test_run_report();
        report.usage.model_calls = 2;
        let terminal = RuntimeTerminalResult {
            status: HarnessTerminalStatus::Ended,
            output: Some(serde_json::json!({ "ok": true })),
            report,
        };

        controller.apply_terminal_result(&terminal, &paths);

        assert_eq!(controller.snapshot.run.status, TuiRunStatus::Terminal);
        assert_eq!(
            controller.snapshot.run.terminal_status,
            Some(HarnessTerminalStatus::Ended)
        );
        assert_eq!(
            controller.snapshot.run.latest_output,
            Some(serde_json::json!({ "ok": true }))
        );
        assert_eq!(controller.snapshot.run.usage.model_calls, 2);
        assert_eq!(
            controller.snapshot.reports.current_report_path,
            Some(paths.report_path)
        );
        assert_eq!(
            controller
                .snapshot
                .reports
                .current_report
                .as_ref()
                .map(|report| report.run_id.as_str()),
            Some("run-test")
        );
    }

    #[test]
    fn idle_ready_state_uses_no_run_visual_treatment() {
        let app =
            TuiApp::ready_with_runtime_inputs(test_controller(), HarnessArgs::default(), None);

        assert_eq!(run_visual_state(&app), RunVisualState::NoRun);
        assert!(app.can_send_message());
        assert_eq!(
            center_header_line(&app).to_string(),
            "Run --  Phase: idle  Status: Ready"
        );
    }

    #[test]
    fn composer_is_visible_only_for_idle_or_terminal_ready_run_panel() {
        let mut app =
            TuiApp::ready_with_runtime_inputs(test_controller(), HarnessArgs::default(), None);
        assert!(app.can_send_message());

        let snapshot = ready_snapshot(&app).clone();
        let (_sender, receiver) = mpsc::channel();
        app.state = TuiState::Running {
            snapshot: Box::new(snapshot),
            receiver,
            progress: vec![TuiRunProgress {
                message: "starting".into(),
            }],
            events: TuiEventBuffer::new(HarnessTraceContent::Redacted, 8),
            cancel: Arc::new(AtomicBool::new(false)),
        };
        app.focus = TuiFocus::Panel(VisiblePanel::Run);
        assert!(!app.can_send_message());
        assert!(app.can_cancel_run());
        assert_eq!(run_visual_state(&app), RunVisualState::Active);
    }

    #[test]
    fn terminal_run_view_shows_summary_and_reopens_composer() {
        let mut controller = test_controller();
        let state_dir = std::env::temp_dir().join("agentpm-tui-terminal-view-test");
        let paths = RunOutputPaths::resolve(&state_dir, "run-terminal-view", None).unwrap();
        let mut report = test_run_report();
        report.run_id = "run-terminal-view".into();
        report.duration_ms = Some(1_250);
        report.terminal_output = Some(serde_json::json!("done output"));
        report.phase_summaries = vec![crate::harness_observability::PhaseReportSummary {
            phase_execution_id: "phase-exec-1".into(),
            phase_id: "start".into(),
            outcome: Some("done".into()),
            transition_to: Some("$end".into()),
            status: "completed".into(),
        }];
        report.checkpoint_summaries = vec![crate::harness_observability::CheckpointReportSummary {
            checkpoint_id: "approve-response".into(),
            before_phase: "start".into(),
            status: "approved".into(),
            on_reject: Some("$abort".into()),
        }];
        report
            .write_pretty(&paths.report_path, &HarnessTraceContent::Redacted)
            .unwrap();
        let terminal = RuntimeTerminalResult {
            status: HarnessTerminalStatus::Ended,
            output: Some(serde_json::json!("done output")),
            report,
        };
        controller.apply_terminal_result(&terminal, &paths);
        let app = TuiApp::ready_with_runtime_inputs(controller, HarnessArgs::default(), None);

        assert_eq!(run_visual_state(&app), RunVisualState::Terminal);
        let snapshot = ready_snapshot(&app);
        assert_eq!(snapshot.run.run_id.as_deref(), Some("run-terminal-view"));
        assert_eq!(snapshot.run.phase_id.as_deref(), Some("start"));
        assert_eq!(
            center_header_line(&app).to_string(),
            "Run view  Terminal: $end -> ended  [✓ ENDED]"
        );
        let summary_text = run_summary_content(&app)
            .iter()
            .map(Line::to_string)
            .collect::<Vec<_>>()
            .join("\n");
        let output_text = assistant_output_page(&app, 5, 80)
            .lines
            .iter()
            .map(Line::to_string)
            .collect::<Vec<_>>()
            .join("\n");
        let usage_text = usage_content(&app)
            .iter()
            .map(Line::to_string)
            .collect::<Vec<_>>()
            .join("\n");
        assert!(summary_text.contains("Terminal status"));
        assert!(summary_text.contains("Checkpoints"));
        assert!(summary_text.contains("1 total"));
        assert!(summary_text.contains("Phase path"));
        assert!(summary_text.contains("start -> $end"));
        assert!(output_text.contains("done output"));
        assert!(usage_text.contains("cost: unknown"));
        assert!(app.can_send_message());
    }

    #[test]
    fn terminal_run_view_falls_back_to_latest_phase_result_output() {
        let mut controller = test_controller();
        emit_phase_result_output(&mut controller, "phase result output");
        let state_dir = std::env::temp_dir().join("agentpm-tui-phase-output-test");
        let paths = RunOutputPaths::resolve(&state_dir, "run-phase-output", None).unwrap();
        let mut report = test_run_report();
        report.run_id = "run-phase-output".into();
        report.terminal_output = None;
        report
            .write_pretty(&paths.report_path, &HarnessTraceContent::Redacted)
            .unwrap();
        let terminal = RuntimeTerminalResult {
            status: HarnessTerminalStatus::Ended,
            output: None,
            report,
        };
        controller.apply_terminal_result(&terminal, &paths);
        let app = TuiApp::ready_with_runtime_inputs(controller, HarnessArgs::default(), None);

        let output_text = assistant_output_page(&app, 5, 80)
            .lines
            .iter()
            .map(Line::to_string)
            .collect::<Vec<_>>()
            .join("\n");
        assert!(output_text.contains("phase result output"));
    }

    #[test]
    fn terminal_run_view_shows_assistant_content_turns() {
        let mut controller = test_controller();
        emit_assistant_content(&mut controller, "phase-exec-1", "first assistant answer");
        emit_assistant_content(&mut controller, "phase-exec-2", "second assistant answer");
        let state_dir = std::env::temp_dir().join("agentpm-tui-assistant-output-test");
        let paths = RunOutputPaths::resolve(&state_dir, "run-assistant-output", None).unwrap();
        let mut report = test_run_report();
        report.run_id = "run-assistant-output".into();
        report.terminal_output = None;
        report
            .write_pretty(&paths.report_path, &HarnessTraceContent::Redacted)
            .unwrap();
        let terminal = RuntimeTerminalResult {
            status: HarnessTerminalStatus::Ended,
            output: None,
            report,
        };
        controller.apply_terminal_result(&terminal, &paths);
        let app = TuiApp::ready_with_runtime_inputs(controller, HarnessArgs::default(), None);

        let output = assistant_output_text(&app).expect("assistant output should render");
        assert!(output.contains("phase-exec-1"));
        assert!(output.contains("Assistant"));
        assert!(output.contains("first assistant answer"));
        assert!(output.contains("phase-exec-2"));
        assert!(output.contains("second assistant answer"));
        assert!(app.has_latest_output());
    }

    #[test]
    fn active_run_view_reads_latest_phase_result_output_from_trace() {
        let mut controller = test_controller();
        emit_phase_result_output(&mut controller, "active phase result output");
        controller.refresh_snapshot();
        let snapshot = controller.snapshot().clone();
        let (_sender, receiver) = mpsc::channel();
        let mut app =
            TuiApp::ready_with_runtime_inputs(test_controller(), HarnessArgs::default(), None);
        app.state = TuiState::Running {
            snapshot: Box::new(snapshot),
            receiver,
            progress: vec![],
            events: TuiEventBuffer::new(HarnessTraceContent::Redacted, 8),
            cancel: Arc::new(AtomicBool::new(false)),
        };

        let output_text = assistant_output_page(&app, 5, 80)
            .lines
            .iter()
            .map(Line::to_string)
            .collect::<Vec<_>>()
            .join("\n");
        assert!(output_text.contains("active phase result output"));
    }

    #[test]
    fn active_run_assistant_output_shows_only_current_phase_transcript() {
        let mut app =
            TuiApp::ready_with_runtime_inputs(test_controller(), HarnessArgs::default(), None);
        let mut snapshot = ready_snapshot(&app).clone();
        snapshot.run.status = TuiRunStatus::Active;
        snapshot.run.phase_id = Some("respond".into());
        snapshot.run.transcript = vec![
            TuiRunTranscriptItem {
                phase_label: Some("inspect".into()),
                kind: TuiRunTranscriptKind::Assistant {
                    content: "inspect draft".into(),
                },
            },
            TuiRunTranscriptItem {
                phase_label: Some("respond".into()),
                kind: TuiRunTranscriptKind::Assistant {
                    content: "respond draft".into(),
                },
            },
            TuiRunTranscriptItem {
                phase_label: Some("respond".into()),
                kind: TuiRunTranscriptKind::Repair {
                    message: "needs completion".into(),
                },
            },
        ];
        let (_sender, receiver) = mpsc::channel();
        app.state = TuiState::Running {
            snapshot: Box::new(snapshot),
            receiver,
            progress: vec![],
            events: TuiEventBuffer::new(HarnessTraceContent::Redacted, 8),
            cancel: Arc::new(AtomicBool::new(false)),
        };

        let output = assistant_output_text(&app).expect("assistant output should render");
        assert!(!output.contains("inspect draft"));
        assert!(output.contains("respond"));
        assert!(output.contains("respond draft"));
        assert!(output.contains("Repair"));
        assert!(output.contains("needs completion"));
    }

    #[test]
    fn running_snapshot_refreshes_from_live_events() {
        let mut app =
            TuiApp::ready_with_runtime_inputs(test_controller(), HarnessArgs::default(), None);
        let mut snapshot = ready_snapshot(&app).clone();
        snapshot.run.status = TuiRunStatus::Active;
        snapshot.run.run_id = Some("run-live".into());
        snapshot.run.phase_id = Some("starting".into());
        snapshot.run.usage.model_calls = 7;
        let events = TuiEventBuffer::new(HarnessTraceContent::Full, 8);
        let mut sink = events.sink();
        sink.record(&HarnessEventEnvelope {
            schema_version: 1,
            event_id: "evt-phase".into(),
            session_id: "sess-test".into(),
            run_id: Some("run-live".into()),
            session_sequence: 1,
            run_sequence: Some(1),
            timestamp: chrono::Utc::now(),
            event_type: HarnessEventType::PhaseStarted,
            phase_execution_id: Some("phase-exec-1".into()),
            correlation_id: None,
            parent_event_id: None,
            payload: HarnessEventPayload::Phase {
                phase_id: "respond".into(),
                outcome: None,
                transition_to: None,
                output: None,
            },
        })
        .unwrap();
        let mut fields = BTreeMap::new();
        fields.insert("assistant_content".into(), serde_json::json!("live answer"));
        sink.record(&HarnessEventEnvelope {
            schema_version: 1,
            event_id: "evt-model".into(),
            session_id: "sess-test".into(),
            run_id: Some("run-live".into()),
            session_sequence: 2,
            run_sequence: Some(2),
            timestamp: chrono::Utc::now(),
            event_type: HarnessEventType::ModelRequestCompleted,
            phase_execution_id: Some("phase-exec-1".into()),
            correlation_id: None,
            parent_event_id: None,
            payload: HarnessEventPayload::Lifecycle {
                message: "Model request completed.".into(),
                fields,
            },
        })
        .unwrap();
        let (_sender, receiver) = mpsc::channel();
        app.state = TuiState::Running {
            snapshot: Box::new(snapshot),
            receiver,
            progress: vec![],
            events,
            cancel: Arc::new(AtomicBool::new(false)),
        };

        poll_run_result(&mut app);

        let snapshot = app
            .snapshot()
            .expect("running snapshot should remain available");
        assert_eq!(snapshot.run.phase_id.as_deref(), Some("respond"));
        assert_eq!(snapshot.run.usage.model_calls, 1);
        assert_eq!(snapshot.run.transcript.len(), 1);
        let output = assistant_output_text(&app).expect("assistant output should render");
        assert!(output.contains("live answer"));
    }

    #[test]
    fn active_run_output_ignores_previous_report_and_trace_output() {
        let mut app =
            TuiApp::ready_with_runtime_inputs(test_controller(), HarnessArgs::default(), None);
        let report_path = std::env::temp_dir().join(format!(
            "agentpm-tui-previous-report-{}.json",
            std::process::id()
        ));
        let mut old_report = test_run_report();
        old_report.run_id = "run-old".into();
        old_report.terminal_output = Some(serde_json::json!("previous terminal output"));
        old_report
            .write_pretty(&report_path, &HarnessTraceContent::Full)
            .unwrap();
        let mut snapshot = ready_snapshot(&app).clone();
        snapshot.run.status = TuiRunStatus::Active;
        snapshot.run.run_id = Some("run-new".into());
        snapshot.run.phase_id = Some("starting".into());
        snapshot.run.latest_output = None;
        snapshot.run.transcript.clear();
        snapshot.reports.current_report_path = Some(report_path.clone());
        snapshot.trace.events.push(HarnessEventEnvelope {
            schema_version: 1,
            event_id: "evt-old-phase".into(),
            session_id: "sess-test".into(),
            run_id: Some("run-old".into()),
            session_sequence: 1,
            run_sequence: Some(1),
            timestamp: chrono::Utc::now(),
            event_type: HarnessEventType::PhaseResultReady,
            phase_execution_id: Some("phase-exec-old".into()),
            correlation_id: None,
            parent_event_id: None,
            payload: HarnessEventPayload::Phase {
                phase_id: "old".into(),
                outcome: Some("done".into()),
                transition_to: None,
                output: Some(serde_json::json!("previous phase output")),
            },
        });
        let (_sender, receiver) = mpsc::channel();
        app.state = TuiState::Running {
            snapshot: Box::new(snapshot),
            receiver,
            progress: vec![],
            events: TuiEventBuffer::new(HarnessTraceContent::Redacted, 8),
            cancel: Arc::new(AtomicBool::new(false)),
        };

        assert!(assistant_output_text(&app).is_none());
        let page_text = assistant_output_page(&app, 5, 80)
            .lines
            .iter()
            .map(Line::to_string)
            .collect::<Vec<_>>()
            .join("\n");
        assert!(page_text.contains("No assistant or PhaseResult output yet."));
        assert!(!page_text.contains("previous terminal output"));
        assert!(!page_text.contains("previous phase output"));

        let _ = std::fs::remove_file(report_path);
    }

    #[test]
    fn output_viewer_owns_keymap_and_blocks_send_or_cancel() {
        let mut controller = test_controller();
        let paths = RunOutputPaths::resolve(
            &std::env::temp_dir().join("agentpm-tui-output-viewer-test"),
            "run-output-viewer",
            None,
        )
        .unwrap();
        let terminal = RuntimeTerminalResult {
            status: HarnessTerminalStatus::Ended,
            output: Some(serde_json::json!("full assistant output")),
            report: test_run_report(),
        };
        controller.apply_terminal_result(&terminal, &paths);
        let mut app = TuiApp::ready_with_runtime_inputs(controller, HarnessArgs::default(), None);
        assert!(app.can_send_message());

        app.open_output_viewer();
        assert!(app.output_viewer.is_some());
        assert!(!app.can_send_message());
        assert!(handle_output_viewer_key(&mut app, KeyCode::Char('c')));
        assert!(app.output_viewer.is_some());
        assert!(handle_output_viewer_key(&mut app, KeyCode::Esc));
        assert!(app.output_viewer.is_none());
    }

    #[test]
    fn controller_controls_emit_or_reject_through_state_layer() {
        let mut controller = test_controller();
        let cancel = controller.request_cancel().unwrap();
        assert!(cancel.accepted);
        assert!(
            controller
                .snapshot
                .trace
                .events
                .iter()
                .any(|event| event.event_type == HarnessEventType::CancellationRequested)
        );
        assert!(
            controller
                .record_approval_decision(TuiApprovalDecision::Approve)
                .is_err()
        );
        assert!(
            controller
                .record_approval_decision(TuiApprovalDecision::Deny)
                .is_err()
        );
    }

    #[test]
    fn approval_control_does_not_fabricate_engine_approval_events() {
        let mut controller = test_controller_with_checkpoint();
        let mut model = ScriptedModelRuntime::new(Vec::new());
        let mut dispatcher = ScriptedActionDispatcher::default();
        let mut knowledge = crate::harness_runtime::NoopKnowledgeRuntime;
        let mut approvals = ScriptedApprovalController::default();
        approvals.push(
            "approve-response",
            crate::harness_runtime::ApprovalDecision::Pending,
        );
        let mut hooks = crate::harness_runtime::NoopHookRuntime;
        let mut services = HarnessRuntimeServices {
            model: &mut model,
            dispatcher: &mut dispatcher,
            knowledge: &mut knowledge,
            memory: None,
            embedding_provider: None,
            approvals: &mut approvals,
            hooks: &mut hooks,
            service_events: None,
        };
        let paths = RunOutputPaths::resolve(
            &std::env::temp_dir().join("agentpm-tui-approval-test-state"),
            "run-approval",
            None,
        )
        .unwrap();

        controller
            .start_run_with_services(
                "run-approval".into(),
                "requires approval",
                &paths,
                &mut services,
            )
            .unwrap();
        assert_eq!(
            controller.snapshot.run.status,
            TuiRunStatus::PendingApproval
        );

        let err = controller
            .record_approval_decision(TuiApprovalDecision::Approve)
            .unwrap_err();
        assert!(err.to_string().contains("ApprovalController"));
        assert!(
            controller
                .snapshot
                .trace
                .events
                .iter()
                .any(|event| event.event_type == HarnessEventType::ApprovalRequested)
        );
        assert!(
            !controller
                .snapshot
                .trace
                .events
                .iter()
                .any(|event| matches!(
                    event.event_type,
                    HarnessEventType::ApprovalApproved | HarnessEventType::ApprovalDenied
                ))
        );
    }

    #[test]
    fn panic_payload_message_handles_common_payloads() {
        assert_eq!(panic_payload_message(Box::new("boom")), "boom");
        assert_eq!(
            panic_payload_message(Box::new(String::from("kaboom"))),
            "kaboom"
        );
        assert_eq!(
            panic_payload_message(Box::new(42_u8)),
            "unknown panic payload"
        );
    }

    fn test_controller() -> TuiSessionController {
        test_controller_with_loop(test_loop_manifest())
    }

    fn emit_phase_result_output(controller: &mut TuiSessionController, output: &str) {
        controller
            .session
            .emitter
            .emit(
                HarnessEventType::PhaseResultReady,
                HarnessEventPayload::Phase {
                    phase_id: "start".into(),
                    outcome: Some("done".into()),
                    transition_to: Some("$end".into()),
                    output: Some(serde_json::json!(output)),
                },
                HarnessEventBuilder {
                    run_id: Some("run-phase-output".into()),
                    phase_execution_id: Some("phase-exec-1".into()),
                    ..HarnessEventBuilder::default()
                },
            )
            .unwrap();
    }

    fn emit_assistant_content(
        controller: &mut TuiSessionController,
        phase_execution_id: &str,
        content: &str,
    ) {
        let mut fields = BTreeMap::new();
        fields.insert("assistant_content".into(), serde_json::json!(content));
        controller
            .session
            .emitter
            .emit(
                HarnessEventType::ModelRequestCompleted,
                HarnessEventPayload::Lifecycle {
                    message: "Model request completed.".into(),
                    fields,
                },
                HarnessEventBuilder {
                    run_id: Some("run-assistant-output".into()),
                    phase_execution_id: Some(phase_execution_id.into()),
                    ..HarnessEventBuilder::default()
                },
            )
            .unwrap();
    }

    fn ready_snapshot(app: &TuiApp) -> &TuiSessionSnapshot {
        let TuiState::Ready { controller } = &app.state else {
            panic!("ready app expected");
        };
        controller.snapshot()
    }

    fn test_controller_with_checkpoint() -> TuiSessionController {
        let mut manifest = test_loop_manifest();
        manifest.r#loop.checkpoints = vec![crate::manifest::LoopCheckpoint {
            id: "approve-response".into(),
            r#type: "approval".into(),
            before_phase: "start".into(),
            on_reject: "$abort".into(),
        }];
        test_controller_with_loop(manifest)
    }

    fn test_controller_with_loop(
        loop_manifest: crate::manifest::LoopManifest,
    ) -> TuiSessionController {
        let plan = test_plan();
        let runtime = runtime_snapshot_from_plan(&plan);
        let mut session = HarnessSession::with_runtime_snapshot(runtime);
        let events = TuiEventBuffer::new(HarnessTraceContent::Redacted, 32);
        session.emitter.add_sink(Box::new(events.sink()));
        let mut options = harness_engine_options_from_plan(&plan);
        options.retain_active_on_approval_required = true;
        let engine = HarnessEngine::new(loop_manifest, options);
        let snapshot = build_session_snapshot(&plan, &session, &events, None, None);
        TuiSessionController {
            plan: Box::new(plan),
            session,
            engine: Some(engine),
            events,
            cancellation_requested: Arc::new(AtomicBool::new(false)),
            snapshot,
        }
    }

    fn test_plan() -> ResolvedHarnessPlan {
        let root = std::env::temp_dir().join("agentpm-tui-plan");
        ResolvedHarnessPlan {
            workspace_root: root.clone(),
            lock_path: root.join("agent.lock"),
            state_dir: root.join(".agentpm-state"),
            config: crate::harness_config::ResolvedHarnessConfig {
                workspace_root: root.clone(),
                config_path: None,
                config: crate::harness_config::HarnessConfig::default(),
                state_dir: root.join(".agentpm-state"),
                state_dir_source: crate::harness_config::HarnessConfigSource {
                    kind: HarnessConfigSourceKind::HarnessDefault,
                    path: None,
                },
                model_source: crate::harness_config::HarnessConfigSource {
                    kind: HarnessConfigSourceKind::HarnessDefault,
                    path: None,
                },
            },
            selected_agent: Some(crate::harness_plan::ResolvedAgentRoot {
                root_key: "agent:@zack/test@0.1.0".into(),
                name: "@zack/test".into(),
                version: "0.1.0".into(),
                manifest_path: root.join("agent.json"),
                package_key: Some("agent:@zack/test@0.1.0".into()),
                tools: vec!["@zack/tool".into()],
                skills: vec!["@zack/skill".into()],
                knowledge: Vec::new(),
                memory: Vec::new(),
                profiles: vec!["@zack/profile".into()],
                loop_key: "loop:@zack/loop@0.1.0".into(),
            }),
            loop_package: Some(crate::harness_plan::ResolvedPackageInfo {
                key: "loop:@zack/loop@0.1.0".into(),
                kind: crate::semver::types::PackageKind::Loop,
                name: "@zack/loop".into(),
                version: "0.1.0".into(),
                root: root.join(".agentpm/packages/loops/zack/loop/0.1.0"),
            }),
            package_graph: BTreeMap::new(),
            runtime_scopes: BTreeMap::new(),
            consumer_context: crate::harness_plan::ConsumerContextReadiness {
                state: CapabilityState::Available,
                file: Some("context.md".into()),
                path: Some(root.join("context.md")),
                byte_size: Some(114),
                approximate_tokens: Some(29),
                sha256: None,
            },
            profile_bindings: Default::default(),
            profiles: BTreeMap::new(),
            capabilities: vec![
                crate::harness_plan::StaticCapabilityCandidate {
                    kind: "profile".into(),
                    identity: "@zack/profile".into(),
                    scope: "global".into(),
                    source: "agent_binding".into(),
                    state: CapabilityState::Available,
                },
                crate::harness_plan::StaticCapabilityCandidate {
                    kind: "skill".into(),
                    identity: "@zack/skill".into(),
                    scope: "global".into(),
                    source: "agent_binding".into(),
                    state: CapabilityState::Available,
                },
                crate::harness_plan::StaticCapabilityCandidate {
                    kind: "tool".into(),
                    identity: "@zack/tool".into(),
                    scope: "global".into(),
                    source: "agent_binding".into(),
                    state: CapabilityState::Available,
                },
                crate::harness_plan::StaticCapabilityCandidate {
                    kind: "tool".into(),
                    identity: "@zack/suppressed".into(),
                    scope: "global".into(),
                    source: "agent_binding".into(),
                    state: CapabilityState::Suppressed,
                },
                crate::harness_plan::StaticCapabilityCandidate {
                    kind: "memory".into(),
                    identity: "@zack/memory".into(),
                    scope: "global".into(),
                    source: "agent_binding".into(),
                    state: CapabilityState::Pending,
                },
            ],
            report: crate::harness_plan::PreflightReport {
                status: PreflightStatus::Ready,
                diagnostics: Vec::new(),
                mcp_exports: crate::harness_plan::PreflightMcpExports {
                    enabled: false,
                    host: "127.0.0.1".into(),
                    restart: crate::harness_config::HarnessRestartPolicy::default(),
                    surfaces: Vec::new(),
                },
                mcp_imports: crate::harness_plan::PreflightMcpImports {
                    enabled: false,
                    servers: Vec::new(),
                },
            },
        }
    }

    fn test_loop_manifest() -> crate::manifest::LoopManifest {
        crate::manifest::LoopManifest {
            kind: "loop".into(),
            name: "@zack/loop".into(),
            version: "0.1.0".into(),
            description: None,
            readme: None,
            license: None,
            r#loop: crate::manifest::LoopMetadata {
                archetype: None,
                entry_phase: "start".into(),
                limits: None,
                phases: vec![crate::manifest::LoopPhase {
                    id: "start".into(),
                    objective: "Start.".into(),
                    access: None,
                    outcomes: vec![crate::manifest::LoopOutcome {
                        id: "done".into(),
                        description: "Done.".into(),
                    }],
                }],
                transitions: Vec::new(),
                checkpoints: Vec::new(),
                error_policy: None,
            },
        }
    }

    fn test_run_report() -> RunReport {
        RunReport {
            report_version: crate::harness_observability::HARNESS_REPORT_SCHEMA_VERSION,
            session_id: "sess-test".into(),
            run_id: "run-test".into(),
            agent: crate::harness_observability::ReportPackageIdentity {
                name: "@zack/test".into(),
                version: "0.1.0".into(),
            },
            loop_package: crate::harness_observability::ReportPackageIdentity {
                name: "@zack/loop".into(),
                version: "0.1.0".into(),
            },
            started_at: chrono::Utc::now(),
            ended_at: Some(chrono::Utc::now()),
            duration_ms: Some(1),
            terminal_status: HarnessTerminalStatus::Ended,
            terminal_output: None,
            preflight_status: PreflightStatus::Ready,
            diagnostics: Vec::new(),
            runtime: Default::default(),
            runtime_sources: BTreeMap::new(),
            consumer_context: None,
            scope_summaries: Vec::new(),
            phase_summaries: Vec::new(),
            checkpoint_summaries: Vec::new(),
            action_summaries: Vec::new(),
            tool_summaries: Vec::new(),
            mcp_summaries: Vec::new(),
            mcp_imports: Vec::new(),
            knowledge_summaries: Vec::new(),
            memory_summaries: Vec::new(),
            memory_write_review_summaries: Vec::new(),
            usage: RunUsage::default(),
            retry_count: 0,
            repair_count: 0,
            error_count: 0,
            approval_summary: BTreeMap::new(),
            cancellation_summary: BTreeMap::new(),
            trace_path: None,
        }
    }

    fn test_harness_args() -> HarnessArgs {
        HarnessArgs {
            agent: None,
            config: None,
            state_dir: None,
            scopes: Vec::new(),
            machine: false,
            headless: false,
            json: false,
            verbose: false,
            input: None,
            input_file: None,
            report: None,
        }
    }
}
