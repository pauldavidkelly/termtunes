# TermTunes

## What This Is

A TUI (Terminal User Interface) music player for Plex Media Server that lives in Tmux. Provides playlist-based music playback with vim-style keyboard controls, designed to integrate seamlessly into a terminal-based workflow alongside NVIM and task managers.

## Core Value

Keep music playback inside the terminal workflow - no context switching to external apps, everything stays in Tmux.

## Requirements

### Validated

(None yet — ship to validate)

### Active

- [ ] Connect to existing Plex Media Server
- [ ] Display list of available playlists
- [ ] Select and play a playlist
- [ ] Playback controls (play, pause, skip forward/back)
- [ ] Shuffle mode for playlists
- [ ] Display current track information (artist, album, track name)
- [ ] Show playback progress and time
- [ ] Vim keybindings for navigation and control
- [ ] Visual spectrum equalizer animation (aesthetic, toggleable)
- [ ] Favorite playlists with quick keybindings (e.g., press 1-9 to instantly start a favorite)
- [ ] Local audio playback

### Out of Scope

- Album/artist browsing — Focus is playlist-based listening, not library exploration
- Queue management — Simple playlist playback, not manual queue building
- Smart features (radio, mixes, recommendations) — Using Plex's existing playlists
- Mobile or non-terminal interfaces — Terminal-only

## Context

**Usage pattern:** Background music while working - primarily mixes and ambient playlists. Music plays while user focuses on code or tasks in other Tmux panes.

**Existing setup:** User has running Plex Media Server with music library and playlists already configured. Uses Tmux with NVIM for editing and a task manager, creating an integrated terminal workspace.

**User profile:** Vim power user with vim-tmux plugin for pane navigation. Keyboard-centric workflow, no mouse usage.

## Constraints

- **Platform**: Must run on WSL and Linux
- **Integration**: Must work seamlessly in Tmux environment
- **Input**: Keyboard-only, vim-style keybindings required
- **Audio**: Local playback on the machine running the TUI
- **Dependencies**: Must connect to existing Plex Media Server (local or remote)

## Key Decisions

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| Playlist-focused over library browsing | User's primary use case is selecting pre-made playlists for background music | — Pending |
| Vim keybindings | User is vim power user, consistency with their existing workflow (NVIM, vim-tmux) | — Pending |
| Toggleable visualizer | Adds terminal aesthetic but might be distracting during work | — Pending |

---
*Last updated: 2026-02-08 after initialization*
