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
