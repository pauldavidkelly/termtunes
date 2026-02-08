# Architecture Research

**Domain:** TUI Music Player with Plex Media Server Integration
**Researched:** 2026-02-08
**Confidence:** HIGH

## Standard Architecture

TUI music players universally follow an **event-driven, message-passing architecture** with clear separation between UI rendering, audio playback, API communication, and state management. After studying rmpc (MPD client), jellyfin-tui (Jellyfin client), ytermusic (YouTube Music client), and youtui (YouTube Music client), a consistent pattern emerges: a central event loop coordinates loosely-coupled subsystems that communicate through typed message channels.

For TermTunes specifically, the recommended architecture is **Go with Bubble Tea (Elm Architecture)** because: (1) the plexgo SDK provides a mature, actively maintained Go client for the Plex API, (2) Bubble Tea's Model-Update-View pattern naturally maps to a music player's state transitions, and (3) Go's goroutines elegantly handle the concurrent operations (API calls, audio streaming, UI rendering) that a streaming music player demands.

### System Overview

```
+---------------------------------------------------------------------+
|                        Presentation Layer                            |
|  +-------------+  +-----------+  +----------+  +-----------+        |
|  | Library View|  |Queue View |  |Player Bar|  |Search View|        |
|  +------+------+  +-----+-----+  +----+-----+  +-----+-----+       |
|         |               |              |              |              |
+---------+---------------+--------------+--------------+--------------+
|                        Application Core                              |
|  +------------------------------------------------------------------+|
|  |                    Bubble Tea Event Loop                          ||
|  |  Model (state) <-- Update (messages) --> View (render)           ||
|  +------------------------------------------------------------------+|
|  +----------------+  +-----------------+  +----------------+         |
|  | Input Handler  |  | Command Router  |  | State Manager  |         |
|  | (vim bindings) |  | (tea.Cmd funcs) |  | (app model)    |         |
|  +----------------+  +-----------------+  +----------------+         |
+---------+---------------+--------------+--------------+--------------+
|                        Service Layer                                 |
|  +------------------+  +------------------+  +------------------+    |
|  | Plex Client      |  | Audio Engine     |  | Cache Manager    |    |
|  | (plexgo SDK)     |  | (mpv via go-mpv) |  | (local state)    |    |
|  +------------------+  +------------------+  +------------------+    |
+---------------------------------------------------------------------+
|                        External Systems                              |
|  +------------------+  +------------------+                          |
|  | Plex Media       |  | OS Audio         |                          |
|  | Server (HTTP)    |  | Subsystem        |                          |
|  +------------------+  +------------------+                          |
+---------------------------------------------------------------------+
```

### Component Responsibilities

| Component | Responsibility | Typical Implementation |
|-----------|----------------|------------------------|
| Bubble Tea Event Loop | Central coordinator: receives all messages, dispatches to Update, triggers View renders | Root `tea.Model` with hierarchical sub-models for each view |
| Input Handler | Captures keyboard events, translates vim-style bindings to application commands | Keymap configuration struct, context-sensitive binding resolution |
| Library View | Browse Plex music libraries: artists, albums, tracks | Sub-model implementing `tea.Model`, list/table Bubble components |
| Queue View | Display and manage current playback queue | Sub-model with reorderable list, drag-select support |
| Player Bar | Persistent playback controls: progress, now-playing, volume | Always-visible footer widget, updated by playback state messages |
| Search View | Search Plex library with real-time results | Sub-model with text input, debounced API queries via `tea.Cmd` |
| Plex Client | All Plex API communication: auth, library browsing, playlist CRUD, stream URL resolution | Wrapper around plexgo SDK, returns typed domain models |
| Audio Engine | Audio playback: play, pause, seek, volume, queue advancement | mpv via go-mpv bindings (handles HTTP streaming, format decoding, gapless playback) |
| Cache Manager | Local caching of library metadata, album art, session state | In-memory LRU cache with optional SQLite persistence |
| State Manager | Centralized application state: current track, queue, playback status, navigation context | Single Go struct embedded in root Model, modified only in Update |

## Recommended Project Structure

```
src/
├── cmd/
│   └── termtunes/
│       └── main.go              # Entry point, Bubble Tea program init
├── internal/
│   ├── app/
│   │   ├── app.go               # Root tea.Model, message routing
│   │   ├── messages.go          # All custom message types
│   │   └── keymap.go            # Vim-style keybinding definitions
│   ├── ui/
│   │   ├── library/
│   │   │   ├── model.go         # Library browser sub-model
│   │   │   └── view.go          # Library rendering
│   │   ├── queue/
│   │   │   ├── model.go         # Queue sub-model
│   │   │   └── view.go          # Queue rendering
│   │   ├── player/
│   │   │   ├── bar.go           # Now-playing bar widget
│   │   │   └── view.go          # Player bar rendering
│   │   ├── search/
│   │   │   ├── model.go         # Search sub-model
│   │   │   └── view.go          # Search rendering
│   │   └── components/
│   │       ├── list.go          # Reusable list widget
│   │       ├── modal.go         # Modal overlay component
│   │       └── statusbar.go     # Status bar component
│   ├── plex/
│   │   ├── client.go            # Plex API wrapper (over plexgo)
│   │   ├── auth.go              # Authentication flow (token, PIN)
│   │   ├── library.go           # Library/metadata operations
│   │   ├── streaming.go         # Stream URL resolution, transcoding
│   │   └── models.go            # Domain types (Artist, Album, Track)
│   ├── player/
│   │   ├── engine.go            # Audio engine interface
│   │   ├── mpv.go               # mpv backend implementation
│   │   ├── queue.go             # Queue management logic
│   │   └── state.go             # Playback state (playing, paused, etc.)
│   ├── config/
│   │   ├── config.go            # App configuration loading
│   │   └── keymap.go            # User-customizable keybindings
│   └── cache/
│       ├── cache.go             # LRU metadata cache
│       └── store.go             # Optional persistent storage
├── go.mod
└── go.sum
```

### Structure Rationale

- **cmd/termtunes/:** Single entry point. Initializes Bubble Tea program, loads config, wires dependencies.
- **internal/app/:** The Bubble Tea root model and message definitions. This is the "glue" layer that owns the event loop and routes messages to sub-models. Keeping message types centralized here prevents circular imports.
- **internal/ui/:** Each major view is its own package with a model (state + Update) and view (rendering). This mirrors how rmpc and jellyfin-tui organize their pane/screen systems. Components shared across views live in `components/`.
- **internal/plex/:** Complete isolation of Plex API concerns. The rest of the app never imports plexgo directly -- it goes through this layer's domain types. This enables testing with mock Plex responses and protects against SDK API changes.
- **internal/player/:** Audio engine abstraction. The `engine.go` interface allows swapping mpv for beep or another backend without touching UI code. Queue logic lives here because queue management is fundamentally a playback concern (next track, shuffle, repeat).
- **internal/config/:** User configuration including custom keybindings. Loaded at startup, potentially hot-reloadable later.
- **internal/cache/:** Metadata caching to reduce API calls. Plex libraries can be large; caching artist/album/track metadata locally is essential for responsive navigation.

## Architectural Patterns

### Pattern 1: Elm Architecture (Model-Update-View)

**What:** All state lives in a single Model struct. All state changes happen in Update. All rendering happens in View. Messages are the only way to trigger changes.
**When to use:** Always -- this is the core pattern enforced by Bubble Tea.
**Trade-offs:** Extremely predictable state management and easy debugging. Can feel verbose for simple interactions. Sub-model composition requires explicit message forwarding.

**Example:**
```go
// Root model owns all sub-models and shared state
type Model struct {
    // Shared state
    playback  player.State
    queue     player.Queue
    plexToken string

    // Sub-models (each implements tea.Model)
    library   library.Model
    queueView queue.Model
    search    search.Model
    playerBar player.BarModel

    // Navigation
    activeView View  // enum: ViewLibrary, ViewQueue, ViewSearch
    width      int
    height     int
}

func (m Model) Update(msg tea.Msg) (tea.Model, tea.Cmd) {
    var cmds []tea.Cmd

    switch msg := msg.(type) {
    case tea.KeyMsg:
        // Global keybindings first (quit, view switching)
        if cmd := m.handleGlobalKey(msg); cmd != nil {
            return m, cmd
        }
        // Route to active view
        switch m.activeView {
        case ViewLibrary:
            newLib, cmd := m.library.Update(msg)
            m.library = newLib.(library.Model)
            cmds = append(cmds, cmd)
        }

    case PlayTrackMsg:
        // Cross-cutting: update queue, start playback, notify player bar
        m.queue.SetCurrent(msg.Track)
        cmds = append(cmds, m.startPlayback(msg.Track))

    case tea.WindowSizeMsg:
        // Broadcast to all sub-models
        m.width, m.height = msg.Width, msg.Height
        // ... update all sub-models
    }

    // Always update player bar (it shows on every view)
    newBar, cmd := m.playerBar.Update(msg)
    m.playerBar = newBar.(player.BarModel)
    cmds = append(cmds, cmd)

    return m, tea.Batch(cmds...)
}
```

### Pattern 2: Command-Based Async Operations

**What:** All blocking operations (API calls, audio commands) are wrapped in `tea.Cmd` functions that execute in goroutines and return messages with results.
**When to use:** Any operation that would block the event loop: Plex API calls, audio engine commands, file I/O.
**Trade-offs:** Keeps UI responsive. Adds indirection (request message -> command -> result message). Must handle loading/error states in the model.

**Example:**
```go
// Command: fetch albums from Plex (runs in goroutine)
func fetchAlbums(client *plex.Client, artistID string) tea.Cmd {
    return func() tea.Msg {
        albums, err := client.GetAlbums(artistID)
        if err != nil {
            return AlbumsFetchErrorMsg{Err: err}
        }
        return AlbumsFetchedMsg{Albums: albums}
    }
}

// In Update, dispatch the command
case ArtistSelectedMsg:
    m.loading = true
    return m, fetchAlbums(m.plexClient, msg.ArtistID)

// Handle the result
case AlbumsFetchedMsg:
    m.loading = false
    m.albums = msg.Albums
    return m, nil
```

### Pattern 3: Audio Engine as Event Source

**What:** The audio engine runs independently and pushes state changes (track ended, position updated, error occurred) as Bubble Tea messages into the event loop. The UI never polls the engine.
**When to use:** Always for playback state. The audio engine is a long-lived goroutine that produces events.
**Trade-offs:** Clean separation between audio and UI. Requires a bridge that converts engine events into `tea.Msg` types. Position updates need throttling to avoid flooding the event loop.

**Example:**
```go
// Audio engine event listener (runs as tea.Cmd)
func listenToEngine(engine *player.Engine) tea.Cmd {
    return func() tea.Msg {
        // Blocks until engine produces an event
        event := <-engine.Events()
        switch event.Type {
        case player.EventTrackEnded:
            return TrackEndedMsg{}
        case player.EventPositionUpdate:
            return PositionUpdateMsg{Position: event.Position}
        case player.EventError:
            return PlaybackErrorMsg{Err: event.Error}
        }
        return nil
    }
}

// After handling each engine message, re-subscribe
case TrackEndedMsg:
    m.playback.State = Stopped
    cmd := m.advanceQueue()  // Start next track
    return m, tea.Batch(cmd, listenToEngine(m.engine))
```

## Data Flow

### Request Flow

```
[Keyboard Input]
    |
    v
[Bubble Tea Event Loop] --> [Input Handler resolves vim binding]
    |
    v
[Update function] --> routes to active view's Update
    |
    v
[Sub-model Update] --> may return tea.Cmd for async work
    |                       |
    v                       v
[Model state updated]   [Goroutine executes]
    |                       |
    v                       v
[View re-renders]       [Result message returned to event loop]
```

### Playback Flow (Critical Path)

```
1. User selects track in Library View
    |
    v
2. PlayTrackMsg dispatched to root Update
    |
    v
3. Root Update: updates queue state, returns startPlayback command
    |
    v
4. startPlayback command (goroutine):
   a. Calls plex.Client.GetStreamURL(trackID) --> resolves Part.key
   b. Constructs full URL: http://{server}:32400/{part_key}?X-Plex-Token={token}
   c. Sends URL to AudioEngine.Play(url)
   d. Returns PlaybackStartedMsg
    |
    v
5. AudioEngine (mpv):
   a. Opens HTTP URL directly (mpv handles HTTP streaming natively)
   b. Decodes audio (mp3/flac/aac/etc -- mpv supports all Plex formats)
   c. Outputs to OS audio subsystem
   d. Emits events: position updates, track ended, errors
    |
    v
6. Engine events --> listenToEngine command --> messages to event loop
    |
    v
7. Player Bar updates progress display, handles track advancement
```

### Plex Authentication Flow

```
1. App startup: check for saved token in config
    |
    +--> Token exists: validate with Plex API
    |       |
    |       +--> Valid: proceed to library browse
    |       +--> Invalid: start auth flow
    |
    +--> No token: start auth flow
            |
            v
2. Auth flow:
   a. Request PIN from plex.tv/api/v2/pins
   b. Display PIN code and URL to user
   c. Poll plex.tv/api/v2/pins/{id} until authorized
   d. Extract auth_token from response
   e. Save token to config file
   f. Connect to Plex server with token
```

### State Management

```
Root Model (single source of truth)
    |
    +-- playback: { state, currentTrack, position, duration, volume }
    |     ^
    |     | (updated by AudioEngine events)
    |
    +-- queue: { tracks[], currentIndex, shuffle, repeat }
    |     ^
    |     | (updated by user actions + track end events)
    |
    +-- library: { sections[], currentPath, artists[], albums[], tracks[] }
    |     ^
    |     | (updated by Plex API response messages)
    |
    +-- navigation: { activeView, previousView, modalStack }
          ^
          | (updated by keyboard input)
```

### Key Data Flows

1. **Library Navigation:** Key press -> View switch or drill-down -> Plex API fetch command -> Response populates model -> View re-renders list
2. **Track Playback:** Track selection -> Queue update + stream URL resolution -> mpv plays HTTP stream -> Position events update player bar
3. **Search:** Keystroke -> Debounced search command -> Plex API search -> Results populate search model -> View renders results
4. **Queue Management:** Add/remove/reorder action -> Queue state updated -> If current track affected, audio engine notified

## Scaling Considerations

| Scale | Architecture Adjustments |
|-------|--------------------------|
| Small library (<1K tracks) | No caching needed. Fetch on demand. Simple in-memory state. |
| Medium library (1K-50K tracks) | Add LRU metadata cache. Paginate API requests. Lazy-load album art. |
| Large library (50K+ tracks) | SQLite local cache with incremental sync. Virtual scrolling in lists. Background prefetch of adjacent pages. |

### Scaling Priorities

1. **First bottleneck: API latency for large libraries.** Plex pagination defaults are generous but browsing a 50K-track library without caching means repeated network round-trips. Solution: cache library metadata locally after first fetch, with TTL-based invalidation.
2. **Second bottleneck: UI responsiveness during network operations.** Already handled by the async command pattern -- no blocking in Update/View. But search needs debouncing to avoid flooding the Plex API.

## Anti-Patterns

### Anti-Pattern 1: Blocking the Event Loop

**What people do:** Make Plex API calls or audio engine commands directly in the `Update()` function.
**Why it's wrong:** Bubble Tea's event loop is single-threaded. A 200ms API call freezes the entire UI. The player bar stops updating. Keyboard input queues up.
**Do this instead:** Always wrap blocking operations in `tea.Cmd` functions. These execute in separate goroutines and return results as messages.

### Anti-Pattern 2: Polling Playback State

**What people do:** Set up a ticker to poll the audio engine for current position every 100ms.
**Why it's wrong:** Creates unnecessary load, timing jitter in the progress bar, and couples the UI refresh rate to an arbitrary poll interval.
**Do this instead:** Have the audio engine push position updates as events. Use mpv's observe_property mechanism to get callbacks on position changes. Convert these to Bubble Tea messages.

### Anti-Pattern 3: Direct Plex SDK Usage in UI Code

**What people do:** Import plexgo directly in view models and call API methods inline.
**Why it's wrong:** Couples UI to a specific SDK version. Makes testing impossible without a real Plex server. Leaks API response types into UI layer. Any SDK breaking change ripples through the entire codebase.
**Do this instead:** Create a `plex.Client` abstraction layer that returns domain types (Artist, Album, Track). UI code only knows about domain types. Test with mock client implementations.

### Anti-Pattern 4: Monolithic Root Model

**What people do:** Put all state and all Update logic in a single enormous Model struct and Update function.
**Why it's wrong:** The Update function becomes thousands of lines. Every message type is handled in one switch statement. Adding a new view requires modifying the god function.
**Do this instead:** Decompose into sub-models. Each view owns its own `tea.Model` implementation. The root model routes messages and composes views. Sub-models communicate through the root via messages, never directly.

### Anti-Pattern 5: Storing Stream URLs Long-Term

**What people do:** Cache the full streaming URL (including auth token) when a track is added to the queue.
**Why it's wrong:** Plex tokens can rotate, and transcoding sessions expire. A cached URL from 30 minutes ago may return 401 or 404.
**Do this instead:** Store track metadata IDs in the queue. Resolve the stream URL at play time, just before handing it to the audio engine. This ensures fresh tokens and valid session URLs.

## Integration Points

### External Services

| Service | Integration Pattern | Notes |
|---------|---------------------|-------|
| Plex Media Server | HTTP REST API via plexgo SDK | All requests require `X-Plex-Token` header. Server at port 32400. Supports JSON responses (set Accept header). |
| plex.tv Auth | OAuth-style PIN flow via plex.tv API | PIN requested, user authorizes in browser, app polls for completion. Token persisted locally. |
| OS Audio (via mpv) | libmpv C bindings via go-mpv | mpv handles HTTP streaming, codec decoding, audio output natively. Supports gapless playback, seek, volume. |

### Internal Boundaries

| Boundary | Communication | Notes |
|----------|---------------|-------|
| UI <-> Plex Client | `tea.Cmd` functions returning domain types as messages | UI dispatches fetch commands, receives typed result messages. Never calls client synchronously. |
| UI <-> Audio Engine | Bidirectional via messages and event channel | Commands flow UI->Engine via method calls in `tea.Cmd`. Events flow Engine->UI via channel converted to `tea.Msg`. |
| Plex Client <-> Cache | Internal to plex package | Client checks cache before API call. Cache populated on successful responses. Transparent to UI layer. |
| Root Model <-> Sub-models | `tea.Msg` routing in root `Update()` | Root forwards relevant messages to sub-models. Sub-models return commands that produce messages routed back through root. |
| Config <-> All Components | Read at startup, injected via constructors | Config loaded once, passed as dependency. Hot-reload possible later via file watcher + config reload message. |

## Build Order (Dependency Chain)

Based on component dependencies, the recommended implementation order:

1. **Config + Plex Auth** (no dependencies) -- Must authenticate before anything else works
2. **Plex Client wrapper** (depends on: config, auth) -- Need API access to populate any view
3. **Audio Engine** (depends on: config) -- Standalone, testable with any HTTP audio URL
4. **Root App Model + Navigation** (depends on: nothing at runtime, but shapes everything) -- Establish the Bubble Tea skeleton early
5. **Player Bar** (depends on: audio engine events) -- Core UX element visible on every screen
6. **Library View** (depends on: plex client) -- Primary browsing interface
7. **Queue View** (depends on: audio engine, library) -- Requires tracks to exist in queue
8. **Search View** (depends on: plex client) -- Additive feature, not required for basic playback
9. **Cache Layer** (depends on: plex client) -- Performance optimization, add when navigation feels slow
10. **Visualizer** (depends on: audio engine) -- Optional, purely additive

**Rationale:** Auth and API client are pure prerequisites -- nothing works without them. The audio engine is independent and can be developed in parallel. The Bubble Tea skeleton (root model + navigation) should be established early because all views plug into it. Player bar comes before library because you need to see playback state while developing the library browser. Library before queue because you need to browse to populate the queue. Search and cache are enhancements that layer onto existing functionality.

## Sources

- [rmpc architecture (MPD TUI client)](https://deepwiki.com/mierak/rmpc/1-overview) -- HIGH confidence, detailed architecture documentation
- [ytermusic architecture (YouTube Music TUI)](https://deepwiki.com/ccgauche/ytermusic) -- HIGH confidence, detailed architecture documentation
- [jellyfin-tui (Jellyfin TUI client)](https://github.com/dhonus/jellyfin-tui) -- MEDIUM confidence, repository structure analysis
- [youtui (YouTube Music TUI)](https://github.com/nick42d/youtui) -- MEDIUM confidence, repository structure analysis
- [Bubble Tea framework](https://github.com/charmbracelet/bubbletea) -- HIGH confidence, official documentation
- [Bubble Tea state machine pattern](https://zackproser.com/blog/bubbletea-state-machine) -- MEDIUM confidence, community pattern
- [Building Bubble Tea programs](https://leg100.github.io/en/posts/building-bubbletea-programs/) -- MEDIUM confidence, community best practices
- [plexgo SDK](https://github.com/LukeHagar/plexgo) -- MEDIUM confidence, repository documentation
- [go-mpv bindings](https://github.com/gen2brain/go-mpv) -- MEDIUM confidence, repository documentation
- [gopxl/beep v2](https://github.com/gopxl/beep) -- MEDIUM confidence, official documentation
- [Plex Media Server API](https://developer.plex.tv/pms/) -- HIGH confidence, official Plex documentation
- [Plex streaming overview](https://support.plex.tv/articles/200250387-streaming-media-direct-play-and-direct-stream/) -- HIGH confidence, official Plex documentation
- [Plex download API](https://www.plexopedia.com/plex-media-server/api/library/download-media-file/) -- MEDIUM confidence, community documentation verified against official

---
*Architecture research for: TUI Music Player with Plex Integration (TermTunes)*
*Researched: 2026-02-08*
