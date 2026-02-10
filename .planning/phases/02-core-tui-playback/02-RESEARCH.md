# Phase 2: Core TUI and Playback - Research

**Researched:** 2026-02-10
**Domain:** Ratatui multi-panel TUI layout, rodio playback controls (volume, skip, position tracking), vim-style keybinding architecture
**Confidence:** HIGH

## Summary

Phase 2 transforms the Phase 1 proof-of-concept into a fully functional terminal music player. The existing codebase already has the foundation: authenticated Plex client, playlist/track fetching, background download with mpsc channel, lazy Player initialization, and basic j/k/Enter/Space keybindings. Phase 2 extends this in three directions: (1) richer TUI layout with a multi-line player bar showing track metadata and progress, (2) additional playback controls (next/previous track, volume up/down), and (3) elapsed time / progress tracking via rodio's `Sink::get_pos()` and Plex's track `duration` field.

The most critical finding is that **rodio's `Sink::get_pos()` returns the position of the currently playing source**, automatically resetting when a new track starts. Combined with the Plex API's `duration` field (milliseconds), this gives everything needed for a progress bar without any custom position tracking. Volume control uses `Sink::set_volume(f32)` where 1.0 is normal and values above 1.0 amplify (with potential clipping). Next/previous track requires app-level management of the current track index within the playlist, triggering a new download-and-play cycle for each track change -- rodio's `Sink::skip_one()` is not useful here since the app loads one track at a time rather than queuing multiple.

The ratatui side is straightforward: `Layout::vertical` splits the terminal into a main content area (playlist/track list) and a multi-line player bar at the bottom. The player bar uses `Line::from(vec![Span::styled(...), ...])` for multi-colored metadata display and either `Gauge` or `LineGauge` for the progress bar. The `Stylize` trait provides ergonomic shorthand (e.g., `"text".red().bold()`).

**Primary recommendation:** Use the existing download-per-track architecture. Track next/previous via an index into the loaded `tracks` Vec. Get elapsed time from `Sink::get_pos()`, total duration from Plex's `Track.duration` field (milliseconds). Render player bar as a 3-line bottom panel: Line 1 = track info (name, artist, album), Line 2 = progress bar (LineGauge), Line 3 = playback state + volume + elapsed/total time.

## Standard Stack

### Core (Phase 2 specific -- no new dependencies)

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| ratatui | 0.30 | Multi-panel layout, Gauge/LineGauge for progress, Line/Span for styled player bar | Already in Cargo.toml. Provides all needed widgets. |
| rodio | 0.21 | `set_volume()`, `volume()`, `get_pos()` for playback controls and position tracking | Already in Cargo.toml. Sink API has all needed methods. |
| crossterm | 0.29 | Keyboard event handling for additional keybindings (n/N, +/-, etc.) | Already in Cargo.toml. No changes needed. |

### Supporting (already in Cargo.toml)

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| reqwest | 0.13 | Download next/previous tracks on demand | Same background download pattern from Phase 1 |
| color-eyre | 0.6 | Error handling for download/playback failures during skip | Consistent error handling throughout |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| LineGauge for progress bar | Gauge widget | Gauge is a full-height bar (wastes vertical space). LineGauge is single-line, better for compact player bar. Use LineGauge. |
| Plex `duration` for total time | rodio `Decoder::total_duration()` | Decoder's `total_duration()` returns `Option<Duration>` and may be `None` for some formats. Plex always provides duration in metadata. Use Plex duration. |
| Track index for next/prev | rodio `Sink::skip_one()` queue | skip_one is for pre-queued sources. Current architecture downloads one track at a time. Track index is simpler and correct. |
| Linear volume (set_volume direct) | Logarithmic/dB volume curve | Linear is perceptually wrong (doubling amplitude != doubling perceived loudness). BUT for Phase 2 MVP, linear 0.0-1.0 with 0.05 steps is adequate. Revisit log scale in Phase 3/4. |

### Installation

No new dependencies required. All libraries are already in `Cargo.toml` from Phase 1.

## Architecture Patterns

### Recommended Changes to Project Structure

```
src/
├── main.rs              # Entry point (no changes needed)
├── app.rs               # MODIFY: Add current_track_index, volume state, next/prev/volume methods
├── auth.rs              # No changes
├── config.rs            # No changes
├── plex.rs              # No changes (Track already has artist, album, duration fields)
├── player.rs            # MODIFY: Add set_volume, volume, get_pos, current_duration methods
├── tui.rs               # No changes
└── ui.rs                # MODIFY: Rewrite to multi-panel layout with player bar
```

### Pattern 1: Track Index Management for Next/Previous

**What:** The app maintains a `current_track_index: Option<usize>` pointing into the `tracks: Vec<Track>`. Next increments it (wrapping or clamping), previous decrements it. Each change triggers a new download-and-play cycle using the existing `start_track_download()` pattern.
**When to use:** For PLAY-04 (next track) and PLAY-05 (previous track).

```rust
// In app.rs -- track index management

/// Index of the currently playing track in self.tracks.
/// None when no track has been played yet.
current_track_index: Option<usize>,

/// Skip to the next track in the playlist.
fn next_track(&mut self) -> Result<()> {
    if self.tracks.is_empty() { return Ok(()); }
    let next = match self.current_track_index {
        Some(idx) if idx + 1 < self.tracks.len() => idx + 1,
        Some(_) => 0, // Wrap to beginning
        None => 0,
    };
    self.current_track_index = Some(next);
    self.track_state.select(Some(next));
    self.start_track_download()
}

/// Skip to the previous track in the playlist.
fn prev_track(&mut self) -> Result<()> {
    if self.tracks.is_empty() { return Ok(()); }
    let prev = match self.current_track_index {
        Some(0) => self.tracks.len() - 1, // Wrap to end
        Some(idx) => idx - 1,
        None => 0,
    };
    self.current_track_index = Some(prev);
    self.track_state.select(Some(prev));
    self.start_track_download()
}
```

**Why not use rodio's queue:** The current architecture downloads one track at a time via background thread + mpsc channel. Pre-queuing would require downloading multiple tracks ahead, adding memory pressure and complexity. The download-on-demand pattern is correct for a Plex player where tracks come from a network server.

### Pattern 2: Volume Control via Rodio Sink

**What:** Expose `set_volume(f32)` and `volume() -> f32` through the Player struct. The UI binds +/= to increase and -/_ to decrease in fixed steps. Volume state lives in the Sink (rodio manages it internally), so the Player just delegates.
**When to use:** For PLAY-06 (volume up) and PLAY-07 (volume down).

```rust
// In player.rs -- volume control

/// Volume step size for each key press.
const VOLUME_STEP: f32 = 0.05;

/// Minimum volume (muted).
const VOLUME_MIN: f32 = 0.0;

/// Maximum volume (normal output, no amplification).
const VOLUME_MAX: f32 = 1.0;

/// Get the current volume level (0.0 to 1.0).
pub fn volume(&self) -> f32 {
    self.sink.volume()
}

/// Increase volume by one step, clamping at VOLUME_MAX.
pub fn volume_up(&self) {
    let new = (self.sink.volume() + VOLUME_STEP).min(VOLUME_MAX);
    self.sink.set_volume(new);
}

/// Decrease volume by one step, clamping at VOLUME_MIN.
pub fn volume_down(&self) {
    let new = (self.sink.volume() - VOLUME_STEP).max(VOLUME_MIN);
    self.sink.set_volume(new);
}
```

**Design decision -- max 1.0 not higher:** Values above 1.0 amplify samples and can cause clipping/distortion. Capping at 1.0 prevents audio quality degradation. The Plex server already normalizes tracks to reasonable levels.

### Pattern 3: Progress Tracking via get_pos() + Plex Duration

**What:** Elapsed time comes from `Sink::get_pos()` (returns `Duration`). Total duration comes from the Plex `Track.duration` field (milliseconds, stored as `Option<u64>`). The ratio `elapsed / total` drives the progress bar.
**When to use:** For DISP-04 (progress bar), DISP-05 (elapsed/total time).

```rust
// In player.rs -- playback position and duration

/// Get the current playback position.
pub fn get_pos(&self) -> std::time::Duration {
    self.sink.get_pos()
}

// In app.rs or ui.rs -- computing progress ratio
fn progress_ratio(player: &Player, track: &Track) -> f64 {
    let elapsed = player.get_pos();
    let total_ms = track.duration.unwrap_or(0);
    if total_ms == 0 {
        return 0.0;
    }
    let total = std::time::Duration::from_millis(total_ms);
    let ratio = elapsed.as_secs_f64() / total.as_secs_f64();
    ratio.clamp(0.0, 1.0) // Clamp because get_pos() can briefly exceed duration
}

/// Format a Duration as "MM:SS".
fn format_duration(d: std::time::Duration) -> String {
    let total_secs = d.as_secs();
    let mins = total_secs / 60;
    let secs = total_secs % 60;
    format!("{:02}:{:02}", mins, secs)
}
```

**Critical note on get_pos() accuracy:** Rodio updates position every ~5ms via periodic_access. There is a known behavior where `get_pos()` can briefly report a value beyond the track's actual duration (see Sources). The `clamp(0.0, 1.0)` on the ratio handles this edge case.

### Pattern 4: Multi-Panel Player Bar Layout

**What:** The UI splits vertically into: main content (Fill), player bar (3 lines). The player bar has three rows: track info, progress bar, and playback controls/status.
**When to use:** For DISP-01 through DISP-07 (all display requirements).

```rust
// In ui.rs -- player bar layout

use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, LineGauge, Paragraph};

/// Render the full UI with player bar.
pub fn render(frame: &mut Frame, app: &mut App) {
    let area = frame.area();

    // Split: main content + player bar (3 lines for info + progress + status)
    let has_player = app.player().is_some();
    let player_height = if has_player { 3 } else { 1 }; // 1 line for status bar when no player

    let [main_area, player_area] =
        Layout::vertical([Constraint::Fill(1), Constraint::Length(player_height)])
            .areas(area);

    // Render main content (playlists or tracks)
    match app.view() {
        AppView::Playlists => render_playlists(frame, app, main_area),
        AppView::Tracks | AppView::Playing => render_tracks(frame, app, main_area),
        AppView::Downloading => render_downloading(frame, main_area),
    }

    // Render player bar or status bar
    if has_player {
        render_player_bar(frame, app, player_area);
    } else {
        render_status_bar(frame, app, player_area);
    }
}

/// Render the 3-line player bar.
fn render_player_bar(frame: &mut Frame, app: &App, area: Rect) {
    let [info_area, progress_area, status_area] =
        Layout::vertical([
            Constraint::Length(1), // Track info
            Constraint::Length(1), // Progress bar
            Constraint::Length(1), // Playback state + volume + time
        ])
        .areas(area);

    // Line 1: Track name - Artist - Album
    let track_info = Line::from(vec![
        Span::styled(" >> ", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
        Span::styled("Track Name", Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
        Span::styled(" - ", Style::default().fg(Color::DarkGray)),
        Span::styled("Artist", Style::default().fg(Color::Cyan)),
        Span::styled(" - ", Style::default().fg(Color::DarkGray)),
        Span::styled("Album", Style::default().fg(Color::Yellow)),
    ]);
    frame.render_widget(Paragraph::new(track_info), info_area);

    // Line 2: Progress bar (LineGauge)
    let gauge = LineGauge::default()
        .ratio(0.42) // computed from get_pos() / duration
        .filled_style(Style::default().fg(Color::Cyan))
        .unfilled_style(Style::default().fg(Color::DarkGray));
    frame.render_widget(gauge, progress_area);

    // Line 3: State | Volume | Elapsed / Total
    let status_line = Line::from(vec![
        Span::styled(" Playing", Style::default().fg(Color::Green)),
        Span::styled(" | ", Style::default().fg(Color::DarkGray)),
        Span::styled("Vol: 80%", Style::default().fg(Color::White)),
        Span::styled(" | ", Style::default().fg(Color::DarkGray)),
        Span::styled("01:23 / 03:45", Style::default().fg(Color::White)),
    ]);
    frame.render_widget(Paragraph::new(status_line), status_area);
}
```

### Pattern 5: Keybinding Architecture

**What:** Extend `handle_key()` in app.rs with new bindings. Group by context (global vs view-specific).
**When to use:** For KEY-01 through KEY-05.

```rust
// In app.rs -- extended keybindings

async fn handle_key(&mut self, code: KeyCode, modifiers: KeyModifiers) -> Result<()> {
    match (code, modifiers) {
        // === Global (any view) ===
        (KeyCode::Char('q'), _) => self.running = false,
        (KeyCode::Char('c'), m) if m.contains(KeyModifiers::CONTROL) => self.running = false,

        // Spacebar: toggle play/pause (KEY-03)
        (KeyCode::Char(' '), _) => {
            if let Some(player) = &self.player {
                player.toggle_pause();
            }
        }

        // Volume up (PLAY-06) -- + or =
        (KeyCode::Char('+'), _) | (KeyCode::Char('='), _) => {
            if let Some(player) = &self.player {
                player.volume_up();
            }
        }

        // Volume down (PLAY-07) -- - or _
        (KeyCode::Char('-'), _) | (KeyCode::Char('_'), _) => {
            if let Some(player) = &self.player {
                player.volume_down();
            }
        }

        // Next track (PLAY-04) -- n or >
        (KeyCode::Char('n'), _) | (KeyCode::Char('>'), _) => {
            if matches!(self.view, AppView::Playing) {
                self.next_track()?;
            }
        }

        // Previous track (PLAY-05) -- N or <
        (KeyCode::Char('N'), _) | (KeyCode::Char('<'), _) => {
            if matches!(self.view, AppView::Playing) {
                self.prev_track()?;
            }
        }

        // === View-specific navigation ===
        // Navigate down (KEY-01)
        (KeyCode::Char('j'), _) | (KeyCode::Down, _) => self.move_selection_down(),
        // Navigate up (KEY-01)
        (KeyCode::Char('k'), _) | (KeyCode::Up, _) => self.move_selection_up(),
        // Select / Enter (KEY-02)
        (KeyCode::Enter, _) => self.select_item().await?,
        // Back
        (KeyCode::Esc, _) | (KeyCode::Backspace, _) => self.go_back(),

        _ => {}
    }
    Ok(())
}
```

### Anti-Patterns to Avoid

- **Pre-downloading multiple tracks into rodio's queue:** Wastes bandwidth and memory. The user may skip, go back, or quit. Download one track at a time on demand.
- **Using `Decoder::total_duration()` for progress bar:** Returns `Option<Duration>` that may be `None` for some codecs. Plex always provides duration in the track metadata. Use Plex's value.
- **Storing volume as a separate field in App:** Rodio's Sink already stores volume internally. Query `sink.volume()` when needed. Avoid state duplication.
- **Blocking the event loop during track skip:** Next/previous must use the same background download + mpsc channel pattern as the initial track selection. Never download synchronously.
- **Ignoring the `current_track_index` when user selects a track from the list:** When the user presses Enter on a track in the list view, set `current_track_index` to that index. Otherwise, next/prev will be relative to the wrong position.
- **Rendering progress bar without clamping:** `get_pos()` can briefly exceed the track's duration. Always clamp the ratio to 0.0..=1.0 to prevent `LineGauge` from panicking (it panics if ratio > 1.0).

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Progress bar rendering | Custom character-based progress bar | ratatui `LineGauge` widget | Handles unicode, terminal width, styling. Single-line thin progress bar. |
| Volume control | Custom amplitude multiplication | rodio `Sink::set_volume()` / `Sink::volume()` | Thread-safe, applies to the audio pipeline automatically. |
| Playback position tracking | Manual timer or elapsed counter | rodio `Sink::get_pos()` | Automatically accounts for pauses, speed changes. Updated every ~5ms internally. |
| Multi-styled text on one line | Manual ANSI escape codes | ratatui `Line::from(vec![Span::styled(...), ...])` | Type-safe, composable, handles terminal compatibility. |
| Time formatting (MM:SS) | Pull in a datetime crate | Simple `Duration.as_secs()` arithmetic | Two lines of code: `secs / 60` and `secs % 60`. No dependency needed. |

**Key insight:** Phase 2 requires zero new dependencies. Every capability (progress bar, volume control, position tracking, styled text) is provided by libraries already in `Cargo.toml`. The work is pure glue logic and UI layout.

## Common Pitfalls

### Pitfall 1: LineGauge Panics on Ratio > 1.0

**What goes wrong:** `LineGauge::ratio(f64)` panics if the value is not between 0.0 and 1.0 inclusively. If `get_pos()` returns a duration slightly beyond the track's total duration, the computed ratio exceeds 1.0 and the app crashes.
**Why it happens:** Rodio's `get_pos()` position updates are based on a ~5ms periodic callback. There is a brief window where the reported position exceeds the actual track duration, especially near the end of a track. This is a documented behavior.
**How to avoid:** Always clamp: `ratio.clamp(0.0, 1.0)` before passing to `LineGauge::ratio()`.
**Warning signs:** App crashes near the end of a track with a panic from ratatui.

### Pitfall 2: get_pos() Returns Stale Position After New Track Loads

**What goes wrong:** After stopping the old sink and creating a new one for the next track, `get_pos()` returns the old position briefly until the new source's `track_position()` starts updating.
**Why it happens:** The current code in `player.rs` calls `self.sink.stop()` then creates a new `Sink::connect_new()`. The new sink starts with `Duration::ZERO` for `get_pos()`, but there may be a brief race between the stop and the new source's first position update.
**How to avoid:** After creating a new sink and appending a new source, `get_pos()` will return `Duration::ZERO` immediately (since `append()` internally uses `track_position()`). The existing pattern of creating a fresh Sink per track is correct and handles this naturally. No additional work needed.
**Warning signs:** Progress bar jumps from the old track's position to 0:00 on skip. This is actually correct behavior.

### Pitfall 3: Download In Progress When User Presses Next/Previous

**What goes wrong:** User presses next/prev while a download is in progress. The old download completes and starts playing the wrong track, overwriting the new download.
**Why it happens:** The `download_rx` channel still has a pending result from the previous download. If not handled, `check_download_complete()` will process it and play the wrong track.
**How to avoid:** When starting a new track download (next/prev), drop the old `download_rx` by setting `self.download_rx = None` before creating the new channel. This causes the old download thread's `tx.send()` to fail silently (receiver dropped), which is the correct behavior.
**Warning signs:** Pressing next rapidly plays the wrong track momentarily.

### Pitfall 4: Player Not Initialized When User Presses Volume/Skip Keys

**What goes wrong:** User presses +/- or n/N before any track has played. The player is `None` (lazy initialization from Phase 1), causing the code to silently do nothing -- but if not handled, it could panic.
**Why it happens:** Player is created lazily on first track playback. Volume/skip keys are global.
**How to avoid:** Always guard with `if let Some(player) = &self.player { ... }`. The existing code already does this for toggle_pause. Apply the same pattern to all new playback controls.
**Warning signs:** None (silent no-op is correct). But if you unwrap instead of if-let, panic on first key press.

### Pitfall 5: Forgetting to Update current_track_index When User Selects from List

**What goes wrong:** User navigates the track list and presses Enter on track #5. `current_track_index` is not updated. Later, pressing "next" goes to track #1 (previous index + 1) instead of track #6.
**Why it happens:** The `select_item()` method triggers download via `start_track_download()` using `track_state.selected()`, but does not sync `current_track_index`.
**How to avoid:** In `select_item()`, when starting track playback from the track list, always set `self.current_track_index = self.track_state.selected()`.
**Warning signs:** Next/prev skips to unexpected tracks after manual selection.

### Pitfall 6: Player Bar Flickers or Shows Stale Data

**What goes wrong:** The player bar shows stale track info briefly after a track change, or the progress bar flickers between 0% and the real position.
**Why it happens:** The event loop polls at 100ms intervals. Between the track change and the next render, stale data may be displayed.
**How to avoid:** Store the current track's metadata (name, artist, album, duration) in the App struct when a new track starts playing. The player bar reads from this stored data, not from the tracks Vec via index lookup. Update this metadata atomically when `check_download_complete()` transitions to Playing state.
**Warning signs:** Brief flash of old track info on skip.

## Code Examples

### Complete Player Bar Rendering (Verified Pattern)

```rust
// Source: ratatui docs.rs, Context7 ratatui documentation

use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{LineGauge, Paragraph};

/// Metadata for the currently playing track, stored in App for display.
pub struct NowPlaying {
    pub track_name: String,
    pub artist: String,
    pub album: String,
    pub duration_ms: u64,
}

fn render_player_bar(frame: &mut Frame, app: &App, area: Rect) {
    let [info_area, progress_area, status_area] =
        Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .areas(area);

    if let (Some(player), Some(now_playing)) = (app.player(), app.now_playing()) {
        // Line 1: Track info
        let state_icon = if player.is_paused() { "||" } else { ">>" };
        let info_line = Line::from(vec![
            Span::styled(
                format!(" {} ", state_icon),
                Style::default().fg(if player.is_paused() { Color::Yellow } else { Color::Green })
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                &now_playing.track_name,
                Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
            ),
            Span::styled(" - ", Style::default().fg(Color::DarkGray)),
            Span::styled(&now_playing.artist, Style::default().fg(Color::Cyan)),
            Span::styled(" - ", Style::default().fg(Color::DarkGray)),
            Span::styled(&now_playing.album, Style::default().fg(Color::Yellow)),
        ]);
        frame.render_widget(Paragraph::new(info_line), info_area);

        // Line 2: Progress bar
        let elapsed = player.get_pos();
        let total = std::time::Duration::from_millis(now_playing.duration_ms);
        let ratio = if total.as_millis() > 0 {
            (elapsed.as_secs_f64() / total.as_secs_f64()).clamp(0.0, 1.0)
        } else {
            0.0
        };

        let gauge = LineGauge::default()
            .ratio(ratio)
            .filled_style(Style::default().fg(Color::Cyan))
            .unfilled_style(Style::default().fg(Color::DarkGray));
        frame.render_widget(gauge, progress_area);

        // Line 3: Status line
        let volume_pct = (player.volume() * 100.0) as u8;
        let elapsed_str = format_duration(elapsed);
        let total_str = format_duration(total);

        let playback_state = if player.is_paused() {
            Span::styled(" Paused ", Style::default().fg(Color::Yellow))
        } else if player.is_playing() {
            Span::styled(" Playing ", Style::default().fg(Color::Green))
        } else {
            Span::styled(" Stopped ", Style::default().fg(Color::DarkGray))
        };

        let status_line = Line::from(vec![
            playback_state,
            Span::styled("| ", Style::default().fg(Color::DarkGray)),
            Span::styled(format!("Vol: {}%", volume_pct), Style::default().fg(Color::White)),
            Span::styled(" | ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("{} / {}", elapsed_str, total_str),
                Style::default().fg(Color::White),
            ),
        ]);
        frame.render_widget(Paragraph::new(status_line), status_area);
    }
}

fn format_duration(d: std::time::Duration) -> String {
    let total_secs = d.as_secs();
    format!("{:02}:{:02}", total_secs / 60, total_secs % 60)
}
```

### Volume Control in Player (Verified Pattern)

```rust
// Source: docs.rs/rodio/latest/rodio/struct.Sink.html (set_volume, volume verified)

const VOLUME_STEP: f32 = 0.05; // 5% per key press
const VOLUME_MIN: f32 = 0.0;
const VOLUME_MAX: f32 = 1.0;

impl Player {
    pub fn volume(&self) -> f32 {
        self.sink.volume()
    }

    pub fn volume_up(&self) {
        let new = (self.sink.volume() + VOLUME_STEP).min(VOLUME_MAX);
        self.sink.set_volume(new);
    }

    pub fn volume_down(&self) {
        let new = (self.sink.volume() - VOLUME_STEP).max(VOLUME_MIN);
        self.sink.set_volume(new);
    }

    pub fn get_pos(&self) -> std::time::Duration {
        self.sink.get_pos()
    }
}
```

### Next/Previous Track with Download Cancellation (Verified Pattern)

```rust
// Source: Existing codebase pattern (app.rs start_track_download), rodio Sink API

impl App {
    fn play_track_at_index(&mut self, index: usize) -> Result<()> {
        if index >= self.tracks.len() { return Ok(()); }

        // Cancel any in-progress download by dropping the receiver
        self.download_rx = None;

        // Update index and list selection
        self.current_track_index = Some(index);
        self.track_state.select(Some(index));

        // Use existing download mechanism
        self.start_track_download()
    }

    fn next_track(&mut self) -> Result<()> {
        if self.tracks.is_empty() { return Ok(()); }
        let next = match self.current_track_index {
            Some(idx) if idx + 1 < self.tracks.len() => idx + 1,
            _ => 0,
        };
        self.play_track_at_index(next)
    }

    fn prev_track(&mut self) -> Result<()> {
        if self.tracks.is_empty() { return Ok(()); }
        let prev = match self.current_track_index {
            Some(0) | None => self.tracks.len().saturating_sub(1),
            Some(idx) => idx - 1,
        };
        self.play_track_at_index(prev)
    }
}
```

### LineGauge Progress Bar (Verified Pattern)

```rust
// Source: Context7 ratatui docs, docs.rs/ratatui LineGauge

use ratatui::widgets::LineGauge;
use ratatui::style::Style;

let progress = LineGauge::default()
    .ratio(0.42) // Must be 0.0..=1.0 -- WILL PANIC otherwise
    .filled_style(Style::default().fg(Color::Cyan))
    .unfilled_style(Style::default().fg(Color::DarkGray))
    .label("01:23 / 03:45"); // Optional: label on the left side
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Manual progress bar with `▓░` characters | ratatui `LineGauge` with unicode support | ratatui 0.26+ | Higher precision (8 fractional parts per cell with `use_unicode`). |
| rodio `OutputStream::try_default()` tuple | `OutputStreamBuilder::open_default_stream()` | rodio 0.20+ | Already handled in Phase 1 codebase. |
| rodio no position tracking | `Sink::get_pos()` with automatic `track_position()` in append | rodio 0.19+ | Position tracking is now built-in. No manual `track_position()` call needed. |
| Separate `Gauge` for thin progress | `LineGauge` dedicated single-line widget | ratatui 0.26+ | Purpose-built for single-line progress bars like player bars. |
| Manual `Style::default().fg(Color::Red)` | Stylize trait: `"text".red().bold()` | ratatui 0.22+ | Ergonomic shorthand, same result. Both work. |

**Deprecated/outdated:**
- `rodio::Sink::new()` tuple return -- use `Sink::connect_new(&stream.mixer())` (already done in Phase 1).
- `Gauge` for thin progress bars -- use `LineGauge` which is designed for single-line display.

## Open Questions

1. **Auto-advance to next track when current track finishes**
   - What we know: `sink.empty()` returns true when the track finishes. The event loop already checks this implicitly (the player reports `is_finished()`).
   - What's unclear: Whether auto-advance should be implemented in Phase 2 or deferred to Phase 3 (where repeat modes are added). Auto-advance is strongly expected behavior for a music player.
   - Recommendation: Implement basic auto-advance in Phase 2. When `player.is_finished()` and we have a `current_track_index`, call `next_track()`. This is simple, expected, and does not conflict with Phase 3's repeat/shuffle (those will modify how the next index is computed).

2. **Volume persistence across tracks**
   - What we know: The current code creates a fresh `Sink::connect_new()` for each new track. The Sink starts with default volume (1.0).
   - What's unclear: Whether volume resets to 1.0 when a new Sink is created (yes, it does -- each Sink has its own volume state).
   - Recommendation: Store the last volume level in the App or Player struct. After creating a new Sink, immediately call `sink.set_volume(saved_volume)` to restore the user's chosen level.

3. **Keybinding for next/previous track**
   - What we know: Phase 2 requirements specify skip next (PLAY-04) and skip previous (PLAY-05) but do not mandate specific keys.
   - What's unclear: Which keys to bind. Common choices: `n/N`, `>/< `, `]/ [`.
   - Recommendation: Use `n` for next and `N` (shift-n) for previous. This follows vim conventions (n = next match, N = previous match) and is discoverable. Also bind `>` and `<` as alternatives.

4. **Player bar height: 3 lines vs 2 lines**
   - What we know: Requirements demand: track name, artist, album, playback state, volume, progress bar, elapsed/total time. That is a lot of information.
   - What's unclear: Whether 3 lines is optimal or if 2 lines (info + progress) would suffice with all status info on the progress line.
   - Recommendation: Start with 3 lines (info, progress, status). This gives clear visual separation. If terminal height is a concern (Phase 4 handles responsive layout), 3 lines is minimal for readability.

## Sources

### Primary (HIGH confidence)
- [rodio Sink API - docs.rs](https://docs.rs/rodio/latest/rodio/struct.Sink.html) -- Complete Sink method reference: get_pos, set_volume, volume, skip_one, append, try_seek verified
- [rodio Source trait - docs.rs](https://docs.rs/rodio/latest/rodio/source/trait.Source.html) -- total_duration(), track_position() verified
- [ratatui Gauge/LineGauge - docs.rs](https://docs.rs/ratatui/latest/ratatui/widgets/struct.LineGauge.html) -- LineGauge API: ratio, filled_style, unfilled_style, label verified via Context7
- [ratatui Layout - docs.rs](https://docs.rs/ratatui/latest/ratatui/layout/index.html) -- Layout::vertical, Layout::horizontal, Constraint types verified via Context7
- [ratatui Styling Text recipe](https://ratatui.rs/recipes/render/style-text/) -- Line::from(vec![Span::styled(...)]) pattern verified via Context7
- [ratatui Display Text recipe](https://ratatui.rs/recipes/render/display-text/) -- Stylize trait shorthand verified via Context7
- [rodio sink.rs source](https://docs.rs/rodio/latest/src/rodio/sink.rs.html) -- Internal implementation: append() calls track_position(), periodic_access updates position every ~5ms

### Secondary (MEDIUM confidence)
- [rodio issue #405 - symphonia total_duration](https://github.com/RustAudio/rodio/issues/405) -- Confirmed COMPLETED: Symphonia decoder total_duration now works (fixed via PR #513)
- [rodio issue #714 - better volume control](https://github.com/RustAudio/rodio/issues/714) -- Logarithmic volume added in PR #715, but linear 0.0-1.0 still works
- [rodio Sink::get_pos() beyond duration forum post](https://users.rust-lang.org/t/rodio-sink-get-pos-gives-a-pos-beyond-the-duration-of-the-audio-file/131594) -- Confirmed: get_pos() can report values beyond track duration
- [Python PlexAPI documentation](https://python-plexapi.readthedocs.io/en/latest/) -- Confirmed Plex Track.duration is in milliseconds
- [Trix terminal music player (GitHub)](https://github.com/RIZAmohammadkhan/TerminalMusicPlayer) -- Reference implementation: ratatui + rodio music player with vim keybindings

### Tertiary (LOW confidence)
- [spotify_player](https://github.com/aome510/spotify-player) -- Reference for TUI music player layout patterns (not directly examined, architecture inferred from docs)

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH -- no new dependencies, all APIs verified via docs.rs and Context7
- Architecture: HIGH -- patterns verified against existing codebase (player.rs, app.rs, ui.rs) and official ratatui/rodio documentation
- Pitfalls: HIGH -- get_pos() overflow, LineGauge panic, download cancellation all verified via official sources and community reports
- Keybindings: MEDIUM -- key choices (n/N, +/-, etc.) are recommendations based on vim conventions, not user-locked decisions

**Research date:** 2026-02-10
**Valid until:** 2026-03-10 (stable domain: ratatui 0.30 and rodio 0.21 APIs are not changing imminently)
