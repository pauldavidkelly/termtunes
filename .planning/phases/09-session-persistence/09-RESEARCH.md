# Phase 9: Session Persistence - Research

**Researched:** 2026-02-11
**Domain:** TOML session file extension + ambient state save/restore + backward-compatible deserialization
**Confidence:** HIGH

## Summary

Phase 9 extends the existing session persistence system to include ambient channel state (track selection, volume, on/off state) so that a user's complete ambient setup survives app restarts. The app already has a well-established session persistence pattern: a `Session` struct in `config.rs` serialized as TOML to `~/.local/share/termtunes/session.toml`, saved on graceful exit (`save_session_state()` in app.rs:1665), and restored best-effort on startup (`restore_session()` in app.rs:1685). The existing Session tracks: playlist rating key, playlist title, track index, volume, shuffle state, and repeat mode.

The core work is: (1) add new fields to the `Session` struct for ambient track identification, ambient volume, and ambient on/off state; (2) extend `save_session_state()` to capture ambient state from `App` and `Player`; (3) extend `restore_session()` to download and start the ambient track on startup; and (4) ensure backward compatibility so existing v1.0 session.toml files (without ambient fields) load without error.

The critical design challenge is **ambient track identification for restore**. Currently, when a user selects an ambient track from the browser, only the track name and raw audio bytes are stored in `Player`. No Plex API identifiers (part_key, rating_key) are retained in App-level state. To restore the ambient track on startup, we need to persist enough information to reconstruct the stream URL. The minimal set is the `part_key` (the path like `/library/parts/12345/file.flac` used in `plex_client.stream_url()`). The track name should also be persisted for display. Optionally, the track's `rating_key` can be stored for future use (e.g., verifying the track still exists on the server).

Backward compatibility is straightforward thanks to serde's `#[serde(default)]` attribute. Adding `#[serde(default)]` to each new field on `Session` means old session files (missing the new keys) deserialize cleanly with default values. The existing `load_session()` already uses `.ok()` to silently swallow errors, but relying on default values is cleaner and preserves the existing session data rather than discarding the entire file.

**Primary recommendation:** Add `ambient_part_key`, `ambient_track_name`, `ambient_volume`, and `ambient_enabled` fields to Session with `#[serde(default)]`. Store the part_key when selecting an ambient track in `browser_select_track()`. On restore, if `ambient_enabled` is true and `ambient_part_key` is present, download and start the ambient track automatically. Default `ambient_volume` to `None` (first-use triggers the "30% lower than main" calculation per PERSIST-05).

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| serde | 1 | Serialize/deserialize Session struct with `#[serde(default)]` for backward compat | Already in use; derive macros for TOML round-trip |
| toml | 0.8 | TOML serialization format for session.toml | Already in use; `toml::to_string_pretty` / `toml::from_str` |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| tracing | 0.1 | Logging ambient session save/restore events | Already in use; structured logging for debugging |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| TOML session file | JSON session file | TOML is already the established format; switching adds no value and breaks existing files |
| Storing `part_key` for ambient track | Storing full stream URL | Part key is more portable (server URL may change); stream URL includes the token which rotates |
| `#[serde(default)]` on each field | `#[serde(default)]` on the struct | Per-field is more precise; struct-level default requires `Default::default()` to produce correct defaults for ALL fields which may not match desired first-use behavior |

**Installation:**
```bash
# No new dependencies needed. Existing Cargo.toml is sufficient.
```

## Architecture Patterns

### Recommended Project Structure
```
src/
  config.rs     # Extended: new fields on Session struct with #[serde(default)]
  app.rs        # Extended: save/restore ambient state, store ambient_part_key on selection
  player.rs     # Unchanged (ambient accessors already exist)
  main.rs       # Unchanged (restore_session already called at startup)
  plex.rs       # Unchanged
```

### Pattern 1: Backward-Compatible Session Extension with `#[serde(default)]`
**What:** Add new optional/defaulted fields to the Session struct so old session files deserialize without error.
**When to use:** Every time the Session struct is extended with new persistent state.
**Example:**
```rust
// Source: serde.rs official docs - field attributes - #[serde(default)]
// Each new field gets a default that represents "no saved state" or
// "use first-use logic".

#[derive(Serialize, Deserialize, Default, Debug)]
pub struct Session {
    // --- Existing v1.0 fields (unchanged) ---
    pub playlist_rating_key: Option<String>,
    pub playlist_title: Option<String>,
    pub track_index: Option<usize>,
    pub volume: f32,
    pub shuffle_enabled: bool,
    pub repeat_mode: String,

    // --- New v1.1 ambient fields ---

    /// Part key of the ambient track (e.g., "/library/parts/12345/file.flac").
    /// Used to reconstruct the stream URL on restore.
    #[serde(default)]
    pub ambient_part_key: Option<String>,

    /// Display name of the ambient track (for UI before download completes).
    #[serde(default)]
    pub ambient_track_name: Option<String>,

    /// Ambient volume level (0.0 to 1.0). None means "first use" --
    /// triggers PERSIST-05 default (30% lower than main volume).
    #[serde(default)]
    pub ambient_volume: Option<f32>,

    /// Whether ambient was playing (true) or muted (false) at save time.
    /// Default false: ambient does not auto-start on first use.
    #[serde(default)]
    pub ambient_enabled: bool,
}
```

### Pattern 2: Ambient Track Identification Capture at Selection Time
**What:** When the user selects a track from the ambient browser, store the part_key in App state so it can be persisted on exit.
**When to use:** Any time we need to persist a reference to a Plex track that can be used to re-fetch it later.
**Example:**
```rust
// Source: Existing browser_select_track() pattern in app.rs:1594
// Currently extracts part_key for stream URL construction but does NOT save it.
// Add a new App field: ambient_part_key: Option<String>

fn browser_select_track(&mut self, idx: usize) -> Result<()> {
    let (stream_url, track_name, part_key) = {
        let tracks = match &self.browser_state {
            BrowserState::Tracks { tracks, .. } => tracks,
            _ => return Ok(()),
        };
        let track = match tracks.get(idx) {
            Some(t) => t,
            None => return Ok(()),
        };
        let part_key = track
            .media
            .first()
            .and_then(|m| m.parts.first())
            .map(|p| p.key.clone());
        let pk = match &part_key {
            Some(key) => key.as_str(),
            None => return Ok(()),
        };
        (self.plex_client.stream_url(pk), track.title.clone(), part_key)
    };

    // Store the part_key for session persistence
    self.ambient_part_key = part_key;

    // ... rest of download logic unchanged
}
```

### Pattern 3: First-Use Default Volume (PERSIST-05)
**What:** On first-ever use (no saved ambient volume in session), default ambient volume to 30% lower than current main volume. On subsequent restarts, restore the saved ambient volume.
**When to use:** Distinguishing "never set" from "explicitly set to some value" using `Option<f32>` instead of a bare `f32`.
**Example:**
```rust
// In restore_session():
// ambient_volume in Session is Option<f32>:
//   None  = first use -> compute default from main volume
//   Some(v) = saved value -> use directly

let ambient_vol = match session.ambient_volume {
    Some(v) => v.clamp(0.0, 1.0),
    None => {
        // PERSIST-05: Default to 30% lower than main music volume
        // main volume is session.volume (already restored above)
        (session.volume - 0.30).max(0.0)
    }
};
self.ambient_volume = ambient_vol;
self.pre_mute_ambient_volume = ambient_vol;
```

### Pattern 4: Ambient Auto-Resume on Startup (PERSIST-04)
**What:** If the session indicates ambient was playing (`ambient_enabled: true`) and a part_key is saved, download and start the ambient track during `restore_session()`.
**When to use:** Restoring ambient playback state on app restart.
**Example:**
```rust
// In restore_session(), after restoring main playlist/track state:

if session.ambient_enabled {
    if let Some(ref part_key) = session.ambient_part_key {
        // Construct stream URL from the saved part_key
        let stream_url = self.plex_client.stream_url(part_key);
        let track_name = session
            .ambient_track_name
            .clone()
            .unwrap_or_else(|| "Ambient".to_string());

        tracing::info!(
            channel = "ambient",
            track = %track_name,
            "Restoring ambient track from session"
        );

        // Spawn background download (same pattern as browser_select_track)
        let (tx, rx) = std::sync::mpsc::channel();
        self.ambient_download_rx = Some(rx);
        std::thread::spawn(move || {
            let result = Player::download_track(&stream_url)
                .map(|bytes| (bytes, track_name));
            let _ = tx.send(result);
        });
    }
}
```

### Anti-Patterns to Avoid
- **Storing the full stream URL in session.toml:** The stream URL includes the Plex auth token as a query parameter (`?X-Plex-Token=...`). Tokens rotate on re-auth. Store the part_key instead and reconstruct the URL at restore time with the current token.
- **Using `ambient_volume: f32` with 0.0 meaning "first use":** 0.0 is a valid user-set volume (muted). Use `Option<f32>` where `None` means "never set" and `Some(0.0)` means "user muted it."
- **Auto-playing ambient without checking if main playlist restored successfully:** If the main playlist restore fails (playlist deleted, server unreachable), ambient restore should also be skipped. Don't start ambient in isolation.
- **Blocking on ambient download during restore:** The ambient track download must be async/background, not blocking. The existing `ambient_download_rx` + `check_ambient_download_complete()` pattern handles this. Don't introduce a blocking call in `restore_session()`.
- **Saving session on every ambient volume change:** The existing pattern saves session only on graceful exit. This is correct -- saving on every keypress would cause unnecessary disk I/O. Keep saving only in `save_session_state()`.
- **Adding a separate "ambient_session.toml" file:** Keep all session state in one file. The TOML format handles additional fields cleanly, and having one file to manage is simpler.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Backward-compatible deserialization | Custom TOML parsing to handle missing fields | `#[serde(default)]` on each new Session field | Serde handles this automatically; custom parsing is error-prone |
| Track identification persistence | Custom lookup table mapping track names to URLs | Store the Plex `part_key` directly in session | Part key is the natural identifier; it's stable and reconstructible |
| First-use volume calculation | Complex volume initialization logic | Simple `Option<f32>` check: `None` = compute default, `Some` = use saved | Pattern is simple; no library needed |
| Session file migration/versioning | Version numbers and migration functions | Serde `#[serde(default)]` makes new fields additive | Adding fields with defaults is inherently backward-compatible; no migration needed |

**Key insight:** The existing session persistence infrastructure (load/save/restore pattern, TOML format, best-effort error handling) is sufficient. Phase 9 is purely additive -- extending the Session struct and wiring save/restore for the new fields. No new infrastructure needed.

## Common Pitfalls

### Pitfall 1: Ambient Part Key Becomes Stale After Server Library Reorganization
**What goes wrong:** The user reorganizes their Plex music library (moves/renames files), and the saved `ambient_part_key` no longer resolves to a valid stream URL. The download fails on restore.
**Why it happens:** Plex part keys reference internal file paths. Library changes may invalidate them.
**How to avoid:** The restore code MUST handle download failures gracefully (log warning, skip ambient restore, continue with main playback). The existing `check_ambient_download_complete()` already handles `Ok(Err(e))` by logging and clearing `ambient_download_rx`. Same pattern applies: failed ambient restore is silent and non-blocking.
**Warning signs:** "Failed to download ambient track" in the log after a library reorganization.

### Pitfall 2: Session File with New Fields Written, Then Old App Version Reads It
**What goes wrong:** This is the reverse of backward compatibility. If a user downgrades from v1.1 to v1.0, the old Session struct encounters unknown fields (`ambient_part_key`, etc.) during deserialization.
**Why it happens:** The current Session struct does NOT have `#[serde(deny_unknown_fields)]`, so by default serde/toml ignores unknown fields during deserialization. This means the old app version will silently ignore the new ambient fields and load the session correctly.
**How to avoid:** Do NOT add `#[serde(deny_unknown_fields)]` to the Session struct. The current permissive behavior is correct for forward compatibility. Verify this by checking that the existing Session struct does not have that attribute (confirmed: it does not).
**Warning signs:** None expected. This works by default.

### Pitfall 3: Ambient Volume Default Calculation Returns Negative Value
**What goes wrong:** If the user's main volume is below 0.30 (e.g., 0.15), the PERSIST-05 formula `main_volume - 0.30` produces a negative value.
**Why it happens:** Simple subtraction without floor.
**How to avoid:** Always clamp: `(session.volume - 0.30).max(0.0)`. If main volume is very low, ambient defaults to 0.0 (silent). This is reasonable -- if the user has main volume at 15%, ambient at 0% is a sane default. The user can increase it manually.
**Warning signs:** Ambient volume displays as 0% on first use when main volume is below 30%.

### Pitfall 4: Pre-Mute Volume Not Persisted
**What goes wrong:** User sets ambient to 0.6, mutes with `m` (ambient goes to 0.0, pre_mute saved as 0.6), quits. On restore, session has `ambient_volume: Some(0.0)` and `ambient_enabled: false`. The pre-mute volume (0.6) is lost. When the user unmutes after restart, ambient goes to the default 0.3 instead of their saved 0.6.
**Why it happens:** Only `ambient_volume` (the current value, 0.0 when muted) is persisted, not `pre_mute_ambient_volume`.
**How to avoid:** When saving session state, if ambient is muted (volume == 0.0), save `pre_mute_ambient_volume` instead. This way the session captures the user's intended volume, not the muted state. Alternatively, persist both values. The simpler approach: `ambient_volume` in Session should always be the user's intended volume (pre-mute if currently muted).
**Warning signs:** Ambient volume resets to default after mute+quit+restart cycle.

### Pitfall 5: Ambient Restore Triggers Before Player Is Initialized
**What goes wrong:** `restore_session()` tries to start an ambient download, but the Player hasn't been created yet (Player is lazy-initialized on first main track play). The ambient download completes and `check_ambient_download_complete()` calls `load_ambient_track()` which requires `self.player` to be `Some`.
**Why it happens:** The Player is created in `check_download_complete()` when the first main track is downloaded. If ambient download completes before any main track is played, `self.player` is `None`.
**How to avoid:** In `check_ambient_download_complete()`, if `self.player` is `None`, initialize it (same pattern as `check_download_complete()`). OR: defer ambient download until the Player exists. The cleanest approach: in `check_ambient_download_complete()`, create the Player if needed (matching the same `Player::new()` + error handling from `check_download_complete()`). This also handles the edge case where the user starts TermTunes, ambient restores and downloads, but they haven't started any main music yet.
**Warning signs:** Ambient restore silently fails (no ambient playing after restart despite being saved) because `load_ambient_track` finds `self.player` is `None`.

### Pitfall 6: Ambient Part Key Contains Server-Specific Token in Path
**What goes wrong:** The part_key itself should be a server-relative path (e.g., `/library/parts/12345/file.flac`), but if someone accidentally stores the full URL (including `?X-Plex-Token=...`), the token would be persisted in the session file.
**Why it happens:** Confusion between `part_key` (the path component from `Track::media[0].parts[0].key`) and the full stream URL.
**How to avoid:** Store exactly `track.media[0].parts[0].key` -- this is always a relative path without any token. The `stream_url()` method adds the server URL and token. Verify by checking the `Part` struct: `key` is just a path string.
**Warning signs:** Session.toml contains `ambient_part_key = "http://...?X-Plex-Token=..."` instead of a path.

## Code Examples

Verified patterns from the existing codebase:

### Existing Session Struct (Before Extension)
```rust
// Source: config.rs lines 62-81
// Current Session struct -- all fields that exist in v1.0 session files.
// New ambient fields will be added below these.
#[derive(Serialize, Deserialize, Default, Debug)]
pub struct Session {
    pub playlist_rating_key: Option<String>,
    pub playlist_title: Option<String>,
    pub track_index: Option<usize>,
    pub volume: f32,
    pub shuffle_enabled: bool,
    pub repeat_mode: String,
}
```

### Existing Save Pattern
```rust
// Source: app.rs lines 1665-1677
// save_session_state constructs a Session from App state and writes it.
// This will be extended to include ambient fields.
fn save_session_state(&self) {
    let session = config::Session {
        playlist_rating_key: self.current_playlist_rating_key.clone(),
        playlist_title: Some(self.current_playlist_title.clone()),
        track_index: self.current_track_index,
        volume: self.saved_volume,
        shuffle_enabled: self.shuffle_enabled,
        repeat_mode: self.repeat_mode.to_string_repr().to_string(),
        // New fields will go here:
        // ambient_part_key: self.ambient_part_key.clone(),
        // ambient_track_name: self.player.as_ref().and_then(|p| p.ambient_track_name().map(String::from)),
        // ambient_volume: Some(if self.ambient_volume > 0.0 { self.ambient_volume } else { self.pre_mute_ambient_volume }),
        // ambient_enabled: self.ambient_volume > 0.0,
    };
    if let Err(e) = config::save_session(&session) {
        tracing::error!("Failed to save session state: {}", e);
    }
}
```

### Existing Restore Pattern
```rust
// Source: app.rs lines 1685-1764
// restore_session loads Session from disk and positions the user.
// Currently restores main playlist/track state.
// Will be extended to also restore ambient state.
pub async fn restore_session(&mut self) {
    let session = match config::load_session() {
        Some(s) => s,
        None => return,
    };
    // ... existing main restore logic ...
    // New: ambient restore logic will go after main restore.
}
```

### Existing Browser Track Selection (Where Part Key Is Available)
```rust
// Source: app.rs lines 1594-1639
// browser_select_track() has access to the track's part_key via
// track.media[0].parts[0].key -- this is what we need to persist.
fn browser_select_track(&mut self, idx: usize) -> Result<()> {
    let (stream_url, track_name) = {
        let tracks = match &self.browser_state {
            BrowserState::Tracks { tracks, .. } => tracks,
            _ => return Ok(()),
        };
        let track = match tracks.get(idx) { Some(t) => t, None => return Ok(()) };
        let part_key = track.media.first()
            .and_then(|m| m.parts.first())
            .map(|p| p.key.as_str());
        // part_key is available here but NOT currently saved to App state
        let part_key = match part_key { Some(key) => key, None => return Ok(()) };
        (self.plex_client.stream_url(part_key), track.title.clone())
    };
    // ... spawns download ...
}
```

### Existing load_session Error Handling
```rust
// Source: config.rs lines 159-166
// Best-effort loading: returns None on any error.
// #[serde(default)] on new fields means old files parse cleanly.
pub fn load_session() -> Option<Session> {
    let path = session_path();
    if !path.exists() { return None; }
    let contents = std::fs::read_to_string(&path).ok()?;
    toml::from_str(&contents).ok()
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| No ambient persistence | Session includes ambient track, volume, enabled state | Phase 9 | Complete ambient setup survives restarts |
| `Session` struct with 6 fields | Extended to ~10 fields with `#[serde(default)]` | Phase 9 | Backward compatible with v1.0 session files |
| Ambient track lost on restart | Part key stored in App and persisted | Phase 9 | Ambient auto-resumes after restart |
| No first-use ambient default | `Option<f32>` volume with computed default | Phase 9 | PERSIST-05: ambient starts 30% lower than main |

**Deprecated/outdated:**
- Nothing deprecated. All existing session logic remains unchanged; new fields are purely additive.

## Open Questions

1. **Should ambient restore block on download before entering the event loop, or use the background download pattern?**
   - What we know: The existing ambient download pattern is asynchronous (background thread + mpsc channel + `check_ambient_download_complete()`). The main track restore does NOT auto-play -- it positions the user at the track and waits for Enter.
   - What's unclear: Whether ambient should auto-download immediately or wait until the user explicitly starts any playback.
   - Recommendation: Use the background download pattern (non-blocking). Start the ambient download in `restore_session()` by spawning a thread (same as `browser_select_track`). The event loop's `check_ambient_download_complete()` will pick it up naturally. This provides the smoothest UX: by the time the user starts their main track, the ambient may already be loaded.

2. **Should we persist `pre_mute_ambient_volume` as a separate field?**
   - What we know: If the user mutes ambient (volume=0), quits, restarts, and unmutes, they expect their previous volume. We can either (a) persist both `ambient_volume` and `pre_mute_ambient_volume`, or (b) persist only the "intended" volume (pre-mute if currently muted) and derive the enabled/disabled state from `ambient_enabled`.
   - What's unclear: Whether the extra field adds complexity for minimal benefit.
   - Recommendation: Persist only `ambient_volume` (set to the pre-mute value when currently muted) and `ambient_enabled` (false when muted). On restore, if `ambient_enabled` is false, set `ambient_volume` to 0.0 and `pre_mute_ambient_volume` to the persisted volume. This uses two fields (volume + enabled) instead of three, and the restore logic is straightforward.

3. **Should ambient auto-resume require that the main playlist also restored successfully?**
   - What we know: The current restore_session returns early if the main playlist can't be found or fetched. If it returns early, ambient restore would also be skipped (since we'd add ambient restore after main restore in the same function).
   - What's unclear: Whether a user might want ambient to play even if they open TermTunes without a restorable main playlist.
   - Recommendation: Keep ambient restore gated on successful main restore (by placing it after the main restore code in `restore_session()`). The ambient feature is an accompaniment to main music; starting ambient alone without main context would be unusual. If the main restore fails, the user starts fresh, which is the right UX.

## Sources

### Primary (HIGH confidence)
- Existing codebase: `config.rs` (Session struct, load_session, save_session), `app.rs` (save_session_state, restore_session, browser_select_track, ambient state fields), `player.rs` (ambient_track_name, ambient_audio_data, load_ambient)
- Serde official documentation: `#[serde(default)]` field attribute for backward-compatible deserialization -- https://serde.rs/field-attrs and https://serde.rs/attr-default
- TOML crate: `toml::from_str` silently ignores unknown fields by default (no `deny_unknown_fields`), confirmed by inspecting existing Session struct

### Secondary (MEDIUM confidence)
- Phase 8 research and verification: Confirms all ambient UI, volume controls, and toggle mechanisms are in place and working
- Phase 6 research: Confirms dual-sink audio engine with Player::load_ambient, ambient_audio_data caching, and loop mechanism

### Tertiary (LOW confidence)
- None. All findings are derived from existing codebase analysis and verified serde documentation.

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - no new dependencies, purely extending existing Session struct and save/restore logic
- Architecture: HIGH - all patterns (Session struct extension, save/restore methods, background download) already exist in the codebase and are being extended, not invented
- Backward compatibility: HIGH - `#[serde(default)]` is a well-documented serde feature, and the existing Session struct already uses `Default` derive; verified with serde official docs
- Pitfalls: HIGH - all derived from direct codebase analysis (Player initialization timing, pre-mute volume loss, part_key vs URL confusion)
- Ambient restore flow: HIGH - uses the exact same background download + mpsc pattern already proven in browser_select_track and check_ambient_download_complete

**Research date:** 2026-02-11
**Valid until:** 2026-03-11 (stable domain -- serde/toml behavior well-established, codebase patterns proven)
