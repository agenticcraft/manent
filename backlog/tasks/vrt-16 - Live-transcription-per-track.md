---
id: VRT-16
title: Live transcription per track
status: To Do
assignee: []
created_date: 2026-10-29 12:17
updated_date: '2026-10-29T18:45:00Z'
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
Live captions per track with app-side chunking (validated in VRT-32): resample each source to 16 kHz mono (rubato), cut at the quietest 20 ms frame between 4 and 8 s after the previous cut, send each chunk to the engine, and stream the text to the Live screen (light grey, auto-scroll with 'Jump to live'). Works for Dutch, English and meetings that switch between them, with automatic language detection. The rolling and final passes that add speakers are VRT-17. See doc-2 Technical architecture §1.2, §1.3 and doc-3 UI design.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Text appears within ~4 s (median) and at most ~8 s of speech
- [ ] #2 Caption chunks persisted with track and timestamps until a rolling pass replaces them
- [ ] #3 Chunker and resampler unit-tested (cut point within 4-8 s, at the quietest frame)
- [ ] #4 Dutch and English transcribed live, including a meeting that switches between them
<!-- AC:END -->
