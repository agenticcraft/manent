---
id: VRT-27
title: 'Settings: transcription and storage'
status: To Do
assignee: []
created_date: 2026-10-29 12:20
updated_date: '2026-10-29T21:51:00Z'
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
Settings > Transcription (model status and download, diarization latency) and Settings > Storage (recordings location, audio format to keep). No language setting: the model detects Dutch/English itself (VRT-32). Settings apply to the engine without restarting the app. See doc-3 UI design.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Model status and diarization latency visible/changeable and persisted
- [ ] #2 Model can be (re)downloaded from settings without restarting the app
- [ ] #3 Storage location and kept format are configurable; new recordings use them
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
From VRT-5: tracks are recorded at the device's native format (here 48 kHz stereo, 16-bit FLAC): a mic track is ~29 MB per 5 min (~350 MB/hour); silent system audio compresses to almost nothing. Offer a storage option (e.g. mono and/or 16 kHz for archived meetings) - transcription only needs 16 kHz mono.
<!-- SECTION:NOTES:END -->
