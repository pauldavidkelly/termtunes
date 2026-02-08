# Pitfalls Research

**Domain:** TUI Music Player with Plex Integration (TermTunes)
**Researched:** 2026-02-08
**Confidence:** MEDIUM-HIGH (multiple sources corroborate; WSL audio pitfalls verified via GitHub issues)

## Critical Pitfalls

### Pitfall 1: WSLg Audio Breaks After Pause/Resume

**What goes wrong:**
Audio playback hangs indefinitely when resuming after pausing for more than ~5 seconds in WSL2. The PulseAudio stream fails to reinitialize, leaving the media player stuck until killed. This affects cmus, ncmpcpp, mpv, and any application using PulseAudio through WSLg. Separate from this, WSLg audio suffers from chronic latency (sounds echoed back up to 10 seconds later), crackling/stuttering (alternating 1-second audio / 1-second silence), and complete audio dropout under CPU load.

**Why it happens:**
WSLg provides only a rudimentary PulseAudio connection between Linux and Windows. The PulseAudio bridge does not properly handle stream state transitions (playing -> paused -> playing). The audio buffer management in the WSL PulseAudio shim is fragile -- when a stream is corked (paused) for more than a few seconds, the buffer state becomes inconsistent and the stream cannot resume. A fix exists in the PulseAudio repo but has not been merged into the WSLg system image as of WSL 2.6.1.0.

**How to avoid:**
- Decouple audio output from the TUI process entirely. Use a client/server architecture (like MPD) or stream audio via HTTP to a local player on the Windows side.
- If using PulseAudio directly: implement a watchdog that detects stalled playback (no audio callback for N ms after resume) and automatically tears down and recreates the audio stream rather than hanging.
- Consider outputting audio via a Windows-native player (e.g., streaming the Plex URL to a Windows media player process) and controlling it from the TUI, bypassing WSL audio entirely.
- Increase PulseAudio buffer sizes to reduce crackling: set `PULSE_LATENCY_MSEC=60` or higher.
- Test with `PULSE_SERVER=tcp:127.0.0.1` explicitly set, not relying on WSLg's automatic socket.

**Warning signs:**
- Audio works initially but hangs after first pause/resume cycle
- `pactl list sinks` shows the sink but `pactl list sink-inputs` shows no active inputs after resume
- Log messages about "buffer underrun" or "stream cork/uncork" failures
- Users on Windows 10 or older WSL versions report total silence

**Phase to address:**
Phase 1 (Foundation) -- this is an architectural decision. If WSL audio proves unreliable, the entire playback architecture must accommodate it. Must be validated in the first sprint with a proof-of-concept audio playback test.

---

### Pitfall 2: Plex Authentication Token Lifecycle Mismanagement

**What goes wrong:**
Third-party Plex apps hardcode or cache authentication tokens without handling expiration, revocation, or the full PIN-based OAuth flow. Tokens become invalid when users change passwords (with "sign out connected devices" checked), when servers restart (transient tokens last only 48 hours), or when Plex rotates credentials. The app silently fails or crashes instead of re-authenticating.

**Why it happens:**
The Plex auth system has multiple token types with different lifetimes: transient tokens (48 hours, invalid on server restart), user tokens (long-lived but revocable), and the newer JWT-based tokens (7-day refresh cycle). Developers often grab a token from the browser's developer tools during testing and hardcode it, never implementing the proper PIN-based flow. The PIN-based flow requires polling or forwarding, generating and persisting a stable Client Identifier, and handling PIN expiration gracefully.

**How to avoid:**
- Implement the full PIN-based authentication flow from day one. Generate a PIN via the Plex API, construct an auth URL, open it in the user's browser, and poll for completion.
- Store the Client Identifier (UUID) persistently -- reuse it across sessions. Plex uses this to identify your app instance.
- Send all required headers on every request: `X-Plex-Product`, `X-Plex-Client-Identifier`, `X-Plex-Token`, `accept: application/json`.
- Validate stored tokens on startup by making a test request (HTTP 200 = valid, 401 = expired).
- Implement automatic re-authentication: on any 401 response, trigger the PIN flow again rather than crashing.
- Consider the newer JWT flow (register public key, refresh every 7 days, exchange for X-Plex-Token) for more robust long-term auth.

**Warning signs:**
- App works for the developer but fails for other users
- "Works for a while then stops" reports from testers
- No token refresh logic in the codebase
- Token stored in plaintext config files without validation on load

**Phase to address:**
Phase 1 (Foundation) -- authentication is the gateway to every Plex feature. Build the PIN flow first, validate it works end-to-end, before building any library browsing.

---

### Pitfall 3: Rendering Large Music Libraries Causes UI Lag

**What goes wrong:**
Rendering a music library with thousands of tracks (common for Plex users with 10k-100k+ tracks) causes the TUI to become unresponsive. Scrolling lags 1-2 seconds per input. The entire UI freezes during library fetches. This is a two-layer problem: the Plex API is slow for large libraries (database queries on hundreds of thousands of metadata entries), and TUI frameworks like ratatui compute layout for ALL items on every render frame, not just visible ones.

**Why it happens:**
On the Plex side: music metadata is complex, library scans can take days, and API responses for large collections are slow. The `/library/sections/{id}/all` endpoint returns everything by default. On the TUI side: ratatui's Table and List widgets convert all items to vectors on every render cycle and compute `text().height()` for every item to determine which are visible. With 15k+ items, this causes noticeable frame drops.

**How to avoid:**
- **Mandatory pagination on the Plex side:** Use the `X-Plex-Container-Start` and `X-Plex-Container-Size` headers (or query params) to page through results. Never fetch an entire library in one call. The `/children` and `/grandchildren` endpoints require mandatory paging per Plex's own API guidelines.
- **Local caching:** Cache library metadata locally (SQLite or similar) and sync incrementally. Do not re-fetch the full library on every app launch.
- **Virtual scrolling on the TUI side:** Only pass visible items (plus a small buffer) to the ratatui widget. Pre-filter your dataset before passing it to `Table::new()` or `List::new()`. Do not construct widget items for off-screen rows.
- **Async data loading:** Fetch library data on a background thread/task. Show a loading indicator. Never block the render loop on network I/O.
- Profile with `cargo-flamegraph` to identify actual rendering hotspots rather than guessing.

**Warning signs:**
- UI freezes for seconds when opening the library view
- Scrolling feels "chunky" rather than smooth
- Memory usage spikes when loading large libraries
- The app feels fine with a test library of 50 tracks but unusable with a real collection

**Phase to address:**
Phase 2 (Library Browsing) -- but the architecture for pagination and caching must be designed in Phase 1. Retrofitting pagination onto a "fetch everything" design requires a rewrite.

---

### Pitfall 4: Not Reporting Playback State to Plex (Scrobbling/Timeline)

**What goes wrong:**
The player works but Plex has no idea anything is playing. "Now Playing" doesn't show on the Plex dashboard. Play counts don't update. "On Deck" and "Continue Listening" don't work. Tautulli shows no activity. The app feels like a second-class citizen -- technically functional but not integrated into the Plex ecosystem.

**Why it happens:**
Plex expects clients to actively report their playback state via timeline updates to `/:/timeline`. This includes reporting play, pause, stop, and periodic progress updates. Third-party developers focus on getting audio to play and forget that Plex's entire UX (dashboard, recommendations, history) depends on clients reporting back. Without timeline updates, it's as if the music was never played.

**How to avoid:**
- Implement timeline reporting from the start of playback development, not as an afterthought.
- Send timeline updates: on play, on pause, on stop, on seek, and periodically during playback (every 10-30 seconds).
- Include required fields: `ratingKey`, `key`, `playbackTime`, `state` (playing/paused/stopped), `duration`.
- Register the player as a controllable client so it appears in Plex's "Cast To" interface.
- Test with Tautulli running to verify your timeline reports are received correctly.

**Warning signs:**
- Plex dashboard shows "No active sessions" while music is playing
- Play counts on tracks remain at 0 after listening
- "On Deck" for music never updates
- Tautulli history is empty for your client

**Phase to address:**
Phase 3 (Playback) -- timeline reporting should be implemented alongside basic playback, not deferred to a later "polish" phase.

---

### Pitfall 5: Terminal Escape Sequence Cleanup Failure on Crash/Exit

**What goes wrong:**
When the TUI app crashes, is killed (SIGKILL/SIGTERM), or exits abnormally, the terminal is left in a broken state. Symptoms include: invisible cursor, no echo of typed characters, mouse tracking escape sequences flooding as raw text, alternate screen buffer still active (previous terminal content lost), raw mode still enabled (no line editing). Users must run `reset` or close and reopen the terminal.

**Why it happens:**
TUI apps enable raw mode, alternate screen, mouse tracking, and disable cursor visibility. These are terminal state changes communicated via escape sequences. If the app doesn't run cleanup code (disable raw mode, leave alternate screen, show cursor, disable mouse tracking), the terminal stays in the modified state. Signal handlers for SIGINT/SIGTERM may not be registered, and SIGKILL cannot be caught at all. Panic handlers in Rust don't automatically restore terminal state.

**How to avoid:**
- Register signal handlers for SIGINT, SIGTERM, and SIGHUP that restore terminal state before exiting.
- Set a custom panic hook in Rust that restores terminal state before printing the panic message.
- Use `scopeguard` or RAII patterns to ensure cleanup runs on all exit paths.
- Store the original terminal state (tcgetattr) at startup and restore it in all cleanup paths.
- Specifically disable mouse tracking modes (SGR extended mouse tracking) in cleanup -- this is the most commonly missed one.
- Test by sending SIGTERM to your app and verifying the terminal recovers cleanly.

**Warning signs:**
- Terminal behaves strangely after the app exits (especially after Ctrl+C)
- Raw escape sequences appear as garbled text after exit
- Mouse clicks produce escape sequence text instead of normal behavior
- Users report needing to run `reset` after using the app

**Phase to address:**
Phase 1 (Foundation) -- terminal state management is foundational infrastructure. Every feature built on top of it inherits the cleanup behavior (or lack thereof).

---

## Technical Debt Patterns

Shortcuts that seem reasonable but create long-term problems.

| Shortcut | Immediate Benefit | Long-term Cost | When Acceptable |
|----------|-------------------|----------------|-----------------|
| Fetch entire library on startup | Simple implementation, no pagination logic | Unusable for libraries >5k tracks, high memory, slow startup | Never -- design pagination from start |
| Hardcode Plex token in config | Skip auth flow implementation, faster dev | Tokens expire, no multi-user support, security risk | Only during initial PoC (first week), must replace before any user testing |
| Synchronous network requests on render thread | Simpler control flow, no async complexity | UI freezes on every API call, unusable on slow networks | Never -- async from day one |
| Skip terminal cleanup on panic | Faster to get something running | Every crash leaves terminal broken, terrible UX | Never -- implement in first commit |
| Single audio backend hardcoded | Less code, faster to ship | Breaks on different Linux distros, WSL versions, or when preferred backend unavailable | MVP only -- add backend abstraction by Phase 3 |
| Polling Plex API for library changes | Simple, no websocket complexity | Unnecessary API calls, delayed updates, potential rate limiting | Acceptable for v1, add webhooks later |
| Storing all track metadata in memory | Fast access, no database code | Memory bloat with large libraries (100k tracks x metadata = hundreds of MB) | Acceptable up to ~10k tracks, need local cache beyond that |

## Integration Gotchas

Common mistakes when connecting to Plex.

| Integration | Common Mistake | Correct Approach |
|-------------|----------------|------------------|
| Plex Auth | Using token copied from browser DevTools | Implement PIN-based OAuth flow; store Client Identifier persistently |
| Plex Library API | Calling `/library/sections/{id}/all` without pagination | Use `X-Plex-Container-Start` and `X-Plex-Container-Size` params; page through results |
| Plex Streaming | Forcing transcoding by not declaring client capabilities | Send proper `X-Plex-Client-Profile-Extra` headers declaring supported audio codecs (FLAC, AAC, MP3, etc.) to get direct play |
| Plex Streaming | Building stream URL manually | Use the `part.key` from metadata response with the server base URL and token; handle HTTPS certificate issues for local servers |
| Plex Timeline | Not sending playback progress | POST to `/:/timeline` with state/time every 10-30 seconds; on play/pause/stop/seek |
| Plex Rate Limits | Rapid-fire API calls during library browsing | Implement request debouncing/throttling; batch metadata requests; cache aggressively. Plex returns HTTP 429 and can lock the server database under heavy load |
| Plex Server Discovery | Hardcoding server IP/port | Use Plex.tv's `/api/v2/resources` to discover servers; handle servers behind relay (indirect connections) |
| WSLg PulseAudio | Assuming PulseAudio "just works" in WSL | Test pause/resume explicitly; implement stream recreation on failure; consider Windows-side audio as fallback |
| Tmux | Enabling mouse tracking that conflicts with tmux mouse mode | Detect `$TMUX` environment variable; adjust mouse behavior or provide tmux-aware mode; document Shift+click for passthrough |

## Performance Traps

Patterns that work at small scale but fail as usage grows.

| Trap | Symptoms | Prevention | When It Breaks |
|------|----------|------------|----------------|
| Rendering all list items to ratatui | Scrolling lag, high CPU during scroll | Virtual scrolling: only pass visible items + buffer to widget | >5k items in a single list |
| Re-fetching library metadata on every view switch | View transitions take seconds, network spike | Cache metadata locally; invalidate on library update webhook | Any library >500 albums |
| FFT/spectrum analysis on the render thread | Frame drops during audio visualization, choppy audio | Run FFT on dedicated thread, share results via atomic/channel | Always, even with small FFT windows |
| No audio buffer pre-loading (loading next track on demand) | Gaps between tracks, perceived as broken gapless playback | Pre-buffer next track when current track reaches 80-90% | Any playlist/queue playback |
| String allocations in render loop | GC pressure / allocator contention, gradual slowdown | Pre-allocate Spans/Lines, reuse buffers, use `Cow<str>` | Noticeable at 60fps with complex layouts |
| Album art conversion on every frame | CPU spike when album art is visible, jerky scrolling | Convert album art to terminal representation once, cache the result | Any view showing album art |

## Security Mistakes

Domain-specific security issues beyond general application security.

| Mistake | Risk | Prevention |
|---------|------|------------|
| Storing Plex token in plaintext config file with world-readable permissions | Any local user can access the Plex account, stream/delete media | Store token with 600 permissions; use OS keyring (libsecret on Linux) when available; never log tokens |
| Logging full API request URLs (which contain X-Plex-Token) | Token exposed in log files, crash reports | Strip token from logged URLs; use header-based auth rather than query params where possible |
| Not validating Plex server TLS certificates | MITM attacks on local network, credential theft | Verify certificates; allow user opt-in for self-signed certs with explicit warning, never silently skip |
| Exposing Plex server address/port in UI without user consent | Privacy leak if screen is shared/recorded | Mask server details by default; show friendly server name instead of IP:port |

## UX Pitfalls

Common user experience mistakes in TUI music players.

| Pitfall | User Impact | Better Approach |
|---------|-------------|-----------------|
| Vim keybindings that shadow essential music controls (e.g., `q` quits instead of queue, `p` pastes instead of pause) | Users accidentally quit during playback; muscle memory conflicts | Use vim-inspired navigation (hjkl, /, gg, G) but music-specific action keys (space=pause, enter=play, a=add to queue); make all bindings configurable |
| No visual feedback during network operations | User thinks app is frozen when fetching from Plex | Show loading spinners/progress indicators for any operation >200ms; use async with visual feedback |
| Blocking UI on search | Typing a search query freezes until results return | Debounce search input (300ms); show results incrementally; allow cancellation |
| Audio visualizer dominates screen space | Core music controls (now playing, queue, library) are squeezed or hidden | Make visualizer optional and toggleable; default to minimal/off; never let it push essential info off-screen |
| No indication of playback source/quality | User doesn't know if they're getting direct play (lossless) or transcoded (lossy) | Show codec, bitrate, and direct play/transcode status in the now-playing view |
| Inconsistent behavior inside/outside tmux | Keys work differently, mouse behaves differently, colors look wrong | Detect tmux via `$TMUX` env var; adjust `$TERM` handling; test explicitly in tmux during development; document tmux-specific configuration |
| Album art rendering breaks layout in some terminals | Garbled characters, misaligned columns, broken UI | Use Sixel/Kitty protocol with terminal capability detection; gracefully degrade to no art or ASCII art; never assume unicode/emoji width is consistent |

## "Looks Done But Isn't" Checklist

Things that appear complete but are missing critical pieces.

- [ ] **Authentication:** Often missing token refresh on 401 -- verify the app re-authenticates automatically when tokens expire
- [ ] **Playback:** Often missing gapless playback -- verify no audible gap between consecutive tracks by pre-buffering the next track
- [ ] **Playback:** Often missing timeline reporting -- verify Plex dashboard shows "Now Playing" during playback and play counts increment
- [ ] **Library browsing:** Often missing pagination -- verify performance with a library of 10k+ tracks, not just 50 test tracks
- [ ] **Search:** Often missing debouncing -- verify typing quickly doesn't fire 10 API calls; verify slow network doesn't freeze UI
- [ ] **Terminal cleanup:** Often missing signal handler cleanup -- verify `kill -TERM <pid>` leaves terminal in clean state
- [ ] **Terminal cleanup:** Often missing mouse tracking disable on exit -- verify no escape sequence garbage after exit
- [ ] **Audio:** Often missing pause/resume reliability on WSL -- verify pausing for 30+ seconds and resuming works
- [ ] **Queue:** Often missing queue persistence -- verify queue survives app restart (or explicitly document it doesn't)
- [ ] **Keybindings:** Often missing tmux compatibility -- verify all keybindings work inside tmux, not just bare terminal
- [ ] **Album art:** Often missing terminal capability detection -- verify album art degrades gracefully on terminals without Sixel/Kitty support

## Recovery Strategies

When pitfalls occur despite prevention, how to recover.

| Pitfall | Recovery Cost | Recovery Steps |
|---------|---------------|----------------|
| WSL audio completely broken | MEDIUM | Implement Windows-side audio fallback (spawn mpv/ffplay on Windows, control from TUI); or switch to HTTP streaming to a local Windows player |
| Plex token expired with no refresh logic | LOW | Add 401-intercept middleware that triggers re-auth flow; wrap all API calls in a retry-with-reauth helper |
| UI lag from large library | MEDIUM | Add virtual scrolling layer between data and widget; requires refactoring list/table rendering but not full rewrite if data layer already supports pagination |
| No timeline reporting (post-launch) | LOW | Add timeline update calls at play/pause/stop/seek points and a periodic timer; mostly additive, no architectural change needed |
| Terminal state corruption on crash | LOW | Add panic hook and signal handlers; single focused change, testable in isolation |
| Missing gapless playback | MEDIUM | Requires pre-buffering architecture; if audio plays track-by-track, need to add a decode-ahead pipeline. Easier if audio backend is abstracted. |
| Keybinding conflicts discovered post-launch | LOW | Make keybindings configurable via config file; ship sensible defaults but let users remap everything |

## Pitfall-to-Phase Mapping

How roadmap phases should address these pitfalls.

| Pitfall | Prevention Phase | Verification |
|---------|------------------|--------------|
| WSLg audio pause/resume failure | Phase 1: Foundation | PoC test: play, pause 30s, resume. Must work before building anything on top. |
| Plex auth token lifecycle | Phase 1: Foundation | End-to-end test: fresh auth via PIN flow, token persistence, token expiry simulation (manual 401), re-auth |
| Terminal state cleanup on crash/exit | Phase 1: Foundation | Test: send SIGTERM, SIGINT during playback; verify terminal state is clean |
| Large library rendering lag | Phase 2: Library Browsing | Test with 10k+ track library; measure frame time during scrolling; must maintain <16ms per frame (60fps) |
| Plex API rate limiting | Phase 2: Library Browsing | Test rapid browsing/searching; verify no 429 responses; implement request throttling |
| Missing timeline reporting | Phase 3: Playback | Verify with Tautulli or Plex dashboard that now-playing, progress, and play counts update correctly |
| Gapless playback gaps | Phase 3: Playback | Listen to albums with seamless track transitions (e.g., Pink Floyd, Radiohead); verify no audible gap |
| Audio visualizer CPU overhead | Phase 4: Polish/Visualizer | Monitor CPU usage with visualizer on vs. off; must not cause audio dropout or frame drops |
| Tmux keybinding conflicts | Phase 1: Foundation (design), Phase 4: Polish (verify) | Test all keybindings inside tmux with mouse mode on; document any unavoidable conflicts |
| Album art rendering inconsistency | Phase 4: Polish | Test in multiple terminals (kitty, alacritty, gnome-terminal, Windows Terminal via WSL); verify graceful degradation |
| Unicode/wide character layout breaks | Phase 2: Library Browsing | Test with tracks that have CJK characters, emoji, and long artist names; verify layout doesn't break |

## Sources

- [WSLg audio breaks on pause/resume - GitHub Issue #1376](https://github.com/microsoft/wslg/issues/1376) -- MEDIUM confidence (open issue with known fix pending)
- [WSLg extreme audio latency - GitHub Issue #607](https://github.com/microsoft/wslg/issues/607) -- HIGH confidence (multiple corroborating issues)
- [WSLg sound stuttering - GitHub Issue #1257](https://github.com/microsoft/wslg/issues/1257) -- HIGH confidence
- [WSLg choppy audio on Windows 10 - GitHub Issue #908](https://github.com/microsoft/wslg/issues/908) -- HIGH confidence
- [Plex authentication forum thread](https://forums.plex.tv/t/authenticating-with-plex/609370) -- HIGH confidence (official Plex developer documentation)
- [Plex token support article](https://support.plex.tv/articles/204059436-finding-an-authentication-token-x-plex-token/) -- HIGH confidence (official)
- [Plex direct play/transcode overview](https://support.plex.tv/articles/200430303-streaming-overview/) -- HIGH confidence (official)
- [Plex API rate limit exceeded - Forum](https://forums.plex.tv/t/api-rate-limit-exceeded-status-429/886080) -- MEDIUM confidence (community reports)
- [Ratatui Table performance with large datasets - Issue #1004](https://github.com/ratatui/ratatui/issues/1004) -- HIGH confidence (framework maintainer confirmed)
- [Ratatui rendering best practices - Discussion #579](https://github.com/ratatui/ratatui/discussions/579) -- HIGH confidence (framework maintainer authored)
- [Jellyfin-TUI issues (session management, large playlists)](https://github.com/dhonus/jellyfin-tui/issues) -- MEDIUM confidence (analogous project, not identical domain)
- [Termusic audio backend issues and changelog](https://github.com/tramhao/termusic) -- MEDIUM confidence (Rust TUI music player with similar architecture)
- [Tmux mouse mode FAQ](https://github.com/tmux/tmux/wiki/FAQ) -- HIGH confidence (official tmux documentation)
- [Terminal Unicode rendering issues - Windows Terminal Discussion #13724](https://github.com/microsoft/terminal/discussions/13724) -- MEDIUM confidence (terminal-specific, cross-references needed)
- [Plex large music library performance - Forum](https://forums.plex.tv/t/slow-library-performance-due-to-increasingly-growing-database/740511) -- MEDIUM confidence (community reports, multiple users)
- [Cava audio visualizer CPU usage](https://github.com/karlstav/cava) -- MEDIUM confidence (analogous application)
- [Terminal escape sequence cleanup - OpenCode Issue #6912](https://github.com/anomalyco/opencode/issues/6912) -- HIGH confidence (well-documented failure mode)
- [Raw terminal input gotchas](https://viewsourcecode.org/snaptoken/kilo/03.rawInputAndOutput.html) -- HIGH confidence (authoritative tutorial)

---
*Pitfalls research for: TUI Music Player with Plex Integration (TermTunes)*
*Researched: 2026-02-08*
