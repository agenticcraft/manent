---
id: VRT-59
title: 'Option: paste dictated text automatically'
status: Done
assignee:
  - '@claude'
created_date: '2026-11-17T09:52:00Z'
updated_date: '2026-11-17T20:26:00Z'
labels:
  - feature
milestone: m-4
dependencies:
  - VRT-58
priority: low
ordinal: 63000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
A setting, off by default, under the dictation setting (built on VRT-58, findings in VRT-53, decision-11): after dictation Manent pastes the text where the cursor is. It uses the RemoteDesktop portal with the Clipboard portal: the text goes on the clipboard through the portal and Ctrl+V goes through the portal's EIS connection (libei client: the reis crate; Mutter's NotifyKeyboard* calls don't reach app windows). GNOME asks once; in that dialog 'Allow Remote Interaction' is off by default and must be switched on, so the settings text says so. The restore token comes back new after every start and must be stored each time.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 With the setting on, released dictation is typed into the focused field (browser field, LibreOffice, terminal) once the shortcut's modifiers are let go; GNOME asks only the first time, also after a restart; a session that ended is started again with the token
- [x] #2 Turning the setting off ends the portal session and forgets the restore token; if the user refuses the portal, the setting turns itself off with a short explanation and dictation keeps working via the clipboard
- [x] #3 Smoke script and screenshots in light and dark
- [x] #4 Characters the keyboard layout can't type are pasted with the paste key setting: Ctrl+V (default; GTK apps, browsers, LibreOffice) or Ctrl+Shift+V (terminals); checked on a real desktop in Ptyxis, Firefox and LibreOffice
- [x] #5 Typed text leaves the clipboard alone; when characters were pasted, the user's previous clipboard text is restored (read through the Clipboard portal), or the reason it isn't is documented
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. reis 0.7.1 (tokio) + ashpd remote_desktop/clipboard (+screencast, needed to compile remote_desktop).
2. dictation/autopaste.rs: RemoteDesktop session (keyboard, persist until revoked, restore token), Clipboard portal requested before Start, EIS keyboard via reis on its own thread, tasks serving SelectionTransfer and following SelectionOwnerChanged.
3. Paste: read the previous text (SelectionRead), offer the dictated text, Ctrl+V / Ctrl+Shift+V through EIS, wait until the app read it, offer the previous text again.
4. Settings: Paste automatically (on/off, the desktop asks once), Paste with (Ctrl+V / Ctrl+Shift+V); token stored after every start, forgotten when off; a refused dialog turns it off with a reason.
5. Smoke in the isolated session (GTK field, alacritty, restart, refuse); real desktop check by the user in Ptyxis, Firefox, LibreOffice.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Implemented as planned.
- reis' tokio event stream isn't Send (callback map), so the EIS connection (handshake + events) runs on its own thread with a current-thread runtime; the paste itself only sends key events through the shared device and flushes.
- ashpd's remote_desktop module needs its screencast feature to compile (no screen is shared).
- Clipboard signal streams borrow their proxy, so each task gets its own Clipboard proxy and reports when it is subscribed.
- Restore: only text can be put back (SelectionRead of text/plain;charset=utf-8, text/plain or UTF8_STRING). If the clipboard held no text (an image, or empty), the dictated text stays on it. If no app asks for the text within 1.5 s (no text field focused), it stays on the clipboard and the 'Ready to paste' notification shows. The restored text is served by Manent's portal session, so it is gone if Manent quits.
- On success there is no notification; the text appears where the cursor is.
- If pasting fails (session gone), dictation falls back to wl-copy with the 'Ready to paste' notification.
- Smoke (isolated GNOME session, scripts/smoke-dictation.py): refusing the dialog -> off with 'Pasting wasn't allowed...'; accepting (Allow Remote Interaction switched on) -> on, token stored; Ctrl+V into a GTK4 text field + the earlier clipboard ('earlier clipboard é') back; Ctrl+Shift+V into alacritty; after a restart on without a dialog and pastes; off -> token null. All checks pass.

Typing instead of pasting (user feedback: pasting on release was unreliable): the log showed the session ended (keyboard disconnected) and every later paste failed with 'Invalid session', and Ctrl+V was sent while Ctrl+Shift of the shortcut were still down (in LibreOffice that is Paste Special). Mutter 51 has no ei_text (libmutter imports no eis text functions), so text goes as key strokes: keys.rs compiles the xkb keymap the EIS keyboard comes with (crate xkbcommon 0.9, approved; system libxkbcommon) and maps each character to a key plus modifier keys (fewest modifiers, lowest key; real keys over <LVL3>/<MDSW>), with the current group and locked modifiers (Caps Lock), rebuilt when they change. Mutter sends ei_keyboard.modifiers with the seat's held modifiers: typing waits (max 3 s) until none are down. Runs the layout can't type (é on US) are pasted through the Clipboard portal and the previous clipboard text is put back once at the end. 3 ms between characters. A dead session (EIS closed) is started again with the token before the next paste, and once more after a failure. Smoke: typed into GTK with Ctrl+Shift held 1.5 s after Space, clipboard untouched; typed into alacritty; Russian layout: latin letters pasted, text identical, clipboard restored; after restart typed. Mutter replaces EIS keyboards about 1 s after a layout change (the test keyboard follows it). Known: an app that matches shortcuts by keysym (alacritty) doesn't see Ctrl+Shift+V as paste on a non-latin layout.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Auto-type after dictation (Settings > Shortcuts > Dictation > Type automatically, off by default, Linux). A RemoteDesktop portal session (GNOME asks once; restore token persisted, forgotten on turn-off) gives an EIS keyboard (reis); the text is typed with the desktop's xkb keymap (xkbcommon) once the shortcut's modifiers are released. Characters the layout can't type are pasted through the Clipboard portal with the chosen paste key (Ctrl+V / Ctrl+Shift+V) and the previous clipboard text is restored. An ended session is restarted with the token. Typing replaced the first paste-only version after user feedback (paste fired with Ctrl+Shift still held; a dropped session was not restarted). Verified: smoke-dictation.py in the isolated GNOME session (GTK field with modifiers held, alacritty, Russian layout with pasted letters and clipboard restore, restart, refuse/turn off), unit tests for the key mapping (us, Caps Lock, altgr-intl, fr, runs), and by the user on the real desktop in Ptyxis, Firefox and LibreOffice. Known: apps matching shortcuts by keysym (alacritty) don't paste with a non-latin layout.
<!-- SECTION:FINAL_SUMMARY:END -->
