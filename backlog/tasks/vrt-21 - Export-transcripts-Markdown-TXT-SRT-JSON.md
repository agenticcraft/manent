---
id: VRT-21
title: Export transcripts (Markdown, TXT, SRT, JSON)
status: To Do
assignee: []
created_date: 2026-10-29 12:17
updated_date: '2026-11-01T09:53:00Z'
labels:
  - frontend
  - backend
milestone: m-2
dependencies:
  - VRT-20
documentation:
  - doc-1
priority: medium
ordinal: 21000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Export a meeting's final transcript from the meeting screen (doc-3) in Markdown, TXT, SRT and JSON. Speakers as shown in the app: the person's name, 'Speaker N', and the user's own name for the mic (setting speaker.me_name). Timestamps from the segments (track time = playback time). SRT: one cue per block, split into lines of at most about 7 s using the word times. JSON: blocks with their words (start/end/conf), so other tools can use the timing. Written through the dialog plugin's save dialog (dialog:allow-save, decision-3).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 All four formats export with speaker names and timestamps
- [ ] #2 Exports covered by snapshot tests
<!-- AC:END -->
