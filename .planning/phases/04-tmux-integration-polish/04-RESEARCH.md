# Phase 4: Tmux Integration and Polish - Research

**Researched:** 2026-02-10
**Domain:** Terminal UI responsiveness, tmux integration, session persistence
**Confidence:** HIGH

## Summary

Phase 4 covers three distinct domains: (1) making the existing ratatui layout adapt to narrow terminal panes (30-40 columns), (2) writing now-playing metadata to a file for tmux status bar consumption, and (3) persisting playback session state (playlist + track position) across application restarts.

The existing codebase is well-positioned for all three. The ratatui constraint-based layout system already handles resizing at the buffer level -- `Terminal::draw()` automatically resizes internal buffers for fullscreen viewports on every call. What is missing is adaptive layout logic (checking `frame.area().width` and conditionally simplifying the UI), explicit `Event::Resize` handling in the event loop (for immediate redraw), tmux file output, and a session state struct in the config/data layer.

**Primary recommendation:** Split into two plans: (1) responsive layout + resize handling (DISP-09, DISP-10), and (2) tmux status bar file + session persistence (POL-03, POL-04, POL-05). No new dependencies needed -- all work uses the existing stack (ratatui, crossterm, serde, toml, dirs).

## Standard Stack

### Core (already in Cargo.toml -- no additions needed)

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| ratatui | 0.30 | TUI framework -- constraint-based layout, `frame.area()` for width checks | Already used; Layout/Constraint system is the responsive layout mechanism |
| crossterm | 0.29 | Terminal events -- `Event::Resize(cols, rows)` for resize detection | Already used; resize events come from the same `event::read()` already in the loop |
| serde + toml | 1.x / 0.8 | Session state serialization to TOML file | Already used for Config; same pattern extends to session state |
| dirs | 6.x | XDG-compliant paths for data dir (`~/.local/share/termtunes/`) | Already used for log file path |

### Supporting (no new crates)

No new dependencies are required for Phase 4. The existing stack covers all needs:
- Layout adaptation: `frame.area().width` + conditional rendering (pure ratatui)
- Resize handling: `Event::Resize` from crossterm (already in event loop, just needs a match arm)
- Tmux file: `std::fs::write` (stdlib)
- Session persistence: `serde::Serialize/Deserialize` + `toml::to_string_pretty` (already used)

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Plain `std::fs::write` for tmux file | `atomic-write-file` crate for atomic writes | Overkill for a single-line status file that is read by tmux `#(cat ...)` every 1-5s; partial reads are harmless for display-only data |
| TOML for session state | JSON (`serde_json`, already a dep) | TOML is consistent with existing config; session file lives alongside config |
| Separate session file | Embed session in config.toml | Separate file is cleaner -- session state is ephemeral/volatile, config is user-edited; mixing them risks config corruption on frequent writes |

## Architecture Patterns

### Current Project Structure (relevant files)

```
src/
  app.rs       # App struct, event loop, state machine -- needs: session save/restore, resize event
  ui.rs        # render() -- needs: adaptive layout based on frame width
  config.rs    # Config struct, load/save -- needs: session state struct + load/save
  main.rs      # Startup -- needs: session restore on init, session save before exit
  tui.rs       # Terminal lifecycle -- no changes needed
  player.rs    # Audio player -- no changes needed
  plex.rs      # Plex API client -- no changes needed
```

### Pattern 1: Adaptive Layout Based on Terminal Width

**What:** Check `frame.area().width` at render time and conditionally simplify the layout for narrow panes. Ratatui uses immediate-mode rendering, so you simply render different widgets/layouts based on conditions each frame.

**When to use:** Every render call -- the width check is essentially free.

**Implementation approach:**

```rust
// In ui.rs render()
pub fn render(frame: &mut Frame, app: &mut App) {
    let area = frame.area();
    let width = area.width;

    // Define breakpoints
    const NARROW: u16 = 40;  // 30-40 column panes
    const MIN_WIDTH: u16 = 20; // Below this, show "too small" message

    if width < MIN_WIDTH {
        // Render a "terminal too small" message
        let msg = Paragraph::new("Terminal\ntoo narrow")
            .alignment(Alignment::Center);
        frame.render_widget(msg, area);
        return;
    }

    // Existing layout logic, but adapt for narrow widths:
    // - In narrow mode: truncate track names, hide album, simplify status line
    // - In normal mode: render as-is (current code)
}
```

**Key width adaptations needed:**
- Player bar line 1 (track info): In narrow mode, show only track name (truncate), hide artist/album
- Player bar line 3 (status): In narrow mode, show only state icon + time, hide volume/shuffle/repeat text
- Track list items: Truncate to fit width (currently `">> {title} - {artist}"` can exceed 40 chars)
- Status bar help text: Drastically shorten for narrow widths
- Playlist items: Truncate long playlist names

### Pattern 2: Terminal Resize Event Handling

**What:** Add `Event::Resize` arm to the event matching in `app.rs`. For fullscreen ratatui apps, `Terminal::draw()` auto-resizes buffers, so the resize event just needs to trigger an immediate redraw (which already happens on the next loop iteration with 100ms poll timeout).

**When to use:** In the event loop, alongside Key events.

**Implementation approach:**

```rust
// In app.rs run() method, the event polling section:
if event::poll(Duration::from_millis(100))? {
    match event::read()? {
        Event::Key(key) if key.kind == KeyEventKind::Press => {
            self.handle_key(key.code, key.modifiers).await?;
        }
        Event::Resize(_cols, _rows) => {
            // ratatui auto-resizes on next draw() call for fullscreen viewports.
            // We just need the loop to continue so draw() is called again.
            // No explicit action needed -- the match arm prevents the event
            // from being silently dropped, and the loop continues naturally.
        }
        _ => {}
    }
}
```

**Critical insight:** The current code uses `if let Event::Key(key) = event::read()` which silently drops Resize events. Changing to a `match` statement is the fix. The actual buffer resize is handled by ratatui internally on the next `terminal.draw()` call.

### Pattern 3: Tmux Status Bar File Integration

**What:** Write current track info to a well-known file that tmux reads via `#(cat /path/to/file)` in status-right.

**When to use:** Every time the now-playing state changes (track starts, track ends, app exits).

**Implementation approach:**

```rust
// File location: ~/.local/share/termtunes/now_playing
// Content when playing: "Artist - Track Name"
// Content when paused:  "|| Artist - Track Name"
// Content when stopped/no track/app closed: "" (empty file)

fn write_now_playing(np: Option<&NowPlaying>, is_paused: bool) {
    let data_dir = dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("~/.local/share"))
        .join("termtunes");
    let path = data_dir.join("now_playing");

    let content = match np {
        Some(np) if is_paused => format!("|| {} - {}", np.artist, np.track_name),
        Some(np) => format!("{} - {}", np.artist, np.track_name),
        None => String::new(),
    };

    let _ = std::fs::write(&path, &content); // Best-effort, don't crash on failure
}
```

**Tmux configuration (user-facing documentation):**

```tmux
# In ~/.tmux.conf:
set -g status-interval 5
set -g status-right "#(cat ~/.local/share/termtunes/now_playing) | %H:%M"
```

### Pattern 4: Session Persistence

**What:** Save the current playback session (which playlist, which track index, playback position, volume, shuffle/repeat state) to a TOML file on exit, and restore it on startup.

**When to use:** Save on graceful exit (`q` key or signal shutdown). Restore during App::new() initialization.

**Implementation approach:**

```rust
// New struct in config.rs:
#[derive(Serialize, Deserialize, Default, Debug)]
pub struct Session {
    /// Rating key of the last-played playlist.
    pub playlist_rating_key: Option<String>,
    /// Title of the last-played playlist (for display before fetching).
    pub playlist_title: Option<String>,
    /// Index of the last-played track in the playlist.
    pub track_index: Option<usize>,
    /// Volume level (0.0 to 1.0).
    pub volume: f32,
    /// Whether shuffle was enabled.
    pub shuffle_enabled: bool,
    /// Repeat mode (stored as string for TOML readability).
    pub repeat_mode: String, // "off", "all", "one"
}

// File location: ~/.local/share/termtunes/session.toml
// Separate from config.toml because:
// 1. Session state changes frequently (every track change)
// 2. Config is user-editable; session is app-managed
// 3. Session loss is non-critical; config loss is bad
```

**Restore flow:**
1. On startup, after playlists are fetched, check for session.toml
2. If session exists and playlist_rating_key matches a fetched playlist, fetch that playlist's tracks
3. Set track_state selection to saved track_index
4. Restore volume, shuffle, repeat mode
5. Do NOT auto-play -- just position the user where they left off (they press Enter to resume)

**Save flow:**
1. Before exiting (in the graceful shutdown path), serialize current state to session.toml
2. Also clear the now_playing file on exit

### Anti-Patterns to Avoid

- **Hardcoding width breakpoints inline:** Define constants (e.g., `const NARROW_WIDTH: u16 = 40;`) at the top of ui.rs, not scattered through render functions.
- **Auto-playing on session restore:** Resuming playback automatically would be surprising behavior. Restore the position but let the user initiate playback.
- **Saving session on every track change:** Only save on app exit. Frequent writes to disk for volatile state are wasteful and risk partial writes. If the app crashes, losing session is acceptable.
- **Using config.toml for session state:** Config is user-editable and contains auth tokens. Session state is volatile and app-managed. Keep them separate.
- **Blocking on now_playing file writes:** Use best-effort writes with `let _ = std::fs::write(...)`. The file is for tmux display; failure is cosmetic, not critical.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Text truncation with ellipsis | Custom character-by-character truncation | Simple `String::truncate` + "..." suffix when `str.len() > max` | Unicode-aware truncation is tricky but for ASCII music metadata, simple truncation suffices; ratatui auto-truncates lines that exceed area width |
| XDG data directory path | Hardcoded `~/.local/share` | `dirs::data_dir()` (already used in main.rs) | Consistent with existing code; respects XDG_DATA_HOME |
| TOML serialization | Manual string formatting | `toml::to_string_pretty` + serde derive (already used for Config) | Exact same pattern as config::save_config |
| Atomic file writes for now_playing | tmp file + rename dance | Plain `std::fs::write` | Single-line file read by `cat` every 1-5s; partial reads display garbage for one interval then self-correct |

**Key insight:** Phase 4 requires no new libraries. Every capability needed is already in the dependency tree.

## Common Pitfalls

### Pitfall 1: Panic on Zero-Width or Zero-Height Terminal

**What goes wrong:** Layout calculations with `Constraint::Percentage` or `Constraint::Ratio` can produce zero-size areas, and some widgets panic when rendered into a zero-size Rect.
**Why it happens:** User resizes terminal to extremely small size (1-2 rows/columns), or a tmux pane is collapsed.
**How to avoid:** Add a minimum size check at the top of `render()`. If width < 20 or height < 5, render a simple "too small" message and return early -- skip all layout calculations.
**Warning signs:** Panic stack traces mentioning `Layout::split`, `LineGauge::render`, or division by zero.

### Pitfall 2: Now-Playing File Left Stale After Crash

**What goes wrong:** If the app crashes (panic, SIGKILL), the now_playing file still contains the last track info, so tmux displays stale data indefinitely.
**Why it happens:** Cleanup only runs on graceful exit; crashes bypass it.
**How to avoid:** Two strategies: (1) Include a timestamp in the file, and have the tmux cat script check staleness. (2) Accept it -- the user will notice when they reopen the app (which clears/updates the file). Option 2 is simpler and acceptable.
**Warning signs:** Tmux showing a track name when no music is playing.

### Pitfall 3: Session Restore Fails Silently When Playlist Was Deleted

**What goes wrong:** User saves session with playlist X, then deletes playlist X from Plex. On restore, the rating key no longer exists, and `fetch_tracks` returns an error or empty list.
**Why it happens:** Session state references server-side data that can change independently.
**How to avoid:** Treat session restore as best-effort. If the playlist is not found in the fetched playlists list (match by rating_key), skip session restore and start fresh. Log a warning.
**Warning signs:** Empty tracks list after restore, or error fetching tracks for saved rating key.

### Pitfall 4: Truncation Breaks Multi-Byte Unicode Characters

**What goes wrong:** Truncating a string at a byte offset can split a multi-byte UTF-8 character, causing a panic or garbled output.
**Why it happens:** `String::truncate(n)` panics if `n` is not on a char boundary.
**How to avoid:** Use `str.chars().take(n).collect::<String>()` for character-aware truncation, or use `unicode-width` crate for display-width-aware truncation. For Phase 4, character-based truncation with `.chars().take(max).collect::<String>()` is sufficient since most music metadata is ASCII/Latin.
**Warning signs:** Panic with "byte index is not a char boundary" in truncation code.

### Pitfall 5: Event::Resize Not Matched in Current Event Loop

**What goes wrong:** The current code uses `if let Event::Key(key) = event::read()` which silently drops all non-Key events including Resize. This means resize events are consumed but not acted upon, and the next draw happens only at the 100ms poll timeout anyway.
**Why it happens:** Original event loop only cared about Key events.
**How to avoid:** Change to `match event::read()` with arms for both `Event::Key` and `Event::Resize`. For fullscreen ratatui, Resize doesn't need explicit handling (draw auto-resizes), but matching it explicitly is good practice and prevents confusion.
**Warning signs:** Technically the app already handles resize (the next draw() call resizes buffers), but the pattern should be explicit for clarity and correctness.

### Pitfall 6: Session File Permission Mismatch with Config

**What goes wrong:** Session file is created with default permissions (0o644) while config file uses 0o600. Session file may contain playlist titles and rating keys (not secret, but still user data).
**Why it happens:** Inconsistent permission handling between config and session save functions.
**How to avoid:** Use the same permission pattern (0o600) for session.toml as for config.toml, or use 0o644 since session data is non-sensitive. Recommend 0o600 for consistency.
**Warning signs:** Different file permissions between config.toml and session.toml.

## Code Examples

Verified patterns from the existing codebase and official documentation:

### Checking Terminal Width for Adaptive Layout

```rust
// Source: ratatui docs - frame.area() returns Rect with width/height
// Applied to existing ui.rs pattern

pub fn render(frame: &mut Frame, app: &mut App) {
    let area = frame.area();

    // Minimum viable display
    if area.width < 20 || area.height < 5 {
        let msg = Paragraph::new("Terminal too small")
            .style(Style::default().fg(Color::Red))
            .alignment(Alignment::Center);
        frame.render_widget(msg, area);
        return;
    }

    let is_narrow = area.width < 40;

    // Pass width info to sub-renderers
    // ... rest of layout
}
```

### Handling Resize Events in Event Loop

```rust
// Source: crossterm docs + ratatui FAQ
// Applied to existing app.rs event loop pattern

// BEFORE (current code):
if event::poll(Duration::from_millis(100))? {
    if let Event::Key(key) = event::read()? {
        if key.kind == KeyEventKind::Press {
            self.handle_key(key.code, key.modifiers).await?;
        }
    }
}

// AFTER (with resize handling):
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
```

### Writing Now-Playing File for Tmux

```rust
// Source: stdlib std::fs::write + existing dirs usage in main.rs

use std::path::PathBuf;

/// Write now-playing info to a file for tmux status bar consumption.
///
/// File: ~/.local/share/termtunes/now_playing
/// Format: "Artist - Track" (playing) or "" (stopped/no track)
///
/// Best-effort: errors are logged but do not affect playback.
fn write_now_playing_file(content: &str) {
    let path = dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from(
            format!("{}/.local/share", std::env::var("HOME").unwrap_or_default())
        ))
        .join("termtunes")
        .join("now_playing");

    if let Err(e) = std::fs::write(&path, content) {
        tracing::warn!("Failed to write now_playing file: {}", e);
    }
}
```

### Session State Serialization

```rust
// Source: existing config.rs pattern (Config struct + save_config/load_config)

#[derive(Serialize, Deserialize, Default, Debug)]
pub struct Session {
    pub playlist_rating_key: Option<String>,
    pub playlist_title: Option<String>,
    pub track_index: Option<usize>,
    pub volume: f32,
    pub shuffle_enabled: bool,
    pub repeat_mode: String,
}

pub fn session_path() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from(
            format!("{}/.local/share", std::env::var("HOME").unwrap_or_default())
        ))
        .join("termtunes")
        .join("session.toml")
}

pub fn load_session() -> Option<Session> {
    let path = session_path();
    if !path.exists() {
        return None;
    }
    let contents = std::fs::read_to_string(&path).ok()?;
    toml::from_str(&contents).ok()
}

pub fn save_session(session: &Session) -> Result<()> {
    let path = session_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let contents = toml::to_string_pretty(session)?;
    std::fs::write(&path, &contents)?;
    Ok(())
}
```

### Truncating Track Names for Narrow Display

```rust
// Source: Rust stdlib -- character-safe truncation

/// Truncate a string to fit within `max_chars` characters, adding "..." suffix.
fn truncate_for_display(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        s.to_string()
    } else if max_chars <= 3 {
        s.chars().take(max_chars).collect()
    } else {
        let truncated: String = s.chars().take(max_chars - 3).collect();
        format!("{}...", truncated)
    }
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| `tui::Terminal` with manual resize | ratatui 0.30 `Terminal::draw()` auto-resizes buffers for fullscreen viewports | ratatui 0.26+ | No manual `autoresize()` call needed for fullscreen apps |
| `if let Event::Key(key) = event::read()` | `match event::read()` with explicit Resize arm | Best practice since crossterm 0.14 | Prevents silently dropped events; clearer intent |
| Custom layout breakpoint systems | ratatui `Constraint::Min`/`Max` + conditional rendering | Always available in ratatui | Simple width checks + immediate-mode rendering = responsive UI |

**Deprecated/outdated:**
- `terminal.autoresize()`: Still available but unnecessary for fullscreen viewports -- `draw()` handles it automatically. Only needed for inline viewports.

## Open Questions

1. **What content format for the now_playing file?**
   - What we know: Simple text file, one line, read by tmux `#(cat ...)`.
   - What's unclear: Should we include play/pause state? Duration? Just artist+track?
   - Recommendation: Keep it minimal -- `"Artist - Track"` when playing, empty when stopped. Users who want more detail can check the TUI itself. Paused state indicator is useful: `"|| Artist - Track"`.

2. **Should session restore auto-play or just position?**
   - What we know: Prior decision says "restores last session on startup" (POL-05).
   - What's unclear: Does "restore" mean auto-play or just set the playlist/track selection?
   - Recommendation: Restore position only (select playlist, select track, restore volume/shuffle/repeat) but do NOT auto-play. Unexpected audio on app start is poor UX, especially in a work tmux environment. The user presses Enter or Space to resume.

3. **When to save session state?**
   - What we know: Need to persist across restarts.
   - What's unclear: Save on every track change, or only on exit?
   - Recommendation: Save on graceful exit only. If app crashes, session loss is acceptable. Saving on every track change adds unnecessary disk I/O and complicates the code. The existing exit paths (q key, Ctrl+C, signal handlers) all converge to a single exit point after the event loop.

4. **Minimum terminal width/height for the app?**
   - What we know: Requirement says "30-40 columns". Need to handle even smaller gracefully.
   - What's unclear: Exact minimum before showing "too small" message.
   - Recommendation: Below 20 columns or 5 rows: show "too small" message. Between 20-39 columns: narrow/compact mode. 40+ columns: full mode (current layout).

## Sources

### Primary (HIGH confidence)
- Ratatui official docs (https://ratatui.rs/concepts/layout/) - Layout constraints, responsive design
- Ratatui FAQ (https://ratatui.rs/faq/) - Resize event handling pattern
- Crossterm docs (https://docs.rs/crossterm/latest/crossterm/event/) - Event::Resize type
- Ratatui Terminal docs (https://docs.rs/ratatui/latest/ratatui/struct.Terminal.html) - Auto-resize behavior for fullscreen viewports
- Context7 /websites/ratatui_rs - Layout patterns, resize handling examples
- Context7 /crossterm-rs/crossterm - Event polling and resize events

### Secondary (MEDIUM confidence)
- Tmux man page (https://man7.org/linux/man-pages/man1/tmux.1.html) - `#(shell-command)` in status-right, status-interval
- Tao of Tmux status bar guide (https://tao-of-tmux.readthedocs.io/en/latest/manuscript/09-status-bar.html) - Shell command execution in status line
- Serde official docs (https://serde.rs/) - Serialize/Deserialize derive patterns

### Tertiary (LOW confidence)
- None -- all findings verified with official sources

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH -- No new dependencies; all capabilities already in Cargo.toml
- Architecture: HIGH -- Patterns directly extend existing code (config.rs save/load, ui.rs render, app.rs event loop)
- Pitfalls: HIGH -- Verified against actual codebase analysis (found Event::Resize not handled, found exact code to change)

**Research date:** 2026-02-10
**Valid until:** 2026-03-10 (stable -- ratatui 0.30, crossterm 0.29, no API changes expected)
