-- A recording cut off when the app died, recovered at the next start (VRT-41).
ALTER TABLE meetings ADD COLUMN interrupted INTEGER NOT NULL DEFAULT 0;
