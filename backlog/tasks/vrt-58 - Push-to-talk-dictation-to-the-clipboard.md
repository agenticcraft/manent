---
id: VRT-58
title: Push-to-talk dictation to the clipboard
status: Done
assignee:
  - '@claude'
created_date: '2026-11-17T09:52:00Z'
updated_date: '2026-11-17T15:20:00Z'
labels:
  - feature
milestone: m-4
dependencies:
  - VRT-53
priority: medium
ordinal: 62000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Build dictation as chosen in VRT-53 and decision-11. Hold a global shortcut (default Ctrl+Shift+Space, changeable like the other shortcuts, decision-10), speak, release: Manent records the mic only, transcribes it with the local engine and puts the text on the clipboard, and a notification says it is ready to paste. No meeting, no speakers, nothing saved. Off until the user turns it on in Settings > Shortcuts.

From the spike: the portal sends Activated on press (then repeats every 30 ms after 500 ms while held) and one Deactivated on release; the shortcut loop must read both. The mic opens on press (~150 ms to first samples). The clipboard is set with wl-copy (RPM Requires wl-clipboard), which works while another window has focus. Notifications through the Notification portal (ashpd feature notification). Transcription takes ~0.3 s for 5 s of speech and ~1.8 s for 30 s.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Holding the shortcut records the mic and releasing it puts the transcribed text on the clipboard, with Manent in the background; key repeats while holding don't restart it, and a press shorter than 0.3 s does nothing
- [x] #2 A notification says the text is ready (or that nothing was heard, or that the engine isn't installed); while dictating a calm indicator shows it is listening (tray icon or notification)
- [x] #3 Dutch and English are transcribed; é, ë, quotes and newlines survive the clipboard
- [x] #4 Nothing is stored: no meeting, no audio file, no speaker embedding; the audio stays in memory
- [x] #5 Settings > Shortcuts turns dictation on and off and shows its key; the choice survives a restart
- [x] #6 Unit tests for the hold logic; a smoke script covers dictation to the clipboard; screenshots of the settings in light and dark
- [x] #7 With the RPM, wl-clipboard is installed as a dependency; without wl-copy (a development build) dictation says so instead of failing silently
- [x] #8 During a meeting recording the dictation shortcut does nothing but show a notification that dictation isn't available while recording (decision-11)
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Shortcuts: one portal session binds the recording keys and/or dictation; read Deactivated; HoldTracker drops repeats.
2. dictation module: Hold state machine (min 0.3 s, max 5 min, busy, blocked during a recording), mic capture on its own thread into memory (16 kHz), engine via a WAV in XDG_RUNTIME_DIR (tmpfs) removed at once, wl-copy, portal notifications, tray dot while listening.
3. Settings > Shortcuts > Dictation (on/off, key); IPC set_dictation_enabled.
4. RPM depends on wl-clipboard.
5. Unit tests (hold logic, repeats), ignored clipboard test, smoke-dictation.py in the isolated GNOME session with a virtual mic; screenshots.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Built as planned. Notes:
- shortcuts::apply binds whichever of the recording keys and dictation are on in one GlobalShortcuts session; if a new bind fails (dialog cancelled) the previous set is bound again. Both settings are saved from the resulting status.
- HoldTracker: activations while held are repeats; once the desktop has sent a Deactivated, a key stays held until its release (on the real desktop GNOME's repeats had gaps over 700 ms, which counted as a second press). Without releases, presses more than 700 ms apart count. This also stops a held Ctrl+Alt+R from toggling twice.
- Audio: the mic chosen in Settings > Sources (or the default) is opened on its own thread; samples are downmixed and resampled to 16 kHz in memory. The engine helper only reads files, so the WAV goes to XDG_RUNTIME_DIR (tmpfs, RAM) and is removed straight after; the smoke checks nothing is left there or in the data folder.
- Notifications go through the portal (ashpd feature notification, no new crates). With GNOME's Do Not Disturb on (the user's desktop) they go silently to the message list; the tray dot going away is then the signal.
- pretty_trigger orders modifiers Ctrl+Alt+Shift+Super (GNOME reports Shift+Ctrl+Space).
- wl-copy refuses a seat without a keyboard (only the headless test session); spikes/dictation/with_keyboard.py gives it one.
- Measured (smoke, isolated session, real engine): NL 11.2 s speech -> clipboard 1.2 s after release; EN 6.5 s -> 0.4 s; 1.5 s silence -> 'Nothing was heard'. On the user's desktop: 12 s in 1.0 s, 8 s in 0.5 s.

Verification:
- AC1-5, 8: scripts/smoke-dictation.py in the isolated GNOME session, all checks pass (bind via GNOME's dialog, engine-missing notice, tap ignored, NL+EN to the clipboard, silence, nothing stored in the data folder or runtime dir, blocked during a recording, still on after a restart, off lets go of the key). Tray dot: tray::set_dictating; checked by the user on the real desktop.
- AC3 special characters: #[ignore] test wayland_clipboard_keeps_special_characters (é, ë, quotes, em dash, newlines, tab) passes in the isolated session.
- AC6: unit tests for Hold (dictation) and HoldTracker (shortcuts); screenshots settings-dictation-{off,on}-{light,dark}.png checked.
- AC7: release RPM built locally (not published): rpm -qpR lists wl-clipboard. Without wl-copy: unit test without_wl_copy_it_says_what_to_install; the notification shows that text.
- cargo fmt, clippy, cargo test (160 passed), npm run check: clean.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Push-to-talk dictation to the clipboard (decision-11). Hold the dictation shortcut (default Ctrl+Shift+Space, bound through the GlobalShortcuts portal like the recording keys, turned on separately in Settings > Shortcuts > Dictation), speak, release: the mic chosen in Settings > Sources is recorded into memory, transcribed by the local engine (NL/EN), and the text goes on the clipboard with wl-copy; a portal notification says it is ready (or nothing heard, engine missing, wl-copy missing, blocked during a meeting recording). The tray shows a calm dot while listening. Nothing is stored. The shortcut loop now reads releases and drops key repeats for all shortcuts (a held Ctrl+Alt+R no longer toggles twice). The RPM requires wl-clipboard. New smoke script scripts/smoke-dictation.py runs in the isolated GNOME session of spikes/dictation with a temporary virtual mic.
<!-- SECTION:FINAL_SUMMARY:END -->
