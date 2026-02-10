# Architecture for v1.1: Multi-Channel Audio & Track Browsing

**Project:** TermTunes v1.1
**Researched:** 2026-02-10
**Confidence:** HIGH

## Context

TermTunes v1.0 is a working 3,507-line Rust TUI music player. The architecture is established
and proven: single-threaded event loop in app.rs, synchronous player.rs wrapping rodio, async
Plex API calls in plex.rs, ratatui rendering in ui.rs. v1.1 extends this architecture without
changing its fundamentals.

This document covers the architectural changes needed for:
1. A second audio channel (ambient Sink) on the existing OutputStream
2. Track browsing from Plex music library sections
3. Integration with the existing App state machine and event loop
4. Session persistence for ambient state

## Current Architecture (v1.0)

```
+------------------------------------------------------------------+
|                        Event Loop (app.rs)                        |
|  100ms poll cycle: handle keys, check downloads, update viz, draw |
+-----+----+----------+------------+------------+------------------+
      |    |          |            |            |
      v    v          v            v            v
  [Input]  [Timer]  [Downloads]  [Playback]  [Render]
  handle   check    check mpsc   auto-adv    terminal
  keys     signals  try_recv()   on empty    draw(ui)
      |               |            |
      v               v            v
+----------+   +----------+   +---------+
| plex.rs  |   | player.rs|   | ui.rs   |
| PlexAPI  |   | rodio    |   | ratatui |
| reqwest  |   | Sink     |   | widgets |
+----------+   +----------+   +---------+
                    |
              [OutputStream]
              [ALSA/Pulse]
```

### Current Audio Architecture (Single Channel)

```
OutputStream (one instance, created in Player::new())
    |
    +-- mixer() --> Sink (recreated per track via Sink::connect_new)
                       |
                       +-- VisualizerSource<Decoder<Cursor<Vec<u8>>>>
```

### Key Existing Patterns (preserved in v1.1)

| Pattern | How It Works | v1.1 Impact |
|---------|-------------|-------------|
| Download-then-play | `std::thread::spawn` + `reqwest::blocking::get` -> mpsc -> decode -> Sink.append | Ambient uses same pattern with separate mpsc channel |
| Lazy Player init | `player: Option<Player>`, created on first track play | Player now holds `Option<Sink>` for ambient too |
| Event loop polling | `event::poll(100ms)` + `try_recv()` for downloads | Add `ambient_download_rx.try_recv()` check |
| Volume persistence | `saved_volume: f32` on App, applied to new Sinks | Add `ambient_volume: f32` with `[/]` keybindings |
| Visualizer tap | `VisualizerSource` wraps main source, copies samples | Remains on main channel ONLY -- ambient excluded |
| Session restore | `session.toml` with playlist/track/volume/shuffle/repeat | Extend with `ambient_*` fields |
| Sink recreation | New Sink per track via `Sink::connect_new(stream.mixer())` | Ambient Sink follows same pattern |

### Current Key Binding Map (for conflict checking)

```
q           Quit                    Ctrl+C      Quit
Space       Toggle play/pause       j/Down      Navigate down
k/Up        Navigate up             Enter       Select item
Esc/Bksp    Go back                 n/>         Next track
N/<         Previous track          +/=         Volume up (main)
-/_         Volume down (main)      l/Right     Seek forward
h/Left      Seek backward           s           Toggle shuffle
r           Cycle repeat mode       v           Toggle visualizer
f           Start favorite assign   1-9         Play/assign favorites
```

**Unused keys available:** `a`, `A`, `[`, `]`, `b`, `d`, `g`, `G`, `m`, `o`, `p`, `t`, `w`, `x`, `y`, `z`

## v1.1 Architecture Changes

### Core Insight: Two Sinks, One OutputStream

rodio explicitly supports multiple simultaneous Sinks on the same OutputStream. From the official docs: "All sounds are mixed together by rodio before being sent to the operating system. There is no restriction on the number of sinks that can be created."

The ambient channel is a **second Sink** connected to the **same OutputStream's mixer**. No new audio devices, threads, or mixing code needed.

```
OutputStream (one instance, already exists in Player)
    |
    +-- mixer()
         |
         +-- main_sink: Sink (existing - music playback)
         |      |
         |      +-- VisualizerSource<Decoder<Cursor<Vec<u8>>>>
         |
         +-- ambient_sink: Sink (NEW - ambient track, re-appended on loop)
                |
                +-- Decoder<Cursor<Vec<u8>>>  (re-appended when empty)
```

### Change 1: Dual-Sink Player (player.rs)

**Current Player struct:**
```rust
pub struct Player {
    _stream: OutputStream,
    sink: Sink,
    _audio_data: Option<Vec<u8>>,
    current_track: Option<String>,
}
```

**Expanded Player struct:**
```rust
pub struct Player {
    _stream: OutputStream,

    // Main channel (existing, unchanged)
    sink: Sink,
    _audio_data: Option<Vec<u8>>,
    current_track: Option<String>,

    // Ambient channel (NEW)
    ambient_sink: Option<Sink>,
    ambient_audio_data: Option<Vec<u8>>,
    ambient_track_name: Option<String>,
    ambient_volume: f32,
}
```

**Why `Option<Sink>` for ambient:** The ambient channel is not always active. Creating the
Sink lazily (on first ambient track load) matches the existing lazy Player init pattern.
When no ambient is playing, `ambient_sink` is `None` and all ambient-related event loop
checks are skipped (zero overhead).

**Why same OutputStream:** Opening two OutputStreams would open two ALSA devices, which
fails on WSL2 where PulseAudio provides a single default sink. Both Sinks MUST share one
OutputStream and use its mixer for software mixing.

**New Player methods:**
```rust
/// Load an ambient track. Creates new Sink on same mixer, starts playback.
pub fn load_ambient(&mut self, audio_bytes: Vec<u8>, track_name: String, volume: f32) -> Result<()>

/// Stop ambient playback, release Sink and cached data.
pub fn stop_ambient(&mut self)

/// Re-append cached ambient bytes when Sink empties (loop mechanism).
pub fn replay_ambient(&mut self, volume: f32) -> Result<()>

/// Toggle ambient pause/play. No-op if no ambient loaded.
pub fn toggle_ambient_pause(&self)

/// Set ambient volume independently (0.0..=1.0).
pub fn set_ambient_volume(&mut self, vol: f32)

/// Volume up/down for ambient channel (0.05 step, matches main pattern).
pub fn ambient_volume_up(&mut self)
pub fn ambient_volume_down(&mut self)

/// Accessors: ambient_volume(), ambient_track_name(), is_ambient_playing(),
/// is_ambient_finished(), has_ambient(), has_ambient_data()
```

**Looping approach -- manual re-append, NOT `repeat_infinite()`:**

rodio's `repeat_infinite()` has a confirmed memory leak (issue #673, open as of 2025-04-15,
not fixed in rodio 0.21). Memory grows ~10MB per 15 seconds until eventually stabilizing at
a high water mark (271-312MB for a 3MB file). This is unacceptable for a background app.

Instead, use the existing `replay_current()` pattern: when the ambient Sink empties, re-decode
from cached `ambient_audio_data` bytes and append to a fresh Sink. The 100ms event loop tick
means the gap between loops is at most 100ms -- imperceptible for ambient audio (rain, forest
sounds, etc.) which typically have gradual fade characteristics.

```rust
// In event loop, after check_download_complete():
fn check_ambient_loop(&mut self) -> Result<()> {
    if let Some(player) = &mut self.player {
        if player.is_ambient_finished() && player.has_ambient_data() {
            player.replay_ambient(self.ambient_volume)?;
        }
    }
    Ok(())
}
```

**No VisualizerSource on ambient.** The visualizer should reflect the music, not ambient
noise (rain, white noise). Ambient sounds would dominate low frequencies and make the
spectrum display unreadable for actual music.

### Change 2: Dual Download Channels (app.rs)

**New App fields for ambient:**
```rust
// Ambient channel state
ambient_track_name: Option<String>,      // Display name of loaded ambient track
ambient_part_key: Option<String>,        // Plex part key for session persistence
ambient_volume: f32,                      // Independent from saved_volume (main)
ambient_enabled: bool,                    // Toggle on/off without losing selection

// Ambient download
ambient_download_rx: Option<std::sync::mpsc::Receiver<Result<(Vec<u8>, String)>>>,
```

**Event loop changes:**
```
Event Loop (per 100ms tick):
1. check_download_complete()           -- existing
2. check_ambient_download_complete()   -- NEW (same try_recv pattern)
3. check_ambient_loop()                -- NEW (re-append if ambient sink empty)
4. auto-advance main track             -- existing
5. update visualizer                   -- existing (main channel only)
6. draw UI                             -- existing (extended for ambient panel)
7. poll keyboard events                -- existing (new keybindings added)
```

**Why separate mpsc channels:** Main and ambient downloads have different lifecycle behaviors.
Main downloads transition AppView (Tracks -> Downloading -> Playing). Ambient downloads do
NOT change AppView -- the user stays wherever they were. Mixing them in one channel requires
discriminator tagging and complex branching. Two channels keep logic clean.

### Change 3: Browser as Modal Overlay (app.rs + ui.rs)

**AppView addition:**
```rust
pub enum AppView {
    Playlists,
    Tracks,
    Downloading,
    Playing,
    Browser,  // NEW: modal overlay for ambient track selection
}
```

**Browser state fields:**
```rust
browser_sections: Vec<LibrarySection>,   // Music library sections
browser_tracks: Vec<Track>,              // Tracks in current browse context
browser_state: ListState,                // Selection cursor
browser_mode: BrowserMode,              // What the browser is showing
browser_section_key: Option<String>,    // Currently selected section
previous_view: AppView,                 // Where to return on Esc
```

**BrowserMode enum:**
```rust
pub enum BrowserMode {
    Sections,      // Listing music library sections
    Tracks,        // Browsing tracks within a section
    SearchInput,   // User is typing a search query
    SearchResults, // Showing search results
}
```

**Why a single Browser view with BrowserMode instead of multiple AppView variants:**
The browser is a modal overlay -- it appears over the current view and disappears when done.
It should not be a navigation destination in the view hierarchy alongside Playlists/Tracks.
Using BrowserMode as internal state keeps the browser self-contained and avoids complicating
the existing view transition logic in `go_back()` and `select_item()`.

**Why popup overlay, not full-screen view:**
The user should see their current playback context while selecting an ambient track. A popup
keeps the main track list visible beneath, providing spatial context. ratatui supports this
via the Clear widget + bordered Block pattern (see official popup example).

```
+-----------------------------------+
|  Playlists / Tracks               |
|   +---------------------------+   |
|   | Select Ambient Track      |   |  <- Popup overlay (70% x 60%)
|   | / Search: rain________    |   |
|   |                           |   |
|   | > Rain and Thunder  4:32  |   |
|   |   Ocean Waves       3:15  |   |
|   |   Forest Morning    5:01  |   |
|   +---------------------------+   |
|                                   |
+-----------------------------------+
|  [A] Rain Sounds  Vol: 40%        |  <- Ambient status (1 line)
|  >> Track Name - Artist - Album   |  <- Player bar (3 lines)
|  ================================ |
|  Playing | Vol: 80% | 2:34/4:12   |
+-----------------------------------+
```

**Keybinding additions:**

| Key | Context | Action |
|-----|---------|--------|
| `a` | Any view except Browser | Toggle ambient on/off (pause/resume) |
| `A` | Any view except Browser | Open ambient browser (modal popup) |
| `[` | Any view except Browser | Ambient volume down |
| `]` | Any view except Browser | Ambient volume up |
| `j/k` | Browser | Navigate list |
| `Enter` | Browser (Sections) | Enter section, list tracks |
| `Enter` | Browser (Tracks/SearchResults) | Select track, download as ambient |
| `/` | Browser | Enter search input mode |
| `Esc` | Browser | Close browser, return to `previous_view` |

**Why `a/A`:** Mnemonic for "ambient". `a` toggles (quick action), `A` opens browser (heavy
action). Both keys are currently unused. No conflicts with existing bindings.

**Why `[/]`:** Main volume uses `+/-`. Ambient needs distinct keys. `[/]` are adjacent on the
keyboard, unused, and visually suggest "enclosed/separate" channel.

### Change 4: Plex Track Browsing API (plex.rs)

Three new endpoints, following the identical pattern as existing `fetch_playlists()`/`fetch_tracks()`:

**a) List library sections:**
```
GET {server_url}/library/sections
Filter: type == "artist" (identifies music libraries)
Response: { MediaContainer: { Directory: [{ key, title, type }] } }
```

**b) List tracks in a section:**
```
GET {server_url}/library/sections/{key}/all?type=10
type=10 is the Plex type ID for audio tracks
Optional: &limit=100 for large libraries
Response: { MediaContainer: { Metadata: [Track] } } -- same Track struct
```

**c) Search tracks:**
```
GET {server_url}/hubs/search?query={text}&sectionId={section_key}&limit=50
Response: { MediaContainer: { Hub: [{ type, Metadata: [Track] }] } }
Extract the hub where type == "track"
```

**New type:**
```rust
#[derive(Deserialize, Debug, Clone)]
pub struct LibrarySection {
    pub key: String,
    pub title: String,
    #[serde(rename = "type")]
    pub section_type: String,
}
```

**Why reuse existing `Track` struct:** The Plex API returns tracks in the same JSON format
whether they come from a playlist or a library section. The existing `Track` struct with
`media[].parts[].key` works for both. `stream_url()` works identically.

**MVP scope:** Implement `fetch_music_sections()` and `fetch_section_tracks()` first. Search
(`search_tracks()`) adds value but can be a follow-up within the same phase if time permits.

### Change 5: Ambient Status Panel (ui.rs)

**Single line above the player bar:**
```
| [A] Rain Sounds  Vol: 40%  |  <- Length(1), cyan text, only when ambient loaded
```

**Layout adjustment:**
```rust
let ambient_height = if app.has_ambient() { 1 } else { 0 };
// Insert between visualizer/main area and player bar
```

In narrow mode (< 40 cols), the ambient line is hidden -- ambient still plays, just no
visual indicator. This follows the existing narrow-mode pattern that hides volume/shuffle/repeat
indicators.

### Change 6: Session Persistence (config.rs)

**Expanded Session struct:**
```rust
pub struct Session {
    // Existing fields (unchanged)
    pub playlist_rating_key: Option<String>,
    pub playlist_title: Option<String>,
    pub track_index: Option<usize>,
    pub volume: f32,
    pub shuffle_enabled: bool,
    pub repeat_mode: String,

    // Ambient state (NEW)
    #[serde(default)]
    pub ambient_part_key: Option<String>,       // Plex part key for re-download
    #[serde(default)]
    pub ambient_track_title: Option<String>,    // Display name
    #[serde(default)]
    pub ambient_volume: Option<f32>,            // 0.0..1.0
    #[serde(default)]
    pub ambient_enabled: Option<bool>,          // Was ambient active?
}
```

**Why `#[serde(default)]`:** Existing v1.0 session.toml files lack ambient fields. Default
deserialization prevents parse errors on upgrade. Missing fields become `None`.

**Why store `part_key` not `rating_key`:** The part key (e.g., `/library/parts/12345/file.flac`)
is what `plex_client.stream_url()` needs to construct the download URL. Storing it directly
avoids an extra API call to resolve track metadata on session restore.

**Session restore flow for ambient:**
```
restore_session()
    +-- existing main track restore (unchanged)
    +-- if session.ambient_part_key is Some:
            +-- construct stream URL via plex_client.stream_url(part_key)
            +-- spawn background download thread (same pattern)
            +-- on completion: Player::load_ambient(bytes, name, volume)
            +-- if session.ambient_enabled == Some(true): playing
            +-- if Some(false): load but immediately pause
```

## Component Boundaries

| Component | Responsibility | Communicates With |
|-----------|---------------|-------------------|
| `player.rs` | Owns OutputStream, main Sink, ambient Sink. Exposes ambient audio methods. No UI knowledge. | Called by `app.rs` |
| `app.rs` | Orchestrates: ambient state, download triggers, loop detection, volume persistence, browser view, keybindings. | Calls `player.rs`, `plex.rs`. Read by `ui.rs` |
| `plex.rs` | Library section listing, track fetching/search. Returns domain types. No audio knowledge. | Called by `app.rs` |
| `ui.rs` | Renders ambient status panel, browser popup overlay. No audio or API knowledge. | Reads from `app.rs` accessors |
| `config.rs` | Extends Session struct with ambient fields. Backward-compatible deserialization. | Read/written by `app.rs` |
| `visualizer.rs` | **NO CHANGE.** Taps only main Sink source. Ambient excluded. | Unchanged |
| `tui.rs` | **NO CHANGE.** Terminal lifecycle. | Unchanged |
| `auth.rs` | **NO CHANGE.** Plex authentication. | Unchanged |

## Data Flow: Ambient Track Selection and Playback

```
1. User presses 'A' (open ambient browser)
   |
   v
2. App: save previous_view, set view = Browser, mode = Sections
   |  fetch_music_sections() if not cached
   |
   v
3. User navigates j/k in sections, presses Enter
   |
   v
4. App: fetch_section_tracks(section_key), mode = Tracks
   |
   v
5. User navigates j/k in tracks, presses Enter on track
   |
   v
6. App: construct stream URL, spawn download thread (ambient_download_rx)
   |  restore previous_view (browser closes immediately)
   |
   v  .... user continues in previous view, no disruption ....
   |
   v
7. Event loop: check_ambient_download_complete() -> try_recv()
   |
   v
8. Player::load_ambient(bytes, name, volume)
   |  -> stop existing ambient Sink (if any)
   |  -> create new ambient Sink on same mixer
   |  -> decode audio, append to ambient Sink
   |  -> cache bytes for looping
   |
   v
9. Event loop: check_ambient_loop() (every 100ms tick)
   |  -> when ambient Sink empties, replay_ambient() from cached bytes
```

## Patterns to Follow

### Pattern: Parallel Sink Lifecycle
Each Sink (main, ambient) has independent lifecycle: created -> playing -> stopped.
Main Sink lifecycle is managed by `load_and_play()` and `replay_current()`.
Ambient Sink lifecycle mirrors this with `load_ambient()` and `replay_ambient()`.
**Critical rule:** Never stop the OutputStream. Both Sinks depend on it.

### Pattern: Volume Independence
Main volume: `+/=` (up), `-/_` (down), stored in `saved_volume`
Ambient volume: `]` (up), `[` (down), stored in `ambient_volume`
Both persisted independently in Session. Both applied to new Sinks on creation.

### Pattern: Non-Disruptive Background Operations
Ambient download and looping happen without changing AppView. The main track download
transitions through AppView::Downloading. The ambient download does NOT. This prevents
the ambient layer from interrupting the user's browsing/navigation flow.

### Pattern: Browser as Ephemeral Modal
The browser stores `previous_view` on open, restores it on close. No persistent browser
state survives between opens. Each `A` press fetches fresh sections/tracks.

## Anti-Patterns to Avoid

### Anti-Pattern: Using `repeat_infinite()`
**Why bad:** Confirmed memory leak (rodio issue #673, open, not fixed in 0.21). Memory grows
~10MB per 15 seconds until stabilizing at 100-300MB. Unacceptable for a background app.
**Instead:** Manual re-append from cached bytes when Sink empties. Matches existing
`replay_current()` pattern. Max 100ms gap, imperceptible for ambient audio.

### Anti-Pattern: Separate Audio Device
**Why bad:** Two OutputStreams would compete for ALSA device. Fails on WSL2 PulseAudio.
**Instead:** `Sink::connect_new(self._stream.mixer())` for ambient Sink, sharing OutputStream.

### Anti-Pattern: Manual Audio Mixing
**Why bad:** Unnecessary complexity. rodio's internal mixer handles this automatically.
**Instead:** Two Sinks with independent volume/pause. rodio mixes at the mixer level.

### Anti-Pattern: Shared Download Channel
**Why bad:** Main downloads change AppView to Downloading/Playing. Ambient downloads should not.
**Instead:** Separate mpsc channels with separate handler methods.

### Anti-Pattern: Visualizer on Ambient
**Why bad:** Ambient noise (rain, white noise) dominates low frequencies, making the spectrum
unreadable for actual music.
**Instead:** VisualizerSource wraps only the main Sink. Ambient is invisible to the visualizer.

### Anti-Pattern: Full Library Hierarchy Browser
**Why bad:** Scope creep. Ambient selection needs "find rain sounds", not Artist > Album > Track.
**Instead:** Flat track list per library section + search filtering.

### Anti-Pattern: Ambient Playlists
**Why bad:** Explicitly out of scope per PROJECT.md. Doubles ambient complexity.
**Instead:** Single track + manual loop via re-append.

## File Changes Summary

| File | Change Type | What Changes |
|------|------------|-------------|
| `player.rs` | Extend | Add ambient Sink, ambient audio methods |
| `app.rs` | Extend | Add ambient state, browser mode, keybindings, download handling, loop check |
| `plex.rs` | Extend | Add LibrarySection type, section listing, track browsing methods |
| `ui.rs` | Extend | Add ambient status panel, browser popup overlay |
| `config.rs` | Extend | Add ambient fields to Session struct |
| `visualizer.rs` | **NO CHANGE** | |
| `tui.rs` | **NO CHANGE** | |
| `auth.rs` | **NO CHANGE** | |

No new files needed. All changes extend existing modules following established patterns.

## Build Order (dependency-driven)

```
Phase 1: Player dual-sink (player.rs)         <- No dependencies, audio foundation
    |
    |  CAN PARALLELIZE WITH:
    |
Phase 2: Plex track browsing API (plex.rs)    <- No dependencies, data layer
    |
    v
Phase 3: App ambient controls (app.rs)        <- Depends on Phase 1 (Player methods)
    |                                              ambient toggle, volume, download flow
    v
Phase 4: Browser UI + overlay (ui.rs + app.rs) <- Depends on Phase 2 (API) + Phase 3 (state)
    |                                              modal popup, section/track navigation, search
    v
Phase 5: Session persistence (config.rs)       <- Depends on Phase 3 (ambient state fields)
    |                                              expand Session, save/restore ambient
    v
Phase 6: Integration testing + polish          <- Depends on all above
                                                   WSL2 dual-sink, narrow terminals, errors
```

## Sources

- [rodio official docs - multiple Sinks](https://docs.rs/rodio/latest/rodio/) -- HIGH confidence: "no restriction on number of sinks"
- [rodio Sink::connect_new](https://docs.rs/rodio/latest/rodio/struct.Sink.html) -- HIGH confidence: takes `&Mixer`, creates independent channel
- [rodio issue #673 - repeat_infinite memory leak](https://github.com/RustAudio/rodio/issues/673) -- HIGH confidence: confirmed, unfixed in 0.21
- [ratatui popup example](https://ratatui.rs/examples/apps/popup/) -- HIGH confidence: official overlay pattern
- [Plex API search hub](https://plexapi.dev/api-reference/search/perform-a-search) -- MEDIUM confidence: `/hubs/search` with sectionId
- [Plex API library sections](https://support.plex.tv/articles/201638786-plex-media-server-url-commands/) -- MEDIUM confidence: `/library/sections`
- [Plex API music tracks type=10](https://www.plexopedia.com/plex-media-server/api/library/music/) -- MEDIUM confidence: community docs
- Existing TermTunes v1.0 codebase (all 8 source files, 3,507 lines) -- HIGH confidence: direct analysis

---
*Architecture research for: TermTunes v1.1 (Multi-Channel Audio & Track Browsing)*
*Researched: 2026-02-10*
