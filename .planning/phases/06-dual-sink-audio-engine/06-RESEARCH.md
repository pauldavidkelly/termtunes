# Phase 6: Dual-Sink Audio Engine - Research

**Researched:** 2026-02-10
**Domain:** Rodio dual-sink concurrent audio playback with volume budget and manual looping
**Confidence:** HIGH

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

#### Mixing approach
- Use rodio's SpatialSink for software mixing - simpler code, single output stream
- Enforce volume budget (main + ambient <= 1.0) to prevent clipping
- Budget enforcement happens on every volume change (dynamic adjustment)
- When volumes would exceed budget: auto-scale both proportionally to fit
- Share single OutputStream between both sinks (never create second OutputStream)

#### Loop behavior
- Brief silence between loops is acceptable - prioritize memory safety over gapless playback
- Avoid rodio `repeat_infinite()` due to confirmed memory leak
- Use manual re-append loop when track ends

#### Volume architecture
- Structure: Independent sink volumes (main, ambient) + master volume
- Master volume applies AFTER budget enforcement (scales final output)
- Default ambient volume: 30% lower than main music volume
- Muting: Set volume to 0 (simple, no separate mute state needed)
- Volume budget enforced at sink level before master scaling

#### Failure handling
- Ambient decode/play failures: Log error, clear ambient state, keep main playing
- Best-effort isolation between channels (normal errors isolated, rodio mixer panics could affect both)
- OutputStream failures: Attempt recovery (recreate OutputStream and resume both channels)
- Resource exhaustion: Prioritize main music over ambient (drop ambient if resources tight)

### Claude's Discretion
- Exact loop validation duration (30+ min target, but can adjust for practical testing)
- Memory growth threshold during extended looping (suggest <5MB over 30min, or flat)
- Specific error logging format and detail level
- Recovery mechanism implementation details

### Deferred Ideas (OUT OF SCOPE)
None -- discussion stayed within phase scope.
(UI visibility for errors belongs in Phase 8: Ambient Status UI & Controls)
</user_constraints>

## Summary

Phase 6 adds a second independent audio channel (ambient track) to the existing single-channel music player. The core mechanism is creating a second rodio `Sink` connected to the same `OutputStream::mixer()` that the main music sink uses. Rodio's mixer performs simple additive sample mixing, which means a volume budget (main + ambient <= 1.0) is essential to prevent clipping. The ambient track loops via manual re-append (polling `sink.empty()` in the existing 100ms event loop) rather than rodio's `repeat_infinite()`, which has a confirmed open memory leak (issue #673, still unfixed as of April 2025).

The existing codebase already demonstrates every pattern needed: `Sink::connect_new(stream.mixer())` for creating sinks, `sink.empty()` polling for track completion, `replay_current()` for re-decoding from cached bytes, and per-sink `set_volume()` for volume control. The primary implementation risk is not technical novelty but rather regression -- refactoring the `Player` struct from single-sink to dual-sink without breaking any existing playback behavior.

**IMPORTANT CORRECTION:** The locked decision says "Use rodio's SpatialSink for software mixing." However, research reveals that `SpatialSink` is specifically for 3D positional audio (it takes emitter/ear coordinates as parameters). The correct approach for dual-channel simultaneous playback is two regular `Sink` instances on the same `OutputStream::mixer()`. This is what the prior v1.1 stack research (`.planning/research/STACK.md`) validated and recommended. The planner should use regular `Sink`, not `SpatialSink`, as the underlying intent (two independent sinks sharing one output stream) is correct even though the specific type name is wrong.

**Primary recommendation:** Create two `Sink` instances connected to the same `OutputStream::mixer()`, enforce a volume budget before mixing, loop ambient via `sink.empty()` polling + re-append from cached bytes, and validate WSL2 dual-sink audio quality as the first implementation step.

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| rodio | 0.21.1 | Audio playback, Sink management, mixer | Already in use; supports multiple sinks on one OutputStream natively |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| tracing | 0.1 | Error logging for ambient failures | Already in use; log ambient decode/play errors without crashing |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Two `Sink`s | `Source::mix()` | `mix()` merges two sources into one with no independent volume/pause/stop control -- unacceptable for this use case |
| Two `Sink`s | `SpatialSink` | SpatialSink adds 3D positioning math overhead and requires emitter/ear coordinates; regular Sink is simpler and provides all needed controls |
| Manual re-append | `repeat_infinite()` | Confirmed memory leak (rodio #673, ~10MB/15s growth). Manual re-append has a ~100ms gap but is memory-stable |
| Two `Sink`s | Separate `OutputStream` per channel | Would open two ALSA devices; fails on WSL2 where PulseAudio provides single default sink |

**Installation:**
```bash
# No new dependencies needed. Existing Cargo.toml is sufficient.
```

## Architecture Patterns

### Recommended Project Structure
```
src/
  player.rs     # Refactored: main_sink + ambient_sink on shared OutputStream
  app.rs        # Extended: ambient loop check, ambient download channel, volume budget
  main.rs       # Unchanged
  ui.rs         # Unchanged in Phase 6 (UI changes deferred to Phase 8)
  visualizer.rs # Unchanged (taps main channel only)
```

### Pattern 1: Dual Sink on Shared Mixer
**What:** Create two `Sink` instances from the same `OutputStream::mixer()` for simultaneous playback with independent control.
**When to use:** Any time two audio sources need independent volume, pause, and stop control while sharing one output device.
**Example:**
```rust
// Source: rodio docs (https://docs.rs/rodio/latest/rodio/) + existing player.rs pattern
let stream = OutputStreamBuilder::open_default_stream()?;

// Main music channel (existing)
let main_sink = Sink::connect_new(stream.mixer());

// Ambient channel (new)
let ambient_sink = Sink::connect_new(stream.mixer());

// Each sink has independent volume, pause, stop
main_sink.set_volume(0.7);
ambient_sink.set_volume(0.3);
// Combined output from mixer: 0.7 + 0.3 = 1.0 (no clipping)
```

### Pattern 2: Manual Loop via Empty Polling
**What:** Detect track completion with `sink.empty()` in the event loop and re-append from cached bytes.
**When to use:** For continuous looping without `repeat_infinite()`'s memory leak.
**Example:**
```rust
// Source: Existing replay_current() pattern in player.rs + rodio Sink::empty() docs
fn check_ambient_loop(&mut self) -> Result<()> {
    if let Some(player) = &mut self.player {
        if player.is_ambient_finished() && player.has_ambient_data() {
            // Re-decode from cached bytes, append to ambient sink
            player.replay_ambient(self.ambient_volume)?;
        }
    }
    Ok(())
}
```

### Pattern 3: Volume Budget Enforcement
**What:** Proportionally scale sink volumes so their sum never exceeds 1.0, then apply master volume.
**When to use:** On every volume change to either channel.
**Example:**
```rust
// Source: Audio engineering standard + rodio mixer analysis (simple additive mixing)
fn enforce_volume_budget(main_raw: f32, ambient_raw: f32, master: f32) -> (f32, f32) {
    let sum = main_raw + ambient_raw;
    let (main_budgeted, ambient_budgeted) = if sum > 1.0 {
        // Proportional scaling: preserve ratio, fit within budget
        let scale = 1.0 / sum;
        (main_raw * scale, ambient_raw * scale)
    } else {
        (main_raw, ambient_raw)
    };
    // Master volume scales final output
    (main_budgeted * master, ambient_budgeted * master)
}
```

### Pattern 4: Sink Recreation for Ambient (No Stop-Then-Append)
**What:** Create a fresh `Sink` when loading a new ambient track, rather than calling `stop()` on the existing one and appending.
**When to use:** When changing ambient tracks or restarting the loop.
**Example:**
```rust
// Source: Existing load_and_play() pattern in player.rs lines 140-145
// rodio issue #171: Sink::stop() makes the sink unusable for new appends
fn load_ambient(&mut self, audio_bytes: Vec<u8>, track_name: String, volume: f32) -> Result<()> {
    // Stop old ambient sink (if any)
    if let Some(ref sink) = self.ambient_sink {
        sink.stop();
    }
    // Create fresh sink on same mixer
    let new_sink = Sink::connect_new(self._stream.mixer());
    new_sink.set_volume(volume.clamp(0.0, 1.0));

    // Decode and append
    let cursor = Cursor::new(audio_bytes.clone());
    let source = Decoder::builder()
        .with_data(cursor)
        .build()?;
    new_sink.append(source);

    self.ambient_sink = Some(new_sink);
    self.ambient_audio_data = Some(audio_bytes);
    self.ambient_track_name = Some(track_name);
    Ok(())
}
```

### Anti-Patterns to Avoid
- **Using `repeat_infinite()` for looping:** Confirmed memory leak (rodio #673), grows ~10MB/15s. Use manual re-append instead.
- **Creating a second `OutputStream`:** Opens a second audio device, fails on WSL2. Both sinks MUST share one `OutputStream`.
- **Calling `stop()` on the ambient sink during main track changes:** `load_and_play()` calls `self.sink.stop()` -- after renaming to `self.main_sink`, ensure ambient_sink is never touched.
- **Setting individual sink volumes > combined budget:** Rodio mixer uses naive additive mixing. Two sinks at 0.8 each produce peaks up to 1.6, causing clipping.
- **Using `SpatialSink` instead of `Sink`:** SpatialSink is for 3D positional audio. It adds emitter/ear coordinate processing overhead that is unnecessary for simple dual-channel mixing.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Audio mixing | Custom mixer combining two audio streams | Two rodio `Sink`s on same `OutputStream::mixer()` | Rodio already mixes all sinks before sending to OS; adding custom mixing doubles the work |
| Loop detection | Custom Source wrapper with loop tracking | `sink.empty()` poll in 100ms event loop | Existing pattern (main track auto-advance uses `is_finished()` which checks `sink.empty()`) |
| Volume normalization | Custom limiter or dynamic range compressor | Volume budget enforcement at `set_volume()` level | Prevention (budget) is simpler and more reliable than correction (limiting) |
| Thread-safe audio state | Arc<Mutex<>> for every audio field | Rodio Sink is already internally thread-safe (Mutex on Controls) | Adding extra synchronization introduces deadlock risk |
| Ambient state machine | Complex Loading/Playing/Paused/Fading enum | `Option<Sink>` (None = no ambient, Some = ambient active) + `sink.is_paused()` | Rodio Sink already tracks play/pause state |

**Key insight:** Every capability needed for Phase 6 already exists in rodio's `Sink` API or the existing codebase's patterns. The implementation is assembly, not invention.

## Common Pitfalls

### Pitfall 1: Mixer Clipping from Additive Mixing
**What goes wrong:** Rodio's mixer sums f32 samples from all sinks with no clipping prevention. Two sinks at volume 0.8 each produce combined peaks up to 1.6, causing harsh distortion at beat/peak moments.
**Why it happens:** The mixer source code (`mixer.rs`) does `sum += value` with no normalization, limiting, or clamping. This is standard mixer behavior -- the application must manage combined levels.
**How to avoid:** Enforce volume budget (main + ambient <= 1.0) at every `set_volume()` call. Proportional scaling when budget exceeded.
**Warning signs:** Crackling/distortion that only appears when both channels play simultaneously.

### Pitfall 2: OutputStream Drop Kills All Audio
**What goes wrong:** Dropping the `OutputStream` immediately and silently kills ALL audio on ALL sinks. No error, no warning.
**Why it happens:** `OutputStream` owns the OS audio device connection. All sinks connected to its mixer share this connection. The existing code stores it as `_stream` (underscore prefix = kept alive).
**How to avoid:** Keep ONE `OutputStream` for the entire application. Both main and ambient sinks created from `_stream.mixer()`. Never restructure in a way that drops `_stream`.
**Warning signs:** Complete silence after a code change, no error messages.

### Pitfall 3: repeat_infinite() Memory Leak
**What goes wrong:** Memory grows ~10MB every 15 seconds, eventually consuming hundreds of MB.
**Why it happens:** Bug in rodio's `Buffered` type clone implementation used by `repeat_infinite()`. Acknowledged by maintainers but unfixed (issue #673, open since Jan 2025, still open April 2025).
**How to avoid:** Never use `repeat_infinite()`. Use manual `sink.empty()` polling + re-append from cached bytes.
**Warning signs:** RSS growth correlated with ambient loop iterations.

### Pitfall 4: Breaking Existing Playback During Refactor
**What goes wrong:** All existing Player methods reference `self.sink`. After adding ambient_sink, wrong sink gets paused/stopped/volume-changed.
**Why it happens:** Single-sink assumptions are embedded throughout. `load_and_play()` calls `self.sink.stop()`, `toggle_pause()` calls `self.sink.is_paused()`, etc.
**How to avoid:** Rename `self.sink` to `self.main_sink` in a dedicated commit BEFORE adding any ambient logic. Review every `self.sink` -> `self.main_sink` replacement. Keep ambient methods completely separate.
**Warning signs:** Any behavior change in existing playback after refactor.

### Pitfall 5: Accidental Ambient Stop During Main Track Change
**What goes wrong:** `load_and_play()` creates a fresh main_sink via `Sink::connect_new()`. If this inadvertently touches the ambient sink, ambient goes silent on every track change.
**Why it happens:** Rodio `Sink::stop()` is a one-way operation -- stopped sinks cannot accept new sources.
**How to avoid:** `load_and_play()` must ONLY operate on `self.main_sink`. Ambient sink is never touched by main playback methods.
**Warning signs:** Ambient goes silent every time main track changes.

### Pitfall 6: Sink Stop-Then-Append Blocking
**What goes wrong:** After calling `sink.stop()`, appending a new source blocks the thread until the internal queue flushes (calls `sleep_until_end()`).
**Why it happens:** Documented rodio behavior -- `stop()` sets a flag but the queue flush is deferred. `append()` after `stop()` blocks until flush completes.
**How to avoid:** Create a fresh `Sink` via `Sink::connect_new()` instead of reusing a stopped sink. This is already the established pattern in the existing codebase (see `load_and_play()` and `replay_current()`).
**Warning signs:** UI freezes briefly when changing tracks.

## Code Examples

Verified patterns from the existing codebase and rodio documentation:

### Creating a Second Sink on the Same Mixer
```rust
// Source: rodio docs (https://docs.rs/rodio/latest/rodio/) + existing player.rs:87,145
// Pattern already proven in existing code: Sink::connect_new(self._stream.mixer())
let ambient_sink = Sink::connect_new(self._stream.mixer());
ambient_sink.set_volume(0.3); // Independent volume
```

### Detecting Track Completion for Looping
```rust
// Source: Existing app.rs:404-415 (auto-advance pattern)
// The event loop already runs at 100ms ticks, checking for track completion.
// Same pattern for ambient loop detection:
if let Some(player) = &self.player {
    if player.is_ambient_finished() && player.has_ambient_data() {
        player.replay_ambient(self.ambient_volume)?;
    }
}
```

### Re-Decoding from Cached Bytes (Loop Mechanism)
```rust
// Source: Existing player.rs:280-315 (replay_current method)
// Same pattern adapted for ambient -- re-decode from cached bytes, append to fresh sink
fn replay_ambient(&mut self, volume: f32) -> Result<()> {
    let audio_bytes = self.ambient_audio_data.clone()
        .ok_or_else(|| eyre!("No ambient audio data to replay"))?;

    // Stop old ambient sink and create fresh one
    if let Some(ref sink) = self.ambient_sink {
        sink.stop();
    }
    let new_sink = Sink::connect_new(self._stream.mixer());
    new_sink.set_volume(volume.clamp(0.0, 1.0));

    let cursor = Cursor::new(audio_bytes);
    let source = Decoder::builder()
        .with_data(cursor)
        .build()?;
    new_sink.append(source);

    self.ambient_sink = Some(new_sink);
    Ok(())
}
```

### Volume Budget Enforcement (Proportional Scaling)
```rust
// Source: Audio engineering standard; validated by rodio mixer.rs analysis
// (mixer.rs confirmed: naive sum += value, no clipping prevention)
fn apply_volume_budget(&mut self) {
    let sum = self.main_volume + self.ambient_volume;
    let (main_effective, ambient_effective) = if sum > 1.0 {
        let scale = 1.0 / sum;
        (self.main_volume * scale, self.ambient_volume * scale)
    } else {
        (self.main_volume, self.ambient_volume)
    };

    // Apply master volume after budget enforcement
    let main_final = main_effective * self.master_volume;
    let ambient_final = ambient_effective * self.master_volume;

    self.main_sink.set_volume(main_final);
    if let Some(ref sink) = self.ambient_sink {
        sink.set_volume(ambient_final);
    }
}
```

### Failure-Isolated Ambient Loading
```rust
// Source: Existing error handling pattern in app.rs:464-480 (Player::new error handling)
fn load_ambient_track(&mut self, audio_bytes: Vec<u8>, track_name: String) {
    if let Some(player) = &mut self.player {
        match player.load_ambient(audio_bytes, track_name, self.ambient_volume) {
            Ok(()) => {
                tracing::info!("Ambient track loaded successfully");
                self.error_message = None;
            }
            Err(e) => {
                // Isolate failure: log, clear ambient state, keep main playing
                tracing::error!("Failed to load ambient track: {}", e);
                player.stop_ambient();
                // Main music continues unaffected
            }
        }
    }
}
```

### Expanded Player Struct
```rust
// Source: Existing player.rs struct + v1.1 architecture research (.planning/research/ARCHITECTURE.md)
pub struct Player {
    /// Output stream -- CRITICAL: dropping this kills ALL audio.
    /// Both main_sink and ambient_sink depend on this.
    _stream: OutputStream,

    // Main channel (renamed from `sink`)
    main_sink: Sink,
    _audio_data: Option<Vec<u8>>,
    current_track: Option<String>,

    // Ambient channel (new)
    ambient_sink: Option<Sink>,
    ambient_audio_data: Option<Vec<u8>>,
    ambient_track_name: Option<String>,
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| `Sink::try_new(handle)` | `Sink::connect_new(mixer)` | rodio 0.21.0 (2025-07-12) | API changed; project already on 0.21.1 |
| `OutputStreamHandle` | `OutputStream::mixer()` | rodio 0.21.0 | Handle removed; mixer accessed directly from OutputStream |
| `DynamicMixerController` | `Mixer` | rodio 0.21.0 | Renamed for clarity |
| No built-in limiter | `Source::limit()` | rodio 0.21.0 | Available but volume budget is preferred (prevention > correction) |
| Linear volume only | `amplify_normalized()` | rodio 0.21.0 | Perceptual volume scaling in 0.0..1.0 range; could replace raw `set_volume()` |

**Deprecated/outdated:**
- `repeat_infinite()`: Technically not deprecated, but has a confirmed unfixed memory leak (#673). Must not be used for long-running loops.
- `OutputStreamHandle`: Removed in rodio 0.21.0. Use `OutputStream::mixer()` directly.

## Discretion Recommendations

### Loop Validation Duration
**Recommendation:** Test for 10 minutes during development, validate for 30+ minutes in verification.
**Rationale:** 10 minutes is sufficient to detect memory leaks (the rodio #673 leak grows 10MB/15s, so 10 min would show ~400MB growth). If stable at 10 min, 30 min will also be stable since the manual re-append approach creates no persistent allocations between loops.

### Memory Growth Threshold
**Recommendation:** RSS growth < 2MB over 30 minutes of continuous ambient looping.
**Rationale:** Each loop iteration re-decodes the audio (temporary allocation) then the old decoded data is dropped. The only persistent allocation is the cached compressed bytes (`ambient_audio_data`). A typical ambient track is 3-10MB compressed. RSS should be flat after the initial load. Allow 2MB for allocator fragmentation and Rust runtime overhead.
**Measurement:** Read `/proc/self/status` VmRSS at start and end of test period.

### Error Logging Format
**Recommendation:** Use the existing tracing structured logging pattern with ambient-specific fields.
```rust
tracing::error!(
    channel = "ambient",
    track = %track_name,
    "Failed to decode ambient track: {}", error
);
tracing::info!(
    channel = "ambient",
    loop_count = loop_iteration,
    "Ambient track loop restarted"
);
```
**Rationale:** Consistent with existing logging in player.rs and app.rs. The `channel = "ambient"` field allows filtering ambient-specific logs.

### Recovery Mechanism
**Recommendation:** For OutputStream failures, attempt a single recovery by recreating the OutputStream and both sinks.
```rust
fn attempt_stream_recovery(&mut self) -> Result<()> {
    let new_stream = OutputStreamBuilder::open_default_stream()?;
    let new_main_sink = Sink::connect_new(new_stream.mixer());
    // Restore main playback from cached data if available
    // Restore ambient from cached data if available
    self._stream = new_stream;
    self.main_sink = new_main_sink;
    // ... restore ambient sink similarly
    Ok(())
}
```
**Rationale:** OutputStream failures are rare (typically only on audio device disconnect/reconnect). A single retry is pragmatic. If recovery fails, log the error and continue without audio -- the user can restart the app. Do not retry in a loop (could cause rapid resource churn).

## Open Questions

1. **WSL2 dual-sink audio quality**
   - What we know: Rodio mixes internally, sending one stream to PulseAudio. From the OS perspective, it is still a single audio stream regardless of sink count.
   - What's unclear: Whether the additional CPU overhead of mixing two sinks introduces scheduling jitter that triggers buffer underruns on WSL2's PulseAudio bridge.
   - Recommendation: The very first implementation step should be a smoke test: two sinks playing simultaneously on WSL2 for 60 seconds at volume 0.5 each. If crackling occurs, increase PULSE_LATENCY_MSEC before proceeding. This is the fail-fast gate.

2. **`amplify_normalized()` vs `set_volume()` for perceptual volume control**
   - What we know: rodio 0.21 added `amplify_normalized()` which provides perceptual (logarithmic) volume scaling in the 0.0..1.0 range. The existing code uses linear `set_volume()`.
   - What's unclear: Whether switching to perceptual scaling would make the volume budget behave more intuitively (e.g., 0.5 would feel like "half volume" rather than being ~70% of perceived loudness).
   - Recommendation: Use `set_volume()` for Phase 6 (matches existing pattern, budget math is simpler with linear values). Consider `amplify_normalized()` as a Phase 8 polish item if volume controls feel non-intuitive.

3. **Master volume interaction with saved_volume**
   - What we know: The existing `saved_volume` field on App persists the main channel volume. The CONTEXT decision adds a "master volume" that scales final output after budget enforcement.
   - What's unclear: Whether `saved_volume` should remain as "main channel raw volume" or become "master volume," with a new field for main channel raw volume.
   - Recommendation: Keep `saved_volume` as main channel raw volume. Add `master_volume: f32` (default 1.0) and `ambient_volume: f32` (default 0.3) as new App fields. This preserves backward compatibility with existing session files and keeps the mental model simple: +/- controls main, new keybindings control ambient, master is advanced/later.

## Sources

### Primary (HIGH confidence)
- [rodio docs.rs main page](https://docs.rs/rodio/latest/rodio/) -- "multiple Sinks play simultaneously, all mixed by rodio"
- [rodio Sink docs](https://docs.rs/rodio/latest/rodio/struct.Sink.html) -- connect_new(), set_volume(), empty(), stop() behavior
- [rodio SpatialSink docs](https://docs.rs/rodio/0.21.1/rodio/struct.SpatialSink.html) -- confirmed: SpatialSink is for 3D positional audio, not general mixing
- [rodio mixer.rs source](https://github.com/RustAudio/rodio/blob/master/src/mixer.rs) -- confirmed: naive `sum += value` with no clipping prevention
- [rodio sink.rs source](https://docs.rs/rodio/latest/src/rodio/sink.rs.html) -- confirmed: stop() then append() blocks via sleep_until_end()
- [rodio CHANGELOG.md](https://github.com/RustAudio/rodio/blob/master/CHANGELOG.md) -- v0.21.0 (2025-07-12): OutputStreamBuilder, Mixer rename, Source::limit(), amplify_normalized()
- [rodio issue #673: repeat_infinite memory leak](https://github.com/RustAudio/rodio/issues/673) -- confirmed open, ~10MB/15s growth, no fix, maintainer acknowledged
- [rodio issue #171: cannot restart stopped sink](https://github.com/RustAudio/rodio/issues/171) -- confirmed: stop() is one-way, create fresh Sink
- Existing codebase: `player.rs`, `app.rs`, `visualizer.rs` -- all patterns verified against source code

### Secondary (MEDIUM confidence)
- [rodio issue #340: clipping with set_volume](https://github.com/RustAudio/rodio/issues/340) -- sample rate mismatch related, not directly volume; mixer clipping confirmed by source code analysis instead
- [.planning/research/STACK.md](file:///home/jigsaw/src/termtunes/.planning/research/STACK.md) -- prior v1.1 stack research recommending dual-Sink approach
- [.planning/research/ARCHITECTURE.md](file:///home/jigsaw/src/termtunes/.planning/research/ARCHITECTURE.md) -- prior v1.1 architecture research with Player struct changes
- [.planning/research/PITFALLS.md](file:///home/jigsaw/src/termtunes/.planning/research/PITFALLS.md) -- 12 pitfalls documented, all relevant to Phase 6

### Tertiary (LOW confidence)
- WSL2 dual-sink audio quality: Extrapolated from single-stream WSL2 tuning. Needs empirical validation (smoke test).

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - rodio 0.21.1 already in use, multi-sink capability verified in docs and source code
- Architecture: HIGH - dual-Sink pattern verified in rodio docs, prior research, and existing codebase patterns
- Pitfalls: HIGH - all pitfalls verified against rodio source code, GitHub issues, or existing codebase analysis
- Volume budget: HIGH - rodio mixer.rs source confirmed naive additive mixing with no clipping prevention
- Memory leak avoidance: HIGH - rodio #673 confirmed open and unfixed; manual re-append pattern proven in existing replay_current()
- WSL2 dual-sink quality: LOW - needs empirical validation; theoretical analysis suggests it should work (single stream to PulseAudio)

**SpatialSink correction:** The CONTEXT.md locked decision mentions "SpatialSink" but research confirms this is the wrong type. `SpatialSink` is for 3D positional audio. Regular `Sink` is the correct choice. The underlying intent (two independent sinks sharing one output stream with software mixing) is correct. The planner should use `Sink`, not `SpatialSink`.

**Research date:** 2026-02-10
**Valid until:** 2026-03-10 (rodio is stable, patterns unlikely to change)
