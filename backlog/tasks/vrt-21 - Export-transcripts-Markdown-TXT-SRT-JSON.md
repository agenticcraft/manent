---
id: VRT-21
title: Export transcripts (Markdown, TXT, SRT, JSON)
status: Done
assignee:
  - '@claude'
created_date: 2026-10-29 12:17
updated_date: '2026-11-01T10:10:00Z'
labels:
  - frontend
  - backend
milestone: m-2
dependencies:
  - VRT-20
documentation:
  - doc-1
priority: medium
ordinal: 21000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Export a meeting's final transcript from the meeting screen (doc-3) in Markdown, TXT, SRT and JSON. Speakers as shown in the app: the person's name, 'Speaker N', and the user's own name for the mic (setting speaker.me_name). Timestamps from the segments (track time = playback time). SRT: one cue per block, split into lines of at most about 7 s using the word times. JSON: blocks with their words (start/end/conf), so other tools can use the timing. Written through the dialog plugin's save dialog (dialog:allow-save, decision-3).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 All four formats export with speaker names and timestamps
- [x] #2 Exports covered by snapshot tests
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Implemented (2026-11-01): export.rs renders Markdown, TXT, SRT (cues <= 7 s split between words) and JSON (blocks with words, start/end/conf rounded, ISO UTC date) from the stored segments, merged into blocks like the app; speakers named as shown (person, 'Speaker N', the user's own name for the mic, or the caption source). export_meeting command writes the file chosen in the save dialog (dialog:allow-save); Export menu on the meeting screen. Snapshot tests for all four formats (tests/snapshots, UPDATE_SNAPSHOTS=1 to accept) plus tests for block merging, cue splitting, speaker naming and date conversion. Verified with scripts/smoke-export.py on the real app: menu offers the 4 formats; a Dutch interview with two named people exported to all formats; names in Markdown/JSON, 8 TXT lines, 25 SRT cues all <= 7 s, JSON 467 words. The save dialog itself can't be driven by WebDriver (same command behind it).
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Transcript export from the meeting screen in Markdown, TXT, SRT and JSON, with speakers named as in the app and timestamps on the playback timeline; covered by snapshot tests and checked on the real app.
<!-- SECTION:FINAL_SUMMARY:END -->
