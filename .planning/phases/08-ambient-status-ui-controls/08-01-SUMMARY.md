---
phase: 08-ambient-status-ui-controls
plan: 01
subsystem: ui
tags: [ratatui, keybindings, ambient-audio, volume-control, tui-layout]

# Dependency graph
requires:
  - phase: 06-dual-sink-audio-engine
    provides: "Dual-sink Player with ambient_sink, set_ambient_volume(), ambient_track_name()"
  - phase: 07-track-browsing-ambient-playback
    provides: "Browser overlay for selecting ambient tracks, ambient_volume field on App"
provides:
  - "1-line ambient status panel (track name, state icon, volume percentage)"
  - "Dedicated [/] keybindings for ambient volume up/down"
  - "Improved m toggle with pre-mute volume memory"
  - "4-way conditional layout (viz+ambient, viz-only, ambient-only, neither)"
affects: [09-session-persistence, future-ui-phases]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Conditional layout branching for optional UI panels"
    - "Pre-mute volume memory pattern for toggle keybindings"
    - "Direct Sink::set_volume() for ambient (not sink recreation)"

key-files:
  created: []
  modified:
    - src/app.rs
    - src/ui.rs
    - src/player.rs

key-decisions:
  - "Direct Sink::set_volume() for ambient volume changes instead of sink recreation -- recreation restarts playback"
  - "Pre-mute volume memory (pre_mute_ambient_volume field) for accurate unmute restore"
  - "Unified toggle_ambient() replaces separate mute_ambient()/unmute_ambient() methods"
  - "Ambient panel only appears when ambient_track_name().is_some() (not on startup)"

patterns-established:
  - "Conditional layout: 4-branch if/else for optional panel combinations"
  - "render_main_content() helper to reduce duplicated match statements across layout branches"
  - "render_ambient_panel() with narrow-mode adaptation (drops volume display)"

# Metrics
duration: 37min
completed: 2026-02-11
---

# Phase 8 Plan 1: Ambient Status UI Controls Summary

**1-line ambient status panel with [/] volume keybindings and m toggle using pre-mute volume memory, fixed to use direct Sink::set_volume() to preserve playback position**

## Performance

- **Duration:** ~37 min (including user verification pause)
- **Started:** 2026-02-11T13:11:00Z
- **Completed:** 2026-02-11T13:48:19Z
- **Tasks:** 3 (2 auto + 1 checkpoint with fix)
- **Files modified:** 3

## Accomplishments
- Ambient status panel shows track name, play/pause icon, and volume percentage in a 1-line row
- `[`/`]` keybindings adjust ambient volume by 5% per press without interrupting playback
- `m` toggle saves pre-mute volume and restores exact value on unmute (not hardcoded 0.3)
- Fixed ambient volume change to use direct Sink::set_volume() instead of sink recreation (which restarted playback)
- 4-way conditional layout handles all combinations of visualizer and ambient panel

## Task Commits

Each task was committed atomically:

1. **Task 1: Add ambient volume keybindings and pre-mute toggle to app.rs** - `acbe6ac` (feat)
2. **Task 2: Add ambient status panel rendering and conditional layout to ui.rs** - `4ae1fc4` (feat)
3. **Task 3 fix: Preserve ambient playback position on volume change** - `78409fb` (fix)

## Files Created/Modified
- `src/app.rs` - Added pre_mute_ambient_volume field, ambient_volume_up/down methods, toggle_ambient(), [/] keybindings
- `src/ui.rs` - Added render_ambient_panel() function, render_main_content() helper, 4-branch conditional layout, updated help text with [/]:amb vol and m:amb
- `src/player.rs` - Replaced set_ambient_volume() sink recreation with direct Sink::set_volume() call

## Decisions Made
- **Direct Sink::set_volume() over sink recreation:** The original set_ambient_volume() stopped the old sink, created a new one, and re-decoded audio from cached bytes on every volume change. This caused the ambient track to restart from the beginning on each [/] keypress. Testing confirmed that rodio's Sink::set_volume() works reliably for ambient sinks in practice, so the simpler direct approach was adopted. The STATE.md note about unreliability was based on earlier precautionary testing that no longer applies.
- **Unified toggle_ambient():** Replaced separate mute_ambient()/unmute_ambient() methods with a single toggle that uses pre_mute_ambient_volume to remember the volume before muting. Simpler API surface and enables accurate restore.
- **Panel visibility gated on ambient_track_name():** The ambient panel only appears after the user loads an ambient track via the browser, not on startup. This avoids showing an empty/confusing panel when no ambient is active.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed ambient track restart on volume change**
- **Found during:** Task 3 (human verification checkpoint)
- **Issue:** Pressing [/] to change ambient volume caused the track to restart from the beginning. The set_ambient_volume() method in player.rs recreated the entire sink (stop, create new, re-decode audio bytes) which always started playback from position 0.
- **Fix:** Replaced sink recreation with direct Sink::set_volume() call on the existing ambient sink. Volume change is applied immediately without interrupting playback position.
- **Files modified:** src/player.rs
- **Verification:** User tested -- [/] keypresses now change volume without restarting the track
- **Committed in:** 78409fb

---

**Total deviations:** 1 auto-fixed (1 bug fix)
**Impact on plan:** Essential fix for correct ambient volume behavior. The plan's implementation notes acknowledged the recreation approach might cause issues but deferred optimization. User testing caught the restart behavior and it was fixed immediately.

## Issues Encountered
- The plan explicitly noted that set_ambient_volume() uses sink recreation and that Sink::set_volume() was "unreliable." User verification revealed that the recreation approach was worse (restart on every keypress). Direct Sink::set_volume() was tested and works correctly, invalidating the earlier assumption.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Ambient UI controls complete -- all UI requirements (UI-01, UI-05, UI-06, UI-07) satisfied
- Phase 8 has only 1 plan, so phase is complete
- Ready for Phase 9 (session persistence) which will persist ambient_volume across sessions
- The STATE.md decision about "Sink::set_volume() unreliable for ambient" should be revised to reflect that direct set_volume() works correctly

---
*Phase: 08-ambient-status-ui-controls*
*Completed: 2026-02-11*
