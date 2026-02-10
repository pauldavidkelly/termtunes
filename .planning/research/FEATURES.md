# Feature Research: Multi-Channel Audio (v1.1)

**Domain:** Ambient track layering for TUI music player
**Researched:** 2026-02-10
**Confidence:** HIGH (codebase verified, rodio docs confirmed, ambient app ecosystem surveyed)

## Context

TermTunes v1.0 plays a single audio channel (music from Plex playlists). v1.1 adds a
second audio channel -- an ambient track that plays underneath the main music. The user
case is deep work focus: lofi playlist playing, rain or forest sounds layered beneath it,
independent volume controls so the ambient sits at 20-30% and the music at 70%.

This research covers ONLY multi-channel audio features, ambient track browsing/selection,
and mixing controls. General player features (shuffle, repeat, favorites, etc.) are
already built and documented in the v1.0 research.

## Existing Architecture (Constraints)

Before detailing features, here is what already exists and constrains design:

- **Audio backend:** rodio 0.21 with symphonia-all. Single `OutputStream` + single `Sink`.
- **Player struct:** Owns `_stream: OutputStream` and `sink: Sink`. One track at a time.
- **Volume:** `saved_volume: f32` on App, applied to the single Sink. +/- keys step by 0.05.
- **Download model:** Track bytes downloaded to memory via `reqwest::blocking`, decoded from
  `Cursor<Vec<u8>>`, appended to Sink. One download at a time via mpsc channel.
- **Visualizer:** Wraps the source in `VisualizerSource` that taps samples for FFT.
- **UI:** ratatui vertical layout: main content (list) + optional visualizer + player bar.
- **Plex API:** Currently only fetches playlists and playlist tracks. No library browsing.

**Key rodio fact (verified via docs.rs):** Multiple Sinks can be created from the same
`OutputStream.mixer()`. All Sinks mix automatically before output. Each Sink has independent
`set_volume()`. This is the foundation for the ambient channel -- a second Sink on the
same OutputStream, with its own volume control.

## Table Stakes

Features users expect from any dual-channel / ambient mixing system. Missing these
makes the feature feel broken.

| Feature | Why Expected | Complexity | Dependencies | Notes |
|---------|--------------|------------|--------------|-------|
| Second audio channel (ambient Sink) | The entire point of v1.1. Without a second independently controlled audio stream, there is no ambient layer. Every ambient mixer app (Moodist, myNoise, Noisli, Coffitivity) provides independent sound streams. | MEDIUM | Extends `Player` struct with second Sink | rodio supports this natively -- create second `Sink::connect_new(stream.mixer())`. Each Sink has independent volume. Main complexity is lifecycle management. |
| Independent volume controls | Users of Moodist, myNoise, Noisli all expect per-channel volume sliders. The whole value is tuning the mix ratio. Without independent volume, the channels are useless -- you cannot make rain quiet under loud music. | LOW | Requires second Sink exists | rodio `Sink::set_volume()` on each Sink independently. Need new keybindings for ambient volume vs main volume. |
| Ambient mute/unmute toggle | Every ambient app has a quick way to silence the ambient without stopping it. When a coworker starts talking, you mute the rain, keep the music. When the call ends, unmute -- rain resumes from where it was. | LOW | Requires second Sink exists | `Sink::pause()` / `Sink::play()` on the ambient Sink. Single keybinding toggle. |
| Ambient track selection from Plex playlists | The user needs to pick an ambient track from somewhere. TermTunes is Plex-native, so ambient tracks should come from Plex playlists too. The user likely has an "Ambient" or "Nature Sounds" playlist on their Plex server. | MEDIUM | Existing playlist fetch infrastructure | Reuse the existing `PlexClient::fetch_playlists()` + `fetch_tracks()` flow. UI needs a way to browse and select a track specifically for the ambient channel. |
| Ambient track looping | Ambient sounds must loop seamlessly. A 3-minute rain track that stops after 3 minutes is useless -- the whole point is continuous background texture. Every ambient app (Moodist, myNoise, Ambient Mixer, Noisli) loops sounds infinitely. | MEDIUM | Requires ambient Sink management | rodio Sink is sequential -- when it empties, re-append the same source. Detect `sink.empty()` in the event loop and re-decode from cached bytes. The main player already caches `_audio_data` for `replay_current()`, same pattern applies. |
| Persist ambient state across session | If the user has rain at 25% volume playing under their music, restarts TermTunes, and the ambient is gone, that is a broken experience. Session persistence already exists for main channel (playlist, track, volume, shuffle, repeat). Ambient state must be included. | LOW | Extends `Session` struct | Add `ambient_playlist_key`, `ambient_track_index`, `ambient_volume`, `ambient_enabled` to session.toml. Same save/restore pattern as existing session. |
| Visual indication of ambient status | The user needs to see at a glance: is ambient playing? What track? What volume? Without this, the ambient layer is invisible and confusing. All ambient apps show what sounds are active and their levels. | LOW | Extends player bar UI | Add ambient info to the player bar (line 3 or a new line). Show track name + volume % + muted indicator. |

## Differentiators

Features that go beyond the basics and make the ambient layer genuinely useful for
the deep work use case. Not strictly required, but significantly improve the experience.

| Feature | Value Proposition | Complexity | Dependencies | Notes |
|---------|-------------------|------------|--------------|-------|
| Ambient playlist assignment to hotkey (0 key) | Main music uses 1-9 for favorite playlists. Ambient could use the 0 key (or a modifier like `a1`-`a9`) for favorite ambient playlists. One-keypress ambient activation matches the TermTunes philosophy of "press a key, get your environment." | LOW | Extends favorites config | Reuse the favorite playlist infrastructure. Add an `ambient_favorites` section to config.toml. Keeps the "press key, done" workflow. |
| Plex library browsing for ambient track selection | Users may not have ambient sounds organized as playlists. They might have an "Ambient" music library section with albums like "Rain Sounds," "Forest," "Ocean." Library-level browsing (section -> artist/album -> track) gives access to the full ambient library. | HIGH | New Plex API endpoints (`/library/sections/{id}/all`, type filters) | This is the biggest new Plex API surface area. Plex supports `GET /library/sections/{id}/all` for artists, `/albums` for albums, and track-level browsing. Adds new UI views (library browser). Consider deferring to v1.2 if playlist-based selection is sufficient. |
| Master volume control | A single key to scale both channels proportionally. If main is at 70% and ambient is at 25%, master volume down should reduce both while preserving the ratio. Standard in mixing consoles and DAWs. | MEDIUM | Requires tracking both volumes | Implement as a multiplier applied to both Sinks. Existing +/- keys become master, new keys for per-channel. Or: existing +/- stay on main, new keys for ambient, and a third pair for master. Need to decide keybinding scheme carefully. |
| Crossfade on ambient track change | When switching ambient tracks (e.g., rain to ocean), an abrupt cut is jarring. A 1-2 second crossfade between old and new ambient track creates a smooth transition. Ambient Mixer and myNoise both fade between sounds. | HIGH | Requires managing two ambient Sinks temporarily during crossfade | rodio has no built-in crossfade between Sinks. Would need to manually ramp volume down on old Sink while ramping up on new Sink over ~1-2 seconds. Can use the event loop tick (100ms) to step volume. Complex but doable. |
| Ambient-only mode (no music, just ambient) | Sometimes the user just wants rain sounds with no music. The ambient channel should work independently. Moodist, Noisli, and myNoise all work without any "main" content. | LOW | Ambient Sink is independent of main Sink | This should work naturally if the two Sinks are independent. Main channel can be empty/stopped while ambient plays. Just need to ensure the UI handles this state gracefully. |
| Quick mix presets | Save named preset mixes (e.g., "Deep Work" = lofi playlist + rain at 25%, "Reading" = classical playlist + fireplace at 15%). Recall with a keybinding or command. | MEDIUM | Extends config with named presets | Useful for the "different moods for different tasks" workflow. Store as TOML entries: playlist key, ambient track key, main volume, ambient volume. Could map to function keys or a command palette. |
| Ambient track search/filter | When browsing ambient tracks, a `/` search (vim-style) to filter by name. With hundreds of ambient tracks, scrolling through a list is slow. | LOW | Extends list navigation | The same search/filter pattern that could apply to track lists generally. Type `/rain` to filter to tracks containing "rain." Standard vim pattern. |

## Anti-Features

Features that seem useful for multi-channel mixing but would be actively harmful for
this use case. Do NOT build these.

| Anti-Feature | Why It Seems Useful | Why Avoid | What to Do Instead |
|--------------|--------------------|-----------|--------------------|
| More than 2 channels | Ambient mixer apps support 8+ simultaneous sounds (rain + thunder + birds + fire). More channels = more immersive. | Exponential complexity in UI, keybindings, and volume management. TUI has limited screen space. The target user wants exactly 2 layers: music + one ambient texture. If they need complex soundscapes, they should use Moodist or myNoise in a browser tab alongside TermTunes. | Hard-cap at 2 channels: main music + one ambient. Simple, focused, keyboard-friendly. |
| Audio effects / processing on ambient | Reverb, EQ, spatial panning on the ambient track. myNoise has stereo width control. | Turns TermTunes into a DAW. rodio does not have built-in effects beyond volume. Adding audio processing requires either custom Source wrappers or external crates. Massive scope increase for minimal value -- the ambient tracks should already be mixed properly. | Use pre-mixed ambient tracks. PulseAudio/PipeWire system EQ if needed. |
| Real-time ambient generation | Generate rain/white noise/brown noise procedurally instead of playing files. Moodist and Noisli generate sounds algorithmically. | Requires implementing audio synthesis. Very different from file playback. Adds a completely new code path. The user already has ambient sounds in their Plex library. | Play ambient files from Plex. If user wants generated noise, they can run a separate tool (`sox play -n synth brownoise`). |
| Ambient track playlists with auto-advance | Play through a sequence of ambient tracks (rain for 30 min, then ocean, then forest). | The whole point of ambient is continuity. Auto-advancing ambient tracks is jarring. The user picks one ambient texture and leaves it. If they want variety, they change it manually. | Single ambient track with infinite loop. User changes manually when they want a different texture. |
| Volume curves / ducking | Automatically lower music volume when ambient gets louder, or duck ambient during track transitions. Standard in radio/podcast production. | Complex audio routing. Requires real-time analysis of both channels. Over-engineering for the use case. The user sets their preferred ratio once and leaves it. | Let the user set a static volume ratio. Trust them to adjust when they want to. |
| Ambient timer / sleep timer | Play ambient for N minutes then fade out. Moodist and Noisli have sleep timers. | TermTunes is for focus during work, not for falling asleep. The ambient should play as long as the user is working. Adding timers adds UI complexity for a use case that does not match the product. | Ambient plays until explicitly stopped or TermTunes exits. |

## Feature Dependencies

```
[Existing v1.0 Infrastructure]
    +-- OutputStream (already created in Player::new())
        +-- enables --> [Ambient Sink] (second Sink::connect_new on same mixer)

[Ambient Sink]
    +-- enables --> [Independent Volume Control]
    +-- enables --> [Ambient Mute/Unmute]
    +-- enables --> [Ambient Track Looping] (re-append source when Sink empties)
    +-- enables --> [Ambient-Only Mode] (naturally works if main is stopped)
    +-- enables --> [Visual Ambient Status] (read ambient Sink state for UI)

[Ambient Track Selection]
    +-- requires --> [Plex Playlist Fetch] (already exists)
    +-- OR requires --> [Plex Library Browse] (NEW - high complexity)
    +-- enables --> [Ambient Track Download] (same download_track pattern)
    +-- enables --> [Ambient Favorite Hotkeys] (extends config)

[Ambient State Persistence]
    +-- requires --> [Ambient Sink] (need state to persist)
    +-- requires --> [Session Infrastructure] (already exists)
    +-- extends --> session.toml with ambient fields

[UI Updates]
    +-- requires --> [Ambient Sink] (need state to display)
    +-- extends --> Player bar with ambient status line
    +-- extends --> Keybinding help text
    +-- extends --> Tmux now-playing file (optionally include ambient info)
```

### Key Dependency Chain

The critical path is:

1. **Ambient Sink creation** -- foundation for everything else
2. **Ambient track selection UI** -- user needs a way to pick a track
3. **Ambient track download + playback** -- actually play the selected track
4. **Ambient looping** -- keep it playing continuously
5. **Volume controls + UI feedback** -- user can tune the mix
6. **Session persistence** -- remember state across restarts

Steps 1-4 are sequential dependencies. Steps 5-6 can be done in parallel with 3-4.

## Feature Categories for Requirements Organization

Based on the dependency analysis, features group into these natural categories:

### Category 1: Ambient Audio Engine
Core audio infrastructure for the second channel.
- Second Sink creation and lifecycle management
- Ambient track loading, decoding, and playback
- Ambient track looping (auto-replay when finished)
- Independent volume control (set_volume on ambient Sink)
- Ambient pause/resume (mute toggle)

### Category 2: Ambient Track Selection
How the user browses and picks an ambient track.
- Browse Plex playlists for ambient track selection (reuse existing flow)
- Ambient track browsing UI (new view or modal overlay)
- Ambient favorite hotkey assignment and activation
- (Deferred) Plex library-level browsing for ambient tracks

### Category 3: Mixing Controls & Keybindings
Keyboard interface for controlling the mix.
- Ambient volume up/down keybindings
- Ambient mute/unmute toggle keybinding
- Main volume keybindings (existing, may need disambiguation)
- (Optional) Master volume keybinding

### Category 4: UI & Status Display
Visual feedback for the ambient layer.
- Ambient status in player bar (track name, volume, playing/paused)
- Updated keybinding help text
- Tmux now-playing file update (include ambient track info)
- Narrow-mode handling for ambient status

### Category 5: State Persistence
Remember ambient configuration across sessions.
- Ambient fields in session.toml
- Save ambient state on exit
- Restore ambient state on startup
- Ambient favorite playlists in config.toml

## Keybinding Design Considerations

The existing keybinding space is:

| Key | Current Function |
|-----|-----------------|
| `j/k` | Navigate up/down |
| `Enter` | Select item |
| `Space` | Play/pause (main) |
| `+/=` | Volume up (main) |
| `-/_` | Volume down (main) |
| `n/>` | Next track |
| `N/<` | Previous track |
| `h/l` | Seek backward/forward |
| `s` | Toggle shuffle |
| `r` | Toggle repeat |
| `v` | Toggle visualizer |
| `f` | Assign favorite |
| `1-9` | Play favorite |
| `q` | Quit |
| `Esc` | Go back |

**Recommended ambient keybinding scheme (needs validation in requirements):**

| Key | Proposed Function | Rationale |
|-----|-------------------|-----------|
| `a` | Enter ambient mode / open ambient track browser | Mnemonic: "a" for ambient. Currently unused. |
| `m` | Mute/unmute ambient | Mnemonic: "m" for mute. Currently unused. |
| `[` | Ambient volume down | Bracket keys are adjacent to +/- on keyboard. Creates visual parallel: +/- for main, [/] for ambient. |
| `]` | Ambient volume up | Same rationale as `[`. |
| `0` | Play favorite ambient | Extends the 1-9 favorite system. 0 is "the other channel." |

This scheme uses only currently-unbound keys. The `a` key as an entry point to ambient
browsing keeps the main flow uncluttered -- user presses `a`, sees ambient track selection,
picks a track, presses `Esc` to return to the main view with ambient now playing.

## Complexity Assessment Summary

| Feature | Complexity | Rationale |
|---------|------------|-----------|
| Second Sink + independent volume | LOW | rodio natively supports this. ~50 lines of code to add to Player. |
| Ambient mute/unmute | LOW | Single method call: `sink.pause()` / `sink.play()`. |
| Ambient track download + play | LOW | Identical pattern to main track: download bytes, decode, append to Sink. |
| Ambient looping | MEDIUM | Need event loop logic to detect empty Sink and re-append. Must handle edge cases (track decoding failure on loop, concurrent download). |
| Ambient track selection UI | MEDIUM | New browsing flow within the TUI. Need a new AppView state or modal. Must show playlists, then tracks, for the ambient channel specifically. |
| Session persistence for ambient | LOW | Add 4 fields to Session struct, extend save/restore. Pattern already established. |
| Player bar ambient status | LOW | Add one Line of spans to the player bar rendering. |
| Ambient favorite hotkeys | LOW | Same infrastructure as existing favorites, different config key. |
| Plex library browsing | HIGH | New API surface (sections, artists, albums, tracks). New UI views. Significant scope. Recommend deferring. |
| Crossfade between ambient tracks | HIGH | Manual volume ramping across event loop ticks. Temporary dual-Sink management. Edge cases with rapid switching. Recommend deferring. |
| Master volume control | MEDIUM | Need to track a master multiplier and apply to both Sinks. Adds a third volume concept to the UI. |

## MVP Recommendation for v1.1

### Must Have (v1.1 launch)

1. **Ambient Sink with independent volume** -- the core feature
2. **Ambient track selection from Plex playlists** -- user must be able to pick a track
3. **Ambient looping** -- ambient must loop forever
4. **Ambient mute/unmute** -- quick toggle
5. **Ambient volume controls** -- independent of main volume
6. **Ambient status in player bar** -- user needs visual feedback
7. **Ambient session persistence** -- state survives restarts

### Defer (v1.2+)

- Plex library browsing (use playlists for now, covers 90% of the use case)
- Crossfade between ambient tracks (abrupt cut is acceptable for v1.1)
- Master volume control (user can adjust channels independently)
- Quick mix presets (nice-to-have, not blocking)
- Ambient track search/filter (useful once track lists get long)

## Sources

- [rodio docs - Sink](https://docs.rs/rodio/latest/rodio/struct.Sink.html) -- verified: multiple Sinks share one OutputStream, independent volume control
- [rodio docs - mixer](https://docs.rs/rodio/latest/rodio/dynamic_mixer/index.html) -- alternative to multiple Sinks for parallel playback
- [rodio GitHub](https://github.com/RustAudio/rodio) -- confirmed: no restriction on simultaneous Sink count
- [Moodist](https://moodist.mvze.net/) -- open-source ambient sound app, layered sounds with individual volume, presets
- [myNoise](https://mynoise.net/) -- 10-slider mixer, per-element volume, animate mode, save-to-URL
- [Noisli](https://www.noisli.com) -- intuitive slider-based mixing, simple UX for layering sounds
- [Coffitivity](https://coffitivity.com/) -- specifically mixes user's music with ambient coffee shop sounds
- [Ambient Mixer](https://www.ambient-mixer.com/) -- 8-channel mixer with crossfade, mute per channel, looping options
- [Focurio](https://focurio.web.app/) -- ambient sound mixer with focus timer
- [A Soft Murmur](https://asoftmurmur.com/) -- simple ambient mixing for focus
- [Deepfocus.io](https://deepfocus.io/) -- ambient sounds with music, timed sessions
- [Plex API - Search Hub](https://plexapi.dev/api-reference/search/perform-a-search) -- hub-based search with type filtering
- [Plex API - Library Sections](https://www.plexopedia.com/plex-media-server/api/library/music/) -- music library endpoints for artist/album/track browsing
- [10HourLoop - Crossfade Guide](https://10hourloop.com/blog/what-is-crossfade-audio-looping/) -- crossfade looping patterns for ambient audio (1-3 second crossfades)

---
*Feature research for: Multi-channel audio / ambient track layering (TermTunes v1.1)*
*Researched: 2026-02-10*
