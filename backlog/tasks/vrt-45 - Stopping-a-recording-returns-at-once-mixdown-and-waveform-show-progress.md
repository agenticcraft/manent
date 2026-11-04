---
id: VRT-45
title: Stopping a recording returns at once; mixdown and waveform show progress
status: Done
assignee: []
created_date: '2026-11-04T10:10:00Z'
updated_date: '2026-11-04T10:46:00Z'
labels:
  - ux
  - recording
dependencies: []
priority: high
ordinal: 49000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Stopping a 55-minute recording froze the Live screen for about 30 s (stop_recording waits for the playback mixdown to mix.opus), then the meeting's player waited about 45 s more for its waveform (computed on first open by decoding the mix). Nothing on screen said what was happening, so the app looked hung. Found while testing the installed RPM (VRT-23).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 stop_recording returns within a second of the capture stopping, also for long recordings; the mixdown runs afterwards as a job (or a step of the final-pass job) and the Library row and the meeting show it
- [x] #2 The meeting opens straight away: while mix.opus or the waveform is not ready, the player says so (for example 'Preparing audio…', with progress) instead of looking empty or frozen; the transcript is readable meanwhile
- [x] #3 The waveform is computed as part of the mixdown (one decode, not a second one on first open)
- [x] #4 A crash or quit during the mixdown leaves a meeting that recovers it on the next start (the tracks are already safe)
- [x] #5 smoke-live.py or smoke-playback.py checks a stop returns quickly and the meeting becomes playable
<!-- AC:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Stop no longer waits for the playback mixdown: stop_recording finishes the capture, marks the meeting processing and hands it to Jobs::finish_recording, which makes mix.opus on its own thread (it doesn't need the engine, so it never waits behind another transcription) and then queues the final pass. The app opens the recording straight away (also after a tray stop while watching it); the meeting page shows 'Preparing audio… N%' and the player appears on the meeting:playback event. The waveform now comes from the mixdown's power per 20 ms frame (one decode; it was a second full decode on first open, ~45 s for 55 min), written with mix.opus, which is written aside and renamed so a half file is never played. A quit during the mixdown: the startup transcribe job makes the playback file first (and without the engine just finishes). Verified: 2 new job tests + mixdown/waveform tests (131 Rust tests), smoke-playback at 5 s and 60 s (Stop 0.31 s, 'Preparing audio' seen, waveform 600 bars), ui-smoke, smoke-live, smoke-library; the other recording scripts now go back to the Library after Stop. Not run: smoke-tray (the installed Manent's tray icon was on the bus).
<!-- SECTION:FINAL_SUMMARY:END -->
