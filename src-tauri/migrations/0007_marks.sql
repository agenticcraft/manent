-- Moments the user marked while recording (VRT-52). at_ms is the position in the recording
-- (track time); label is optional (empty).
CREATE TABLE marks (
    id         INTEGER PRIMARY KEY,
    meeting_id INTEGER NOT NULL REFERENCES meetings (id) ON DELETE CASCADE,
    at_ms      INTEGER NOT NULL,
    label      TEXT NOT NULL DEFAULT '',
    created_at INTEGER NOT NULL
);
CREATE INDEX marks_meeting ON marks (meeting_id, at_ms);
