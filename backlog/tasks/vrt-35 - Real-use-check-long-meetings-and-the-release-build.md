---
id: VRT-35
title: 'Real-use check: long meetings and the release build'
status: In Progress
assignee:
  - '@claude'
created_date: '2026-11-01T10:37:00Z'
updated_date: '2026-11-01T10:37:00Z'
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
- [ ] #1 Release build measured against debug (RTF, startup)
- [ ] #2 37-minute imports (English and Dutch): time, helper peak memory and quality recorded; meeting screen stays responsive
- [ ] #3 20+ minute live recording: captions and rolling passes keep up, memory stays flat, final pass time recorded
- [ ] #4 Problems found are fixed or ticketed
<!-- AC:END -->
