---
id: VRT-46
title: Rename a meeting from its page
status: Done
assignee: []
created_date: '2026-11-04T10:43:00Z'
updated_date: '2026-11-04T10:46:00Z'
labels:
  - ux
dependencies: []
priority: medium
ordinal: 50000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found while testing the RPM: titles can only be set while recording (the Live title field); afterwards a meeting keeps its automatic or typed title. rename_meeting exists already (also rewrites transcript.md).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 On the meeting page the title is editable in place: click it (or Tab + Enter), type, Enter or leaving the field saves, Esc cancels; an empty title goes back to the automatic one
- [x] #2 The Library row, search hits and transcript.md (and so Nextcloud) show the new title
- [x] #3 A smoke script renames a meeting and checks it after a restart
<!-- AC:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
The meeting page title is a button (pencil on hover/focus) that turns into an input: Enter or blur saves via rename_meeting (which rewrites transcript.md and so wakes Nextcloud sync), Esc cancels, an empty title falls back to the automatic one. The title column now takes the free width. smoke-library.py renames a meeting, checks Esc, transcript.md, the Library row, a restart and clearing the title; screenshots light and dark.
<!-- SECTION:FINAL_SUMMARY:END -->
