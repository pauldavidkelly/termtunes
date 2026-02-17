---
phase: quick-3
plan: 01
subsystem: audio
tags: [cpal, rodio, wsl2, pulseaudio, buffer-tuning]

# Dependency graph
requires:
  - phase: 01-foundation-audio-poc
    provides: "Audio pipeline with rodio/cpal OutputStream and Sink"
provides:
  - "Explicit 4096-sample cpal buffer for WSL2 audio stability"
  - "PULSE_LATENCY_MSEC increased to 300ms"
  - "Stream error callback routed through tracing"
  - "Corrected .asoundrc comments"
affects: []

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Builder chain with fallback: from_default_device() -> explicit config -> or_else(open_default_stream)"
    - "Error callbacks routed to tracing instead of stderr (hidden by TUI)"

key-files:
  created: []
  modified:
    - "src/player.rs"
    - "src/main.rs"

key-decisions:
  - "BufferSize::Fixed(4096) chosen for stability-focused background music use case (rodio docs recommendation)"
  - "PULSE_LATENCY_MSEC 300ms chosen as double the previous 150ms to absorb WSLg scheduling jitter"
  - "Fallback to open_default_stream() if explicit buffer config fails (graceful degradation)"
  - "Used rodio::cpal::BufferSize instead of adding cpal as direct dependency"

patterns-established:
  - "Audio stream builder with explicit buffer + error callback + fallback"

requirements-completed: [AUDIO-STUTTER-FIX]

# Metrics
duration: 3min
completed: 2026-02-17
---

# Quick Task 3: Fix Audio Playback Stuttering Summary

**Explicit 4096-sample cpal buffer and 300ms PULSE_LATENCY_MSEC to mitigate WSLg PulseAudio bridge stuttering on WSL2**

## Performance

- **Duration:** 3 min
- **Started:** 2026-02-17T17:31:26Z
- **Completed:** 2026-02-17T17:34:20Z
- **Tasks:** 2
- **Files modified:** 2

## Accomplishments
- Increased PULSE_LATENCY_MSEC from 150 to 300ms with updated comment explaining why 150ms was insufficient
- Replaced open_default_stream() with builder chain setting BufferSize::Fixed(4096) and tracing error callback
- Added graceful fallback to default stream if explicit buffer configuration fails
- Corrected misleading .asoundrc comments that described buffer_size parameters the pulse plugin does not support

## Task Commits

Each task was committed atomically:

1. **Task 1: Increase PULSE_LATENCY_MSEC to 300** - `895ce89` (fix)
2. **Task 2: Replace open_default_stream with explicit buffer size and error callback** - `e0dd365` (fix)

## Files Created/Modified
- `src/main.rs` - PULSE_LATENCY_MSEC increased from 150 to 300, comment updated
- `src/player.rs` - OutputStreamBuilder with explicit 4096-sample buffer, tracing error callback, fallback to default, corrected .asoundrc comments

## Decisions Made
- BufferSize::Fixed(4096) selected per rodio's recommendation for "stability-focused (background music, non-interactive)" use case (~93ms at 44100 Hz)
- 300ms PULSE_LATENCY_MSEC (double previous 150ms) to absorb WSLg PulseAudio bridge jitter that caused stuttering after ~20s
- Fallback to open_default_stream() ensures the app still works if explicit buffer size is rejected by the audio backend
- Used `rodio::cpal::BufferSize` (re-export) instead of adding cpal as a direct dependency

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Used rodio::cpal::BufferSize instead of cpal::BufferSize**
- **Found during:** Task 2 (stream builder replacement)
- **Issue:** Plan specified `cpal::BufferSize::Fixed(4096)` but `cpal` is not a direct dependency -- it is accessed through rodio's re-export
- **Fix:** Changed to `rodio::cpal::BufferSize::Fixed(4096)`
- **Files modified:** src/player.rs
- **Verification:** cargo check passes
- **Committed in:** e0dd365 (Task 2 commit)

---

**Total deviations:** 1 auto-fixed (1 blocking)
**Impact on plan:** Minimal -- qualified path through rodio's cpal re-export instead of direct cpal import. Same functionality.

## Issues Encountered
None beyond the cpal import path (documented above as deviation).

## User Setup Required
None - no external service configuration required.

## Next Steps
- Monitor audio playback on WSL2 to verify stuttering is reduced
- If stuttering persists, consider further increasing buffer size or PULSE_LATENCY_MSEC
- Check termtunes.log for "Audio stream error (possible underrun)" messages to diagnose any remaining issues

---
*Quick Task: 3-fix-audio-playback-stuttering*
*Completed: 2026-02-17*
