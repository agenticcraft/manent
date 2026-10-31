---
id: VRT-15
title: Engine trait and libparakeet helper client
status: Done
assignee:
  - '@claude'
created_date: 2026-10-29 12:17
updated_date: '2026-10-30T09:40:00Z'
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
Rust Engine trait (transcribe_chunk, transcribe_file, speaker_profiles) implemented over the veritas-engine helper (VRT-14). Every call hands the helper a 16 kHz mono WAV path: live chunks are written to <app data>/tmp and use the same transcribe call as files (transcribe_path_json_vad_with; audio up to 30 s is transcribed whole), files and rolling passes use it with the Silero vad component for longer audio, and diarize_profiles_pcm_json gives speaker turns and embeddings. Never transcribe_and_diarize (drops speech, 9 GB peak; VRT-29). The app merges words and speakers by overlap. No language parameter: parakeet 0.6B v3 detects Dutch/English itself (VRT-32). The helper sets parakeet_capi_set_concurrency (2 ASR backends) so live chunks don't queue behind a rolling pass.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 transcribe_file returns segments with word timestamps and speaker labels
- [x] #2 speaker_profiles returns per-speaker embeddings, clean seconds and the encoder id
- [x] #3 Integration tests run a Dutch and an English fixture through the engine
- [x] #4 transcribe_chunk returns text with word timestamps for a 4-8 s chunk in under 1 s
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. engine/transcript.rs: Word/Transcript/SpeakerTurn/SpeakerProfile/Diarization/Segment, parsing of the helper documents, merge (max overlap, else nearest turn; new block on speaker change or 1.5 s pause); unit tests.
2. engine/mod.rs: Engine trait (async, 16 kHz mono) implemented for EngineManager; chunk -> tmp WAV (hound); transcribe_file runs transcribe and diarize in parallel and merges.
3. Fixtures src-tauri/tests/fixtures (FLEURS CC-BY-4.0: Dutch clip, English clip, two-speaker English) and #[ignore] integration tests against the real helper (VERITAS_TEST_ENGINE_DATA).
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Results (2026-10-30, debug test build, release libparakeet): transcribe_chunk English 6.0 s chunk 0.51 s WER 0%, Dutch 10.7 s 0.89 s WER 3%; transcribe_file on the 41 s two-speaker fixture 4.2 s, 4 blocks with the right alternating speakers, WER 6.6% (same as the spike); speaker_profiles: 2 usable profiles, 256-dim, norm 1, clean > 5 s, encoder sha256:72040372... Chunks go through a tmp WAV instead of transcribe_pcm_batch_json: same speed, one protocol (doc-2 updated). Run: VERITAS_TEST_ENGINE_DATA=<data dir> cargo test -p veritas -- --ignored engine_tests --test-threads 1. Merge unit tests: overlap, nearest turn, pause split, no diarization.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Engine trait over the helper: transcribe_chunk (words, ~0.5 s per 6 s chunk), transcribe_file (words merged with diarization into speaker blocks), speaker_profiles (embeddings + encoder id). Unit tests for parsing and the merge; ignored integration tests run Dutch, English and a two-speaker fixture through the real engine and pass. The engine module carries a temporary dead_code allow until VRT-16 uses it.
<!-- SECTION:FINAL_SUMMARY:END -->
