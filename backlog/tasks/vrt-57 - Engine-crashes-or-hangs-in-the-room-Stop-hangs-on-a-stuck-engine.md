---
id: VRT-57
title: Engine crashes or hangs in the room; Stop hangs on a stuck engine
status: Done
assignee: []
created_date: '2026-11-12T13:41:00Z'
updated_date: '2026-11-12T13:48:00Z'
labels:
  - engine
milestone: m-3
dependencies: []
priority: high
type: bug
ordinal: 61000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found 2026-11-11/12: a 56-minute recording In the room. The helper crashed (SIGSEGV in libparakeet's speaker encoder) after 11 minutes; after the restart no rolling pass finished or failed any more, the helper used CPU for hours, and Stop never returned, so the meeting stayed 'recording' until the next start recovered it. Its final pass crashed the same way. Cause: since VRT-47 the mic and the system track are diarized at once, and the helper runs requests in parallel, but parakeet_capi.h says a speaker (voice) context must not be used by two threads at once. A request that never returns then blocks the rolling thread, and stop_recording joins it.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 The helper never runs two diarizations at once (the voice and diarization contexts are used by one thread at a time); an in-room final pass with speech on both tracks transcribes without crashing
- [x] #2 Stop returns within seconds even when the engine doesn't answer: the passes and caption workers are given a short time, then the helper is stopped so waiting requests fail, and the recording is finished and its final pass queued
- [x] #3 Tests: an engine test diarizing two tracks at once; a unit or smoke check that Stop returns with a stuck engine
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Evidence: two crashes of the installed helper, 2026-11-11 11:12 and 2026-11-12 14:34, both SIGSEGV inside libparakeet's speaker encoder (vd::), both on meeting 12 (In the room). After the first one no rolling pass finished or failed, the helper used CPU for hours and Stop never returned. parakeet_capi.h: a speaker ctx must not be used by two threads at once; since VRT-47 two diarizations ran in parallel. The new engine test (two tracks with speech, in the room, four final passes) crashes in round 1 without the lock and passes with it. The exact hang was not reproduced: killing the helper (SIGKILL, SIGABRT with core dump, in the room) during rolling passes never hung the debug build; Stop now gives the live threads 15 s and then stops the helper, unit-tested with a worker that never gets an answer.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
The helper runs one diarization at a time (a mutex around the speaker and diarization contexts, as parakeet_capi.h requires); Stop waits at most 15 s for the caption workers and the rolling pass and then stops the helper, so waiting requests fail instead of blocking Stop. Tests: engine test with two diarized tracks (fails without the fix), unit tests for the Stop wait, all engine tests, the in-room crash recording (passes resume, Stop in 0.6 s).
<!-- SECTION:FINAL_SUMMARY:END -->
