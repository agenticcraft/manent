---
id: VRT-53
title: 'Spike: push-to-talk dictation into any text field'
status: Done
assignee:
  - '@claude'
created_date: '2026-11-04T21:43:00Z'
updated_date: '2026-11-17T11:20:00Z'
labels:
  - spike
milestone: m-4
dependencies:
  - VRT-40
priority: medium
ordinal: 57000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Requested (2026-11-04, refined 2026-11-17): use Manent as a dictation tool outside meetings. Hold a global shortcut (for instance Ctrl+Shift+Space), speak, release it, and the text is ready where the cursor is. Nothing leaves the computer: the mic only, the local engine (parakeet, NL/EN), no system audio, no speakers and no meeting saved.

Chosen approach (2026-11-17): the text goes to the clipboard and a notification says it's ready, and the user pastes (VRT-58). A setting, off by default, pastes it automatically by sending Ctrl+V through the RemoteDesktop portal (VRT-59). An IBus input-method engine is not pursued.

This spike checks the unknowns before building:
- Hold and release: the GlobalShortcuts portal (VRT-40, decision-10) sends Activated on press and Deactivated on release (ashpd 0.13 receive_deactivated). The shortcut code now only listens to Activated and debounces key repeats (700 ms).
- The clipboard from the background: on Wayland a client usually needs keyboard focus to set the clipboard, and Manent is not focused while the user dictates. Check what works on GNOME (arboard or the Tauri clipboard plugin, native Wayland vs XWayland, the data-control protocol), and that the clipboard keeps the text after Manent's window is hidden.
- Auto-paste: the RemoteDesktop portal (keyboard emulation after the user allows it once; persist the permission with a restore token). Ctrl+V vs Ctrl+Shift+V in a terminal.
- Notifications: the notification portal or the Tauri notification plugin.
- Speed: a short clip through the 0.6B model, without the meeting pipeline (rolling and diarization).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Hold-to-talk with the portal is tried: press-to-capture latency, release, key repeats while holding, a very short press
- [x] #2 Setting the clipboard while Manent is in the background works on GNOME Wayland (or the way that does is found): text with é, ë, quotes and newlines arrives intact in a browser field, LibreOffice and a terminal
- [x] #3 Auto-paste through the RemoteDesktop portal is tried: what the user must allow, whether a restore token keeps that across restarts, and how a terminal (Ctrl+Shift+V) is handled
- [x] #4 Measured: time from releasing the shortcut to text on the clipboard for 5 s and 30 s of speech, Dutch and English, on the test machine
- [x] #5 A recommendation, with a Backlog decision for any new dependency (clipboard, notifications, the RemoteDesktop portal feature of ashpd), and VRT-58 and VRT-59 updated with the findings
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Spike 2026-11-17 on GNOME 51.1 / mutter 51.1 / xdg-desktop-portal 1.22 (GlobalShortcuts v2, RemoteDesktop v2, Clipboard v1, Notification v1). Probes in spikes/dictation, run in a headless GNOME Shell with its own D-Bus, runtime dir, home and portals (run-isolated.sh), so they never touch the user's session.

Hold-to-talk (GlobalShortcuts portal):
- Ctrl+Shift+Space binds (GNOME's 'Add Keyboard Shortcuts' dialog once). Press -> Activated after 4 ms. Holding: after 500 ms an extra Activated every 30 ms (key repeat). Release -> exactly one Deactivated (~10 ms). A 0.1 s tap gives one Activated + one Deactivated, so a minimum length (0.3 s) is the app's job.
- Manent's shortcut loop now only reads Activated with a 700 ms debounce: dictation needs receive_deactivated (ashpd 0.13 has it) and must treat repeats as 'still held'.
- GlobalShortcuts is now version 2 on GNOME 51, so ConfigureShortcuts exists (decision-10 opened GNOME Settings because v1 lacked it).
- Mic start: PipeWire capture delivers its first samples ~150 ms after starting it (pw-record, median of 5). Keeping the mic open while idle would show GNOME's mic indicator all the time, so start it on press.

Clipboard from the background (another window focused; Wayland has no data-control protocol on GNOME):
- wl-copy: works. It maps a tiny window, takes keyboard focus for 2-5 ms, sets the clipboard and leaves; Mutter's focus log shows focus going straight back to the window the user was in. é, ë, curly quotes and newlines intact.
- GTK3 over X11 (XWayland): works. xclip: fails (no valid timestamp). GTK3/GTK4 native Wayland without focus: fails, as expected (Manent's own window is Wayland GTK3).
- The RemoteDesktop portal's Clipboard interface: works without any focus trick, but needs the consent dialog.

Auto-paste (RemoteDesktop + Clipboard portals):
- The dialog ('Remote Desktop') has 'Remember This Selection' (on), 'Allow Remote Interaction' (OFF by default; Share stays disabled until it is on) and 'Allow Clipboard Access' (on).
- Start returns devices=keyboard, clipboard_enabled, a restore_token. A new process with that token starts in 2-4 ms without a dialog and gets a new token (store it each time).
- Keys must go through ConnectToEIS (libei): Mutter's NotifyKeyboard* reaches the Shell but not app windows in this session.
- End to end: SetSelection + SelectionTransfer/SelectionWrite put the text on the clipboard, Ctrl+V through EIS pasted it into a GTK text field 44 ms later; special characters and newlines intact.
- Paste key per app (clipboard via the portal):
  | app | Ctrl+V | Ctrl+Shift+V | Shift+Insert |
  | GTK4 text field | yes | - | yes |
  | Firefox, Chrome | yes | yes | yes |
  | LibreOffice Writer | yes | NO: opens Paste Special | yes |
  | alacritty | no | yes | pastes PRIMARY |
  | VTE (Ptyxis' widget) | types ^V | app binding (Ptyxis: yes) | pastes PRIMARY |
  With the text also in PRIMARY (wl-copy --primary), Shift+Insert worked in all of them. A Wayland app can't see which window is focused, so no single key fits everything without that.
- Ptyxis itself couldn't run a shell in the headless session (it needs the systemd user instance); its Ctrl+Shift+V binding still needs a check on a real desktop (VRT-59 smoke).

Speed (release build of manent-engine, warm, median of 3, release -> text):
| speech | EN | NL |
| 5 s | 0.25 s | 0.29 s |
| 30 s | 1.74 s | 1.78 s |
Clipboard and paste add < 50 ms. The engine is already running when the app is (load 0.5 s otherwise).

Harness note: GTK apps under test need GTK_IM_MODULE=gtk-im-context-simple, because GTK's Wayland input method waits for IBus, which the headless session doesn't run.

Decided with the user 2026-11-17: decision-11 accepted (Ctrl+V default paste key, Ctrl+Shift+V as a setting); dictation during a meeting recording is blocked with a notification.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Spike done on GNOME 51 in a headless, isolated GNOME session (spikes/dictation). Hold-to-talk works with the GlobalShortcuts portal (Activated on press, repeats while held, one Deactivated on release). The clipboard can be set from the background with wl-copy (2-5 ms focus trick, focus returns) or, with consent, through the Clipboard portal. Auto-paste works end to end through the RemoteDesktop portal: consent once ('Allow Remote Interaction' must be switched on), restore token works after a restart without a dialog, Ctrl+V via EIS (libei) pastes in 44 ms; Mutter's old NotifyKeyboard* doesn't reach apps. Ctrl+V works in GTK apps, browsers and LibreOffice, terminals need Ctrl+Shift+V. Transcription: ~0.3 s for 5 s and ~1.8 s for 30 s of speech; mic first samples ~150 ms after press. Decision-11 (accepted) records the approach and dependencies; VRT-58 and VRT-59 are updated. Open for VRT-59: Ptyxis' Ctrl+Shift+V on a real desktop, restoring the user's clipboard.
<!-- SECTION:FINAL_SUMMARY:END -->
