---
id: VRT-15
title: Engine trait and LocalAI client
status: To Do
assignee: []
created_date: '2026-10-29 12:17'
updated_date: '2026-10-29 12:19'
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
Rust Engine trait (transcribe_stream, transcribe_file, enroll_speaker) with a LocalAI implementation (realtime/streaming or VAD-chunked per spike outcome, plus file diarization). Keeps the door open for a direct parakeet.cpp FFI engine later. See doc-2 Technical architecture §1.3.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 transcribe_stream yields partial and final segments with timestamps and speaker label
- [ ] #2 transcribe_file returns segments with word timestamps and speakers
- [ ] #3 Integration test runs a fixture WAV through the engine
<!-- AC:END -->
