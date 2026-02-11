use std::io::{self, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
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
use crate::visualizer::{self, VisualizerData, VisualizerState};
use rand::seq::SliceRandom;

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
// Repeat mode
// ---------------------------------------------------------------------------

/// Repeat mode for playlist playback.
///
/// Cycles: Off -> All -> One -> Off via the 'r' keybinding.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RepeatMode {
    Off,
    All,
    One,
}

impl RepeatMode {
    pub fn cycle(self) -> Self {
        match self {
            Self::Off => Self::All,
            Self::All => Self::One,
            Self::One => Self::Off,
        }
    }

    pub fn indicator(&self) -> &'static str {
        match self {
            Self::Off => "",
            Self::All => "[Repeat: All]",
            Self::One => "[Repeat: One]",
        }
    }

    /// Convert to a string representation for session persistence.
    pub fn to_string_repr(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::All => "all",
            Self::One => "one",
        }
    }

    /// Create a RepeatMode from a string representation (session restore).
    /// Returns Off for any unrecognized string.
    pub fn from_string_repr(s: &str) -> Self {
        match s {
            "all" => Self::All,
            "one" => Self::One,
            _ => Self::Off,
        }
    }
}

// ---------------------------------------------------------------------------
// NowPlaying metadata
// ---------------------------------------------------------------------------

/// Metadata for the currently playing track, stored for UI display.
/// Updated atomically when a new track starts playing (in check_download_complete).
pub struct NowPlaying {
    pub track_name: String,
    pub artist: String,
    pub album: String,
    pub duration_ms: u64,
}

// ---------------------------------------------------------------------------
// Tmux now-playing file
// ---------------------------------------------------------------------------

/// Write content to the now-playing file for tmux status bar consumption.
///
/// File: `~/.local/share/termtunes/now_playing`
/// Format: "Artist - Track" (playing), "|| Artist - Track" (paused), "" (stopped).
///
/// Best-effort: errors are logged but never propagated. This is cosmetic
/// (tmux display) and must never affect playback or cause panics.
fn write_now_playing_file(content: &str) {
    let path = config::now_playing_path();
    if let Some(parent) = path.parent() {
        if let Err(e) = std::fs::create_dir_all(parent) {
            tracing::warn!("Failed to create now_playing parent dir: {}", e);
            return;
        }
    }
    if let Err(e) = std::fs::write(&path, content) {
        tracing::warn!("Failed to write now_playing file: {}", e);
    }
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

    /// Rating key of the currently selected/playing playlist (for session persistence).
    /// Set when entering Tracks/Playing view or starting a favorite.
    current_playlist_rating_key: Option<String>,

    /// Audio player (initialized lazily when first track is played).
    player: Option<Player>,

    /// Channel receiver for completed track downloads.
    /// The background download thread sends (audio_bytes, track_name) when done.
    download_rx: Option<std::sync::mpsc::Receiver<Result<(Vec<u8>, String)>>>,

    /// Error message to display in the status bar. Set when audio device
    /// initialization fails or download errors occur. Cleared on next
    /// successful action.
    error_message: Option<String>,

    /// Index of the currently playing track in `self.tracks`.
    /// None before the first track is played.
    current_track_index: Option<usize>,

    /// Metadata of the currently playing track for UI display.
    /// Updated when download completes and playback starts.
    now_playing: Option<NowPlaying>,

    /// Persisted volume level across track changes.
    /// Each new rodio Sink starts at 1.0, so we save and restore this value.
    saved_volume: f32,

    /// Whether shuffle mode is active (toggled with 's').
    shuffle_enabled: bool,

    /// Shuffled index array -- maps shuffle positions to track indices.
    shuffle_order: Vec<usize>,

    /// Current position within the shuffle_order array.
    shuffle_position: usize,

    /// Current repeat mode (Off, All, One) -- cycled with 'r'.
    repeat_mode: RepeatMode,

    /// True when user pressed 'f' in Playlists view and we are waiting
    /// for a number key (1-9) to assign the selected playlist as a favorite.
    awaiting_favorite_key: bool,

    /// Whether the visualizer display is enabled (toggled with 'v').
    visualizer_enabled: bool,

    /// Shared data between the audio thread (writes samples) and UI thread
    /// (reads for FFT). Created once at app startup, passed to each new
    /// VisualizerSource when a track starts playing.
    visualizer_data: Arc<Mutex<VisualizerData>>,

    /// Temporal smoothing state for visualizer bars (fast attack, slow decay).
    visualizer_state: VisualizerState,

    /// Dynamic number of bars based on available terminal width (set by UI).
    visualizer_num_bars: usize,

    /// Raw ambient volume setting (0.0 to 1.0), independent of main volume.
    /// Default: 0.7 (30% lower than default main volume of 1.0, per user decision).
    ambient_volume: f32,

    /// Master volume multiplier applied AFTER budget enforcement.
    /// Scales the combined output. Default: 1.0 (no scaling).
    master_volume: f32,

    /// Channel receiver for completed ambient track downloads.
    /// The background download thread sends (audio_bytes, track_name) when done.
    /// Separate from download_rx so ambient downloads don't block main track downloads.
    ambient_download_rx: Option<std::sync::mpsc::Receiver<Result<(Vec<u8>, String)>>>,
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
            current_playlist_rating_key: None,
            player: None,
            download_rx: None,
            error_message: None,
            current_track_index: None,
            now_playing: None,
            saved_volume: 1.0,
            shuffle_enabled: false,
            shuffle_order: Vec::new(),
            shuffle_position: 0,
            repeat_mode: RepeatMode::Off,
            awaiting_favorite_key: false,
            visualizer_enabled: false,
            visualizer_data: visualizer::create_visualizer_data(visualizer::FFT_SIZE),
            visualizer_state: VisualizerState::new(visualizer::NUM_BARS),
            visualizer_num_bars: visualizer::NUM_BARS,
            ambient_volume: 0.7,
            master_volume: 1.0,
            ambient_download_rx: None,
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

    /// Get a reference to the NowPlaying metadata (if a track is playing).
    pub fn now_playing(&self) -> Option<&NowPlaying> {
        self.now_playing.as_ref()
    }

    /// Get the index of the currently playing track in the tracks list.
    pub fn current_track_index(&self) -> Option<usize> {
        self.current_track_index
    }

    /// Get the saved volume level (0.0 to 1.0).
    pub fn saved_volume(&self) -> f32 {
        self.saved_volume
    }

    /// Whether shuffle mode is currently active.
    pub fn shuffle_enabled(&self) -> bool {
        self.shuffle_enabled
    }

    /// Get the current repeat mode.
    pub fn repeat_mode(&self) -> RepeatMode {
        self.repeat_mode
    }

    /// Whether the app is waiting for a number key (1-9) to assign a favorite.
    pub fn awaiting_favorite_key(&self) -> bool {
        self.awaiting_favorite_key
    }

    /// Get a reference to the favorites map (key "1"-"9" -> FavoritePlaylist).
    pub fn favorites(&self) -> &std::collections::HashMap<String, config::FavoritePlaylist> {
        &self.config.favorites
    }

    /// Whether the audio visualizer is enabled (toggled with 'v').
    pub fn visualizer_enabled(&self) -> bool {
        self.visualizer_enabled
    }

    /// Get the current smoothed visualizer bar values for rendering.
    pub fn visualizer_bars(&self) -> &[f64] {
        self.visualizer_state.bars()
    }

    /// Update the visualizer FFT computation and smoothing.
    ///
    /// Only performs work when the visualizer is enabled AND a track is
    /// playing. When disabled or paused, this is a no-op (zero CPU overhead).
    /// Called once per render tick (~10 Hz) in the event loop.
    pub fn update_visualizer(&mut self) {
        if !self.visualizer_enabled || self.now_playing.is_none() {
            return;
        }

        if let Some(bars) = visualizer::compute_spectrum_bars(
            &self.visualizer_data,
            self.visualizer_num_bars,
        ) {
            self.visualizer_state.update(&bars);
        }
    }

    /// Set the dynamic number of visualizer bars (called by UI based on width).
    pub fn set_visualizer_num_bars(&mut self, n: usize) {
        self.visualizer_num_bars = n.clamp(4, 64);
    }

    /// Get the ambient volume level (0.0 to 1.0).
    pub fn ambient_volume(&self) -> f32 {
        self.ambient_volume
    }

    /// Get the master volume multiplier (0.0 to 1.0).
    pub fn master_volume(&self) -> f32 {
        self.master_volume
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

            // Check for completed ambient downloads (non-blocking)
            self.check_ambient_download_complete();

            // Auto-advance to next track when current track finishes.
            // Uses advance_track which respects repeat mode (One, All, Off).
            if self.view == AppView::Playing && self.download_rx.is_none() {
                if let Some(player) = &self.player {
                    if player.is_finished() {
                        tracing::info!("Track finished, auto-advancing");
                        let had_next = self.advance_track()?;
                        if !had_next {
                            // No next track -- clear now-playing file (playback stopped)
                            write_now_playing_file("");
                        }
                    }
                }
            }

            // Check ambient loop (runs regardless of view -- ambient loops even when browsing)
            self.check_ambient_loop();

            // Update visualizer FFT data before drawing (runs at render tick
            // rate ~10 Hz). No-op when visualizer is disabled or paused.
            self.update_visualizer();

            // Draw the UI
            terminal.draw(|frame| {
                ui::render(frame, self);
            })?;

            // Poll for events with 100ms timeout.
            if event::poll(Duration::from_millis(100))? {
                match event::read()? {
                    Event::Key(key) if key.kind == KeyEventKind::Press => {
                        self.handle_key(key.code, key.modifiers).await?;
                    }
                    Event::Resize(_w, _h) => {
                        // Fullscreen ratatui auto-resizes buffers on next draw().
                        // No action needed -- loop continues and draw() is called.
                    }
                    _ => {}
                }
            }
        }

        // Save session state and clear now-playing file on graceful exit
        self.save_session_state();
        write_now_playing_file("");

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

                    // Start playback with the saved volume level
                    if let Some(player) = &mut self.player {
                        match player.load_and_play(audio_bytes, track_name.clone(), self.saved_volume, Some(Arc::clone(&self.visualizer_data))) {
                            Ok(()) => {
                                self.view = AppView::Playing;
                                self.error_message = None;

                                // Populate NowPlaying metadata from the current track
                                if let Some(idx) = self.current_track_index {
                                    if let Some(track) = self.tracks.get(idx) {
                                        let artist = track.artist.clone().unwrap_or_else(|| "Unknown Artist".to_string());
                                        let album = track.album.clone().unwrap_or_else(|| "Unknown Album".to_string());
                                        // Write now-playing file for tmux status bar
                                        write_now_playing_file(&format!("{} - {}", artist, track_name));
                                        self.now_playing = Some(NowPlaying {
                                            track_name,
                                            artist,
                                            album,
                                            duration_ms: track.duration.unwrap_or(0),
                                        });
                                    }
                                }
                            }
                            Err(e) => {
                                tracing::error!("Failed to start playback: {}", e);
                                self.error_message = Some(format!("Playback error: {}", e));
                                self.view = AppView::Tracks;
                            }
                        }
                    }

                    // Apply volume budget to ensure the new main_sink gets
                    // the budget-enforced volume (corrects for ambient if active).
                    self.apply_volume_budget();

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
                    // Update tmux now-playing file with pause/resume state
                    if let Some(np) = &self.now_playing {
                        if player.is_paused() {
                            write_now_playing_file(&format!("|| {} - {}", np.artist, np.track_name));
                        } else {
                            write_now_playing_file(&format!("{} - {}", np.artist, np.track_name));
                        }
                    }
                }
            }
            // Volume up (PLAY-06) -- + or =
            (KeyCode::Char('+'), _) | (KeyCode::Char('='), _) => {
                self.volume_up();
            }
            // Volume down (PLAY-07) -- - or _
            (KeyCode::Char('-'), _) | (KeyCode::Char('_'), _) => {
                self.volume_down();
            }
            // Next track (PLAY-04) -- n or >
            (KeyCode::Char('n'), _) | (KeyCode::Char('>'), _) => {
                if matches!(self.view, AppView::Playing | AppView::Tracks) {
                    self.next_track()?;
                }
            }
            // Previous track (PLAY-05) -- N or <
            (KeyCode::Char('N'), _) | (KeyCode::Char('<'), _) => {
                if matches!(self.view, AppView::Playing | AppView::Tracks) {
                    self.prev_track()?;
                }
            }
            // Navigate down
            (KeyCode::Char('j'), _) | (KeyCode::Down, _) => self.move_selection_down(),
            // Navigate up
            (KeyCode::Char('k'), _) | (KeyCode::Up, _) => self.move_selection_up(),
            // Select / Enter
            (KeyCode::Enter, _) => self.select_item().await?,
            // Toggle shuffle (DIFF-01) -- s
            (KeyCode::Char('s'), _) => {
                self.toggle_shuffle();
                tracing::info!(shuffle = self.shuffle_enabled, "Shuffle toggled");
            }
            // Cycle repeat mode (DIFF-02) -- r
            (KeyCode::Char('r'), _) => {
                self.repeat_mode = self.repeat_mode.cycle();
                tracing::info!(mode = ?self.repeat_mode, "Repeat mode cycled");
            }
            // Seek forward (DIFF-03) -- l or Right (Playing view only)
            (KeyCode::Char('l'), _) | (KeyCode::Right, _) => {
                if matches!(self.view, AppView::Playing) {
                    if let (Some(player), Some(np)) = (&self.player, &self.now_playing) {
                        if let Err(e) = player.seek_forward(np.duration_ms) {
                            tracing::warn!("Seek forward failed: {}", e);
                        }
                    }
                }
            }
            // Seek backward (DIFF-03) -- h or Left (Playing view only)
            (KeyCode::Char('h'), _) | (KeyCode::Left, _) => {
                if matches!(self.view, AppView::Playing) {
                    if let Some(player) = &self.player {
                        if let Err(e) = player.seek_backward() {
                            tracing::warn!("Seek backward failed: {}", e);
                        }
                    }
                }
            }
            // TEMPORARY: Load selected track as ambient for Phase 6 testing.
            // Phase 7 replaces this with the track browser ambient selection.
            (KeyCode::Char('a'), _) => {
                if matches!(self.view, AppView::Playing | AppView::Tracks) {
                    self.start_ambient_from_selected()?;
                }
            }
            // Toggle ambient mute (AUDIO-04)
            (KeyCode::Char('m'), _) => {
                if self.ambient_volume > 0.0 {
                    self.mute_ambient();
                    tracing::info!(channel = "ambient", "Ambient muted");
                } else {
                    self.unmute_ambient();
                    tracing::info!(channel = "ambient", "Ambient unmuted");
                }
            }
            // Toggle visualizer (POL-01, POL-02) -- v
            (KeyCode::Char('v'), _) => {
                self.visualizer_enabled = !self.visualizer_enabled;
                tracing::info!(enabled = self.visualizer_enabled, "Visualizer toggled");
            }
            // Assign favorite (DIFF-04) -- f key starts favorite assignment mode
            (KeyCode::Char('f'), _) => {
                if matches!(self.view, AppView::Playlists) {
                    self.awaiting_favorite_key = true;
                }
            }
            // Number keys 1-9: assign favorite (if awaiting) or start favorite
            (KeyCode::Char(c @ '1'..='9'), _) => {
                if self.awaiting_favorite_key {
                    self.assign_favorite(c)?;
                    self.awaiting_favorite_key = false;
                } else {
                    self.start_favorite(c).await?;
                }
            }
            // Back (from Tracks/Playing to Playlists)
            (KeyCode::Esc, _) | (KeyCode::Backspace, _) => {
                self.awaiting_favorite_key = false;
                self.go_back();
            }
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
                        self.current_playlist_rating_key = Some(rating_key.clone());

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

                        // Regenerate shuffle order for the new playlist
                        if self.shuffle_enabled {
                            self.regenerate_shuffle_order();
                        }

                        self.view = AppView::Tracks;
                    }
                }
            }
            AppView::Tracks | AppView::Playing => {
                // Sync current_track_index with list selection so next/prev
                // work correctly after manual track selection (Pitfall 5).
                self.current_track_index = self.track_state.selected();

                // If shuffle is enabled, sync the shuffle position to the
                // manually selected track (per Research Open Question 3).
                if self.shuffle_enabled {
                    if let Some(selected) = self.current_track_index {
                        if let Some(pos) = self.shuffle_order.iter().position(|&i| i == selected) {
                            self.shuffle_position = pos;
                        }
                    }
                }

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
    ///
    /// Clears navigation state (tracks list, shuffle order) for the UI, but
    /// preserves session-relevant fields (playlist key/title, track index,
    /// now_playing) so that `save_session_state()` can still capture the last
    /// playing context when the user quits from the Playlists view.
    fn go_back(&mut self) {
        match self.view {
            AppView::Tracks | AppView::Playing => {
                self.view = AppView::Playlists;
                self.tracks.clear();
                // NOTE: Do NOT clear current_playlist_title, current_playlist_rating_key,
                // current_track_index, or now_playing here. These are needed by
                // save_session_state() if the user quits from the Playlists view
                // while music is still playing. They will be overwritten naturally
                // when the user selects a new playlist (select_item/start_favorite).
                //
                // Clear shuffle navigation state (regenerated when entering a playlist).
                self.shuffle_order.clear();
                self.shuffle_position = 0;
            }
            _ => {}
        }
    }

    // -----------------------------------------------------------------------
    // Track navigation
    // -----------------------------------------------------------------------

    /// Play the track at the given index in the tracks list.
    ///
    /// This is the shared core method used by next_track, prev_track, and
    /// select_item. It cancels any in-progress download, updates the index
    /// and list selection, then triggers a new download.
    fn play_track_at_index(&mut self, index: usize) -> Result<()> {
        if index >= self.tracks.len() {
            return Ok(());
        }
        // Cancel any in-progress download by dropping the old receiver
        self.download_rx = None;
        // Update index and sync list selection
        self.current_track_index = Some(index);
        self.track_state.select(Some(index));
        // Use existing download mechanism
        self.start_track_download()
    }

    /// Get the next track index based on current mode (shuffle or sequential).
    ///
    /// Returns None if at end of playlist/shuffle order (caller decides wrap behavior).
    fn next_track_index(&self) -> Option<usize> {
        if self.tracks.is_empty() {
            return None;
        }
        if self.shuffle_enabled {
            let next_pos = self.shuffle_position + 1;
            if next_pos < self.shuffle_order.len() {
                Some(self.shuffle_order[next_pos])
            } else {
                None // End of shuffle order
            }
        } else {
            match self.current_track_index {
                Some(idx) if idx + 1 < self.tracks.len() => Some(idx + 1),
                _ => None, // End of playlist
            }
        }
    }

    /// Get the previous track index based on current mode (shuffle or sequential).
    ///
    /// Returns None if at beginning of playlist/shuffle order.
    fn prev_track_index(&self) -> Option<usize> {
        if self.tracks.is_empty() {
            return None;
        }
        if self.shuffle_enabled {
            if self.shuffle_position > 0 {
                Some(self.shuffle_order[self.shuffle_position - 1])
            } else {
                None // Beginning of shuffle order
            }
        } else {
            match self.current_track_index {
                Some(0) | None => Some(self.tracks.len() - 1), // Wrap to end
                Some(idx) => Some(idx - 1),
            }
        }
    }

    /// Skip to the next track, respecting shuffle and repeat modes.
    ///
    /// For user-initiated skip (n/> key). RepeatMode::One is NOT handled here --
    /// that is only for auto-advance. User skip always goes to next track.
    fn next_track(&mut self) -> Result<()> {
        if self.tracks.is_empty() {
            return Ok(());
        }
        match self.next_track_index() {
            Some(idx) => {
                if self.shuffle_enabled {
                    self.shuffle_position += 1;
                }
                self.play_track_at_index(idx)
            }
            None => {
                // End of playlist/shuffle order
                match self.repeat_mode {
                    RepeatMode::All => {
                        // Wrap around (reshuffle if shuffle enabled)
                        if self.shuffle_enabled {
                            self.regenerate_shuffle_order();
                            if let Some(&first) = self.shuffle_order.first() {
                                self.shuffle_position = 0;
                                self.play_track_at_index(first)
                            } else {
                                Ok(())
                            }
                        } else {
                            self.play_track_at_index(0)
                        }
                    }
                    _ => Ok(()), // Off or One: do nothing at end
                }
            }
        }
    }

    /// Skip to the previous track, respecting shuffle and repeat modes.
    ///
    /// Previous always wraps (existing behavior preserved).
    fn prev_track(&mut self) -> Result<()> {
        if self.tracks.is_empty() {
            return Ok(());
        }
        match self.prev_track_index() {
            Some(idx) => {
                if self.shuffle_enabled {
                    self.shuffle_position = self.shuffle_position.saturating_sub(1);
                }
                self.play_track_at_index(idx)
            }
            None => {
                // At beginning of shuffle order or playlist
                match self.repeat_mode {
                    RepeatMode::All => {
                        // Wrap to end
                        if self.shuffle_enabled {
                            let last_pos = self.shuffle_order.len().saturating_sub(1);
                            if let Some(&last) = self.shuffle_order.last() {
                                self.shuffle_position = last_pos;
                                self.play_track_at_index(last)
                            } else {
                                Ok(())
                            }
                        } else {
                            let last = self.tracks.len().saturating_sub(1);
                            self.play_track_at_index(last)
                        }
                    }
                    _ => {
                        // Off or One: wrap to end anyway (prev always wraps)
                        let last = self.tracks.len().saturating_sub(1);
                        self.play_track_at_index(last)
                    }
                }
            }
        }
    }

    /// Auto-advance to next track when current track finishes.
    ///
    /// Unlike next_track (user-initiated), this respects RepeatMode::One
    /// by replaying the current track from cached audio bytes.
    ///
    /// Returns true if a new track was started, false if playback stopped
    /// (end of playlist with RepeatMode::Off).
    fn advance_track(&mut self) -> Result<bool> {
        match self.repeat_mode {
            RepeatMode::One => {
                // Replay current track from cached bytes (no re-download)
                if let Some(player) = &mut self.player {
                    if let Err(e) = player.replay_current(self.saved_volume, Some(Arc::clone(&self.visualizer_data))) {
                        tracing::warn!("Repeat One replay failed: {}, falling back to download", e);
                        // Fallback: re-download if cached replay fails
                        if let Some(idx) = self.current_track_index {
                            self.play_track_at_index(idx)?;
                        }
                    }
                }
                // Apply volume budget to the fresh main_sink
                self.apply_volume_budget();
                Ok(true)
            }
            RepeatMode::All => {
                match self.next_track_index() {
                    Some(idx) => {
                        if self.shuffle_enabled {
                            self.shuffle_position += 1;
                        }
                        self.play_track_at_index(idx)?;
                        Ok(true)
                    }
                    None => {
                        // End of playlist/shuffle order -- wrap
                        if self.shuffle_enabled {
                            self.regenerate_shuffle_order();
                            if let Some(&first) = self.shuffle_order.first() {
                                self.shuffle_position = 0;
                                self.play_track_at_index(first)?;
                                Ok(true)
                            } else {
                                Ok(false)
                            }
                        } else {
                            self.play_track_at_index(0)?;
                            Ok(true)
                        }
                    }
                }
            }
            RepeatMode::Off => {
                match self.next_track_index() {
                    Some(idx) => {
                        if self.shuffle_enabled {
                            self.shuffle_position += 1;
                        }
                        self.play_track_at_index(idx)?;
                        Ok(true)
                    }
                    None => Ok(false), // End of playlist -- stop
                }
            }
        }
    }

    // -----------------------------------------------------------------------
    // Shuffle management
    // -----------------------------------------------------------------------

    /// Toggle shuffle mode on/off.
    ///
    /// When enabling: generates a shuffled index array with the current track
    /// at position 0 (so the user continues from where they are).
    /// When disabling: shuffle_order becomes unused (sequential navigation).
    fn toggle_shuffle(&mut self) {
        self.shuffle_enabled = !self.shuffle_enabled;
        if self.shuffle_enabled && !self.tracks.is_empty() {
            let mut indices: Vec<usize> = (0..self.tracks.len()).collect();
            indices.shuffle(&mut rand::rng());
            // Put current track at position 0 so user continues from here
            if let Some(current) = self.current_track_index {
                if let Some(pos) = indices.iter().position(|&i| i == current) {
                    indices.swap(0, pos);
                }
            }
            self.shuffle_position = 0;
            self.shuffle_order = indices;
        }
    }

    /// Regenerate the shuffle order (e.g., after wrapping in Repeat All).
    ///
    /// Places the current track at position 0 to avoid immediate repeat.
    fn regenerate_shuffle_order(&mut self) {
        if self.tracks.is_empty() {
            self.shuffle_order.clear();
            self.shuffle_position = 0;
            return;
        }
        let mut indices: Vec<usize> = (0..self.tracks.len()).collect();
        indices.shuffle(&mut rand::rng());
        if let Some(current) = self.current_track_index {
            if let Some(pos) = indices.iter().position(|&i| i == current) {
                indices.swap(0, pos);
            }
        }
        self.shuffle_order = indices;
        self.shuffle_position = 0;
    }

    // -----------------------------------------------------------------------
    // Favorite playlist management
    // -----------------------------------------------------------------------

    /// Assign the currently selected playlist as a favorite for the given key.
    ///
    /// Saves the assignment to config.toml so it persists across restarts.
    /// Only valid when in the Playlists view with a playlist selected.
    fn assign_favorite(&mut self, key: char) -> Result<()> {
        if let Some(idx) = self.playlist_state.selected() {
            if let Some(playlist) = self.playlists.get(idx) {
                let fav = config::FavoritePlaylist {
                    rating_key: playlist.rating_key.clone(),
                    title: playlist.title.clone(),
                };
                tracing::info!(key = %key, playlist = %playlist.title, "Assigned favorite playlist");
                self.config.favorites.insert(key.to_string(), fav);
                config::save_config(&self.config)?;
            }
        }
        Ok(())
    }

    /// Start playback of the favorite playlist assigned to the given key.
    ///
    /// Fetches tracks from the Plex server, resets track state, and starts
    /// playing the first track. Does nothing if no favorite is assigned to
    /// the key.
    async fn start_favorite(&mut self, key: char) -> Result<()> {
        let key_str = key.to_string();
        let fav = match self.config.favorites.get(&key_str) {
            Some(fav) => fav.clone(),
            None => return Ok(()), // No favorite assigned to this key
        };

        tracing::info!(key = %key, playlist = %fav.title, "Starting favorite playlist");

        // Fetch tracks for the favorite playlist
        match self.plex_client.fetch_tracks(&fav.rating_key).await {
            Ok(tracks) => {
                self.tracks = tracks;
                self.current_playlist_title = fav.title;
                self.current_playlist_rating_key = Some(fav.rating_key);
            }
            Err(e) => {
                tracing::error!(key = %key, "Failed to fetch favorite playlist: {}", e);
                self.error_message = Some(format!("Favorite not found: {}", e));
                return Ok(());
            }
        }

        // Reset track state
        self.track_state = ListState::default();
        if !self.tracks.is_empty() {
            self.track_state.select(Some(0));
        }

        // Reset shuffle order for new playlist if shuffle is enabled
        if self.shuffle_enabled {
            self.regenerate_shuffle_order();
        }

        // Start playing the first track
        if !self.tracks.is_empty() {
            self.play_track_at_index(0)?;
        }
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Volume control (with budget enforcement)
    // -----------------------------------------------------------------------

    /// Increase main volume and apply budget enforcement.
    ///
    /// Volume is managed by App (not Player) to enable budget enforcement.
    /// The saved_volume field is the authoritative source for main channel
    /// raw volume. Player sinks receive computed values after budget + master.
    fn volume_up(&mut self) {
        self.saved_volume = (self.saved_volume + 0.05).min(1.0);
        self.apply_volume_budget();
    }

    /// Decrease main volume and apply budget enforcement.
    fn volume_down(&mut self) {
        self.saved_volume = (self.saved_volume - 0.05).max(0.0);
        self.apply_volume_budget();
    }

    /// Enforce volume budget: main + ambient <= 1.0, then scale by master.
    ///
    /// Per user decision: when volumes exceed budget, auto-scale both
    /// proportionally to fit. Master volume applies AFTER budget enforcement.
    fn apply_volume_budget(&mut self) {
        let sum = self.saved_volume + self.ambient_volume;
        let (main_budgeted, ambient_budgeted) = if sum > 1.0 {
            let scale = 1.0 / sum;
            (self.saved_volume * scale, self.ambient_volume * scale)
        } else {
            (self.saved_volume, self.ambient_volume)
        };

        // Apply master volume after budget enforcement
        let main_final = main_budgeted * self.master_volume;
        let ambient_final = ambient_budgeted * self.master_volume;

        if let Some(player) = &self.player {
            player.set_main_volume(main_final);
            player.set_ambient_volume(ambient_final);
        }
    }

    /// Mute ambient channel by setting its volume to 0.
    /// Per user decision: muting is simply volume=0, no separate state.
    fn mute_ambient(&mut self) {
        self.ambient_volume = 0.0;
        self.apply_volume_budget();
    }

    /// Unmute ambient channel by restoring default volume (0.7).
    /// Since muting is just volume=0, unmuting restores a reasonable default.
    fn unmute_ambient(&mut self) {
        self.ambient_volume = 0.7;
        self.apply_volume_budget();
    }

    // -----------------------------------------------------------------------
    // Ambient loop and test trigger
    // -----------------------------------------------------------------------

    /// Check if the ambient track has finished and restart the loop.
    ///
    /// Polls player.is_ambient_finished() on every event loop tick (~100ms).
    /// If the ambient track has ended and cached data exists, re-append from
    /// cached bytes. Uses manual re-append (NOT repeat_infinite) to avoid the
    /// confirmed memory leak in rodio #673.
    ///
    /// Per user decision: brief silence between loops is acceptable --
    /// prioritize memory safety over gapless playback.
    fn check_ambient_loop(&mut self) {
        if let Some(player) = &mut self.player {
            if player.is_ambient_finished() && player.has_ambient_data() {
                // Compute budget-enforced ambient volume
                let sum = self.saved_volume + self.ambient_volume;
                let ambient_effective = if sum > 1.0 {
                    (self.ambient_volume / sum) * self.master_volume
                } else {
                    self.ambient_volume * self.master_volume
                };

                match player.replay_ambient(ambient_effective) {
                    Ok(()) => {
                        // Loop restarted successfully -- no user-visible action needed
                    }
                    Err(e) => {
                        // Failure isolation: log error, stop ambient, keep main playing
                        tracing::error!(
                            channel = "ambient",
                            "Ambient loop restart failed: {}, stopping ambient", e
                        );
                        player.stop_ambient();
                    }
                }
            }
        }
    }

    /// Load an ambient track with failure isolation.
    ///
    /// Per user decision: ambient decode/play failures log an error, clear
    /// ambient state, and keep main music playing. Ambient failures never
    /// crash the app or interrupt main playback.
    fn load_ambient_track(&mut self, audio_bytes: Vec<u8>, track_name: String) {
        // Compute budget-enforced ambient volume
        let sum = self.saved_volume + self.ambient_volume;
        let ambient_effective = if sum > 1.0 {
            (self.ambient_volume / sum) * self.master_volume
        } else {
            self.ambient_volume * self.master_volume
        };

        if let Some(player) = &mut self.player {
            match player.load_ambient(audio_bytes, track_name, ambient_effective) {
                Ok(()) => {
                    tracing::info!(channel = "ambient", "Ambient track loaded successfully");
                    self.error_message = None;
                }
                Err(e) => {
                    // Isolate failure: log, clear ambient state, keep main playing
                    tracing::error!(channel = "ambient", "Failed to load ambient track: {}", e);
                    player.stop_ambient();
                    // Do NOT set error_message -- ambient errors are silent per phase scope
                    // (UI error visibility deferred to Phase 8)
                }
            }
        }
    }

    /// TEMPORARY: Start ambient playback using the currently selected track.
    ///
    /// Spawns a background thread to download the selected track, then the
    /// event loop polls `check_ambient_download_complete()` to load it as
    /// the ambient channel when the download finishes.
    ///
    /// Uses the same background-thread + mpsc pattern as main track downloads
    /// (`start_track_download` / `check_download_complete`) to avoid the
    /// tokio runtime nesting panic that occurs when calling
    /// `reqwest::blocking` from within an async context.
    ///
    /// TODO(Phase 7): Remove this method when track browser is implemented.
    fn start_ambient_from_selected(&mut self) -> Result<()> {
        if let Some(idx) = self.track_state.selected() {
            if let Some(track) = self.tracks.get(idx) {
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
                            "Track has no media parts, cannot play as ambient"
                        );
                        return Ok(());
                    }
                };

                let stream_url = self.plex_client.stream_url(part_key);
                let track_name = track.title.clone();

                tracing::info!(
                    channel = "ambient",
                    track = %track_name,
                    "Starting ambient track download"
                );

                // Spawn background download thread (same pattern as start_track_download)
                let (tx, rx) = std::sync::mpsc::channel();
                self.ambient_download_rx = Some(rx);

                std::thread::spawn(move || {
                    let result = Player::download_track(&stream_url)
                        .map(|bytes| (bytes, track_name));
                    let _ = tx.send(result);
                });
            }
        }
        Ok(())
    }

    /// Check if a background ambient download has completed.
    ///
    /// Uses try_recv on the mpsc channel so it never blocks the event loop.
    /// When download completes, loads the audio as the ambient channel via
    /// `load_ambient_track()` with failure isolation.
    fn check_ambient_download_complete(&mut self) {
        if let Some(rx) = &self.ambient_download_rx {
            match rx.try_recv() {
                Ok(Ok((audio_bytes, track_name))) => {
                    tracing::info!(
                        channel = "ambient",
                        track = %track_name,
                        size = audio_bytes.len(),
                        "Ambient download complete, loading"
                    );
                    self.ambient_download_rx = None;
                    self.load_ambient_track(audio_bytes, track_name);
                }
                Ok(Err(e)) => {
                    tracing::error!(
                        channel = "ambient",
                        "Failed to download ambient track: {}", e
                    );
                    // Failure isolated -- main playback unaffected
                    self.ambient_download_rx = None;
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {
                    // Download still in progress -- keep waiting
                }
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    tracing::error!(
                        channel = "ambient",
                        "Ambient download thread disconnected unexpectedly"
                    );
                    self.ambient_download_rx = None;
                }
            }
        }
    }

    // -----------------------------------------------------------------------
    // Session persistence
    // -----------------------------------------------------------------------

    /// Save the current session state to disk for persistence across restarts.
    ///
    /// Constructs a Session from current app state and writes it to session.toml.
    /// Called on graceful exit only (not on every track change).
    fn save_session_state(&self) {
        let session = config::Session {
            playlist_rating_key: self.current_playlist_rating_key.clone(),
            playlist_title: Some(self.current_playlist_title.clone()),
            track_index: self.current_track_index,
            volume: self.saved_volume,
            shuffle_enabled: self.shuffle_enabled,
            repeat_mode: self.repeat_mode.to_string_repr().to_string(),
        };
        if let Err(e) = config::save_session(&session) {
            tracing::error!("Failed to save session state: {}", e);
        }
    }

    /// Restore session state from disk, positioning the user at the saved
    /// playlist and track without auto-playing.
    ///
    /// Best-effort: any errors (missing playlist, failed fetch) are logged
    /// but do not prevent the app from starting. The user sees the Tracks view
    /// at the saved position and must press Enter/Space to resume playback.
    pub async fn restore_session(&mut self) {
        let session = match config::load_session() {
            Some(s) => s,
            None => return,
        };

        // Need a playlist rating key to restore
        let rating_key = match &session.playlist_rating_key {
            Some(key) if !key.is_empty() => key.clone(),
            _ => return,
        };

        // Find the saved playlist in the fetched playlists
        let playlist_idx = match self.playlists.iter().position(|p| p.rating_key == rating_key) {
            Some(idx) => idx,
            None => {
                tracing::warn!(
                    rating_key = %rating_key,
                    "Session playlist not found in server playlists (may have been deleted)"
                );
                return;
            }
        };

        // Fetch tracks for the saved playlist
        let tracks = match self.plex_client.fetch_tracks(&rating_key).await {
            Ok(t) => t,
            Err(e) => {
                tracing::warn!("Failed to fetch tracks for session restore: {}", e);
                return;
            }
        };

        if tracks.is_empty() {
            tracing::warn!("Session playlist has no tracks, skipping restore");
            return;
        }

        // Restore playlist state
        let playlist_title = session
            .playlist_title
            .unwrap_or_else(|| self.playlists[playlist_idx].title.clone());
        self.current_playlist_title = playlist_title;
        self.current_playlist_rating_key = Some(rating_key);
        self.tracks = tracks;

        // Restore track selection (clamped to valid range)
        let track_idx = session
            .track_index
            .unwrap_or(0)
            .min(self.tracks.len().saturating_sub(1));
        self.track_state = ListState::default();
        self.track_state.select(Some(track_idx));
        self.current_track_index = Some(track_idx);

        // Restore playback settings
        self.saved_volume = session.volume.clamp(0.0, 1.0);
        self.shuffle_enabled = session.shuffle_enabled;
        self.repeat_mode = RepeatMode::from_string_repr(&session.repeat_mode);

        // Position at Tracks view (NOT Playing -- do NOT auto-play)
        self.view = AppView::Tracks;

        // Select the playlist in playlist_state to match
        self.playlist_state.select(Some(playlist_idx));

        // Regenerate shuffle order if shuffle was enabled
        if self.shuffle_enabled {
            self.regenerate_shuffle_order();
        }

        tracing::info!(
            playlist = %self.current_playlist_title,
            track_index = track_idx,
            volume = self.saved_volume,
            shuffle = self.shuffle_enabled,
            repeat = ?self.repeat_mode,
            "Session restored"
        );
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
