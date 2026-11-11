//! mch-tui binary — the 60 FPS terminal event loop.

use std::io;
use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;

use mch_kali::lazy::{LazyLoadConfig, LazyLoader};
use mch_tui::{App, Telemetry, render};

/// ~60 FPS: poll for input for at most this long per frame.
const FRAME_MS: u64 = 1000 / 60;
/// Refresh live telemetry every N frames (~2s at 60 FPS).
const REFRESH_EVERY: u64 = 120;

fn main() -> anyhow::Result<()> {
    let memory_path = std::env::var("MCH_BRAIN_DB").unwrap_or_else(|_| {
        std::env::temp_dir()
            .join("mch-overnight-brain.sqlite")
            .to_string_lossy()
            .to_string()
    });
    let loader = LazyLoader::new(LazyLoadConfig::default());

    let rt = tokio::runtime::Runtime::new()?;
    let telemetry = rt.block_on(Telemetry::sample(&memory_path, &loader));
    let mut app = App::new(telemetry);

    // Enter the alternate screen + raw mode.
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    let result = run_loop(&mut terminal, &mut app, &rt, &memory_path, &loader);

    // Always restore the terminal, even on error.
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    result
}

fn run_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
    rt: &tokio::runtime::Runtime,
    memory_path: &str,
    loader: &LazyLoader,
) -> anyhow::Result<()> {
    loop {
        app.tick();

        // Periodically refresh telemetry from the live sources.
        if app.tick % REFRESH_EVERY == 0 {
            app.telemetry = rt.block_on(Telemetry::sample(memory_path, loader));
        }

        terminal.draw(|f| render(f, app))?;

        if event::poll(Duration::from_millis(FRAME_MS))? {
            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press {
                    continue;
                }
                match key.code {
                    KeyCode::Char('c')
                        if key.modifiers.contains(KeyModifiers::CONTROL) =>
                    {
                        app.should_quit = true;
                    }
                    KeyCode::Char(c) => app.on_char(c),
                    KeyCode::Backspace => app.on_backspace(),
                    KeyCode::Enter => {
                        app.submit();
                    }
                    KeyCode::Esc => app.should_quit = true,
                    _ => {}
                }
            }
        }

        if app.should_quit {
            break;
        }
    }
    Ok(())
}
