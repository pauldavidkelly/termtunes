mod app;
mod config;
mod tui;

use color_eyre::Result;
use tracing_subscriber::EnvFilter;

use app::App;

fn main() -> Result<()> {
    // 1. Install color-eyre for enhanced error reporting and panic hooks
    color_eyre::install()?;

    // 2. Initialize tracing subscriber -- write logs to file so they don't
    //    interfere with the TUI display. Uses RUST_LOG env var for filtering.
    let log_dir = dirs::data_dir()
        .unwrap_or_else(|| {
            std::path::PathBuf::from(format!(
                "{}/.local/share",
                std::env::var("HOME").unwrap_or_default()
            ))
        })
        .join("termtunes");
    std::fs::create_dir_all(&log_dir)?;

    let log_file = std::fs::File::create(log_dir.join("termtunes.log"))?;
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_writer(log_file)
        .with_ansi(false)
        .init();

    tracing::info!("TermTunes starting up");

    // 3. Install panic hook BEFORE terminal init so panics restore terminal
    tui::install_panic_hook();

    // 4. Register signal handlers (SIGINT, SIGTERM, SIGHUP)
    let shutdown = tui::install_signal_handlers();

    // 5. Load config (creates new with UUID on first run)
    let config = config::load_config()?;
    tracing::info!(client_id = %config.client_id, "Config loaded");

    // 6. Save config (ensures file exists on first run)
    config::save_config(&config)?;

    // 7. Initialize the terminal (enters alternate screen, enables raw mode)
    let mut terminal = ratatui::init();

    // 8. Create and run the app
    let mut app = App::new(config, shutdown);
    let result = app.run(&mut terminal);

    // 9. Restore terminal state (leave alternate screen, disable raw mode)
    ratatui::restore();

    // 10. Propagate any error from the app run
    result
}
