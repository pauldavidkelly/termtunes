use std::collections::HashMap;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

use color_eyre::Result;
use serde::{Deserialize, Serialize};

/// Top-level application configuration.
///
/// Stored as TOML at `~/.config/termtunes/config.toml`.
/// Contains a persistent client_id (UUID v4) used as X-Plex-Client-Identifier,
/// the last-used server identifier, and a map of server configurations.
#[derive(Serialize, Deserialize, Default, Debug)]
pub struct Config {
    /// Persistent UUID for the X-Plex-Client-Identifier header.
    pub client_id: String,

    /// Machine identifier of the last-used Plex server.
    pub last_server: Option<String>,

    /// Server configurations keyed by machine identifier.
    #[serde(default)]
    pub servers: HashMap<String, ServerConfig>,

    /// Favorite playlist assignments keyed by number key ("1" through "9").
    /// Allows instant playlist activation with a single key press.
    #[serde(default)]
    pub favorites: HashMap<String, FavoritePlaylist>,
}

/// Configuration for a single Plex server.
#[derive(Serialize, Deserialize, Debug)]
pub struct ServerConfig {
    /// Human-readable server name.
    pub name: String,

    /// Server URL (e.g., "http://192.168.1.100:32400").
    pub url: String,

    /// X-Plex-Token for authenticating with this server.
    pub token: String,
}

/// A favorite playlist assignment, mapping a hotkey (1-9) to a playlist.
///
/// Stored in the config file so favorites persist across application restarts.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FavoritePlaylist {
    /// The Plex rating key identifying the playlist.
    pub rating_key: String,

    /// Human-readable playlist title (for display in UI).
    pub title: String,
}

/// Session state for persistence across application restarts.
///
/// Stored as TOML at `~/.local/share/termtunes/session.toml`.
/// Contains the last-played playlist and track position, plus playback
/// settings (volume, shuffle, repeat). Separate from config.toml because
/// session state is volatile/app-managed while config is user-editable.
#[derive(Serialize, Deserialize, Default, Debug)]
pub struct Session {
    /// Rating key of the last-played playlist.
    pub playlist_rating_key: Option<String>,

    /// Title of the last-played playlist (for display).
    pub playlist_title: Option<String>,

    /// Index of the last-played track in the playlist.
    pub track_index: Option<usize>,

    /// Volume level (0.0 to 1.0).
    pub volume: f32,

    /// Whether shuffle was enabled.
    pub shuffle_enabled: bool,

    /// Repeat mode stored as string for TOML readability ("off", "all", "one").
    pub repeat_mode: String,

    /// Part key of the ambient track (e.g., "/library/parts/12345/file.flac").
    /// Used to reconstruct the stream URL on restore.
    #[serde(default)]
    pub ambient_part_key: Option<String>,

    /// Display name of the ambient track.
    #[serde(default)]
    pub ambient_track_name: Option<String>,

    /// Ambient volume level (0.0 to 1.0). None means "first use" --
    /// triggers PERSIST-05 default (30% lower than main music volume).
    #[serde(default)]
    pub ambient_volume: Option<f32>,

    /// Whether ambient was playing (true) or muted/off (false) at save time.
    /// Default false: ambient does not auto-start on first use.
    #[serde(default)]
    pub ambient_enabled: bool,
}

/// Resolve the path to the config file using XDG conventions.
///
/// Returns `~/.config/termtunes/config.toml` on Linux, with a fallback
/// to `~/.config` if `dirs::config_dir()` returns None.
pub fn config_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from(format!("{}/.config", std::env::var("HOME").unwrap_or_default())))
        .join("termtunes")
        .join("config.toml")
}

/// Load the configuration from disk, or create a new one with a fresh UUID.
///
/// If the config file exists, it is read and deserialized from TOML.
/// If it does not exist, a new Config is created with a random client_id.
pub fn load_config() -> Result<Config> {
    let path = config_path();
    if path.exists() {
        let contents = std::fs::read_to_string(&path)?;
        let config: Config = toml::from_str(&contents)?;
        Ok(config)
    } else {
        Ok(Config {
            client_id: uuid::Uuid::new_v4().to_string(),
            ..Default::default()
        })
    }
}

/// Save the configuration to disk as pretty-printed TOML.
///
/// Creates parent directories if they do not exist.
/// Sets file permissions to 0o600 (owner read/write only) because the
/// config may contain Plex auth tokens.
pub fn save_config(config: &Config) -> Result<()> {
    let path = config_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let contents = toml::to_string_pretty(config)?;
    std::fs::write(&path, &contents)?;

    // Set permissions to 0o600 (owner read/write only) for security.
    // Config file may contain Plex auth tokens.
    let metadata = std::fs::metadata(&path)?;
    let mut perms = metadata.permissions();
    perms.set_mode(0o600);
    std::fs::set_permissions(&path, perms)?;

    Ok(())
}

// ---------------------------------------------------------------------------
// Session persistence (separate from config -- volatile, app-managed state)
// ---------------------------------------------------------------------------

/// Resolve the path to the session state file.
///
/// Returns `~/.local/share/termtunes/session.toml` with the same
/// fallback pattern as `config_path()` for when `dirs::data_dir()` returns None.
pub fn session_path() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| {
            PathBuf::from(format!(
                "{}/.local/share",
                std::env::var("HOME").unwrap_or_default()
            ))
        })
        .join("termtunes")
        .join("session.toml")
}

/// Load session state from disk, returning None if missing or unparseable.
///
/// Session restore is best-effort: errors are silently swallowed so a
/// corrupted or outdated session file never prevents the app from starting.
pub fn load_session() -> Option<Session> {
    let path = session_path();
    if !path.exists() {
        return None;
    }
    let contents = std::fs::read_to_string(&path).ok()?;
    toml::from_str(&contents).ok()
}

/// Save session state to disk as TOML.
///
/// Creates parent directories if needed. Uses 0o600 permissions for
/// consistency with the config file.
pub fn save_session(session: &Session) -> Result<()> {
    let path = session_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let contents = toml::to_string_pretty(session)?;
    std::fs::write(&path, &contents)?;

    // Set permissions to 0o600 for consistency with config.toml.
    let metadata = std::fs::metadata(&path)?;
    let mut perms = metadata.permissions();
    perms.set_mode(0o600);
    std::fs::set_permissions(&path, perms)?;

    Ok(())
}

/// Resolve the path to the now-playing file for tmux status bar integration.
///
/// Returns `~/.local/share/termtunes/now_playing`. Tmux can read this via
/// `#(cat ~/.local/share/termtunes/now_playing)` in status-right.
pub fn now_playing_path() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| {
            PathBuf::from(format!(
                "{}/.local/share",
                std::env::var("HOME").unwrap_or_default()
            ))
        })
        .join("termtunes")
        .join("now_playing")
}
