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

mod render;
mod state;
use render::{assistant_output_text, render_app};
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
                _ if shell_quit_requested(app, key.code, key.modifiers) && app.is_run_active() => {
                    app.request_run_cancel();
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
                KeyCode::Char('a') | KeyCode::Char('A') if app.can_decide_approval() => {
                    app.record_approval_decision(TuiApprovalDecision::Approve);
                }
                KeyCode::Char('d') | KeyCode::Char('D') if app.can_decide_approval() => {
                    app.record_approval_decision(TuiApprovalDecision::Deny);
                }
                KeyCode::Char('c') | KeyCode::Char('C') if app.can_cancel_run() => {
                    app.request_run_cancel();
                }
                KeyCode::Char('x') | KeyCode::Char('X') if app.can_invoke_memory_operation() => {
                    app.invoke_selected_memory_operation();
                }
                KeyCode::Char(']') if app.can_invoke_memory_operation() => {
                    app.cycle_memory_operation();
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
    let mut final_approval_control = None;
    let mut final_memory_operation_control = None;
    if let TuiState::Running {
        snapshot,
        receiver,
        progress,
        events,
        approvals,
        memory_controls,
        ..
    } = &mut app.state
    {
        snapshot.trace.events = events.events();
        snapshot.run.transcript = events.transcript();
        snapshot.run.latest_output = events.latest_phase_output();
        snapshot.run.usage = events.live_run_usage();
        snapshot.run.approval = approvals.pending();
        snapshot.run.approval_control = approvals.snapshot();
        snapshot.run.status = if snapshot.run.approval.is_some() {
            TuiRunStatus::PendingApproval
        } else {
            TuiRunStatus::Active
        };
        snapshot.run.memory_operation_control = memory_controls.snapshot();
        snapshot.run.memory_operations =
            memory_controls.operations_for_phase(snapshot.run.phase_id.as_deref());
        if let Some(phase_id) = events.current_phase_id()
            && snapshot.run.phase_id.as_deref() != Some(phase_id.as_str())
        {
            snapshot.run.phase_id = Some(phase_id);
            snapshot.run.phase_objective = None;
            snapshot.run.memory_operations =
                memory_controls.operations_for_phase(snapshot.run.phase_id.as_deref());
        }
        loop {
            match receiver.try_recv() {
                Ok(TuiRunMessage::Progress(item)) => progress.push(item),
                Ok(TuiRunMessage::Finished(result)) => {
                    final_approval_control = approvals.snapshot();
                    final_memory_operation_control = memory_controls.snapshot();
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
        app.run_error = result.error.clone();
        let mut controller = result.controller;
        if result.error.is_some() && controller.snapshot.run.status != TuiRunStatus::Terminal {
            terminalize_worker_error_snapshot(&mut controller.snapshot.run);
        }
        controller.snapshot.run.approval_control = final_approval_control;
        controller.snapshot.run.memory_operation_control = final_memory_operation_control;
        app.state = TuiState::Ready {
            controller: Box::new(controller),
        };
        app.panel = VisiblePanel::Run;
        app.focus = TuiFocus::Composer;
        app.composer_input.clear();
        app.reconcile_focus();
    }
}

fn terminalize_worker_error_snapshot(run: &mut TuiRunSnapshot) {
    run.status = TuiRunStatus::Terminal;
    run.terminal_status = Some(HarnessTerminalStatus::Failed);
    run.approval = None;
    run.memory_operations.clear();
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
    controller
        .cancellation_requested
        .store(false, Ordering::SeqCst);
    let cancel = Arc::clone(&controller.cancellation_requested);
    let events = controller.events.clone();
    let approvals = TuiApprovalHandle::new();
    let memory_controls = TuiMemoryControlHandle::new();
    events.reset_run_output();
    let mut snapshot = controller.snapshot().clone();
    snapshot.run.status = TuiRunStatus::Active;
    snapshot.run.run_id = Some(run_id.clone());
    snapshot.run.run_number = Some(snapshot.usage.started_runs + 1);
    snapshot.run.phase_id = Some("starting".into());
    snapshot.run.started_at = Some(Utc::now());
    snapshot.run.phase_objective = Some("Preparing the Harness services for this Run.".into());
    snapshot.run.terminal_status = None;
    snapshot.run.latest_output = None;
    snapshot.run.transcript.clear();
    snapshot.run.usage = Default::default();
    snapshot.run.approval = None;
    snapshot.run.approval_control = None;
    snapshot.run.memory_operation_control = None;
    snapshot.reports.current_report_path = None;
    snapshot.reports.current_trace_path = None;
    snapshot.reports.current_report = None;
    snapshot.reports.current_trace_events.clear();
    snapshot.reports.current_report_error = None;
    snapshot.reports.current_trace_error = None;
    app.assistant_output_page = 0;
    let receiver = spawn_tui_run_worker(
        *controller,
        run_id,
        input.clone(),
        approvals.clone(),
        memory_controls.clone(),
    );
    app.state = TuiState::Running {
        snapshot: Box::new(snapshot),
        receiver,
        progress: vec![TuiRunProgress {
            message: "Starting Run.".into(),
        }],
        events,
        cancel,
        approvals,
        memory_controls,
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
    memory_operation_index: usize,
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
        approvals: TuiApprovalHandle,
        memory_controls: TuiMemoryControlHandle,
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
            memory_operation_index: 0,
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
            memory_operation_index: 0,
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
            memory_operation_index: 0,
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
        self.output_viewer.is_none() && self.resolution_prompt.is_none() && self.is_run_active()
    }

    fn can_decide_approval(&self) -> bool {
        self.output_viewer.is_none()
            && matches!(self.focus, TuiFocus::Panel(VisiblePanel::Run))
            && self.panel == VisiblePanel::Run
            && self
                .run_snapshot()
                .is_some_and(|run| run.approval.is_some())
    }

    fn selected_memory_operation(&self) -> Option<TuiMemoryOperationSnapshot> {
        let operations = &self.run_snapshot()?.memory_operations;
        if operations.is_empty() {
            return None;
        }
        operations
            .get(self.memory_operation_index % operations.len())
            .cloned()
    }

    fn can_invoke_memory_operation(&self) -> bool {
        self.output_viewer.is_none()
            && matches!(self.focus, TuiFocus::Panel(VisiblePanel::Run))
            && self.panel == VisiblePanel::Run
            && self.is_run_active()
            && self.selected_memory_operation().is_some()
    }

    fn cycle_memory_operation(&mut self) {
        let count = self
            .run_snapshot()
            .map(|run| run.memory_operations.len())
            .unwrap_or_default();
        if count > 1 {
            self.memory_operation_index = (self.memory_operation_index + 1) % count;
        }
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
                    message: "Cancellation requested; waiting for the active operation to stop."
                        .into(),
                });
            }
            TuiState::Loading { .. } | TuiState::Failed { .. } => {}
        }
    }

    fn record_approval_decision(&mut self, decision: TuiApprovalDecision) {
        let TuiState::Running {
            approvals,
            progress,
            ..
        } = &mut self.state
        else {
            return;
        };
        match approvals.decide(decision) {
            Ok(ack) => progress.push(TuiRunProgress {
                message: ack.message,
            }),
            Err(err) => self.run_error = Some(format!("{err:#}")),
        }
    }

    fn invoke_selected_memory_operation(&mut self) {
        let Some(operation) = self.selected_memory_operation() else {
            return;
        };
        let TuiState::Running {
            memory_controls,
            progress,
            ..
        } = &mut self.state
        else {
            return;
        };
        match memory_controls.request(&operation) {
            Ok(ack) => progress.push(TuiRunProgress {
                message: ack.message,
            }),
            Err(err) => self.run_error = Some(format!("{err:#}")),
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
pub(super) mod test_support {
    use super::super::{harness_engine_options_from_plan, runtime_snapshot_from_plan};
    use super::*;
    use crate::harness_config::HarnessConfigSourceKind;
    use crate::harness_engine::{HarnessEngine, HarnessSession};
    use crate::harness_observability::{
        HarnessEventBuilder, HarnessEventPayload, HarnessEventType, RunReport, RunUsage,
    };
    use crate::harness_plan::ResolvedHarnessPlan;
    use std::collections::BTreeMap;
    use std::sync::{Arc, atomic::AtomicBool};

    pub(super) fn test_controller() -> TuiSessionController {
        test_controller_with_loop(test_loop_manifest())
    }

    pub(super) fn emit_phase_result_output(controller: &mut TuiSessionController, output: &str) {
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

    pub(super) fn emit_assistant_content(
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

    pub(super) fn ready_snapshot(app: &TuiApp) -> &TuiSessionSnapshot {
        let TuiState::Ready { controller } = &app.state else {
            panic!("ready app expected");
        };
        controller.snapshot()
    }

    pub(super) fn test_controller_with_loop(
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

    pub(super) fn test_plan() -> ResolvedHarnessPlan {
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

    pub(super) fn test_loop_manifest() -> crate::manifest::LoopManifest {
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

    pub(super) fn test_run_report() -> RunReport {
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

    pub(super) fn test_harness_args() -> HarnessArgs {
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

#[cfg(test)]
mod tests {
    use super::test_support::*;
    use super::*;
    use crate::harness_config::HarnessConfigSourceKind;
    use crate::harness_engine::{MemoryOperationInvocationResult, RuntimeTerminalResult};
    use crate::harness_observability::{
        HarnessEventPayload, HarnessEventSink, HarnessEventType, RunOutputPaths,
    };
    use crate::harness_plan::{HarnessPlanProgress, HarnessPlanProgressStage};
    use std::collections::BTreeMap;
    use std::sync::mpsc;

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
    }

    #[test]
    fn tui_approval_handle_queues_one_engine_decision_without_events() {
        let handle = TuiApprovalHandle::new();
        assert!(
            handle
                .decide(TuiApprovalDecision::Approve)
                .unwrap_err()
                .to_string()
                .contains("pending approval")
        );
        handle.set_pending(TuiApprovalSnapshot {
            checkpoint_id: "approve-response".into(),
            before_phase: "respond".into(),
        });

        let ack = handle.decide(TuiApprovalDecision::Approve).unwrap();
        assert!(ack.accepted);
        assert!(ack.message.contains(TUI_APPROVAL_STATUS_APPROVED));
        assert_eq!(
            handle.snapshot().map(|snapshot| snapshot.status),
            Some(TUI_APPROVAL_STATUS_APPROVED.into())
        );
        assert_eq!(handle.take_decision(), Some(TuiApprovalDecision::Approve));
        assert!(handle.take_decision().is_none());
    }

    #[test]
    fn running_app_routes_approval_decision_to_worker_handle() {
        let mut app =
            TuiApp::ready_with_runtime_inputs(test_controller(), HarnessArgs::default(), None);
        let snapshot = ready_snapshot(&app).clone();
        let (_sender, receiver) = mpsc::channel();
        let approvals = TuiApprovalHandle::new();
        approvals.set_pending(TuiApprovalSnapshot {
            checkpoint_id: "approve-response".into(),
            before_phase: "respond".into(),
        });
        app.state = TuiState::Running {
            snapshot: Box::new(snapshot),
            receiver,
            progress: Vec::new(),
            events: TuiEventBuffer::new(HarnessTraceContent::Redacted, 8),
            cancel: Arc::new(AtomicBool::new(false)),
            approvals: approvals.clone(),
            memory_controls: TuiMemoryControlHandle::new(),
        };
        app.focus = TuiFocus::Panel(VisiblePanel::Run);
        poll_run_result(&mut app);
        assert!(app.can_decide_approval());

        app.record_approval_decision(TuiApprovalDecision::Deny);
        assert_eq!(approvals.take_decision(), Some(TuiApprovalDecision::Deny));
        poll_run_result(&mut app);
        assert_eq!(
            app.run_snapshot()
                .and_then(|run| run.approval_control.as_ref())
                .map(|control| control.status.as_str()),
            Some(TUI_APPROVAL_STATUS_DENIED)
        );
    }

    #[test]
    fn worker_error_terminalizes_active_running_snapshot() {
        let mut app =
            TuiApp::ready_with_runtime_inputs(test_controller(), HarnessArgs::default(), None);
        let mut running_snapshot = ready_snapshot(&app).clone();
        running_snapshot.run.status = TuiRunStatus::Active;
        running_snapshot.run.run_id = Some("run-error".into());
        running_snapshot.run.phase_id = Some("respond".into());

        let mut finished_controller = test_controller();
        finished_controller.snapshot.run = running_snapshot.run.clone();
        finished_controller
            .cancellation_requested
            .store(true, Ordering::SeqCst);
        let (sender, receiver) = mpsc::channel();
        sender
            .send(TuiRunMessage::Finished(Box::new(TuiRunWorkerResult {
                controller: finished_controller,
                error: Some("approval `approve-response` failed: cancelled".into()),
            })))
            .unwrap();

        app.state = TuiState::Running {
            snapshot: Box::new(running_snapshot),
            receiver,
            progress: Vec::new(),
            events: TuiEventBuffer::new(HarnessTraceContent::Redacted, 8),
            cancel: Arc::new(AtomicBool::new(true)),
            approvals: TuiApprovalHandle::new(),
            memory_controls: TuiMemoryControlHandle::new(),
        };

        poll_run_result(&mut app);

        assert_eq!(
            app.run_error.as_deref(),
            Some("approval `approve-response` failed: cancelled")
        );
        let run = app.run_snapshot().expect("run snapshot");
        assert_eq!(run.status, TuiRunStatus::Terminal);
        assert_eq!(run.terminal_status, Some(HarnessTerminalStatus::Failed));
        assert!(!app.can_cancel_run());

        app.composer_input = "start another run".into();
        start_run_from_composer(&mut app);
        let TuiState::Running { cancel, .. } = &app.state else {
            panic!("new run should enter running state");
        };
        assert!(
            !cancel.load(Ordering::SeqCst),
            "new runs must clear any previous cancellation request"
        );
    }

    #[test]
    fn memory_control_handle_queues_one_external_operation_at_a_time() {
        let handle = TuiMemoryControlHandle::new();
        let operation = TuiMemoryOperationSnapshot {
            package: "@zack/memory".into(),
            operation: "delete_user_memory".into(),
            operation_type: "delete".into(),
            description: "Delete user memory.".into(),
        };
        let first = handle.request(&operation).unwrap();
        assert!(first.accepted);
        assert!(
            handle
                .request(&operation)
                .unwrap_err()
                .to_string()
                .contains("busy")
        );
        let queued = handle.take().expect("queued operation");
        assert_eq!(queued.package, "@zack/memory");
        assert_eq!(queued.operation, "delete_user_memory");
        handle
            .complete(Ok(MemoryOperationInvocationResult {
                package: "@zack/memory".into(),
                package_version: "0.1.0".into(),
                operation: "delete_user_memory".into(),
                identity: "@zack/memory/operations/delete_user_memory".into(),
                count: 2,
            }))
            .unwrap();
        assert_eq!(
            handle.snapshot().map(|snapshot| snapshot.status),
            Some(TUI_MEMORY_CONTROL_STATUS_COMPLETED.into())
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
}
