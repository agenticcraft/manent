---
id: VRT-47
title: 'Call or in the room: speakers on the mic track'
status: Done
assignee: []
created_date: '2026-11-04T12:27:00Z'
updated_date: '2026-11-04T12:38:00Z'
labels:
  - speakers
  - ux
milestone: m-3
dependencies: []
priority: high
ordinal: 51000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The user also records in-person meetings with only the laptop microphone. Today the mic track is always "Me" (AGENTS.md, doc-2 §1.2) and only the system track is diarized, so everyone in the room ends up as "Me". The user wants to choose this on the recording screen: "Online meeting" (default, as today) or "In person" (the mic holds several people), remembered across meetings.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 The Live screen has a Call / In the room switch, usable during the recording; the last choice is remembered for the next recording, also after a restart (Call on a fresh install)
- [x] #2 The choice is stored per meeting; in the room, the rolling passes and the final pass diarize the mic track too, its voices become "Speaker N" numbered together with the system track's voices, nameable and recognised like any other speaker
- [x] #3 Call behaves exactly as before (mic = Me, bleed removed)
- [x] #4 Re-transcribe uses the meeting's stored choice
- [x] #5 Rust tests cover the in-room labelling (shared numbering across tracks, no label collisions); a smoke script switches the mode and checks it is remembered after a restart; screenshots light and dark
- [x] #6 AGENTS.md and doc-2 describe the two modes
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
Names: Call (only you on the mic) / In the room (everyone in the room on the mic; also hybrid). meetings.in_room column (migration 0006) + setting recording.in_room for the last choice; start_recording takes the setting, set_meeting_in_room switches during the recording (DB + setting + an AtomicBool the rolling thread reads). Rolling/final passes: in the room the mic is diarized too (transcribe_file), bleed removed first; labels numbered across tracks (final: by first appearance over all tracks; rolling: shared numbering and voices, continuity per track). Live captions of the mic in the room: no "Me". Live screen: segmented switch in the header. Re-transcribe reads meetings.in_room.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Names: Call / In the room (better names than online/in person; "In the room" also covers hybrid meetings). Engine test in_a_room_the_people_on_the_mic_are_told_apart: two English speakers plus a Dutch one on the mic alone get labels 1, 2, 3, no Me. Not verified live: a real in-room recording through the rolling passes (needs people talking into the mic); the unit tests cover the rolling labels per track. Edge: mic captions left in an in-room meeting (only when its final pass failed) still export as the user's name.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Live screen switch Call / In the room (radiogroup next to the source chips, default Call), stored per meeting (migration 0006, meetings.in_room) and as the last choice (setting recording.in_room) via set_recording_in_room; the rolling thread reads it per pass (PassOptions.in_room). In the room the mic is transcribed with diarization; label_blocks removes bleed first, then labels all diarized tracks together: final pass numbers speakers over all tracks by first appearance, rolling passes keep continuity per track with shared numbering and voices. Mic captions in the room show "In the room" instead of the user's name. Re-transcribe reads meetings.in_room. Tests: unit tests for cross-track numbering, per-track continuity, in-room bleed removal and call behaviour; engine test with three people on the mic; smoke-live checks the default, switching, the next recording and a restart, screenshots light/dark. doc-2 §1.2-1.4 and AGENTS.md updated.
<!-- SECTION:FINAL_SUMMARY:END -->
