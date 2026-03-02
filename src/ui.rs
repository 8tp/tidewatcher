use std::cmp;

use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Modifier, Style},
    symbols,
    text::{Line, Span},
    widgets::{Block, Clear, Gauge, Paragraph, Sparkline, Wrap},
    Frame,
};

use crate::{
    app::{App, FocusPanel},
    metrics::{CoreSnapshot, DiskSnapshot, NetworkSnapshot, ProcessSnapshot, Snapshot},
    theme::{Theme, TideArtVariant},
};

const MIN_WIDTH: u16 = 96;
const MIN_HEIGHT: u16 = 26;
const TIDE_SPLIT_MIN_WIDTH: u16 = 72;
const TIDE_ART_MIN_WIDTH: u16 = 24;
const TIDE_ART_MAX_WIDTH: u16 = 34;

pub fn render(frame: &mut Frame, app: &App) {
    let theme = app.theme();
    let area = frame.area();

    frame.render_widget(
        Block::default().style(Style::new().bg(theme.background)),
        area,
    );

    if area.width < MIN_WIDTH || area.height < MIN_HEIGHT {
        render_too_small(frame, area, theme);
        return;
    }

    let [header_area, body_area, footer_area] = Layout::vertical([
        Constraint::Length(2),
        Constraint::Min(0),
        Constraint::Length(2),
    ])
    .areas(area);

    render_header(frame, app, header_area, theme);

    let process_height = if body_area.height >= 18 { 11 } else { 9 };
    let [overview_area, process_area] =
        Layout::vertical([Constraint::Min(0), Constraint::Length(process_height)]).areas(body_area);

    render_overview(frame, app, overview_area, theme);
    render_process_panel(frame, app, process_area, theme);
    render_footer(frame, app, footer_area, theme);

    if app.process_detail_open() {
        render_process_detail_modal(frame, app, theme);
    }
}

fn render_header(frame: &mut Frame, app: &App, area: Rect, theme: Theme) {
    let line = Line::from(vec![
        Span::styled(
            "Tidewatcher",
            Style::new()
                .fg(theme.highlight)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("   theme ", Style::new().fg(theme.muted)),
        Span::styled(app.theme().name, Style::new().fg(theme.text)),
        Span::styled("   refresh ", Style::new().fg(theme.muted)),
        Span::styled(
            format_duration(app.refresh_interval().as_millis() as u64),
            Style::new().fg(theme.text),
        ),
        Span::styled("   focus ", Style::new().fg(theme.muted)),
        Span::styled(app.focus().label(), Style::new().fg(theme.text)),
        Span::styled("   sort ", Style::new().fg(theme.muted)),
        Span::styled(app.process_sort().label(), Style::new().fg(theme.text)),
        Span::styled("   proc ", Style::new().fg(theme.muted)),
        Span::styled(app.process_count().to_string(), Style::new().fg(theme.text)),
    ]);

    frame.render_widget(
        Paragraph::new(line).style(Style::new().bg(theme.surface).fg(theme.text)),
        area,
    );
}

fn render_overview(frame: &mut Frame, app: &App, area: Rect, theme: Theme) {
    if area.width >= 120 {
        let [top_row, bottom_row] =
            Layout::vertical([Constraint::Percentage(50), Constraint::Percentage(50)]).areas(area);
        let [cpu_area, memory_area] =
            Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)])
                .areas(top_row);
        let [disk_area, network_area] =
            Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)])
                .areas(bottom_row);

        render_cpu_panel(frame, app, cpu_area, theme);
        render_memory_panel(frame, app, memory_area, theme);
        render_disk_panel(frame, app, disk_area, theme);
        render_network_panel(frame, app, network_area, theme);
    } else {
        let [cpu_area, memory_area, disk_area, network_area] = Layout::vertical([
            Constraint::Percentage(25),
            Constraint::Percentage(25),
            Constraint::Percentage(25),
            Constraint::Percentage(25),
        ])
        .areas(area);

        render_cpu_panel(frame, app, cpu_area, theme);
        render_memory_panel(frame, app, memory_area, theme);
        render_disk_panel(frame, app, disk_area, theme);
        render_network_panel(frame, app, network_area, theme);
    }
}

fn render_cpu_panel(frame: &mut Frame, app: &App, area: Rect, theme: Theme) {
    let block = Block::bordered()
        .title("CPU")
        .style(Style::new().bg(theme.surface).fg(theme.text))
        .border_style(panel_border_style(app.is_selected(FocusPanel::Cpu), theme));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let snapshot = app.snapshot();

    if inner.height >= 10 {
        let [gauge_area, history_area, cores_area] = Layout::vertical([
            Constraint::Length(3),
            Constraint::Length(5),
            Constraint::Min(0),
        ])
        .areas(inner);
        render_usage_gauge(
            frame,
            gauge_area,
            "System CPU Avg",
            snapshot.global_cpu,
            theme,
        );
        render_history(frame, history_area, "Wave", app.cpu_history(), 100, theme);
        render_core_list(frame, cores_area, snapshot, theme);
    } else if inner.height >= 6 {
        let [gauge_area, cores_area] =
            Layout::vertical([Constraint::Length(3), Constraint::Min(0)]).areas(inner);
        render_usage_gauge(
            frame,
            gauge_area,
            "System CPU Avg",
            snapshot.global_cpu,
            theme,
        );
        render_core_list(frame, cores_area, snapshot, theme);
    } else if inner.height >= 3 {
        render_usage_gauge(frame, inner, "System CPU Avg", snapshot.global_cpu, theme);
    } else {
        render_panel_notice(frame, inner, "Expand for CPU details", theme);
    }
}

fn render_memory_panel(frame: &mut Frame, app: &App, area: Rect, theme: Theme) {
    let block = Block::bordered()
        .title("Memory")
        .style(Style::new().bg(theme.surface).fg(theme.text))
        .border_style(panel_border_style(
            app.is_selected(FocusPanel::Memory),
            theme,
        ));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let snapshot = app.snapshot();

    if inner.height >= 10 {
        let [gauge_area, history_area, stats_area] = Layout::vertical([
            Constraint::Length(3),
            Constraint::Length(5),
            Constraint::Min(0),
        ])
        .areas(inner);
        render_usage_gauge(
            frame,
            gauge_area,
            "RAM In Use",
            snapshot.memory_percent(),
            theme,
        );
        render_history(
            frame,
            history_area,
            "Pressure",
            app.memory_history(),
            100,
            theme,
        );
        render_memory_stats(frame, stats_area, snapshot, theme);
    } else if inner.height >= 6 {
        let [gauge_area, stats_area] =
            Layout::vertical([Constraint::Length(3), Constraint::Min(0)]).areas(inner);
        render_usage_gauge(
            frame,
            gauge_area,
            "RAM In Use",
            snapshot.memory_percent(),
            theme,
        );
        render_memory_stats(frame, stats_area, snapshot, theme);
    } else if inner.height >= 3 {
        render_usage_gauge(frame, inner, "RAM In Use", snapshot.memory_percent(), theme);
    } else {
        render_panel_notice(frame, inner, "Expand for memory details", theme);
    }
}

fn render_disk_panel(frame: &mut Frame, app: &App, area: Rect, theme: Theme) {
    let block = Block::bordered()
        .title("Disk")
        .style(Style::new().bg(theme.surface).fg(theme.text))
        .border_style(panel_border_style(app.is_selected(FocusPanel::Disk), theme));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let snapshot = app.snapshot();

    if inner.height >= 6 {
        let [gauge_area, details_area] =
            Layout::vertical([Constraint::Length(3), Constraint::Min(0)]).areas(inner);
        render_usage_gauge(frame, gauge_area, "Storage", snapshot.disk_percent(), theme);
        render_disk_list(frame, details_area, snapshot, theme);
    } else if inner.height >= 3 {
        render_usage_gauge(frame, inner, "Storage", snapshot.disk_percent(), theme);
    } else {
        render_panel_notice(frame, inner, "Expand for disk details", theme);
    }
}

fn render_network_panel(frame: &mut Frame, app: &App, area: Rect, theme: Theme) {
    let block = Block::bordered()
        .title("Network")
        .style(Style::new().bg(theme.surface).fg(theme.text))
        .border_style(panel_border_style(
            app.is_selected(FocusPanel::Network),
            theme,
        ));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let snapshot = app.snapshot();

    if inner.height >= 7 {
        let [summary_area, details_area] =
            Layout::vertical([Constraint::Length(4), Constraint::Min(0)]).areas(inner);
        render_network_summary(frame, summary_area, snapshot, theme);
        render_network_list(frame, details_area, snapshot, theme);
    } else if inner.height >= 4 {
        render_network_summary(frame, inner, snapshot, theme);
    } else {
        render_panel_notice(frame, inner, "Expand for network details", theme);
    }
}

fn render_process_panel(frame: &mut Frame, app: &App, area: Rect, theme: Theme) {
    let title = format!("Processes [{} | per-core]", app.process_sort().label());
    let block = Block::bordered()
        .title(title)
        .style(Style::new().bg(theme.surface).fg(theme.text))
        .border_style(panel_border_style(
            app.is_selected(FocusPanel::Processes),
            theme,
        ));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.height >= 3 {
        if inner.width >= TIDE_SPLIT_MIN_WIDTH && inner.height >= 5 {
            let tide_width = (inner.width / 3).clamp(TIDE_ART_MIN_WIDTH, TIDE_ART_MAX_WIDTH);
            let [list_area, tide_area] =
                Layout::horizontal([Constraint::Min(38), Constraint::Length(tide_width)])
                    .areas(inner);
            render_process_list(frame, list_area, app, theme);
            render_tide_art(frame, tide_area, app, theme);
        } else {
            render_process_list(frame, inner, app, theme);
        }
    } else {
        render_panel_notice(frame, inner, "Expand for process details", theme);
    }
}

fn render_process_detail_modal(frame: &mut Frame, app: &App, theme: Theme) {
    let Some(process) = app.selected_process() else {
        return;
    };

    let area = centered_rect(frame.area(), 74, 58);
    let block = Block::bordered()
        .title("Process Detail")
        .style(Style::new().bg(theme.surface).fg(theme.text))
        .border_style(Style::new().fg(theme.highlight));
    let inner = block.inner(area);

    frame.render_widget(Clear, area);
    frame.render_widget(block, area);

    let mut lines = Vec::new();
    if let Some(message) = app.status_message() {
        lines.push(Line::from(vec![
            Span::styled("status ", Style::new().fg(theme.muted)),
            Span::styled(message.to_string(), Style::new().fg(theme.highlight)),
        ]));
        lines.push(Line::from(""));
    }

    lines.push(Line::from(vec![
        Span::styled("name   ", Style::new().fg(theme.muted)),
        Span::styled(process.name.clone(), Style::new().fg(theme.text)),
    ]));
    lines.push(Line::from(vec![
        Span::styled("pid    ", Style::new().fg(theme.muted)),
        Span::styled(process.pid.to_string(), Style::new().fg(theme.text)),
        Span::styled("   parent ", Style::new().fg(theme.muted)),
        Span::styled(
            process
                .parent_pid
                .map(|pid| pid.to_string())
                .unwrap_or_else(|| "-".to_string()),
            Style::new().fg(theme.text),
        ),
        Span::styled("   status ", Style::new().fg(theme.muted)),
        Span::styled(process.status.clone(), Style::new().fg(theme.text)),
    ]));
    lines.push(Line::from(vec![
        Span::styled("cpu/core ", Style::new().fg(theme.muted)),
        Span::styled(
            format!("{:.1}%", process.cpu_usage),
            Style::new().fg(theme.text),
        ),
        Span::styled("   rss ", Style::new().fg(theme.muted)),
        Span::styled(format_bytes(process.memory), Style::new().fg(theme.text)),
        Span::styled("   io ", Style::new().fg(theme.muted)),
        Span::styled(
            format!(
                "{} read / {} write",
                format_rate(process.read_bytes, app.snapshot().sample_interval_secs()),
                format_rate(process.written_bytes, app.snapshot().sample_interval_secs())
            ),
            Style::new().fg(theme.text),
        ),
    ]));
    lines.push(Line::from(vec![
        Span::styled("uptime ", Style::new().fg(theme.muted)),
        Span::styled(
            format_runtime_secs(process.run_time_secs),
            Style::new().fg(theme.text),
        ),
        Span::styled("   sort ", Style::new().fg(theme.muted)),
        Span::styled(app.process_sort().label(), Style::new().fg(theme.text)),
    ]));
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "command",
        Style::new()
            .fg(theme.highlight)
            .add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::from(Span::styled(
        process.command.clone(),
        Style::new().fg(theme.text),
    )));
    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled("Up/Down", Style::new().fg(theme.highlight)),
        Span::styled(" select   ", Style::new().fg(theme.muted)),
        Span::styled("s", Style::new().fg(theme.highlight)),
        Span::styled(" sort   ", Style::new().fg(theme.muted)),
        Span::styled("x", Style::new().fg(theme.highlight)),
        Span::styled(" term   ", Style::new().fg(theme.muted)),
        Span::styled("k", Style::new().fg(theme.highlight)),
        Span::styled(" kill   ", Style::new().fg(theme.muted)),
        Span::styled("Esc", Style::new().fg(theme.highlight)),
        Span::styled(" close", Style::new().fg(theme.muted)),
    ]));

    frame.render_widget(
        Paragraph::new(lines)
            .wrap(Wrap { trim: true })
            .style(Style::new().bg(theme.surface).fg(theme.text)),
        inner,
    );
}

fn render_footer(frame: &mut Frame, app: &App, area: Rect, theme: Theme) {
    let status = app.status_message().unwrap_or("");
    let controls = if app.process_detail_open() {
        "Up/Down select   s sort   x term   k kill   Esc close   q quit"
    } else if app.focus() == FocusPanel::Processes {
        "Left/Right/Tab focus   Up/Down select   Enter detail   s sort   t theme   +/- refresh   q quit"
    } else {
        "Left/Right/Tab focus   Up/Down move   t theme   +/- refresh   q quit"
    };

    let line = if status.is_empty() {
        Line::from(Span::styled(controls, Style::new().fg(theme.muted)))
    } else {
        Line::from(vec![
            Span::styled("status ", Style::new().fg(theme.muted)),
            Span::styled(status.to_string(), Style::new().fg(theme.highlight)),
            Span::styled("   ", Style::new().fg(theme.muted)),
            Span::styled(controls, Style::new().fg(theme.muted)),
        ])
    };

    frame.render_widget(
        Paragraph::new(line).style(Style::new().bg(theme.surface).fg(theme.text)),
        area,
    );
}

fn render_too_small(frame: &mut Frame, area: Rect, theme: Theme) {
    let message = vec![
        Line::from(Span::styled(
            "Tidewatcher needs a little more room.",
            Style::new()
                .fg(theme.highlight)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            "Expand the terminal to at least 96x26 cells.",
            Style::new().fg(theme.muted),
        )),
    ];

    let block = Block::bordered()
        .title("Viewport")
        .style(Style::new().bg(theme.surface).fg(theme.text))
        .border_style(Style::new().fg(theme.panel_border));

    frame.render_widget(Paragraph::new(message).block(block), area);
}

fn render_usage_gauge(frame: &mut Frame, area: Rect, title: &str, percent: f32, theme: Theme) {
    frame.render_widget(
        Gauge::default()
            .block(
                Block::bordered()
                    .title(title)
                    .style(Style::new().bg(theme.surface).fg(theme.text))
                    .border_style(Style::new().fg(theme.accent_soft)),
            )
            .ratio(ratio(percent))
            .use_unicode(true)
            .label(format!("{percent:.1}%"))
            .gauge_style(
                Style::new()
                    .fg(theme.accent)
                    .bg(theme.gauge_track)
                    .add_modifier(Modifier::BOLD),
            ),
        area,
    );
}

fn render_history(
    frame: &mut Frame,
    area: Rect,
    title: &str,
    history: &[u64],
    max: u64,
    theme: Theme,
) {
    frame.render_widget(
        Sparkline::default()
            .block(
                Block::bordered()
                    .title(title)
                    .style(Style::new().bg(theme.surface).fg(theme.text))
                    .border_style(Style::new().fg(theme.accent_soft)),
            )
            .data(history)
            .max(max.max(1))
            .bar_set(symbols::bar::NINE_LEVELS)
            .style(Style::new().fg(theme.highlight).bg(theme.surface)),
        area,
    );
}

fn render_network_summary(frame: &mut Frame, area: Rect, snapshot: &Snapshot, theme: Theme) {
    let packets_in: u64 = snapshot
        .networks
        .iter()
        .map(|network| network.packets_received)
        .sum();
    let packets_out: u64 = snapshot
        .networks
        .iter()
        .map(|network| network.packets_transmitted)
        .sum();
    let lines = if snapshot.networks.is_empty() {
        vec![Line::from(Span::styled(
            "No active interfaces detected",
            Style::new().fg(theme.muted),
        ))]
    } else {
        vec![
            Line::from(vec![
                Span::styled("Down ", Style::new().fg(theme.muted)),
                Span::styled(
                    format_rate(
                        snapshot.total_network_received(),
                        snapshot.sample_interval_secs(),
                    ),
                    Style::new().fg(theme.text),
                ),
                Span::styled("   Up ", Style::new().fg(theme.muted)),
                Span::styled(
                    format_rate(
                        snapshot.total_network_transmitted(),
                        snapshot.sample_interval_secs(),
                    ),
                    Style::new().fg(theme.text),
                ),
            ]),
            Line::from(vec![
                Span::styled("Pkts ", Style::new().fg(theme.muted)),
                Span::styled(
                    format!("{packets_in}/{packets_out}"),
                    Style::new().fg(theme.text),
                ),
                Span::styled("   Ifaces ", Style::new().fg(theme.muted)),
                Span::styled(
                    snapshot.networks.len().to_string(),
                    Style::new().fg(theme.text),
                ),
            ]),
        ]
    };

    frame.render_widget(
        Paragraph::new(lines).block(
            Block::bordered()
                .title("Throughput")
                .style(Style::new().bg(theme.surface).fg(theme.text))
                .border_style(Style::new().fg(theme.accent_soft)),
        ),
        area,
    );
}

fn render_core_list(frame: &mut Frame, area: Rect, snapshot: &Snapshot, theme: Theme) {
    let lines = core_lines(snapshot, area, theme);
    frame.render_widget(
        Paragraph::new(lines).block(
            Block::bordered()
                .title("Per Core")
                .style(Style::new().bg(theme.surface).fg(theme.text))
                .border_style(Style::new().fg(theme.accent_soft)),
        ),
        area,
    );
}

fn render_memory_stats(frame: &mut Frame, area: Rect, snapshot: &Snapshot, theme: Theme) {
    let stats = vec![
        stat_line(
            "RAM",
            &format!(
                "{} / {}",
                format_bytes(snapshot.used_memory),
                format_bytes(snapshot.total_memory)
            ),
            "in use",
            theme,
        ),
        stat_line(
            "Avail",
            &format_bytes(snapshot.available_memory),
            "reusable",
            theme,
        ),
        stat_line(
            "Free",
            &format_bytes(snapshot.free_memory),
            "unallocated",
            theme,
        ),
        if snapshot.total_swap == 0 {
            stat_line("Swap", "n/a", "unavailable", theme)
        } else {
            stat_line(
                "Swap",
                &format!(
                    "{} / {}",
                    format_bytes(snapshot.used_swap),
                    format_bytes(snapshot.total_swap)
                ),
                "used",
                theme,
            )
        },
        stat_line(
            "CPU",
            &format!("{:.1}%", snapshot.global_cpu),
            "global load",
            theme,
        ),
        stat_line(
            "Cores",
            &snapshot.cores.len().to_string(),
            "reported",
            theme,
        ),
    ];

    frame.render_widget(
        Paragraph::new(stats).block(
            Block::bordered()
                .title("Stats")
                .style(Style::new().bg(theme.surface).fg(theme.text))
                .border_style(Style::new().fg(theme.accent_soft)),
        ),
        area,
    );
}

fn render_disk_list(frame: &mut Frame, area: Rect, snapshot: &Snapshot, theme: Theme) {
    let lines = disk_lines(snapshot, area, theme);
    frame.render_widget(
        Paragraph::new(lines).block(
            Block::bordered()
                .title("Mount Series")
                .style(Style::new().bg(theme.surface).fg(theme.text))
                .border_style(Style::new().fg(theme.accent_soft)),
        ),
        area,
    );
}

fn render_network_list(frame: &mut Frame, area: Rect, snapshot: &Snapshot, theme: Theme) {
    let lines = network_lines(snapshot, area, theme);
    frame.render_widget(
        Paragraph::new(lines).block(
            Block::bordered()
                .title("Interface Series")
                .style(Style::new().bg(theme.surface).fg(theme.text))
                .border_style(Style::new().fg(theme.accent_soft)),
        ),
        area,
    );
}

fn render_process_list(frame: &mut Frame, area: Rect, app: &App, theme: Theme) {
    let lines = process_lines(app, area, theme);
    frame.render_widget(
        Paragraph::new(lines).style(Style::new().bg(theme.surface).fg(theme.text)),
        area,
    );
}

fn render_tide_art(frame: &mut Frame, area: Rect, app: &App, theme: Theme) {
    let lines = tide_lines(
        usize::from(area.width),
        usize::from(area.height),
        app.animation_frame(),
        theme,
    );

    frame.render_widget(
        Paragraph::new(lines).style(Style::new().bg(theme.surface).fg(theme.text)),
        area,
    );
}

fn render_panel_notice(frame: &mut Frame, area: Rect, message: &str, theme: Theme) {
    frame.render_widget(
        Paragraph::new(message).style(Style::new().fg(theme.muted).bg(theme.surface)),
        area,
    );
}

fn core_lines(snapshot: &Snapshot, area: Rect, theme: Theme) -> Vec<Line<'static>> {
    let available_rows = usize::from(area.height.saturating_sub(2));
    let bar_width = usize::from(area.width.saturating_sub(18)).clamp(8, 24);
    let visible = cmp::max(1, available_rows.saturating_sub(1));
    let mut lines = snapshot
        .cores
        .iter()
        .take(visible)
        .map(|core| render_core_line(core, bar_width, theme))
        .collect::<Vec<_>>();

    if snapshot.cores.len() > visible {
        lines.push(Line::from(Span::styled(
            format!("+ {} more cores", snapshot.cores.len() - visible),
            Style::new().fg(theme.muted),
        )));
    }

    if lines.is_empty() {
        lines.push(Line::from(Span::styled(
            "No CPU data available",
            Style::new().fg(theme.muted),
        )));
    }

    lines
}

fn disk_lines(snapshot: &Snapshot, area: Rect, theme: Theme) -> Vec<Line<'static>> {
    let available_lines = usize::from(area.height.saturating_sub(2));
    let label_width = usize::from(area.width.saturating_sub(42)).clamp(8, 18);
    let trend_width =
        usize::from(area.width.saturating_sub((label_width + 28) as u16)).clamp(8, 26);

    if snapshot.disks.is_empty() {
        return vec![Line::from(Span::styled(
            "No disk data available",
            Style::new().fg(theme.muted),
        ))];
    }

    let mut lines = vec![Line::from(vec![
        Span::styled(
            format!("{:<label_width$}", "mount"),
            Style::new()
                .fg(theme.highlight)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" used ", Style::new().fg(theme.highlight)),
        Span::styled(" read/s ", Style::new().fg(theme.highlight)),
        Span::styled(" write/s ", Style::new().fg(theme.highlight)),
        Span::styled("trend", Style::new().fg(theme.highlight)),
    ])];

    if available_lines <= 1 {
        return lines;
    }

    let visible = snapshot.disks.len().min(available_lines - 1);
    for disk in snapshot.disks.iter().take(visible) {
        lines.push(render_disk_line(
            disk,
            label_width,
            trend_width,
            snapshot.sample_interval_secs(),
            theme,
        ));
    }

    lines
}

fn network_lines(snapshot: &Snapshot, area: Rect, theme: Theme) -> Vec<Line<'static>> {
    let available_lines = usize::from(area.height.saturating_sub(2));
    let label_width = usize::from(area.width.saturating_sub(42)).clamp(8, 18);
    let trend_width =
        usize::from(area.width.saturating_sub((label_width + 28) as u16)).clamp(8, 26);

    if snapshot.networks.is_empty() {
        return vec![Line::from(Span::styled(
            "No network data available",
            Style::new().fg(theme.muted),
        ))];
    }

    let mut lines = vec![Line::from(vec![
        Span::styled(
            format!("{:<label_width$}", "iface"),
            Style::new()
                .fg(theme.highlight)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" down/s ", Style::new().fg(theme.highlight)),
        Span::styled(" up/s ", Style::new().fg(theme.highlight)),
        Span::styled(" total ", Style::new().fg(theme.highlight)),
        Span::styled("trend", Style::new().fg(theme.highlight)),
    ])];

    if available_lines <= 1 {
        return lines;
    }

    let visible = snapshot.networks.len().min(available_lines - 1);
    for network in snapshot.networks.iter().take(visible) {
        lines.push(render_network_line(
            network,
            label_width,
            trend_width,
            snapshot.sample_interval_secs(),
            theme,
        ));
    }

    lines
}

fn process_lines(app: &App, area: Rect, theme: Theme) -> Vec<Line<'static>> {
    let available_lines = usize::from(area.height);
    let name_width = usize::from(area.width.saturating_sub(36)).clamp(10, 28);
    let mut lines = vec![Line::from(vec![
        header_cell("PID", 6, theme),
        header_cell("Core%", 6, theme),
        Span::raw(" "),
        header_cell("RSS", 7, theme),
        Span::raw(" "),
        header_cell("I/O", 7, theme),
        Span::raw(" "),
        header_cell("Name", name_width, theme),
    ])];

    if available_lines <= 1 {
        return lines;
    }

    let processes = app.sorted_processes();
    if processes.is_empty() {
        lines.push(Line::from(Span::styled(
            "No process data available",
            Style::new().fg(theme.muted),
        )));
        return lines;
    }

    let visible = processes.len().min(available_lines - 1);
    for process in processes.into_iter().take(visible) {
        lines.push(render_process_line(
            process,
            name_width,
            app.snapshot().sample_interval_secs(),
            app.selected_process_pid() == Some(process.pid),
            theme,
        ));
    }

    lines
}

#[derive(Clone, Copy)]
enum TideLayer {
    Sky,
    Boat,
    Foam,
    Water,
}

fn tide_lines(width: usize, height: usize, frame: usize, theme: Theme) -> Vec<Line<'static>> {
    tide_art_rows(width, height, frame, theme.tide_art)
        .into_iter()
        .map(|(layer, text)| {
            let color = match layer {
                TideLayer::Sky => theme.muted,
                TideLayer::Boat => theme.highlight,
                TideLayer::Foam => theme.highlight,
                TideLayer::Water => theme.accent,
            };

            Line::from(Span::styled(text, Style::new().fg(color)))
        })
        .collect()
}

fn tide_art_rows(
    width: usize,
    height: usize,
    frame: usize,
    variant: TideArtVariant,
) -> Vec<(TideLayer, String)> {
    if width == 0 || height == 0 {
        return Vec::new();
    }

    if width < 12 || height < 5 {
        return (0..height)
            .map(|row| {
                (
                    TideLayer::Water,
                    wave_row(width, frame, row, scene_wave_patterns(variant)),
                )
            })
            .collect();
    }

    let art_height = height.min(7);
    let rows = match variant {
        TideArtVariant::OceanCurrent => ocean_current_rows(width, art_height, frame),
        TideArtVariant::HarborFog => harbor_fog_rows(width, art_height, frame),
        TideArtVariant::SakuraTide => sakura_tide_rows(width, art_height, frame),
        TideArtVariant::MatchaGlass => matcha_glass_rows(width, art_height, frame),
        TideArtVariant::LanternEmber => lantern_ember_rows(width, art_height, frame),
        TideArtVariant::MoonlitKoi => moonlit_koi_rows(width, art_height, frame),
        TideArtVariant::WinterPlum => winter_plum_rows(width, art_height, frame),
    };

    pad_scene(width, height, rows)
}

fn ocean_current_rows(width: usize, art_height: usize, frame: usize) -> Vec<(TideLayer, String)> {
    let (sky_rows, feature_rows, water_rows) = scene_bands(art_height);
    let boat_width = 8;
    let boat_x = drift_x(
        width,
        boat_width,
        frame,
        &[0, 1, 2, 1, 0, 0, 1, 0],
        4,
        false,
    );
    let flutter_to_starboard = frame % 6 < 3;
    let boat = if flutter_to_starboard {
        ["   |\\", " __|_\\__", " \\_____/"]
    } else {
        ["   |/", " __|_/__", " \\_____/"]
    };

    let mut rows = Vec::with_capacity(art_height);
    for row in 0..sky_rows {
        rows.push((TideLayer::Sky, ocean_sky_row(width, frame, row)));
    }

    let boat_start = boat.len().saturating_sub(feature_rows);
    for hull in &boat[boat_start..] {
        rows.push((TideLayer::Boat, positioned_line(width, boat_x, hull)));
    }

    rows.extend(water_rows_for_scene(
        width,
        frame,
        water_rows,
        TideArtVariant::OceanCurrent,
    ));
    rows
}

fn harbor_fog_rows(width: usize, art_height: usize, frame: usize) -> Vec<(TideLayer, String)> {
    let (sky_rows, feature_rows, water_rows) = scene_bands(art_height);
    let tower_x = width.saturating_sub(6);
    let mut rows = Vec::with_capacity(art_height);

    for row in 0..sky_rows {
        rows.push((TideLayer::Sky, fog_sky_row(width, frame, row)));
    }

    if feature_rows == 3 {
        rows.push((
            TideLayer::Boat,
            positioned_line(width, tower_x, if frame % 4 < 2 { " <*>" } else { " <o>" }),
        ));
    }

    rows.push((TideLayer::Boat, positioned_line(width, tower_x, " |#| ")));
    rows.push((TideLayer::Boat, harbor_pier_row(width, tower_x)));

    rows.extend(water_rows_for_scene(
        width,
        frame,
        water_rows,
        TideArtVariant::HarborFog,
    ));
    rows
}

fn sakura_tide_rows(width: usize, art_height: usize, frame: usize) -> Vec<(TideLayer, String)> {
    let (sky_rows, feature_rows, water_rows) = scene_bands(art_height);
    let gate_width = 10;
    let gate_x = drift_x(width, gate_width, frame, &[0, 1, 1, 0], 6, false);
    let gate_three = ["  ======  ", " ===||=== ", "   ||||   "];
    let gate_two = [" ====== ", "  ||||  "];

    let mut rows = Vec::with_capacity(art_height);
    for row in 0..sky_rows {
        rows.push((TideLayer::Sky, sakura_sky_row(width, frame, row)));
    }

    let gate = if feature_rows == 3 {
        gate_three.as_slice()
    } else {
        gate_two.as_slice()
    };
    for beam in gate {
        rows.push((TideLayer::Boat, positioned_line(width, gate_x, beam)));
    }

    rows.extend(water_rows_for_scene(
        width,
        frame,
        water_rows,
        TideArtVariant::SakuraTide,
    ));
    rows
}

fn matcha_glass_rows(width: usize, art_height: usize, frame: usize) -> Vec<(TideLayer, String)> {
    let (sky_rows, feature_rows, water_rows) = scene_bands(art_height);
    let reed_x = 1 + frame % 2;
    let skiff_width = 8;
    let skiff_x = drift_x(width, skiff_width, frame, &[0, 1, 1, 0, 0], 5, false);
    let mut rows = Vec::with_capacity(art_height);

    for row in 0..sky_rows {
        rows.push((TideLayer::Sky, matcha_sky_row(width, frame, row)));
    }

    if feature_rows == 3 {
        rows.push((
            TideLayer::Boat,
            scene_feature_row(width, &[(reed_x, "//"), (skiff_x + 3, "|\\")]),
        ));
    }

    rows.push((
        TideLayer::Boat,
        scene_feature_row(width, &[(reed_x, "||"), (skiff_x, " __|_\\__")]),
    ));
    rows.push((
        TideLayer::Boat,
        scene_feature_row(width, &[(reed_x, "||"), (skiff_x, " \\_____/")]),
    ));

    rows.extend(water_rows_for_scene(
        width,
        frame,
        water_rows,
        TideArtVariant::MatchaGlass,
    ));
    rows
}

fn lantern_ember_rows(width: usize, art_height: usize, frame: usize) -> Vec<(TideLayer, String)> {
    let (sky_rows, feature_rows, water_rows) = scene_bands(art_height);
    let boat_width = 8;
    let boat_x = drift_x(
        width,
        boat_width,
        frame,
        &[0, 0, 1, 1, 2, 1, 0, 0],
        5,
        false,
    );
    let lantern = if frame % 4 < 2 { "o" } else { "O" };
    let mid = if feature_rows == 3 {
        " __|_\\__"
    } else {
        " __|o\\__"
    };
    let mut rows = Vec::with_capacity(art_height);

    for row in 0..sky_rows {
        rows.push((TideLayer::Sky, ember_sky_row(width, frame, row)));
    }

    if feature_rows == 3 {
        rows.push((TideLayer::Boat, positioned_line(width, boat_x + 4, lantern)));
    }
    rows.push((TideLayer::Boat, positioned_line(width, boat_x, mid)));
    rows.push((TideLayer::Boat, positioned_line(width, boat_x, " \\_____/")));

    rows.extend(water_rows_for_scene(
        width,
        frame,
        water_rows,
        TideArtVariant::LanternEmber,
    ));
    rows
}

fn moonlit_koi_rows(width: usize, art_height: usize, frame: usize) -> Vec<(TideLayer, String)> {
    let (sky_rows, feature_rows, water_rows) = scene_bands(art_height);
    let koi = "><((`>";
    let leap_row = [1usize, 0, 0, 1, 2, 2, 1, 0][frame % 8].min(feature_rows.saturating_sub(1));
    let koi_x = drift_x(width, koi.len(), frame, &[0, 3, 6, 9, 12, 9, 6, 3], 2, true);
    let mut rows = Vec::with_capacity(art_height);

    for row in 0..sky_rows {
        rows.push((TideLayer::Sky, koi_sky_row(width, frame, row)));
    }

    for row in 0..feature_rows {
        let line = if row == leap_row {
            positioned_line(width, koi_x, koi)
        } else if row == leap_row.saturating_add(1) {
            positioned_line(
                width,
                koi_x + 1,
                if frame.is_multiple_of(2) {
                    "o o"
                } else {
                    ". ."
                },
            )
        } else {
            " ".repeat(width)
        };
        rows.push((TideLayer::Boat, line));
    }

    rows.extend(water_rows_for_scene(
        width,
        frame,
        water_rows,
        TideArtVariant::MoonlitKoi,
    ));
    rows
}

fn winter_plum_rows(width: usize, art_height: usize, frame: usize) -> Vec<(TideLayer, String)> {
    let (sky_rows, feature_rows, water_rows) = scene_bands(art_height);
    let blossom = if frame % 6 < 3 { "*" } else { "o" };
    let mut rows = Vec::with_capacity(art_height);

    for row in 0..sky_rows {
        rows.push((TideLayer::Sky, winter_sky_row(width, frame, row)));
    }

    if feature_rows == 3 {
        rows.push((TideLayer::Boat, positioned_line(width, 1, "\\\\")));
    }
    rows.push((TideLayer::Boat, positioned_line(width, 2, "\\\\___")));
    rows.push((
        TideLayer::Boat,
        scene_feature_row(width, &[(4, "\\\\__"), (9, blossom)]),
    ));

    rows.extend(water_rows_for_scene(
        width,
        frame,
        water_rows,
        TideArtVariant::WinterPlum,
    ));
    rows
}

fn pad_scene(
    width: usize,
    height: usize,
    rows: Vec<(TideLayer, String)>,
) -> Vec<(TideLayer, String)> {
    let top_padding = height.saturating_sub(rows.len());
    let mut padded = Vec::with_capacity(height);
    padded.extend((0..top_padding).map(|_| (TideLayer::Sky, " ".repeat(width))));
    padded.extend(rows);
    padded
}

fn scene_bands(art_height: usize) -> (usize, usize, usize) {
    let sky_rows = if art_height >= 7 { 2 } else { 1 };
    let feature_rows = if art_height >= 6 { 3 } else { 2 };
    let water_rows = art_height.saturating_sub(sky_rows + feature_rows);
    (sky_rows, feature_rows, water_rows)
}

fn water_rows_for_scene(
    width: usize,
    frame: usize,
    water_rows: usize,
    variant: TideArtVariant,
) -> Vec<(TideLayer, String)> {
    (0..water_rows)
        .map(|row| {
            let layer = if row == 0 {
                TideLayer::Foam
            } else {
                TideLayer::Water
            };
            (
                layer,
                wave_row(width, frame, row, scene_wave_patterns(variant)),
            )
        })
        .collect()
}

fn ocean_sky_row(width: usize, frame: usize, row: usize) -> String {
    let mut line = vec![' '; width];
    let twinkles = if row == 0 {
        [
            (1 + (frame % 3), '.'),
            (width / 3, if frame.is_multiple_of(2) { '*' } else { '+' }),
            (width.saturating_sub(10), '.'),
        ]
    } else {
        [
            (width / 4, '.'),
            (
                width / 2 + (frame % 4),
                if (frame + row).is_multiple_of(2) {
                    '+'
                } else {
                    '.'
                },
            ),
            (width.saturating_sub(7), '.'),
        ]
    };

    for (position, glyph) in twinkles {
        if position < width {
            line[position] = glyph;
        }
    }

    if row == 0 {
        let moon = if frame % 8 < 4 { "( )" } else { "(_)" };
        paint_text(&mut line, width.saturating_sub(moon.len() + 2), moon);
    }

    line.into_iter().collect()
}

fn fog_sky_row(width: usize, frame: usize, row: usize) -> String {
    let pattern = if row == 0 {
        "..   ...    ..   ..."
    } else {
        " ...   ..  ...   .. "
    };
    shifted_pattern(width, pattern, frame + row * 2)
}

fn sakura_sky_row(width: usize, frame: usize, row: usize) -> String {
    let mut line = vec![' '; width];
    let petals = if row == 0 {
        [
            (1 + frame % 5, '*'),
            (width / 3 + frame % 3, '.'),
            (width / 2 + 1, '*'),
            (width.saturating_sub(8), '.'),
        ]
    } else {
        [
            (2 + frame % 4, '.'),
            (width / 4 + 1, '*'),
            (width / 2 + frame % 5, '.'),
            (width.saturating_sub(6), '*'),
        ]
    };

    for (position, glyph) in petals {
        if position < width {
            line[position] = glyph;
        }
    }

    if row == 0 {
        paint_text(&mut line, width.saturating_sub(6), "(_)");
    }

    line.into_iter().collect()
}

fn matcha_sky_row(width: usize, frame: usize, row: usize) -> String {
    let mut line = vec![' '; width];
    let orb = if frame % 6 < 3 { "( )" } else { "(-)" };
    if row == 0 {
        paint_text(&mut line, width.saturating_sub(6), orb);
        if width > 12 {
            paint_text(&mut line, 2, ".");
            paint_text(&mut line, width / 3, ".");
        }
    } else {
        paint_text(&mut line, width / 4, "__");
        paint_text(&mut line, width / 2 + 2, "__");
    }
    line.into_iter().collect()
}

fn ember_sky_row(width: usize, frame: usize, row: usize) -> String {
    let pattern = if row == 0 {
        ".  *   . *   ."
    } else {
        "  * .   *   . "
    };
    shifted_pattern(width, pattern, frame + row)
}

fn koi_sky_row(width: usize, frame: usize, row: usize) -> String {
    let mut line = vec![' '; width];
    if row == 0 {
        paint_text(
            &mut line,
            width.saturating_sub(6),
            if frame % 8 < 4 { "( )" } else { "(_)" },
        );
        if width > 14 {
            line[2] = '.';
            line[width / 3] = '.';
        }
    } else {
        let ripple_x = (width / 2 + frame % 5).min(width.saturating_sub(3));
        paint_text(&mut line, ripple_x, "..");
    }
    line.into_iter().collect()
}

fn winter_sky_row(width: usize, frame: usize, row: usize) -> String {
    let mut line = vec![' '; width];
    if row == 0 {
        paint_text(
            &mut line,
            width.saturating_sub(6),
            if frame % 6 < 3 { "(*)" } else { "( )" },
        );
        if width > 10 {
            line[2] = '.';
        }
    } else {
        let flakes = [1 + frame % 4, width / 3, width.saturating_sub(9)];
        for position in flakes {
            if position < width {
                line[position] = '.';
            }
        }
    }
    line.into_iter().collect()
}

fn harbor_pier_row(width: usize, tower_x: usize) -> String {
    let mut line = vec![' '; width];
    let pier_start = tower_x.saturating_sub(8);
    let pier_len = width.saturating_sub(pier_start).min(9);
    paint_text(&mut line, pier_start, &"=".repeat(pier_len));
    paint_text(&mut line, tower_x + 1, "|_|");
    line.into_iter().collect()
}

fn drift_x(
    width: usize,
    art_width: usize,
    frame: usize,
    pattern: &[usize],
    margin: usize,
    from_left: bool,
) -> usize {
    if width <= art_width {
        return 0;
    }

    let max_x = width - art_width;
    if from_left {
        let base_x = margin.min(max_x);
        return (base_x + pattern[frame % pattern.len()]).min(max_x);
    }

    let base_x = max_x.saturating_sub(margin);
    (base_x + pattern[frame % pattern.len()]).min(max_x)
}

fn scene_feature_row(width: usize, segments: &[(usize, &str)]) -> String {
    let mut line = vec![' '; width];
    for (offset, text) in segments {
        paint_text(&mut line, *offset, text);
    }
    line.into_iter().collect()
}

fn shifted_pattern(width: usize, pattern: &str, frame: usize) -> String {
    let pattern = pattern.chars().collect::<Vec<_>>();
    let offset = frame % pattern.len();

    (0..width)
        .map(|index| pattern[(index + offset) % pattern.len()])
        .collect()
}

fn scene_wave_patterns(variant: TideArtVariant) -> &'static [&'static str] {
    match variant {
        TideArtVariant::OceanCurrent => &["~~__^^__~~", "_~~^^~~__~", "~~^^__~~^^", "^__~~^^~~_"],
        TideArtVariant::HarborFog => &["~~..__..~~", ".~~__..~~.", "~~..~~__..", "__..~~..~~"],
        TideArtVariant::SakuraTide => &["~~__~~..~~", "_~~..~~__.", "~~..__~~..", "..~~__~~__"],
        TideArtVariant::MatchaGlass => &["__--__--__", "-__--__--_", "__--__--__", "--__--__--"],
        TideArtVariant::LanternEmber => &["~~==~~==~~", "=~~==~~===", "~~==__==~~", "==~~==~~=="],
        TideArtVariant::MoonlitKoi => &["~~__~~^^~~", "_~~^^~~__^", "~~^^__~~^^", "^__~~^^~~_"],
        TideArtVariant::WinterPlum => &["_-__-__-__", "__-_-__-_-", "-__-__-__-", "__--__--__"],
    }
}

fn positioned_line(width: usize, offset: usize, text: &str) -> String {
    let mut line = vec![' '; width];
    paint_text(&mut line, offset, text);
    line.into_iter().collect()
}

fn paint_text(line: &mut [char], offset: usize, text: &str) {
    for (index, character) in text.chars().enumerate() {
        if let Some(slot) = line.get_mut(offset + index) {
            *slot = character;
        }
    }
}

fn wave_row(width: usize, frame: usize, row: usize, patterns: &[&str]) -> String {
    let pattern = patterns[row % patterns.len()].chars().collect::<Vec<_>>();
    let offset = (frame * 2 + row * 3) % pattern.len();

    (0..width)
        .map(|index| pattern[(index + offset) % pattern.len()])
        .collect()
}

fn render_core_line(core: &CoreSnapshot, bar_width: usize, theme: Theme) -> Line<'static> {
    let meter = usage_bar(core.usage, bar_width);

    Line::from(vec![
        Span::styled(format!("{:<6}", core.label), Style::new().fg(theme.muted)),
        Span::styled(format!("{:>5.1}%", core.usage), Style::new().fg(theme.text)),
        Span::raw(" "),
        Span::styled(meter, Style::new().fg(theme.accent)),
    ])
}

fn render_disk_line(
    disk: &DiskSnapshot,
    label_width: usize,
    trend_width: usize,
    sample_interval_secs: f64,
    theme: Theme,
) -> Line<'static> {
    Line::from(vec![
        Span::styled(
            format!("{:<label_width$}", truncate_text(disk.label(), label_width)),
            Style::new().fg(theme.muted),
        ),
        Span::styled(
            format!("{:>5.1}%", disk.used_percent()),
            Style::new().fg(theme.text),
        ),
        Span::raw(" "),
        Span::styled(
            format!("{:>7}", format_rate(disk.read_bytes, sample_interval_secs)),
            Style::new().fg(theme.text),
        ),
        Span::raw(" "),
        Span::styled(
            format!(
                "{:>7}",
                format_rate(disk.written_bytes, sample_interval_secs)
            ),
            Style::new().fg(theme.text),
        ),
        Span::raw(" "),
        Span::styled(
            trend_sparkline(&disk.history, trend_width),
            Style::new().fg(theme.accent),
        ),
    ])
}

fn render_network_line(
    network: &NetworkSnapshot,
    label_width: usize,
    trend_width: usize,
    sample_interval_secs: f64,
    theme: Theme,
) -> Line<'static> {
    Line::from(vec![
        Span::styled(
            format!(
                "{:<label_width$}",
                truncate_text(&network.interface, label_width)
            ),
            Style::new().fg(theme.muted),
        ),
        Span::styled(
            format!("{:>7}", format_rate(network.received, sample_interval_secs)),
            Style::new().fg(theme.text),
        ),
        Span::raw(" "),
        Span::styled(
            format!(
                "{:>7}",
                format_rate(network.transmitted, sample_interval_secs)
            ),
            Style::new().fg(theme.text),
        ),
        Span::raw(" "),
        Span::styled(
            format!("{:>7}", format_compact_bytes(network.total_activity())),
            Style::new().fg(theme.text),
        ),
        Span::raw(" "),
        Span::styled(
            trend_sparkline(&network.history, trend_width),
            Style::new().fg(theme.accent),
        ),
    ])
}

fn render_process_line(
    process: &ProcessSnapshot,
    name_width: usize,
    sample_interval_secs: f64,
    selected: bool,
    theme: Theme,
) -> Line<'static> {
    let style = if selected {
        Style::new()
            .fg(theme.highlight)
            .bg(theme.accent_soft)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::new().fg(theme.text)
    };

    Line::from(vec![
        Span::styled(format!("{:<6}", process.pid), style),
        Span::styled(format!("{:>6.1}", process.cpu_usage), style),
        Span::raw(" "),
        Span::styled(
            format!("{:>7}", format_compact_bytes(process.memory)),
            style,
        ),
        Span::raw(" "),
        Span::styled(
            format!(
                "{:>7}",
                format_rate(process.io_activity(), sample_interval_secs)
            ),
            style,
        ),
        Span::raw(" "),
        Span::styled(
            format!("{:<name_width$}", truncate_text(&process.name, name_width)),
            style,
        ),
    ])
}

fn header_cell(label: &str, width: usize, theme: Theme) -> Span<'static> {
    Span::styled(
        format!("{label:<width$}"),
        Style::new()
            .fg(theme.highlight)
            .add_modifier(Modifier::BOLD),
    )
}

fn stat_line(label: &str, value: &str, suffix: &str, theme: Theme) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("{label:<6}"), Style::new().fg(theme.muted)),
        Span::styled(value.to_owned(), Style::new().fg(theme.text)),
        Span::styled(format!(" {suffix}"), Style::new().fg(theme.muted)),
    ])
}

fn panel_border_style(selected: bool, theme: Theme) -> Style {
    Style::new().fg(if selected {
        theme.highlight
    } else {
        theme.panel_border
    })
}

fn ratio(percent: f32) -> f64 {
    (percent.clamp(0.0, 100.0) as f64) / 100.0
}

fn format_duration(milliseconds: u64) -> String {
    if milliseconds < 1_000 {
        format!("{milliseconds}ms")
    } else {
        format!("{:.1}s", milliseconds as f64 / 1_000.0)
    }
}

fn format_runtime_secs(seconds: u64) -> String {
    let hours = seconds / 3_600;
    let minutes = (seconds % 3_600) / 60;
    let remaining = seconds % 60;

    if hours > 0 {
        format!("{hours}h {minutes:02}m {remaining:02}s")
    } else if minutes > 0 {
        format!("{minutes}m {remaining:02}s")
    } else {
        format!("{remaining}s")
    }
}

fn format_rate(bytes: u64, seconds: f64) -> String {
    format!(
        "{}/s",
        format_compact_bytes(rate_per_second(bytes, seconds))
    )
}

fn rate_per_second(bytes: u64, seconds: f64) -> u64 {
    if seconds <= 0.0 {
        bytes
    } else {
        (bytes as f64 / seconds).round() as u64
    }
}

fn usage_bar(percent: f32, width: usize) -> String {
    if width == 0 {
        return String::new();
    }

    let filled = ((percent.clamp(0.0, 100.0) / 100.0) * width as f32).round() as usize;
    let filled = filled.min(width);

    let mut bar = String::with_capacity(width);
    bar.push_str(&"#".repeat(filled));
    bar.push_str(&".".repeat(width - filled));
    bar
}

fn format_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];

    if bytes == 0 {
        return "0 B".to_string();
    }

    let mut value = bytes as f64;
    let mut index = 0usize;

    while value >= 1024.0 && index < UNITS.len() - 1 {
        value /= 1024.0;
        index += 1;
    }

    if index == 0 {
        format!("{value:.0} {}", UNITS[index])
    } else {
        format!("{value:.1} {}", UNITS[index])
    }
}

fn format_compact_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "K", "M", "G", "T"];

    if bytes == 0 {
        return "0B".to_string();
    }

    let mut value = bytes as f64;
    let mut index = 0usize;

    while value >= 1024.0 && index < UNITS.len() - 1 {
        value /= 1024.0;
        index += 1;
    }

    if value >= 10.0 || index == 0 {
        format!("{value:.0}{}", UNITS[index])
    } else {
        format!("{value:.1}{}", UNITS[index])
    }
}

fn truncate_text(text: &str, width: usize) -> String {
    if width == 0 {
        return String::new();
    }

    let text_width = text.chars().count();
    if text_width <= width {
        return text.to_string();
    }

    if width <= 3 {
        return ".".repeat(width);
    }

    let mut truncated = text.chars().take(width - 3).collect::<String>();
    truncated.push_str("...");
    truncated
}

fn trend_sparkline(values: &[u64], width: usize) -> String {
    const BARS: [char; 8] = [' ', '.', ':', '-', '=', '+', '*', '#'];

    if width == 0 {
        return String::new();
    }

    let slice = if values.len() > width {
        &values[values.len() - width..]
    } else {
        values
    };

    let max = slice.iter().copied().max().unwrap_or(0);
    let padding = width.saturating_sub(slice.len());
    let mut sparkline = ".".repeat(padding);

    for value in slice {
        let index = if max == 0 {
            0
        } else {
            ((*value as f64 / max as f64) * (BARS.len() as f64 - 1.0)).round() as usize
        };
        sparkline.push(BARS[index.min(BARS.len() - 1)]);
    }

    sparkline
}

fn centered_rect(area: Rect, width_percent: u16, height_percent: u16) -> Rect {
    let [_, vertical, _] = Layout::vertical([
        Constraint::Percentage((100 - height_percent) / 2),
        Constraint::Percentage(height_percent),
        Constraint::Percentage((100 - height_percent) / 2),
    ])
    .areas(area);
    let [_, horizontal, _] = Layout::horizontal([
        Constraint::Percentage((100 - width_percent) / 2),
        Constraint::Percentage(width_percent),
        Constraint::Percentage((100 - width_percent) / 2),
    ])
    .areas(vertical);
    horizontal
}

#[cfg(test)]
mod tests {
    use super::{
        format_bytes, format_compact_bytes, rate_per_second, tide_art_rows, trend_sparkline,
        usage_bar,
    };
    use crate::theme::TideArtVariant;

    #[test]
    fn usage_bar_scales_to_requested_width() {
        assert_eq!(usage_bar(50.0, 10), "#####.....");
    }

    #[test]
    fn bytes_are_formatted_human_readably() {
        assert_eq!(format_bytes(1_073_741_824), "1.0 GiB");
    }

    #[test]
    fn compact_bytes_use_short_units() {
        assert_eq!(format_compact_bytes(1_572_864), "1.5M");
    }

    #[test]
    fn tide_art_fills_the_requested_canvas() {
        let rows = tide_art_rows(28, 7, 2, TideArtVariant::OceanCurrent);

        assert_eq!(rows.len(), 7);
        assert!(rows.iter().all(|(_, row)| row.chars().count() == 28));
    }

    #[test]
    fn tide_art_changes_between_frames() {
        let first = tide_art_rows(28, 7, 0, TideArtVariant::OceanCurrent)
            .into_iter()
            .map(|(_, row)| row)
            .collect::<Vec<_>>();
        let second = tide_art_rows(28, 7, 1, TideArtVariant::OceanCurrent)
            .into_iter()
            .map(|(_, row)| row)
            .collect::<Vec<_>>();

        assert_ne!(first, second);
    }

    #[test]
    fn tide_art_varies_by_theme() {
        let ocean = tide_art_rows(28, 7, 0, TideArtVariant::OceanCurrent)
            .into_iter()
            .map(|(_, row)| row)
            .collect::<Vec<_>>();
        let sakura = tide_art_rows(28, 7, 0, TideArtVariant::SakuraTide)
            .into_iter()
            .map(|(_, row)| row)
            .collect::<Vec<_>>();

        assert_ne!(ocean, sakura);
    }

    #[test]
    fn rates_are_normalized_by_sample_window() {
        assert_eq!(rate_per_second(2_048, 2.0), 1_024);
    }

    #[test]
    fn trend_sparkline_respects_requested_width() {
        assert_eq!(trend_sparkline(&[1, 2, 3, 4], 3).chars().count(), 3);
    }
}
