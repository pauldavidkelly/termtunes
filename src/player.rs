use std::io::Cursor;

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
    /// Uses OutputStreamBuilder::open_default_stream() which tries the default
    /// device first, then falls back to alternative devices/configs.
    pub fn new() -> Result<Self> {
        let stream = OutputStreamBuilder::open_default_stream()
            .map_err(|e| color_eyre::eyre::eyre!("Failed to open audio output: {}", e))?;
        let sink = Sink::connect_new(stream.mixer());

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
    pub fn load_and_play(&mut self, audio_bytes: Vec<u8>, track_name: String) -> Result<()> {
        // Stop current playback
        self.sink.stop();

        // Create a fresh Sink connected to the same output stream.
        // This avoids the blocking behavior of append-after-stop.
        self.sink = Sink::connect_new(self._stream.mixer());

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
}
