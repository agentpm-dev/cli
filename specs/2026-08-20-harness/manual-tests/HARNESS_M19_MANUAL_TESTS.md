# Harness M19 TUI Manual Verification

These checks use the generated fixture directory from `scripts/setup-harness-m19-manual.sh`.

## Setup

```bash
cargo build -p agentpm-cli
scripts/setup-harness-m19-manual.sh
source harness-m19-test/env.sh
```

If the setup script says Memory contracts were not built, run:

```bash
harness-m19-test/build-memory-contracts.sh
```

## 1. Bootstrap And Ready Workspace

```bash
(cd "$M19_WORK" && "$APM" harness)
```

Verify the TUI opens with the loading shell, then reaches a ready workspace. The top bar should show `AgentPM Harness`, the configured branding (`Acme Operations · M19 TUI fixture`), and trace/content labels. The left rail should show readiness for Agent, Loop, Consumer Context, Model, Tools, Skills, Knowledge, Memory, Profiles, Hooks, MCP Exports, and MCP Imports.

## 2. Diagnostics, Details, And Workspace Paging

```bash
(cd "$M19_DIAGNOSTICS_WORK" && "$APM" harness)
```

Verify warning diagnostics render with spacing and the divider below readiness items. Press `D` to expand/collapse diagnostic messages. Shrink terminal height until the Preflight panel pages, then use `Shift+1` to cycle pages. The footer should stay pinned at the panel bottom.

## 3. Interactive Resolution

```bash
(cd "$M19_MISSING_SCOPE_WORK" && "$APM" harness)
(cd "$M19_MISSING_MODEL_WORK" && "$APM" harness)
(cd "$M19_SELECTION_WORK" && "$APM" harness)
```

For missing scope, press `S` and enter `user=m19-user`. For missing model, press `P` and enter `openai/gpt-4o-mini`. For agent selection, press `A` and enter `@zack/m19-tui-agent`. Verify invalid prompt answers stay editable instead of crashing or trapping the TUI.

## 4. Active Run Versus Terminal Run

```bash
(cd "$M19_WORK" && "$APM" harness)
```

Submit a prompt. While active, the Run view should show current Run/phase state, no editable composer, a visible `C Cancel Run` control, active progress, and the current phase output only. After completion, the Run Summary replaces the active phase objective, the composer returns with placeholder text only, latest output remains visible, and the bottom-right status shows the completed Run id, terminal label, and duration.

## 5. Approvals, Cancellation, And Repeated Runs

```bash
(cd "$M19_APPROVAL_WORK" && "$APM" harness)
```

Use a prompt that reaches the approval checkpoint. Verify `A` approves, `D` denies, and `C` cancels without stranding the worker. Start another Run after cancellation and verify it is not automatically cancelled and the header follows the current/last Run correctly.

## 6. External Memory Operation Controls

```bash
(cd "$M19_MEMORY_CONTROL_WORK" && "$APM" harness)
```

Reach the approval checkpoint, press `X`, and verify the Run control box shows Memory operation queued/running/completed or failed status. Approve afterward and verify the terminal Run view retains the Memory operation outcome.

## 7. Trace Modes, Report Paths, And Expanded Output

Run any terminal case, then switch between Run, Trace, Memory, and Reports. Press `O` on the Run tab after output exists. Verify the expanded output view owns focus, shows report/trace paths, supports paging/scrolling, and its footer replaces normal Run keybindings until `Esc` closes it.

## 8. Responsive Layouts

Repeat the ready workspace case at three terminal widths:

- wide (`>=120` columns): left rail, center panel, and right event stream are visible
- medium (`88-119` columns): left rail and center remain visible, right event stream is accessible via panel switching
- small (`<88` columns): one panel is visible at a time, and Workspace, Run, Trace, Event Stream, Memory, and Reports remain reachable

In the small layout, the expanded output view should behave as the focused Run-panel detail view rather than a hidden second panel.

## 9. Fatal Bootstrap Failure

```bash
(cd "$M19_BROKEN_WORK" && "$APM" harness)
```

Verify the TUI reaches the failed bootstrap/preflight state rather than an interactive resolution prompt. The top bar should show `[Trace: Unavailable]`, the Workspace panel should show `Preflight failed` with the parse error and `Press Q to exit.`, and the event stream should show `preflight_failed`.

## 10. Non-TTY Fallback

```bash
TERM=dumb "$APM" harness --config "$M19_WORK/agentpm.harness.json"
```

Verify the command does not enter an unusable raw-mode TUI on a dumb terminal and returns an actionable non-TTY path.
