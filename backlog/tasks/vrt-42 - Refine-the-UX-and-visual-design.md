---
id: VRT-42
title: Refine the UX and visual design
status: To Do
assignee: []
created_date: '2026-11-01T21:43:00Z'
labels:
  - ui
  - design
milestone: m-3
dependencies: []
priority: medium
ordinal: 46000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The app works end to end; now refine how it looks and feels, screen by screen, against doc-3 (calm, minimal, the transcript as the hero, neutral colours plus one accent, light and dark). Start with a review round on the real app, in light and dark, at the default and the minimum window size, and turn the findings into concrete changes. Points already noticed while building:
- **Library:** the hierarchy of title, date, length and speakers. The 'processing' state and job progress inline. Empty state and first run (no engine yet: guide to the download).
- **Live screen:** the timer, the level meters and the source chips at a glance. How captions turn into speaker blocks (the jump when a rolling pass replaces captions). Pause and mute states.
- **Meeting view:**
  - the header actions (Export, Re-transcribe);
  - long transcripts: a sticky speaker or time context while scrolling, jump to top;
  - speaker chips: named, unnamed and suggested;
  - the gap note (VRT-36);
  - the player and waveform: the played part, hover time.
- **Speakers and Settings:** grouping and spacing of the sections. Explanatory text kept short. Destructive actions (delete voices, remove engine) consistent.
- **Consistency:** typography scale, spacing, focus rings, hover and active states, transitions (minimal), icons, dark-mode contrast, tooltips, keyboard use (Space to play/pause, Esc to close popovers).
- **App icon and tray icon:** a recognisable Veritas mark that also reads at 16-22 px on light and dark panels.
Record the decisions in doc-3 and extend the smoke scripts' screenshots.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Review round done: findings per screen, with light and dark screenshots, recorded in the notes
- [ ] #2 Agreed changes built and doc-3 updated with the decisions
- [ ] #3 Smoke scripts still pass; screenshots of every screen in light and dark checked
- [ ] #4 Keyboard: Space plays and pauses in the meeting view, Esc closes popovers, visible focus everywhere
<!-- AC:END -->
