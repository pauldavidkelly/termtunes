# Phase 8: Ambient Status UI & Controls - Research

**Researched:** 2026-02-11
**Domain:** Ratatui UI layout extension + keybinding design for ambient channel visibility and control
**Confidence:** HIGH

## Summary

Phase 8 adds user-facing visibility and control for the ambient audio channel that was built in Phase 6 (dual-sink engine) and Phase 7 (track browsing/selection). The existing codebase already has all ambient state accessible: `Player::ambient_track_name()`, `App::ambient_volume()`, `Player::has_ambient_sink()`, and the mute toggle keybinding (`m`). What is missing is: (1) a dedicated UI panel that displays ambient status at a glance, (2) keybindings for adjusting ambient volume up/down independently, and (3) proper keybinding for toggling ambient on/off that goes beyond the current basic mute/unmute.

The implementation is purely UI and keybinding wiring -- no new libraries, no new audio engine work, no Plex API calls. The ratatui layout needs to be extended to insert a 1-line ambient status panel between the main content and the player bar. This follows the exact same layout pattern already used for the visualizer area (conditionally inserted between content and player bar). The ambient panel should show: track name (or "No ambient"), play/pause state icon, and volume percentage. Keybindings for ambient volume up/down need dedicated keys that do not conflict with existing bindings.

The main decision points are: (1) where to place the ambient panel in the layout, (2) which keys to use for ambient volume adjustment, and (3) how to handle the 'm' key -- currently it toggles between volume=0 and volume=0.3, but the requirements specify "toggle ambient on/off" which maps cleanly to the existing mute/unmute behavior. The current `mute_ambient()`/`unmute_ambient()` implementation already handles this (ambient_volume=0 is "off", ambient_volume>0 is "on"), but unmuting always restores to 0.3 regardless of the previous volume. This should be improved to remember the pre-mute volume.

**Primary recommendation:** Add a 1-line ambient status panel below the main content area (above the player bar), wire `[` and `]` as ambient volume down/up keybindings, and add a `pre_mute_ambient_volume` field to remember volume before muting so toggle restore is accurate.

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| ratatui | 0.30 | TUI framework -- Layout extension, Paragraph/Span widgets | Already in use; same patterns as existing player bar |
| crossterm | 0.29 | Terminal input -- new keybinding handlers | Already in use; raw mode key capture |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| tracing | 0.1 | Logging volume changes and toggle events | Already in use; structured logging |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| 1-line ambient panel | Merge ambient info into existing player bar line 3 | Cleaner separation, ambient panel can be hidden when no ambient track loaded |
| `[`/`]` for ambient volume | `a`/`A` for ambient down/up | `[`/`]` is more intuitive for "secondary" volume; `a`/`A` could be confused with "ambient" as a general prefix |
| Remembering pre-mute volume | Always unmuting to 0.3 default | Users who set ambient to 0.5 would lose their setting on mute/unmute cycle |

**Installation:**
```bash
# No new dependencies needed. Existing Cargo.toml is sufficient.
```

## Architecture Patterns

### Recommended Project Structure
```
src/
  ui.rs         # Extended: render_ambient_panel() function, layout adjustment
  app.rs        # Extended: ambient_volume_up/down methods, pre_mute_volume field, new keybindings
  player.rs     # Unchanged (all ambient accessors already exist from Phase 6)
  main.rs       # Unchanged
  plex.rs       # Unchanged
```

### Pattern 1: Conditional Layout Extension for Ambient Panel
**What:** Insert a 1-line ambient status panel into the vertical layout, conditionally shown when an ambient track is loaded (or always shown with "No ambient" text).
**When to use:** When the ambient channel has state worth showing to the user.
**Example:**
```rust
// Source: Existing visualizer conditional layout pattern in ui.rs lines 57-96
// The visualizer already demonstrates this pattern: conditionally insert an
// area between main content and player bar based on state.

// New layout when ambient is active:
// [main_area: Fill(1)] [ambient_area: Length(1)] [viz_area: Length(8)] [bar_area: Length(3)]
// When ambient is inactive, ambient_area is omitted.

let has_ambient = app.player().is_some_and(|p| p.has_ambient_sink())
    || app.player().is_some_and(|p| p.ambient_track_name().is_some());

if has_ambient && show_viz {
    // 4-part layout: main, ambient, visualizer, player bar
    let [main_area, ambient_area, viz_area, bar_area] = Layout::vertical([
        Constraint::Fill(1),
        Constraint::Length(1),
        Constraint::Length(8),
        Constraint::Length(bar_height),
    ])
    .areas(area);
    // ... render each area
} else if has_ambient {
    // 3-part layout: main, ambient, player bar
    let [main_area, ambient_area, bar_area] = Layout::vertical([
        Constraint::Fill(1),
        Constraint::Length(1),
        Constraint::Length(bar_height),
    ])
    .areas(area);
    // ... render each area
}
```

### Pattern 2: Inline Multi-Span Status Line
**What:** Build a single-line status display using `Line::from(vec![Span, Span, ...])` with different colors for each component.
**When to use:** Any compact status display that combines multiple data points in one line.
**Example:**
```rust
// Source: Existing player bar line 3 pattern (ui.rs lines 354-425)
// The player bar status line already uses this pattern for state + volume + time.

fn render_ambient_panel(frame: &mut Frame, app: &App, area: Rect) {
    let player = app.player();

    let track_name = player
        .and_then(|p| p.ambient_track_name())
        .unwrap_or("No ambient");

    let is_active = app.ambient_volume() > 0.0;

    let state_icon = if is_active {
        Span::styled(" AMB >> ", Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD))
    } else {
        Span::styled(" AMB || ", Style::default().fg(Color::DarkGray))
    };

    let name_span = Span::styled(
        track_name,
        Style::default().fg(if is_active { Color::White } else { Color::DarkGray }),
    );

    let sep = Span::styled(" | ", Style::default().fg(Color::DarkGray));

    let vol_pct = (app.ambient_volume() * 100.0).round() as u8;
    let vol_span = Span::styled(
        format!("Vol: {}%", vol_pct),
        Style::default().fg(if is_active { Color::Magenta } else { Color::DarkGray }),
    );

    let line = Line::from(vec![state_icon, name_span, sep, vol_span]);
    frame.render_widget(Paragraph::new(line), area);
}
```

### Pattern 3: Pre-Mute Volume Memory
**What:** Store the ambient volume before muting so that unmuting restores the exact previous level instead of a hardcoded default.
**When to use:** Any toggle-off/toggle-on control where the user expects their setting to be preserved.
**Example:**
```rust
// Source: Common UX pattern; fixes current hardcoded 0.3 restore in unmute_ambient()

// New field on App:
pre_mute_ambient_volume: f32, // default: 0.3

fn toggle_ambient(&mut self) {
    if self.ambient_volume > 0.0 {
        // Muting: save current volume, set to 0
        self.pre_mute_ambient_volume = self.ambient_volume;
        self.ambient_volume = 0.0;
    } else {
        // Unmuting: restore saved volume
        self.ambient_volume = self.pre_mute_ambient_volume;
    }
    self.apply_ambient_volume();
}
```

### Pattern 4: Dedicated Ambient Volume Keybindings
**What:** Add `[` and `]` keys for ambient volume down/up, separate from the main `+`/`-` volume keys.
**When to use:** When two independent volume channels need independent keybindings.
**Example:**
```rust
// Source: Existing volume_up/volume_down pattern in app.rs lines 1247-1256
// Same step size (0.05), same clamping pattern, different target.

// In handle_key():
(KeyCode::Char(']'), _) => {
    self.ambient_volume_up();
}
(KeyCode::Char('['), _) => {
    self.ambient_volume_down();
}

fn ambient_volume_up(&mut self) {
    self.ambient_volume = (self.ambient_volume + 0.05).min(1.0);
    self.apply_ambient_volume();
}

fn ambient_volume_down(&mut self) {
    self.ambient_volume = (self.ambient_volume - 0.05).max(0.0);
    self.apply_ambient_volume();
}
```

### Anti-Patterns to Avoid
- **Rendering the ambient panel inside the player bar:** The player bar is already 3 lines and tightly structured. Adding ambient info there makes both harder to maintain. Use a separate 1-line panel.
- **Always showing the ambient panel even when no ambient exists:** This wastes a line of screen space. Show it only when an ambient track has been loaded (or is playing).
- **Separate `ambient_muted: bool` state alongside `ambient_volume`:** The existing design uses volume=0 as "muted." Adding a separate boolean creates two sources of truth. Keep the single `ambient_volume` field with `pre_mute_ambient_volume` for restore.
- **Using the same `+`/`-` keys for ambient volume when in some "mode":** Modal volume control is confusing. Dedicated keys are clearer and allow simultaneous adjustment.
- **Forgetting to update the status bar help text:** The status bar at the bottom shows keybinding hints. New keybindings must be added there.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Layout with conditional panels | Manual Rect math for inserting ambient panel | `Layout::vertical()` with conditional `Constraint::Length(1)` | Already proven pattern (visualizer area uses this exact approach) |
| Multi-colored inline text | Custom rendering with multiple Paragraph widgets | `Line::from(vec![Span::styled(...), ...])` in a single Paragraph | Built-in, already used in player bar line 1 and line 3 |
| Volume step clamping | Custom min/max logic | `(vol + 0.05).min(1.0)` / `(vol - 0.05).max(0.0)` | Existing pattern from main volume, simple and proven |

**Key insight:** Phase 8 is wiring existing UI patterns (Span-based status lines, conditional Layout areas) to existing ambient state (ambient_volume, ambient_track_name). No new rendering techniques or state management needed.

## Common Pitfalls

### Pitfall 1: Ambient Panel Steals Space from Main Content Area
**What goes wrong:** Adding a 1-line ambient panel reduces the main content area by 1 line. In very small terminals (minimum height), this could leave too little space for the track list.
**Why it happens:** The main content area uses `Constraint::Fill(1)` which gets whatever is left after fixed-height areas.
**How to avoid:** Only show the ambient panel when terminal height is sufficient. Use the same approach as the visualizer: check `area.height >= MIN_HEIGHT_THRESHOLD` before inserting the panel. The existing `MIN_VIZ_HEIGHT` (20) check is a good model. A reasonable ambient panel threshold is lower (e.g., 8 lines minimum) since it is only 1 line.
**Warning signs:** Track list becomes unusable in small terminal panes.

### Pitfall 2: Ambient Volume Recreation Latency on Rapid Key Presses
**What goes wrong:** Each ambient volume change calls `apply_ambient_volume()` which calls `Player::set_ambient_volume()`. The current implementation recreates the entire ambient sink on every volume change (stop -> new sink -> re-decode -> append). Rapid `]` key presses cause audible gaps or stuttering.
**Why it happens:** The `set_ambient_volume()` workaround exists because `Sink::set_volume()` was unreliable for ambient sinks. But recreating the sink on every 0.05 step is heavy.
**How to avoid:** Try using `Sink::set_volume()` first (it may work for volume adjustments on an existing sink even if it was unreliable for initial volume setting). Only fall back to sink recreation if `set_volume()` does not take effect. Alternatively, debounce volume changes (only apply after 200ms of no further changes). The simplest approach: just try `set_volume()` directly and verify it works. The original issue may have been specific to initial sink creation, not ongoing volume changes.
**Warning signs:** Ambient audio stutters or goes silent when pressing `[`/`]` rapidly.

### Pitfall 3: Forgetting Narrow Mode for Ambient Panel
**What goes wrong:** The ambient panel renders fine in normal width but overflows or looks broken in narrow terminals (<40 columns).
**Why it happens:** The existing code has `is_narrow` checks throughout render functions. A new panel must also handle narrow mode.
**How to avoid:** In narrow mode, show abbreviated ambient info (just the state icon and track name, drop volume). Follow the same pattern as the player bar's narrow mode handling (lines 298-311 in ui.rs).
**Warning signs:** Text overflow or missing content in narrow terminal panes.

### Pitfall 4: Status Bar Help Text Gets Too Long
**What goes wrong:** Adding `[/]:amb vol  m:amb mute` to the status bar help text pushes the line past the terminal width, causing truncation or wrapping.
**Why it happens:** The status bar is a single line with many keybindings already listed.
**How to avoid:** In narrow mode, drop the new ambient keybinding hints. In normal mode, abbreviate: `[/]:amb` or include selectively. The status bar text already gets long -- consider what is most important.
**Warning signs:** Status bar text wraps or is truncated, hiding important keybinding hints.

### Pitfall 5: Mute Toggle Does Not Reflect in Ambient Panel Immediately
**What goes wrong:** User presses `m` to mute ambient, but the ambient panel still shows the old volume for a render frame.
**Why it happens:** The `mute_ambient()` method updates `ambient_volume` and calls `apply_ambient_volume()`, but if the UI reads the old value before the next render tick, there is a visual lag.
**How to avoid:** This is not actually an issue -- the event loop handles the keypress synchronously and then draws the UI immediately after. The `handle_key()` call completes before `terminal.draw()` is called. As long as `mute_ambient()` updates `self.ambient_volume` synchronously (which it does), the next draw will show the correct value. No extra work needed.
**Warning signs:** None expected -- this is a false alarm, documented to prevent unnecessary workaround code.

## Code Examples

Verified patterns from the existing codebase:

### Ambient State Accessors (Already Exist)
```rust
// Source: player.rs lines 499-505, app.rs lines 439-447
// All state needed for UI rendering is already accessible:

// In Player:
pub fn ambient_track_name(&self) -> Option<&str>  // Track name or None
pub fn ambient_volume(&self) -> Option<f32>         // Sink volume or None
pub fn has_ambient_sink(&self) -> bool              // Whether ambient is loaded

// In App:
pub fn ambient_volume(&self) -> f32                 // Raw ambient volume (0.0-1.0)
pub fn player(&self) -> Option<&Player>             // Access to player for ambient state
```

### Conditional Visualizer Layout Pattern (Template for Ambient)
```rust
// Source: ui.rs lines 57-96 -- existing conditional layout insertion
// This EXACT pattern will be extended for the ambient panel.
if show_viz {
    let [main_area, viz_area, bar_area] = Layout::vertical([
        Constraint::Fill(1),
        Constraint::Length(8),
        Constraint::Length(bar_height),
    ])
    .areas(area);
    // ...
} else {
    let [main_area, bar_area] =
        Layout::vertical([Constraint::Fill(1), Constraint::Length(bar_height)]).areas(area);
    // ...
}
// Extend by adding Constraint::Length(1) for ambient panel when ambient is active.
```

### Player Bar Status Line Pattern (Template for Ambient Panel)
```rust
// Source: ui.rs lines 354-425 -- existing multi-span status line
// Same pattern: build a Line from styled Spans, render as Paragraph.
let status_line = Line::from(vec![
    Span::styled(" Playing ", Style::default().fg(Color::Green)),
    Span::styled(" | ", Style::default().fg(Color::DarkGray)),
    Span::styled("Vol: 80%", Style::default().fg(Color::White)),
    Span::styled(" | ", Style::default().fg(Color::DarkGray)),
    Span::styled("03:45 / 05:12", Style::default().fg(Color::White)),
]);
frame.render_widget(Paragraph::new(status_line), line3_area);
```

### Main Volume Keybinding Pattern (Template for Ambient Volume)
```rust
// Source: app.rs lines 644-650 -- existing volume up/down pattern
// Volume up (PLAY-06) -- + or =
(KeyCode::Char('+'), _) | (KeyCode::Char('='), _) => {
    self.volume_up();
}
// Volume down (PLAY-07) -- - or _
(KeyCode::Char('-'), _) | (KeyCode::Char('_'), _) => {
    self.volume_down();
}

fn volume_up(&mut self) {
    self.saved_volume = (self.saved_volume + 0.05).min(1.0);
    self.apply_main_volume();
}
```

## Keybinding Design

### Current Keybinding Map (All Occupied Keys)
| Key | Action | Context |
|-----|--------|---------|
| `q` | Quit | Global |
| `Ctrl+C` | Quit | Global |
| `Space` | Toggle play/pause | Global |
| `+`/`=` | Main volume up | Global |
| `-`/`_` | Main volume down | Global |
| `n`/`>` | Next track | Tracks/Playing |
| `N`/`<` | Previous track | Tracks/Playing |
| `j`/Down | Move selection down | Lists |
| `k`/Up | Move selection up | Lists |
| `Enter` | Select item | Lists |
| `s` | Toggle shuffle | Global |
| `r` | Cycle repeat mode | Global |
| `l`/Right | Seek forward | Playing |
| `h`/Left | Seek backward | Playing |
| `b` | Open browser | Global |
| `m` | Toggle ambient mute | Global |
| `v` | Toggle visualizer | Global |
| `f` | Assign favorite | Playlists |
| `1`-`9` | Play favorite | Global |
| `Esc`/Backspace | Go back | Navigation |

### Recommended New Keybindings
| Key | Action | Rationale |
|-----|--------|-----------|
| `]` | Ambient volume up | Paired bracket keys, visually "secondary" to main `+`/`-` |
| `[` | Ambient volume down | Paired bracket keys, easy to reach from home row |
| `m` | Toggle ambient on/off | Already bound -- keep as-is, improve to remember pre-mute volume |

**Why `[`/`]`:**
- Visually paired (like `+`/`-` for main volume)
- Not occupied by any existing binding
- Accessible on standard keyboard layouts without Shift
- Convey "enclosure" / "secondary" meaning distinct from main `+`/`-`

**Alternative considered:** `a`/`A` (ambient mnemonic). Rejected because lowercase `a` could be confused with a navigation key, and Shift keys are already used for `N`/`<` (previous track).

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| No ambient UI | Dedicated ambient status panel | Phase 8 | User gains visibility into ambient state |
| `m` key always unmutes to 0.3 | `m` key restores pre-mute volume | Phase 8 | Better UX for volume preservation |
| No ambient volume keybindings | `[`/`]` for ambient volume | Phase 8 | User can adjust without opening browser |

**Deprecated/outdated:**
- `mute_ambient()` / `unmute_ambient()` as separate methods with hardcoded 0.3: Replace with single `toggle_ambient()` using pre-mute volume memory.

## Open Questions

1. **Should the ambient panel be visible even when no ambient track has been loaded?**
   - What we know: Showing "No ambient" uses 1 line of space for no value. Hiding it saves space.
   - What's unclear: Whether users want the panel always visible as a reminder that the feature exists.
   - Recommendation: Only show when an ambient track is loaded (has_ambient_data or has_ambient_sink). This saves space and the user already knows about the feature from the keybinding help text. The panel appearing after they select a track from the browser is a natural confirmation.

2. **Should ambient volume step size match main volume (0.05 = 5%)?**
   - What we know: Main volume uses 0.05 step. Ambient is typically lower (0.0-0.5 range), so 20 key presses covers the full range from 0 to 1.0.
   - What's unclear: Whether a smaller step (0.02 = 2%) would be better for fine-tuning ambient levels.
   - Recommendation: Use 0.05 (same as main) for consistency. 20 presses for full range is reasonable. If too coarse, reduce to 0.03 later as a refinement.

3. **Volume recreation vs set_volume for ambient volume changes**
   - What we know: `Player::set_ambient_volume()` currently recreates the entire ambient sink on each call due to unreliable `Sink::set_volume()`. This may cause audible gaps on rapid key presses.
   - What's unclear: Whether `Sink::set_volume()` actually fails for ongoing ambient playback, or if the original issue was limited to initial volume setting.
   - Recommendation: First try changing `set_ambient_volume()` to just call `ambient_sink.set_volume()` directly without recreation. Test with rapid key presses. If volume changes are reflected correctly, use the simpler approach. If not, fall back to sink recreation. This is the highest-risk code change in Phase 8.

## Sources

### Primary (HIGH confidence)
- Existing codebase: `ui.rs` (layout patterns, player bar rendering, visualizer conditional insertion), `app.rs` (keybinding handling, volume methods, ambient state), `player.rs` (ambient accessors: ambient_track_name, ambient_volume, has_ambient_sink)
- [Ratatui Layout documentation](https://ratatui.rs/concepts/layout/) -- Layout::vertical with Constraint patterns
- [Ratatui Paragraph widget docs](https://docs.rs/ratatui/latest/ratatui/widgets/struct.Paragraph.html) -- Paragraph with Line/Span styled text
- [Ratatui text styling recipes](https://ratatui.rs/recipes/render/style-text/) -- Span::styled patterns

### Secondary (MEDIUM confidence)
- [Ratatui popup example](https://ratatui.rs/examples/apps/popup/) -- Layout::Flex::Center pattern (used in Phase 7, relevant as layout reference)

### Tertiary (LOW confidence)
- Ambient volume `Sink::set_volume()` reliability: The original `set_ambient_volume()` workaround (sink recreation) was added during Phase 6 implementation. Whether `set_volume()` actually fails for ongoing playback needs empirical validation. Recommendation to try simpler approach first is LOW confidence.

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - no new dependencies, all ratatui/crossterm widgets already in use
- Architecture: HIGH - conditional layout insertion and multi-span status lines are proven patterns in the existing codebase (visualizer area, player bar)
- Keybinding design: HIGH - `[`/`]` are unoccupied, pattern matches existing `+`/`-` for main volume
- Pitfalls: HIGH - all derived from existing codebase analysis and known patterns
- Ambient volume set_volume reliability: LOW - needs empirical validation, may require keeping sink recreation approach

**Research date:** 2026-02-11
**Valid until:** 2026-03-11 (stable domain -- ratatui 0.30, existing codebase patterns, no fast-moving dependencies)
