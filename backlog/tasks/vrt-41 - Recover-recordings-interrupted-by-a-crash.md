---
id: VRT-41
title: Recover recordings interrupted by a crash
status: To Do
assignee: []
created_date: '2026-11-01T21:40:00Z'
labels:
  - recording
  - robustness
milestone: m-3
dependencies: []
priority: high
ordinal: 45000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found in VRT-22: when the app process dies during a recording (a crash, a kill, or WebDriver's Close Window), the meeting stays in status 'recording' with duration 0. Its FLAC files have no final header (the totals are written by FlacWriter::finish). At startup nothing picks these up: only 'processing' meetings are re-queued. Recover them at startup:
- repair the FLAC files' stream info (sample count) from the frames written, or rewrite them by decoding what is readable;
- set the track lengths and duration, make mix.opus and the waveform;
- mark the meeting 'processing' so the final pass transcribes it.
Show the user that the meeting was recovered after an interruption, for example with a note in the meeting view.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A meeting left in 'recording' by a killed app is recovered at the next start: playable, with the right duration, and transcribed
- [ ] #2 Unit test on a truncated FLAC file; smoke test that kills the app during a recording
- [ ] #3 The meeting shows it was recovered after an interruption
<!-- AC:END -->
