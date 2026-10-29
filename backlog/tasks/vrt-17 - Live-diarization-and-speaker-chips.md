---
id: VRT-17
title: Live diarization and speaker chips
status: To Do
assignee: []
created_date: '2026-10-29 12:17'
updated_date: '2026-10-29 12:19'
labels:
  - transcription
  - frontend
milestone: m-2
dependencies:
  - VRT-16
documentation:
  - doc-2
priority: high
ordinal: 17000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Diarize the system track; label the mic track as 'Me' by default. Resolve mic bleed by preferring the system-track label on overlap. Render speaker blocks with colored name chips in the live transcript. See doc-2 Technical architecture §1.2, §1.4.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Remote speakers get distinct labels (Speaker 1, 2, ...)
- [ ] #2 Mic speech is labelled 'Me' (renamable)
- [ ] #3 Overlap/bleed logic covered by unit tests
- [ ] #4 Current speaker block highlighted in the UI
- [ ] #5 Verified by playing a two-person podcast clip while speaking: at least three distinct speakers within ~1-2 s
<!-- AC:END -->
