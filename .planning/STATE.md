# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-02-11)

**Core value:** Keep music playback inside the terminal workflow - no context switching to external apps, everything stays in Tmux.
**Current focus:** Planning next milestone

## Current Position

**v1.1 milestone shipped:** 2026-02-11
**Phases completed:** 9 phases (v1.0: 1-5, v1.1: 6-9)
**Plans completed:** 16 total (v1.0: 10, v1.1: 6)
**Status:** Both milestones complete + quick tasks in progress
**Last activity:** 2026-02-11 -- Completed quick task 1 (hierarchical ambient browser)

Progress: v1.0 ✅ 10/10 plans | v1.1 ✅ 6/6 plans

## Performance Metrics

**Velocity:**
- Total plans completed: 17
- Average duration: ~8 min
- Total execution time: ~2.6 hours

**By Phase:**

| Phase | Plans | Total | Avg/Plan |
|-------|-------|-------|----------|
| 01-foundation-audio-poc | 3/3 | ~58 min | ~19 min |
| 02-core-tui-playback | 2/2 | ~14 min | ~7 min |
| 03-differentiators | 2/2 | ~11 min | ~5.5 min |
| 04-tmux-integration-polish | 2/2 | ~4 min | ~2 min |
| 05-audio-visualizer | 1/1 | ~5 min | ~5 min |
| 06-dual-sink-audio-engine | 2/2 | ~extended | ~variable |
| 07-track-browsing-ambient-playback | 2/2 | ~11 min | ~5.5 min |
| 08-ambient-status-ui-controls | 1/1 | ~37 min | ~37 min |
| 09-session-persistence | 1/1 | ~2 min | ~2 min |

**Recent Trend:**
- Last 5 plans: 07-01 (~3 min), 07-02 (~8 min, 2 fix iterations), 08-01 (~37 min, 1 fix iteration + user verify pause), 09-01 (~2 min, 0 fix iterations)

*Updated after each plan completion*

## Accumulated Context

### Decisions

All v1.0 decisions logged in PROJECT.md Key Decisions table.

Key v1.1 decisions:
- Regular Sink (not SpatialSink) for dual-channel -- SpatialSink is for 3D positional audio
- Volume management moved from Player to App (budget enforcement initially, then independent channels)
- **REVISED:** Default ambient_volume: 0.3 (was 0.7, too loud), master_volume: 1.0
- rodio `repeat_infinite()` has confirmed memory leak -- use manual re-append loop
- Single OutputStream shared by both sinks (never create second OutputStream)
- **REVISED:** Volume budget REPLACED with independent channels -- proportional budget caused UX issues (volume capped at 59%, ambient audible at 0% main, +/- barely affected ambient)
- **REVISED:** rodio Sink::set_volume() works correctly for ambient sinks -- earlier sink recreation approach caused track restart on every volume change; direct set_volume() preserves playback position (Phase 8 fix)
- **NEW:** Background thread + mpsc channel required for ambient downloads (reqwest::blocking nests tokio runtime)
- **NEW:** UI must show saved_volume (user intent), not player.volume() (sink value)
- **NEW:** Logging defaults to info level when RUST_LOG not set (EnvFilter fallback)
- **VALIDATED:** WSL2 dual-channel audio works cleanly -- fail-fast gate passed
- **NEW:** 'b' key opens ambient track browser (replacing temporary 'a' from Phase 6)
- **NEW:** Music library sections cached on App (Option<Vec<LibrarySection>>), refresh only on restart
- **NEW:** Browser captures ALL input when open (only Ctrl+C escapes for emergency quit)
- **NEW:** BrowserState enum with associated ListState for two-level modal navigation
- **NEW:** popup_area() + Clear widget pattern for centered popup overlays
- **NEW:** includeMedia=1 required for Plex library section tracks endpoint (media not included by default)
- **NEW:** Ambient volume 0.3 validated by user as correct background level (0.7 overpowered main)
- **NEW:** Pre-mute volume memory (pre_mute_ambient_volume) for accurate m toggle restore
- **NEW:** Unified toggle_ambient() replaces separate mute/unmute methods
- **NEW:** Ambient status panel gated on ambient_track_name().is_some() (not shown until track loaded)
- **NEW:** 4-branch conditional layout for viz+ambient, viz-only, ambient-only, neither combinations
- **NEW:** Ambient part_key stored at selection time for session persistence (not full URL -- tokens rotate)
- **NEW:** Pre-mute ambient volume preserved in session save (save intended volume, not muted 0.0)
- **NEW:** Player init guard in check_ambient_download_complete() for ambient-before-main edge case
- **NEW:** First-use ambient default: (main_volume - 0.30).max(0.0) via Option<f32> None distinction
- **NEW:** #[serde(default)] per-field for backward-compatible Session struct extension
- **NEW:** BrowserState expanded from 3 to 7 variants for hierarchical Playlists/Artists navigation
- **NEW:** Artist search captures all chars when query non-empty (j/k/q become search chars when typing)
- **NEW:** ambient_playlist/ambient_playlist_index fields for Play All sequential ambient cycling
- **NEW:** Back from ArtistTracks goes to Artists (not Albums) to avoid caching album state

### Pending Todos

None yet.

### Blockers/Concerns

- None -- WSL2 dual-sink audio quality validated (Phase 6 fail-fast gate passed)

## Session Continuity

Last session: 2026-02-11
Stopped at: Completed quick task 1 (hierarchical ambient browser)
Next: Quick task complete. Hierarchical browser functional with search + Play All.
