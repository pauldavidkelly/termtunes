use std::io::{self, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use color_eyre::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Span;
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph};
use ratatui::DefaultTerminal;

use crate::auth;
use crate::config::{self, Config, ServerConfig};
use crate::plex::{self, Playlist, PlexClient, PlexServer, Track};

// ---------------------------------------------------------------------------
// App state machine
// ---------------------------------------------------------------------------

/// The current view / mode of the application.
#[derive(Debug, PartialEq)]
enum AppView {
    /// Displaying the list of audio playlists.
    Playlists,
    /// Displaying the tracks within a selected playlist.
    Tracks,
}

// ---------------------------------------------------------------------------
// App struct
// ---------------------------------------------------------------------------

/// Main application state.
///
/// Holds the authenticated Plex client, loaded playlists and tracks,
/// navigation state for list widgets, and the UI state machine.
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

    /// Current view (Playlists or Tracks).
    view: AppView,

    /// Selection state for the playlist list.
    playlist_state: ListState,

    /// Selection state for the track list.
    track_state: ListState,

    /// Title of the currently selected playlist (for status bar).
    current_playlist_title: String,
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
        }
    }

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

            // Draw the UI
            terminal.draw(|frame| {
                self.draw(frame);
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

    /// Draw the current UI frame.
    fn draw(&mut self, frame: &mut ratatui::Frame) {
        let area = frame.area();

        // Layout: main list area + status bar at bottom
        let [main_area, status_area] = Layout::vertical([
            Constraint::Fill(1),
            Constraint::Length(1),
        ])
        .areas(area);

        match self.view {
            AppView::Playlists => {
                let items: Vec<ListItem> = self
                    .playlists
                    .iter()
                    .map(|p| {
                        let count = p
                            .leaf_count
                            .map(|c| format!(" ({} tracks)", c))
                            .unwrap_or_default();
                        ListItem::new(format!("{}{}", p.title, count))
                    })
                    .collect();

                let list = List::new(items)
                    .block(
                        Block::default()
                            .title(" Playlists ")
                            .borders(Borders::ALL)
                            .border_style(Style::default().fg(Color::DarkGray)),
                    )
                    .highlight_style(
                        Style::default()
                            .fg(Color::Black)
                            .bg(Color::Cyan)
                            .add_modifier(Modifier::BOLD),
                    )
                    .highlight_symbol("> ");

                frame.render_stateful_widget(list, main_area, &mut self.playlist_state);

                // Status bar: connected server + playlist count
                let status = Paragraph::new(Span::styled(
                    format!(
                        " Connected to {} | {} playlists | j/k:nav Enter:select q:quit",
                        self.server_name,
                        self.playlists.len()
                    ),
                    Style::default().fg(Color::White).bg(Color::DarkGray),
                ))
                .style(Style::default().bg(Color::DarkGray));
                frame.render_widget(status, status_area);
            }
            AppView::Tracks => {
                let items: Vec<ListItem> = self
                    .tracks
                    .iter()
                    .map(|t| {
                        let artist = t
                            .artist
                            .as_deref()
                            .unwrap_or("Unknown Artist");
                        ListItem::new(format!("{} - {}", t.title, artist))
                    })
                    .collect();

                let list = List::new(items)
                    .block(
                        Block::default()
                            .title(format!(" {} ", self.current_playlist_title))
                            .borders(Borders::ALL)
                            .border_style(Style::default().fg(Color::DarkGray)),
                    )
                    .highlight_style(
                        Style::default()
                            .fg(Color::Black)
                            .bg(Color::Cyan)
                            .add_modifier(Modifier::BOLD),
                    )
                    .highlight_symbol("> ");

                frame.render_stateful_widget(list, main_area, &mut self.track_state);

                // Status bar: playlist name + track count
                let status = Paragraph::new(Span::styled(
                    format!(
                        " {} | {} tracks | j/k:nav Esc:back q:quit",
                        self.current_playlist_title,
                        self.tracks.len()
                    ),
                    Style::default().fg(Color::White).bg(Color::DarkGray),
                ))
                .style(Style::default().bg(Color::DarkGray));
                frame.render_widget(status, status_area);
            }
        }
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
            // Navigate down
            (KeyCode::Char('j'), _) | (KeyCode::Down, _) => self.move_selection_down(),
            // Navigate up
            (KeyCode::Char('k'), _) | (KeyCode::Up, _) => self.move_selection_up(),
            // Select / Enter
            (KeyCode::Enter, _) => self.select_item().await?,
            // Back (from Tracks to Playlists)
            (KeyCode::Esc, _) | (KeyCode::Backspace, _) => self.go_back(),
            _ => {}
        }
        Ok(())
    }

    /// Move the selection cursor down in the current list.
    fn move_selection_down(&mut self) {
        let (state, len) = match self.view {
            AppView::Playlists => (&mut self.playlist_state, self.playlists.len()),
            AppView::Tracks => (&mut self.track_state, self.tracks.len()),
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
            AppView::Tracks => (&mut self.track_state, self.tracks.len()),
        };
        if len == 0 {
            return;
        }
        let current = state.selected().unwrap_or(0);
        let prev = if current == 0 { len - 1 } else { current - 1 };
        state.select(Some(prev));
    }

    /// Handle Enter key -- select a playlist (fetch tracks) or a track (no-op for now).
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
            AppView::Tracks => {
                // Track selection is a no-op for now -- playback
                // will be implemented in Plan 03.
            }
        }
        Ok(())
    }

    /// Go back from Tracks view to Playlists view.
    fn go_back(&mut self) {
        if self.view == AppView::Tracks {
            self.view = AppView::Playlists;
            self.tracks.clear();
            self.current_playlist_title.clear();
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
