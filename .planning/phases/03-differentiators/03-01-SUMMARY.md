---
phase: 03-differentiators
plan: 01
subsystem: playback
tags: [shuffle, repeat, seek, rand, rodio, state-machine]

# Dependency graph
requires:
  - phase: 02-core-tui-playback
    provides: "Player struct with load_and_play, toggle_pause, volume; App with next_track/prev_track/play_track_at_index"
provides:
  - "RepeatMode enum (Off, All, One) with cycle() and indicator()"
  - "Shuffle state machine (toggle, regenerate, position tracking)"
  - "Player seek_forward/seek_backward via rodio try_seek"
  - "Player replay_current for Repeat One cached replay"
  - "advance_track() auto-advance respecting repeat mode"
  - "Keybindings: s (shuffle), r (repeat), h/l/Left/Right (seek)"
affects: [03-02-PLAN, ui, player-bar]

# Tech tracking
tech-stack:
  added: [rand 0.9]
  patterns: [shuffle-index-array, repeat-mode-enum, cached-replay, seek-step-constant]

key-files:
  created: []
  modified: [Cargo.toml, src/player.rs, src/app.rs]

key-decisions:
  - "Shuffle uses index array with current track at position 0 on toggle"
  - "Repeat One replays from cached _audio_data bytes (no re-download)"
  - "Seek keybindings (h/l/Left/Right) only active in Playing view to avoid conflicts"
  - "User skip (n/N) ignores RepeatMode::One -- only auto-advance replays"
  - "prev_track always wraps regardless of repeat mode (preserves existing UX)"

patterns-established:
  - "Shuffle index array: generate shuffled Vec<usize>, swap current to pos 0, track position separately"
  - "RepeatMode cycle: Off -> All -> One -> Off via enum method"
  - "advance_track vs next_track: auto-advance respects Repeat One, user skip does not"

# Metrics
duration: 3min
completed: 2026-02-10
---

# Phase 3 Plan 1: Playback State Mechanics Summary

**Shuffle mode with index array, repeat modes (Off/All/One) with cached replay, and 5-second seek via rodio try_seek**

## Performance

- **Duration:** 3 min
- **Started:** 2026-02-10T16:43:36Z
- **Completed:** 2026-02-10T16:47:06Z
- **Tasks:** 2
- **Files modified:** 3

## Accomplishments
- Player has seek_forward/seek_backward methods using rodio's try_seek with 5-second steps
- Player has replay_current for Repeat One mode that re-decodes from cached audio bytes without re-downloading
- App has full shuffle state machine: toggle generates shuffled index array, next/prev navigate through it, regenerate on wrap
- App has RepeatMode enum (Off -> All -> One -> Off) affecting both user-initiated and auto-advance navigation
- advance_track method handles all three modes: One replays, All wraps/reshuffles, Off stops at end
- Four new keybindings wired: s (shuffle toggle), r (repeat cycle), h/Left (seek back), l/Right (seek forward)

## Task Commits

Each task was committed atomically:

1. **Task 1: Add rand dependency and implement seek + replay in Player** - `5ec7891` (feat)
2. **Task 2: Implement shuffle state, RepeatMode, advance_track, and wire keybindings in App** - `e31176d` (feat)

## Files Created/Modified
- `Cargo.toml` - Added rand 0.9 dependency for shuffle randomization
- `src/player.rs` - Added SEEK_STEP constant, seek_forward(), seek_backward(), replay_current() methods
- `src/app.rs` - Added RepeatMode enum, shuffle state fields, toggle_shuffle/regenerate_shuffle_order, next/prev_track_index, rewrote next/prev_track, added advance_track, wired s/r/h/l keybindings

## Decisions Made
- **Shuffle index array with current at position 0:** When shuffle is toggled on, the current track is swapped to position 0 in the shuffled array so playback continues from the current track rather than jumping to a random one.
- **Repeat One only on auto-advance:** User-initiated skip (n/N keys) always goes to the next/previous track even in Repeat One mode. Only the auto-advance (track finished) triggers replay. This matches standard music player UX.
- **Seek keybindings guarded to Playing view:** h/l/Left/Right only trigger seek when in AppView::Playing to avoid conflicts with potential horizontal navigation in other views.
- **prev_track always wraps:** Previous track wraps around regardless of repeat mode, preserving the existing UX from Phase 2.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed non-exhaustive match in prev_track_index**
- **Found during:** Task 2 (prev_track_index implementation)
- **Issue:** `match self.current_track_index { Some(idx) if idx > 0 => ..., Some(0) => ..., None => ... }` was non-exhaustive because the compiler cannot verify the guard `idx > 0` covers all `Some` cases.
- **Fix:** Reordered match arms to `Some(0) | None => ... , Some(idx) => ...` which is exhaustive without guards.
- **Files modified:** src/app.rs
- **Verification:** cargo build succeeds, cargo clippy passes
- **Committed in:** e31176d (Task 2 commit)

---

**Total deviations:** 1 auto-fixed (1 bug)
**Impact on plan:** Minor syntax adjustment for Rust exhaustiveness checker. No scope change.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Shuffle, repeat, and seek mechanics are fully functional and ready for UI indicators in Plan 03-02
- Public accessors (shuffle_enabled(), repeat_mode(), RepeatMode::indicator()) are ready for UI consumption
- All state is properly managed across track changes, playlist switches, and auto-advance

---
*Phase: 03-differentiators*
*Completed: 2026-02-10*

## Self-Check: PASSED
- All 4 files verified present on disk
- Both task commits (5ec7891, e31176d) verified in git log
- cargo build succeeds with no errors
