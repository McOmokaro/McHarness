//! Rendering: the four Bento panes and the command bar.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

use crate::telemetry::Telemetry;
use crate::theme;
use crate::{App, bento_layout};

/// A titled charcoal panel with a subtle border.
fn panel<'a>(title: &'a str, accent: Color) -> Block<'a> {
    Block::default()
        .title(Span::styled(
            format!(" {title} "),
            Style::default().fg(accent).add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme::BORDER))
        .style(Style::default().bg(theme::PANEL))
}

fn kv<'a>(key: &'a str, value: String, value_color: Color) -> Line<'a> {
    Line::from(vec![
        Span::styled(format!("{key:<16}"), Style::default().fg(theme::MUTED)),
        Span::styled(value, Style::default().fg(value_color)),
    ])
}

/// Draw the whole command center.
pub fn render(f: &mut Frame, app: &App) {
    let area = f.area();
    f.render_widget(
        Block::default().style(Style::default().bg(theme::BG)),
        area,
    );

    let l = bento_layout(area);
    render_brain(f, l.brain, &app.telemetry, app);
    render_keyfarm(f, l.keyfarm, &app.telemetry);
    render_arsenal(f, l.arsenal, &app.telemetry);
    render_overnight(f, l.overnight, &app.telemetry);
    render_command(f, l.command, app);
}

fn render_brain(f: &mut Frame, area: Rect, t: &Telemetry, app: &App) {
    let b = &t.brain;
    let spinning = b.replay_status == "Consolidating";
    let status_color = if spinning { theme::ORANGE } else { theme::MUTED };
    let status = if spinning {
        format!("{} {}", app.spinner(), b.replay_line())
    } else {
        b.replay_line()
    };
    let lines = vec![
        kv("Episodic", format!("{} memories", b.episodic), theme::NEON),
        kv("Semantic", format!("{} rules", b.semantic), theme::NEON),
        kv("Batch size", format!("{}", b.batch_size), theme::TEXT),
        Line::from(""),
        Line::from(Span::styled(status, Style::default().fg(status_color))),
    ];
    f.render_widget(
        Paragraph::new(lines).block(panel("THE BRAIN", theme::NEON)).wrap(Wrap { trim: true }),
        area,
    );
}

fn render_keyfarm(f: &mut Frame, area: Rect, t: &Telemetry) {
    let mut lines: Vec<Line> = Vec::new();
    for e in &t.keyfarm.entries {
        lines.push(Line::from(vec![
            Span::styled(format!("{:<22}", e.model), Style::default().fg(theme::TEXT)),
            Span::styled(format!("{:<22}", e.key_masked), Style::default().fg(theme::MUTED)),
            Span::styled(
                format!("● {}", e.health.label()),
                Style::default().fg(e.health.color()),
            ),
        ]));
    }
    if lines.is_empty() {
        lines.push(Line::from(Span::styled(
            "no keys configured",
            Style::default().fg(theme::MUTED),
        )));
    }
    let worst = t.keyfarm.worst_health();
    let block = panel("KEY FARM", theme::ORANGE).title_bottom(
        Line::from(Span::styled(
            format!(" worst: {} ", worst.label()),
            Style::default().fg(worst.color()),
        ))
        .right_aligned(),
    );
    f.render_widget(
        Paragraph::new(lines).block(block).wrap(Wrap { trim: true }),
        area,
    );
}

fn render_arsenal(f: &mut Frame, area: Rect, t: &Telemetry) {
    let a = &t.arsenal;
    let targets = if a.targets.is_empty() {
        "—".to_string()
    } else {
        a.targets.join(", ")
    };
    let running = if a.running.is_empty() {
        "idle".to_string()
    } else {
        a.running.join(", ")
    };
    let lines = vec![
        kv("Targets", targets, theme::ORANGE),
        kv("Running", format!("{} {}", if a.running.is_empty() { "" } else { "▶" }, running), theme::NEON),
        kv("Swarm", a.swarm_status.clone(), theme::TEXT),
        kv(
            "Toolbox",
            format!("{}/{} tools ready", a.installed_tools, a.total_tools),
            theme::TEXT,
        ),
    ];
    f.render_widget(
        Paragraph::new(lines).block(panel("ARSENAL", theme::ORANGE)).wrap(Wrap { trim: true }),
        area,
    );
}

fn render_overnight(f: &mut Frame, area: Rect, t: &Telemetry) {
    let o = &t.overnight;
    let lines = vec![
        kv(
            "Last run",
            o.last_run.clone().unwrap_or_else(|| "never".into()),
            theme::TEXT,
        ),
        kv(
            "Last report",
            o.last_report.clone().unwrap_or_else(|| "—".into()),
            theme::TEXT,
        ),
        kv("Fuel today", format!("{} tokens", o.fuel_today), theme::NEON),
    ];
    f.render_widget(
        Paragraph::new(lines)
            .block(panel("OVERNIGHT AGENT", theme::NEON))
            .wrap(Wrap { trim: true }),
        area,
    );
}

fn render_command(f: &mut Frame, area: Rect, app: &App) {
    let lines = vec![
        Line::from(vec![
            Span::styled("mch ▸ ", Style::default().fg(theme::NEON).add_modifier(Modifier::BOLD)),
            Span::styled(app.input.clone(), Style::default().fg(theme::TEXT)),
            Span::styled("█", Style::default().fg(theme::ORANGE)),
        ]),
        Line::from(Span::styled(
            crate::commands::help_line(),
            Style::default().fg(theme::MUTED),
        )),
    ];
    f.render_widget(
        Paragraph::new(lines).block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(theme::BORDER_ACTIVE))
                .style(Style::default().bg(theme::BG)),
        ),
        area,
    );
}
