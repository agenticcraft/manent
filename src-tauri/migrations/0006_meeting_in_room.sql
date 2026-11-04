-- The meeting was in a room: the mic hears several people and is diarized too (VRT-47).
ALTER TABLE meetings ADD COLUMN in_room INTEGER NOT NULL DEFAULT 0;
