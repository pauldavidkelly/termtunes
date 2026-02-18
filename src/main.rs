mod app;
mod auth;
mod config;
mod player;
mod plex;
mod tui;
mod ui;
mod visualizer;

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
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("info"))
        )
        .with_writer(log_file)
        .with_ansi(false)
        .init();

    tracing::info!("TermTunes starting up");

    // 3. Set PULSE_LATENCY_MSEC to improve WSL2 audio quality.
    //    Must be set BEFORE creating any OutputStream/audio device.
    //    The WSLg PulseAudio bridge introduces scheduling jitter that
    //    causes buffer underruns (crackling/clicking) at low latencies.
    //    500ms provides enough buffer headroom to absorb this jitter while
    //    remaining imperceptible for music playback. Lower values cause
    //    audible artifacts on WSL2: 60ms caused crackling, 150ms caused
    //    stuttering after ~20 seconds, 300ms was insufficient for sustained
    //    playback beyond ~1 minute.
    //    Safety: called at startup before any threads are spawned.
    unsafe { std::env::set_var("PULSE_LATENCY_MSEC", "500") };

    // 3b. Check WSL2 audio dependencies early and warn the user while we
    //     are still on the normal terminal (not alternate screen).
    check_wsl2_audio_deps();

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

    // 11b. Restore session state from previous run (best-effort).
    // Positions user at saved playlist/track without auto-playing.
    app.restore_session().await;

    let result = app.run(&mut terminal).await;

    // 12. Restore terminal state (leave alternate screen, disable raw mode)
    ratatui::restore();

    // 13. Propagate any error from the app run
    result
}

/// Check WSL2 audio dependencies at startup and print warnings.
///
/// On WSL2, audio requires: (1) the ALSA PulseAudio plugin (`libasound2-plugins`)
/// to bridge ALSA -> PulseAudio, and (2) the WSLg PulseAudio socket. This check
/// runs before the TUI starts so messages appear on the normal terminal.
fn check_wsl2_audio_deps() {
    // Only relevant on WSL2
    let is_wsl = std::fs::read_to_string("/proc/version")
        .map(|v| v.contains("microsoft") || v.contains("WSL"))
        .unwrap_or(false);

    if !is_wsl {
        return;
    }

    tracing::info!("WSL2 detected, checking audio dependencies");

    // Check for the ALSA PulseAudio plugin library
    let plugin_paths = [
        "/usr/lib/x86_64-linux-gnu/alsa-lib/libasound_module_pcm_pulse.so",
        "/usr/lib/alsa-lib/libasound_module_pcm_pulse.so",
        "/usr/lib/aarch64-linux-gnu/alsa-lib/libasound_module_pcm_pulse.so",
    ];
    let has_plugin = plugin_paths.iter().any(|p| std::path::Path::new(p).exists());

    if !has_plugin {
        eprintln!();
        eprintln!("  WARNING: WSL2 audio requires the ALSA PulseAudio plugin.");
        eprintln!("  Audio playback will not work without it.");
        eprintln!();
        eprintln!("  Install with:  sudo apt-get install -y libasound2-plugins");
        eprintln!();
        tracing::warn!("libasound2-plugins not found -- audio will fail on WSL2");
    }

    // Check for the WSLg PulseAudio socket
    let has_socket = std::path::Path::new("/mnt/wslg/PulseServer").exists();
    if !has_socket {
        eprintln!();
        eprintln!("  WARNING: WSLg PulseAudio socket not found.");
        eprintln!("  Try restarting WSL:  wsl --shutdown");
        eprintln!();
        tracing::warn!("WSLg PulseAudio socket not found at /mnt/wslg/PulseServer");
    }

    // Check PULSE_SERVER env var
    let pulse_server = std::env::var("PULSE_SERVER").unwrap_or_default();
    if pulse_server.is_empty() {
        tracing::warn!("PULSE_SERVER environment variable not set");
    } else {
        tracing::info!(pulse_server = %pulse_server, "PulseAudio server configured");
    }
}
