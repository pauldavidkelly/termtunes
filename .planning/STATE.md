# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-02-08)

**Core value:** Keep music playback inside the terminal workflow - no context switching to external apps, everything stays in Tmux.
**Current focus:** Phase 2 complete, ready for Phase 3

## Current Position

Phase: 2 of 5 (Core TUI and Playback) -- COMPLETE
Plan: 2 of 2 in current phase
Status: Phase 2 complete, ready for Phase 3 planning
Last activity: 2026-02-10 -- Completed 02-02 (Player Bar UI)

Progress: [████████████████████] 100% (Phase 2)

## Performance Metrics

**Velocity:**
- Total plans completed: 5
- Average duration: ~14 min
- Total execution time: ~1.21 hours

**By Phase:**

| Phase | Plans | Total | Avg/Plan |
|-------|-------|-------|----------|
| 01-foundation-audio-poc | 3/3 | ~58 min | ~19 min |
| 02-core-tui-playback | 2/2 | ~14 min | ~7 min |

**Recent Trend:**
- Last 5 plans: 01-02 (~5 min), 01-03 (~38 min), 02-01 (~3 min), 02-02 (~11 min)
- Trend: Phase 2 plans faster due to well-researched plans with no new dependencies

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

### Pending Todos

None yet.

### Blockers/Concerns

- ~~WSL2 audio reliability (PulseAudio pause/resume >5s) is unvalidated -- Phase 1 must prove this works~~ RESOLVED: Validated in 01-03, pause/resume works reliably after >5s on WSL2

## Session Continuity

Last session: 2026-02-10
Stopped at: Completed 02-02-PLAN.md (Player Bar UI) -- Phase 2 complete
Resume file: .planning/phases/02-core-tui-playback/02-02-SUMMARY.md
