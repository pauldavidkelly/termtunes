---
phase: 04-tmux-integration-polish
plan: 01
subsystem: ui
tags: [ratatui, tmux, adaptive-layout, resize, truncation]

# Dependency graph
requires:
  - phase: 02-core-tui-playback
    provides: "Base UI rendering (ui.rs) and event loop (app.rs)"
  - phase: 03-differentiators
    provides: "Shuffle/repeat indicators and favorite key UI elements"
provides:
  - "Adaptive narrow-mode layout for tmux panes under 40 columns"
  - "Minimum size guard for terminals under 20x5"
  - "Character-safe text truncation helper"
  - "Explicit Event::Resize handling in event loop"
affects: [04-02-PLAN]

# Tech tracking
tech-stack:
  added: []
  patterns: ["is_narrow flag for conditional layout rendering", "truncate_for_display() char-safe truncation", "Constants for layout thresholds (MIN_WIDTH, MIN_HEIGHT, NARROW_WIDTH)"]

key-files:
  created: []
  modified: [src/ui.rs, src/app.rs]

key-decisions:
  - "Width thresholds: MIN_WIDTH=20, MIN_HEIGHT=5, NARROW_WIDTH=40 as named constants"
  - "Playlist truncation drops track count suffix first before truncating title"
  - "Narrow player bar shows only state icon + track name (line 1) and state + time (line 3)"
  - "truncate_for_display uses .chars().take() not String::truncate() for UTF-8 safety"

patterns-established:
  - "is_narrow flag: computed once in render() and passed to sub-renderers for layout decisions"
  - "Width parameter: sub-renderers receive terminal width for truncation calculations"
  - "Graceful degradation: minimum size -> narrow mode -> full mode (progressive enhancement)"

# Metrics
duration: 2min
completed: 2026-02-10
---

# Phase 4 Plan 1: Adaptive Layout Summary

**Adaptive TUI layout with narrow-mode rendering for tmux panes, minimum-size guard, and explicit resize handling**

## Performance

- **Duration:** 2 min
- **Started:** 2026-02-10T17:59:09Z
- **Completed:** 2026-02-10T18:01:38Z
- **Tasks:** 1
- **Files modified:** 2

## Accomplishments
- Terminals under 20 columns or 5 rows show a centered "Terminal too small" message in red
- Terminals 20-39 columns wide use simplified narrow layout (abbreviated player bar, truncated names, short help text)
- All track names, playlist names, and player bar text are truncated with ellipsis using character-safe method
- Event loop explicitly matches Event::Resize (ratatui auto-handles buffer resize on next draw)

## Task Commits

Each task was committed atomically:

1. **Task 1: Add adaptive layout to ui.rs and explicit resize handling to app.rs** - `c8acfa2` (feat)

## Files Created/Modified
- `src/ui.rs` - Added MIN_WIDTH/MIN_HEIGHT/NARROW_WIDTH constants, minimum size guard, is_narrow flag, truncate_for_display() helper, width-aware sub-renderer signatures, narrow-mode layout branches in player bar and status bar
- `src/app.rs` - Changed event polling from if-let to match with explicit Event::Resize arm

## Decisions Made
- Width thresholds defined as named constants (MIN_WIDTH=20, MIN_HEIGHT=5, NARROW_WIDTH=40) for clarity and easy tuning
- Playlist name truncation strategy: drop track count suffix first, then truncate title with ellipsis -- preserves the most useful information
- Narrow player bar line 1 shows only state icon + track name (artist/album omitted) -- track name is most important for identification
- Narrow player bar line 3 shows only state + time (volume/shuffle/repeat dropped) -- essential info for narrow panes
- Status bar in narrow mode shows abbreviated keys ("q:quit j/k:nav Enter:sel Space:pause") -- only the most critical keybindings

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Adaptive layout complete, ready for 04-02 (error handling & reconnection polish)
- UI renders correctly at any terminal width from 20+ columns
- No new warnings introduced (all clippy warnings are pre-existing)

## Self-Check: PASSED

- FOUND: src/ui.rs
- FOUND: src/app.rs
- FOUND: 04-01-SUMMARY.md
- FOUND: commit c8acfa2

---
*Phase: 04-tmux-integration-polish*
*Completed: 2026-02-10*
