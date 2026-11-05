---
id: VRT-40
title: Global hotkey to start and stop recording
status: Done
assignee: []
created_date: '2026-11-01T21:40:00Z'
updated_date: '2026-11-05T22:02:00Z'
labels:
  - ui
  - platform
milestone: m-3
dependencies: []
priority: medium
ordinal: 44000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Split from VRT-22 (tray first, 2026-11-01). Findings:
- **tauri-plugin-global-shortcut** uses X11 key grabs on Linux. Under GNOME Wayland (GNOME 50) it only sees keys while an X11 window has focus, so it does not work there. It does work on X11, macOS and Windows.
- **The Wayland way** is the XDG GlobalShortcuts portal (org.freedesktop.portal.GlobalShortcuts). It is available here (xdg-desktop-portal-gnome 50, interface version 1). The app binds a shortcut such as 'toggle-recording' with a preferred trigger (Ctrl+Alt+R); GNOME asks the user to confirm it once, and it can be changed in GNOME's keyboard settings. Using it needs a D-Bus client (ashpd, which uses zbus): a new dependency, so a decision first.
- **Fallback:** a command `veritas --toggle-recording` that tells the running app (tauri-plugin-single-instance), for desktops without the portal; the user binds it to a key in the desktop's settings.
The hotkey toggles recording exactly like the tray's 'Start/Stop recording' (tray::toggle_recording). Settings > Shortcuts shows the current trigger and how to change it.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Decision recorded for the dependencies (portal client on Wayland, plugin elsewhere)
- [x] #2 On GNOME Wayland the hotkey starts and stops a recording while the app is in the background
- [x] #3 Settings > Shortcuts shows the shortcut and lets the user change it (or opens the desktop's settings)
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Spike 2026-11-04 (spikes/shortcuts, ashpd 0.13): works on GNOME 50 Wayland once registered as host app ai.agenticcraft.manent with a matching desktop file; the first bind shows GNOME's dialog, later binds are silent; Ctrl+Alt+R / Ctrl+Alt+M activations arrive while another app has focus. Portal version 1 (no ConfigureShortcuts). Details and dependency choice: decision-10.

Verified by hand on the 0.1.5 RPM (2026-11-05): the shortcuts work on GNOME Wayland. Open: AC3's Change keys button (gnome-control-center applications ai.agenticcraft.manent) not yet confirmed.

Confirmed Change keys opens GNOME's settings for Manent (2026-11-05).
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Global shortcuts through the XDG GlobalShortcuts portal on Linux (ashpd, decision-10): Ctrl+Alt+R starts or stops a recording like the tray, Ctrl+Alt+M marks a moment (VRT-52), also with Manent in the background. Off until Settings > Shortcuts > Turn on, when GNOME asks once; later starts bind silently. Settings shows the bound keys and Change keys opens GNOME's settings. The RPM installs ai.agenticcraft.manent.desktop (the app id the portal needs) and hides Tauri's Manent.desktop. Repeated key events within 700 ms count once. macOS and Windows report shortcuts as unavailable for now. Tested: unit tests, smoke-marks.py --bind on a debug build, and by hand on the 0.1.5 RPM.
<!-- SECTION:FINAL_SUMMARY:END -->
