---
id: VRT-54
title: Record in the sidebar opens the record page, it doesn't start
status: Done
assignee: []
created_date: '2026-11-06T20:05:00Z'
updated_date: '2026-11-06T20:11:00Z'
labels:
  - ux
milestone: m-3
dependencies: []
priority: medium
ordinal: 58000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Correction (2026-11-06) of VRT-49, where a click on Record started a recording straight away: it should only take the app to the record page, so a stray click never records. The page needs something to show while idle.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Record in the sidebar opens the record page from any screen without starting a recording; while recording it returns to the running recording
- [x] #2 The idle record page has a Start recording button, the sources with their test meters and the Call / In the room switch; the switch set here applies to the recording that starts and is remembered
- [x] #3 A failed start is shown on the page; dropping a file to import from the idle page goes to the Library, where the progress is
- [x] #4 smoke-live.py checks that the click doesn't record, the page, the switch and Start; screenshots light and dark
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
Rust: set_recording_in_room also works without a recording (setting only), new get_in_room. UI: MeetingKindSwitch extracted from Live; routes/RecordReady.tsx (SourcesSettings for sources and meters, the switch, Start); page 'live' without a session shows it; Sidebar item only navigates. Tray, shortcut and Library's New recording still start directly.
<!-- SECTION:PLAN:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Record in the sidebar now only navigates: idle it opens a new record page (routes/RecordReady.tsx) with the sources and test meters, the Call / In the room switch and Start recording; while recording it returns to the recording. set_recording_in_room also works without a recording (saves the choice), get_in_room reads it; MeetingKindSwitch is shared with the Live screen. The tray, the shortcut and the Library's New recording still start directly. Tested: smoke-live.py (no recording on click or on leaving, meters with sound playing, switch, Start, restart); smoke-marks.py and ui-smoke.py; a real failed start (no audio server) shows the error and Start can be retried; a drop event from the idle page imports and goes to the Library (one-off checks, not in a script). screens.py shoots the page.
<!-- SECTION:FINAL_SUMMARY:END -->
