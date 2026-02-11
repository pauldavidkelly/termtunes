# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-02-10)

**Core value:** Keep music playback inside the terminal workflow - no context switching to external apps, everything stays in Tmux.
**Current focus:** Phase 8 complete -- ready for Phase 9

## Current Position

Phase: 8 of 9 (Ambient Status UI Controls) -- COMPLETE
Plan: 1 of 1 in current phase (all plans complete)
Status: Phase complete
Last activity: 2026-02-11 -- Completed 08-01-PLAN.md (ambient status panel and controls)

Progress: [###############.........] 15/17 plans (v1.0: 10/10, v1.1: 5/7)

## Performance Metrics

**Velocity:**
- Total plans completed: 15
- Average duration: ~8.5 min
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

**Recent Trend:**
- Last 5 plans: 06-02 (extended - 8 fix iterations), 07-01 (~3 min), 07-02 (~8 min, 2 fix iterations), 08-01 (~37 min, 1 fix iteration + user verify pause)

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

### Pending Todos

None yet.

### Blockers/Concerns

- None -- WSL2 dual-sink audio quality validated (Phase 6 fail-fast gate passed)

## Session Continuity

Last session: 2026-02-11
Stopped at: Completed 08-01-PLAN.md (ambient status panel and controls)
Next: Phase 9 plans (session persistence)
