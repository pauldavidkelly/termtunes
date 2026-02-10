# Domain Pitfalls: Multi-Channel Audio & Track Browsing

**Domain:** Adding concurrent ambient audio layer to existing TUI music player
**Researched:** 2026-02-10
**Confidence:** MEDIUM-HIGH
**Context:** TermTunes -- working Rust/rodio 0.21/ratatui TUI player on WSL2. Adding second audio channel (ambient tracks that loop beneath main music) with independent volume control and track browsing UI.

---

## Critical Pitfalls

Mistakes that cause rewrites, audio corruption, or broken existing functionality.

---

### Pitfall 1: Mixer Clipping When Two Sinks Sum to > 1.0

**What goes wrong:**
Rodio's internal mixer sums f32 samples from all connected Sinks before sending to the OS audio backend. When main music is at volume 0.8 and ambient is at volume 0.6, their combined peak samples can reach 1.4, exceeding the f32 range of -1.0 to 1.0. The audio backend clips these peaks, producing harsh crackling/distortion at every beat or loud ambient swell. Users hear it as "random crackling" and blame WSL2 audio -- but it is actually summing overflow in the mixer.

**Why it happens:**
The existing player caps volume at 1.0 per-sink (`volume.clamp(0.0, 1.0)`), which is correct for single-channel playback. But rodio's mixer does naive additive mixing -- it does not normalize or limit the combined output. Two sources at full volume will produce samples up to 2.0, which clip when converted to the output format. This is not a bug in rodio; it is standard audio mixer behavior. The problem is that developers coming from single-source playback never encounter it and do not design for it.

**Consequences:**
- Harsh crackling/distortion whenever both channels have simultaneous peaks
- Users blame WSL2 audio quality (since the app already had WSL2 audio issues)
- Volume controls "feel broken" -- turning up ambient makes main music distort
- The issue is intermittent (only at peaks), making it hard to reproduce and debug

**Prevention:**
- Set a combined volume budget. If main is at volume M and ambient is at volume A, ensure M + A <= 1.0 at all times. Use a master gain factor: `effective_main = M * master`, `effective_ambient = A * master`, where `master = 1.0 / max(M + A, 1.0)`.
- Simpler approach: cap each channel's effective volume so their sum never exceeds 1.0. For example, main max 0.7, ambient max 0.3. Or use a fixed ratio like 70/30.
- Apply the volume budget at the Sink level (`sink.set_volume()`), not in a custom Source wrapper. Sink volume is applied before mixing, so if both sinks are at 0.5, the mixer sum maxes at 1.0.
- Test with both channels at maximum volume playing loud source material. If no distortion, the budget is working.

**Detection:**
- Listen test: play loud music + loud ambient simultaneously, listen for crackling distinct from WSL2 baseline
- Log-based: if you can tap the mixed output, check for samples > 0.95 (near clipping)
- User reports of "crackling that was not there before the update"

**Phase to address:** Phase 1 (ambient audio foundation) -- the volume budget must be part of the initial two-sink architecture. Retrofitting it later requires changing every volume control path.

**Confidence:** HIGH -- this is basic digital audio mixing math, confirmed by rodio's documented behavior ("all sounds are mixed together by rodio") and the known clipping issue (GitHub issue #340).

---

### Pitfall 2: OutputStream Lifetime -- Dropping It Kills ALL Audio

**What goes wrong:**
The existing code stores a single `OutputStream` (as `_stream`) and creates new Sinks from it via `Sink::connect_new(self._stream.mixer())`. When adding a second sink for ambient audio, a developer might accidentally create a second `OutputStream`, or restructure the Player struct in a way that the original `OutputStream` gets dropped during a refactor. Dropping the `OutputStream` immediately and silently kills ALL audio on ALL sinks connected to it -- no error, no warning, just silence.

**Why it happens:**
Rodio's `OutputStream` owns the connection to the OS audio device. All Sinks created from its mixer share this connection. The API gives no error when appending to a Sink whose OutputStream has been dropped -- the audio just never reaches the speakers. The existing code already handles this correctly with `_stream` (the underscore-prefix convention for "kept alive"), but refactoring for two-channel support may break this invariant. This is rodio's single most common gotcha (GitHub issues #330, #555).

**Consequences:**
- Complete silence with no error messages
- Extremely difficult to debug -- everything appears to work, logs show playback started, but no sound comes out
- Intermittent if the OutputStream is conditionally recreated (e.g., ambient channel creates its own stream)

**Prevention:**
- Use ONE `OutputStream` for the entire application. Both the main sink and ambient sink MUST be created from the same `OutputStream::mixer()`. Never create a second `OutputStream`.
- Keep the existing `_stream` field. Add the ambient sink as a new field alongside the existing sink, both connected to the same `_stream.mixer()`.
- Add a comment at the `OutputStream` field: `// CRITICAL: dropping this kills ALL audio. Both main_sink and ambient_sink depend on it.`
- In tests, verify that creating a second Sink from the same mixer does not require or create a new OutputStream.

**Detection:**
- Complete silence after a code change that restructured the Player struct
- `_stream` variable name changed or moved to a different scope
- Audio works on first track but fails after a player recreate

**Phase to address:** Phase 1 (ambient audio foundation) -- this is the first thing to get right when modifying the Player struct.

**Confidence:** HIGH -- confirmed by rodio documentation, multiple GitHub issues, and the existing codebase already handles this (proving it is a real concern).

---

### Pitfall 3: `repeat_infinite()` Memory Leak for Ambient Loops

**What goes wrong:**
The natural approach for looping ambient audio is `source.repeat_infinite()`, which rodio provides for exactly this purpose. However, `repeat_infinite()` has a documented memory leak (rodio GitHub issue #673): memory grows ~10MB every 15 seconds due to a bug in the `Buffered` type's clone implementation. For an ambient track that loops for hours, this will consume gigabytes of RAM and eventually crash the application or trigger OOM kills.

**Why it happens:**
`repeat_infinite()` internally calls `.buffered()` and clones the buffered source each time it loops. The `Buffered` type's clone implementation does not properly release old buffer segments, causing cumulative memory growth. The maintainers acknowledge the issue but note that the buffered implementation "may require a complete rewrite." The leak eventually stabilizes at some multiple of the source size (one user saw 3MB file stabilize at ~300MB), but this is still unacceptable for a long-running music player.

**Consequences:**
- Memory usage grows continuously during ambient playback
- After hours of use (the primary use case -- user has ambient on 90% of the time), the app consumes hundreds of MB or GBs
- OOM kill on memory-constrained systems, or system slowdown
- Users see the app as "leaky" and unreliable for the exact feature they use most

**Prevention:**
- Do NOT use `repeat_infinite()` for ambient looping. Instead, implement manual loop detection and re-append.
- Approach 1 (recommended): Use `sink.empty()` polling in the event loop (already present for main track auto-advance). When the ambient sink reports empty, re-decode from cached bytes and append again. This mirrors the existing `replay_current()` pattern.
- Approach 2: Create a custom `Source` wrapper that re-reads from a `Cursor<Vec<u8>>` when the inner decoder returns `None`, avoiding the `Buffered` allocation entirely. This is more complex but provides seamless looping without the gap that approach 1 might introduce.
- Approach 3: Pre-decode the entire ambient track into a `Vec<f32>` sample buffer, then wrap in a custom `Source` that loops over the buffer index. Simple, but uses memory proportional to decoded audio (much larger than compressed).
- For any approach, monitor memory usage during development. A 5-minute test with looping ambient should show stable RSS.

**Detection:**
- Monitor process RSS over time during ambient playback (`/proc/self/status` VmRSS)
- Memory growth correlated with ambient loop iterations
- Eventually: OOM kill or system slowdown after hours of use

**Phase to address:** Phase 1 (ambient audio foundation) -- the looping mechanism is a core design decision. Must be validated early because it affects the entire ambient playback architecture.

**Confidence:** HIGH -- documented open issue #673 on rodio GitHub with confirmed reproduction, maintainer acknowledgment, and no fix as of 2025.

---

### Pitfall 4: Breaking Existing Playback When Refactoring Player Struct

**What goes wrong:**
The existing `Player` struct has a single `sink` field and methods like `load_and_play()`, `toggle_pause()`, `is_finished()`, `replay_current()`, `seek_forward()`, `seek_backward()` that all operate on `self.sink`. Adding an ambient channel requires either: (a) adding a second sink field, or (b) restructuring into a different architecture. Either path risks breaking the battle-tested main playback path through subtle changes: wrong sink paused, volume applied to wrong channel, `is_finished()` checking wrong sink, seek operating on ambient instead of main.

**Why it happens:**
The current code is clean but tightly coupled to a single-sink model. Every method implicitly operates on "the one sink." When there are two sinks, every method must explicitly specify which sink it targets. Missing even one creates a bug. Additionally, `App.check_download_complete()` calls `player.load_and_play()` which recreates the sink -- if the ambient sink shares state, this recreation could disrupt ambient playback.

**Consequences:**
- Spacebar pauses ambient instead of (or in addition to) main music
- Volume up/down changes ambient volume instead of main
- `is_finished()` returns true when ambient finishes its loop (not when main track ends)
- Auto-advance triggers when ambient loop ends, skipping to next main track
- Seek operates on ambient channel, doing nothing audible

**Prevention:**
- Separate concerns cleanly: create an `AmbientPlayer` struct (or similar) alongside the existing `Player`. Do NOT modify the existing `Player` struct's method signatures. The `Player` continues to manage main playback exactly as it does today.
- If using a combined struct, rename the existing sink to `main_sink` and add `ambient_sink`. Update ALL existing method references from `self.sink` to `self.main_sink` in a single, reviewable commit before adding any ambient logic.
- The `is_finished()` method must ONLY check `main_sink.empty()`. The ambient sink is never "finished" (it loops).
- `toggle_pause()` should pause/resume BOTH sinks simultaneously (user expectation: spacebar controls all audio).
- Volume controls should be channel-specific: existing +/- keys control main, new keybindings control ambient.
- Write tests for: pause pauses both, volume changes only target channel, is_finished ignores ambient, seek only affects main.

**Detection:**
- Any behavior change in existing playback after the refactor (regression)
- Ambient track influences main track auto-advance
- Volume controls feel "wrong" or affect unexpected channel

**Phase to address:** Phase 1 (ambient audio foundation) -- the struct refactoring is the first implementation step and the highest-risk change to existing functionality.

**Confidence:** HIGH -- directly observable from the codebase structure. Every method in `player.rs` references `self.sink`.

---

## Moderate Pitfalls

Issues that cause bugs or degraded experience but are recoverable.

---

### Pitfall 5: Ambient Pause/Resume Desynchronizes from Main

**What goes wrong:**
User pauses (spacebar), both channels pause. User unpauses, both resume. But after extended pause on WSL2 (>5 seconds), the PulseAudio stream for one sink may fail to resume while the other succeeds. Result: main music plays but ambient is silent, or vice versa. The existing codebase already documents this WSL2 issue in the Player struct comment: "WSL2 workaround if pause/resume fails after extended pauses."

**Why it happens:**
WSLg's PulseAudio bridge has documented issues with stream cork/uncork (pause/resume) after extended pauses. With two sinks feeding the same mixer, the resume is attempted for both, but the underlying cpal/ALSA layer sees this as a single stream. If the resume fails partway, one sink may resume while the other remains corked. There is no error returned -- the sink just produces silence.

**Prevention:**
- Pause both sinks in the same call, resume both in the same call. Do not pause them in separate event handler branches.
- After resume, add a brief verification delay (100-200ms), then check if both sinks are producing audio. If one is not, recreate that sink from the same OutputStream mixer and re-append its source.
- The existing `_audio_data` pattern (keeping raw bytes for re-creation) should be replicated for ambient audio data. If resume fails, tear down and recreate the ambient sink from cached bytes.
- Consider the existing PULSE_LATENCY_MSEC=150 setting -- this already provides buffer headroom. Verify that adding a second sink does not require increasing this further.

**Detection:**
- After unpausing on WSL2, one channel plays but the other is silent
- Issue only occurs after pauses > 5 seconds
- More common under CPU load or after WSL2 has been idle

**Phase to address:** Phase 1 (ambient audio foundation), with testing in Phase 2 (integration). The resume verification should be designed into the architecture, not bolted on.

**Confidence:** MEDIUM -- extrapolated from documented single-stream WSL2 resume issues. Two sinks may actually be better (single mixer stream) or worse (more complex state). Needs empirical validation.

---

### Pitfall 6: Ambient Volume Default Too Loud, First Impression is Bad

**What goes wrong:**
Developer sets ambient default volume to 0.5 (a "reasonable" default). User starts ambient for the first time. The ambient track is mastered at a different loudness than the main music. Combined, they sound muddy, distorted, or the ambient overpowers the music. First impression: "this feature sounds terrible." User turns off ambient and never uses it again.

**Why it happens:**
Different audio sources are mastered at wildly different loudness levels. A "rain sounds" ambient track may have constant -6dB RMS while music varies from -20dB to 0dB. There is no perceptual normalization in rodio. Additionally, the combined volume budget (Pitfall 1) means the ambient volume setting affects perceived main music volume even when it doesn't cause clipping.

**Consequences:**
- Feature perceived as low quality on first use
- Users do not realize they can adjust ambient volume separately
- If ambient defaults mask music, users blame the app rather than adjusting settings

**Prevention:**
- Default ambient volume should be LOW: 0.15-0.25 range. It is much easier for users to turn up a quiet ambient than to struggle with an overpowering one.
- Show ambient volume prominently in the UI so users know it is adjustable.
- Persist ambient volume separately in session state (like `saved_volume` is persisted for main).
- Consider different volume step sizes for ambient (0.02 instead of 0.05) since ambient volume is more sensitive -- small changes have big perceptual impact.

**Detection:**
- User feedback about ambient being "too loud" or "drowning out music"
- Users disabling ambient feature after trying it once

**Phase to address:** Phase 2 (ambient UX polish) -- defaults and step sizes can be tuned after the core feature works.

**Confidence:** MEDIUM -- based on audio engineering principles and UX common sense. The specific default depends on the ambient content.

---

### Pitfall 7: `Sink::stop()` on Ambient Sink During Main Track Change

**What goes wrong:**
The existing `load_and_play()` calls `self.sink.stop()` then creates a fresh Sink via `Sink::connect_new()`. This is the documented workaround for rodio's "append blocks after stop" issue (#171). If the refactored code accidentally calls `stop()` on the ambient sink (or on a shared reference), the ambient track stops and the ambient sink becomes unusable. A new Sink must be created, but the ambient source data may not be readily available for re-append.

**Why it happens:**
In rodio, `Sink::stop()` is a one-way operation. A stopped Sink cannot accept new sources -- you must create a new Sink. The existing code creates a fresh Sink for each main track, which is correct. But if the ambient sink is inadvertently stopped (e.g., by a method that operates on "the player's sink" generically), the ambient track goes silent and cannot be restarted without recreating the sink and re-decoding the ambient audio.

**Consequences:**
- Ambient track silently stops when user changes main music track
- No error message -- ambient just disappears
- User must manually restart ambient after every track change

**Prevention:**
- Main track changes should ONLY call `stop()` on `main_sink`. The `ambient_sink` must be completely untouched by the main track change flow.
- In the refactored `load_and_play()` method, explicitly name the sink: `self.main_sink.stop()`, not `self.sink.stop()`.
- Keep ambient source bytes cached (like `_audio_data` for main). If ambient sink needs recreation for any reason, it can be rebuilt from cache.
- Add an assertion or log in ambient code path that fires if `ambient_sink.empty()` returns true unexpectedly (detecting accidental stop).

**Detection:**
- Ambient goes silent every time a new main track starts
- Ambient works fine until first track change, then disappears

**Phase to address:** Phase 1 (ambient audio foundation) -- the sink isolation is part of the core architecture.

**Confidence:** HIGH -- directly visible in the existing `load_and_play()` code which calls `self.sink.stop()`.

---

### Pitfall 8: Track Browsing UI State Conflicts with Playing State

**What goes wrong:**
Adding track browsing (for ambient track selection) introduces a second navigation context in the TUI. The existing app uses `track_state: ListState` for the main track list. If ambient tracks share the same list widget or the same navigation keybindings, scrolling through ambient tracks will move the main track selection cursor, or pressing Enter on an ambient track will try to play it as a main track.

**Why it happens:**
The current `App` has a single `view` state machine (Playlists -> Tracks -> Playing) and single `track_state`. Adding ambient browsing requires either a new view state, a modal/overlay, or a split-pane UI. Each approach has different implications for input handling. The `handle_key()` method currently dispatches based on `self.view`, and a new view state must be integrated without breaking existing navigation flows.

**Consequences:**
- Pressing Enter in ambient browser starts main playback of that track instead
- Navigation keys (j/k) move wrong list when focus is ambiguous
- Escape from ambient browser exits to wrong parent view
- Selection cursor visual highlight appears in wrong list

**Prevention:**
- Add an explicit focus/context concept: `enum FocusContext { MainTracks, AmbientBrowser }`. Key handling checks focus before dispatching.
- Use distinct keybindings for ambient browsing (e.g., `a` to open ambient browser, separate Enter handling when in ambient context).
- Ambient track selection should use its OWN `ListState`, completely separate from `track_state`.
- The `AppView` enum should gain a new variant (e.g., `AmbientBrowsing`) or use a sub-state within existing views.
- Keep the state machine transitions documented. Draw the state diagram before implementing.

**Detection:**
- Wrong track plays when selecting from ambient browser
- Navigation affects wrong list
- Cannot return to correct view after ambient browsing

**Phase to address:** Phase 2 (track browsing UI) -- this is the primary UI challenge of the browsing feature.

**Confidence:** HIGH -- directly observable from the app.rs state machine design.

---

### Pitfall 9: WSL2 Audio Quality Degrades with Two Simultaneous Sinks

**What goes wrong:**
The existing PULSE_LATENCY_MSEC=150 and .asoundrc buffer tuning were calibrated for single-stream playback. Adding a second concurrent audio source (ambient) increases the audio processing load on the WSLg PulseAudio bridge. This may push the system past the buffer underrun threshold, reintroducing the crackling that the existing buffer tuning solved.

**Why it happens:**
WSLg's PulseAudio bridge runs with fixed resources. Two concurrent streams mean twice the sample data flowing through the bridge per unit time. The ALSA -> PulseAudio -> Windows audio path has limited buffer capacity, and the 150ms latency setting may not provide enough headroom for two streams. Additionally, rodio mixes internally before sending to the OS, so from PulseAudio's perspective it is still one stream -- but the mixing itself adds CPU overhead on the audio thread, which can cause scheduling jitter that triggers underruns.

**Prevention:**
- Test early: as the very first step of Phase 1, create two Sinks playing simultaneously on the existing OutputStream and listen for degradation compared to single-sink baseline.
- If degradation occurs, try increasing PULSE_LATENCY_MSEC to 200 or 250. Music playback can tolerate up to 300ms latency without perceptible delay (there is no interactive component requiring low latency).
- Monitor CPU usage of the audio thread. Rodio's mixer runs on a background thread -- if this thread is CPU-starved on WSL2, consider lowering the ambient track's sample rate (e.g., 22050 Hz for ambient vs 44100 Hz for main).
- The .asoundrc buffer settings may need updating. The current v2 config uses PulseAudio defaults -- explicit `buffer_size` and `periods` settings might be needed.

**Detection:**
- Crackling appears after adding ambient that was not present with main-only playback
- Crackling worsens under CPU load (compiling, other WSL2 processes)
- Crackling resolves when ambient is muted (volume 0) but not when ambient sink exists at any volume

**Phase to address:** Phase 1 (ambient audio foundation) -- must be validated before building the full feature. If two-sink audio quality is unacceptable on WSL2, the architecture must change (e.g., pre-mix ambient into main at the Source level instead of using separate Sinks).

**Confidence:** MEDIUM -- extrapolated from existing WSL2 audio tuning. The fact that rodio mixes internally (sending one stream to PulseAudio) means this may be a non-issue. Needs empirical testing.

---

## Minor Pitfalls

Issues that cause inconvenience or polish problems.

---

### Pitfall 10: Ambient Track Downloads Block the Event Loop

**What goes wrong:**
The existing download pattern uses `std::thread::spawn` with `reqwest::blocking` to download main tracks without blocking the TUI. If ambient track downloads reuse this pattern but are initiated at the same time as a main track download (e.g., user starts favorite playlist while ambient is loading), two blocking download threads run simultaneously. On slow connections, this halves bandwidth for each, making both feel slow. On fast connections, this is fine.

**Prevention:**
- Use the same download channel pattern (mpsc) but with distinct channels for main vs ambient downloads.
- Consider prioritizing main track downloads over ambient (main track download blocks the Playing view transition, ambient can load in the background without blocking anything).
- Add a "loading ambient..." indicator so users know why ambient has not started yet.

**Phase to address:** Phase 2 (ambient download integration) -- after the core playback works with pre-loaded test audio.

**Confidence:** MEDIUM -- depends on network conditions and user behavior patterns.

---

### Pitfall 11: Session Persistence Does Not Include Ambient State

**What goes wrong:**
The existing session persistence saves playlist, track index, volume, shuffle, and repeat mode. If ambient state (selected ambient track, ambient volume, whether ambient was playing) is not persisted, users lose their ambient setup every restart. Given that ambient is used "90% of the time," this becomes a major annoyance.

**Prevention:**
- Extend the `Session` struct with ambient fields: `ambient_track_url`, `ambient_volume`, `ambient_enabled`.
- Use serde's `#[serde(default)]` on new fields so existing session.toml files remain compatible (new fields get defaults).
- Save ambient state in the same `save_session_state()` call -- no separate file needed.
- Restore ambient state in `restore_session()` -- but like main playback, do NOT auto-play ambient on restore. Let the user explicitly resume.

**Phase to address:** Phase 3 (polish and persistence) -- after ambient playback and browsing work reliably.

**Confidence:** HIGH -- directly observable gap in the current Session struct.

---

### Pitfall 12: Visualizer Only Shows Main Audio, Not Combined

**What goes wrong:**
The existing visualizer taps into the main audio source via `VisualizerSource`. When ambient audio is playing simultaneously, the visualizer only shows the main track's spectrum. Users may expect the visualizer to reflect "what they hear" (combined audio), leading to confusion when the visualizer seems to miss the ambient layer.

**Prevention:**
- Decide early: should the visualizer show main-only or combined? For a music player, main-only makes more sense (the visualizer represents the song, not the ambient background).
- Document this decision in the UI (tooltip or help text).
- If combined visualization is desired later, it requires tapping the mixer output rather than individual sink inputs -- this is architecturally different and should not be attempted in the initial implementation.

**Phase to address:** Phase 3 (polish) -- this is a UX decision, not a technical blocker.

**Confidence:** HIGH -- observable from the existing VisualizerSource architecture.

---

## Phase-Specific Warnings

| Phase Topic | Likely Pitfall | Mitigation | Severity |
|-------------|---------------|------------|----------|
| Phase 1: Two-sink architecture | Pitfall 2 (OutputStream lifetime) | Single OutputStream, both sinks from same mixer | Critical |
| Phase 1: Two-sink architecture | Pitfall 4 (breaking existing playback) | Rename sink -> main_sink first, review all references | Critical |
| Phase 1: Ambient looping | Pitfall 3 (repeat_infinite memory leak) | Manual loop via empty() check + re-append | Critical |
| Phase 1: Volume control | Pitfall 1 (mixer clipping) | Volume budget: main + ambient <= 1.0 | Critical |
| Phase 1: WSL2 validation | Pitfall 9 (audio quality degradation) | Two-sink test before building features | Moderate |
| Phase 1: Main track change | Pitfall 7 (accidental ambient stop) | Explicit main_sink.stop(), never touch ambient | Moderate |
| Phase 2: Pause/resume | Pitfall 5 (desync on WSL2) | Resume both sinks together, verify both active | Moderate |
| Phase 2: Track browsing UI | Pitfall 8 (UI state conflicts) | Separate ListState, explicit FocusContext | Moderate |
| Phase 2: Volume defaults | Pitfall 6 (ambient too loud) | Default 0.15-0.25, fine-grained steps | Minor |
| Phase 2: Concurrent downloads | Pitfall 10 (blocking event loop) | Separate channels, prioritize main | Minor |
| Phase 3: Session persistence | Pitfall 11 (ambient state not saved) | Extend Session struct with serde defaults | Minor |
| Phase 3: Visualizer | Pitfall 12 (main-only display) | Document decision, main-only is fine | Minor |

---

## Testing Approaches to Catch Issues Early

### Smoke Test: Two-Sink Baseline (Do This FIRST)

Before writing any feature code, validate that two sinks playing simultaneously on WSL2 produces acceptable audio quality:

1. Create a minimal test: open one OutputStream, create two Sinks from its mixer
2. Load a music file into Sink 1, a different file into Sink 2
3. Set both to volume 0.5 (safe budget)
4. Play for 60 seconds on WSL2
5. Listen for: crackling, dropouts, one channel going silent
6. If any issues: adjust PULSE_LATENCY_MSEC and .asoundrc before proceeding

### Memory Stability Test

1. Start ambient looping with chosen approach (manual re-append, not repeat_infinite)
2. Monitor RSS every 30 seconds for 5 minutes
3. RSS should be stable (within 1-2MB variation)
4. If growing: the looping approach has a leak

### Integration Regression Test

After refactoring Player struct:
1. All existing functionality works exactly as before (play, pause, seek, volume, next, prev, repeat, shuffle)
2. No behavior change when ambient is not active
3. Main track auto-advance still triggers correctly
4. Session restore works with both old (no ambient) and new session files

### Pause/Resume Stress Test (WSL2-specific)

1. Play main + ambient simultaneously
2. Pause for 1s, resume. Verify both play.
3. Pause for 5s, resume. Verify both play.
4. Pause for 30s, resume. Verify both play.
5. Repeat 10 times. Count failures.

---

## Sources

- [rodio GitHub issue #673: repeat_infinite memory leak](https://github.com/RustAudio/rodio/issues/673) -- HIGH confidence
- [rodio GitHub issue #340: clipping with set_volume](https://github.com/RustAudio/rodio/issues/340) -- HIGH confidence
- [rodio GitHub issue #171: cannot restart stopped sink](https://github.com/RustAudio/rodio/issues/171) -- HIGH confidence
- [rodio GitHub issue #330: OutputStream drop kills audio](https://github.com/RustAudio/rodio/issues/330) -- HIGH confidence
- [rodio documentation: Source trait](https://docs.rs/rodio/latest/rodio/source/trait.Source.html) -- HIGH confidence
- [rodio documentation: multiple sinks and mixer](https://docs.rs/rodio/latest/rodio/index.html) -- HIGH confidence
- [WSLg GitHub issue #908: choppy audio](https://github.com/microsoft/wslg/issues/908) -- MEDIUM confidence
- [WSLg GitHub issue #1257: sound stuttering](https://github.com/microsoft/wslg/issues/1257) -- MEDIUM confidence
- [PulseAudio troubleshooting: buffer underruns](https://wiki.archlinux.org/title/PulseAudio/Troubleshooting) -- MEDIUM confidence
- Existing TermTunes codebase analysis: `/home/jigsaw/src/termtunes/src/player.rs`, `/home/jigsaw/src/termtunes/src/app.rs`, `/home/jigsaw/src/termtunes/src/visualizer.rs` -- HIGH confidence
