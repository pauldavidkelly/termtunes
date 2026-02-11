---
phase: 08-ambient-status-ui-controls
verified: 2026-02-11T13:52:51Z
status: passed
score: 7/7 truths verified
re_verification: false
---

# Phase 8: Ambient Status UI & Controls Verification Report

**Phase Goal:** User has full visibility into ambient state and can control it efficiently with dedicated keybindings
**Verified:** 2026-02-11T13:52:51Z
**Status:** passed
**Re-verification:** No — initial verification

## Goal Achievement

### Observable Truths

| #   | Truth                                                                                                                                                                                     | Status     | Evidence                                                                                                                                                                                  |
| --- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 1   | User sees a 1-line ambient status panel showing track name, play/pause icon, and volume percentage when an ambient track is loaded                                                       | ✓ VERIFIED | `render_ambient_panel()` exists in ui.rs:467, renders state icon (AMB >> / AMB \|\|), track name via `ambient_track_name()`, and volume percentage. Conditional on `has_ambient`.       |
| 2   | User presses ] to increase ambient volume by 5% and sees the change in the ambient panel immediately                                                                                     | ✓ VERIFIED | KeyCode::Char(']') handler at app.rs:718 calls `ambient_volume_up()` which increments `ambient_volume` by 0.05 and calls `apply_ambient_volume()`. Panel reads `app.ambient_volume()`. |
| 3   | User presses [ to decrease ambient volume by 5% and sees the change in the ambient panel immediately                                                                                     | ✓ VERIFIED | KeyCode::Char('[') handler at app.rs:722 calls `ambient_volume_down()` which decrements `ambient_volume` by 0.05 and calls `apply_ambient_volume()`. Panel reads `app.ambient_volume()`. |
| 4   | User presses m to toggle ambient off (mute) and sees the panel state change to paused/dimmed                                                                                             | ✓ VERIFIED | KeyCode::Char('m') handler at app.rs:710 calls `toggle_ambient()` which saves volume to `pre_mute_ambient_volume` and sets `ambient_volume` to 0. Panel shows "AMB \|\|" in DarkGray. |
| 5   | User presses m again to unmute and ambient restores to the exact volume it was before muting (not hardcoded 0.3)                                                                         | ✓ VERIFIED | `toggle_ambient()` at app.rs:1310 restores `ambient_volume` from `pre_mute_ambient_volume` field (app.rs:252) initialized to 0.3 but updated on each mute.                              |
| 6   | Ambient panel does not appear when no ambient track has been loaded                                                                                                                       | ✓ VERIFIED | Layout determination at ui.rs:75 checks `has_ambient` via `ambient_track_name().is_some()`. Panel only rendered in branches where `has_ambient` is true.                                 |
| 7   | Help text in status bar includes [/]:amb vol keybinding hints                                                                                                                            | ✓ VERIFIED | ui.rs:544 shows `[/]:amb vol` and `m:amb` in full-width help text. ui.rs:539 shows `m:amb` in narrow mode.                                                                              |

**Score:** 7/7 truths verified

### Required Artifacts

| Artifact         | Expected                                                                                                        | Status     | Details                                                                                                                                                                                                                                             |
| ---------------- | --------------------------------------------------------------------------------------------------------------- | ---------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `src/app.rs`     | pre_mute_ambient_volume field, ambient_volume_up/down methods, toggle_ambient method, bracket keybindings      | ✓ VERIFIED | Field at line 252, methods at 1283/1289/1310, keybindings at 718/722. All substantive implementations with proper volume clamping, state saving, and apply_ambient_volume() calls. Wired to handle_key() event dispatcher.                         |
| `src/ui.rs`      | render_ambient_panel function, conditional 1-line layout insertion, updated help text                          | ✓ VERIFIED | render_ambient_panel() at line 467 with full implementation (state icon, track name, volume display, narrow mode handling). Conditional 4-branch layout at lines 77-129. Help text updated at 539/544. Wired to render() main UI function.         |
| `src/player.rs`  | set_ambient_volume() calls Sink::set_volume() directly (not sink recreation)                                   | ✓ VERIFIED | set_ambient_volume() at player.rs:456 uses direct `sink.set_volume()` call. Comment at 451 explains this preserves playback position (fix from Task 3). Wired from app.rs apply_ambient_volume() at line 1301.                                    |

### Key Link Verification

| From             | To                 | Via                                                                                                       | Status     | Details                                                                                                                                                                                        |
| ---------------- | ------------------ | --------------------------------------------------------------------------------------------------------- | ---------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| src/app.rs       | src/app.rs         | handle_key dispatches [ and ] to ambient_volume_up/down                                                  | ✓ WIRED    | KeyCode::Char(']') at line 718 calls ambient_volume_up(), KeyCode::Char('[') at line 722 calls ambient_volume_down(). Both methods exist and modify ambient_volume field.                   |
| src/app.rs       | src/player.rs      | apply_ambient_volume calls player.set_ambient_volume                                                      | ✓ WIRED    | apply_ambient_volume() at line 1298 calls `player.set_ambient_volume(ambient_final)` at line 1301. Method exists in player.rs at line 456.                                                   |
| src/ui.rs        | src/app.rs         | render_ambient_panel reads app.ambient_volume() and app.player().ambient_track_name()                    | ✓ WIRED    | render_ambient_panel() at line 467 calls app.ambient_volume() at lines 472/502 and app.player().ambient_track_name() at line 470. Both methods exist and return correct values.             |
| src/ui.rs        | src/ui.rs          | render() conditionally inserts ambient_area into Layout::vertical when has_ambient is true               | ✓ WIRED    | render() determines has_ambient at line 75, then branches to 4-part layout (lines 77-90) or 3-part ambient-only layout (lines 103-118). render_ambient_panel() called at lines 88 and 113.  |

### Requirements Coverage

| Requirement | Description                                                                            | Status       | Evidence                                                                                                                                                      |
| ----------- | -------------------------------------------------------------------------------------- | ------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| UI-01       | User sees ambient track status in dedicated UI panel (track name, play/pause state)   | ✓ SATISFIED  | render_ambient_panel() shows track name via ambient_track_name() and state icon (AMB >> active / AMB \|\| muted) based on ambient_volume() > 0.0          |
| UI-05       | UI shows current ambient volume level                                                  | ✓ SATISFIED  | render_ambient_panel() displays volume percentage at line 502-510: `format!("Vol: {}%", vol_pct)` where vol_pct is `(app.ambient_volume() * 100.0).round()` |
| UI-06       | User can adjust ambient volume up/down with dedicated keybindings                      | ✓ SATISFIED  | [ / ] keybindings at app.rs:718/722 call ambient_volume_up/down methods which adjust volume by 0.05 increments                                               |
| UI-07       | User can toggle ambient on/off with dedicated keybinding                               | ✓ SATISFIED  | m keybinding at app.rs:710 calls toggle_ambient() which saves/restores volume using pre_mute_ambient_volume field                                            |

### Anti-Patterns Found

**No anti-patterns found.** All implementations are substantive with no TODO/FIXME comments, no stub implementations, and no placeholder logic.

### Human Verification Required

None. All automated checks passed and all observable truths can be programmatically verified through code inspection.

**Note:** The SUMMARY.md documents that Task 3 included human verification which confirmed:
- Ambient panel appears only after loading an ambient track (not on startup)
- [ / ] keybindings change volume without restarting playback (after fix in commit 78409fb)
- m toggle correctly saves and restores pre-mute volume
- Panel shows correct state icon, track name, and volume percentage
- Visualizer + ambient panel 4-way layout works correctly

### Gaps Summary

**No gaps found.** All 7 observable truths verified, all 3 required artifacts pass all levels (exists, substantive, wired), all 4 key links verified as wired, and all 4 requirements satisfied.

Phase goal achieved: User has full visibility into ambient state (dedicated UI panel showing track name, play/pause state, volume level) and can control it efficiently with dedicated keybindings ([ / ] for volume, m for toggle with pre-mute memory).

---

_Verified: 2026-02-11T13:52:51Z_
_Verifier: Claude (gsd-verifier)_
