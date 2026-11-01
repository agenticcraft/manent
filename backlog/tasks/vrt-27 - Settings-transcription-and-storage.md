---
id: VRT-27
title: 'Settings: transcription and storage'
status: To Do
assignee: []
created_date: 2026-10-29 12:20
updated_date: '2026-11-01T09:53:00Z'
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
- [ ] #1 The rolling pass interval is configurable and persisted, and it applies to the next recording
- [ ] #2 The model can be checked and re-downloaded, and engine plus model removed, without restarting the app
- [ ] #3 The storage location and kept format are configurable; new recordings use them, and existing meetings keep working after a move
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
From VRT-5: tracks are recorded at the device's native format (here 48 kHz stereo, 16-bit FLAC): a mic track is ~29 MB per 5 min (~350 MB/hour); silent system audio compresses to almost nothing. Offer a storage option (e.g. mono and/or 16 kHz for archived meetings) - transcription only needs 16 kHz mono.
<!-- SECTION:NOTES:END -->
