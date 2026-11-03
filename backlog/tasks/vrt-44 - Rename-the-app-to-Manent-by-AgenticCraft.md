---
id: VRT-44
title: Rename the app to Manent (by AgenticCraft)
status: Done
assignee:
  - '@claude'
created_date: '2026-11-03T19:24:00Z'
updated_date: '2026-11-03T19:49:00Z'
labels:
  - rename
  - release
milestone: m-3
dependencies: []
priority: high
ordinal: 48000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
decision-8. Rename Veritas to Manent everywhere a user or a system sees it, with a migration for data that depends on the name:
- **Identity:** productName and window title 'Manent', identifier ai.agenticcraft.manent, crate and binary names (manent, manent_lib, manent-engine), package.json, index.html, the tray texts, the UI strings, the thread names.
- **Environment variables:** MANENT_DATA_DIR, MANENT_NO_BROWSER and MANENT_TEST_ENGINE_DATA. The old VERITAS_* names are still read, with a warning, until the scripts are moved over.
- **Data folder:** on the first start, if the new app data folder is empty and the old nl.veritas.app folder exists, move it (a rename on the same disk, otherwise a copy and then a delete). The database moves along and is renamed manent.db. Meeting and track paths that point into the old folder are rewritten, and the meetings-folder setting too.
- **Keychain:** the Nextcloud app password moves from service 'veritas-nextcloud' to 'manent-nextcloud'.
- **Sync:** the default server folder becomes 'Manent' for new connections; an existing connection keeps 'Veritas' (stored as its folder setting).
- **transcript.md:** written with 'manent: 1'; the parser also accepts 'veritas: 1'; the backfill rewrites old files.
- **Docs:** AGENTS.md, doc-1 to doc-3, the scripts and the README. The VRT ids, the repository name and the spikes stay as they are.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 The app, tray, window and installer say Manent; identifier ai.agenticcraft.manent
- [x] #2 An existing Veritas data folder (database, meetings, settings, voices) is migrated on first start and everything still plays, searches and syncs; unit test for the path rewrite
- [x] #3 The Nextcloud connection survives the rename (keychain entry moved, server folder kept)
- [x] #4 transcript.md says manent: 1 and old files still parse; all smoke scripts updated and passing
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Done as decided in decision-8.

**Name**
- productName and window title Manent; identifier ai.agenticcraft.manent.
- Crates manent / manent_lib / manent-engine (the helper binary), package.json and its lock, the tray texts, the UI, the thread names, the scripts, AGENTS.md, doc-1 to doc-3, and the Backlog project_name.
- Kept as they are: the VRT ids, the repository name, the spikes, the migrations (sqlx checksums) and decisions and tasks (history).

**Migration (src-tauri/src/legacy.rs)**
- Before the database opens, the nl.veritas.app folder is merged into the new data folder:
  - it counts as new while it has no database and no meetings, because WebKit puts its caches there before the app's setup runs;
  - entries already there stay; across disks it copies, then deletes.
- veritas.db, with its WAL files, becomes manent.db.
- After the database opens:
  - Db::rewrite_path_prefix rewrites meeting folders, track paths and the chosen meetings folder;
  - a sync connection without a folder setting keeps 'Veritas'.
- Other fallbacks:
  - the keychain falls back to the 'veritas-nextcloud' entry and moves it;
  - transcript.md with 'veritas: 1' still parses, and the backfill rewrites it;
  - VERITAS_* environment variables are still read with a warning (DATA_DIR, NO_BROWSER).
- Unit tests: the move with webview caches present, a second start, path rewriting, the sync folder, an old transcript header.

**Tests**
- scripts/smoke-rename.py PASS. An install as Veritas left it, in a temp XDG_DATA_HOME (12 meetings, connected to the test Nextcloud with the app password under the old keychain service), is carried over on the first start:
  - manent.db, and 25 paths rewritten;
  - a meeting plays and search works;
  - 6 transcript.md files rewritten;
  - sync is idle with the folder 'Veritas';
  - the keychain entry moved.
  The real ~/.local/share/nl.veritas.app was not touched; it migrates when the user starts the new build.
- Also PASS on the renamed build: ui-smoke, smoke-tray (menu 'Show Manent'), smoke-transcript-md (header manent: 1; this test data folder still had veritas.db and was renamed on the way), smoke-sync (default folder Manent/, offline message as a sentence).
- 128 Rust tests, clippy 0, npm check green.

**Still to do before a release:** the trademark check (TMview: MANENT, classes 9 and 42), per decision-8.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
The app is Manent by AgenticCraft (ai.agenticcraft.manent). An existing Veritas install moves over by itself on the first start: data folder, database, paths, transcripts, and the Nextcloud connection with its keychain entry.
<!-- SECTION:FINAL_SUMMARY:END -->
