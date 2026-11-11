//! mch-tui — the McHarness Command Center.
//!
//! A dark-mode Bento Grid terminal UI: four live panes (Brain, Key Farm,
//! Arsenal, Overnight Agent) over a command bar, rendered at up to 60 FPS.
//!
//! The crate is split so the *logic* is unit-testable without a TTY: layout
//! geometry, telemetry adapters, the command parser, and the app state machine
//! live here; only the raw-mode event loop lives in the binary.

pub mod commands;
pub mod layout;
pub mod telemetry;
pub mod theme;
pub mod ui;

pub use commands::{Command, parse};
pub use layout::{BentoLayout, bento_layout};
pub use telemetry::Telemetry;
pub use telemetry::{Arsenal, BrainTelemetry, KeyEntry, KeyFarm, OvernightTelemetry};
pub use theme::Health;
pub use ui::render;

/// Braille spinner frames for smooth loading states.
pub const SPINNER: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

/// The whole application state.
#[derive(Debug, Clone)]
pub struct App {
    pub telemetry: Telemetry,
    /// Current command-bar text.
    pub input: String,
    /// Rolling activity log (newest last).
    pub log: Vec<String>,
    /// Monotonic frame counter driving animations.
    pub tick: u64,
    /// Set by `/quit`.
    pub should_quit: bool,
}

impl App {
    pub fn new(telemetry: Telemetry) -> Self {
        Self {
            telemetry,
            input: String::new(),
            log: vec!["McHarness command center online.".into()],
            tick: 0,
            should_quit: false,
        }
    }

    /// Advance one frame.
    pub fn tick(&mut self) {
        self.tick = self.tick.wrapping_add(1);
    }

    /// The current spinner glyph.
    pub fn spinner(&self) -> &'static str {
        SPINNER[(self.tick as usize) % SPINNER.len()]
    }

    /// Append a character to the command bar.
    pub fn on_char(&mut self, c: char) {
        self.input.push(c);
    }

    /// Delete the last character of the command bar.
    pub fn on_backspace(&mut self) {
        self.input.pop();
    }

    /// Submit the command bar; returns the parsed command and logs the effect.
    pub fn submit(&mut self) -> Command {
        let cmd = parse(&self.input);
        match &cmd {
            Command::Hunt(t) => self.push_log(format!("hunt → {t} (dispatching swarm)")),
            Command::Trade(a) => self.push_log(format!("trade → {a}")),
            Command::Brain(a) => self.push_log(format!("brain → {a}")),
            Command::Overnight => self.push_log("overnight run requested".into()),
            Command::Help => self.push_log(commands::help_line().into()),
            Command::Quit => {
                self.should_quit = true;
                self.push_log("shutting down…".into());
            }
            Command::Unknown(u) => self.push_log(format!("unknown command: {u}")),
            Command::Empty => {}
        }
        self.input.clear();
        cmd
    }

    fn push_log(&mut self, line: String) {
        self.log.push(line);
        if self.log.len() > 200 {
            self.log.remove(0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    #[test]
    fn spinner_cycles() {
        let mut app = App::new(Telemetry::default());
        assert_eq!(app.spinner(), SPINNER[0]);
        app.tick();
        assert_eq!(app.spinner(), SPINNER[1]);
    }

    #[test]
    fn typing_and_submit_logs() {
        let mut app = App::new(Telemetry::default());
        for c in "/hunt 10.0.0.5".chars() {
            app.on_char(c);
        }
        assert_eq!(app.input, "/hunt 10.0.0.5");
        let cmd = app.submit();
        assert_eq!(cmd, Command::Hunt("10.0.0.5".into()));
        assert!(app.input.is_empty());
        assert!(app.log.last().unwrap().contains("10.0.0.5"));
    }

    #[test]
    fn quit_sets_flag() {
        let mut app = App::new(Telemetry::default());
        app.input = "/quit".into();
        app.submit();
        assert!(app.should_quit);
    }

    #[test]
    fn renders_without_panicking() {
        let backend = TestBackend::new(120, 40);
        let mut terminal = Terminal::new(backend).unwrap();
        let app = App::new(Telemetry {
            keyfarm: KeyFarm::demo(),
            ..Default::default()
        });
        terminal.draw(|f| render(f, &app)).unwrap();
        // The buffer should contain one of our pane titles.
        let buf = terminal.backend().buffer().clone();
        let text: String = buf.content.iter().map(|c| c.symbol()).collect();
        assert!(text.contains("THE BRAIN"));
        assert!(text.contains("KEY FARM"));
        assert!(text.contains("ARSENAL"));
        assert!(text.contains("OVERNIGHT"));
    }
}
