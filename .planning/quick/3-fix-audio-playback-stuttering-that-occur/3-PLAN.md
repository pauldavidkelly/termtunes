---
phase: quick-3
plan: 01
type: execute
wave: 1
depends_on: []
files_modified:
  - src/player.rs
  - src/main.rs
autonomous: true
requirements: [AUDIO-STUTTER-FIX]

must_haves:
  truths:
    - "Audio stream opens with explicit 4096-sample buffer size on WSL2"
    - "PULSE_LATENCY_MSEC is set to 300 before any audio initialization"
    - "Audio stream errors are logged via tracing instead of hidden by TUI"
    - "Stream creation falls back to default if explicit buffer size fails"
    - "Misleading .asoundrc comments about buffer_size are corrected"
  artifacts:
    - path: "src/player.rs"
      provides: "Explicit buffer size, error callback, corrected comments"
      contains: "BufferSize::Fixed(4096)"
    - path: "src/main.rs"
      provides: "Increased PulseAudio latency"
      contains: "PULSE_LATENCY_MSEC.*300"
  key_links:
    - from: "src/player.rs"
      to: "cpal::BufferSize"
      via: "OutputStreamBuilder::with_buffer_size"
      pattern: "with_buffer_size.*BufferSize::Fixed"
    - from: "src/player.rs"
      to: "tracing"
      via: "with_error_callback"
      pattern: "with_error_callback.*tracing::warn"
---

<objective>
Fix audio playback stuttering that occurs ~20 seconds into playback on WSL2.

Purpose: The root cause is WSLg PulseAudio bridge degradation (known platform issue). While this cannot be fully fixed at the application level, increasing the cpal buffer size from default to 4096 samples and raising PULSE_LATENCY_MSEC from 150 to 300 gives the audio pipeline enough headroom to absorb WSL2 scheduling jitter. Additionally, routing stream error callbacks through tracing (instead of stderr, which is hidden by the TUI) enables diagnosing future underruns.

Output: Modified src/player.rs and src/main.rs with buffer tuning, error logging, and corrected comments.
</objective>

<execution_context>
@/home/jigsaw/.claude/get-shit-done/workflows/execute-plan.md
@/home/jigsaw/.claude/get-shit-done/templates/summary.md
</execution_context>

<context>
@.planning/STATE.md
@.planning/quick/3-fix-audio-playback-stuttering-that-occur/DEBUG-INVESTIGATION.md
@src/player.rs
@src/main.rs
</context>

<tasks>

<task type="auto">
  <name>Task 1: Increase PULSE_LATENCY_MSEC to 300</name>
  <files>src/main.rs</files>
  <action>
In `src/main.rs` line 55, change the PULSE_LATENCY_MSEC value from "150" to "300".

Update the comment block above it (lines 47-54) to reflect the new value. Change "150ms provides enough buffer" to "300ms provides enough buffer" and note that 150ms was found insufficient for sustained playback on WSL2 (stuttering after ~20 seconds).

The line to change:
```rust
unsafe { std::env::set_var("PULSE_LATENCY_MSEC", "300") };
```
  </action>
  <verify>`grep 'PULSE_LATENCY_MSEC.*300' src/main.rs` returns a match</verify>
  <done>PULSE_LATENCY_MSEC is set to "300" with updated comment explaining why 150ms was insufficient</done>
</task>

<task type="auto">
  <name>Task 2: Replace open_default_stream with explicit buffer size and error callback</name>
  <files>src/player.rs</files>
  <action>
In `src/player.rs`, replace the stream creation at line 60 inside `Player::new()`.

**Current code (line 60-95):**
```rust
let stream = OutputStreamBuilder::open_default_stream().map_err(|e| {
    // ... WSL2 error handling ...
})?;
```

**Replace with** a builder chain that sets an explicit buffer size and error callback, with fallback to default if the explicit configuration fails. The full replacement for lines 60-95:

```rust
let stream = OutputStreamBuilder::from_default_device()
    .and_then(|builder| {
        builder
            .with_buffer_size(cpal::BufferSize::Fixed(4096))
            .with_error_callback(|err| {
                tracing::warn!("Audio stream error (possible underrun): {err}");
            })
            .open_stream()
    })
    .or_else(|_| {
        tracing::info!("Explicit buffer config failed, falling back to default stream");
        OutputStreamBuilder::open_default_stream()
    })
    .map_err(|e| {
        let msg = format!("Failed to open audio output: {}", e);
        if is_wsl2() {
            // Provide WSL2-specific diagnostics
            let has_plugin = alsa_pulse_plugin_exists();
            let has_socket = std::path::Path::new("/mnt/wslg/PulseServer").exists();
            let pulse_server = std::env::var("PULSE_SERVER").unwrap_or_default();

            tracing::error!(
                has_alsa_pulse_plugin = has_plugin,
                has_wslg_socket = has_socket,
                pulse_server = %pulse_server,
                "WSL2 audio device initialization failed"
            );

            if !has_plugin {
                color_eyre::eyre::eyre!(
                    "{}\n\nWSL2 audio requires the ALSA PulseAudio plugin.\n\
                     Install it with: sudo apt-get install -y libasound2-plugins\n\
                     Then restart the application.",
                    msg
                )
            } else if !has_socket {
                color_eyre::eyre::eyre!(
                    "{}\n\nWSLg PulseAudio socket not found at /mnt/wslg/PulseServer.\n\
                     WSLg may not be running. Try restarting WSL with: wsl --shutdown\n\
                     Then reopen your terminal and try again.",
                    msg
                )
            } else {
                color_eyre::eyre::eyre!("{}", msg)
            }
        } else {
            color_eyre::eyre::eyre!("{}", msg)
        }
    })?;
```

Key points:
- `cpal::BufferSize::Fixed(4096)` sets a stability-focused buffer (4096 samples at 44100 Hz = ~93ms). This is within rodio's recommended range for "background music, non-interactive" use.
- `with_error_callback` routes stream errors to tracing instead of stderr (which is hidden by the TUI alternate screen). This will capture buffer underrun events in the log file.
- The `.or_else(|_| OutputStreamBuilder::open_default_stream())` fallback ensures the app still works if the explicit buffer size is rejected by the audio backend.
- All existing WSL2 diagnostic error handling is preserved in the final `.map_err()`.
- No new imports needed: `cpal` is already a dependency and `cpal::BufferSize` is accessible through it. Check that `cpal` is in the existing use statements -- if not, add `use cpal::BufferSize;` or use the fully qualified `cpal::BufferSize::Fixed(4096)`.

Also fix the misleading `.asoundrc` comments at lines 560-568. Replace the current comment block:
```rust
// ALSA configuration that routes audio through PulseAudio with buffer
// sizes tuned for WSL2's WSLg PulseAudio bridge. Without explicit
// buffer tuning, the default ALSA period/buffer sizes are too small
// for the WSL2 PulseAudio shim, causing audible crackling and
// clicking artifacts (buffer underruns).
//
// buffer_size 8192 (4 periods of 2048 frames) at 44100 Hz gives
// ~186ms of buffer which is large enough to absorb WSL2 scheduling
// jitter without perceptible latency for music playback.
```

With this corrected version:
```rust
// ALSA configuration that routes audio through PulseAudio for WSL2's
// WSLg audio bridge. The `type pulse` PCM plugin does not accept
// buffer_size/period_size parameters directly -- PulseAudio buffer
// tuning is handled via PULSE_LATENCY_MSEC (set in main.rs) and
// the cpal buffer size (set in Player::new via OutputStreamBuilder).
```
  </action>
  <verify>
Run `cargo check` to verify the code compiles without errors.
Run `cargo clippy` to ensure no new warnings.
Verify with `grep -n 'BufferSize::Fixed(4096)' src/player.rs` and `grep -n 'with_error_callback' src/player.rs`.
  </verify>
  <done>
Player::new() opens stream with explicit 4096-sample buffer and tracing error callback, falls back to default on failure. Misleading .asoundrc comments corrected to explain that buffer tuning is via PULSE_LATENCY_MSEC and cpal, not ALSA config parameters.
  </done>
</task>

</tasks>

<verification>
1. `cargo check` passes -- code compiles
2. `cargo clippy` has no new warnings
3. `grep 'PULSE_LATENCY_MSEC.*300' src/main.rs` -- confirms increased latency
4. `grep 'BufferSize::Fixed(4096)' src/player.rs` -- confirms explicit buffer
5. `grep 'with_error_callback' src/player.rs` -- confirms error logging
6. `grep -c 'buffer_size 8192' src/player.rs` returns 0 -- misleading comment removed
</verification>

<success_criteria>
- Audio stream opens with BufferSize::Fixed(4096) and falls back to default if that fails
- PULSE_LATENCY_MSEC set to 300 (up from 150)
- Stream errors routed through tracing (visible in log file, not hidden by TUI)
- Misleading .asoundrc buffer comments replaced with accurate explanation
- Code compiles cleanly (cargo check + cargo clippy)
</success_criteria>

<output>
After completion, create `.planning/quick/3-fix-audio-playback-stuttering-that-occur/3-SUMMARY.md`
</output>
