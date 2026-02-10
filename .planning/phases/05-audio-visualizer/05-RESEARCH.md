# Phase 5: Audio Visualizer - Research

**Researched:** 2026-02-10
**Domain:** Real-time audio spectrum visualization in a Rust TUI (rodio + ratatui)
**Confidence:** MEDIUM-HIGH

## Summary

Implementing a toggleable audio spectrum visualizer in TermTunes requires solving three distinct problems: (1) tapping into the audio stream to capture raw samples without disrupting playback, (2) performing FFT to convert time-domain samples into frequency-domain spectrum data, and (3) rendering the spectrum as an animated bar display in the ratatui TUI without causing UI lag or audio dropouts.

The recommended approach is a **custom rodio Source wrapper** (a "tap" or "passthrough") that copies each audio sample into a shared ring buffer as it flows through the playback pipeline. A separate FFT computation runs on the UI thread (not the audio thread) at the render tick rate (~10 Hz), reading the latest samples from the ring buffer and producing frequency bin magnitudes. The ratatui `BarChart` widget renders these magnitudes as vertical bars with Unicode fractional block characters, creating a classic spectrum analyzer aesthetic.

This approach is well-established in the Rust audio visualization ecosystem. The `spectrum-analyzer` crate (v1.7.0) wraps microfft with windowing functions and frequency-to-magnitude conversion, eliminating the need to hand-roll FFT logic. For the shared buffer between audio and UI threads, a simple `Arc<Mutex<Vec<f32>>>` is sufficient given the low contention (audio thread writes, UI thread reads at ~10 Hz), though a lock-free ring buffer (`ringbuf` crate) is a lower-latency alternative if Mutex contention becomes measurable.

**Primary recommendation:** Implement a `VisualizerSource<S: Source>` wrapper that copies samples to a shared buffer, use `spectrum-analyzer` for FFT, render with ratatui `BarChart` (bar_width=1, bar_gap=0, NINE_LEVELS bar set), toggle with `v` key, compute FFT only when visualizer is visible.

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| spectrum-analyzer | 1.7.0 | FFT + windowing + frequency spectrum extraction | Purpose-built for exactly this use case. Wraps microfft (fastest no_std FFT). Handles Hann windowing, frequency bin mapping, and magnitude scaling. Used by audio-visualizer ecosystem. Eliminates ~200 lines of hand-rolled FFT code. |
| ratatui (existing) | 0.30 | BarChart widget for spectrum display | Already in the project. BarChart with bar_width=1, bar_gap=0, and NINE_LEVELS bar set renders a classic spectrum analyzer. No new dependency needed. |
| rodio (existing) | 0.21 | Audio playback + custom Source trait | Already in the project. The Source trait allows wrapping the Decoder to intercept samples. No new dependency needed. |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| ringbuf | 0.4.8 | Lock-free SPSC ring buffer | Only if Arc<Mutex<Vec>> shows measurable contention. For an audio visualizer updating at ~10 Hz, Mutex should be fine. Keep as optimization path, not initial implementation. |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| spectrum-analyzer | realfft + manual windowing | More control but ~200 lines of FFT setup, windowing, bin mapping. spectrum-analyzer does all this in one function call. Only use if spectrum-analyzer's microfft dependency causes issues. |
| spectrum-analyzer | rustfft directly | Lower level, more flexible, but requires manual window function application, frequency bin computation, and magnitude extraction. Overkill for a visualization use case. |
| BarChart widget | Sparkline widget | Sparkline renders one row of fractional bars (1 line height). BarChart renders multi-row bars (N lines height). BarChart is better for spectrum display because it fills the allocated area vertically. Sparkline is better for inline/compact displays. |
| BarChart widget | Custom Canvas widget | More control over rendering (could draw individual pixels) but significantly more code. BarChart with NINE_LEVELS provides 8 sub-character height levels per row, which is sufficient for aesthetic visualization. |
| Arc<Mutex<Vec>> | ringbuf (lock-free) | Lock-free avoids any potential audio thread stall. But the audio thread holds the Mutex for microseconds (copying ~2048 samples) and the UI reads at ~10 Hz, so contention is near-zero. Add ringbuf only if profiling shows issues. |
| Internal FFT | External cava process | cava is a dedicated terminal audio visualizer that handles its own audio capture and FFT. Some Rust TUI players (cli-music-player) shell out to cava for visualization. Tradeoff: external dependency, requires ALSA loopback or PulseAudio monitor, harder to control rendering, but zero FFT code in the app. Not recommended for TermTunes because we already have audio samples in-process. |

**Installation:**
```bash
# Only one new dependency needed
cargo add spectrum-analyzer
```

```toml
# Cargo.toml addition
spectrum-analyzer = "1.7"
```

## Architecture Patterns

### Recommended Project Structure
```
src/
├── app.rs           # Add visualizer_enabled toggle + v keybinding
├── player.rs        # Add VisualizerSource wrapper + shared sample buffer
├── ui.rs            # Add visualizer rendering area + BarChart
├── visualizer.rs    # NEW: FFT computation, frequency binning, BarChart data
├── main.rs          # (unchanged)
├── config.rs        # (unchanged)
├── auth.rs          # (unchanged)
├── plex.rs          # (unchanged)
└── tui.rs           # (unchanged)
```

### Pattern 1: Source Tap (Audio Sample Interception)
**What:** A wrapper struct `VisualizerSource<S>` that implements `rodio::Source` + `Iterator`, delegating all calls to the inner source but copying each sample to a shared buffer.
**When to use:** Every time a track is loaded for playback. The wrapper is inserted between the Decoder and the Sink.
**Why this works:** rodio's Source trait is composable -- sources wrap other sources. The audio thread calls `next()` on the outermost source, which propagates down. Our wrapper sits in this chain and copies each sample before returning it.

```rust
use std::sync::{Arc, Mutex};
use std::time::Duration;
use rodio::Source;

/// Shared buffer for audio samples used by the visualizer.
/// The audio thread writes samples here; the UI thread reads them for FFT.
pub type SampleBuffer = Arc<Mutex<Vec<f32>>>;

/// A Source wrapper that copies audio samples to a shared buffer
/// for spectrum visualization, then passes them through unchanged.
pub struct VisualizerSource<S> {
    inner: S,
    buffer: SampleBuffer,
    /// Position within the current write batch
    write_pos: usize,
    /// Size of the FFT window (must be power of 2)
    fft_size: usize,
}

impl<S> VisualizerSource<S> {
    pub fn new(inner: S, buffer: SampleBuffer, fft_size: usize) -> Self {
        Self {
            inner,
            buffer,
            write_pos: 0,
            fft_size,
        }
    }
}

impl<S: Source<Item = f32>> Iterator for VisualizerSource<S> {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        let sample = self.inner.next()?;

        // Copy sample to shared buffer (circular write)
        if let Ok(mut buf) = self.buffer.try_lock() {
            if buf.len() == self.fft_size {
                buf[self.write_pos % self.fft_size] = sample;
                self.write_pos = (self.write_pos + 1) % self.fft_size;
            }
        }
        // If lock fails (UI reading), skip this sample -- no audio impact

        Some(sample)
    }
}

impl<S: Source<Item = f32>> Source for VisualizerSource<S> {
    fn current_span_len(&self) -> Option<usize> {
        self.inner.current_span_len()
    }
    fn channels(&self) -> u16 {
        self.inner.channels()
    }
    fn sample_rate(&self) -> u32 {
        self.inner.sample_rate()
    }
    fn total_duration(&self) -> Option<Duration> {
        self.inner.total_duration()
    }
}
```

### Pattern 2: FFT on Render Tick (Not Audio Thread)
**What:** FFT computation happens in the UI event loop at the render tick rate (~10 Hz), reading from the shared sample buffer. The audio thread never does FFT -- it only copies samples.
**When to use:** Every render cycle when the visualizer is enabled.
**Why this works:** FFT of 2048 samples with microfft takes ~50-100 microseconds on modern hardware. At 10 Hz render rate, this is negligible overhead. Keeping FFT off the audio thread ensures zero risk of audio dropouts from FFT latency spikes.

```rust
use spectrum_analyzer::{samples_fft_to_spectrum, FrequencyLimit};
use spectrum_analyzer::windows::hann_window;
use spectrum_analyzer::scaling::divide_by_N_sqrt;

/// Number of frequency bands to display in the visualizer.
const NUM_BARS: usize = 16;
/// FFT window size (must be power of 2). 2048 at 44100 Hz = ~46ms window.
const FFT_SIZE: usize = 2048;

/// Compute spectrum bars from the shared sample buffer.
/// Returns a Vec of magnitudes (0.0..=1.0) for each frequency band.
pub fn compute_spectrum_bars(buffer: &SampleBuffer) -> Vec<f64> {
    let samples = {
        let buf = buffer.lock().unwrap();
        buf.clone()
    };

    if samples.len() < FFT_SIZE || samples.iter().all(|&s| s == 0.0) {
        return vec![0.0; NUM_BARS];
    }

    // Apply Hann window to reduce spectral leakage
    let hann_window = hann_window(&samples[..FFT_SIZE]);

    // Compute FFT and get frequency spectrum
    let spectrum = match samples_fft_to_spectrum(
        &hann_window,
        44100, // sample rate (get from source in real implementation)
        FrequencyLimit::Range(20.0, 16000.0), // audible range
        Some(&divide_by_N_sqrt),
    ) {
        Ok(s) => s,
        Err(_) => return vec![0.0; NUM_BARS],
    };

    // Bin frequencies into NUM_BARS bands (logarithmic spacing)
    // ... (see Code Examples section for full binning logic)
    todo!()
}
```

### Pattern 3: Conditional Rendering (Toggle with v)
**What:** The visualizer area is only allocated and rendered when `app.visualizer_enabled` is true. When toggled off, the layout reverts to the existing design with zero overhead.
**When to use:** Layout calculation in ui.rs.
**Why this works:** ratatui's layout system handles dynamic area allocation. When visualizer is on, the main content area shrinks to make room for the visualizer area. When off, the full area is used for the track list.

```rust
// In ui.rs render function, when visualizer is enabled:
let areas = if app.visualizer_enabled() && app.now_playing().is_some() {
    // Split main area: track list on top, visualizer in middle, player bar at bottom
    Layout::vertical([
        Constraint::Fill(1),       // Track list
        Constraint::Length(8),     // Visualizer (8 rows)
        Constraint::Length(3),     // Player bar
    ]).areas(area)
} else {
    // Existing layout: track list + player bar
    Layout::vertical([
        Constraint::Fill(1),       // Track list
        Constraint::Length(3),     // Player bar
    ]).areas(area)
};
```

### Anti-Patterns to Avoid
- **Running FFT on the audio thread:** Even fast FFTs have variable latency. A GC-like latency spike during FFT on the audio thread could cause an audible dropout. Always run FFT on the UI thread, reading from a buffer the audio thread writes to.
- **Locking the Mutex for the entire FFT computation:** Lock the Mutex only to copy the buffer out, then release. Run FFT on the local copy. This minimizes contention window to ~microseconds.
- **Recomputing FFT every frame when visualizer is hidden:** Check `visualizer_enabled` before any FFT work. The sample buffer still fills (Source wrapper is always active) but no CPU is spent on FFT when visualization is off.
- **Using too large an FFT window:** 2048 samples is optimal. 4096 gives better frequency resolution but worse time resolution (more latency). 1024 is faster but lower frequency resolution. 2048 at 44100 Hz gives ~46ms time window and ~21 Hz frequency resolution, which is ideal for music visualization.
- **Not handling mono/stereo correctly:** If the source is stereo, the samples alternate L-R-L-R. For visualization, either average the channels or pick one. Feeding interleaved stereo samples directly into FFT gives incorrect results.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| FFT computation | Custom FFT implementation | `spectrum-analyzer` crate | FFT is mathematically subtle (windowing, bin mapping, magnitude scaling, frequency resolution). spectrum-analyzer handles all of this in a single function call. Hand-rolling FFT introduces bugs in windowing, off-by-one in bin indices, and incorrect magnitude scaling. |
| Window functions | Manual Hann/Hamming window | `spectrum_analyzer::windows::hann_window()` | Window function correctness affects visualization quality significantly. Off-by-one errors in window computation cause spectral leakage artifacts. |
| Frequency binning (linear to log) | Custom logarithmic frequency band mapping | See Code Examples section for the standard pattern | Logarithmic binning maps frequencies to bars following human pitch perception. The pattern is well-established but has edge cases (empty bins at low frequencies, overloaded bins at high frequencies). Use the proven pattern. |
| Unicode bar rendering | Custom character selection for partial bars | ratatui `BarChart` with `symbols::bar::NINE_LEVELS` | NINE_LEVELS provides 8 sub-character height levels (" ", "▁", "▂", "▃", "▄", "▅", "▆", "▇", "█") which is the standard terminal approach. BarChart handles all the character selection math. |

**Key insight:** The audio visualization domain has three layers of complexity that are easy to underestimate: (1) FFT math (windowing, scaling, normalization), (2) perceptual mapping (logarithmic frequency binning to match human hearing), and (3) visual smoothing (temporal decay/attack to prevent jittery bars). Using spectrum-analyzer eliminates layer 1 entirely and provides the data needed for layers 2 and 3.

## Common Pitfalls

### Pitfall 1: Audio Dropouts from Visualizer Overhead
**What goes wrong:** FFT computation or buffer synchronization causes the audio thread to stall, producing audible clicks or gaps.
**Why it happens:** Mutex contention between the audio thread (writing samples) and UI thread (reading for FFT). Or running FFT on the audio thread itself.
**How to avoid:** Use `try_lock()` in the audio thread Source wrapper -- if the UI thread holds the lock, skip writing that sample (losing one sample is inaudible). Never run FFT on the audio thread. Copy the buffer out of the Mutex before running FFT.
**Warning signs:** Audio clicks that correlate with visualizer being enabled. CPU usage spike when visualizer is toggled on.

### Pitfall 2: Feeding Interleaved Stereo Samples to FFT
**What goes wrong:** The spectrum shows incorrect frequencies, often appearing as noise or having frequency content at double the expected frequencies.
**Why it happens:** Stereo audio interleaves samples: L0, R0, L1, R1, L2, R2... If fed directly into FFT, the algorithm sees alternating values from different channels, not a coherent signal. The resulting spectrum is meaningless.
**How to avoid:** In the Source wrapper, only copy every Nth sample where N = number of channels (picking the left channel), or average L+R pairs before storing. The simplest approach: track a channel counter and only write when `channel_counter % channels == 0`.
**Warning signs:** Spectrum looks like noise even during a pure sine tone test. Bars seem random rather than tracking the music.

### Pitfall 3: Jittery/Flickering Visualizer Bars
**What goes wrong:** Bar heights jump wildly between frames, making the visualizer look chaotic rather than smooth.
**Why it happens:** Raw FFT magnitudes change dramatically between consecutive windows, especially for transient sounds. Without temporal smoothing, each frame is independent.
**How to avoid:** Apply exponential smoothing between frames: `displayed_value = displayed_value * decay + new_value * (1 - decay)`. A decay factor of 0.7-0.8 gives smooth "falling" bars. Also apply a minimum "attack" rate so bars rise quickly but fall slowly (mimicking real equalizer hardware).
**Warning signs:** Visualizer looks like flickering noise rather than smooth bars tracking the beat.

### Pitfall 4: Visualizer Layout Breaking Narrow Mode
**What goes wrong:** Enabling the visualizer in a narrow tmux pane (< 40 cols) pushes the track list or player bar off-screen, or the bars are too narrow to be meaningful.
**Why it happens:** The visualizer area takes fixed height (8 rows) and needs minimum width for bars. In narrow/short terminals, there's not enough space.
**How to avoid:** Hide the visualizer automatically when terminal width < NARROW_WIDTH (40) or height < minimum threshold (e.g., 15 rows). Show a brief message ("Terminal too small for visualizer") or silently disable. The `v` key should still toggle the setting, but rendering should be conditional on available space.
**Warning signs:** Player bar disappears or becomes unusable when visualizer is enabled in small terminals.

### Pitfall 5: Visualizer Still Consuming CPU When Paused
**What goes wrong:** CPU usage stays elevated even when playback is paused and the visualizer shows static bars.
**Why it happens:** The FFT computation runs on every render tick regardless of whether new samples are available.
**How to avoid:** Track whether new samples have been written since the last FFT computation. If the audio is paused (no new samples), skip FFT and re-render the previous bar values. Add a "stale" flag to the shared buffer that the Source wrapper sets on write and the visualizer clears after reading.
**Warning signs:** CPU usage doesn't drop when playback is paused but visualizer is visible.

### Pitfall 6: BarChart Values Must Be u64
**What goes wrong:** Compilation error or loss of precision when passing float FFT magnitudes to BarChart.
**Why it happens:** ratatui's BarChart data accepts `(&str, u64)` tuples. FFT magnitudes are floating point (0.0 to ~1.0 after normalization).
**How to avoid:** Scale the normalized magnitude (0.0..1.0) to a u64 range (0..100 or 0..1000) before passing to BarChart. The BarChart's max value should be set to match the scale. Example: `(magnitude * 100.0) as u64` with `BarChart::max(100)`.
**Warning signs:** All bars at the same height, or bars always at max/min.

## Code Examples

Verified patterns from research:

### Logarithmic Frequency Binning
```rust
/// Map FFT frequency bins to display bars using logarithmic spacing.
///
/// Human hearing perceives pitch logarithmically (each octave doubles in
/// frequency). Linear binning would waste most bars on high frequencies
/// that sound similar. Logarithmic binning allocates bars proportionally
/// to how we hear.
fn bin_spectrum_logarithmic(
    spectrum: &[(f32, f32)], // (frequency_hz, magnitude) pairs
    num_bars: usize,
    min_freq: f32,
    max_freq: f32,
) -> Vec<f64> {
    let log_min = min_freq.ln();
    let log_max = max_freq.ln();
    let log_step = (log_max - log_min) / num_bars as f32;

    let mut bars = vec![0.0f64; num_bars];
    let mut counts = vec![0usize; num_bars];

    for &(freq, mag) in spectrum {
        if freq < min_freq || freq > max_freq {
            continue;
        }
        let log_freq = freq.ln();
        let bar_index = ((log_freq - log_min) / log_step) as usize;
        let bar_index = bar_index.min(num_bars - 1);
        bars[bar_index] += mag as f64;
        counts[bar_index] += 1;
    }

    // Average the magnitudes in each bin
    for i in 0..num_bars {
        if counts[i] > 0 {
            bars[i] /= counts[i] as f64;
        }
    }

    bars
}
```

### Exponential Smoothing for Bar Animation
```rust
/// Smooth bar values between frames for aesthetic animation.
///
/// Fast attack (bars rise quickly to new peaks) and slow decay
/// (bars fall gradually) mimics classic hardware equalizers.
fn smooth_bars(
    current: &mut Vec<f64>,  // displayed values (mutated in place)
    target: &[f64],          // new FFT values
    attack: f64,             // rise speed (0.6-0.8 typical)
    decay: f64,              // fall speed (0.85-0.95 typical)
) {
    for (i, &new_val) in target.iter().enumerate() {
        if i >= current.len() {
            break;
        }
        if new_val > current[i] {
            // Rising: fast attack
            current[i] = current[i] * (1.0 - attack) + new_val * attack;
        } else {
            // Falling: slow decay
            current[i] = current[i] * decay + new_val * (1.0 - decay);
        }
    }
}
```

### BarChart Rendering for Spectrum
```rust
use ratatui::widgets::{BarChart, Block, Borders};
use ratatui::style::{Color, Style};

/// Render the spectrum visualizer as a BarChart in the given area.
fn render_visualizer(
    frame: &mut Frame,
    area: Rect,
    bar_values: &[f64], // 0.0..=1.0 normalized magnitudes
) {
    let max_value: u64 = 100;

    // Convert f64 magnitudes to BarChart data: (&str, u64) tuples
    let data: Vec<(&str, u64)> = bar_values
        .iter()
        .map(|&v| ("", (v * max_value as f64).round() as u64))
        .collect();

    let chart = BarChart::default()
        .block(
            Block::default()
                .title(" Visualizer ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray)),
        )
        .data(&data)
        .bar_width(1)
        .bar_gap(0)
        .bar_style(Style::default().fg(Color::Cyan))
        .value_style(Style::default().fg(Color::Cyan)) // hide value labels
        .max(max_value);

    frame.render_widget(chart, area);
}
```

### Mono Channel Extraction in Source Wrapper
```rust
/// Modified Source wrapper that handles stereo by extracting left channel only.
impl<S: Source<Item = f32>> Iterator for VisualizerSource<S> {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        let sample = self.inner.next()?;
        let channels = self.inner.channels() as usize;

        // Only capture left channel samples (every Nth where N = channels)
        if self.channel_counter % channels == 0 {
            if let Ok(mut buf) = self.buffer.try_lock() {
                if buf.len() == self.fft_size {
                    buf[self.write_pos % self.fft_size] = sample;
                    self.write_pos = (self.write_pos + 1) % self.fft_size;
                }
            }
        }
        self.channel_counter += 1;

        Some(sample)
    }
}
```

### Integration with Existing Player::load_and_play
```rust
// In player.rs, modify load_and_play to wrap the decoder with VisualizerSource:

pub fn load_and_play(
    &mut self,
    audio_bytes: Vec<u8>,
    track_name: String,
    volume: f32,
    sample_buffer: Option<SampleBuffer>, // NEW parameter
) -> Result<()> {
    self.sink.stop();
    self.sink = Sink::connect_new(self._stream.mixer());
    self.sink.set_volume(volume.clamp(0.0, 1.0));

    let byte_len = audio_bytes.len() as u64;
    let cursor = Cursor::new(audio_bytes.clone());
    let source = Decoder::builder()
        .with_data(cursor)
        .with_byte_len(byte_len)
        .with_seekable(true)
        .build()
        .map_err(|e| color_eyre::eyre::eyre!("Failed to decode audio: {}", e))?;

    // Wrap with visualizer tap if buffer is provided
    if let Some(buffer) = sample_buffer {
        let viz_source = VisualizerSource::new(source, buffer, FFT_SIZE);
        self.sink.append(viz_source);
    } else {
        self.sink.append(source);
    }

    self._audio_data = Some(audio_bytes);
    self.current_track = Some(track_name);
    Ok(())
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| External cava process for visualization | Internal FFT with spectrum-analyzer crate | 2024-2025 | No external dependency, tighter integration, works on all platforms without ALSA loopback |
| Manual FFT with rustfft | spectrum-analyzer wrapping microfft | spectrum-analyzer 1.0 (2022) | One function call instead of manual window/FFT/bin pipeline. microfft is fastest no_std FFT. |
| tui-rs BarChart | ratatui BarChart with NINE_LEVELS | ratatui 0.20+ (2023) | 8 sub-character height levels for smooth fractional bars. Better visual quality. |
| Polling audio position for sync | Source trait tap (passthrough wrapper) | Standard rodio pattern | Direct sample access without polling. Zero-latency sync between audio and visualization. |

**Deprecated/outdated:**
- tui-rs: Unmaintained, replaced by ratatui. All examples should use ratatui 0.30.
- Manual microfft usage: spectrum-analyzer wraps it with windowing and scaling. No reason to use microfft directly for visualization.

## Open Questions

1. **Decoder sample type compatibility with spectrum-analyzer**
   - What we know: rodio's Decoder outputs `f32` samples. spectrum-analyzer's `samples_fft_to_spectrum` accepts `&[f32]`. Types align.
   - What's unclear: Whether the Decoder's `convert_samples()` chain (which rodio applies internally) affects the sample values in a way that matters for FFT. The samples should still be normalized -1.0..1.0 audio but this needs verification.
   - Recommendation: Verify in implementation that FFT output looks reasonable with a test tone. If samples are not in expected range, add a normalization step before FFT.

2. **Sample rate availability in the visualizer**
   - What we know: spectrum-analyzer needs the sample rate to map FFT bins to frequencies. The Source wrapper has access to `self.inner.sample_rate()`. The shared buffer does not carry this metadata.
   - What's unclear: Whether sample rate changes mid-track (it shouldn't for decoded audio, but spans theoretically allow it).
   - Recommendation: Store the sample rate alongside the shared buffer (e.g., in an `Arc<Mutex<VisualizerData>>` struct containing both the sample vec and the sample rate). Set it once when the Source wrapper is created.

3. **BarChart bar_width interaction with terminal width**
   - What we know: With bar_width=1 and bar_gap=0, each bar takes exactly 1 column. With 16 bars + borders (2 cols), minimum width is 18 cols. With bar_gap=1, minimum is 33 cols.
   - What's unclear: Whether bar_gap=0 looks good aesthetically or if minimal gap (1) is needed for readability.
   - Recommendation: Start with bar_gap=0 and bar_width=1 for maximum density. Adjust number of bars dynamically based on available width: `num_bars = (width - 2) / (bar_width + bar_gap)`. Test both visually.

4. **Visualizer height allocation**
   - What we know: BarChart needs at least 3 rows to be useful (1 border top, 1 bar area, 1 border bottom). More rows give finer vertical resolution with NINE_LEVELS.
   - What's unclear: Optimal height for aesthetic balance between track list and visualizer.
   - Recommendation: Use 6-8 rows for the visualizer area. With NINE_LEVELS (8 sub-levels per row) and 6 content rows (8 total with borders), that gives 48 visual height levels. This should provide smooth animation. Make it conditional on terminal height >= 20.

## Sources

### Primary (HIGH confidence)
- rodio Source trait documentation (https://docs.rs/rodio/latest/rodio/source/trait.Source.html) - Source trait API, Iterator requirement, channels/sample_rate/current_span_len methods
- spectrum-analyzer crate (https://lib.rs/crates/spectrum-analyzer) - v1.7.0, microfft-based FFT, hann_window, samples_fft_to_spectrum API
- ratatui BarChart widget documentation (https://docs.rs/ratatui-widgets/0.3.0/ratatui_widgets/barchart/) - BarChart API, bar_width, bar_gap, NINE_LEVELS bar set, data format
- ratatui Sparkline widget documentation (https://docs.rs/ratatui-widgets/0.3.0/ratatui_widgets/sparkline/) - SparklineBar, RenderDirection, custom styles
- realfft crate documentation (https://github.com/henquist/realfft) - RealFftPlanner API, forward/inverse FFT, confirmed Context7

### Secondary (MEDIUM confidence)
- spectrum-analyzer blog post by phip1611 (https://phip1611.de/blog/frequency-spectrum-analysis-with-fft-in-rust/) - FFT theory, window functions, frequency resolution explanation
- Live Audio Visualization blog post (https://phip1611.de/blog/live-audio-visualization-with-rust-in-a-gui-window/) - Architecture pattern: cpal capture -> buffer -> FFT -> render
- spectroscope project (https://codeberg.org/tranzystorekk/spectroscope) - ratatui + cava TUI visualizer reference, ALSA loopback pattern
- cli-music-player project (https://github.com/professor-lee/cli-music-player) - rodio + ratatui + internal FFT fallback pattern, confirms approach
- ringbuf crate (https://github.com/agerasev/ringbuf) - Lock-free SPSC ring buffer, v0.4.8, SharedRb for cross-thread audio

### Tertiary (LOW confidence)
- Pitfalls research from Phase 0 (internal) - FFT/spectrum analysis on render thread pitfall, CPU overhead concerns
- rodio GitHub issues on buffer reuse and Source wrapping - Community patterns, not official documentation

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - spectrum-analyzer is the established Rust crate for this exact use case, confirmed via multiple sources
- Architecture: MEDIUM-HIGH - Source wrapper pattern is well-understood from rodio docs and community projects, but specific integration with TermTunes's existing Player struct needs careful implementation
- Pitfalls: HIGH - Audio thread contention, stereo handling, and temporal smoothing are well-documented issues with established solutions
- Rendering: HIGH - ratatui BarChart is proven for this use case, NINE_LEVELS provides adequate resolution

**Research date:** 2026-02-10
**Valid until:** 2026-03-10 (stable domain, 30 days)
