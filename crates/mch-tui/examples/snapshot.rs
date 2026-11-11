//! Render one frame to a headless backend and print it as text.
//! Run: `cargo run -p mch-tui --example snapshot`

use mch_tui::{App, KeyFarm, Telemetry, render};
use mch_tui::telemetry::{Arsenal, BrainTelemetry, OvernightTelemetry};
use ratatui::Terminal;
use ratatui::backend::TestBackend;

fn main() {
    let telemetry = Telemetry {
        brain: BrainTelemetry {
            episodic: 1_284,
            semantic: 57,
            replay_status: "Sleeping".into(),
            next_consolidation_min: 24,
            batch_size: 10,
        },
        keyfarm: KeyFarm::demo(),
        arsenal: Arsenal {
            targets: vec!["10.0.0.5".into(), "app.example.com".into()],
            running: vec!["nmap".into(), "ffuf".into()],
            swarm_status: "Hunting (3 agents)".into(),
            installed_tools: 6,
            total_tools: 12,
        },
        overnight: OvernightTelemetry {
            last_run: Some("2026-11-06T03:00Z".into()),
            last_report: Some("sent ✓ (Telegram)".into()),
            fuel_today: 41_902,
        },
    };
    let mut app = App::new(telemetry);
    app.input = "/hunt app.example.com".into();

    let backend = TestBackend::new(118, 36);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| render(f, &app)).unwrap();

    let buf = terminal.backend().buffer().clone();
    let mut out = String::new();
    for y in 0..buf.area.height {
        let mut row = String::new();
        for x in 0..buf.area.width {
            row.push_str(buf[(x, y)].symbol());
        }
        out.push_str(row.trim_end());
        out.push('\n');
    }
    println!("{out}");
}
