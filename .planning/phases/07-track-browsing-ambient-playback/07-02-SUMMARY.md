---
phase: 07-track-browsing-ambient-playback
plan: 02
subsystem: ui
tags: [ratatui, popup, overlay, clear-widget, flex-center, browser, ambient, volume]

# Dependency graph
requires:
  - phase: 07-track-browsing-ambient-playback
    plan: 01
    provides: "BrowserState enum, browser_state()/browser_state_mut() accessors, browser input routing"
  - phase: 06-dual-sink-audio-engine
    provides: "Dual-sink audio engine, ambient download pipeline, load_ambient_track()"
provides:
  - "Centered popup overlay rendering with Clear widget and Flex::Center"
  - "render_browser_overlay() for Sections and Tracks browser levels"
  - "popup_area() helper for reusable centered popup layout"
  - "Updated help text with b:browse and m:mute keybindings"
  - "Correct includeMedia=1 API parameter for section track media details"
  - "Balanced ambient volume default (0.3) for background audio"
affects: [phase-08 (ambient controls, volume UI)]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Popup overlay: popup_area() + Clear widget + Block border + List with ListState"
    - "Flex::Center layout for auto-centering popup on terminal resize"
    - "Magenta highlight style to distinguish browser from main Cyan selection"
    - "Overlay rendered last in render() to appear on top of all content"

key-files:
  created: []
  modified:
    - "src/ui.rs"
    - "src/plex.rs"
    - "src/app.rs"

key-decisions:
  - "Ambient volume default lowered from 0.7 to 0.3 (user-validated balance)"
  - "Popup dimensions: 70% width, 80% height of terminal"
  - "includeMedia=1 required for Plex library section tracks endpoint"

patterns-established:
  - "popup_area(area, percent_x, percent_y) -> Rect for centered popups"
  - "Clear + Block + List pattern for modal overlays"

# Metrics
duration: ~8min (execution), ~158min (wall-clock with user verification)
completed: 2026-02-11
---

# Phase 7 Plan 2: Browser Overlay Rendering Summary

**Centered popup overlay with Clear widget and magenta-styled List for two-level browser, plus API media fix and ambient volume balance tuning**

## Performance

- **Duration:** ~8 min execution (wall-clock ~158 min including user verification cycles)
- **Started:** 2026-02-11T09:59:08Z
- **Completed:** 2026-02-11T12:38:22Z
- **Tasks:** 2 (1 auto + 1 human-verify with 2 fix iterations)
- **Files modified:** 3

## Accomplishments
- Rendered browser overlay as centered popup using ratatui Clear widget + Flex::Center layout
- Two-level browser rendering: Music Libraries (sections) and Select Ambient Track (tracks with artist)
- Fixed Plex API section tracks endpoint to include Media array for stream URL construction
- Tuned ambient volume default from 0.7 to 0.3 for proper background audio balance
- Updated help text with b:browse and m:mute keybindings

## Task Commits

Each task was committed atomically:

1. **Task 1: Add browser popup overlay rendering to ui.rs** - `aed35c9` (feat)
2. **Fix: Add includeMedia=1 to section tracks API** - `5cd9128` (fix)
3. **Fix: Lower default ambient volume from 0.7 to 0.3** - `95efea0` (fix)

## Files Created/Modified
- `src/ui.rs` - Added Flex/Clear/BrowserState imports, popup_area() helper, render_browser_overlay() with Sections and Tracks rendering, browser overlay call in render(), updated help text
- `src/plex.rs` - Added includeMedia=1 query parameter to fetch_section_tracks() to ensure Media array is included in response
- `src/app.rs` - Improved browser_select_track() diagnostic logging, lowered ambient_volume default from 0.7 to 0.3, updated unmute restore value

## Decisions Made
- Popup dimensions set to 70% width, 80% height -- large enough to show content, small enough to keep context visible
- Magenta border and highlight style for browser -- visually distinct from main Cyan selection, consistent across both browser levels
- Ambient volume default lowered from 0.7 to 0.3 -- user validated that 30% provides audible ambient presence without overpowering main music
- includeMedia=1 added to section tracks API -- Plex library endpoint does not include Media array by default (unlike playlist endpoint)

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Plex section tracks API returned empty Media arrays**
- **Found during:** Task 2 (human verification)
- **Issue:** fetch_section_tracks() endpoint /library/sections/{key}/all?type=10 returns tracks without Media array by default, causing browser_select_track() to silently fail when constructing stream URLs
- **Fix:** Added includeMedia=1 query parameter to fetch_section_tracks() in plex.rs; improved diagnostic logging in browser_select_track()
- **Files modified:** src/plex.rs, src/app.rs
- **Verification:** User confirmed ambient playback starts after track selection
- **Committed in:** 5cd9128

**2. [Rule 1 - Bug] Ambient volume overpowered main music at 0.7 default**
- **Found during:** Task 2 (human verification)
- **Issue:** Default ambient_volume of 0.7 was too loud relative to main music at 1.0, causing ambient to overpower rather than sit underneath
- **Fix:** Lowered ambient_volume default from 0.7 to 0.3 in initialization, doc comment, and unmute restore value
- **Files modified:** src/app.rs
- **Verification:** User confirmed 0.3 provides proper background audio balance
- **Committed in:** 95efea0

---

**Total deviations:** 2 auto-fixed (2 bugs)
**Impact on plan:** Both fixes essential for correct user experience. The API fix was a data availability issue not caught during Phase 7 Plan 1 (research noted field parity was "assumed, not independently verified"). The volume fix reflects real-world listening balance.

## Issues Encountered
- Plex API field parity between playlist tracks and library section tracks was not guaranteed -- the research correctly flagged this as low-confidence (Tertiary source), but the plan did not include a verification step for media data presence
- Volume balance requires subjective user judgment -- automated tests cannot validate "sounds right"

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Phase 7 is now complete: full ambient track browser with browsing, selection, and playback
- Ready for Phase 8 (ambient controls, volume UI, status display)
- Key patterns established: popup overlay rendering, browser state rendering, ambient volume at 0.3

## Self-Check: PASSED

All files, commits, and code patterns verified:
- 3/3 modified files exist
- 3/3 task commits found in git history
- 5/5 key code patterns present (popup_area, render_browser_overlay, b:browse, includeMedia, ambient_volume 0.3)

---
*Phase: 07-track-browsing-ambient-playback*
*Completed: 2026-02-11*
