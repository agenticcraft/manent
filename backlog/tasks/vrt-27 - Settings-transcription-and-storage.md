---
id: VRT-27
title: 'Settings: transcription and storage'
status: To Do
assignee: []
created_date: '2026-10-29 12:20'
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
Settings > Transcription (model: standard 0.6B v3 by default, small as an English-only fallback; language hint auto/NL/EN; diarization latency) and Settings > Storage (recordings location, audio format to keep). Settings apply to the engine without restarting the app. See doc-3 UI design.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Model, language hint and diarization latency can be changed and are persisted
- [ ] #2 Changing the model switches the engine (downloading it if needed) without restarting the app
- [ ] #3 Storage location and kept format are configurable; new recordings use them
<!-- AC:END -->
