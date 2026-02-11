# Roadmap: TermTunes

## Milestones

- v1.0 MVP — Phases 1-5 (shipped 2026-02-10)
- v1.1 Multi-Channel Audio — Phases 6-9 (in progress)

## Phases

<details>
<summary>v1.0 MVP (Phases 1-5) — SHIPPED 2026-02-10</summary>

- [x] Phase 1: Foundation and Audio Proof-of-Concept (3/3 plans) — completed 2026-02-10
- [x] Phase 2: Core TUI and Playback (2/2 plans) — completed 2026-02-10
- [x] Phase 3: Differentiators (2/2 plans) — completed 2026-02-10
- [x] Phase 4: Tmux Integration and Polish (2/2 plans) — completed 2026-02-10
- [x] Phase 5: Audio Visualizer (1/1 plan) — completed 2026-02-10

**Archive:** See `.planning/milestones/v1.0-ROADMAP.md` for full phase details

</details>

### v1.1 Multi-Channel Audio (In Progress)

**Milestone Goal:** Layer ambient tracks underneath music playlists for enhanced focus during deep work.

**Phase Numbering:**
- Integer phases (6, 7, 8, 9): Planned milestone work
- Decimal phases (7.1, 7.2): Urgent insertions if needed (marked INSERTED)

- [ ] **Phase 6: Dual-Sink Audio Engine** — Second audio channel with independent volume and loop control
- [x] **Phase 7: Track Browsing & Ambient Playback** — Browse Plex library, select track, play on ambient channel
- [ ] **Phase 8: Ambient Status UI & Controls** — Status panel, volume display, and dedicated keybindings
- [ ] **Phase 9: Session Persistence** — Save and restore ambient state across app restarts

## Phase Details

### Phase 6: Dual-Sink Audio Engine
**Goal**: User can play two audio sources simultaneously -- main music and an ambient track -- with independent volume and continuous looping
**Depends on**: Phase 5 (v1.0 complete)
**Requirements**: AUDIO-01, AUDIO-02, AUDIO-03, AUDIO-04, AUDIO-05, AUDIO-06, AUDIO-07
**Success Criteria** (what must be TRUE):
  1. User hears ambient audio playing at the same time as main music with no crackling, distortion, or dropout on WSL2
  2. User can set ambient volume independently and the combined output never clips (volume budget enforced, main + ambient <= 1.0)
  3. Ambient track loops continuously for 30+ minutes with stable memory usage (no growth beyond initial load)
  4. User can mute/unmute ambient at the audio engine level without affecting main music playback
  5. All existing v1.0 playback functionality works identically (no regressions from Player refactor)
**Plans**: 2 plans

Plans:
- [ ] 06-01-PLAN.md -- Refactor Player to dual-sink architecture with volume budget enforcement
- [ ] 06-02-PLAN.md -- Wire ambient loop into event loop, test trigger, and WSL2 verification

### Phase 7: Track Browsing & Ambient Playback
**Goal**: User can browse their Plex music library, select a track, and have it play as the ambient channel
**Depends on**: Phase 6
**Requirements**: TRACK-01, TRACK-02, TRACK-03, TRACK-04, TRACK-05, TRACK-06, UI-02, UI-03, UI-04, UI-08, UI-09
**Success Criteria** (what must be TRUE):
  1. User presses a keybinding and sees a modal browser overlay listing Plex music library sections
  2. User navigates into a section and sees its tracks, selects one with vim-style keybindings (j/k, Enter, Esc)
  3. Selected track downloads and automatically starts playing on the ambient channel without interrupting main music
  4. User can change the ambient track by reopening the browser and selecting a different track while music continues
  5. Browser overlay closes cleanly after selection or cancel, returning to normal view
**Plans**: 2 plans

Plans:
- [x] 07-01-PLAN.md -- Plex library API endpoints, BrowserState enum, browser input routing and key handler
- [x] 07-02-PLAN.md -- Browser popup overlay rendering in ui.rs and end-to-end verification

### Phase 8: Ambient Status UI & Controls
**Goal**: User has full visibility into ambient state and can control it efficiently with dedicated keybindings
**Depends on**: Phase 7
**Requirements**: UI-01, UI-05, UI-06, UI-07
**Success Criteria** (what must be TRUE):
  1. User sees a dedicated UI panel showing the ambient track name, play/pause state, and current volume level
  2. User can adjust ambient volume up/down with dedicated keybindings and sees the change reflected in the UI immediately
  3. User can toggle ambient on/off with a single keybinding and sees the state change in the ambient panel
**Plans**: TBD

Plans:
- [ ] 08-01: TBD

### Phase 9: Session Persistence
**Goal**: User's ambient setup survives app restarts -- track selection, volume, and playback state all restored automatically
**Depends on**: Phase 8
**Requirements**: PERSIST-01, PERSIST-02, PERSIST-03, PERSIST-04, PERSIST-05
**Success Criteria** (what must be TRUE):
  1. User quits and restarts TermTunes and the same ambient track resumes playing at the same volume with the same on/off state
  2. On first-ever use (no saved ambient state), ambient volume defaults to 30% lower than main music volume
  3. Existing v1.0 session files load without error (backward compatibility preserved)
**Plans**: TBD

Plans:
- [ ] 09-01: TBD

## Progress

**Execution Order:**
Phases execute in numeric order: 6 -> 7 -> 8 -> 9

| Phase | Milestone | Plans Complete | Status | Completed |
|-------|-----------|----------------|--------|-----------|
| 1. Foundation and Audio PoC | v1.0 | 3/3 | Complete | 2026-02-10 |
| 2. Core TUI and Playback | v1.0 | 2/2 | Complete | 2026-02-10 |
| 3. Differentiators | v1.0 | 2/2 | Complete | 2026-02-10 |
| 4. Tmux Integration and Polish | v1.0 | 2/2 | Complete | 2026-02-10 |
| 5. Audio Visualizer | v1.0 | 1/1 | Complete | 2026-02-10 |
| 6. Dual-Sink Audio Engine | v1.1 | 0/2 | Not started | - |
| 7. Track Browsing & Ambient Playback | v1.1 | 2/2 | Complete | 2026-02-11 |
| 8. Ambient Status UI & Controls | v1.1 | 0/1 | Not started | - |
| 9. Session Persistence | v1.1 | 0/1 | Not started | - |

---

*Roadmap created: 2026-02-10*
*Last updated: 2026-02-11 after Phase 7 execution*
