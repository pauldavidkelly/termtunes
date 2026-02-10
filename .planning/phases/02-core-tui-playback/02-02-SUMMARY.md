---
phase: 02-core-tui-playback
plan: 02
subsystem: ui
tags: [ratatui, linegauge, player-bar, tui-layout, colored-spans]

# Dependency graph
requires:
  - phase: 02-core-tui-playback
    plan: 01
    provides: "Player volume/position APIs, NowPlaying metadata struct, track navigation methods, auto-advance logic"
provides:
  - "Multi-panel TUI layout with conditional 3-line player bar"
  - "Track info display: state icon + track name + artist + album with colored spans"
  - "LineGauge progress bar with clamped ratio (panic-safe)"
  - "Status line: playback state + volume percentage + elapsed/total time"
  - "Playing track indicator (>>) in track list with green+bold highlight"
  - "Updated keybinding help text for all Phase 2 controls"
  - "Error message display integrated into player bar (line 3, red)"
affects: [03-search-queue, ui-rendering, future-ui-enhancements]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Conditional layout: 3-line player bar when track playing, 1-line status bar when idle"
    - "Multi-colored line rendering: Line::from(vec![Span::styled(...)]) for mixed-color text"
    - "LineGauge ratio clamping: always clamp to 0.0..=1.0 before passing to widget"
    - "Error overlay: error messages replace status line (line 3) in player bar"

key-files:
  created: []
  modified:
    - src/ui.rs
    - src/app.rs

key-decisions:
  - "State icons: >> (green, playing), || (yellow, paused), -- (gray, stopped) for instant visual feedback"
  - "LineGauge ratio clamped to prevent panic when get_pos() briefly exceeds duration"
  - "Error messages shown in player bar line 3 (red) instead of replacing entire bar"
  - "current_track_index() accessor added to app.rs to support track highlighting in UI"

patterns-established:
  - "Conditional player bar: render 3-line bar when playing, 1-line status bar when idle"
  - "Colored span composition: build multi-colored lines with Vec<Span> for rich TUI text"
  - "Safe LineGauge usage: always clamp ratio and handle zero duration"

# Metrics
duration: 11min
completed: 2026-02-10
---

# Phase 2 Plan 2: Player Bar UI Summary

**Multi-panel TUI with 3-line player bar showing colored track info, LineGauge progress bar, and volume/time status with playing track indicator in track list**

## Performance

- **Duration:** 11 min (including checkpoint verification)
- **Started:** 2026-02-10T16:08:00Z
- **Completed:** 2026-02-10T16:19:15Z
- **Tasks:** 2 (1 auto + 1 checkpoint)
- **Files modified:** 2

## Accomplishments
- Complete rewrite of ui.rs with conditional multi-panel layout (3-line player bar vs 1-line status bar)
- Rich player bar: state icons (>>/||/--), colored track metadata (white/cyan/yellow), LineGauge progress with clamped ratio, volume percentage, elapsed/total time
- Track list enhanced with ">>" playing indicator in green+bold for the currently playing track
- Error messages integrated into player bar line 3 without losing track info and progress
- Human-verified end-to-end: all 12 verification steps passed (browse/select/play/pause/volume/next/prev/auto-advance/back/quit)

## Task Commits

Each task was committed atomically:

1. **Task 1: Rewrite ui.rs with multi-panel layout and 3-line player bar** - `6036e51` (feat)
2. **Task 2: Verify complete Phase 2 TUI and playback controls** - checkpoint:human-verify (approved, no commit needed)

## Files Created/Modified
- `src/ui.rs` - Complete rewrite: multi-panel layout with render_player_bar (3-line: track info, LineGauge progress, status), render_playlists, render_tracks (with playing indicator), render_downloading, render_status_bar (updated help text), format_duration helper
- `src/app.rs` - Added `current_track_index()` accessor for UI track highlighting

## Decisions Made
- State icons chosen for instant visual recognition: >> (green, playing), || (yellow, paused), -- (gray, stopped)
- LineGauge ratio always clamped to 0.0..=1.0 to prevent panics when rodio get_pos() briefly exceeds track duration
- Error messages shown in player bar line 3 (red) rather than replacing entire player bar, keeping track info and progress visible during transient errors
- Added current_track_index() accessor to app.rs -- minor public API addition to support track list highlighting

## Deviations from Plan

None - plan executed exactly as written. The current_track_index() accessor addition was explicitly mentioned in the plan as the preferred approach.

## Issues Encountered

None - ui.rs compiled on first attempt with no clippy warnings.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness
- All Phase 2 DISP-01 through DISP-07 requirements satisfied (track name, artist, album, progress bar, elapsed/total time, playback state, volume)
- All Phase 2 keybindings functional (j/k navigate, Enter select, Space pause, n/N next/prev, +/- volume, q quit, Esc back)
- Auto-advance verified working end-to-end
- Volume persistence across track changes verified
- TUI foundation ready for Phase 3 (search, queue, additional views)

## Self-Check: PASSED

- FOUND: src/ui.rs
- FOUND: src/app.rs
- FOUND: 02-02-SUMMARY.md
- FOUND: 6036e51 (Task 1 commit)

---
*Phase: 02-core-tui-playback*
*Plan: 02*
*Completed: 2026-02-10*
