---
id: VRT-27
title: 'Settings: transcription and storage'
status: To Do
assignee: []
created_date: 2026-10-29 12:20
updated_date: '2026-10-29T17:05:00Z'
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
Settings > Transcription (language: auto, Nederlands, English; live and offline model per language; diarization latency) and Settings > Storage (recordings location, audio format to keep). The language can also be set per meeting. Settings apply to the engine without restarting the app. See doc-3 UI design.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Language (auto, NL, EN), live/offline model and diarization latency can be changed and are persisted
- [ ] #2 Language can be overridden per meeting and is passed to the engine
- [ ] #3 Changing the model switches the engine (downloading it if needed) without restarting the app
- [ ] #4 Storage location and kept format are configurable; new recordings use them
<!-- AC:END -->
