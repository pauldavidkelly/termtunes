use color_eyre::Result;
use serde::Deserialize;

use crate::auth::build_plex_headers;

// ---------------------------------------------------------------------------
// Server discovery types
// ---------------------------------------------------------------------------

/// A Plex server discovered via the resources endpoint.
#[derive(Debug, Clone)]
pub struct PlexServer {
    /// Human-readable server name.
    pub name: String,
    /// Machine identifier (stable across restarts, used as config key).
    pub client_identifier: String,
    /// Best connection URI for this server (selected from connections list).
    pub uri: String,
}

/// Raw resource entry from `GET /api/v2/resources`.
#[derive(Deserialize, Debug)]
struct Resource {
    name: String,
    #[serde(rename = "clientIdentifier")]
    client_identifier: String,
    #[serde(default)]
    provides: String,
    #[serde(default)]
    connections: Vec<Connection>,
}

/// A single connection option for a Plex resource (server).
#[derive(Deserialize, Debug)]
struct Connection {
    uri: String,
    #[serde(default)]
    local: bool,
    #[serde(default)]
    relay: bool,
}

// ---------------------------------------------------------------------------
// Playlist types
// ---------------------------------------------------------------------------

/// Top-level wrapper for the playlists response.
/// GET {server}/playlists?playlistType=audio returns:
/// { "MediaContainer": { "Metadata": [...] } }
#[derive(Deserialize, Debug)]
pub struct PlaylistContainer {
    #[serde(rename = "MediaContainer")]
    pub media_container: PlaylistMediaContainer,
}

/// The inner container holding the list of playlists.
#[derive(Deserialize, Debug)]
pub struct PlaylistMediaContainer {
    #[serde(rename = "Metadata", default)]
    pub metadata: Vec<Playlist>,
}

/// A single audio playlist on the Plex server.
#[derive(Deserialize, Debug, Clone)]
pub struct Playlist {
    #[serde(rename = "ratingKey")]
    pub rating_key: String,
    pub title: String,
    #[serde(rename = "leafCount", default)]
    pub leaf_count: Option<u32>,
    #[serde(default)]
    pub duration: Option<u64>,
}

// ---------------------------------------------------------------------------
// Track types
// ---------------------------------------------------------------------------

/// Top-level wrapper for the track list response.
/// GET {server}/playlists/{ratingKey}/items returns:
/// { "MediaContainer": { "Metadata": [...] } }
#[derive(Deserialize, Debug)]
pub struct TrackContainer {
    #[serde(rename = "MediaContainer")]
    pub media_container: TrackMediaContainer,
}

/// The inner container holding the list of tracks.
#[derive(Deserialize, Debug)]
pub struct TrackMediaContainer {
    #[serde(rename = "Metadata", default)]
    pub metadata: Vec<Track>,
}

/// A single audio track.
#[derive(Deserialize, Debug, Clone)]
pub struct Track {
    #[serde(rename = "ratingKey")]
    pub rating_key: String,
    pub title: String,
    /// Artist name (from grandparentTitle in the Plex response).
    #[serde(rename = "grandparentTitle", default)]
    pub artist: Option<String>,
    /// Album name (from parentTitle in the Plex response).
    #[serde(rename = "parentTitle", default)]
    pub album: Option<String>,
    #[serde(default)]
    pub duration: Option<u64>,
    #[serde(rename = "Media", default)]
    pub media: Vec<Media>,
}

/// Media container within a Track (codec/bitrate level).
#[derive(Deserialize, Debug, Clone)]
pub struct Media {
    #[serde(rename = "Part", default)]
    pub parts: Vec<Part>,
}

/// A single media part -- contains the key used to construct the stream URL.
#[derive(Deserialize, Debug, Clone)]
pub struct Part {
    pub key: String,
}

// ---------------------------------------------------------------------------
// Artist types
// ---------------------------------------------------------------------------

/// A music artist from a Plex library section.
/// Fetched via GET {server}/library/sections/{key}/all?type=8
#[derive(Deserialize, Debug, Clone)]
pub struct Artist {
    #[serde(rename = "ratingKey")]
    pub rating_key: String,
    pub title: String,
    #[serde(default)]
    pub thumb: Option<String>,
}

/// Top-level wrapper for the artist list response.
#[derive(Deserialize, Debug)]
pub struct ArtistContainer {
    #[serde(rename = "MediaContainer")]
    pub media_container: ArtistMediaContainer,
}

/// The inner container holding the list of artists.
#[derive(Deserialize, Debug)]
pub struct ArtistMediaContainer {
    #[serde(rename = "Metadata", default)]
    pub metadata: Vec<Artist>,
}

// ---------------------------------------------------------------------------
// Album types
// ---------------------------------------------------------------------------

/// An album from a Plex artist's children.
/// Fetched via GET {server}/library/metadata/{ratingKey}/children
#[derive(Deserialize, Debug, Clone)]
pub struct Album {
    #[serde(rename = "ratingKey")]
    pub rating_key: String,
    pub title: String,
    #[serde(default)]
    pub year: Option<u32>,
    #[serde(rename = "parentTitle", default)]
    pub parent_title: Option<String>,
}

/// Top-level wrapper for the album list response.
#[derive(Deserialize, Debug)]
pub struct AlbumContainer {
    #[serde(rename = "MediaContainer")]
    pub media_container: AlbumMediaContainer,
}

/// The inner container holding the list of albums.
#[derive(Deserialize, Debug)]
pub struct AlbumMediaContainer {
    #[serde(rename = "Metadata", default)]
    pub metadata: Vec<Album>,
}

// ---------------------------------------------------------------------------
// Library section types
// ---------------------------------------------------------------------------

/// A Plex library section (movie, TV, music, photo).
#[derive(Deserialize, Debug, Clone)]
pub struct LibrarySection {
    /// Unique section key (used in API paths, e.g., "3").
    pub key: String,
    /// Human-readable name (e.g., "Music", "Ambient Sounds").
    pub title: String,
    /// Section type: "movie", "show", "artist" (music), "photo".
    #[serde(rename = "type")]
    pub section_type: String,
}

/// Top-level wrapper for the library sections response.
/// GET {server}/library/sections returns:
/// { "MediaContainer": { "Directory": [...] } }
#[derive(Deserialize, Debug)]
pub struct SectionsContainer {
    #[serde(rename = "MediaContainer")]
    pub media_container: SectionsMediaContainer,
}

/// The inner container holding the list of library sections.
#[derive(Deserialize, Debug)]
pub struct SectionsMediaContainer {
    #[serde(rename = "Directory", default)]
    pub directory: Vec<LibrarySection>,
}

// ---------------------------------------------------------------------------
// Plex API client
// ---------------------------------------------------------------------------

/// Client for interacting with a specific Plex media server.
///
/// Holds the server URL, auth token, and client identifier needed for all
/// requests. Created after successful authentication and server selection.
pub struct PlexClient {
    client: reqwest::Client,
    server_url: String,
    token: String,
    client_id: String,
}

impl PlexClient {
    /// Create a new PlexClient for a specific server.
    pub fn new(server_url: &str, token: &str, client_id: &str) -> Self {
        Self {
            client: reqwest::Client::new(),
            server_url: server_url.trim_end_matches('/').to_string(),
            token: token.to_string(),
            client_id: client_id.to_string(),
        }
    }

    /// Build headers for requests to the Plex media server.
    /// Includes the standard Plex headers plus X-Plex-Token.
    fn server_headers(&self) -> reqwest::header::HeaderMap {
        let mut headers = build_plex_headers(&self.client_id);
        headers.insert("X-Plex-Token", self.token.parse().unwrap());
        headers
    }

    /// Fetch all audio playlists from the server.
    ///
    /// GET {server_url}/playlists?playlistType=audio
    pub async fn fetch_playlists(&self) -> Result<Vec<Playlist>> {
        let headers = self.server_headers();
        let resp = self
            .client
            .get(format!("{}/playlists", self.server_url))
            .headers(headers)
            .query(&[("playlistType", "audio")])
            .send()
            .await?
            .error_for_status()?
            .json::<PlaylistContainer>()
            .await?;
        Ok(resp.media_container.metadata)
    }

    /// Fetch all tracks in a playlist by its rating key.
    ///
    /// GET {server_url}/playlists/{rating_key}/items
    pub async fn fetch_tracks(&self, rating_key: &str) -> Result<Vec<Track>> {
        let headers = self.server_headers();
        let resp = self
            .client
            .get(format!(
                "{}/playlists/{}/items",
                self.server_url, rating_key
            ))
            .headers(headers)
            .send()
            .await?
            .error_for_status()?
            .json::<TrackContainer>()
            .await?;
        Ok(resp.media_container.metadata)
    }

    /// Fetch all library sections from the server.
    ///
    /// GET {server_url}/library/sections
    pub async fn fetch_library_sections(&self) -> Result<Vec<LibrarySection>> {
        let headers = self.server_headers();
        let resp = self
            .client
            .get(format!("{}/library/sections", self.server_url))
            .headers(headers)
            .send()
            .await?
            .error_for_status()?
            .json::<SectionsContainer>()
            .await?;
        Ok(resp.media_container.directory)
    }

    /// Fetch all tracks in a music library section.
    ///
    /// GET {server_url}/library/sections/{section_key}/all?type=10&includeMedia=1
    /// type=10 is the Plex content type for audio tracks.
    /// includeMedia=1 ensures the Media array (with Part keys for stream URLs)
    /// is included in the response -- without this, the library section endpoint
    /// may return tracks with empty Media arrays.
    pub async fn fetch_section_tracks(&self, section_key: &str) -> Result<Vec<Track>> {
        let headers = self.server_headers();
        let resp = self
            .client
            .get(format!(
                "{}/library/sections/{}/all",
                self.server_url, section_key
            ))
            .headers(headers)
            .query(&[("type", "10"), ("includeMedia", "1")])
            .send()
            .await?
            .error_for_status()?
            .json::<TrackContainer>()
            .await?;
        Ok(resp.media_container.metadata)
    }

    /// Fetch all artists in a music library section.
    ///
    /// GET {server_url}/library/sections/{section_key}/all?type=8
    /// type=8 is the Plex content type for artists.
    pub async fn fetch_section_artists(&self, section_key: &str) -> Result<Vec<Artist>> {
        let headers = self.server_headers();
        let resp = self
            .client
            .get(format!(
                "{}/library/sections/{}/all",
                self.server_url, section_key
            ))
            .headers(headers)
            .query(&[("type", "8")])
            .send()
            .await?
            .error_for_status()?
            .json::<ArtistContainer>()
            .await?;
        Ok(resp.media_container.metadata)
    }

    /// Fetch all albums for an artist by their rating key.
    ///
    /// GET {server_url}/library/metadata/{artist_rating_key}/children
    pub async fn fetch_artist_albums(&self, artist_rating_key: &str) -> Result<Vec<Album>> {
        let headers = self.server_headers();
        let resp = self
            .client
            .get(format!(
                "{}/library/metadata/{}/children",
                self.server_url, artist_rating_key
            ))
            .headers(headers)
            .send()
            .await?
            .error_for_status()?
            .json::<AlbumContainer>()
            .await?;
        Ok(resp.media_container.metadata)
    }

    /// Fetch all tracks in an album by its rating key.
    ///
    /// GET {server_url}/library/metadata/{album_rating_key}/children?includeMedia=1
    /// includeMedia=1 ensures Track.media is populated for stream URL construction.
    pub async fn fetch_album_tracks(&self, album_rating_key: &str) -> Result<Vec<Track>> {
        let headers = self.server_headers();
        let resp = self
            .client
            .get(format!(
                "{}/library/metadata/{}/children",
                self.server_url, album_rating_key
            ))
            .headers(headers)
            .query(&[("includeMedia", "1")])
            .send()
            .await?
            .error_for_status()?
            .json::<TrackContainer>()
            .await?;
        Ok(resp.media_container.metadata)
    }

    /// Construct a stream URL for a media part.
    ///
    /// The part key is a path like "/library/parts/12345/file.flac".
    /// The stream URL appends the auth token as a query parameter.
    pub fn stream_url(&self, part_key: &str) -> String {
        format!(
            "{}{}?X-Plex-Token={}",
            self.server_url, part_key, self.token
        )
    }
}

// ---------------------------------------------------------------------------
// Server discovery (uses plex.tv, not a specific server)
// ---------------------------------------------------------------------------

/// Discover the user's Plex servers via the plex.tv resources API.
///
/// GET https://plex.tv/api/v2/resources?includeHttps=1&includeRelay=1
/// Filters for entries where `provides` contains "server" and selects the
/// best connection for each: prefer local, then non-relay direct, then relay.
pub async fn discover_servers(
    client: &reqwest::Client,
    token: &str,
    client_id: &str,
) -> Result<Vec<PlexServer>> {
    let mut headers = build_plex_headers(client_id);
    headers.insert("X-Plex-Token", token.parse().unwrap());

    let resources: Vec<Resource> = client
        .get("https://plex.tv/api/v2/resources")
        .headers(headers)
        .query(&[("includeHttps", "1"), ("includeRelay", "1")])
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    let servers = resources
        .into_iter()
        .filter(|r| r.provides.contains("server"))
        .filter_map(|r| {
            let uri = pick_best_connection(&r.connections)?;
            Some(PlexServer {
                name: r.name,
                client_identifier: r.client_identifier,
                uri,
            })
        })
        .collect();

    Ok(servers)
}

/// Select the best connection URI from a list of server connections.
///
/// Priority: local connections first, then non-relay direct connections,
/// then relay connections as a last resort.
fn pick_best_connection(connections: &[Connection]) -> Option<String> {
    // First: prefer local connections
    if let Some(conn) = connections.iter().find(|c| c.local && !c.relay) {
        return Some(conn.uri.clone());
    }
    // Second: prefer direct (non-relay, non-local) connections
    if let Some(conn) = connections.iter().find(|c| !c.relay) {
        return Some(conn.uri.clone());
    }
    // Last resort: relay connection
    connections.first().map(|c| c.uri.clone())
}
