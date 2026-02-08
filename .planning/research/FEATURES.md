# Feature Research

**Domain:** TUI music player for Plex Media Server
**Researched:** 2026-02-08
**Confidence:** MEDIUM-HIGH

## Feature Landscape

### Table Stakes (Users Expect These)

Features users assume exist. Missing these = product feels incomplete.

| Feature | Why Expected | Complexity | Notes |
|---------|--------------|------------|-------|
| Play/pause/stop | Every music player has this. Non-negotiable. | LOW | Single keybinding each. Must feel instant. |
| Skip forward/back | Basic track navigation. All competitors have it. | LOW | Both next/previous track jumps. |
| Playlist listing and selection | The entire product concept is playlist-based Plex playback. | MEDIUM | Requires Plex API integration for playlist fetch. Core navigation screen. |
| Current track info display | Users need to know what is playing (artist, album, track name). Every TUI player shows this. | LOW | Plex API provides metadata. Display in a status region. |
| Playback progress bar with time | cmus, ncmpcpp, spotify-tui, jellyfin-tui, kew all show elapsed/remaining time and a progress bar. Users expect visual time feedback. | LOW | Render a bar with elapsed/total. Update on a timer. |
| Volume control | Every player has volume up/down. Keyboard-driven players use +/- keys universally. | LOW | Map to +/- keys. Control system or stream volume. |
| Shuffle mode | Listed in PROJECT.md requirements. All playlist-based players support shuffle. | LOW | Randomize playlist order on toggle. |
| Repeat/loop mode | All competitors offer at least repeat-all. Most offer repeat-one as well. | LOW | Cycle through: off, repeat-all, repeat-one. |
| Seek within track | Users expect left/right arrow seeking. cmus, ncmpcpp, spotify-tui, kew, jellyfin-tui all support it. | LOW | +/-5s or +/-10s increments. Arrow keys or h/l in vim mode. |
| Vim-style keybindings | Stated in PROJECT.md. j/k navigation, slash-search, etc. Core to the target user profile. | MEDIUM | Must feel native to vim users. j/k/g/G/ctrl-d/ctrl-u for navigation. Space for play/pause. |
| Keyboard-only operation | TUI players are keyboard-driven by definition. Mouse is optional at best. | LOW | Design all interactions around keyboard input from day one. |
| Responsive terminal resize | TUI apps must handle terminal resize gracefully. Tmux panes get resized constantly. | MEDIUM | Re-render layout on SIGWINCH. Test in small pane sizes (e.g., 40x15). |

### Differentiators (Competitive Advantage)

Features that set the product apart. Not required, but valuable.

| Feature | Value Proposition | Complexity | Notes |
|---------|-------------------|------------|-------|
| Favorite playlists with number keybindings (1-9) | Instant playlist switching without navigating. No competitor does this. Matches the "background music while coding" use case -- press 2 for ambient, 3 for jazz, done. | LOW | Map 1-9 to stored playlist IDs. Config file for mapping. This is TermTunes' killer feature. |
| Toggleable audio visualizer | Terminal aesthetic that makes the player feel alive. kew, ncmpcpp, musikcube, plex-audio-btop-tui all have visualizers. Toggleable means it is not distracting during work. | HIGH | Requires audio stream analysis (FFT). Consider using CAVA protocol or similar. Toggle with a single key (v). |
| Tmux status bar integration | Show "now playing" in tmux status line so user sees track info even when focused on another pane. No Plex TUI does this. Directly serves the "no context switching" core value. | LOW | Write current track info to a file or expose via tmux display. A tmux plugin or simple script reads it. |
| Plex-native integration | Unlike generic players, TermTunes speaks Plex natively. No MPD middleman, no Spotify account. Direct Plex API for playlists and streaming. Fills a gap -- very few Plex TUI music players exist (plex-audio-btop-tui is the only real one, and it is macOS-focused). | MEDIUM | Plex API for auth, library, playlists, streaming. Well-documented via python-plexapi and plexapi.dev. |
| Minimal resource footprint | Runs in a tmux pane alongside nvim, compilers, etc. Must not hog CPU/memory. Competitors like cmus pride themselves on being lightweight. | LOW | Choose efficient runtime. Avoid Electron-level overhead. |
| Compact layout for small panes | Designed to work well in a narrow tmux pane (e.g., 30-40 columns). Most TUI players assume full terminal width. | MEDIUM | Responsive layout that degrades gracefully. Hide non-essential elements in small panes. |
| Session persistence / resume | Remember last playing playlist and position across restarts. When user opens a new tmux session, music picks up where it left off. | MEDIUM | Persist state to a local file. Load on startup. |
| MPRIS integration | Allows controlling TermTunes via media keys, playerctl, and other standard Linux media controls. jellyfin-tui, termusic, kew all support MPRIS. | MEDIUM | Implement org.mpris.MediaPlayer2 D-Bus interface. Standard on Linux. Not available on WSL without extra setup -- note as limitation. |
| Gapless playback | Smooth transitions between tracks in a playlist. Plexamp's "Sweet Fades" are beloved. cmus, musikcube, kew all support gapless. Matters for ambient/mix playlists. | HIGH | Requires audio backend that supports pre-buffering next track. Depends on audio library choice. |
| Synced lyrics display | jellyfin-tui, plex-audio-btop-tui, and kew support lyrics. Plex stores lyrics metadata. Nice for the occasional focused listen. | MEDIUM | Fetch lyrics from Plex metadata or LRCLIB API. Display in a toggleable panel. Lower priority -- user primarily listens to background music. |

### Anti-Features (Commonly Requested, Often Problematic)

Features that seem good but create problems. Explicitly do NOT build these.

| Feature | Why Requested | Why Problematic | Alternative |
|---------|---------------|-----------------|-------------|
| Full library browsing (artist/album/track tree) | Users of cmus/ncmpcpp expect library browsing. | Massively increases scope. TermTunes is playlist-focused, not a library browser. Plex has 98k+ track libraries -- browsing that in a TUI is a separate product. The core value is "pick a playlist and work." | Plex web UI or Plexamp for library browsing. TermTunes shows playlists only. |
| Manual queue building | spotify-tui and jellyfin-tui have queue management. | Adds significant UI complexity (queue view, reorder, add/remove). Contradicts the "simple playlist playback" design. Users who want queues want a different product. | Play the playlist as-is. Shuffle is the only queue manipulation needed. |
| Smart recommendations / radio / auto-mix | Plexamp has Sonic Adventure, radio stations, AI-generated playlists. | Requires complex Plex API features or ML. Far outside scope. Users already curate playlists in Plex. | Rely on Plex's existing playlist curation. Users create playlists in Plexamp/web UI. |
| Crossfade / audio effects / EQ | Plexamp has 7-band EQ and Sweet Fades. Audiophile users request this. | Significant audio processing complexity. Adds latency. Most terminal users just want playback, not audio engineering. | If users want EQ, they configure PulseAudio/PipeWire system-wide. Not TermTunes' job. |
| Mouse support | Some TUI apps support mouse clicks. | Target user explicitly does not use mouse (vim power user with vim-tmux). Mouse handling adds complexity and creates ambiguity in event handling. | Keyboard-only design. Document keybindings clearly. |
| Tag editing | termusic and ncmpcpp have tag editors. | Modifying Plex library metadata from a terminal player is dangerous. Plex has its own metadata management. | Use Plex web UI for metadata editing. |
| Downloading / offline mode | jellyfin-tui supports offline caching. | Plex already handles transcoding and streaming. Downloading duplicates Plex functionality and creates storage management headaches. | Stream from Plex. If network is down, music is unavailable -- this is acceptable for the use case. |
| Multi-server / multi-source | Support multiple Plex servers, or also play local files. | Adds configuration complexity and UI branching. One server is the expected use case. | Configure one Plex server. If user switches servers, update config. |
| Discord Rich Presence | jellyfin-tui supports it. | Niche feature. Adds a dependency. Target user is working, not socializing. | Omit entirely. |
| Last.fm scrobbling | jellyfin-tui and many GUI players support it. | Adds external API dependency and auth flow. Nice-to-have but not core. | Consider as a v2+ feature if requested. Not MVP. |

## Feature Dependencies

```
[Plex API Connection]
    +-- requires --> [Authentication/Token]
    +-- enables --> [Playlist Listing]
                       +-- enables --> [Playlist Selection & Playback]
                                          +-- enables --> [Track Info Display]
                                          +-- enables --> [Progress Bar / Time]
                                          +-- enables --> [Skip Forward/Back]
                                          +-- enables --> [Seek Within Track]
                                          +-- enables --> [Shuffle Mode]
                                          +-- enables --> [Repeat Mode]
                       +-- enables --> [Favorite Playlist Keybindings]

[Audio Backend]
    +-- requires --> [Audio Output Setup (ALSA/PulseAudio/PipeWire)]
    +-- enables --> [Volume Control]
    +-- enables --> [Playback Controls]
    +-- enables --> [Audio Visualizer] (requires audio stream/FFT data)
    +-- enables --> [Gapless Playback] (requires pre-buffering support)

[TUI Framework]
    +-- enables --> [Vim Keybindings]
    +-- enables --> [Track Info Display]
    +-- enables --> [Progress Bar]
    +-- enables --> [Responsive Resize]
    +-- enables --> [Visualizer Rendering]
    +-- enables --> [Lyrics Panel]

[Playback State]
    +-- enables --> [Tmux Status Bar Integration]
    +-- enables --> [MPRIS Integration]
    +-- enables --> [Session Persistence]
```

### Dependency Notes

- **Playlist Playback requires Plex API Connection:** Cannot play anything without authenticating and fetching playlist data from Plex.
- **Audio Visualizer requires Audio Backend:** FFT analysis depends on having access to the audio stream or system audio data.
- **Favorite Playlist Keybindings require Playlist Listing:** Must know available playlists before mapping them to keys.
- **Gapless Playback requires Audio Backend with pre-buffering:** Not all audio libraries support this natively. Audio backend choice constrains this feature.
- **MPRIS requires D-Bus:** Works on native Linux. May not work on WSL without extra configuration -- document as known limitation.
- **Tmux Status Bar integration requires Playback State:** Needs a way to export "now playing" info for tmux to consume.

## MVP Definition

### Launch With (v1)

Minimum viable product -- what is needed to validate the concept of "playlist music in a tmux pane."

- [ ] Plex authentication (token-based) -- gate to everything else
- [ ] Playlist listing from Plex server -- the main navigation screen
- [ ] Select and play a playlist -- core loop
- [ ] Play/pause/stop -- basic playback control
- [ ] Skip forward/back -- track navigation
- [ ] Shuffle mode -- essential for background listening
- [ ] Current track info (artist, album, track) -- know what is playing
- [ ] Playback progress bar with time -- visual feedback
- [ ] Volume control (+/-) -- basic necessity
- [ ] Vim keybindings (j/k/enter/space/q) -- core UX commitment
- [ ] Favorite playlist keybindings (1-9) -- the killer differentiator, include from day one

### Add After Validation (v1.x)

Features to add once core playback is solid and daily-driveable.

- [ ] Seek within track (arrow keys or h/l) -- add when playback is stable
- [ ] Repeat mode (off/all/one) -- straightforward addition
- [ ] Toggleable audio visualizer -- the "wow factor" feature, add when audio backend is proven
- [ ] Tmux status bar integration -- export now-playing for tmux status line
- [ ] Responsive layout for small panes -- polish for real tmux usage
- [ ] Session persistence (remember last playlist/position) -- quality of life

### Future Consideration (v2+)

Features to defer until product-market fit is established and core is rock-solid.

- [ ] MPRIS integration -- nice for media key control, but not blocking
- [ ] Gapless playback -- requires audio backend maturity
- [ ] Synced lyrics display -- nice-to-have for focused listening sessions
- [ ] Last.fm scrobbling -- only if users request it
- [ ] Search/filter within playlists -- useful for large playlists

## Feature Prioritization Matrix

| Feature | User Value | Implementation Cost | Priority |
|---------|------------|---------------------|----------|
| Plex auth + playlist listing | HIGH | MEDIUM | P1 |
| Play/pause/stop/skip | HIGH | LOW | P1 |
| Current track info display | HIGH | LOW | P1 |
| Progress bar with time | HIGH | LOW | P1 |
| Volume control | HIGH | LOW | P1 |
| Shuffle mode | HIGH | LOW | P1 |
| Vim keybindings | HIGH | MEDIUM | P1 |
| Favorite playlist keybindings (1-9) | HIGH | LOW | P1 |
| Seek within track | MEDIUM | LOW | P2 |
| Repeat mode | MEDIUM | LOW | P2 |
| Audio visualizer (toggleable) | MEDIUM | HIGH | P2 |
| Tmux status bar integration | MEDIUM | LOW | P2 |
| Responsive resize / small panes | MEDIUM | MEDIUM | P2 |
| Session persistence | MEDIUM | LOW | P2 |
| MPRIS integration | LOW | MEDIUM | P3 |
| Gapless playback | LOW | HIGH | P3 |
| Synced lyrics | LOW | MEDIUM | P3 |
| Last.fm scrobbling | LOW | MEDIUM | P3 |
| Playlist search/filter | LOW | LOW | P3 |

**Priority key:**
- P1: Must have for launch
- P2: Should have, add when possible
- P3: Nice to have, future consideration

## Competitor Feature Analysis

| Feature | jellyfin-tui | spotify-tui | kew | cmus | ncmpcpp | musikcube | plex-audio-btop-tui | **TermTunes Plan** |
|---------|-------------|-------------|-----|------|---------|-----------|--------------------|--------------------|
| Playback controls | Yes | Yes | Yes | Yes | Yes | Yes | Yes | **P1** |
| Library browsing | Full | Full | Yes | Yes | Yes | Yes | Yes | **No -- playlist only** |
| Queue management | Double queue | Yes | Yes | Playlist | Yes | Yes | No | **No -- by design** |
| Vim keybindings | Yes | Partial | Yes | Vi-native | Configurable | No | No | **P1 -- native** |
| Visualizer | No | Audio analysis | Yes | No | Spectrum/wave | Yes | Waveform | **P2** |
| Album art | Sixel | No | Sixel/ASCII | No | No (with timg hack) | No | Pixelated | **No -- too complex for small panes** |
| Lyrics | Synced | No | LRC files | No | Fetched | No | Synced | **P3** |
| MPRIS | Yes | N/A | Yes | Partial | Via MPD | No | No | **P3** |
| Gapless playback | Unknown | N/A (Spotify handles) | Yes | Yes | Via MPD | Yes | Unknown | **P3** |
| Shuffle/repeat | Yes | Yes | Yes | Yes | Yes | Yes | Unknown | **P1** |
| Seek | Yes | Yes | Yes | Yes | Yes | Unknown | Yes | **P2** |
| Offline/download | Yes | No | N/A | N/A | N/A | N/A | No | **No** |
| Scrobbling | Last.fm | N/A | No | Via plugin | No | No | No | **No (v2+ maybe)** |
| Favorite quick-keys | No | No | Favorites list | No | No | No | No | **P1 -- unique** |
| Tmux integration | No | No | No | No | No | No | No | **P2 -- unique** |
| Small pane support | No | No | No | No | No | No | No | **P2 -- unique** |

## Sources

- [termusic - Rust TUI music player](https://github.com/tramhao/termusic) -- album art protocols, backend options
- [jellyfin-tui - Jellyfin terminal client](https://github.com/dhonus/jellyfin-tui) -- most feature-rich media server TUI, key reference for server-backed TUI player patterns
- [spotify-tui - Spotify terminal client](https://github.com/Rigellute/spotify-tui) -- popular Rust TUI player, keybinding conventions, audio analysis
- [spotify-player - Spotify with full feature parity](https://github.com/aome510/spotify-player) -- streaming architecture, fuzzy search, image rendering
- [kew - Terminal music player](https://github.com/ravachol/kew) -- sixel art, visualizer, LRC lyrics, favorites, clean keybinding model
- [plex-audio-btop-tui - Plex audio TUI](https://github.com/MacsInSpace/plex-audio-btop-tui) -- direct competitor, Plex API patterns, waveform viz, lyrics
- [Plex-TUI](https://github.com/keegan/Plex-TUI) -- another Plex TUI attempt
- [cmus - C* Music Player](https://cmus.github.io/) -- vi-style keybindings, lightweight, gapless playback
- [ncmpcpp - NCurses MPD client](https://rybczak.net/ncmpcpp/) -- visualizer, lyrics, vim keybindings via config
- [musikcube - cross-platform terminal player](https://musikcube.com/) -- gapless, crossfade, streaming server, large library support
- [CAVA - Cross-platform Audio Visualizer](https://github.com/karlstav/cava) -- reference for terminal audio visualization
- [Plexamp features](https://www.plex.tv/plexamp/) -- gapless, EQ, Sweet Fades, what audiophiles expect from Plex music
- [tmux-now-playing plugin](https://github.com/spywhere/tmux-now-playing) -- pattern for tmux status bar music integration
- [MPRIS D-Bus Specification](https://specifications.freedesktop.org/mpris/latest/) -- standard for Linux media player integration
- [Plex API Documentation](https://developer.plex.tv/pms/) -- playlist and audio streaming endpoints
- [LinuxLinks - 16 Best Terminal Music Players](https://www.linuxlinks.com/best-free-open-source-terminal-based-music-players/) -- ecosystem overview
- [Slant - 9 Best CLI Music Players](https://www.slant.co/topics/2429/~best-command-line-music-players) -- community rankings

---
*Feature research for: TUI music player (Plex-backed, tmux-integrated)*
*Researched: 2026-02-08*
