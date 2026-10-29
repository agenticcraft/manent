---
id: VRT-7
title: Library screen
status: To Do
assignee: []
created_date: 2026-10-29 12:17
updated_date: '2026-10-29T21:58:00Z'
labels:
  - frontend
milestone: m-1
dependencies:
  - VRT-3
  - VRT-6
documentation:
  - doc-3
priority: medium
ordinal: 7000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Searchable list of meetings (title, date, duration, speaker avatars) with 'New recording' and 'Import audio' actions, in the clean UI style. See doc-3 UI design.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Meetings listed from DB, newest first
- [ ] #2 Title search filters the list
- [ ] #3 Empty state with clear call to action
- [ ] #4 Works in light and dark mode
- [ ] #5 A meeting can be deleted, which removes its audio files and DB rows
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
From VRT-6: lib.rs has a temporary #[allow(dead_code)] on mod db because only tests use the query functions; remove it once the Library uses them.
<!-- SECTION:NOTES:END -->
