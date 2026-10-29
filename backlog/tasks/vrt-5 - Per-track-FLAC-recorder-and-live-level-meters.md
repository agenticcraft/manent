---
id: VRT-5
title: Per-track FLAC recorder and live level meters
status: To Do
assignee: []
created_date: '2026-10-29 12:17'
updated_date: '2026-10-29 12:19'
labels:
  - audio
  - backend
milestone: m-1
dependencies:
  - VRT-4
documentation:
  - doc-2
priority: high
ordinal: 5000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Consume each source's ring buffer and write its own FLAC track at the native sample rate into meetings/<id>/; emit RMS/peak level events to the UI at ~20 Hz. Support pause/resume. See doc-2 Technical architecture §1.2, §1.5.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 mic.flac and system.flac written per meeting
- [ ] #2 Pause/resume works without corrupting files
- [ ] #3 Level events emitted at ~20 Hz per source
- [ ] #4 5-minute recording has no dropouts; capture CPU < 5%
<!-- AC:END -->
