---
id: VRT-6
title: SQLite storage with migrations
status: Done
assignee:
  - '@claude'
created_date: 2026-10-29 12:17
updated_date: '2026-10-29T22:00:00Z'
labels:
  - backend
  - storage
milestone: m-1
dependencies:
  - VRT-3
documentation:
  - doc-2
priority: high
ordinal: 6000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Add sqlx + SQLite in the OS app-data folder with migrations for meetings, tracks, segments, speakers and settings, plus an FTS5 index over segment text. See doc-2 Technical architecture §1.5.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Migrations run on app start
- [x] #2 FTS5 virtual table for segment text
- [x] #3 Query functions covered by unit tests
- [x] #4 Tables meetings, tracks, segments, speakers, settings exist as in doc-2 §1.5
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Deps: sqlx 0.8 (runtime-tokio, sqlite bundled, migrate, macros for migrate!), tokio (dev, tests).
2. migrations/0001_init.sql: meetings, tracks, segments (pass caption|rolling|final instead of is_final, per the three-pass design), speakers, voice_samples (doc-2 §1.4), settings; foreign keys with cascades; segments_fts (FTS5, unicode61 remove_diacritics 2 for Dutch) kept in sync by triggers.
3. db.rs: Db::open(path) (WAL, foreign_keys, create if missing) + migrate; query functions: meetings (create, finish, list, get, delete), tracks (add, list), segments (insert, replace a pass in a time range, list), search (FTS, snippet), speakers + voice samples (create, list, rename, merge, delete), settings (get/set JSON).
4. Open the DB on app start (app data dir), run migrations, manage as Tauri state.
5. Tests: in-memory DB per test for every query function, cascade deletes, FTS search incl. Dutch diacritics and updates/deletes.
6. Verify: cargo test/clippy/fmt; start the real app and inspect the created DB file with sqlite3 (tables, migration row, FTS works); update doc-2 §1.5 to the actual schema.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Implementation: src-tauri/migrations/0001_init.sql (meetings, tracks, segments with pass caption|rolling|final instead of is_final, speakers, voice_samples, settings, segments_fts FTS5 unicode61 remove_diacritics 2 + sync triggers, cascades); src-tauri/src/db.rs Db (open with WAL + foreign keys + create_if_missing, open_in_memory for tests, sqlx::migrate! embedded) and query functions for meetings, tracks, segments (insert, replace_segments by pass and time window, set speaker), search (quoted prefix terms, snippet with [ ]), speakers (create/list/rename/delete/merge), voice samples (LE f32 blobs, per encoder), settings (JSON). lib.rs setup opens <app data>/veritas.db before any window and manages Db as state. Shared test helper moved to src/test_util.rs. Temporary #[allow(dead_code)] on mod db until VRT-7 (noted there). doc-2 §1.5 updated to the actual schema.
Verification: 11 db unit tests (schema tables, meeting CRUD + ordering, status CHECK, cascades incl. FTS rows, rolling pass replacing captions only in its window, Dutch/English search with diacritics/case/prefix/AND and quote-safety, search after replace, speakers/samples/merge/delete incl. biometric cascade, settings JSON, file DB persists and migrates once); 32 tests total, clippy 0 warnings. Real app: launching it created <app data>/veritas.db (WAL) with _sqlx_migrations '1|init|1' and all tables; FTS on the real file found 'ideeën' for 'ideeen'. Note: the sqlite3 CLI has foreign keys off by default, so a CLI delete doesn't cascade (the app enables them per connection); the probe row was cleaned up.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Storage in place: sqlx + bundled SQLite at <app data>/veritas.db, migrated on app start, with the doc-2 schema (meetings, tracks, three-pass segments, speakers + voice samples, settings) and a diacritics-insensitive FTS5 index kept in sync by triggers. Db exposes tested query functions for every table, including atomic pass replacement for the rolling/final transcript passes, speaker merge and biometric-safe deletes, and Dutch/English search. Verified with 11 unit tests and by inspecting the database the real app created. doc-2 §1.5 updated.
<!-- SECTION:FINAL_SUMMARY:END -->
