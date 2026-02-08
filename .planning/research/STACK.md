# Stack Research

**Domain:** TUI Music Player with Plex Integration
**Researched:** 2026-02-08
**Confidence:** HIGH

## Recommended Stack

### Core Technologies

| Technology | Version | Purpose | Why Recommended |
|------------|---------|---------|-----------------|
| Rust | 1.82+ (2024 edition) | Language | Every proven TUI music player in the terminal space uses Rust (spotify_player, ncspot, rmpc). Zero-cost abstractions matter for real-time audio + UI rendering. Single binary deployment is ideal for a tool that lives in tmux. |
| Ratatui | 0.30.0 | TUI rendering framework | The standard Rust TUI framework. Immediate-mode rendering with sub-millisecond performance. Modular workspace architecture (as of 0.30). Used by spotify_player, rmpc, and 10,000+ projects. No serious competitor exists in the Rust TUI space. |
| Crossterm | 0.29.x | Terminal backend | Cross-platform terminal manipulation. Default backend for ratatui. Handles raw mode, keyboard events, mouse input. Pure Rust with no system dependencies. Required for WSL2 compatibility. |
| Rodio | 0.21.1 | Audio playback | The standard Rust audio playback library. Built on cpal for hardware output and Symphonia for decoding. Supports Sink (sequential) and Mixer (parallel) playback. Handles MP3, FLAC, AAC, WAV, OGG via Symphonia. Proven in spotify_player. |
| Tokio | 1.47.x (LTS) | Async runtime | Required for reqwest HTTP client. Handles concurrent Plex API calls, audio streaming, and UI events. LTS release supported until September 2026. Used by every async Rust project. |
| Reqwest | 0.13.x | HTTP client | For Plex API communication. Async with tokio. Supports streaming responses (critical for audio download). JSON and XML parsing. TLS via rustls (no OpenSSL dependency on WSL2). |

### Supporting Libraries

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| Symphonia | latest | Audio decoding | Automatically pulled in by rodio. Pure Rust decoder for MP3, FLAC, AAC, M4A, OGG, WAV. No FFmpeg dependency. Decode audio streamed from Plex. |
| Serde + serde_json | 1.x / 1.0.149 | Serialization | JSON parsing for Plex API responses. Derive macros for Plex data models. Config file parsing. |
| quick-xml | latest | XML parsing | Plex API returns XML by default (not JSON). Parse library metadata, track listings, playlist data. Serde integration for typed deserialization. |
| crossbeam | 0.8.x | Concurrency primitives | Lock-free channels for thread communication between UI thread, audio thread, and network thread. Used by rmpc for the same pattern. |
| dirs | latest | Platform directories | Find XDG config/cache/data directories. Store Plex auth tokens, cached metadata, config files. |
| toml | latest | Config parsing | User configuration file format. Rust ecosystem standard (Cargo.toml precedent). Human-readable for keybindings, server URLs, preferences. |
| clap | 4.x | CLI argument parsing | Parse command-line flags for server URL, debug mode, config path override. Derive API for zero-boilerplate. |
| tracing | latest | Structured logging | Debug logging during development. Async-aware. Can write to file without disrupting TUI. |

### Development Tools

| Tool | Purpose | Notes |
|------|---------|-------|
| cargo | Build system | Rust's built-in. Workspace support for modular architecture. |
| cargo-watch | Auto-rebuild | `cargo watch -x run` for rapid iteration during TUI development. |
| bacon | Background checker | Better than cargo-watch for TUI apps. Runs checks/clippy in background without stealing terminal. |
| cargo-clippy | Linting | Catch common Rust mistakes. Run with `-- -W clippy::all`. |

## Installation

```bash
# Install Rust toolchain
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Create project
cargo init termtunes

# Core dependencies (Cargo.toml)
# [dependencies]
# ratatui = { version = "0.30", features = ["crossterm"] }
# crossterm = { version = "0.29", features = ["event-stream"] }
# rodio = { version = "0.21", features = ["symphonia-all"] }
# tokio = { version = "1", features = ["full"] }
# reqwest = { version = "0.13", features = ["json", "stream"] }
# serde = { version = "1", features = ["derive"] }
# serde_json = "1"
# quick-xml = { version = "0.37", features = ["serialize"] }
# crossbeam = "0.8"
# dirs = "6"
# toml = "0.8"
# clap = { version = "4", features = ["derive"] }
# tracing = "0.1"
# tracing-subscriber = "0.3"

# Dev dependencies
# [dev-dependencies]
# tokio-test = "0.4"

# System dependency for audio on WSL2 (one-time)
# sudo apt install libasound2-dev  # ALSA headers for cpal/rodio
```

## Alternatives Considered

| Recommended | Alternative | When to Use Alternative |
|-------------|-------------|-------------------------|
| Rust | Go + Bubbletea | If you prioritize faster initial development over runtime performance. Bubbletea v2 (RC) has excellent Elm Architecture. gopxl/beep handles audio. go-plex-client exists. Trade-off: fewer proven TUI music player reference projects. |
| Rust | Python + Textual | If Plex API richness matters most. python-plexapi 4.18.0 is the most mature Plex client (actively maintained, full API coverage). Trade-off: audio playback requires shelling out to mpv/VLC, and Textual's rendering is noticeably slower than ratatui. |
| Ratatui (immediate mode) | tui-realm | If you want React/Elm-style component architecture on top of ratatui. Adds complexity. Only use if the app grows beyond ~15 views. |
| Rodio | cpal (direct) | If you need precise low-level audio control (custom DSP, visualization). Rodio is built on cpal and abstracts it well. Only go lower if rodio's Sink/Source model is insufficient. |
| Reqwest (custom Plex client) | plex-api crate (0.0.12) | Never for production. The crate explicitly states "work in progress, not ready for any use" and has breaking changes every release. Build a thin HTTP client with reqwest instead. |
| TOML config | RON (Rusty Object Notation) | If you want Rust-native config syntax. RON is used by rmpc. TOML is more familiar to users coming from other tools. Personal preference. |
| quick-xml | serde-xml-rs | If the XML parsing needs are very simple. quick-xml is faster and more actively maintained. |

## What NOT to Use

| Avoid | Why | Use Instead |
|-------|-----|-------------|
| plex-api (Rust crate) | Version 0.0.12, explicitly "not ready for any use", breaking changes every release. No stable API surface. | Custom HTTP client with reqwest + quick-xml. The Plex REST API is simple enough that a thin wrapper is sufficient and more maintainable than depending on an unstable crate. |
| tui-rs | Unmaintained since August 2023. Ratatui is the active fork. tui-rs will not receive security patches. | Ratatui 0.30.x |
| ncurses/termion backend | ncurses requires C bindings (complicates WSL2 builds). Termion is Linux-only. | Crossterm (pure Rust, cross-platform, WSL2 native). |
| GStreamer/FFmpeg bindings | Heavy C dependencies. Painful to compile on WSL2. Overkill for music playback. | Rodio + Symphonia (pure Rust, handles all common audio formats). |
| SDL2 audio | Requires SDL2 system library. Heavyweight for terminal app. | Rodio (Rust-native, lighter). |
| Python subprocess to mpv | Fragile. Requires external binary. Hard to control playback state programmatically. | Rodio for native in-process audio with full programmatic control. |

## Stack Patterns by Variant

**If Plex server is local (same machine or LAN):**
- Use direct file download via `/library/parts/{id}` endpoint
- Prefer direct play (no transcoding) for lower latency and better quality
- Audio files stream directly into rodio via reqwest byte stream
- Because: Avoids Plex transcoding overhead, gets original quality

**If Plex server is remote (over internet):**
- Use Plex transcode endpoint `/music/:/transcode/universal/start.m3u8`
- Request compressed format matching available bandwidth
- Buffer more aggressively before starting playback
- Because: Bandwidth-constrained, transcoding shifts CPU to server

**If running in tmux specifically:**
- Set terminal to raw mode via crossterm
- Handle SIGWINCH for terminal resize
- Audio plays through WSL2 PulseAudio bridge to Windows audio
- Because: Tmux forwards most escape sequences but resize needs explicit handling

## Version Compatibility

| Package A | Compatible With | Notes |
|-----------|-----------------|-------|
| ratatui@0.30 | crossterm@0.28 or 0.29 | Ratatui 0.30 supports both via feature flags (crossterm_0_28, crossterm_0_29). Use 0.29 (default). |
| rodio@0.21 | cpal@latest | Rodio pins its own cpal version. Do not add cpal directly unless you need low-level access. |
| reqwest@0.13 | tokio@1.x | Both are on stable major versions. No known conflicts. |
| crossterm@0.29 | tokio@1.x | event-stream feature requires tokio for async event reading. Compatible. |

## WSL2 Audio Setup (Critical Prerequisite)

Audio playback in WSL2 requires PulseAudio bridging to Windows. This is a one-time setup:

```bash
# Install ALSA dev headers (build dependency for rodio/cpal)
sudo apt install libasound2-dev

# Install PulseAudio (runtime dependency)
sudo apt install pulseaudio

# PulseAudio in WSL2 connects to Windows via WSLg (Windows 11)
# or via manual PulseAudio TCP bridge (Windows 10)
# Verify: pactl info | grep "Server Name"
```

**Confidence note:** WSL2 audio via PulseAudio is functional but has known quirks. The WSLg subsystem in Windows 11 provides PulseAudio automatically. On Windows 10, manual setup is required. PipeWire integration is not yet available in WSL2 as of early 2026. This is a MEDIUM confidence area -- testing during Phase 1 is essential.

## Sources

- [Ratatui official site](https://ratatui.rs/) -- v0.30.0 confirmed, HIGH confidence
- [Ratatui GitHub releases](https://github.com/ratatui/ratatui/releases) -- v0.30.0, Dec 26 2025
- [Rodio on lib.rs](https://lib.rs/crates/rodio) -- v0.21.1, Jul 14 2025, HIGH confidence
- [Rodio GitHub](https://github.com/RustAudio/rodio) -- Symphonia default decoder confirmed
- [Crossterm GitHub](https://github.com/crossterm-rs/crossterm) -- v0.29 confirmed, HIGH confidence
- [Tokio official site](https://tokio.rs/) -- v1.47.x LTS, HIGH confidence
- [Reqwest GitHub](https://github.com/seanmonstar/reqwest) -- v0.13.x confirmed, HIGH confidence
- [serde_json releases](https://github.com/serde-rs/json/releases) -- v1.0.149, Jan 6 2026
- [PlexAPI on PyPI](https://pypi.org/project/PlexAPI/) -- v4.18.0, Jan 31 2026 (confirms mature Python alternative exists)
- [plex-api Rust crate](https://lib.rs/crates/plex-api) -- v0.0.12, "not ready for any use", HIGH confidence to AVOID
- [Plex developer docs](https://developer.plex.tv/pms/) -- REST API structure, authentication, MEDIUM confidence
- [Plexopedia download API](https://www.plexopedia.com/plex-media-server/api/library/download-media-file/) -- download endpoint structure, MEDIUM confidence
- [spotify_player](https://github.com/aome510/spotify-player) -- Architecture reference: ratatui + rodio + librespot
- [rmpc DeepWiki](https://deepwiki.com/mierak/rmpc/1-overview) -- Architecture reference: ratatui 0.30 + crossterm 0.29 + crossbeam channels
- [WSL2 audio issue](https://github.com/microsoft/WSL/issues/5816) -- PulseAudio status, MEDIUM confidence
- [Bubbletea GitHub](https://github.com/charmbracelet/bubbletea) -- v1.3.10 stable, v2.0.0-rc.2 (Go alternative reference)
- [Textual on PyPI](https://pypi.org/project/textual/) -- v7.5.0, Jan 30 2026 (Python alternative reference)

---
*Stack research for: TUI Music Player with Plex Integration (TermTunes)*
*Researched: 2026-02-08*
