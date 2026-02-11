---
phase: 09-session-persistence
plan: 01
subsystem: persistence
tags: [serde, toml, session, ambient, backward-compat]

# Dependency graph
requires:
  - phase: 08-ambient-status-ui-controls
    provides: "Ambient volume controls, mute toggle, pre_mute_ambient_volume, ambient status panel"
  - phase: 07-track-browsing-ambient-playback
    provides: "Browser track selection, ambient download via background thread + mpsc, load_ambient_track"
  - phase: 06-dual-sink-audio-engine
    provides: "Dual-sink Player with ambient_track_name(), ambient looping, load_ambient()"
provides:
  - "Extended Session struct with ambient_part_key, ambient_track_name, ambient_volume, ambient_enabled fields"
  - "Ambient state round-trip: save on quit, restore on restart with background download"
  - "First-use ambient volume default (30% lower than main music volume)"
  - "Pre-mute volume preservation across quit/restart cycles"
  - "Player init guard for ambient-before-main edge case"
  - "Backward-compatible session deserialization via #[serde(default)]"
affects: []

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "#[serde(default)] per-field for backward-compatible struct extension"
    - "Option<f32> for first-use vs saved volume distinction"
    - "Pre-mute volume preservation in session save (save intended volume, not muted 0.0)"

key-files:
  created: []
  modified:
    - "src/config.rs"
    - "src/app.rs"

key-decisions:
  - "Store part_key (not full URL) for ambient track identification -- portable across token rotation"
  - "Save pre_mute_ambient_volume when muted (ambient_volume=0.0) so user's intended volume survives quit/restart"
  - "Player::new() takes no args -- init guard uses same no-arg pattern as check_download_complete()"
  - "Ambient restore gated on successful main playlist restore (placed after main restore in same function)"
  - "First-use default: (main_volume - 0.30).max(0.0) per PERSIST-05"

patterns-established:
  - "Backward-compatible Session extension: add #[serde(default)] to each new field"
  - "Capture part_key at selection time for later persistence"

# Metrics
duration: 2min
completed: 2026-02-11
---

# Phase 9 Plan 1: Session Persistence Summary

**Ambient channel state (track, volume, on/off, pre-mute) persisted in session.toml with backward-compatible serde deserialization and auto-resume on restart**

## Performance

- **Duration:** 2 min
- **Started:** 2026-02-11T14:23:00Z
- **Completed:** 2026-02-11T14:25:17Z
- **Tasks:** 2
- **Files modified:** 2

## Accomplishments
- Extended Session struct with 4 new ambient fields (part_key, track_name, volume, enabled) using `#[serde(default)]` for v1.0 backward compatibility
- Full ambient round-trip: select ambient track -> quit -> restart -> auto-resume at saved volume with correct on/off state
- Pre-mute volume preserved across quit/restart cycles (mute -> quit -> restart -> unmute restores correct volume)
- Player initialization guard handles ambient download completing before any main track is played
- First-use default computes ambient volume as 30% lower than main music volume

## Task Commits

Each task was committed atomically:

1. **Task 1: Extend Session struct with ambient fields, capture part_key on selection, wire save logic** - `4a9b5be` (feat)
2. **Task 2: Extend restore_session with ambient auto-resume, Player init guard, and first-use default** - `68374c5` (feat)

## Files Created/Modified
- `src/config.rs` - Extended Session struct with ambient_part_key, ambient_track_name, ambient_volume (Option<f32>), ambient_enabled fields with #[serde(default)]
- `src/app.rs` - Added ambient_part_key field to App, capture part_key in browser_select_track(), extended save_session_state() with ambient fields and pre-mute preservation, extended restore_session() with ambient volume/download restore and first-use default, added Player init guard in check_ambient_download_complete()

## Decisions Made
- Store `part_key` (server-relative path like `/library/parts/12345/file.flac`) instead of full stream URL -- URLs contain auth tokens that rotate on re-auth
- When ambient is muted (volume == 0.0), save `pre_mute_ambient_volume` to session instead of 0.0, so the user's intended volume survives quit/restart/unmute cycles
- Player::new() takes no arguments (confirmed from player.rs) -- the visualizer_data is passed later during load_and_play(), not during Player construction
- Ambient restore is gated on successful main playlist restore -- if main restore fails, user starts fresh (ambient is an accompaniment to main music)
- Use `Option<f32>` for ambient_volume to distinguish "never set" (None -> compute default) from "explicitly set" (Some(v) -> use saved value)

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Corrected Player::new() signature in init guard**
- **Found during:** Task 2 (Player init guard implementation)
- **Issue:** Plan specified `Player::new(self.visualizer_data.clone())` but actual signature is `Player::new()` with no arguments
- **Fix:** Used `Player::new()` without arguments, matching the existing pattern in `check_download_complete()`
- **Files modified:** src/app.rs
- **Verification:** `cargo build` passes
- **Committed in:** 68374c5 (Task 2 commit)

---

**Total deviations:** 1 auto-fixed (1 bug - incorrect function signature in plan)
**Impact on plan:** Minor correction to match actual codebase. No scope creep.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- All v1.1 multi-channel audio milestone features are now complete (Phase 6-9)
- Ambient track selection, playback, volume control, UI, and persistence are all wired end-to-end
- No blockers or concerns

## Self-Check: PASSED

- All files exist (src/config.rs, src/app.rs, 09-01-SUMMARY.md)
- All commits exist (4a9b5be, 68374c5)
- Content verified: ambient_part_key present in both config.rs and app.rs
- `cargo build` passes with no new warnings

---
*Phase: 09-session-persistence*
*Completed: 2026-02-11*
