# Roadmap: TermTunes

## Overview

TermTunes goes from zero to a daily-driveable terminal music player in five phases, ordered by risk. Phase 1 validates the two make-or-break unknowns -- WSL2 audio playback and Plex authentication -- before any UI investment. Phases 2 and 3 build the complete TUI with playback controls and differentiating features (favorite playlist hotkeys, shuffle, seek). Phase 4 optimizes for the real usage context (narrow tmux panes, session persistence, status bar integration). Phase 5 adds the aesthetic visualizer last, since it is the highest complexity and lowest priority for background listening.

## Phases

**Phase Numbering:**
- Integer phases (1, 2, 3): Planned milestone work
- Decimal phases (2.1, 2.2): Urgent insertions (marked with INSERTED)

Decimal phases appear between their surrounding integers in numeric order.

- [ ] **Phase 1: Foundation and Audio Proof-of-Concept** - Validate WSL2 audio, Plex authentication, and terminal lifecycle before investing in UI
- [ ] **Phase 2: Core TUI and Playback** - Build the playlist browser, player bar, and full playback controls with vim keybindings
- [ ] **Phase 3: Differentiators** - Add favorite playlist hotkeys, shuffle, repeat, and seek -- the features that make TermTunes unique
- [ ] **Phase 4: Tmux Integration and Polish** - Optimize for narrow tmux panes, add status bar integration and session persistence
- [ ] **Phase 5: Audio Visualizer** - Add toggleable spectrum visualizer as the final aesthetic layer

## Phase Details

### Phase 1: Foundation and Audio Proof-of-Concept
**Goal**: Audio plays reliably on WSL2 through a Plex-authenticated connection, with proper terminal state management -- proving the project's technical viability before any UI work
**Depends on**: Nothing (first phase)
**Requirements**: AUTH-01, AUTH-02, AUTH-03, AUTH-04, PLAY-01, PLAY-02, PLAY-03, KEY-04, POL-06
**Success Criteria** (what must be TRUE):
  1. User can authenticate with their Plex server via the PIN-based OAuth flow and the token persists across application restarts
  2. User can play, pause, and resume a track from a Plex playlist with audio output on WSL2 -- including after pauses longer than 5 seconds
  3. Application detects an expired or invalid Plex token on startup and prompts re-authentication instead of silently failing
  4. Application restores terminal to a clean state after quit (q key), crash, or signal termination -- no corrupted terminal
  5. Application compiles and runs on both WSL2 and native Linux
**Plans**: 3 plans

Plans:
- [ ] 01-01-PLAN.md -- Scaffold Rust project, terminal lifecycle (panic hooks, signal handlers), config persistence
- [ ] 01-02-PLAN.md -- Plex PIN-based OAuth authentication, server discovery, playlist/track API client
- [ ] 01-03-PLAN.md -- Audio playback via rodio, play/pause controls, status bar, terminal restoration test script

### Phase 2: Core TUI and Playback
**Goal**: User can browse playlists, select one, and control playback with full vim-style keyboard controls through a functional terminal UI
**Depends on**: Phase 1
**Requirements**: LIST-01, LIST-02, LIST-03, PLAY-04, PLAY-05, PLAY-06, PLAY-07, DISP-01, DISP-02, DISP-03, DISP-04, DISP-05, DISP-06, DISP-07, KEY-01, KEY-02, KEY-03, KEY-05
**Success Criteria** (what must be TRUE):
  1. User can see all their Plex playlists listed, navigate with j/k, and press Enter to start playback
  2. User can skip forward/back between tracks, adjust volume up/down, and toggle play/pause -- all with keyboard-only controls
  3. Player bar displays current track name, artist, album, playback state, volume level, and a progress bar with elapsed/total time
  4. All interaction works without mouse input -- vim keybindings are the only navigation method
**Plans**: 2 plans

Plans:
- [ ] 02-01-PLAN.md -- Player controls (volume, position), app state (NowPlaying, track index, auto-advance, keybindings)
- [ ] 02-02-PLAN.md -- UI rewrite with 3-line player bar (track info, progress, status) and human verification

### Phase 3: Differentiators
**Goal**: User can assign favorite playlists to number keys for instant access, shuffle and repeat playlists, and seek within tracks
**Depends on**: Phase 2
**Requirements**: PLAY-08, PLAY-09, PLAY-10, PLAY-11, LIST-04, LIST-05, DISP-08, KEY-06
**Success Criteria** (what must be TRUE):
  1. User can assign up to 9 playlists as favorites and start any favorite by pressing its number key (1-9) from anywhere in the app
  2. User can toggle shuffle mode and cycle through repeat modes (off/all/one), with indicators visible in the player bar
  3. User can seek forward and backward within the current track using h/l keys
**Plans**: TBD

Plans:
- [ ] 03-01: TBD
- [ ] 03-02: TBD

### Phase 4: Tmux Integration and Polish
**Goal**: Application works seamlessly in narrow tmux panes, persists sessions across restarts, and writes now-playing info for tmux status bar display
**Depends on**: Phase 3
**Requirements**: DISP-09, DISP-10, POL-03, POL-04, POL-05
**Success Criteria** (what must be TRUE):
  1. Application adapts its layout cleanly in terminal panes as narrow as 30-40 columns, and handles terminal resize without visual corruption
  2. Tmux status bar displays the currently playing track name (read from a file written by the application)
  3. User can close the application and reopen it later to resume the same playlist at the same position
**Plans**: TBD

Plans:
- [ ] 04-01: TBD
- [ ] 04-02: TBD

### Phase 5: Audio Visualizer
**Goal**: User can toggle an aesthetic spectrum visualizer that runs alongside playback without degrading audio quality or UI responsiveness
**Depends on**: Phase 4
**Requirements**: POL-01, POL-02
**Success Criteria** (what must be TRUE):
  1. User can press v to toggle a visual spectrum/equalizer animation that responds to the playing audio
  2. Visualizer does not cause audio dropouts, UI lag, or noticeable CPU overhead during normal playback
**Plans**: TBD

Plans:
- [ ] 05-01: TBD

## Progress

**Execution Order:**
Phases execute in numeric order: 1 -> 2 -> 3 -> 4 -> 5

| Phase | Plans Complete | Status | Completed |
|-------|----------------|--------|-----------|
| 1. Foundation and Audio PoC | 3/3 | Complete | 2026-02-10 |
| 2. Core TUI and Playback | 2/2 | Complete | 2026-02-10 |
| 3. Differentiators | 0/2 | Not started | - |
| 4. Tmux Integration and Polish | 0/2 | Not started | - |
| 5. Audio Visualizer | 0/1 | Not started | - |

---
*Roadmap created: 2026-02-10*
*Last updated: 2026-02-10*
