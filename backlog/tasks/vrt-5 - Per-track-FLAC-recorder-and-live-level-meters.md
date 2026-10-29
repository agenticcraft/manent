---
id: VRT-5
title: Per-track FLAC recorder and live level meters
status: Done
assignee:
  - '@claude'
created_date: 2026-10-29 12:17
updated_date: '2026-10-29T21:52:00Z'
labels:
  - audio
  - backend
milestone: m-1
dependencies:
  - VRT-4
documentation:
  - doc-2
priority: high
ordinal: 5000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Consume each source's ring buffer and write its own FLAC track at the native sample rate into meetings/<id>/; emit RMS/peak level events to the UI at ~20 Hz. Support pause/resume. See doc-2 Technical architecture §1.2, §1.5.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 mic.flac and system.flac written per meeting
- [x] #2 Pause/resume works without corrupting files
- [x] #3 Level events emitted at ~20 Hz per source
- [x] #4 5-minute recording has no dropouts; capture CPU < 5%
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. audio/flac.rs: streaming FLAC writer on flacenc (16-bit, 4096-sample frames written as they fill; placeholder 42-byte header rewritten at finish with total samples, MD5 and frame sizes). Unit tests decode with claxon (independent decoder) and compare samples + MD5.
2. audio/levels.rs: RMS/peak (dBFS) per block; unit tests on known signals.
3. audio/recorder.rs: one capture thread per recording owns the CpalSources (cpal streams are not Send); every 50 ms drain each ring -> FLAC (unless paused) + level event via callback (~20 Hz). Pause discards samples on all tracks (files stay valid, tracks stay aligned). Files: mic.flac / system.flac in meetings/<id>/. Consumer logic generic over rings so it is unit-testable without hardware.
4. Tauri commands start_recording/pause_recording/resume_recording/stop_recording + 'audio:level' event; ipc.ts types. Meeting dir under the app data dir (DB rows come with VRT-6/VRT-8).
5. Remove the temporary dead_code allows from VRT-4.
6. Verify: unit tests; hardware test recording 5 min mic + system (durations equal, 0 drops, files decode with ffprobe/claxon), pause/resume in a hardware run, level event rate, CPU of the recording process < 5%.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
From VRT-4: audio/mod.rs has temporary #[allow(dead_code)] / #[allow(unused_imports)] on the capture and ring modules because only tests used them; remove them once the recorder consumes CpalSource/RingReader.

Implementation (src-tauri/src/audio): flac.rs streaming FLAC writer on flacenc 0.5 (16-bit, 4096-sample frames encoded and written as they fill; 42-byte header placeholder rewritten at finish with total samples, MD5, block and frame sizes); levels.rs RMS/peak dBFS; recorder.rs Recording (capture thread owns the cpal streams, 50 ms tick drains rings -> FLAC unless paused, LevelEvent callback; pause discards samples on all tracks), Track, track_file_names (mic.flac, system.flac, mic-2.flac). Commands start_recording / pause_recording / resume_recording / stop_recording (RecorderState), 'audio:level' event; ipc.ts types. Meeting folder: <app data>/meetings/<start ms> until VRT-6/VRT-8 add rows. Temporary dead_code allows from VRT-4 removed.
Findings: FLAC with 0 total samples means 'unknown' (decoders report None) - expected for empty files. In unoptimised debug builds flacenc is slow (level loop at 7.4 Hz); added [profile.dev.package."*"] opt-level = 2 so tauri dev keeps real time (19.2 Hz in a debug build afterwards). Storage: native 48 kHz stereo mic ~350 MB/hour -> noted in VRT-27.
Verification:
- 21 unit tests (FLAC round trip vs claxon incl. MD5 and odd lengths, clipping, empty file; levels; file names; pause keeps files valid; run_loop levels + pause alignment).
- Hardware 10 s (release): both tracks 9.98 s written over 12 s wall with a 2 s pause, 0 drops, 19.4 level events/s, tracks aligned 0.0 ms; system track contains the 440 Hz test tone (-54.7 dBFS, same as VRT-1); recording process CPU 3%.
- Hardware 5 min (release): 299.947 s on both tracks (302 s wall - 2 s pause), 0 dropped samples, 19.5 events/s (5900 per source), alignment 0.0 ms, CPU 2% (7.5 s CPU / 302 s), RSS 12 MB.
- In the real app (ui-smoke via tauri-driver): start_recording -> audio:level events received in the webview at 19.2/s per source with paused events during pause -> stop_recording returns mic.flac + system.flac of 2.90 s (4 s - 1 s pause), 0 drops.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Recording works end to end: each selected source is written to its own streaming FLAC file (mic.flac, system.flac) in a per-meeting folder at the device's native format, with ~20 Hz RMS/peak level events to the UI and pause/resume that keeps files valid and tracks aligned. Exposed as start/pause/resume/stop Tauri commands plus the audio:level event. Verified with 21 unit tests (FLAC checked against an independent decoder), a 5-minute hardware recording (0 drops, 0.0 ms track drift, 2% CPU, 12 MB RSS) and the full flow inside the running app. Follow-up noted in VRT-27: ~350 MB/hour per stereo mic track; offer a smaller archive format.
<!-- SECTION:FINAL_SUMMARY:END -->
