---
id: VRT-8
title: Live recording screen
status: To Do
assignee: []
created_date: '2026-10-29 12:17'
updated_date: '2026-10-29 12:18'
labels:
  - frontend
milestone: m-1
dependencies:
  - VRT-5
  - VRT-6
  - VRT-7
documentation:
  - doc-3
priority: high
ordinal: 8000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Recording view: editable title, timer (tabular numerals), per-source chips with mini level meter and mute toggle, Pause/Stop, soft red recording pulse. Transcript area is a placeholder until Phase 2. See doc-3 UI design.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Start creates a meeting and begins recording the configured sources
- [ ] #2 Level meters animate per source; mute toggles a source
- [ ] #3 Pause/Resume/Stop work and the meeting appears in the Library
- [ ] #4 Title is editable and persisted
<!-- AC:END -->
