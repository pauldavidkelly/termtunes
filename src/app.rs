use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use color_eyre::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::style::{Color, Style};
use ratatui::widgets::Paragraph;
use ratatui::DefaultTerminal;

use crate::config::Config;

/// Main application state.
///
/// Holds the loaded config, a running flag for the event loop,
/// and a reference to the shutdown flag set by signal handlers.
pub struct App {
    /// Application configuration (loaded from config.toml).
    pub config: Config,

    /// Whether the event loop should continue running.
    pub running: bool,

    /// Shared flag set to true by signal handlers (SIGINT, SIGTERM, SIGHUP).
    pub shutdown: Arc<AtomicBool>,
}

impl App {
    /// Create a new App instance.
    pub fn new(config: Config, shutdown: Arc<AtomicBool>) -> Self {
        Self {
            config,
            running: true,
            shutdown,
        }
    }

    /// Run the main event loop.
    ///
    /// On each iteration:
    /// 1. Check if a shutdown signal was received
    /// 2. Draw the placeholder UI
    /// 3. Poll for keyboard events (100ms timeout)
    /// 4. Handle 'q' key to exit
    pub fn run(&mut self, terminal: &mut DefaultTerminal) -> Result<()> {
        while self.running {
            // Check for signal-based shutdown (SIGINT, SIGTERM, SIGHUP)
            if self.shutdown.load(Ordering::Relaxed) {
                self.running = false;
                break;
            }

            // Draw the UI
            terminal.draw(|frame| {
                let area = frame.area();
                let text = Paragraph::new("TermTunes - press q to quit")
                    .style(Style::default().fg(Color::Cyan))
                    .centered();
                frame.render_widget(text, area);
            })?;

            // Poll for keyboard events with 100ms timeout.
            // This keeps the loop responsive to signals while not
            // burning CPU with busy-waiting.
            if event::poll(Duration::from_millis(100))? {
                if let Event::Key(key) = event::read()? {
                    // Only handle key press events (not release/repeat)
                    if key.kind == KeyEventKind::Press {
                        match (key.code, key.modifiers) {
                            // 'q' to quit
                            (KeyCode::Char('q'), _) => self.running = false,
                            // Ctrl+C to quit -- in raw mode the terminal driver
                            // does NOT send SIGINT, so we must handle it here
                            // as a key event instead.
                            (KeyCode::Char('c'), m) if m.contains(KeyModifiers::CONTROL) => {
                                self.running = false;
                            }
                            _ => {}
                        }
                    }
                }
            }
        }

        Ok(())
    }
}
