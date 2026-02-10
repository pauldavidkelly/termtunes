---
phase: 01-foundation-audio-poc
plan: 03
subsystem: audio
tags: [rust, rodio, reqwest, ratatui, wsl2, pulseaudio, crossterm, mpsc]

# Dependency graph
requires:
  - phase: 01-01
    provides: "Terminal lifecycle, panic hooks, signal handlers, config persistence"
  - phase: 01-02
    provides: "Plex auth, API client, playlist/track browser, stream_url()"
provides:
  - "Audio playback engine via rodio Sink with download-then-play pattern"
  - "Play/pause toggle with spacebar, reliable after >5 second pauses on WSL2"
  - "Status bar showing track name and playback state (>> playing, || paused)"
  - "Background track download with mpsc channel for non-blocking UI"
  - "Automated terminal restoration test script (SIGINT/SIGTERM/SIGHUP)"
  - "Complete Phase 1 proof-of-concept: auth -> browse -> play -> pause -> resume -> quit"
affects: [02-playback-engine, 02-queue-management, 03-ui-enhancement]

# Tech tracking
tech-stack:
  added: [rodio, reqwest-blocking]
  patterns: [download-then-play, sink-lifetime-management, mpsc-background-download, pulse-latency-tuning, wsl2-audio-buffer-config]

key-files:
  created:
    - src/player.rs
    - src/ui.rs
    - scripts/test_terminal_restore.sh
  modified:
    - src/app.rs
    - src/main.rs

key-decisions:
  - "Download full track into Vec<u8> before playback (not streaming) for WSL2 reliability"
  - "PULSE_LATENCY_MSEC=60 set at startup for WSL2 audio latency"
  - "rodio OutputStream kept alive for entire app lifetime to prevent audio device drops"
  - "Background download via std::thread::spawn + mpsc channel to keep UI responsive"
  - "Audio buffer size tuned larger for WSL2 PulseAudio stability (eliminates crackling)"
  - "Player initialization deferred until first playback (lazy init)"

patterns-established:
  - "Player pattern: OutputStream + Sink lifetime coupling -- OutputStream MUST outlive Sink usage"
  - "Download pattern: reqwest::blocking::get on background thread, Vec<u8> sent via mpsc"
  - "Status bar pattern: bottom row Paragraph widget with play/pause state colors"
  - "WSL2 audio pattern: PULSE_LATENCY_MSEC=60 + larger buffer config for PulseAudio stability"

# Metrics
duration: ~38min
completed: 2026-02-10
---

# Phase 1 Plan 3: Audio Playback and Phase 1 PoC Summary

**rodio-based audio playback from Plex tracks on WSL2 with download-then-play pattern, spacebar pause/resume (reliable after >5s pauses), status bar with playback state, and automated terminal restoration test script**

## Performance

- **Duration:** ~38 min (including human verification checkpoint and audio quality tuning)
- **Started:** 2026-02-10T14:50:11Z
- **Completed:** 2026-02-10T15:28:00Z
- **Tasks:** 3 (2 auto + 1 human-verify checkpoint)
- **Files created:** 3
- **Files modified:** 2

## Accomplishments
- Implemented complete audio playback engine with rodio Sink wrapping OutputStream lifetime management
- Built non-blocking download flow: background thread downloads track bytes via reqwest::blocking, sends to UI thread via mpsc channel
- Integrated play/pause toggle (spacebar) that works reliably on WSL2 including after >5 second pauses
- Created status bar showing track name with play/pause state indicators (>> green for playing, || yellow for paused)
- Automated terminal restoration test script validates SIGINT, SIGTERM, and SIGHUP exit paths (3/3 pass)
- Tuned WSL2 audio buffer settings to eliminate crackling/clicking artifacts
- Complete Phase 1 end-to-end flow proven: auth -> browse playlists -> select track -> hear audio -> pause/resume -> quit cleanly

## Task Commits

Each task was committed atomically:

1. **Task 1: Implement audio player module and integrate playback into TUI** - `caf1f98` (feat)
2. **Task 2: Create automated terminal restoration test script** - `748fab9` (feat)
3. **Fix 1: Handle WSL2 audio device initialization failure gracefully** - `f6a7fea` (fix)
4. **Fix 2: Tune WSL2 audio buffer settings to eliminate crackling** - `e2b0521` (fix)
5. **Task 3: Verify complete Phase 1 proof-of-concept** - (human-verify checkpoint, approved)

## Files Created/Modified
- `src/player.rs` - Audio engine: Player struct with rodio Sink, download_track (reqwest::blocking), load_and_play, toggle_pause, playback state queries
- `src/ui.rs` - TUI rendering: vertical layout with main area (playlists/tracks/downloading states) and status bar with play/pause colors
- `scripts/test_terminal_restore.sh` - Automated test: validates terminal restoration after SIGINT, SIGTERM, SIGHUP (3 test cases, all passing)
- `src/app.rs` - Wired playback into event loop: Enter triggers background download, spacebar toggles pause, mpsc channel for async download completion
- `src/main.rs` - Added mod player/ui declarations, PULSE_LATENCY_MSEC=60 env var at startup for WSL2

## Decisions Made
- **Download-then-play:** Full track downloaded into Vec<u8> before playback rather than streaming. Simpler and more reliable on WSL2 where PulseAudio can be finicky with streaming sources.
- **PULSE_LATENCY_MSEC=60:** Set at startup before any audio device init to reduce WSL2 PulseAudio latency from the default 250ms.
- **OutputStream lifetime:** Stored in Player struct alongside Sink to prevent premature drop which kills audio output. This is a common rodio pitfall.
- **Background download:** Used std::thread::spawn + mpsc::channel to download track bytes without blocking the TUI event loop. Status shows "Downloading..." during fetch.
- **Lazy player init:** Player created on first track play, not at app startup. Avoids audio device init when user is just browsing.
- **Buffer tuning for WSL2:** Increased audio buffer size to eliminate crackling/clicking caused by WSL2 PulseAudio underruns.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] WSL2 audio device initialization failure**
- **Found during:** Task 1 (testing audio playback on WSL2)
- **Issue:** rodio OutputStream::try_default() could fail on WSL2 when PulseAudio is not ready or misconfigured. The app would crash with an unhelpful error.
- **Fix:** Added graceful error handling for audio device init failure with user-visible error message. Player initialization returns Result and app handles the None case.
- **Files modified:** src/player.rs, src/app.rs, src/main.rs, src/ui.rs
- **Verification:** App handles audio device unavailability gracefully, shows error in status bar
- **Committed in:** `f6a7fea`

**2. [Rule 1 - Bug] Audio crackling/clicking on WSL2**
- **Found during:** Task 3 human-verify checkpoint (audio quality check)
- **Issue:** Audio playback had crackling/clicking artifacts caused by WSL2 PulseAudio buffer underruns at default buffer sizes.
- **Fix:** Tuned audio buffer configuration for WSL2 PulseAudio stability by adjusting buffer sizes in player initialization.
- **Files modified:** src/player.rs, src/main.rs
- **Verification:** Human verified audio plays smoothly without crackling after fix
- **Committed in:** `e2b0521`

---

**Total deviations:** 2 auto-fixed (2 bug fixes)
**Impact on plan:** Both fixes were necessary for correct audio playback on WSL2. The crackling fix was critical for usability. No scope creep.

## Issues Encountered
- WSL2 PulseAudio requires specific buffer tuning to avoid audio artifacts. The default rodio settings produce crackling on WSL2 due to PulseAudio bridge timing differences. Resolved by adjusting buffer sizes and setting PULSE_LATENCY_MSEC=60.
- Audio device initialization can fail on WSL2 if PulseAudio service is not running. Added graceful fallback rather than crashing.

## User Setup Required
None - audio playback works automatically on WSL2 with PulseAudio configured (standard WSL2 setup).

## Next Phase Readiness
- Phase 1 proof-of-concept fully validated: auth, browse, play, pause/resume, terminal restoration all working
- WSL2 audio reliability confirmed (biggest technical risk validated)
- Ready for Phase 2 which will build on this foundation with queue management, progress tracking, etc.
- Player module provides clean interface for future enhancements (seek, volume, queue)
- No blockers for Phase 2

## Self-Check: PASSED

All referenced files exist (5/5), all commits verified (4/4), summary file created.

---
*Phase: 01-foundation-audio-poc*
*Completed: 2026-02-10*
