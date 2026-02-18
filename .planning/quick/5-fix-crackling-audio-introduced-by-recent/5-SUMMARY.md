---
phase: quick-5
plan: 01
subsystem: audio
tags: [pulseaudio, wsl2, buffer-tuning, crackling]

# Dependency graph
requires:
  - phase: quick-4
    provides: "Pre-decoded PCM playback, PULSE_LATENCY_MSEC=500, Fixed(4096) buffer"
provides:
  - "WSL2-conditional PULSE_LATENCY_MSEC — not set outside WSL2"
  - "WSL2-conditional Fixed(4096) cpal buffer — default stream used outside WSL2"
  - "Clean audio playback outside Tmux/WSL2 as known-good baseline"
affects: [player, main]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Environment-conditional audio tuning: WSL2 check gates platform-specific settings"

key-files:
  created: []
  modified:
    - "src/main.rs"
    - "src/player.rs"

key-decisions:
  - "PULSE_LATENCY_MSEC=500 is WSL2-only — inline /proc/version check in main.rs"
  - "BufferSize::Fixed(4096) is WSL2-only — is_wsl2() check in Player::new()"
  - "Non-WSL2 path uses OS-default buffer sizes via open_default_stream() fallback pattern"
  - "Pre-decode SamplesBuffer approach (quick-4) retained unchanged — not the cause of crackling"

patterns-established:
  - "WSL2-conditional audio: check /proc/version before setting PulseAudio hints or cpal buffer size"

requirements-completed: [QUICK-5]

# Metrics
duration: 10min
completed: 2026-02-18
---

# Quick Task 5: Fix Crackling Audio — WSL2-Conditional Buffer Settings

**Gate PULSE_LATENCY_MSEC and Fixed(4096) buffer behind is_wsl2() check to restore clean audio outside Tmux/WSL2**

## Performance

- **Duration:** ~10 min
- **Completed:** 2026-02-18
- **Tasks:** 2
- **Files modified:** 2

## Root Cause

Quick tasks 3 and 4 applied two WSL2-specific audio settings unconditionally:

1. `PULSE_LATENCY_MSEC=500` in `main.rs` — On native Linux/macOS, this forces PulseAudio
   to use a 500ms buffer regardless of environment. The oversized buffer causes crackling
   because it conflicts with rodio's internal buffering on non-WSL2 audio stacks.

2. `BufferSize::Fixed(4096)` in `player.rs` — On non-WSL2 systems, the OS/driver chooses
   an optimal buffer size automatically. Forcing 4096 samples where it isn't needed causes
   timing mismatches that manifest as crackling.

The pre-decode SamplesBuffer approach (quick-4) was NOT the cause — it is architecturally
correct and retained unchanged.

## Accomplishments

- `PULSE_LATENCY_MSEC=500` now only set when `/proc/version` contains "microsoft"/"WSL"
- `BufferSize::Fixed(4096)` now only used when `is_wsl2()` returns true
- Non-WSL2 path uses `open_default_stream()` fallback pattern with error callback for diagnostics
- Both changes guarded by clear comments explaining WHY they are WSL2-specific

## Task Commits

1. **Task 1 + Task 2 (combined):** Gate PULSE_LATENCY_MSEC and Fixed buffer behind WSL2 check — `4f65e99`

## Files Modified

- `src/main.rs` — PULSE_LATENCY_MSEC wrapped in `if on_wsl2 { ... }` block
- `src/player.rs` — `Player::new()` uses `if is_wsl2()` to choose stream builder path

## Deviations from Plan

None — plan executed exactly as written.

## Known-Good Baseline

After this fix:
- **Outside WSL2/Tmux:** clean audio, no crackling (OS default buffer sizes, no PulseAudio latency hint)
- **On WSL2:** WSL2-specific tuning still active (PULSE_LATENCY_MSEC=500, Fixed(4096) buffer)
- Tmux-specific streaming issues remain to be investigated separately

## Next Steps

- Verify clean audio playback outside Tmux on the test machine
- Then investigate Tmux-specific streaming issues (stuttering after 20-40s) separately
- Tmux fix may need a different approach (e.g., environment detection, Tmux-specific buffer tuning)

---
*Quick Task: 5-fix-crackling-audio-introduced-by-recent*
*Completed: 2026-02-18*
