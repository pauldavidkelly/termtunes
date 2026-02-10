# Phase 1: Foundation and Audio Proof-of-Concept - Context

**Gathered:** 2026-02-10
**Status:** Ready for planning

<domain>
## Phase Boundary

Validate the technical foundation - WSL2 audio playback, Plex authentication with token persistence, and terminal lifecycle management - before investing in any UI work. This phase proves the project's technical viability.

</domain>

<decisions>
## Implementation Decisions

### Plex Authentication Flow
- Display URL + PIN in terminal, user manually opens browser (no auto-open)
- Store auth token in `~/.config/termtunes/config.toml` (XDG standard location, TOML format)
- On expired/invalid token: automatically prompt re-authentication (no error messages, just restart PIN flow)
- **Multi-server support:** Store multiple server tokens, default to last-used server on startup, allow server selection via settings/config

### Audio Playback Verification
- User selects playlist + track via simple CLI menu to start playback
- Manual keyboard control for pause/resume testing (spacebar toggles play/pause)
- **Controls in Phase 1:** Play/pause ONLY - proves audio works, no volume/skip yet
- Minimal TUI with status display (single-line status bar showing track name and play/pause state)

### Terminal State Handling
- Catch signals: SIGINT, SIGTERM, SIGHUP (covers Ctrl+C, kill, terminal close/tmux scenarios)
- Terminal restoration: Claude's discretion on what to restore (cursor, screen buffer, input mode)
- Panic hooks: Claude decides whether to install panic hook for cleanup
- **Verification:** Automated validation script that launches app, kills it various ways, checks terminal state

### Claude's Discretion
- Exact terminal restoration steps (cursor visibility, alternate screen, input mode)
- Whether to use panic hooks for terminal cleanup
- CLI menu implementation for playlist/track selection
- Status bar design and layout details

</decisions>

<specifics>
## Specific Ideas

- Multi-server workflow: default to last-used server, but allow switching via settings
- Automated terminal restoration testing: script should test quit (q), Ctrl+C, kill signal, SIGHUP

</specifics>

<deferred>
## Deferred Ideas

None - discussion stayed within phase scope

</deferred>

---

*Phase: 01-foundation-audio-poc*
*Context gathered: 2026-02-10*
