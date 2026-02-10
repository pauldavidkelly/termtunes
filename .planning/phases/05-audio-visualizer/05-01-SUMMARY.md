---
phase: 05-audio-visualizer
plan: 01
subsystem: ui
tags: [fft, spectrum-analyzer, ratatui, barchart, rodio, audio-visualization]

# Dependency graph
requires:
  - phase: 01-foundation-audio-poc
    provides: "rodio audio playback pipeline (Player, Decoder, Sink)"
  - phase: 02-core-tui-playback
    provides: "ratatui TUI layout, player bar, track list rendering"
  - phase: 04-tmux-integration-polish
    provides: "adaptive layout with narrow/short terminal detection"
provides:
  - "Toggleable audio spectrum visualizer responding to playing audio"
  - "VisualizerSource rodio wrapper for real-time sample capture"
  - "FFT-based frequency spectrum computation with logarithmic binning"
  - "Exponential smoothing for animated bar display (fast attack, slow decay)"
  - "v keybinding for visualizer toggle"
affects: []

# Tech tracking
tech-stack:
  added: [spectrum-analyzer 1.7]
  patterns: [source-tap-pattern, fft-on-ui-thread, try_lock-audio-thread, circular-buffer, logarithmic-frequency-binning, exponential-smoothing]

key-files:
  created:
    - src/visualizer.rs
  modified:
    - Cargo.toml
    - src/player.rs
    - src/app.rs
    - src/ui.rs
    - src/main.rs

key-decisions:
  - "spectrum-analyzer crate for FFT (wraps microfft with windowing and scaling)"
  - "try_lock() in audio thread to never block playback (skip sample on lock failure)"
  - "FFT computed on UI thread at render tick rate (~10Hz), not on audio thread"
  - "32 default bars with dynamic adjustment based on terminal width (4..64 range)"
  - "Auto-hide visualizer in narrow (<40 cols) and short (<20 rows) terminals"
  - "8-row visualizer area (6 content + 2 border) for adequate bar resolution"

patterns-established:
  - "Source tap pattern: VisualizerSource<S> wraps any rodio Source, copies left-channel samples to shared buffer"
  - "Arc<Mutex<VisualizerData>> for audio/UI thread communication with try_lock in audio path"
  - "Conditional 3-part layout: track list + visualizer + player bar when visualizer enabled"

# Metrics
duration: 5min
completed: 2026-02-10
---

# Phase 5 Plan 1: Audio Visualizer Summary

**Toggleable FFT spectrum visualizer using spectrum-analyzer for frequency analysis and ratatui BarChart for animated bar rendering, toggled with v key**

## Performance

- **Duration:** 5 min
- **Started:** 2026-02-10T18:49:20Z
- **Completed:** 2026-02-10T18:54:49Z
- **Tasks:** 2
- **Files modified:** 7

## Accomplishments
- Real-time audio spectrum visualizer that responds to playing audio via FFT
- Zero-overhead when disabled: no FFT computation, no sample capture overhead, no layout impact
- Smooth bar animation with fast attack (0.7) and slow decay (0.9) mimicking hardware equalizers
- Auto-hides in narrow/short terminals to preserve existing layout quality
- v keybinding toggles visualizer, integrated with help text

## Task Commits

Each task was committed atomically:

1. **Task 1: Visualizer engine and player integration** - `b0e7b7b` (feat)
2. **Task 2: App state toggle, v keybinding, and UI layout integration** - `b818e2f` (feat)

## Files Created/Modified
- `src/visualizer.rs` - New module: VisualizerSource wrapper, SampleBuffer, FFT computation with logarithmic binning, VisualizerState smoothing, BarChart rendering
- `Cargo.toml` - Added spectrum-analyzer 1.7 dependency
- `src/player.rs` - Modified load_and_play and replay_current to accept optional VisualizerData, wrap decoder in VisualizerSource
- `src/app.rs` - Added visualizer fields, v keybinding, update_visualizer() call in event loop, visualizer data passed to player
- `src/ui.rs` - Conditional 3-part layout with visualizer area, render_visualizer_area function, v:viz in help text
- `src/main.rs` - Added mod visualizer declaration

## Decisions Made
- Used spectrum-analyzer crate (wraps microfft) instead of hand-rolling FFT -- eliminates ~200 lines of windowing/bin mapping code
- try_lock() in audio thread source wrapper ensures zero risk of audio dropouts from lock contention
- FFT computed on UI thread at render tick rate (~10Hz), never on audio thread
- 32 default bars with dynamic width-based adjustment (4..64 range) for optimal display
- Visualizer auto-hides in narrow (<40 cols) and short (<20 rows) terminals per existing adaptive layout pattern
- 8-row visualizer area (Constraint::Length(8)) provides 6 rows of bar content for adequate visual resolution

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Phase 5 is the final phase. Audio visualizer completes the v1.0 feature set.
- All 5 phases complete: foundation, core TUI, differentiators, tmux integration, audio visualizer.

## Self-Check: PASSED

All files exist, all commits verified, all content markers found, build succeeds.

---
*Phase: 05-audio-visualizer*
*Completed: 2026-02-10*
