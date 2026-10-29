---
id: VRT-22
title: Tray icon and global hotkey
status: To Do
assignee: []
created_date: '2026-10-29 12:17'
updated_date: '2026-10-29 12:18'
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
Tray icon showing recording state with start/stop and open-app actions; configurable global hotkey (default Ctrl+Alt+R) to toggle recording; option to start minimized to tray. Uses Tauri tray-icon and global-shortcut plugin.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Tray reflects recording state and can start/stop
- [ ] #2 Global hotkey toggles recording while the app is in background
- [ ] #3 Hotkey configurable in Settings > Shortcuts
<!-- AC:END -->
