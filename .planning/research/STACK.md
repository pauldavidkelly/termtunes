# Stack Additions for v1.1: Multi-Channel Audio & Track Browsing

**Project:** TermTunes v1.1
**Researched:** 2026-02-10
**Confidence:** HIGH
**Scope:** Additions/changes to existing stack only. v1.0 stack is validated and unchanged.

## Executive Summary

No new crate dependencies are needed. Rodio 0.21 already supports concurrent playback through multiple `Sink` instances connected to the same `OutputStream.mixer()`. The existing Plex API client with `Accept: application/json` headers already works for library browsing endpoints. The entire v1.1 feature set can be built with zero new dependencies.

## What Changes from v1.0

### New Dependencies Required

**None.** This is the key finding. The existing `Cargo.toml` has everything needed:

| Existing Crate | v1.1 Usage | What's New |
|----------------|-----------|------------|
| rodio 0.21 | Second `Sink` for ambient channel | Using `Sink::connect_new()` on same `mixer()` -- already supported, just not used |
| reqwest 0.13 | Library section browsing endpoints | New API calls using existing `PlexClient` patterns |
| serde + serde_json | Deserialize library/section/artist/album responses | New struct types for library data, same pattern as `Playlist`/`Track` |
| ratatui 0.30 | Track browser UI panel | New view/panel, same widget patterns |

### Rodio Multi-Sink Architecture (HIGH Confidence)

The existing `Player` struct creates one `OutputStream` and one `Sink`. For v1.1, a second `Sink` is created on the same `OutputStream.mixer()`:

```rust
// Existing pattern in player.rs (v1.0):
let stream = OutputStreamBuilder::open_default_stream()?;
let sink = Sink::connect_new(stream.mixer());

// v1.1 addition -- same stream, second sink:
let ambient_sink = Sink::connect_new(stream.mixer());
```

**Per rodio docs (verified):** "All the sounds are mixed together by rodio before being sent to the operating system or the hardware." Multiple sinks on the same mixer play concurrently with independent volume control via `set_volume()`. There is no restriction on the number of simultaneous sinks.

**Key capabilities per Sink (independent for each channel):**
- `set_volume(f32)` -- per-sink volume (0.0 to 1.0+)
- `play()` / `pause()` -- per-sink pause/resume
- `stop()` -- per-sink stop
- `append(source)` -- per-sink audio source queue
- `empty()` -- per-sink queue status

### Ambient Looping Strategy (MEDIUM Confidence)

Rodio's `Source` trait provides `.repeat_infinite()` for looping. For the ambient channel, the audio needs to loop continuously under the main music.

**Approach:** Use rodio's built-in `repeat_infinite()` on the decoded source before appending to the ambient sink.

```rust
let source = Decoder::builder()
    .with_data(cursor)
    .build()?;
// Loop forever for ambient playback
let looping_source = source.repeat_infinite();
ambient_sink.append(looping_source);
```

**Memory concern:** `repeat_infinite()` buffers the entire decoded audio in memory. For a typical ambient track (3-10 minutes of audio, ~30-100MB decoded PCM), this is acceptable. There is a known issue (rodio #673) where memory grows temporarily during initial loops before stabilizing. For ambient tracks that are already fully downloaded into memory (matching the existing download-then-play pattern), this is a non-issue -- the data is already in memory.

**Alternative (if memory is a concern for very long ambient files):** Instead of `repeat_infinite()`, detect when the ambient sink is empty (`ambient_sink.empty()`) in the event loop and re-append the source from cached bytes. This matches the existing `replay_current()` pattern. This is the safer approach and avoids the `repeat_infinite()` memory quirk entirely.

**Recommendation:** Use the manual re-append approach (check `empty()` in event loop) because:
1. It matches the existing `replay_current()` pattern
2. No memory growth concern
3. Allows clean crossover between ambient tracks
4. The event loop already runs at 100ms ticks, so the gap between loops is imperceptible

### Source Trait Methods Useful for Ambient (HIGH Confidence)

These are available on any rodio `Source` and need no extra dependencies:

| Method | Purpose for v1.1 | Notes |
|--------|-------------------|-------|
| `fade_in(duration)` | Smooth ambient track start | Prevents jarring sudden volume |
| `fade_out(duration)` | Smooth ambient track stop | Not available as standalone -- use volume ramp instead |
| `amplify(f32)` | Fine-grained ambient volume | Multiplies samples, useful on top of sink volume |
| `repeat_infinite()` | Loop ambient track forever | Available but manual re-append is preferred |
| `pausable(bool)` | Wrap source for pause control | Already handled by Sink's play()/pause() |

### Plex API for Library Browsing (HIGH Confidence)

The existing `PlexClient` uses `Accept: application/json` headers (set in `build_plex_headers()` in auth.rs), so all library endpoints return JSON, not XML. This is already proven by the working playlist/track endpoints.

**New endpoints needed:**

| Endpoint | Purpose | Type Param | Response |
|----------|---------|------------|----------|
| `GET /library/sections` | List all libraries (find music sections) | N/A | `MediaContainer` with section list |
| `GET /library/sections/{id}/all` | All artists in a music section | `type=8` (artist) | `MediaContainer` with artist `Metadata[]` |
| `GET /library/sections/{id}/all?type=9` | All albums in a section | `type=9` (album) | `MediaContainer` with album `Metadata[]` |
| `GET /library/sections/{id}/all?type=10` | All tracks in a section | `type=10` (track) | `MediaContainer` with track `Metadata[]` |
| `GET /library/metadata/{ratingKey}/children` | Albums for an artist, or tracks for an album | N/A | `MediaContainer` with child `Metadata[]` |
| `GET /hubs/search?query=X&sectionId=Y` | Search within a library section | N/A | `MediaContainer` with `Hub[]` results |

**Plex music type IDs (verified via Plexopedia and Python PlexAPI source):**
- `type=8` = Artist
- `type=9` = Album
- `type=10` = Track

**Implementation pattern:** These endpoints use the same `reqwest` + `serde_json` pattern as the existing `fetch_playlists()` and `fetch_tracks()` methods. The response structure is nearly identical -- `MediaContainer` wrapping `Metadata[]`. New serde structs are needed for library sections and the hierarchical artist/album/track response, but no new HTTP patterns.

**Pagination:** For large libraries, the endpoints support `X-Plex-Container-Start` and `X-Plex-Container-Size` headers (or `?start=N&size=M` query params). The existing code does not paginate playlists. For track browsing, pagination may be needed if a library has thousands of tracks. Start without pagination, add if needed.

### Track Browsing Data Model

The existing `Track` struct in plex.rs already has all fields needed for library-sourced tracks (title, artist, album, duration, media/parts). The only new types needed are:

```
LibrarySection { key, title, type_ }   -- to identify music libraries
Artist { ratingKey, title, thumb }      -- for artist browsing (optional)
Album  { ratingKey, title, year, thumb } -- for album browsing (optional)
```

For a minimal v1.1, the track browser can go directly to `GET /library/sections/{id}/all?type=10` to list all tracks in a library, reusing the existing `Track` struct. Hierarchical artist/album navigation can be a follow-up.

## What NOT to Add

| Temptation | Why Avoid | What to Do Instead |
|------------|-----------|-------------------|
| `cpal` direct dependency | You might think you need low-level mixer control for two channels. You do not. Rodio's Sink-per-channel model handles it. Adding cpal creates API surface conflict with rodio's pinned cpal version. | Use rodio's multi-Sink pattern on `stream.mixer()` |
| `crossbeam` channels | You might want lock-free channels between the ambient player and the UI. The existing `std::sync::mpsc` pattern works fine for ambient downloads. | Keep using `std::sync::mpsc` for download completion, `Mutex<>` for shared state |
| Audio mixing/DSP library | You might think you need a mixer to blend two audio streams. Rodio's hardware mixer does this automatically when multiple sinks feed the same output stream. | Let rodio handle mixing implicitly |
| `tokio::sync::mpsc` | You might want async channels for the ambient download. The existing blocking download on `std::thread` + sync mpsc pattern is proven and simpler. | Keep the `std::thread::spawn` + `std::sync::mpsc::channel` pattern |
| Separate `OutputStream` per channel | You might think each audio channel needs its own output stream. This would open TWO audio devices, which may fail on WSL2 (single PulseAudio sink). | Both sinks MUST share one `OutputStream` |
| Complex state machine for ambient | You might want an elaborate state machine (Loading, Playing, Paused, Fading, Crossfading). For v1.1, ambient is simple: play/stop/volume. | Simple `Option<AmbientState>` with play/stop, volume up/down |

## Integration with Existing Architecture

### Player Struct Changes

The `Player` struct currently holds one `_stream`, one `sink`, and one `_audio_data`. For v1.1:

```
Player {
    _stream: OutputStream,          // UNCHANGED - shared by both channels
    sink: Sink,                      // UNCHANGED - main music channel
    ambient_sink: Option<Sink>,      // NEW - created lazily, connected to same mixer
    _audio_data: Option<Vec<u8>>,    // UNCHANGED - main track cached bytes
    ambient_audio_data: Option<Vec<u8>>, // NEW - ambient track cached bytes
    current_track: Option<String>,   // UNCHANGED
    ambient_track: Option<String>,   // NEW - ambient track name for UI
}
```

Key design: `ambient_sink` is `Option<Sink>` because the ambient channel may not always be active. Created with `Sink::connect_new(self._stream.mixer())` when the first ambient track is loaded.

### WSL2 Compatibility (HIGH Confidence)

No concerns. Both sinks feed the same `OutputStream`, which opens a single ALSA/PulseAudio device. The existing WSL2 audio setup (`.asoundrc` config, PulseAudio routing) handles mixed audio from a single output device. Multiple sinks are mixed by rodio in software before reaching the OS audio layer.

### Download Pattern Compatibility

The existing download-then-play pattern works for ambient tracks:
1. User selects an ambient track from the library browser
2. Download on background `std::thread` (same as main track downloads)
3. On completion, decode and append to `ambient_sink` with `fade_in()`
4. When ambient track finishes (sink.empty()), re-append from cached bytes

The ambient download channel needs its own `mpsc::Receiver` separate from the main track download receiver. This is a second `download_rx` field on the App struct (e.g., `ambient_download_rx`).

## Cargo.toml Changes

**None required.** The existing Cargo.toml at v1.0 already includes everything needed:

```toml
# Already present -- no changes needed:
rodio = { version = "0.21", features = ["symphonia-all"] }
reqwest = { version = "0.13", features = ["json", "blocking", "query"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
ratatui = { version = "0.30", features = ["crossterm"] }
```

## Alternatives Considered for v1.1

| Decision | Chosen | Alternative | Why Chosen |
|----------|--------|-------------|------------|
| Multi-channel audio | Two `Sink`s on one `OutputStream` | Single `Sink` with `source.mix(other_source)` | Two sinks give independent volume/pause/stop control. `source.mix()` merges into one source with no independent control. |
| Ambient looping | Manual re-append in event loop | `repeat_infinite()` | Avoids memory growth issue (#673). Matches existing replay pattern. Cleaner ambient track switching. |
| Track browsing depth | Flat track list (`type=10`) | Hierarchical artist > album > track | Flat list is simpler, gets to MVP faster. Hierarchy can be added in v1.2. |
| Library search | `GET /hubs/search?sectionId=X` | Client-side filter on full track list | Server-side search handles large libraries, does fuzzy matching. But for v1.1, a simple title filter on the loaded track list may suffice for small/medium libraries. |
| Ambient volume UI | Separate volume control (e.g., `[`/`]`) | Shared volume with main | Users need independent control. Main music at 80%, ambient at 20% is a common pattern. |

## Sources

- [Rodio docs.rs](https://docs.rs/rodio/latest/rodio/) -- v0.21.1, multi-Sink and Mixer docs, HIGH confidence
- [Rodio Sink docs](https://docs.rs/rodio/latest/rodio/struct.Sink.html) -- connect_new(), volume control, HIGH confidence
- [Rodio Source trait](https://docs.rs/rodio/latest/rodio/source/trait.Source.html) -- repeat_infinite(), fade_in(), mix(), HIGH confidence
- [Rodio issue #673](https://github.com/RustAudio/rodio/issues/673) -- repeat_infinite() memory growth, stabilizes, MEDIUM confidence
- [Plexopedia music API](https://www.plexopedia.com/plex-media-server/api/library/music/) -- /library/sections/{id}/all, type=8/9/10, HIGH confidence
- [Plexopedia albums API](https://www.plexopedia.com/plex-media-server/api/library/music-albums-artist/) -- type=9 confirmed, HIGH confidence
- [Plex search API](https://plexapi.dev/api-reference/search/perform-a-search) -- /hubs/search endpoint, MEDIUM confidence
- [Python PlexAPI audio.py](https://github.com/pkkid/python-plexapi/blob/master/plexapi/audio.py) -- TYPE values confirmation, HIGH confidence
- [Plex URL commands](https://support.plex.tv/articles/201638786-plex-media-server-url-commands/) -- general endpoint patterns, HIGH confidence

---
*Stack additions research for: TermTunes v1.1 (Multi-Channel Audio & Track Browsing)*
*Researched: 2026-02-10*
