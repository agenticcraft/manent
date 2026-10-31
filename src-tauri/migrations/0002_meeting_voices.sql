-- A meeting's speakers as the transcription passes label them (VRT-18, doc-2 §1.4): the
-- cleanest voice embedding per label and who that is (named, auto-named or suggested).
-- Labels are "1", "2", ... per meeting; the mic track ("me") is not in here.
CREATE TABLE meeting_voices (
    meeting_id    INTEGER NOT NULL REFERENCES meetings (id) ON DELETE CASCADE,
    label         TEXT    NOT NULL,
    speaker_id    INTEGER REFERENCES speakers (id) ON DELETE SET NULL,
    suggested_id  INTEGER REFERENCES speakers (id) ON DELETE SET NULL,
    distance      REAL,
    embedding     BLOB,    -- little-endian f32 vector
    encoder_id    TEXT,
    clean_seconds REAL    NOT NULL DEFAULT 0,
    confirmed     INTEGER NOT NULL DEFAULT 0, -- 1 when the user named or confirmed it
    PRIMARY KEY (meeting_id, label)
);
