---
id: VRT-22
title: Tray icon
status: Done
assignee:
  - '@claude'
created_date: 2026-10-29 12:17
updated_date: '2026-11-01T21:41:00Z'
labels:
  - desktop
  - frontend
milestone: m-3
dependencies:
  - VRT-8
documentation:
  - doc-1
priority: medium
ordinal: 22000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Tray icon showing the recording state, with Start/Stop recording, Show Veritas and Quit. Closing the window during a recording hides it to the tray. The global hotkey moved to VRT-40 (the user chose tray first; the standard plugin doesn't work on GNOME Wayland).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Tray reflects recording state and can start/stop
- [x] #2 Closing the window during a recording hides it and the recording goes on; Quit finishes a running recording first
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Built:
- **Tray:** src-tauri/src/tray.rs, using Tauri's tray-icon and image-png features (no new crate).
  - On Linux it is an AppIndicator; GNOME shows it with the AppIndicator extension.
  - Icons: icons/tray.png and icons/tray-recording.png (the app icon with a red dot).
  - Menu: Start/Stop recording, Show Veritas, Quit Veritas.
  - Start and stop call the same commands as the window. start_recording and stop_recording update the tray and emit recording:started / recording:stopped, and App.tsx follows them.
  - Live saves a title still being typed when it unmounts, so a stop from the tray keeps it.
- **Closing the window during a recording** hides it, but only when the tray exists. Otherwise the window closes as before.
- **Quit** stops a running recording first (complete files, then the final pass at the next start).
- **Not done:** 'start minimized to tray', from the original description. Nothing asked for it beyond the tray itself.
- **Tests:** scripts/smoke-tray.py PASS. It drives the menu over D-Bus (com.canonical.dbusmenu) like the panel does, and closes the window like the title bar button does (wmctrl, needs GDK_BACKEND=x11).
- **Found:** a killed app leaves the meeting in 'recording' with unfinished FLAC files. Ticketed as VRT-41.
- **Checks:** clippy 0, 112 tests, npm check green.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
The tray shows when Veritas records, starts and stops recordings, keeps a recording going when the window is closed, and finishes it properly on Quit. The hotkey is VRT-40; crash recovery of recordings is VRT-41.
<!-- SECTION:FINAL_SUMMARY:END -->
