use std::io::{self, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use color_eyre::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::widgets::ListState;
use ratatui::DefaultTerminal;

use crate::auth;
use crate::config::{self, Config, ServerConfig};
use crate::player::Player;
use crate::plex::{self, Playlist, PlexClient, PlexServer, Track};
use crate::ui;

// ---------------------------------------------------------------------------
// App state machine
// ---------------------------------------------------------------------------

/// The current view / mode of the application.
#[derive(Debug, PartialEq)]
pub enum AppView {
    /// Displaying the list of audio playlists.
    Playlists,
    /// Displaying the tracks within a selected playlist.
    Tracks,
    /// A track is being downloaded (shown while fetching audio bytes).
    Downloading,
    /// Tracks view with active playback (user can select another track).
    Playing,
}

// ---------------------------------------------------------------------------
// App struct
// ---------------------------------------------------------------------------

/// Main application state.
///
/// Holds the authenticated Plex client, loaded playlists and tracks,
/// navigation state for list widgets, the audio player, and the UI state
/// machine.
pub struct App {
    /// Application configuration (loaded from config.toml).
    config: Config,

    /// Whether the event loop should continue running.
    running: bool,

    /// Shared flag set to true by signal handlers (SIGINT, SIGTERM, SIGHUP).
    shutdown: Arc<AtomicBool>,

    /// Authenticated Plex API client for the selected server.
    plex_client: PlexClient,

    /// Name of the connected Plex server (for status bar display).
    server_name: String,

    /// Audio playlists fetched from the server.
    playlists: Vec<Playlist>,

    /// Tracks in the currently selected playlist.
    tracks: Vec<Track>,

    /// Current view (Playlists, Tracks, Downloading, or Playing).
    view: AppView,

    /// Selection state for the playlist list.
    playlist_state: ListState,

    /// Selection state for the track list.
    track_state: ListState,

    /// Title of the currently selected playlist (for status bar).
    current_playlist_title: String,

    /// Audio player (initialized lazily when first track is played).
    player: Option<Player>,

    /// Channel receiver for completed track downloads.
    /// The background download thread sends (audio_bytes, track_name) when done.
    download_rx: Option<std::sync::mpsc::Receiver<Result<(Vec<u8>, String)>>>,

    /// Error message to display in the status bar. Set when audio device
    /// initialization fails or download errors occur. Cleared on next
    /// successful action.
    error_message: Option<String>,
}

impl App {
    /// Create a new App instance with an authenticated Plex connection.
    pub fn new(
        config: Config,
        shutdown: Arc<AtomicBool>,
        plex_client: PlexClient,
        server_name: String,
        playlists: Vec<Playlist>,
    ) -> Self {
        let mut playlist_state = ListState::default();
        if !playlists.is_empty() {
            playlist_state.select(Some(0));
        }

        Self {
            config,
            running: true,
            shutdown,
            plex_client,
            server_name,
            playlists,
            tracks: Vec::new(),
            view: AppView::Playlists,
            playlist_state,
            track_state: ListState::default(),
            current_playlist_title: String::new(),
            player: None,
            download_rx: None,
            error_message: None,
        }
    }

    // -----------------------------------------------------------------------
    // Public accessors for ui.rs
    // -----------------------------------------------------------------------

    /// Get the current view.
    pub fn view(&self) -> &AppView {
        &self.view
    }

    /// Get a reference to the playlists.
    pub fn playlists(&self) -> &[Playlist] {
        &self.playlists
    }

    /// Get a reference to the tracks.
    pub fn tracks(&self) -> &[Track] {
        &self.tracks
    }

    /// Get the current playlist title.
    pub fn current_playlist_title(&self) -> &str {
        &self.current_playlist_title
    }

    /// Get a mutable reference to the playlist ListState (for rendering).
    pub fn playlist_state_mut(&mut self) -> &mut ListState {
        &mut self.playlist_state
    }

    /// Get a mutable reference to the track ListState (for rendering).
    pub fn track_state_mut(&mut self) -> &mut ListState {
        &mut self.track_state
    }

    /// Get a reference to the player (if initialized).
    pub fn player(&self) -> Option<&Player> {
        self.player.as_ref()
    }

    /// Get the current error message (if any), for display in the status bar.
    pub fn error_message(&self) -> Option<&str> {
        self.error_message.as_deref()
    }

    // -----------------------------------------------------------------------
    // Event loop
    // -----------------------------------------------------------------------

    /// Run the main event loop.
    ///
    /// Draws the UI and handles keyboard input. Async because selecting
    /// a playlist triggers an API call to fetch tracks.
    pub async fn run(&mut self, terminal: &mut DefaultTerminal) -> Result<()> {
        while self.running {
            // Check for signal-based shutdown (SIGINT, SIGTERM, SIGHUP)
            if self.shutdown.load(Ordering::Relaxed) {
                self.running = false;
                break;
            }

            // Check for completed downloads (non-blocking)
            self.check_download_complete()?;

            // Draw the UI
            terminal.draw(|frame| {
                ui::render(frame, self);
            })?;

            // Poll for keyboard events with 100ms timeout.
            if event::poll(Duration::from_millis(100))? {
                if let Event::Key(key) = event::read()? {
                    if key.kind == KeyEventKind::Press {
                        self.handle_key(key.code, key.modifiers).await?;
                    }
                }
            }
        }

        Ok(())
    }

    /// Check if a background download has completed.
    ///
    /// Uses try_recv on the mpsc channel so it never blocks the event loop.
    /// When download completes, initializes the Player (if needed) and starts
    /// playback.
    fn check_download_complete(&mut self) -> Result<()> {
        if let Some(rx) = &self.download_rx {
            match rx.try_recv() {
                Ok(Ok((audio_bytes, track_name))) => {
                    tracing::info!(
                        track = %track_name,
                        size = audio_bytes.len(),
                        "Download complete, starting playback"
                    );

                    // Initialize player if this is the first track
                    if self.player.is_none() {
                        match Player::new() {
                            Ok(p) => {
                                self.player = Some(p);
                                // Clear any previous audio error
                                self.error_message = None;
                            }
                            Err(e) => {
                                tracing::error!("Failed to create audio player: {}", e);
                                // Show the error in the status bar instead of crashing.
                                // Extract the most useful part of the error message.
                                self.error_message = Some(format!("{}", e));
                                self.view = AppView::Tracks;
                                self.download_rx = None;
                                return Ok(());
                            }
                        }
                    }

                    // Start playback
                    if let Some(player) = &mut self.player {
                        match player.load_and_play(audio_bytes, track_name) {
                            Ok(()) => {
                                self.view = AppView::Playing;
                                self.error_message = None;
                            }
                            Err(e) => {
                                tracing::error!("Failed to start playback: {}", e);
                                self.error_message = Some(format!("Playback error: {}", e));
                                self.view = AppView::Tracks;
                            }
                        }
                    }

                    self.download_rx = None;
                }
                Ok(Err(e)) => {
                    tracing::error!("Download failed: {}", e);
                    self.view = AppView::Tracks;
                    self.download_rx = None;
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {
                    // Download still in progress -- keep waiting
                }
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    // Sender dropped without sending -- thread panicked?
                    tracing::error!("Download thread disconnected unexpectedly");
                    self.view = AppView::Tracks;
                    self.download_rx = None;
                }
            }
        }
        Ok(())
    }

    /// Handle a key press event.
    async fn handle_key(&mut self, code: KeyCode, modifiers: KeyModifiers) -> Result<()> {
        match (code, modifiers) {
            // Quit
            (KeyCode::Char('q'), _) => self.running = false,
            // Ctrl+C to quit (raw mode intercepts SIGINT)
            (KeyCode::Char('c'), m) if m.contains(KeyModifiers::CONTROL) => {
                self.running = false;
            }
            // Spacebar: toggle play/pause (any view, if player active)
            (KeyCode::Char(' '), _) => {
                if let Some(player) = &self.player {
                    player.toggle_pause();
                }
            }
            // Navigate down
            (KeyCode::Char('j'), _) | (KeyCode::Down, _) => self.move_selection_down(),
            // Navigate up
            (KeyCode::Char('k'), _) | (KeyCode::Up, _) => self.move_selection_up(),
            // Select / Enter
            (KeyCode::Enter, _) => self.select_item().await?,
            // Back (from Tracks/Playing to Playlists)
            (KeyCode::Esc, _) | (KeyCode::Backspace, _) => self.go_back(),
            _ => {}
        }
        Ok(())
    }

    /// Move the selection cursor down in the current list.
    fn move_selection_down(&mut self) {
        let (state, len) = match self.view {
            AppView::Playlists => (&mut self.playlist_state, self.playlists.len()),
            AppView::Tracks | AppView::Playing => (&mut self.track_state, self.tracks.len()),
            AppView::Downloading => return, // No navigation while downloading
        };
        if len == 0 {
            return;
        }
        let current = state.selected().unwrap_or(0);
        let next = if current >= len - 1 { 0 } else { current + 1 };
        state.select(Some(next));
    }

    /// Move the selection cursor up in the current list.
    fn move_selection_up(&mut self) {
        let (state, len) = match self.view {
            AppView::Playlists => (&mut self.playlist_state, self.playlists.len()),
            AppView::Tracks | AppView::Playing => (&mut self.track_state, self.tracks.len()),
            AppView::Downloading => return,
        };
        if len == 0 {
            return;
        }
        let current = state.selected().unwrap_or(0);
        let prev = if current == 0 { len - 1 } else { current - 1 };
        state.select(Some(prev));
    }

    /// Handle Enter key -- select a playlist (fetch tracks) or a track (start
    /// download + playback).
    async fn select_item(&mut self) -> Result<()> {
        match self.view {
            AppView::Playlists => {
                if let Some(idx) = self.playlist_state.selected() {
                    if let Some(playlist) = self.playlists.get(idx) {
                        let rating_key = playlist.rating_key.clone();
                        self.current_playlist_title = playlist.title.clone();

                        tracing::info!(
                            playlist = %playlist.title,
                            key = %rating_key,
                            "Fetching tracks for playlist"
                        );

                        self.tracks = self
                            .plex_client
                            .fetch_tracks(&rating_key)
                            .await?;

                        tracing::info!(count = self.tracks.len(), "Fetched tracks");

                        // Reset track selection to first item
                        self.track_state = ListState::default();
                        if !self.tracks.is_empty() {
                            self.track_state.select(Some(0));
                        }
                        self.view = AppView::Tracks;
                    }
                }
            }
            AppView::Tracks | AppView::Playing => {
                self.start_track_download()?;
            }
            AppView::Downloading => {
                // Ignore Enter while downloading
            }
        }
        Ok(())
    }

    /// Start downloading the selected track on a background thread.
    ///
    /// Gets the stream URL from the Plex client, spawns a std::thread to
    /// download the audio bytes using reqwest::blocking, and sends the result
    /// back via an mpsc channel. The event loop checks for completion on each
    /// iteration via check_download_complete().
    fn start_track_download(&mut self) -> Result<()> {
        if let Some(idx) = self.track_state.selected() {
            if let Some(track) = self.tracks.get(idx) {
                // Get the stream URL from the track's media parts
                let part_key = track
                    .media
                    .first()
                    .and_then(|m| m.parts.first())
                    .map(|p| p.key.as_str());

                let part_key = match part_key {
                    Some(key) => key,
                    None => {
                        tracing::warn!(
                            track = %track.title,
                            "Track has no media parts, cannot play"
                        );
                        return Ok(());
                    }
                };

                let stream_url = self.plex_client.stream_url(part_key);
                let track_name = track.title.clone();

                tracing::info!(
                    track = %track_name,
                    url = %stream_url,
                    "Starting track download"
                );

                // Set state to Downloading
                self.view = AppView::Downloading;

                // Spawn background download thread
                let (tx, rx) = std::sync::mpsc::channel();
                self.download_rx = Some(rx);

                std::thread::spawn(move || {
                    let result = Player::download_track(&stream_url)
                        .map(|bytes| (bytes, track_name));
                    let _ = tx.send(result);
                });
            }
        }
        Ok(())
    }

    /// Go back from Tracks/Playing view to Playlists view.
    fn go_back(&mut self) {
        match self.view {
            AppView::Tracks | AppView::Playing => {
                self.view = AppView::Playlists;
                self.tracks.clear();
                self.current_playlist_title.clear();
            }
            _ => {}
        }
    }
}

// ---------------------------------------------------------------------------
// Authentication flow (called before TUI starts)
// ---------------------------------------------------------------------------

/// Authenticate with Plex and return a connected PlexClient + server name.
///
/// This runs on the normal terminal (not alternate screen) so the user can
/// see the auth URL. If a valid token exists in config, it validates and
/// skips the PIN flow. If the token is invalid or missing, the PIN flow
/// runs automatically (per locked decision: no error messages, just restart
/// the PIN flow).
pub async fn authenticate(config: &mut Config) -> Result<(PlexClient, String)> {
    let http_client = reqwest::Client::new();
    let client_id = config.client_id.clone();

    // Check if we have a last-used server with a stored token
    if let Some(ref server_id) = config.last_server {
        if let Some(server) = config.servers.get(server_id) {
            tracing::info!("Validating existing token for server: {}", server.name);
            let valid = auth::validate_token(
                &http_client,
                &server.token,
                &client_id,
            )
            .await?;

            if valid {
                tracing::info!("Token is valid, connecting to {}", server.name);
                let name = server.name.clone();
                let plex = PlexClient::new(&server.url, &server.token, &client_id);
                return Ok((plex, name));
            }

            tracing::info!("Token is invalid, starting re-authentication");
        }
    }

    // No valid token -- run the PIN authentication flow.
    // This prints to stdout (normal terminal, not TUI alternate screen).
    let token = run_pin_auth(&http_client, &client_id).await?;

    // Discover servers with the new token
    let servers = plex::discover_servers(&http_client, &token, &client_id).await?;

    if servers.is_empty() {
        return Err(color_eyre::eyre::eyre!(
            "No Plex servers found for this account"
        ));
    }

    // Select a server (auto-select if only one, otherwise prompt)
    let server = if servers.len() == 1 {
        println!("Auto-selected server: {}", servers[0].name);
        servers.into_iter().next().unwrap()
    } else {
        select_server(&servers)?
    };

    // Save server config
    let server_config = ServerConfig {
        name: server.name.clone(),
        url: server.uri.clone(),
        token: token.clone(),
    };
    config
        .servers
        .insert(server.client_identifier.clone(), server_config);
    config.last_server = Some(server.client_identifier.clone());
    config::save_config(config)?;

    tracing::info!(
        server = %server.name,
        url = %server.uri,
        "Server saved to config"
    );

    let name = server.name.clone();
    let plex = PlexClient::new(&server.uri, &token, &client_id);
    Ok((plex, name))
}

/// Run the Plex PIN-based authentication flow.
///
/// Displays the auth URL on stdout and polls until the user authenticates
/// in their browser. Returns the auth token.
async fn run_pin_auth(
    http_client: &reqwest::Client,
    client_id: &str,
) -> Result<String> {
    let (pin_id, _code, auth_url) =
        auth::start_auth(http_client, client_id).await?;

    println!();
    println!("  Open this URL in your browser:");
    println!();
    println!("    {}", auth_url);
    println!();
    println!("  Waiting for authentication...");
    io::stdout().flush()?;

    let token = auth::wait_for_auth(http_client, pin_id, client_id).await?;

    println!("  Authenticated successfully!");
    println!();

    Ok(token)
}

/// Present a numbered list of servers for the user to choose from.
///
/// Reads a number from stdin. Used only during initial auth flow (before
/// the TUI is started) when the user has multiple Plex servers.
fn select_server(servers: &[PlexServer]) -> Result<PlexServer> {
    println!("  Multiple Plex servers found:");
    println!();
    for (i, server) in servers.iter().enumerate() {
        println!("    {}. {} ({})", i + 1, server.name, server.uri);
    }
    println!();
    print!("  Select a server [1-{}]: ", servers.len());
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    let choice: usize = input
        .trim()
        .parse()
        .map_err(|_| color_eyre::eyre::eyre!("Invalid selection"))?;

    if choice < 1 || choice > servers.len() {
        return Err(color_eyre::eyre::eyre!(
            "Selection out of range: {}",
            choice
        ));
    }

    Ok(servers[choice - 1].clone())
}
