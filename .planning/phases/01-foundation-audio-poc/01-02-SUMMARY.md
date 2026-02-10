---
phase: 01-foundation-audio-poc
plan: 02
subsystem: auth
tags: [rust, reqwest, plex, oauth, pin-auth, ratatui, list-widget, serde, tokio]

# Dependency graph
requires:
  - phase: 01-01
    provides: "Compilable Rust binary with TUI event loop, config module with TOML persistence and stable client_id UUID"
provides:
  - "Plex PIN-based OAuth authentication with token persistence"
  - "Token validation on startup with automatic re-authentication"
  - "Plex server discovery with multi-server support"
  - "Plex API client for playlists and tracks"
  - "TUI playlist/track browser with j/k navigation"
  - "Stream URL construction for audio playback"
affects: [01-03, 02-subsonic-auth, 03-playback-engine]

# Tech tracking
tech-stack:
  added: []
  patterns: [plex-pin-oauth, plex-api-json-headers, list-widget-stateful-nav, app-state-machine]

key-files:
  created:
    - src/auth.rs
    - src/plex.rs
  modified:
    - src/app.rs
    - src/main.rs
    - Cargo.toml

key-decisions:
  - "Auth flow runs before TUI init on normal terminal (not alternate screen) so URL is readable"
  - "tokio::main for async runtime -- event loop still uses synchronous crossterm polling"
  - "Server configs keyed by machine identifier (clientIdentifier) in config HashMap"
  - "Auto-select single server, numbered prompt for multi-server selection"
  - "AppView enum state machine for Playlists/Tracks views"
  - "reqwest query feature required for URL query parameters"

patterns-established:
  - "Plex headers pattern: build_plex_headers() constructs X-Plex-Product, X-Plex-Client-Identifier, Accept: application/json"
  - "PIN auth pattern: start_auth() returns (pin_id, code, url), wait_for_auth() polls every 1s with 5min timeout"
  - "Token validation pattern: GET /api/v2/user with X-Plex-Token, check status code"
  - "Server discovery pattern: GET /api/v2/resources, filter provides=server, pick best connection (local > direct > relay)"
  - "Stateful list navigation: ListState with wrap-around, j/k/Up/Down movement"
  - "Async event loop pattern: handle_key is async to allow API calls during TUI interaction"

# Metrics
duration: ~5min
completed: 2026-02-10
---

# Phase 1 Plan 2: Plex Auth and API Client Summary

**Plex PIN-based OAuth with token persistence, server discovery, and TUI playlist/track browser using ratatui List widgets with j/k navigation**

## Performance

- **Duration:** ~5 min
- **Started:** 2026-02-10T14:36:18Z
- **Completed:** 2026-02-10T14:41:21Z
- **Tasks:** 2
- **Files created:** 2
- **Files modified:** 3

## Accomplishments
- Implemented complete Plex PIN-based OAuth flow: create PIN, display auth URL, poll for token, validate on startup
- Built Plex API client with server discovery, playlist fetching, track fetching, and stream URL construction
- Integrated auth flow into app startup with automatic re-authentication on invalid/expired tokens
- Created TUI playlist/track browser with stateful list navigation (j/k/arrows/Enter/Esc)
- Multi-server support: config stores server details keyed by machine identifier, remembers last-used server

## Task Commits

Each task was committed atomically:

1. **Task 1: Implement Plex authentication and server discovery** - `fc6ffc6` (feat)
2. **Task 2: Integrate auth flow into app startup with token validation and server selection** - `3f3a880` (feat)

## Files Created/Modified
- `src/auth.rs` - Plex PIN-based OAuth: build_plex_headers, create_pin, check_pin, start_auth, wait_for_auth, validate_token
- `src/plex.rs` - Plex API client: PlexClient (fetch_playlists, fetch_tracks, stream_url), discover_servers, serde types for all Plex responses
- `src/app.rs` - App state machine with AppView enum, authenticate(), playlist/track list rendering with ratatui, j/k/Enter/Esc navigation
- `src/main.rs` - Converted to #[tokio::main] for async runtime, auth before TUI init, playlist fetch on startup
- `Cargo.toml` - Added reqwest "query" feature for URL query parameter support

## Decisions Made
- **Auth before TUI:** Authentication runs before entering the TUI alternate screen so the auth URL is displayed on the normal terminal where the user can see and copy it.
- **tokio::main:** Converted main() to async with #[tokio::main] to support async Plex API calls. The event loop still uses synchronous crossterm polling for responsiveness.
- **Machine identifier keys:** Server configs in config.toml are keyed by Plex machine identifier (clientIdentifier) which is stable across server restarts and IP changes.
- **Auto-select single server:** When only one Plex server is found, it is automatically selected. Multiple servers prompt a numbered list on stdin.
- **reqwest query feature:** Discovered that reqwest's .query() method requires the "query" feature flag (not enabled by default). Added to Cargo.toml.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Added reqwest "query" feature to Cargo.toml**
- **Found during:** Task 1 (building auth.rs and plex.rs)
- **Issue:** reqwest's `.query()` method is feature-gated behind the "query" feature, which was not in the original Cargo.toml dependencies.
- **Fix:** Added "query" to reqwest features list in Cargo.toml
- **Files modified:** Cargo.toml
- **Verification:** cargo build succeeds, .query() calls compile
- **Committed in:** `fc6ffc6` (Task 1 commit)

**2. [Rule 3 - Blocking] Added mod auth/plex declarations to main.rs in Task 1**
- **Found during:** Task 1 (verifying compilation)
- **Issue:** Plan specified adding module declarations in Task 2 Step 1, but Task 1 verification requires `cargo build` to succeed with the new files. Without mod declarations, auth.rs and plex.rs are not compiled.
- **Fix:** Added `mod auth; mod plex;` to main.rs as part of Task 1 instead of waiting for Task 2
- **Files modified:** src/main.rs
- **Verification:** cargo build succeeds with all serde structs and functions compiled
- **Committed in:** `fc6ffc6` (Task 1 commit)

---

**Total deviations:** 2 auto-fixed (2 blocking issues)
**Impact on plan:** Both fixes were necessary for compilation. No scope creep -- identical outcome, just earlier module declarations and a missing feature flag.

## Issues Encountered
- reqwest 0.13.2 has the `.query()` method behind a feature gate that is not obvious from the API documentation. This was discovered during the first build attempt and fixed by adding the "query" feature to Cargo.toml.

## User Setup Required
None - Plex authentication is handled interactively at runtime via PIN flow.

## Next Phase Readiness
- Auth and API client complete, ready for Plan 01-03 (audio playback PoC)
- PlexClient.stream_url() is ready to construct download URLs for tracks
- Track selection in TUI is wired but currently a no-op (playback will be added in Plan 03)
- Config persistence verified with multi-server support
- No blockers for next plan

## Self-Check: PASSED

All referenced files exist, all commits verified, summary file created.

---
*Phase: 01-foundation-audio-poc*
*Completed: 2026-02-10*
