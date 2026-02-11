# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-02-10)

**Core value:** Keep music playback inside the terminal workflow - no context switching to external apps, everything stays in Tmux.
**Current focus:** Phase 7 - Ambient Track Selection

## Current Position

Phase: 7 of 9 (Ambient Track Selection)
Plan: 0 of ? in current phase
Status: Ready for planning
Last activity: 2026-02-11 -- Completed Phase 6 (dual-sink audio engine validated on WSL2)

Progress: [############............] 12/17 plans (v1.0: 10/10, v1.1: 2/7)

## Performance Metrics

**Velocity:**
- Total plans completed: 12
- Average duration: ~9 min (06-02 was extended due to iterative debugging)
- Total execution time: ~1.8 hours

**By Phase:**

| Phase | Plans | Total | Avg/Plan |
|-------|-------|-------|----------|
| 01-foundation-audio-poc | 3/3 | ~58 min | ~19 min |
| 02-core-tui-playback | 2/2 | ~14 min | ~7 min |
| 03-differentiators | 2/2 | ~11 min | ~5.5 min |
| 04-tmux-integration-polish | 2/2 | ~4 min | ~2 min |
| 05-audio-visualizer | 1/1 | ~5 min | ~5 min |
| 06-dual-sink-audio-engine | 2/2 | ~extended | ~variable |

**Recent Trend:**
- Last 5 plans: 04-02 (~2 min), 05-01 (~5 min), 06-01 (~5 min), 06-02 (extended - 8 fix iterations)
- Note: 06-02 required extensive iterative debugging of rodio volume behavior and volume architecture redesign

*Updated after each plan completion*

## Accumulated Context

### Decisions

All v1.0 decisions logged in PROJECT.md Key Decisions table.

Key v1.1 decisions:
- Regular Sink (not SpatialSink) for dual-channel -- SpatialSink is for 3D positional audio
- Volume management moved from Player to App (budget enforcement initially, then independent channels)
- Default ambient_volume: 0.7, master_volume: 1.0
- rodio `repeat_infinite()` has confirmed memory leak -- use manual re-append loop
- Single OutputStream shared by both sinks (never create second OutputStream)
- **REVISED:** Volume budget REPLACED with independent channels -- proportional budget caused UX issues (volume capped at 59%, ambient audible at 0% main, +/- barely affected ambient)
- **NEW:** rodio Sink::set_volume() unreliable for ambient sinks -- must recreate entire sink on volume change (stop old, create new at target volume, re-decode cached bytes)
- **NEW:** Background thread + mpsc channel required for ambient downloads (reqwest::blocking nests tokio runtime)
- **NEW:** UI must show saved_volume (user intent), not player.volume() (sink value)
- **NEW:** Logging defaults to info level when RUST_LOG not set (EnvFilter fallback)
- **VALIDATED:** WSL2 dual-channel audio works cleanly -- fail-fast gate passed

### Pending Todos

None yet.

### Blockers/Concerns

- None -- WSL2 dual-sink audio quality validated (Phase 6 fail-fast gate passed)

## Session Continuity

Last session: 2026-02-11
Stopped at: Completed 06-02-PLAN.md (ambient loop, validation, and volume architecture redesign)
Next: Phase 7 planning (ambient track selection)
