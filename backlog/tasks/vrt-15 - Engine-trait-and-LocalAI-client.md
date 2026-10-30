---
id: VRT-15
title: Engine trait and libparakeet helper client
status: To Do
assignee: []
created_date: 2026-10-29 12:17
updated_date: '2026-10-30T09:02:00Z'
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
Rust Engine trait (transcribe_chunk, transcribe_file, speaker_profiles) implemented over the veritas-engine helper (VRT-14). Calls (doc-2 §1.3): transcribe_pcm_batch_json for 4-8 s live chunks, transcribe_path_json_vad_with (Silero vad component) for files and rolling passes, diarize_profiles_pcm_json for speaker segments and embeddings. Never transcribe_and_diarize (drops speech, 9 GB peak; VRT-29). The app merges words and speakers by overlap. No language parameter: parakeet 0.6B v3 detects Dutch/English itself (VRT-32). Use parakeet_capi_set_concurrency so live chunks don't queue behind a rolling pass.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 transcribe_file returns segments with word timestamps and speaker labels
- [ ] #2 speaker_profiles returns per-speaker embeddings, clean seconds and the encoder id
- [ ] #3 Integration tests run a Dutch and an English fixture through the engine
- [ ] #4 transcribe_chunk returns text with word timestamps for a 4-8 s chunk in under 1 s
<!-- AC:END -->
