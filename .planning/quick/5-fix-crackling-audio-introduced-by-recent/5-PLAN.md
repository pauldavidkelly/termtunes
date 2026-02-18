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

# Quick Task 5: Fix Crackling Audio — Make WSL2 Settings Conditional

## Problem

Quick tasks 3 and 4 introduced two unconditional audio settings meant for WSL2/Tmux:
1. `PULSE_LATENCY_MSEC=500` set globally in `main.rs` — forces PulseAudio to buffer
   500ms of audio regardless of environment. On native Linux/PulseAudio (non-WSL2)
   this causes crackling because the latency hint conflicts with rodio's own buffering.
2. `BufferSize::Fixed(4096)` in `player.rs` — forces a fixed 4096-sample cpal buffer
   globally. On non-WSL2 systems the OS-chosen default buffer is more appropriate.

Result: both main track and ambient track crackle when running outside Tmux/WSL2.

## Root Cause Analysis

`is_wsl2()` helper already exists in `player.rs` (private fn). The fix is to:
- Only set `PULSE_LATENCY_MSEC` when `is_wsl2()` is true
- Only use `BufferSize::Fixed(4096)` when `is_wsl2()` is true; otherwise use
  `OutputStreamBuilder::open_default_stream()` directly

The pre-decode SamplesBuffer approach from quick-4 is correct and should remain.

## Tasks

### Task 1: Make PULSE_LATENCY_MSEC conditional on WSL2

**File:** `src/main.rs`

Change the unconditional `set_var("PULSE_LATENCY_MSEC", "500")` to only fire on WSL2.
Inline the WSL2 detection (read `/proc/version`, check for "microsoft"/"WSL") rather
than importing from player.rs (is_wsl2 is private there).

Before:
```rust
unsafe { std::env::set_var("PULSE_LATENCY_MSEC", "500") };
```

After:
```rust
// Only set PULSE_LATENCY_MSEC on WSL2. On native Linux/macOS this env var
// forces PulseAudio to use an oversized buffer which causes crackling.
// Must be set BEFORE creating any OutputStream/audio device.
if std::fs::read_to_string("/proc/version")
    .map(|v| v.contains("microsoft") || v.contains("WSL"))
    .unwrap_or(false)
{
    unsafe { std::env::set_var("PULSE_LATENCY_MSEC", "500") };
    tracing::info!("WSL2 detected: PULSE_LATENCY_MSEC set to 500ms");
}
```

### Task 2: Make Fixed(4096) buffer conditional on WSL2

**File:** `src/player.rs`

Change `Player::new()` to use the explicit buffer size only on WSL2, and fall back to
`open_default_stream()` on non-WSL2 platforms where the default is more appropriate.

Before (always tries Fixed(4096) first):
```rust
let stream = OutputStreamBuilder::from_default_device()
    .and_then(|builder| {
        builder
            .with_buffer_size(rodio::cpal::BufferSize::Fixed(4096))
            .with_error_callback(|err| {
                tracing::warn!("Audio stream error (possible underrun): {err}");
            })
            .open_stream()
    })
    .or_else(|_| {
        tracing::info!("Explicit buffer config failed, falling back to default stream");
        OutputStreamBuilder::open_default_stream()
    })
```

After (Fixed(4096) only on WSL2):
```rust
let stream = if is_wsl2() {
    // On WSL2, use explicit 4096-sample buffer to absorb WSLg PulseAudio jitter.
    // Falls back to default if the explicit config is rejected by the backend.
    OutputStreamBuilder::from_default_device()
        .and_then(|builder| {
            builder
                .with_buffer_size(rodio::cpal::BufferSize::Fixed(4096))
                .with_error_callback(|err| {
                    tracing::warn!("Audio stream error (possible underrun): {err}");
                })
                .open_stream()
        })
        .or_else(|_| {
            tracing::info!("WSL2 explicit buffer config failed, falling back to default stream");
            OutputStreamBuilder::open_default_stream()
        })
} else {
    // On non-WSL2, let the OS pick the optimal buffer size to avoid crackling.
    // Add error callback for diagnostics but use the default buffer size.
    OutputStreamBuilder::from_default_device()
        .and_then(|builder| {
            builder
                .with_error_callback(|err| {
                    tracing::warn!("Audio stream error (possible underrun): {err}");
                })
                .open_stream()
        })
        .or_else(|_| {
            tracing::info!("Explicit stream config failed, falling back to default stream");
            OutputStreamBuilder::open_default_stream()
        })
}
```

## Success Criteria

- [ ] `PULSE_LATENCY_MSEC` only set when `/proc/version` contains "microsoft"/"WSL"
- [ ] `BufferSize::Fixed(4096)` only used when `is_wsl2()` returns true
- [ ] `cargo build` passes with no errors
- [ ] Comment clearly explains WHY each setting is WSL2-specific
