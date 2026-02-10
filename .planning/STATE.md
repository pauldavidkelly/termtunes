# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-02-08)

**Core value:** Keep music playback inside the terminal workflow - no context switching to external apps, everything stays in Tmux.
**Current focus:** Phase 5 COMPLETE -- Audio Visualizer (FINAL PHASE)

## Current Position

Phase: 5 of 5 (Audio Visualizer) -- COMPLETE
Plan: 1 of 1 in current phase (05-01 complete)
Status: ALL PHASES COMPLETE -- v1.0 feature set shipped
Last activity: 2026-02-10 -- Completed 05-01 (Audio Visualizer)

Progress: [████████████████████] 100% (All Phases)

## Performance Metrics

**Velocity:**
- Total plans completed: 10
- Average duration: ~9 min
- Total execution time: ~1.55 hours

**By Phase:**

| Phase | Plans | Total | Avg/Plan |
|-------|-------|-------|----------|
| 01-foundation-audio-poc | 3/3 | ~58 min | ~19 min |
| 02-core-tui-playback | 2/2 | ~14 min | ~7 min |
| 03-differentiators | 2/2 | ~11 min | ~5.5 min |
| 04-tmux-integration-polish | 2/2 | ~4 min | ~2 min |
| 05-audio-visualizer | 1/1 | ~5 min | ~5 min |

**Recent Trend:**
- Last 5 plans: 03-02 (~8 min), 04-01 (~2 min), 04-02 (~2 min), 05-01 (~5 min)
- Trend: Plans consistently fast as codebase is well-understood and plans are precise

*Updated after each plan completion*

## Accumulated Context

### Decisions

Decisions are logged in PROJECT.md Key Decisions table.
Recent decisions affecting current work:

- [Roadmap]: Rust stack selected (ratatui + rodio + crossterm + reqwest) per research synthesis
- [Roadmap]: WSL2 audio PoC before any UI work -- highest risk validated first
- [Roadmap]: 5 phases, standard depth, 42 v1 requirements mapped
- [01-01]: Ctrl+C handled as crossterm KeyEvent in raw mode (signal-hook only catches external SIGINT)
- [01-01]: color_eyre::Result used throughout for consistent error handling
- [01-01]: Tracing output to ~/.local/share/termtunes/termtunes.log (keeps TUI clean)
- [01-01]: Config file permissions 0o600 (will store auth tokens later)
- [01-02]: Auth flow runs before TUI init on normal terminal (not alternate screen) so URL is readable
- [01-02]: tokio::main for async runtime, event loop still uses synchronous crossterm polling
- [01-02]: Server configs keyed by machine identifier (clientIdentifier) in config HashMap
- [01-02]: AppView enum state machine for Playlists/Tracks navigation views
- [01-02]: reqwest "query" feature required for URL query parameters (.query() is feature-gated)
- [01-03]: Download full track into Vec<u8> before playback (not streaming) for WSL2 reliability
- [01-03]: PULSE_LATENCY_MSEC=60 set at startup for WSL2 audio latency
- [01-03]: rodio OutputStream kept alive for entire app lifetime (dropping kills audio)
- [01-03]: Background download via std::thread::spawn + mpsc channel for non-blocking UI
- [01-03]: Audio buffer size tuned larger for WSL2 PulseAudio stability (eliminates crackling)
- [01-03]: Player initialization deferred until first playback (lazy init)
- [02-01]: Volume capped at 1.0 (no amplification) to prevent audio clipping
- [02-01]: Volume step 0.05 (5% per key press) for fine-grained control
- [02-01]: Track navigation wraps around (last->first, first->last)
- [02-01]: saved_volume stored in App struct, restored on each new Sink creation
- [02-01]: NowPlaying metadata populated from Plex Track data in check_download_complete
- [02-02]: State icons >> (green, playing), || (yellow, paused), -- (gray, stopped) for instant visual feedback
- [02-02]: LineGauge ratio clamped to 0.0..=1.0 to prevent panic when get_pos() exceeds duration
- [02-02]: Error messages shown in player bar line 3 (red) instead of replacing entire bar
- [02-02]: current_track_index() accessor added to app.rs for UI track highlighting
- [03-01]: Shuffle uses index array with current track at position 0 on toggle
- [03-01]: Repeat One replays from cached _audio_data bytes (no re-download)
- [03-01]: Seek keybindings (h/l/Left/Right) only active in Playing view
- [03-01]: User skip (n/N) ignores RepeatMode::One -- only auto-advance replays
- [03-01]: prev_track always wraps regardless of repeat mode
- [03-02]: Favorites keyed by string '1'-'9' in config HashMap for TOML serialization
- [03-02]: Two-key modal: press f enters awaiting_favorite_key state, then 1-9 assigns
- [03-02]: Favorite activation (1-9) works from any view, assignment (f) only from Playlists view
- [03-02]: Shuffle indicator in magenta, repeat indicator in blue for visual distinction
- [04-01]: Width thresholds as constants: MIN_WIDTH=20, MIN_HEIGHT=5, NARROW_WIDTH=40
- [04-01]: Playlist truncation drops track count suffix first, then truncates title with ellipsis
- [04-01]: Narrow player bar: line 1 = icon + track name only; line 3 = state + time only
- [04-01]: truncate_for_display uses .chars().take() for UTF-8 safety (not byte-based truncation)
- [04-02]: Session saved only on graceful exit (q or signal), not per track change
- [04-02]: now_playing file writes are best-effort (tracing::warn on failure, never propagated)
- [04-02]: Session restore does NOT auto-play -- positions user at saved track in Tracks view
- [04-02]: go_back() preserves session-relevant fields so save works from any view
- [04-02]: RepeatMode string conversion via to_string_repr/from_string_repr for TOML serialization
- [05-01]: spectrum-analyzer crate for FFT (wraps microfft with windowing and scaling)
- [05-01]: try_lock() in audio thread to never block playback (skip sample on lock failure)
- [05-01]: FFT computed on UI thread at render tick rate (~10Hz), not on audio thread
- [05-01]: Auto-hide visualizer in narrow (<40 cols) and short (<20 rows) terminals
- [05-01]: 32 default bars with dynamic width-based adjustment (4..64 range)

### Pending Todos

None yet.

### Blockers/Concerns

- ~~WSL2 audio reliability (PulseAudio pause/resume >5s) is unvalidated -- Phase 1 must prove this works~~ RESOLVED: Validated in 01-03, pause/resume works reliably after >5s on WSL2

## Session Continuity

Last session: 2026-02-10
Stopped at: Completed 05-01-PLAN.md (Audio Visualizer) -- ALL PHASES COMPLETE
Resume file: .planning/phases/05-audio-visualizer/05-01-SUMMARY.md
