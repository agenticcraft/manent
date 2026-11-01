-- Audio missing from a recording because capture froze while the computer slept (VRT-36).
-- at_ms is the position in the recording (track time) where the audio stops.
CREATE TABLE meeting_gaps (
    meeting_id INTEGER NOT NULL REFERENCES meetings (id) ON DELETE CASCADE,
    at_ms      INTEGER NOT NULL,
    missing_ms INTEGER NOT NULL
);
CREATE INDEX meeting_gaps_meeting ON meeting_gaps (meeting_id, at_ms);
