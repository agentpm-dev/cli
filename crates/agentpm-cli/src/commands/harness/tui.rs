use super::HarnessArgs;
use crate::harness_config::{HarnessTraceContent, HarnessTraceLevel};
use crate::harness_observability::{HarnessEventEnvelope, HarnessTerminalStatus};
use crate::harness_plan::{CapabilityState, PreflightDiagnosticSeverity, PreflightStatus};
use crate::prelude::*;
use anyhow::{Context, bail};
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
    sync::{
        Arc, Mutex,
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

mod state;
use state::*;

type PanicHook = Box<dyn Fn(&PanicHookInfo<'_>) + Sync + Send + 'static>;

pub(super) fn run_tui_surface(args: HarnessArgs, workspace_root: std::path::PathBuf) -> Result<()> {
    ensure_tui_terminal_available()?;

    let mut terminal = TuiTerminal::enter()?;
    let mut app = TuiApp::loading(args.clone());
    let bootstrap = spawn_bootstrap_worker(args, workspace_root);

    run_shell_loop(&mut terminal, &mut app, bootstrap)
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
    bootstrap: Receiver<BootstrapMessage>,
) -> Result<()> {
    let mut bootstrap = Some(bootstrap);
    loop {
        poll_bootstrap_result(app, &mut bootstrap);
        terminal.draw(|frame| render_app(frame, app))?;
        if event::poll(Duration::from_millis(250)).context("polling terminal input")? {
            let Event::Key(key) = event::read().context("reading terminal input")? else {
                continue;
            };
            if key.kind != KeyEventKind::Press {
                continue;
            }
            match key.code {
                KeyCode::Char('q') | KeyCode::Esc => break,
                KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => break,
                KeyCode::Char('1') | KeyCode::Char('w') | KeyCode::Char('W')
                    if app.layout_mode == LayoutMode::Single =>
                {
                    app.panel = VisiblePanel::Workspace
                }
                KeyCode::Char('2') | KeyCode::Char('r') | KeyCode::Char('R') => {
                    app.panel = VisiblePanel::Run
                }
                KeyCode::Char('3') | KeyCode::Char('t') | KeyCode::Char('T') => {
                    app.panel = VisiblePanel::Trace
                }
                KeyCode::Char('4') | KeyCode::Char('m') | KeyCode::Char('M') => {
                    app.panel = VisiblePanel::Memory
                }
                KeyCode::Char('5') => app.panel = VisiblePanel::Reports,
                KeyCode::Tab => {
                    app.panel = if app.layout_mode == LayoutMode::Single {
                        app.panel.next()
                    } else {
                        app.panel.next_center()
                    }
                }
                _ => {}
            }
        }
    }
    Ok(())
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
                            *app = TuiApp::ready(controller);
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
}

impl VisiblePanel {
    fn label(self) -> &'static str {
        match self {
            Self::Workspace => "Workspace",
            Self::Run => "Run",
            Self::Trace => "Trace",
            Self::Memory => "Memory",
            Self::Reports => "Reports",
        }
    }

    fn next(self) -> Self {
        match self {
            Self::Workspace => Self::Run,
            Self::Run => Self::Trace,
            Self::Trace => Self::Memory,
            Self::Memory => Self::Reports,
            Self::Reports => Self::Workspace,
        }
    }

    fn next_center(self) -> Self {
        match self {
            Self::Workspace | Self::Run => Self::Trace,
            Self::Trace => Self::Memory,
            Self::Memory => Self::Reports,
            Self::Reports => Self::Run,
        }
    }
}

struct TuiApp {
    state: TuiState,
    panel: VisiblePanel,
    accent: Color,
    warnings: Vec<String>,
    layout_mode: LayoutMode,
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
    Failed {
        message: String,
    },
}

impl TuiApp {
    fn loading(args: HarnessArgs) -> Self {
        Self {
            state: TuiState::Loading {
                args,
                progress: Vec::new(),
            },
            panel: VisiblePanel::Run,
            accent: DEFAULT_ACCENT,
            warnings: Vec::new(),
            layout_mode: LayoutMode::Single,
        }
    }

    fn ready(controller: TuiSessionController) -> Self {
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
            accent,
            warnings,
            layout_mode: LayoutMode::Single,
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
            state: TuiState::Failed { message },
            panel: VisiblePanel::Workspace,
            accent: DEFAULT_ACCENT,
            warnings: Vec::new(),
            layout_mode: LayoutMode::Single,
        }
    }
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
    if app.layout_mode != LayoutMode::Single && app.panel == VisiblePanel::Workspace {
        app.panel = VisiblePanel::Run;
    }

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

fn tui_run_status_label(status: TuiRunStatus) -> &'static str {
    match status {
        TuiRunStatus::Idle => "ready",
        TuiRunStatus::Active => "active",
        TuiRunStatus::PendingApproval => "approval_required",
        TuiRunStatus::Terminal => "terminal",
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
        VisiblePanel::Workspace | VisiblePanel::Run => render_center_panel(frame, area, app),
        VisiblePanel::Trace => render_center_content(frame, area, center_trace_lines(app)),
        VisiblePanel::Memory => render_center_content(frame, area, memory_lines(app)),
        VisiblePanel::Reports => render_center_content(frame, area, report_lines(app)),
    }
}

fn render_workspace_panel(frame: &mut Frame<'_>, area: Rect, app: &TuiApp) {
    let block = Block::default()
        .title(" Preflight - Workspace Readiness ")
        .title_style(panel_title_style())
        .borders(Borders::ALL)
        .border_style(Style::default().fg(PANEL_BORDER));
    let inner = panel_inner(&block, area);
    let lines = workspace_lines(app);
    frame.render_widget(block, area);
    frame.render_widget(
        Paragraph::new(lines)
            .style(Style::default().fg(TEXT_PRIMARY))
            .wrap(Wrap { trim: false }),
        inner,
    );
}

fn render_center_panel(frame: &mut Frame<'_>, area: Rect, app: &TuiApp) {
    let lines = run_lines(app);
    render_center_content(frame, area, lines);
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

    let mut spans = vec![
        key_span("Q", app.accent),
        Span::raw(" Quit  "),
        key_span("Tab", app.accent),
        Span::raw(" Switch  "),
    ];
    if app.layout_mode == LayoutMode::Single {
        spans.extend([key_span("1", app.accent), Span::raw(" Workspace  ")]);
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
        Span::raw(format!("    Panel: {}", app.panel.label())),
    ]);
    let line = Line::from(spans);
    frame.render_widget(
        Paragraph::new(line)
            .alignment(Alignment::Left)
            .style(Style::default().fg(TEXT_MUTED)),
        content_area,
    );
}

fn key_span(label: &'static str, accent: Color) -> Span<'static> {
    Span::styled(
        label,
        Style::default().fg(accent).add_modifier(Modifier::BOLD),
    )
}

fn workspace_lines(app: &TuiApp) -> Vec<Line<'_>> {
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
            lines
        }
        TuiState::Failed { message } => vec![
            Line::from(styled("Preflight failed", Color::Red)),
            Line::from(""),
            Line::from(message.clone()),
            Line::from(""),
            Line::from("Press Q to exit."),
        ],
        TuiState::Ready { controller } => {
            let mut lines = Vec::new();
            for (index, category) in controller
                .snapshot()
                .workspace
                .categories
                .iter()
                .enumerate()
            {
                if index > 0 {
                    lines.push(Line::from(""));
                }
                lines.push(state_line(&category.label, category.state));
                lines.push(info_line("  ", category.summary.clone()));
                if let Some(source) = &category.source {
                    lines.push(info_line("  ", format!("from {source}")));
                }
            }
            let warnings = warning_lines(app, controller.snapshot());
            if !warnings.is_empty() {
                lines.push(Line::from(""));
                lines.extend(warnings);
            }
            lines
        }
    }
}

fn run_lines(app: &TuiApp) -> Vec<Line<'_>> {
    match &app.state {
        TuiState::Loading { .. } => vec![
            center_header_line(app),
            Line::from(""),
            center_tabs_line(app),
            Line::from(""),
            Line::from(styled("Bootstrap", app.accent)),
            Line::from("Resolving workspace, config, lockfile, and Agent graph..."),
        ],
        TuiState::Failed { .. } => vec![
            center_header_line(app),
            Line::from(""),
            center_tabs_line(app),
            Line::from(""),
            Line::from(styled("Run unavailable", Color::Red)),
            Line::from("Fix preflight errors, then restart the Harness."),
        ],
        TuiState::Ready { controller } => {
            let plan = controller.plan();
            let mut lines = vec![
                center_header_line(app),
                Line::from(""),
                center_tabs_line(app),
                Line::from(""),
                Line::from(format!(
                    "Preflight: {}",
                    preflight_status_label(plan.report.status)
                )),
                Line::from(""),
                Line::from(
                    "Milestone 19A shell is ready. Interactive Run composer and controls land in later TUI milestones.",
                ),
            ];
            if matches!(
                plan.report.status,
                PreflightStatus::SelectionRequired | PreflightStatus::Failed
            ) {
                lines.push(Line::from(""));
                lines.push(Line::from(styled(
                    "This workspace is not ready to run yet.",
                    STATUS_WARNING,
                )));
            }
            lines
        }
    }
}

fn trace_lines(app: &TuiApp) -> Vec<Line<'_>> {
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

fn center_trace_lines(app: &TuiApp) -> Vec<Line<'_>> {
    let mut lines = vec![
        center_header_line(app),
        Line::from(""),
        center_tabs_line(app),
        Line::from(""),
    ];
    if let TuiState::Ready { controller } = &app.state
        && controller
            .snapshot()
            .reports
            .current_trace_path
            .as_ref()
            .is_some()
    {
        match read_tui_trace(&controller.snapshot().reports, 100) {
            Ok(events) if !events.is_empty() => {
                lines.extend(
                    events
                        .iter()
                        .map(|event| Line::from(event_trace_line(event))),
                );
                return lines;
            }
            Ok(_) => {}
            Err(err) => {
                lines.push(Line::from(styled("Trace unavailable", STATUS_WARNING)));
                lines.push(Line::from(err.to_string()));
                return lines;
            }
        }
    }
    lines.extend(trace_lines(app));
    lines
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
            match read_tui_report(&controller.snapshot().reports) {
                Ok(Some(report)) => {
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
                Ok(None) => {
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
                Err(err) => {
                    lines.push(Line::from(styled("Report unavailable", STATUS_WARNING)));
                    lines.push(Line::from(err.to_string()));
                }
            }
            lines
        }
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
    let (phase, status) = match &app.state {
        TuiState::Loading { .. } => ("bootstrap", "loading".to_string()),
        TuiState::Ready { controller } => (
            controller
                .snapshot()
                .run
                .phase_id
                .as_deref()
                .unwrap_or("idle"),
            tui_run_status_label(controller.snapshot().run.status).into(),
        ),
        TuiState::Failed { .. } => ("preflight", "failed".to_string()),
    };
    Line::from(vec![
        Span::styled(
            "Run --",
            Style::default()
                .fg(TEXT_PRIMARY)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("  "),
        Span::styled("Phase: ", Style::default().fg(TEXT_MUTED)),
        Span::styled(
            phase.to_string(),
            Style::default()
                .fg(TEXT_PRIMARY)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("  "),
        Span::styled("Status: ", Style::default().fg(TEXT_MUTED)),
        Span::styled(
            status,
            Style::default()
                .fg(TEXT_PRIMARY)
                .add_modifier(Modifier::BOLD),
        ),
    ])
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

fn warning_lines<'a>(app: &'a TuiApp, snapshot: &'a TuiSessionSnapshot) -> Vec<Line<'a>> {
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
            PreflightDiagnosticSeverity::Warning
                | PreflightDiagnosticSeverity::Suppressed
                | PreflightDiagnosticSeverity::Pending
        ) {
            lines.push(Line::from(vec![
                styled(
                    diagnostic_severity_label(diagnostic.severity),
                    STATUS_WARNING,
                ),
                Span::raw(format!(" - {}", diagnostic.message)),
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
        HarnessEventPayload, HarnessEventSink, HarnessEventType, RunOutputPaths, RunReport,
        RunUsage,
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
    fn visible_panel_cycles_through_single_panel_tabs() {
        assert_eq!(VisiblePanel::Workspace.next(), VisiblePanel::Run);
        assert_eq!(VisiblePanel::Run.next(), VisiblePanel::Trace);
        assert_eq!(VisiblePanel::Trace.next(), VisiblePanel::Memory);
        assert_eq!(VisiblePanel::Memory.next(), VisiblePanel::Reports);
        assert_eq!(VisiblePanel::Reports.next(), VisiblePanel::Workspace);
    }

    #[test]
    fn visible_panel_cycles_center_tabs_without_workspace() {
        assert_eq!(VisiblePanel::Workspace.next_center(), VisiblePanel::Trace);
        assert_eq!(VisiblePanel::Run.next_center(), VisiblePanel::Trace);
        assert_eq!(VisiblePanel::Trace.next_center(), VisiblePanel::Memory);
        assert_eq!(VisiblePanel::Memory.next_center(), VisiblePanel::Reports);
        assert_eq!(VisiblePanel::Reports.next_center(), VisiblePanel::Run);
    }

    #[test]
    fn layout_mode_breakpoints_are_single_source_of_truth() {
        assert_eq!(layout_mode_for_width(120), LayoutMode::Wide);
        assert_eq!(layout_mode_for_width(119), LayoutMode::Medium);
        assert_eq!(layout_mode_for_width(88), LayoutMode::Medium);
        assert_eq!(layout_mode_for_width(87), LayoutMode::Single);
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
        let plan = test_plan();
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
            engine,
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
