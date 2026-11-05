---
id: VRT-51
title: Windows installer with libparakeet built from source
status: To Do
assignee: []
created_date: '2026-11-04T21:27:00Z'
labels:
  - packaging
milestone: m-3
dependencies:
  - VRT-11
  - VRT-33
priority: low
ordinal: 55000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Split from VRT-23. LocalAI publishes no Windows build of libparakeet, so parakeet.cpp (CMake) is built from source and served as a verified download from our own mirror (VRT-33), like the Linux engine. The installer (MSI or NSIS) holds the app and the engine helper only (decision-1); signing avoids SmartScreen warnings.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 libparakeet for Windows is built from the pinned parakeet.cpp source and served with its SHA-256; the engine integration tests pass with it on Windows
- [ ] #2 A signed Windows installer with the app and the engine helper installs, downloads the engine and transcribes
- [ ] #3 How Windows users get updates is decided (Backlog decision) and works from one release to the next
<!-- AC:END -->
