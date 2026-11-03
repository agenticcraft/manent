---
id: VRT-43
title: App and tray icon
status: Done
assignee:
  - '@claude'
created_date: '2026-11-02T17:29:00Z'
updated_date: '2026-11-03T20:13:00Z'
labels:
  - ui
  - design
milestone: m-3
dependencies: []
priority: low
ordinal: 47000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Split from VRT-42. The app icon is still the Tauri default. The tray icon is that icon with a red dot (VRT-22). Design a recognisable Veritas mark that works at 16-22 px on light and dark panels, as a tray icon (idle and recording) and as the app icon (Linux .png sizes, Windows .ico, macOS .icns). Calm and neutral, with the one accent (doc-3). Propose two or three directions to the user first.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Two or three directions shown to the user; one chosen
- [x] #2 App icon in every size Tauri needs; tray icons for idle and recording that read on light and dark panels
- [x] #3 Checked in the Linux panel (GNOME 50, AppIndicator extension, Dash to Panel): icon and menu readable
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Three directions drawn as SVG (scratchpad sheet sent to the user):
- A: a waveform turning into text lines;
- B: an M as a sound wave;
- C: a speech bubble with text lines.

Option B was chosen on 2026-11-03.

- **Files:** src-tauri/icons/source/manent.svg and manent-recording.svg (the tray variant with a red dot and a white ring).
- **Generated:** 'npx tauri icon' from a 1024 px render makes the PNG sizes, icon.ico, icon.icns, 64x64 and the Square*/Store logos. The Android and iOS icons it also makes were removed until the companion's platform is chosen (VRT-26.1); they can be generated again the same way.
- **Tray:** tray.png and tray-recording.png at 64 px.
- **Tests:** smoke-tray.py PASS (the icon changes while recording).
- **Docs:** doc-3 has the icon decision.
- **App switcher:** in an unpackaged dev build GNOME takes the switcher icon from a .desktop file, which comes with packaging (VRT-23).

Checked in the panel on 2026-11-03: the icon looks good. The menu showed empty: a race in GNOME's AppIndicator extension. The first LayoutUpdated, sent as the tray registers, arrives before the extension's D-Bus proxy is set up ('JS ERROR: Invalid value undefined for property parent' in CancellableChild); the extension never asks for the menu again. Blueman isn't affected (its menu exists before it registers). Fix in tray.rs (Linux only): set the menu again 3 s after start, which sends a new LayoutUpdated that the extension handles. Confirmed by the user, and smoke-tray PASS. The app-switcher icon needs the .desktop file of a packaged build; moved to VRT-23.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Manent has its own icon, an M drawn as a sound wave on an indigo tile, in every size the app, installers and tray need. The tray menu now also shows in GNOME's AppIndicator extension, working around a race in it.
<!-- SECTION:FINAL_SUMMARY:END -->
