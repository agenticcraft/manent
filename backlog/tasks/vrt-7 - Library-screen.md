---
id: VRT-7
title: Library screen
status: Done
assignee:
  - '@claude'
created_date: 2026-10-29 12:17
updated_date: '2026-10-29T22:16:00Z'
labels:
  - frontend
milestone: m-1
dependencies:
  - VRT-3
  - VRT-6
documentation:
  - doc-3
priority: medium
ordinal: 7000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Searchable list of meetings (title, date, duration, speaker avatars) with 'New recording' and 'Import audio' actions, in the clean UI style. See doc-3 UI design.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Meetings listed from DB, newest first
- [x] #2 Title search filters the list
- [x] #3 Empty state with clear call to action
- [x] #4 Works in light and dark mode
- [x] #5 A meeting can be deleted, which removes its audio files and DB rows
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Rust: AppPaths state (data dir = VERITAS_DATA_DIR override or the app data dir; used by DB, recordings and deletes so tests never touch real data).
2. db: meeting_speakers() (distinct named speakers per meeting from segments); commands list_meetings (meetings + speakers, newest first) and delete_meeting (DB rows by cascade + audio folder, only if the folder is inside <data>/meetings).
3. ipc.ts types; Library UI: list rows (title or 'Untitled meeting', localized date, duration h:mm:ss tabular, status when not done, speaker avatars with initials in speaker colors), title search (client-side filter), empty state and no-match state, delete with inline confirm.
4. Tests: Rust unit tests for meeting_speakers, the delete path guard; smoke test with a seeded data dir (order newest first, search filter, delete removes row + folder), light/dark screenshots inspected.
5. Dead-code allow on mod db: keep only for functions still unused (noted for VRT-8/16/18).
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
From VRT-6: lib.rs has a temporary #[allow(dead_code)] on mod db because only tests use the query functions; remove it once the Library uses them.

Implementation: src-tauri/src/paths.rs AppPaths (data dir = VERITAS_DATA_DIR or the app data dir; database(), meetings_root(), deletable_meeting_dir guard: must exist and lie strictly inside meetings/ after canonicalising); db.meeting_speakers(); commands list_meetings (MeetingSummary = meeting + distinct named speakers) and delete_meeting (removes the folder only when inside meetings/, then DB rows by cascade); start_recording now uses AppPaths. UI: src/routes/Library.tsx (load, client-side title search incl. 'Untitled meeting', empty state, no-match state, error line), src/components/MeetingRow.tsx (title, status pill when not done, localized date + duration, up to 4 speaker avatars with initials in speaker colours, delete with inline confirm), src/lib/format.ts, shadcn Input. scripts/wdlib.py (shared WebDriver client) + scripts/smoke-library.py. AGENTS.md: smoke tests must run with VERITAS_DATA_DIR. Dead-code allow on mod db still needed (noted in VRT-8).
Found and fixed during verification: avatar overlap clipped the initials ('AD' -> 'A'); a light-mode screenshot taken right after switching theme caught colour transitions mid-way (script now waits 0.6 s). Also removed a leftover test recording from VRT-6 verification in the real app data folder (before the data-dir override existed).
Verification: 34 Rust tests (new: meeting_speakers, delete-path guard incl. root, '..' escape, missing dir, '/'); clippy 0, fmt, npm run check clean. smoke-library.py on the real app with a seeded temp data dir: empty state; newest-first order; durations 1:02:05 / 30:30 / 0:42; 'Processing' pill; avatars AD/BH; search 'weekly' -> 1 row; no-match message; delete asks for confirmation, removes the row, the audio folder and the DB rows (segments by cascade), leaves other folders; a meeting whose folder is outside meetings/ is deleted from the DB but its folder is kept. Light/dark/confirm screenshots inspected. ui-smoke.py still passes.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Library screen done: meetings from the database, newest first, with title, localized date, duration, status and speaker avatars; live title search with a no-match state; calm empty state with the record/import actions; delete with an inline confirmation that removes the audio folder and the database rows. Deletion is guarded so only folders inside <data>/meetings can ever be removed, and a VERITAS_DATA_DIR override keeps tests away from real data. Verified with unit tests and a seeded end-to-end smoke check on the real app, screenshots in light and dark.
<!-- SECTION:FINAL_SUMMARY:END -->
