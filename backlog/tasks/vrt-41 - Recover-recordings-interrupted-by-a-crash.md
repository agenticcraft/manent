---
id: VRT-41
title: Recover recordings interrupted by a crash
status: Done
assignee:
  - '@claude'
created_date: '2026-11-01T21:40:00Z'
updated_date: '2026-11-01T22:01:00Z'
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
- [x] #1 A meeting left in 'recording' by a killed app is recovered at the next start: playable, with the right duration, and transcribed
- [x] #2 Unit test on a truncated FLAC file; smoke test that kills the app during a recording
- [x] #3 The meeting shows it was recovered after an interruption
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Built:
- **Crash-safe writing (flac.rs):**
  - The placeholder header now has valid block sizes. Before, it had min > max, which made a cut-off file unreadable for decoders.
  - Each frame is flushed at once, about 12 small writes a second. Before, up to 8 KB of buffered frames were lost: 1.7 s at mono 16 kHz.
- **Repair (audio::repair_flac, audio/recover.rs):**
  - It decodes the complete frames with claxon (already a dependency) and writes them again through FlacWriter: temp file, then rename.
  - It also patches the invalid placeholder of files recorded before this fix.
  - Unit tests: a killed 30 s stereo recording keeps all complete blocks with identical samples; an old-header file; a crash before anything was flushed (error); no header (error).
- **Startup:** meetings in 'recording' get a 'recover' job (jobs.rs run_recover). It:
  - repairs the tracks, and drops tracks without audio (the meeting too if none is left);
  - sets the durations;
  - makes mix.opus;
  - sets meetings.interrupted (migration 0004);
  - transcribes the meeting (or marks it done without the engine).
  The Library shows 'Recovering …'; the meeting view shows a note that the recording was cut off and recovered.
- **Real app:** scripts/smoke-recover.py PASS. The app was killed (SIGKILL) 20 s into a recording with speech. The next start recovered it: 20.2 s on both tracks, playable, transcribed, with the note in light and dark mode. Four older cut-off meetings from earlier tests (old header format) were recovered as well; their empty system tracks were dropped.
- **Found and fixed on the way:** WebKitGTK sometimes reports a duration of 0 for a short Ogg file that is fine (it isn't the file: the same file is fine on the next open). AudioPlayer now ignores a 0 duration (it keeps the meeting's length) and loads the file once more, which gives the real duration. smoke-transcript.py passes.
- **Noticed:** the waveform of a 25-min two-track recording takes 30 s in the debug build on first open. That is over smoke-transcript's 20 s wait the first time; the result is cached after that.
- **Checks:** 116 Rust tests, clippy 0, npm check green.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
A recording cut off by a crash or kill is recovered at the next start: complete audio up to the last frame, playable, transcribed, and marked as recovered. Recordings now lose at most a fraction of a second when the app dies.
<!-- SECTION:FINAL_SUMMARY:END -->
