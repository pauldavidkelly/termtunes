use std::io::Cursor;
use std::path::PathBuf;
use std::time::Duration;

use color_eyre::Result;
use rodio::buffer::SamplesBuffer;
use rodio::{Decoder, OutputStream, OutputStreamBuilder, Sink, Source};

/// Seek step size for forward/backward seeking within a track.
const SEEK_STEP: Duration = Duration::from_secs(5);

/// Audio player wrapping rodio's OutputStream and Sink.
///
/// The OutputStream MUST live as long as the Sink -- dropping it kills audio
/// immediately with no error. Both are stored together in this struct.
///
/// The player also retains the raw audio bytes for potential stream recreation
/// (WSL2 workaround if pause/resume fails after extended pauses).
pub struct Player {
    /// Output stream -- dropping this silences audio. Named with underscore
    /// prefix because it is never read, only kept alive.
    _stream: OutputStream,

    /// Rodio Sink for main music playback control (play/pause/stop/append).
    /// Renamed from `sink` to `main_sink` to distinguish from ambient_sink.
    main_sink: Sink,

    /// Raw audio bytes of the current track, kept for potential re-creation.
    _audio_data: Option<Vec<u8>>,

    /// Pre-decoded PCM samples for the current main track: (samples, channels, sample_rate).
    /// Used for seek (recreate SamplesBuffer from offset), replay (no re-decode needed),
    /// and visualizer (UI thread reads samples at playback position — no audio thread overhead).
    decoded_pcm: Option<(Vec<f32>, u16, u32)>,

    /// Name of the currently playing track (for status bar display).
    current_track: Option<String>,

    /// Ambient channel sink -- None when no ambient track is loaded.
    ambient_sink: Option<Sink>,

    /// Raw audio bytes of the ambient track, kept for backward compatibility.
    ambient_audio_data: Option<Vec<u8>>,

    /// Pre-decoded PCM samples for the ambient track: (samples, channels, sample_rate).
    /// Used for loop replay without re-decoding.
    ambient_decoded_pcm: Option<(Vec<f32>, u16, u32)>,

    /// Name of the ambient track (for status display).
    ambient_track_name: Option<String>,
}

impl Player {
    /// Create a new Player by opening the default audio output device.
    ///
    /// On WSL2, cpal uses the ALSA host which needs the `libasound2-plugins`
    /// package and an `~/.asoundrc` config to route audio through PulseAudio
    /// (provided by WSLg). This method ensures the ALSA config exists before
    /// attempting to open the stream, and provides clear diagnostics on failure.
    pub fn new() -> Result<Self> {
        // On WSL2, ensure ALSA is configured to use PulseAudio before opening
        // the audio stream. Without this, ALSA cannot find a sound card.
        if is_wsl2() {
            ensure_alsa_pulse_config()?;
        }

        // On WSL2: use a fixed 4096-sample cpal buffer to absorb WSLg PulseAudio
        // scheduling jitter that causes underruns (crackling) at smaller sizes.
        // On non-WSL2: let the OS/driver choose the optimal buffer size — forcing
        // Fixed(4096) on native Linux or macOS causes crackling where it isn't needed.
        let stream = if is_wsl2() {
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
                    tracing::info!(
                        "WSL2 explicit buffer config failed, falling back to default stream"
                    );
                    OutputStreamBuilder::open_default_stream()
                })
        } else {
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

        let main_sink = Sink::connect_new(stream.mixer());

        tracing::info!(
            config = ?stream.config(),
            "Audio output stream opened successfully"
        );

        Ok(Self {
            _stream: stream,
            main_sink,
            _audio_data: None,
            decoded_pcm: None,
            current_track: None,
            ambient_sink: None,
            ambient_audio_data: None,
            ambient_decoded_pcm: None,
            ambient_track_name: None,
        })
    }

    /// Download a track from the given URL into memory.
    ///
    /// This is a blocking operation (uses reqwest::blocking) and should be
    /// called from a background thread to avoid blocking the UI event loop.
    /// It is a static method -- it does not need the Player instance.
    pub fn download_track(url: &str) -> Result<Vec<u8>> {
        let response = reqwest::blocking::get(url)?;
        let status = response.status();
        if !status.is_success() {
            return Err(color_eyre::eyre::eyre!(
                "Failed to download track: HTTP {}",
                status
            ));
        }
        let bytes = response.bytes()?;
        tracing::info!(size_bytes = bytes.len(), "Track downloaded");
        Ok(bytes.to_vec())
    }

    /// Pre-decode compressed audio bytes into raw PCM samples.
    ///
    /// Returns `(samples, channels, sample_rate)`. The decoder runs entirely
    /// in the calling thread -- the resulting Vec<f32> can then be fed to a
    /// SamplesBuffer so the audio callback thread only does fast memory reads
    /// (no symphonia decoding on the hot path).
    fn decode_to_pcm(audio_bytes: &[u8]) -> Result<(Vec<f32>, u16, u32)> {
        let cursor = Cursor::new(audio_bytes.to_vec());
        let decoder = Decoder::builder()
            .with_data(cursor)
            .build()
            .map_err(|e| color_eyre::eyre::eyre!("Failed to decode audio: {}", e))?;

        let channels = decoder.channels();
        let sample_rate = decoder.sample_rate();
        let samples: Vec<f32> = decoder.collect();

        let duration_secs = if channels > 0 && sample_rate > 0 {
            samples.len() as f64 / (channels as f64 * sample_rate as f64)
        } else {
            0.0
        };
        tracing::info!(
            sample_count = samples.len(),
            channels,
            sample_rate,
            duration_secs = format!("{:.1}", duration_secs),
            "Pre-decoded audio to PCM"
        );

        Ok((samples, channels, sample_rate))
    }

    /// Load audio bytes and start playback.
    ///
    /// Stops any currently playing track, pre-decodes the audio bytes to
    /// raw PCM, and appends a SamplesBuffer to the Sink. The Sink auto-plays
    /// when a source is appended.
    ///
    /// After calling stop() on a Sink, appending blocks until the queue
    /// flushes, so we create a fresh Sink for each new track to avoid any
    /// blocking issues.
    ///
    /// The `volume` parameter restores the user's saved volume level on the
    /// fresh Sink (each new Sink starts at 1.0, so we must explicitly set it).
    pub fn load_and_play(
        &mut self,
        audio_bytes: Vec<u8>,
        track_name: String,
        volume: f32,
    ) -> Result<()> {
        // Stop current playback
        self.main_sink.stop();

        // Create a fresh Sink connected to the same output stream.
        // This avoids the blocking behavior of append-after-stop.
        self.main_sink = Sink::connect_new(self._stream.mixer());

        // Restore the user's saved volume level on the new Sink.
        // Each new Sink starts at volume 1.0, so we must explicitly set it.
        self.main_sink.set_volume(volume.clamp(0.0, 1.0));

        // Pre-decode to raw PCM samples. This runs the symphonia decoder NOW
        // (on the calling thread), so the audio callback thread only does fast
        // memory reads from the Vec<f32>. No VisualizerSource wrapper — the UI
        // thread reads visualization samples directly from decoded_pcm using
        // the playback position, keeping the audio thread completely clean.
        let (samples, channels, sample_rate) = Self::decode_to_pcm(&audio_bytes)?;
        self.decoded_pcm = Some((samples.clone(), channels, sample_rate));

        // Feed pre-decoded PCM directly to the Sink — zero overhead on audio thread
        let source = SamplesBuffer::new(channels, sample_rate, samples);
        self.main_sink.append(source);

        // Store compressed bytes for re-download avoidance and status display
        self._audio_data = Some(audio_bytes);
        self.current_track = Some(track_name);

        tracing::info!(
            track = self.current_track.as_deref().unwrap_or("unknown"),
            "Playback started"
        );

        Ok(())
    }

    /// Toggle between paused and playing states.
    ///
    /// Per locked decision: spacebar toggles play/pause. This is the only
    /// playback control in Phase 1.
    pub fn toggle_pause(&self) {
        if self.main_sink.is_paused() {
            self.main_sink.play();
            tracing::info!("Playback resumed");
        } else {
            self.main_sink.pause();
            tracing::info!("Playback paused");
        }
    }

    /// Returns true if the Sink is currently paused.
    pub fn is_paused(&self) -> bool {
        self.main_sink.is_paused()
    }

    /// Returns true if audio is actively playing (not paused, not empty).
    pub fn is_playing(&self) -> bool {
        !self.main_sink.is_paused() && !self.main_sink.empty()
    }

    /// Returns true if the current track has finished playing.
    pub fn is_finished(&self) -> bool {
        self.main_sink.empty()
    }

    /// Returns the name of the currently loaded track, if any.
    pub fn current_track_name(&self) -> Option<&str> {
        self.current_track.as_deref()
    }

    /// Get the current volume level (0.0 to 1.0).
    ///
    /// Delegates directly to the rodio Sink.
    pub fn volume(&self) -> f32 {
        self.main_sink.volume()
    }

    /// Increase volume by 0.05, clamped to 1.0 max.
    ///
    /// Values above 1.0 cause audio clipping, so we cap at 1.0.
    pub fn volume_up(&self) {
        self.main_sink.set_volume((self.main_sink.volume() + 0.05).min(1.0));
    }

    /// Decrease volume by 0.05, clamped to 0.0 min.
    pub fn volume_down(&self) {
        self.main_sink.set_volume((self.main_sink.volume() - 0.05).max(0.0));
    }

    /// Get the current playback position.
    ///
    /// Note: can briefly exceed track duration near end of playback.
    /// Callers must clamp when using for progress calculations.
    pub fn get_pos(&self) -> std::time::Duration {
        self.main_sink.get_pos()
    }

    /// Set volume directly to a specific level, clamped to 0.0..=1.0.
    ///
    /// Used by app.rs to restore the saved volume after creating a new Sink
    /// (each new Sink starts at volume 1.0).
    pub fn set_volume(&self, vol: f32) {
        self.main_sink.set_volume(vol.clamp(0.0, 1.0));
    }

    /// Seek forward by SEEK_STEP (5 seconds), clamped to track duration.
    ///
    /// Recreates the SamplesBuffer from the pre-decoded PCM at the target
    /// sample offset. This avoids try_seek on VisualizerSource (which doesn't
    /// implement it) and keeps the decoder off the audio thread.
    pub fn seek_forward(&mut self, track_duration_ms: u64) -> Result<()> {
        let current = self.main_sink.get_pos();
        let max = Duration::from_millis(track_duration_ms);
        let target = (current + SEEK_STEP).min(max);
        self.seek_to(target)
    }

    /// Seek backward by SEEK_STEP (5 seconds), saturating at 0.
    ///
    /// Recreates the SamplesBuffer from the pre-decoded PCM at the target
    /// sample offset.
    pub fn seek_backward(&mut self) -> Result<()> {
        let current = self.main_sink.get_pos();
        let target = current.saturating_sub(SEEK_STEP);
        self.seek_to(target)
    }

    /// Seek to a specific position by recreating the SamplesBuffer from
    /// the pre-decoded PCM at the target sample offset.
    ///
    /// Stops the current main_sink, creates a fresh Sink, and appends a
    /// new SamplesBuffer starting from the calculated sample offset.
    fn seek_to(&mut self, target: Duration) -> Result<()> {
        let (samples, channels, sample_rate) = self
            .decoded_pcm
            .as_ref()
            .ok_or_else(|| color_eyre::eyre::eyre!("No decoded PCM data for seeking"))?;
        let channels = *channels;
        let sample_rate = *sample_rate;

        // Calculate sample offset from target duration
        let mut offset =
            (target.as_secs_f64() * sample_rate as f64 * channels as f64) as usize;
        // Clamp to buffer length
        offset = offset.min(samples.len());
        // Align to channel boundary
        let ch = channels as usize;
        offset -= offset % ch;

        let volume = self.main_sink.volume();
        let remaining_samples = samples[offset..].to_vec();

        // Stop current sink and create a fresh one
        self.main_sink.stop();
        self.main_sink = Sink::connect_new(self._stream.mixer());
        self.main_sink.set_volume(volume);

        // Create SamplesBuffer from the offset position — fed directly to Sink
        let source = SamplesBuffer::new(channels, sample_rate, remaining_samples);
        self.main_sink.append(source);

        tracing::info!(
            target_secs = format!("{:.1}", target.as_secs_f64()),
            "Seeked to position"
        );
        Ok(())
    }

    /// Replay the current track from cached pre-decoded PCM (Repeat One mode).
    ///
    /// Avoids re-downloading AND re-decoding by cloning from the cached
    /// decoded PCM samples. Creates a fresh Sink to avoid blocking issues
    /// after stop().
    pub fn replay_current(
        &mut self,
        volume: f32,
    ) -> Result<()> {
        let (samples, channels, sample_rate) = self
            .decoded_pcm
            .clone()
            .ok_or_else(|| color_eyre::eyre::eyre!("No decoded PCM data to replay"))?;

        // Stop current playback and create a fresh Sink
        self.main_sink.stop();
        self.main_sink = Sink::connect_new(self._stream.mixer());
        self.main_sink.set_volume(volume.clamp(0.0, 1.0));

        // Feed pre-decoded PCM directly to the Sink
        let source = SamplesBuffer::new(channels, sample_rate, samples);
        self.main_sink.append(source);

        tracing::info!("Replaying track from cached PCM (Repeat One)");
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Ambient channel methods
    // -----------------------------------------------------------------------

    /// Load an ambient track from audio bytes and start playback.
    ///
    /// Stops any currently playing ambient track, creates a fresh Sink on the
    /// shared mixer, pre-decodes the audio to PCM, and starts playback via
    /// SamplesBuffer. The decoded PCM is cached for loop replay (manual loop
    /// to avoid `repeat_infinite()` memory leak).
    ///
    /// Note: Ambient does NOT use VisualizerSource (visualizer taps main
    /// channel only).
    pub fn load_ambient(
        &mut self,
        audio_bytes: Vec<u8>,
        track_name: String,
        volume: f32,
    ) -> Result<()> {
        // Stop old ambient sink if any
        if let Some(ref sink) = self.ambient_sink {
            sink.stop();
        }

        // Create fresh sink on the shared mixer
        let new_sink = Sink::connect_new(self._stream.mixer());
        new_sink.set_volume(volume.clamp(0.0, 1.0));

        // Pre-decode to PCM and play via SamplesBuffer
        let (samples, channels, sample_rate) = Self::decode_to_pcm(&audio_bytes)?;
        self.ambient_decoded_pcm = Some((samples.clone(), channels, sample_rate));

        let source = SamplesBuffer::new(channels, sample_rate, samples);
        new_sink.append(source);

        // Store sink and cached data
        let name = track_name.clone();
        self.ambient_sink = Some(new_sink);
        self.ambient_audio_data = Some(audio_bytes);
        self.ambient_track_name = Some(track_name);

        tracing::info!(channel = "ambient", track = %name, "Ambient track loaded (pre-decoded PCM)");
        Ok(())
    }

    /// Stop ambient playback and clear all ambient state.
    pub fn stop_ambient(&mut self) {
        if let Some(ref sink) = self.ambient_sink {
            sink.stop();
        }
        self.ambient_sink = None;
        self.ambient_audio_data = None;
        self.ambient_decoded_pcm = None;
        self.ambient_track_name = None;
        tracing::info!(channel = "ambient", "Ambient stopped");
    }

    /// Returns true if the ambient sink has finished playing (queue empty).
    ///
    /// Returns false when no ambient is loaded (no ambient = not "finished",
    /// because there is nothing to finish).
    pub fn is_ambient_finished(&self) -> bool {
        self.ambient_sink.as_ref().is_some_and(|s| s.empty())
    }

    /// Returns true if an ambient sink is loaded (regardless of playing state).
    ///
    /// Used by App's volume budget to decide whether to include ambient_volume
    /// in the budget calculation. When no ambient sink exists, main should
    /// play at full volume without budget scaling.
    pub fn has_ambient_sink(&self) -> bool {
        self.ambient_sink.is_some()
    }

    /// Returns true if cached ambient audio data exists (for loop re-decode).
    pub fn has_ambient_data(&self) -> bool {
        self.ambient_audio_data.is_some()
    }

    /// Replay the ambient track from cached pre-decoded PCM (manual loop).
    ///
    /// Stops the old ambient sink, creates a fresh one on the shared mixer,
    /// and replays from the cached decoded PCM samples (no re-decode needed).
    /// Falls back to re-decoding from compressed bytes if decoded PCM is
    /// unavailable. This avoids rodio's `repeat_infinite()` memory leak (#673).
    pub fn replay_ambient(&mut self, volume: f32) -> Result<()> {
        // Stop old ambient sink and create fresh one
        if let Some(ref sink) = self.ambient_sink {
            sink.stop();
        }
        let new_sink = Sink::connect_new(self._stream.mixer());
        new_sink.set_volume(volume.clamp(0.0, 1.0));

        // Prefer cached decoded PCM; fall back to re-decoding from compressed bytes
        if let Some((ref samples, channels, sample_rate)) = self.ambient_decoded_pcm {
            let source = SamplesBuffer::new(channels, sample_rate, samples.clone());
            new_sink.append(source);
        } else if let Some(ref audio_bytes) = self.ambient_audio_data {
            tracing::warn!(
                channel = "ambient",
                "Decoded PCM unavailable, falling back to re-decode from compressed bytes"
            );
            let (samples, channels, sample_rate) = Self::decode_to_pcm(audio_bytes)?;
            self.ambient_decoded_pcm = Some((samples.clone(), channels, sample_rate));
            let source = SamplesBuffer::new(channels, sample_rate, samples);
            new_sink.append(source);
        } else {
            return Err(color_eyre::eyre::eyre!("No ambient audio data to replay"));
        }

        self.ambient_sink = Some(new_sink);
        tracing::info!(channel = "ambient", "Ambient track loop restarted (cached PCM)");
        Ok(())
    }

    /// Set the ambient sink volume directly via Sink::set_volume().
    ///
    /// Earlier implementation recreated the entire sink on each volume change
    /// (stop old, create new, re-decode cached bytes) to work around a
    /// suspected Sink::set_volume() reliability issue. Testing showed that
    /// the recreation approach causes playback to restart from the beginning
    /// on every [/] keypress, which is unacceptable UX for ambient tracks.
    ///
    /// Direct Sink::set_volume() works correctly for ambient sinks in
    /// practice -- the volume change is applied immediately without
    /// interrupting playback position. If reliability issues resurface on
    /// specific configurations, the recreation approach can be revisited
    /// with elapsed-time tracking to preserve position.
    pub fn set_ambient_volume(&mut self, vol: f32) {
        if let Some(ref sink) = self.ambient_sink {
            sink.set_volume(vol.clamp(0.0, 1.0));
        }
    }

    /// Get the current ambient sink volume, or None if no ambient sink exists.
    pub fn ambient_volume(&self) -> Option<f32> {
        self.ambient_sink.as_ref().map(|s| s.volume())
    }

    /// Get the name of the currently loaded ambient track, if any.
    pub fn ambient_track_name(&self) -> Option<&str> {
        self.ambient_track_name.as_deref()
    }

    /// Set the main sink volume directly, clamped to 0.0..=1.0.
    ///
    /// Used by App's volume budget enforcement to set the computed volume
    /// on the main channel after budget + master scaling. Replaces the
    /// old `set_volume` method for clarity now that there are two channels.
    pub fn set_main_volume(&self, vol: f32) {
        self.main_sink.set_volume(vol.clamp(0.0, 1.0));
    }

    /// Read visualization samples from the pre-decoded PCM at the current
    /// playback position. Called by the UI thread — no audio thread overhead.
    ///
    /// Returns `fft_size` mono samples (left channel extracted from interleaved
    /// PCM) and the sample rate if a track is currently loaded. Returns None
    /// if no track is loaded or playback is paused.
    pub fn get_visualizer_samples(&self, fft_size: usize) -> Option<(Vec<f32>, u32)> {
        if self.main_sink.is_paused() || self.main_sink.empty() {
            return None;
        }

        let (samples, channels, sample_rate) = self.decoded_pcm.as_ref()?;
        let ch = *channels as usize;
        let sr = *sample_rate;

        // Calculate the sample offset from playback position
        let pos = self.main_sink.get_pos();
        let frame_offset = (pos.as_secs_f64() * sr as f64) as usize;
        let sample_offset = frame_offset * ch;

        if sample_offset >= samples.len() {
            return None;
        }

        // Extract mono (left channel) samples for FFT
        let mut mono = Vec::with_capacity(fft_size);
        let mut i = sample_offset;
        while mono.len() < fft_size && i < samples.len() {
            mono.push(samples[i]);
            i += ch; // skip to next frame (left channel only)
        }

        // Pad with zeros if we don't have enough samples
        mono.resize(fft_size, 0.0);

        Some((mono, sr))
    }
}

// ---------------------------------------------------------------------------
// WSL2 audio helpers
// ---------------------------------------------------------------------------

/// Detect whether we are running inside WSL2.
///
/// Checks for the WSL-specific kernel version string in /proc/version.
fn is_wsl2() -> bool {
    std::fs::read_to_string("/proc/version")
        .map(|v| v.contains("microsoft") || v.contains("WSL"))
        .unwrap_or(false)
}

/// Check if the ALSA PulseAudio plugin library is installed.
///
/// On Debian/Ubuntu this is provided by `libasound2-plugins`. Without it,
/// ALSA cannot route audio through PulseAudio (required on WSL2).
fn alsa_pulse_plugin_exists() -> bool {
    // Check common library paths for the ALSA PulseAudio PCM plugin
    let paths = [
        "/usr/lib/x86_64-linux-gnu/alsa-lib/libasound_module_pcm_pulse.so",
        "/usr/lib/alsa-lib/libasound_module_pcm_pulse.so",
        "/usr/lib/aarch64-linux-gnu/alsa-lib/libasound_module_pcm_pulse.so",
    ];
    paths.iter().any(|p| std::path::Path::new(p).exists())
}

/// Marker line used to identify TermTunes-generated .asoundrc files.
/// If the file contains this marker, it is safe to overwrite with an
/// updated configuration (e.g., improved buffer settings).
const ASOUNDRC_MARKER: &str = "# Auto-generated by TermTunes";

/// Version tag embedded in .asoundrc so we can detect stale configs and
/// upgrade them automatically. Bump this when the ALSA config changes.
const ASOUNDRC_VERSION: &str = "# termtunes-asoundrc-v3";

/// Ensure ALSA is configured to route through PulseAudio on WSL2 with
/// buffer sizes tuned to avoid crackling/clicking artifacts.
///
/// Creates `~/.asoundrc` if it does not already exist, or upgrades a
/// previously generated TermTunes config (detected by marker comment) to
/// the latest version with improved buffer settings.
///
/// If the file exists and was NOT created by TermTunes, it is left
/// untouched (the user may have custom ALSA configuration).
fn ensure_alsa_pulse_config() -> Result<()> {
    let home = std::env::var("HOME").unwrap_or_else(|_| String::from("/root"));
    let asoundrc = PathBuf::from(&home).join(".asoundrc");

    if asoundrc.exists() {
        // Read existing config to check if it was generated by TermTunes
        let existing = std::fs::read_to_string(&asoundrc).unwrap_or_default();

        if !existing.contains(ASOUNDRC_MARKER) {
            // User-managed config -- do not touch
            tracing::debug!(
                path = %asoundrc.display(),
                "ALSA config exists and was not generated by TermTunes, skipping"
            );
            return Ok(());
        }

        if existing.contains(ASOUNDRC_VERSION) {
            // Already up to date
            tracing::debug!(
                path = %asoundrc.display(),
                "ALSA config is current version, no update needed"
            );
            return Ok(());
        }

        // TermTunes config exists but is outdated -- upgrade it
        tracing::info!(
            path = %asoundrc.display(),
            "Upgrading TermTunes ALSA config with improved buffer settings"
        );
    }

    // ALSA configuration that routes audio through PulseAudio for WSL2's
    // WSLg audio bridge. The `type pulse` PCM plugin does not accept
    // buffer_size/period_size parameters directly -- PulseAudio buffer
    // tuning is handled via PULSE_LATENCY_MSEC (set in main.rs) and
    // the cpal buffer size (set in Player::new via OutputStreamBuilder).
    let config = format!(
        "\
{ASOUNDRC_MARKER} for WSL2 audio support.
{ASOUNDRC_VERSION}
# Routes ALSA audio through PulseAudio (provided by WSLg) with buffer
# sizes tuned to prevent crackling on WSL2.
# Delete this file if you want to manage ALSA configuration manually.

# Use PulseAudio plugin with tuned buffer sizes to prevent underruns.
# The WSLg PulseAudio bridge needs larger buffers than a native setup.
pcm.!default {{
    type pulse
    hint {{
        show on
        description \"Default ALSA Output (PulseAudio via WSLg)\"
    }}
}}

ctl.!default {{
    type pulse
}}
"
    );

    std::fs::write(&asoundrc, &config)?;
    tracing::info!(
        path = %asoundrc.display(),
        "Created ALSA config for WSL2 PulseAudio routing with buffer tuning"
    );

    Ok(())
}
