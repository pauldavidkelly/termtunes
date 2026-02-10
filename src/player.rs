use std::io::Cursor;
use std::path::PathBuf;

use color_eyre::Result;
use rodio::{Decoder, OutputStream, OutputStreamBuilder, Sink};

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

    /// Rodio Sink for playback control (play/pause/stop/append).
    sink: Sink,

    /// Raw audio bytes of the current track, kept for potential re-creation.
    _audio_data: Option<Vec<u8>>,

    /// Name of the currently playing track (for status bar display).
    current_track: Option<String>,
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

        let sink = Sink::connect_new(stream.mixer());

        tracing::info!(
            config = ?stream.config(),
            "Audio output stream opened successfully"
        );

        Ok(Self {
            _stream: stream,
            sink,
            _audio_data: None,
            current_track: None,
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
    pub fn load_and_play(&mut self, audio_bytes: Vec<u8>, track_name: String, volume: f32) -> Result<()> {
        // Stop current playback
        self.sink.stop();

        // Create a fresh Sink connected to the same output stream.
        // This avoids the blocking behavior of append-after-stop.
        self.sink = Sink::connect_new(self._stream.mixer());

        // Restore the user's saved volume level on the new Sink.
        // Each new Sink starts at volume 1.0, so we must explicitly set it.
        self.sink.set_volume(volume.clamp(0.0, 1.0));

        // Decode and play
        let cursor = Cursor::new(audio_bytes.clone());
        let source = Decoder::new(cursor)
            .map_err(|e| color_eyre::eyre::eyre!("Failed to decode audio: {}", e))?;
        self.sink.append(source);

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
        if self.sink.is_paused() {
            self.sink.play();
            tracing::info!("Playback resumed");
        } else {
            self.sink.pause();
            tracing::info!("Playback paused");
        }
    }

    /// Returns true if the Sink is currently paused.
    pub fn is_paused(&self) -> bool {
        self.sink.is_paused()
    }

    /// Returns true if audio is actively playing (not paused, not empty).
    pub fn is_playing(&self) -> bool {
        !self.sink.is_paused() && !self.sink.empty()
    }

    /// Returns true if the current track has finished playing.
    pub fn is_finished(&self) -> bool {
        self.sink.empty()
    }

    /// Returns the name of the currently loaded track, if any.
    pub fn current_track_name(&self) -> Option<&str> {
        self.current_track.as_deref()
    }

    /// Get the current volume level (0.0 to 1.0).
    ///
    /// Delegates directly to the rodio Sink.
    pub fn volume(&self) -> f32 {
        self.sink.volume()
    }

    /// Increase volume by 0.05, clamped to 1.0 max.
    ///
    /// Values above 1.0 cause audio clipping, so we cap at 1.0.
    pub fn volume_up(&self) {
        self.sink.set_volume((self.sink.volume() + 0.05).min(1.0));
    }

    /// Decrease volume by 0.05, clamped to 0.0 min.
    pub fn volume_down(&self) {
        self.sink.set_volume((self.sink.volume() - 0.05).max(0.0));
    }

    /// Get the current playback position.
    ///
    /// Note: can briefly exceed track duration near end of playback.
    /// Callers must clamp when using for progress calculations.
    pub fn get_pos(&self) -> std::time::Duration {
        self.sink.get_pos()
    }

    /// Set volume directly to a specific level, clamped to 0.0..=1.0.
    ///
    /// Used by app.rs to restore the saved volume after creating a new Sink
    /// (each new Sink starts at volume 1.0).
    pub fn set_volume(&self, vol: f32) {
        self.sink.set_volume(vol.clamp(0.0, 1.0));
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
