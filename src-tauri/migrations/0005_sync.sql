-- Nextcloud sync (VRT-39, decision-7).
-- A meeting that has a folder on the server: where, where it came from, and what of it was
-- uploaded last (so only changes go up again).
CREATE TABLE sync_meetings (
    meeting_id      INTEGER PRIMARY KEY REFERENCES meetings (id) ON DELETE CASCADE,
    remote_dir      TEXT    NOT NULL,          -- e.g. "Veritas/2026-11-02 14.05 Planning"
    origin          TEXT    NOT NULL CHECK (origin IN ('phone', 'desktop')),
    transcript_hash TEXT,                      -- sha256 of the transcript.md last uploaded
    mix_uploaded    INTEGER NOT NULL DEFAULT 0,
    gone            INTEGER NOT NULL DEFAULT 0 -- the folder was deleted on the server: leave it
);
-- A recording found in the inbox, and what became of it.
CREATE TABLE sync_inbox (
    remote_path TEXT    PRIMARY KEY,           -- e.g. "Veritas/Inbox/rec-0912.opus"
    etag        TEXT    NOT NULL,
    meeting_id  INTEGER REFERENCES meetings (id) ON DELETE SET NULL,
    state       TEXT    NOT NULL CHECK (state IN ('importing', 'done', 'failed')),
    error       TEXT
);
