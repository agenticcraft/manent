---
id: VRT-8
title: Live recording screen
status: Done
assignee:
  - '@claude'
created_date: 2026-10-29 12:17
updated_date: '2026-10-29T22:22:00Z'
labels:
  - frontend
milestone: m-1
dependencies:
  - VRT-5
  - VRT-6
  - VRT-7
documentation:
  - doc-3
priority: high
ordinal: 8000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Recording view: editable title, timer (tabular numerals), per-source chips with mini level meter and mute toggle, Pause/Stop, soft red recording pulse. Transcript area is a placeholder until Phase 2. See doc-3 UI design.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Start creates a meeting and begins recording the configured sources
- [x] #2 Level meters animate per source; mute toggles a source
- [x] #3 Pause/Resume/Stop work and the meeting appears in the Library
- [x] #4 Title is editable and persisted
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Recorder: per-source mute (Control::SetMuted) writes silence of the same length (tracks stay aligned); LevelEvent gets 'muted'. Unit test: muted stretch decodes as exact zeros, durations aligned.
2. Commands: start_recording creates the meeting row first (folder = meetings/<db id>; row removed again if capture fails), uses the configured sources (settings 'recording.sources', default: system default mic + output) and returns meeting id, start time and the sources; set_source_muted; rename_meeting; stop_recording adds track rows and finishes the meeting (duration, status done). resolve_sources unit-tested.
3. UI: 'New recording' enabled -> Live screen (not a sidebar page; a 'Recording' indicator replaces it while live). Top bar: editable title (saved on blur/Enter), timer (paused time excluded, tabular), recording pulse (grey while paused), per-source chips (icon, name, mini level meter from audio:level, mute toggle), Pause/Resume, Stop. Transcript placeholder. Stop -> Library.
4. Verify: Rust tests; smoke-live.py on the real app with a temp data dir: start, chips + meters move, mute shows muted + muted stretch is exact silence in the FLAC, title edit persisted in DB, pause stops timer, resume, stop -> meeting in Library with title and duration, DB track rows + files; screenshots light/dark.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
From VRT-7: the #[allow(dead_code)] on mod db (lib.rs) is still needed because segment/speaker/settings queries are unused; recording rows (create_meeting/finish_meeting/add_track) come with this ticket. Remove the allow once the remaining queries have users (VRT-16/VRT-18 at the latest).

Implementation: recorder mute (Control::SetMuted; a muted track writes silence of the same length, LevelEvent.muted). Commands: start_recording creates the meeting row first (folder meetings/<db id>, row and folder removed again if capture fails), records the configured sources (settings 'recording.sources' via resolve_sources, default system default mic + output) and returns {meeting_id, started_at, sources}; current_recording (UI reload); pause/resume; set_source_muted; stop_recording adds track rows and finishes the meeting (duration = longest track, status done); rename_meeting. RecorderState now a tokio Mutex (async commands). db.set_meeting_dir. UI: App holds the session and shows a pulsing 'Recording' sidebar entry while live; Library's New recording starts it; src/routes/Live.tsx (editable title saved on blur/Enter, timer excluding paused time, recording pulse/grey when paused, Pause/Resume, Stop -> Library, transcript placeholder), SourceChip + LevelMeter (meterFraction in lib/format.ts). scripts/smoke-live.py; ui-smoke.py updated to the new API; AGENTS.md lists the smoke scripts.
Verification: 37 Rust tests (new: muted stretch decodes as exact zeros with length kept; resolve_sources configured/defaults); clippy 0; npm run check clean. smoke-live.py on the real app (temp data dir, test tone): start -> Live with 2 chips + Recording entry; mic meter moves with room noise; system meter shows the playing tone (steady 9% for a steady -55 dBFS tone) and drops to 0 when it stops; title persisted to the DB while recording; mute shows on the chip and the mic FLAC contains 2.3 s of exact digital silence; pause freezes the timer and greys the indicator, resume continues; stop -> Library lists 'Smoke test meeting' with its duration; DB: status done, duration 7.7 s for ~8.1 s wall minus pause, folder meetings/<id>, track rows mic+loopback with files, tracks aligned to the sample. First run's 'system meter animates' check was wrong for a steady tone and was replaced by the respond-to-stop check. Light, dark-muted and paused screenshots inspected. ui-smoke.py and smoke-library.py still pass; real data dir untouched.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Recording from the UI works end to end: 'New recording' creates a meeting and records the configured sources (default: system default mic + output) into meetings/<id>/, the Live screen shows an editable title (saved immediately), a timer that excludes pauses, a soft red pulse, per-source chips with live level meters and mute (silence recorded, tracks stay aligned), and Pause/Resume/Stop; stopping stores the tracks and duration and returns to the Library where the meeting appears. Verified with unit tests and a hardware smoke check through the real UI (including exact-silence check for mute), plus light/dark screenshots.
<!-- SECTION:FINAL_SUMMARY:END -->
