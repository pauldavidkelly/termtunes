---
phase: 03-differentiators
plan: 02
subsystem: ui
tags: [favorites, hotkeys, config-persistence, shuffle-indicator, repeat-indicator, toml, serde, ratatui]

# Dependency graph
requires:
  - phase: 03-01
    provides: "Shuffle state machine, RepeatMode enum with indicator(), seek mechanics, public accessors"
  - phase: 02-core-tui-playback
    provides: "Player bar UI, playlist/track views, config save_config"
provides:
  - "FavoritePlaylist struct with serde derives for config persistence"
  - "Favorites HashMap<String, FavoritePlaylist> in Config with serde(default)"
  - "assign_favorite (f + 1-9) and start_favorite (1-9) methods"
  - "awaiting_favorite_key modal state for two-key favorite assignment"
  - "[Shuffle] magenta and [Repeat: All/One] blue indicators on player bar line 3"
  - "Favorite [N] prefix indicators on playlist list items"
  - "Updated help text with all Phase 3 keybindings"
affects: [04-polish, ui, config]

# Tech tracking
tech-stack:
  added: []
  patterns: [two-key-modal-input, config-persistence-favorites, incremental-span-building]

key-files:
  created: []
  modified: [src/config.rs, src/app.rs, src/ui.rs, src/player.rs]

key-decisions:
  - "Favorites keyed by string '1'-'9' in config HashMap for TOML serialization"
  - "Two-key modal: press f enters awaiting_favorite_key state, then 1-9 assigns"
  - "Favorite activation (1-9) works from any view, assignment (f) only from Playlists view"
  - "Shuffle indicator in magenta, repeat indicator in blue for visual distinction"

patterns-established:
  - "Two-key modal input: bool flag gates second keypress, Esc cancels"
  - "Config persistence: HashMap in Config struct with serde(default) for backward compat"
  - "Incremental span building: construct Vec<Span> then push conditional indicators"

# Metrics
duration: 8min
completed: 2026-02-10
---

# Phase 3 Plan 2: Favorite Hotkeys and UI Indicators Summary

**Favorite playlist hotkeys (f+1-9 assign, 1-9 activate) with TOML persistence, plus [Shuffle]/[Repeat] player bar indicators**

## Performance

- **Duration:** 8 min (including human verification)
- **Started:** 2026-02-10T17:27:00Z
- **Completed:** 2026-02-10T17:35:15Z
- **Tasks:** 2 (1 auto + 1 human-verify)
- **Files modified:** 4

## Accomplishments
- FavoritePlaylist struct with serde derives added to config.rs, persisted in config.toml HashMap
- Two-key favorite assignment flow: press 'f' in playlist view, then 1-9 to assign; press 1-9 from anywhere to instantly load and play a favorite playlist
- [Shuffle] indicator (magenta) and [Repeat: All/One] indicator (blue) displayed on player bar line 3
- Playlist list shows [N] prefix for favorited playlists
- Help text updated with all Phase 3 keybindings (s/r/h/l/f/1-9)
- Backward seek fixed by providing byte_len to Decoder builder (bugfix deviation)
- All 8 Phase 3 requirements verified end-to-end by user

## Task Commits

Each task was committed atomically:

1. **Task 1: Add FavoritePlaylist to config, implement favorite assignment/activation in App, and update UI with indicators** - `119b695` (feat)
1.5. **Bugfix: Enable backward seeking by providing byte_len to Decoder builder** - `9a22ca6` (fix)
2. **Task 2: Verify all Phase 3 features end-to-end** - human-verify checkpoint, approved by user

## Files Created/Modified
- `src/config.rs` - Added FavoritePlaylist struct with Serialize/Deserialize, favorites HashMap in Config
- `src/app.rs` - Added awaiting_favorite_key state, assign_favorite/start_favorite methods, f/1-9 keybindings, favorites accessor, shuffle regeneration on playlist switch
- `src/ui.rs` - Added [Shuffle]/[Repeat] indicators on player bar, [N] favorite prefixes on playlist list, awaiting_favorite_key prompt, updated help text
- `src/player.rs` - Fixed seek_backward by passing byte_len to Decoder::new_with_options

## Decisions Made
- **Favorites keyed by string '1'-'9':** HashMap<String, FavoritePlaylist> in config for clean TOML serialization. String keys rather than numeric to avoid TOML table-vs-array ambiguity.
- **Two-key modal for assignment:** Pressing 'f' sets awaiting_favorite_key flag, next 1-9 keypress assigns. Esc cancels. This avoids needing modifier keys.
- **Assignment only from Playlists view:** The 'f' key only enters assignment mode when viewing the playlist list (you need to see what you're assigning). Activation via 1-9 works from any view.
- **Distinct indicator colors:** Shuffle in magenta, repeat in blue. Both can appear simultaneously on player bar line 3.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed backward seeking by providing byte_len to Decoder builder**
- **Found during:** Task 2 verification (seek backward test)
- **Issue:** Seeking backward with h/Left had no effect because the Symphonia Decoder was built without knowing the total byte length of the audio data, preventing it from seeking to byte offsets before the current position.
- **Fix:** Passed the cached audio data length as `byte_len` parameter to `Decoder::new_with_options`, enabling the decoder to seek to any position in the byte stream.
- **Files modified:** src/player.rs
- **Verification:** User verified h/Left arrow jumps backward ~5 seconds during playback
- **Committed in:** 9a22ca6

---

**Total deviations:** 1 auto-fixed (1 bug)
**Impact on plan:** Essential fix for seek backward functionality. No scope change.

## Issues Encountered
None beyond the seek backward bug documented above.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- All Phase 3 differentiators complete: shuffle, repeat, seek, favorites, UI indicators
- Config persistence pattern established for favorites -- can be extended in future phases
- Player bar indicator pattern ready for additional status items in Phase 4
- All 8 Phase 3 requirements (PLAY-08, PLAY-09, PLAY-10, PLAY-11, LIST-04, LIST-05, DISP-08, KEY-06) verified

---
*Phase: 03-differentiators*
*Completed: 2026-02-10*

## Self-Check: PASSED
- All 4 source files verified present on disk
- Both task commits (119b695, 9a22ca6) verified in git log
- Summary file verified present at .planning/phases/03-differentiators/03-02-SUMMARY.md
