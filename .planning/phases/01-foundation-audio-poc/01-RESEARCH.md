# Phase 1: Foundation and Audio Proof-of-Concept - Research

**Researched:** 2026-02-10
**Domain:** Plex authentication, audio playback on WSL2, terminal lifecycle management (Rust)
**Confidence:** MEDIUM-HIGH

## Summary

Phase 1 proves the project's technical viability by validating three foundational pillars: Plex PIN-based OAuth authentication with token persistence, audio playback via rodio on WSL2, and terminal state management with clean restoration on all exit paths. The research confirms that all three are achievable with the chosen Rust stack, but each has specific pitfalls that require careful implementation.

The most critical finding is that **rodio requires Read+Seek for its Decoder**, which means audio from Plex must be fully downloaded into memory (via reqwest blocking bytes into a `Cursor<Vec<u8>>`) before playback can begin. True HTTP streaming without buffering requires a custom `MediaSource` implementation for Symphonia -- but for Phase 1's proof-of-concept, downloading tracks into memory is the correct approach (simpler, supports seeking, sufficient for validating audio works). The second critical finding is that WSL2 audio via WSLg's PulseAudio bridge has documented issues with pause/resume after >5 seconds, requiring a watchdog or stream recreation strategy. Third, ratatui provides well-documented patterns for panic hooks and terminal restoration, and the `signal-hook` crate handles SIGINT/SIGTERM/SIGHUP cleanly.

**Primary recommendation:** Download full tracks into `Cursor<Vec<u8>>` for rodio playback. Implement Plex PIN auth from scratch with reqwest (no SDK). Install panic hooks and signal handlers from the first commit. Test WSL2 pause/resume early and implement stream recreation if needed.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- **Plex Auth:** Display URL + PIN in terminal, user manually opens browser (no auto-open)
- **Token Storage:** `~/.config/termtunes/config.toml` (XDG standard location, TOML format)
- **Token Expiry:** Automatically prompt re-authentication (no error messages, just restart PIN flow)
- **Multi-server:** Store multiple server tokens, default to last-used server on startup, allow server selection via settings/config
- **Playback Selection:** User selects playlist + track via simple CLI menu to start playback
- **Keyboard Control:** Spacebar toggles play/pause
- **Controls in Phase 1:** Play/pause ONLY -- no volume/skip yet
- **Status Display:** Minimal TUI with single-line status bar showing track name and play/pause state
- **Signals:** Catch SIGINT, SIGTERM, SIGHUP
- **Verification:** Automated validation script that tests quit (q), Ctrl+C, kill signal, SIGHUP

### Claude's Discretion
- Exact terminal restoration steps (cursor visibility, alternate screen, input mode)
- Whether to use panic hooks for terminal cleanup
- CLI menu implementation for playlist/track selection
- Status bar design and layout details

### Deferred Ideas (OUT OF SCOPE)
None -- discussion stayed within phase scope
</user_constraints>

## Standard Stack

### Core (Phase 1 specific)

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| rodio | 0.21.1 | Audio playback (Sink: play/pause/stop, Decoder: format detection) | Pure Rust audio via Symphonia. Used by spotify_player. Handles MP3/FLAC/AAC/WAV/OGG. |
| reqwest | 0.13.x (blocking feature) | HTTP client for Plex API + audio download | Blocking mode for audio download into memory. Async mode for API calls. rustls for TLS. |
| crossterm | 0.29.x | Terminal raw mode, alternate screen, cursor control, keyboard events | Default ratatui backend. Pure Rust. WSL2 compatible. |
| ratatui | 0.30.0 | Minimal TUI rendering (status bar only in Phase 1) | `ratatui::init()` / `ratatui::restore()` handle terminal setup/teardown. |
| tokio | 1.47.x | Async runtime for Plex API calls and PIN polling | LTS. Required by reqwest async. |
| signal-hook | 0.3.x | Unix signal handling (SIGINT, SIGTERM, SIGHUP) | Safe signal handling via flag-based or iterator-based patterns. Avoids unsafe. |
| serde + serde_json | 1.x | JSON deserialization for Plex API responses | Derive macros for typed Plex response models. |
| toml | 0.8.x | Config file serialization/deserialization | Serde integration. Read/write `config.toml`. |
| dirs | 6.x | XDG directory resolution | `dirs::config_dir()` returns `~/.config` on Linux. |
| uuid | 1.x | Generate persistent Client Identifier | Plex requires a stable `X-Plex-Client-Identifier` UUID per app instance. |

### Supporting

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| color-eyre | 0.6.x | Error reporting with panic hook integration | Installs panic hook that restores terminal before displaying backtrace. |
| tracing + tracing-subscriber | 0.1.x / 0.3.x | Structured logging to file | Debug audio/network issues without disrupting TUI. Write to `~/.local/share/termtunes/termtunes.log`. |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Full download into Cursor | Custom Symphonia MediaSource for streaming | Streaming avoids memory buffering but adds complexity. Not needed for Phase 1 PoC. Revisit in Phase 2 if tracks >100MB. |
| signal-hook | ctrlc crate | ctrlc only handles SIGINT. signal-hook handles all three required signals (SIGINT, SIGTERM, SIGHUP). |
| color-eyre | manual panic hook | color-eyre integrates panic hook + error reporting in one setup call. Less code. |
| reqwest blocking | reqwest async + tokio::spawn_blocking | Blocking is simpler for the download-then-play pattern. Async adds unnecessary complexity for sequential download. |

### Installation

```bash
# System dependency (one-time, for rodio/cpal ALSA backend)
sudo apt install libasound2-dev

# Cargo.toml dependencies
# [dependencies]
# ratatui = { version = "0.30", features = ["crossterm"] }
# crossterm = { version = "0.29", features = ["event-stream"] }
# rodio = { version = "0.21", features = ["symphonia-all"] }
# tokio = { version = "1", features = ["full"] }
# reqwest = { version = "0.13", features = ["json", "blocking"] }
# serde = { version = "1", features = ["derive"] }
# serde_json = "1"
# toml = "0.8"
# dirs = "6"
# uuid = { version = "1", features = ["v4"] }
# signal-hook = "0.3"
# color-eyre = "0.6"
# tracing = "0.1"
# tracing-subscriber = { version = "0.3", features = ["env-filter"] }
```

## Architecture Patterns

### Recommended Project Structure (Phase 1 only)

```
src/
├── main.rs              # Entry point: init terminal, run app, restore
├── app.rs               # App struct, event loop, state management
├── auth.rs              # Plex PIN-based OAuth flow
├── config.rs            # Config file (TOML) read/write, multi-server storage
├── plex.rs              # Plex API client: playlists, tracks, stream URL resolution
├── player.rs            # Audio engine: rodio Sink wrapper, play/pause/stop
├── tui.rs               # Terminal setup/restore, panic hook, signal handling
└── ui.rs                # Minimal UI: playlist/track selection menu, status bar
```

### Pattern 1: Terminal Lifecycle (RAII + Panic Hook + Signal Handler)

**What:** Triple-layer terminal restoration: (1) normal exit via `ratatui::restore()`, (2) panic via custom hook, (3) signal via signal-hook flag.
**When to use:** Always -- from the first commit.

```rust
// Source: ratatui.rs/recipes/apps/panic-hooks/ (verified)
use std::panic::{set_hook, take_hook};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use crossterm::execute;
use crossterm::terminal::{EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::cursor;
use std::io::stdout;
use signal_hook::flag;
use signal_hook::consts::{SIGINT, SIGTERM, SIGHUP};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

fn restore_terminal() {
    let _ = disable_raw_mode();
    let _ = execute!(stdout(), LeaveAlternateScreen, cursor::Show);
}

fn install_panic_hook() {
    let original_hook = take_hook();
    set_hook(Box::new(move |panic_info| {
        restore_terminal();
        original_hook(panic_info);
    }));
}

fn install_signal_handlers() -> Arc<AtomicBool> {
    let shutdown = Arc::new(AtomicBool::new(false));
    flag::register(SIGINT, Arc::clone(&shutdown)).expect("register SIGINT");
    flag::register(SIGTERM, Arc::clone(&shutdown)).expect("register SIGTERM");
    flag::register(SIGHUP, Arc::clone(&shutdown)).expect("register SIGHUP");
    shutdown
}

fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;
    install_panic_hook();
    let shutdown = install_signal_handlers();

    let mut terminal = ratatui::init();

    // Main event loop
    loop {
        if shutdown.load(Ordering::Relaxed) {
            break; // Signal received -- exit gracefully
        }
        // ... draw UI, handle events ...
    }

    ratatui::restore();
    Ok(())
}
```

**Recommendation (Claude's Discretion):** YES, install panic hooks. The cost is ~10 lines of code. The benefit is terminal always restores cleanly on panics. Use `color-eyre` for combined panic hook + error reporting. Restore: (1) disable raw mode, (2) leave alternate screen, (3) show cursor. These three cover all terminal state changes ratatui makes.

### Pattern 2: Plex PIN-Based Authentication (Polling)

**What:** Generate a PIN, display URL to user, poll plex.tv until user authenticates in browser, extract token.
**When to use:** On first launch (no token) or when token validation fails (401).

```rust
// Source: forums.plex.tv/t/authenticating-with-plex/609370 (verified)
// Endpoints:
//   POST https://plex.tv/api/v2/pins        -- create PIN
//   GET  https://plex.tv/api/v2/pins/{id}    -- check PIN status
//   GET  https://plex.tv/api/v2/user         -- validate existing token
//
// Required headers on ALL requests:
//   X-Plex-Product: "TermTunes"
//   X-Plex-Client-Identifier: <persistent UUID from config>
//   Accept: application/json
//
// PIN creation params: strong=true
// PIN check params: code=<pin_code>
//
// Auth URL for user: https://app.plex.tv/auth#?clientID={uuid}&code={code}&context[device][product]=TermTunes
//
// Poll GET pins/{id} every ~1 second until response.authToken is non-null
// authToken is the user's X-Plex-Token for all subsequent API calls

use serde::Deserialize;

#[derive(Deserialize)]
struct PinResponse {
    id: u64,
    code: String,
    #[serde(rename = "authToken")]
    auth_token: Option<String>,
}

async fn create_pin(client: &reqwest::Client, client_id: &str) -> Result<PinResponse> {
    client.post("https://plex.tv/api/v2/pins")
        .query(&[("strong", "true")])
        .header("X-Plex-Product", "TermTunes")
        .header("X-Plex-Client-Identifier", client_id)
        .header("Accept", "application/json")
        .send().await?
        .json().await
}

async fn check_pin(client: &reqwest::Client, pin_id: u64, client_id: &str) -> Result<PinResponse> {
    client.get(format!("https://plex.tv/api/v2/pins/{}", pin_id))
        .header("X-Plex-Product", "TermTunes")
        .header("X-Plex-Client-Identifier", client_id)
        .header("Accept", "application/json")
        .send().await?
        .json().await
}

async fn validate_token(client: &reqwest::Client, token: &str, client_id: &str) -> bool {
    client.get("https://plex.tv/api/v2/user")
        .header("X-Plex-Token", token)
        .header("X-Plex-Client-Identifier", client_id)
        .header("Accept", "application/json")
        .send().await
        .map(|r| r.status().is_success())
        .unwrap_or(false)
}
```

### Pattern 3: Audio Download and Playback via Rodio

**What:** Download track from Plex into memory, decode with rodio, play through Sink.
**When to use:** When user selects a track for playback.

```rust
// Source: docs.rs/rodio (verified), plexopedia.com (verified)
use std::io::Cursor;
use rodio::{Decoder, OutputStream, Sink};

// Step 1: Get track stream URL from Plex
// GET http://{server}:32400/playlists/{id}/items?X-Plex-Token={token}
// Response contains Track > Media > Part with key attribute
// Stream URL = http://{server}:32400{part.key}?X-Plex-Token={token}

// Step 2: Download track into memory
fn download_track(url: &str) -> Result<Vec<u8>> {
    // Use reqwest blocking client for simplicity
    let response = reqwest::blocking::get(url)?;
    let bytes = response.bytes()?;
    Ok(bytes.to_vec())
}

// Step 3: Play via rodio
fn play_track(audio_data: Vec<u8>) -> Result<(Sink, OutputStream)> {
    let stream = OutputStream::try_default()?;
    let sink = Sink::connect_new(&stream.mixer());
    let cursor = Cursor::new(audio_data);
    let source = Decoder::new(cursor)?;
    sink.append(source);
    Ok((sink, stream))
}

// Step 4: Control playback
// sink.pause()   -- pause playback
// sink.play()    -- resume playback
// sink.stop()    -- stop and clear queue
// sink.is_paused() -- check state
// sink.empty()   -- check if track finished
```

**Critical note:** The `OutputStream` must be kept alive for the duration of playback. If it drops, audio stops immediately. Store both `Sink` and `OutputStream` in the app state.

### Pattern 4: Config File with Multi-Server Support

**What:** TOML config at `~/.config/termtunes/config.toml` storing client ID, server tokens, and last-used server.
**When to use:** On startup (load), after auth (save), on server switch (update).

```rust
// Source: docs.rs/toml, docs.rs/dirs (verified)
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Default)]
struct Config {
    client_id: String,            // Persistent UUID for X-Plex-Client-Identifier
    last_server: Option<String>,  // Machine identifier of last-used server

    #[serde(default)]
    servers: HashMap<String, ServerConfig>,  // keyed by machine identifier
}

#[derive(Serialize, Deserialize)]
struct ServerConfig {
    name: String,
    url: String,           // e.g., "http://192.168.1.100:32400"
    token: String,         // X-Plex-Token for this server
}

fn config_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("~/.config"))
        .join("termtunes")
        .join("config.toml")
}

fn load_config() -> Result<Config> {
    let path = config_path();
    if path.exists() {
        let contents = std::fs::read_to_string(&path)?;
        Ok(toml::from_str(&contents)?)
    } else {
        Ok(Config {
            client_id: uuid::Uuid::new_v4().to_string(),
            ..Default::default()
        })
    }
}

fn save_config(config: &Config) -> Result<()> {
    let path = config_path();
    std::fs::create_dir_all(path.parent().unwrap())?;
    let contents = toml::to_string_pretty(config)?;
    std::fs::write(&path, contents)?;
    Ok(())
}

// Example config.toml output:
// client_id = "a1b2c3d4-e5f6-7890-abcd-ef1234567890"
// last_server = "abc123machine"
//
// [servers.abc123machine]
// name = "My Plex Server"
// url = "http://192.168.1.100:32400"
// token = "xYz789PlexToken"
//
// [servers.def456machine]
// name = "Remote Server"
// url = "https://remote.example.com:32400"
// token = "aBc123OtherToken"
```

### Pattern 5: Server Discovery After Authentication

**What:** After obtaining auth token from PIN flow, discover user's servers via plex.tv API.
**When to use:** After successful authentication, to find available servers.

```rust
// Source: plexapi.dev, forums.plex.tv (verified)
// Endpoint: GET https://plex.tv/api/v2/resources?includeHttps=1&includeRelay=1
// Headers: X-Plex-Token, X-Plex-Client-Identifier, Accept: application/json
//
// Response contains array of resources. Filter for "server" type.
// Each server has: name, clientIdentifier (machine ID), connections[] (uri, local flag)
// Pick the best connection: prefer local, then direct, then relay.
```

### Anti-Patterns to Avoid

- **Caching stream URLs:** Plex tokens can rotate and sessions expire. Always resolve the stream URL at play time from the track's `Part.key`, never cache the full URL.
- **Blocking the event loop on audio download:** Download the track on a background thread or via tokio::spawn_blocking, then send the bytes to the main thread via channel. Never download synchronously in the render loop.
- **Dropping OutputStream:** Rodio's `OutputStream` must live as long as playback continues. Dropping it silences audio instantly with no error. Store it in the app state alongside the Sink.
- **Hardcoding a Plex token during development:** Even for PoC, implement the PIN flow. A hardcoded token expires in 48 hours (transient) or when the user changes their password.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Audio decoding (MP3/FLAC/AAC) | Custom decoder | rodio + Symphonia | Codec edge cases are endless. Symphonia handles format detection, seeking, gapless. |
| Terminal raw mode management | Manual tcgetattr/tcsetattr | crossterm + ratatui | ratatui::init()/restore() wraps crossterm's enable_raw_mode, EnterAlternateScreen, etc. |
| Signal handling | unsafe libc::signal | signal-hook | signal-hook provides safe Rust API with flag-based and iterator-based patterns. |
| UUID generation | Random string | uuid crate v4 | Proper RFC 4122 UUIDs. Plex expects well-formed client identifiers. |
| Config file parsing | Custom parser | toml + serde derive | Serde derive handles serialization/deserialization with zero boilerplate. |
| Plex API client | plex-api crate (0.0.12) | Custom reqwest wrapper | The crate explicitly states "not ready for any use." The Plex REST API is simple enough to call directly. |

**Key insight:** Phase 1 has zero dependencies that require hand-rolling. Every component has a battle-tested Rust crate. The only custom code is the thin Plex API wrapper and the app glue logic.

## Common Pitfalls

### Pitfall 1: WSL2 Audio Pause/Resume Failure

**What goes wrong:** Audio hangs indefinitely when resuming after pausing >5 seconds. The PulseAudio stream fails to reinitialize in WSL2's WSLg subsystem.
**Why it happens:** WSLg provides a rudimentary PulseAudio bridge. Buffer state becomes inconsistent when a stream is corked (paused) for extended periods.
**How to avoid:**
1. Test pause/resume for 5s, 10s, 30s in the first sprint -- before building anything else.
2. If resume fails, implement a watchdog: detect stalled playback (sink reports playing but no audio callback for N ms), then stop the sink, recreate OutputStream, re-decode from the saved audio buffer at the last known position, and resume.
3. Set `PULSE_LATENCY_MSEC=60` environment variable to increase buffer sizes and reduce crackling.
4. Verify audio works with `pactl info` before running the app: `pactl info | grep "Server Name"` should show a PulseAudio server.
**Warning signs:** Audio works initially but hangs after first pause/resume. `sink.is_paused()` returns false but no sound plays.

### Pitfall 2: Plex Token Lifecycle Mismanagement

**What goes wrong:** Token works for developer but expires for users. App silently fails or crashes instead of re-authenticating.
**Why it happens:** Multiple token types with different lifetimes: transient (48h, invalid on restart), user tokens (long-lived but revocable), JWT (7-day refresh).
**How to avoid:**
1. Validate token on startup with `GET https://plex.tv/api/v2/user` -- 200 means valid, 401 means expired.
2. On any 401 from the Plex server, trigger the PIN auth flow again automatically.
3. Persist a Client Identifier UUID in config -- reuse it across sessions. Plex uses this to identify the app instance.
4. Never store tokens in logs or error messages.
**Warning signs:** "Works for a while then stops." No token validation on startup. Token stored without validation on load.

### Pitfall 3: OutputStream Dropped Prematurely

**What goes wrong:** Audio starts but immediately stops. No error message. Everything looks correct.
**Why it happens:** Rodio's `OutputStream` is the connection to the system audio device. When it drops (goes out of scope), the audio device is released. The `Sink` becomes orphaned.
**How to avoid:** Store both `OutputStream` and `Sink` in the same long-lived struct (e.g., `App` or `Player`). Never let `OutputStream` be a local variable in a function that returns.
**Warning signs:** Audio plays for a fraction of a second. No error returned. Adding `sleep_until_end()` in the same function works but event loop version doesn't.

### Pitfall 4: Terminal State Corruption on Abnormal Exit

**What goes wrong:** After crash or signal, terminal shows no cursor, no echo, raw escape sequences as text, alternate screen content lost.
**Why it happens:** ratatui enables raw mode + alternate screen. Without cleanup, these persist. SIGKILL (kill -9) cannot be caught at all.
**How to avoid:**
1. Install panic hook BEFORE initializing terminal (Pattern 1 above).
2. Register signal handlers via signal-hook for SIGINT/SIGTERM/SIGHUP.
3. Main loop checks shutdown flag on each iteration.
4. Normal exit calls `ratatui::restore()`.
5. Document that `kill -9` cannot be caught -- user should run `reset` if needed.
**Warning signs:** Terminal behaves strangely after Ctrl+C during development.

### Pitfall 5: Plex API Returns XML by Default

**What goes wrong:** Responses parse as invalid JSON. Confusing errors about unexpected `<` character.
**Why it happens:** Plex API defaults to XML. Must explicitly request JSON.
**How to avoid:** Include `Accept: application/json` header on EVERY Plex request. Both to plex.tv and to the local server.
**Warning signs:** serde_json deserialization errors on Plex API responses.

## Code Examples

### Complete Plex Auth Flow (Verified Pattern)

```rust
// Source: forums.plex.tv/t/authenticating-with-plex/609370

// 1. Load or create config (get client_id)
let mut config = load_config()?;
if config.client_id.is_empty() {
    config.client_id = uuid::Uuid::new_v4().to_string();
    save_config(&config)?;
}

// 2. Check if we have a valid token for the last-used server
let needs_auth = match &config.last_server {
    Some(server_id) => {
        match config.servers.get(server_id) {
            Some(server) => !validate_token(&client, &server.token, &config.client_id).await,
            None => true,
        }
    }
    None => true,
};

// 3. If no valid token, start PIN auth flow
if needs_auth {
    let pin = create_pin(&client, &config.client_id).await?;

    // Display to user (minimal TUI or stdout before TUI init)
    println!("Open this URL in your browser:");
    println!("https://app.plex.tv/auth#?clientID={}&code={}&context%5Bdevice%5D%5Bproduct%5D=TermTunes",
        config.client_id, pin.code);
    println!("\nWaiting for authentication...");

    // Poll until authenticated or timeout (5 minutes)
    let token = loop {
        tokio::time::sleep(Duration::from_secs(1)).await;
        let status = check_pin(&client, pin.id, &config.client_id).await?;
        if let Some(token) = status.auth_token {
            break token;
        }
    };

    // 4. Discover servers using the new token
    // GET https://plex.tv/api/v2/resources?includeHttps=1&includeRelay=1
    // Filter for type == "server", pick best connection
    // Save server config with name, URL, token, machine identifier

    save_config(&config)?;
}
```

### Complete Audio Playback Lifecycle (Verified Pattern)

```rust
// Source: docs.rs/rodio/0.21.1 (Sink API verified)

use rodio::{Decoder, OutputStream, Sink};
use std::io::Cursor;

struct Player {
    _stream: OutputStream,  // Must live as long as Sink
    sink: Sink,
    audio_data: Option<Vec<u8>>,  // Keep for potential re-creation
}

impl Player {
    fn new() -> Result<Self> {
        let stream = OutputStream::try_default()?;
        let sink = Sink::connect_new(&stream.mixer());
        Ok(Player {
            _stream: stream,
            sink,
            audio_data: None,
        })
    }

    fn load_and_play(&mut self, audio_bytes: Vec<u8>) -> Result<()> {
        self.sink.stop();  // Clear any existing playback
        self.audio_data = Some(audio_bytes.clone());
        let cursor = Cursor::new(audio_bytes);
        let source = Decoder::new(cursor)?;
        self.sink.append(source);
        // Sink auto-plays when source is appended
        Ok(())
    }

    fn toggle_pause(&self) {
        if self.sink.is_paused() {
            self.sink.play();
        } else {
            self.sink.pause();
        }
    }

    fn is_finished(&self) -> bool {
        self.sink.empty()
    }
}
```

### Plex Playlist + Track Fetching (Verified Pattern)

```rust
// Source: plexapi.dev, plexopedia.com (verified endpoints)

use serde::Deserialize;

// GET {server_url}/playlists?playlistType=audio
// Headers: X-Plex-Token, Accept: application/json
#[derive(Deserialize)]
struct PlaylistContainer {
    #[serde(rename = "MediaContainer")]
    media_container: PlaylistMediaContainer,
}

#[derive(Deserialize)]
struct PlaylistMediaContainer {
    #[serde(rename = "Metadata", default)]
    metadata: Vec<Playlist>,
}

#[derive(Deserialize)]
struct Playlist {
    #[serde(rename = "ratingKey")]
    rating_key: String,
    title: String,
    #[serde(rename = "leafCount")]
    leaf_count: Option<u32>,
    duration: Option<u64>,
}

// GET {server_url}/playlists/{ratingKey}/items
// Headers: X-Plex-Token, Accept: application/json
#[derive(Deserialize)]
struct Track {
    #[serde(rename = "ratingKey")]
    rating_key: String,
    title: String,
    #[serde(rename = "grandparentTitle")]
    artist: Option<String>,
    #[serde(rename = "parentTitle")]
    album: Option<String>,
    duration: Option<u64>,
    #[serde(rename = "Media")]
    media: Vec<Media>,
}

#[derive(Deserialize)]
struct Media {
    #[serde(rename = "Part")]
    parts: Vec<Part>,
}

#[derive(Deserialize)]
struct Part {
    key: String,  // e.g., "/library/parts/12345/1234567890/file.flac"
}

// Construct stream URL:
// {server_url}{part.key}?X-Plex-Token={token}
// e.g., http://192.168.1.100:32400/library/parts/12345/1234567890/file.flac?X-Plex-Token=abc123
```

### Minimal Status Bar UI (Claude's Discretion)

```rust
// Source: ratatui.rs official docs (verified)
use ratatui::widgets::Paragraph;
use ratatui::layout::{Layout, Constraint};
use ratatui::style::{Color, Style};

fn render_status_bar(frame: &mut Frame, track_name: &str, is_paused: bool) {
    let area = frame.area();

    // Reserve bottom line for status bar
    let [_main_area, status_area] = Layout::vertical([
        Constraint::Fill(1),
        Constraint::Length(1),
    ]).areas(area);

    let state_icon = if is_paused { "||" } else { ">>" };
    let status_text = format!(" {} {} ", state_icon, track_name);

    let status = Paragraph::new(status_text)
        .style(Style::default().fg(Color::White).bg(Color::DarkGray));

    frame.render_widget(status, status_area);
}
```

### CLI Menu for Playlist/Track Selection (Claude's Discretion)

```rust
// Recommendation: Use ratatui's List widget with ListState for selection.
// j/k or arrow keys to navigate, Enter to select, q to quit.
// This keeps everything in the TUI -- no separate dialoguer/inquire dependency needed.

use ratatui::widgets::{List, ListItem, ListState, Block};

struct MenuState {
    items: Vec<String>,
    list_state: ListState,
}

impl MenuState {
    fn new(items: Vec<String>) -> Self {
        let mut state = ListState::default();
        if !items.is_empty() {
            state.select(Some(0));
        }
        MenuState { items, list_state: state }
    }

    fn next(&mut self) {
        if let Some(i) = self.list_state.selected() {
            let next = (i + 1) % self.items.len();
            self.list_state.select(Some(next));
        }
    }

    fn previous(&mut self) {
        if let Some(i) = self.list_state.selected() {
            let prev = if i == 0 { self.items.len() - 1 } else { i - 1 };
            self.list_state.select(Some(prev));
        }
    }

    fn selected(&self) -> Option<&String> {
        self.list_state.selected().and_then(|i| self.items.get(i))
    }
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| ratatui manual init (Terminal::new + enable_raw_mode) | `ratatui::init()` / `ratatui::restore()` convenience functions | ratatui 0.28+ (2024) | Simplifies setup/teardown to one function call each. Still supports manual for custom needs. |
| Plex X-Plex-Token only | JWT authentication option | Plex 2025 ("API Unlocked") | JWT provides 7-day token refresh. PIN flow still works and is simpler. JWT is optional. |
| rodio OutputStream::try_default() returns tuple | OutputStream::try_default() returns single struct with .mixer() | rodio 0.20+ | API cleanup. Use `Sink::connect_new(&stream.mixer())` instead of old tuple pattern. |
| Manual crossterm enable_raw_mode / disable_raw_mode | `ratatui::init()` handles it internally | ratatui 0.28+ | Less boilerplate. init() calls enable_raw_mode + EnterAlternateScreen + cursor::Hide. |

**Deprecated/outdated:**
- `rodio::OutputStream::try_default()` returning `(OutputStream, OutputStreamHandle)` -- old tuple API. Current returns single `OutputStream` with `.mixer()`.
- `tui-rs` -- unmaintained since August 2023. Ratatui is the active fork.
- `plex-api` Rust crate -- v0.0.12, explicitly "not ready for any use."

## Open Questions

1. **WSL2 pause/resume reliability with rodio specifically**
   - What we know: WSLg PulseAudio has documented issues with pause/resume >5s (GitHub issues #1376, #607). The issue is in the PulseAudio shim, not in rodio.
   - What's unclear: Whether rodio's `Sink::pause()`/`Sink::play()` triggers the problematic PulseAudio cork/uncork, or whether rodio implements its own pause (stopping sample submission without corking the stream).
   - Recommendation: Build the PoC audio test as the FIRST task. Test 5s, 10s, 30s pause. If it fails, implement the workaround: store audio bytes + position, destroy sink/stream, recreate, seek to position, resume.

2. **Plex JSON response structure variations**
   - What we know: Plex API can return JSON when `Accept: application/json` is set. Response fields use camelCase.
   - What's unclear: Whether all Plex server versions consistently return the same JSON structure for playlist items. Older Plex versions may have different field names or nesting.
   - Recommendation: Implement serde deserialization with `#[serde(default)]` on optional fields. Test against the user's actual Plex server early. Use `serde_json::Value` for initial exploration, then create typed structs.

3. **Audio download memory usage for large files**
   - What we know: Downloading full track into `Vec<u8>` works for typical tracks (3-10 MB for MP3, 20-50 MB for FLAC).
   - What's unclear: Whether a 100 MB+ hi-res FLAC file causes noticeable memory pressure or download delay.
   - Recommendation: For Phase 1, full download is fine. Log track size during download. Revisit streaming approach in Phase 2 if memory usage is problematic.

4. **Config file permissions on WSL2**
   - What we know: The Plex token should not be world-readable. Standard practice is 600 permissions.
   - What's unclear: Whether WSL2 respects file permissions the same as native Linux (it depends on automount settings in /etc/wsl.conf).
   - Recommendation: Set file permissions to 0o600 after writing config. Log a warning if permissions are too open.

## Sources

### Primary (HIGH confidence)
- [rodio Sink API docs](https://docs.rs/rodio/latest/rodio/struct.Sink.html) -- Complete Sink method reference
- [rodio Decoder API docs](https://docs.rs/rodio/latest/rodio/decoder/struct.Decoder.html) -- Decoder construction, Read+Seek requirement
- [rodio crate overview](https://docs.rs/rodio/latest/rodio/) -- OutputStream, Source, playback architecture
- [ratatui panic hooks recipe](https://ratatui.rs/recipes/apps/panic-hooks/) -- Terminal restoration pattern
- [ratatui best practices discussion #220](https://github.com/ratatui/ratatui/discussions/220) -- Event loop, panic hooks, signal handling
- [ratatui GitHub](https://github.com/ratatui/ratatui) -- v0.30.0, init/restore convenience functions
- [signal-hook docs](https://docs.rs/signal-hook) -- Safe signal handling API
- [Plex auth forum post](https://forums.plex.tv/t/authenticating-with-plex/609370) -- Complete PIN flow with endpoints and headers
- [plexapi.dev - Get All Playlists](https://plexapi.dev/api-reference/playlists/get-all-playlists) -- GET /playlists endpoint, response schema
- [Plexopedia - Get Playlist Items](https://www.plexopedia.com/plex-media-server/api/playlists/view-items/) -- Track/Media/Part structure
- [Plexopedia - Download Media](https://www.plexopedia.com/plex-media-server/api/library/download-media-file/) -- Stream URL construction from Part.key

### Secondary (MEDIUM confidence)
- [rodio WSL2 issue #354](https://github.com/RustAudio/rodio/issues/354) -- rodio + WSL2 + PulseAudio chain: rodio -> cpal -> ALSA -> PulseAudio
- [rodio streaming issue #439](https://github.com/RustAudio/rodio/issues/439) -- Async stream playback requires custom approach
- [rodio streaming issue #159](https://github.com/RustAudio/rodio/issues/159) -- Custom Source or MediaSource for streaming
- [WSLg audio pause/resume issue #1376](https://github.com/microsoft/wslg/issues/1376) -- Documented PulseAudio cork/uncork failure
- [WSLg audio latency issue #607](https://github.com/microsoft/wslg/issues/607) -- Multiple corroborating reports
- [Plex JWT auth forum](https://forums.plex.tv/t/jwt-authentication/931646) -- New JWT option alongside traditional tokens
- [Plex resources endpoint discussion](https://forums.plex.tv/t/question-on-https-clients-plex-tv-api-v2-resources-and-jwt-authentication/934478) -- Server discovery via /api/v2/resources
- [Plexopedia - Server List](https://www.plexopedia.com/plex-media-server/api/server/list/) -- GET /servers endpoint

### Tertiary (LOW confidence)
- [WSLg PulseAudio 2025 issues](https://github.com/microsoft/wslg/issues/1378) -- Ongoing PulseAudio configuration problems on Ubuntu 24.04
- [Plex "API Unlocked" blog post](https://www.plex.tv/blog/plex-pro-week-25-api-unlocked/) -- New API documentation initiative, details unclear

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH -- all crate versions verified via docs.rs and lib.rs, APIs confirmed via official documentation
- Architecture: HIGH -- patterns verified from ratatui official recipes, rodio official examples, Plex forum documentation
- Plex API: MEDIUM-HIGH -- endpoints verified via plexapi.dev and Plexopedia, but JSON response structures may vary by Plex server version
- WSL2 audio: MEDIUM -- documented issues confirmed via GitHub issues, but actual rodio+WSL2 behavior needs hands-on validation
- Pitfalls: HIGH -- all pitfalls sourced from GitHub issues, official framework discussions, and Plex developer forums

**Research date:** 2026-02-10
**Valid until:** 2026-03-10 (stable domain: crate APIs, Plex API endpoints unlikely to change in 30 days)
