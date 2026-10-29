---
id: VRT-16
title: Live transcription per track
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
  - VRT-5
  - VRT-8
  - VRT-15
documentation:
  - doc-2
  - doc-3
priority: high
ordinal: 16000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Resample each source to 16 kHz mono (rubato), feed 20-40 ms chunks into one engine session per track, store final segments in the DB and stream partial/final segments to the Live screen (partials in lighter grey, auto-scroll with 'Jump to live'). See doc-2 Technical architecture §1.2 and doc-3 UI design.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Transcript appears within ~1-2 s of speech
- [ ] #2 Dutch and English both transcribed live
- [ ] #3 Final segments persisted with track, timestamps and words
- [ ] #4 Resampler unit-tested
<!-- AC:END -->
