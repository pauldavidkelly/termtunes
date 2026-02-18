---
phase: quick-4
plan: 01
type: execute
wave: 1
depends_on: []
files_modified:
  - src/player.rs
  - src/main.rs
autonomous: true
requirements: [QUICK-4]

must_haves:
  truths:
    - "Audio plays continuously for 5+ minutes without stopping on WSL2"
    - "Seeking forward/backward works correctly with pre-decoded PCM"
    - "Repeat One mode replays from pre-decoded samples without re-decoding"
    - "Ambient tracks play and loop correctly using pre-decoded PCM"
    - "Visualizer still displays spectrum analysis during playback"
  artifacts:
    - path: "src/player.rs"
      provides: "Pre-decode logic, SamplesBuffer playback, PCM-based seeking"
      contains: "SamplesBuffer"
    - path: "src/main.rs"
      provides: "Increased PULSE_LATENCY_MSEC"
      contains: "500"
  key_links:
    - from: "src/player.rs"
      to: "rodio::buffer::SamplesBuffer"
      via: "Pre-decoded PCM fed to Sink instead of Decoder"
      pattern: "SamplesBuffer::new"
    - from: "src/player.rs"
      to: "src/visualizer.rs"
      via: "VisualizerSource wraps SamplesBuffer"
      pattern: "VisualizerSource::new"
---

<objective>
Fix audio playback stopping after ~1 minute on WSL2 by pre-decoding audio to raw PCM samples before feeding to rodio sinks.

Purpose: The root cause is symphonia decoder running inside the cpal audio callback thread. WSL2 scheduling jitter causes the decoder to miss callback deadlines, leading to PulseAudio buffer underruns that kill the stream. Pre-decoding eliminates the decoder from the audio thread entirely -- the callback only does fast memory reads from a Vec<f32>.

Output: Modified src/player.rs with pre-decode + SamplesBuffer approach, and src/main.rs with PULSE_LATENCY_MSEC bumped to 500.
</objective>

<execution_context>
@/home/jigsaw/.claude/get-shit-done/workflows/execute-plan.md
@/home/jigsaw/.claude/get-shit-done/templates/summary.md
</execution_context>

<context>
@src/player.rs
@src/main.rs
@src/visualizer.rs
</context>

<tasks>

<task type="auto">
  <name>Task 1: Add pre-decode helper and convert main playback to SamplesBuffer</name>
  <files>src/player.rs</files>
  <action>
Add a private helper function `decode_to_pcm(audio_bytes: &[u8]) -> Result<(Vec<f32>, u16, u32)>` that:
1. Creates a `Cursor::new(audio_bytes.to_vec())` and builds a `Decoder` from it using `Decoder::builder().with_data(cursor).build()`
2. Reads the `channels()` and `sample_rate()` from the decoder BEFORE consuming it
3. Collects ALL samples via `.collect::<Vec<f32>>()` (the Decoder implements Iterator<Item=f32>)
4. Returns `(samples, channels, sample_rate)`
5. Logs the decoded sample count and duration at info level

Add a new struct field `decoded_pcm: Option<(Vec<f32>, u16, u32)>` to `Player` (stores the pre-decoded samples, channels, sample_rate for the current main track). Initialize to None in `Player::new()`.

Modify `load_and_play()`:
1. Call `decode_to_pcm(&audio_bytes)` to get `(samples, channels, sample_rate)`
2. Store `decoded_pcm = Some((samples.clone(), channels, sample_rate))`
3. Create `SamplesBuffer::new(channels, sample_rate, samples)` instead of using `Decoder`
4. Wrap with `VisualizerSource::new(samples_buffer, data, FFT_SIZE)` if visualizer data provided, otherwise append SamplesBuffer directly
5. Continue storing `_audio_data = Some(audio_bytes)` (compressed bytes still needed for re-download avoidance check)
6. Remove the Decoder builder code that used `with_byte_len`/`with_seekable` -- no longer needed

Update imports: add `use rodio::buffer::SamplesBuffer;` -- remove unused imports if Decoder is no longer used directly in load_and_play (but keep Decoder import since decode_to_pcm uses it).

Modify `replay_current()`:
1. Instead of re-decoding from `_audio_data` compressed bytes each time, clone from `decoded_pcm` which already has the raw PCM
2. Create `SamplesBuffer::new(channels, sample_rate, samples.clone())` from the cached decoded PCM
3. Wrap with VisualizerSource if visualizer data provided
4. Remove the Decoder builder code

Modify `seek_forward()` and `seek_backward()`:
1. Change both to take `&mut self` instead of `&self` (needed because we recreate the source from an offset)
2. In both methods, after calculating `target` Duration:
   - Read `decoded_pcm` to get `(samples, channels, sample_rate)`
   - Calculate the sample offset: `let offset = (target.as_secs_f64() * sample_rate as f64 * channels as f64) as usize`
   - Clamp offset to samples.len()
   - Align offset to channel boundary: `offset - (offset % channels as usize)`
   - Create new `SamplesBuffer::new(channels, sample_rate, samples[offset..].to_vec())`
   - Stop current main_sink, create fresh Sink, restore volume
   - Append the new SamplesBuffer (no visualizer wrap needed for seek -- the visualizer data shared buffer continues being written to by whatever source is playing)
   - Actually, DO wrap with VisualizerSource for seek too. Add a `visualizer_data: Option<Arc<Mutex<VisualizerData>>>` field to Player struct. Store it in `load_and_play` when provided. Then seek methods can re-wrap.
   - Return `Ok(())`
3. Update the return type from `Result<(), rodio::source::SeekError>` to `Result<()>` (using color_eyre) since we no longer use try_seek
4. If `decoded_pcm` is None, return an error

Add `visualizer_data: Option<Arc<Mutex<VisualizerData>>>` field to Player struct. Initialize to None. Set it in `load_and_play` when Some is passed. Use it in seek and replay methods.

IMPORTANT: The `&self` to `&mut self` change for seek methods will require updating callers in app.rs. The seek calls in app.rs at lines ~729 and ~739 use `&self.player` -- they need to change to `&mut self.player`. Check both seek_forward and seek_backward call sites. Also update the return type handling (they currently match on `rodio::source::SeekError`, change to handle `color_eyre::Result`).
  </action>
  <verify>
Run `cargo check` -- must compile with no errors. Key things to verify:
- SamplesBuffer import resolves
- decode_to_pcm returns correct tuple
- All methods using decoded_pcm handle the Option correctly
- seek methods signature change is compatible with app.rs callers
  </verify>
  <done>
Main track playback uses pre-decoded SamplesBuffer. Seeking recreates SamplesBuffer from offset. Replay uses cached decoded PCM. Visualizer still wraps the source. `cargo check` passes.
  </done>
</task>

<task type="auto">
  <name>Task 2: Convert ambient channel to SamplesBuffer and bump PULSE_LATENCY_MSEC</name>
  <files>src/player.rs, src/main.rs</files>
  <action>
In src/player.rs:

Add a new struct field `ambient_decoded_pcm: Option<(Vec<f32>, u16, u32)>` to Player. Initialize to None.

Modify `load_ambient()`:
1. Call `decode_to_pcm(&audio_bytes)` to get `(samples, channels, sample_rate)`
2. Store `ambient_decoded_pcm = Some((samples.clone(), channels, sample_rate))`
3. Create `SamplesBuffer::new(channels, sample_rate, samples)` instead of Decoder
4. Append SamplesBuffer to the new ambient sink (no VisualizerSource for ambient -- correct, ambient does not use visualizer)
5. Continue storing `ambient_audio_data = Some(audio_bytes)` for backward compatibility

Modify `replay_ambient()`:
1. Clone from `ambient_decoded_pcm` instead of re-decoding from `ambient_audio_data` compressed bytes
2. Create `SamplesBuffer::new(channels, sample_rate, samples.clone())`
3. Append to fresh ambient sink
4. Remove Decoder builder code

Modify `stop_ambient()`:
1. Also clear `ambient_decoded_pcm = None` alongside existing cleanup

In src/main.rs:

Change line 56 from `"300"` to `"500"` for PULSE_LATENCY_MSEC.
Update the comment above it to reflect the change: mention that 300ms was found insufficient for sustained playback beyond ~1 minute, and 500ms provides additional headroom while remaining imperceptible for music.

Also in src/player.rs, clean up: if `_audio_data` and `ambient_audio_data` (compressed bytes) are ONLY used for re-download avoidance checks (checking if data exists), NOT for re-decoding, then:
- Keep `_audio_data` as-is (it IS referenced for replay fallback in app.rs)
- Actually, keep `ambient_audio_data` too -- `has_ambient_data()` checks it
- But replay_ambient no longer needs the compressed bytes since it uses decoded_pcm. If `ambient_decoded_pcm` is None but `ambient_audio_data` exists, fall back to decoding from compressed bytes for robustness.
  </action>
  <verify>
Run `cargo check` -- must compile with no errors.
Run `cargo build --release` -- must build successfully.
Run `cargo clippy -- -D warnings` -- should pass or only have pre-existing warnings.

Verify in src/main.rs that PULSE_LATENCY_MSEC is set to "500".
Verify in src/player.rs that both load_ambient and replay_ambient use SamplesBuffer.
  </verify>
  <done>
Both main and ambient channels use pre-decoded SamplesBuffer. PULSE_LATENCY_MSEC is 500ms. Full `cargo build --release` succeeds. The audio callback thread now only performs fast memory reads -- no symphonia decoding on the hot path.
  </done>
</task>

</tasks>

<verification>
1. `cargo build --release` compiles successfully
2. `cargo clippy -- -D warnings` passes (or only pre-existing warnings)
3. grep confirms SamplesBuffer usage: `grep -n "SamplesBuffer" src/player.rs` shows new/append calls
4. grep confirms no Decoder in audio hot path: `load_and_play`, `replay_current`, `replay_ambient` should NOT contain `Decoder::builder()` calls (only `decode_to_pcm` helper uses Decoder, which runs before appending to sink)
5. grep confirms PULSE_LATENCY_MSEC is "500" in src/main.rs
</verification>

<success_criteria>
- Audio playback uses pre-decoded PCM via SamplesBuffer for both main and ambient channels
- Symphonia decoder is completely removed from the audio callback thread
- Seeking works by creating SamplesBuffer from sample offset (no try_seek on Decoder)
- Repeat One replays from cached decoded PCM (no re-decode)
- Ambient loop replays from cached decoded PCM
- VisualizerSource still wraps main channel source for spectrum display
- PULSE_LATENCY_MSEC increased from 300 to 500
- Project compiles and builds in release mode
</success_criteria>

<output>
After completion, create `.planning/quick/4-fix-audio-playback-stopping-unexpectedly/4-SUMMARY.md`
</output>
