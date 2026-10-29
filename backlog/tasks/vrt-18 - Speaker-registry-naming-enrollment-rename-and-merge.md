---
id: VRT-18
title: 'Speaker registry: naming, enrollment, rename and merge'
status: To Do
assignee: []
created_date: '2026-10-29 12:17'
updated_date: '2026-10-29 12:19'
labels:
  - transcription
  - frontend
  - backend
milestone: m-2
dependencies:
  - VRT-17
documentation:
  - doc-2
  - doc-3
priority: high
ordinal: 18000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
App-owned speaker registry: click a speaker chip to name it, enroll an embedding from 5-10 s of their audio, re-register known speakers with LocalAI on engine start, rename/merge applied retroactively, plus the Speakers screen (rename, merge, delete voice data). See doc-2 Technical architecture §1.4 and doc-3 UI design.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Naming a speaker updates all segments in the meeting
- [ ] #2 Named speaker is recognized automatically after an app restart
- [ ] #3 Two speakers can be merged
- [ ] #4 Deleting a person removes their voice embedding
<!-- AC:END -->
