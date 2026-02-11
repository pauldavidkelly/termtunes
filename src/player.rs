use std::io::Cursor;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use color_eyre::Result;
use rodio::{Decoder, OutputStream, OutputStreamBuilder, Sink};

use crate::visualizer::{VisualizerData, VisualizerSource, FFT_SIZE};

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

    /// Name of the currently playing track (for status bar display).
    current_track: Option<String>,

    /// Ambient channel sink -- None when no ambient track is loaded.
    ambient_sink: Option<Sink>,

    /// Raw audio bytes of the ambient track, kept for loop re-decode.
    ambient_audio_data: Option<Vec<u8>>,

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

        let stream = OutputStreamBuilder::open_default_stream().map_err(|e| {
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
            current_track: None,
            ambient_sink: None,
            ambient_audio_data: None,
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

    /// Load audio bytes and start playback.
    ///
    /// Stops any currently playing track, creates a Decoder from the audio
    /// bytes (via Cursor<Vec<u8>>), and appends to the Sink. The Sink
    /// auto-plays when a source is appended.
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
        visualizer_data: Option<Arc<Mutex<VisualizerData>>>,
    ) -> Result<()> {
        // Stop current playback
        self.main_sink.stop();

        // Create a fresh Sink connected to the same output stream.
        // This avoids the blocking behavior of append-after-stop.
        self.main_sink = Sink::connect_new(self._stream.mixer());

        // Restore the user's saved volume level on the new Sink.
        // Each new Sink starts at volume 1.0, so we must explicitly set it.
        self.main_sink.set_volume(volume.clamp(0.0, 1.0));

        // Decode and play.
        // Use the builder with byte_len and seekable so the symphonia backend
        // knows the stream length and supports backward seeking (try_seek to
        // an earlier position). Without this, only forward seeks succeed.
        let byte_len = audio_bytes.len() as u64;
        let cursor = Cursor::new(audio_bytes.clone());
        let source = Decoder::builder()
            .with_data(cursor)
            .with_byte_len(byte_len)
            .with_seekable(true)
            .build()
            .map_err(|e| color_eyre::eyre::eyre!("Failed to decode audio: {}", e))?;

        // Wrap with visualizer tap if data is provided. The VisualizerSource
        // copies samples to the shared buffer as they flow through, enabling
        // real-time FFT visualization without affecting audio quality.
        if let Some(data) = visualizer_data {
            let viz_source = VisualizerSource::new(source, data, FFT_SIZE);
            self.main_sink.append(viz_source);
        } else {
            self.main_sink.append(source);
        }

        // Store for potential re-creation and status display
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
    /// Uses rodio's try_seek which may not be supported by all decoders.
    /// Callers should handle the error gracefully (log and ignore).
    pub fn seek_forward(&self, track_duration_ms: u64) -> Result<(), rodio::source::SeekError> {
        let current = self.main_sink.get_pos();
        let max = Duration::from_millis(track_duration_ms);
        let target = (current + SEEK_STEP).min(max);
        self.main_sink.try_seek(target)
    }

    /// Seek backward by SEEK_STEP (5 seconds), saturating at 0.
    ///
    /// Uses rodio's try_seek which may not be supported by all decoders.
    /// Callers should handle the error gracefully (log and ignore).
    pub fn seek_backward(&self) -> Result<(), rodio::source::SeekError> {
        let current = self.main_sink.get_pos();
        let target = current.saturating_sub(SEEK_STEP);
        self.main_sink.try_seek(target)
    }

    /// Replay the current track from cached audio bytes (Repeat One mode).
    ///
    /// Avoids re-downloading the track by re-decoding from the in-memory
    /// audio data. Creates a fresh Sink to avoid blocking issues after stop().
    pub fn replay_current(
        &mut self,
        volume: f32,
        visualizer_data: Option<Arc<Mutex<VisualizerData>>>,
    ) -> Result<()> {
        let audio_bytes = self
            ._audio_data
            .clone()
            .ok_or_else(|| color_eyre::eyre::eyre!("No audio data to replay"))?;

        // Stop current playback and create a fresh Sink
        self.main_sink.stop();
        self.main_sink = Sink::connect_new(self._stream.mixer());
        self.main_sink.set_volume(volume.clamp(0.0, 1.0));

        // Decode from cached bytes and start playback.
        // Use the builder with byte_len and seekable for backward seek support.
        let byte_len = audio_bytes.len() as u64;
        let source = Decoder::builder()
            .with_data(Cursor::new(audio_bytes))
            .with_byte_len(byte_len)
            .with_seekable(true)
            .build()
            .map_err(|e| color_eyre::eyre::eyre!("Failed to decode audio for replay: {}", e))?;

        // Wrap with visualizer tap if data is provided
        if let Some(data) = visualizer_data {
            let viz_source = VisualizerSource::new(source, data, FFT_SIZE);
            self.main_sink.append(viz_source);
        } else {
            self.main_sink.append(source);
        }

        tracing::info!("Replaying track (Repeat One)");
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Ambient channel methods
    // -----------------------------------------------------------------------

    /// Load an ambient track from audio bytes and start playback.
    ///
    /// Stops any currently playing ambient track, creates a fresh Sink on the
    /// shared mixer, decodes the audio, and starts playback. The raw audio
    /// bytes are cached for loop re-decode (manual loop to avoid
    /// `repeat_infinite()` memory leak).
    ///
    /// Note: Ambient does NOT use VisualizerSource (visualizer taps main
    /// channel only) and does NOT need seekable decoding (no seeking on
    /// ambient tracks).
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

        // Decode audio bytes (no byte_len/seekable needed for ambient)
        let cursor = Cursor::new(audio_bytes.clone());
        let source = Decoder::builder()
            .with_data(cursor)
            .build()
            .map_err(|e| color_eyre::eyre::eyre!("Failed to decode ambient audio: {}", e))?;
        new_sink.append(source);

        // Store sink and cached data for loop re-decode
        let name = track_name.clone();
        self.ambient_sink = Some(new_sink);
        self.ambient_audio_data = Some(audio_bytes);
        self.ambient_track_name = Some(track_name);

        tracing::info!(channel = "ambient", track = %name, "Ambient track loaded");
        Ok(())
    }

    /// Stop ambient playback and clear all ambient state.
    pub fn stop_ambient(&mut self) {
        if let Some(ref sink) = self.ambient_sink {
            sink.stop();
        }
        self.ambient_sink = None;
        self.ambient_audio_data = None;
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

    /// Replay the ambient track from cached bytes (manual loop mechanism).
    ///
    /// Stops the old ambient sink, creates a fresh one on the shared mixer,
    /// re-decodes from the cached compressed bytes, and starts playback.
    /// This avoids rodio's `repeat_infinite()` memory leak (issue #673).
    pub fn replay_ambient(&mut self, volume: f32) -> Result<()> {
        let audio_bytes = self
            .ambient_audio_data
            .clone()
            .ok_or_else(|| color_eyre::eyre::eyre!("No ambient audio data to replay"))?;

        // Stop old ambient sink and create fresh one
        if let Some(ref sink) = self.ambient_sink {
            sink.stop();
        }
        let new_sink = Sink::connect_new(self._stream.mixer());
        new_sink.set_volume(volume.clamp(0.0, 1.0));

        // Decode from cached bytes
        let cursor = Cursor::new(audio_bytes);
        let source = Decoder::builder()
            .with_data(cursor)
            .build()
            .map_err(|e| {
                color_eyre::eyre::eyre!("Failed to decode ambient audio for replay: {}", e)
            })?;
        new_sink.append(source);

        self.ambient_sink = Some(new_sink);
        tracing::info!(channel = "ambient", "Ambient track loop restarted");
        Ok(())
    }

    /// Set the ambient sink volume, clamped to 0.0..=1.0.
    pub fn set_ambient_volume(&self, vol: f32) {
        if let Some(ref sink) = self.ambient_sink {
            sink.set_volume(vol.clamp(0.0, 1.0));
        }
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
const ASOUNDRC_VERSION: &str = "# termtunes-asoundrc-v2";

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

    // ALSA configuration that routes audio through PulseAudio with buffer
    // sizes tuned for WSL2's WSLg PulseAudio bridge. Without explicit
    // buffer tuning, the default ALSA period/buffer sizes are too small
    // for the WSL2 PulseAudio shim, causing audible crackling and
    // clicking artifacts (buffer underruns).
    //
    // buffer_size 8192 (4 periods of 2048 frames) at 44100 Hz gives
    // ~186ms of buffer which is large enough to absorb WSL2 scheduling
    // jitter without perceptible latency for music playback.
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
    fallback \"sysdefault\"
    hint {{
        show on
        description \"Default ALSA Output (PulseAudio via WSLg)\"
    }}
}}

ctl.!default {{
    type pulse
    fallback \"sysdefault\"
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
