mod app;
mod auth;
mod config;
mod player;
mod plex;
mod tui;
mod ui;

use color_eyre::Result;
use tracing_subscriber::EnvFilter;

use app::App;

/// Entry point. Uses #[tokio::main] to provide an async runtime for Plex
/// API calls (authentication, server discovery, playlist/track fetching).
/// The TUI event loop itself is synchronous (crossterm polling).
#[tokio::main]
async fn main() -> Result<()> {
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

    // 3. Set PULSE_LATENCY_MSEC to improve WSL2 audio latency.
    //    Must be set BEFORE creating any OutputStream/audio device.
    //    See research: WSLg PulseAudio bridge benefits from increased buffer.
    std::env::set_var("PULSE_LATENCY_MSEC", "60");

    // 4. Install panic hook BEFORE terminal init so panics restore terminal
    tui::install_panic_hook();

    // 5. Register signal handlers (SIGINT, SIGTERM, SIGHUP)
    let shutdown = tui::install_signal_handlers();

    // 6. Load config (creates new with UUID on first run)
    let mut config = config::load_config()?;
    tracing::info!(client_id = %config.client_id, "Config loaded");

    // 7. Save config (ensures file exists on first run)
    config::save_config(&config)?;

    // 8. Authenticate with Plex (before entering TUI so the auth URL
    //    is displayed on the normal terminal, not the alternate screen)
    let (plex_client, server_name) =
        app::authenticate(&mut config).await?;

    tracing::info!(server = %server_name, "Authenticated with Plex server");

    // 9. Fetch initial playlists
    let playlists = plex_client.fetch_playlists().await?;
    tracing::info!(count = playlists.len(), "Fetched playlists");

    // 10. Initialize the terminal (enters alternate screen, enables raw mode)
    let mut terminal = ratatui::init();

    // 11. Create and run the app with authenticated Plex client
    let mut app = App::new(config, shutdown, plex_client, server_name, playlists);
    let result = app.run(&mut terminal).await;

    // 12. Restore terminal state (leave alternate screen, disable raw mode)
    ratatui::restore();

    // 13. Propagate any error from the app run
    result
}
