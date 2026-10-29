---
id: VRT-15
title: Engine trait and LocalAI client
status: To Do
assignee: []
created_date: 2026-10-29 12:17
updated_date: '2026-10-29T18:45:00Z'
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
Rust Engine trait (transcribe_chunk, transcribe_file, speaker_profiles) with a LocalAI implementation over the file APIs: /v1/audio/transcriptions (verbose_json, word timestamps) for live chunks and offline passes, /v1/audio/diarization with include_speaker_profiles for speakers and embeddings. Audio is sent as FLAC. No language parameter: parakeet 0.6B v3 detects Dutch/English itself (VRT-32). No realtime WebSocket. Keeps the door open for a direct parakeet.cpp FFI engine later. See doc-2 Technical architecture §1.3.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 transcribe_chunk returns text with word timestamps for a 4-8 s chunk in about 1 s
- [ ] #2 transcribe_file returns segments with word timestamps and speaker labels
- [ ] #3 speaker_profiles returns per-speaker embeddings, clean seconds and the encoder id
- [ ] #4 Integration tests run a Dutch and an English fixture through the engine
<!-- AC:END -->
