# Phase 6: Dual-Sink Audio Engine - Context

**Gathered:** 2026-02-10
**Status:** Ready for planning

<domain>
## Phase Boundary

Add a second independent audio channel (ambient track) that plays simultaneously with main music playback. This phase delivers the core audio engine capability with two independent sinks sharing a single OutputStream, volume control with budget enforcement, continuous looping, and failure isolation. UI controls and track selection are separate phases.

</domain>

<decisions>
## Implementation Decisions

### Mixing approach
- Use rodio's SpatialSink for software mixing - simpler code, single output stream
- Enforce volume budget (main + ambient ≤ 1.0) to prevent clipping
- Budget enforcement happens on every volume change (dynamic adjustment)
- When volumes would exceed budget: auto-scale both proportionally to fit
- Share single OutputStream between both sinks (never create second OutputStream)

### Loop behavior
- Brief silence between loops is acceptable - prioritize memory safety over gapless playback
- Avoid rodio `repeat_infinite()` due to confirmed memory leak
- Use manual re-append loop when track ends

### Volume architecture
- Structure: Independent sink volumes (main, ambient) + master volume
- Master volume applies AFTER budget enforcement (scales final output)
- Default ambient volume: 30% lower than main music volume
- Muting: Set volume to 0 (simple, no separate mute state needed)
- Volume budget enforced at sink level before master scaling

### Failure handling
- Ambient decode/play failures: Log error, clear ambient state, keep main playing
- Best-effort isolation between channels (normal errors isolated, rodio mixer panics could affect both)
- OutputStream failures: Attempt recovery (recreate OutputStream and resume both channels)
- Resource exhaustion: Prioritize main music over ambient (drop ambient if resources tight)

### Claude's Discretion
- Exact loop validation duration (30+ min target, but can adjust for practical testing)
- Memory growth threshold during extended looping (suggest <5MB over 30min, or flat)
- Specific error logging format and detail level
- Recovery mechanism implementation details

</decisions>

<specifics>
## Specific Ideas

- Volume budget research finding: main + ambient <= 1.0 prevents mixer clipping
- Known issue: rodio `repeat_infinite()` has memory leak - use manual loop instead
- Architecture constraint: Single OutputStream shared by both sinks (validated approach)
- Success criteria from roadmap: 30+ minutes stable looping with no memory growth beyond initial load

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope.

(UI visibility for errors belongs in Phase 8: Ambient Status UI & Controls)

</deferred>

---

*Phase: 06-dual-sink-audio-engine*
*Context gathered: 2026-02-10*
