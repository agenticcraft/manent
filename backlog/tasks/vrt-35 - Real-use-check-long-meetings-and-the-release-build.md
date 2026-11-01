---
id: VRT-35
title: 'Real-use check: long meetings and the release build'
status: Done
assignee:
  - '@claude'
created_date: '2026-11-01T10:37:00Z'
updated_date: '2026-11-01T13:05:00Z'
labels:
  - test
  - transcription
milestone: m-2
dependencies: []
priority: high
ordinal: 39000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Everything so far was verified with recordings of 1-5 minutes in a debug build. Before daily use, check what a real meeting does:
- Release build (npm run tauri build -- --no-bundle): speed of the engine helper and the app, compared with debug.
- Long imports (37-minute recordings from spikes/localai/samples/meetings/interviews, English and Dutch): job time, peak memory of the helper, transcript quality against the YouTube captions where they exist (as VRT-32 did), and the meeting screen with thousands of words (load time, scrolling, playback highlight).
- A long live recording (20+ minutes of speech through the system audio): captions keep up without falling behind, rolling passes stay on time, memory of app and helper stays flat, the final pass time.
Record the numbers in the notes and fix what breaks. Known limit of the test machine: the speaker output is muted, so acoustic bleed can't be produced here (covered by the engine test of VRT-17).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Release build measured against debug (RTF, startup)
- [x] #2 37-minute imports (English and Dutch): time, helper peak memory and quality recorded; meeting screen stays responsive
- [x] #3 20+ minute live recording: captions and rolling passes keep up, memory stays flat, final pass time recorded
- [x] #4 Problems found are fixed or ticketed
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Measured 2026-11-01 on the Ryzen AI 9 HX 370 laptop, release build unless noted. Scripts: scripts/measure-import.py, scripts/measure-live.py.

**Release vs debug**
- Build: release 4 min (app 13.8 MB binary). Startup: library shown after ~0.95 s in both builds; engine loaded in 0.5 s.
- Same 25-minute import: release 182 s (x8.2 real time), debug 221 s (x6.8). The difference is decoding, mixdown and waveform (~40 s vs ~65 s); transcription is libparakeet in both.

**Long imports**
| File | Audio | Job | Helper peak | App peak | Words | vs YouTube captions |
|---|---|---|---|---|---|---|
| interview-nl (NL interview) | 40.1 min | 303 s (x7.9) | 2685 MB | 373 MB | 8074 | 14.4% |
| podcast-en (EN podcast) | 37.3 min | 280 s (x8.0) | 2514 MB | 348 MB | 6857 | (no captions) |
| panel-nl-1 first 25 min (NL panel) | 25.0 min | 182 s (x8.2) | ~2640 MB | 311 MB | 4655 | 17.8% |
YouTube captions are ASR output themselves: a rough reference. On the meeting screen, 8074 words render in 0.6 s, scrolling to the end takes 0.2 s, and click-to-seek near the end plays and highlights the right word.

**25-minute live recording** (NL panel through the system audio, sleep blocked)
- Captions per 5 minutes: median 2.1-2.7 s after the words were spoken, max 9.7-12.8 s; overall median 2.4 s, p95 9.5 s. No drift.
- Rolling passes covered all but the last 15-45 s throughout (interval 45 s).
- Memory: app 268 to 280 MB; helper 2290 to 2987 MB.
- Final pass 217 s after Stop (two tracks, 0.145 of the length). Final system-track transcript vs captions: 20.1%, against 17.8% for the import of the same audio.

**Helper memory** (helper alone, 120 rounds of 3 caption chunks plus a 75 s transcribe and diarize, about a 90-minute meeting)
- Grows to ~2.6-2.7 GB within 20 rounds, then stays flat up to round 120. Not a leak: libparakeet buffers that grow with the input.
- MALLOC_ARENA_MAX=2 lowers it by ~200 MB but doesn't change the shape; not applied.

**Problems found**
- Idle suspend during a recording froze capture for 7 minutes (twice). The recording continued after resume, but the audio is missing. Ticketed as VRT-36. Measuring with systemd-inhibit --what=sleep:idle worked, which is a candidate approach.
- The progress bar used a debug-build estimate (0.2 of the length). It stuck at ~65% and then jumped to done. Fixed in jobs.rs: 0.11 per track plus 0.04 per extra track, and a 15% share for decoding.
- smoke-engine.py's 'helper stopped with the app' check allowed only 2 s and failed once on a busy machine just after boot. Exit takes 0.7-1.2 s, including after a helper crash and restart. The check now waits up to 5 s.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Release build, two 37-40 minute imports and a 25-minute live recording measured. Captions keep up (median 2.4 s), rolling passes stay within one interval, and helper memory levels off at ~2.7 GB (no leak). The meeting screen handles 8000 words easily. Fixed the progress estimate; idle suspend during recording ticketed as VRT-36.
<!-- SECTION:FINAL_SUMMARY:END -->
