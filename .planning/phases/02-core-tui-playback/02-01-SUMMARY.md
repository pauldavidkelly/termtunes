---
phase: 02-core-tui-playback
plan: 01
subsystem: playback
tags: [rodio, volume, keybindings, auto-advance, ratatui]

# Dependency graph
requires:
  - phase: 01-foundation-audio-poc
    provides: "Player struct with load_and_play, toggle_pause, is_finished; App struct with event loop, handle_key, start_track_download"
provides:
  - "Player volume API: volume(), volume_up(), volume_down(), set_volume()"
  - "Player position API: get_pos() for elapsed time tracking"
  - "NowPlaying struct with track_name, artist, album, duration_ms for UI display"
  - "Track navigation: next_track(), prev_track(), play_track_at_index()"
  - "Auto-advance to next track on completion"
  - "Volume persistence across track changes via saved_volume"
  - "Keybindings: n/> (next), N/< (prev), +/= (vol up), -/_ (vol down)"
affects: [02-02-player-bar-ui, ui-rendering, playback-controls]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Volume persistence: save volume in App, restore on new Sink creation"
    - "Track index management: shared play_track_at_index core method for next/prev/select"
    - "Download cancellation: drop old mpsc receiver before starting new download"
    - "NowPlaying metadata: populated atomically in check_download_complete"

key-files:
  created: []
  modified:
    - src/player.rs
    - src/app.rs

key-decisions:
  - "Volume capped at 1.0 (no amplification beyond normal) to prevent clipping"
  - "Volume step of 0.05 (5% per key press) for fine control"
  - "Track navigation wraps around (last->first, first->last)"
  - "saved_volume stored in App struct (not Player) since Sink is recreated per track"
  - "NowPlaying populated from Plex track metadata, not player internal state"

patterns-established:
  - "Play-at-index pattern: shared method for all track changes (next/prev/select)"
  - "Volume save-on-change: every volume_up/volume_down saves to self.saved_volume"
  - "Auto-advance check: runs every event loop iteration when in Playing view"

# Metrics
duration: 3min
completed: 2026-02-10
---

# Phase 2 Plan 1: Playback Controls and App State Summary

**Volume control, next/prev track navigation, auto-advance, and NowPlaying metadata via rodio Sink API with volume persistence across track changes**

## Performance

- **Duration:** 3 min
- **Started:** 2026-02-10T16:05:07Z
- **Completed:** 2026-02-10T16:07:59Z
- **Tasks:** 2
- **Files modified:** 2

## Accomplishments
- Extended Player with 5 new methods: volume(), volume_up(), volume_down(), get_pos(), set_volume()
- Added volume parameter to load_and_play() for persistence across track changes
- Built NowPlaying struct and track index management for UI data layer
- Wired all Phase 2 keybindings (n/N/+/-/>/<) with view guards
- Implemented auto-advance to next track when current track finishes
- Volume persistence: saved_volume restored on every new Sink creation

## Task Commits

Each task was committed atomically:

1. **Task 1: Add volume control and position tracking to Player** - `2ed9668` (feat)
2. **Task 2: Add NowPlaying state, track index management, auto-advance, and keybindings to App** - `e6f6a1c` (feat)

## Files Created/Modified
- `src/player.rs` - Added volume(), volume_up(), volume_down(), get_pos(), set_volume() methods; modified load_and_play() to accept and apply saved volume
- `src/app.rs` - Added NowPlaying struct, current_track_index, saved_volume fields; added play_track_at_index/next_track/prev_track methods; auto-advance logic in run(); volume save-on-change; extended handle_key with Phase 2 keybindings; clear playback state on go_back

## Decisions Made
- Volume capped at 1.0 to prevent audio clipping (linear scale is sufficient for Phase 2 MVP)
- Volume step size of 0.05 gives 20 steps from muted to max, fine enough for keyboard control
- Track navigation wraps around: next at end goes to first, prev at first goes to last
- saved_volume stored in App struct because each new rodio Sink starts at volume 1.0
- NowPlaying metadata cloned from Plex Track data (artist, album, duration) rather than querying player

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

None - all code compiled on first attempt after both tasks.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness
- Player now exposes volume and position APIs ready for UI consumption in Plan 02-02
- NowPlaying struct provides all metadata needed for the player bar (track name, artist, album, duration)
- All keybindings wired and functional, ready for visual feedback in player bar
- Dead code warnings for now_playing(), saved_volume(), get_pos(), set_volume() will resolve when ui.rs consumes them in Plan 02-02

## Self-Check: PASSED

- FOUND: src/player.rs
- FOUND: src/app.rs
- FOUND: 02-01-SUMMARY.md
- FOUND: 2ed9668 (Task 1 commit)
- FOUND: e6f6a1c (Task 2 commit)
- cargo build: Finished successfully

---
*Phase: 02-core-tui-playback*
*Plan: 01*
*Completed: 2026-02-10*
