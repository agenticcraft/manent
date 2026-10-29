---
id: VRT-9
title: 'Settings: audio sources'
status: To Do
assignee: []
created_date: '2026-10-29 12:17'
updated_date: '2026-10-29 12:18'
labels:
  - frontend
  - audio
milestone: m-1
dependencies:
  - VRT-4
  - VRT-7
documentation:
  - doc-3
priority: medium
ordinal: 9000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Settings > Sources: pick microphone and output device to capture, test meters, and 'auto-detect default device'. Persist choices. See doc-3 UI design.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Device pickers list mic and loopback sources
- [ ] #2 Test meter shows live level for the selected device
- [ ] #3 Auto-detect default device follows OS default changes
- [ ] #4 Choices persisted in settings table
<!-- AC:END -->
