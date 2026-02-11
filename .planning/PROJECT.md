# TermTunes

## What This Is

A TUI (Terminal User Interface) music player for Plex Media Server that lives in Tmux. Provides playlist-based music playback with vim-style keyboard controls, designed to integrate seamlessly into a terminal-based workflow alongside NVIM and task managers.

## Core Value

Keep music playback inside the terminal workflow - no context switching to external apps, everything stays in Tmux.

## Current State: v1.1 Shipped

**Latest:** v1.1 Multi-Channel Audio (shipped 2026-02-11)

**Delivered:**
- Dual-sink audio architecture for simultaneous main music + ambient playback
- Track browser with vim-style navigation to select individual tracks
- Independent volume controls ([/] keybindings) with pre-mute memory
- Ambient status panel showing track name, state, and volume
- Full session persistence across app restarts

**What's next:** Planning future enhancements (TBD)

## Requirements

### Validated

**v1.0 MVP:**
- ✓ Connect to existing Plex Media Server — v1.0
- ✓ Display list of available playlists — v1.0
- ✓ Select and play a playlist — v1.0
- ✓ Playback controls (play, pause, skip forward/back) — v1.0
- ✓ Shuffle mode for playlists — v1.0
- ✓ Display current track information (artist, album, track name) — v1.0
- ✓ Show playback progress and time — v1.0
- ✓ Vim keybindings for navigation and control — v1.0
- ✓ Visual spectrum equalizer animation (aesthetic, toggleable) — v1.0
- ✓ Favorite playlists with quick keybindings (1-9 to instantly start) — v1.0
- ✓ Local audio playback — v1.0

**v1.1 Multi-Channel Audio:**
- ✓ Browse and select individual tracks from Plex library — v1.1
- ✓ Play ambient track on separate audio channel — v1.1
- ✓ Ambient track auto-loops continuously — v1.1
- ✓ Independent volume control for ambient channel — v1.1
- ✓ Toggle ambient channel on/off — v1.1
- ✓ Ambient track persists across sessions — v1.1
- ✓ UI panel showing ambient track status — v1.1

### Active

(Planning next milestone)

### Out of Scope

- Ambient playlists — v1.1 focuses on single looping tracks, playlists deferred to future
- Queue management — Simple playlist playback, not manual queue building
- Smart features (radio, mixes, recommendations) — Using Plex's existing playlists
- Mobile or non-terminal interfaces — Terminal-only
- Full library exploration — Track browsing limited to ambient selection, not general library browsing

## Context

**Usage pattern:** Background music while working - primarily mixes and ambient playlists. Music plays while user focuses on code or tasks in other Tmux panes.

**Existing setup:** User has running Plex Media Server with music library and playlists already configured. Uses Tmux with NVIM for editing and a task manager, creating an integrated terminal workspace.

**User profile:** Vim power user with vim-tmux plugin for pane navigation. Keyboard-centric workflow, no mouse usage.

**Current state (v1.1):**
- 4,477 lines of Rust code
- Tech stack: ratatui (TUI), rodio (audio with dual-sink), crossterm (terminal), reqwest (HTTP), spectrum-analyzer (FFT)
- Features: PIN-based Plex auth, playlist browser, track browser, dual-channel audio (main + ambient), independent volume controls, pre-mute memory, full session persistence, audio visualizer
- Validated on WSL2 and Linux
- All 42 v1.0 + 27 v1.1 requirements satisfied (69 total)

## Constraints

- **Platform**: Must run on WSL and Linux
- **Integration**: Must work seamlessly in Tmux environment
- **Input**: Keyboard-only, vim-style keybindings required
- **Audio**: Local playback on the machine running the TUI
- **Dependencies**: Must connect to existing Plex Media Server (local or remote)

## Key Decisions

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| Playlist-focused over library browsing | User's primary use case is selecting pre-made playlists for background music | ✓ Good — v1.0 validated this is the right scope |
| Vim keybindings | User is vim power user, consistency with their existing workflow (NVIM, vim-tmux) | ✓ Good — natural navigation, no learning curve |
| Toggleable visualizer | Adds terminal aesthetic but might be distracting during work | ✓ Good — v toggle makes it optional, FFT adds zero overhead |
| WSL2 validation first (Phase 1) | Audio reliability on WSL2 was highest risk | ✓ Good — caught buffer tuning issues early |
| Download-then-play pattern | Streaming caused WSL2 audio dropouts | ✓ Good — eliminated all playback issues |
| Favorite hotkeys (1-9) | Differentiation from other players | ✓ Good — instant playlist access is killer feature |
| Session persistence | Resume playback across app restarts | ✓ Good — essential for background listening workflow |
| Tmux status bar file | Show now-playing in tmux status line | ✓ Good — maintains context awareness across panes |
| Independent volume channels (v1.1) | Replace proportional budget with user-controlled channels | ✓ Good — 8 WSL2 fixes revealed simpler UX, direct control |
| Dual-sink on shared mixer (v1.1) | Two independent Sinks on same OutputStream.mixer() | ✓ Good — clean isolation, no interference between channels |
| Manual ambient loop (v1.1) | replay_ambient() from cached bytes vs rodio repeat_infinite | ✓ Good — stable memory, avoids rodio memory leak |
| Pre-mute memory (v1.1) | Save volume before mute, restore exact value on unmute | ✓ Good — preserves user intent across mute/quit/restart cycles |
| Sink recreation for ambient volume (v1.1) | Stop + recreate Sink instead of set_volume() | ⚠️ Workaround — rodio bug, but works reliably on WSL2 |

---
*Last updated: 2026-02-11 after v1.1 milestone completion*
