---
phase: quick
plan: 1
type: execute
wave: 1
depends_on: []
files_modified:
  - src/plex.rs
  - src/app.rs
  - src/ui.rs
autonomous: true

must_haves:
  truths:
    - "Pressing 'b' opens browser with two choices: Playlists and Artists"
    - "Selecting Artists shows a searchable list of artists (type to filter)"
    - "Selecting an artist shows their albums"
    - "Selecting an album shows its tracks with option to play individual or whole album"
    - "Selecting Playlists shows audio playlists"
    - "Selecting a playlist shows its tracks with option to play individual or whole playlist"
    - "Esc/Backspace navigates back through each level, closing browser at top"
  artifacts:
    - path: "src/plex.rs"
      provides: "Artist/Album types and fetch methods"
      contains: "fetch_section_artists"
    - path: "src/app.rs"
      provides: "Multi-level BrowserState enum and navigation handlers"
      contains: "BrowserState"
    - path: "src/ui.rs"
      provides: "Render functions for each browser level"
      contains: "render_browser_overlay"
  key_links:
    - from: "src/app.rs"
      to: "src/plex.rs"
      via: "fetch_section_artists, fetch_artist_albums, fetch_album_tracks"
      pattern: "plex_client\\.fetch_(section_artists|artist_albums|album_tracks)"
    - from: "src/ui.rs"
      to: "src/app.rs"
      via: "BrowserState pattern match"
      pattern: "BrowserState::(TopLevel|Artists|Albums|ArtistTracks|Playlists|PlaylistTracks)"
---

<objective>
Replace the flat ambient track browser (Library Sections -> 15k tracks) with a hierarchical browser supporting two navigation paths: Playlists (playlist list -> tracks) and Artists (artist list with search -> albums -> tracks). Add "play all" option to play an entire album or playlist on ambient repeat.

Purpose: The current browser dumps 15k+ tracks in a flat list, making it impossible to find specific ambient tracks. A hierarchical structure with search makes the browser actually usable.

Output: Updated `src/plex.rs`, `src/app.rs`, `src/ui.rs` with hierarchical browser.
</objective>

<execution_context>
@/home/jigsaw/.claude/get-shit-done/workflows/execute-plan.md
@/home/jigsaw/.claude/get-shit-done/templates/summary.md
</execution_context>

<context>
@src/plex.rs
@src/app.rs
@src/ui.rs
@src/player.rs
</context>

<tasks>

<task type="auto">
  <name>Task 1: Add Plex API types and methods for artist/album hierarchy</name>
  <files>src/plex.rs</files>
  <action>
Add new deserialization types and PlexClient methods to support the artist/album hierarchy:

**New types:**
- `Artist` struct: deserialize from Plex metadata with fields `rating_key` (ratingKey), `title` (artist name), `thumb` (optional). Plex returns artists from `/library/sections/{key}/all?type=8`.
- `Album` struct: deserialize with `rating_key`, `title` (album name), `year` (optional u32), `parent_title` (parentTitle, artist name). Plex returns albums from `/library/metadata/{ratingKey}/children`.
- Reuse existing `Track` type for album tracks (same structure).
- Container wrappers: `ArtistContainer` and `AlbumContainer` following the existing pattern (`MediaContainer` -> `Metadata`).

**New PlexClient methods:**
- `fetch_section_artists(&self, section_key: &str) -> Result<Vec<Artist>>`: GET `{server}/library/sections/{section_key}/all?type=8` — type=8 is Plex content type for artists.
- `fetch_artist_albums(&self, artist_rating_key: &str) -> Result<Vec<Album>>`: GET `{server}/library/metadata/{artist_rating_key}/children` — returns albums for an artist.
- `fetch_album_tracks(&self, album_rating_key: &str) -> Result<Vec<Track>>`: GET `{server}/library/metadata/{album_rating_key}/children?includeMedia=1` — returns tracks for an album. MUST include `includeMedia=1` query param (same as existing `fetch_section_tracks`) so that Track.media is populated for stream URL construction.

Follow existing patterns exactly: `server_headers()`, `error_for_status()`, `.json::<ContainerType>()`, return `Ok(resp.media_container.metadata)`.

Note: The album children endpoint returns Track objects with the same JSON shape as playlist tracks, so the existing `TrackContainer`/`Track` types work for deserialization.
  </action>
  <verify>Run `cargo check` -- should compile with no errors. The new types and methods follow established patterns and won't break existing code.</verify>
  <done>PlexClient has three new async methods (`fetch_section_artists`, `fetch_artist_albums`, `fetch_album_tracks`) and two new types (`Artist`, `Album`) that compile successfully.</done>
</task>

<task type="auto">
  <name>Task 2: Redesign BrowserState enum and navigation logic for hierarchical browsing</name>
  <files>src/app.rs</files>
  <action>
Replace the existing `BrowserState` enum and all browser navigation methods in `app.rs` to support the new hierarchical flow.

**Replace `BrowserState` enum with:**
```rust
pub enum BrowserState {
    Closed,
    /// Top-level: "Playlists" or "Artists" choice
    TopLevel {
        list_state: ListState,
    },
    /// Artist path: list of artists in a music library section, with search filter
    Artists {
        section_key: String,
        all_artists: Vec<plex::Artist>,     // Full unfiltered list (for search)
        filtered_indices: Vec<usize>,        // Indices into all_artists matching search
        search_query: String,                // Current search/filter text
        list_state: ListState,
    },
    /// Artist path: albums for a selected artist
    Albums {
        section_key: String,
        artist_name: String,
        albums: Vec<plex::Album>,
        list_state: ListState,
    },
    /// Artist path: tracks within an album (with ">> Play All" as first item)
    ArtistTracks {
        section_key: String,
        album_title: String,
        artist_name: String,
        tracks: Vec<plex::Track>,
        list_state: ListState,
    },
    /// Playlist path: list of audio playlists
    Playlists {
        playlists: Vec<plex::Playlist>,
        list_state: ListState,
    },
    /// Playlist path: tracks within a playlist (with ">> Play All" as first item)
    PlaylistTracks {
        playlist_title: String,
        tracks: Vec<plex::Track>,
        list_state: ListState,
    },
}
```

**Update `open_ambient_browser()`:**
Instead of fetching library sections, open at TopLevel with a ListState selecting index 0. TopLevel has exactly 2 items: "Playlists" and "Artists".

**Update `handle_browser_key()`:**
Add character input handling for the Artists state search:
- When in `BrowserState::Artists`, printable characters (a-z, 0-9, space, etc.) append to `search_query` and re-filter `filtered_indices`.
- Backspace in Artists state: if `search_query` is non-empty, delete last character and re-filter. If empty, go back to TopLevel.
- j/k/Up/Down navigate the filtered list.
- Enter drills into the selected item.
- Esc always goes back one level. At TopLevel, close the browser.

**Filtering logic (Artists state):**
When `search_query` changes, recompute `filtered_indices` as: indices of `all_artists` where `artist.title.to_lowercase().contains(search_query.to_lowercase())`. If query is empty, all indices included. Reset list_state to select index 0 after filtering.

**Navigation methods to update/create:**

1. `browser_enter_top_level_item()` — Enter on TopLevel:
   - Index 0 ("Playlists"): Fetch playlists via existing `self.plex_client.fetch_playlists().await?`, transition to `BrowserState::Playlists`.
   - Index 1 ("Artists"): Need a music section key. Use cached_sections if available, otherwise fetch. Pick the first music section (type == "artist"). Fetch artists via `self.plex_client.fetch_section_artists(&section_key).await?`. Transition to `BrowserState::Artists` with all artists, empty search, full filtered_indices.

2. `browser_enter_artist()` — Enter on Artists list:
   - Get the actual artist from `all_artists[filtered_indices[selected_index]]`.
   - Fetch albums via `self.plex_client.fetch_artist_albums(&artist.rating_key).await?`.
   - Transition to `BrowserState::Albums`.

3. `browser_enter_album()` — Enter on Albums list:
   - Fetch tracks via `self.plex_client.fetch_album_tracks(&album.rating_key).await?`.
   - Transition to `BrowserState::ArtistTracks`. Note: list_state starts at 0 which is the "Play All" virtual item.

4. `browser_select_artist_track()` — Enter on ArtistTracks list:
   - Index 0 = "Play All": Start ambient playback of ALL tracks in the album. Download and play the first track, store the full track list for sequential ambient playback (see "Play All" implementation below).
   - Index > 0 = individual track: Use existing `browser_select_track` logic (construct stream URL, spawn download, close browser). The track index into the `tracks` vec is `selected_index - 1` (offset by the Play All item).

5. `browser_enter_playlist()` — Enter on Playlists list:
   - Fetch tracks via existing `self.plex_client.fetch_tracks(&playlist.rating_key).await?`.
   - Transition to `BrowserState::PlaylistTracks`.

6. `browser_select_playlist_track()` — Enter on PlaylistTracks list:
   - Same as ArtistTracks: Index 0 = "Play All", Index > 0 = individual track at `selected_index - 1`.

**"Play All" implementation:**
For "Play All" on both album and playlist tracks, the simplest approach that fits the existing ambient architecture:
- Select the first track for ambient download (same as browser_select_track).
- Store the full track list in a new field `ambient_playlist: Option<Vec<plex::Track>>` and `ambient_playlist_index: usize` on App.
- In `check_ambient_loop()`, when the ambient track finishes AND `ambient_playlist` is Some: increment `ambient_playlist_index`, if within bounds download next track, if past end wrap to 0 (infinite repeat).
- This replaces the current single-track repeat loop behavior when a playlist/album is loaded.

**New App fields:**
- `ambient_playlist: Option<Vec<plex::Track>>` — tracks for ambient "Play All" mode
- `ambient_playlist_index: usize` — current position in ambient playlist

**Update `check_ambient_loop()`:**
Currently replays the same cached audio. Change to: if `ambient_playlist.is_some()`, advance to next track in the list (wrapping at end), download and play it. If `ambient_playlist.is_none()`, use existing single-track replay behavior.

**Update `browser_select_track()` for individual track selection:**
When selecting an individual track (not Play All), set `ambient_playlist = None` to ensure single-track repeat behavior.

**Update `browser_move_down()` and `browser_move_up()`:**
Handle all new BrowserState variants. For ArtistTracks and PlaylistTracks, the list length is `tracks.len() + 1` (accounting for the "Play All" virtual first item).

**Backspace/Esc navigation mapping:**
- TopLevel -> Closed
- Artists -> TopLevel
- Albums -> Artists (restore search state by re-creating from cached all_artists and section_key)
- ArtistTracks -> Albums (need to re-fetch or cache albums; simplest: go back to Artists level)
- Playlists -> TopLevel
- PlaylistTracks -> Playlists

For simplicity on back-navigation from ArtistTracks: go back to the Artists level (re-entering the artist will re-fetch albums which is fast). This avoids caching album state.

**Remove `cached_sections` field** from App struct if it's only used for the old browser flow. Actually keep it -- it's still useful for finding the music section key when entering Artists mode from TopLevel.
  </action>
  <verify>Run `cargo check` -- should compile. Then `cargo build` to verify full build succeeds.</verify>
  <done>
- BrowserState has 7 variants covering the full hierarchy
- TopLevel shows Playlists/Artists choice
- Artists level has inline search filtering
- Albums level shows artist's albums
- Track levels have "Play All" as first item
- "Play All" stores track list in ambient_playlist for sequential ambient cycling
- check_ambient_loop advances through ambient_playlist when present
- All back-navigation works through each level
- Individual track selection sets ambient_playlist = None for single-track repeat
  </done>
</task>

<task type="auto">
  <name>Task 3: Update browser overlay UI rendering for all hierarchy levels</name>
  <files>src/ui.rs</files>
  <action>
Update `render_browser_overlay()` in `src/ui.rs` to render all 6 visible BrowserState variants (everything except Closed). Each level needs its own rendering logic within the popup overlay.

**TopLevel rendering:**
- Title: " Ambient Browser "
- Two items: "Playlists" and "Artists" as ListItems
- Same highlight style (Magenta bg) as existing browser

**Artists rendering:**
- Title: " Artists " with search query shown if non-empty, e.g., " Artists - Search: jazz "
- Split the popup area: top 1-2 lines for a search input display, remainder for the artist list
- Search display line: render a Paragraph showing "Search: {query}_" (underscore cursor) in Magenta if query non-empty, or "Type to search..." in DarkGray if empty
- Artist list: render only the filtered artists. Map `filtered_indices` to get artist names from `all_artists`. Show as "{artist.title}" per ListItem.
- If filtered list is empty and search is non-empty, show "No matches" centered in the list area.

**Albums rendering:**
- Title: " {artist_name} - Albums "
- Items: "{album.title}" with year suffix if present, e.g., "Album Name (2023)"
- Same List + highlight style

**ArtistTracks rendering:**
- Title: " {album_title} - {artist_name} "
- First item (index 0): ">> Play All ({tracks.len()} tracks)" in Yellow+Bold style
- Remaining items: "  {track.title}" with artist info, same format as existing browser tracks

**Playlists rendering:**
- Title: " Playlists "
- Items: "{playlist.title}" with optional track count suffix " ({leaf_count} tracks)"
- Same List + highlight style

**PlaylistTracks rendering:**
- Title: " {playlist_title} "
- First item: ">> Play All ({tracks.len()} tracks)" in Yellow+Bold
- Remaining items: "  {track.title} - {artist}" same as existing track display

**For the "Play All" items:** Use `ListItem::new(Line::from(vec![Span::styled(...)]))` with `Color::Yellow` and `Modifier::BOLD` to visually distinguish from regular tracks.

**Search input area (Artists state only):**
Split the popup rect vertically: `[Constraint::Length(2), Constraint::Fill(1)]`. Top 2 lines for search bar (1 line text + 1 line border/separator). Bottom area for the artist list.

**Update the `app.browser_state_mut()` match** to handle all new variants, accessing each variant's `list_state` for `render_stateful_widget`.

**Add a new public accessor on App** if needed for read-only access to browser state fields used by UI rendering. The existing `browser_state_mut()` should suffice since it returns `&mut BrowserState` which allows both reading and ListState mutation.
  </action>
  <verify>Run `cargo build` -- should compile with no errors or warnings. Run `cargo clippy` to check for any obvious issues.</verify>
  <done>
- render_browser_overlay handles all 6 visible BrowserState variants
- TopLevel shows "Playlists" and "Artists" options
- Artists level shows search bar + filtered artist list
- Albums shows album list with year
- ArtistTracks and PlaylistTracks show "Play All" as yellow first item + track list
- Playlists shows playlist list with track counts
- All variants use consistent Magenta highlight styling
  </done>
</task>

</tasks>

<verification>
1. `cargo build` compiles without errors
2. `cargo clippy` has no warnings
3. Run the app with `cargo run` -- press 'b' to open browser
4. Verify TopLevel shows "Playlists" and "Artists"
5. Navigate into Artists -> verify search works (type characters, list filters)
6. Select artist -> shows albums -> select album -> shows tracks with "Play All"
7. Test "Play All" -> ambient plays through multiple tracks sequentially
8. Test individual track selection -> ambient plays single track on repeat
9. Navigate into Playlists -> select playlist -> shows tracks with "Play All"
10. Esc/Backspace navigates back correctly at every level
11. 'q' closes browser from any level
</verification>

<success_criteria>
- Ambient browser opens with Playlists/Artists top-level choice
- Artists path: searchable artist list -> albums -> tracks with Play All
- Playlists path: playlist list -> tracks with Play All
- "Play All" plays entire album/playlist sequentially on ambient channel, looping
- Individual track plays single track on repeat (existing behavior preserved)
- Back navigation works through all levels
- Search in Artists filters the list as user types
- No regression in existing ambient features (volume, mute, session persistence)
</success_criteria>

<output>
After completion, create `.planning/quick/1-add-hierarchical-ambient-track-browser-w/1-SUMMARY.md`
</output>
