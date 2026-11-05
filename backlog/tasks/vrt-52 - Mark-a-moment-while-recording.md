---
id: VRT-52
title: Mark a moment while recording
status: Done
assignee: []
created_date: '2026-11-04T21:32:00Z'
updated_date: '2026-11-05T22:01:00Z'
labels:
  - ux
milestone: m-3
dependencies:
  - VRT-40
priority: medium
ordinal: 56000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Requested (2026-11-04): while recording, mark a moment ("this matters") with one click on the Live screen or with a global hotkey (the mark-moment shortcut bound with VRT-40, decision-10), and find the marks back afterwards.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 The Live screen has a Mark button; a click or the mark-moment hotkey (also with the app in the background) stores a mark at the current recording time and shows it in the live transcript
- [x] #2 Marks are stored per meeting (database) with their time and an optional label that can be edited or the mark deleted on the meeting page
- [x] #3 The meeting page shows marks in the transcript and on the player; clicking one plays from there
- [x] #4 Markdown export and transcript.md list the marks with their times
- [x] #5 Rust tests for storing marks and their place in exports; a smoke script marks via the button and the IPC path the hotkey uses, checks the meeting page and a restart; screenshots light and dark
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Verified with the engine in a temp folder (smoke-marks.py --speech): marks among live blocks and after the final pass, transcript.md including a later label, md/txt/json exports. Marks also show on the Live screen before the first words and without captions. Open: AC1's hotkey with the app in the background, tested by hand on the RPM.

Shortcuts verified by hand on the 0.1.5 RPM (2026-11-05).
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Marks: a Mark button on the Live screen and the mark-moment global shortcut store the current recording position (Recording::position_ms, table marks, migration 0007). They show at once on the Live screen, also before the first words and without captions; on the meeting page between the transcript blocks and as ticks on the player, with click-to-play, an in-place label and delete. Markdown, text and JSON exports and transcript.md list them (older transcript.md files still parse). Tests: DB, exports, transcript.md round trip; smoke-marks.py, with --speech through the real engine in a temp folder; hotkey tested by hand on the RPM. Known limit: a mark from the shortcut gives no visible feedback while Manent is in the background, and without a recording it is only logged.
<!-- SECTION:FINAL_SUMMARY:END -->
