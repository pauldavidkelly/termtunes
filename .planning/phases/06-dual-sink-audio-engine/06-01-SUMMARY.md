---
phase: 06-dual-sink-audio-engine
plan: 01
subsystem: audio
tags: [rodio, dual-sink, mixer, volume-budget, ambient-audio]

# Dependency graph
requires:
  - phase: 05-audio-visualizer
    provides: "VisualizerSource tap on main channel (unchanged by this plan)"
provides:
  - "Dual-sink Player with main_sink + ambient_sink on shared OutputStream"
  - "Ambient lifecycle methods (load, stop, replay, volume, status)"
  - "Volume budget enforcement (main + ambient <= 1.0 with proportional scaling)"
  - "Master volume multiplier applied after budget enforcement"
  - "Mute/unmute ambient helpers"
affects: [06-02-ambient-loop-and-validation, 07-ambient-track-selection, 08-ambient-status-ui-controls]

# Tech tracking
tech-stack:
  added: []
  patterns: ["Dual-sink on shared mixer", "Volume budget with proportional scaling", "App-owned volume state (not Player-owned)"]

key-files:
  created: []
  modified:
    - "src/player.rs"
    - "src/app.rs"

key-decisions:
  - "Renamed sink to main_sink before any new code (safe refactor pattern)"
  - "Volume management moved from Player to App for budget enforcement"
  - "Regular Sink used (not SpatialSink) per research correction"
  - "ambient_volume default 0.7, master_volume default 1.0"

patterns-established:
  - "Dual-sink architecture: main_sink always exists, ambient_sink is Option<Sink>"
  - "Volume budget enforcement: all volume changes flow through apply_volume_budget()"
  - "Ambient methods completely isolated from main playback methods"
  - "Fresh Sink creation pattern for ambient (same as existing main pattern)"

# Metrics
duration: 5min
completed: 2026-02-10
---

# Phase 6 Plan 1: Dual-Sink Audio Engine Summary

**Dual-sink Player refactor with ambient lifecycle methods and proportional volume budget enforcement via App-owned volume state**

## Performance

- **Duration:** 5 min
- **Started:** 2026-02-10T21:31:17Z
- **Completed:** 2026-02-10T21:36:46Z
- **Tasks:** 2
- **Files modified:** 2

## Accomplishments
- Refactored Player from single-sink to dual-sink architecture (main_sink + ambient_sink)
- Implemented full ambient lifecycle: load, stop, replay from cached bytes, volume control, status
- Added volume budget enforcement with proportional scaling when main + ambient > 1.0
- Rewired volume_up/volume_down to flow through budget enforcement instead of direct Player calls
- Wired budget into playback start (check_download_complete) and replay (advance_track)

## Task Commits

Each task was committed atomically:

1. **Task 1: Refactor Player from single-sink to dual-sink with ambient methods** - `4e42b74` (feat)
2. **Task 2: Add volume budget enforcement and ambient volume state to App** - `6f198ba` (feat)

## Files Created/Modified
- `src/player.rs` - Dual-sink Player with main_sink (renamed from sink) and ambient_sink (Option<Sink>) plus ambient lifecycle methods
- `src/app.rs` - Volume budget enforcement, ambient_volume/master_volume fields, budget-wired volume controls, mute/unmute helpers

## Decisions Made
- **sink -> main_sink rename first:** Done as a pure rename step before any new code, minimizing risk of regression. All existing methods verified to only operate on main_sink.
- **Regular Sink (not SpatialSink):** Research confirmed SpatialSink is for 3D positional audio. The user's intent (two independent sinks on shared mixer) maps to regular Sink. CONTEXT.md mentions SpatialSink but the correction was documented in research.
- **App-owned volume state:** Moved volume management from Player to App. Player sinks receive computed values after budget + master scaling. saved_volume remains as main channel raw volume. This was necessary because budget enforcement requires knowledge of both channel volumes, which App has but Player does not.
- **Default ambient_volume 0.7:** Per user decision (30% lower than default main volume of 1.0).

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Player has dual-sink architecture ready for ambient track loading (Plan 06-02)
- Volume budget enforcement is wired into all volume change paths
- Ambient loop detection methods (is_ambient_finished, has_ambient_data, replay_ambient) ready for event loop integration in Plan 06-02
- All pre-existing v1.0 playback behavior preserved (zero regressions -- same Sink lifecycle, volume controls work identically from user perspective)

## Self-Check: PASSED

All files exist, all commits verified.

---
*Phase: 06-dual-sink-audio-engine*
*Completed: 2026-02-10*
