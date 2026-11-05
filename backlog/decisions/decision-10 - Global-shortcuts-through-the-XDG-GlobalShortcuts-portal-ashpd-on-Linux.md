---
id: decision-10
title: Global shortcuts through the XDG GlobalShortcuts portal (ashpd) on Linux
date: '2026-11-04T21:31:00Z'
status: accepted
---
## Context

VRT-40 (a global hotkey to start and stop recording) and VRT-52 (marking a moment, also with a hotkey). The user's desktop is GNOME 50 on Wayland. tauri-plugin-global-shortcut uses X11 key grabs, which only see keys while an X11 window has focus, so it does not work there. The Wayland way is the XDG GlobalShortcuts portal (xdg-desktop-portal 1.22, -gnome 50, interface version 1 here).

## Decision

On Linux, use the portal through **ashpd 0.13** (feature `global_shortcuts`; it uses zbus 5, already in the tree through keyring), as a Linux-only dependency. The app registers itself as host app `ai.agenticcraft.manent` (org.freedesktop.host.portal.Registry), creates a session and binds `toggle-recording` (preferred Ctrl+Alt+R) and `mark-moment` (Ctrl+Alt+M); GNOME asks the user once and keeps the binding, which the user changes in GNOME Settings > Apps > Manent. macOS and Windows get tauri-plugin-global-shortcut when their audio code lands (VRT-11, VRT-12).

Spike findings (2026-11-04, spikes/shortcuts):
- Without an app id the portal refuses ("An app id is required"). Registering needs a desktop file named after the id: `ai.agenticcraft.manent.desktop`. Tauri's bundler always names it `Manent.desktop`, so the RPM ships `ai.agenticcraft.manent.desktop` itself and the generated one is hidden (`NoDisplay=true`, desktopTemplate).
- The first BindShortcuts shows GNOME's "Add Keyboard Shortcuts" dialog; later binds return the stored triggers without a dialog. Activations arrive while other apps have focus.
- ConfigureShortcuts needs interface version 2; here it is version 1, so the app opens GNOME's settings instead.

## Consequences

- One new direct dependency (ashpd); no X11 fallback on Linux.
- A development build has no desktop file of its own: shortcuts work only when `ai.agenticcraft.manent.desktop` is installed (the RPM, or a copy in ~/.local/share/applications for testing); otherwise Settings says they are unavailable.
- The dialog only appears when the user turns shortcuts on in Settings, never by surprise at start.
