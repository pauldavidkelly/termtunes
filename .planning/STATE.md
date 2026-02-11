# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-02-10)

**Core value:** Keep music playback inside the terminal workflow - no context switching to external apps, everything stays in Tmux.
**Current focus:** Phase 7 complete -- ready for Phase 8

## Current Position

Phase: 7 of 9 (Ambient Track Selection) -- COMPLETE
Plan: 2 of 2 in current phase (all plans complete)
Status: Phase complete
Last activity: 2026-02-11 -- Completed 07-02-PLAN.md (browser overlay rendering + ambient volume balance)

Progress: [##############..........] 14/17 plans (v1.0: 10/10, v1.1: 4/7)

## Performance Metrics

**Velocity:**
- Total plans completed: 14
- Average duration: ~8 min
- Total execution time: ~1.98 hours

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

**Recent Trend:**
- Last 5 plans: 06-01 (~5 min), 06-02 (extended - 8 fix iterations), 07-01 (~3 min), 07-02 (~8 min, 2 fix iterations)

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
- **NEW:** rodio Sink::set_volume() unreliable for ambient sinks -- must recreate entire sink on volume change (stop old, create new at target volume, re-decode cached bytes)
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

### Pending Todos

None yet.

### Blockers/Concerns

- None -- WSL2 dual-sink audio quality validated (Phase 6 fail-fast gate passed)

## Session Continuity

Last session: 2026-02-11
Stopped at: Completed 07-02-PLAN.md (browser overlay rendering + ambient volume balance)
Next: Phase 8 plans (ambient controls, volume UI, status display)
