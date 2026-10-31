---
id: VRT-16
title: Live transcription per track
status: In Progress
assignee:
  - '@claude'
created_date: 2026-10-29 12:17
updated_date: '2026-10-30T09:41:00Z'
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

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Track rows created at recording start (seconds updated at stop), so captions can carry track_id while recording.
2. Feed: Track::tick sends exactly what it writes (silence while muted, nothing while paused) to a per-track ASR worker thread over mpsc (the capture thread is not the realtime callback, so allocating there is fine). Caption times are then track-relative ms, the same as the files and the mix.
3. Worker: downmix to mono -> StreamResampler to 16 kHz -> chunker (cut at the quietest 20 ms frame between 4 and 8 s after the previous cut; flush the rest at stop if >= 0.5 s; skip chunks quieter than about -50 dB) -> Engine::transcribe_chunk -> offset the words by the chunk start -> event transcript:caption {meeting_id, track_id, kind, start_ms, end_ms, text, words} plus db.insert_segments(pass caption).
4. Engine off or not installed: no captions, and the Live screen says so with a link to Settings; per-chunk errors are shown as status, never stop the recording.
5. Live screen: caption blocks in time order (mic = 'Me', system = 'System'), caption colour, auto-scroll with 'Jump to live'.
6. Tests: chunker and resampler unit tests; smoke-live extended (play a Dutch/English FLEURS WAV through the speakers and check captions and timing); remove the dead_code allow on mod engine in lib.rs.
<!-- SECTION:PLAN:END -->
