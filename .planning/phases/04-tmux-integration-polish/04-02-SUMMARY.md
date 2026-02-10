---
phase: 04-tmux-integration-polish
plan: 02
subsystem: playback
tags: [tmux, session-persistence, now-playing, toml, serde]

# Dependency graph
requires:
  - phase: 01-foundation-audio-poc
    provides: "Config infrastructure (config.rs) with dirs, TOML, 0o600 permissions"
  - phase: 02-core-tui-playback
    provides: "Player, App struct, event loop, playback controls"
  - phase: 04-01
    provides: "Adaptive layout with narrow-mode rendering"
provides:
  - "Session struct in config.rs for save/load of playback state"
  - "now_playing file at ~/.local/share/termtunes/now_playing for tmux status bar"
  - "Session persistence across restarts (playlist, track, volume, shuffle, repeat)"
  - "Best-effort session restore on startup without auto-play"
affects: [05-hardening]

# Tech tracking
tech-stack:
  added: []
  patterns: ["Best-effort file writes (let _ = fs::write) for non-critical features", "Session save on graceful exit only (not per-action)", "Preserve session fields across navigation (go_back keeps last-playing context)"]

key-files:
  created: []
  modified: [src/config.rs, src/app.rs, src/main.rs]

key-decisions:
  - "Session saved only on graceful exit (q key or signal shutdown), not on every track change"
  - "now_playing file writes are best-effort with tracing::warn on failure"
  - "Session restore does NOT auto-play -- user must press Enter/Space to resume"
  - "go_back() preserves session-relevant fields (playlist key/title, track index) so save works from any view"
  - "RepeatMode string conversion via to_string_repr/from_string_repr for clean TOML serialization"

patterns-established:
  - "Session persistence: save on exit, restore on startup, position-only (no auto-play)"
  - "Tmux integration via file: write structured text to a well-known path, tmux reads with #(cat ...)"
  - "Navigation vs session state separation: go_back clears navigation state but preserves session context"

# Metrics
duration: 15min
completed: 2026-02-10
---

# Phase 4 Plan 2: Tmux Integration & Session Persistence Summary

**Tmux now-playing file at ~/.local/share/termtunes/now_playing and session persistence via session.toml with save/restore across app restarts**

## Performance

- **Duration:** ~15 min (across checkpoint with user verification)
- **Started:** 2026-02-10T18:00:00Z
- **Completed:** 2026-02-10T18:28:00Z
- **Tasks:** 2
- **Files modified:** 3

## Accomplishments
- Tmux status bar can display current track via `#(cat ~/.local/share/termtunes/now_playing)` with play/pause state
- Session state (playlist, track index, volume, shuffle, repeat) saves to session.toml on graceful exit
- Session restores on startup, positioning user at saved playlist/track in Tracks view without auto-play
- Session persistence works correctly from any view (Playlists, Tracks, or Playing) when quitting

## Task Commits

Each task was committed atomically:

1. **Task 1: Add Session struct, now-playing file writer, session save/restore** - `bf8ae46` (feat)
2. **Task 2 bug fix: Preserve session state when quitting from Playlists view** - `af39c38` (fix)

## Files Created/Modified
- `src/config.rs` - Added Session struct with serde derives, session_path(), load_session(), save_session(), now_playing_path()
- `src/app.rs` - Added write_now_playing_file(), save_session_state(), restore_session(), current_playlist_rating_key field, RepeatMode string conversion methods; fixed go_back() to preserve session fields
- `src/main.rs` - Added restore_session() call after App creation before run()

## Decisions Made
- Session save happens only on graceful exit (not per track change) to avoid disk I/O on every action
- now_playing file writes are best-effort -- errors logged via tracing::warn, never propagated
- Session restore positions user at Tracks view without auto-playing (user must manually start playback)
- go_back() now preserves playlist key/title and track index so session save works from Playlists view
- RepeatMode uses explicit to_string_repr/from_string_repr methods rather than Display/FromStr traits

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed session state loss when quitting from Playlists view**
- **Found during:** Task 2 verification (user-reported bug)
- **Issue:** go_back() cleared current_playlist_title, current_playlist_rating_key, current_track_index, and now_playing when navigating from Tracks/Playing to Playlists. When save_session_state() ran on quit, these fields were None/empty, and restore_session() returned early due to missing rating key, losing all session state.
- **Fix:** Removed the clearing of session-relevant fields from go_back(). These fields are now preserved for save_session_state() and naturally overwritten when the user selects a new playlist.
- **Files modified:** src/app.rs (go_back method)
- **Verification:** cargo build + cargo clippy pass clean
- **Committed in:** af39c38

---

**Total deviations:** 1 auto-fixed (1 bug)
**Impact on plan:** Essential fix for session persistence correctness. No scope creep.

## Issues Encountered
- Session persistence only worked when quitting from Tracks/Playing view, not from Playlists view. Root cause was go_back() clearing session fields before save_session_state() ran. Fixed by preserving session context across view navigation.

## User Setup Required
None - no external service configuration required.

**Optional tmux setup:** Add to tmux.conf for now-playing display:
```
set -g status-right "#(cat ~/.local/share/termtunes/now_playing) | %H:%M"
```

## Next Phase Readiness
- Phase 4 complete -- all requirements satisfied (DISP-09, DISP-10, POL-03, POL-04, POL-05)
- Ready for Phase 5 (Hardening)
- No new blockers or concerns

## Self-Check: PASSED

---
*Phase: 04-tmux-integration-polish*
*Completed: 2026-02-10*
