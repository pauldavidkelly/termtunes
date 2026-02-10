---
phase: 01-foundation-audio-poc
plan: 01
subsystem: infra
tags: [rust, ratatui, crossterm, signal-hook, toml, uuid, color-eyre, tracing]

# Dependency graph
requires: []
provides:
  - "Compilable Rust binary with TUI event loop"
  - "Terminal lifecycle management (panic hook, signal handlers, clean restore)"
  - "Config module with TOML persistence and stable client_id UUID"
  - "App state skeleton with shutdown flag and event polling"
affects: [01-02, 01-03, 02-subsonic-auth, 03-playback-engine]

# Tech tracking
tech-stack:
  added: [ratatui, crossterm, rodio, tokio, reqwest, serde, toml, dirs, uuid, signal-hook, color-eyre, tracing, tracing-subscriber]
  patterns: [panic-hook-before-init, signal-flag-polling, xdg-config-path, raw-mode-ctrl-c-handling]

key-files:
  created:
    - Cargo.toml
    - src/main.rs
    - src/tui.rs
    - src/config.rs
    - src/app.rs
  modified: []

key-decisions:
  - "Handle Ctrl+C as crossterm KeyEvent in raw mode rather than relying solely on signal-hook SIGINT"
  - "Use color_eyre::Result throughout for consistent error handling"
  - "Log to ~/.local/share/termtunes/termtunes.log to keep terminal clean"
  - "Config permissions set to 0o600 for security (will store auth tokens later)"

patterns-established:
  - "Panic hook pattern: install_panic_hook() calls restore_terminal() before delegating to original hook"
  - "Signal handler pattern: Arc<AtomicBool> shutdown flag polled in event loop"
  - "Ctrl+C in raw mode: crossterm captures Ctrl+C as KeyEvent(C, ctrl), must handle explicitly in event loop"
  - "Config pattern: load_config creates with new UUID if missing, save_config ensures parent dirs"

# Metrics
duration: ~15min
completed: 2026-02-10
---

# Phase 1 Plan 1: Project Scaffold Summary

**Rust TUI binary with ratatui, clean terminal restoration across all exit paths (q/Ctrl+C/SIGTERM/SIGHUP/panic), and TOML config persistence with stable client_id UUID**

## Performance

- **Duration:** ~15 min (execution) + human verification checkpoint
- **Started:** 2026-02-10T13:06:34Z
- **Completed:** 2026-02-10T14:46:00Z
- **Tasks:** 2 (1 auto + 1 human-verify checkpoint)
- **Files created:** 5

## Accomplishments
- Scaffolded Rust project with all Phase 1 dependencies (ratatui, rodio, crossterm, reqwest, etc.)
- Implemented terminal lifecycle management: panic hook, signal handlers (SIGINT/SIGTERM/SIGHUP), and restore function
- Created config module with XDG path resolution, TOML serialization, and multi-server support structure
- Built app state skeleton with event loop, shutdown flag polling, and keyboard event handling
- All 7 verification checks passed by human reviewer

## Task Commits

Each task was committed atomically:

1. **Task 1: Scaffold Rust project with dependencies, terminal lifecycle, and config module** - `4a8282e` (feat)
2. **Task 1.5: Fix Ctrl+C handling in raw mode** - `dffbf26` (fix)
3. **Task 2: Verify terminal restoration across all exit paths** - (human-verify checkpoint, no commit)

## Files Created/Modified
- `Cargo.toml` - Project manifest with all Phase 1 dependencies (ratatui, rodio, crossterm, reqwest, etc.)
- `src/main.rs` - Entry point: color_eyre init, tracing setup, panic hook, signal handlers, terminal init, app run, restore
- `src/tui.rs` - Terminal lifecycle: restore_terminal(), install_panic_hook(), install_signal_handlers()
- `src/config.rs` - Config persistence: Config/ServerConfig structs, load_config(), save_config() with XDG paths
- `src/app.rs` - App state: event loop with shutdown flag polling, q key and Ctrl+C handling, placeholder UI

## Decisions Made
- **Ctrl+C as KeyEvent:** In crossterm raw mode, Ctrl+C is captured as a key event rather than generating SIGINT. Added explicit handling in the event loop alongside the signal-hook SIGINT handler for robustness.
- **color_eyre for errors:** Used color_eyre::Result throughout all modules for consistent, readable error propagation.
- **File logging:** Tracing output goes to `~/.local/share/termtunes/termtunes.log` to keep the TUI screen clean.
- **600 permissions on config:** Config file gets restrictive permissions since it will store auth tokens in future plans.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed Ctrl+C not working in raw mode**
- **Found during:** Task 2 (human-verify checkpoint)
- **Issue:** Pressing Ctrl+C in the TUI did nothing. In crossterm raw mode, Ctrl+C is intercepted as a KeyEvent and never reaches the OS as SIGINT. The signal-hook handler only catches actual SIGINT signals (e.g., from `kill`), not keyboard Ctrl+C.
- **Fix:** Added explicit handling for `KeyCode::Char('c')` with `KeyModifiers::CONTROL` in the event loop to set `self.running = false`.
- **Files modified:** `src/app.rs`
- **Verification:** Human verified Ctrl+C now exits cleanly with terminal restored.
- **Committed in:** `dffbf26`

---

**Total deviations:** 1 auto-fixed (1 bug fix)
**Impact on plan:** Essential fix for correct Ctrl+C behavior. The plan's signal-hook approach works for external signals but raw mode requires in-loop key handling. No scope creep.

## Issues Encountered
- Ctrl+C behavior difference between raw mode and normal terminal operation was discovered during human verification. This is a well-known crossterm behavior but was not anticipated in the plan. Fixed inline before checkpoint approval.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Project foundation complete with all dependencies, terminal lifecycle, and config persistence
- Ready for Plan 01-02 (audio playback PoC) which will use rodio (already in Cargo.toml)
- Config module ready for Plan 01-03 (Subsonic auth) to store server tokens
- No blockers for next plan

## Self-Check: PASSED

All referenced files exist (5/5), all commits verified (2/2), summary file created.

---
*Phase: 01-foundation-audio-poc*
*Completed: 2026-02-10*
