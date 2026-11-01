---
id: VRT-26.4
title: 'Mobile app: transcripts on the phone, with search'
status: To Do
assignee: []
created_date: '2026-11-01T10:07:00Z'
updated_date: '2026-11-01T11:55:00Z'
labels:
  - mobile
  - frontend
milestone: m-4
dependencies:
  - VRT-26.3
parent_task_id: VRT-26
priority: low
ordinal: 38000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Show on the phone what the desktop made of its recordings (decision-4, decision-5): the meeting list and each transcript with speaker names and timestamps, read-only; naming speakers stays on the desktop. The transcripts are the transcript.md files next to the audio (VRT-37: YAML header for the meeting list, a block per speaker turn with its time to seek to), read from the user's Nextcloud folder (or over a paired connection). The phone keeps them for reading offline and indexes them for search across meetings (e.g. SQLite FTS on the phone), and re-downloads one when its ETag changes (a speaker named on the desktop). Which meetings: the phone's own recordings, or all meetings the desktop syncs (VRT-39). Playback of the audio with click-to-seek like the desktop (VRT-20), streamed or downloaded on demand. Everything can be removed from the phone.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Transcripts appear on the phone with speakers and timestamps, from transcript.md in Nextcloud
- [ ] #2 Search across all synced transcripts, offline
- [ ] #3 Updates on the desktop (naming, re-transcribe) reach the phone
- [ ] #4 Readable offline; removable from the phone
<!-- AC:END -->
