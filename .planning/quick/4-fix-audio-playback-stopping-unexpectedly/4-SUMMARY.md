---
phase: quick-4
plan: 01
subsystem: audio
tags: [rodio, SamplesBuffer, pre-decode, PCM, WSL2, PulseAudio]

# Dependency graph
requires:
  - phase: quick-3
    provides: "Buffer tuning baseline (4096 cpal buffer, PULSE_LATENCY_MSEC 300)"
provides:
  - "Pre-decoded PCM playback via SamplesBuffer for both main and ambient channels"
  - "Symphonia decoder fully removed from audio callback thread"
  - "Offset-based seeking without try_seek on Decoder"
  - "PULSE_LATENCY_MSEC increased to 500ms"
affects: [player, audio-playback]

# Tech tracking
tech-stack:
  added: [rodio::buffer::SamplesBuffer]
  patterns: [pre-decode-to-pcm, offset-based-seeking]

key-files:
  created: []
  modified:
    - src/player.rs
    - src/main.rs
    - src/app.rs

key-decisions:
  - "Pre-decode entire track to Vec<f32> before feeding to rodio sink -- eliminates symphonia from audio thread"
  - "SamplesBuffer replaces Decoder in all audio hot paths (load, replay, seek, ambient)"
  - "Seek implemented by recreating SamplesBuffer from sample offset (VisualizerSource does not implement try_seek)"
  - "PULSE_LATENCY_MSEC bumped from 300 to 500 for sustained playback beyond 1 minute"
  - "Keep compressed audio bytes (_audio_data, ambient_audio_data) alongside decoded PCM for backward compatibility"

patterns-established:
  - "Pre-decode pattern: decode_to_pcm() runs decoder once upfront, stores (Vec<f32>, channels, sample_rate)"
  - "Seek by offset: calculate sample position, slice Vec, create new SamplesBuffer"
  - "Ambient replay fallback: prefer decoded PCM, fall back to re-decode from compressed bytes"

requirements-completed: [QUICK-4]

# Metrics
duration: 7min
completed: 2026-02-18
---

# Quick Task 4: Fix Audio Playback Stopping Unexpectedly - Summary

**Pre-decode audio to raw PCM via SamplesBuffer, eliminating symphonia decoder from the audio callback thread to prevent WSL2 scheduling-jitter-induced stream deaths**

## Performance

- **Duration:** 7 min
- **Started:** 2026-02-18T07:44:55Z
- **Completed:** 2026-02-18T07:52:14Z
- **Tasks:** 2
- **Files modified:** 3

## Accomplishments
- Symphonia decoder completely removed from the audio callback thread -- callback now only reads from pre-decoded Vec<f32>
- Both main and ambient channels use SamplesBuffer for playback, replay, and (main only) seeking
- PULSE_LATENCY_MSEC increased from 300ms to 500ms for additional WSL2 PulseAudio headroom
- Seeking reimplemented as SamplesBuffer-from-offset (no try_seek on Decoder/VisualizerSource)
- Visualizer data stored on Player struct so seek/replay methods can re-wrap sources

## Task Commits

Each task was committed atomically:

1. **Task 1: Add pre-decode helper and convert main playback to SamplesBuffer** - `ccca046` (feat)
2. **Task 2: Convert ambient channel to SamplesBuffer and bump PULSE_LATENCY_MSEC** - `1a913e5` (feat)

## Files Created/Modified
- `src/player.rs` - Added decode_to_pcm() helper, decoded_pcm/ambient_decoded_pcm/visualizer_data fields, SamplesBuffer-based load/replay/seek for both channels
- `src/main.rs` - Bumped PULSE_LATENCY_MSEC from "300" to "500" with updated rationale comment
- `src/app.rs` - Updated seek call sites for &mut self signature change (seek_forward/seek_backward)

## Decisions Made
- Pre-decode entire track to Vec<f32> before feeding to rodio sink. This eliminates the symphonia decoder from the audio callback thread, which was the root cause of stream deaths on WSL2.
- Seek by recreating SamplesBuffer from offset rather than using try_seek, because VisualizerSource does not implement try_seek.
- Store visualizer_data on the Player struct so seek and replay methods can re-wrap sources with VisualizerSource.
- Keep compressed bytes alongside decoded PCM (not replace them) for backward compatibility and re-download avoidance checks.
- Ambient replay includes fallback to re-decode from compressed bytes if decoded PCM is somehow None.

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

- Reference vs value issue in `seek_to()`: Rust's `as_ref()` on `Option<(Vec<f32>, u16, u32)>` returns references to the inner values; needed to copy u16/u32 primitives before using them in calculations and SamplesBuffer::new(). Fixed by extracting with `let channels = *channels;`.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness
- Audio playback should now sustain for 5+ minutes without stopping on WSL2
- The pre-decode approach trades memory (full PCM in RAM) for audio thread reliability
- If memory usage becomes a concern for very long tracks, streaming decode with a large ring buffer could be explored

## Self-Check: PASSED

All files and commits verified:
- src/player.rs: FOUND
- src/main.rs: FOUND
- src/app.rs: FOUND
- 4-SUMMARY.md: FOUND
- Commit ccca046: FOUND
- Commit 1a913e5: FOUND

---
*Phase: quick-4*
*Completed: 2026-02-18*
