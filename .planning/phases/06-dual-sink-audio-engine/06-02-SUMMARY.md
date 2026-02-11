---
phase: 06-dual-sink-audio-engine
plan: 02
subsystem: audio
tags: [rodio, dual-sink, ambient-loop, volume-independent, wsl2-audio, failure-isolation]

# Dependency graph
requires:
  - phase: 06-01
    provides: "Dual-sink Player with main_sink + ambient_sink and ambient lifecycle methods"
provides:
  - "Ambient loop detection wired into event loop (auto-restarts from cached bytes)"
  - "Temporary 'a' keybinding for ambient playback testing"
  - "Ambient mute/unmute toggle via 'm' keybinding"
  - "Failure-isolated ambient loading (errors logged, main unaffected)"
  - "Independent main/ambient volume channels (no proportional budget)"
  - "Background thread ambient download (same pattern as main track)"
  - "Ambient sink recreation on volume change (rodio set_volume workaround)"
  - "WSL2 dual-channel audio validated"
affects: [07-ambient-track-selection, 08-ambient-status-ui-controls]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Independent volume channels: main and ambient scaled by master_volume only, no coupling"
    - "Ambient sink recreation: stop + create new Sink + re-decode cached bytes on volume change"
    - "Background thread + mpsc channel for ambient downloads (avoids tokio runtime nesting)"

key-files:
  created: []
  modified:
    - "src/app.rs"
    - "src/player.rs"
    - "src/main.rs"
    - "src/ui.rs"

key-decisions:
  - "Replaced proportional volume budget with independent channels after 8 fix iterations"
  - "Ambient sink recreated on volume change because rodio Sink::set_volume() unreliable for ambient"
  - "Background thread for ambient download to avoid tokio runtime nesting panic"
  - "UI shows saved_volume (user intent) not sink volume (budget-scaled value)"
  - "Logging defaults to info level when RUST_LOG not set"

patterns-established:
  - "Independent volume: apply_main_volume() and apply_ambient_volume() replace apply_volume_budget()"
  - "Sink recreation pattern: stop old sink, create new at target volume, re-decode from cached bytes"
  - "Ambient download: std::thread::spawn + mpsc channel, polled via check_ambient_download_complete()"

# Metrics
duration: extended (multiple verification cycles with user)
completed: 2026-02-11
---

# Phase 6 Plan 2: Ambient Loop and WSL2 Validation Summary

**Ambient loop detection, test trigger, and failure isolation wired into event loop with independent main/ambient volume channels validated on WSL2**

## Performance

- **Duration:** Extended (multiple verification cycles -- 8 bug fixes discovered through user testing)
- **Started:** 2026-02-11
- **Completed:** 2026-02-11T09:25:50Z
- **Tasks:** 2
- **Files modified:** 4

## Accomplishments
- Wired ambient loop detection into event loop (auto-restarts ambient from cached bytes when track ends)
- Added temporary 'a' keybinding to load selected track as ambient and 'm' to mute/unmute
- Implemented failure-isolated ambient loading (decode/play errors logged, main playback unaffected)
- Discovered and fixed rodio Sink::set_volume() unreliability for ambient sinks (sink recreation workaround)
- Replaced proportional volume budget with independent channel volumes after iterative debugging
- Validated dual-channel audio on WSL2 -- both channels play simultaneously without crackling/distortion
- Fixed logging infrastructure to default to info level when RUST_LOG not set

## Task Commits

Task 1 had 1 initial commit + 8 fix commits discovered during human verification:

1. **Task 1: Wire ambient loop, test trigger, and failure isolation** - `cb9f6ec` (feat)
   - Fix 1: `1159ac9` - Background thread for ambient download (tokio runtime panic)
   - Fix 2: `3e95066` - Apply volume after ambient load
   - Fix 3: `3725673` - Budget only when ambient sink active
   - Fix 4: `dabfeb6` - UI shows user intent, not sink value
   - Fix 5: `8edb82a` - Fix logging default filter
   - Fix 6: `f11a3ac` - Deep diagnostic logging for ambient volume
   - Fix 7: `936764e` - Recreate ambient sink on volume change
   - Fix 8: `f30b785` - Replace budget with independent channels
2. **Task 2: Verify dual-channel audio on WSL2** - Human verification approved

## Files Created/Modified
- `src/app.rs` - Ambient loop check in event loop, test trigger ('a'), mute toggle ('m'), failure-isolated loading, independent volume channels (apply_main_volume, apply_ambient_volume), background ambient download
- `src/player.rs` - set_ambient_volume recreates sink (rodio workaround), has_ambient_sink(), ambient_volume() read-back
- `src/main.rs` - Logging filter defaults to info when RUST_LOG unset
- `src/ui.rs` - Volume display shows saved_volume (user intent) instead of sink value

## Decisions Made

1. **Independent volume channels replace proportional budget:** The original design (main + ambient <= 1.0 with proportional scaling) created severe UX problems: volume capped at 59%, ambient audible at 0% main volume, +/- barely affected ambient (~3% changes). After 8 iterative fixes, replaced with independent channels where each is scaled only by master_volume.

2. **Ambient sink recreation on volume change:** rodio's `Sink::set_volume()` updates an internal Mutex but the periodic_access callback doesn't reliably apply it to ambient sinks. Confirmed through extensive logging (before/after values correct, audio unchanged). Workaround: stop old sink, create new Sink at target volume, re-decode from cached audio bytes, append.

3. **Background thread for ambient download:** `reqwest::blocking::get()` creates an internal tokio runtime. Calling it from within the app's async event loop causes a "cannot drop runtime in context where blocking not allowed" panic. Solution: same std::thread::spawn + mpsc channel pattern used for main track downloads.

4. **UI volume display source:** Changed from `player.volume()` (which returns the effective/scaled sink value) to `app.saved_volume()` (the user's intended setting). Prevents confusing display when budget scaling was active.

5. **Logging default level:** `EnvFilter::from_default_env()` defaults to OFF/ERROR when RUST_LOG is not set. Changed to fallback to "info" level for useful default logging.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Tokio runtime nesting panic on ambient download**
- **Found during:** Task 2 verification (user pressed 'a', app crashed)
- **Issue:** `Player::download_track()` uses `reqwest::blocking` which internally creates a tokio runtime. Called from within async event loop, this panics with "Cannot drop a runtime in a context where blocking is not allowed"
- **Fix:** Replaced synchronous download with background std::thread::spawn + mpsc channel pattern (same as main track downloads). Added `ambient_download_rx` field, `check_ambient_download_complete()` method
- **Files modified:** src/app.rs
- **Committed in:** 1159ac9

**2. [Rule 1 - Bug] Volume budget not applied after ambient load**
- **Found during:** Task 2 verification (ambient played at full volume)
- **Issue:** `load_ambient_track()` loaded audio but didn't call volume enforcement afterward
- **Fix:** Added `apply_volume_budget()` call after successful ambient load
- **Files modified:** src/app.rs
- **Committed in:** 3e95066

**3. [Rule 1 - Bug] Volume budget active without ambient sink**
- **Found during:** Task 2 verification (main volume capped at 59%)
- **Issue:** Budget calculation always included `ambient_volume=0.7` even with no ambient playing. With saved_volume=1.0, sum=1.7 triggered scaling, capping main at 59%
- **Fix:** Added `Player::has_ambient_sink()` check; budget uses effective_ambient=0.0 when no ambient loaded
- **Files modified:** src/app.rs, src/player.rs
- **Committed in:** 3725673

**4. [Rule 1 - Bug] UI displaying budget-scaled volume instead of user intent**
- **Found during:** Task 2 verification (volume showed 59% instead of 100%)
- **Issue:** UI read `player.volume()` (the sink's effective value after budget scaling) instead of the user's intended volume
- **Fix:** Changed UI to display `app.saved_volume()` (user intent)
- **Files modified:** src/ui.rs
- **Committed in:** dabfeb6

**5. [Rule 3 - Blocking] Logging not working (empty log file)**
- **Found during:** Task 2 verification (needed logs to debug volume issues)
- **Issue:** `EnvFilter::from_default_env()` defaults to OFF/ERROR when RUST_LOG env var not set
- **Fix:** Changed to fallback: `EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"))`
- **Files modified:** src/main.rs
- **Committed in:** 8edb82a

**6. [Rule 1 - Bug] Diagnostic logging insufficient for ambient volume debugging**
- **Found during:** Task 2 verification (needed to confirm whether set_ambient_volume was actually called)
- **Issue:** Logs showed budget calculations but not whether the volume was applied to the sink
- **Fix:** Added `Player::ambient_volume()` read-back method, before/after logging in set_ambient_volume
- **Files modified:** src/app.rs, src/player.rs
- **Committed in:** f11a3ac

**7. [Rule 1 - Bug] rodio Sink::set_volume() doesn't reliably update ambient audio**
- **Found during:** Task 2 verification (logs correct, audio unchanged)
- **Issue:** rodio's periodic_access callback doesn't reliably pick up volume changes for ambient sinks. Confirmed: set_ambient_volume IS called with correct values, Mutex updates, but audio output doesn't change
- **Fix:** Recreate entire ambient sink on volume change (stop old, create new at target volume, re-decode from cached bytes, append)
- **Files modified:** src/player.rs, src/app.rs
- **Committed in:** 936764e

**8. [Rule 1 - Bug] Proportional volume budget creates counterintuitive coupling**
- **Found during:** Task 2 verification (ambient doesn't change with +/-, ambient at 0% main)
- **Issue:** Budget proportional scaling meant +/- changed ambient by ~3% (inaudible), and at 0% main, ambient played at full 70% (sum=0.7 <= 1.0, no scaling)
- **Fix:** Removed proportional budget entirely. Replaced with independent channels: apply_main_volume() and apply_ambient_volume(), each scaled only by master_volume
- **Files modified:** src/app.rs
- **Committed in:** f30b785

---

**Total deviations:** 8 auto-fixed (7 bugs, 1 blocking)
**Impact on plan:** All fixes were necessary for correct audio behavior on WSL2. The volume architecture was fundamentally redesigned from budget-based coupling to independent channels based on real-world testing feedback. No scope creep -- all changes serve the plan's stated goal of verified dual-channel audio.

## Issues Encountered

The proportional volume budget was the most significant issue. It required 6 iterative fix cycles to diagnose because each fix revealed the next layer of the problem:
1. Budget not applied -> applied it
2. Budget applied when it shouldn't be -> conditional on ambient sink
3. UI showing wrong value -> show user intent
4. Can't see what's happening -> fix logging
5. Volume set correctly but audio doesn't change -> rodio bug, recreate sink
6. Volume changes inaudible -> budget coupling is fundamentally flawed, remove budget

This is a classic "onion" debugging pattern where each fix peels back a layer to reveal the real root cause. The final architecture (independent channels) is simpler and more correct than the original budget design.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Dual-channel audio validated on WSL2 (the fail-fast gate for v1.1 is passed)
- Ambient loop detection works reliably with sink recreation pattern
- Independent volume channels established (simpler than budget, no coupling)
- Temporary 'a' keybinding ready to be replaced by Phase 7 track browser
- 'm' mute toggle working for ambient channel
- All v1.0 playback features working identically (no regressions)
- Pattern note for Phase 7+: ambient volume changes require sink recreation (not set_volume)
- Pattern note for Phase 8: apply_volume_budget() no longer exists -- use apply_main_volume() and apply_ambient_volume()

## Self-Check: PASSED

All files exist (src/app.rs, src/player.rs, src/main.rs, src/ui.rs, 06-02-SUMMARY.md).
All 9 commits verified (cb9f6ec, 1159ac9, 3e95066, 3725673, dabfeb6, 8edb82a, f11a3ac, 936764e, f30b785).

---
*Phase: 06-dual-sink-audio-engine*
*Completed: 2026-02-11*
