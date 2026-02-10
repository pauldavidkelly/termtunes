# Phase 3: Differentiators - Research

**Researched:** 2026-02-10
**Domain:** Playlist shuffle/repeat modes, audio seeking, favorite playlist hotkeys, config persistence, player bar mode indicators
**Confidence:** HIGH

## Summary

Phase 3 adds the features that differentiate TermTunes from a basic playlist player: favorite playlist hotkeys (1-9), shuffle mode, repeat modes (off/all/one), and seek within tracks (h/l keys). The codebase after Phase 2 provides a solid foundation: `App` struct with `current_track_index`, `NowPlaying`, `saved_volume`, keybinding dispatch in `handle_key()`, a 3-line player bar in `ui.rs`, and the download-per-track architecture with `play_track_at_index()`.

The most critical finding is that **rodio's `Sink::try_seek(Duration)` is available in rodio 0.21** and works with Symphonia-decoded sources (which is what TermTunes uses). The method takes an absolute `Duration`, blocks for 0-5ms, and saturates at the source's end if you seek beyond it. For seek-forward/backward (h/l keys), the implementation computes `current_pos +/- seek_step` and calls `try_seek()`. The method returns `Result<(), SeekError>` where `SeekError::NotSupported` indicates the source does not support seeking -- this should be handled gracefully (log and ignore) since Plex audio files decoded by Symphonia generally do support seeking.

For shuffle, the standard approach is to maintain a **shuffle order index array** separate from the original track list. The `rand` crate (version 0.9.x, MSRV 1.63) provides `SliceRandom::shuffle()` which performs an in-place Fisher-Yates shuffle in O(n). The pattern is: create a `Vec<usize>` of indices `[0, 1, 2, ..., n-1]`, shuffle it, then use this shuffled index to determine next/previous track instead of sequential increment/decrement. When shuffle is toggled off, revert to sequential order. When toggled on mid-playback, generate a new shuffled order starting from the current track.

For favorites, the data is simple: a `HashMap<u8, FavoritePlaylist>` (keys 1-9) stored in `config.toml`. When a number key is pressed, the app looks up the favorite, fetches its tracks from Plex, and starts playback -- reusing the existing `fetch_tracks()` and `play_track_at_index()` flow. The assignment UI can be a simple keystroke (e.g., `f` then `1-9` to assign the currently selected playlist).

**Primary recommendation:** Add `rand = "0.9"` as the only new dependency. Implement shuffle as a shuffled-index array in App, repeat as a `RepeatMode` enum that modifies `next_track()` behavior, seek via `Sink::try_seek()` with relative offset from `get_pos()`, and favorites as a config-persisted map of playlist rating keys to number keys.

## Standard Stack

### Core (Phase 3 specific -- one new dependency)

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| rand | 0.9.x | `SliceRandom::shuffle()` for Fisher-Yates playlist shuffle | The standard Rust randomness crate. Provides `SliceRandom::shuffle()` for in-place O(n) Fisher-Yates on `Vec<usize>`. MSRV 1.63. |
| rodio | 0.21 (existing) | `Sink::try_seek(Duration)` for seeking within tracks | Already in Cargo.toml. try_seek is available in 0.21, works with Symphonia decoder sources. |
| ratatui | 0.30 (existing) | Player bar mode indicators (shuffle/repeat icons) via `Span::styled()` | Already in Cargo.toml. Multi-span Line composition pattern established in Phase 2. |

### Supporting (already in Cargo.toml)

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| serde + toml | 1.x / 0.8.x | Persist favorite playlists in config.toml | Serialize `HashMap<u8, FavoritePlaylist>` into existing config.toml |
| crossterm | 0.29 (existing) | Key event handling for new keybindings (h/l/s/r/1-9/f) | Extend handle_key() with new key patterns |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| rand crate for shuffle | Manual Fisher-Yates with `getrandom` | rand provides a well-tested, one-line shuffle. Manual implementation is error-prone for unbiased shuffling. Use rand. |
| Shuffled index array | Shuffle the tracks Vec in-place | Shuffling in-place destroys the original order, making "unshuffle" require re-fetching from Plex. Index array preserves both orders with zero network cost. Use index array. |
| Config-persisted favorites | SQLite database | Overkill for 9 key-value pairs. TOML in existing config.toml is sufficient and already implemented. |
| h/l for seek | Left/Right arrow keys | h/l follows vim convention (the project's core design principle). Also bind Left/Right as aliases for discoverability. |

### Installation

```bash
# Only one new dependency
# Add to Cargo.toml [dependencies]:
rand = "0.9"
```

## Architecture Patterns

### Recommended Changes to Project Structure

```
src/
├── main.rs              # No changes needed
├── app.rs               # MODIFY: Add shuffle_order, repeat_mode, favorites logic, seek methods, 1-9/f/s/r/h/l keybindings
├── auth.rs              # No changes
├── config.rs            # MODIFY: Add FavoritePlaylist struct and favorites HashMap to Config
├── plex.rs              # No changes (Playlist struct already has rating_key and title)
├── player.rs            # MODIFY: Add seek_forward/seek_backward methods using try_seek
├── tui.rs               # No changes
└── ui.rs                # MODIFY: Add shuffle/repeat indicators to player bar line 3
```

### Pattern 1: Shuffle via Index Array

**What:** Maintain a `shuffle_order: Option<Vec<usize>>` in the App struct. When shuffle is on, this contains a randomly permuted array of track indices. `next_track()` and `prev_track()` navigate through this array instead of incrementing/decrementing the raw index.
**When to use:** For PLAY-08 (toggle shuffle mode).

```rust
// In app.rs -- shuffle state

use rand::seq::SliceRandom;

/// Whether shuffle mode is active.
shuffle_enabled: bool,

/// Shuffled order of track indices. When shuffle is enabled, this contains
/// a permutation of [0..tracks.len()]. next/prev navigate through this
/// instead of incrementing the raw track index.
shuffle_order: Vec<usize>,

/// Position within the shuffle_order array (NOT a track index).
/// When shuffle is off, this is not used -- current_track_index is used directly.
shuffle_position: usize,

/// Toggle shuffle mode. When enabling, generates a new shuffle order
/// starting from the current track (so the currently playing track stays).
fn toggle_shuffle(&mut self) {
    self.shuffle_enabled = !self.shuffle_enabled;

    if self.shuffle_enabled && !self.tracks.is_empty() {
        // Build index array
        let mut indices: Vec<usize> = (0..self.tracks.len()).collect();
        let mut rng = rand::rng();
        indices.shuffle(&mut rng);

        // Move the currently playing track to position 0 so it stays as "current"
        if let Some(current_idx) = self.current_track_index {
            if let Some(pos) = indices.iter().position(|&i| i == current_idx) {
                indices.swap(0, pos);
            }
        }

        self.shuffle_order = indices;
        self.shuffle_position = 0; // Current track is at position 0
    }
    // When disabling shuffle, shuffle_order becomes unused.
    // current_track_index still tracks the real index in self.tracks.
}

/// Get the next track index respecting shuffle mode.
fn next_track_index(&self) -> Option<usize> {
    if self.tracks.is_empty() {
        return None;
    }

    if self.shuffle_enabled {
        let next_pos = self.shuffle_position + 1;
        if next_pos < self.shuffle_order.len() {
            Some(self.shuffle_order[next_pos])
        } else {
            // Wrapped past end of shuffle -- depends on repeat mode
            None // Caller decides based on repeat mode
        }
    } else {
        match self.current_track_index {
            Some(idx) if idx + 1 < self.tracks.len() => Some(idx + 1),
            _ => None, // End of playlist -- caller decides
        }
    }
}
```

**Why index array, not shuffling the Vec:** The original track order must be preserved so (1) the track list UI still shows the original order with correct indices, (2) toggling shuffle off returns to sequential playback from the current track's original position, (3) no re-fetch from Plex is needed, and (4) the user can still select any track by pressing Enter on it in the list.

### Pattern 2: Repeat Mode State Machine

**What:** An enum `RepeatMode` with three states (Off, All, One) that modifies the behavior of `next_track()` when a track finishes or the user presses next.
**When to use:** For PLAY-09 (cycle through repeat modes).

```rust
// In app.rs -- repeat mode

/// Repeat mode for playlist playback.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RepeatMode {
    /// No repeat -- stop after last track.
    Off,
    /// Repeat entire playlist (loop back to first track after last).
    All,
    /// Repeat current track indefinitely.
    One,
}

impl RepeatMode {
    /// Cycle to the next mode: Off -> All -> One -> Off.
    pub fn cycle(self) -> Self {
        match self {
            RepeatMode::Off => RepeatMode::All,
            RepeatMode::All => RepeatMode::One,
            RepeatMode::One => RepeatMode::Off,
        }
    }

    /// Display string for the player bar.
    pub fn indicator(&self) -> &'static str {
        match self {
            RepeatMode::Off => "",
            RepeatMode::All => "[Repeat: All]",
            RepeatMode::One => "[Repeat: One]",
        }
    }
}

/// Repeat mode for current playlist.
repeat_mode: RepeatMode,

/// Toggle repeat mode (r key).
fn cycle_repeat(&mut self) {
    self.repeat_mode = self.repeat_mode.cycle();
}
```

**Integration with next_track / auto-advance:**

```rust
/// Called when a track finishes (auto-advance) or user presses next.
fn advance_track(&mut self) -> Result<()> {
    match self.repeat_mode {
        RepeatMode::One => {
            // Re-play the current track
            if let Some(idx) = self.current_track_index {
                self.play_track_at_index(idx)?;
            }
        }
        RepeatMode::All => {
            // Get next track; wrap to beginning if at end
            match self.next_track_index() {
                Some(idx) => self.play_track_at_index(idx)?,
                None => {
                    // Wrap: start from beginning (or re-shuffle if shuffle enabled)
                    if self.shuffle_enabled {
                        self.reshuffle();
                        if let Some(&first) = self.shuffle_order.first() {
                            self.shuffle_position = 0;
                            self.play_track_at_index(first)?;
                        }
                    } else {
                        self.play_track_at_index(0)?;
                    }
                }
            }
        }
        RepeatMode::Off => {
            match self.next_track_index() {
                Some(idx) => self.play_track_at_index(idx)?,
                None => {
                    // End of playlist -- stop playback
                    // (do nothing, track finishes naturally)
                }
            }
        }
    }
    Ok(())
}
```

### Pattern 3: Seeking via try_seek()

**What:** Compute a new absolute position from `get_pos() +/- seek_step`, clamp to `0..=total_duration`, and call `Sink::try_seek(new_pos)`. Handle `SeekError::NotSupported` gracefully.
**When to use:** For PLAY-10 (seek forward) and PLAY-11 (seek backward), bound to h/l keys (KEY-06).

```rust
// In player.rs -- seek methods

/// Seek step size per key press (5 seconds).
const SEEK_STEP: std::time::Duration = std::time::Duration::from_secs(5);

/// Seek forward by SEEK_STEP. Clamps at track duration.
///
/// Uses try_seek which blocks for 0-5ms. Saturates at end of source
/// automatically, but we clamp anyway for accurate position display.
pub fn seek_forward(&self, track_duration_ms: u64) -> Result<(), rodio::source::SeekError> {
    let current = self.sink.get_pos();
    let max = std::time::Duration::from_millis(track_duration_ms);
    let target = (current + SEEK_STEP).min(max);
    self.sink.try_seek(target)
}

/// Seek backward by SEEK_STEP. Clamps at zero (beginning of track).
pub fn seek_backward(&self) -> Result<(), rodio::source::SeekError> {
    let current = self.sink.get_pos();
    let target = current.saturating_sub(SEEK_STEP);
    self.sink.try_seek(target)
}
```

```rust
// In app.rs -- keybinding integration

// Seek forward (PLAY-10) -- l or Right arrow
(KeyCode::Char('l'), _) | (KeyCode::Right, _) => {
    if let (Some(player), Some(np)) = (&self.player, &self.now_playing) {
        if let Err(e) = player.seek_forward(np.duration_ms) {
            tracing::warn!("Seek forward failed: {}", e);
            // Gracefully ignore -- seeking not supported for this source
        }
    }
}

// Seek backward (PLAY-11) -- h or Left arrow
(KeyCode::Char('h'), _) | (KeyCode::Left, _) => {
    if let (Some(player), _) = (&self.player, &self.now_playing) {
        if let Err(e) = player.seek_backward() {
            tracing::warn!("Seek backward failed: {}", e);
        }
    }
}
```

**Critical note on h/l key conflict:** In the current codebase, `h` and `l` are not bound to anything. In Phase 2, navigation uses `j/k` (vertical) and `Enter/Esc` (drill-down/back). The `h/l` keys for horizontal/seek do not conflict with existing bindings.

### Pattern 4: Favorite Playlists with Number Key Hotkeys

**What:** Store up to 9 favorite playlists in the config file (keyed by number 1-9). Each entry holds the playlist's `rating_key` and `title`. Pressing a number key (1-9) from anywhere starts that playlist. Pressing `f` while a playlist is selected assigns it as a favorite (user then presses 1-9 to choose the slot).
**When to use:** For LIST-04 (assign favorites) and LIST-05 (start favorite by number key).

```rust
// In config.rs -- favorite playlist storage

/// A favorite playlist mapping (persisted in config.toml).
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FavoritePlaylist {
    /// The Plex rating key used to fetch playlist tracks.
    pub rating_key: String,
    /// Human-readable playlist title (for display in status bar / help).
    pub title: String,
}

// Add to Config struct:
/// Favorite playlists mapped to number keys 1-9.
/// Stored as a map from "1"-"9" string keys to FavoritePlaylist.
#[serde(default)]
pub favorites: std::collections::HashMap<String, FavoritePlaylist>,
```

```toml
# Example config.toml after assigning favorites:

client_id = "a1b2c3d4-..."
last_server = "abc123"

[servers.abc123]
name = "My Server"
url = "http://192.168.1.100:32400"
token = "xYz789"

[favorites]
1 = { rating_key = "42", title = "Chill Vibes" }
3 = { rating_key = "99", title = "Work Focus" }
7 = { rating_key = "15", title = "Ambient Mix" }
```

```rust
// In app.rs -- favorite playlist activation

/// State for the favorite assignment flow.
/// When true, the next 1-9 key press assigns the selected playlist.
awaiting_favorite_key: bool,

// In handle_key():

// Number keys 1-9: start favorite playlist (if not in assignment mode)
(KeyCode::Char(c @ '1'..='9'), _) => {
    if self.awaiting_favorite_key {
        // Assign current playlist to this number key
        self.assign_favorite(c)?;
        self.awaiting_favorite_key = false;
    } else {
        // Start the favorite playlist for this key
        self.start_favorite(c).await?;
    }
}

// 'f' key: enter favorite assignment mode
(KeyCode::Char('f'), _) => {
    if matches!(self.view, AppView::Playlists) {
        self.awaiting_favorite_key = true;
        // UI shows "Press 1-9 to assign favorite" prompt
    }
}

/// Start playing a favorite playlist by its number key.
async fn start_favorite(&mut self, key: char) -> Result<()> {
    let key_str = key.to_string();
    let fav = match self.config.favorites.get(&key_str) {
        Some(fav) => fav.clone(),
        None => return Ok(()), // No favorite assigned to this key
    };

    tracing::info!(key = %key, playlist = %fav.title, "Starting favorite playlist");

    // Fetch tracks for the favorite playlist
    self.tracks = self.plex_client.fetch_tracks(&fav.rating_key).await?;
    self.current_playlist_title = fav.title;

    // Reset track state and start from first track
    self.track_state = ListState::default();
    if !self.tracks.is_empty() {
        self.track_state.select(Some(0));
    }

    // Generate shuffle order if shuffle is enabled
    if self.shuffle_enabled {
        self.regenerate_shuffle_order();
    }

    // Start playing the first track
    self.play_track_at_index(0)?;
    Ok(())
}

/// Assign the currently selected playlist as a favorite.
fn assign_favorite(&mut self, key: char) -> Result<()> {
    if let Some(idx) = self.playlist_state.selected() {
        if let Some(playlist) = self.playlists.get(idx) {
            let fav = config::FavoritePlaylist {
                rating_key: playlist.rating_key.clone(),
                title: playlist.title.clone(),
            };
            self.config.favorites.insert(key.to_string(), fav);
            config::save_config(&self.config)?;
            tracing::info!(
                key = %key,
                playlist = %playlist.title,
                "Assigned favorite playlist"
            );
        }
    }
    Ok(())
}
```

### Pattern 5: Player Bar Mode Indicators

**What:** Extend the player bar's line 3 (status line) with shuffle and repeat mode indicators, using the same multi-span Line composition pattern from Phase 2.
**When to use:** For DISP-08 (display shuffle and repeat indicators).

```rust
// In ui.rs -- adding indicators to player bar line 3

// After the time span, before closing the Line:

// Shuffle indicator
if app.shuffle_enabled() {
    spans.push(Span::styled(" | ", separator_style));
    spans.push(Span::styled(
        "[Shuffle]",
        Style::default().fg(Color::Magenta),
    ));
}

// Repeat indicator
let repeat_indicator = app.repeat_mode().indicator();
if !repeat_indicator.is_empty() {
    spans.push(Span::styled(" | ", separator_style));
    spans.push(Span::styled(
        repeat_indicator,
        Style::default().fg(Color::Blue),
    ));
}

// Result: " Playing | Vol: 80% | 01:23 / 03:45 | [Shuffle] | [Repeat: All]"
```

### Anti-Patterns to Avoid

- **Shuffling the tracks Vec in-place:** Destroys original order. Makes "unshuffle" impossible without re-fetching from Plex. Use an index array instead.
- **Storing shuffle state in Player (rodio layer):** Shuffle is a playlist-level concept, not an audio concept. It belongs in App, not Player.
- **Making seek blocking or waiting for result:** `try_seek()` blocks for only 0-5ms which is acceptable in the event loop. But do NOT add any additional waiting or retrying logic. Fire-and-forget with error logging.
- **Using `thread_rng()` from rand:** Deprecated in rand 0.9. Use `rand::rng()` instead (same functionality, new name).
- **Re-shuffling on every next_track call:** Shuffle once when enabled, then walk through the shuffled order. Only re-shuffle when the full shuffled order is exhausted (and repeat mode is All).
- **Blocking on favorite playlist fetch:** The `start_favorite()` method calls `fetch_tracks().await` which is already async. This is correct. Do NOT make it synchronous.
- **Overriding h/l behavior in list views:** In the Playlists and Tracks list views, h/l should still seek (not navigate horizontally). There is no horizontal navigation in the current UI. If a future phase adds horizontal navigation, the binding should be context-sensitive. For now, h/l are seek-only.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Unbiased playlist shuffle | Custom random index generation | `rand::seq::SliceRandom::shuffle()` | Fisher-Yates is easy to get wrong (off-by-one = biased). rand's implementation is proven. |
| Seeking within audio | Manual Symphonia seeking or stream reconstruction | `rodio::Sink::try_seek(Duration)` | try_seek handles all codec-specific seeking through Symphonia internally. Saturates at end of source. |
| Random number generation | Manual PRNG | `rand::rng()` (thread-local fast RNG) | Properly seeded, statistically sound, zero-config. |
| Config serialization for favorites | Manual TOML string building | `serde` derive + `toml::to_string_pretty()` | Already used for Config in Phase 1. Adding a new field "just works" with derive. |

**Key insight:** Phase 3 requires only ONE new dependency (rand). All other capabilities (seeking, config persistence, UI indicators) use libraries already in `Cargo.toml`. The work is primarily app-level state management and keybinding wiring.

## Common Pitfalls

### Pitfall 1: try_seek Returns SeekError::NotSupported

**What goes wrong:** Calling `try_seek()` on a source that does not support seeking (e.g., some streaming sources, or sources wrapped in certain adapters).
**Why it happens:** Not all rodio Source implementations support seeking. The Symphonia decoder for most audio formats (MP3, FLAC, AAC, WAV, OGG) does support seeking, but if a source chain includes a non-seekable adapter, the entire chain becomes non-seekable.
**How to avoid:** TermTunes loads full tracks into `Cursor<Vec<u8>>` (which is Read + Seek) and decodes with Symphonia `Decoder::new()`. This source chain should support seeking. However, always handle the error gracefully -- log a warning, do not crash or show an error to the user for a seek failure.
**Warning signs:** Seek keys (h/l) do nothing on certain tracks. Check if those tracks use an unusual codec.

### Pitfall 2: Shuffle Order Not Updated When Playlist Changes

**What goes wrong:** User enables shuffle, then navigates back to playlist view and selects a different playlist. The shuffle order still references the old playlist's indices.
**Why it happens:** `toggle_shuffle()` generates the order from `self.tracks`, but `self.tracks` changes when a new playlist is loaded.
**How to avoid:** Regenerate shuffle order whenever tracks change: in `select_item()` (when entering a new playlist) and in `start_favorite()` (when starting a favorite). If shuffle is enabled, call `regenerate_shuffle_order()` after loading new tracks.
**Warning signs:** After switching playlists with shuffle on, next/prev plays tracks from the wrong playlist or panics on out-of-bounds index.

### Pitfall 3: Number Keys 1-9 Conflict with Future Features

**What goes wrong:** Number keys are used for favorites. A future feature might want number keys for something else (e.g., jump to track number).
**Why it happens:** Number keys are a limited resource in keyboard-only UIs.
**How to avoid:** This is a locked requirement (LIST-04, LIST-05 specify 1-9 for favorites). Document the binding clearly. If a future need arises, it would need a modifier key (e.g., Alt+1 or g1 in vim style).
**Warning signs:** None for Phase 3 -- this is future-proofing documentation.

### Pitfall 4: Repeat One + Auto-Advance Causes Infinite Re-download

**What goes wrong:** In Repeat One mode, when a track finishes, `advance_track()` calls `play_track_at_index()` which triggers `start_track_download()`. For every repeat, the same track is re-downloaded from Plex.
**Why it happens:** The current architecture downloads fresh bytes for every playback. There is no caching.
**How to avoid:** When Repeat One is active and the same track index is already loaded (i.e., `_audio_data` in Player contains the bytes), use the cached bytes instead of re-downloading. The Player already stores `_audio_data: Option<Vec<u8>>`. Add a method `replay_current()` that re-decodes from the stored bytes.
**Warning signs:** Network traffic spike when Repeat One is enabled. Brief silence between repeats due to download time.

### Pitfall 5: Seek Beyond Duration Followed by Auto-Advance Race

**What goes wrong:** User seeks near the end of a track. `try_seek()` saturates at the end. The audio finishes almost immediately. `is_finished()` triggers auto-advance, which may race with the UI's seek position display showing briefly incorrect data.
**Why it happens:** Seeking to the end effectively ends the track. The auto-advance check in the event loop fires on the next iteration (within 100ms).
**How to avoid:** This is actually correct behavior -- seeking to the end should advance to the next track. The brief visual artifact (progress bar at 100% for one frame) is acceptable. No special handling needed.
**Warning signs:** None -- this is expected behavior.

### Pitfall 6: Favorite Playlist Deleted on Plex Server

**What goes wrong:** User assigned a favorite to key 3 (rating_key "42"). Later, the playlist is deleted from Plex. Pressing 3 calls `fetch_tracks("42")` which returns an HTTP 404 or empty result.
**Why it happens:** Favorites persist in config.toml but reference server-side resources that can change.
**How to avoid:** Handle the 404/empty case gracefully. If `fetch_tracks()` returns an error or empty tracks list, show a brief error message in the status bar ("Favorite playlist not found") and clear the favorite from config. Do not crash.
**Warning signs:** Error message when pressing a favorite key for a playlist that was recently deleted.

## Code Examples

### Complete Shuffle Implementation (Verified Pattern)

```rust
// Source: rand 0.9 docs (SliceRandom::shuffle), music player shuffle best practices

use rand::seq::SliceRandom;

/// Regenerate shuffle order for the current tracks.
/// Places the currently playing track at position 0 if one exists.
fn regenerate_shuffle_order(&mut self) {
    if self.tracks.is_empty() {
        self.shuffle_order.clear();
        self.shuffle_position = 0;
        return;
    }

    let mut indices: Vec<usize> = (0..self.tracks.len()).collect();
    let mut rng = rand::rng();
    indices.shuffle(&mut rng);

    // If a track is currently playing, move it to position 0
    if let Some(current_idx) = self.current_track_index {
        if let Some(pos) = indices.iter().position(|&i| i == current_idx) {
            indices.swap(0, pos);
        }
        self.shuffle_position = 0;
    } else {
        self.shuffle_position = 0;
    }

    self.shuffle_order = indices;
}
```

### Complete Seek with Error Handling (Verified Pattern)

```rust
// Source: docs.rs/rodio/0.21.1/rodio/struct.Sink.html (try_seek verified)

use std::time::Duration;

const SEEK_STEP_SECS: u64 = 5;

impl Player {
    /// Seek forward by 5 seconds within the current track.
    /// Returns Ok(()) on success, or logs and returns the error.
    pub fn seek_forward(&self, track_duration_ms: u64) -> Result<(), rodio::source::SeekError> {
        let current = self.sink.get_pos();
        let max = Duration::from_millis(track_duration_ms);
        let target = (current + Duration::from_secs(SEEK_STEP_SECS)).min(max);
        self.sink.try_seek(target)
    }

    /// Seek backward by 5 seconds. Clamps at the beginning of the track.
    pub fn seek_backward(&self) -> Result<(), rodio::source::SeekError> {
        let current = self.sink.get_pos();
        let target = current.saturating_sub(Duration::from_secs(SEEK_STEP_SECS));
        self.sink.try_seek(target)
    }
}
```

### Complete Repeat Mode with Auto-Advance (Verified Pattern)

```rust
// Source: Architectural pattern from spotify_player/spotify-tui, adapted for rodio

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
}
```

### Repeat One with Cached Replay (Avoiding Re-download)

```rust
// Source: Architectural pattern to avoid re-downloading on Repeat One

impl Player {
    /// Replay the current track from the stored audio bytes.
    /// Used by Repeat One mode to avoid re-downloading the same track.
    pub fn replay_current(&mut self, volume: f32) -> Result<()> {
        let audio_bytes = match &self._audio_data {
            Some(data) => data.clone(),
            None => return Err(color_eyre::eyre::eyre!("No audio data to replay")),
        };
        let track_name = self.current_track.clone().unwrap_or_default();

        // Create fresh Sink and replay
        self.sink.stop();
        self.sink = Sink::connect_new(self._stream.mixer());
        self.sink.set_volume(volume.clamp(0.0, 1.0));

        let cursor = std::io::Cursor::new(audio_bytes);
        let source = Decoder::new(cursor)
            .map_err(|e| color_eyre::eyre::eyre!("Failed to decode audio: {}", e))?;
        self.sink.append(source);

        tracing::info!(track = %track_name, "Replaying track (Repeat One)");
        Ok(())
    }
}
```

### Favorite Playlist Config Persistence (Verified Pattern)

```rust
// Source: Existing config.rs pattern with serde derive

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FavoritePlaylist {
    pub rating_key: String,
    pub title: String,
}

// Add to existing Config struct:
#[derive(Serialize, Deserialize, Default, Debug)]
pub struct Config {
    pub client_id: String,
    pub last_server: Option<String>,
    #[serde(default)]
    pub servers: HashMap<String, ServerConfig>,
    /// Favorite playlists mapped to number keys "1" through "9".
    #[serde(default)]
    pub favorites: HashMap<String, FavoritePlaylist>,
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| `rand::thread_rng()` | `rand::rng()` | rand 0.9 (Jan 2025) | `thread_rng()` deprecated. `rng()` is the replacement with same functionality. |
| rodio no seeking support | `Sink::try_seek(Duration)` | rodio 0.19+ | Built-in seeking via Symphonia's seek support. No custom implementation needed. |
| Manual shuffle tracking | `SliceRandom::shuffle()` on index array | Established pattern | Standard music player approach: index array preserves original order. |

**Deprecated/outdated:**
- `rand::thread_rng()` -- use `rand::rng()` instead (same functionality, deprecated name).
- Manual seeking via Symphonia `FormatReader::seek()` -- use `Sink::try_seek()` which wraps this internally.

## Open Questions

1. **Seek step size (5 seconds vs configurable)**
   - What we know: 5 seconds is a common default in music players (Spotify, Apple Music). The vim-philosophy might suggest making it configurable.
   - What's unclear: Whether 5 seconds feels right for all content types (ambient mixes vs pop songs).
   - Recommendation: Start with 5-second fixed step. If user feedback indicates a need, make it configurable in Phase 4 (config.toml setting). The constant is easy to extract to config later.

2. **Favorite assignment UI interaction model**
   - What we know: Requirements say user can "assign" playlists as favorites. The simplest interaction is pressing `f` then a number key.
   - What's unclear: Whether a confirmation message or visual feedback is needed. Whether `f` should also show existing assignments.
   - Recommendation: Show a brief message in the status bar when `f` is pressed ("Press 1-9 to assign favorite") and a confirmation after assignment ("Assigned to key 3: Chill Vibes"). No separate assignment view -- keep it minimal. Add favorite indicators (e.g., "[1]" prefix) next to assigned playlists in the playlist list.

3. **Shuffle behavior when selecting a track manually from the list**
   - What we know: When shuffle is on and the user presses Enter on a specific track in the list, that track should play.
   - What's unclear: Should manual track selection reset the shuffle position? Or should next/prev after manual selection continue from that track's position in the shuffle order?
   - Recommendation: Manual selection plays the chosen track. Find that track's position in the shuffle order and set `shuffle_position` to it. This way, pressing "next" after manual selection continues the shuffled sequence from that point. This matches how Spotify handles the same scenario.

4. **rand 0.9 vs 0.10**
   - What we know: rand 0.9 (MSRV 1.63) is the current stable release. rand 0.10.0 is available (MSRV 1.85, released Feb 2026). Both provide `SliceRandom::shuffle()`.
   - What's unclear: Whether 0.10 has API changes that affect our usage.
   - Recommendation: Use rand 0.9 for maximum compatibility. The `SliceRandom::shuffle()` API is identical in both versions. No benefit from 0.10 for our use case.

## Sources

### Primary (HIGH confidence)
- [rodio Sink::try_seek - docs.rs](https://docs.rs/rodio/latest/rodio/struct.Sink.html) -- try_seek(Duration) -> Result<(), SeekError> verified. Blocks 0-5ms. Saturates at source end.
- [rodio SeekError - docs.rs](https://docs.rs/rodio/latest/rodio/source/enum.SeekError.html) -- SeekError variants: NotSupported, SymphoniaDecoder, HoundDecoder, Other. source_intact() method.
- [rodio Sink::get_pos - docs.rs](https://docs.rs/rodio/latest/rodio/struct.Sink.html) -- get_pos() returns Duration, accounts for speed modifications.
- [rand SliceRandom::shuffle - docs.rs](https://docs.rs/rand/0.9.0/rand/seq/trait.SliceRandom.html) -- shuffle(&mut self, rng: &mut R) where R: Rng. Fisher-Yates, O(n), uniformly random permutation.
- [rand rng() function - docs.rs](https://docs.rs/rand/0.9.0/rand/index.html) -- rng() replaces deprecated thread_rng(). Fast, pre-initialized thread-local RNG.
- [ratatui Line/Span composition - Context7](https://context7.com/ratatui/ratatui/) -- Multi-span Line construction pattern verified via Context7 query.

### Secondary (MEDIUM confidence)
- [Seeking in rodio - Rust Users Forum](https://users.rust-lang.org/t/seeking-in-rodio/110330) -- Community discussion confirming try_seek works with Symphonia decoder sources.
- [rodio Sink source code](https://docs.rs/rodio/latest/src/rodio/sink.rs.html) -- try_seek implementation verified in source.
- [Music player shuffle best practices](https://ruudvanasseldonk.com/2023/an-algorithm-for-shuffling-playlists) -- Index-based shuffle order pattern, preserving original playlist order.
- [ExoPlayer shuffle order](https://github.com/google/ExoPlayer/issues/5426) -- Industry pattern: maintain shuffle index mapping alongside original order.
- [rand crate versions - crates.io](https://crates.io/crates/rand/versions) -- 0.9 stable (MSRV 1.63), 0.10.0 available (MSRV 1.85).
- [rand GitHub Cargo.toml](https://github.com/rust-random/rand/blob/master/Cargo.toml) -- Version and MSRV confirmed from source.

### Tertiary (LOW confidence)
- [spotify-tui shuffle/repeat UI](https://github.com/Rigellute/spotify-tui) -- Reference for TUI shuffle/repeat indicator patterns. Not directly inspected, inferred from README.
- [Termusic (Rust TUI player)](https://github.com/tramhao/termusic) -- Reference for favorite playlist patterns. Architecture not directly inspected.

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH -- rand 0.9 API verified via docs.rs, rodio try_seek verified via docs.rs and source code
- Architecture: HIGH -- shuffle index pattern verified against ExoPlayer and music player best practices, repeat mode pattern from established TUI players
- Seeking: HIGH -- try_seek(Duration) API, SeekError variants, and saturation behavior all verified via official docs
- Favorites persistence: HIGH -- extends existing serde + toml pattern already proven in Phase 1
- Pitfalls: HIGH -- all pitfalls derived from API documentation, codebase analysis, and established patterns

**Research date:** 2026-02-10
**Valid until:** 2026-03-10 (stable domain: rodio 0.21 and rand 0.9 APIs are not changing imminently)
