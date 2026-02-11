# Milestones

## v1.0 MVP (Shipped: 2026-02-10)

**Phases completed:** 5 phases, 10 plans, 0 tasks

**Key accomplishments:**
- WSL2 audio validation - Complete Plex authentication and reliable audio playback with pause/resume on WSL2
- Full-featured TUI - Multi-panel interface with vim keybindings, track navigation, and visual feedback
- Unique features - Favorite playlist hotkeys (1-9), shuffle, repeat modes, and seek controls
- Tmux integration - Adaptive layout for narrow panes, status bar file, session persistence
- Audio visualizer - FFT-based spectrum display with zero audio degradation

**Delivered:**
A complete terminal music player for Plex that integrates seamlessly into tmux-based workflows with vim keybindings, favorite playlist hotkeys, and an aesthetic audio visualizer.

**Stats:**
- 3,507 lines of Rust code
- 2-day development cycle (2026-02-08 → 2026-02-10)
- Tech stack: ratatui, rodio, crossterm, reqwest, spectrum-analyzer
- 100% requirements coverage (42/42 requirements)

---


## v1.1 Multi-Channel Audio (Shipped: 2026-02-11)

**Phases completed:** 4 phases (6-9), 6 plans, 21 commits

**Key accomplishments:**
- Dual-sink audio architecture - Play two independent audio streams (main music + ambient) simultaneously on WSL2 without crackling
- Track browsing UI - Modal browser with vim-style navigation to select individual tracks from Plex library
- Independent volume controls - Separate volume management for main and ambient channels with [/] keybindings
- Pre-mute memory - Volume toggle (m) preserves user's custom volume across mute/unmute and app restarts
- Session persistence - Ambient track selection, volume, and playback state survive app restarts
- Continuous looping - Ambient tracks loop indefinitely using manual re-append for stable memory usage

**Delivered:**
Enhanced TermTunes with multi-channel audio capability, allowing users to layer ambient tracks underneath music playlists for enhanced focus during deep work. All ambient state persists across sessions.

**Stats:**
- 4,477 lines of Rust code (total codebase)
- 2-day development cycle (2026-02-10 → 2026-02-11)
- 100% requirements coverage (27/27 v1.1 requirements)
- Notable: Independent volume channels replaced proportional budget after 8 iterative WSL2 bug fixes

---

