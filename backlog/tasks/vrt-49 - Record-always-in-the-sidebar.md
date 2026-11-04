---
id: VRT-49
title: Record always in the sidebar
status: Done
assignee: []
created_date: '2026-11-04T12:28:00Z'
updated_date: '2026-11-04T12:30:00Z'
labels:
  - ux
milestone: m-3
dependencies: []
priority: medium
ordinal: 53000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found while testing 0.1.2: the sidebar only shows "Recording" while a recording runs, so starting one means going to the Library first. Record should always be one click away from any screen.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 The sidebar always has a Record item at the top: idle it starts a recording (like New recording and the tray) and opens the Live screen; while recording it is the pulsing "Recording" item that returns to the Live screen
- [x] #2 Starting from the sidebar works from every screen (Library, a meeting, Speakers, Settings); an error starting (no sources) is shown, not swallowed
- [x] #3 ui-smoke or another smoke script starts and stops a recording from the sidebar; screenshots light and dark
<!-- AC:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
The sidebar always shows Record on top (a red dot, neutral text). Idle, a click starts a recording from any screen and opens the Live screen; a start error shows above the page (record-error) and clears on navigation. While recording it is the pulsing red "Recording" item that returns to the Live screen. smoke-live.py starts a recording from Settings via the sidebar, leaves for Speakers, returns via Recording and stops; screenshots of both states in light and dark.
<!-- SECTION:FINAL_SUMMARY:END -->
