---
id: VRT-50
title: 'macOS installer: signed and notarized dmg'
status: To Do
assignee: []
created_date: '2026-11-04T21:27:00Z'
labels:
  - packaging
milestone: m-3
dependencies:
  - VRT-12
priority: low
ordinal: 54000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Split from VRT-23, which shipped Fedora first (decision-9: an RPM from our own signed dnf repository). macOS needs a dmg signed with a Developer ID and notarized, or Gatekeeper blocks it; the engine helper is signed along. Updates: decide between the Tauri updater and a download page (decision-9 chose no in-app updater on Linux, where dnf updates).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A dmg with the app and the engine helper, signed with a Developer ID and notarized, installs and opens without Gatekeeper warnings on a clean Mac
- [ ] #2 How macOS users get updates is decided (Backlog decision) and works from one release to the next
<!-- AC:END -->
