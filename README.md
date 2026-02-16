# TermTunes

A TUI music player for Plex Media Server that lives in your terminal.

## About

TermTunes keeps music playback inside your terminal workflow -- no context switching to external apps, no browser tabs, no desktop players. It is designed for Tmux-based workflows where you run an editor, task manager, and music player side by side in terminal panes.

TermTunes connects to your existing Plex Media Server, letting you browse and play your playlists with vim-style keyboard controls.

## Features

**Playback**
- Playlist browser with selection and playback
- Shuffle and repeat modes (off / all / one)
- Next and previous track navigation

**Dual-Channel Audio**
- Simultaneous main music and ambient track playback
- Independent volume controls for each channel
- Ambient track auto-loops continuously
- Mute/unmute ambient with volume memory (restores your last level)

**UI**
- Vim-style keybindings throughout
- Audio spectrum visualizer (toggleable)
- Now-playing tmux status bar integration

**Persistence**
- Session persistence -- resume where you left off across restarts
- Favorite playlists with 1-9 hotkeys for instant access
- Configuration and session stored as human-readable TOML

**Authentication**
- PIN-based Plex authentication (opens browser, no manual token copying)

## Prerequisites

- **Rust toolchain** -- install via [rustup](https://rustup.rs)
- **A running Plex Media Server** with a music library
- **Linux or WSL2**

## Installation

### System Dependencies (Linux)

TermTunes uses rodio for audio playback, which depends on CPAL, which requires ALSA development headers to compile:

```bash
sudo apt install libasound2-dev pkg-config
```

### Additional Dependencies (WSL2)

WSL2 needs the ALSA-to-PulseAudio bridge plugin so audio output is routed through WSLg:

```bash
sudo apt install libasound2-plugins
```

This requires WSLg (Windows 11), which provides a PulseAudio server at `/mnt/wslg/PulseServer`.

### Build and Run

```bash
git clone <repo-url>
cd termtunes
cargo build --release
./target/release/termtunes
```

## First Run

On first launch, TermTunes walks you through Plex authentication:

1. TermTunes generates a PIN and displays a URL in the terminal
2. Open the URL in a browser and sign in to your Plex account
3. TermTunes detects the authorization and discovers your server automatically
4. Credentials are saved to the config file -- subsequent launches connect without any prompts

## Configuration

TermTunes stores files in standard XDG locations:

| File | Path | Purpose |
|------|------|---------|
| Config | `~/.config/termtunes/config.toml` | Server credentials, favorite playlists |
| Session | `~/.local/share/termtunes/session.toml` | Playback state (track, volume, shuffle, repeat) |
| Logs | `~/.local/share/termtunes/termtunes.log` | Application log output |
| Now Playing | `~/.local/share/termtunes/now_playing` | Current track info for tmux integration |

The config file is set to `0600` permissions since it contains Plex auth tokens.

## Tmux Integration

TermTunes writes the current track info to `~/.local/share/termtunes/now_playing`. You can display this in your tmux status bar by adding it to your tmux config:

```bash
set -g status-right '#(cat ~/.local/share/termtunes/now_playing)'
```

## Keybindings

| Key | Action |
|-----|--------|
| `Space` | Play / Pause |
| `j` / `k` | Navigate down / up |
| `h` / `l` | Navigate left / right |
| `Enter` | Select |
| `n` / `N` | Next / Previous track |
| `+` / `-` | Volume up / down (main channel) |
| `[` / `]` | Volume up / down (ambient channel) |
| `m` | Mute / unmute ambient |
| `s` | Toggle shuffle |
| `r` | Toggle repeat |
| `v` | Toggle visualizer |
| `b` | Open ambient track browser |
| `f` | Toggle favorite |
| `1`-`9` | Quick-play favorite playlist |
| `q` | Quit |
| `Ctrl+c` | Emergency quit (works even in browser overlay) |

## Logging

Set the `RUST_LOG` environment variable for debug output:

```bash
RUST_LOG=debug ./target/release/termtunes
```

Defaults to `info` level when `RUST_LOG` is not set.

## Tech Stack

- **Rust 2021** -- systems language
- **ratatui** -- TUI framework
- **rodio / CPAL** -- audio playback (dual-sink architecture)
- **crossterm** -- terminal input and rendering
- **reqwest** -- HTTP client for Plex API
- **spectrum-analyzer** -- FFT-based audio visualizer
