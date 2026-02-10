use std::sync::{Arc, Mutex};
use std::time::Duration;

use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::widgets::{BarChart, Block, Borders};
use ratatui::Frame;
use rodio::Source;
use spectrum_analyzer::scaling::divide_by_N_sqrt;
use spectrum_analyzer::windows::hann_window;
use spectrum_analyzer::{samples_fft_to_spectrum, FrequencyLimit};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// FFT window size (must be power of 2). 2048 at 44100 Hz = ~46ms window.
/// Good balance between frequency resolution (~21 Hz) and time resolution.
pub const FFT_SIZE: usize = 2048;

/// Default number of frequency bands for the visualizer display.
/// The actual count is adjusted dynamically based on available terminal width.
pub const NUM_BARS: usize = 32;

// ---------------------------------------------------------------------------
// Shared visualizer data
// ---------------------------------------------------------------------------

/// Shared data structure between the audio thread (writes samples) and
/// the UI thread (reads samples for FFT). Wrapped in Arc<Mutex<>> for
/// cross-thread access.
pub struct VisualizerData {
    /// Circular buffer of audio samples (fixed size = FFT_SIZE).
    pub samples: Vec<f32>,
    /// Sample rate of the audio source (set on first sample).
    pub sample_rate: u32,
    /// Flag set by the audio thread when new samples are written.
    /// Cleared by the UI thread after reading. Prevents wasted FFT
    /// cycles when playback is paused.
    pub has_new_data: bool,
}

/// Factory function to create a new VisualizerData wrapped in Arc<Mutex<>>.
///
/// Pre-allocates the sample buffer with zeros to avoid allocations on the
/// audio thread.
pub fn create_visualizer_data(fft_size: usize) -> Arc<Mutex<VisualizerData>> {
    Arc::new(Mutex::new(VisualizerData {
        samples: vec![0.0; fft_size],
        sample_rate: 44100, // default, overwritten on first sample
        has_new_data: false,
    }))
}

// ---------------------------------------------------------------------------
// VisualizerSource -- rodio Source wrapper (audio thread tap)
// ---------------------------------------------------------------------------

/// A rodio Source wrapper that copies audio samples to a shared buffer
/// for spectrum visualization, then passes them through unchanged.
///
/// This runs on the audio thread. It MUST NOT do any heavy computation
/// (no FFT, no allocations). Only sample copying via try_lock() so the
/// audio thread is never blocked.
pub struct VisualizerSource<S> {
    inner: S,
    data: Arc<Mutex<VisualizerData>>,
    /// Current write position in the circular buffer.
    write_pos: usize,
    /// FFT window size (buffer length).
    fft_size: usize,
    /// Counter to track channel position for mono extraction from stereo.
    channel_counter: usize,
    /// Number of channels in the inner source (cached to avoid repeated calls).
    channels: u16,
    /// Whether we have stored the sample rate yet.
    sample_rate_stored: bool,
}

impl<S: Source<Item = f32>> VisualizerSource<S> {
    pub fn new(inner: S, data: Arc<Mutex<VisualizerData>>, fft_size: usize) -> Self {
        let channels = inner.channels();
        Self {
            inner,
            data,
            write_pos: 0,
            fft_size,
            channel_counter: 0,
            channels,
            sample_rate_stored: false,
        }
    }
}

impl<S: Source<Item = f32>> Iterator for VisualizerSource<S> {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        let sample = self.inner.next()?;

        // Only capture left channel samples (every Nth where N = channels).
        // For mono (channels=1), every sample is captured.
        // For stereo (channels=2), only even-indexed samples (left channel).
        if self.channel_counter.is_multiple_of(self.channels as usize) {
            // Use try_lock to never block the audio thread. If the UI thread
            // holds the lock (reading for FFT), we skip this sample -- losing
            // one sample out of ~44100/sec is completely inaudible.
            if let Ok(mut data) = self.data.try_lock() {
                // Store sample rate on first sample
                if !self.sample_rate_stored {
                    data.sample_rate = self.inner.sample_rate();
                    self.sample_rate_stored = true;
                }

                if data.samples.len() == self.fft_size {
                    data.samples[self.write_pos % self.fft_size] = sample;
                    self.write_pos = (self.write_pos + 1) % self.fft_size;
                    data.has_new_data = true;
                }
            }
        }
        self.channel_counter += 1;

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

// ---------------------------------------------------------------------------
// FFT computation (UI thread only)
// ---------------------------------------------------------------------------

/// Compute spectrum bars from the shared visualizer data.
///
/// Locks the data briefly to clone the sample buffer, then releases the lock
/// before performing FFT. Returns a Vec of normalized magnitudes (0.0..1.0)
/// for each frequency band.
///
/// If `has_new_data` is false (playback paused or no audio), returns None
/// to signal the caller should use previous values (which will decay via
/// smoothing).
pub fn compute_spectrum_bars(
    data: &Arc<Mutex<VisualizerData>>,
    num_bars: usize,
) -> Option<Vec<f64>> {
    // Lock briefly to clone data, then release
    let (samples, sample_rate) = {
        let mut guard = data.lock().ok()?;
        if !guard.has_new_data {
            // No new samples (paused/stopped) -- return zeros to allow decay
            return Some(vec![0.0; num_bars]);
        }
        let samples = guard.samples.clone();
        let sample_rate = guard.sample_rate;
        guard.has_new_data = false;
        (samples, sample_rate)
    };

    if samples.len() < FFT_SIZE || samples.iter().all(|&s| s == 0.0) {
        return Some(vec![0.0; num_bars]);
    }

    // Apply Hann window to reduce spectral leakage
    let windowed = hann_window(&samples[..FFT_SIZE]);

    // Compute FFT and get frequency spectrum
    let spectrum = samples_fft_to_spectrum(
        &windowed,
        sample_rate,
        FrequencyLimit::Range(20.0, 16000.0),
        Some(&divide_by_N_sqrt),
    )
    .ok()?;

    // Collect frequency-magnitude pairs for logarithmic binning
    let freq_mag: Vec<(f32, f32)> = spectrum
        .data()
        .iter()
        .map(|(freq, mag)| (freq.val(), mag.val()))
        .collect();

    // Bin into logarithmic frequency bands
    let bars = bin_spectrum_logarithmic(&freq_mag, num_bars, 20.0, 16000.0);

    // Normalize to 0.0..1.0 range
    let max_val = bars.iter().cloned().fold(0.0f64, f64::max);
    if max_val > 0.0 {
        Some(bars.iter().map(|&v| (v / max_val).min(1.0)).collect())
    } else {
        Some(vec![0.0; num_bars])
    }
}

/// Map FFT frequency bins to display bars using logarithmic spacing.
///
/// Human hearing perceives pitch logarithmically (each octave doubles in
/// frequency). Linear binning would waste most bars on high frequencies
/// that sound similar. Logarithmic binning allocates bars proportionally
/// to how we hear.
fn bin_spectrum_logarithmic(
    spectrum: &[(f32, f32)],
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

// ---------------------------------------------------------------------------
// VisualizerState -- temporal smoothing
// ---------------------------------------------------------------------------

/// Holds the smoothed bar values for the visualizer display.
///
/// Applies exponential smoothing between frames: fast attack (bars rise
/// quickly to new peaks) and slow decay (bars fall gradually), mimicking
/// classic hardware equalizer displays.
pub struct VisualizerState {
    smoothed_bars: Vec<f64>,
}

impl VisualizerState {
    /// Create a new VisualizerState with the given number of bars.
    pub fn new(num_bars: usize) -> Self {
        Self {
            smoothed_bars: vec![0.0; num_bars],
        }
    }

    /// Update smoothed bars toward target values.
    ///
    /// - Fast attack (0.7): bars rise quickly when new value exceeds current
    /// - Slow decay (0.9): bars fall gradually when new value is lower
    pub fn update(&mut self, target: &[f64]) {
        // Resize if target has different length
        if self.smoothed_bars.len() != target.len() {
            self.smoothed_bars.resize(target.len(), 0.0);
        }

        let attack = 0.7;
        let decay = 0.9;

        for (i, &new_val) in target.iter().enumerate() {
            if new_val > self.smoothed_bars[i] {
                // Rising: fast attack
                self.smoothed_bars[i] =
                    self.smoothed_bars[i] * (1.0 - attack) + new_val * attack;
            } else {
                // Falling: slow decay
                self.smoothed_bars[i] =
                    self.smoothed_bars[i] * decay + new_val * (1.0 - decay);
            }
        }
    }

    /// Get the current smoothed bar values.
    pub fn bars(&self) -> &[f64] {
        &self.smoothed_bars
    }
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

/// Render the spectrum visualizer as a BarChart in the given area.
///
/// Converts f64 magnitudes (0.0..1.0) to u64 values scaled to 0..100,
/// then renders a ratatui BarChart with Cyan bars and DarkGray borders.
pub fn render_visualizer(frame: &mut Frame, area: Rect, bar_values: &[f64]) {
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
        .value_style(Style::default().fg(Color::Reset).bg(Color::Reset))
        .max(max_value);

    frame.render_widget(chart, area);
}
