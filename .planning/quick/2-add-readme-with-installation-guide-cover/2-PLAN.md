---
phase: quick-2
plan: 01
type: execute
wave: 1
depends_on: []
files_modified: [README.md]
autonomous: true
must_haves:
  truths:
    - "User can follow README to install system dependencies on Linux/WSL2"
    - "User can build and run termtunes from source"
    - "User understands what termtunes is and what features it offers"
    - "User knows the keybindings for controlling playback"
  artifacts:
    - path: "README.md"
      provides: "Complete project README with installation guide"
      contains: "libasound2-dev"
  key_links: []
---

<objective>
Create a comprehensive README.md for TermTunes covering project description, features, installation guide (with platform-specific audio package requirements), configuration, usage, and keybindings.

Purpose: Users (and the developer) need a single reference document for building, installing, and using TermTunes, especially the non-obvious audio dependency requirements on Linux and WSL2.
Output: README.md at project root
</objective>

<execution_context>
@/home/jigsaw/.claude/get-shit-done/workflows/execute-plan.md
@/home/jigsaw/.claude/get-shit-done/templates/summary.md
</execution_context>

<context>
@.planning/PROJECT.md
@Cargo.toml
@src/config.rs
</context>

<tasks>

<task type="auto">
  <name>Task 1: Create README.md with full installation guide and feature documentation</name>
  <files>README.md</files>
  <action>
Create README.md at the project root with these sections in order:

**Header:** "TermTunes" title with one-line description: A TUI music player for Plex Media Server that lives in your terminal.

**About:** 2-3 sentences explaining core value -- keep music playback inside the terminal workflow, no context switching. Designed for Tmux-based workflows alongside editors and task managers. Connects to existing Plex Media Server.

**Features:** Bullet list organized in groups:
- Playback: playlist browser, shuffle, repeat, next/prev track
- Dual-Channel Audio: simultaneous main music + ambient track, independent volume controls, ambient auto-loops, mute/unmute with volume memory
- UI: vim-style keybindings, audio spectrum visualizer (toggleable), now-playing tmux status bar integration
- Persistence: session persistence (resume where you left off), favorite playlists with 1-9 hotkeys, config and session stored as TOML
- Auth: PIN-based Plex authentication (opens browser, no manual token copying)

**Prerequisites section:**
- Rust toolchain (link to https://rustup.rs)
- A running Plex Media Server with music library
- Linux or WSL2

**Installation section with three subsections:**

1. "System Dependencies (Linux)" -- explain rodio uses CPAL which requires ALSA development headers:
   ```
   sudo apt install libasound2-dev pkg-config
   ```

2. "Additional Dependencies (WSL2)" -- explain WSL2 needs ALSA-to-PulseAudio bridge for audio output through WSLg:
   ```
   sudo apt install libasound2-plugins
   ```
   Note: Requires WSLg (Windows 11) which provides PulseAudio at /mnt/wslg/PulseServer.

3. "Build & Run":
   ```
   git clone <repo-url>
   cd termtunes
   cargo build --release
   ./target/release/termtunes
   ```

**First Run section:** Explain the PIN-based auth flow:
1. On first launch, termtunes generates a PIN and displays a URL
2. Open the URL in a browser and sign in to your Plex account
3. termtunes detects the authorization and discovers your server
4. Credentials are saved to config -- subsequent launches connect automatically

**Configuration section:** Document file locations:
- Config: `~/.config/termtunes/config.toml` (server credentials, favorites)
- Session: `~/.local/share/termtunes/session.toml` (playback state)
- Logs: `~/.local/share/termtunes/termtunes.log`
- Now Playing: `~/.local/share/termtunes/now_playing` (for tmux integration)

**Tmux Integration section:** Brief note that termtunes writes now-playing info to `~/.local/share/termtunes/now_playing`. Users can read this file in their tmux status-right config, e.g.:
```
set -g status-right '#(cat ~/.local/share/termtunes/now_playing)'
```

**Keybindings section:** Table format with two columns (Key, Action):
- Space: Play/Pause
- j/k: Navigate up/down
- h/l: Navigate left/right
- Enter: Select
- n/N: Next/Previous track
- +/-: Volume up/down (main channel)
- [/]: Volume up/down (ambient channel)
- m: Mute/unmute ambient
- s: Toggle shuffle
- r: Toggle repeat
- v: Toggle visualizer
- b: Open ambient track browser
- f: Toggle favorite
- 1-9: Quick-play favorite playlist
- q: Quit
- Ctrl+c: Emergency quit (works even in browser overlay)

**Logging section:** One line: set RUST_LOG env var for debug output, e.g. `RUST_LOG=debug ./target/release/termtunes`. Defaults to info level.

**Tech Stack section:** Brief list: Rust 2021, ratatui (TUI framework), rodio/CPAL (audio), crossterm (terminal), reqwest (HTTP/Plex API), spectrum-analyzer (FFT visualizer).

Style notes:
- Use clean, minimal markdown. No badges, no unnecessary decoration.
- Code blocks with proper language hints (bash, toml).
- Keep it practical and scannable -- a developer should find what they need in seconds.
- Do NOT use emojis anywhere.
  </action>
  <verify>
    Verify the file exists and has all required sections:
    - `test -f README.md` succeeds
    - File contains "libasound2-dev" (Linux audio deps)
    - File contains "libasound2-plugins" (WSL2 audio deps)
    - File contains "cargo build" (build instructions)
    - File contains keybinding table
    - File contains config file paths
  </verify>
  <done>README.md exists at project root with complete installation guide covering Linux and WSL2 audio dependencies, build instructions, first-run auth flow, configuration paths, tmux integration, full keybindings reference, and tech stack summary.</done>
</task>

</tasks>

<verification>
- README.md exists at project root
- All platform-specific audio package instructions are present (libasound2-dev for Linux, libasound2-plugins for WSL2)
- Build instructions are complete and accurate (cargo build --release)
- Keybindings match the actual implementation
- File paths match actual config/session/log locations
</verification>

<success_criteria>
A new user on Linux or WSL2 can follow the README from top to bottom to install dependencies, build termtunes, authenticate with Plex, and understand all keybindings -- without needing to read source code or ask questions.
</success_criteria>

<output>
After completion, create `.planning/quick/2-add-readme-with-installation-guide-cover/2-SUMMARY.md`
</output>
