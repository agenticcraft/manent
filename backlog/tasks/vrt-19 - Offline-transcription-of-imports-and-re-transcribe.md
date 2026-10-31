---
id: VRT-19
title: Offline transcription of imports and re-transcribe
status: Done
assignee:
  - '@claude'
created_date: 2026-10-29 12:17
updated_date: '2026-10-31T21:01:00Z'
labels:
  - transcription
  - backend
  - frontend
milestone: m-2
dependencies:
  - VRT-6
  - VRT-15
  - VRT-18
documentation:
  - doc-2
priority: high
ordinal: 19000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Drag-and-drop or import wav/mp3/m4a/mp4 and transcribe with diarization via the file API (vad:true for long audio), and 're-transcribe' existing meetings. Uses the multilingual offline model (parakeet-cpp-bundle-standard or the VRT-32 choice) so Dutch, English and mixed recordings work. Runs in a job queue with progress. See doc-2 Technical architecture §1.3.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Imported file becomes a meeting with a speaker-labelled transcript
- [x] #2 Re-transcribe replaces segments of an existing meeting
- [x] #3 Job progress visible; jobs survive navigation
- [x] #4 Known speakers are recognized in imports
- [x] #5 Dutch, English and mixed NL/EN recordings transcribe correctly
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Implemented (2026-10-31):
- decision-3: symphonia (mp3, aac, alac, isomp4 + defaults) for decoding, tauri-plugin-dialog (dialog:allow-open) for the Import button; drag and drop uses Tauri's built-in event.
- audio/import.rs: streams any supported file into a mono FLAC track at its own rate (import.flac), skips damaged packets, clear errors for unreadable files; unit tests decode wav/mp3/m4a/mp4/ogg fixtures (tests/fixtures/import).
- jobs.rs: one-at-a-time job queue (import, transcribe) with progress events (jobs:changed); the post-recording final pass now runs through it too, and meetings left 'processing' are re-queued at startup. Import: meeting (title = file name, date = file mtime) + track + mix.opus + the shared transcribe step (final pass, name carry-over, recognition). Without the engine an import is refused; tests cover an unreadable file (no meeting left) and an import without the engine (playable meeting, clear message).
- UI: Library 'Import audio' (dialog), jobs panel with progress and dismiss, drop overlay; Meeting screen shows the transcript (shared TranscriptBlocks), Re-transcribe button and job progress. Unnamed speaker chips are outlined with a colour dot so they never look like a named person with the same colour.
Verification (scripts/smoke-import.py, real app): ami-b.mp3 (English, 3 min), nl-b.m4a (Dutch interview), meeting-nl-en.mp4 (video, mixed) imported together in 53 s; jobs listed with progress and still there after navigating away; WER nl 8.7% (interview), nl 11.2% / en 8.2% (mixed); the known person MEE014 recognised in 418/418 words; Re-transcribe from the meeting screen replaced the transcript. A meeting left 'processing' by an earlier test run was picked up at startup. Not tested: drag and drop and the native dialog themselves (WebDriver can't drive them; both call the same import command), and very long files (> 20 min) in one pass.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Imports and re-transcribe: recordings in wav/mp3/m4a/mp4/flac/ogg/mkv are imported (button or drag and drop), decoded into the meeting's own track, mixed for playback and transcribed with speakers and recognised people; any meeting can be re-transcribed. All of it, including the final pass after a recording, runs in a background job queue with progress that survives navigation and app restarts. Verified with English, Dutch and mixed files on the real app.
<!-- SECTION:FINAL_SUMMARY:END -->
