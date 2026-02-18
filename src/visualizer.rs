use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::widgets::{BarChart, Block, Borders};
use ratatui::Frame;
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
// FFT computation (UI thread only)
// ---------------------------------------------------------------------------

/// Compute spectrum bars directly from raw PCM samples.
///
/// Takes mono samples (already extracted from interleaved PCM by the caller)
/// and a sample rate. Performs FFT and returns normalized magnitudes (0.0..1.0)
/// for each frequency band.
///
/// This function runs entirely on the UI thread with no shared state —
/// the audio thread has zero visualization overhead. Samples are read from
/// the pre-decoded PCM buffer in Player at the current playback position.
pub fn compute_spectrum_bars_from_pcm(
    samples: &[f32],
    sample_rate: u32,
    num_bars: usize,
) -> Option<Vec<f64>> {
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
