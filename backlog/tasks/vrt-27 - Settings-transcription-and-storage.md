---
id: VRT-27
title: 'Settings: transcription and storage'
status: Done
assignee:
  - '@claude'
created_date: 2026-10-29 12:20
updated_date: '2026-11-01T10:37:00Z'
labels:
  - frontend
  - backend
milestone: m-2
dependencies:
  - VRT-9
  - VRT-15
documentation:
  - doc-3
priority: medium
ordinal: 27000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
What remains of Settings > Transcription and Storage (doc-3). Already built in VRT-14: engine and model status, download with progress and resume, and the model licences.
Remaining:
- Transcription: how often speaker labels are updated while recording (the rolling pass interval, now fixed at 45 s in rolling.rs; shorter costs CPU, longer delays names). Plus 'Check the model' (verify the sha256 again and re-download a damaged file) and 'Remove engine and model' to free 1.1 GB.
- Storage: the recordings location, which moves the meetings folder and keeps the database paths right, and which audio to keep: FLAC tracks plus mix.opus (as now), or only mix.opus to save space. Only mix.opus means no re-transcribe and no speaker separation afterwards, so the UI must say so.
No language setting: the model ignores a requested language (VRT-32, rechecked 2026-11-01).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 The rolling pass interval is configurable and persisted, and it applies to the next recording
- [x] #2 The model can be checked and re-downloaded, and engine plus model removed, without restarting the app
- [x] #3 The storage location and kept format are configurable; new recordings use them, and existing meetings keep working after a move
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Settings keys: transcription.rolling_interval_s (default 45), storage.keep ('all' | 'mix'), storage.meetings_dir (null = <app data>/meetings). Commands get/set; rolling::spawn takes the interval (read at recording start).
2. Engine: engine.json records the sha256 of the extracted files at install; engine_check verifies engine files and model + NOTICE (re-download needed -> not installed, damaged file removed); engine_remove stops the helper and deletes engine and model (incl. partial downloads). Both refused while recording or a job runs.
3. Storage location: AppPaths gets a shared, changeable meetings root. move_meetings(to): refused while recording or a job runs; moves every meeting folder (rename, or copy + delete across file systems), updates meetings.dir and tracks.path, then stores the setting. Media server, jobs and deletes use the new root at once.
4. Kept audio: 'mix' deletes a meeting's FLAC tracks after its final pass (waveform cached first); Re-transcribe then explains the tracks are gone.
5. UI: Settings > Transcription (update interval, Check model, Remove engine and model), Settings > Storage (location + Change, kept audio). Tests: unit (paths, move, keep), smoke on the real app.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
From VRT-5: tracks are recorded at the device's native format (here 48 kHz stereo, 16-bit FLAC): a mic track is ~29 MB per 5 min (~350 MB/hour); silent system audio compresses to almost nothing. Offer a storage option (e.g. mono and/or 16 kHz for archived meetings) - transcription only needs 16 kHz mono.

Implemented (2026-11-01):
- Settings > Transcription: 'Update speakers while recording' (30/45/60/90 s, setting transcription.rolling_interval_s; read at recording start and passed to rolling::spawn; logged). 'Check model' (sha256 of model + NOTICE against the pins, of the engine files against hashes now stored in engine.json at install; damaged files are removed so Download fetches them again) and 'Remove engine and model' (confirm; stops the helper, deletes engine/ and models/ incl. partial downloads). Both refused while recording or transcribing.
- Settings > Storage: meetings folder (Change… with the folder dialog, Back to default): AppPaths has a shared, changeable meetings root; move_meeting_folders refuses the same/nested folders before creating anything (bug found by its test), renames or copies; point_meetings_to updates meetings.dir and tracks.path. 'Audio to keep': tracks + playback file, or playback file only (tracks deleted after the final pass, waveform cached first, never without mix.opus; Re-transcribe explains). Sizes measured on real recordings: FLAC ~400 MB/h, mix ~13 MB/h (the UI says so).
Verification: unit tests (move incl. refusal leaves nothing, copy, shared root, DB repointing, damaged engine files). scripts/smoke-storage.py on the real app: interval saved and the next recording used 30 s (driver log); check finds a tampered NOTICE, Download restores it; remove deletes engine and model (restored from hard-link backup); 34 meetings / 62 tracks moved, DB repointed, a moved meeting plays and exports, moved back; playback-file-only import keeps mix.opus + waveform.json, tracks gone, Re-transcribe refuses with the reason. The folder dialog itself can't be driven (same command behind it).
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Settings for transcription and storage: how often speakers update while recording, check/repair and remove the engine and model, move the meetings folder (database follows, refused while busy), and optionally keep only the small playback file after transcription. Verified with unit tests and on the real app.
<!-- SECTION:FINAL_SUMMARY:END -->
