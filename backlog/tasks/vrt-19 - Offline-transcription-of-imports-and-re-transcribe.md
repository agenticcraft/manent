---
id: VRT-19
title: Offline transcription of imports and re-transcribe
status: To Do
assignee: []
created_date: 2026-10-29 12:17
updated_date: '2026-10-29T17:05:00Z'
labels:
  - transcription
  - backend
  - frontend
milestone: m-2
dependencies:
  - VRT-6
  - VRT-15
  - VRT-18
documentation:
  - doc-2
priority: high
ordinal: 19000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Drag-and-drop or import wav/mp3/m4a/mp4 and transcribe with diarization via the file API (vad:true for long audio), and 're-transcribe' existing meetings. Uses the multilingual offline model (parakeet-cpp-bundle-standard or the VRT-32 choice) so Dutch, English and mixed recordings work. Runs in a job queue with progress. See doc-2 Technical architecture §1.3.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Imported file becomes a meeting with a speaker-labelled transcript
- [ ] #2 Re-transcribe replaces segments of an existing meeting
- [ ] #3 Job progress visible; jobs survive navigation
- [ ] #4 Known speakers are recognized in imports
- [ ] #5 Dutch, English and mixed NL/EN recordings transcribe correctly
<!-- AC:END -->
