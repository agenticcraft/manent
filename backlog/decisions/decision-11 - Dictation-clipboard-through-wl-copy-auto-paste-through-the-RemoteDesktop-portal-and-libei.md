---
id: decision-11
title: >-
  Dictation: clipboard through wl-copy, auto-paste through the RemoteDesktop
  portal and libei
date: '2026-11-17T10:22:00Z'
status: accepted
---
## Context

Push-to-talk dictation (VRT-53 spike, VRT-58, VRT-59) on GNOME Wayland: hold a global shortcut, speak, release, and the text is ready where the cursor is. On Wayland an app can't set the clipboard or send keys to another app without focus, and GNOME has no data-control protocol. The spike (findings in VRT-53, probes in spikes/dictation) tried the options on GNOME 51.

## Decision

- **Hold-to-talk:** the GlobalShortcuts portal we already use (decision-10), with Deactivated for the release (ashpd 0.13, no new dependency). Repeats while held are ignored; a press under 0.3 s does nothing. The mic opens on press (first samples after ~150 ms), never while idle.
- **Clipboard (VRT-58, default, no consent needed):** Manent runs `wl-copy` (package wl-clipboard, an RPM `Requires`). It briefly takes focus with a tiny window (2-5 ms) and focus returns to the user's window. No Rust dependency. The GTK3-over-XWayland route also works but needs a second display connection in the app; xclip and native Wayland GTK without focus don't work.
- **Notifications:** the Notification portal through ashpd (feature `notification`, same crate).
- **Auto-paste (VRT-59, a setting, off by default):** the RemoteDesktop portal with the Clipboard portal (ashpd features `remote_desktop`, `clipboard`). The user allows it once ('Allow Remote Interaction' must be switched on in GNOME's dialog; 'Remember This Selection' keeps it); Manent stores the restore token after every start. With auto-paste on, the text goes on the clipboard through the portal (no focus trick) and Ctrl+V goes through the portal's EIS connection, which needs a libei client: the pure-Rust crate `reis` (new dependency, checked when VRT-59 starts). Mutter's older NotifyKeyboard* calls don't reach app windows.
- **During a meeting recording:** the dictation shortcut is blocked with a notification (the mic is already recorded and the engine busy with captions).
- **Paste key:** Ctrl+V by default (works in GTK apps, Firefox, Chrome, LibreOffice). Terminals need Ctrl+Shift+V (Ctrl+V types ^V); Ctrl+Shift+V opens Paste Special in LibreOffice. The app can't see which window is focused, so the key is a setting.

## Consequences

- New: the RPM requires wl-clipboard; ashpd gets the `notification`, `remote_desktop` and `clipboard` features; `reis` arrives with VRT-59.
- Dictation overwrites the user's clipboard. Restoring it afterwards is possible only through the Clipboard portal (auto-paste), so it belongs in VRT-59.
- X11 sessions, macOS and Windows need their own clipboard and paste code later (behind the same trait).
