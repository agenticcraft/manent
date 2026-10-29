-- Veritas schema v1 (doc-2 §1.4, §1.5). Times are milliseconds: absolute times since the
-- Unix epoch, positions within a meeting since its start.

CREATE TABLE meetings (
    id          INTEGER PRIMARY KEY,
    title       TEXT    NOT NULL DEFAULT '',
    started_at  INTEGER NOT NULL,
    duration_ms INTEGER NOT NULL DEFAULT 0,
    status      TEXT    NOT NULL DEFAULT 'recording'
                CHECK (status IN ('recording', 'processing', 'done', 'failed')),
    dir         TEXT    NOT NULL
);
CREATE INDEX meetings_started_at ON meetings (started_at DESC);

-- One recorded file per source (mic.flac, system.flac, ...).
CREATE TABLE tracks (
    id          INTEGER PRIMARY KEY,
    meeting_id  INTEGER NOT NULL REFERENCES meetings (id) ON DELETE CASCADE,
    source_kind TEXT    NOT NULL CHECK (source_kind IN ('mic', 'loopback')),
    source_id   TEXT    NOT NULL,
    device_name TEXT    NOT NULL,
    path        TEXT    NOT NULL,
    seconds     REAL    NOT NULL DEFAULT 0
);
CREATE INDEX tracks_meeting ON tracks (meeting_id);

-- People known by voice. Voice data is biometric: deleting a speaker deletes the samples.
CREATE TABLE speakers (
    id    INTEGER PRIMARY KEY,
    name  TEXT    NOT NULL,
    color INTEGER NOT NULL DEFAULT 1
);

CREATE TABLE voice_samples (
    id            INTEGER PRIMARY KEY,
    speaker_id    INTEGER NOT NULL REFERENCES speakers (id) ON DELETE CASCADE,
    embedding     BLOB    NOT NULL, -- little-endian f32 vector
    encoder_id    TEXT    NOT NULL, -- sha256 identity of the speaker encoder
    clean_seconds REAL    NOT NULL,
    meeting_id    INTEGER REFERENCES meetings (id) ON DELETE SET NULL,
    created_at    INTEGER NOT NULL
);
CREATE INDEX voice_samples_speaker ON voice_samples (speaker_id);

-- Transcript segments from the three passes (doc-2 §1.3): live captions, rolling pass, final pass.
CREATE TABLE segments (
    id            INTEGER PRIMARY KEY,
    meeting_id    INTEGER NOT NULL REFERENCES meetings (id) ON DELETE CASCADE,
    track_id      INTEGER REFERENCES tracks (id) ON DELETE SET NULL,
    pass          TEXT    NOT NULL CHECK (pass IN ('caption', 'rolling', 'final')),
    speaker_id    INTEGER REFERENCES speakers (id) ON DELETE SET NULL,
    speaker_label TEXT,   -- diarization label within the meeting ("0", "1", ...)
    start_ms      INTEGER NOT NULL,
    end_ms        INTEGER NOT NULL,
    text          TEXT    NOT NULL,
    words_json    TEXT    -- [{"start_ms","end_ms","text"}]
);
CREATE INDEX segments_meeting_time ON segments (meeting_id, start_ms);

-- Full-text search over transcripts; remove_diacritics so 'een' also finds 'één'.
CREATE VIRTUAL TABLE segments_fts USING fts5 (
    text,
    content = 'segments',
    content_rowid = 'id',
    tokenize = 'unicode61 remove_diacritics 2'
);
CREATE TRIGGER segments_ai AFTER INSERT ON segments BEGIN
    INSERT INTO segments_fts (rowid, text) VALUES (new.id, new.text);
END;
CREATE TRIGGER segments_ad AFTER DELETE ON segments BEGIN
    INSERT INTO segments_fts (segments_fts, rowid, text) VALUES ('delete', old.id, old.text);
END;
CREATE TRIGGER segments_au AFTER UPDATE OF text ON segments BEGIN
    INSERT INTO segments_fts (segments_fts, rowid, text) VALUES ('delete', old.id, old.text);
    INSERT INTO segments_fts (rowid, text) VALUES (new.id, new.text);
END;

-- App settings as JSON values.
CREATE TABLE settings (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
