---
id: VRT-15
title: Engine trait and LocalAI client
status: To Do
assignee: []
created_date: 2026-10-29 12:17
updated_date: '2026-10-29T17:05:00Z'
labels:
  - transcription
  - backend
milestone: m-2
dependencies:
  - VRT-14
documentation:
  - doc-2
priority: high
ordinal: 15000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Rust Engine trait (transcribe_stream, transcribe_file, enroll_speaker) with a LocalAI implementation: realtime WebSocket transcription sessions for live (per VRT-2/VRT-32: semantic_vad streaming models or VAD-chunked fallback) and the file APIs for offline transcription and speaker profiles. Every call takes a language parameter (auto, nl, en) so Dutch, English and mixed meetings are handled; the engine picks the right model per language. Keeps the door open for a direct parakeet.cpp FFI engine later. See doc-2 Technical architecture §1.3.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 transcribe_stream yields partial and final segments with timestamps and speaker label
- [ ] #2 transcribe_file returns segments with word timestamps and speakers
- [ ] #3 Language parameter (auto, nl, en) is passed through and selects the model per VRT-32
- [ ] #4 Integration tests run a Dutch and an English fixture WAV through the engine
<!-- AC:END -->
