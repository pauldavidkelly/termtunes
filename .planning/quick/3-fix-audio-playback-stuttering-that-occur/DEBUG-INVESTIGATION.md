# Audio Playback Stuttering Investigation

**Symptom:** After ~20 seconds of playback (main or ambient), audio starts stuttering -- a note or two plays every 2-3 seconds. Persists after machine restart and release rebuild.

**Environment:** WSL2, Linux 6.6.87.2-microsoft-standard-WSL2, rodio 0.21.1, cpal 0.16.0, symphonia 0.5.5

---

## Architecture Overview

### Audio pipeline
```
User track (Vec<u8> in memory)
  -> Decoder (symphonia, Cursor<Vec<u8>>)
  -> VisualizerSource (tap: copies samples to shared buffer via try_lock)
  -> UniformSourceIterator (sample rate + channel conversion for mixer)
  -> MixerSource (sums all active sources per-sample)
  -> cpal audio callback (OS audio thread, writes to ALSA PCM buffer)
  -> ALSA PulseAudio plugin (pcm type pulse in .asoundrc)
  -> WSLg PulseAudio bridge (/mnt/wslg/PulseServer)
  -> Windows audio system
```

### Threading model
- **Main thread:** tokio async runtime running `App::run()` event loop
  - `crossterm::event::poll(100ms)` -- blocking poll for terminal input
  - Calls `update_visualizer()` each iteration (acquires Mutex lock, clones 2048 f32s, releases lock, then does FFT -- lock NOT held during FFT)
  - Checks download channels (try_recv, non-blocking)
  - Checks ambient loop state
  - Renders UI via ratatui
- **cpal audio thread:** Single OS-level thread running audio callback
  - Calls `MixerSource::next()` per sample
  - `MixerSource` iterates all active sources: calls each source's `.next()`
  - VisualizerSource uses `try_lock()` -- never blocks the audio thread
  - Acquires `pending_sources` Mutex briefly when `has_pending` is true
- **Download threads:** Spawned per track download via `std::thread::spawn`
  - Use `reqwest::blocking::get()` -- fully isolated from audio thread
  - Communicate results via `std::sync::mpsc` channels (try_recv on main thread)

### Key configuration
- `PULSE_LATENCY_MSEC=150` set before any audio stream creation (main.rs:55)
- `.asoundrc` sets `pcm.!default { type pulse }` (no buffer_size/period_size params)
- `OutputStreamBuilder::open_default_stream()` uses `BufferSize::Default` (no explicit buffer)

---

## Areas Investigated

### 1. Event loop blocking the audio thread -- ELIMINATED

**Hypothesis:** The main event loop (crossterm poll, UI rendering, FFT) could starve the audio thread.

**Evidence against:**
- cpal's audio callback runs on a **separate OS thread**, not a tokio task. The tokio runtime blocking cannot starve it.
- `crossterm::event::poll(100ms)` blocks the main thread, not the audio thread.
- `VisualizerSource::next()` uses `try_lock()` (line visualizer.rs:108) -- it **never blocks** the audio thread. If the UI thread holds the Mutex, the audio thread simply skips that one sample (inaudible at 44100 Hz).
- `compute_spectrum_bars()` holds the Mutex only during a `Vec::clone()` of 2048 f32 values (~8KB) -- this completes in microseconds.
- FFT computation happens AFTER releasing the Mutex lock (visualizer.rs:174-190).

**Verdict:** The audio thread and main thread are properly isolated. The main thread cannot cause audio stuttering through thread starvation.

### 2. Sink/Mixer resource accumulation -- ELIMINATED

**Hypothesis:** Creating new Sinks (via `Sink::connect_new`) without properly cleaning up old ones could accumulate mixer sources.

**Evidence against:**
- Examining rodio's `Sink::connect_new` (sink.rs:72-76): it creates a new queue and adds the output to the mixer. The old Sink is dropped when `self.main_sink = Sink::connect_new(...)` replaces it.
- Dropping a Sink sets `Controls.stopped = true` and drops the `queue_tx` Arc, which signals the queue output to stop producing samples.
- In `MixerSource::sum_current_sources()` (mixer.rs:197-208), sources that return `None` from `.next()` are removed from `current_sources` (they're not pushed to `still_current`).
- The pattern of stop() + replace is used consistently for both main and ambient sinks.

**Verdict:** Old sinks are properly cleaned up. No mixer source accumulation.

### 3. Memory growth from audio data caching -- ELIMINATED

**Hypothesis:** Storing `_audio_data` and `ambient_audio_data` as `Vec<u8>` could cause memory pressure.

**Evidence against:**
- Only one main track and one ambient track are cached at a time.
- Old data is replaced (not appended) when new tracks load.
- `audio_bytes.clone()` in `load_and_play` creates a temporary copy, but the original is stored and the Cursor wraps a clone. After decoding starts, the Cursor's copy is consumed.
- Typical audio files are 3-10MB. Two cached files = 6-20MB total. This is negligible.

**Verdict:** Memory usage from audio caching is bounded and small.

### 4. Decoder performance in audio callback -- LOW RISK

**Hypothesis:** symphonia's decoder doing heavy work inside the audio callback (called per-sample) could cause latency spikes.

**Evidence:**
- The decoder reads from a `Cursor<Vec<u8>>` (in-memory), not from disk/network. No I/O latency.
- symphonia decodes in chunks (packets/frames), not per-sample. The rodio source adapter buffers decoded samples.
- symphonia performance is reported as +/-15% of FFmpeg -- fast enough for realtime.
- If the decoder were the bottleneck, stuttering would start immediately, not after 20 seconds.

**Verdict:** Unlikely root cause. The 20-second delay before stuttering starts rules out steady-state decoder performance as the cause.

### 5. Missing ALSA buffer configuration -- CONTRIBUTING FACTOR

**Hypothesis:** The `.asoundrc` config lacks explicit buffer size parameters despite comments claiming "buffer sizes tuned."

**Evidence:**
- The generated `.asoundrc` (player.rs:569-592) contains ONLY:
  ```
  pcm.!default {
      type pulse
      fallback "sysdefault"
      hint { ... }
  }
  ctl.!default {
      type pulse
      fallback "sysdefault"
  }
  ```
- The comment at line 560-568 describes "buffer_size 8192 (4 periods of 2048 frames)" but **these parameters are NOT present in the actual config**.
- The `type pulse` ALSA PCM plugin does NOT support `buffer_size` or `period_size` parameters directly -- PulseAudio buffer tuning is done via `PULSE_LATENCY_MSEC` env var or PulseAudio daemon config.
- `PULSE_LATENCY_MSEC=150` IS set at startup (main.rs:55), which provides 150ms of client-side buffering.

**Verdict:** The misleading comments are a documentation issue, not a code bug. The actual buffer tuning relies on `PULSE_LATENCY_MSEC=150` which IS correctly configured. However, 150ms may not be sufficient for WSL2's scheduling jitter under sustained load.

### 6. cpal default buffer size -- SIGNIFICANT FINDING

**Hypothesis:** Using `BufferSize::Default` in `OutputStreamBuilder::open_default_stream()` may result in a buffer that's too small for WSL2.

**Evidence:**
- `open_default_stream()` uses `OutputStreamConfig::default()` which sets `buffer_size: BufferSize::Default` (stream.rs:93).
- With `BufferSize::Default`, cpal lets the ALSA backend choose. On WSL2 with the PulseAudio ALSA plugin, this may select a small buffer.
- rodio's documentation for `with_buffer_size()` (stream.rs:242-279) specifically recommends:
  - **Stability-focused (background music, non-interactive): 2048-4096**
  - A small buffer might cause "Playback interruptions such as buffer underruns" and "Rodio to log errors like: `alsa::poll() returned POLLERR`"
- The code does NOT call `with_buffer_size()` to set an explicit buffer.

**Verdict:** This is a likely contributing factor. WSL2's PulseAudio bridge introduces extra latency/jitter that the default ALSA buffer may not accommodate. An explicit `BufferSize::Fixed(4096)` or higher could help.

### 7. WSLg PulseAudio bridge -- PRIMARY SUSPECT

**Hypothesis:** The stuttering is a known WSLg issue where PulseAudio audio degrades after a period of playback.

**Evidence:**
- Multiple WSLg GitHub issues document identical symptoms:
  - [Issue #908](https://github.com/microsoft/wslg/issues/908): "very choppy audio playback... sound can be heard every other second or so"
  - [Issue #1257](https://github.com/microsoft/wslg/issues/1257): "Sound stuttering" with VLC
  - [Issue #1342](https://github.com/microsoft/wslg/issues/1342): "sound lags extremely badly"
  - [Issue #684](https://github.com/microsoft/wslg/issues/684): "Improve Realtime Audio performance"
- Reports specifically mention "fragmented sounds starting 10-20 seconds after starting audio playback" -- this matches the user's ~20 second onset almost exactly.
- The WSLg audio path is: app -> ALSA -> PulseAudio plugin -> Unix socket -> WSLg PulseAudio server -> TCP/shared memory -> Windows PulseAudio -> Windows audio drivers. This multi-hop path is inherently fragile.
- WSLg's PulseAudio server runs in a container with limited resources and no realtime scheduling guarantees.
- The problem has been reported across many applications (VLC, Chromium, custom apps) and WSL versions, suggesting it's a platform-level issue, not application-specific.

**Verdict:** This is the most likely root cause. The WSLg PulseAudio bridge has documented issues with sustained audio playback that match the reported symptoms exactly.

---

## Root Cause Assessment

### Most Likely: WSLg PulseAudio Bridge Degradation (Confidence: HIGH)

The stuttering pattern (clean for ~20s, then periodic dropouts every 2-3s) matches known WSLg PulseAudio issues exactly. The WSLg audio bridge has documented problems with sustained playback that cause audio to become choppy after an initial period of normal operation. This affects all applications using audio on WSL2, not just termtunes.

### Contributing Factor: Default ALSA/cpal Buffer Size (Confidence: MEDIUM)

Using `BufferSize::Default` rather than an explicit larger buffer may make the application more susceptible to WSLg's scheduling jitter. A larger cpal buffer would give more headroom to absorb timing irregularities.

### Code is Otherwise Sound

The application's audio architecture is well-designed:
- Proper thread isolation (audio thread separate from UI)
- Non-blocking mutex usage on the audio thread (try_lock)
- Non-blocking event loop checks (try_recv)
- Background download threads don't interfere with playback
- Sink lifecycle management is correct (stop + replace, old sinks properly dropped)
- `PULSE_LATENCY_MSEC=150` is set before any audio initialization

---

## Recommended Fix Directions

### 1. Increase cpal buffer size (Application-level fix)

Replace `OutputStreamBuilder::open_default_stream()` with an explicit builder chain that sets a larger buffer:

```rust
// In Player::new()
let stream = OutputStreamBuilder::from_default_device()
    .and_then(|builder| {
        builder
            .with_buffer_size(cpal::BufferSize::Fixed(4096))
            .open_stream()
    })
    .or_else(|_| OutputStreamBuilder::open_default_stream())
    .map_err(|e| ...)?;
```

This gives the audio system a larger buffer (4096 samples at 44100 Hz = ~93ms additional latency) to absorb WSLg jitter. Falls back to default if the explicit size fails.

**Files:** `/home/jigsaw/src/termtunes/src/player.rs` (line 60)

### 2. Increase PULSE_LATENCY_MSEC (Quick experiment)

Try increasing from 150 to 300 or 500:
```rust
unsafe { std::env::set_var("PULSE_LATENCY_MSEC", "300") };
```

**Files:** `/home/jigsaw/src/termtunes/src/main.rs` (line 55)

### 3. Fix misleading .asoundrc comments

The comments describe buffer tuning that isn't present in the actual config. Either:
- Remove the buffer_size comments (since `type pulse` doesn't support them), OR
- Add a comment explaining that buffer tuning is via `PULSE_LATENCY_MSEC` instead

**Files:** `/home/jigsaw/src/termtunes/src/player.rs` (lines 560-568)

### 4. Add error callback for stream diagnostics

The current code uses the default error callback which prints to stderr (hidden by TUI). Adding a tracing-based callback would capture buffer underrun events in the log:

```rust
let stream = OutputStreamBuilder::from_default_device()?
    .with_buffer_size(cpal::BufferSize::Fixed(4096))
    .with_error_callback(|err| {
        tracing::warn!("Audio stream error: {}", err);
    })
    .open_stream()
    ...
```

**Files:** `/home/jigsaw/src/termtunes/src/player.rs` (line 60)

### 5. Platform documentation

Document that WSL2 audio has known limitations and suggest workarounds:
- Increasing `PULSE_LATENCY_MSEC` via environment variable before running
- Using PipeWire instead of PulseAudio if available
- Running on native Linux or using Windows-native audio for best quality

---

## Files Examined

| File | Lines | Role |
|------|-------|------|
| `/home/jigsaw/src/termtunes/src/player.rs` | 603 | Audio player: Sink management, decode, WSL2 ALSA config |
| `/home/jigsaw/src/termtunes/src/app.rs` | ~2200 | Main event loop, download management, visualizer updates |
| `/home/jigsaw/src/termtunes/src/visualizer.rs` | 337 | FFT visualization: VisualizerSource (audio thread tap), spectrum computation |
| `/home/jigsaw/src/termtunes/src/main.rs` | 157 | Startup: PULSE_LATENCY_MSEC, tokio runtime, WSL2 checks |
| `/home/jigsaw/src/termtunes/src/tui.rs` | 47 | Terminal restore, panic hooks, signal handlers |
| `/home/jigsaw/src/termtunes/src/ui.rs` | ~500 | UI rendering (no audio interaction) |
| `/home/jigsaw/src/termtunes/Cargo.toml` | 23 | Dependencies: rodio 0.21, cpal 0.16, symphonia 0.5.5 |
| rodio 0.21.1 `src/sink.rs` | ~100 | Sink::connect_new, Drop behavior |
| rodio 0.21.1 `src/mixer.rs` | 301 | Mixer: per-sample source iteration, source cleanup |
| rodio 0.21.1 `src/stream.rs` | 644 | OutputStream: cpal callback, BufferSize::Default, builder API |

## External References

- [WSLg Issue #908: Choppy sound](https://github.com/microsoft/wslg/issues/908)
- [WSLg Issue #1257: Sound stuttering](https://github.com/microsoft/wslg/issues/1257)
- [WSLg Issue #1342: Extreme audio lag](https://github.com/microsoft/wslg/issues/1342)
- [WSLg Issue #684: Improve Realtime Audio performance](https://github.com/microsoft/wslg/issues/684)
- [cpal Issue #446: Default buffer size strategy for ALSA](https://github.com/RustAudio/cpal/issues/446)
- [rodio OutputStreamBuilder docs](https://docs.rs/rodio/latest/rodio/stream/struct.OutputStreamBuilder.html)
- [rodio Issue #448: Buffer underrun then error flood/freeze](https://github.com/RustAudio/rodio/issues/448)
