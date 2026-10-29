---
id: VRT-27
title: 'Settings: transcription and storage'
status: To Do
assignee: []
created_date: 2026-10-29 12:20
updated_date: '2026-10-29T18:45:00Z'
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
