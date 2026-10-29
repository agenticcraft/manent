---
id: VRT-6
title: SQLite storage with migrations
status: To Do
assignee: []
created_date: '2026-10-29 12:17'
updated_date: '2026-10-29 12:19'
labels:
  - backend
  - storage
milestone: m-1
dependencies:
  - VRT-3
documentation:
  - doc-2
priority: high
ordinal: 6000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Add sqlx + SQLite in the OS app-data folder with migrations for meetings, tracks, segments, speakers and settings, plus an FTS5 index over segment text. See doc-2 Technical architecture §1.5.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Migrations run on app start
- [ ] #2 FTS5 virtual table for segment text
- [ ] #3 Query functions covered by unit tests
- [ ] #4 Tables meetings, tracks, segments, speakers, settings exist as in doc-2 §1.5
<!-- AC:END -->
