# Project Research Summary

**Project:** TermTunes
**Domain:** TUI Music Player with Plex Media Server Integration
**Researched:** 2026-02-08
**Confidence:** MEDIUM

## Executive Summary

TermTunes is a terminal-based music player that connects to an existing Plex Media Server for playlist-centric background listening inside tmux. The established pattern for building TUI music players is an event-driven, message-passing architecture with strict separation between UI rendering, audio playback, network communication, and state management. Proven projects in this space (spotify_player, rmpc, jellyfin-tui, kew) all follow this pattern. The product's differentiators are narrow but strong: favorite playlist hotkeys (1-9), tmux status bar integration, and a design optimized for small tmux panes -- none of which exist in any competitor.

**Critical resolution needed:** The stack and architecture research produced conflicting recommendations. STACK.md recommends Rust (ratatui + rodio + crossterm + reqwest), citing that every proven TUI music player uses Rust and that pure-Rust audio via rodio avoids system dependencies on WSL2. ARCHITECTURE.md recommends Go (Bubble Tea + plexgo SDK + mpv), citing the mature plexgo SDK and mpv's native HTTP streaming and gapless playback. **The synthesis recommendation is Rust.** The rationale: (1) WSL2 audio is the project's highest-risk area, and rodio/Symphonia are pure Rust with no C dependencies beyond ALSA headers, whereas mpv requires compiling libmpv C bindings on WSL2; (2) every reference TUI music player (spotify_player, rmpc, ncspot) uses Rust/ratatui, providing battle-tested architectural patterns to follow; (3) the Plex REST API is simple enough that a thin reqwest wrapper outperforms depending on any SDK; (4) PROJECT.md lists WSL as a primary platform, making minimal system dependencies a hard requirement. The plexgo SDK advantage is real but insufficient to overcome Rust's ecosystem lead in this exact domain.

The top risks are: WSL2 audio reliability (PulseAudio pause/resume breaks after ~5 seconds), Plex authentication token lifecycle management, and UI performance with large music libraries (10k-100k+ tracks). All three must be addressed architecturally in Phase 1 -- they cannot be patched later. A proof-of-concept audio test on WSL2 should be the very first deliverable, before any UI work begins.

## Key Findings

### Recommended Stack

The Rust ecosystem provides a complete, proven toolchain for TUI music players with no gaps. Every core dependency is pure Rust (no C/FFmpeg/system library requirements beyond ALSA headers), which is critical for WSL2 compatibility. The stack has been validated in production by multiple open-source TUI music players.

**Core technologies:**
- **Rust 1.82+ (2024 edition):** Language -- zero-cost abstractions for real-time audio + UI, single binary deployment ideal for tmux tools
- **Ratatui 0.30:** TUI framework -- immediate-mode rendering with sub-millisecond performance, used by every major Rust TUI project
- **Crossterm 0.29:** Terminal backend -- pure Rust, cross-platform, WSL2 native, no system dependencies
- **Rodio 0.21 + Symphonia:** Audio playback -- pure Rust decoder for MP3/FLAC/AAC/OGG/WAV, no FFmpeg dependency
- **Tokio 1.47 (LTS):** Async runtime -- handles concurrent Plex API calls, audio streaming, UI events
- **Reqwest 0.13:** HTTP client -- async with tokio, streaming responses for audio download, rustls for TLS (no OpenSSL)

**Supporting libraries:** serde/serde_json (JSON), quick-xml (Plex XML), crossbeam (lock-free channels between threads), dirs (XDG paths), toml (config), clap (CLI args), tracing (debug logging)

**What NOT to use:** plex-api Rust crate (v0.0.12, explicitly "not ready"), tui-rs (unmaintained), ncurses/termion backends (C deps or Linux-only), GStreamer/FFmpeg bindings (heavy C deps), SDL2 audio (heavyweight).

See: `.planning/research/STACK.md`

### Expected Features

The feature research identified a focused MVP that validates the core concept of "pick a playlist and work" with 11 features, a strong set of differentiators unique to TermTunes, and explicit anti-features that keep scope contained.

**Must have (table stakes -- P1):**
- Plex authentication (token-based PIN flow)
- Playlist listing and selection from Plex
- Play/pause/stop/skip forward/back
- Current track info display (artist, album, track)
- Playback progress bar with time
- Volume control (+/-)
- Shuffle mode
- Vim keybindings (j/k/enter/space/q)
- Favorite playlist keybindings (1-9) -- the killer differentiator, include from day one

**Should have (differentiators -- P2):**
- Seek within track
- Repeat mode (off/all/one)
- Toggleable audio visualizer
- Tmux status bar integration ("now playing" in tmux)
- Responsive layout for small tmux panes
- Session persistence (resume last playlist)

**Defer (v2+):**
- MPRIS integration, gapless playback, synced lyrics, Last.fm scrobbling, playlist search/filter

**Explicit anti-features (never build):**
- Full library browsing, manual queue management, smart recommendations, crossfade/EQ, mouse support, tag editing, downloading/offline mode, multi-server support

See: `.planning/research/FEATURES.md`

### Architecture Approach

The architecture follows the universal TUI music player pattern: an event-driven main loop coordinating loosely-coupled subsystems (UI, audio, network, state) through typed message channels. In Rust with ratatui, this means a main event loop on the primary thread dispatching keyboard/terminal events, an audio thread running rodio with crossbeam channels for commands and state updates, and a tokio runtime for async Plex API calls. The key patterns are: (1) never block the render loop on network or audio operations, (2) audio engine pushes state changes as events rather than being polled, (3) Plex API wrapped behind a domain-typed abstraction layer, (4) sub-components with clear boundaries communicating through messages.

**Major components:**
1. **Event Loop (main thread)** -- ratatui rendering, keyboard input, terminal events, dispatches to other components
2. **Plex Client (async/tokio)** -- authentication, playlist/track fetching, stream URL resolution, timeline reporting; wrapped behind domain types
3. **Audio Engine (dedicated thread)** -- rodio Sink for playback, accepts commands via channel, pushes state events back
4. **State Manager** -- centralized playback state (current track, position, volume, queue, shuffle/repeat)
5. **UI Views** -- playlist view, player bar (always visible), future views plug in via the same pattern
6. **Config** -- TOML-based user configuration, keybindings, Plex server details, favorite playlist mappings

**Critical architectural decisions:**
- Resolve stream URLs at play time, not queue time (tokens expire, sessions invalidate)
- Pagination from day one for Plex API calls (never fetch entire library)
- Terminal state cleanup in panic hooks and signal handlers from the first commit

See: `.planning/research/ARCHITECTURE.md`

### Critical Pitfalls

1. **WSL2 audio breaks after pause/resume** -- PulseAudio stream fails to reinitialize after pausing >5 seconds. Implement a watchdog that detects stalled playback and automatically recreates the audio stream. Test pause/resume in Phase 1 before building anything else. Consider Windows-side audio as fallback.

2. **Plex token lifecycle mismanagement** -- Tokens expire on password change, server restart (transient tokens last 48h), or rotation. Implement full PIN-based auth flow from day one. Validate tokens on startup. Auto re-authenticate on any 401 response. Persist Client Identifier UUID.

3. **Large library rendering causes UI lag** -- Plex libraries with 10k-100k+ tracks overwhelm TUI frameworks that render all items every frame. Use mandatory pagination for Plex API calls and virtual scrolling for ratatui widgets. Design the pagination architecture in Phase 1 even though library browsing comes later.

4. **Missing Plex timeline reporting** -- Without reporting playback state to `/:/timeline`, Plex dashboard shows nothing, play counts don't update, "On Deck" breaks. Implement alongside playback, not as a later polish step.

5. **Terminal state corruption on crash/exit** -- Raw mode, alternate screen, mouse tracking persist after abnormal exit. Register signal handlers (SIGINT/SIGTERM/SIGHUP), set custom panic hook, use RAII/scopeguard patterns. Implement in the first commit.

See: `.planning/research/PITFALLS.md`

## Implications for Roadmap

Based on combined research, the following phase structure respects dependency chains, groups architecturally related work, and addresses pitfalls at the earliest viable point.

### Phase 1: Foundation and Audio Proof-of-Concept
**Rationale:** WSL2 audio is the highest-risk technical unknown. If audio does not work reliably on WSL2, the entire project premise fails. Plex auth is the gateway to every feature. Terminal state management is foundational infrastructure. All three must be validated before investing in UI.
**Delivers:** Working audio playback of a Plex track on WSL2, proper terminal lifecycle management, Plex PIN authentication, basic project scaffolding.
**Addresses features:** Plex authentication, basic play/pause/stop (headless, no TUI yet)
**Avoids pitfalls:** WSL2 audio pause/resume failure (validated via PoC), Plex token lifecycle (PIN flow built), terminal state corruption (cleanup hooks from first commit)
**Stack focus:** Rust project setup, rodio + ALSA, reqwest + Plex auth, crossterm terminal management, crossbeam channels

### Phase 2: Core TUI and Playback Controls
**Rationale:** With audio and auth proven, build the ratatui skeleton and core playback UI. The player bar (now-playing, progress, volume) is visible on every screen and must work before any navigation views. Playlist listing is the primary screen.
**Delivers:** Functional TUI with playlist listing from Plex, track playback with full controls, player bar with progress/time/volume, vim keybindings.
**Addresses features:** Playlist listing/selection, play/pause/stop/skip, current track info, progress bar, volume control, vim keybindings (j/k/enter/space/q), keyboard-only operation
**Avoids pitfalls:** Blocking the event loop (async from start), rendering lag (virtual scrolling for playlist lists)
**Architecture:** Event loop + state manager + playlist view + player bar + Plex client integration

### Phase 3: Differentiators and Playlist Features
**Rationale:** Core playback is stable. Now add the features that make TermTunes unique: favorite playlist hotkeys, shuffle mode, and Plex timeline reporting. These are low-complexity, high-value additions that build on Phase 2 infrastructure.
**Delivers:** Favorite playlist keybindings (1-9), shuffle mode, repeat mode, seek within track, Plex timeline reporting (now playing on dashboard, play counts).
**Addresses features:** Favorite playlist keybindings (P1), shuffle (P1), repeat (P2), seek (P2)
**Avoids pitfalls:** Missing timeline reporting (implemented here, not deferred), stream URL caching (resolve at play time)

### Phase 4: Tmux Integration and Polish
**Rationale:** With daily-driveable playback, optimize for the actual usage context: a narrow tmux pane alongside nvim. Add tmux status bar integration, responsive layout for small panes, session persistence, and terminal resize handling.
**Delivers:** Tmux status bar "now playing", responsive layout for 30-40 column panes, session persistence across restarts, polished resize handling.
**Addresses features:** Tmux status bar integration (P2), responsive resize (P2), session persistence (P2), compact layout for small panes (P2)
**Avoids pitfalls:** Tmux keybinding conflicts (tested explicitly), inconsistent behavior inside/outside tmux

### Phase 5: Audio Visualizer
**Rationale:** The visualizer is HIGH complexity and purely aesthetic. It requires FFT audio stream analysis, a dedicated render path, and careful CPU budgeting to avoid degrading playback. Defer until core product is solid and daily-driveable.
**Delivers:** Toggleable spectrum visualizer (key: v), runs FFT on dedicated thread, gracefully hidden in small panes.
**Addresses features:** Toggleable audio visualizer (P2)
**Avoids pitfalls:** Visualizer CPU overhead causing audio dropout (dedicated thread, profiled), visualizer dominating screen space (toggleable, hidden in small panes)

### Phase 6: Future Enhancements (v2+)
**Rationale:** Only pursue after core product has been daily-driven and validated.
**Delivers:** MPRIS integration, gapless playback, synced lyrics, Last.fm scrobbling, playlist search/filter.

### Phase Ordering Rationale

- **Audio before UI:** WSL2 audio is the single biggest risk. Proving it works before investing in TUI development prevents wasted effort on a product that cannot play sound.
- **Auth before everything:** Every Plex feature requires a valid token. The PIN flow is the gateway.
- **Player bar before navigation views:** The player bar is visible on every screen. Getting it right early means all subsequent UI work benefits from having playback feedback visible.
- **Differentiators in Phase 3, not Phase 2:** Favorite keybindings are low-complexity but depend on playlist infrastructure from Phase 2. Grouping them with shuffle/repeat/seek keeps Phase 2 focused on core playback.
- **Tmux integration after core playback:** Tmux-specific polish requires a working player to test against. Cannot optimize for small panes without having the full UI to optimize.
- **Visualizer last among planned features:** Highest implementation cost, lowest priority for the "background music while coding" use case. The product is fully functional without it.

### Research Flags

Phases likely needing deeper research during planning:
- **Phase 1:** WSL2 audio reliability needs hands-on validation. Research identified the problem but the solution (stream recreation watchdog) needs prototyping. PulseAudio configuration tuning may be needed.
- **Phase 5:** Audio visualizer FFT implementation has sparse Rust-specific documentation. May need to evaluate CAVA protocol integration vs. custom FFT. Research the `rustfft` crate and rodio's Source chain for tapping audio data.

Phases with standard patterns (skip research-phase):
- **Phase 2:** ratatui TUI skeleton, event loop, list widgets -- extremely well-documented with official examples and multiple reference projects (rmpc, spotify_player).
- **Phase 3:** Favorite keybindings, shuffle, repeat, seek -- straightforward state management. Plex timeline API is well-documented.
- **Phase 4:** Tmux integration is a thin layer (write to file, read from tmux config). Session persistence is simple serialization.

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| Stack | HIGH | All recommended crates are actively maintained with recent releases. Versions verified. Multiple TUI music players validate the exact stack. WSL2 audio setup is the one MEDIUM-confidence area within this. |
| Features | MEDIUM-HIGH | Feature landscape well-mapped from 10+ competitor analysis. MVP definition is crisp. Anti-features are well-reasoned. Slight uncertainty on whether the playlist-only approach satisfies users long-term. |
| Architecture | MEDIUM | Architecture research diverged on language (Go vs Rust), requiring synthesis-level resolution. The resolved Rust architecture follows proven patterns from rmpc and spotify_player. The Go-based architecture document contains valuable patterns (Elm Architecture, command-based async) that translate well to Rust with crossbeam channels. |
| Pitfalls | MEDIUM-HIGH | WSL2 audio pitfalls verified via multiple GitHub issues with reproduction steps. Plex auth pitfalls verified via official Plex developer forums. Ratatui performance pitfalls confirmed by framework maintainers. One gap: no first-hand WSL2 audio testing yet. |

**Overall confidence:** MEDIUM

The confidence is MEDIUM rather than HIGH because of two factors: (1) WSL2 audio reliability is the make-or-break technical risk and has not been validated hands-on, and (2) the architecture research produced a conflicting language recommendation that required synthesis-level resolution. The stack, features, and pitfalls research are individually strong, but the architecture conflict lowers the overall score.

### Gaps to Address

- **WSL2 audio reliability:** No first-hand testing. Research identifies the risk and mitigation strategies, but Phase 1 must include a concrete PoC that validates pause/resume, latency, and stability. If WSL2 PulseAudio proves unworkable, fallback to Windows-side audio (spawning a Windows-native player controlled from the TUI) must be designed.

- **Plex API for music playlists specifically:** Research covers Plex API in general, but the exact endpoints for music playlist enumeration, track ordering, and audio stream URL resolution need validation against a live server. The python-plexapi library is well-documented; the raw REST API for music has community documentation but not official comprehensive docs.

- **Rodio pause/resume behavior:** Rodio's `Sink::pause()` and `Sink::play()` behavior when combined with WSL2 PulseAudio needs testing. If rodio does not handle stream re-creation gracefully, a custom audio backend wrapper may be needed.

- **Architecture document language mismatch:** The ARCHITECTURE.md recommends Go/Bubble Tea while STACK.md recommends Rust/ratatui. This summary resolves in favor of Rust, but the Go architecture's patterns (Elm Architecture, sub-model composition, command-based async) should inform the Rust implementation design. The project structure from ARCHITECTURE.md needs to be re-mapped to Rust module conventions.

## Sources

### Primary (HIGH confidence)
- [Ratatui official site](https://ratatui.rs/) -- v0.30.0, framework capabilities and examples
- [Ratatui GitHub releases](https://github.com/ratatui/ratatui/releases) -- v0.30.0, Dec 2025
- [Rodio on lib.rs](https://lib.rs/crates/rodio) -- v0.21.1, Jul 2025, Symphonia integration
- [Crossterm GitHub](https://github.com/crossterm-rs/crossterm) -- v0.29, terminal backend
- [Tokio official site](https://tokio.rs/) -- v1.47.x LTS
- [Plex developer docs](https://developer.plex.tv/pms/) -- REST API structure, authentication
- [Plex auth forum thread](https://forums.plex.tv/t/authenticating-with-plex/609370) -- PIN flow details
- [rmpc architecture](https://deepwiki.com/mierak/rmpc/1-overview) -- ratatui + crossterm + crossbeam patterns
- [spotify_player](https://github.com/aome510/spotify-player) -- ratatui + rodio architecture reference
- [Ratatui Table performance issue #1004](https://github.com/ratatui/ratatui/issues/1004) -- large dataset rendering confirmed
- [WSLg audio latency issue #607](https://github.com/microsoft/wslg/issues/607) -- multiple corroborating reports
- [WSLg audio stuttering issue #1257](https://github.com/microsoft/wslg/issues/1257) -- confirmed

### Secondary (MEDIUM confidence)
- [WSLg pause/resume issue #1376](https://github.com/microsoft/wslg/issues/1376) -- open issue, fix pending
- [Plex download API (Plexopedia)](https://www.plexopedia.com/plex-media-server/api/library/download-media-file/) -- community docs, verified against official
- [jellyfin-tui](https://github.com/dhonus/jellyfin-tui) -- analogous project patterns
- [kew terminal player](https://github.com/ravachol/kew) -- feature reference, visualizer patterns
- [plex-audio-btop-tui](https://github.com/MacsInSpace/plex-audio-btop-tui) -- direct competitor, Plex TUI patterns
- [Plex API rate limit forum](https://forums.plex.tv/t/api-rate-limit-exceeded-status-429/886080) -- community reports
- [Bubble Tea framework](https://github.com/charmbracelet/bubbletea) -- Go alternative architecture reference
- [plexgo SDK](https://github.com/LukeHagar/plexgo) -- Go Plex client (not recommended for use, but informs API understanding)

### Tertiary (LOW confidence)
- [plex-api Rust crate](https://lib.rs/crates/plex-api) -- v0.0.12, explicitly "not ready for any use" (confirmed to AVOID)
- [WSL2 audio general issue #5816](https://github.com/microsoft/WSL/issues/5816) -- older issue, PulseAudio status evolving

---
*Research completed: 2026-02-08*
*Ready for roadmap: yes*
