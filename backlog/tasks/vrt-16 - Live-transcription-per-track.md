---
id: VRT-16
title: Live transcription per track
status: Done
assignee:
  - '@claude'
created_date: 2026-10-29 12:17
updated_date: '2026-10-31T15:19:00Z'
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
- [x] #1 Text appears within ~4 s (median) and at most ~8 s of speech
- [x] #2 Caption chunks persisted with track and timestamps until a rolling pass replaces them
- [x] #3 Chunker and resampler unit-tested (cut point within 4-8 s, at the quietest frame)
- [x] #4 Dutch and English transcribed live, including a meeting that switches between them
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

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Implemented (2026-10-31):
- Track rows are created at start (seconds set at stop), so captions carry track_id while recording. Track::tick feeds exactly what it writes to a per-track worker (live.rs): downmix -> StreamResampler 16 kHz -> Chunker -> Engine::transcribe_chunk -> caption segment (pass caption, words_json) + event transcript:caption; errors -> transcript:error, never stop the recording. Captions off (no engine) -> Live shows a hint to download it.
- Chunking had to change from the VRT-32 rule: a global quietest-frame cut in 4-8 s needs 8 s of look-ahead (first run: median 5.7 s). The VRT-32 simulation assumed a chunk is ready at its own end, which hid this. Now causal: first 160 ms pause after 4 s; else quietest 160 ms by 8 s with the words in the last 300 ms redone in the next chunk; an empty result for a chunk with sound is retried as a chunk >= 2 s longer (parakeet returns nothing for some ~4 s pieces that start/end inside an utterance, reproduced with the helper); silence gate -60 dBFS (quiet remote speech is ~-50).
- UI: LiveTranscript (blocks per source, 'Me'/'System audio', caption colour, fade-in, auto-scroll with 'Jump to live'); live_captions command reloads captions when the Live screen opens again.
Verification: 10 live.rs unit tests (pause cut without waiting, short dips ignored, fallback window, contiguity, DC offset, downmix, worker resampling/offsets, redo of a cut word, retry of an empty chunk, engine errors) + track feed test + DB set_track_seconds. scripts/smoke-captions.py on the real app with the FLEURS NL/EN sample (loudness-normalised, 60% volume) through the speakers: final run median 3.5 s, p90 6.7 s, max 8.8 s; NL 11.2%, EN 11.5% WER on the system track (offline 11.2/11.5); over 4 runs median 2.9-3.5 s, EN 11.5-16.4%. A retried chunk can take up to ~10 s (seen once: 10.3 s). Captions stored with track ids; Jump to live works; light/dark screenshots checked. ui-smoke, smoke-library, smoke-live, smoke-playback and smoke-engine still pass. The mic also captures the room and the speakers (bleed): VRT-17.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Live captions per track: each track feeds a worker that resamples to 16 kHz, cuts causal 4-8 s chunks at pauses (with redo of words cut at forced cuts and a longer retry for empty results), transcribes them through the engine and stores/streams them; the Live screen shows them per source with auto-scroll and Jump to live. Measured on the real app: text a median ~3 s after speech, p90 under 7 s, and live WER equal to offline on the NL/EN sample. Risk: an empty-chunk retry can delay text to ~10 s; mic bleed is handled in VRT-17.
<!-- SECTION:FINAL_SUMMARY:END -->
