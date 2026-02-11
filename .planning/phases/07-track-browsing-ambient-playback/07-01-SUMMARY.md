---
phase: 07-track-browsing-ambient-playback
plan: 01
subsystem: api, ui
tags: [plex, library-sections, browser, state-machine, ambient, ratatui]

# Dependency graph
requires:
  - phase: 06-dual-sink-audio-engine
    provides: "Dual-sink audio engine, ambient download pipeline, load_ambient_track()"
provides:
  - "LibrarySection struct and Plex library sections API (fetch_library_sections)"
  - "Section tracks API (fetch_section_tracks with type=10)"
  - "BrowserState enum (Closed, Sections, Tracks) with two-level navigation"
  - "Browser input routing that captures all keys when open"
  - "Section caching via cached_sections field"
  - "Browser track selection wired to ambient download pipeline"
affects: [07-02-PLAN (UI overlay rendering), phase-08 (ambient controls)]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Two-level browser state machine (BrowserState enum with associated data)"
    - "Input routing pattern: check modal state before main key handler"
    - "Section caching: Option<Vec<LibrarySection>> populated on first fetch"
    - "Extract-before-async pattern to avoid borrow conflicts in browser key handler"

key-files:
  created: []
  modified:
    - "src/plex.rs"
    - "src/app.rs"

key-decisions:
  - "Use 'b' key for browser open (replacing temporary 'a' from Phase 6)"
  - "Cache music library sections on App (Option<Vec<LibrarySection>>), refresh only on restart"
  - "Filter sections by type=='artist' to show music-only libraries"
  - "Browser captures ALL input when open (only Ctrl+C escapes for emergency quit)"

patterns-established:
  - "BrowserState enum with associated ListState for modal navigation"
  - "Input routing guard at top of handle_key() for modal overlays"

# Metrics
duration: 3min
completed: 2026-02-11
---

# Phase 7 Plan 1: Track Browsing & Ambient Playback Summary

**Plex library section API and two-level browser state machine with input capture, section caching, and ambient download pipeline integration**

## Performance

- **Duration:** ~3 min
- **Started:** 2026-02-11T09:53:47Z
- **Completed:** 2026-02-11T09:56:44Z
- **Tasks:** 2
- **Files modified:** 2

## Accomplishments
- Extended PlexClient with library sections API (fetch_library_sections, fetch_section_tracks)
- Built complete BrowserState state machine with Closed/Sections/Tracks levels
- Implemented browser input routing that captures all keys when browser is open
- Wired browser track selection to existing ambient download pipeline
- Added section caching to avoid redundant API calls
- Removed temporary Phase 6 'a' keybinding and start_ambient_from_selected() method

## Task Commits

Each task was committed atomically:

1. **Task 1: Extend PlexClient with library section and section tracks API methods** - `3c843fd` (feat)
2. **Task 2: Add BrowserState enum, browser input routing, and browser key handler** - `4aec337` (feat)

## Files Created/Modified
- `src/plex.rs` - Added LibrarySection, SectionsContainer, SectionsMediaContainer structs; fetch_library_sections() and fetch_section_tracks() methods
- `src/app.rs` - Added BrowserState enum, browser_state/cached_sections fields, browser accessors, input routing, handle_browser_key(), browser navigation methods (move/open/enter/select/back), removed start_ambient_from_selected()

## Decisions Made
- Used 'b' key for browser open (replacing temporary 'a' from Phase 6) -- 'b' for "browse" is more intuitive and frees 'a' for future use
- Cache music library sections on first fetch (Option<Vec<LibrarySection>>) -- sections rarely change during a session, avoids network round-trip on every browser open
- Filter sections by type=="artist" -- Plex library sections include movies, TV, photos; only music sections are relevant for ambient track selection
- Browser captures ALL input when open (except Ctrl+C) -- prevents accidental quit, pause, or main view navigation while browsing

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- BrowserState, browser_state() and browser_state_mut() accessors are ready for ui.rs rendering in Plan 02
- Plan 02 will add the visual popup overlay using ratatui Clear + Block + List pattern
- All browser state transitions and input handling are complete -- Plan 02 only needs to render

---
*Phase: 07-track-browsing-ambient-playback*
*Completed: 2026-02-11*
