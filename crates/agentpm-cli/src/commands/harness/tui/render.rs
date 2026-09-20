use super::*;

pub(super) fn render_app(frame: &mut Frame<'_>, app: &mut TuiApp) {
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
    let working_height = working_section_height(app);
    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(0),
            Constraint::Length(1),
            Constraint::Length(working_height),
        ])
        .split(area);
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
        ])
        .split(outer[0]);
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
    render_working_section(frame, outer[2], app);
}

fn working_section_height(app: &TuiApp) -> u16 {
    let run = app.run_snapshot();
    let mut content_lines = 2;
    if run.and_then(|run| run.approval.as_ref()).is_some() {
        if run.and_then(|run| run.approval_control.as_ref()).is_some() {
            content_lines += 1;
        }
    } else {
        if latest_run_progress(app).is_some() {
            content_lines += 1;
        }
        if run.and_then(|run| run.approval_control.as_ref()).is_some() {
            content_lines += 1;
        }
    }
    if app.selected_memory_operation().is_some() {
        content_lines += 1;
    }
    if run
        .and_then(|run| run.memory_operation_control.as_ref())
        .is_some()
    {
        content_lines += 1;
    }
    (content_lines + 2).clamp(5, 9)
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
            composer_visible_input(&app.composer_input, area.width.saturating_sub(4) as usize),
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

fn composer_visible_input(input: &str, width: usize) -> String {
    if width == 0 {
        return String::new();
    }
    let char_count = input.chars().count();
    if char_count <= width {
        return input.to_string();
    }
    if width == 1 {
        return input.chars().last().unwrap_or_default().to_string();
    }
    let tail = input
        .chars()
        .skip(char_count.saturating_sub(width - 1))
        .collect::<String>();
    format!("…{tail}")
}

fn render_working_section(frame: &mut Frame<'_>, area: Rect, app: &TuiApp) {
    let run = app.run_snapshot();
    if let Some(approval) = run.and_then(|run| run.approval.as_ref()) {
        let mut lines = vec![Line::from(vec![
            styled(
                format!(
                    "Checkpoint {} is waiting before phase {}",
                    approval.checkpoint_id, approval.before_phase
                ),
                STATUS_WARNING,
            ),
            Span::raw("    "),
            key_span("A", app.accent),
            Span::raw(" Approve  "),
            key_span("D", app.accent),
            Span::raw(" Deny"),
            memory_operation_inline_control(app),
        ])];
        if let Some(control) = run.and_then(|run| run.approval_control.as_ref()) {
            lines.push(approval_control_line(control));
        }
        append_memory_operation_status_line(app, &mut lines);
        lines.push(Line::from(vec![
            Span::styled(
                "Run remains active until the approval decision is routed through the Engine.",
                Style::default().fg(TEXT_MUTED),
            ),
            Span::raw("    "),
            key_span("C", app.accent),
            Span::raw(" Cancel Run"),
        ]));
        render_run_section(frame, area, " Approval Required ", lines, STATUS_WARNING);
        return;
    }
    let phase = app
        .run_snapshot()
        .and_then(|run| run.phase_id.as_deref())
        .unwrap_or("starting");
    let mut lines = vec![Line::from(vec![
        styled(
            format!("Run is active - phase {phase} in progress"),
            STATUS_WARNING,
        ),
        Span::raw("    "),
        key_span("C", app.accent),
        Span::raw(" Cancel Run"),
        memory_operation_inline_control(app),
    ])];
    append_memory_operation_status_line(app, &mut lines);
    lines.push(Line::from(Span::styled(
        "Composer reopens when this Run reaches a terminal state.",
        Style::default().fg(TEXT_MUTED),
    )));
    if let Some(progress) = latest_run_progress(app) {
        lines.push(Line::from(vec![
            styled("status ", STATUS_WARNING),
            Span::styled(progress.to_string(), Style::default().fg(TEXT_MUTED)),
        ]));
    }
    if let Some(control) = run.and_then(|run| run.approval_control.as_ref()) {
        lines.push(approval_control_line(control));
    }
    render_run_section(frame, area, " Run In Progress ", lines, STATUS_WARNING);
}

fn memory_operation_inline_control(app: &TuiApp) -> Span<'static> {
    if app.selected_memory_operation().is_none() {
        return Span::raw("");
    }
    Span::styled(
        "    X Memory Op",
        Style::default().fg(app.accent).add_modifier(Modifier::BOLD),
    )
}

fn append_memory_operation_status_line(app: &TuiApp, lines: &mut Vec<Line<'static>>) {
    if let Some(operation) = app.selected_memory_operation() {
        let count = app
            .run_snapshot()
            .map(|run| run.memory_operations.len())
            .unwrap_or_default();
        let mut operation_line = vec![
            Span::styled("External Memory ", Style::default().fg(TEXT_MUTED)),
            Span::styled(
                format!("{}/operations/{}", operation.package, operation.operation),
                Style::default().fg(app.accent).add_modifier(Modifier::BOLD),
            ),
            Span::raw("    "),
            key_span("X", app.accent),
            Span::raw(" Invoke"),
        ];
        if count > 1 {
            operation_line.extend([
                Span::raw("  "),
                key_span("]", app.accent),
                Span::raw(" Next"),
            ]);
        }
        lines.push(Line::from(operation_line));
    }
    if let Some(control) = app
        .run_snapshot()
        .and_then(|run| run.memory_operation_control.as_ref())
    {
        lines.push(memory_control_line(control));
    }
}

fn latest_run_progress(app: &TuiApp) -> Option<&str> {
    let TuiState::Running { progress, .. } = &app.state else {
        return None;
    };
    progress.last().map(|item| item.message.as_str())
}

fn approval_control_line(control: &TuiApprovalControlSnapshot) -> Line<'static> {
    let color = match control.status.as_str() {
        TUI_APPROVAL_STATUS_APPROVED => STATUS_READY,
        TUI_APPROVAL_STATUS_DENIED => STATUS_WARNING,
        _ => TEXT_MUTED,
    };
    Line::from(vec![
        styled(format!("approval {} ", control.status), color),
        Span::styled(
            format!("{} - {}", control.checkpoint_id, control.message),
            Style::default().fg(TEXT_MUTED),
        ),
    ])
}

fn memory_control_line(control: &TuiMemoryOperationControlSnapshot) -> Line<'static> {
    let color = match control.status.as_str() {
        TUI_MEMORY_CONTROL_STATUS_COMPLETED => STATUS_READY,
        TUI_MEMORY_CONTROL_STATUS_FAILED => Color::Red,
        _ => STATUS_WARNING,
    };
    Line::from(vec![
        styled(format!("memory {} ", control.status), color),
        Span::styled(
            format!("{} - {}", control.identity, control.message),
            Style::default().fg(TEXT_MUTED),
        ),
    ])
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
    if app.can_decide_approval() {
        spans.extend([key_span("A", app.accent), Span::raw(" Approve  ")]);
        spans.extend([key_span("D", app.accent), Span::raw(" Deny  ")]);
    }
    if app.can_cancel_run() {
        spans.extend([key_span("C", app.accent), Span::raw(" Cancel Run  ")]);
    }
    if app.can_invoke_memory_operation() {
        spans.extend([key_span("X", app.accent), Span::raw(" Memory Op  ")]);
        if app
            .run_snapshot()
            .is_some_and(|run| run.memory_operations.len() > 1)
        {
            spans.extend([key_span("]", app.accent), Span::raw(" Next Op  ")]);
        }
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
    let run_id = run_header_text(snapshot.run.run_number, snapshot.run.run_id.as_deref());
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
    let mut lines = match (
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
    };
    if let Some(control) = &snapshot.run.approval_control {
        lines.push(label_value_line(
            "Approval",
            format!("{} - {}", control.status, control.message),
        ));
    }
    if let Some(control) = &snapshot.run.memory_operation_control {
        lines.push(label_value_line(
            "Memory control",
            format!("{} - {}", control.status, control.message),
        ));
    }
    lines
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

pub(super) fn assistant_output_text(app: &TuiApp) -> Option<String> {
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
        if let Some(err) = &snapshot.reports.current_trace_error {
            return vec![
                Line::from(styled("Trace unavailable", STATUS_WARNING)),
                Line::from(err.clone()),
            ];
        }
        let events = if !snapshot.reports.current_trace_events.is_empty() {
            &snapshot.reports.current_trace_events
        } else {
            &snapshot.trace.events
        };
        let lines = scoped_center_trace_events(snapshot, events)
            .into_iter()
            .map(|event| Line::from(event_trace_line(event)))
            .collect::<Vec<_>>();
        if !lines.is_empty() {
            return lines;
        }
        return vec![Line::from(if snapshot.run.run_id.is_some() {
            "No trace events recorded for this Run yet."
        } else {
            "preflight_completed has not been recorded yet."
        })];
    }
    trace_lines(app)
}

fn scoped_center_trace_events<'a>(
    snapshot: &TuiSessionSnapshot,
    events: &'a [HarnessEventEnvelope],
) -> Vec<&'a HarnessEventEnvelope> {
    match snapshot.run.run_id.as_deref() {
        Some(run_id) => events
            .iter()
            .filter(|event| event.run_id.as_deref() == Some(run_id))
            .collect(),
        None => events
            .iter()
            .filter(|event| event.event_type == HarnessEventType::PreflightCompleted)
            .collect(),
    }
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
                run_header_text(snapshot.run.run_number, snapshot.run.run_id.as_deref()),
                snapshot.run.phase_id.as_deref().unwrap_or("idle"),
                run_status_display_text(&snapshot.run),
                run_status_display_color(&snapshot.run),
            )
        }
        TuiState::Running { snapshot, .. } => (
            run_header_text(snapshot.run.run_number, snapshot.run.run_id.as_deref()),
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
            run_header_text(snapshot.run.run_number, snapshot.run.run_id.as_deref()),
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

fn run_header_text(run_number: Option<u64>, run_id: Option<&str>) -> String {
    run_number
        .map(|number| format!("Run #{number}"))
        .or_else(|| {
            run_id.map(|run_id| format!("Run {}", run_id.rsplit('-').next().unwrap_or(run_id)))
        })
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

#[cfg(test)]
mod tests {
    use super::super::test_support::*;
    use super::*;
    use crate::harness_engine::RuntimeTerminalResult;
    use crate::harness_observability::{
        HarnessEventEnvelope, HarnessEventPayload, HarnessEventSink, HarnessEventType,
        RunOutputPaths,
    };
    use std::collections::BTreeMap;
    use std::sync::{Arc, atomic::AtomicBool, mpsc};

    #[test]
    fn layout_mode_breakpoints_are_single_source_of_truth() {
        assert_eq!(layout_mode_for_width(120), LayoutMode::Wide);
        assert_eq!(layout_mode_for_width(119), LayoutMode::Medium);
        assert_eq!(layout_mode_for_width(88), LayoutMode::Medium);
        assert_eq!(layout_mode_for_width(87), LayoutMode::Single);
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
    fn center_trace_before_first_run_only_shows_preflight_completed() {
        let mut app =
            TuiApp::ready_with_runtime_inputs(test_controller(), HarnessArgs::default(), None);
        let TuiState::Ready { controller } = &mut app.state else {
            panic!("ready app expected");
        };
        controller.snapshot.trace.events = vec![
            test_trace_event("evt-session", HarnessEventType::SessionStarted, None),
            test_trace_event("evt-preflight", HarnessEventType::PreflightCompleted, None),
            test_trace_event("evt-surface", HarnessEventType::McpSurfaceReady, None),
        ];

        let lines = center_trace_event_lines(&app)
            .into_iter()
            .map(|line| line.to_string())
            .collect::<Vec<_>>();

        assert_eq!(lines.len(), 1);
        assert!(lines[0].contains("preflight_completed"));
        assert!(!lines[0].contains("session_started"));
        assert!(!lines[0].contains("mcp_surface_ready"));
    }

    #[test]
    fn center_trace_filters_events_to_displayed_run() {
        let mut app =
            TuiApp::ready_with_runtime_inputs(test_controller(), HarnessArgs::default(), None);
        let mut snapshot = ready_snapshot(&app).clone();
        snapshot.run.status = TuiRunStatus::Active;
        snapshot.run.run_id = Some("run-current".into());
        snapshot.trace.events = vec![
            test_trace_event(
                "evt-old-model",
                HarnessEventType::ModelRequestStarted,
                Some("run-old"),
            ),
            test_trace_event(
                "evt-current-phase",
                HarnessEventType::PhaseStarted,
                Some("run-current"),
            ),
            test_trace_event("evt-session", HarnessEventType::PreflightCompleted, None),
        ];
        let (_sender, receiver) = mpsc::channel();
        app.state = TuiState::Running {
            snapshot: Box::new(snapshot),
            receiver,
            progress: vec![],
            events: TuiEventBuffer::new(HarnessTraceContent::Redacted, 8),
            cancel: Arc::new(AtomicBool::new(false)),
            approvals: TuiApprovalHandle::new(),
            memory_controls: TuiMemoryControlHandle::new(),
        };

        let lines = center_trace_event_lines(&app)
            .into_iter()
            .map(|line| line.to_string())
            .collect::<Vec<_>>();

        assert_eq!(lines.len(), 1);
        assert!(lines[0].contains("phase_started"));
        assert!(lines[0].contains("run-current"));
        assert!(!lines[0].contains("run-old"));
        assert!(!lines[0].contains("preflight_completed"));
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
    fn composer_visible_input_tracks_the_prompt_tail() {
        assert_eq!(composer_visible_input("short", 10), "short");
        assert_eq!(composer_visible_input("abcdefghij", 5), "…ghij");
        assert_eq!(composer_visible_input("abcdefghij", 1), "j");
        assert_eq!(composer_visible_input("abcdefghij", 0), "");
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
    fn run_header_prefers_session_ordinal_over_backend_run_id_suffix() {
        assert_eq!(
            run_header_text(Some(1), Some("run-18d6e15e0bfc5af0-2")),
            "Run #1"
        );
        assert_eq!(
            run_header_text(Some(2), Some("run-18d6e15e0bfc5af0-3")),
            "Run #2"
        );
        assert_eq!(run_header_text(None, None), "Run --");
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
            approvals: TuiApprovalHandle::new(),
            memory_controls: TuiMemoryControlHandle::new(),
        };
        app.focus = TuiFocus::Panel(VisiblePanel::Run);
        assert!(!app.can_send_message());
        assert!(app.can_cancel_run());
        assert_eq!(run_visual_state(&app), RunVisualState::Active);
    }

    #[test]
    fn active_run_cancel_is_available_from_other_panels() {
        let mut app =
            TuiApp::ready_with_runtime_inputs(test_controller(), HarnessArgs::default(), None);
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
            approvals: TuiApprovalHandle::new(),
            memory_controls: TuiMemoryControlHandle::new(),
        };
        app.panel = VisiblePanel::Trace;
        app.focus = TuiFocus::Panel(VisiblePanel::Trace);

        assert!(app.can_cancel_run());
    }

    #[test]
    fn latest_run_progress_is_available_for_active_status_box() {
        let mut app =
            TuiApp::ready_with_runtime_inputs(test_controller(), HarnessArgs::default(), None);
        let snapshot = ready_snapshot(&app).clone();
        let (_sender, receiver) = mpsc::channel();
        app.state = TuiState::Running {
            snapshot: Box::new(snapshot),
            receiver,
            progress: vec![
                TuiRunProgress {
                    message: "Starting Run.".into(),
                },
                TuiRunProgress {
                    message: "Cancellation requested; waiting for the active operation to stop."
                        .into(),
                },
            ],
            events: TuiEventBuffer::new(HarnessTraceContent::Redacted, 8),
            cancel: Arc::new(AtomicBool::new(false)),
            approvals: TuiApprovalHandle::new(),
            memory_controls: TuiMemoryControlHandle::new(),
        };

        assert_eq!(
            latest_run_progress(&app),
            Some("Cancellation requested; waiting for the active operation to stop.")
        );
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
            status: TUI_APPROVAL_STATUS_APPROVED.into(),
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
    fn run_summary_content_shows_control_outcomes() {
        let mut app =
            TuiApp::ready_with_runtime_inputs(test_controller(), HarnessArgs::default(), None);
        let TuiState::Ready { controller } = &mut app.state else {
            panic!("ready app expected");
        };
        controller.snapshot.run.approval_control = Some(TuiApprovalControlSnapshot {
            checkpoint_id: "approve-response".into(),
            status: TUI_APPROVAL_STATUS_APPROVED.into(),
            message: "Approval approved for checkpoint `approve-response`.".into(),
        });
        controller.snapshot.run.memory_operation_control =
            Some(TuiMemoryOperationControlSnapshot {
                identity: "@zack/memory/operations/delete_user_memory".into(),
                status: TUI_MEMORY_CONTROL_STATUS_COMPLETED.into(),
                message: "Completed; affected 2 record(s).".into(),
            });

        let lines = run_summary_content(&app)
            .into_iter()
            .map(|line| line.to_string())
            .collect::<Vec<_>>();

        assert!(lines.iter().any(|line| line.contains("Approval")));
        assert!(lines.iter().any(|line| line.contains("approved")));
        assert!(lines.iter().any(|line| line.contains("Memory control")));
        assert!(lines.iter().any(|line| line.contains("completed")));
    }

    #[test]
    fn approval_view_can_show_external_memory_operation_control() {
        let mut app =
            TuiApp::ready_with_runtime_inputs(test_controller(), HarnessArgs::default(), None);
        let TuiState::Ready { controller } = &mut app.state else {
            panic!("ready app expected");
        };
        controller.snapshot.run.status = TuiRunStatus::PendingApproval;
        controller.snapshot.run.approval = Some(TuiApprovalSnapshot {
            checkpoint_id: "approve-memory-control".into(),
            before_phase: "respond".into(),
        });
        controller.snapshot.run.memory_operations = vec![TuiMemoryOperationSnapshot {
            package: "@zack/m19-memory".into(),
            operation: "external_delete_current_note".into(),
            operation_type: "delete".into(),
            description: "Delete current note.".into(),
        }];
        controller.snapshot.run.memory_operation_control =
            Some(TuiMemoryOperationControlSnapshot {
                identity: "@zack/m19-memory/operations/external_delete_current_note".into(),
                status: TUI_MEMORY_CONTROL_STATUS_COMPLETED.into(),
                message: "Completed; affected 0 record(s).".into(),
            });

        let mut lines = Vec::new();
        append_memory_operation_status_line(&app, &mut lines);
        let text = lines
            .iter()
            .map(Line::to_string)
            .collect::<Vec<_>>()
            .join("\n");

        assert!(text.contains("External Memory"));
        assert!(text.contains("@zack/m19-memory/operations/external_delete_current_note"));
        assert!(text.contains("Invoke"));
        assert!(text.contains("completed"));
    }

    #[test]
    fn working_section_reserves_space_for_memory_operation_status() {
        let mut app =
            TuiApp::ready_with_runtime_inputs(test_controller(), HarnessArgs::default(), None);
        let TuiState::Ready { controller } = &mut app.state else {
            panic!("ready app expected");
        };
        controller.snapshot.run.status = TuiRunStatus::PendingApproval;
        controller.snapshot.run.approval = Some(TuiApprovalSnapshot {
            checkpoint_id: "approve-memory-control".into(),
            before_phase: "respond".into(),
        });
        controller.snapshot.run.approval_control = Some(TuiApprovalControlSnapshot {
            checkpoint_id: "approve-memory-control".into(),
            status: TUI_APPROVAL_STATUS_APPROVED.into(),
            message: "Approval approved.".into(),
        });
        controller.snapshot.run.memory_operations = vec![TuiMemoryOperationSnapshot {
            package: "@zack/m19-memory".into(),
            operation: "external_delete_current_note".into(),
            operation_type: "delete".into(),
            description: "Delete current note.".into(),
        }];
        controller.snapshot.run.memory_operation_control =
            Some(TuiMemoryOperationControlSnapshot {
                identity: "@zack/m19-memory/operations/external_delete_current_note".into(),
                status: TUI_MEMORY_CONTROL_STATUS_COMPLETED.into(),
                message: "Completed; affected 0 record(s).".into(),
            });

        assert_eq!(working_section_height(&app), 7);
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
            approvals: TuiApprovalHandle::new(),
            memory_controls: TuiMemoryControlHandle::new(),
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
            approvals: TuiApprovalHandle::new(),
            memory_controls: TuiMemoryControlHandle::new(),
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
            approvals: TuiApprovalHandle::new(),
            memory_controls: TuiMemoryControlHandle::new(),
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

    fn test_trace_event(
        event_id: &str,
        event_type: HarnessEventType,
        run_id: Option<&str>,
    ) -> HarnessEventEnvelope {
        HarnessEventEnvelope {
            schema_version: 1,
            event_id: event_id.into(),
            session_id: "sess-test".into(),
            run_id: run_id.map(str::to_string),
            session_sequence: 1,
            run_sequence: run_id.map(|_| 1),
            timestamp: chrono::Utc::now(),
            event_type,
            phase_execution_id: None,
            correlation_id: None,
            parent_event_id: None,
            payload: HarnessEventPayload::Lifecycle {
                message: "test event".into(),
                fields: BTreeMap::new(),
            },
        }
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
            approvals: TuiApprovalHandle::new(),
            memory_controls: TuiMemoryControlHandle::new(),
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
}
