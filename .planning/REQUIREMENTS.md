# Requirements: TermTunes

**Defined:** 2026-02-08
**Core Value:** Keep music playback inside the terminal workflow - no context switching to external apps, everything stays in Tmux.

## v1 Requirements

Requirements for initial release. Each maps to roadmap phases.

### Authentication

- [ ] **AUTH-01**: User can authenticate with Plex server via PIN-based OAuth flow
- [ ] **AUTH-02**: Application persists authentication token across restarts
- [ ] **AUTH-03**: Application detects expired tokens and prompts re-authentication
- [ ] **AUTH-04**: Application validates token on startup

### Playback

- [ ] **PLAY-01**: User can play a selected playlist from Plex
- [ ] **PLAY-02**: User can pause playback
- [ ] **PLAY-03**: User can stop playback
- [ ] **PLAY-04**: User can skip to next track
- [ ] **PLAY-05**: User can skip to previous track
- [ ] **PLAY-06**: User can increase volume
- [ ] **PLAY-07**: User can decrease volume
- [ ] **PLAY-08**: User can toggle shuffle mode for current playlist
- [ ] **PLAY-09**: User can cycle through repeat modes (off/all/one)
- [ ] **PLAY-10**: User can seek forward within current track
- [ ] **PLAY-11**: User can seek backward within current track

### Playlists

- [ ] **LIST-01**: Application displays list of all available playlists from Plex server
- [ ] **LIST-02**: User can navigate playlist list with keyboard
- [ ] **LIST-03**: User can select a playlist to play
- [ ] **LIST-04**: User can assign up to 9 playlists as favorites (1-9 keybindings)
- [ ] **LIST-05**: User can start a favorite playlist by pressing its assigned number key

### Display

- [ ] **DISP-01**: Application displays current track name
- [ ] **DISP-02**: Application displays current track artist
- [ ] **DISP-03**: Application displays current track album
- [ ] **DISP-04**: Application displays playback progress bar
- [ ] **DISP-05**: Application displays elapsed time and total duration
- [ ] **DISP-06**: Application displays current playback state (playing/paused/stopped)
- [ ] **DISP-07**: Application displays current volume level
- [ ] **DISP-08**: Application displays shuffle and repeat mode indicators
- [ ] **DISP-09**: Application adapts layout for small terminal panes (30-40 columns)
- [ ] **DISP-10**: Application handles terminal resize gracefully

### Keybindings

- [ ] **KEY-01**: User can navigate lists with j/k (vim-style down/up)
- [ ] **KEY-02**: User can select items with Enter
- [ ] **KEY-03**: User can toggle play/pause with Space
- [ ] **KEY-04**: User can quit application with q
- [ ] **KEY-05**: All navigation and controls work without mouse input
- [ ] **KEY-06**: User can seek with h/l (vim-style left/right)

### Integration & Polish

- [ ] **POL-01**: Application displays toggleable audio visualizer (spectrum)
- [ ] **POL-02**: User can toggle visualizer on/off with v key
- [ ] **POL-03**: Application writes current track info to file for tmux status bar integration
- [ ] **POL-04**: Application persists playback session (playlist, position) across restarts
- [ ] **POL-05**: Application restores last session on startup
- [ ] **POL-06**: Application runs reliably on WSL2 and Linux

## v2 Requirements

Deferred to future release. Tracked but not in current roadmap.

### Advanced Features

- **ADV-01**: Application supports MPRIS D-Bus interface for media key control
- **ADV-02**: Application supports gapless playback between tracks
- **ADV-03**: Application displays synced lyrics when available
- **ADV-04**: Application supports Last.fm scrobbling
- **ADV-05**: User can search/filter playlists by name

## Out of Scope

Explicitly excluded. Documented to prevent scope creep.

| Feature | Reason |
|---------|--------|
| Full library browsing (artists/albums/tracks) | Playlist-focused design. Library browsing is a separate product. Use Plex web UI for exploration. |
| Manual queue management | Contradicts simple playlist playback model. Adds significant UI complexity. |
| Smart recommendations/radio/AI mixes | Complex Plex API features outside scope. Users curate playlists in Plex. |
| Crossfade/EQ/audio effects | Significant audio processing complexity. System-level configuration more appropriate. |
| Mouse support | Target user is keyboard-only (vim power user). Mouse handling adds unnecessary complexity. |
| Tag editing | Dangerous to modify Plex metadata from terminal. Use Plex web UI. |
| Downloading/offline mode | Plex handles streaming. Offline storage creates complexity. Streaming-only is acceptable. |
| Multi-server support | One server is expected use case. Single-server keeps configuration simple. |

## Traceability

Which phases cover which requirements. Updated during roadmap creation.

| Requirement | Phase | Status |
|-------------|-------|--------|
| (To be populated by roadmap) | | |

**Coverage:**
- v1 requirements: 38 total
- Mapped to phases: 0
- Unmapped: 38 ⚠️

---
*Requirements defined: 2026-02-08*
*Last updated: 2026-02-08 after initial definition*
