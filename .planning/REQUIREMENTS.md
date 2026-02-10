# Requirements: TermTunes v1.1

**Defined:** 2026-02-10
**Core Value:** Keep music playback inside the terminal workflow - no context switching to external apps, everything stays in Tmux.

## v1.1 Requirements

Requirements for multi-channel audio (ambient layer). Each maps to roadmap phases.

### Audio Engine

- [ ] **AUDIO-01**: System creates second audio sink for ambient channel on same OutputStream
- [ ] **AUDIO-02**: User can set ambient volume independently from main music volume
- [ ] **AUDIO-03**: System enforces volume budget (main + ambient <= 1.0) to prevent clipping
- [ ] **AUDIO-04**: User can toggle ambient channel on/off (mute/unmute)
- [ ] **AUDIO-05**: Ambient track loops continuously using manual re-append (not repeat_infinite)
- [ ] **AUDIO-06**: System maintains stable memory usage during extended ambient looping
- [ ] **AUDIO-07**: Main music playback continues uninterrupted while ambient plays

### Track Selection

- [ ] **TRACK-01**: User can browse Plex music library sections
- [ ] **TRACK-02**: User can view list of tracks within a library section
- [ ] **TRACK-03**: User can select a track from library for ambient channel
- [ ] **TRACK-04**: System downloads selected ambient track to local temp storage
- [ ] **TRACK-05**: Ambient track starts playing automatically after download completes
- [ ] **TRACK-06**: User can change ambient track without stopping main music

### UI & Controls

- [ ] **UI-01**: User sees ambient track status in dedicated UI panel (track name, play/pause state)
- [ ] **UI-02**: User can open track browser as modal popup overlay
- [ ] **UI-03**: Track browser displays without interrupting current playback
- [ ] **UI-04**: User can navigate track browser with vim-style keybindings
- [ ] **UI-05**: UI shows current ambient volume level
- [ ] **UI-06**: User can adjust ambient volume up/down with dedicated keybindings
- [ ] **UI-07**: User can toggle ambient on/off with dedicated keybinding
- [ ] **UI-08**: User can open ambient track browser with dedicated keybinding
- [ ] **UI-09**: Ambient track browser closes after selection or cancel

### Persistence

- [ ] **PERSIST-01**: System saves ambient track selection across app restarts
- [ ] **PERSIST-02**: System saves ambient volume setting across app restarts
- [ ] **PERSIST-03**: System saves ambient on/off state across app restarts
- [ ] **PERSIST-04**: System resumes ambient playback on startup if it was playing
- [ ] **PERSIST-05**: Ambient volume defaults to 30% lower than main music on first use

## v2 Requirements

Deferred to future release. Tracked but not in current roadmap.

### Advanced Features

- **ADV-01**: User can assign ambient track to favorite hotkey (0 or a1-a9)
- **ADV-02**: User can search/filter tracks in browser
- **ADV-03**: User can browse hierarchical library (artists -> albums -> tracks)
- **ADV-04**: User can create ambient playlists (multiple looping tracks)
- **ADV-05**: User can set crossfade between ambient tracks
- **ADV-06**: System provides master volume control affecting both channels

## Out of Scope

Explicitly excluded. Documented to prevent scope creep.

| Feature | Reason |
|---------|--------|
| More than 2 audio channels | UI complexity explosion, diminishing returns for terminal workflow |
| Ambient playlists | v1.1 focuses on single looping tracks, playlists deferred to v2 |
| Streaming ambient (vs download-then-play) | Existing download pattern is proven reliable on WSL2 |
| Crossfade between tracks | Abrupt loop is acceptable for MVP, adds complexity |
| Master volume (affects both channels) | Independent control is simpler and more flexible |
| Real-time audio mixing at Source level | Dual-Sink architecture is cleaner, validated in research |
| Hierarchical artist/album browsing | Flat track list sufficient for ambient selection use case |
| Ambient visualizer | Main music visualizer is the focus, ambient is background |

## Traceability

Which phases cover which requirements. Updated during roadmap creation.

| Requirement | Phase | Status |
|-------------|-------|--------|
| AUDIO-01 | Phase 6 | Pending |
| AUDIO-02 | Phase 6 | Pending |
| AUDIO-03 | Phase 6 | Pending |
| AUDIO-04 | Phase 6 | Pending |
| AUDIO-05 | Phase 6 | Pending |
| AUDIO-06 | Phase 6 | Pending |
| AUDIO-07 | Phase 6 | Pending |
| TRACK-01 | Phase 7 | Pending |
| TRACK-02 | Phase 7 | Pending |
| TRACK-03 | Phase 7 | Pending |
| TRACK-04 | Phase 7 | Pending |
| TRACK-05 | Phase 7 | Pending |
| TRACK-06 | Phase 7 | Pending |
| UI-01 | Phase 8 | Pending |
| UI-02 | Phase 7 | Pending |
| UI-03 | Phase 7 | Pending |
| UI-04 | Phase 7 | Pending |
| UI-05 | Phase 8 | Pending |
| UI-06 | Phase 8 | Pending |
| UI-07 | Phase 8 | Pending |
| UI-08 | Phase 7 | Pending |
| UI-09 | Phase 7 | Pending |
| PERSIST-01 | Phase 9 | Pending |
| PERSIST-02 | Phase 9 | Pending |
| PERSIST-03 | Phase 9 | Pending |
| PERSIST-04 | Phase 9 | Pending |
| PERSIST-05 | Phase 9 | Pending |

**Coverage:**
- v1.1 requirements: 27 total
- Mapped to phases: 27
- Unmapped: 0

---
*Requirements defined: 2026-02-10*
*Last updated: 2026-02-10 after roadmap traceability mapping*
