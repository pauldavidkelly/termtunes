# Phase 7: Track Browsing & Ambient Playback - Research

**Researched:** 2026-02-11
**Domain:** Plex library browsing API + ratatui modal overlay + ambient track selection pipeline
**Confidence:** HIGH

## Summary

Phase 7 adds a modal track browser overlay that lets the user browse their Plex music library sections, select a track, and load it as the ambient channel. The ambient playback engine already exists (Phase 6 delivered dual-sink audio, background download thread, ambient loop detection, and failure-isolated loading). This phase is purely about the browsing UI and the Plex API calls to discover music library sections and their tracks.

The Plex API provides two endpoints needed: `GET /library/sections` to list library sections (filtering for `type="artist"` which identifies music libraries), and `GET /library/sections/{id}/all?type=10` to list all tracks in a music section as a flat list. The existing `PlexClient` already handles JSON deserialization with the `Accept: application/json` header, so extending it with two new methods is straightforward. Per the requirements document, hierarchical artist/album browsing is explicitly **out of scope** -- the track list is flat.

The ratatui popup overlay pattern is well-documented: calculate a centered `Rect`, render `Clear` to erase the background region, then render a `Block` with borders containing a `List` widget with `ListState` for navigation. The existing codebase already uses `List` + `ListState` for playlist/track navigation with vim-style j/k/Enter/Esc keybindings -- the same pattern applies within the modal. The key architectural choice is a two-level browser state machine: Level 1 shows music library sections, Level 2 shows tracks within the selected section. Esc at Level 2 returns to Level 1; Esc at Level 1 closes the browser.

**Primary recommendation:** Add a `BrowserState` enum to App with two levels (sections list, tracks list), render the browser as a centered popup overlay using ratatui's `Clear` widget pattern, and extend `PlexClient` with `fetch_library_sections()` and `fetch_section_tracks()` methods. Wire track selection to the existing `load_ambient_track()` pipeline with the existing background download thread pattern.

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| ratatui | 0.30 | TUI framework -- popup overlay, List widget, Clear widget | Already in use; popup pattern documented in official examples |
| crossterm | 0.29 | Terminal input -- keybinding handling for browser navigation | Already in use; raw mode key capture |
| reqwest | 0.13 | HTTP client for Plex API calls (library sections, tracks) | Already in use; async + blocking variants |
| serde | 1 | JSON deserialization for Plex API responses | Already in use; derive macros for response types |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| tokio | 1 | Async runtime for Plex API calls | Already in use; fetch_library_sections is async |
| tracing | 0.1 | Logging browser operations and API errors | Already in use; structured logging |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Built-in Clear + Block popup | tui-popup crate | Extra dependency for minimal benefit; the pattern is ~15 lines of code |
| Flat track list (type=10) | Hierarchical artist/album browse | Explicitly out of scope per requirements (ADV-03 deferred to v2) |
| Async fetch in background thread | tokio::spawn for Plex API calls | Event loop is already async -- PlexClient methods use async/await directly. Background threads are only needed for `reqwest::blocking` (downloads), not for `reqwest` async calls |

**Installation:**
```bash
# No new dependencies needed. Existing Cargo.toml is sufficient.
```

## Architecture Patterns

### Recommended Project Structure
```
src/
  app.rs        # Extended: BrowserState enum, browser keybindings, browser API calls
  plex.rs       # Extended: fetch_library_sections(), fetch_section_tracks(), LibrarySection struct
  ui.rs         # Extended: render_browser_overlay() popup rendering
  player.rs     # Unchanged (ambient loading already works from Phase 6)
  main.rs       # Unchanged
```

### Pattern 1: Two-Level Browser State Machine
**What:** A `BrowserState` enum tracks the browser's current view (closed, showing sections, showing tracks) with associated data and navigation state.
**When to use:** Any modal with hierarchical navigation (enter/exit levels).
**Example:**
```rust
// Source: Architectural pattern derived from existing AppView state machine
/// State of the ambient track browser overlay.
pub enum BrowserState {
    /// Browser is not visible.
    Closed,
    /// Showing music library sections. User picks one to see its tracks.
    Sections {
        sections: Vec<LibrarySection>,
        list_state: ListState,
    },
    /// Showing tracks within a selected section. User picks one for ambient.
    Tracks {
        section_title: String,
        tracks: Vec<Track>,
        list_state: ListState,
    },
}
```

### Pattern 2: Centered Popup Overlay with Clear
**What:** Render a popup on top of existing content using `Clear` widget to erase the region, then render bordered content.
**When to use:** Any modal overlay that should not destroy the underlying render.
**Example:**
```rust
// Source: ratatui official popup example (https://ratatui.rs/examples/apps/popup/)
fn popup_area(area: Rect, percent_x: u16, percent_y: u16) -> Rect {
    let vertical = Layout::vertical([Constraint::Percentage(percent_y)])
        .flex(Flex::Center);
    let horizontal = Layout::horizontal([Constraint::Percentage(percent_x)])
        .flex(Flex::Center);
    let [area] = vertical.areas(area);
    let [area] = horizontal.areas(area);
    area
}

// In render function:
if let Some(browser) = app.browser_state() {
    let popup = popup_area(frame.area(), 70, 80);
    frame.render_widget(Clear, popup);
    // Render browser list inside popup area with Block border
    render_browser_content(frame, browser, popup);
}
```

### Pattern 3: Input Routing Based on Browser State
**What:** When browser is open, keyboard input routes to browser handler instead of main app handler.
**When to use:** Any modal that captures input focus.
**Example:**
```rust
// Source: Pattern derived from existing handle_key method in app.rs
async fn handle_key(&mut self, code: KeyCode, modifiers: KeyModifiers) -> Result<()> {
    // Browser captures all input when open
    if !matches!(self.browser_state, BrowserState::Closed) {
        return self.handle_browser_key(code).await;
    }
    // ... existing key handling
}
```

### Pattern 4: Async Section/Track Fetch (Not Background Thread)
**What:** Use the existing async `PlexClient` methods directly for fetching library sections and track lists. Only the audio download uses a background thread.
**When to use:** API calls that return metadata (JSON). The background thread + mpsc pattern is only needed for `reqwest::blocking` audio byte downloads.
**Example:**
```rust
// Source: Existing select_item() pattern in app.rs lines 730-788
async fn handle_browser_key(&mut self, code: KeyCode) -> Result<()> {
    match code {
        KeyCode::Enter => {
            match &self.browser_state {
                BrowserState::Sections { sections, list_state } => {
                    if let Some(idx) = list_state.selected() {
                        let section = &sections[idx];
                        // Async fetch -- same pattern as fetch_tracks in select_item()
                        let tracks = self.plex_client
                            .fetch_section_tracks(&section.key)
                            .await?;
                        self.browser_state = BrowserState::Tracks {
                            section_title: section.title.clone(),
                            tracks,
                            list_state: ListState::default().with_selected(Some(0)),
                        };
                    }
                }
                BrowserState::Tracks { tracks, list_state, .. } => {
                    // User selected a track -- start ambient download
                    if let Some(idx) = list_state.selected() {
                        self.start_ambient_download_from_browser(idx)?;
                        self.browser_state = BrowserState::Closed;
                    }
                }
                _ => {}
            }
        }
        // ... j/k/Esc handling
    }
}
```

### Pattern 5: Reuse Existing Ambient Download Pipeline
**What:** When the user selects a track in the browser, trigger the same background download + ambient load pipeline already built in Phase 6.
**When to use:** Every ambient track selection from the browser.
**Example:**
```rust
// Source: Existing start_ambient_from_selected() in app.rs lines 1341-1382
// Adapted to use track from browser state instead of track_state selection
fn start_ambient_download_from_browser(&mut self, track_idx: usize) -> Result<()> {
    // Extract track info from browser state
    // Get stream URL from track's media parts
    // Spawn background download thread (same pattern as start_ambient_from_selected)
    // Set ambient_download_rx for event loop polling
    // Close browser overlay
    Ok(())
}
```

### Anti-Patterns to Avoid
- **Blocking the event loop with API calls:** The existing `PlexClient` uses async. Never use `reqwest::blocking` for metadata fetches -- that is only for audio byte downloads on background threads.
- **Creating a new ListState on every render:** Store `ListState` inside `BrowserState` so selection persists across renders. The existing codebase stores `playlist_state` and `track_state` on App -- same pattern.
- **Handling browser keys in the main match block:** Route input to a separate `handle_browser_key()` method when the browser is open. Mixing browser keys into the existing match creates confusion about which keys are active.
- **Using the 'a' temporary keybinding for the browser:** The Phase 6 'a' keybinding was marked as `TODO(Phase 7): Remove this method when track browser is implemented`. Replace it with the proper browser open keybinding.
- **Fetching sections on every browser open:** Cache the sections list on App (or in BrowserState) so reopening the browser does not require a network round-trip every time. Invalidate on reconnect or after a timeout.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Popup centering | Manual pixel math for centering | `Layout::vertical/horizontal` with `Constraint::Percentage` + `Flex::Center` | Handles terminal resize automatically |
| Popup background clearing | Custom buffer manipulation | `Clear` widget from ratatui | Built-in, zero-config, handles all edge cases |
| Scrolling list in popup | Custom scroll offset tracking | `List` + `ListState` (existing pattern) | Already handles viewport scrolling, selection highlight, wrap-around |
| Plex JSON deserialization | Manual JSON parsing | serde derive `#[derive(Deserialize)]` with `#[serde(rename)]` | Existing pattern in plex.rs (PlaylistContainer, TrackContainer) |
| Ambient download pipeline | New download mechanism | Existing `start_ambient_from_selected` + `check_ambient_download_complete` pattern | Already battle-tested in Phase 6 with failure isolation |

**Key insight:** Phase 7 is an integration phase, not an invention phase. Every component needed already exists: Plex API client, List + ListState navigation, background download + ambient load pipeline, popup overlay pattern. The work is wiring these together with new API endpoints and a new UI overlay.

## Common Pitfalls

### Pitfall 1: Browser Swallows Global Keybindings
**What goes wrong:** When the browser overlay is open, pressing 'q' quits the app instead of being ignored, or spacebar pauses playback instead of being handled by the browser.
**Why it happens:** The main `handle_key()` processes keys before checking browser state.
**How to avoid:** Check browser state FIRST in `handle_key()`. If browser is open, route ALL keys to `handle_browser_key()` and return early. Only 'q' should still quit (or not -- design choice).
**Warning signs:** App quits or playback pauses while browser is open.

### Pitfall 2: Plex Library Sections Include Non-Music Types
**What goes wrong:** The browser shows movie libraries, TV show libraries, and photo libraries alongside music.
**Why it happens:** `GET /library/sections` returns ALL library sections. Music sections have `type: "artist"` in the response.
**How to avoid:** Filter the response: only include sections where `type == "artist"`. The Plex API returns a `type` string field on each Directory/section entry.
**Warning signs:** Selecting a movie library and getting unexpected or empty results from `type=10` track query.

### Pitfall 3: Large Music Libraries Cause UI Freeze
**What goes wrong:** A music section with thousands of tracks takes several seconds to fetch via the API, freezing the UI during the async call.
**Why it happens:** `self.plex_client.fetch_section_tracks()` is called with `.await` inside `handle_browser_key()`, which runs in the event loop. For large libraries, this API call can take 1-5 seconds.
**How to avoid:** Two options: (1) Accept brief freeze for v1.1 (simplest, matches existing pattern -- `fetch_tracks()` for playlists already blocks the event loop). (2) Show a "Loading..." indicator in the browser popup while fetching. Option 1 is recommended for consistency with existing behavior.
**Warning signs:** UI becomes unresponsive for seconds when entering a large music section.

### Pitfall 4: Browser State Leaks After Track Selection
**What goes wrong:** After selecting a track and closing the browser, the browser's section/track data remains in memory.
**Why it happens:** BrowserState::Closed does not clear the cached sections/tracks vectors.
**How to avoid:** When transitioning to `BrowserState::Closed`, the enum variant naturally drops the data. The enum-based design handles this automatically -- `BrowserState::Closed` has no fields.
**Warning signs:** Memory growth after opening/closing the browser many times (unlikely in practice given typical library sizes).

### Pitfall 5: Track Has No Media Parts
**What goes wrong:** User selects a track that has no `Media` or `Part` entries, causing the download URL construction to fail.
**Why it happens:** Some Plex library entries are metadata-only (no associated file). This is rare but possible.
**How to avoid:** Check for media parts before starting download, show an error or skip. This is already handled in the existing `start_track_download()` method -- same pattern.
**Warning signs:** Silent failure to start ambient after browser selection.

### Pitfall 6: Async Borrow Conflict in Browser Key Handler
**What goes wrong:** The `handle_browser_key()` method needs to read from `self.browser_state` (to get the selected section key) and also call `self.plex_client.fetch_section_tracks()` (which borrows `self`). This creates a double-borrow.
**Why it happens:** Rust's borrow checker does not allow borrowing `self` mutably while also borrowing a field of `self`.
**How to avoid:** Extract needed data (section key, section title) into local variables before the async call. The existing `select_item()` method (app.rs line 730) uses this exact pattern: it extracts `rating_key` and `playlist.title.clone()` before calling `self.plex_client.fetch_tracks()`.
**Warning signs:** Compiler error about conflicting borrows of `self`.

## Code Examples

Verified patterns from official sources and the existing codebase:

### New Plex API Types (plex.rs)
```rust
// Source: Plex API documentation (plexopedia.com, plexapi.dev)
// Response structure matches existing PlaylistContainer/TrackContainer pattern

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

#[derive(Deserialize, Debug)]
pub struct SectionsMediaContainer {
    #[serde(rename = "Directory", default)]
    pub directory: Vec<LibrarySection>,
}
```

### New PlexClient Methods (plex.rs)
```rust
// Source: Existing fetch_playlists() and fetch_tracks() patterns in plex.rs

/// Fetch all library sections from the server.
/// Filters for music sections (type="artist") on the caller side.
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
/// GET {server_url}/library/sections/{section_key}/all?type=10
/// type=10 is the Plex content type for audio tracks.
pub async fn fetch_section_tracks(&self, section_key: &str) -> Result<Vec<Track>> {
    let headers = self.server_headers();
    let resp = self
        .client
        .get(format!(
            "{}/library/sections/{}/all",
            self.server_url, section_key
        ))
        .headers(headers)
        .query(&[("type", "10")])
        .send()
        .await?
        .error_for_status()?
        .json::<TrackContainer>()
        .await?;
    Ok(resp.media_container.metadata)
}
```

### Popup Area Calculation (ui.rs)
```rust
// Source: ratatui official popup example (https://ratatui.rs/examples/apps/popup/)
use ratatui::layout::Flex;

/// Calculate a centered popup area as a percentage of the full terminal.
fn popup_area(area: Rect, percent_x: u16, percent_y: u16) -> Rect {
    let vertical = Layout::vertical([Constraint::Percentage(percent_y)])
        .flex(Flex::Center);
    let horizontal = Layout::horizontal([Constraint::Percentage(percent_x)])
        .flex(Flex::Center);
    let [area] = vertical.areas(area);
    let [area] = horizontal.areas(area);
    area
}
```

### Browser Overlay Rendering (ui.rs)
```rust
// Source: ratatui Clear widget docs + existing render_tracks/render_playlists pattern
use ratatui::widgets::Clear;

fn render_browser_overlay(frame: &mut Frame, app: &mut App) {
    let popup = popup_area(frame.area(), 70, 80);

    // Clear the popup area to prevent bleed-through from underlying content
    frame.render_widget(Clear, popup);

    match app.browser_state_mut() {
        BrowserState::Sections { sections, list_state } => {
            let items: Vec<ListItem> = sections
                .iter()
                .map(|s| ListItem::new(s.title.clone()))
                .collect();

            let list = List::new(items)
                .block(
                    Block::default()
                        .title(" Music Libraries ")
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(Color::Magenta)),
                )
                .highlight_style(
                    Style::default()
                        .fg(Color::Black)
                        .bg(Color::Magenta)
                        .add_modifier(Modifier::BOLD),
                )
                .highlight_symbol("> ");

            frame.render_stateful_widget(list, popup, list_state);
        }
        BrowserState::Tracks { section_title, tracks, list_state } => {
            let items: Vec<ListItem> = tracks
                .iter()
                .map(|t| {
                    let artist = t.artist.as_deref().unwrap_or("Unknown");
                    ListItem::new(format!("{} - {}", t.title, artist))
                })
                .collect();

            let list = List::new(items)
                .block(
                    Block::default()
                        .title(format!(" {} - Select Ambient Track ", section_title))
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(Color::Magenta)),
                )
                .highlight_style(
                    Style::default()
                        .fg(Color::Black)
                        .bg(Color::Magenta)
                        .add_modifier(Modifier::BOLD),
                )
                .highlight_symbol("> ");

            frame.render_stateful_widget(list, popup, list_state);
        }
        BrowserState::Closed => {} // Should not reach here
    }
}
```

### Input Routing (app.rs)
```rust
// Source: Derived from existing handle_key pattern in app.rs
async fn handle_key(&mut self, code: KeyCode, modifiers: KeyModifiers) -> Result<()> {
    // Browser captures input when open (except Ctrl+C for emergency quit)
    if !matches!(self.browser_state, BrowserState::Closed) {
        if code == KeyCode::Char('c') && modifiers.contains(KeyModifiers::CONTROL) {
            self.running = false;
            return Ok(());
        }
        return self.handle_browser_key(code).await;
    }
    // ... existing key handling (unchanged)
}
```

### Browser Key Handler (app.rs)
```rust
// Source: Derived from existing select_item() and move_selection_down/up patterns
async fn handle_browser_key(&mut self, code: KeyCode) -> Result<()> {
    match code {
        KeyCode::Char('j') | KeyCode::Down => {
            // Move selection down in current browser list
            self.browser_move_down();
        }
        KeyCode::Char('k') | KeyCode::Up => {
            // Move selection up in current browser list
            self.browser_move_up();
        }
        KeyCode::Enter => {
            match &self.browser_state {
                BrowserState::Sections { .. } => {
                    // Extract data, fetch section tracks, transition to Tracks level
                    self.browser_enter_section().await?;
                }
                BrowserState::Tracks { .. } => {
                    // Extract track, start ambient download, close browser
                    self.browser_select_track()?;
                }
                _ => {}
            }
        }
        KeyCode::Esc | KeyCode::Backspace => {
            match &self.browser_state {
                BrowserState::Tracks { .. } => {
                    // Go back to sections level (re-show cached sections)
                    self.browser_back_to_sections();
                }
                BrowserState::Sections { .. } => {
                    // Close the browser
                    self.browser_state = BrowserState::Closed;
                }
                _ => {}
            }
        }
        KeyCode::Char('q') => {
            // Close the browser (not quit app)
            self.browser_state = BrowserState::Closed;
        }
        _ => {}
    }
    Ok(())
}
```

### Opening the Browser (app.rs)
```rust
// Source: Derived from existing keybinding pattern
// In handle_key, replace the temporary 'a' keybinding:
(KeyCode::Char('b'), _) => {
    // Open ambient track browser -- fetch music library sections
    self.open_ambient_browser().await?;
}

async fn open_ambient_browser(&mut self) -> Result<()> {
    let all_sections = self.plex_client.fetch_library_sections().await?;
    // Filter for music sections (type="artist")
    let music_sections: Vec<LibrarySection> = all_sections
        .into_iter()
        .filter(|s| s.section_type == "artist")
        .collect();

    if music_sections.is_empty() {
        tracing::warn!("No music library sections found on server");
        return Ok(());
    }

    let mut list_state = ListState::default();
    list_state.select(Some(0));

    self.browser_state = BrowserState::Sections {
        sections: music_sections,
        list_state,
    };
    Ok(())
}
```

## Plex API Reference

### Library Sections Endpoint
| Aspect | Detail |
|--------|--------|
| Method | GET |
| URL | `{server_url}/library/sections` |
| Auth | `X-Plex-Token` header (already handled by `server_headers()`) |
| Accept | `application/json` (already set in `build_plex_headers()`) |
| Response | `{ "MediaContainer": { "Directory": [...] } }` |
| Music filter | `type == "artist"` on each Directory entry |

### Section Tracks Endpoint
| Aspect | Detail |
|--------|--------|
| Method | GET |
| URL | `{server_url}/library/sections/{key}/all?type=10` |
| Auth | Same as above |
| Query params | `type=10` (Plex content type for audio tracks) |
| Response | `{ "MediaContainer": { "Metadata": [...] } }` |
| Track fields | Same `Track` struct already defined in plex.rs |

### Plex Content Type Values (for reference)
| Type | Value | Used In |
|------|-------|---------|
| Artist | 8 | `type=8` (not needed -- sections are already scoped) |
| Album | 9 | `type=9` (not needed -- flat track list, no hierarchy) |
| Track | 10 | `type=10` (used to get all tracks in a section) |

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Temporary 'a' keybinding | Proper 'b' browser keybinding | Phase 7 | Remove `start_ambient_from_selected()` method |
| No modal overlays | Clear + Block popup pattern | ratatui 0.26+ (Flex::Center) | Centered popups without manual math |
| App has no browser state | BrowserState enum on App | Phase 7 | New state field, new key handler |

**Deprecated/outdated:**
- `start_ambient_from_selected()`: Marked as `TODO(Phase 7)` in app.rs. Remove when browser is implemented.
- `awaiting_favorite_key` pattern: The browser uses a different pattern (enum-based state vs boolean flag), but the favorite pattern is not deprecated -- both coexist.

## Prior Decisions That Constrain This Phase

These decisions from Phase 6 are locked and directly affect Phase 7 implementation:

1. **Single OutputStream shared by both sinks** -- Browser-triggered ambient loads use the same `Player::load_ambient()` method, which creates ambient sinks on the existing `_stream.mixer()`.

2. **Volume management in App, not Player** -- After browser selection triggers `load_ambient_track()`, the App calls `apply_ambient_volume()`. This is already wired.

3. **rodio `Sink::set_volume()` unreliable for ambient sinks** -- The ambient sink recreation pattern (stop + create new + re-decode) is already in Player. Browser selection does not need to worry about this.

4. **Background thread + mpsc channel for ambient downloads** -- The browser must use the same pattern (`std::thread::spawn` + `mpsc::channel`) for downloading the selected track's audio bytes. Calling `reqwest::blocking` from within the async event loop causes a tokio runtime nesting panic. The existing `ambient_download_rx` polling in `check_ambient_download_complete()` handles the rest.

## Open Questions

1. **Keybinding for opening the browser**
   - What we know: The temporary 'a' keybinding exists from Phase 6. Phase 8 will add more ambient controls.
   - What's unclear: Which key should open the browser? 'b' for "browse"? 'a' for "ambient"? 'A' (shift-a)?
   - Recommendation: Use 'b' for "browse" to distinguish from the mute toggle ('m') and future ambient controls. The 'a' keybinding from Phase 6 should be removed. However, the planner can choose any unoccupied key.

2. **Section caching strategy**
   - What we know: Library sections rarely change. Fetching on every browser open adds latency.
   - What's unclear: Whether to cache sections permanently (until app restart) or refresh on each open.
   - Recommendation: Cache sections on first fetch, store as `Option<Vec<LibrarySection>>` on App. Refetch only on app restart. Library sections are structural and almost never change during a session.

3. **Behavior when no music sections exist**
   - What we know: Some Plex servers may have no music libraries.
   - What's unclear: What to show the user.
   - Recommendation: Log a warning and do not open the browser. Optionally show a brief error message. This is an edge case that should not block the implementation.

4. **Track list pagination for very large libraries**
   - What we know: Plex supports `X-Plex-Container-Start` and `X-Plex-Container-Size` for pagination.
   - What's unclear: Whether fetching all tracks at once is practical for very large libraries (10,000+ tracks).
   - Recommendation: Fetch all tracks at once for v1.1 (simplest). Plex returns track metadata (not audio data), so even 10,000 tracks is only a few MB of JSON. Pagination can be added in v2 if needed.

## Sources

### Primary (HIGH confidence)
- [Ratatui popup example](https://ratatui.rs/examples/apps/popup/) -- popup_area() with Flex::Center, Clear widget, conditional rendering
- [Ratatui overwrite regions recipe](https://ratatui.rs/recipes/render/overwrite-regions/) -- Clear widget pattern for popups
- [Ratatui Clear widget docs](https://docs.rs/ratatui/latest/ratatui/widgets/struct.Clear.html) -- render before popup content
- [Ratatui List widget docs](https://docs.rs/ratatui/latest/ratatui/widgets/struct.List.html) -- StatefulWidget with ListState
- [Ratatui ListState docs](https://docs.rs/ratatui/latest/ratatui/widgets/struct.ListState.html) -- select_next(), select_previous()
- Existing codebase: `plex.rs` (PlexClient, Track, Media, Part structs), `app.rs` (handle_key, select_item, start_ambient_from_selected), `ui.rs` (render_playlists, render_tracks patterns)

### Secondary (MEDIUM confidence)
- [Plexopedia - Get Libraries](https://www.plexopedia.com/plex-media-server/api/server/libraries/) -- GET /library/sections, Directory type field values
- [Plexopedia - Music Library API](https://www.plexopedia.com/plex-media-server/api/library/music/) -- GET /library/sections/{id}/all endpoint
- [cloudbitvps/plex-api](https://github.com/cloudbitvps/plex-api/blob/master/README.md) -- type=10 for tracks, type=8 for artists, type=9 for albums
- [Plex API overview wiki](https://github.com/Arcanemagus/plex-api/wiki/Plex-Web-API-Overview) -- MediaContainer response structure
- [plexapi.dev](https://plexapi.dev/api-reference/library/get-all-libraries) -- Official API docs reference

### Tertiary (LOW confidence)
- Track list JSON structure for /library/sections/{id}/all?type=10 -- verified that it returns `MediaContainer.Metadata` array matching the existing `Track` struct, but exact field parity with playlist track responses is assumed (not independently verified with a live API call). The `TrackContainer` struct in plex.rs should work, but field differences are possible.

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - no new dependencies, all libraries already in use and proven
- Architecture: HIGH - popup overlay pattern is official ratatui example, browser state machine follows existing AppView pattern
- Plex API: MEDIUM-HIGH - endpoints documented across multiple sources, type values confirmed, but /library/sections JSON response field names for the Directory array (vs Metadata array) need runtime verification
- Pitfalls: HIGH - all derived from existing codebase patterns and known issues from Phase 6
- Code examples: HIGH - all patterns derived from existing codebase or official ratatui documentation

**Research date:** 2026-02-11
**Valid until:** 2026-03-11 (stable domain -- ratatui 0.30, Plex API, patterns all mature)
