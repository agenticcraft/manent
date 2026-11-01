---
id: VRT-40
title: Global hotkey to start and stop recording
status: To Do
assignee: []
created_date: '2026-11-01T21:40:00Z'
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
- [ ] #1 Decision recorded for the dependencies (portal client on Wayland, plugin elsewhere)
- [ ] #2 On GNOME Wayland the hotkey starts and stops a recording while the app is in the background
- [ ] #3 Settings > Shortcuts shows the shortcut and lets the user change it (or opens the desktop's settings)
<!-- AC:END -->
