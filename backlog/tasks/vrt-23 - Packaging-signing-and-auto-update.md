---
id: VRT-23
title: 'Packaging, signing and auto-update'
status: To Do
assignee: []
created_date: '2026-10-29 12:17'
updated_date: '2026-10-29 12:18'
labels:
  - release
milestone: m-3
dependencies:
  - VRT-13
  - VRT-14
documentation:
  - doc-1
priority: low
ordinal: 23000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Produce installers for Linux (AppImage/rpm/deb), Windows (MSI/NSIS) and macOS (dmg), with code signing/notarization and Tauri updater, including the local-ai sidecar per target triple.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Installers build in CI for all three platforms
- [ ] #2 macOS build signed and notarized
- [ ] #3 Auto-update works from one release to the next
<!-- AC:END -->
