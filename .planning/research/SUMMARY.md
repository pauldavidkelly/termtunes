# Project Research Summary

**Project:** TermTunes v1.1 - Multi-Channel Audio & Track Browsing
**Domain:** TUI music player enhancement (ambient audio layering)
**Researched:** 2026-02-10
**Confidence:** HIGH

## Executive Summary

TermTunes v1.1 adds a second audio channel for ambient track layering beneath the main music playback. The good news: the existing rodio 0.21 stack fully supports this with zero new dependencies. Multiple Sinks can share one OutputStream, each with independent volume control. The architecture is clean: create a second Sink from the same mixer, implement manual looping via event loop detection (not `repeat_infinite()` due to confirmed memory leaks), and add Plex library browsing endpoints using the existing JSON API pattern.

The critical risks are all audio-related, not architectural. Mixer clipping (when combined channel volumes exceed 1.0) causes harsh distortion that users will blame on WSL2 audio quality. The rodio `repeat_infinite()` method has a documented memory leak that will consume gigabytes over hours of ambient playback. WSL2 pause/resume behavior with two concurrent sinks is unproven and may cause channel desynchronization. All three are preventable with correct implementation: enforce a volume budget (main + ambient <= 1.0), use manual re-append looping from cached bytes, and test pause/resume behavior early on WSL2.

The roadmap should be built around risk mitigation. Phase 1 validates the dual-sink architecture with minimal feature surface (hardcoded test audio, no UI). Phase 2 builds the core feature (browsing, download, looping) once audio quality is proven. Phase 3 adds polish (session persistence, favorites). This order ensures that if WSL2 multi-sink audio is fundamentally broken, the project fails fast before investing in UI/UX work.

## Key Findings

### Recommended Stack

No new dependencies required. The existing Cargo.toml already contains everything needed for v1.1.

**Core technologies:**
- rodio 0.21 with symphonia-all: Multi-sink support built-in via `Sink::connect_new(stream.mixer())` — each Sink has independent volume, pause, stop control while sharing one OutputStream
- reqwest 0.13 with JSON features: Plex library browsing endpoints use identical HTTP pattern as existing playlist API — same `Accept: application/json` headers, same `MediaContainer` response structure
- ratatui 0.30: Track browser UI uses existing modal popup pattern (Clear widget + bordered Block) — well-documented in official examples

**Critical finding:** Rodio's `repeat_infinite()` has a confirmed memory leak (issue #673, unfixed in 0.21) that grows ~10MB per 15 seconds until stabilizing at 100-300MB. For ambient tracks that loop for hours, this is unacceptable. Use manual loop detection (`sink.empty()` check in event loop + re-append from cached bytes) instead. This matches the existing `replay_current()` pattern and has zero overhead.

### Expected Features

**Must have (table stakes):**
- Second audio channel (ambient Sink) with independent lifecycle
- Independent volume controls (main music vs ambient track)
- Ambient mute/unmute toggle (quick silence without stopping)
- Ambient track selection from Plex (playlist-based initially)
- Ambient track looping (seamless infinite playback)
- Session persistence for ambient state (survives restart)
- Visual indication of ambient status (track name, volume, playing/paused)

**Should have (competitive advantage):**
- Plex library browsing for ambient tracks (section -> tracks, flat view)
- Ambient favorite hotkey assignment (extends 1-9 playlist favorites with 0 or `a1-a9`)
- Ambient-only mode (ambient plays without main music)
- Ambient track search/filter (when track lists get long)

**Defer (v2+):**
- Hierarchical artist/album browsing (flat track list suffices for v1.1)
- Crossfade between ambient tracks (abrupt cut acceptable for MVP)
- Master volume control (independent channel control is simpler)
- Quick mix presets (nice-to-have)
- More than 2 channels (scope creep, UI complexity explosion)

### Architecture Approach

The architecture is extension, not rewrite. All existing v1.0 code remains unchanged. The Player struct gains an `ambient_sink: Option<Sink>` field alongside the existing `sink: Sink`, both connected to the same `_stream.mixer()`. The App event loop adds two new checks per 100ms tick: `check_ambient_download_complete()` and `check_ambient_loop()`. The UI adds a single-line ambient status panel above the player bar and a modal popup browser overlay for track selection.

**Major components:**
1. **Player (player.rs)** — Owns one OutputStream, two Sinks (main + ambient). Exposes ambient audio methods (load_ambient, replay_ambient, set_ambient_volume, toggle_ambient_pause). No UI knowledge.
2. **App (app.rs)** — Orchestrates ambient state, download channels (separate mpsc for ambient), loop detection, browser view state machine. Extends existing event loop without disrupting main playback flow.
3. **PlexAPI (plex.rs)** — Adds library section listing and track browsing endpoints. Uses existing `Track` struct (same JSON format whether from playlist or library). Search is optional Phase 2 enhancement.
4. **UI (ui.rs)** — Renders ambient status panel (1 line, only when ambient loaded) and browser popup overlay (70% x 60% centered). Follows existing ratatui popup pattern.
5. **Session (config.rs)** — Extends Session struct with `ambient_part_key`, `ambient_volume`, `ambient_enabled` (all `#[serde(default)]` for backward compatibility).

**Key architectural pattern:** Non-disruptive background operations. Ambient downloads and looping happen without changing AppView. Main track downloads transition to AppView::Downloading. Ambient does not. This prevents the ambient layer from interrupting the user's navigation flow.

### Critical Pitfalls

1. **Mixer clipping (HIGH severity)** — When main volume + ambient volume > 1.0, rodio's mixer sums samples beyond the f32 range, causing harsh crackling at every peak. Users will blame WSL2 audio quality. Prevention: enforce a volume budget (main + ambient <= 1.0). Either use a master gain factor, or cap channels individually (e.g., main max 0.7, ambient max 0.3). Apply at Sink level before mixing.

2. **`repeat_infinite()` memory leak (HIGH severity)** — Rodio issue #673 confirmed: memory grows ~10MB per 15 seconds due to buffered source cloning bug. For ambient that loops for hours, this will consume gigabytes. Prevention: manual loop detection (event loop checks `sink.empty()`, re-appends from cached bytes). Matches existing `replay_current()` pattern. 100ms max gap is imperceptible.

3. **OutputStream lifetime (CRITICAL severity)** — Dropping the OutputStream kills ALL audio on ALL sinks instantly and silently. The existing code correctly keeps `_stream` alive. Refactoring for two sinks must preserve this: both main_sink and ambient_sink MUST share the single OutputStream. Never create a second OutputStream (would open two audio devices, fails on WSL2 PulseAudio).

4. **Breaking existing playback during refactor (HIGH severity)** — Every Player method currently references `self.sink`. Adding ambient requires renaming to `self.main_sink` + adding `self.ambient_sink`. Missing even one reference breaks main playback (e.g., spacebar pauses ambient instead of main, `is_finished()` checks wrong sink, auto-advance triggers on ambient loop). Prevention: rename sink -> main_sink in a single reviewable commit before adding ambient logic.

5. **WSL2 pause/resume desynchronization (MEDIUM severity)** — Existing single-sink code documents WSL2 pause/resume failures after extended pauses (>5 seconds). With two sinks, PulseAudio stream resume may succeed for one but fail for the other, causing one channel to go silent. Prevention: pause/resume both sinks in the same call, add verification delay (100-200ms), check both active, recreate if needed from cached bytes.

## Implications for Roadmap

Based on research, suggested phase structure:

### Phase 1: Dual-Sink Audio Foundation
**Rationale:** Validate that the core architectural assumption (two sinks on one OutputStream) works on WSL2 before building any features. If WSL2 audio quality degrades with two concurrent sinks, or if mixer clipping is unmanageable, the entire feature is DOA. Fail fast with minimal code investment.

**Delivers:** Player struct refactored with `main_sink` + `ambient_sink: Option<Sink>`, both from `stream.mixer()`. Hardcoded test audio files for both channels. Volume budget enforced (main + ambient <= 1.0). Manual loop detection implemented and tested for memory stability. All existing playback functionality still works (regression suite passes).

**Addresses:**
- Second audio channel (table stakes)
- Independent volume controls (table stakes)
- Ambient looping mechanism (table stakes)

**Avoids:**
- Pitfall 1 (mixer clipping) via volume budget
- Pitfall 2 (OutputStream lifetime) via explicit shared stream
- Pitfall 3 (repeat_infinite leak) via manual re-append
- Pitfall 4 (breaking existing playback) via careful refactor
- Pitfall 9 (WSL2 audio degradation) via early testing

**Research needs:** None. This is pure implementation of documented rodio patterns. The critical unknowns (WSL2 dual-sink behavior, volume budget tuning) can only be resolved empirically.

### Phase 2: Track Selection & Download
**Rationale:** Once dual-sink audio is proven stable, build the user-facing feature: browse Plex for ambient tracks, download, play. This phase adds the most complexity (browser UI state machine, new Plex endpoints, download orchestration) but depends entirely on Phase 1's foundation.

**Delivers:** Plex library browsing API (`fetch_music_sections()`, `fetch_section_tracks()`). Browser UI as modal popup overlay (AppView::Browser + BrowserMode state). Ambient track selection triggers download via separate mpsc channel. `check_ambient_download_complete()` in event loop. Ambient track loads into ambient_sink and starts looping.

**Uses:**
- Existing PlexClient pattern for new endpoints
- Existing download-then-play pattern with separate channel
- Existing modal popup UI pattern from ratatui docs

**Implements:**
- Browser UI component (Architecture component 4)
- Plex library browsing (Architecture component 3 extension)
- Ambient download flow (Architecture component 2 extension)

**Addresses:**
- Ambient track selection from Plex (table stakes)
- Visual indication of ambient status (table stakes)
- Plex library browsing (competitive feature)

**Avoids:**
- Pitfall 8 (UI state conflicts) via separate browser ListState and explicit focus context
- Pitfall 10 (download blocking) via separate mpsc channels

**Research needs:** MEDIUM. Plex library endpoints are documented but pagination behavior for large libraries is unclear. Search endpoint integration may need validation. Suggest targeted research if the library has thousands of tracks.

### Phase 3: Controls, Keybindings, UX Polish
**Rationale:** The feature works but lacks discoverability and convenience. Add keybindings for ambient volume/mute, favorites integration, session persistence. This phase makes the feature actually usable for daily workflow.

**Delivers:** Keybindings (`a` = toggle ambient, `A` = open browser, `[/]` = volume, `0` = favorite ambient). Ambient status panel in UI (1 line above player bar). Session persistence extended with ambient fields. Ambient favorite playlists in config. Default ambient volume tuned (0.15-0.25). Volume step size adjusted (0.02 for fine control).

**Addresses:**
- Ambient mute/unmute toggle (table stakes)
- Session persistence (table stakes)
- Ambient favorite hotkeys (competitive)
- Ambient-only mode (competitive, emerges naturally)

**Avoids:**
- Pitfall 6 (ambient volume too loud) via conservative default and fine-grained steps
- Pitfall 11 (session persistence gap) via Session struct extension
- Pitfall 12 (visualizer confusion) via clear decision (main-only, documented)

**Research needs:** None. All standard TUI patterns and config serialization.

### Phase 4: Testing & Validation
**Rationale:** Dedicated testing phase to catch the moderate/minor pitfalls that won't surface during feature development but will bite in production. WSL2-specific issues (pause/resume, latency tuning) require extended soak testing.

**Delivers:** Smoke test suite (two-sink baseline, memory stability, pause/resume stress). Integration regression test (all v1.0 functionality unchanged). Performance validation (audio quality under CPU load, long-running sessions). Session backward compatibility test (v1.0 session.toml loads cleanly).

**Avoids:**
- Pitfall 5 (WSL2 pause/resume desync) via stress testing and verification logic
- Pitfall 7 (ambient sink accidentally stopped) via integration tests
- Pitfall 9 (WSL2 audio degradation) via extended listening tests

**Research needs:** None. Pure validation and debugging.

### Phase Ordering Rationale

- **Phase 1 before 2:** Architectural foundation must be validated before building UI/UX on top. If dual-sink audio is broken on WSL2, no amount of browser polish will save it.
- **Phase 2 before 3:** Feature must exist before it can be made convenient. Browser selection is the core value; keybindings are accelerators.
- **Phase 3 before 4:** Polish must be complete before comprehensive testing. Testing half-finished UX wastes time on soon-to-change code.
- **Phase 4 last:** Dedicated testing phase catches integration issues and edge cases after all features are implemented.

The research highlights a clear dependency chain: audio stability -> data access -> user interface -> quality assurance. Each phase has a clear success criterion that gates the next phase.

### Research Flags

Phases likely needing deeper research during planning:
- **Phase 2:** Plex library browsing pagination behavior for large libraries (thousands of tracks) is not well-documented. The `/library/sections/{id}/all` endpoint supports `X-Plex-Container-Start` and `X-Plex-Container-Size` headers, but optimal pagination size and handling are unclear. Suggest targeted research if the user's ambient library is large.

Phases with standard patterns (skip research-phase):
- **Phase 1:** Well-documented rodio patterns. All pitfalls are known from GitHub issues and documentation.
- **Phase 3:** Standard ratatui keybinding and serde config patterns. No unknowns.
- **Phase 4:** Standard testing methodology. WSL2 quirks are project-specific, not research-addressable.

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| Stack | HIGH | Zero new dependencies. All capabilities verified in rodio 0.21 docs and source. Existing codebase proves patterns work. |
| Features | HIGH | Table stakes features derived from competitive analysis (Moodist, myNoise, Noisli, Coffitivity). User workflow is clear: select ambient, set volume, resume across sessions. |
| Architecture | HIGH | Direct codebase analysis. All 8 source files reviewed. Extension points identified. State machine well-understood. Modal popup pattern documented in ratatui official examples. |
| Pitfalls | MEDIUM-HIGH | Critical pitfalls confirmed via rodio GitHub issues (#673, #340, #171, #330). WSL2-specific issues extrapolated from existing single-sink problems but not empirically validated for dual-sink. |

**Overall confidence:** HIGH

The only MEDIUM confidence area is WSL2 dual-sink behavior, which cannot be resolved through research — it requires empirical testing in Phase 1. All other aspects are backed by official documentation, existing working code, or confirmed GitHub issues.

### Gaps to Address

- **WSL2 dual-sink audio quality:** The existing code proves single-sink works with PULSE_LATENCY_MSEC=150 and .asoundrc tuning. Whether two sinks degrade quality is unknown until tested. Mitigation: Phase 1 smoke test validates this immediately. If quality is unacceptable, fallback option is to pre-mix ambient into main at the Source level (single sink with custom Source combiner) instead of dual-sink architecture. This is architecturally simpler but loses independent pause/mute.

- **Plex library pagination threshold:** Unknown at what library size pagination becomes necessary. Some users may have 50 ambient tracks (no pagination needed), others may have 5,000 (pagination critical for UX). Mitigation: start without pagination, add if user reports slow browsing or truncated lists. The API supports it, so it is not a blocker.

- **Ambient volume defaults per content type:** Different ambient content has wildly different loudness profiles. Rain sounds at -6dB RMS, white noise at 0dB RMS, nature sounds at -12dB RMS. A single default volume (0.20) may be too loud for some, too quiet for others. Mitigation: conservative default (0.15), fine-grained step size (0.02), prominent UI indicator so users know to adjust. Session persistence remembers their preferred volume per ambient track over time.

## Sources

### Primary (HIGH confidence)
- [rodio official documentation](https://docs.rs/rodio/latest/rodio/) — Sink creation, mixer, volume control, Source trait methods
- [rodio Sink docs](https://docs.rs/rodio/latest/rodio/struct.Sink.html) — connect_new(), set_volume(), pause(), play(), stop(), empty()
- [rodio Source trait](https://docs.rs/rodio/latest/rodio/source/trait.Source.html) — repeat_infinite(), fade_in(), mix(), amplify()
- [rodio GitHub issue #673](https://github.com/RustAudio/rodio/issues/673) — repeat_infinite memory leak, confirmed, unfixed in 0.21
- [rodio GitHub issue #340](https://github.com/RustAudio/rodio/issues/340) — mixer clipping when summed sources exceed 1.0
- [rodio GitHub issue #171](https://github.com/RustAudio/rodio/issues/171) — stopped Sink cannot accept new sources, must recreate
- [rodio GitHub issue #330](https://github.com/RustAudio/rodio/issues/330) — OutputStream drop kills all audio silently
- [ratatui popup example](https://ratatui.rs/examples/apps/popup/) — official modal overlay pattern
- TermTunes v1.0 codebase — all 8 source files (3,507 lines), direct analysis of existing patterns

### Secondary (MEDIUM confidence)
- [Plexopedia music API](https://www.plexopedia.com/plex-media-server/api/library/music/) — /library/sections/{id}/all endpoint, type=8/9/10 IDs
- [Plex API search hub](https://plexapi.dev/api-reference/search/perform-a-search) — /hubs/search endpoint with sectionId parameter
- [Python PlexAPI source](https://github.com/pkkid/python-plexapi/blob/master/plexapi/audio.py) — TYPE constant values confirmation
- [Plex URL commands](https://support.plex.tv/articles/201638786-plex-media-server-url-commands/) — general endpoint patterns

### Tertiary (LOW confidence)
- [WSLg GitHub issue #908](https://github.com/microsoft/wslg/issues/908) — choppy audio reports (single-sink context)
- [WSLg GitHub issue #1257](https://github.com/microsoft/wslg/issues/1257) — sound stuttering (PulseAudio bridge issues)
- [PulseAudio troubleshooting](https://wiki.archlinux.org/title/PulseAudio/Troubleshooting) — buffer underrun mitigation strategies

---
*Research completed: 2026-02-10*
*Ready for roadmap: yes*
