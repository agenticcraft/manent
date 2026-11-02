---
id: VRT-42
title: Refine the UX and visual design
status: Done
assignee:
  - '@claude'
created_date: '2026-11-01T21:43:00Z'
updated_date: '2026-11-02T17:29:00Z'
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
- [x] #1 Review round done: findings per screen, with light and dark screenshots, recorded in the notes
- [x] #2 Agreed changes built and doc-3 updated with the decisions
- [x] #3 Smoke scripts still pass; screenshots of every screen in light and dark checked
- [x] #4 Keyboard: Space plays and pauses in the meeting view, Esc closes popovers, visible focus everywhere
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Review round 1 (2026-11-02). scripts/screens.py takes 48 screenshots: every screen, light and dark, at 1200x800 and 800x560.

Findings:
- **Consistency:**
  - Content width differs per screen: Library and Meeting max-w-3xl, Settings and Speakers 672 px, Live full width. The header actions (top right) don't line up with the content below.
  - Settings buttons come in three sizes and styles.
  - Transcript blocks are indented 12 px against the title and the player.
  - Live: Stop (a secondary grey button) looks weaker than Pause (outline), though Stop is the main action.
- **Bugs:**
  - Popovers don't close on Esc or on a click outside; the speaker popover and the Export menu were open at the same time.
  - Space does nothing in the meeting view.
- **Proposals, all accepted:**
  - an automatic title (day and time) for untitled meetings;
  - the meeting's speakers in the header;
  - tighter spacing for short turns;
  - date and length on search results.
- **Not yet:** app and tray icon (a recognisable mark at 16-22 px).

Built after review round 1:
- **One content column:** max-w-3xl, centred, set in App for every screen. Blocks use -mx-3 so their text lines up with the title.
- **Buttons:** Settings buttons all sm; Remove engine is quiet and turns red on hover; Stop is the primary button on the Live screen.
- **Popovers:** a shared useDismiss hook (src/lib/dismiss.ts) closes them on a press outside or on Esc. A press on the chip of an open popover no longer closes and then reopens it.
- **Keyboard:** Space plays and pauses in the meeting view (AudioPlayer). A base CSS rule gives every control a visible focus outline.
- **Proposals:**
  - meetingTitle() names untitled meetings by when they were held, in muted text, also as the Live placeholder;
  - MeetingSpeakers shows the speakers and their share under the title, and a click cues their first turn;
  - turns of up to 60 characters sit on one line (not the growing live block);
  - search hits show date and length.

Review round 2 (scripts/screens.py, 48 screenshots) checked; '0%' shares became '<1%'.

**Tests:**
- smoke-transcript.py now presses real keys: Space toggles playback, Esc closes the speaker popover and the Export menu.
- PASS: ui-smoke, smoke-library, smoke-transcript, smoke-export, smoke-tray, smoke-transcript-md and smoke-awake.
- Fixed in the scripts:
  - smoke-library expected 'Untitled meeting', and a 'processing' seed that startup has re-queued since VRT-19; it now uses 'failed'.
  - smoke-export now opens its meeting by id.

doc-3 updated with the decisions. App and tray icon split off to VRT-43.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
The app now uses one centred content column, consistent buttons, Esc and outside-press closing for every popover, Space for play/pause and visible focus. Untitled meetings are named by when they were held; the meeting header shows who spoke and how much; short turns read compactly; search results show date and length. The icon is VRT-43.
<!-- SECTION:FINAL_SUMMARY:END -->
